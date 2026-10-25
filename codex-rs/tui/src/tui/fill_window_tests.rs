use super::prepare_filled_inline_viewport;
use crate::custom_terminal::Terminal;
use crate::insert_history::HistoryLineWrapPolicy;
use crate::terminal_hyperlinks::plain_hyperlink_lines;
use crate::test_backend::VT100Backend;
use crate::tui::PendingHistoryLines;
use crate::tui::Tui;
use crate::tui::scrollback::ScrollbackStrategy;
use color_eyre::eyre::Result;
use pretty_assertions::assert_eq;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::style::Style;
use ratatui::text::Line;

struct Window {
    terminal: Terminal<VT100Backend>,
    size: Size,
    strategy: ScrollbackStrategy,
}

impl Window {
    fn new(strategy: ScrollbackStrategy, width: u16, height: u16) -> Result<Self> {
        let backend = VT100Backend::with_scrollback(width, height, /*scrollback_len*/ 128);
        let mut terminal =
            Terminal::with_options_and_cursor_position(backend, Position { x: 0, y: 0 })?;
        terminal.set_viewport_area(Rect::new(0, 0, width, 0));
        Ok(Self {
            terminal,
            size: Size::new(width, height),
            strategy,
        })
    }

    /// Exercise the same prepare, native insertion, invalidation, and drawing order as Tui.
    fn draw(&mut self, history: &[&str], active: &[&str]) -> Result<()> {
        let mut pending = if history.is_empty() {
            Vec::new()
        } else {
            vec![PendingHistoryLines {
                lines: plain_hyperlink_lines(
                    history
                        .iter()
                        .map(|text| Line::from((*text).to_string()))
                        .collect(),
                ),
                wrap_policy: HistoryLineWrapPolicy::PreWrap,
            }]
        };
        let minimum_height = active.len() as u16 + 2;
        let repaint = prepare_filled_inline_viewport(
            &mut self.terminal,
            &pending,
            minimum_height,
            self.size,
            self.strategy,
        )?;
        let overlap = !pending.is_empty() && self.terminal.viewport_area.top() <= 1;
        Tui::flush_pending_history_lines(
            &mut self.terminal,
            &mut pending,
            self.strategy,
            self.size,
        )?;
        if repaint || overlap {
            self.terminal.invalidate_viewport();
        }
        self.terminal.draw_with_size(self.size, |frame| {
            let area = frame.area();
            for (index, text) in active.iter().enumerate() {
                if index as u16 >= area.height.saturating_sub(2) {
                    break;
                }
                frame
                    .buffer
                    .set_string(area.x, area.y + index as u16, text, Style::default());
            }
            frame
                .buffer
                .set_string(area.x, area.bottom() - 2, "PROMPT draft", Style::default());
            frame
                .buffer
                .set_string(area.x, area.bottom() - 1, "STATUS", Style::default());
            frame.set_cursor_position((area.x + 8, area.bottom() - 2));
        })?;
        Ok(())
    }

    fn rows(&self) -> Vec<String> {
        self.terminal
            .backend()
            .vt100()
            .screen()
            .rows(0, self.size.width)
            .collect()
    }
}

#[test]
fn filled_window_keeps_header_at_top_and_composer_at_bottom_across_first_commit() -> Result<()> {
    for strategy in [ScrollbackStrategy::Standard, ScrollbackStrategy::FullScreen] {
        let mut window = Window::new(strategy, /*width*/ 32, /*height*/ 12)?;
        let header = ["Codex", "model: test", "directory: /project"];
        window.draw(&[], &header)?;
        let rows = window.rows();
        assert_eq!(&rows[..3], &header);
        assert_eq!(&rows[10..], &["PROMPT draft", "STATUS"]);

        window.draw(&header, &[])?;
        let rows = window.rows();
        assert_eq!(&rows[..3], &header, "{strategy:?}");
        assert_eq!(&rows[10..], &["PROMPT draft", "STATUS"]);
        window.draw(&["User request", "Agent answer"], &[])?;
        let rows = window.rows();
        assert_eq!(
            &rows[..5],
            &[
                "Codex",
                "model: test",
                "directory: /project",
                "User request",
                "Agent answer"
            ]
        );
        assert_eq!(&rows[10..], &["PROMPT draft", "STATUS"]);
    }
    Ok(())
}

#[test]
fn filled_window_preserves_a_single_first_history_row() -> Result<()> {
    for strategy in [ScrollbackStrategy::Standard, ScrollbackStrategy::FullScreen] {
        let mut window = Window::new(strategy, /*width*/ 24, /*height*/ 8)?;
        window.draw(&[], &[])?;
        window.draw(&["first history"], &[])?;
        let rows = window.rows();
        assert_eq!(rows[0], "first history", "{strategy:?}");
        assert_eq!(&rows[6..], &["PROMPT draft", "STATUS"]);
        window.draw(&["second history"], &[])?;
        assert_eq!(&window.rows()[..2], &["first history", "second history"]);
        assert_eq!(&window.rows()[6..], &["PROMPT draft", "STATUS"]);
        assert_eq!(
            window.terminal.backend().vt100().screen().cursor_position(),
            (6, 8)
        );
    }
    Ok(())
}

