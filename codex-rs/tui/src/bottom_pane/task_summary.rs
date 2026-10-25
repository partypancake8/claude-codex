//! Compact, bounded presentation of child-thread counts supplied by the app.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;

use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::render::renderable::Renderable;

#[derive(Default, PartialEq, Eq)]
pub(super) struct TaskSummary {
    total: usize,
    working: usize,
}

impl TaskSummary {
    pub(super) fn is_empty(&self) -> bool {
        self.total == 0
    }

    pub(super) fn update(&mut self, total: usize, working: usize) -> bool {
        let next = Self { total, working };
        if *self == next {
            return false;
        }
        *self = next;
        true
    }

    pub(super) fn with_navigation<'a>(
        &'a self,
        navigation: Option<Line<'static>>,
        shortcut: &'a str,
    ) -> TaskSummaryRow<'a> {
        TaskSummaryRow {
            summary: self,
            navigation,
            shortcut,
        }
    }
}

pub(super) struct TaskSummaryRow<'a> {
    summary: &'a TaskSummary,
    navigation: Option<Line<'static>>,
    shortcut: &'a str,
}

impl Renderable for TaskSummaryRow<'_> {
    fn desired_height(&self, width: u16) -> u16 {
        u16::from((!self.summary.is_empty() || self.navigation.is_some()) && width > 0)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let total = self.summary.total;
        let working = self.summary.working;
        let noun = if total == 1 { "agent" } else { "agents" };
        let mut line = self.navigation.clone().unwrap_or_default();
        if !self.summary.is_empty() {
            if !line.spans.is_empty() {
                line.push_span(" · ".dim());
            }
            if self.shortcut == "←" {
                line.push_span("← ".dim());
            }
            line.push_span(format!("{total} {noun}").cyan());
            if working > 0 {
                line.push_span(format!(" · {working} working").dim());
            }
            if self.shortcut != "←" {
                line.push_span(format!(" ({})", self.shortcut).dim());
            }
        }
        if line.spans.is_empty() {
            return;
        }
        line.spans.insert(0, "  ".into());
        truncate_line_with_ellipsis_if_overflow(line, usize::from(area.width)).render(area, buf);
    }
}

#[cfg(test)]
#[path = "task_summary_tests.rs"]
mod tests;
