use super::*;
use crate::keymap::KeymapContext;
use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyModifiers;

#[test]
fn task_shortcut_yields_to_configured_transcript_bindings_and_chords() {
    let event = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
    let contexts = KeymapContextSet::new(KeymapContext::Global).with(KeymapContext::Editor);
    assert!(RuntimeKeymap::defaults().task_shortcut_is_pressed(event, contexts));

    let mut config = TuiKeymap::default();
    config.global.open_transcript =
        Some(KeybindingsSpec::One(KeybindingSpec("ctrl-t".to_string())));
    let runtime = RuntimeKeymap::from_config(&config).expect("legacy transcript shortcut");
    assert!(!runtime.task_shortcut_is_pressed(event, contexts));

    config.global.open_transcript = Some(KeybindingsSpec::One(KeybindingSpec(
        "ctrl-t ctrl-t".to_string(),
    )));
    let runtime = RuntimeKeymap::from_config(&config).expect("transcript chord");
    assert!(!runtime.task_shortcut_is_pressed(event, contexts));
}
