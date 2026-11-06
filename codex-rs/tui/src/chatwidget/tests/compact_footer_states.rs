use super::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[tokio::test]
async fn configured_usage_stays_visible_while_working_drafting_and_watching_agents() {
    let (mut chat, _events, _operations) = make_chatwidget_manual(Some("gpt-5.4")).await;
    chat.local_settings.tui.show_tooltips = false;
    chat.local_settings.tui.status_line = Some(
        [
            "usage-resets-available",
            "context-used-compact",
            "weekly-used",
            "current-dir",
            "model-short",
            "session-lines",
        ]
        .map(str::to_string)
        .to_vec(),
    );
    chat.session_lines.record(
        "visible-footer",
        &codex_app_server_protocol::ThreadItemPayload::FileChange {
            id: "edit".to_string(),
            status: AppServerPatchApplyStatus::Completed,
            changes: vec![FileUpdateChange {
                path: "example.rs".to_string(),
                kind: PatchChangeKind::Update { move_path: None },
                diff: "@@ -1 +1,2 @@\n-old\n+new\n+new\n".to_string(),
            }],
        },
    );
    chat.on_rate_limit_snapshot(Some(RateLimitSnapshot {
        limit_id: None,
        limit_name: None,
        primary: Some(RateLimitWindow {
            used_percent: 11,
            window_duration_mins: Some(300),
            resets_at: None,
        }),
        secondary: Some(RateLimitWindow {
            used_percent: 44,
            window_duration_mins: Some(10080),
            resets_at: None,
        }),
        credits: None,
        individual_limit: None,
        plan_type: None,
        spend_control_reached: None,
        rate_limit_reached_type: None,
    }));
    chat.has_codex_backend_auth = true;
    let reset_request = chat.start_rate_limit_reset_startup_check();
    assert!(chat.finish_rate_limit_reset_hint_refresh(
        reset_request,
        Vec::new(),
        Ok(codex_app_server_protocol::RateLimitResetCreditsSummary {
            available_count: 1,
            credits: None,
        }),
    ));
    chat.clear_pending_rate_limit_reset_hint();
    chat.refresh_status_line();
    let configured = status_line_text(&chat).expect("configured status line");
    chat.bottom_pane.set_collaboration_modes_enabled(true);
    chat.bottom_pane.set_command_center_enabled(true);
    let mut snapshots = Vec::new();
    for (name, running, draft, child) in [
        ("idle", false, "", false),
        ("draft", false, "Please check the result", false),
        ("working", true, "", false),
        ("working with draft", true, "Please check the result", false),
        ("watching agent", true, "", true),
        (
            "watching agent with draft",
            true,
            "Please check the result",
            true,
        ),
    ] {
        chat.bottom_pane.set_task_running(running);
        chat.bottom_pane
            .set_composer_text(draft.to_string(), Vec::new(), Vec::new());
        chat.set_active_agent_label(child.then(|| "Reviewer".to_string()));
        chat.set_task_summary(usize::from(child), usize::from(child));
        for width in [49, 60, 80, 120] {
            let area = Rect::new(0, 0, width, 24);
            let mut buffer = Buffer::empty(area);
            chat.render(area, &mut buffer);
            let rows = (0..area.height)
                .map(|y| {
                    (0..width)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                        .trim_end()
                        .to_string()
                })
                .collect::<Vec<_>>();
            for field in configured.split(" | ") {
                assert!(
                    rows.iter().any(|row| row.contains(field)),
                    "configured field {field} hidden in {name} at {width} columns: {rows:?}"
                );
            }
            assert!(!rows.iter().any(|row| row.contains("context left")));
            let (cursor_x, cursor_y) = chat.cursor_pos(area).expect("editable prompt");
            assert!(cursor_x < width && cursor_y < area.height);
            assert!(rows[usize::from(cursor_y)].starts_with("❯ "));
            if running && !draft.is_empty() {
                assert!(rows.last().unwrap().contains("to queue"));
            }
            snapshots.push(format!(
                "{name}, {width} columns:\n{}",
                rows[17..].join("\n")
            ));
        }
    }
    insta::assert_snapshot!(normalize_snapshot_paths(snapshots.join("\n\n")));
}
