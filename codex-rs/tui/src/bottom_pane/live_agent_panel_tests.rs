use super::*;
use pretty_assertions::assert_eq;

fn running_row(name: &str, activity: &str) -> LiveAgentRow {
    LiveAgentRow {
        name: name.to_string(),
        status: "Running".to_string(),
        marker: "●".fg(crate::custom_palette::SAGE),
        activity: Some(activity.to_string()),
        is_active: true,
    }
}

fn rendered(panel: &LiveAgentPanel, width: u16) -> String {
    let area = Rect::new(0, 0, width, panel.desired_height(width));
    let mut buf = Buffer::empty(area);
    panel.render(area, &mut buf);
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn live_panel_renders_name_status_and_activity_at_actual_split_pane_widths() {
    let mut panel = LiveAgentPanel::default();
    panel.update(vec![running_row(
        "reviewer",
        "Inspecting native thread updates.",
    )]);
    for width in [60, 49] {
        assert_eq!(
            rendered(&panel, width),
            "  subagents · /tasks to watch\n  ● reviewer · Running\n    Inspecting native thread updates."
        );
    }
    panel.update(vec![running_row(
        "reviewer/very-long-subtask-name-that-needs-truncation",
        "Checking a very long operation without expanding the composer or wrapping the panel.",
    )]);
    for width in [60, 49] {
        let text = rendered(&panel, width);
        assert!(text.contains("Running"));
        assert!(text.contains('…'));
        assert!(
            panel
                .lines(width)
                .iter()
                .all(|line| line.width() <= usize::from(width))
        );
        assert_eq!(panel.desired_height(width), 3);
    }
}

#[test]
fn activity_updates_and_completion_remain_visible_without_duplicate_redraws() {
    let mut panel = LiveAgentPanel::default();
    let mut row = running_row("reviewer", "Inspecting thread routing.");
    assert!(panel.update(vec![row.clone()]));
    assert!(!panel.update(vec![row.clone()]));
    row.activity = Some("Checking the next live notification.".to_string());
    assert!(panel.update(vec![row.clone()]));
    assert!(!rendered(&panel, 60).contains("Inspecting thread routing."));
    assert!(rendered(&panel, 60).contains("Checking the next live notification."));
    row.status = "Finished".to_string();
    row.marker = "✓".fg(crate::custom_palette::SAGE);
    row.is_active = false;
    assert!(panel.update(vec![row]));
    assert!(rendered(&panel, 60).contains("✓ reviewer · Finished"));
    assert_eq!(panel.desired_height(60), 3);
    assert!(panel.update(Vec::new()));
    assert_eq!(panel.desired_height(60), 0);
    assert_eq!(rendered(&panel, 60), "");
}

#[test]
fn active_children_take_priority_and_many_children_keep_bounded_height() {
    let mut panel = LiveAgentPanel::default();
    let mut rows = (0..5)
        .map(|index| running_row(&format!("agent-{index}"), "Working on a public task."))
        .collect::<Vec<_>>();
    rows[0].is_active = false;
    rows[0].status = "Finished".to_string();
    panel.update(rows);
    let text = rendered(&panel, 49);
    assert!(!text.contains("agent-0"));
    assert!(text.contains("agent-1"));
    assert!(text.contains("agent-3"));
    assert!(text.contains("+2 more · /tasks"));
    assert_eq!(panel.desired_height(49), 8);
}
