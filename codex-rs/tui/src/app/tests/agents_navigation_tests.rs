use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn agents_navigation_requires_shared_app_server() -> Result<()> {
    let endpoint = crate::RemoteAppServerEndpoint::UnixSocket {
        socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
    };
    for (target, enabled) in [
        (AppServerTarget::Embedded, false),
        (
            AppServerTarget::Remote {
                endpoint: endpoint.clone(),
            },
            true,
        ),
        (
            AppServerTarget::LocalDaemon {
                endpoint,
                allow_embedded_fallback: true,
            },
            true,
        ),
    ] {
        let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let mut app_server = start_config_write_test_app_server(&app).await?;
        app.app_server_target = target;
        let init = app.chatwidget_init_for_forked_or_resumed_thread(
            &mut tui,
            app.config.clone(),
            /*initial_user_message*/ None,
        );
        app.replace_chat_widget(ChatWidget::new_with_app_event(init));
        while events.try_recv().is_ok() {}
        let composer = render_bottom_popup(&app.chat_widget, /*width*/ 96);
        assert_eq!(composer.contains("← for agents"), enabled);
        if matches!(app.app_server_target, AppServerTarget::Remote { .. }) {
            let footer = composer.lines().last().expect("composer footer").trim();
            insta::assert_snapshot!(normalize_snapshot_paths(footer), @"default (shift+tab) · ← for agents");
        }
        app.handle_tui_event(
            &mut tui,
            &mut app_server,
            TuiEvent::Key(KeyCode::Left.into()),
        )
        .await?;
        if enabled {
            let event = events.try_recv()?;
            assert_matches!(event, AppEvent::OpenAgentsOverview);
            app.handle_event(&mut tui, &mut app_server, event).await?;
            assert!(app.chat_widget.active_view_is(AGENTS_OVERVIEW_VIEW_ID));
        } else {
            assert!(events.try_recv().is_err());
            assert!(app.chat_widget.no_modal_or_popup_active());
        }
        app_server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn externally_archived_thread_keeps_advertised_agents_navigation() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    app.app_server_target = AppServerTarget::LocalDaemon {
        endpoint: crate::RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
        },
        allow_embedded_fallback: true,
    };
    let init = app.chatwidget_init_for_forked_or_resumed_thread(
        &mut tui,
        app.config.clone(),
        /*initial_user_message*/ None,
    );
    app.replace_chat_widget(ChatWidget::new_with_app_event(init));
    let id = ThreadId::new();
    app.active_thread_id = Some(id);
    app.primary_thread_id = Some(id);
    app.chat_widget
        .handle_thread_session(test_thread_session(id, app.config.cwd.to_path_buf()));
    app.ensure_thread_channel(id);
    app.handle_app_server_event(
        &app_server,
        codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
            ServerNotification::ThreadArchived(ThreadArchivedNotification {
                thread_id: id.to_string(),
            }),
        )),
    )
    .await;
    while events.try_recv().is_ok() {}

    // Left must still edit an existing draft before it can navigate from an empty composer.
    app.chat_widget
        .restore_user_message_to_composer("ab".into());
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyCode::Left.into()),
    )
    .await?;
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyCode::Char('!').into()),
    )
    .await?;
    assert_eq!(app.chat_widget.composer_text_with_pending(), "a!b");
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::OpenAgentsOverview))
    );
    for key in [
        KeyCode::End.into(),
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
    ] {
        app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(key))
            .await?;
    }
    assert!(app.chat_widget.composer_is_empty());
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 96).contains("for agents"));
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new_with_kind(
            KeyCode::Left,
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        )),
    )
    .await?;
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::OpenAgentsOverview))
    );
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyCode::Left.into()),
    )
    .await?;
    let event = events.try_recv()?;
    assert_matches!(event, AppEvent::OpenAgentsOverview);
    app.handle_event(&mut tui, &mut app_server, event).await?;
    insta::assert_snapshot!(
        "externally_archived_thread_agents",
        render_bottom_popup(&app.chat_widget, /*width*/ 96)
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn pending_windows_sandbox_setup_blocks_thread_replacement() -> Result<()> {
    use crate::app_event::WindowsSandboxEnableMode;

    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    app.chat_widget.windows_sandbox_local_server = true;
    while events.try_recv().is_ok() {}
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let current = ThreadId::new();
    let other = ThreadId::new();
    app.active_thread_id = Some(current);
    app.primary_thread_id = Some(current);
    let preset = codex_utils_approval_presets::builtin_approval_presets()
        .into_iter()
        .find(|preset| preset.id == "auto")
        .expect("auto preset");
    app.windows_sandbox.pending_setup = Some((WindowsSandboxEnableMode::Elevated, preset, None));
    let prompt = crate::chatwidget::create_initial_user_message(
        Some("review this".to_string()),
        Vec::new(),
        Vec::new(),
    );
    app.chat_widget.initial_user_message = prompt.clone();

    app.select_agent_thread(&mut tui, &mut app_server, other)
        .await?;
    assert_eq!(app.active_thread_id, Some(current));
    assert_eq!(app.chat_widget.initial_user_message, prompt);
    let cell = match events.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected setup navigation message, got {other:?}"),
    };
    let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 100));
    insta::assert_snapshot!(rendered, @"● Finish Windows sandbox setup before switching threads.");

    app.chat_widget.initial_user_message = None;
    app.run_agents_overview_action(
        &mut tui,
        &mut app_server,
        current,
        crate::app_event::AgentsOverviewAction::Archive,
    )
    .await?;
    assert_eq!(app.active_thread_id, Some(current));
    assert!(events.try_recv().is_err());

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn local_children_open_live_picker_from_empty_prompt_on_every_backend() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let endpoint = crate::RemoteAppServerEndpoint::UnixSocket {
        socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
    };
    let main = ThreadId::new();
    let child = ThreadId::new();
    app.primary_thread_id = Some(main);
    app.active_thread_id = Some(main);
    app.upsert_agent_picker_thread(
        main, /*agent_nickname*/ None, /*agent_role*/ None, /*is_closed*/ false,
    );
    app.upsert_agent_picker_thread(
        child,
        Some("Reviewer".to_string()),
        /*agent_role*/ None,
        /*is_closed*/ false,
    );
    app.agent_navigation.mark_parent_owned(child);
    app.enqueue_thread_notification(child, turn_started_notification(child, "live-turn"))
        .await?;

    for target in [
        AppServerTarget::Embedded,
        AppServerTarget::Remote {
            endpoint: endpoint.clone(),
        },
        AppServerTarget::LocalDaemon {
            endpoint,
            allow_embedded_fallback: false,
        },
    ] {
        app.app_server_target = target;
        let init = app.chatwidget_init_for_forked_or_resumed_thread(
            &mut tui,
            app.config.clone(),
            /*initial_user_message*/ None,
        );
        app.replace_chat_widget(ChatWidget::new_with_app_event(init));
        app.sync_active_agent_label();
        while events.try_recv().is_ok() {}
        app.handle_tui_event(
            &mut tui,
            &mut app_server,
            TuiEvent::Key(KeyCode::Left.into()),
        )
        .await?;
        let event = events.try_recv()?;
        assert_matches!(&event, AppEvent::OpenAgentPicker);
        app.handle_event(&mut tui, &mut app_server, event).await?;
        let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 100);
        assert!(rendered.contains("Reviewer"));
        assert!(rendered.contains("Running · view only"));
        app.chat_widget.handle_key_event(KeyCode::Esc.into());

        app.chat_widget
            .apply_external_edit("unfinished draft".to_string());
        while events.try_recv().is_ok() {}
        app.handle_tui_event(
            &mut tui,
            &mut app_server,
            TuiEvent::Key(KeyCode::Left.into()),
        )
        .await?;
        assert!(app.chat_widget.no_modal_or_popup_active());
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            "unfinished draft"
        );
        assert!(
            std::iter::from_fn(|| events.try_recv().ok()).all(|event| !matches!(
                event,
                AppEvent::OpenAgentPicker | AppEvent::OpenAgentsOverview
            ))
        );
    }
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn task_picker_updates_live_activity_and_retains_selection() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let main = ThreadId::new();
    let child = ThreadId::new();
    app.primary_thread_id = Some(main);
    app.active_thread_id = Some(main);
    app.upsert_agent_picker_thread(
        main, /*agent_nickname*/ None, /*agent_role*/ None, /*is_closed*/ false,
    );
    app.upsert_agent_picker_thread(
        child,
        Some("Reviewer".to_string()),
        /*agent_role*/ None,
        /*is_closed*/ false,
    );
    app.agent_navigation.mark_parent_owned(child);
    app.chat_widget
        .apply_external_edit("Keep this draft while watching agents".to_string());
    let draft_state = app.chat_widget.capture_thread_input_state();
    let child_index = app
        .agent_picker_selection_view_params(None)
        .items
        .iter()
        .position(|row| row.name == "Reviewer")
        .expect("reviewer row");
    let params = app.agent_picker_selection_view_params(Some(child_index));
    app.chat_widget.show_selection_view(params);

    app.enqueue_thread_notification(child, turn_started_notification(child, "live-turn"))
        .await?;
    app.enqueue_thread_notification(
        child,
        ServerNotification::ItemCompleted(
            codex_app_server_protocol::ItemCompletedNotification::new(
                child.to_string(),
                "live-turn".to_string(),
                codex_app_server_protocol::ThreadItemPayload::AgentMessage {
                    id: "activity".to_string(),
                    text: "Checking the event routing".to_string(),
                    phase: None,
                    memory_citation: None,
                    delivery: None,
                    questions: None,
                },
                None,
                1,
            ),
        ),
    )
    .await?;

    let params = app.agent_picker_selection_view_params(Some(child_index));
    assert_eq!(
        params.items[child_index].description.as_deref(),
        Some("Running · view only · Checking the event routing")
    );
    assert_eq!(
        app.chat_widget
            .selected_index_for_present_view(super::super::agent_picker::AGENT_PICKER_VIEW_ID),
        Some(child_index)
    );
    let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 120);
    assert!(rendered.contains("Checking the event routing"));
    insta::assert_snapshot!("live_child_agent_picker", rendered);
    assert_eq!(app.chat_widget.capture_thread_input_state(), draft_state);

    app.enqueue_thread_notification(
        child,
        turn_completed_notification(child, "live-turn", TurnStatus::Failed),
    )
    .await?;
    let params = app.agent_picker_selection_view_params(Some(child_index));
    assert_eq!(
        params.items[child_index].description.as_deref(),
        Some("Failed · view only · Checking the event routing")
    );
    assert_eq!(params.title.as_deref(), Some("Tasks · 0 running"));
    app.mark_agent_picker_thread_closed(child);
    let params = app.agent_picker_selection_view_params(Some(child_index));
    assert_eq!(
        params.items[child_index].description.as_deref(),
        Some("Failed · offline · view only · Checking the event routing")
    );
    Ok(())
}

