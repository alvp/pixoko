//! Unit C3 (specs/ui-palette-handle.md), the handle of the palette: rows 4 and 5 (geometry and
//! destination; state machine and preview), the pending resize of the panel (row 7 in its part of
//! the panel) and property 2 (the transaction does not touch the document). Row 9 (the icon row)
//! is in `edit_layout.rs`. The egui bench has no window; nothing is captured from the desktop.
//!
//! **Cursor.** `Grab`/`Grabbing` are read from `platform_output.cursor_icon`. If egui does not
//! deliver them in the bench this goes to the manual test (condition 9 of the approval).

use egui::{Color32, CursorIcon, Event};
use proptest::prelude::*;
use px_core::{Command, Image, ImageId, Mode, Rgba};
use px_ui::color::ColorChoice;
use px_ui::edit_layout::{BUTTON_H_U, ICON_ROW_Y_U, PaletteButton, button_rect_u};
use px_ui::help::HelpTarget;
use px_ui::palette_grid::{CELL_U, Grid, PITCH_U, total_rows_cols};
use px_ui::palette_ops::PaletteOp;
use px_ui::palette_resize::{
    HANDLE_HINT, HandleDrag, HandleEnd, MAX_ENTRIES_HINT, MAX_LEN, PendingResize, autoscroll_rows,
    dimmed, drag_status, drag_zone_cells, handle_bars, handle_index, resize_prompt,
};
use px_ui::theme::{ACCENT, RING_DEST, SURFACE_1, SURFACE_2, SURFACE_3, TEXT_DIM, contrast_ratio};
use px_ui::view::ScreenRect;

#[path = "common/c3_bench.rs"]
mod bench;
#[path = "common/icons.rs"]
mod icons;
#[path = "common/zones.rs"]
mod zones;

use bench::{Bench, Frame, PPPS, SCREEN_HIGH, indexed};

const BLACK: Color32 = Color32::from_rgb(0, 0, 0);

fn bench8(ppp: f32) -> Bench {
    Bench::new(ppp, SCREEN_HIGH, indexed(8), ColorChoice::Entry(3))
}

fn color32(color: Rgba) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

/// The cell `index` is painted with a fill within one unit per channel of `expected`.
fn cell_near(bench: &Bench, f: &Frame, index: usize, expected: Color32) -> bool {
    let cell = bench.cell_rect(index);
    let close = |a: u8, b: u8| a.abs_diff(b) <= 1;
    f.fills.iter().any(|(rect, fill)| {
        bench::near_rect(*rect, cell)
            && close(fill.r(), expected.r())
            && close(fill.g(), expected.g())
            && close(fill.b(), expected.b())
    })
}

fn dimmed_color32(color: Rgba) -> Color32 {
    let d = dimmed(color);
    Color32::from_rgb(d.r, d.g, d.b)
}

fn color_of(bench: &Bench, index: usize) -> Rgba {
    bench
        .image
        .palette()
        .and_then(|p| p.get(index as u8).ok())
        .expect("the entry exists")
}

// ---------------------------------------------------------------------------------------------
// Row 4: geometry and destination
// ---------------------------------------------------------------------------------------------

#[test]
fn row_4_the_handle_only_exists_in_indexed_images_and_sits_in_cell_len() {
    for len in [1usize, 16, 255, 256] {
        assert_eq!(handle_index(&indexed(len)), Some(len), "len {len}");
    }
    for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        let image = Image::new(4, 4, mode).expect("image");
        assert_eq!(handle_index(&image), None, "{mode:?}");
    }
    // With 256 entries the handle is cell 256: row 17 at 16 columns.
    assert_eq!(256 / 16, 16, "row index 16 is the 17th row");
    assert_eq!(total_rows_cols(257, 16), 17);
}

