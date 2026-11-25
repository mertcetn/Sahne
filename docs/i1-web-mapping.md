# I1 web import mapping

This contract defines the first inspection and conversion cases for I2. Runtime delivery remains gated on M4. Scope is local web projects, with native manifest format 1 and API 1 as the target. Official Wallpaper Engine docs describe `project.json` and the web callbacks. The field mappings below define Sahne's allowlist. [Web project structure](https://docs.wallpaperengine.io/en/web/first/gettingstarted.html) · [Property listener reference](https://docs.wallpaperengine.io/en/web/api/propertylistener.html)

| Source evidence | Mapping |
| --- | --- |
| `type: "web"`, `file` | Treat type as a hint. Require a bounded retained HTML file at the exact validated relative path and compile the content against the native subset. |
| `title` | Use as the native display name after native length and character validation. Missing or invalid names require a valid replacement before conversion. |
| Foreign IDs, Workshop fields, preview and tags | Preserve source identity in the report if relevant. The host supplies a valid native ID separately. |
| Relative CSS and PNG references | Use the exact validated retained entry. Resolve with native path rules and compile into the existing resource table. Use the compiled scene's resource ID and decoded RGBA for images. |
| External URLs, file/directory properties, unsupported image formats | Report the capability or resource as unsupported. |
| Missing or unsafe paths | Report the reference with its relative source location. Keep case collisions, traversal and budget failures distinct in diagnostic codes. |
| Unknown browser features or computed resource references | Report known unsupported features and mark unresolved dynamic requirements as unknown. |

Source metadata is capped at 16 KiB. Apply the existing native file count, file size, aggregate size and decoded image budgets. Output permissions are empty. Reports associate SHA-256 source identity and adapter version with the converted record. The native provenance record and receiver comparison verify the resulting native package against its expected inputs. [Compiled provenance](m3-compiled-provenance.md)

## Settings and callbacks

I2 candidates are checkbox booleans, finite sliders with explicit finite min/max/default, fixed combo choices with bounded string values, and UTF-8 text defaults. Limit to 64 properties, 64 ASCII bytes per identifier, 64 combo choices, and 256 UTF-8 bytes per string value or label. Keys use the native ASCII identifier grammar. Reject duplicate keys or choice values, incompatible defaults and out-of-range updates. Color, conditional expressions and file/directory settings require separate mappings. [Wallpaper Engine user properties](https://docs.wallpaperengine.io/en/web/customization/properties.html)

After handler registration, deliver one snapshot of all admitted settings with effective values. Subsequent deliveries contain only changed keys, each with its value. The host validates updates and coalesces multiple changes to the latest value per key before the next callback. A combo label is display metadata. Use the stable combo value for logic.

Deliver the effective integer FPS after registration and on changes through the general-properties mapping. The host computes the minimum of its cap, user limit and native preferred FPS, each in 1–60. Imported code has no authority over scheduling. For pause, stop animation time and regular callbacks when the first native pause reason is set. Resume only when the last reason clears. If a future compatibility implementation maps `setPaused`, reserve a bounded transition callback for each effective state change. Deliver queued settings and FPS once on resume, then release regular work under native timer limits. Coalesce paused updates and skip missed frame callbacks. The callback mapping remains an implementation requirement for I2. [Property listener reference](https://docs.wallpaperengine.io/en/web/api/propertylistener.html) · [Native scripting contract](scripting-api.md)

## Proposed report v1

Use UTF-8 JSON with a 64 KiB encoded cap. Required fields are `reportVersion: 1`, `inspectorVersion` (1–64 ASCII bytes), lowercase hexadecimal `sourceSha256` (64 digits), `target` (native format version, API version and capability catalog version), `complete` (boolean), `diagnostics` (at most 64), and `verdict`. Catalog v1 uses `html-subset`, `css-subset`, `png-rgba`, `primitive-settings`, `fps-limit`, and `pause-resume`. Mark support according to the selected build's verified features.

Each diagnostic carries a capability when applicable, a stable reason code, an optional validated relative path and optional one-based line and byte column, and display text limited to 256 UTF-8 bytes. Initial codes are `unsupported-feature`, `dynamic-unknown`, `missing-resource`, `unsafe-resource`, `path-collision`, `invalid-metadata`, `budget-exceeded`, and `mapping-pending`. A dropped diagnostic sets `complete` to false. Use `unsupported` for invalid input or definitive blockers, `unknown` for incomplete evidence without a definitive blocker, `needs-review` for an explicitly identified manual or lossy action, and `convertible` only when required mappings are available and inspection is complete. Reports carry evidence for decisions. Native package and runtime validation enforce authority.

The [synthetic fixture matrix](import-fixtures/web-mapping.json) records expected I2 outcomes. It uses original in-memory inputs and proposed verdicts. It is a contract fixture for a future inspector.
