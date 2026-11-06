use super::*;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::unbounded_channel;

#[test]
fn braille_loop_keeps_working_fixed_as_elapsed_clock_advances() {
    let (tx, _rx) = unbounded_channel();
    let widget = StatusIndicatorWidget::new(
        AppEventSender::new(tx),
        FrameRequester::test_dummy(),
        /*animations_enabled*/ true,
        Default::default(),
    );
    let mut timer = StatusTimer::default();
    timer.pause_at(timer.last_resume_at);
    let mut frames = Vec::new();
    for (elapsed_ms, expected_glyph) in [
        (0, "⠋"),
        (89, "⠋"),
        (90, "⠙"),
        (180, "⠹"),
        (270, "⠸"),
        (360, "⠼"),
        (450, "⠴"),
        (540, "⠦"),
        (630, "⠧"),
        (720, "⠇"),
        (810, "⠏"),
        (899, "⠏"),
        (900, "⠋"),
        (1000, "⠙"),
        (59999, "⠦"),
        (60000, "⠦"),
        (3600000, "⠋"),
    ] {
        timer.elapsed_running = Duration::from_millis(elapsed_ms);
        for width in [20, 60] {
            let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 1);
            let mut buffer = Buffer::empty(area);
            widget.with_timer(&timer).render(area, &mut buffer);
            assert_eq!(buffer[(0, 0)].symbol(), expected_glyph);
            assert_eq!(buffer[(0, 0)].fg, crate::custom_palette::TEAL);
            assert_eq!(buffer[(1, 0)].symbol(), " ");
            let text = buffer
                .content()
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>();
            assert_eq!(
                text.chars().skip(1).take(9).collect::<String>(),
                " Working "
            );
            frames.push(format!(
                "{elapsed_ms}ms / {width} columns: {}",
                text.trim_end()
            ));
        }
    }
    insta::assert_snapshot!(frames.join("\n"));
}

#[test]
fn reduced_motion_preserves_elapsed_clock_without_braille() {
    let (tx, _rx) = unbounded_channel();
    let widget = StatusIndicatorWidget::new(
        AppEventSender::new(tx),
        FrameRequester::test_dummy(),
        /*animations_enabled*/ false,
        Default::default(),
    );
    let mut timer = StatusTimer::default();
    timer.pause_at(timer.last_resume_at);
    let mut frames = Vec::new();
    for elapsed_ms in [0, 90, 1000, 60000] {
        timer.elapsed_running = Duration::from_millis(elapsed_ms);
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 60, /*height*/ 1,
        );
        let mut buffer = Buffer::empty(area);
        widget.with_timer(&timer).render(area, &mut buffer);
        let text = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        assert_eq!(buffer[(0, 0)].symbol(), "W");
        frames.push(format!("{elapsed_ms}ms: {}", text.trim_end()));
    }
    insta::assert_snapshot!(frames.join("\n"));
}