#[test]
fn row_4_len_at_gives_the_clamped_index_of_the_cell_under_the_point() {
    for n in 1u32..=3 {
        let k = i64::from(n);
        for columns in [10usize, 16, 23] {
            for preview in [1usize, 8, 16, 100, 256] {
                let cells = drag_zone_cells(preview, columns);
                let rows = cells / columns;
                for first_row in [0, rows.saturating_sub(2)] {
                    let visible = 3.min(rows - first_row).max(1);
                    let grid = Grid::with_columns(n, columns, cells, visible, first_row);
                    for shown in 0..visible {
                        for col in 0..columns {
                            let index = (first_row + shown) * columns + col;
                            let center = (
                                col as i64 * PITCH_U * k + CELL_U * k / 2,
                                shown as i64 * PITCH_U * k + CELL_U * k / 2,
                            );
                            let expected = u16::try_from(index.clamp(1, MAX_LEN)).unwrap();
                            assert_eq!(
                                grid.len_at(center.0, center.1),
                                Some(expected),
                                "n {k}, {columns} cols, cell {index}"
                            );
                            // The 2 u gap splits at its middle: 1 u goes to each neighbor.
                            if col + 1 < columns {
                                let left = col as i64 * PITCH_U * k + CELL_U * k;
                                let expected_left = expected;
                                let expected_right =
                                    u16::try_from((index + 1).clamp(1, MAX_LEN)).unwrap();
                                assert_eq!(grid.len_at(left, center.1), Some(expected_left));
                                assert_eq!(grid.len_at(left + k, center.1), Some(expected_right));
                            }
                        }
                    }
                    // Outside the zone: left of the grid, above it, past the last column and
                    // under the last row of the zone.
                    assert_eq!(grid.len_at(-2 * k, 0), None);
                    assert_eq!(grid.len_at(0, -2 * k), None);
                    assert_eq!(grid.len_at(columns as i64 * PITCH_U * k + k, 0), None);
                    assert_eq!(grid.len_at(0, visible as i64 * PITCH_U * k + k), None);
                }
            }
        }
    }
}

#[test]
fn row_4_the_zone_has_one_extra_row_while_dragging_and_stops_at_cell_256() {
    assert_eq!(
        drag_zone_cells(8, 16),
        2 * 16,
        "9 cells are a row; one more"
    );
    assert_eq!(drag_zone_cells(15, 16), 2 * 16);
    assert_eq!(
        drag_zone_cells(16, 16),
        3 * 16,
        "the handle falls in the second row"
    );
    assert_eq!(
        drag_zone_cells(255, 16),
        17 * 16,
        "capped at the row of cell 256"
    );
    assert_eq!(drag_zone_cells(256, 16), 17 * 16);
    // The handle of a 256-entry palette has its own row, the 17th, at 16 columns.
    let grid = Grid::with_columns(1, 16, drag_zone_cells(256, 16), 17, 0);
    let handle = grid.cell_rect(256).expect("cell 256 exists");
    assert_eq!((handle.x, handle.y), (0, 16 * PITCH_U));
    assert_eq!(grid.len_at(handle.x + 4, handle.y + 4), Some(256));
}

#[test]
fn row_4_the_bars_of_the_handle_are_two_by_eight_units_two_apart_and_centered() {
    for n in 1u32..=3 {
        let k = i64::from(n);
        let cell = ScreenRect {
            x: 5 * k,
            y: 7 * k,
            width: CELL_U * k,
            height: CELL_U * k,
        };
        let [left, right] = handle_bars(cell, n);
        for bar in [left, right] {
            assert_eq!((bar.width, bar.height), (2 * k, 8 * k));
            assert_eq!(bar.y, cell.y + 2 * k);
        }
        assert_eq!(right.x - (left.x + left.width), 2 * k, "2 u apart");
        assert_eq!(
            left.x - cell.x,
            cell.x + cell.width - (right.x + right.width)
        );
    }
    // Contrast of the bars against the face (≥ 3:1), at rest and with the pointer over it.
    assert!(contrast_ratio(TEXT_DIM, SURFACE_2) >= 3.0);
    assert!(contrast_ratio(ACCENT, SURFACE_3) >= 3.0);
}

// ---------------------------------------------------------------------------------------------
// Row 5: the state machine and the preview
// ---------------------------------------------------------------------------------------------

