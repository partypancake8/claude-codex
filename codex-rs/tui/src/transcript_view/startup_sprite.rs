//! Locate the complete startup mascot in the retained viewport, including after scrolling.
use super::*;
use crate::history_cell::SessionHeaderHistoryCell;
use crate::history_cell::SessionInfoCell;

impl TranscriptView {
    pub(crate) fn startup_sprite_area(&self, cells: &[Arc<dyn HistoryCell>]) -> Option<Rect> {
        let header = cells.iter().find(|cell| {
            cell.as_any().is::<SessionInfoCell>() || cell.as_any().is::<SessionHeaderHistoryCell>()
        })?;
        let key = EntryKey::cell(header);
        self.visible_cell_area(key, SessionInfoCell::sprite_area(self.area.width)?)
    }

    fn visible_cell_area(&self, key: EntryKey, mut area: Rect) -> Option<Rect> {
        let start = self
            .visible
            .iter()
            .position(|row| row.key == key && row.row == usize::from(area.y))?;
        let visible = self.visible.get(start..start + usize::from(area.height))?;
        if !visible
            .iter()
            .enumerate()
            .all(|(offset, row)| row.key == key && row.row == usize::from(area.y) + offset)
        {
            return None;
        }
        area.x += self.area.x;
        area.y = self.area.y + start as u16;
        Some(area)
    }
}

#[cfg(test)]
#[path = "startup_sprite_tests.rs"]
mod tests;
