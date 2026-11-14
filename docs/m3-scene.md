# M3 scene-data foundation

Date: November 14, 2026. Status: implemented experimental data model, codec and validation. HTML/CSS parsing, manifest/source loading, rendering integration and G3 acceptance remain pending.

## Run the fixed check

From the repository root:

```powershell
cargo run --release -p wallpaper-cli --locked -- --scene-check
cargo test -p wallpaper-scene --locked
```

The CLI validates an original fixed three-node scene, encodes it, independently decodes/validates it, and compares both the data and re-encoded bytes. Expected output includes `Scene self-check passed`, version 1, three nodes, one resource and 147 encoded bytes; exit status is zero. No window, source file, package, image decoder or script runtime is opened. Supplying a file path or extra argument is rejected with exit 2. This is a developer self-check, not a wallpaper preview or compile command.

## Data contract

The dependency-free `wallpaper-scene` crate represents resolved flat drawing data, independently of the future HTML/CSS parser. A raw `Scene` must pass validation to become an immutable `ValidatedScene`; only that value exposes encoding. `decode` returns a newly validated value, never trusting the sender's earlier validation. Errors contain bounded codes and scene/node/resource indices, without echoing untrusted text or paths. Mapping those indices to source-file spans is compiler work still to come.

The scene has a version, integer canvas dimensions, background RGBA color, image resource declarations and an ordered node list. Nodes have unique nonzero scene-local IDs, resolved canvas-relative pixel rectangles, and exactly one of:

- Rectangle: a single RGBA fill.
- Text: literal UTF-8, a finite pixel font size and RGBA color; future rendering uses the bundled font.
- Image: a reference to a declared nonzero resource ID.

Colors are straight/unpremultiplied sRGB bytes. Node order is back to front. Position can be negative or outside the canvas; zero-area bounds are valid data. These declarations do not implement compositing, cropping, text shaping/overflow or layout. Resource dimensions are untrusted metadata, not decoded pixels or proof of resource existence. Future resource loading must independently validate retained bytes, decoded dimensions and budgets before GPU upload. Scene-local IDs are not live host handles or runtime mutation authority.

Hierarchy, CSS cascade, selectors, group opacity, clipping, transforms and animation are deliberately absent from this initial data schema and remain M3 work. Do not flatten groups or lose unsupported semantics to make them fit this foundation; extend/version the schema and validate those semantics before compiler/rendering integration. The [content contract](supported-features.md) remains authoritative about planned source features; none becomes available merely because a data record exists.

## Enforced budgets

| Value | Initial bound |
| --- | --- |
| Encoded scene input | 2 MiB |
| Nodes / image declarations | 4,096 / 1,024 |
| Canvas dimension / total pixels | 1..8,192 per axis / at most 33,554,432 pixels |
| Node x/y | Finite, absolute magnitude at most 32,768 px |
| Node width/height | Finite, 0..32,768 px |
| Font size | Finite, 1..512 px |
| UTF-8 text | At most 16 KiB/node and 1 MiB total; byte counts, not characters |
| Text controls | LF preserved; other Unicode control characters rejected |
| Image dimension | 1..4,096 per axis |
| Aggregate declared decoded RGBA8 | At most 128 MiB, including unused declarations |

Existing node/text/image limits derive from the prototype package budgets. The new 1 MiB aggregate text and 2 MiB scene caps bound allocation and repeated validation independently of the larger proposed source-package cap. With all permitted counts/text simultaneously at maximum, the current encoding still fits the scene cap. The canvas cap limits declared pixel work and accommodates an 8K canvas within the pixel budget; it is not GPU-allocation or hardware-support certification. The geometry/font bounds keep initial finite arithmetic controlled and are provisional parser/rendering policy, not browser-equivalence claims.

The scene envelope is separate from M2's 64 KiB control frames. It cannot be sent as one oversized control message. Chunking/resource ownership and the production source/scene transport must be defined and tested before integration; no IPC limit changes in this step.

## Experimental binary encoding

All integers and IEEE-754 binary32 values use little-endian bytes. No padding, optional extension fields or trailing bytes are accepted.

| Record | Fields, in order |
| --- | --- |
| Header, 30 bytes | Eight magic bytes (`SAHNSCN` followed by NUL); u16 scene version; u32 canvas width; u32 height; four background color bytes; u32 resource count; u32 node count |
| Image declaration, 12 bytes | u32 ID, u32 width, u32 height |
| Node prefix, 21 bytes | u32 node ID; f32 x, y, width, height; u8 content tag |
| Tag 1: rectangle | Four RGBA bytes |
| Tag 2: text | Four RGBA bytes; f32 font size; u32 UTF-8 byte length; exact UTF-8 bytes |
| Tag 3: image | u32 declared image ID |

Version 1 is an experimental flat-data revision, not the complete native package format or a promise of stable creator-facing output. Future incompatible features need an explicit version decision. Unknown versions/tags, duplicate/zero IDs, missing resource references, invalid numbers/text, truncation and trailing bytes fail the whole decode.

Before count-driven allocation, the decoder bounds input size, counts and the minimum remaining record bytes. Text lengths and aggregate text are checked before copying strings. Checked offset/aggregate arithmetic and safe slice/UTF-8 operations protect parsing; no unsafe code or external decoder is used. This does not validate hostile HTML/CSS, archive paths, fonts or media, and it does not enable untrusted content execution.

## Validation and dependency review

Fourteen scene tests cover exact wire bytes, roundtrip/order/Unicode, every truncated prefix, trailing bytes, unknown magic/version/tags, hostile counts/lengths, NaN/infinity in every geometry/font field, ID/reference errors, canvas/geometry/image/text/count boundaries, aggregate limits, malformed UTF-8/controls, simultaneous maximum budgets and 1,024 deterministic byte mutations. These mutation checks are a regression corpus, not exhaustive fuzzing or a security proof.

All thirty-five workspace Rust tests, Clippy with warnings denied, formatting and the workspace release build passed locally on Windows x64 with Rust 1.99.1 MSVC. Release CLI self-check/help/version and invalid-argument exit 2 passed. The existing incremental-cache hardlink/copy notices remain. The CI workflow now invokes the fixed CLI check after building; hosted execution and non-Windows builds were not run. There is no new visual behavior requiring user testing.

The only new dependency edge is from the CLI to this repository's own safe Rust scene crate. It adds no third-party package, feature or version: all 231 external Cargo.lock package blocks remain identical to the prior commit. Necessity is a shared compiler-output/host-validation boundary; maintenance/provenance stays in this project, with no external publisher, build script, native code or new license material. Target-neutral APIs compile on local Windows; other targets remain untested. Existing engine/renderer advisory and maintenance findings are unchanged; adding safe bounded code is not a claim that every input or the application is secure.

Primary Rust documentation reviewed November 14, 2026: [finite floating-point checks](https://doc.rust-lang.org/std/primitive.f32.html#method.is_finite), [checked integer arithmetic](https://doc.rust-lang.org/std/primitive.usize.html#method.checked_add) and [validated UTF-8 conversion](https://doc.rust-lang.org/std/str/fn.from_utf8.html). Current online docs describe Rust 1.100; this code uses long-stable APIs available in the unchanged pinned 1.99.1 toolchain.
