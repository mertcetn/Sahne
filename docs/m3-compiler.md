# M3 strict HTML/CSS compiler foundation

This document describes the existing version-1 `compile` path used by the preview. The separate [version-2 `compile_groups` contract](m3-group-compiler.md) adds explicitly opted-in effect parsing and hierarchical output; it is not yet rendered. Version-1 effect rejections and fixtures below remain unchanged.

Updated: November 24, 2026. Status: original fixed-source compilation into the experimental flat scene, including [bounded PNG resources and a local image-window reference](m3-images.md). Full G3, effects/animation and arbitrary input remain pending; [preview evidence](m3-preview.md) is limited to recorded local cases.

## Run

```powershell
cargo run --release -p wallpaper-cli --locked -- --compile-check
cargo test -p wallpaper-compiler --locked
./tools/test-sandbox.ps1 -Cases @('compiler-basic', 'compiler-invalid')
```

The CLI embeds the original files in examples/static-scene at build time, validates their in-memory package, compiles them and independently decodes/validates the scene data. It prints `Compiler self-check passed` with three nodes, three named elements and 123 encoded bytes. It opens no files/window and executes no script. Extra arguments or an external path still fail with exit 2.

The two Windows AppContainer cases exercise the same compiler under the existing zero-capability/Job/watchdog/channel policy. The positive case checks three nodes, the title's resolved coordinates/binding and a scene roundtrip. The negative case requires precise rejection of Flexbox/display, NaN/percentage dimensions, opacity, duplicate IDs, mismatched tags, remote image URLs and script elements. The report mask is `0x80017`; no scene bytes or untrusted source are transported through the test-only result body. Production source/scene IPC remains M4 work.

## Accepted source subset

This is an explicit native **fragment grammar**, not a general HTML parser or browser compatibility layer. It never inserts html/head/body wrappers, closes a missing tag, drops unsupported source or applies browser error recovery. Full documents, DOCTYPE, style/script elements, event attributes, SVG and unknown elements/attributes fail with a source diagnostic. A future importer may report these as unsupported; it must not claim compatibility simply by stripping them.

Before scene elements, zero or more top-level `<link rel="stylesheet" href="style.css">` declarations may reference retained CSS entries. Paths are validated package-root-relative names with exact-case lookup, not browser URL resolution relative to the HTML directory. They never fetch a URL. Links inside/after scene elements, other rel values or extra attributes fail.

Scene elements are lowercase div, span and img. Div/span require explicit matching end tags; only img/link are void and may use `/>`. Supported element attributes are id, class and style, plus required src for img. Values must be single- or double-quoted, attribute names lowercase, and duplicates fail. IDs/classes are case-sensitive ASCII identifiers: initial letter/underscore followed by letters/digits/underscore/hyphen, at most 64 bytes. Author IDs are unique. Class lists have at most sixteen entries.

Div can contain boxes and whitespace/comments between boxes, but not raw non-whitespace text. Span is a leaf text box; nested elements/comments/mixed inline flow fail. Img resolves a retained lowercase `.png` path, decodes only the [bounded RGBA8 subset](m3-images.md), and emits a scene resource/image primitive. Repeated paths share one decoded asset; image backgrounds precede the image. Missing, malformed or unsupported images fail at the referring HTML element. Group opacity, transforms and ancestor clipping remain unimplemented in this flat revision.

Text preserves literal whitespace inside span, with literal CR/CRLF normalized to LF. Five named references (amp, lt, gt, quot, apos) and semicolon-terminated decimal/hex Unicode references are decoded. Unknown, malformed or control-character references fail; escaped markup stays literal text. Controls other than LF are rejected in scene text. Attribute class whitespace permits ASCII whitespace. Style-attribute character references are excluded so inline CSS byte offsets remain exact in the original HTML.

## CSS and layout

Stylesheets support only a single ID or class selector per rule. Selector lists, compound/type/descendant selectors, pseudo-classes/elements, escapes, strings and at-rules are rejected. Comments are recognized outside strings; strings themselves are outside this grammar. Unknown properties/values fail even in a rule that matches no element. There is no silent declaration dropping.

| Property | Accepted values / behavior |
| --- | --- |
| position | absolute only; also the native default |
| left, top | Finite plain decimal px, -32,768..32,768; default zero |
| width, height | Finite plain decimal px, 0..32,768; both required on every box |
| font-size | Finite plain decimal px, 1..512; default 16, inherited |
| color | #RGB/#RGBA/#RRGGBB/#RRGGBBAA or transparent; default white, inherited |
| background-color | Same color forms; default transparent, not inherited |

