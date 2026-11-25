# Import Adapter Capability Boundary

Status: I1 M2 security/inspection boundary specified; no importer, report parser or compatibility runtime is implemented. The subsequent [M3 mapping contract](i1-web-mapping.md) specifies metadata, resources, capabilities and callback semantics. I2 implementation remains after M4. This is not an expansion of supported native content or Wallpaper Engine compatibility.

## Authority

Native [package validation](package-format.md), [content features](supported-features.md), [scripting contract](scripting-api.md) and [threat model](threat-model.md) remain authoritative. An importer is an untrusted-input frontend, not a source of runtime authority. A detected requirement describes what a source may need; it never grants that capability. A converted package must pass independent host validation just like an original native package.

| Boundary | Required behavior |
| --- | --- |
| Permissions | Native v1 permissions remains empty; input declarations cannot request network, process, arbitrary filesystem, GPU/shader, audio or broker access |
| Runtime/IPC | Imported metadata cannot name an endpoint, native handle/address, executable, loader or host method to invoke; only explicitly versioned native operations may be emitted |
| Resources | Resolve bounded, validated local input in the future import/helper pipeline; pass validated resource data/host-issued IDs onward, not raw filesystem paths to JS or scene operations |
| Budgets | Host policy caps memory, work/jobs/timers, frames/messages/queues and FPS; input cannot raise them, reset retry counts, extend a deadline or request a weaker worker |
| Source | JavaScript source only when the validated runtime path exists; no foreign bytecode, native modules, command execution or sample evaluation during inspection |
| Lifecycle | Future adapters use native pause/time rules; no background callbacks, timer backlog or active work during a native pause merely to imitate foreign behavior |
| Missing features | Report unsupported/unknown/unimplemented capabilities; do not fabricate success with inert API stubs or silently widen the native subset |

The fixed M2 JS fixture environment has no creator-facing document/timer adapter and cannot consume converted content. Its test-only arguments, including sentinel paths and endpoint probes, are not APIs an import adapter may expose. Existing native startup/system access is not a package permission grant.

## Non-executing inspection

Discovery must not run source scripts, evaluate metadata conditions, use a browser/JS runtime for detection, invoke package/build hooks, start applications, load shaders, fetch URLs, download Workshop items or perform unbounded decoding/decompression. Merely parsing an untrusted format requires bounded parsing/helper isolation before implementation accepts it; the current ignored samples are not executable fixtures.

Preserve source inputs. Conversion writes a separate result only when implemented and explicitly selected. Do not resolve a resource by reopening an unchecked mutable path after validation. Apply native traversal, absolute/drive-relative/UNC/alternate-stream, links/reparse points, duplicates/case collisions, expanded-size and decoded-size rules. Source hashes track identity/provenance, not trusted authorship, permission or redistribution rights.

Dynamic/computed API/resource use, eval, obfuscation and unavailable mappings are uncertainty, not evidence of compatibility. Inspection may return unknown. Claimed source type from project metadata must be checked against observed bounded content; a filename or manifest hint cannot authorize a renderer/backend or PKG extractor.

## Proposed sidecar report shape

The following is a design proposal for I2, not a schema currently accepted by Sahne. Report metadata stays separate from native manifest v1; no new manifest field or IPC tag is introduced here. M3 must finalize exact encoding, enum/catalog versions, numeric/string/list budgets and original synthetic fixtures before implementation.

| Field group | Intended content and constraints |
| --- | --- |
| reportVersion / inspectorVersion | Explicit schema and producer identity; reject unsupported versions when a reader exists |
| source | Detected type, content identity, normalized relative evidence locations and untrusted manifest hints; no exported absolute user paths |
| target | Exact native format/API and actual implemented feature catalog of the selected Sahne build; planned features are not available capabilities |
| requirements | Detected feature/API/resource requirements with evidence and confidence; never an allow/permission list |
| diagnostics | Structured reason codes, bounded escaped display text and relative source locations; unsupported, unknown/dynamic, invalid resource, exceeded budget and mapping pending remain distinct |
| coverage | What was inspected, exclusions/uncertainty and whether evidence/diagnostics were truncated by host limits; no silent truncation or assumed complete analysis |
| actions | Proposed supported conversion, blocked requirements, or explicitly identified manual/lossy work; no executable instructions |
| verdict | convertible, needs-review, unsupported or unknown, based on the selected implemented native subset and evidence completeness |

Convertible requires every required feature/resource to have a validated available mapping and sufficient evidence; an incomplete/dynamic input cannot be promoted merely because some static assets map. Needs-review/lossy conversion requires a visible loss report and explicit selection in the future import flow. The current app produces none of these reports; existing sample assessment remains manual static research.

M3 must still define primitive settings types/defaults and initial/change-only delivery, host-capped FPS, pause/resume mapping, provenance/version fields and resource ID associations against the final native compiler/API contracts. This boundary deliberately makes no callback parity claim. The overall I1 checklist stays open until that work is complete. Scene PKG extraction/playback, browser backend and additional host services remain separately scoped decisions in the [Import plan](wallpaper-engine-import-plan.md).

Review basis: the native contract documents linked above and the approved import plan, November 11, 2026. The external property-listener reference could not be refreshed during this task because the request timed out; no API delivery semantics were finalized from that unavailable page. Recheck current official API documentation before M3 mapping work. This document is a project boundary/design review, not a passed runtime compatibility test.
