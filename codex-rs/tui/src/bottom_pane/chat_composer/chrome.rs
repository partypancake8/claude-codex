//! Main-conversation effort title and navigation footer, separate from editable text.
//!
//! Embedded question inputs keep their existing compact layout. The app-owned bottom pane
//! places navigation below the composer. Effort shares its top rule when width permits.

use super::*;

impl ChatComposer {
    pub(crate) fn use_separate_navigation_row(&mut self) {
        self.separate_navigation_row = true;
    }

    /// Main-window usage remains visible while composing a follow-up to a running turn.
    /// Queue guidance belongs to the navigation row; modal and embedded inputs retain their
    /// existing instructional footer behavior.
    pub(super) fn persistent_status_line(&self) -> Option<Line<'static>> {
        (self.separate_navigation_row
            && self.footer.status_line_enabled
            && matches!(
                self.footer_mode(),
                FooterMode::ComposerEmpty | FooterMode::ComposerHasDraft
            ))
        .then(|| self.footer.status_line_value.clone())
        .flatten()
    }

    fn queue_hint_in_navigation(&self) -> bool {
        self.is_task_running
            && self.footer_mode() == FooterMode::ComposerHasDraft
            && self.persistent_status_line().is_some()
            && self.footer.queue_key.is_some()
    }

    pub(crate) fn task_navigation_uses_left(&self) -> bool {
        self.agents_navigation_available()
    }

    pub(crate) fn effort_header_line(&self) -> Option<Line<'static>> {
        if !self.effort_observed {
            return None;
        }
        let effort = self
            .active_reasoning_effort
            .as_ref()
            .map(ReasoningEffort::as_str)
            .unwrap_or("default");
        Some(Line::from(format!("◐ {effort} · /model  ").dim()).right_aligned())
    }

    pub(super) fn navigation_row_visible(&self) -> bool {
        self.separate_navigation_row
            && matches!(
                self.footer_mode(),
                FooterMode::ComposerEmpty | FooterMode::ComposerHasDraft
            )
            && self.custom_footer_height().is_none()
            && !self.popup_active()
            && (self.collaboration_modes_enabled
                || self.footer.collaboration_mode_indicator.is_some()
                || self.agents_navigation_enabled
                || self.footer.active_agent_label.is_some()
                || self.queue_hint_in_navigation())
    }

    pub(crate) fn navigation_footer_line(&self) -> Option<Line<'static>> {
        if !self.navigation_row_visible() {
            return None;
        }
        let mut line = Line::default();
        if self.collaboration_modes_enabled || self.footer.collaboration_mode_indicator.is_some() {
            let label = match self.footer.collaboration_mode_indicator {
                Some(CollaborationModeIndicator::Plan) => "plan",
                None => "default",
            };
            line.push_span(label.fg(crate::custom_palette::YELLOW));
            if self.collaboration_modes_enabled && !self.is_task_running && self.draft.input_enabled
            {
                line.push_span(" (shift+tab)".dim());
            }
        }
        let mut indicators = self.mode_indicator_line(/*show_cycle_hint*/ false);
        // The collaboration label already leads this row; retain the other live indicators.
        if self.footer.collaboration_mode_indicator.is_some() {
            indicators = status_line_right_indicator_line(
                /*collaboration_mode_indicator*/ None,
                self.footer.goal_status_indicator.as_ref(),
                self.footer.ide_context_active,
                /*show_cycle_hint*/ false,
            );
            if let Some(vim) = self.vim_mode_indicator_span() {
                let indicator = indicators.get_or_insert_with(Line::default);
                if !indicator.spans.is_empty() {
                    indicator.push_span(" · ".dim());
                }
                indicator.push_span(vim);
            }
        }
        if let Some(mut indicators) = indicators {
            for span in &mut indicators.spans {
                if matches!(span.content.as_ref(), "Vim: Normal" | "Vim: Insert") {
                    span.style = span.style.fg(if span.content == "Vim: Insert" {
                        crate::custom_palette::SAGE
                    } else {
                        crate::custom_palette::LAVENDER
                    });
                    span.content = span.content.to_lowercase().into();
                }
            }
            if !line.spans.is_empty() {
                line.push_span(" · ".dim());
            }
            line.extend(indicators.spans);
        }
        if let Some(label) = self.footer.active_agent_label.as_ref() {
            if !line.spans.is_empty() {
                line.push_span(" · ".dim());
            }
            line.push_span(label.clone().dim());
        }
        if self.queue_hint_in_navigation()
            && let Some(key) = self.footer.queue_key
        {
            if !line.spans.is_empty() {
                line.push_span(" · ".dim());
            }
            line.push_span(key);
            line.push_span(" to queue".dim());
        }
        (!line.spans.is_empty()).then_some(line)
    }
}
