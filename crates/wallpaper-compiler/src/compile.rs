use super::*;
use css::{Declaration, Property, Rule, Selector};
use std::collections::BTreeMap;
use wallpaper_format::Package;
use wallpaper_media::{RgbaImage, decode_png, validate_assets};
use wallpaper_scene::hierarchy::{
    self, Hierarchy, Layer, LayerContent, Transform, ValidatedHierarchy,
};
use wallpaper_scene::{Color, Content, Node, Rect, SCENE_VERSION, Scene, ValidatedScene};

#[derive(Clone, Copy)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub background: Color,
}

/// Static output plus retained-source references. These IDs are NOT live host authority.
#[derive(Debug)]
pub struct Compiled {
    scene: ValidatedScene,
    symbols: BTreeMap<String, Vec<u32>>,
    script: Option<RelativePath>,
    images: BTreeMap<u32, RgbaImage>,
}
impl Compiled {
    pub fn scene(&self) -> &ValidatedScene {
        &self.scene
    }
    pub fn into_scene(self) -> ValidatedScene {
        self.scene
    }
    pub fn symbols(&self) -> &BTreeMap<String, Vec<u32>> {
        &self.symbols
    }
    /// Candidate flat-scene mutation targets. The session owner grants authority.
    pub fn script_targets(&self) -> BTreeMap<String, u32> {
        let nodes: BTreeMap<_, _> = self
            .scene
            .scene()
            .nodes
            .iter()
            .map(|node| (node.id, &node.content))
            .collect();
        self.symbols
            .iter()
            .filter_map(|(name, ids)| {
                let mut text = ids
                    .iter()
                    .filter(|id| matches!(nodes.get(*id), Some(Content::Text { .. })));
                let target = match (text.next(), text.next(), ids.as_slice()) {
                    (Some(id), None, _) => id,
                    (None, None, [id])
                        if matches!(nodes.get(id), Some(Content::Rectangle { .. })) =>
                    {
                        id
                    }
                    _ => return None,
                };
                Some((name.clone(), *target))
            })
            .collect()
    }
    pub fn images(&self) -> &BTreeMap<u32, RgbaImage> {
        &self.images
    }
    /// Deferred source association only; no script is interpreted by this compiler.
    pub fn script(&self) -> Option<&RelativePath> {
        self.script.as_ref()
    }
}

/// Explicit opt-in v2 output. Author IDs refer to groups, never generated draw IDs.
#[derive(Debug)]
pub struct CompiledGroups {
    scene: ValidatedHierarchy,
    symbols: BTreeMap<String, u32>,
    script: Option<RelativePath>,
    images: BTreeMap<u32, RgbaImage>,
}
impl CompiledGroups {
    pub fn scene(&self) -> &ValidatedHierarchy {
        &self.scene
    }
    pub fn symbols(&self) -> &BTreeMap<String, u32> {
        &self.symbols
    }
    pub fn script(&self) -> Option<&RelativePath> {
        self.script.as_ref()
    }
    pub fn images(&self) -> &BTreeMap<u32, RgbaImage> {
        &self.images
    }
}
enum Compilation {
    Flat(Compiled),
    Groups(CompiledGroups),
}

#[derive(Clone, Copy)]
struct Style {
    x: f32,
    y: f32,
    width: Option<f32>,
    height: Option<f32>,
    foreground: Color,
    background: Color,
    font_size: f32,
    rank: [u8; 12],
    transform: Option<Transform>,
    origin_top_left: bool,
    opacity: f32,
    clip: bool,
}
impl Style {
    fn new(parent: Option<Self>) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: None,
            height: None,
            foreground: parent.map_or(Color([255; 4]), |style| style.foreground),
            background: Color([0; 4]),
            font_size: parent.map_or(16.0, |style| style.font_size),
            rank: [0; 12],
            transform: None,
            origin_top_left: false,
            opacity: 1.0,
            clip: false,
        }
    }
    fn apply(&mut self, declaration: &Declaration, specificity: u8) {
        let index = match declaration.property {
            Property::Left(_) => 0,
            Property::Top(_) => 1,
            Property::Width(_) => 2,
            Property::Height(_) => 3,
            Property::Color(_) => 4,
            Property::BackgroundColor(_) => 5,
            Property::FontSize(_) => 6,
            Property::PositionAbsolute => 7,
            Property::Opacity(_) => 8,
            Property::Transform(_) => 9,
            Property::TransformOriginTopLeft => 10,
            Property::OverflowClip(_) => 11,
        };
        if specificity < self.rank[index] {
            return;
        }
        self.rank[index] = specificity;
        match declaration.property {
            Property::Left(value) => self.x = value,
            Property::Top(value) => self.y = value,
            Property::Width(value) => self.width = Some(value),
            Property::Height(value) => self.height = Some(value),
            Property::Color(value) => self.foreground = value,
            Property::BackgroundColor(value) => self.background = value,
            Property::FontSize(value) => self.font_size = value,
            Property::PositionAbsolute => {}
            Property::Opacity(value) => self.opacity = value,
            Property::Transform(value) => self.transform = value,
            Property::TransformOriginTopLeft => self.origin_top_left = true,
            Property::OverflowClip(value) => self.clip = value,
        }
    }
}

