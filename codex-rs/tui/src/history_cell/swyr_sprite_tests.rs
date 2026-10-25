use super::*;

#[test]
fn original_website_pixels_and_shadow_are_preserved() {
    let original = source_frame(Duration::ZERO, false);
    assert_eq!(original.dimensions(), (236, 206));
    // Independently computed from swyrbot.js's original canvas draw loop,
    // including transparent pixels, 5px body cells and 6px hard shadow.
    let hash = original
        .as_raw()
        .iter()
        .fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    assert_eq!(hash, 0x1d4926804e36aac9);
    if let Some(directory) = std::env::var_os("CODEX_SWYR_REVIEW_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).expect("create explicit review artifact directory");
        original
            .save(directory.join("runtime-source.png"))
            .expect("export runtime source frame");
        image_at(Duration::ZERO, false, 120, 100)
            .save(directory.join("runtime-compact.png"))
            .expect("export runtime compact frame");
        image_at(Duration::from_millis(2_000), true, 120, 100)
            .save(directory.join("runtime-wave.png"))
            .expect("export runtime wave frame");
    }
}

#[test]
fn reduced_motion_renders_the_original_raster_without_animation() {
    let still = source_frame(Duration::ZERO, false);
    for milliseconds in [2_000, 13_500, 26_000] {
        assert_eq!(
            source_frame(Duration::from_millis(milliseconds), false),
            still
        );
        assert_ne!(
            source_frame(Duration::from_millis(milliseconds), true),
            still
        );
    }
}

#[test]
fn native_images_preserve_palette_transparency_and_fixed_footprint() {
    for (width, height) in [(120, 100), (144, 120), (96, 100)] {
        for milliseconds in (0..28_000).step_by(125) {
            let image = image_at(Duration::from_millis(milliseconds), true, width, height);
            assert_eq!(image.dimensions(), (width, height));
            assert!(image.pixels().all(|pixel| matches!(
                pixel.0,
                [0, 0, 0, 0] | [15, 61, 26, 255] | [63, 185, 80, 255]
            )));
            assert_eq!(image.get_pixel(0, 0).0, [0, 0, 0, 0]);
        }
    }
}

#[test]
fn text_only_terminal_layout_never_renders_lossy_glyphs() {
    for width in [40, 64, 80, 110, 120] {
        assert_eq!(dimensions(width), None);
    }
}
