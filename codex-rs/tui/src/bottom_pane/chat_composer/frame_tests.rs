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
fn effort_title_stays_clear_of_rule_across_resizes() {
    let (mut composer, _rx) = new_test_composer();
    composer.use_separate_navigation_row();
    composer.set_text_content("draft".to_string(), Vec::new(), Vec::new());
    let mut frames = Vec::new();
    for effort in [ReasoningEffort::High, ReasoningEffort::Medium] {
        composer.set_active_reasoning_effort_baseline(Some(&effort));
        for width in [60, 49, 24, 23, 100, 49] {
            let height = composer.desired_height(width);
            let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
            let mut buffer = Buffer::empty(area);
            composer.render(area, &mut buffer);
            let rows = (0..height)
                .map(|y| {
                    (0..width)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>();
            if width >= 24 {
                let title = format!(" ◐ {} · /model  ", effort.as_str());
                assert!(rows[0].ends_with(&title));
                assert!(rows[0].contains("─ ◐"));
            } else {
                assert_eq!(rows[0], "─".repeat(usize::from(width)));
            }
            assert_eq!(rows[2], "─".repeat(usize::from(width)));
            assert!(rows[1].contains("draft"));
            frames.push(format!("{effort:?}, {width} columns:\n{}", rows.join("\n")));
        }
    }
    insta::assert_snapshot!(frames.join("\n\n"));
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

#[test]
fn narrow_usage_footer_keeps_model_and_edits_across_draft_and_resize() {
    let (mut composer, _rx) = new_test_composer();
    composer.use_separate_navigation_row();
    composer.set_status_line_enabled(/*enabled*/ true);
    composer.set_status_line(Some(Line::from(
        "resets:0 | ctx:10% | 7d:44% | ~/src/codex-custom | 6-astra | +120/-30",
    )));
    let mut frames = Vec::new();
    for (draft, running) in [
        ("", false),
        ("follow-up draft", true),
        ("follow-up draft", false),
    ] {
        composer.set_text_content(draft.to_string(), Vec::new(), Vec::new());
        composer.set_task_running(running);
        for width in [120, 49, 60, 80, 49, 120] {
            let height = composer.desired_height(width);
            let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
            let mut buffer = Buffer::empty(area);
            composer.render(area, &mut buffer);
            let rows = (0..height)
                .map(|y| {
                    (0..width)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                        .trim_end()
                        .to_string()
                })
                .collect::<Vec<_>>();
            let rendered = rows.join("\n");
            for field in [
                "resets:0",
                "ctx:10%",
                "7d:44%",
                "~/src/codex-custom",
                "6-astra",
                "+120/-30",
            ] {
                assert!(
                    rendered.contains(field),
                    "missing {field} at {width} columns:\n{rendered}"
                );
            }
            assert_eq!(height, if width < 80 { 5 } else { 4 });
            let (_, cursor_y) = composer.cursor_pos(area).expect("editable prompt");
            assert!(rows[usize::from(cursor_y)].starts_with("❯ "));
            frames.push(format!(
                "{width} columns / draft={draft:?} / running={running}:\n{rendered}"
            ));
        }
    }
    insta::assert_snapshot!(frames.join("\n\n"));
}
