//! Unit C3 (specs/ui-palette-handle.md), the color bar and the hexadecimal field under the grid:
//! rows 10 and 11 (text, painting and behavior of the bar and the field; the tail under the grid
//! and the minimums) and properties 3 (contrast of the bar text) and 4 (geometry of the tail, the
//! editor and the minimums). The egui bench has no window; nothing is captured from the desktop.

use egui::{Color32, Key};
use proptest::prelude::*;
use px_core::{Image, Mode, Palette, Rgba};
use px_ui::chrome::{COLUMN_MIN_COLS, MENU_H_U, column_width_u, head_u};
use px_ui::color::{ColorChoice, Role};
use px_ui::color_bar::{bar_ink, bar_label};
use px_ui::color_edit::{Commit, EditTarget, hex_text};
use px_ui::edit_layout::{
    BAR_H_U, BAR_X_U, BAR_Y_U, FIELD_H_U, FIELD_W_U, FIELD_X_U, FIELD_Y_U, PaletteButton,
    button_rect_u, editor_height_u, min_panel_height_px, min_window_height_pt, min_window_width_pt,
    panel_layout, panel_layout_closed, tail_height_u,
};
use px_ui::help::HelpTarget;
use px_ui::palette_grid::{CHECK_A, CHECK_B, GRID_Y_U, PITCH_U, text_width_u, total_rows_cols};
use px_ui::text::{INTERFACE_CHARSET, text_scale};
use px_ui::theme::{SURFACE_0, TEXT, WARNING, contrast_ratio};

#[path = "common/c3_bench.rs"]
mod bench;
#[path = "common/icons.rs"]
mod icons;
#[path = "common/zones.rs"]
mod zones;

use bench::{Bench, PPPS, SCREEN_HIGH, indexed};

const BLACK: Color32 = Color32::from_rgb(0, 0, 0);
const WHITE: Color32 = Color32::from_rgb(255, 255, 255);
const REASON_DIGIT: &str = "solo dígitos 0-9 y a-f";

fn color32(color: Rgba) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

fn image_of(mode: Mode) -> Image {
    Image::new(8, 4, mode).expect("image")
}

/// The bar, in points: from the top of the tail, 4 u from the left, 16 u high, the width of the text
/// area of the column.
fn bar_rect(bench: &Bench) -> egui::Rect {
    let (n, ppp) = (bench.n(), bench.ppp());
    let top = bench.tail_top() + BAR_Y_U * n;
    let width = text_width_u(16) * n;
    egui::Rect::from_min_size(
        egui::pos2((BAR_X_U * n) as f32 / ppp, top as f32 / ppp),
        egui::vec2(width as f32 / ppp, (BAR_H_U * n) as f32 / ppp),
    )
}

fn field_center(bench: &Bench) -> (i64, i64) {
    let n = bench.n();
    (
        (FIELD_X_U + FIELD_W_U / 2) * n,
        bench.tail_top() + (FIELD_Y_U + FIELD_H_U / 2) * n,
    )
}

fn indexed_with_alpha(alpha: u8) -> Image {
    let colors = [
        Rgba::opaque(200, 40, 40),
        Rgba::opaque(40, 200, 40),
        Rgba::new(40, 40, 200, alpha),
        Rgba::opaque(250, 250, 250),
    ];
    let palette = Palette::from_colors(colors).expect("palette");
    Image::new_indexed(4, 4, palette).expect("image")
}

// ---------------------------------------------------------------------------------------------
// Row 10: the text of the bar
// ---------------------------------------------------------------------------------------------

