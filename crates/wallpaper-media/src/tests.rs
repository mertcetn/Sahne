use super::*;
use wallpaper_scene::{Color, ImageResource, SCENE_VERSION, Scene};

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend(kind);
    out.extend(data);
    out.extend(crc32fast::hash(&out[4..]).to_be_bytes());
    out
}
fn ihdr(width: u32, height: u32, mode: [u8; 5]) -> Vec<u8> {
    let mut data = width.to_be_bytes().to_vec();
    data.extend(height.to_be_bytes());
    data.extend(mode);
    chunk(b"IHDR", &data)
}
fn encoded() -> Vec<u8> {
    let mut output = vec![];
    {
        let mut encoder = png::Encoder::new(&mut output, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[32, 160, 128, 255, 230, 64, 96, 128])
            .unwrap();
        writer.finish().unwrap();
    }
    output
}
fn container(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    for chunk in chunks {
        bytes.extend(chunk);
    }
    bytes
}
fn scene(resources: Vec<ImageResource>) -> ValidatedScene {
    Scene {
        version: SCENE_VERSION,
        width: 10,
        height: 10,
        background: Color([0; 4]),
        resources,
        nodes: vec![],
    }
    .validate()
    .unwrap()
}

#[test]
fn exact_pixels_and_straight_alpha() {
    let image = decode_png(&encoded(), 8).unwrap();
    assert_eq!((image.width(), image.height()), (2, 1));
    assert_eq!(image.pixels(), [32, 160, 128, 255, 230, 64, 96, 128]);
    let mut tagged = encoded();
    tagged.splice(33..33, chunk(b"sRGB", &[0]));
    assert_eq!(decode_png(&tagged, 8).unwrap(), image);
}

#[test]
fn dimensions_and_budgets_precede_decode() {
    for (w, h) in [(0, 1), (1, 0), (4097, 1), (1, 4097), (u32::MAX, u32::MAX)] {
        let bytes = container(&[ihdr(w, h, [8, 6, 0, 0, 0])]);
        assert_eq!(decode_png(&bytes, u64::MAX), Err(Error::InvalidDimensions));
    }
    assert_eq!(decode_png(&encoded(), 7), Err(Error::BudgetExceeded));
    let max = container(&[ihdr(4096, 4096, [8, 6, 0, 0, 0])]);
    assert_eq!(
        decode_png(&max, 64 * 1024 * 1024 - 1),
        Err(Error::BudgetExceeded)
    );
    assert_eq!(
        decode_png(&vec![0; MAX_ENCODED_IMAGE_BYTES + 1], 8),
        Err(Error::BudgetExceeded)
    );
}

#[test]
fn reject_other_pixel_modes_and_interlace() {
    for mode in [
        [16, 6, 0, 0, 0],
        [8, 2, 0, 0, 0],
        [8, 3, 0, 0, 0],
        [8, 6, 1, 0, 0],
        [8, 6, 0, 1, 0],
        [8, 6, 0, 0, 1],
    ] {
        assert_eq!(
            decode_png(&container(&[ihdr(1, 1, mode)]), 4),
            Err(Error::UnsupportedPng)
        );
    }
}

