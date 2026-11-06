//! Sawyer's original website swyrbot, rendered as native terminal image pixels.
//!
//! Bitmap and idle/wave/dance transforms are ported from swyr.com_V3-redesign's
//! public/js/swyrbot.js, commit 7d3d977. The negative-space S is the face.
//! The original bitmap is never reduced to character cells. Raster placement
//! uses transparent letterboxing and nearest-neighbor scaling after animation.

use image::Rgba;
use image::RgbaImage;
use std::sync::OnceLock;
use std::time::Duration;

const WIDTH: usize = 46;
const HEIGHT: usize = 40;
const BASE: &str = "8.4.18.4.8/8.4.18.4.8/8.4.18.4.8/8.4.18.4.8/12.4.10.4.12/12.4.10.4.12/12.4.10.4.12/12.4.10.4.12/8.26.8/8.26.8/8.26.8/8.26.8/4.11.12.11.4/4.11.12.11.4/4.9.16.9.4/4.9.4.8.4.9.4/0.13.4.8.4.13/0.13.4.25/0.13.4.25/0.13.14.15/0.15.12.15/0.15.14.13/0.25.4.13/0.25.4.13/0.4.4.5.4.8.4.5.4.4/0.4.4.5.4.8.4.5.4.4/0.4.4.5.16.5.4.4/0.4.4.7.12.7.4.4/0.4.4.7.12.7.4.4/0.4.4.26.4.4/0.4.4.26.4.4/0.4.4.26.4.4/12.4.10.4.12/12.4.10.4.12/12.4.10.4.12/12.4.10.4.12/8.4.18.4.8/8.4.18.4.8/8.4.18.4.8/8.4.18.4.8";
type Grid = [bool; WIDTH * HEIGHT];

pub(crate) const FRAME_INTERVAL: Duration = Duration::from_millis(125);

/// Reserve a compact image slot only in terminals with native graphics support.
pub(crate) fn dimensions(terminal_width: u16) -> Option<(u16, u16)> {
    // Twelve image columns plus five spacing columns leave fifteen for header text.
    (terminal_width >= 32 && image_protocol().is_some()).then_some((12, 5))
}