#[test]
fn row_10_the_text_of_the_bar_is_idx_n_col_n_and_idx_n_transp() {
    assert_eq!(bar_label(EditTarget::Entry(0), false), "Idx-0");
    assert_eq!(bar_label(EditTarget::Entry(10), false), "Idx-10");
    assert_eq!(bar_label(EditTarget::Entry(255), false), "Idx-255");
    assert_eq!(bar_label(EditTarget::Entry(3), true), "Idx-3 transp.");
    assert_eq!(bar_label(EditTarget::Slot(3), false), "Col-3");
    for ppp in PPPS {
        // Every mode that has a target paints its text, in the charset and in one line.
        for (image, choice, expected) in [
            (indexed(12), ColorChoice::Entry(10), "Idx-10"),
            (image_of(Mode::Rgba), ColorChoice::Slot(3), "Col-3"),
            (image_of(Mode::Gray), ColorChoice::Slot(3), "Col-3"),
            (image_of(Mode::GrayAlpha), ColorChoice::Slot(3), "Col-3"),
        ] {
            let mut bench = Bench::new(ppp, SCREEN_HIGH, image, choice);
            let f = bench.rest();
            assert!(f.paints(expected), "ppp {ppp}: {expected}");
            for painted in &f.texts {
                assert!(
                    painted
                        .text
                        .chars()
                        .all(|c| INTERFACE_CHARSET.contains(c) || c.is_ascii())
                );
            }
        }
    }
}

#[test]
fn row_10_the_bar_says_transp_for_the_transparent_entry() {
    let mut palette = Palette::from_colors([Rgba::opaque(1, 2, 3), Rgba::opaque(4, 5, 6)]).unwrap();
    palette.set_transparent_index(Some(1)).unwrap();
    let image = Image::new_indexed(2, 2, palette).unwrap();
    let mut bench = Bench::new(1.0, SCREEN_HIGH, image, ColorChoice::Entry(1));
    let f = bench.rest();
    assert!(f.paints("Idx-1 transp."));
    // The bar paints the stored color, not the resolved one (alpha 0).
    assert!(f.there_is_fill(bar_rect(&bench), color32(Rgba::opaque(4, 5, 6))));
}

#[test]
fn row_10_the_bar_is_painted_with_the_stored_color_of_what_the_editor_edits() {
    for ppp in PPPS {
        let mut bench = Bench::new(ppp, SCREEN_HIGH, indexed(8), ColorChoice::Entry(3));
        bench.background = ColorChoice::Entry(5);
        let stored = |bench: &Bench, i: u8| bench.image.palette().unwrap().get(i).unwrap();
        // Foreground (the editor closed): the entry 3.
        let f = bench.rest();
        assert!(
            f.there_is_fill(bar_rect(&bench), color32(stored(&bench, 3))),
            "ppp {ppp}"
        );
        assert!(f.paints("Idx-3"));
        // The editor open on the background: the entry 5 and its text.
        bench.editing = Role::Background;
        bench.view.editor_open = true;
        let f = bench.rest();
        assert!(
            f.there_is_fill(bar_rect(&bench), color32(stored(&bench, 5))),
            "ppp {ppp}"
        );
        assert!(f.paints("Idx-5") && !f.paints("Idx-3"));
    }
}

#[test]
fn row_10_the_text_is_black_or_white_by_contrast_and_with_alpha_it_goes_in_a_box() {
    let mut bench = Bench::new(
        1.0,
        SCREEN_HIGH,
        indexed_with_alpha(128),
        ColorChoice::Entry(0),
    );
    for (entry, ink) in [(0u8, WHITE), (1, BLACK), (3, BLACK)] {
        bench.foreground = ColorChoice::Entry(entry);
        let f = bench.rest();
        let label = format!("Idx-{entry}");
        let painted = f.text(&label).expect("the text of the bar");
        assert_eq!(painted.color, ink, "entry {entry}");
        assert_eq!(bar_ink(Rgba::opaque(0, 0, 0)).text, WHITE);
        assert!(
            contrast_ratio(
                painted.color,
                color32(bench.image.palette().unwrap().get(entry).unwrap())
            ) >= 4.5
        );
    }
    // Alpha under 255: the checker of the swatches behind and an opaque SURFACE_0 box for the text.
    bench.foreground = ColorChoice::Entry(2);
    let f = bench.rest();
    let bar = bar_rect(&bench);
    let inside = |c: Color32| {
        f.fills
            .iter()
            .any(|(r, fill)| *fill == c && bar.contains_rect(r.expand(0.05)))
    };
    assert!(
        inside(Color32::from_rgb(CHECK_A.r, CHECK_A.g, CHECK_A.b)),
        "checker"
    );
    assert!(
        inside(Color32::from_rgb(CHECK_B.r, CHECK_B.g, CHECK_B.b)),
        "checker"
    );
    assert!(inside(SURFACE_0), "box for the text");
    assert_eq!(f.text("Idx-2").expect("text").color, TEXT);
}

