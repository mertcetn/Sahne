//! Internal flat compilation result stream. No source or compressed image bytes.
use super::{RgbaImage, pixel_bytes, validate_assets};
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
};
use wallpaper_scene::{MAX_SCENE_BYTES, ValidatedScene};

const SIGNATURE: &[u8; 4] = b"SFO\x01";

pub struct FlatOutput {
    pub scene: ValidatedScene,
    pub images: BTreeMap<u32, RgbaImage>,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

pub fn write(
    mut writer: impl Write,
    scene: &ValidatedScene,
    images: &BTreeMap<u32, RgbaImage>,
) -> io::Result<()> {
    validate_assets(scene, images).map_err(|_| invalid("Invalid flat output assets"))?;
    let encoded = scene.encode();
    if encoded.len() > MAX_SCENE_BYTES {
        return Err(invalid("Flat output scene limit exceeded"));
    }
    writer.write_all(SIGNATURE)?;
    writer.write_all(&(encoded.len() as u32).to_le_bytes())?;
    writer.write_all(&encoded)?;
    for resource in &scene.scene().resources {
        writer.write_all(&resource.id.to_le_bytes())?;
        writer.write_all(images[&resource.id].pixels())?;
    }
    Ok(())
}

/// Read one result. The caller owns the exchange deadline and trailing data checks.
pub fn read(mut reader: impl Read) -> io::Result<FlatOutput> {
    if read_u32(&mut reader)?.to_le_bytes() != *SIGNATURE {
        return Err(invalid("Unknown flat output format"));
    }
    let length = read_u32(&mut reader)? as usize;
    if length > MAX_SCENE_BYTES {
        return Err(invalid("Flat output scene limit exceeded"));
    }
    let mut encoded = vec![0; length];
    reader.read_exact(&mut encoded)?;
    let scene =
        wallpaper_scene::decode(&encoded).map_err(|_| invalid("Invalid flat output scene"))?;
    let mut images = BTreeMap::new();
    for resource in &scene.scene().resources {
        if read_u32(&mut reader)? != resource.id {
            return Err(invalid("Invalid flat output image ID"));
        }
        // The scene decoder checks dimensions and the aggregate pixel budget
        // before any pixel buffers are allocated.
        let size = pixel_bytes(resource.width, resource.height)
            .map_err(|_| invalid("Invalid flat output image dimensions"))?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(size)
            .map_err(|_| io::Error::other("Could not allocate flat output pixels"))?;
        pixels.resize(size, 0);
        reader.read_exact(&mut pixels)?;
        let image = RgbaImage::from_rgba8(resource.width, resource.height, pixels)
            .map_err(|_| invalid("Invalid flat output pixels"))?;
        images.insert(resource.id, image);
    }
    validate_assets(&scene, &images).map_err(|_| invalid("Invalid flat output assets"))?;
    Ok(FlatOutput { scene, images })
}