#[test]
fn row_5_the_machine_clicks_below_4n_and_drags_from_4n() {
    let mut drag = HandleDrag::press(8, (100, 100));
    drag.update((100 + 4 * 2 - 1, 100), 2, Some(12));
    assert!(!drag.dragging());
    assert_eq!(drag.release(), HandleEnd::Click);
    let mut drag = HandleDrag::press(8, (100, 100));
    drag.update((100 + 4 * 2, 100), 2, Some(12));
    assert!(drag.dragging());
    assert_eq!((drag.original_len(), drag.preview_len()), (8, 12));
    assert_eq!(drag.release(), HandleEnd::Resize { len: 12 });
}

#[test]
fn row_5_outside_the_zone_the_preview_is_the_original_and_releasing_does_nothing() {
    let mut drag = HandleDrag::press(8, (0, 0));
    drag.update((50, 0), 1, Some(20));
    assert_eq!(drag.preview_len(), 20);
    drag.update((60, 0), 1, None);
    assert_eq!(drag.preview_len(), 8, "no destination: the original");
    assert_eq!(drag.release(), HandleEnd::Nothing);
    let mut drag = HandleDrag::press(8, (0, 0));
    drag.update((50, 0), 1, Some(8));
    assert_eq!(
        drag.release(),
        HandleEnd::Nothing,
        "the same length is nothing"
    );
}

#[test]
fn row_5_dragging_previews_the_new_entries_black_with_a_ring_and_nothing_else_changes() {
    for ppp in PPPS {
        let mut bench = bench8(ppp);
        let before = bench.image.clone();
        let handle = bench.handle_center();
        bench.moves(handle);
        bench.presses(handle);
        let f = bench.moves(bench.cell_center(12));
        assert!(bench.ops.busy(), "ppp {ppp}");
        assert_eq!(
            bench.ops.handle_state().map(HandleDrag::preview_len),
            Some(12)
        );
        // Entries 8 to 11 are new: black and with a 1 u ring of ACCENT; the rest as they were.
        for index in 8..12 {
            assert!(bench.cell_is(&f, index, BLACK), "ppp {ppp}: cell {index}");
            assert!(
                bench.there_is_ring(&f, index, 1, ACCENT),
                "ppp {ppp}: ring {index}"
            );
        }
        for index in 0..8 {
            assert!(
                bench.cell_is(&f, index, color32(color_of(&bench, index))),
                "ppp {ppp}: entry {index} as it was"
            );
        }
        // The panel asks for nothing and the document is the same until the button goes up.
        assert!(bench.ops_asked().is_empty());
        assert!(bench.choices().is_empty());
        assert_eq!(bench.image, before);
        assert_eq!(bench.doc_state, 0);
        assert_eq!(f.hint(), Some(drag_status(8, 12).as_str()));
    }
}

#[test]
fn row_5_cutting_dims_the_entries_and_stretching_again_without_releasing_restores_them() {
    for ppp in PPPS {
        let mut bench = bench8(ppp);
        let handle = bench.handle_center();
        bench.moves(handle);
        bench.presses(handle);
        let f = bench.moves(bench.cell_center(5));
        assert_eq!(
            bench.ops.handle_state().map(HandleDrag::preview_len),
            Some(5)
        );
        for index in 5..8 {
            let color = color_of(&bench, index);
            assert!(
                cell_near(&bench, &f, index, dimmed_color32(color)),
                "ppp {ppp}: entry {index} dimmed"
            );
            assert!(
                !bench.there_is_ring(&f, index, 1, ACCENT),
                "no ring on a dimmed one"
            );
        }
        for index in 0..5 {
            assert!(bench.cell_is(&f, index, color32(color_of(&bench, index))));
        }
        // 25 % over SURFACE_1: darker than the original and not black.
        let c = color_of(&bench, 6);
        assert_eq!(
            dimmed(c).r,
            ((u16::from(c.r) + 3 * u16::from(SURFACE_1.r())) / 4) as u8
        );
        // Stretch again without releasing: the cut ones come back intact.
        let f = bench.moves(bench.cell_center(12));
        for index in 5..8 {
            assert!(
                bench.cell_is(&f, index, color32(color_of(&bench, index))),
                "ppp {ppp}"
            );
        }
        assert_eq!(f.hint(), Some(drag_status(8, 12).as_str()));
    }
}