#[tokio::test]
async fn task_picker_prioritizes_pending_approval_over_running() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let child = ThreadId::new();
    app.upsert_agent_picker_thread(
        child,
        Some("Reviewer".to_string()),
        /*agent_role*/ None,
        /*is_closed*/ false,
    );
    app.agent_navigation.mark_running(child);
    let channel = ThreadEventChannel::new(/*capacity*/ 8);
    channel
        .store
        .lock()
        .await
        .push_request(exec_approval_request(
            child, "turn", "command", /*approval_id*/ None,
        ));
    app.thread_event_channels.insert(child, channel);

    let params = app.agent_picker_selection_view_params(None);
    assert_eq!(
        params
            .items
            .iter()
            .find(|row| row.name == "Reviewer")
            .expect("reviewer row")
            .description
            .as_deref(),
        Some("Needs approval")
    );
    assert_eq!(params.title.as_deref(), Some("Tasks · 0 running"));
    Ok(())
}

#[tokio::test]
async fn task_picker_background_terminal_row_routes_to_displayed_thread_view() {
    let (app, mut events, _ops) = make_test_app_with_channels().await;
    let params = app.agent_picker_selection_view_params(None);
    let row = params
        .items
        .iter()
        .find(|row| row.name == "Background terminals")
        .expect("background terminal row");
    assert_eq!(row.name, "Background terminals");
    while events.try_recv().is_ok() {}
    (row.actions[0])(&app.app_event_tx);
    assert_matches!(events.try_recv(), Ok(AppEvent::ShowBackgroundTerminals));
}