#[test]
fn row_12_f1_over_the_bar_or_the_field_opens_the_help_of_the_bar() {
    let mut bench = Bench::new(1.0, SCREEN_HIGH, indexed(8), ColorChoice::Entry(3));
    let rect = bar_rect(&bench);
    let f = bench.moves((rect.center().x as i64, rect.center().y as i64));
    assert_eq!(f.info.help, Some(HelpTarget::ColorBar));
    let at = field_center(&bench);
    let f = bench.moves(at);
    assert_eq!(f.info.help, Some(HelpTarget::ColorBar));
    let f = bench.moves((600, 300));
    assert_eq!(f.info.help, None, "outside the panel there is none");
}

#[test]
fn row_10_a_click_on_the_bar_does_nothing() {
    let mut bench = Bench::new(1.0, SCREEN_HIGH, indexed(8), ColorChoice::Entry(3));
    let rect = bar_rect(&bench);
    let at = ((rect.center().x) as i64, (rect.center().y) as i64);
    let f = bench.click(at);
    assert_eq!(f.output, Default::default());
    assert!(bench.history_is_empty());
}

// ---------------------------------------------------------------------------------------------
// Row 10: the hexadecimal field, outside the editor
// ---------------------------------------------------------------------------------------------

fn focus_field(bench: &mut Bench) {
    let at = field_center(bench);
    bench.click(at);
    bench.rest();
    assert!(bench.wants_keyboard(), "a click on the field focuses it");
}

fn write(bench: &mut Bench, text: &str) {
    bench.frame(vec![egui::Event::Key {
        key: Key::A,
        physical_key: Some(Key::A),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            ctrl: true,
            command: true,
            ..egui::Modifiers::NONE
        },
    }]);
    bench.frame(vec![egui::Event::Text(text.to_owned())]);
}

#[test]
fn row_10_with_the_editor_closed_the_field_shows_the_stored_color_and_confirms_with_enter() {
    for ppp in PPPS {
        let mut bench = Bench::new(ppp, SCREEN_HIGH, indexed(8), ColorChoice::Entry(3));
        bench.view.editor_open = false;
        let stored = bench.image.palette().unwrap().get(3).unwrap();
        let f = bench.rest();
        assert!(f.paints(&hex_text(stored)), "ppp {ppp}: the field shows it");
        focus_field(&mut bench);
        write(&mut bench, "#ff8800");
        bench.history.clear();
        bench.key(Key::Enter);
        bench.rest();
        assert_eq!(
            bench.commits(),
            vec![Commit {
                target: EditTarget::Entry(3),
                color: Rgba::opaque(255, 136, 0)
            }],
            "ppp {ppp}"
        );
        assert!(bench.notices().is_empty());
    }
}

#[test]
fn row_10_an_invalid_text_warns_in_the_status_bar_and_the_border_turns_to_warning_until_typing() {
    let mut bench = Bench::new(1.0, SCREEN_HIGH, indexed(8), ColorChoice::Entry(3));
    bench.view.editor_open = false;
    focus_field(&mut bench);
    write(&mut bench, "zzz");
    bench.history.clear();
    let f = bench.key(Key::Enter);
    assert_eq!(f.output.commit, None);
    assert_eq!(f.output.notice.as_deref(), Some(REASON_DIGIT));
    let f = bench.rest();
    assert!(
        bench.wants_keyboard(),
        "Enter keeps the focus on an invalid text"
    );
    assert!(
        f.there_is_stroke(WARNING),
        "the border is WARNING while the text is invalid"
    );
    // Typing clears the warning.
    bench.frame(vec![egui::Event::Text("q".to_owned())]);
    let f = bench.rest();
    assert!(!f.there_is_stroke(WARNING));
    // Esc reverts the text and nothing is warned again.
    bench.history.clear();
    bench.key(Key::Escape);
    let f = bench.rest();
    assert!(bench.notices().is_empty() && bench.commits().is_empty());
    assert!(!f.there_is_stroke(WARNING));
    assert!(f.paints(&hex_text(bench.image.palette().unwrap().get(3).unwrap())));
}

