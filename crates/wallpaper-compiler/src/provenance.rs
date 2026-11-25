use crate::{Viewport, animation::Spec, compile, compile_animated, compile_groups};
use std::{
    collections::BTreeMap,
    io::{self, Write},
};
use wallpaper_format::{Package, RelativePath};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub enum Mode {
    Flat,
    Groups,
    Animated(Spec),
}

/// Compile the retained package and export the result from that same source snapshot.
pub fn write_compiled(
    output: impl Write,
    package: &Package,
    viewport: Viewport,
    mode: Mode,
) -> Result<()> {
    export(output, package, viewport, mode, false)
}

fn export(
    mut output: impl Write,
    package: &Package,
    viewport: Viewport,
    mode: Mode,
    legacy: bool,
) -> Result<()> {
    match mode {
        Mode::Flat => {
            let compiled = compile(package, viewport)?;
            let payload = compiled.scene().encode();
            let decoded = wallpaper_scene::decode(&payload)?;
            wallpaper_media::validate_assets(&decoded, compiled.images())?;
            write_record(
                &mut output,
                package,
                Some(1),
                &payload,
                compiled.symbols(),
                compiled.script(),
                compiled.images(),
            )?;
        }
        Mode::Groups => {
            let compiled = compile_groups(package, viewport)?;
            let payload = compiled.scene().encode();
            let decoded = wallpaper_scene::hierarchy::decode(&payload)?;
            wallpaper_media::validate_hierarchy_assets(&decoded, compiled.images())?;
            write_record(
                &mut output,
                package,
                (!legacy).then_some(2),
                &payload,
                compiled.symbols(),
                compiled.script(),
                compiled.images(),
            )?;
        }
        Mode::Animated(spec) => {
            let compiled = compile_animated(package, viewport, spec)?;
            let payload = compiled.animation().encode();
            let decoded = wallpaper_scene::animation::decode(&payload)?;
            wallpaper_media::validate_hierarchy_assets(decoded.base(), compiled.images())?;
            write_record(
                &mut output,
                package,
                Some(3),
                &payload,
                compiled.symbols(),
                compiled.script(),
                compiled.images(),
            )?;
        }
    }
    output.flush()?;
    Ok(())
}

#[cfg(feature = "fixtures")]
pub fn write_fixture(output: impl Write, name: Option<&str>) -> Result<()> {
    use crate::fixtures;
    let (package, mode) = match name {
        None | Some("groups") => (fixtures::groups_package()?, Mode::Groups),
        Some("static") => (fixtures::static_package()?, Mode::Flat),
        Some("image") => (fixtures::image_package()?, Mode::Flat),
        Some("animated-shapes") => (
            fixtures::animated_shapes_package()?,
            Mode::Animated(fixtures::animated_shapes_spec()),
        ),
        _ => return Err(io::Error::other("Unknown compiled provenance fixture").into()),
    };
    export(
        output,
        &package,
        Viewport {
            width: 1920,
            height: 1080,
            background: wallpaper_scene::Color([12, 17, 22, 255]),
        },
        mode,
        name.is_none(),
    )
}

trait SymbolIds {
    fn ids(&self) -> &[u32];
}
impl SymbolIds for u32 {
    fn ids(&self) -> &[u32] {
        std::slice::from_ref(self)
    }
}
impl SymbolIds for Vec<u32> {
    fn ids(&self) -> &[u32] {
        self
    }
}

fn write_record<V: SymbolIds>(
    mut output: impl Write,
    package: &Package,
    kind: Option<u8>,
    payload: &[u8],
    symbols: &BTreeMap<String, V>,
    script: Option<&RelativePath>,
    images: &BTreeMap<u32, wallpaper_media::RgbaImage>,
) -> io::Result<()> {
    output.write_all(b"SAHNPRV\0")?;
    match kind {
        Some(kind) => output.write_all(&[2, 0, kind])?,
        None => output.write_all(&[1, 0])?,
    }
    field(&mut output, env!("CARGO_PKG_VERSION").as_bytes())?;
    count(&mut output, package.files().len())?;
    for file in package.files() {
        field(&mut output, file.path().as_str().as_bytes())?;
        field(&mut output, file.bytes())?;
    }
    field(&mut output, payload)?;
    count(&mut output, symbols.len())?;
    for (name, ids) in symbols {
        field(&mut output, name.as_bytes())?;
        let ids = ids.ids();
        if kind.is_some() {
            count(&mut output, ids.len())?;
        }
        for id in ids {
            output.write_all(&id.to_le_bytes())?;
        }
    }
    field(
        &mut output,
        script.map_or(b"".as_slice(), |path| path.as_str().as_bytes()),
    )?;
    count(&mut output, images.len())?;
    for (id, image) in images {
        output.write_all(&id.to_le_bytes())?;
        output.write_all(&image.width().to_le_bytes())?;
        output.write_all(&image.height().to_le_bytes())?;
        field(&mut output, image.pixels())?;
    }
    Ok(())
}

fn count(output: &mut impl Write, value: usize) -> io::Result<()> {
    let value = u32::try_from(value).map_err(io::Error::other)?;
    output.write_all(&value.to_le_bytes())
}

fn field(output: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    count(output, bytes.len())?;
    output.write_all(bytes)
}
