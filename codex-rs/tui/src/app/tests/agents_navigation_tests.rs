use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn agents_navigation_requires_local_daemon() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let endpoint = crate::RemoteAppServerEndpoint::UnixSocket {
        socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
    };
    for target in [
        AppServerTarget::Embedded,
        AppServerTarget::Remote {
            endpoint: endpoint.clone(),
        },
        AppServerTarget::LocalDaemon { endpoint },
    ] {
        let enabled = matches!(target, AppServerTarget::LocalDaemon { .. });
        app.app_server_target = target;
        let init = app.chatwidget_init_for_forked_or_resumed_thread(
            &mut tui,
            app.config.clone(),
            /*initial_user_message*/ None,
        );
        app.replace_chat_widget(ChatWidget::new_with_app_event(init));
        while events.try_recv().is_ok() {}
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
            assert!(!app.chat_widget.no_modal_or_popup_active());
        } else {
            assert!(events.try_recv().is_err());
            assert!(app.chat_widget.no_modal_or_popup_active());
            assert!(!render_bottom_popup(&app.chat_widget, /*width*/ 96).contains("for agents"));
        }
    }
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
        AppServerTarget::LocalDaemon { endpoint },
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
        ServerNotification::ItemCompleted(codex_app_server_protocol::ItemCompletedNotification {
            thread_id: child.to_string(),
            turn_id: "live-turn".to_string(),
            completed_at_ms: 1,
            item: ThreadItem::AgentMessage {
                id: "activity".to_string(),
                text: "Checking the event routing".to_string(),
                phase: None,
                memory_citation: None,
                delivery: None,
                questions: None,
            },
        }),
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
            .contains("Subagents are currently disabled")
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
