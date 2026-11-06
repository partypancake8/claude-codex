//! Bounded, best-effort previews for the v2 `/subagents` status output.

use super::ThreadBufferedEvent;
use super::ThreadEventStore;
use super::side::SideParentStatus;
use crate::custom_palette::AMBER;
use crate::custom_palette::ROSE;
use crate::custom_palette::SAGE;
use crate::custom_palette::YELLOW;
use crate::history_cell::HistoryCell;
use crate::history_cell::plain_lines;
use crate::multi_agents::AgentPickerThreadEntry;
use crate::style::accent_color;
use crate::text_formatting::truncate_text;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SubAgentActivityKind;
use codex_app_server_protocol::ThreadItemPayload;
use codex_app_server_protocol::TurnStatus;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use std::collections::HashSet;

const AGENT_STATUS_PREVIEW_LINES: usize = 3;
const AGENT_STATUS_PREVIEW_ITEMS: usize = 6;
const AGENT_STATUS_PREVIEW_GRAPHEMES: usize = 240;
const AGENT_STATUS_PREVIEW_INDENT: u16 = 4;

#[derive(Debug)]
pub(super) struct AgentStatusHistoryCell {
    entries: Vec<AgentStatusThreadPreview>,
}

impl AgentStatusHistoryCell {
    pub(super) fn new(entries: Vec<AgentStatusThreadPreview>) -> Self {
        Self { entries }
    }
}

impl HistoryCell for AgentStatusHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = vec![
            "/subagents".magenta().into(),
            "Sub-agents running".bold().into(),
            "".into(),
        ];

        if self.entries.is_empty() {
            lines.push("  • No sub-agents running.".italic().into());
            return lines;
        }

        for entry in &self.entries {
            lines.push(entry.title_line());
            let preview_width = width.saturating_sub(AGENT_STATUS_PREVIEW_INDENT).max(1);
            let preview_lines = entry.preview_lines(preview_width);
            if preview_lines.is_empty() {
                lines.push(vec!["    ".into(), "No recent activity yet.".dim().italic()].into());
            } else {
                lines.extend(preview_lines.into_iter().map(indent_preview_line));
            }
            lines.push("".into());
        }
        let _ = lines.pop();
        lines
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.display_lines(u16::MAX))
    }
}

#[derive(Debug)]
pub(super) struct AgentStatusThreadPreview {
    agent_path: String,
    activity: Vec<String>,
}

impl AgentStatusThreadPreview {
    pub(super) fn from_store(agent_path: String, store: &ThreadEventStore) -> Self {
        Self::from_events(agent_path, store.buffer.iter().rev())
    }

    pub(super) fn empty(agent_path: String) -> Self {
        Self::from_events(agent_path, std::iter::empty())
    }

    fn from_events<'a>(
        agent_path: String,
        events: impl Iterator<Item = &'a ThreadBufferedEvent>,
    ) -> Self {
        let mut seen_item_ids = HashSet::new();
        let mut activity = Vec::new();
        for event in events {
            let item = match event {
                ThreadBufferedEvent::Notification(notification) => match notification.as_ref() {
                    ServerNotification::ItemCompleted(event) => &event.item,
                    ServerNotification::ItemStarted(event) => &event.item,
                    _ => continue,
                },
                ThreadBufferedEvent::Request(_)
                | ThreadBufferedEvent::HistoryEntryResponse(_)
                | ThreadBufferedEvent::FeedbackSubmission(_) => continue,
            };
            if !seen_item_ids.insert(item.id().to_string()) {
                continue;
            }
            if let Some(summary) = activity_summary(&item.payload) {
                activity.push(summary);
                if activity.len() == AGENT_STATUS_PREVIEW_ITEMS {
                    break;
                }
            }
        }
        activity.reverse();
        Self {
            agent_path,
            activity,
        }
    }

    fn title_line(&self) -> Line<'static> {
        vec![
            "  • ".dim(),
            format!("`{}`", self.agent_path).fg(accent_color()),
        ]
        .into()
    }

    fn preview_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = self
            .activity
            .iter()
            .flat_map(|activity| textwrap::wrap(activity, width as usize))
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.into_owned().dim().into())
            .collect::<Vec<_>>();
        if lines.len() > AGENT_STATUS_PREVIEW_LINES {
            lines.drain(..lines.len() - AGENT_STATUS_PREVIEW_LINES);
        }
        lines
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AgentTaskStatus {
    Running,
    NeedsInput,
    NeedsApproval,
    Finished,
    Failed,
    Interrupted,
    Idle,
    Closed,
}