#[test]
fn row_5_leaving_the_grid_keeps_the_drag_on_the_nearest_cell() {
    let mut bench = bench8(1.0);
    let handle = bench.handle_center();
    bench.moves(handle);
    bench.presses(handle);
    bench.moves(bench.cell_center(12));
    // Below the grid (the tail is not part of the zone): the last row of the zone, column of the
    // pointer. The drag does not drop back to the original length.
    let f = bench.moves((bench.cell_center(5).0, 300));
    assert!(bench.ops.busy());
    let len = bench
        .ops
        .handle_state()
        .map(HandleDrag::preview_len)
        .expect("dragging");
    assert_eq!(len % 16, 5, "the column of the pointer: {len}");
    assert!((1..=MAX_LEN).contains(&len));
    assert_eq!(f.cursor, CursorIcon::Grabbing);
    // To the right of everything (over the canvas): the last column.
    bench.moves((700, bench.cell_center(5).1));
    assert_eq!(
        bench.ops.handle_state().map(HandleDrag::preview_len),
        Some(15)
    );
    let f = bench.releases((700, bench.cell_center(5).1));
    assert_eq!(f.output.op, Some(PaletteOp::ResizeTo { len: 15 }));
}

#[test]
fn row_5_dragging_past_the_maximum_and_out_of_the_grid_applies_256_and_keeps_the_view() {
    let mut bench = Bench::with_panel_height(1.0, 400, indexed(250), ColorChoice::Entry(3));
    bench.scroll.first_row = 5;
    bench.rest();
    assert!(bench.scroll.first_row > 0, "the view is scrolled down");
    let handle = bench.handle_center();
    bench.moves(handle);
    bench.presses(handle);
    // The zone has one row more while dragging: the wheel brings the last row into sight.
    bench.moves(bench.cell_center(255));
    bench.wheel(PITCH_U);
    let first_row = bench.scroll.first_row;
    assert_eq!(first_row, 5, "17 rows, 12 of them visible");
    // Past the maximum: cell 270 is in the zone but the palette cannot have 270 entries.
    bench.moves(bench.cell_center(270));
    assert_eq!(
        bench.ops.handle_state().map(HandleDrag::preview_len),
        Some(256)
    );
    // Out of the grid, over the canvas: the drag goes on, still 256, and the view stays.
    bench.moves((700, 300));
    assert!(bench.ops.busy(), "the drag is alive");
    assert_eq!(
        bench.ops.handle_state().map(HandleDrag::preview_len),
        Some(256)
    );
    assert_eq!(bench.scroll.first_row, first_row, "the view does not jump");
    // Releasing there applies the clamped length, once; the view stays after the change.
    let f = bench.releases((700, 300));
    assert_eq!(f.output.op, Some(PaletteOp::ResizeTo { len: 256 }));
    bench.applies(Command::ResizePalette {
        image: ImageId::from_raw(0),
        len: 256,
        into: None,
    });
    bench.rest();
    bench.rest();
    assert_eq!(
        bench.scroll.first_row, first_row,
        "the view is not reset on release"
    );
    assert!(bench.ops_asked().len() == 1);
}

#[test]
fn row_5_at_the_maximum_the_status_line_says_so_while_it_lasts() {
    let mut bench = Bench::with_panel_height(1.0, 400, indexed(250), ColorChoice::Entry(3));
    bench.scroll.first_row = 5;
    bench.rest();
    let handle = bench.handle_center();
    bench.moves(handle);
    bench.presses(handle);
    bench.moves(bench.cell_center(255));
    bench.wheel(PITCH_U);
    let f = bench.moves(bench.cell_center(270));
    assert_eq!(f.hint(), Some(MAX_ENTRIES_HINT));
    let f = bench.moves((700, 300));
    assert_eq!(f.hint(), Some(MAX_ENTRIES_HINT), "also out of the grid");
    // Back under the maximum the usual text comes back.
    let f = bench.moves(bench.cell_center(255));
    assert_eq!(f.hint(), Some(drag_status(250, 255).as_str()));
}

