//! Unit C2 (specs/ui-single-column.md), pure part: rows 1 to 4 (widths and clamp, grid of the
//! toolbox, vertical split and minimums, `window_layout` and canvas origin) and property 1 (integer
//! geometry without overlaps of the column). No egui.
//!
//! The numbers come from the spec, written as literals: `W(k) = 14·k + 9`; `k_max(w) = max(10,
//! ⌊(w − 137·n)/(14·n)⌋)`; `c(k) = ⌊(14k + 2)/25⌋` buttons per row; with `r` rows of tools, the
//! toolbox measures `25r + 58` u and the palette starts at `head = 25r + 59`. The editor, which
//! needs 233 u, widens the column to 16 columns while it is open or a removal is pending
//! (`wide_cols`; visto bueno de C2).

use proptest::prelude::*;
use px_core::Mode;
use px_ui::chrome::{
    COLUMN_DEFAULT_COLS, COLUMN_EDITOR_COLS, COLUMN_MIN_COLS, MENU_H_U, canvas_origin_px,
    column_cols, column_width_u, columns_per_row, head_u, max_cols, tool_rows,
    toolbox_button_rect_u, toolbox_button_rect_u_at, toolbox_height_u, toolbox_hit_at,
    toolbox_separator_y_u, wide_cols, window_layout, window_layout_cols, zoom_label_y_u,
    zoom_row_y_u,
};
use px_ui::edit_layout::{
    MIN_CANVAS_WIDTH_U, PaletteButton, block_height_closed_px, block_height_px, button_rect_u,
    column_layout, editor_height_u, min_panel_height_px, min_width_terms_u, min_window_height_pt,
    min_window_width_pt, min_window_width_u, panel_layout, panel_layout_closed, tail_height_u,
};
use px_ui::layers_panel::PANEL_H_U as LAYERS_PANEL_H_U;
use px_ui::palette_grid::{
    GRID_X_U, GRID_Y_U, Grid, PITCH_U, PanelView, panel_width_u, right_line_x_u, total_rows_cols,
    track_x_u,
};
use px_ui::status::bar_height_px;
use px_ui::text::text_scale;
use px_ui::view::ScreenRect;

const MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];
const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

fn n_of(ppp: f32) -> i64 {
    i64::from(text_scale(ppp))
}

fn ppp_of_the_step(i: u32) -> f32 {
    (100 + 5 * i) as f32 / 100.0
}

// ---------------------------------------------------------------------------------------------
// Row 1: widths and clamp
// ---------------------------------------------------------------------------------------------

#[test]
fn row_1_the_widths_the_maximum_and_the_clamp_of_the_column() {
    assert_eq!(
        (COLUMN_MIN_COLS, COLUMN_DEFAULT_COLS, COLUMN_EDITOR_COLS),
        (10, 16, 16)
    );
    assert_eq!(
        (column_width_u(10), column_width_u(14), column_width_u(16)),
        (149, 205, 233)
    );
    assert_eq!(panel_width_u(16), 233);
    for n in 1..=8u32 {
        let k = i64::from(n);
        // `k_max(346 u) = 14`, `k_max(361 u) = 16`, `k_max(468 u) = 23`; 277 u leaves just 10.
        assert_eq!(max_cols(346 * k, n), 14, "n = {n}");
        assert_eq!(max_cols(361 * k, n), 16, "n = {n}");
        assert_eq!(max_cols(468 * k, n), 23, "n = {n}");
        assert_eq!(max_cols(277 * k, n), 10, "n = {n}");
        // A window narrower than the floor (only with `PIXOKO_NO_MIN_SIZE`) does not panic.
        for w in [-5, 0, 1, 100 * k] {
            assert_eq!(max_cols(w, n), 10, "n = {n}, w = {w}");
            assert_eq!(column_cols(w, n, 40), 10);
        }
    }
    // The clamp, window width pixel by pixel (also odd) and `wanted` from 1 to 64: the canvas
    // never gets less than 128 u (for windows of at least 277 u) and the column yields first.
    for n in [1u32, 2, 3] {
        let k = i64::from(n);
        for w in (277 * k)..=(900 * k) {
            let k_max = max_cols(w, n);
            assert!((10..=64).contains(&k_max));
            for wanted in 1..=64 {
                let cols = column_cols(w, n, wanted);
                assert_eq!(
                    cols,
                    wanted.clamp(10, k_max),
                    "n = {n}, w = {w}, wanted = {wanted}"
                );
                assert!(w - column_width_u(cols) * k >= MIN_CANVAS_WIDTH_U * k);
            }
        }
    }
}