The literal unitless zero is allowed for lengths where their range permits zero. Exponents, trailing-dot numbers, percentages, calc/var, other units, shorthands, !important, opacity, transforms and animation declarations are rejected in this stage.

Cascade is resolved per property: single ID outranks single class; equal specificity uses declaration/source order; inline declarations override stylesheets. Only color and font-size inherit. Inherited values have no retained parent selector priority, so a child's authored declaration overrides its inherited value normally.

All boxes are absolutely positioned relative to their parent div's origin, or the canvas origin at the root. Dimensions do not inherit or auto-size. Canvas size/background come from the caller's viewport policy, not additional manifest fields. Compiled geometry is validated again after accumulating parent positions. Initial draw order is source preorder, back to front, with visible overflow; this does not implement CSS z-index or group compositing.

Div emits a rectangle, including a transparent one. Span emits an optional background rectangle followed by its text primitive. One author ID can therefore map to multiple scene primitive IDs; the compiled symbol table records that association. These are scene-local compiler identities, not trusted live host handles. Future runtime binding/ownership/generation checks must independently validate/remap them.

The compiler returns validated scene data, symbols and an optional deferred script-source association. It does not execute that script or produce the complete hashed compiled package yet. The caller retains the immutable original Package bytes. Media decoding, compiled asset indexing/hashes, hierarchical effects and native animation remain M3 work; timers/callbacks/production mutations remain M4.

## Bounds and diagnostics

- HTML source: at most 1 MiB; 4,096 logical elements; depth 32; four attributes/element; 4,096 raw bytes/attribute; 16 KiB decoded text/span.
- At most sixteen leading stylesheet references. Aggregate referenced CSS is at most 1 MiB, counting repeated references; inline CSS is already inside the bounded HTML source.
- At most 1,024 rules across referenced sheets, 32 declarations/block and 65,536 declarations across sheets/inline styles. Selector matching is bounded by those rules, elements and class counts; this is not a throughput guarantee.
- The final scene independently retains its 4,096 primitive-node, aggregate text, finite-number, canvas and encoding budgets. A text background consumes another primitive rather than evading the node limit.

Parsing is iterative for the element tree. All-or-error results prevent a partial scene from being accepted after a source failure. Diagnostics contain a validated relative path, byte start/end, one-based line and UTF-8 byte column plus a bounded enum code, without echoing source text. Inline errors point into original HTML; CSS errors retain the stylesheet path. Missing-resource diagnostics point at the HTML reference. This is parser-side source reporting, not a completed Wallpaper Engine sidecar schema.

Sixteen compiler tests cover strict recovery rejection, identifiers/attributes/entities, selectors/numeric/color values, source offsets, independent scene validation, cascade/inheritance, parent coordinates, primitive bindings, missing/deferred resources, cumulative geometry, source/depth/node/text/rule/declaration/aggregate work budgets and 512 deterministic source mutations. The original fixture and its expected coordinates/colors are machine-checked; a later [window-mode visual reference](m3-preview.md) records limited local rendering evidence, without closing G3.

Local validation passed all 63 workspace Rust tests, release build, Clippy with warnings denied, formatting and CLI compile/scene/invalid-argument smoke checks. Full sandbox report `d543a1635d1d431bae2e535d6d4c3484` passed 39 cases, 45 profile cleanups and one registry cleanup, with `G2Complete=false`. Selected earlier compiler evidence is in `51efa2ad6cec41a692dbd10091844b1f`. Existing Cargo cache hardlink/copy notices remain. Hosted CI and non-Windows builds were not run. Development release packaging now includes the original examples/ source tree alongside the existing binaries/docs when the workflow is next authorized to publish; no release push/publication occurs here.

## Review and remaining gates

The compiler inherits unsafe-code prohibition and now uses local format/scene/media crates. The media graph is separately assessed in the [PNG dependency review](m3-png-dependencies.md). The fixtures feature embeds only original checked-in sources, never ignored samples/. This is not a claim that handwritten subset parsing is secure: the grammar, bounds and negative corpus require continued adversarial/fuzz/helper validation before arbitrary source input is enabled.

Primary standards reviewed November 14, 2026: [WHATWG HTML syntax](https://html.spec.whatwg.org/multipage/syntax.html), [CSS Syntax Level 3](https://www.w3.org/TR/css-syntax-3/) and [selector specificity](https://www.w3.org/TR/selectors-4/#specificity). They inform the terminology and relative cascade behavior. This deliberately narrower grammar does not implement their complete tokenization, recovery or selector algorithms. Unsupported behavior stays rejected and documented. Production G2/G3 and arbitrary source/package execution remain open.