#[test]
fn crc_and_truncation_cover_whole_container() {
    let bytes = encoded();
    for end in 0..bytes.len() {
        assert!(decode_png(&bytes[..end], 8).is_err(), "{end}");
    }
    for index in 0..bytes.len() {
        let mut changed = bytes.clone();
        changed[index] ^= 1;
        assert!(decode_png(&changed, 8).is_err(), "{index}");
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(decode_png(&trailing, 8), Err(Error::InvalidPng));
}

#[test]
fn reject_metadata_animation_and_bad_order() {
    for (kind, data) in [
        (b"acTL", vec![0; 8]),
        (b"tEXt", b"key\0value".to_vec()),
        (b"gAMA", vec![0; 4]),
        (b"iCCP", vec![0; 8]),
        (b"cICP", vec![0; 4]),
    ] {
        let mut bytes = encoded();
        bytes.splice(33..33, chunk(kind, &data));
        assert_eq!(decode_png(&bytes, 8), Err(Error::UnsupportedPng));
    }
    let mut duplicate = encoded();
    duplicate.splice(33..33, ihdr(2, 1, [8, 6, 0, 0, 0]));
    assert_eq!(decode_png(&duplicate, 8), Err(Error::InvalidPng));
    for data in [vec![], vec![4], vec![0, 0]] {
        let mut bytes = encoded();
        bytes.splice(33..33, chunk(b"sRGB", &data));
        assert_eq!(decode_png(&bytes, 8), Err(Error::InvalidPng));
    }
    let mut after_data = encoded();
    let end = after_data.len() - 12;
    after_data.splice(end..end, chunk(b"sRGB", &[0]));
    assert_eq!(decode_png(&after_data, 8), Err(Error::InvalidPng));
    assert_eq!(
        decode_png(&container(&[chunk(b"IDAT", &[])]), 8),
        Err(Error::InvalidPng)
    );
    assert_eq!(
        decode_png(
            &container(&[ihdr(2, 1, [8, 6, 0, 0, 0]), chunk(b"IEND", &[])]),
            8
        ),
        Err(Error::InvalidPng)
    );
}

#[test]
fn chunk_budget_and_adversarial_length() {
    let mut chunks = vec![ihdr(2, 1, [8, 6, 0, 0, 0])];
    chunks.extend((0..MAX_PNG_CHUNKS).map(|_| chunk(b"IDAT", &[])));
    chunks.push(chunk(b"IEND", &[]));
    assert_eq!(
        decode_png(&container(&chunks), 8),
        Err(Error::BudgetExceeded)
    );
    let mut bytes = encoded();
    bytes[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(decode_png(&bytes, 8), Err(Error::InvalidPng));
}

#[test]
fn invalid_compressed_data_with_valid_crc_is_rejected() {
    let bytes = container(&[
        ihdr(2, 1, [8, 6, 0, 0, 0]),
        chunk(b"IDAT", &[0, 0, 0, 0]),
        chunk(b"IEND", &[]),
    ]);
    assert_eq!(decode_png(&bytes, 8), Err(Error::Decode));
}

#[test]
fn adjacent_idat_chunks_preserve_pixels() {
    let original = encoded();
    // Encoder's first data chunk follows the 33-byte signature/header.
    assert_eq!(&original[37..41], b"IDAT");
    let len = u32::from_be_bytes(original[33..37].try_into().unwrap()) as usize;
    let data = &original[41..41 + len];
    let split = container(&[
        ihdr(2, 1, [8, 6, 0, 0, 0]),
        chunk(b"IDAT", &data[..3]),
        chunk(b"IDAT", &data[3..]),
        chunk(b"IEND", &[]),
    ]);
    assert_eq!(
        decode_png(&original, 8).unwrap(),
        decode_png(&split, 8).unwrap()
    );
}

#[test]
fn raw_pixels_and_exact_scene_asset_matching() {
    assert_eq!(
        RgbaImage::from_rgba8(0, 1, vec![]),
        Err(Error::InvalidDimensions)
    );
    assert_eq!(
        RgbaImage::from_rgba8(1, 1, vec![0; 3]),
        Err(Error::InvalidPixels)
    );
    let declared = scene(vec![ImageResource {
        id: 7,
        width: 2,
        height: 1,
    }]);
    let mut images = BTreeMap::new();
    assert_eq!(
        validate_assets(&declared, &images),
        Err(Error::AssetMismatch)
    );
    images.insert(8, decode_png(&encoded(), 8).unwrap());
    assert_eq!(
        validate_assets(&declared, &images),
        Err(Error::AssetMismatch)
    );
    images.clear();
    images.insert(7, RgbaImage::from_rgba8(1, 2, vec![0; 8]).unwrap());
    assert_eq!(
        validate_assets(&declared, &images),
        Err(Error::AssetMismatch)
    );
    images.insert(7, decode_png(&encoded(), 8).unwrap());
    validate_assets(&declared, &images).unwrap();
    validate_assets(
        &wallpaper_scene::decode(&declared.encode()).unwrap(),
        &images,
    )
    .unwrap();
    images.insert(9, RgbaImage::from_rgba8(1, 1, vec![0; 4]).unwrap());
    assert_eq!(
        validate_assets(&declared, &images),
        Err(Error::AssetMismatch)
    );
    validate_assets(&scene(vec![]), &BTreeMap::new()).unwrap();
}

#[test]
fn hierarchy_assets_are_checked_against_independently_decoded_v2_metadata() {
    use wallpaper_scene::hierarchy::{Hierarchy, VERSION};
    let raw = Hierarchy {
        version: VERSION,
        width: 10,
        height: 10,
        background: Color([0; 4]),
        resources: vec![ImageResource {
            id: 7,
            width: 2,
            height: 1,
        }],
        layers: vec![],
    };
    let declared = wallpaper_scene::hierarchy::decode(&raw.validate().unwrap().encode()).unwrap();
    let mut images = BTreeMap::new();
    assert_eq!(
        validate_hierarchy_assets(&declared, &images),
        Err(Error::AssetMismatch)
    );
    images.insert(8, decode_png(&encoded(), 8).unwrap());
    assert_eq!(
        validate_hierarchy_assets(&declared, &images),
        Err(Error::AssetMismatch)
    );
    images.clear();
    images.insert(7, RgbaImage::from_rgba8(1, 2, vec![0; 8]).unwrap());
    assert_eq!(
        validate_hierarchy_assets(&declared, &images),
        Err(Error::AssetMismatch)
    );
    images.insert(7, decode_png(&encoded(), 8).unwrap());
    validate_hierarchy_assets(&declared, &images).unwrap();
    images.insert(9, RgbaImage::from_rgba8(1, 1, vec![0; 4]).unwrap());
    assert_eq!(
        validate_hierarchy_assets(&declared, &images),
        Err(Error::AssetMismatch)
    );
}
