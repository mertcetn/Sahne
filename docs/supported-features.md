# Prototype Content Contract

Status: M0 draft; none of these content features are implemented yet.

The prototype accepts UTF-8 source with div, span, and img scene elements. html/head/body and resource declarations need a parser-spike decision before implementation; unsupported constructs must not silently disappear through HTML error recovery. IDs must be unique. No inline event handlers, external URLs, script imports, or executable attributes are accepted.

Initial layout is absolute with fixed px dimensions. span is a text box, not browser mixed inline flow. Use a bundled font; no system-font equivalence is promised. Preserve literal text initially; define newline, line-height, and overflow behavior in G3 visual fixtures before accepting text packages.

Initial selectors are ID and class. Cascade ordering is specificity followed by source order; script-local style overrides authored declarations. Shorthands, !important, percentages, Flexbox, and mixed inline content are excluded from the prototype. The exact property/value allowlist, default values, and inheritance list must be finalized in G3 before implementation exposes each property.

Prototype rendering covers colors/backgrounds, static images, text size, translate, scale, and opacity. Specify transform order, stacking, clipping and group compositing in G3 fixtures. Native animation targets only approved properties; timing, animation/script precedence, and pause semantics are defined in the scripting contract before integration.

Reject unsupported features with file/source diagnostics. Canvas, WebGL, SVG, Grid, shaders, network, audio, and arbitrary website compatibility are excluded. Product additions follow the architecture and require documented behavior plus validation.
