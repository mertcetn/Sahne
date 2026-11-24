//! Original compile-time fixtures. Never reads an example directory at runtime.
use wallpaper_format::Package;

/// Original nested-opacity, anisotropic-scale and fractional/empty-clip sources.
pub fn effect_groups_package() -> wallpaper_format::Result<Package> {
    Package::from_entries(vec![
        (
            "manifest.json".into(),
            include_bytes!("../../../examples/group-effects/manifest.json").to_vec(),
        ),
        (
            "index.html".into(),
            include_bytes!("../../../examples/group-effects/index.html").to_vec(),
        ),
        (
            "style.css".into(),
            include_bytes!("../../../examples/group-effects/style.css").to_vec(),
        ),
        (
            "checker.png".into(),
            include_bytes!("../../../examples/group-effects/checker.png").to_vec(),
        ),
    ])
}

/// Separate overflow fixture for the renderer; preserves the original compiler fixture.
pub fn preview_groups_package() -> wallpaper_format::Result<Package> {
    Package::from_entries(vec![
        (
            "manifest.json".into(),
            include_bytes!("../../../examples/group-preview/manifest.json").to_vec(),
        ),
        (
            "index.html".into(),
            include_bytes!("../../../examples/group-preview/index.html").to_vec(),
        ),
        (
            "style.css".into(),
            include_bytes!("../../../examples/group-preview/style.css").to_vec(),
        ),
        (
            "checker.png".into(),
            include_bytes!("../../../examples/group-preview/checker.png").to_vec(),
        ),
    ])
}

pub fn groups_package() -> wallpaper_format::Result<Package> {
    Package::from_entries(vec![
        (
            "manifest.json".into(),
            include_bytes!("../../../examples/static-groups/manifest.json").to_vec(),
        ),
        (
            "index.html".into(),
            include_bytes!("../../../examples/static-groups/index.html").to_vec(),
        ),
        (
            "style.css".into(),
            include_bytes!("../../../examples/static-groups/style.css").to_vec(),
        ),
        (
            "checker.png".into(),
            include_bytes!("../../../examples/static-groups/checker.png").to_vec(),
        ),
    ])
}

pub fn image_package() -> wallpaper_format::Result<Package> {
    Package::from_entries(vec![
        (
            "manifest.json".into(),
            include_bytes!("../../../examples/static-image/manifest.json").to_vec(),
        ),
        (
            "index.html".into(),
            include_bytes!("../../../examples/static-image/index.html").to_vec(),
        ),
        (
            "style.css".into(),
            include_bytes!("../../../examples/static-image/style.css").to_vec(),
        ),
        (
            "checker.png".into(),
            include_bytes!("../../../examples/static-image/checker.png").to_vec(),
        ),
    ])
}

pub fn static_package() -> wallpaper_format::Result<Package> {
    Package::from_entries(vec![
        (
            "manifest.json".into(),
            include_bytes!("../../../examples/static-scene/manifest.json").to_vec(),
        ),
        (
            "index.html".into(),
            include_bytes!("../../../examples/static-scene/index.html").to_vec(),
        ),
        (
            "style.css".into(),
            include_bytes!("../../../examples/static-scene/style.css").to_vec(),
        ),
    ])
}