#[tokio::test]
async fn task_picker_enable_action_opens_existing_confirmation_without_enabling() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let _ = app.config.features.disable(Feature::Collab);
    let params = app.agent_picker_selection_view_params(None);
    let row = params
        .items
        .iter()
        .find(|row| row.name == "Enable subagents")
        .expect("opt-in row");
    while events.try_recv().is_ok() {}
    (row.actions[0])(&app.app_event_tx);
    let event = events.try_recv()?;
    assert_matches!(&event, AppEvent::OpenMultiAgentEnablePrompt);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    app.handle_event(&mut tui, &mut app_server, event).await?;
    assert!(
        render_bottom_popup(&app.chat_widget, /*width*/ 100)
            .contains("Subagents are disabled in this TUI session")
    );
    assert!(!app.config.features.enabled(Feature::Collab));
    let _ = app.config.features.enable(Feature::Collab);
    let params = app.agent_picker_selection_view_params(None);
    assert!(
        !params
            .items
            .iter()
            .any(|row| row.name == "Enable subagents")
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn task_picker_keeps_background_terminal_selection_when_child_is_discovered() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let main = ThreadId::new();
    app.primary_thread_id = Some(main);
    app.active_thread_id = Some(main);
    app.upsert_agent_picker_thread(
        main, /*agent_nickname*/ None, /*agent_role*/ None, /*is_closed*/ false,
    );
    let params = app.agent_picker_selection_view_params(Some(0));
    assert_eq!(params.items[0].name, "Background terminals");
    app.chat_widget.show_selection_view(params);
    let child = ThreadId::new();
    app.upsert_agent_picker_thread(
        child,
        Some("New child".to_string()),
        /*agent_role*/ None,
        /*is_closed*/ false,
    );
    app.enqueue_thread_notification(child, turn_started_notification(child, "turn"))
        .await?;
    assert_eq!(
        app.chat_widget
            .selected_index_for_present_view(super::super::agent_picker::AGENT_PICKER_VIEW_ID),
        Some(0)
    );
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 100).contains("New child"));
    while events.try_recv().is_ok() {}
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_matches!(events.try_recv(), Ok(AppEvent::ShowBackgroundTerminals));
    Ok(())
}