#[test]
fn row_1_what_was_chosen_is_not_touched_when_the_window_narrows_and_comes_back_when_it_widens() {
    let n = 2;
    let wanted = 20;
    let mut seen = Vec::new();
    for w_u in [900, 600, 468, 400, 346, 300, 277, 300, 400, 600, 900] {
        seen.push(column_cols(w_u * i64::from(n), n, wanted));
    }
    assert_eq!(seen, [20, 20, 20, 18, 14, 11, 10, 11, 18, 20, 20]);
    // With the editor open or a removal pending the column is at least 16 columns.
    assert_eq!(wide_cols(8, true), 16);
    assert_eq!(wide_cols(12, true), 16);
    assert_eq!(wide_cols(20, true), 20);
    assert_eq!(wide_cols(10, false), 10);
    // At the minimum window of 468 u there is always room for 16 columns.
    for n in 1..=8u32 {
        let k = i64::from(n);
        assert_eq!(column_cols(468 * k, n, wide_cols(8, true)), 16, "n = {n}");
    }
}

// ---------------------------------------------------------------------------------------------
// Row 2: grid of the toolbox
// ---------------------------------------------------------------------------------------------

#[test]
fn row_2_the_toolbox_grid_with_5_12_and_20_tools_and_k_from_8_to_24() {
    // `c(k)` of the spec.
    for (cols, per_row) in [
        (8, 4),
        (9, 5),
        (10, 5),
        (11, 6),
        (12, 6),
        (13, 7),
        (14, 7),
        (15, 8),
        (16, 9),
        (17, 9),
        (20, 11),
    ] {
        assert_eq!(columns_per_row(cols), per_row, "k = {cols}");
    }
    for tools in [5usize, 12, 20] {
        for cols in 8..=24 {
            let per_row = (14 * cols + 2) / 25;
            let rows = (tools as i64 + per_row - 1) / per_row;
            assert_eq!(columns_per_row(cols), per_row);
            assert_eq!(tool_rows(tools, cols), rows);
            let width = 14 * cols + 9;
            // 24 u buttons, 25 u pitch; five tools and four zoom buttons in the order of today.
            for i in 0..tools {
                let (i64_i, per_row_i) = (i as i64, per_row);
                assert_eq!(
                    toolbox_button_rect_u_at(i, tools, cols),
                    (
                        4 + 25 * (i64_i % per_row_i),
                        4 + 25 * (i64_i / per_row_i),
                        24,
                        24
                    ),
                    "{tools} tools, k = {cols}, button {i}"
                );
            }
            for i in 0..4usize {
                assert_eq!(
                    toolbox_button_rect_u_at(tools + i, tools, cols),
                    (4 + 25 * i as i64, 25 * rows + 30, 24, 24),
                    "{tools} tools, k = {cols}, zoom {i}"
                );
            }
            assert_eq!(toolbox_separator_y_u(tools, cols), 25 * rows + 7);
            assert_eq!(zoom_label_y_u(tools, cols), 25 * rows + 12);
            assert_eq!(zoom_row_y_u(tools, cols), 25 * rows + 30);
            assert_eq!(toolbox_height_u(tools, cols), 25 * rows + 58);
            assert_eq!(head_u(tools, cols), 25 * rows + 59);
            // Inside the column, without touching the edge, and the zoom in a single row.
            for i in 0..tools + 4 {
                let (x, y, w, h) = toolbox_button_rect_u_at(i, tools, cols);
                assert!(
                    x >= 4 && x + w <= width - 4,
                    "{tools} tools, k = {cols}, {i}"
                );
                assert!(y >= 4 && y + h <= toolbox_height_u(tools, cols) - 4);
                // The center hits its own button and the gap before the next one nothing.
                assert_eq!(
                    toolbox_hit_at((x + 12) * 2, (y + 12) * 2, 2, tools, cols),
                    Some(i)
                );
            }
            let (gap_x, gap_y, ..) = toolbox_button_rect_u_at(0, tools, cols);
            assert_eq!(
                toolbox_hit_at((gap_x + 24) * 3, (gap_y + 5) * 3, 3, tools, cols),
                None
            );
        }
    }
    // The legacy entry points are those of 5 tools and 16 columns.
    for i in 0..9 {
        assert_eq!(toolbox_button_rect_u(i), toolbox_button_rect_u_at(i, 5, 16));
    }
    // The icon row hangs under the first grid row of the palette, which starts at `head`.
    for tools in [5usize, 12, 20] {
        for cols in [8, 14, 16] {
            let head = head_u(tools, cols);
            let n = 2;
            let height = (head + 106 + 60) * n;
            let layout = column_layout(Mode::Indexed, height, n as u32, 0, head * n, false);
            assert!(layout.tail_top_px >= (head + GRID_Y_U + PITCH_U) * n);
            assert_eq!(
                layout.top_px,
                layout.tail_top_px + tail_height_u(Mode::Indexed) * n
            );
        }
    }
}

