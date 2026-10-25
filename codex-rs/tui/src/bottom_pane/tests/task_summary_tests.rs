use super::*;
use pretty_assertions::assert_eq;
use ratatui::style::Stylize;

#[test]
fn task_summary_preserves_usage_footer_and_clears_without_leaving_a_row() {
    let (tx, _rx) = unbounded_channel();
    let mut pane = test_pane(AppEventSender::new(tx));
    pane.set_status_line_enabled(/*enabled*/ true);
    pane.set_status_line(Some(Line::from("gpt-6 · 12% used · +10 −2")));
    let width = 60;
    let initial_height = pane.desired_height(width);
    pane.set_task_summary(/*total*/ 3, /*working*/ 2);
    assert_eq!(pane.desired_height(width), initial_height + 1);
    insta::assert_snapshot!(render_snapshot(
        &pane,
        Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            width,
            pane.desired_height(width)
        ),
    ));
    pane.set_task_summary(/*total*/ 0, /*working*/ 0);
    assert_eq!(pane.desired_height(width), initial_height);
}

#[test]
fn escape_while_editing_tasks_command_does_not_interrupt_running_turn() {
    let (tx, mut rx) = unbounded_channel();
    let mut pane = test_pane(AppEventSender::new(tx));
    pane.set_task_running(/*running*/ true);
    pane.insert_str("/tasks ");
    pane.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(pane.composer_text(), "/tasks ");
    while let Ok(event) = rx.try_recv() {
        assert!(!matches!(event, AppEvent::CodexOp(Op::Interrupt)));
    }
}

#[test]
fn constrained_height_keeps_prompt_cursor_ahead_of_task_summary() {
    let (tx, _rx) = unbounded_channel();
    let mut pane = test_pane(AppEventSender::new(tx));
    let width = 40;
    let height = pane.desired_height(width);
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
    let cursor_before = pane.cursor_pos(area);
    pane.set_task_summary(/*total*/ 3, /*working*/ 2);
    assert_eq!(pane.cursor_pos(area), cursor_before);
    insta::assert_snapshot!(render_snapshot(&pane, area));
}

#[test]
fn live_agents_follow_input_usage_and_mode_at_split_pane_widths() {
    let (tx, _rx) = unbounded_channel();
    let mut pane = test_pane(AppEventSender::new(tx));
    pane.set_status_line_enabled(/*enabled*/ true);
    pane.set_status_line(Some(Line::from("resets:0 | ctx:2% | 7d:8%")));
    pane.set_collaboration_modes_enabled(/*enabled*/ true);
    pane.set_agents_navigation_enabled(/*enabled*/ true);
    pane.set_task_summary(/*total*/ 1, /*working*/ 1);
    pane.insert_str("draft stays editable");
    let row = LiveAgentRow {
        name: "reviewer".to_string(),
        status: "Running".to_string(),
        marker: "●".fg(crate::custom_palette::SAGE),
        activity: Some("Inspecting native thread updates.".to_string()),
        is_active: true,
    };
    let mut frames = Vec::new();
    for width in [60, 49] {
        pane.set_live_agent_panel(Vec::new());
        let base_height = pane.desired_height(width);
        let base_area = Rect::new(0, 0, width, base_height);
        let base_cursor = pane.cursor_pos(base_area);
        let base_frame = render_snapshot(&pane, base_area);
        let constrained = (1..=base_height)
            .map(|height| {
                let area = Rect::new(0, 0, width, height);
                (area, pane.cursor_pos(area), render_snapshot(&pane, area))
            })
            .collect::<Vec<_>>();
        pane.set_live_agent_panel(vec![row.clone()]);
        assert_eq!(pane.desired_height(width), base_height + 3);

        // Child rows disappear before either the editable prompt or its footer.
        assert_eq!(pane.cursor_pos(base_area), base_cursor);
        assert_eq!(render_snapshot(&pane, base_area), base_frame);
        for (area, cursor, frame) in constrained {
            assert_eq!(pane.cursor_pos(area), cursor);
            assert_eq!(render_snapshot(&pane, area), frame);
            if area.height >= 3 {
                let (_, cursor_y) = cursor.expect("visible input cursor");
                assert!(cursor_y < area.height);
                assert!(frame.contains("draft stays editable"));
            }
        }

        let area = Rect::new(0, 0, width, pane.desired_height(width));
        let rendered = render_snapshot(&pane, area);
        let rows = rendered.lines().collect::<Vec<_>>();
        let prompt = rows
            .iter()
            .position(|row| row.contains("draft stays editable"))
            .unwrap();
        let usage = rows
            .iter()
            .position(|row| row.contains("resets:0"))
            .unwrap();
        let mode = rows.iter().position(|row| row.contains("default")).unwrap();
        let panel = rows
            .iter()
            .position(|row| row.contains("subagents"))
            .unwrap();
        assert!(prompt < usage && usage < mode && mode < panel);
        assert!(rows[panel + 1].contains("reviewer · Running"));
        assert!(rows[panel + 2].contains("Inspecting native thread updates."));
        frames.push(format!("{width} columns:\n{rendered}"));
        frames.push(format!("{width} columns, constrained:\n{base_frame}"));
    }
    insta::assert_snapshot!(frames.join("\n\n"));
}