#[tokio::test]
async fn task_picker_does_not_reuse_prior_outcome_for_disconnected_new_turn() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let child = ThreadId::new();
    app.upsert_agent_picker_thread(
        child,
        Some("Reviewer".to_string()),
        /*agent_role*/ None,
        /*is_closed*/ false,
    );
    app.enqueue_thread_notification(child, turn_started_notification(child, "finished-turn"))
        .await?;
    app.enqueue_thread_notification(
        child,
        turn_completed_notification(child, "finished-turn", TurnStatus::Completed),
    )
    .await?;
    app.enqueue_thread_notification(child, turn_started_notification(child, "new-turn"))
        .await?;
    app.mark_agent_picker_thread_closed(child);
    let params = app.agent_picker_selection_view_params(None);
    let child_row = params
        .items
        .iter()
        .find(|row| row.name == "Reviewer")
        .expect("reviewer row");
    assert_eq!(child_row.description.as_deref(), Some("Offline"));
    Ok(())
}

async fn app_with_finished_and_running_children() -> Result<(
    Box<App>,
    (
        tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
        tokio::sync::mpsc::UnboundedReceiver<Op>,
    ),
    ThreadId,
    ThreadId,
)> {
    let (mut app, events, ops) = make_test_app_with_channels().await;
    let main = ThreadId::new();
    app.primary_thread_id = Some(main);
    app.active_thread_id = Some(main);
    app.upsert_agent_picker_thread(
        main, /*agent_nickname*/ None, /*agent_role*/ None, /*is_closed*/ false,
    );
    let finished = ThreadId::new();
    app.upsert_agent_picker_thread(
        finished,
        Some("Battery".to_string()),
        /*agent_role*/ None,
        /*is_closed*/ false,
    );
    app.enqueue_thread_notification(finished, turn_started_notification(finished, "done"))
        .await?;
    app.enqueue_thread_notification(
        finished,
        turn_completed_notification(finished, "done", TurnStatus::Completed),
    )
    .await?;
    let running = ThreadId::new();
    app.upsert_agent_picker_thread(
        running,
        Some("Viewer".to_string()),
        /*agent_role*/ None,
        /*is_closed*/ false,
    );
    app.agent_navigation.mark_running(running);
    app.enqueue_thread_notification(running, turn_started_notification(running, "live"))
        .await?;
    app.sync_task_summary();
    Ok((app, (events, ops), finished, running))
}

