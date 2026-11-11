# Wallpaper Engine Sample Assessment

Inspected: November 11, 2026. Method: read-only directory inventory, manifest parsing, source-text inspection and a bounded package-header read. No HTML/JavaScript/shader execution, network requests from sample code, binary decoding or PKG extraction was performed. These observations are not runtime compatibility results.

## Local-only corpus

The root samples/ directory is ignored by Git. Its contents are optional local research inputs, not repository examples, CI prerequisites, release assets or adopted dependencies. Keep extracted/generated sample assets under samples/ or ignored target/. Do not copy third-party code or media into tracked fixtures as part of planning. CI should eventually use small, original synthetic fixtures for each declared feature.

| Local sample | Declared type and entry | Inventory | Initial assessment |
| --- | --- | --- | --- |
| samples/3293156956 | scene; manifest entry scene.json | 16 files, 14,340,439 total bytes; scene.pkg is 13,428,067 bytes; preview and shader blobs alongside it | Separate scene/asset investigation; no scene playback support claim |
| samples/884307090 | web; index.html | 165 files, 669,459,522 total bytes | Complex incompatibility/reporting fixture, not a first-pass native conversion target |

Byte totals describe disk files, not decoded memory. Local directories can change; re-inventory before drawing further conclusions. The two examples do not establish coverage or compatibility percentages for Wallpaper Engine as a whole.

## Scene observations

The bounded header read contains the ASCII marker PKGV0021. This is an observation of this one file, not a documented supported format version or proof that the full package is valid. scene.json is declared in project.json but is not present as a loose file in this directory. The shaders/blobsSM40 directory contains .dxs files; neither their internals nor dependencies were decoded. The package's inner scene graph, textures, effects and scripts remain unknown until a separately bounded extraction investigation.

## Web observations

The HTML entry loads jQuery 3.2.1 and local plugins/scripts. Another jQuery version exists on disk; its mere presence does not prove it is loaded. Directly observed requirements include:

| Evidence location | Requirement observed | Native-path consequence |
| --- | --- | --- |
| index.html | audio, video, canvas elements and embedded shader source | Outside the initial div/span/img content contract |
| js/sakura.js and js/main.js | WebGL context calls | No WebGL support through a metadata/API shim |
| js/PWLine.js and js/PWCircle.js | Canvas 2D plus wallpaperRegisterAudioListener | Canvas and host audio capabilities are separate blockers |
| js/main.js | wallpaperPropertyListener.applyUserProperties | Candidate future settings mapping, not currently implemented |
| js/main.js initialization | Reads project.json through AJAX, evaluates constructed code, and has navigation/identity-dependent logic | Requires bounded analysis and explicit review; never run/evaluate code to classify input or silently rewrite behavior |
| js/time.js | HTTP weather request path | Network capability is outside the first native importer; runtime activation/reachability was not established |
| style/default.css | Percent dimensions, em units, text shadows, stacking and media object-fit | Requires property-level diagnostics against the actual compiler contract |

The manifest contains 172 property entries: bool 38, color 14, combo 24, directory 1, file 4, slider 55, text 35, textinput 1. These are metadata categories, not 172 validated controls. Conditional expressions and UI text require their own parser/escaping rules and must not be evaluated as JavaScript during import.

The complete web directory exceeds the provisional 64 MiB expanded-package limit. Do not raise limits just to accept this sample. Reachable-resource analysis and a later explicit media-budget decision are needed; unused assets cannot be identified safely by filename counts alone. Full fidelity is outside the approved first native importer scope even if optional effects can be disabled.

## How these inputs guide the plan

- During M2/M3, use the examples to identify capability, metadata, resource-ID and diagnostic needs; do not add Canvas/WebGL/audio/network merely to pass the complex sample.
- After M4, the first importer must report this web example as requiring unsupported features, with paths/evidence and uncertainty, rather than claim successful conversion.
- Use small, original supported web fixtures for initial successful conversion and settings/pause tests. A partial clock/background port is a separate, explicit lossy conversion and is not equivalent playback of this sample.
- Keep PKG extraction/version validation, TEX conversion and scene/SceneScript playback as separate later investigations. Successfully listing/extracting resources is not renderer compatibility.
- Preserve original files and provenance; create converted output separately and never treat original metadata as trusted host policy.

See [Import plan](wallpaper-engine-import-plan.md) and [Research sources](wallpaper-engine-import-research.md).
