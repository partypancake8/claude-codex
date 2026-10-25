use crate::chatwidget::ChatWidget;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::history_cell;
use crate::render::renderable::Renderable;
use codex_protocol::openai_models::ReasoningEffort;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;

async fn window_chat() -> ChatWidget {
    let (mut chat, _sender, _events, _operations) = make_chatwidget_manual_with_sender().await;
    chat.bottom_pane.set_status_line_enabled(/*enabled*/ true);
    chat.bottom_pane
        .set_status_line(Some(Line::from("usage fixture · loc +10/-2")));
    chat.bottom_pane
        .set_collaboration_modes_enabled(/*enabled*/ true);
    chat.bottom_pane
        .set_agents_navigation_enabled(/*enabled*/ true);
    chat.bottom_pane
        .set_active_reasoning_effort_baseline(Some(&ReasoningEffort::Medium));
    chat.bottom_pane
        .set_placeholder_text("Ask Codex to do anything".to_string());
    chat
}

// Retain row numbers and runs of empty rows so snapshots prove full-window spacing
// without making reviewers scan dozens of identical blank strings.
fn numbered_rows(buffer: &Buffer) -> String {
    let mut result = Vec::new();
    let mut empty_start = None;
    for y in buffer.area.top()..buffer.area.bottom() {
        let text = (buffer.area.left()..buffer.area.right())
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>();
        if text.trim().is_empty() {
            empty_start.get_or_insert(y);
            continue;
        }
        if let Some(start) = empty_start.take() {
            result.push(format!("rows {start}..{y}: blank"));
        }
        result.push(format!("{y:02}: {}", text.trim_end()));
    }
    if let Some(start) = empty_start {
        result.push(format!("rows {start}..{}: blank", buffer.area.bottom()));
    }
    result.join("\n")
}

#[tokio::test]
async fn fresh_and_draft_composer_follow_window_bottom_across_resizes() {
    let mut chat = window_chat().await;
    let mut frames = Vec::new();
    for draft in ["", "review the layout"] {
        chat.bottom_pane
            .set_composer_text(draft.to_string(), Vec::new(), Vec::new());
        for (width, height) in [(80, 24), (120, 40), (80, 24)] {
            let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
            let mut buffer = Buffer::empty(area);
            chat.render(area, &mut buffer);
            let input_height = chat.bottom_pane.desired_height(width);
            let input_area = Rect::new(/*x*/ 0, height - input_height, width, input_height);
            let cursor = chat.cursor_pos(area);
            assert_eq!(cursor, chat.bottom_pane.cursor_pos(input_area));
            let (cursor_x, cursor_y) = cursor.expect("visible input cursor");
            assert!(cursor_x < width && cursor_y >= input_area.top() && cursor_y < height);
            let prompt_row = (0..width)
                .map(|x| buffer[(x, cursor_y)].symbol())
                .collect::<String>();
            assert!(
                prompt_row.starts_with("❯ "),
                "cursor must remain on the input row: {prompt_row}"
            );
            assert!(numbered_rows(&buffer).contains("default (shift+tab)"));
            frames.push(format!(
                "{width}x{height}, draft={draft:?}, cursor={cursor:?}\n{}",
                numbered_rows(&buffer)
            ));
        }
    }
    insta::assert_snapshot!(frames.join("\n\n"));
}

#[tokio::test]
async fn transient_output_stays_at_top_while_input_stays_at_bottom() {
    let mut chat = window_chat().await;
    chat.transcript.active_cell = Some(Box::new(history_cell::new_info_event(
        "Transient result".to_string(),
        /*hint*/ None,
    )));
    let mut frames = Vec::new();
    for (width, height) in [(80, 24), (120, 40)] {
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
        let mut buffer = Buffer::empty(area);
        chat.render(area, &mut buffer);
        let content = numbered_rows(&buffer);
        assert!(content.contains("01: ● Transient result"));
        let input_height = chat.bottom_pane.desired_height(width);
        let input_area = Rect::new(/*x*/ 0, height - input_height, width, input_height);
        assert_eq!(
            chat.cursor_pos(area),
            chat.bottom_pane.cursor_pos(input_area)
        );
        frames.push(format!("{width}x{height}\n{content}"));
    }
    insta::assert_snapshot!(frames.join("\n\n"));
}

#[tokio::test]
async fn tiny_windows_drop_ambient_rows_before_the_editable_prompt() {
    let mut chat = window_chat().await;
    chat.bottom_pane
        .set_composer_text("x".to_string(), Vec::new(), Vec::new());
    let mut frames = Vec::new();
    for (width, height) in [(8, 4), (12, 5), (24, 8), (0, 0), (1, 1), (2, 2)] {
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
        let mut buffer = Buffer::empty(area);
        chat.render(area, &mut buffer);
        let content = numbered_rows(&buffer);
        if height >= 4 {
            assert!(
                content.contains("❯ x"),
                "prompt missing at {width}x{height}: {content}"
            );
            let (x, y) = chat.cursor_pos(area).expect("cursor in usable tiny window");
            assert!(x < width && y < height);
        }
        frames.push(format!("{width}x{height}\n{content}"));
    }
    insta::assert_snapshot!(frames.join("\n\n"));
}

#[tokio::test]
async fn three_row_window_keeps_prompt_with_suggestions_and_live_status() {
    let mut frames = Vec::new();
    for draft in ["draft", "/mo", "@src"] {
        for running in [false, true] {
            let mut chat = window_chat().await;
            chat.bottom_pane.set_task_running(running);
            chat.bottom_pane.insert_str(draft);
            if draft != "draft" {
                assert!(!chat.bottom_pane.no_modal_or_popup_active());
            }
            for height in [3, 4] {
                let area = Rect::new(/*x*/ 0, /*y*/ 0, /*width*/ 40, height);
                let mut buffer = Buffer::empty(area);
                chat.render(area, &mut buffer);
                let content = numbered_rows(&buffer);
                assert!(
                    content.contains(&format!("❯ {draft}")),
                    "{draft} in {height} rows: {content}"
                );
                let (x, y) = chat.cursor_pos(area).expect("cursor in three-row window");
                assert!(x < area.width && y < area.height);
                let prompt = (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>();
                assert_eq!(prompt.trim_end(), format!("❯ {draft}"));
                frames.push(format!(
                    "{draft} / running={running} / {height} rows:\n{content}"
                ));
            }
        }
    }
    insta::assert_snapshot!(frames.join("\n\n"));
}