#[test]
fn extra_the_minimum_column_holds_the_whole_icon_row() {
    // The icon row (four icons since C3) ends at 4 + 24 + 8 + 24 + 1 + 24 + 8 + 24 = 117 u and
    // keeps the same 4 u margin at the right as at the left. Since C3 the row no longer sets the
    // minimum column (it was 142 u); the minimum stays at 10 columns (decision 10 of C3).
    let icons = [
        PaletteButton::Remove,
        PaletteButton::RotateLeft,
        PaletteButton::RotateRight,
        PaletteButton::Transparent,
    ];
    let right_edge = |buttons: &[PaletteButton]| {
        buttons
            .iter()
            .map(|b| {
                let (x, _, w, _) = button_rect_u(Mode::Indexed, *b).expect("indexed has buttons");
                x + w
            })
            .max()
            .expect("some button")
    };
    assert_eq!(right_edge(&icons), 117);
    let fits = |cols: i64, right: i64| right + 4 <= column_width_u(cols);
    assert_eq!(COLUMN_MIN_COLS, 10);
    assert_eq!(column_width_u(COLUMN_MIN_COLS), 149);
    assert!(fits(COLUMN_MIN_COLS, 117));
    for cols in COLUMN_MIN_COLS..=64 {
        assert!(fits(cols, 117), "k = {cols}");
        // Five tools of 24 u with gaps are no wider than the row of icons.
        for tools in [5usize, 12, 20] {
            for i in 0..tools + 4 {
                let (x, _, w, _) = toolbox_button_rect_u_at(i, tools, cols);
                assert!(fits(cols, x + w), "{tools} tools, k = {cols}, button {i}");
            }
        }
    }
    // The text buttons of a pending removal never meet the narrow column: it is widened to 16.
    let text = right_edge(&[PaletteButton::Confirm, PaletteButton::Cancel]);
    assert_eq!(text, 229);
    for wanted in 1..=64 {
        assert!(fits(wide_cols(wanted.max(COLUMN_MIN_COLS), true), text));
    }
}

// ---------------------------------------------------------------------------------------------
// Row 3: vertical split and minimums
// ---------------------------------------------------------------------------------------------

#[test]
fn row_3_the_grid_rows_seen_at_the_minimum_closed_and_the_capacity_rule() {
    // Indexed, closed, column of 277 u: the grid shows 7, 5, 3 and 1 rows with `head` of 84, 109,
    // 134 and 159 u.
    for ppp in PPPS {
        let n = n_of(ppp) as u32;
        let k = i64::from(n);
        assert_eq!(min_panel_height_px(n), 277 * k);
        for (head, rows) in [(84, 7), (109, 5), (134, 3), (159, 1)] {
            let layout = column_layout(Mode::Indexed, 277 * k, n, 0, head * k, false);
            assert!(!layout.scrolling, "head {head}");
            assert_eq!(layout.visible_rows, rows, "ppp {ppp}, head {head}");
        }
    }
    // The heads of the table of the spec.
    for (tools, cols, head) in [
        (5, 16, 84),
        (5, 14, 84),
        (5, 8, 109),
        (12, 16, 109),
        (12, 14, 109),
        (12, 8, 134),
        (20, 16, 134),
        (20, 14, 134),
        (20, 10, 159),
        (20, 8, 184),
    ] {
        assert_eq!(head_u(tools, cols), head, "{tools} tools, k = {cols}");
    }
    // The capacity rule: 20 tools at the minimum column (10 columns, 5 per row, 4 rows) fit the
    // minimum window, 26 do not. It fails aloud; it does not clip.
    assert_eq!(tool_rows(20, COLUMN_MIN_COLS), 4);
    assert_eq!(head_u(20, COLUMN_MIN_COLS), 159);
    assert!(head_u(20, COLUMN_MIN_COLS) <= 277 - 106);
    assert_eq!(277 - 106 - head_u(20, COLUMN_MIN_COLS), 12, "12 u to spare");
    assert!(head_u(21, COLUMN_MIN_COLS) > 277 - 106);
    // The layers panel (277 u) and the options window (at most 248 u) fit the minimum.
    for n in 1..=3u32 {
        assert!(LAYERS_PANEL_H_U * i64::from(n) <= min_panel_height_px(n));
        assert!((104 + 18 * 8) * i64::from(n) <= min_panel_height_px(n));
    }
}

