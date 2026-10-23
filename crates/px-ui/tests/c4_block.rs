//! Unit C4 (specs/ui-color-block.md), pure part: rows 1 (geometry and minimums), 2 (cells and
//! click zones of the two bars), 3 (the text fits) and 12 (the pieces of the panel do not overlap),
//! the column of the open editor (condition 1 of the approval) and properties 2 (geometry from a
//! single source) and 3 (text, ink and border of the bars). No window is needed.
//!
//! The expected numbers come from the table «Geometría» of the spec, not from the code.

use egui::{Color32, Context, FontId};
use proptest::prelude::*;
use px_core::Mode;
use px_ui::chrome::{
    COLUMN_EDITOR_COLS, COLUMN_MIN_COLS, MENU_H_U, column_width_u, head_u, wide_cols,
};
use px_ui::color::Role;
use px_ui::color_bar::{bar_ink, bar_label};
use px_ui::color_block::{
    ALPHA_BAR_X_U, BAR_CELL_H_U, BarHit, HUE_BAR_X_U, Piece, SELECTOR_H_U, SELECTOR_W_U,
    SELECTOR_X_U, SELECTOR_Y_U, SQUARE_X_U, SQUARE_Y_U, SWAP_SIZE_U, VBAR_W_U, bar_cell_rect,
    bar_fill_rect, bar_hit, piece_rect, selector_rect, swap_rect,
};
use px_ui::color_edit::EditTarget;
use px_ui::edit_layout::{
    FIELD_H_U, FIELD_W_U, FIELD_X_U, FIELD_Y_U, ICON_ROW_Y_U, PaletteButton,
    block_height_closed_px, block_height_px, button_rect_u, column_layout, editor_height_u,
    min_panel_height_px, min_window_height_pt, min_window_width_pt, panel_layout,
    panel_layout_closed, tail_height_u,
};
use px_ui::palette_grid::{GRID_Y_U, PITCH_U, panel_width_u, text_width_u, total_rows_cols};
use px_ui::text::{INTERFACE_CHARSET, apply_scale, font_size_points, install_fonts, text_scale};
use px_ui::theme::{ACCENT, SURFACE_0, SURFACE_1, TEXT, contrast_ratio};
use px_ui::view::ScreenRect;

const MODES: [Mode; 4] = [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha, Mode::Gray];
const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

