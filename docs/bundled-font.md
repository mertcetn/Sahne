# Bundled Preview Font

Reviewed and retrieved: November 11, 2026.

The preview embeds unmodified Noto Sans variable TrueType data from the official google/fonts repository. No font or media from samples/ is redistributed. The embedded file is the only font loaded into this preview's font database; it does not enumerate installed system fonts. Family selection is Noto Sans, with en-US shaping locale for the fixed English fixture. This is not a general package-font or international fallback implementation.

- Upstream directory revision: 8b0a1d0f5983c89bc2b93f1b5fb55f9e252744b5.
- Source file: ofl/notosans/NotoSans[wdth,wght].ttf.
- Local asset: assets/fonts/NotoSans.ttf (2,049,096 bytes).
- SHA256: bfb7bb691513f12e734dc346c03a03f784912432d7e3fa8e56efcf906fe86b3d.
- License: SIL Open Font License 1.1; copyright and complete license are retained in [Noto Sans OFL](licenses/NotoSans-OFL.txt). The release ZIP's docs/ directory includes this notice for the font embedded in the executable. The font is unmodified; the local filename does not change the font's internal family.

The license allows bundling/embedding subject to its conditions; preserve its copyright/license on distribution. This asset adds no Cargo dependency or changed crate graph. Security assessment: pin the official repository revision, record size/hash, load only compiled-in bytes and reject an empty glyph layout. This narrows the prototype font input surface but is not evidence that the font/parser is vulnerability-free. Existing ttf-parser maintenance and lru advisories remain documented in the [M1 dependency assessment](m1-dependencies.md). No arbitrary external fonts are accepted.

Sources consulted: [Pinned upstream font directory](https://github.com/google/fonts/tree/8b0a1d0f5983c89bc2b93f1b5fb55f9e252744b5/ofl/notosans), [Pinned upstream license](https://github.com/google/fonts/blob/8b0a1d0f5983c89bc2b93f1b5fb55f9e252744b5/ofl/notosans/OFL.txt), [COSMIC Text custom font database API](https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.FontSystem.html).
