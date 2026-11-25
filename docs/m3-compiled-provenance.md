# M3 compiled provenance fixture

The developer CLI can emit a binary provenance record for the embedded group fixture. It compiles the retained package, independently decodes the resulting v2 scene and validates its image assets before emitting the record. This checkpoint covers the fixed v2 group fixture. General package export, flat and animated records, and production verification remain open.

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
