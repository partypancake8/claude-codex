//! Compact, color-separated presentation of the configured usage fields.

use super::StatusLineItem;
use crate::custom_palette as palette;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;

pub(super) fn render(
    segments: &[(StatusLineItem, String)],
    use_colors: bool,
) -> Option<Line<'static>> {
    if !segments.iter().any(|(item, _)| {
        matches!(
            item,
            StatusLineItem::ModelShort
                | StatusLineItem::UsageResetsAvailable
                | StatusLineItem::FiveHourUsed
                | StatusLineItem::WeeklyUsed
                | StatusLineItem::ContextUsedCompact
                | StatusLineItem::SessionLines
        )
    }) {
        return None;
    }

    let mut spans = Vec::new();
    for (item, value) in segments {
        if !spans.is_empty() {
            spans.push(if use_colors {
                " | ".fg(palette::TEAL)
            } else {
                " | ".dim()
            });
        }
        let (text, style) = match item {
            StatusLineItem::UsageResetsAvailable => {
                let amount = value
                    .strip_suffix(" resets available")
                    .or_else(|| value.strip_suffix(" reset available"))
                    .or_else(|| value.strip_prefix("resets:"));
                let text = amount.map_or_else(
                    || value.clone(),
                    |amount| format!("resets:{}", amount.trim()),
                );
                (text, Style::default().fg(palette::SAGE))
            }
            StatusLineItem::FiveHourUsed
            | StatusLineItem::WeeklyUsed
            | StatusLineItem::ContextUsedCompact => {
                let (prefix, label, style) = match item {
                    StatusLineItem::FiveHourUsed => {
                        ("5h ", "5h", Style::default().fg(palette::SAGE))
                    }
                    StatusLineItem::WeeklyUsed => {
                        ("weekly ", "7d", Style::default().fg(palette::ROSE))
                    }
                    _ => ("ctx ", "ctx", Style::default().fg(palette::AMBER)),
                };
                let amount = value.strip_prefix(prefix).unwrap_or(value);
                let amount = amount.strip_suffix(" used").unwrap_or(amount);
                let amount = if amount == "unavailable" {
                    "n/a"
                } else {
                    amount
                };
                (format!("{label}:{amount}"), style)
            }
            StatusLineItem::ModelShort => {
                // The actual effort already appears in the input's top rule.
                let model = value
                    .rsplit_once(' ')
                    .map_or(value.as_str(), |(model, effort)| {
                        if matches!(
                            effort,
                            "default"
                                | "none"
                                | "min"
                                | "minimal"
                                | "low"
                                | "med"
                                | "medium"
                                | "high"
                                | "xhigh"
                                | "max"
                                | "ultra"
                                | "persistent"
                        ) {
                            model
                        } else {
                            value
                        }
                    });
                (model.to_owned(), Style::default().fg(palette::LAVENDER))
            }
            StatusLineItem::SessionLines => {
                let counts = value.strip_prefix("loc ").unwrap_or(value);
                if let Some((added, removed)) = counts.split_once('/') {
                    spans.extend([
                        if use_colors {
                            added.to_owned().fg(palette::SAGE)
                        } else {
                            added.to_owned().dim()
                        },
                        "/".dim(),
                        if use_colors {
                            removed.to_owned().fg(palette::ROSE)
                        } else {
                            removed.to_owned().dim()
                        },
                    ]);
                    continue;
                }
                (counts.to_owned(), Style::default().fg(palette::SAGE))
            }
            StatusLineItem::CurrentDir | StatusLineItem::ProjectRoot => {
                // Keep a deep working directory from displacing the model and edit counts
                // in the compact six-field bar at an ordinary 80-column terminal width.
                (
                    crate::text_formatting::center_truncate_path(value, 20),
                    Style::default().fg(palette::BLUE),
                )
            }
            _ => (value.clone(), Style::default().fg(palette::TEAL)),
        };
        spans.push(Span::styled(
            text,
            if use_colors {
                style
            } else {
                Style::default().dim()
            },
        ));
    }
    Some(Line::from(spans))
}

#[cfg(test)]
#[path = "compact_status_bar_tests.rs"]
mod tests;