#[test]
fn row_5_escape_still_cancels_a_drag_that_is_outside_the_grid() {
    let mut bench = Bench::with_panel_height(1.0, 400, indexed(250), ColorChoice::Entry(3));
    bench.scroll.first_row = 5;
    bench.rest();
    let handle = bench.handle_center();
    bench.moves(handle);
    bench.presses(handle);
    bench.moves(bench.cell_center(255));
    bench.wheel(PITCH_U);
    bench.moves(bench.cell_center(270));
    bench.moves((700, 300));
    assert!(bench.ops.busy());
    bench.escape();
    assert!(bench.ops.handle_state().is_none());
    let f = bench.releases((700, 300));
    assert_eq!(f.output.op, None);
    assert!(bench.ops_asked().is_empty());
}

#[test]
fn row_5_escape_and_losing_the_window_focus_cancel_one_link_at_a_time() {
    for blur in [false, true] {
        let mut bench = bench8(1.0);
        let handle = bench.handle_center();
        bench.moves(handle);
        bench.presses(handle);
        bench.moves(bench.cell_center(12));
        assert!(bench.ops.busy());
        let f = if blur {
            bench.frame(vec![Event::WindowFocused(false)])
        } else {
            bench.escape()
        };
        assert!(bench.ops.handle_state().is_none(), "blur {blur}");
        assert!(!bench.ops.busy());
        assert_eq!(f.output.op, None);
        assert!(
            !f.output.close_editor && !f.output.escape_free,
            "that Esc was the drag's"
        );
        // Letting go afterwards asks for nothing.
        let f = bench.releases(bench.cell_center(12));
        assert_eq!(f.output.op, None);
        assert!(bench.ops_asked().is_empty());
        assert_eq!(bench.doc_state, 0);
    }
}

#[test]
fn row_5_releasing_gives_exactly_one_request_with_the_last_valid_length() {
    for ppp in PPPS {
        let mut bench = bench8(ppp);
        let handle = bench.handle_center();
        bench.moves(handle);
        bench.presses(handle);
        bench.moves(bench.cell_center(12));
        bench.moves(bench.cell_center(5));
        let target = bench.cell_center(10);
        bench.moves(target);
        let f = bench.releases(target);
        assert_eq!(
            f.output.op,
            Some(PaletteOp::ResizeTo { len: 10 }),
            "ppp {ppp}"
        );
        bench.rest();
        bench.rest();
        assert_eq!(bench.ops_asked(), vec![PaletteOp::ResizeTo { len: 10 }]);
        assert!(!bench.ops.busy());
        assert!(bench.choices().is_empty(), "the handle does not choose");
    }
}

#[test]
fn row_5_a_lost_release_is_a_release_and_going_back_to_the_original_asks_for_nothing() {
    let mut bench = bench8(1.0);
    let handle = bench.handle_center();
    bench.moves(handle);
    bench.presses(handle);
    bench.moves(bench.cell_center(12));
    // Back to the original cell without releasing: the preview is the initial one.
    bench.moves(handle);
    assert_eq!(
        bench.ops.handle_state().map(HandleDrag::preview_len),
        Some(8)
    );
    let f = bench.releases(handle);
    assert_eq!(f.output.op, None);
    assert!(bench.ops_asked().is_empty());
}

#[test]
fn row_5_pressing_below_4n_is_a_click_and_the_handle_never_chooses_or_clears_the_range() {
    let mut bench = bench8(2.0);
    bench.ops.set_range_end(5);
    let handle = bench.handle_center();
    bench.moves(handle);
    let pressed = bench.presses(handle);
    assert_eq!(pressed.output.choice, None);
    assert!(!bench.ops.busy(), "armed is not busy");
    let at = (handle.0 + 4 * bench.n() - 1, handle.1);
    let f = bench.releases(at);
    assert_eq!(f.output.op, None);
    assert!(bench.history_is_empty());
    assert_eq!(bench.ops.range_end(), Some(5), "the range stays");
}

