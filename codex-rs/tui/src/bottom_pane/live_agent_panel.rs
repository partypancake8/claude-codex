//! Persistent, bounded child activity supplied by the app's native thread subscriptions.

use crate::custom_palette::TEAL;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::render::renderable::Renderable;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Widget;

const VISIBLE_AGENTS: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LiveAgentRow {
    pub name: String,
    pub status: String,
    pub marker: Span<'static>,
    pub activity: Option<String>,
    pub is_active: bool,
}

#[derive(Default)]
pub(super) struct LiveAgentPanel {
    rows: Vec<LiveAgentRow>,
}

impl LiveAgentPanel {
    pub(super) fn update(&mut self, mut rows: Vec<LiveAgentRow>) -> bool {
        // Keep spawn order within each group while making active children visible first.
        rows.sort_by_key(|row| !row.is_active);
        if self.rows == rows {
            return false;
        }
        self.rows = rows;
        true
    }

    fn lines(&self, width: u16) -> Vec<Line<'static>> {
        if self.rows.is_empty() || width == 0 {
            return Vec::new();
        }
        let mut lines = vec![Line::from(vec![
            "  subagents".fg(TEAL).bold(),
            " · /tasks to watch".dim(),
        ])];
        for row in self.rows.iter().take(VISIBLE_AGENTS) {
            let status = format!(" · {}", row.status);
            let name_width = usize::from(width).saturating_sub(status.chars().count() + 4);
            let name = truncate_line_with_ellipsis_if_overflow(
                Line::from(row.name.clone().bold()),
                name_width,
            );
            let mut line = Line::from(vec!["  ".into(), row.marker.clone(), " ".into()]);
            line.spans.extend(name.spans);
            line.push_span(status.dim());
            lines.push(line);
            if let Some(activity) = &row.activity {
                lines.push(Line::from(format!("    {activity}")).dim());
            }
        }
        if self.rows.len() > VISIBLE_AGENTS {
            lines.push(
                Line::from(format!(
                    "  +{} more · /tasks",
                    self.rows.len() - VISIBLE_AGENTS
                ))
                .dim(),
            );
        }
        lines
            .into_iter()
            .map(|line| truncate_line_with_ellipsis_if_overflow(line, usize::from(width)))
            .collect()
    }
}

impl Renderable for LiveAgentPanel {
    fn desired_height(&self, width: u16) -> u16 {
        self.lines(width).len() as u16
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        for (line, y) in self
            .lines(area.width)
            .into_iter()
            .zip(area.y..area.bottom())
        {
            line.render(Rect::new(area.x, y, area.width, 1), buf);
        }
    }
}

#[cfg(test)]
#[path = "live_agent_panel_tests.rs"]
mod tests;
