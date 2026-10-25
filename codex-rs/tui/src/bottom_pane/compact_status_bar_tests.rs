use super::*;
use pretty_assertions::assert_eq;
use ratatui::style::Color;
use ratatui::style::Modifier;

#[test]
#[allow(clippy::disallowed_methods)] // Verify the explicitly requested indexed palette.
fn formats_observed_values_with_muted_separators_and_distinct_edit_colors() {
    let line = render(
        &[
            (StatusLineItem::FiveHourUsed, "5h unavailable".into()),
            (StatusLineItem::ContextUsedCompact, "ctx 3% used".into()),
            (StatusLineItem::WeeklyUsed, "weekly 44% used".into()),
            (StatusLineItem::CurrentDir, "~".into()),
            (StatusLineItem::ModelShort, "6-astra high".into()),
            (StatusLineItem::SessionLines, "loc +12/-3".into()),
        ],
        /*use_colors*/ true,
    )
    .unwrap();
    assert_eq!(
        line.to_string(),
        "5h:n/a | ctx:3% | 7d:44% | ~ | 6-astra | +12/-3"
    );
    for (index, color) in [
        (0, Color::Indexed(108)),
        (1, Color::Indexed(73)),
        (2, Color::Indexed(137)),
        (4, Color::Indexed(138)),
        (6, Color::Indexed(109)),
        (8, Color::Indexed(139)),
        (10, Color::Indexed(108)),
        (12, Color::Indexed(138)),
    ] {
        assert_eq!(line.spans[index].style.fg, Some(color));
    }
    assert!(
        line.spans
            .iter()
            .all(|span| !span.style.add_modifier.contains(Modifier::BOLD))
    );
}

#[test]
fn keeps_unknown_context_explicit_and_honors_disabled_colors() {
    let line = render(
        &[
            (StatusLineItem::ContextUsedCompact, "ctx unavailable".into()),
            (StatusLineItem::WeeklyUsed, "weekly 0% used".into()),
        ],
        /*use_colors*/ false,
    )
    .unwrap();
    assert_eq!(line.to_string(), "ctx:n/a | 7d:0%");
    assert!(line.spans.iter().all(|span| span.style.fg.is_none()));
}

#[test]
fn removes_only_known_effort_suffixes_from_model_labels() {
    for (value, expected) in [
        ("6-astra default", "6-astra"),
        ("6-astra max", "6-astra"),
        ("6-astra ultra", "6-astra"),
        ("6-astra persistent", "6-astra"),
        ("Custom Model Name", "Custom Model Name"),
        ("Custom Model Name high", "Custom Model Name"),
    ] {
        let line = render(
            &[(StatusLineItem::ModelShort, value.into())],
            /*use_colors*/ false,
        )
        .unwrap();
        assert_eq!(line.to_string(), expected);
    }
}

#[test]
fn long_directory_leaves_room_for_usage_model_and_edits_at_eighty_columns() {
    let line = render(
        &[
            (StatusLineItem::FiveHourUsed, "5h 100% used".into()),
            (StatusLineItem::ContextUsedCompact, "ctx 100% used".into()),
            (StatusLineItem::WeeklyUsed, "weekly 100% used".into()),
            (
                StatusLineItem::CurrentDir,
                "/workspace/projects/long-project-name/nested/component".into(),
            ),
            (StatusLineItem::ModelShort, "6-astra high".into()),
            (StatusLineItem::SessionLines, "loc +120/-30".into()),
        ],
        /*use_colors*/ true,
    )
    .unwrap();
    assert!(line.width() <= 78, "{}", line.to_string());
    assert!(line.to_string().contains("component | 6-astra | +120/-30"));
    assert!(line.to_string().contains('…'));
}

#[test]
#[allow(clippy::disallowed_methods)] // Verify the explicitly requested indexed palette.
fn reset_availability_uses_muted_green_without_inventing_unknown_counts() {
    for (value, expected) in [
        ("resets: n/a", "resets:n/a"),
        ("0 resets available", "resets:0"),
        ("1 reset available", "resets:1"),
        ("2 resets available", "resets:2"),
    ] {
        for use_colors in [true, false] {
            let line = render(
                &[(StatusLineItem::UsageResetsAvailable, value.to_string())],
                use_colors,
            )
            .unwrap();
            assert_eq!(line.to_string(), expected);
            assert_eq!(
                line.spans[0].style.fg,
                use_colors.then_some(Color::Indexed(108))
            );
        }
    }
}