impl AgentTaskStatus {
    pub(super) fn from_entry(
        entry: &AgentPickerThreadEntry,
        store: Option<&ThreadEventStore>,
    ) -> Self {
        if let Some(pending) = store.and_then(ThreadEventStore::side_parent_pending_status) {
            match pending {
                SideParentStatus::NeedsInput => return Self::NeedsInput,
                SideParentStatus::NeedsApproval => return Self::NeedsApproval,
                SideParentStatus::Failed
                | SideParentStatus::Interrupted
                | SideParentStatus::Closed
                | SideParentStatus::Finished => {}
            }
        }
        if entry.is_running && !entry.is_closed {
            return Self::Running;
        }
        let terminal_status = store.and_then(|store| {
            store
                .buffer
                .iter()
                .rev()
                .find_map(|event| match event {
                    ThreadBufferedEvent::Notification(notification) => {
                        match notification.as_ref() {
                            ServerNotification::TurnCompleted(event) => {
                                Some(event.turn.status.clone())
                            }
                            ServerNotification::TurnStarted(_) => Some(TurnStatus::InProgress),
                            _ => None,
                        }
                    }
                    _ => None,
                })
                .or_else(|| store.turns.last().map(|turn| turn.status.clone()))
        });
        match terminal_status {
            Some(TurnStatus::Completed) => Self::Finished,
            Some(TurnStatus::Failed) => Self::Failed,
            Some(TurnStatus::Interrupted) => Self::Interrupted,
            Some(TurnStatus::InProgress) | None if entry.is_closed => Self::Closed,
            Some(TurnStatus::InProgress) | None => Self::Idle,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::NeedsInput => "Needs input",
            Self::NeedsApproval => "Needs approval",
            Self::Finished => "Finished",
            Self::Failed => "Failed",
            Self::Interrupted => "Interrupted",
            Self::Idle => "Idle",
            Self::Closed => "Offline",
        }
    }

    pub(super) fn marker(self) -> Span<'static> {
        match self {
            Self::Running => "●".fg(SAGE),
            Self::NeedsInput | Self::NeedsApproval => "!".fg(AMBER).bold(),
            Self::Finished => "✓".fg(SAGE),
            Self::Failed => "!".fg(ROSE),
            Self::Interrupted => "○".fg(YELLOW),
            Self::Idle | Self::Closed => "○".dim(),
        }
    }
}

/// Reads the newest meaningful public activity without including raw reasoning or tool output.
/// Completed snapshot items cover resumed sessions whose replay buffer is empty.
pub(super) fn latest_activity(store: &ThreadEventStore) -> Option<String> {
    store
        .buffer
        .iter()
        .rev()
        .find_map(|event| match event {
            ThreadBufferedEvent::Notification(notification) => match notification.as_ref() {
                ServerNotification::ItemCompleted(event) => activity_summary(&event.item.payload),
                ServerNotification::ItemStarted(event) => activity_summary(&event.item.payload),
                _ => None,
            },
            ThreadBufferedEvent::Request(_)
            | ThreadBufferedEvent::HistoryEntryResponse(_)
            | ThreadBufferedEvent::FeedbackSubmission(_) => None,
        })
        .or_else(|| {
            store
                .turns
                .iter()
                .rev()
                .flat_map(|turn| turn.items.iter().rev())
                .find_map(|item| activity_summary(&item.payload))
        })
}

