//! The task-view shortcut yields to every explicitly assigned main-surface key.

use super::KeymapContextSet;
use super::RuntimeKeymap;
use super::runtime_action_bindings;
use crate::key_hint;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;

impl RuntimeKeymap {
    pub(crate) fn task_shortcut_is_pressed(
        &self,
        event: KeyEvent,
        contexts: KeymapContextSet,
    ) -> bool {
        key_hint::ctrl(KeyCode::Char('t')).is_press(event)
            && !self.chords.bindings.iter().any(|binding| {
                contexts.contains(binding.action.context) && binding.chord.prefix.is_press(event)
            })
            && !runtime_action_bindings(self).any(|action| {
                contexts.contains(action.id.context) && action.bindings.is_pressed(event)
            })
    }
}

#[cfg(test)]
#[path = "task_shortcut_tests.rs"]
mod tests;