#[test]
fn filled_window_overflow_keeps_recent_history_and_native_scrollback() -> Result<()> {
    for strategy in [ScrollbackStrategy::Standard, ScrollbackStrategy::FullScreen] {
        let mut window = Window::new(strategy, /*width*/ 32, /*height*/ 12)?;
        window.draw(&[], &[])?;
        let lines: Vec<_> = (0..25).map(|index| format!("history {index}")).collect();
        let lines_ref: Vec<_> = lines.iter().map(String::as_str).collect();
        window.draw(&lines_ref, &[])?;
        let rows = window.rows();
        assert_eq!(&rows[..10], &lines[15..], "{strategy:?}");
        assert_eq!(&rows[10..], &["PROMPT draft", "STATUS"]);
        let mut screen = window.terminal.backend().vt100().screen().clone();
        screen.set_scrollback(usize::MAX);
        // vt100 only captures scrollback when its scroll region spans the full screen
        // (grid::scroll_up); Standard uses partial regions whose visible rows are tested above.
        if strategy == ScrollbackStrategy::FullScreen {
            assert!(screen.contents().contains("history 0"));
            assert!(!screen.contents().contains("PROMPT draft"));
        }
    }
    Ok(())
}

#[test]
fn filled_window_active_growth_and_shrink_preserve_completed_history() -> Result<()> {
    for strategy in [ScrollbackStrategy::Standard, ScrollbackStrategy::FullScreen] {
        let mut window = Window::new(strategy, /*width*/ 32, /*height*/ 12)?;
        let history = ["header", "request", "answer"];
        window.draw(&history, &[])?;
        window.draw(&[], &["running 1", "running 2", "running 3", "running 4"])?;
        assert_eq!(&window.rows()[..3], &history, "{strategy:?}");
        assert_eq!(&window.rows()[10..], &["PROMPT draft", "STATUS"]);
        window.draw(&[], &[])?;
        assert_eq!(&window.rows()[..3], &history, "{strategy:?}");
        assert!(!window.rows().join("\n").contains("running"));
        assert_eq!(&window.rows()[10..], &["PROMPT draft", "STATUS"]);
    }
    Ok(())
}

#[test]
fn filled_window_large_active_body_scrolls_history_without_losing_it() -> Result<()> {
    for strategy in [ScrollbackStrategy::Standard, ScrollbackStrategy::FullScreen] {
        let mut window = Window::new(strategy, /*width*/ 32, /*height*/ 12)?;
        window.draw(&["header", "request", "answer"], &[])?;
        let active = [
            "work 1", "work 2", "work 3", "work 4", "work 5", "work 6", "work 7", "work 8",
            "work 9",
        ];
        window.draw(&[], &active)?;
        assert_eq!(window.rows()[0], "answer", "{strategy:?}");
        assert_eq!(&window.rows()[10..], &["PROMPT draft", "STATUS"]);
        let mut screen = window.terminal.backend().vt100().screen().clone();
        screen.set_scrollback(usize::MAX);
        // Standard's partial-region scrollback is not modeled by the vt100 backend.
        if strategy == ScrollbackStrategy::FullScreen {
            assert!(screen.contents().contains("header"));
            assert!(screen.contents().contains("request"));
        }
        window.draw(&[], &[])?;
        assert_eq!(window.rows()[0], "answer", "{strategy:?}");
        assert!(!window.rows().join("\n").contains("work "));
        assert_eq!(&window.rows()[10..], &["PROMPT draft", "STATUS"]);
    }
    Ok(())
}

#[test]
fn filled_window_reserves_wrapped_history_rows_before_insertion() -> Result<()> {
    for strategy in [ScrollbackStrategy::Standard, ScrollbackStrategy::FullScreen] {
        let mut window = Window::new(strategy, /*width*/ 16, /*height*/ 8)?;
        window.draw(&[], &[])?;
        window.draw(&["first", "abcdefghijklmnopabcdefghijklmnopabcd"], &[])?;
        let rows = window.rows();
        assert_eq!(
            &rows[..4],
            &["first", "abcdefghijklmnop", "abcdefghijklmnop", "abcd"],
            "{strategy:?}"
        );
        assert_eq!(&rows[6..], &["PROMPT draft", "STATUS"]);
    }
    Ok(())
}

#[test]
fn filled_window_resize_replay_restores_history_and_bottom_anchor() -> Result<()> {
    for strategy in [ScrollbackStrategy::Standard, ScrollbackStrategy::FullScreen] {
        let mut window = Window::new(strategy, /*width*/ 32, /*height*/ 12)?;
        let history: Vec<_> = (0..9).map(|index| format!("saved row {index}")).collect();
        let history_ref: Vec<_> = history.iter().map(String::as_str).collect();
        window.draw(&history_ref, &[])?;
        for size in [Size::new(16, 8), Size::new(40, 18), Size::new(24, 10)] {
            window
                .terminal
                .backend_mut()
                .resize(size.width, size.height);
            // Match App::clear_terminal_for_resize_replay before replaying canonical history.
            window.terminal.clear_scrollback_and_visible_screen_ansi()?;
            let mut area = window.terminal.viewport_area;
            area.y = 0;
            window.terminal.set_viewport_area(area);
            window.size = size;
            window.draw(&history_ref, &[])?;
            let rows = window.rows();
            let visible = history.len().min(usize::from(size.height - 2));
            assert_eq!(
                &rows[..visible],
                &history[history.len() - visible..],
                "{strategy:?} {size:?}"
            );
            assert_eq!(
                &rows[usize::from(size.height - 2)..],
                &["PROMPT draft", "STATUS"]
            );
            assert_eq!(window.terminal.viewport_area.bottom(), size.height);
        }
    }
    Ok(())
}