pub fn compile(package: &Package, viewport: Viewport) -> Result<Compiled> {
    match compile_internal(package, viewport, false)? {
        Compilation::Flat(value) => Ok(value),
        _ => unreachable!(),
    }
}
/// Explicit v2 compilation; the default flat compilation path remains separate.
pub fn compile_groups(package: &Package, viewport: Viewport) -> Result<CompiledGroups> {
    match compile_internal(package, viewport, true)? {
        Compilation::Groups(value) => Ok(value),
        _ => unreachable!(),
    }
}
/// Explicit typed author-ID binding; CSS animation syntax remains unsupported.
pub fn compile_animated(
    package: &Package,
    viewport: Viewport,
    spec: animation::Spec,
) -> animation::Result<animation::CompiledAnimation> {
    let compiled = compile_groups(package, viewport).map_err(animation::Error::Source)?;
    animation::bind(
        compiled.scene,
        compiled.symbols,
        compiled.script,
        compiled.images,
        spec,
    )
}
fn compile_internal(package: &Package, viewport: Viewport, groups: bool) -> Result<Compilation> {
    let entry = package.manifest().entry();
    let text = package.require_text(entry, ".html", "entry").map_err(|_| {
        Source {
            path: entry,
            text: "",
        }
        .error(ErrorCode::MissingResource, Span { start: 0, end: 0 })
    })?;
    let source = Source { path: entry, text };
    let mut scene = Scene {
        version: SCENE_VERSION,
        width: viewport.width,
        height: viewport.height,
        background: viewport.background,
        resources: vec![],
        nodes: vec![],
    };
    scene.clone().validate().map_err(|error| {
        source.error(
            ErrorCode::SceneValidation(error.code),
            Span { start: 0, end: 0 },
        )
    })?;
    let document = html::parse(source)?;
    let mut rules: Vec<Rule> = vec![];
    let mut total_css_bytes = 0usize;
    let mut declarations = 0usize;
    for reference in &document.stylesheets {
        let file = package
            .file(&reference.path)
            .ok_or_else(|| source.error(ErrorCode::MissingResource, reference.span))?;
        let text = file
            .utf8()
            .map_err(|_| source.error(ErrorCode::InvalidReference, reference.span))?;
        total_css_bytes += text.len();
        if total_css_bytes > MAX_SOURCE_BYTES {
            return Err(source.error(ErrorCode::BudgetExceeded, reference.span));
        }
        let css_source = Source {
            path: &reference.path,
            text,
        };
        let sheet = if groups {
            css::parse_groups(css_source)
        } else {
            css::parse(css_source)
        }?;
        declarations += sheet.declaration_count;
        if rules.len() + sheet.rules.len() > MAX_RULES || declarations > MAX_DECLARATIONS {
            return Err(source.error(ErrorCode::BudgetExceeded, reference.span));
        }
        rules.extend(sheet.rules);
    }
    let mut styles: Vec<Style> = Vec::with_capacity(document.elements.len());
    let mut origins = vec![];
    let mut symbols = BTreeMap::new();
    let mut images = BTreeMap::new();
    let mut image_paths = BTreeMap::new();
    let mut remaining_image_bytes = wallpaper_scene::MAX_DECODED_IMAGE_BYTES;
    let mut layers = vec![];
    let mut group_ids = vec![];
    let mut group_symbols = BTreeMap::new();
    for (index, element) in document.elements.iter().enumerate() {
        let parent = element.parent.map(|parent| styles[parent]);
        let mut style = Style::new(parent);
        for rule in &rules {
            let matched = match &rule.selector {
                Selector::Id(id) => element.id.as_ref() == Some(id),
                Selector::Class(class) => element.classes.contains(class),
            };
            if matched {
                for declaration in &rule.declarations {
                    style.apply(declaration, rule.selector.specificity());
                }
            }
        }
        if let Some((_, span)) = &element.inline_style {
            let inline = if groups {
                css::declarations_groups(source, *span)
            } else {
                css::declarations(source, *span)
            }?;
            declarations += inline.len();
            if declarations > MAX_DECLARATIONS {
                return Err(source.error(ErrorCode::BudgetExceeded, *span));
            }
            for declaration in &inline {
                style.apply(declaration, 3);
            }
        }
        if !groups {
            style.x += parent.map_or(0.0, |style| style.x);
            style.y += parent.map_or(0.0, |style| style.y);
        }
        let (Some(width), Some(height)) = (style.width, style.height) else {
            return Err(source.error(ErrorCode::MissingDimensions, element.span));
        };
        let bounds = Rect {
            x: style.x,
            y: style.y,
            width,
            height,
        };
        let first = scene.nodes.len();
        let group_id = if groups {
            if style.transform.is_some() && !style.origin_top_left {
                return Err(source.error(ErrorCode::MissingTransformOrigin, element.span));
            }
            if layers.len() >= wallpaper_scene::MAX_NODES {
                return Err(source.error(ErrorCode::BudgetExceeded, element.span));
            }
            let id = layers.len() as u32 + 1;
            layers.push(Layer {
                id,
                parent: element.parent.map_or(0, |i| group_ids[i]),
                bounds,
                content: LayerContent::Group {
                    transform: style.transform.unwrap_or_default(),
                    opacity: style.opacity,
                    clip: style.clip,
                },
            });
            origins.push(index);
            group_ids.push(id);
            id
        } else {
            0
        };
        let mut emit = |content| -> Result<()> {
            if (if groups {
                layers.len()
            } else {
                scene.nodes.len()
            }) >= wallpaper_scene::MAX_NODES
            {
                return Err(source.error(ErrorCode::BudgetExceeded, element.span));
            }
            if groups {
                layers.push(Layer {
                    id: layers.len() as u32 + 1,
                    parent: group_id,
                    bounds: Rect {
                        x: 0.0,
                        y: 0.0,
                        width,
                        height,
                    },
                    content: LayerContent::Draw(content),
                });
            } else {
                scene.nodes.push(Node {
                    id: scene.nodes.len() as u32 + 1,
                    bounds,
                    content,
                });
            }
            origins.push(index);
            Ok(())
        };
        match element.kind {
            html::Kind::Div => emit(Content::Rectangle {
                color: style.background,
            })?,
            html::Kind::Span => {
                if style.background.0[3] != 0 {
                    emit(Content::Rectangle {
                        color: style.background,
                    })?;
                }
                emit(Content::Text {
                    text: element.text.clone(),
                    font_size: style.font_size,
                    color: style.foreground,
                })?;
            }
            html::Kind::Image => {
                let path = element
                    .image
                    .as_ref()
                    .ok_or_else(|| source.error(ErrorCode::InvalidReference, element.span))?;
                let file = package
                    .file(path)
                    .ok_or_else(|| source.error(ErrorCode::MissingResource, element.span))?;
                if !path.as_str().ends_with(".png") {
                    return Err(source.error(ErrorCode::InvalidReference, element.span));
                }
                let resource = if let Some(&id) = image_paths.get(path.as_str()) {
                    id
                } else {
                    if images.len() >= wallpaper_scene::MAX_RESOURCES {
                        return Err(source.error(ErrorCode::BudgetExceeded, element.span));
                    }
                    let image = decode_png(file.bytes(), remaining_image_bytes)
                        .map_err(|error| source.error(ErrorCode::Image(error), element.span))?;
                    remaining_image_bytes -= image.pixels().len() as u64;
                    let id = images.len() as u32 + 1;
                    scene.resources.push(wallpaper_scene::ImageResource {
                        id,
                        width: image.width(),
                        height: image.height(),
                    });
                    images.insert(id, image);
                    image_paths.insert(path.as_str().to_owned(), id);
                    id
                };
                if style.background.0[3] != 0 {
                    emit(Content::Rectangle {
                        color: style.background,
                    })?;
                }
                emit(Content::Image { resource })?;
            }
        }
        if let Some(id) = &element.id {
            if groups {
                group_symbols.insert(id.clone(), group_id);
            } else {
                symbols.insert(
                    id.clone(),
                    scene.nodes[first..].iter().map(|node| node.id).collect(),
                );
            }
        }
        styles.push(style);
    }
    let scene_error = |error: wallpaper_scene::SceneError| {
        let span = match error.context {
            wallpaper_scene::Context::Node(index) => origins
                .get(index)
                .map(|&index| document.elements[index].span),
            _ => None,
        }
        .unwrap_or(Span { start: 0, end: 0 });
        source.error(ErrorCode::SceneValidation(error.code), span)
    };
    if groups {
        let scene = Hierarchy {
            version: hierarchy::VERSION,
            width: scene.width,
            height: scene.height,
            background: scene.background,
            resources: scene.resources,
            layers,
        }
        .validate()
        .map_err(scene_error)?;
        wallpaper_media::validate_hierarchy_assets(&scene, &images)
            .map_err(|error| source.error(ErrorCode::Image(error), Span { start: 0, end: 0 }))?;
        return Ok(Compilation::Groups(CompiledGroups {
            scene,
            images,
            symbols: group_symbols,
            script: package.manifest().script().cloned(),
        }));
    }
    let scene = scene.validate().map_err(scene_error)?;
    validate_assets(&scene, &images)
        .map_err(|error| source.error(ErrorCode::Image(error), Span { start: 0, end: 0 }))?;
    Ok(Compilation::Flat(Compiled {
        scene,
        images,
        symbols,
        script: package.manifest().script().cloned(),
    }))
}
