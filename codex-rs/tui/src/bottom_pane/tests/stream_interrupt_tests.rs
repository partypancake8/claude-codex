use super::*;
use pretty_assertions::assert_eq;

#[test]
fn escape_interrupts_hidden_status_stream_without_losing_draft() {
    let (tx, mut rx) = unbounded_channel();
    let mut pane = test_pane(AppEventSender::new(tx));
    pane.set_task_running(/*running*/ true);
    pane.hide_status_indicator();
    pane.insert_str("Keep this draft while interrupting");
    let area = Rect::new(0, 0, 60, pane.desired_height(60));
    let before = render_snapshot(&pane, area);

    pane.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert!(matches!(
        rx.try_recv(),
        Ok(AppEvent::CodexOp(Op::Interrupt))
    ));
    assert!(rx.try_recv().is_err());
    assert_eq!(pane.composer_text(), "Keep this draft while interrupting");
    assert_eq!(render_snapshot(&pane, area), before);
    insta::assert_snapshot!(before);
}