#[test]
fn row_5_with_enabled_false_the_handle_neither_starts_nor_warns() {
    for locked in ["gesture", "slider"] {
        let mut bench = bench8(1.0);
        let handle = bench.handle_center();
        if locked == "gesture" {
            bench.gesture_drawing = true;
        } else {
            // A slider in progress: the editor is open and a drag of R is on.
            bench.view.editor_open = true;
            let layout = bench.layout();
            let y = layout.top_px + 4 * bench.n() + 7 * bench.n();
            let x = 100 * bench.n();
            bench.moves((x, y));
            bench.presses((x, y));
            assert!(bench.editor.dragging());
            bench.history.clear();
        }
        bench.moves(handle);
        bench.presses(handle);
        bench.moves(bench.cell_center(12));
        assert!(bench.ops.handle_state().is_none(), "{locked}");
        assert!(
            bench.ops_asked().is_empty() && bench.notices().is_empty(),
            "{locked}"
        );
    }
}

#[test]
fn row_5_the_cursor_is_grab_over_the_handle_and_grabbing_while_dragging() {
    let mut bench = bench8(1.0);
    let handle = bench.handle_center();
    let f = bench.moves(handle);
    assert_eq!(f.cursor, CursorIcon::Grab);
    assert_eq!(f.hint(), Some(HANDLE_HINT));
    // `F1` over the handle opens its help.
    assert_eq!(f.info.help, Some(HelpTarget::PaletteHandle));
    bench.presses(handle);
    let f = bench.moves(bench.cell_center(12));
    assert_eq!(f.cursor, CursorIcon::Grabbing);
    let f = bench.moves((20, 300));
    assert_ne!(f.cursor, CursorIcon::Grab);
}

#[test]
fn row_5_the_wheel_scrolls_the_rows_during_the_drag_and_the_zone_follows() {
    let mut bench = Bench::with_panel_height(1.0, 241, indexed(40), ColorChoice::Entry(3));
    // Block-height panel: one visible row. Drag the handle (cell 40, row 2) toward the row above.
    bench.scroll.first_row = 2;
    bench.rest();
    let handle = bench.handle_center();
    bench.moves(handle);
    bench.presses(handle);
    bench.moves((handle.0 + 20, handle.1));
    assert!(bench.ops.busy());
    bench.wheel(-PITCH_U);
    assert_eq!(bench.scroll.first_row, 1, "the wheel scrolled one row up");
    assert!(bench.ops.busy(), "and the drag goes on");
    let len = bench
        .ops
        .handle_state()
        .map(HandleDrag::preview_len)
        .expect("dragging");
    assert!((1..=MAX_LEN).contains(&len));
}

#[test]
fn row_5_autoscroll_is_one_row_per_120_ms_inside_the_half_row_bands() {
    assert_eq!(autoscroll_rows(0, 1), 0);
    assert_eq!(autoscroll_rows(119, 1), 0);
    assert_eq!(autoscroll_rows(120, 1), 1);
    assert_eq!(autoscroll_rows(120, -1), -1);
    assert_eq!(autoscroll_rows(250, -1), -2);
    assert_eq!(
        autoscroll_rows(1000, 0),
        0,
        "outside the bands nothing scrolls"
    );
}

// ---------------------------------------------------------------------------------------------
// Row 7 (panel part): the pending resize
// ---------------------------------------------------------------------------------------------

fn pending(len: u16, into: Option<u8>, pixels: u64) -> PendingResize {
    PendingResize {
        len,
        into,
        pixels,
        transparent: None,
    }
}