#[test]
#[allow(clippy::disallowed_methods)] // Verify the explicitly requested indexed palette.
fn effort_and_mode_frame_the_compact_usage_footer() {
    let (tx, _rx) = unbounded_channel();
    let mut pane = test_pane(AppEventSender::new(tx));
    pane.set_status_line_enabled(/*enabled*/ true);
    pane.set_status_line(Some(Line::from(
        "5h:12% | 7d:28% | context:9% | loc +10/-2",
    )));
    pane.set_collaboration_modes_enabled(/*enabled*/ true);
    pane.set_agents_navigation_enabled(/*enabled*/ true);
    pane.set_active_reasoning_effort_baseline(Some(&ReasoningEffort::Medium));
    let width = 100;
    let mut frames = Vec::new();
    for (label, mode) in [
        ("default", None),
        ("plan", Some(CollaborationModeIndicator::Plan)),
    ] {
        pane.set_collaboration_mode_indicator(mode);
        let navigation = pane.composer.navigation_footer_line().expect("mode row");
        assert_eq!(navigation.spans[0].content, label);
        assert_eq!(
            navigation.spans[0].style.fg,
            Some(ratatui::style::Color::Indexed(144)),
        );
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            width,
            pane.desired_height(width),
        );
        frames.push(format!("{label}:\n{}", render_snapshot(&pane, area)));
    }
    pane.set_task_summary(/*total*/ 3, /*working*/ 2);
    frames.push(format!(
        "tasks:\n{}",
        render_snapshot(
            &pane,
            Rect::new(
                /*x*/ 0,
                /*y*/ 0,
                width,
                pane.desired_height(width)
            ),
        )
    ));
    insta::assert_snapshot!(frames.join("\n\n"));
}

#[test]
fn effort_change_refreshes_header_without_starting_tier_animation() {
    let (tx, _rx) = unbounded_channel();
    let mut pane = test_pane(AppEventSender::new(tx));
    pane.set_active_reasoning_effort_baseline(Some(&ReasoningEffort::Medium));
    assert!(pane.composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::High),
        /*animations_enabled*/ false,
    ));
    assert_eq!(
        pane.composer.effort_header_line(),
        Some(Line::from(" ◐ high · /model  ".dim()).right_aligned()),
    );
}

#[test]
fn effort_uses_input_rule_without_an_extra_row_after_working_status() {
    let (tx, _rx) = unbounded_channel();
    let mut pane = test_pane(AppEventSender::new(tx));
    pane.set_task_running(/*running*/ true);
    let width = 80;
    let before_height = pane.desired_height(width);
    let before = render_snapshot(&pane, Rect::new(0, 0, width, before_height));
    let before_rows = before.lines().collect::<Vec<_>>();
    let status = before_rows
        .iter()
        .position(|row| row.contains("Working"))
        .unwrap();
    assert!(before_rows[status + 1].starts_with('─'));
    assert!(before_rows[status + 2].starts_with("❯ "));

    pane.set_active_reasoning_effort_baseline(Some(&ReasoningEffort::High));
    assert_eq!(pane.desired_height(width), before_height);
    let rendered = render_snapshot(&pane, Rect::new(0, 0, width, pane.desired_height(width)));
    let rows = rendered.lines().collect::<Vec<_>>();
    let status = rows.iter().position(|row| row.contains("Working")).unwrap();
    assert!(rows[status + 1].contains("◐ high · /model"));
    assert!(rows[status + 1].starts_with('─'));
    assert!(rows[status + 2].starts_with("❯ "));
    insta::assert_snapshot!(rendered);
}

#[test]
fn custom_navigation_lowercases_vim_mode_only_for_display() {
    let (tx, _rx) = unbounded_channel();
    let mut pane = test_pane(AppEventSender::new(tx));
    pane.set_collaboration_modes_enabled(true);
    pane.composer.enable_vim_in_insert_mode();
    let line = pane
        .composer
        .navigation_footer_line()
        .expect("mode row")
        .to_string();
    assert!(line.contains("default"));
    assert!(line.contains("vim: insert"));
    assert!(!line.contains("Vim:"));
}
