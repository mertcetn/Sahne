# M3 compiled provenance fixture

The developer CLI can emit a binary provenance record for the embedded group fixture. It compiles the retained package, independently decodes the resulting v2 scene and validates its image assets before emitting the record. The default command preserves the original v1 group record. Named fixtures and the compiler API emit version 2. Production verification and collection from disk remain open.

From the repository root in PowerShell 7.4 or later:

```powershell
cargo build --release -p wallpaper-cli --locked
if ($LASTEXITCODE -ne 0) { throw 'CLI build failed' }
./target/release/wallpaper-cli.exe --compiled-provenance-fixture > target/compiled-provenance.bin
if ($LASTEXITCODE -ne 0) { throw 'Fixture export failed' }
Get-FileHash -Algorithm SHA256 target/compiled-provenance.bin
```

Redirect stdout to a file with byte-preserving redirection. Use a separately trusted expected digest when comparing outputs. Publisher authentication and source collection from disk are separate work.

The record starts with eight bytes `SAHNPRV\0` followed by version 1 as a little-endian u16. Each variable-length field has a little-endian u32 byte length. Each table starts with a little-endian u32 entry count. The fields follow this order:

1. CLI package version as UTF-8.
2. Retained source entries in validated path order. Each contains the UTF-8 relative path and original bytes, including the manifest and any unused entries.
3. Encoded v2 scene bytes, including canvas dimensions and background.
4. Author symbol entries in name order. Each contains the UTF-8 name and a little-endian u32 group ID.
5. Deferred script path as UTF-8, or a zero-length field when absent.
6. Decoded images in resource ID order. Each contains little-endian u32 ID, width and height, followed by a length-prefixed RGBA payload.

The existing scene, source and pixel budgets bound the record. The writer streams fields individually. Compiler provenance beyond the package version, including Git revision and toolchain identity, remains a separate concern.

Two independent release CLI launches on November 25, 2026 emitted 1,883 bytes with the same SHA-256 digest:

```text
5194A0544BFEBA5E49BFDF65F869305BD4F90F31A2D83EEBDC23F6B01D74D67E
```

## Version 2 records

Pass `static`, `image`, `groups` or `animated-shapes` after `--compiled-provenance-fixture` and redirect stdout as shown above. Library callers with a validated retained package can use `wallpaper_compiler::provenance::write_compiled` with a viewport and a flat, groups or typed animated mode. The API compiles and exports the same source snapshot and verifies the encoded payload and asset declarations before writing. Treat a writer error as a failed export and discard partial output.

Version 2 adds a one-byte payload kind after the version. Kind 1 carries a flat scene, kind 2 a group hierarchy, and kind 3 an animation envelope with its bound hierarchy and tracks. Each author symbol now contains a u32 ID count followed by that many u32 IDs in compiler order. Other fields retain the v1 framing and ordering.

Two local release runs per named fixture produced these SHA-256 digests:

| Fixture | SHA-256 |
| --- | --- |
| static | 5B1DEF67DE3DAC8DA2445B3E72CB71928274611A2B55393ACDB7AF7E49773E36 |
| image | 25C40B62D981A3AF77E489E6EAF2C7E394DD94713452CAC60F46510748538AF0 |
| groups | C7C630D7F28B97558D71FA51FC676F21171F858BEE1D193E07DD7C9B654B891D |
| animated-shapes | 3C7D6CB8FEEC5757E5203CDE337BB2FDB3489547D42FE4477745FF7DEF775406 |

The omitted-name v1 fixture also reproduced its documented digest. An unknown fixture name exited with code 2.
