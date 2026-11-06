//! Locate the website mascot only while its whole header is visible in native history.

use super::App;
use crate::history_cell::SessionInfoCell;
use crate::insert_history::wrap_history_hyperlink_lines;
use crate::tui::Tui;
use color_eyre::eyre::Result;
use ratatui::layout::Size;

impl App {
    pub(super) fn render_startup_sprite(&self, tui: &mut Tui, screen_size: Size) -> Result<()> {
        if self.overlay.is_some()
            || (tui.is_owned_screen() && self.transcript_view.is_detailed())
            || self.chat_widget.has_active_view()
            || self.chat_widget.raw_output_mode()
        {
            return tui.clear_startup_sprite().map_err(Into::into);
        }
        if tui.is_owned_screen() {
            return match self
                .transcript_view
                .startup_sprite_area(&self.transcript_cells)
            {
                Some(area) => tui
                    .draw_startup_sprite(area, self.local_settings.tui.animations)
                    .map_err(Into::into),
                None => tui.clear_startup_sprite().map_err(Into::into),
            };
        }
        let width = self.chat_widget.history_wrap_width(screen_size.width);
        let Some(mut area) = SessionInfoCell::sprite_area(width) else {
            return tui.clear_startup_sprite().map_err(Into::into);
        };
        let visible_rows = usize::from(tui.terminal.visible_history_rows());
        let mut rendered_rows = 0usize;
        let mut header_offset = None;
        for cell in &self.transcript_cells {
            let lines = cell
                .display_hyperlink_lines_for_mode(width, self.chat_widget.history_render_mode());
            // Native terminal wrapping can consume more rows than the cell's logical lines.
            // Match insertion so an offscreen header cannot redraw over newer history.
            let (_, rows) = wrap_history_hyperlink_lines(
                &lines,
                usize::from(screen_size.width.max(1)),
                self.history_line_wrap_policy(),
            );
            if rows == 0 {
                continue;
            }
            if rendered_rows > 0 && !cell.is_stream_continuation() {
                rendered_rows += 1;
            }
            if header_offset.is_none() && cell.as_any().is::<SessionInfoCell>() {
                header_offset = Some(rendered_rows);
            }
            rendered_rows = rendered_rows.saturating_add(rows);
            if rendered_rows > visible_rows {
                return tui.clear_startup_sprite().map_err(Into::into);
            }
        }
        if let Some(offset) = header_offset {
            area.y = area.y.saturating_add(offset as u16);
            tui.draw_startup_sprite(area, self.local_settings.tui.animations)?;
        } else {
            tui.clear_startup_sprite()?;
        }
        Ok(())
    }
}
