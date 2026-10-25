use super::tests::new_test_composer;
use super::*;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn framed_multiline_draft_keeps_cursor_inside_rules() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_text_content(
        "Review the changes\nand explain the tradeoffs".to_string(),
        Vec::new(),
        Vec::new(),
    );
    composer
        .draft
        .textarea
        .set_cursor(composer.draft.textarea.text().len());
    let width = 48;
    let height = composer.desired_height(width);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal
        .draw(|frame| composer.render(frame.area(), frame.buffer_mut()))
        .expect("render composer");

    assert_eq!(
        composer.cursor_pos(Rect::new(0, 0, width, height)),
        Some((27, 2)),
    );
    insta::assert_snapshot!(terminal.backend());
}

#[test]
fn framed_composer_handles_constrained_terminal_sizes() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_text_content("draft".to_string(), Vec::new(), Vec::new());
    let mut frames = Vec::new();
    for (width, height) in [(0, 0), (1, 1), (2, 2), (12, 4)] {
        let area = Rect::new(0, 0, width, height);
        let mut buffer = Buffer::empty(area);
        composer.render(area, &mut buffer);
        frames.push(format!("{buffer:?}"));
    }
    insta::assert_snapshot!(frames.join("\n"));
}

#[test]
fn tiny_suggestion_popups_preserve_editable_prompt() {
    let mut frames = Vec::new();
    for draft in ["/mo", "@src"] {
        let (mut composer, _rx) = new_test_composer();
        composer.set_text_content(draft.to_string(), Vec::new(), Vec::new());
        composer.draft.textarea.set_cursor(draft.len());
        composer.sync_popups();
        if draft.starts_with('@') {
            composer.on_file_search_result(
                "src".to_string(),
                ["src/main.rs", "src/lib.rs", "src/app.rs"]
                    .into_iter()
                    .map(|path| codex_file_search::FileMatch {
                        score: 1,
                        path: PathBuf::from(path),
                        match_type: codex_file_search::MatchType::File,
                        root: PathBuf::from("/tmp"),
                        indices: None,
                    })
                    .collect(),
            );
        }
        assert!(composer.popup_active());
        for height in [3, 4] {
            let area = Rect::new(/*x*/ 0, /*y*/ 0, /*width*/ 49, height);
            let mut buffer = Buffer::empty(area);
            composer.render(area, &mut buffer);
            let rows = (0..height)
                .map(|y| {
                    (0..area.width)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>();
            assert_eq!(rows[1].trim_end(), format!("❯ {draft}"));
            assert_eq!(composer.cursor_pos(area), Some((2 + draft.len() as u16, 1)));
            assert!(rows[0].starts_with('─') && rows[2].starts_with('─'));
            frames.push(format!("{draft} / {height} rows:\n{}", rows.join("\n")));
        }
    }
    insta::assert_snapshot!(frames.join("\n\n"));
}
