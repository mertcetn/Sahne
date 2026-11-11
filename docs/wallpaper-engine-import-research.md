# Wallpaper Engine Import — Feasibility Note

Research date: November 11, 2026. Status: selective compatibility planning approved; no importer, compatibility API, or browser backend is implemented. See [Import plan](wallpaper-engine-import-plan.md).

## Conclusion

Selective conversion is plausible. Universal Wallpaper Engine compatibility does not follow from PKG extraction or from executing JavaScript. Classify input by content type, distinguish resource extraction from behavior compatibility, and define an explicit supported subset before committing to an importer.

## Content types

| Input | Potential migration path | Current Sahne boundary |
| --- | --- | --- |
| Scene PKG | Extract supported assets, decode supported textures, translate a small scene subset | No PKG/TEX importer or Wallpaper Engine scene runtime |
| Web directory | Read project metadata, validate HTML/CSS/JS/resources, generate a Sahne package | Requires the planned restricted content compiler/runtime; arbitrary browser content is excluded |
| Video | Import the underlying media when a bounded media pipeline exists | Video playback is outside the initial prototype |
| Application | No conversion path within this design | Native application execution is excluded |

Wallpaper Engine's official FAQ distinguishes scene, web, video, and application wallpapers. Scene packages omit some authoring/project information. It points to an unofficial unpacker without guaranteeing correctness. Extraction therefore cannot promise original editable project recovery or equivalent playback. [Official FAQ](https://help.wallpaperengine.io/en/functionality/editingwallpapers.html)

RePKG's own repository describes a reverse-engineered PKG/TEX extractor and texture-to-image converter. This establishes technical extraction precedent, not full Sahne scene compatibility. No RePKG code, binary, or dependency was adopted or security-reviewed here. [RePKG source description](https://github.com/notscuffed/repkg)

## Scene compatibility is an engine project

Extracting images does not reproduce layered effects, materials, particles, models, animation or runtime logic. Wallpaper Engine SceneScript is ECMAScript-based but replaces web facilities with engine-specific APIs. Its reference exposes scene/layer/material/particle objects and lifecycle events. Translating a simple transform may be possible; matching the broader semantics requires a dedicated scene compatibility layer and relevant renderer features. This is outside the current restricted-web design. [SceneScript introduction](https://docs.wallpaperengine.io/en/scene/scenescript/introduction.html), [Reference](https://docs.wallpaperengine.io/en/scene/scenescript/reference.html)

## Web and API compatibility

Wallpaper Engine web projects consist of HTML and related files with project.json metadata. An importer could translate metadata and resources into Sahne's package contract. Matching file extensions does not make browser DOM/CSS, Canvas/WebGL, media or library behavior available in Sahne. A wrapper for Wallpaper Engine's API cannot replace those missing facilities. [Web project structure](https://docs.wallpaperengine.io/en/web/first/gettingstarted.html), [Web scope](https://docs.wallpaperengine.io/en/web/overview.html)

Potential future API mapping, not current support:

| Wallpaper Engine facility | Sahne compatibility assessment |
| --- | --- |
| wallpaperPropertyListener.applyUserProperties | Adapt selected primitive settings after a matching settings contract exists |
| applyGeneralProperties FPS | Map to host-enforced scheduling limits; packages cannot raise limits |
| setPaused | Translate to the defined pause/resume lifecycle, checking timing semantics |
| Directory/file properties | Require explicit resource-import permissions and host-issued IDs; do not expose arbitrary paths |
| Audio, RGB and other host services | Separate capabilities; not enabled by an API shim |

The listener's documented initial/update delivery behavior must be preserved where claimed. Unknown capabilities must be reported explicitly, not silently stubbed. [Property listener API](https://docs.wallpaperengine.io/en/web/api/propertylistener.html), [User properties](https://docs.wallpaperengine.io/web/customization/properties)

JavaScript source normally needs host-API adaptation and feature analysis, not translation into Rust. An optional browser backend could expand web compatibility, but it would be a separate architectural/runtime choice with its own isolation, resource and performance measurements. It would not provide native Scene PKG compatibility by itself.

## Recommended investigation sequence

1. Inspect a small, representative set of locally supplied project folders/packages without executing their code.
2. Report required elements/CSS, JavaScript/browser features, Wallpaper Engine API calls, scene resources and unsupported dependencies. Static analysis cannot prove arbitrary JavaScript compatible; dynamic behavior and computed API use require bounded tests.
3. Prototype web-directory metadata conversion and a narrow settings/pause shim after Sahne's native compiler and sandbox runtime exist.
4. Consider PKG resource extraction separately; label a static-image fallback as lossy and optional rather than equivalent scene conversion.
5. Compare supported fixtures visually and behaviorally. State compatibility per feature/tested input; no automatic full-library promise.

Imported archives, textures, fonts and code remain untrusted. Existing extraction, decoding and runtime limits must apply. Workshop downloading/distribution is separate from local import and is not proposed by this note. The approved scope is planning now, contract preparation during M2/M3, selective web import after M4, and a separate later scene-assets investigation. Architecture and roadmap now reflect that boundary.

## Local evidence follow-up

Two optional ignored local examples were inspected without execution. The web example requires Canvas/WebGL/media/audio and network-related code paths and exceeds the provisional expanded-size budget; it is a reporting/negative fixture, not a first native conversion success target. Scene contents remain unextracted. See [Sample assessment](wallpaper-engine-samples.md).