fn activity_summary(item: &ThreadItemPayload) -> Option<String> {
    let summary = match item {
        ThreadItemPayload::AgentMessage { text, .. } | ThreadItemPayload::Plan { text, .. } => text,
        ThreadItemPayload::Reasoning { summary, .. } => summary.last()?,
        ThreadItemPayload::CommandExecution { command, .. } => {
            let command = truncate_text(
                command,
                AGENT_STATUS_PREVIEW_GRAPHEMES.saturating_sub("$ ".len()),
            );
            return bounded_summary(&format!("$ {command}"));
        }
        ThreadItemPayload::FileChange { changes, .. } => {
            return bounded_summary(&format!("Updated {} file(s)", changes.len()));
        }
        ThreadItemPayload::McpToolCall { server, tool, .. } => {
            return bounded_summary(&format!("MCP {server}/{tool}"));
        }
        ThreadItemPayload::DynamicToolCall {
            namespace, tool, ..
        } => {
            let tool = namespace
                .as_ref()
                .map(|namespace| format!("{namespace}/{tool}"))
                .unwrap_or_else(|| tool.clone());
            return bounded_summary(&format!("Tool {tool}"));
        }
        ThreadItemPayload::CollabAgentToolCall { tool, .. } => {
            let action = match tool {
                CollabAgentTool::SendMessage
                | CollabAgentTool::FollowupTask
                | CollabAgentTool::InterruptAgent
                | CollabAgentTool::ListAgents => return None,
                CollabAgentTool::SpawnAgent => "Spawned an agent",
                CollabAgentTool::SendInput => "Sent input to an agent",
                CollabAgentTool::ResumeAgent => "Resumed an agent",
                CollabAgentTool::Wait => "Waited for an agent",
                CollabAgentTool::CloseAgent => "Closed an agent",
            };
            return Some(action.to_string());
        }
        ThreadItemPayload::SubAgentActivity {
            kind, agent_path, ..
        } => {
            let action = match kind {
                SubAgentActivityKind::Started => "Started",
                SubAgentActivityKind::Interacted => "Contacted",
                SubAgentActivityKind::Interrupted => "Interrupted",
                SubAgentActivityKind::Completed => "Completed",
            };
            return bounded_summary(&format!("{action} {agent_path}"));
        }
        ThreadItemPayload::WebSearch(item) => {
            return bounded_summary(&format!("Web search: {}", item.query));
        }
        ThreadItemPayload::ImageView { path, .. } => {
            let path = path.render_for_ui();
            return bounded_summary(&format!("Viewed {path}"));
        }
        ThreadItemPayload::ImageGeneration(_) => return Some("Generated an image".to_string()),
        ThreadItemPayload::EnteredReviewMode { .. } => {
            return Some("Entered review mode".to_string());
        }
        ThreadItemPayload::ExitedReviewMode { .. } => {
            return Some("Exited review mode".to_string());
        }
        ThreadItemPayload::ContextCompaction { .. } => {
            return Some("Compacted context".to_string());
        }
        ThreadItemPayload::UserMessage { .. }
        | ThreadItemPayload::HookPrompt { .. }
        | ThreadItemPayload::FunctionCallOutput { .. }
        | ThreadItemPayload::Sleep(_) => {
            return None;
        }
    };
    bounded_summary(summary)
}

fn bounded_summary(summary: &str) -> Option<String> {
    let summary = truncate_text(summary, AGENT_STATUS_PREVIEW_GRAPHEMES);
    let summary = summary.split_whitespace().collect::<Vec<_>>().join(" ");
    (!summary.is_empty()).then_some(summary)
}

fn indent_preview_line(mut line: Line<'static>) -> Line<'static> {
    line.spans.insert(0, "    ".into());
    line
}

#[cfg(test)]
#[path = "agent_status_feed_tests.rs"]
mod tests;
