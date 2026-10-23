//! Unit C1 (specs/ui-composition.md), pure part: rows 1 to 4 and 16 (the border of the edited role, compact
//! grid, vertical split open and closed, minimums and criteria, whole window) and property 1
//! (integer geometry without overlaps). No egui.

use proptest::prelude::*;
use px_core::Mode;
use px_ui::chrome::{
    COLUMN_DEFAULT_COLS, COLUMN_MIN_COLS, MENU_H_U, column_cols, head_u, window_layout,
};
use px_ui::edit_layout::{
    PaletteButton, block_height_closed_px, block_height_px, button_hit, button_rect_u,
    column_layout, editor_height_u, min_panel_height_px, min_window_height_pt, min_window_width_pt,
    panel_layout, panel_layout_closed, tail_height_u,
};
use px_ui::layers_panel::PANEL_H_U as LAYERS_PANEL_H_U;
use px_ui::palette_grid::{
    CELL_U, DROP_MARK_W_U, GAP_U, GRID_Y_U, Grid, PITCH_U, RING_ACTIVE_U, ring_rects,
    visible_rows_reserved,
};
use px_ui::status::bar_height_px;
use px_ui::text::text_scale;
use px_ui::theme::{ACCENT, SURFACE_1, contrast_ratio};
use px_ui::view::ScreenRect;

const MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];
const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

fn n_of(ppp: f32) -> i64 {
    i64::from(text_scale(ppp))
}

