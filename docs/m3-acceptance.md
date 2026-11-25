# M3 acceptance record

The native compiler, trusted Windows folder CLI and fixed previews cover the M3 prototype scope. Evidence is specific to the recorded Windows D3D12 setup. M4 owns integrated worker transactions and restricted helper processing for untrusted inputs.

| Requirement | Evidence |
| --- | --- |
| Native sources and resources | Trusted folder collection validates paths, file types, limits, manifests and references. The CLI compiles flat and grouped scenes, validates decoded output and supports deterministic export and verification. [Trusted package CLI](m3-manifest.md#trusted-package-cli) |
| HTML/CSS and diagnostics | The strict parser supports the documented fragment, selector, layout, text and style subset. Compiler tests cover source locations, unsupported syntax, malformed input, finite geometry, and source/work budgets. [Compiler contract](m3-compiler.md) |
| Images and group effects | PNG limits and independent asset validation have CPU evidence. Local window runs cover images and group effects. The 18 native-canvas GPU samples passed, including fractional clipping and nested opacity. [PNG evidence](m3-images.md) · [GPU samples](m3-pixel-check.md) |
| Native animation | The trusted typed fixture covers translation, axis scale, and group opacity. Local window checks cover motion, Space pause/resume, resize and controlled GPU recovery. The fixed one-second pose has a retained window image. [Animation evidence](m3-animated-preview.md) |
| Static idle | Controlled D3D12 static preview showed one presented frame during the six-second quiet-idle case. [Static preview](m3-preview.md) |

CSS animation declarations, general source-authored animation, archive collection, broader device/DPI coverage and untrusted execution remain outside this local acceptance. The recorded AppContainer cases are fixed probes. Integrated package and worker validation remains part of M4.

The next development task is bounded host mutation transactions with revision checks and acknowledgements. Its acceptance must demonstrate that invalid work leaves the last accepted scene intact.

## Final local checks

At source revision `e9abddb`, workspace formatting, Clippy, release build and five scene/compiler CLI checks passed. The workspace run passed 144 tests. The two Windows junction tests were excluded from this run and have passing local results from the prior 146-test checkpoint. A fresh run of the ten package/compiler AppContainer probes could not start because the automatic approval reviewer hit its usage limit. The previously recorded probe and GPU results remain the evidence for those paths.