#[test]
fn row_7_a_pending_resize_dims_the_cut_entries_and_shows_confirm_and_cancel() {
    for ppp in PPPS {
        let mut bench = bench8(ppp);
        bench.ops.begin_resize(pending(5, None, 3));
        let f = bench.rest();
        for index in 5..8 {
            let color = color_of(&bench, index);
            assert!(
                cell_near(&bench, &f, index, dimmed_color32(color)),
                "ppp {ppp}: {index}"
            );
        }
        assert!(f.paints("Confirmar") && f.paints("Cancelar"), "ppp {ppp}");
        assert_eq!(f.hint(), Some(resize_prompt(&pending(5, None, 3)).as_str()));
        // Confirm and Cancel sit where the row of icons was (y = 38 u of the tail).
        let (x, y, w, h) = button_rect_u(Mode::Indexed, PaletteButton::Confirm).expect("rect");
        assert_eq!((y, h), (ICON_ROW_Y_U, BUTTON_H_U));
        assert!(w > 0 && x == 4);
    }
}

#[test]
fn row_7_only_a_kept_swatch_sets_the_destination_and_it_does_not_change_the_choice() {
    let mut bench = bench8(1.0);
    bench.ops.begin_resize(pending(5, None, 3));
    bench.rest();
    // A swatch that is cut off (6) is ignored.
    let f = bench.click(bench.cell_center(6));
    assert_eq!(bench.ops.resize_pending().and_then(|r| r.into), None);
    assert_eq!(f.output.choice, None);
    // A kept one (2) is the destination.
    let f = bench.click(bench.cell_center(2));
    assert_eq!(bench.ops.resize_pending().and_then(|r| r.into), Some(2));
    assert_eq!(f.output.choice, None);
    assert_eq!(bench.foreground, ColorChoice::Entry(3));
    let f = bench.rest();
    assert!(
        bench.there_is_ring(&f, 2, 2, RING_DEST),
        "the destination ring"
    );
    assert_eq!(
        f.hint(),
        Some(resize_prompt(&pending(5, Some(2), 3)).as_str())
    );
}

#[test]
fn row_7_confirm_without_a_destination_warns_and_with_one_asks_for_the_resize() {
    let mut bench = bench8(1.0);
    bench.ops.begin_resize(pending(5, None, 3));
    bench.rest();
    let tail = bench.tail_top();
    let (x, y, w, h) = button_rect_u(Mode::Indexed, PaletteButton::Confirm).expect("rect");
    let n = bench.n();
    let confirm = ((x + w / 2) * n, tail + (y + h / 2) * n);
    let f = bench.click(confirm);
    assert_eq!(f.output.op, None);
    assert_eq!(
        f.output.notice.as_deref(),
        Some("elige un destino pulsando una muestra")
    );
    assert!(bench.ops.resize_pending().is_some(), "it stays pending");
    bench.click(bench.cell_center(2));
    let f = bench.click(confirm);
    assert_eq!(
        f.output.op,
        Some(PaletteOp::Resize {
            len: 5,
            into: Some(2)
        })
    );
    // Without pixels to move Confirm needs no destination.
    let mut bench = bench8(1.0);
    bench.ops.begin_resize(pending(5, None, 0));
    bench.rest();
    let f = bench.click(confirm);
    assert_eq!(f.output.op, Some(PaletteOp::Resize { len: 5, into: None }));
}

#[test]
fn row_7_cancel_and_escape_cancel_the_pending_resize_without_a_notice() {
    for how in ["cancel", "escape"] {
        let mut bench = bench8(1.0);
        bench.ops.begin_resize(pending(5, Some(1), 3));
        bench.rest();
        bench.history.clear();
        if how == "escape" {
            bench.escape();
        } else {
            let tail = bench.tail_top();
            let (x, y, w, h) = button_rect_u(Mode::Indexed, PaletteButton::Cancel).expect("rect");
            let n = bench.n();
            bench.click(((x + w / 2) * n, tail + (y + h / 2) * n));
        }
        assert!(bench.ops.resize_pending().is_none(), "{how}");
        assert!(bench.ops_asked().is_empty(), "{how}");
        assert!(bench.notices().is_empty(), "{how}");
    }
}