fn overlap(a: &ScreenRect, b: &ScreenRect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

// ---------------------------------------------------------------------------------------------
// Row 1: the border of the edited role (the band and the indicator are gone since C4)
// ---------------------------------------------------------------------------------------------

#[test]
fn row_1_the_accent_border_of_2_u_has_3_to_1_contrast_with_the_panel_background() {
    let ratio = contrast_ratio(ACCENT, SURFACE_1);
    assert!(ratio >= 3.0, "ACCENT on SURFACE_1: {ratio}");
}

// ---------------------------------------------------------------------------------------------
// Row 2: compact grid
// ---------------------------------------------------------------------------------------------

#[test]
fn row_2_every_ring_and_the_drop_bar_fit_in_the_2_u_gap_without_touching_a_neighbor() {
    assert_eq!((CELL_U, GAP_U, PITCH_U), (12, 2, 14));
    const { assert!(RING_ACTIVE_U <= GAP_U && DROP_MARK_W_U <= GAP_U) };
    for n in [1u32, 2, 3] {
        let k = i64::from(n);
        let grid = Grid::new(n, 256, 16, 0);
        let own = grid.cell_rect(17).expect("swatch");
        let neighbors: Vec<ScreenRect> = [16usize, 18, 1, 33]
            .iter()
            .map(|i| grid.cell_rect(*i).expect("swatch"))
            .collect();
        for thickness in [1, RING_ACTIVE_U] {
            for ring in ring_rects(own, thickness * k) {
                for other in &neighbors {
                    assert!(!overlap(&ring, other), "n={n}, thickness {thickness}");
                }
            }
        }
        for after in [false, true] {
            let bar = px_ui::palette_grid::drop_mark_rect(own, after, n);
            for other in &neighbors {
                assert!(!overlap(&bar, other), "n={n}, after {after}");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Row 3: vertical split, open and closed
// ---------------------------------------------------------------------------------------------

#[test]
fn row_3_closed_the_icon_row_is_anchored_to_the_bottom_and_the_grid_takes_the_rest() {
    for n in [1u32, 2, 3] {
        let k = i64::from(n);
        for mode in MODES {
            let icons = tail_height_u(mode) * k;
            for height in [300 * k, 450 * k, 700 * k, 2000 * k] {
                let layout = panel_layout_closed(mode, height, n, 0);
                assert!(!layout.scrolling);
                assert_eq!(layout.tail_top_px, height - icons, "{mode:?} n={n}");
                assert_eq!(layout.top_px, layout.tail_top_px + icons);
                let rows = (height - icons - GRID_Y_U * k) / (PITCH_U * k);
                assert_eq!(
                    layout.visible_rows as i64, rows,
                    "{mode:?} n={n} h={height}"
                );
            }
        }
    }
}

#[test]
fn row_3_open_the_editor_is_anchored_below_the_icon_row_and_both_take_space_from_the_grid() {
    for n in [1u32, 2, 3] {
        let k = i64::from(n);
        for mode in MODES {
            let (icons, editor) = (tail_height_u(mode) * k, editor_height_u(mode) * k);
            for height in [400 * k, 700 * k, 2000 * k] {
                let open = panel_layout(mode, height, n, 0);
                let closed = panel_layout_closed(mode, height, n, 0);
                assert_eq!(open.top_px, height - editor, "{mode:?} n={n}");
                assert_eq!(open.tail_top_px, open.top_px - icons);
                // Opening costs the grid exactly the editor's height, in whole rows.
                assert!(open.visible_rows <= closed.visible_rows);
                assert_eq!(
                    open.visible_rows,
                    visible_rows_reserved(height, icons + editor, n)
                );
            }
        }
    }
}

#[test]
fn row_3_the_icon_row_hangs_from_a_short_grid_and_is_anchored_below_a_tall_one() {
    // One formula: the smaller of the bottom of the grid and the bottom of the space minus the row.
    for n in [1u32, 2, 3] {
        let k = i64::from(n);
        let icons = tail_height_u(Mode::Indexed) * k;
        for (entries, rows) in [
            (0usize, 0usize),
            (2, 1),
            (16, 1),
            (32, 2),
            (128, 8),
            (256, 16),
        ] {
            for height in [308 * k, 330 * k, 400 * k, 700 * k, 2000 * k] {
                for open in [false, true] {
                    let layout = if open {
                        panel_layout(Mode::Indexed, height, n, 0)
                    } else {
                        panel_layout_closed(Mode::Indexed, height, n, 0)
                    };
                    let space_bottom = if open {
                        height - editor_height_u(Mode::Indexed) * k
                    } else {
                        height
                    };
                    let grid_bottom = (GRID_Y_U + rows as i64 * PITCH_U) * k;
                    let expected = grid_bottom.min(space_bottom - icons);
                    assert_eq!(
                        layout.tail_top_for(rows, n),
                        expected,
                        "entries {entries} n={n} h={height} open={open}"
                    );
                }
            }
        }
    }
}

#[test]
fn row_3_the_icon_row_examples_in_u() {
    let k = 2i64;
    let n = k as u32;
    // 16 entries, one row: right under the grid (4 + 14 = 18 u) closed or open.
    assert_eq!(
        panel_layout_closed(Mode::Indexed, 700 * k, n, 0).tail_top_for(1, n),
        18 * k
    );
    assert_eq!(
        panel_layout(Mode::Indexed, 700 * k, n, 0).tail_top_for(1, n),
        18 * k
    );
    // 256 entries closed in a 250 u panel: the grid is taller than the space, so the tail is
    // anchored to the bottom (250 − 88 = 162 u); in a 316 u panel the grid (4 + 224 = 228 u) is
    // exactly the space above the tail.
    assert_eq!(
        panel_layout_closed(Mode::Indexed, 250 * k, n, 0).tail_top_for(16, n),
        162 * k
    );
    assert_eq!(
        panel_layout_closed(Mode::Indexed, 316 * k, n, 0).tail_top_for(16, n),
        228 * k
    );
    // Open, 256 entries in a 400 u panel: the editor (202 u) is anchored below and the tail sits
    // right above it (400 − 202 − 88 = 110 u).
    assert_eq!(
        panel_layout(Mode::Indexed, 400 * k, n, 0).tail_top_for(16, n),
        110 * k
    );
    // Panel scrolling as a whole: the row is where the block puts it.
    let scrolling = panel_layout(Mode::Indexed, 200 * k, n, 0);
    assert!(scrolling.scrolling);
    assert_eq!(scrolling.tail_top_for(16, n), scrolling.tail_top_px);
}

#[test]
fn row_3_the_indexed_block_measures_308_u_open_and_106_u_closed_and_scrolls_below_it() {
    for n in [1u32, 2, 3] {
        let k = i64::from(n);
        assert_eq!(block_height_px(Mode::Indexed, n), 308 * k);
        assert_eq!(block_height_closed_px(Mode::Indexed, n), 106 * k);
        assert!(!panel_layout(Mode::Indexed, 308 * k, n, 0).scrolling);
        assert!(panel_layout(Mode::Indexed, 308 * k - 1, n, 0).scrolling);
        assert!(!panel_layout_closed(Mode::Indexed, 106 * k, n, 0).scrolling);
        assert!(panel_layout_closed(Mode::Indexed, 106 * k - 1, n, 0).scrolling);
        // At the open block the grid shows one row; closed at 277 u, thirteen.
        assert_eq!(panel_layout(Mode::Indexed, 308 * k, n, 0).visible_rows, 1);
        assert_eq!(
            panel_layout_closed(Mode::Indexed, 277 * k, n, 0).visible_rows,
            13
        );
        // The 256 swatches (16 rows) fit entirely in a 316 u panel.
        assert_eq!(
            panel_layout_closed(Mode::Indexed, 316 * k, n, 0).visible_rows,
            16
        );
    }
}

#[test]
fn row_3_heights_far_out_of_range_do_not_overflow() {
    for mode in MODES {
        for height in [i64::MIN, -1, 0, 1, i64::MAX / 4] {
            let _ = panel_layout(mode, height, 3, 7);
            let _ = panel_layout_closed(mode, height, 3, 7);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Row 4: minimums and criteria
// ---------------------------------------------------------------------------------------------

#[test]
fn row_4_the_minimums_are_313_u_of_height_and_468_u_of_width_and_meet_the_criteria() {
    let heights = [313.0f32, 250.4, 417.33, 313.0, 313.0];
    let widths = [468.0f32, 374.4, 624.0, 468.0, 468.0];
    for ((ppp, height), width) in PPPS.into_iter().zip(heights).zip(widths) {
        assert!(
            (min_window_height_pt(ppp) - height).abs() < 0.01,
            "ppp {ppp}"
        );
        assert!((min_window_width_pt(ppp) - width).abs() < 0.01, "ppp {ppp}");
        assert!(
            min_window_height_pt(ppp) <= 432.0,
            "60 % of 720 at ppp {ppp}"
        );
        assert!(min_window_width_pt(ppp) <= 683.0, "ppp {ppp}");
    }
    for invalid in [f32::NAN, f32::INFINITY, 0.0, -2.0] {
        assert!((min_window_height_pt(invalid) - 313.0).abs() < 0.01);
    }
}

#[test]
fn row_4_the_minimum_panel_is_the_layers_panel_and_the_toolbox_and_the_closed_block_fit() {
    for n in [1u32, 2, 3, 8] {
        let k = i64::from(n);
        let minimum = min_panel_height_px(n);
        assert_eq!(minimum, 277 * k);
        // The closed block of every mode, the layers panel (277 u) and the toolbox (20 tools at the
        // minimum column: `head` 159 u, plus the closed palette block of 106 u) fit; the open block
        // of indexed and RGBA images (308 and 282 u) does not: it scrolls as a whole (C4).
        for mode in MODES {
            assert!(block_height_closed_px(mode, n) <= minimum, "{mode:?}");
        }
        assert!(block_height_px(Mode::Indexed, n) > minimum);
        assert!(
            LAYERS_PANEL_H_U * k <= minimum,
            "the layers panel does not fit"
        );
        assert!(
            head_u(20, COLUMN_MIN_COLS) * k + block_height_closed_px(Mode::Indexed, n) <= minimum,
            "the toolbox does not fit"
        );
        // Closed is never taller than open, and the minimum does not depend on the editor.
        for mode in MODES {
            assert!(block_height_closed_px(mode, n) < block_height_px(mode, n));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Row 16 (pure part): the whole window
// ---------------------------------------------------------------------------------------------

#[test]
fn row_16_at_the_minimum_window_the_five_zones_tile_it_and_the_layers_panel_and_toolbox_fit() {
    for ppp in PPPS {
        let n = n_of(ppp);
        let width = min_window_width_pt(ppp) * ppp;
        let height = min_window_height_pt(ppp) * ppp;
        let window = (width.round() as i64, height.round() as i64);
        let layout = window_layout(window, n as u32);
        let cols = column_cols(window.0, n as u32, COLUMN_DEFAULT_COLS);
        assert_eq!(
            layout.canvas.height,
            window.1 - (MENU_H_U * n) - i64::from(bar_height_px(n as u32)),
            "ppp {ppp}"
        );
        assert!(
            layout.canvas.height >= LAYERS_PANEL_H_U * n,
            "ppp {ppp}: layers panel"
        );
        assert_eq!(
            layout.toolbar.height,
            head_u(5, cols) * n,
            "ppp {ppp}: toolbox"
        );
        assert_eq!(
            layout.toolbar.height + layout.panel.height,
            277 * n,
            "ppp {ppp}: column at the minimum"
        );
        // Closed, the block fits in the panel under the toolbox without scrolling; open it does not
        // (the toolbox and the open block are taller than the minimum: the block scrolls as a whole).
        assert!(
            !column_layout(
                Mode::Indexed,
                277 * n,
                n as u32,
                0,
                layout.toolbar.height,
                false
            )
            .scrolling
        );
        assert!(
            column_layout(
                Mode::Indexed,
                277 * n,
                n as u32,
                0,
                layout.toolbar.height,
                true
            )
            .scrolling
        );
        // With no toolbox the closed block fits; the open one (308 u) is taller than the 277 u.
        assert!(panel_layout(Mode::Indexed, 277 * n, n as u32, 0).scrolling);
        assert!(!panel_layout_closed(Mode::Indexed, 277 * n, n as u32, 0).scrolling);
        // Zones: tiling without gaps or overlaps.
        let zones = [
            layout.menu,
            layout.canvas,
            layout.panel,
            layout.toolbar,
            layout.status,
        ];
        for (i, a) in zones.iter().enumerate() {
            for b in &zones[i + 1..] {
                assert!(!overlap(a, b), "ppp {ppp}: {a:?} and {b:?}");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Property 1: integer geometry and no overlaps
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_1_the_split_is_integer_in_whole_pixels_and_the_blocks_do_not_overlap(
        step in 0u32..=60,
        mode in prop::sample::select(MODES.to_vec()),
        open in any::<bool>(),
        extra in 0i64..=900,
    ) {
        let ppp = (100 + 5 * step) as f32 / 100.0;
        let n = text_scale(ppp);
        let k = i64::from(n);
        // Any height at or above the block (the minimum panel closed: with the editor open the
        // block is taller than it, C4), in pixels.
        let base = if open { block_height_px(mode, n) } else { min_panel_height_px(n) };
        let height = base + extra;
        let layout = if open {
            panel_layout(mode, height, n, 0)
        } else {
            panel_layout_closed(mode, height, n, 0)
        };
        prop_assert!(!layout.scrolling);
        for v in [layout.top_px, layout.tail_top_px] {
            prop_assert_eq!(v % k, 0);
        }
        // [band · grid · icons · editor] in order, no overlap, and they add up to the panel.
        let icons = tail_height_u(mode) * k;
        let editor = if open { editor_height_u(mode) * k } else { 0 };
        let grid_top = GRID_Y_U * k;
        let grid_bottom = grid_top + layout.visible_rows as i64 * PITCH_U * k;
        prop_assert!(grid_bottom <= layout.tail_top_px);
        prop_assert_eq!(layout.top_px, layout.tail_top_px + icons);
        prop_assert!(layout.top_px + editor <= height);
        prop_assert!(height - (layout.top_px + editor) < k, "pinned to the bottom");
        // What is left over between the grid and the icon row is less than one grid row.
        prop_assert!(layout.tail_top_px - grid_bottom < PITCH_U * k, "less than a row left over");
        // The minimums follow the formula and meet the ceilings.
        let formula = ((20 + MENU_H_U + 277) * k) as f32 / ppp;
        prop_assert!((min_window_height_pt(ppp) - formula).abs() < 0.01);
        prop_assert!(min_window_height_pt(ppp) <= 432.0 + 0.01);
        prop_assert!(min_window_width_pt(ppp) <= 683.0);
    }

    #[test]
    fn property_5_the_icon_row_never_overlaps_the_visible_grid_or_the_editor_or_leaves_the_panel(
        ppp in prop::sample::select(PPPS.to_vec()),
        entries in prop::sample::select(vec![2usize, 16, 32, 128, 256]),
        open in any::<bool>(),
        extra in 0i64..=900,
    ) {
        let n = text_scale(ppp);
        let k = i64::from(n);
        let base = if open {
            block_height_px(Mode::Indexed, n)
        } else {
            min_panel_height_px(n)
        };
        let height = base + extra;
        let layout = if open {
            panel_layout(Mode::Indexed, height, n, 0)
        } else {
            panel_layout_closed(Mode::Indexed, height, n, 0)
        };
        prop_assert!(!layout.scrolling);
        let rows = entries.div_ceil(16);
        let top = layout.tail_top_for(rows, n);
        let icons = tail_height_u(Mode::Indexed) * k;
        let editor_top = if open { layout.top_px } else { height };
        let shown = rows.min(layout.visible_rows) as i64;
        prop_assert_eq!(top % k, 0);
        // Below the visible grid, above the editor, inside the panel.
        prop_assert!(top >= (GRID_Y_U + shown * PITCH_U) * k, "overlaps the grid");
        prop_assert!(top + icons <= editor_top, "overlaps the editor");
        prop_assert!(top >= 0 && top + icons <= height, "outside the panel");
        // Hanging from a short grid, anchored when the grid is taller than the space.
        if rows <= layout.visible_rows {
            prop_assert_eq!(top, (GRID_Y_U + rows as i64 * PITCH_U) * k);
        } else {
            prop_assert_eq!(top, layout.tail_top_px);
        }
    }

    #[test]
    fn property_3_every_point_of_the_icon_row_hits_its_own_button_or_nothing(
        n in 1u32..=8,
        removing in any::<bool>(),
        x in -30i64..=260,
        y in -10i64..=100,
        fx in 0i64..8,
        fy in 0i64..8,
    ) {
        let k = i64::from(n);
        // Fractions of `u` too, so edges and gaps are covered.
        let (px, py) = (x * k + fx % k, y * k + fy % k);
        let hit = button_hit(Mode::Indexed, removing, px, py, n);
        let buttons: &[PaletteButton] = if removing {
            &[PaletteButton::Confirm, PaletteButton::Cancel]
        } else {
            &[
                PaletteButton::Remove,
                PaletteButton::RotateLeft,
                PaletteButton::RotateRight,
                PaletteButton::Transparent,
            ]
        };
        let oracle = buttons.iter().copied().find(|b| {
            let (bx, by, bw, bh) = button_rect_u(Mode::Indexed, *b).expect("indexed has buttons");
            px >= bx * k && px < (bx + bw) * k && py >= by * k && py < (by + bh) * k
        });
        prop_assert_eq!(hit, oracle);
        // Nothing hits outside the icon row (y from 62 to 86 u of the tail).
        if py < 62 * k || py >= (tail_height_u(Mode::Indexed) - 2) * k {
            prop_assert_eq!(hit, None);
        }
        // The modes without a row never hit.
        for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
            prop_assert_eq!(button_hit(mode, removing, px, py, n), None);
        }
    }
}
