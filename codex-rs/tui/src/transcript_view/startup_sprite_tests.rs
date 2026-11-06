use super::super::tests::cell;
use super::super::tests::render;
use super::*;

#[test]
fn mascot_only_draws_when_all_its_rows_are_visible() {
    let cells = vec![
        cell("header 0\nheader 1\nheader 2\nheader 3\nheader 4"),
        cell("response 0\nresponse 1\nresponse 2\nresponse 3\nresponse 4"),
    ];
    let mut view = TranscriptView::default();
    let key = EntryKey::cell(&cells[0]);
    let sprite = Rect::new(1, 0, 12, 5);
    render(&mut view, &cells, 80, 8);
    assert_eq!(view.visible_cell_area(key, sprite), None);
    view.scroll(&cells, -100);
    render(&mut view, &cells, 80, 8);
    assert_eq!(view.visible_cell_area(key, sprite), Some(sprite));
    view.scroll(&cells, 1);
    render(&mut view, &cells, 80, 8);
    assert_eq!(view.visible_cell_area(key, sprite), None);
}

#[test]
fn mascot_respects_viewport_offset_and_bottom_clipping() {
    let cells = vec![cell("header 0\nheader 1\nheader 2\nheader 3\nheader 4")];
    let mut view = TranscriptView::default();
    let key = EntryKey::cell(&cells[0]);
    let sprite = Rect::new(1, 0, 12, 5);
    let area = Rect::new(3, 2, 80, 8);
    view.render(area, &mut Buffer::empty(area), &cells);
    assert_eq!(
        view.visible_cell_area(key, sprite),
        Some(Rect::new(4, 2, 12, 5))
    );
    render(&mut view, &cells, 80, 3);
    assert_eq!(view.visible_cell_area(key, sprite), None);
}
