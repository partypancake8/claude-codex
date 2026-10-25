use super::*;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn observed_agents_render_one_row_at_wide_and_narrow_widths() {
    let mut summary = TaskSummary::default();
    assert!(summary.update(/*total*/ 3, /*working*/ 2));
    let mut snapshots = Vec::new();
    for width in [60, 30, 12] {
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).expect("terminal");
        let row = summary.with_navigation(/*navigation*/ None, "ctrl+t");
        terminal
            .draw(|frame| row.render(frame.area(), frame.buffer_mut()))
            .expect("render task summary");
        snapshots.push(format!("{}", terminal.backend()));
        assert_eq!(row.desired_height(width), 1);
    }
    insta::assert_snapshot!(snapshots.join("\n"));
}

#[test]
fn idle_agents_show_the_available_navigation_key_without_zero_progress() {
    let mut summary = TaskSummary::default();
    summary.update(/*total*/ 1, /*working*/ 0);
    let mut frames = Vec::new();
    for shortcut in ["←", "ctrl+t", "/tasks"] {
        let row = summary.with_navigation(/*navigation*/ None, shortcut);
        let mut terminal = Terminal::new(TestBackend::new(40, 1)).expect("terminal");
        terminal
            .draw(|frame| row.render(frame.area(), frame.buffer_mut()))
            .expect("task navigation");
        let text = terminal.backend().to_string();
        assert!(text.contains("1 agent"));
        assert!(text.contains(shortcut));
        assert!(!text.contains("0 working"));
        frames.push(text);
    }
    insta::assert_snapshot!(frames.join("\n"));
}

#[test]
fn clearing_agents_restores_zero_height_and_repeated_state_is_unchanged() {
    let mut summary = TaskSummary::default();
    assert_eq!(
        summary
            .with_navigation(/*navigation*/ None, "/tasks")
            .desired_height(60),
        0
    );
    assert!(summary.update(/*total*/ 1, /*working*/ 0));
    assert!(!summary.update(/*total*/ 1, /*working*/ 0));
    assert_eq!(
        summary
            .with_navigation(/*navigation*/ None, "/tasks")
            .desired_height(60),
        1
    );
    assert!(summary.update(/*total*/ 0, /*working*/ 0));
    assert_eq!(
        summary
            .with_navigation(/*navigation*/ None, "/tasks")
            .desired_height(60),
        0
    );
}