#[test]
fn row_7_a_change_of_the_document_cancels_the_pending_resize_once() {
    let mut bench = bench8(1.0);
    bench.ops.begin_resize(pending(5, None, 3));
    bench.rest();
    bench.doc_state += 1;
    bench.rest();
    assert!(bench.ops.resize_pending().is_none());
    assert!(bench.ops.take_resize_cancelled());
    assert!(!bench.ops.take_resize_cancelled(), "said once");
    // The handle is inert while the resize is pending.
    let mut bench = bench8(1.0);
    bench.ops.begin_resize(pending(5, None, 3));
    bench.rest();
    let handle = bench.cell_center(5);
    bench.moves(handle);
    bench.presses(handle);
    bench.moves((handle.0 + 40, handle.1));
    assert!(bench.ops.handle_state().is_none());
}

// ---------------------------------------------------------------------------------------------
// Property 2: the transaction does not touch the document
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Step {
    PressHandle,
    PressElsewhere,
    MoveToCell(usize),
    MoveOutside,
    Wheel(i64),
    Escape,
    Blur,
    Release,
}

fn step_strategy() -> impl Strategy<Value = Step> {
    prop_oneof![
        Just(Step::PressHandle),
        Just(Step::PressElsewhere),
        (0usize..300).prop_map(Step::MoveToCell),
        Just(Step::MoveOutside),
        (-3i64..=3).prop_map(Step::Wheel),
        Just(Step::Escape),
        Just(Step::Blur),
        Just(Step::Release),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_2_until_release_nothing_changes_and_release_asks_for_at_most_one_request(
        entries in 1usize..=256,
        steps in proptest::collection::vec(step_strategy(), 0..14),
    ) {
        let mut bench = Bench::with_panel_height(1.0, 400, indexed(entries), ColorChoice::Entry(0));
        // Bring the row of the handle into sight (the panel shows 12 rows).
        bench.scroll.first_row = (entries / 16).saturating_sub(4);
        bench.rest();
        let before = bench.image.clone();
        let range = bench.ops.range_end();
        let mut pointer = bench.handle_center();
        for step in steps {
            let asked_before = bench.ops_asked().len();
            let preview_before = bench.ops.handle_state().map(HandleDrag::preview_len);
            let dragging = bench.ops.busy();
            match &step {
                Step::PressHandle => {
                    pointer = bench.handle_center();
                    bench.moves(pointer);
                    bench.presses(pointer);
                }
                Step::PressElsewhere => {
                    pointer = (700, 300);
                    bench.moves(pointer);
                    bench.presses(pointer);
                }
                Step::MoveToCell(cell) => {
                    pointer = bench.cell_center(*cell);
                    bench.moves(pointer);
                }
                Step::MoveOutside => {
                    pointer = (20, 300);
                    bench.moves(pointer);
                }
                Step::Wheel(rows) => {
                    bench.wheel(rows * PITCH_U);
                }
                Step::Escape => {
                    bench.escape();
                }
                Step::Blur => {
                    bench.frame(vec![Event::WindowFocused(false)]);
                }
                Step::Release => {
                    bench.releases(pointer);
                }
            }
            // The document, the history, the choice and the range never change by themselves.
            prop_assert_eq!(&bench.image, &before);
            prop_assert_eq!(bench.doc_state, 0);
            prop_assert_eq!(bench.ops.range_end(), range);
            if let Some(len) = bench.ops.handle_state().map(HandleDrag::preview_len) {
                prop_assert!((1..=MAX_LEN).contains(&len));
            }
            let asked = bench.ops_asked();
            match step {
                Step::Release => {
                    prop_assert!(asked.len() <= asked_before + 1);
                    if asked.len() > asked_before {
                        let Some(PaletteOp::ResizeTo { len }) = asked.last().copied() else {
                            return Err(TestCaseError::fail("only a resize request may come out"));
                        };
                        prop_assert!((1..=256).contains(&len));
                        prop_assert!(dragging);
                        prop_assert_eq!(Some(usize::from(len)), preview_before);
                        prop_assert_ne!(usize::from(len), entries);
                    }
                }
                Step::Escape | Step::Blur if dragging => {
                    prop_assert_eq!(asked.len(), asked_before, "Esc and blur leave no request");
                    prop_assert!(!bench.ops.busy());
                }
                _ => prop_assert_eq!(asked.len(), asked_before),
            }
        }
    }
}
