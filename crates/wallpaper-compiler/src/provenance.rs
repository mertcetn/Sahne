use crate::{CompiledGroups, Viewport, compile_groups};
use std::io::{self, Write};
use wallpaper_format::Package;

pub fn write_fixture(mut output: impl Write) -> Result<(), Box<dyn std::error::Error>> {
    let package = crate::fixtures::groups_package()?;
    let compiled = compile_groups(
        &package,
        Viewport {
            width: 1920,
            height: 1080,
            background: wallpaper_scene::Color([12, 17, 22, 255]),
        },
    )?;
    let scene = compiled.scene().encode();
    let decoded = wallpaper_scene::hierarchy::decode(&scene)?;
    wallpaper_media::validate_hierarchy_assets(&decoded, compiled.images())?;
    write_record(&mut output, &package, &compiled, &scene)?;
    output.flush()?;
    Ok(())
}

fn write_record(
    mut output: impl Write,
    package: &Package,
    compiled: &CompiledGroups,
    scene: &[u8],
) -> io::Result<()> {
    output.write_all(b"SAHNPRV\0\x01\0")?;
    field(&mut output, env!("CARGO_PKG_VERSION").as_bytes())?;
    count(&mut output, package.files().len())?;
    for file in package.files() {
        field(&mut output, file.path().as_str().as_bytes())?;
        field(&mut output, file.bytes())?;
    }
    field(&mut output, scene)?;
    count(&mut output, compiled.symbols().len())?;
    for (name, id) in compiled.symbols() {
        field(&mut output, name.as_bytes())?;
        output.write_all(&id.to_le_bytes())?;
    }
    field(
        &mut output,
        compiled
            .script()
            .map_or(b"".as_slice(), |path| path.as_str().as_bytes()),
    )?;
    count(&mut output, compiled.images().len())?;
    for (id, image) in compiled.images() {
        output.write_all(&id.to_le_bytes())?;
        output.write_all(&image.width().to_le_bytes())?;
        output.write_all(&image.height().to_le_bytes())?;
        field(&mut output, image.pixels())?;
    }
    Ok(())
}

fn count(output: &mut impl Write, value: usize) -> io::Result<()> {
    let value = u32::try_from(value).map_err(io::Error::other)?;
    output.write_all(&value.to_le_bytes())
}

fn field(output: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    count(output, bytes.len())?;
    output.write_all(bytes)
}