pub(crate) fn image_protocol() -> Option<crate::pets::ImageProtocol> {
    // Sixel erasure clears text cells, which could now belong to newer history
    // after scrolling. Only protocols with independent image deletion are safe.
    // Tests and text snapshots must not depend on the developer's terminal.
    #[cfg(test)]
    {
        None
    }
    #[cfg(not(test))]
    {
        match crate::pets::detect_pet_image_support().protocol() {
            Some(
                protocol @ (crate::pets::ImageProtocol::Kitty
                | crate::pets::ImageProtocol::KittyLocalFile),
            ) => Some(protocol),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Part {
    Upper,
    LeftArm,
    RightArm,
    TopRight,
}

fn move_part(grid: &mut Grid, part: Part, dy: isize, dx: isize) {
    let original = *grid;
    let includes = |row: usize, col: usize| match part {
        Part::Upper => row <= 31,
        Part::LeftArm => (16..=31).contains(&row) && col <= 5,
        Part::RightArm => (16..=31).contains(&row) && col >= 40,
        Part::TopRight => row <= 11 && col >= 37,
    };
    for row in 0..HEIGHT {
        for col in 0..WIDTH {
            if original[row * WIDTH + col] && includes(row, col) {
                grid[row * WIDTH + col] = false;
            }
        }
    }
    for row in 0..HEIGHT {
        for col in 0..WIDTH {
            if original[row * WIDTH + col] && includes(row, col) {
                let target_row = row as isize + dy;
                let target_col = col as isize + dx;
                if (0..HEIGHT as isize).contains(&target_row)
                    && (0..WIDTH as isize).contains(&target_col)
                {
                    grid[target_row as usize * WIDTH + target_col as usize] = true;
                }
            }
        }
    }
}

fn shear(grid: &mut Grid, first: usize, last: usize, dx: isize) {
    let original = *grid;
    for row in first..=last {
        grid[row * WIDTH..(row + 1) * WIDTH].fill(false);
        for col in 0..WIDTH {
            let target = col as isize + dx;
            if original[row * WIDTH + col] && (0..WIDTH as isize).contains(&target) {
                grid[row * WIDTH + target as usize] = true;
            }
        }
    }
}

fn frame(elapsed: Duration, animations_enabled: bool) -> Grid {
    static BASE_GRID: OnceLock<Grid> = OnceLock::new();
    let mut grid = *BASE_GRID.get_or_init(|| {
        let mut grid = [false; WIDTH * HEIGHT];
        for (row, encoded) in BASE.split('/').enumerate() {
            let mut col = 2;
            for (run, count) in encoded.split('.').enumerate() {
                let count = count.parse::<usize>().unwrap_or_default();
                if run % 2 == 1 {
                    grid[row * WIDTH + col..row * WIDTH + col + count].fill(true);
                }
                col += count;
            }
        }
        grid
    });
    if !animations_enabled {
        return grid;
    }
    let seconds = elapsed.as_secs_f64();
    let mut idle_seconds = seconds;
    if seconds >= 1.5 {
        let cycle = ((seconds - 1.5) / 12.0).floor() as u64;
        let phase = (seconds - 1.5) % 12.0;
        if cycle.is_multiple_of(2) && phase < 1.25 {
            let index = (phase * 8.0).floor() as usize;
            let rises = [0, 4, 8, 12, 12, 12, 12, 12, 8, 4];
            move_part(&mut grid, Part::RightArm, -rises[index], /*dx*/ 0);
            if (4..=7).contains(&index) {
                move_part(
                    &mut grid,
                    Part::TopRight,
                    /*dy*/ 0,
                    if index.is_multiple_of(2) { 2 } else { -2 },
                );
            }
            return grid;
        }
        if cycle % 2 == 1 && phase < 8.0 / 6.0 {
            let index = (phase * 6.0).floor() as usize % 4;
            if index == 0 || index == 2 {
                let direction = if index == 0 { -1 } else { 1 };
                move_part(&mut grid, Part::LeftArm, direction * 4, /*dx*/ 0);
                move_part(&mut grid, Part::RightArm, -direction * 4, /*dx*/ 0);
                shear(&mut grid, /*first*/ 0, /*last*/ 11, direction * 2);
                shear(&mut grid, /*first*/ 12, /*last*/ 19, direction);
            }
            return grid;
        }
        idle_seconds = phase
            - if cycle.is_multiple_of(2) {
                1.25
            } else {
                8.0 / 6.0
            };
    }
    if idle_seconds % 3.0 >= 1.5 {
        move_part(&mut grid, Part::Upper, /*dy*/ 1, /*dx*/ 0);
    }
    grid
}

/// The exact website canvas: five pixels per original cell and six-pixel shadow.
pub(crate) fn source_frame(elapsed: Duration, animations_enabled: bool) -> RgbaImage {
    let grid = frame(elapsed, animations_enabled);
    let mut raster = RgbaImage::new(236, 206);
    for (offset, color) in [(6, Rgba([15, 61, 26, 255])), (0, Rgba([63, 185, 80, 255]))] {
        for row in 0..HEIGHT {
            for col in 0..WIDTH {
                if grid[row * WIDTH + col] {
                    for y in 0..5 {
                        for x in 0..5 {
                            raster.put_pixel(
                                col as u32 * 5 + x + offset,
                                row as u32 * 5 + y + offset,
                                color,
                            );
                        }
                    }
                }
            }
        }
    }
    raster
}

/// Fit native pixels to the physical cell area without stretching the artwork.
pub(crate) fn image_at(
    elapsed: Duration,
    animations_enabled: bool,
    canvas_width: u32,
    canvas_height: u32,
) -> RgbaImage {
    let source = source_frame(elapsed, animations_enabled);
    let canvas_width = canvas_width.max(1);
    let canvas_height = canvas_height.max(1);
    let scale = (f64::from(canvas_width) / f64::from(source.width()))
        .min(f64::from(canvas_height) / f64::from(source.height()));
    let width = (f64::from(source.width()) * scale).round().max(1.0) as u32;
    let height = (f64::from(source.height()) * scale).round().max(1.0) as u32;
    let fitted =
        image::imageops::resize(&source, width, height, image::imageops::FilterType::Nearest);
    let mut canvas = RgbaImage::new(canvas_width, canvas_height);
    image::imageops::overlay(
        &mut canvas,
        &fitted,
        i64::from((canvas_width - width) / 2),
        i64::from((canvas_height - height) / 2),
    );
    canvas
}

#[cfg(test)]
#[path = "swyr_sprite_tests.rs"]
mod tests;
