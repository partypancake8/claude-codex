use super::*;
use codex_app_server_protocol::CommandExecutionSource;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

#[test]
fn agent_status_uses_bounded_buffered_activity() {
    let mut store = ThreadEventStore::new(/*capacity*/ 8);
    store.push_notification(ServerNotification::ItemCompleted(
        ItemCompletedNotification::new(
            "thread-child".to_string(),
            "turn-1".to_string(),
            ThreadItemPayload::CommandExecution {
                model_context: None,
                sandbox_type: None,
                id: "command-1".to_string(),
                command: "cargo test -p codex-tui".to_string(),
                cwd: AbsolutePathBuf::try_from("/workspace")
                    .expect("absolute path")
                    .into(),
                process_id: None,
                plugin_id: None,
                script_path: None,
                source: CommandExecutionSource::Agent,
                status: CommandExecutionStatus::Completed,
                command_actions: Vec::new(),
                aggregated_output: Some("unbounded output\n".repeat(10_000)),
                exit_code: Some(0),
                duration_ms: Some(42),
            },
            /*started_at_ms*/ None,
            /*completed_at_ms*/ 1,
        ),
    ));
    store.push_notification(ServerNotification::ItemCompleted(
        ItemCompletedNotification::new(
            "thread-child".to_string(),
            "turn-1".to_string(),
            ThreadItemPayload::AgentMessage {
                id: "message-1".to_string(),
                text: "Finished checking the focused TUI tests.".to_string(),
                phase: None,
                memory_citation: None,
                delivery: None,
                questions: None,
            },
            /*started_at_ms*/ None,
            /*completed_at_ms*/ 2,
        ),
    ));

    assert_eq!(
        latest_activity(&store),
        Some("Finished checking the focused TUI tests.".to_string())
    );
}

#[test]
fn agent_status_uses_reasoning_summaries_only() {
    let mut store = ThreadEventStore::new(/*capacity*/ 8);
    store.push_notification(ServerNotification::ItemCompleted(
        ItemCompletedNotification::new(
            "thread-child".to_string(),
            "turn-1".to_string(),
            ThreadItemPayload::Reasoning {
                id: "reasoning-with-summary".to_string(),
                summary: vec!["safe summary".to_string()],
                content: vec!["hidden raw reasoning".to_string()],
            },
            /*started_at_ms*/ None,
            /*completed_at_ms*/ 1,
        ),
    ));
    store.push_notification(ServerNotification::ItemCompleted(
        ItemCompletedNotification::new(
            "thread-child".to_string(),
            "turn-1".to_string(),
            ThreadItemPayload::Reasoning {
                id: "reasoning-without-summary".to_string(),
                summary: Vec::new(),
                content: vec!["raw-only reasoning".to_string()],
            },
            /*started_at_ms*/ None,
            /*completed_at_ms*/ 2,
        ),
    ));

    assert_eq!(latest_activity(&store), Some("safe summary".to_string()));
}

#[test]
fn agent_status_caps_long_command_previews() {
    let item = codex_app_server_protocol::ThreadItemPayload::AgentMessage {
        id: "message".to_string(),
        text: "word ".repeat(1000),
        phase: None,
        memory_citation: None,
        delivery: None,
        questions: None,
    };
    let preview = activity_summary(&item).expect("preview");
    assert!(preview.chars().count() <= AGENT_STATUS_PREVIEW_GRAPHEMES);
}
