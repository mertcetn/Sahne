# Prototype Content Contract

Status: the [M3 fragment/CSS compiler](m3-compiler.md), [bounded PNG resources and original image-window reference](m3-images.md) and [static window preview](m3-preview.md) implement a deliberately narrow subset with no arbitrary input. Their exact grammar, values, defaults and text-box rules override draft ambiguities below for those implementations. The [scene foundation](m3-scene.md) validates resolved output; full G3, hierarchical effects and animation remain pending.

The parser accepts UTF-8 fragments with div/span boxes, leading local stylesheet links and img references to the bounded RGBA8 PNG subset. html/head/body wrappers, DOCTYPE, style/script elements and unknown constructs are rejected rather than passed through HTML error recovery. IDs must be unique. No inline event handlers, external URLs, script imports or executable attributes are accepted. The exact implemented rules are in the compiler document linked above.

Initial layout is absolute with fixed px dimensions. span is a text box, not browser mixed inline flow. Use a bundled font; no system-font equivalence is promised. Preserve literal text initially; define newline, line-height, and overflow behavior in G3 visual fixtures before accepting text packages.

Initial selectors are ID and class. Cascade ordering is specificity followed by source order; script-local style overrides authored declarations. Shorthands, !important, percentages, Flexbox, and mixed inline content are excluded from the prototype. The exact property/value allowlist, default values, and inheritance list must be finalized in G3 before implementation exposes each property.

Prototype rendering covers colors/backgrounds, static images, text size, translate, scale, and opacity. Specify transform order, stacking, clipping and group compositing in G3 fixtures. Native animation targets only approved properties; timing, animation/script precedence, and pause semantics are defined in the scripting contract before integration.

The [version-2 native group-data contract](m3-groups.md) specifies ordered groups, axis transforms, rectangular ancestor clips and true group-opacity boundaries. The separate [opt-in group compiler](m3-group-compiler.md) now accepts its documented restricted opacity/transform/overflow syntax and validates hierarchical output/images. The default v1 compiler/preview still reject effects; group GPU rendering and visual acceptance remain open. Compiled data alone does not establish rendered support.

Reject unsupported features with file/source diagnostics. Canvas, WebGL, SVG, Grid, shaders, network, audio, and arbitrary website compatibility are excluded. Product additions follow the architecture and require documented behavior plus validation.