#[test]
fn row_3_the_open_block_scrolls_as_a_whole_with_a_low_window_and_head_zero_is_the_old_panel() {
    for ppp in PPPS {
        let n = n_of(ppp) as u32;
        let k = i64::from(n);
        for tools in [5usize, 12, 20] {
            for cols in [16, 14, COLUMN_MIN_COLS] {
                let head = head_u(tools, cols) * k;
                for mode in MODES {
                    let open_block = head + block_height_px(mode, n);
                    let closed_block = head + block_height_closed_px(mode, n);
                    assert!(closed_block <= open_block);
                    // Below the open block: the whole block scrolls and the grid shows one row.
                    let low = open_block - 25 * k;
                    for offset in [0, 7 * k, 5000 * k] {
                        let layout = column_layout(mode, low, n, offset, head, true);
                        assert!(layout.scrolling);
                        assert_eq!(layout.visible_rows, 1);
                        assert_eq!(layout.max_offset_px, 25 * k);
                        assert_eq!(layout.offset_px, offset.clamp(0, 25 * k));
                        assert_eq!(layout.offset_px % k, 0);
                        // Everything hangs from the column's top: the icon row is under the first
                        // grid row of the palette, which starts at `head`, and moves up with the scroll.
                        assert_eq!(
                            layout.tail_top_px,
                            head + (GRID_Y_U + PITCH_U) * k - layout.offset_px
                        );
                    }
                    // With enough height it does not scroll, open or closed.
                    assert!(!column_layout(mode, open_block, n, 0, head, true).scrolling);
                    assert!(!column_layout(mode, closed_block, n, 0, head, false).scrolling);
                }
            }
        }
        // No toolbox: it is the panel of before.
        for mode in MODES {
            for height in [150 * k, 277 * k, 400 * k] {
                for offset in [0, 9 * k, 400 * k] {
                    assert_eq!(
                        column_layout(mode, height, n, offset, 0, true),
                        panel_layout(mode, height, n, offset),
                        "{mode:?}"
                    );
                    assert_eq!(
                        column_layout(mode, height, n, offset, 0, false),
                        panel_layout_closed(mode, height, n, offset),
                        "{mode:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn row_3_opening_the_editor_scrolls_to_the_maximum_and_closing_it_brings_the_previous_offset_back()
{
    let n = 2;
    let mut view = PanelView::default();
    // First frame with the editor closed: nothing to bring back.
    view.sync_editor(false, 200 * n);
    assert_eq!(view.offset_px, 0);
    // Scrolled while closed; opening takes the offset to the maximum of the open block.
    view.offset_px = 40 * n;
    view.sync_editor(true, 200 * n);
    assert_eq!(view.offset_px, 200 * n, "al abrir va al máximo");
    // While it stays open a frame does not touch it, and the wheel moves it as usual.
    view.sync_editor(true, 200 * n);
    assert_eq!(view.offset_px, 200 * n);
    view.offset_px = 150 * n;
    view.sync_editor(true, 200 * n);
    assert_eq!(view.offset_px, 150 * n);
    // Closing brings the value before opening back.
    view.sync_editor(false, 200 * n);
    assert_eq!(view.offset_px, 40 * n, "al cerrar vuelve al valor anterior");
    // Opening again from 0 and closing returns to 0.
    view.offset_px = 0;
    view.sync_editor(true, 90 * n);
    assert_eq!(view.offset_px, 90 * n);
    view.sync_editor(false, 90 * n);
    assert_eq!(view.offset_px, 0);
    // When nothing overflows the maximum is 0.
    view.sync_editor(true, 0);
    assert_eq!(view.offset_px, 0);
}

#[test]
fn row_3_the_minimum_window_at_the_five_scales_and_with_an_invalid_ppp() {
    for ((ppp, height), width) in PPPS
        .into_iter()
        .zip([313.0f32, 250.4, 417.33, 313.0, 313.0])
        .zip([468.0f32, 374.4, 624.0, 468.0, 468.0])
    {
        assert!(
            (min_window_height_pt(ppp) - height).abs() < 0.01,
            "ppp {ppp}"
        );
        assert!((min_window_width_pt(ppp) - width).abs() < 0.01, "ppp {ppp}");
        assert!(min_window_height_pt(ppp) <= 432.0 + 0.01, "ppp {ppp}");
        assert!(min_window_width_pt(ppp) <= 683.0, "ppp {ppp}");
    }
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -2.0] {
        assert!((min_window_height_pt(invalid) - 313.0).abs() < 0.01);
        assert!((min_window_width_pt(invalid) - 468.0).abs() < 0.01);
    }
    // Each term of the width by itself, and the width is the widest of them.
    let [column_and_canvas, menu, dialog, status] = min_width_terms_u();
    assert_eq!(
        column_and_canvas,
        column_width_u(COLUMN_MIN_COLS) + MIN_CANVAS_WIDTH_U
    );
    assert_eq!(column_and_canvas, 277);
    assert_eq!(menu, 4 + 289);
    assert_eq!(dialog, 330 + 2 * 8);
    assert_eq!(status, 66 * 7 + 2 * 3);
    assert_eq!(
        min_window_width_u(),
        column_and_canvas.max(menu).max(dialog).max(status)
    );
    assert_eq!(min_window_width_u(), 468);
}

// ---------------------------------------------------------------------------------------------
// Row 4: window_layout and canvas origin
// ---------------------------------------------------------------------------------------------

fn overlap(a: &ScreenRect, b: &ScreenRect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

#[test]
fn row_4_window_layout_tiles_the_window_with_the_stacked_column_also_at_odd_widths() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        let k = i64::from(n);
        let min_h = i64::from(bar_height_px(n)) + MENU_H_U * k + min_panel_height_px(n);
        let default = ((960.0 * ppp).round() as i64, (720.0 * ppp).round() as i64);
        let mut windows = vec![default, (min_window_width_u() * k, min_h)];
        for extra in 0..=17 {
            windows.push((min_window_width_u() * k + extra, min_h + 2 * extra));
        }
        for window in windows {
            assert_eq!(
                window_layout(window, n),
                window_layout_cols(window, n, 16),
                "ppp {ppp} {window:?}"
            );
            for wanted in [8, 16, 40] {
                let layout = window_layout_cols(window, n, wanted);
                let cols = column_cols(window.0, n, wanted);
                let width = column_width_u(cols) * k;
                // Toolbox and palette: stacked, same `x` and same width `W(k)·n`, at the right edge.
                assert_eq!(
                    layout.toolbar.x, layout.panel.x,
                    "ppp {ppp} {window:?} k {wanted}"
                );
                assert_eq!(layout.toolbar.width, width);
                assert_eq!(layout.panel.width, width);
                assert_eq!(layout.panel.x + layout.panel.width, window.0);
                assert_eq!(layout.toolbar.y + layout.toolbar.height, layout.panel.y);
                assert_eq!(layout.toolbar.y, MENU_H_U * k);
                assert_eq!(layout.toolbar.height, head_u(5, cols) * k);
                // The canvas takes what is left, from the origin, and never less than 128 u.
                assert_eq!(layout.canvas.width, window.0 - width);
                assert!(layout.canvas.width >= MIN_CANVAS_WIDTH_U * k);
                assert_eq!(canvas_origin_px(n), (0, 16 * k));
                assert_eq!((layout.canvas.x, layout.canvas.y), canvas_origin_px(n));
                // Five rectangles without gaps or overlaps.
                let zones = [
                    layout.menu,
                    layout.toolbar,
                    layout.panel,
                    layout.canvas,
                    layout.status,
                ];
                let area: i64 = zones.iter().map(|z| z.width * z.height).sum();
                assert_eq!(area, window.0 * window.1, "ppp {ppp} {window:?} k {wanted}");
                for (i, a) in zones.iter().enumerate() {
                    for b in &zones[i + 1..] {
                        assert!(!overlap(a, b), "ppp {ppp} {window:?}: {a:?} and {b:?}");
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Property 1: integer geometry without overlaps of the column
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_1_the_column_is_integer_in_whole_pixels_and_its_blocks_do_not_overlap(
        step in 0u32..=60,
        extra_w in 0i64..=1500,
        extra_h in 0i64..=900,
        wanted in 1i64..=64,
        tools in 1usize..=24,
        mode in prop::sample::select(MODES.to_vec()),
        open in any::<bool>(),
        entries in 0usize..=256,
    ) {
        let ppp = ppp_of_the_step(step);
        let n = text_scale(ppp);
        let k = i64::from(n);
        let window = (
            min_window_width_u() * k + extra_w,
            i64::from(bar_height_px(n)) + MENU_H_U * k + min_panel_height_px(n) + extra_h,
        );
        let band = window.1 - (MENU_H_U + 20) * k;
        let cols = column_cols(window.0, n, wanted);
        prop_assert_eq!(cols, wanted.clamp(COLUMN_MIN_COLS, max_cols(window.0, n)));
        let width = column_width_u(cols) * k;
        // The canvas is at least 128·n wide and the column is `W(clamp(wanted))·n`.
        prop_assert!(window.0 - width >= MIN_CANVAS_WIDTH_U * k);
        let layout = window_layout_cols(window, n, wanted);
        prop_assert_eq!(layout.panel.width, width);
        prop_assert_eq!(layout.canvas.width, window.0 - width);

        // The blocks [toolbox · palette (band · grid · icons · editor)] add up to the column or,
        // scrolled, to the whole block.
        let head = head_u(tools, cols) * k;
        let block = if open {
            head + block_height_px(mode, n)
        } else {
            head + block_height_closed_px(mode, n)
        };
        let placed = column_layout(mode, band, n, 0, head, open);
        let icons = tail_height_u(mode) * k;
        let editor = if open { editor_height_u(mode) * k } else { 0 };
        for v in [placed.top_px, placed.tail_top_px, placed.offset_px, placed.max_offset_px] {
            prop_assert_eq!(v % k, 0);
        }
        if band >= block {
            prop_assert!(!placed.scrolling);
            let grid_top = head + GRID_Y_U * k;
            let grid_bottom = grid_top + placed.visible_rows as i64 * PITCH_U * k;
            prop_assert!(head <= grid_top);
            prop_assert!(grid_bottom <= placed.tail_top_px);
            prop_assert_eq!(placed.top_px, placed.tail_top_px + icons);
            prop_assert!(placed.top_px + editor <= band);
            prop_assert!(band - (placed.top_px + editor) < k, "anclado abajo");
        } else {
            prop_assert!(placed.scrolling);
            prop_assert_eq!(placed.visible_rows, 1);
            prop_assert_eq!(placed.max_offset_px, (block - band).div_euclid(k) * k);
        }

        // The grid of `cols` columns: every visible cell is inside the column, left of the track.
        let rows = total_rows_cols(entries, cols as usize);
        prop_assert_eq!(rows, entries.div_ceil(cols as usize));
        let grid = Grid::with_columns(n, cols as usize, entries, placed.visible_rows.max(1), 0);
        for index in 0..entries {
            if let Some(cell) = grid.cell_rect(index) {
                prop_assert_eq!(cell.x % k, 0);
                prop_assert_eq!(cell.y % k, 0);
                prop_assert!(GRID_X_U * k + cell.x + cell.width <= track_x_u(cols) * k);
                prop_assert_eq!(grid.index_at(cell.x, cell.y), Some(index));
            }
        }
        prop_assert!(track_x_u(cols) < right_line_x_u(cols));
        prop_assert_eq!(right_line_x_u(cols) + 1, column_width_u(cols));

        // The buttons of the toolbox hit the button of their rectangle.
        for i in 0..tools + 4 {
            let (x, y, w, h) = toolbox_button_rect_u_at(i, tools, cols);
            prop_assert_eq!(toolbox_hit_at(x * k, y * k, n, tools, cols), Some(i));
            prop_assert_eq!(toolbox_hit_at((x + w) * k - 1, (y + h) * k - 1, n, tools, cols), Some(i));
        }

        // The minimums follow the formulas and meet the ceilings.
        let height_2 = ((20 + MENU_H_U + 277) * k) as f32 / ppp;
        prop_assert!((min_window_height_pt(ppp) - height_2).abs() < 0.01);
        prop_assert!(min_window_height_pt(ppp) <= 432.0 + 0.01);
        let width_2 = (min_window_width_u() * k) as f32 / ppp;
        prop_assert!((min_window_width_pt(ppp) - width_2).abs() < 0.01);
        prop_assert!(min_window_width_pt(ppp) <= 683.0);
    }
}
