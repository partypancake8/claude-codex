//! Draw the original website raster in native terminal graphics, preserving the input cursor.

use super::Tui;
use crate::history_cell::swyr_sprite;
use crate::pets::AmbientPetDraw;
use crate::pets::PetImageRenderState;
use crossterm::SynchronizedUpdate;
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::io;
use std::time::Instant;

#[derive(Default)]
pub(super) struct StartupSpriteState {
    started: Option<Instant>,
    images: PetImageRenderState,
    cache: Option<tempfile::TempDir>,
}

impl Tui {
    pub(crate) fn clear_startup_sprite(&mut self) -> io::Result<()> {
        crate::pets::render_startup_sprite_image(
            self.terminal.backend_mut(),
            &mut self.startup_sprite_state.images,
            None,
        )
        .map_err(io::Error::other)
    }

    pub(crate) fn draw_startup_sprite(&mut self, area: Rect, animate: bool) -> io::Result<()> {
        let Some(protocol) = swyr_sprite::image_protocol() else {
            return self.clear_startup_sprite();
        };
        if area.is_empty()
            || area.bottom() > self.terminal.viewport_area.top()
            || area.right() > self.terminal.last_known_screen_size.width
        {
            return self.clear_startup_sprite();
        }
        // Use physical cell dimensions when the terminal reports them. The fallback
        // matches the pet renderer's cell aspect; letterboxing keeps the S unstretched.
        let pixel_size = self.terminal.backend_mut().window_size().ok();
        let (cell_width, cell_height) = pixel_size
            .filter(|size| {
                size.pixels.width > 0
                    && size.pixels.height > 0
                    && size.columns_rows.width > 0
                    && size.columns_rows.height > 0
            })
            .map(|size| {
                (
                    f64::from(size.pixels.width) / f64::from(size.columns_rows.width),
                    f64::from(size.pixels.height) / f64::from(size.columns_rows.height),
                )
            })
            .unwrap_or((10.4, 20.0));
        let width = (f64::from(area.width) * cell_width).round().max(1.0) as u32;
        let height = (f64::from(area.height) * cell_height).round().max(1.0) as u32;
        let elapsed = self
            .startup_sprite_state
            .started
            .get_or_insert_with(Instant::now)
            .elapsed();
        let image = swyr_sprite::image_at(elapsed, animate, width, height);
        let mut key = DefaultHasher::new();
        image.dimensions().hash(&mut key);
        image.as_raw().hash(&mut key);
        if self.startup_sprite_state.cache.is_none() {
            self.startup_sprite_state.cache =
                Some(tempfile::Builder::new().prefix("codex-swyr-").tempdir()?);
        }
        let Some(cache) = self.startup_sprite_state.cache.as_ref() else {
            return Ok(());
        };
        let path = cache.path().join(format!("{:016x}.png", key.finish()));
        if !path.exists() {
            image.save(&path).map_err(io::Error::other)?;
        }
        let request = AmbientPetDraw {
            frame: path,
            protocol,
            x: area.x,
            y: area.y,
            clear_top_y: area.y,
            columns: area.width,
            rows: area.height,
            height_px: height.min(u32::from(u16::MAX)) as u16,
            sixel_dir: cache.path().to_path_buf(),
        };
        let writer = self.terminal.backend_mut();
        let state = &mut self.startup_sprite_state.images;
        io::stdout().sync_update(|_| {
            crate::pets::render_startup_sprite_image(writer, state, Some(request))
                .map_err(io::Error::other)
        })??;
        if animate && self.is_terminal_focused() {
            self.frame_requester()
                .schedule_frame_in(swyr_sprite::FRAME_INTERVAL);
        }
        Ok(())
    }
}
