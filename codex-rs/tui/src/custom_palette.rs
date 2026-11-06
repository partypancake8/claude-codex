//! Muted xterm-256 accents for the custom Codex UI surfaces.

// Sawyer explicitly requested fixed muted 8-bit accents instead of the terminal's
// potentially bright ANSI palette. Keep that exception local to custom chrome.
#![allow(clippy::disallowed_methods)]

use ratatui::style::Color;

pub(crate) const SAGE: Color = Color::Indexed(108);
pub(crate) const ROSE: Color = Color::Indexed(138);
pub(crate) const BLUE: Color = Color::Indexed(109);
pub(crate) const AMBER: Color = Color::Indexed(137);
pub(crate) const LAVENDER: Color = Color::Indexed(139);
pub(crate) const TEAL: Color = Color::Indexed(73);
pub(crate) const YELLOW: Color = Color::Indexed(144);