#[test]
fn row_10_the_alpha_of_the_stored_color_is_kept_or_set_by_the_typed_digits() {
    let mut bench = Bench::new(
        1.0,
        SCREEN_HIGH,
        indexed_with_alpha(128),
        ColorChoice::Entry(2),
    );
    bench.view.editor_open = false;
    focus_field(&mut bench);
    write(&mut bench, "#ff0000");
    bench.history.clear();
    bench.key(Key::Enter);
    bench.rest();
    assert_eq!(
        bench.commits(),
        vec![Commit {
            target: EditTarget::Entry(2),
            color: Rgba::new(255, 0, 0, 128)
        }],
        "six digits keep the alpha"
    );
}

#[test]
fn row_10_the_open_editor_paints_neither_header_nor_field_nor_notice_lines() {
    for ppp in PPPS {
        let mut bench = Bench::new(ppp, SCREEN_HIGH, indexed(8), ColorChoice::Entry(3));
        bench.view.editor_open = true;
        let f = bench.rest();
        assert!(!f.paints_something_with("editar"), "ppp {ppp}: no header");
        assert!(
            !f.paints_something_with("aviso"),
            "ppp {ppp}: no notice lines"
        );
        let hex = hex_text(bench.image.palette().unwrap().get(3).unwrap());
        assert_eq!(
            f.texts.iter().filter(|t| t.text == hex).count(),
            1,
            "one field, not two"
        );
        // The sliders are still there: the letters R G B H S L A.
        for label in ["R", "G", "B", "H", "S", "L", "A"] {
            assert!(f.paints(label), "ppp {ppp}: slider {label}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Row 11: the tail under the grid and the minimums
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_the_tail_hangs_from_a_short_grid_and_is_anchored_below_a_tall_one() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        for open in [false, true] {
            // 8 entries and the handle: one row; the bar starts at 63 u.
            let mut bench =
                Bench::with_panel_height(ppp, 600 * n, indexed(8), ColorChoice::Entry(3));
            bench.view.editor_open = open;
            let f = bench.rest();
            assert_eq!(bench.tail_top(), (GRID_Y_U + PITCH_U) * n);
            assert!(f.there_is_fill(
                bar_rect(&bench),
                color32(bench.image.palette().unwrap().get(3).unwrap())
            ));
            // 256 entries and the handle in a low panel: 17 rows do not fit, so the tail is
            // anchored under the visible rows (open: above the editor).
            let mut bench =
                Bench::with_panel_height(ppp, 330 * n, indexed(256), ColorChoice::Entry(3));
            bench.view.editor_open = open;
            let f = bench.rest();
            let layout = bench.layout();
            let tail = bench.tail_top();
            assert_eq!(tail, layout.tail_top_px.min((GRID_Y_U + 17 * PITCH_U) * n));
            assert!(
                tail + tail_height_u(Mode::Indexed) * n
                    <= if open {
                        layout.top_px
                    } else {
                        bench.panel_h_px()
                    }
            );
            assert!(f.there_is_fill(
                bar_rect(&bench),
                color32(bench.image.palette().unwrap().get(3).unwrap())
            ));
        }
    }
}

#[test]
fn row_11_the_minimums_are_322_u_by_468_u_and_the_closed_block_fits_with_nothing_to_spare() {
    for (i, ppp) in PPPS.iter().enumerate() {
        let n = i64::from(text_scale(*ppp));
        let height = [322.0f32, 257.6, 429.33, 322.0, 322.0][i];
        assert!(
            (min_window_height_pt(*ppp) - height).abs() < 0.01,
            "ppp {ppp}"
        );
        assert!(min_window_height_pt(*ppp) <= 432.0);
        assert!((min_window_width_pt(*ppp) * ppp - (468 * n) as f32).abs() < 0.01);
        assert_eq!(min_panel_height_px(n as u32), 286 * n);
        // 20 tools at the minimum column plus the closed block: exactly the minimum panel.
        assert_eq!(head_u(20, COLUMN_MIN_COLS) + 127, 286);
    }
    for invalid in [f32::NAN, 0.0, -1.0, f32::INFINITY] {
        assert!((min_window_height_pt(invalid) - 322.0).abs() < 0.01);
    }
}

// ---------------------------------------------------------------------------------------------
// Property 3: contrast of the bar text
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn property_3_the_text_of_the_bar_has_contrast_with_what_is_behind_it(
        r in any::<u8>(), g in any::<u8>(), b in any::<u8>(), a in any::<u8>(),
    ) {
        let color = Rgba::new(r, g, b, a);
        let ink = bar_ink(color);
        prop_assert_eq!(bar_ink(color), ink, "deterministic");
        prop_assert!(ink.text == BLACK || ink.text == WHITE || ink.text == TEXT);
        if a == 255 {
            prop_assert!(!ink.boxed);
            prop_assert_eq!(ink.behind, color32(color));
            prop_assert!(ink.text == BLACK || ink.text == WHITE);
            prop_assert!(contrast_ratio(ink.text, ink.behind) >= 4.5);
        } else {
            prop_assert!(ink.boxed);
            prop_assert_eq!(ink.behind, SURFACE_0);
            prop_assert_eq!(ink.text, TEXT);
            prop_assert!(contrast_ratio(ink.text, ink.behind) >= 4.5);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Property 4: geometry of the tail, the editor and the minimums
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_4_the_tail_pieces_are_whole_fit_and_do_not_overlap(
        step in 0u32..=60,
        mode in prop::sample::select(vec![Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha]),
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
        let grid_bottom = (GRID_Y_U + rows.min(layout.visible_rows) as i64 * PITCH_U) * k;
        // Hung from a short grid, anchored below a tall one.
        let by_formula = ((GRID_Y_U + rows as i64 * PITCH_U) * k).min(space_bottom - tail_height_u(mode) * k);
        prop_assert_eq!(tail, by_formula.min(layout.tail_top_px));
        prop_assert_eq!(tail % k, 0);
        prop_assert!(tail >= grid_bottom, "the tail does not overlap the visible grid");
        prop_assert!(tail + tail_height_u(mode) * k <= space_bottom, "and ends before the editor");
        // Pieces in u from the top of the tail: whole, inside the column and disjoint.
        let mut pieces = vec![
            (BAR_X_U, BAR_Y_U, text_width_u(cols), BAR_H_U),
            (FIELD_X_U, FIELD_Y_U, FIELD_W_U, FIELD_H_U),
        ];
        for button in [PaletteButton::Remove, PaletteButton::RotateLeft, PaletteButton::RotateRight, PaletteButton::Transparent] {
            if let Some(rect) = button_rect_u(mode, button) {
                pieces.push(rect);
            }
        }
        for (i, a) in pieces.iter().enumerate() {
            prop_assert!(a.0 >= 0 && a.0 + a.2 + 4 <= width, "inside the column");
            prop_assert!(a.1 + a.3 <= tail_height_u(mode));
            for b in &pieces[i + 1..] {
                let overlap = a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3;
                prop_assert!(!overlap, "{a:?} and {b:?}");
            }
        }
        // The editor, when open, is anchored to the bottom, below the tail.
        if open {
            prop_assert!(layout.top_px + editor_height_u(mode) * k <= height);
            prop_assert!(height - (layout.top_px + editor_height_u(mode) * k) < k);
        }
        // With 256 entries the handle has its own row (17 at 16 columns).
        if mode == Mode::Indexed && entries == 256 {
            prop_assert_eq!(total_rows_cols(257, 16), 17);
        }
        // The minimums follow the formula and meet the ceilings.
        let h = ((20 + MENU_H_U + 286) * k) as f32 / ppp;
        prop_assert!((min_window_height_pt(ppp) - h).abs() < 0.01);
        prop_assert!(min_window_height_pt(ppp) <= 432.0 + 0.01);
        let w = (468 * k) as f32 / ppp;
        prop_assert!((min_window_width_pt(ppp) - w).abs() < 0.01);
        prop_assert!(min_window_width_pt(ppp) <= 683.0);
    }
}
