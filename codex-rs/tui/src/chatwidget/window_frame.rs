//! Keep transient conversation output at the top and input at the window bottom.

use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableItem;
use crossterm::cursor::SetCursorStyle;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub(super) struct WindowFrame<'a> {
    pub(super) transcript: RenderableItem<'a>,
    pub(super) input: RenderableItem<'a>,
}

impl WindowFrame<'_> {
    fn regions(&self, area: Rect) -> (Rect, Rect) {
        let input_height = self
            .input
            .desired_height(area.width)
            .saturating_add(1)
            .min(area.height);
        let transcript_height = area.height.saturating_sub(input_height);
        // The blank separator yields before either input rule or the editable row.
        let separator_height = input_height.saturating_sub(3).min(1);
        (
            Rect::new(area.x, area.y, area.width, transcript_height),
            Rect::new(
                area.x,
                area.y
                    .saturating_add(transcript_height)
                    .saturating_add(separator_height),
                area.width,
                input_height.saturating_sub(separator_height),
            ),
        )
    }
}

impl Renderable for WindowFrame<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let (transcript, input) = self.regions(area);
        self.transcript.render(transcript, buf);
        self.input.render(input, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.transcript
            .desired_height(width)
            .saturating_add(self.input.desired_height(width))
            .saturating_add(1)
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let (_, input) = self.regions(area);
        self.input.cursor_pos(input)
    }

    fn cursor_style(&self, area: Rect) -> SetCursorStyle {
        let (_, input) = self.regions(area);
        self.input.cursor_style(input)
    }
}

#[cfg(test)]
#[path = "window_frame_tests.rs"]
mod tests;
