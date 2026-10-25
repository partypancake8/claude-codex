//! Use otherwise empty window rows without moving finalized history into a UI buffer.
//!
//! Before history insertion, shrink the live viewport by the pending row count. The
//! existing insertion strategy then advances it through that freed space, keeping
//! short conversations at the top and the composer at the bottom. Once the screen
//! fills, native scrollback handles overflow as before.

use super::PendingHistoryLines;
use super::Tui;
use super::scrollback::ScrollbackStrategy;
use crate::custom_terminal::Terminal;
use crate::insert_history::wrap_history_hyperlink_lines;
use ratatui::backend::Backend;
use ratatui::layout::Size;
use std::io;
use std::io::Write;

pub(super) fn prepare_filled_inline_viewport<B>(
    terminal: &mut Terminal<B>,
    pending: &[PendingHistoryLines],
    minimum_height: u16,
    screen_size: Size,
    scrollback: ScrollbackStrategy,
) -> io::Result<bool>
where
    B: Backend<Error = io::Error> + Write,
{
    let pending_rows = pending.iter().fold(0usize, |rows, batch| {
        let (_, batch_rows) = wrap_history_hyperlink_lines(
            &batch.lines,
            usize::from(screen_size.width.max(1)),
            batch.wrap_policy,
        );
        rows.saturating_add(batch_rows)
    });
    let history_rows = usize::from(terminal.visible_history_rows()).saturating_add(pending_rows);
    let free_rows = usize::from(screen_size.height).saturating_sub(history_rows) as u16;
    let height = minimum_height.max(free_rows).min(screen_size.height);
    Tui::update_inline_viewport_for_resize_reflow(terminal, height, screen_size, scrollback)
}

#[cfg(test)]
#[path = "fill_window_tests.rs"]
mod tests;