fn live_panel_text(app: &App) -> String {
    render_bottom_popup(&app.chat_widget, /*width*/ 100)
}

#[tokio::test]
async fn finished_agent_row_expires_after_thirty_seconds_and_running_row_stays() -> Result<()> {
    let (mut app, _events, finished, running) = app_with_finished_and_running_children().await?;
    let base = std::time::Instant::now();
    let text = live_panel_text(&app);
    assert!(text.contains("· Finished"), "{text}");
    assert!(text.contains("· Running"), "{text}");

    app.sync_task_summary_at(
        base + std::time::Duration::from_secs(29),
        /*dismiss_finished*/ false,
    );
    assert!(live_panel_text(&app).contains("· Finished"));

    app.sync_task_summary_at(
        base + super::super::agent_picker::FINISHED_AGENT_ROW_TTL,
        /*dismiss_finished*/ false,
    );
    let text = live_panel_text(&app);
    assert!(!text.contains("· Finished"), "{text}");
    assert!(text.contains("· Running"), "{text}");
    assert!(app.finished_agent_rows.dismissed.contains(&finished));
    assert!(!app.finished_agent_rows.dismissed.contains(&running));

    // A re-emitted completion for the dismissed thread must not bring the row back.
    app.enqueue_thread_notification(
        finished,
        turn_completed_notification(finished, "done", TurnStatus::Completed),
    )
    .await?;
    app.sync_task_summary();
    assert!(!live_panel_text(&app).contains("· Finished"));

    // Long-running rows are never expired.
    app.sync_task_summary_at(
        base + std::time::Duration::from_secs(3_600),
        /*dismiss_finished*/ false,
    );
    assert!(live_panel_text(&app).contains("· Running"));
    Ok(())
}

#[tokio::test]
async fn next_main_agent_submit_dismisses_finished_rows_only() -> Result<()> {
    let (mut app, _events, finished, running) = app_with_finished_and_running_children().await?;

    // Messages sent to a child thread do not acknowledge other finished children.
    app.active_thread_id = Some(running);
    app.note_main_agent_user_turn();
    assert!(live_panel_text(&app).contains("· Finished"));

    app.active_thread_id = app.primary_thread_id;
    app.note_main_agent_user_turn();
    let text = live_panel_text(&app);
    assert!(!text.contains("· Finished"), "{text}");
    assert!(text.contains("· Running"), "{text}");
    assert!(app.finished_agent_rows.dismissed.contains(&finished));

    app.sync_task_summary();
    assert!(!live_panel_text(&app).contains("· Finished"));
    Ok(())
}

#[tokio::test]
async fn tasks_clear_event_dismisses_finished_rows_and_reactivation_restores_them() -> Result<()> {
    let (mut app, _events, finished, _running) = app_with_finished_and_running_children().await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    app.handle_event(&mut tui, &mut app_server, AppEvent::ClearFinishedAgentRows)
        .await?;
    let text = live_panel_text(&app);
    assert!(!text.contains("· Finished"), "{text}");
    assert!(text.contains("· Running"), "{text}");

    // The scheduled expiry wake-up only re-derives the panel; nothing comes back.
    app.handle_event(&mut tui, &mut app_server, AppEvent::RefreshLiveAgentPanel)
        .await?;
    assert!(!live_panel_text(&app).contains("· Finished"));

    // A dismissed thread that starts working again is a new run and shows again.
    app.agent_navigation.mark_running(finished);
    app.enqueue_thread_notification(finished, turn_started_notification(finished, "again"))
        .await?;
    app.sync_task_summary();
    assert!(!app.finished_agent_rows.dismissed.contains(&finished));
    assert!(live_panel_text(&app).contains("Battery"));
    app_server.shutdown().await?;
    Ok(())
}