fn overlap(a: &ScreenRect, b: &ScreenRect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

fn rect_u(r: (i64, i64, i64, i64), n: i64) -> ScreenRect {
    ScreenRect {
        x: r.0 * n,
        y: r.1 * n,
        width: r.2 * n,
        height: r.3 * n,
    }
}

// ---------------------------------------------------------------------------------------------
// Row 1: geometry and minimums
// ---------------------------------------------------------------------------------------------

#[test]
#[ignore = "C4: pending the geometry of the color block"]
fn row_1_the_tail_the_editor_and_the_blocks_measure_what_the_table_says() {
    assert_eq!(GRID_Y_U, 4);
    assert_eq!((FIELD_Y_U, FIELD_W_U, FIELD_H_U), (42, 69, 18));
    assert_eq!(ICON_ROW_Y_U, 62);
    for (mode, tail, editor, closed, open) in [
        (Mode::Indexed, 88, 202, 106, 308),
        (Mode::Rgba, 62, 202, 80, 282),
        (Mode::GrayAlpha, 62, 40, 80, 120),
        (Mode::Gray, 62, 22, 80, 102),
    ] {
        assert_eq!(tail_height_u(mode), tail, "{mode:?}");
        assert_eq!(editor_height_u(mode), editor, "{mode:?}");
        for n in [1u32, 2, 3] {
            let k = i64::from(n);
            assert_eq!(
                block_height_closed_px(mode, n),
                closed * k,
                "{mode:?} n {n}"
            );
            assert_eq!(block_height_px(mode, n), open * k, "{mode:?} n {n}");
            assert_eq!(min_panel_height_px(n), 277 * k);
        }
    }
    // The capacity rule has 12 u to spare and the layers panel (277 u) sets the floor again.
    assert_eq!(head_u(20, COLUMN_MIN_COLS), 159);
    assert_eq!(277 - 106 - 159, 12);
}

#[test]
#[ignore = "C4: pending the geometry of the color block"]
fn row_1_the_minimum_window_and_the_rows_of_the_grid_seen_in_it() {
    for (ppp, height) in PPPS
        .into_iter()
        .zip([313.0f32, 250.4, 417.33, 313.0, 313.0])
    {
        let n = text_scale(ppp);
        assert!(
            (min_window_height_pt(ppp) - height).abs() < 0.01,
            "ppp {ppp}"
        );
        assert!(min_window_height_pt(ppp) <= 432.0, "ppp {ppp}");
        assert!((min_window_width_pt(ppp) * ppp - (468 * i64::from(n)) as f32).abs() < 0.01);
        assert!(min_window_width_pt(ppp) <= 683.0);
    }
    for invalid in [f32::NAN, 0.0, -1.0, f32::INFINITY] {
        assert!((min_window_height_pt(invalid) - 313.0).abs() < 0.01);
    }
    // Rows of the grid seen at the minimum, closed, with 5, 12 and 20 tools at 10 columns.
    for n in [1u32, 2, 3] {
        let k = i64::from(n);
        for (tools, indexed, other) in [(5, 7, 9), (12, 3, 5), (20, 1, 3)] {
            let head = head_u(tools, COLUMN_MIN_COLS) * k;
            for (mode, rows) in [(Mode::Indexed, indexed), (Mode::Rgba, other)] {
                let layout = column_layout(mode, min_panel_height_px(n), n, 0, head, false);
                assert!(!layout.scrolling, "{mode:?}, {tools} tools");
                assert_eq!(layout.visible_rows, rows, "{mode:?}, {tools} tools, n {n}");
            }
        }
    }
    // With the editor open the minimum is the same and the block scrolls as a whole.
    let open = column_layout(Mode::Indexed, min_panel_height_px(2), 2, 0, 0, true);
    assert!(open.scrolling);
}

// ---------------------------------------------------------------------------------------------
// Row 2: cells, fills and click zones of the bars
// ---------------------------------------------------------------------------------------------

#[test]
#[ignore = "C4: pending the geometry of the color block"]
fn row_2_the_cells_and_fills_are_whole_and_the_click_falls_in_the_cell_without_dead_gaps() {
    assert!(contrast_ratio(ACCENT, SURFACE_1) >= 3.0, "the 2 u border");
    for cols in [COLUMN_MIN_COLS, 16, 23] {
        let width = panel_width_u(cols);
        for n in [1u32, 2, 3, 8] {
            let k = i64::from(n);
            let fg = bar_cell_rect(Role::Foreground, cols, n);
            let bg = bar_cell_rect(Role::Background, cols, n);
            assert_eq!(fg, rect_u((2, 0, width - 30, BAR_CELL_H_U), k));
            assert_eq!(bg, rect_u((2, 20, width - 30, BAR_CELL_H_U), k));
            assert_eq!(
                bar_fill_rect(Role::Foreground, cols, n),
                rect_u((4, 2, width - 34, 16), k)
            );
            assert_eq!(
                bar_fill_rect(Role::Background, cols, n),
                rect_u((4, 22, width - 34, 16), k)
            );
            let swap = swap_rect(cols, n);
            assert_eq!(swap, rect_u((width - 26, 8, SWAP_SIZE_U, SWAP_SIZE_U), k));
            assert!(!overlap(&fg, &swap) && !overlap(&bg, &swap));
            // The two cells tile 40 u without a gap, and the click is the whole cell.
            let hit = |x, y| bar_hit(cols, x, y, n);
            for x in [fg.x, fg.x + fg.width - 1, fg.x + fg.width / 2] {
                assert_eq!(hit(x, 0), Some(BarHit::Bar(Role::Foreground)));
                assert_eq!(hit(x, 20 * k - 1), Some(BarHit::Bar(Role::Foreground)));
                assert_eq!(hit(x, 20 * k), Some(BarHit::Bar(Role::Background)));
                assert_eq!(hit(x, 40 * k - 1), Some(BarHit::Bar(Role::Background)));
                assert_eq!(hit(x, 40 * k), None, "under the bars");
                assert_eq!(hit(x, -1), None, "above the bars");
            }
            // Outside the cells: the margin on the left, the gap before the button and past it.
            assert_eq!(hit(fg.x - 1, 10 * k), None);
            assert_eq!(hit(fg.x + fg.width, 10 * k), None);
            assert_eq!(hit(swap.x - 1, 10 * k), None);
            assert_eq!(hit(swap.x + swap.width, 20 * k), None);
            // The button is a click zone of its own.
            for (x, y) in [
                (swap.x, swap.y),
                (swap.x + swap.width - 1, swap.y + swap.height - 1),
            ] {
                assert_eq!(hit(x, y), Some(BarHit::Swap));
            }
            assert_eq!(hit(swap.x, swap.y - 1), None);
            assert_eq!(hit(swap.x, swap.y + swap.height), None);
            // The border of 2 u around the fill stays inside its cell and the tail's 40 u.
            let fill = bar_fill_rect(Role::Background, cols, n);
            assert!(fill.x - 2 * k >= bg.x && fill.y + fill.height + 2 * k <= 40 * k);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Row 3: the longest text fits in the fill of the bar
// ---------------------------------------------------------------------------------------------

fn text_width_px(text: &str, ppp: f32) -> f32 {
    let ctx = Context::default();
    ctx.set_pixels_per_point(ppp);
    install_fonts(&ctx);
    let width = std::cell::Cell::new(0.0);
    let out = ctx.run_ui(egui::RawInput::default(), |ui| {
        apply_scale(ui.ctx());
        let font = FontId::proportional(font_size_points(ppp));
        let size = ui.ctx().fonts_mut(|f| {
            f.layout_no_wrap(text.to_owned(), font, Color32::WHITE)
                .size()
        });
        width.set(size.x * ppp);
    });
    out.drop_without_applying_deltas();
    width.get()
}

#[test]
#[ignore = "C4: pending the geometry of the color block"]
fn row_3_the_longest_text_fits_in_the_fill_of_the_bar_at_the_minimum_column_and_the_five_scales() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        let fill = bar_fill_rect(Role::Foreground, COLUMN_MIN_COLS, n);
        for text in ["transparente", "Idx-255", "Col-9"] {
            let used = (4 * i64::from(n)) as f32 + text_width_px(text, ppp);
            assert!(
                used <= fill.width as f32,
                "ppp {ppp}: {text:?} takes {used} px of {}",
                fill.width
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Condition 1 of the approval: the open editor fits the column
// ---------------------------------------------------------------------------------------------

#[test]
#[ignore = "C4: pending the geometry of the color block"]
fn the_square_the_two_bars_and_the_selector_fit_the_column_of_the_open_editor() {
    assert_eq!(wide_cols(COLUMN_MIN_COLS, true), COLUMN_EDITOR_COLS);
    assert_eq!(column_width_u(COLUMN_EDITOR_COLS), 233);
    for n in [1u32, 2, 3] {
        let k = i64::from(n);
        let square = piece_rect(Piece::Square, n);
        let hue = piece_rect(Piece::HueBar, n);
        let alpha = piece_rect(Piece::AlphaBar, n);
        assert_eq!(square, rect_u((SQUARE_X_U, SQUARE_Y_U, 112, 112), k));
        assert_eq!(hue, rect_u((HUE_BAR_X_U, SQUARE_Y_U, VBAR_W_U, 112), k));
        assert_eq!(alpha, rect_u((ALPHA_BAR_X_U, SQUARE_Y_U, VBAR_W_U, 112), k));
        assert_eq!((HUE_BAR_X_U, ALPHA_BAR_X_U), (120, 136));
        // From x = 4 to 148 with gaps of 4 u, inside the 225 u the editor may use.
        assert_eq!(square.x, 4 * k);
        assert_eq!(alpha.x + alpha.width, 148 * k);
        assert!(alpha.x + alpha.width <= (4 + 225) * k);
        assert!(!overlap(&square, &hue) && !overlap(&hue, &alpha));
        assert_eq!(hue.x - (square.x + square.width), 4 * k);
        assert_eq!(alpha.x - (hue.x + hue.width), 4 * k);
        // The selector: three options of 39 u in a 14 u row under the pieces.
        assert_eq!((SELECTOR_Y_U, SELECTOR_H_U, SELECTOR_W_U), (120, 14, 39));
        assert_eq!(SELECTOR_X_U, [4, 51, 98]);
        for (i, x_u) in SELECTOR_X_U.into_iter().enumerate() {
            let option = selector_rect(i, n);
            assert_eq!(
                option,
                rect_u((x_u, SELECTOR_Y_U, SELECTOR_W_U, SELECTOR_H_U), k)
            );
            assert!(option.y >= square.y + square.height);
            assert!(option.x + option.width <= (4 + 225) * k);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Row 12 (pure part): the pieces of the panel do not overlap
// ---------------------------------------------------------------------------------------------

#[test]
#[ignore = "C4: pending the geometry of the color block"]
fn row_12_the_bars_the_field_and_the_icons_do_not_overlap_in_any_column_width() {
    for cols in [COLUMN_MIN_COLS, 16, 23] {
        let width = panel_width_u(cols);
        for mode in MODES {
            let mut pieces = vec![
                ("fg", rect_u((2, 0, width - 30, 20), 1)),
                ("bg", rect_u((2, 20, width - 30, 20), 1)),
                ("swap", rect_u((width - 26, 8, 24, 24), 1)),
                (
                    "field",
                    rect_u((FIELD_X_U, FIELD_Y_U, FIELD_W_U, FIELD_H_U), 1),
                ),
            ];
            for button in [
                PaletteButton::Remove,
                PaletteButton::RotateLeft,
                PaletteButton::RotateRight,
                PaletteButton::Transparent,
            ] {
                if let Some(r) = button_rect_u(mode, button) {
                    pieces.push(("icon", rect_u(r, 1)));
                }
            }
            for (i, (a_name, a)) in pieces.iter().enumerate() {
                assert!(a.x >= 0 && a.x + a.width + 2 <= width, "{a_name} in {cols}");
                assert!(a.y + a.height <= tail_height_u(mode), "{a_name}");
                for (b_name, b) in &pieces[i + 1..] {
                    assert!(!overlap(a, b), "{a_name} and {b_name} at {cols} columns");
                }
            }
            // The cells measured by the module are the ones of the table.
            assert_eq!(
                bar_cell_rect(Role::Foreground, cols, 1),
                pieces[0].1,
                "{mode:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Property 2: the geometry has a single source
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    #[ignore = "C4: pending the geometry of the color block"]
    fn property_2_every_measure_is_whole_fits_the_column_and_does_not_overlap(
        step in 0u32..=60,
        mode in prop::sample::select(MODES.to_vec()),
        extra_cols in 0i64..=13,
        entries in 1usize..=256,
        open in any::<bool>(),
        extra_h in 0i64..=900,
    ) {
        let ppp = (100 + 5 * step) as f32 / 100.0;
        let n = text_scale(ppp);
        let k = i64::from(n);
        let cols = COLUMN_MIN_COLS + extra_cols;
        let width = column_width_u(cols);
        let height = min_panel_height_px(n) + extra_h;
        let layout = if open {
            panel_layout(mode, height, n, 0)
        } else {
            panel_layout_closed(mode, height, n, 0)
        };
        prop_assert!(!layout.scrolling);
        let cells = if mode == Mode::Indexed { entries + 1 } else { 0 };
        let rows = total_rows_cols(cells, cols as usize);
        let tail = layout.tail_top_for(rows, n);
        let space_bottom = if open { layout.top_px } else { height };
        let by_formula =
            ((GRID_Y_U + rows as i64 * PITCH_U) * k).min(space_bottom - tail_height_u(mode) * k);
        prop_assert_eq!(tail, by_formula.min(layout.tail_top_px));
        prop_assert_eq!(tail % k, 0);
        prop_assert!(tail + tail_height_u(mode) * k <= space_bottom);
        // Every piece of the tail, whole, a multiple of `n`, inside the column and disjoint.
        let mut pieces = vec![
            bar_cell_rect(Role::Foreground, cols, n),
            bar_cell_rect(Role::Background, cols, n),
            swap_rect(cols, n),
            rect_u((FIELD_X_U, FIELD_Y_U, FIELD_W_U, FIELD_H_U), k),
        ];
        for button in [
            PaletteButton::Remove,
            PaletteButton::RotateLeft,
            PaletteButton::RotateRight,
            PaletteButton::Transparent,
        ] {
            if let Some(r) = button_rect_u(mode, button) {
                pieces.push(rect_u(r, k));
            }
        }
        for (i, a) in pieces.iter().enumerate() {
            for v in [a.x, a.y, a.width, a.height] {
                prop_assert_eq!(v % k, 0);
            }
            prop_assert!(a.x >= 0 && a.x + a.width + 2 * k <= width * k);
            prop_assert!(a.y + a.height <= tail_height_u(mode) * k);
            for b in &pieces[i + 1..] {
                prop_assert!(!overlap(a, b), "{a:?} and {b:?}");
            }
        }
        // The editor pieces of the chromatic modes, inside its height and its width.
        if matches!(mode, Mode::Indexed | Mode::Rgba) {
            let mut editor = [
                piece_rect(Piece::Square, n),
                piece_rect(Piece::HueBar, n),
                piece_rect(Piece::AlphaBar, n),
                selector_rect(0, n),
                selector_rect(1, n),
                selector_rect(2, n),
            ];
            editor.sort_by_key(|r| (r.y, r.x));
            for (i, a) in editor.iter().enumerate() {
                for v in [a.x, a.y, a.width, a.height] {
                    prop_assert_eq!(v % k, 0);
                }
                prop_assert!(a.x + a.width <= text_width_u(16) * k + 4 * k);
                prop_assert!(a.y + a.height <= (editor_height_u(mode) - 4) * k);
                for b in &editor[i + 1..] {
                    prop_assert!(!overlap(a, b), "{a:?} and {b:?}");
                }
            }
        }
        let h = ((20 + MENU_H_U + 277) * k) as f32 / ppp;
        prop_assert!((min_window_height_pt(ppp) - h).abs() < 0.01);
        prop_assert!(min_window_height_pt(ppp) <= 432.0 + 0.01);
        let w = (468 * k) as f32 / ppp;
        prop_assert!((min_window_width_pt(ppp) - w).abs() < 0.01);
        prop_assert!(min_window_width_pt(ppp) <= 683.0);
    }
}

// ---------------------------------------------------------------------------------------------
// Property 3: text, ink and border of the bars
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    #[ignore = "C4: pending the new texts of the bars"]
    fn property_3_the_text_of_the_bar_is_the_one_of_the_decision_and_has_contrast(
        r in any::<u8>(), g in any::<u8>(), b in any::<u8>(), a in any::<u8>(),
        index in any::<u8>(),
        entry in any::<bool>(),
        transparent in any::<bool>(),
    ) {
        let target = if entry { EditTarget::Entry(index) } else { EditTarget::Slot(index % 9 + 1) };
        let label = bar_label(target, transparent);
        if transparent {
            prop_assert_eq!(&label, "transparente");
        } else {
            let expected = match target {
                EditTarget::Entry(i) => format!("Idx-{i}"),
                EditTarget::Slot(s) => format!("Col-{s}"),
            };
            prop_assert_eq!(&label, &expected);
        }
        prop_assert!(label.chars().all(|c| INTERFACE_CHARSET.contains(c) || c.is_ascii()));
        // 4 u of margin and at most 7,5 u per character inside the narrowest fill (115 u).
        prop_assert!(4.0 + 7.5 * label.chars().count() as f32 <= 115.0);
        let color = px_core::Rgba::new(r, g, b, a);
        let ink = bar_ink(color);
        prop_assert_eq!(bar_ink(color), ink, "deterministic");
        if a == 255 {
            prop_assert!(!ink.boxed);
            prop_assert!(contrast_ratio(ink.text, ink.behind) >= 4.5);
        } else {
            prop_assert!(ink.boxed && ink.behind == SURFACE_0 && ink.text == TEXT);
            prop_assert!(contrast_ratio(ink.text, ink.behind) >= 4.5);
        }
    }
}
