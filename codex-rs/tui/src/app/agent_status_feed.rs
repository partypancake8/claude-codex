//! Bounded activity previews and authoritative status for the live task picker.

use super::ThreadBufferedEvent;
use super::ThreadEventStore;
use super::side::SideParentStatus;
use crate::custom_palette::AMBER;
use crate::custom_palette::ROSE;
use crate::custom_palette::SAGE;
use crate::custom_palette::YELLOW;
use crate::multi_agents::AgentPickerThreadEntry;
use crate::text_formatting::truncate_text;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SubAgentActivityKind;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::TurnStatus;
use ratatui::style::Stylize;
use ratatui::text::Span;

const AGENT_STATUS_PREVIEW_GRAPHEMES: usize = 120;

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
                ServerNotification::ItemCompleted(event) => activity_summary(&event.item),
                ServerNotification::ItemStarted(event) => activity_summary(&event.item),
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
                .find_map(activity_summary)
        })
}

fn activity_summary(item: &ThreadItem) -> Option<String> {
    let summary = match item {
        ThreadItem::AgentMessage { text, .. } | ThreadItem::Plan { text, .. } => text,
        ThreadItem::Reasoning { summary, .. } => summary.last()?,
        ThreadItem::CommandExecution { command, .. } => {
            let command = truncate_text(
                command,
                AGENT_STATUS_PREVIEW_GRAPHEMES.saturating_sub("$ ".len()),
            );
            return bounded_summary(&format!("$ {command}"));
        }
        ThreadItem::FileChange { changes, .. } => {
            return bounded_summary(&format!("Updated {} file(s)", changes.len()));
        }
        ThreadItem::McpToolCall { server, tool, .. } => {
            return bounded_summary(&format!("MCP {server}/{tool}"));
        }
        ThreadItem::DynamicToolCall {
            namespace, tool, ..
        } => {
            let tool = namespace
                .as_ref()
                .map(|namespace| format!("{namespace}/{tool}"))
                .unwrap_or_else(|| tool.clone());
            return bounded_summary(&format!("Tool {tool}"));
        }
        ThreadItem::CollabAgentToolCall { tool, .. } => {
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
        ThreadItem::SubAgentActivity {
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
        ThreadItem::WebSearch(item) => {
            return bounded_summary(&format!("Web search: {}", item.query));
        }
        ThreadItem::ImageView { path, .. } => {
            let path = path.render_for_ui();
            return bounded_summary(&format!("Viewed {path}"));
        }
        ThreadItem::ImageGeneration(_) => return Some("Generated an image".to_string()),
        ThreadItem::EnteredReviewMode { .. } => return Some("Entered review mode".to_string()),
        ThreadItem::ExitedReviewMode { .. } => return Some("Exited review mode".to_string()),
        ThreadItem::ContextCompaction { .. } => return Some("Compacted context".to_string()),
        ThreadItem::UserMessage { .. }
        | ThreadItem::HookPrompt { .. }
        | ThreadItem::FunctionCallOutput { .. }
        | ThreadItem::Sleep(_) => {
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

#[cfg(test)]
#[path = "agent_status_feed_tests.rs"]
mod tests;
