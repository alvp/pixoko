//! Unit C4 (specs/ui-color-block.md), egui bench without a window (the one of C3, extended with
//! the time of each frame for the tooltips and the meshes of a frame): rows 2 (borders), 3 (text and
//! fill), 4 (gestures, tooltip and `F1`), 6 (hexadecimal field and role), 7 (the HSV model), 8
//! (the square), 9 (the hue and alpha bars), 10 (keyboard), 11 (selector and sliders) and 12 (the
//! whole panel), and property 1 (the models never disagree). Nothing is captured from the desktop.
//!
//! The expected numbers come from the spec; the oracle of the square and of the bars is the
//! conversion `Hsv -> Rgba` (probed apart in row 7), not the code of the panel.

use egui::{Event, Key, Modifiers, PointerButton, Rect, pos2, vec2};
use proptest::prelude::*;
use px_core::{Image, Mode, Palette, Rgba};
use px_ui::color::{ColorChoice, Role};
use px_ui::color_block::{
    Arrow, BarHit, Piece, SELECTOR_X_U, SELECTOR_Y_U, SQUARE_X_U, SQUARE_Y_U, alpha_mesh,
    bar_cell_rect, bar_fill_rect, bar_key, bar_value_at, hue_mesh, piece_rect, selector_rect,
    square_key, square_value_at, sv_texel, swap_rect,
};
use px_ui::color_edit::{
    ColorEditor, Commit, Control, EditTarget, SliderMode, apply_control_aux, hex_text,
    visible_controls,
};
use px_ui::color_editor::{hex_field_id, piece_id, sv_uploads};
use px_ui::color_hsv::{Hsv, hsv_after_rgb};
use px_ui::edit_layout::control_row_u;
use px_ui::help::HelpTarget;
use px_ui::palette_grid::{CHECK_A, CHECK_B, ring_rects};
use px_ui::palette_panel::BarClick;
use px_ui::text::text_scale;
use px_ui::theme::{ACCENT, SURFACE_0, SURFACE_3, TEXT};
use px_ui::view::ScreenRect;

#[path = "common/c3_bench.rs"]
mod bench;
#[path = "common/icons.rs"]
mod icons;
#[path = "common/zones.rs"]
mod zones;

use bench::{Bench, PPPS};

const PANEL_COLS: i64 = 16;
const WARM: Rgba = Rgba::opaque(200, 60, 40);

fn color32(color: Rgba) -> egui::Color32 {
    egui::Color32::from_rgb(color.r, color.g, color.b)
}

/// Indexed 8 entries: 0 warm (hue 7,5), 1 transparent, 2 stored alpha 0 (not the transparent
/// one), 3 opaque dark, 4 gray, 5 black, 6 a translucent green and 7 a pure red.
fn palette_image() -> Image {
    let colors = [
        WARM,
        Rgba::opaque(10, 20, 30),
        Rgba::new(90, 20, 160, 0),
        Rgba::opaque(30, 60, 90),
        Rgba::opaque(90, 90, 90),
        Rgba::opaque(0, 0, 0),
        Rgba::new(40, 180, 90, 128),
        Rgba::opaque(255, 0, 0),
    ];
    let mut palette = Palette::from_colors(colors).expect("palette");
    palette.set_transparent_index(Some(1)).expect("exists");
    Image::new_indexed(8, 4, palette).expect("image")
}

fn stored(bench: &Bench, entry: u8) -> Rgba {
    bench.image.palette().unwrap().get(entry).unwrap()
}

/// A bench with the editor open (and the panel tall enough not to scroll).
fn open_bench(ppp: f32, image: Image, choice: ColorChoice) -> Bench {
    let n = i64::from(text_scale(ppp));
    let mut bench = Bench::with_panel_height(ppp, 700 * n, image, choice);
    bench.view.editor_open = true;
    bench.rest();
    bench.rest();
    bench.history.clear();
    bench
}

/// A point `(x_u, y_u)` in `u` from the corner of the open editor, in physical pixels.
fn in_editor(bench: &Bench, x_u: i64, y_u: i64) -> (i64, i64) {
    let n = bench.n();
    (x_u * n, bench.layout().top_px + y_u * n)
}

/// A point `(x, y)` physical pixels from the corner of the editor, in the window.
fn editor_px(bench: &Bench, x: i64, y: i64) -> (i64, i64) {
    (x, bench.layout().top_px + y)
}

/// A rectangle in physical pixels from the corner of the tail, in points.
fn tail_pt(bench: &Bench, r: ScreenRect) -> Rect {
    let ppp = bench.ppp();
    Rect::from_min_size(
        pos2(r.x as f32 / ppp, (bench.tail_top() + r.y) as f32 / ppp),
        vec2(r.width as f32 / ppp, r.height as f32 / ppp),
    )
}

fn in_tail(bench: &Bench, r: ScreenRect) -> (i64, i64) {
    (r.x + r.width / 2, bench.tail_top() + r.y + r.height / 2)
}

fn press(key: Key, modifiers: Modifiers, repeat: bool) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat,
        modifiers,
    }
}

fn release(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

// ---------------------------------------------------------------------------------------------
// Row 2: borders of the bars
// ---------------------------------------------------------------------------------------------

#[test]
#[ignore = "C4: pending the two bars"]
fn row_2_the_bar_the_field_writes_to_has_the_2_u_accent_border_also_with_the_editor_closed() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        for open in [false, true] {
            for editing in [Role::Foreground, Role::Background] {
                let mut bench =
                    Bench::with_panel_height(ppp, 700 * n, palette_image(), ColorChoice::Entry(3));
                bench.background = ColorChoice::Entry(5);
                bench.editing = editing;
                bench.view.editor_open = open;
                let f = bench.rest();
                for role in [Role::Foreground, Role::Background] {
                    let fill = bar_fill_rect(role, PANEL_COLS, n as u32);
                    let (thickness, tone) = if role == editing {
                        (2, ACCENT)
                    } else {
                        (1, SURFACE_3)
                    };
                    for part in ring_rects(fill, thickness * n) {
                        assert!(
                            f.there_is_fill(tail_pt(&bench, part), tone),
                            "ppp {ppp}, open {open}, editing {editing:?}, bar {role:?}"
                        );
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Row 3: text and fill of each bar
// ---------------------------------------------------------------------------------------------

#[test]
#[ignore = "C4: pending the two bars"]
fn row_3_each_bar_says_what_it_paints_and_paints_the_resolved_color() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        // Indexed: the transparent entry says `transparente`; an entry whose stored alpha is 0 and
        // is not the transparent one keeps `Idx-N`; both are painted over the checker.
        let mut bench = Bench::new(
            ppp,
            bench::SCREEN_HIGH,
            palette_image(),
            ColorChoice::Entry(1),
        );
        bench.background = ColorChoice::Entry(2);
        let f = bench.rest();
        assert!(f.paints("transparente"), "ppp {ppp}");
        assert!(f.paints("Idx-2") && !f.paints("Idx-1"), "ppp {ppp}");
        for role in [Role::Foreground, Role::Background] {
            let fill = tail_pt(&bench, bar_fill_rect(role, PANEL_COLS, n));
            for tone in [CHECK_A, CHECK_B] {
                assert!(
                    f.fills
                        .iter()
                        .any(|(r, c)| *c == color32(tone) && fill.contains_rect(r.expand(0.05))),
                    "ppp {ppp}, {role:?}: the checker"
                );
            }
        }
        // An opaque color: its resolved color, no checker, and the text black or white.
        bench.foreground = ColorChoice::Entry(3);
        bench.background = ColorChoice::Entry(7);
        let f = bench.rest();
        assert!(f.paints("Idx-3") && f.paints("Idx-7"));
        let fill = tail_pt(&bench, bar_fill_rect(Role::Foreground, PANEL_COLS, n));
        assert!(
            f.there_is_fill(fill, color32(stored(&bench, 3))),
            "ppp {ppp}"
        );
        // Translucent: the checker and the text in a box of SURFACE_0.
        bench.foreground = ColorChoice::Entry(6);
        let f = bench.rest();
        let fill = tail_pt(&bench, bar_fill_rect(Role::Foreground, PANEL_COLS, n));
        assert!(
            f.fills
                .iter()
                .any(|(r, c)| *c == SURFACE_0 && fill.contains_rect(r.expand(0.05)))
        );
        assert_eq!(f.text("Idx-6").expect("text").color, TEXT);
        // The modes with slots: `Col-N`, and `transparente` for the slot with alpha 0.
        for (mode, transparent_slot) in [
            (Mode::Rgba, true),
            (Mode::GrayAlpha, true),
            (Mode::Gray, false),
        ] {
            let image = Image::new(8, 4, mode).expect("image");
            let mut bench = Bench::new(ppp, bench::SCREEN_HIGH, image, ColorChoice::Slot(3));
            bench.background = ColorChoice::Slot(9);
            let f = bench.rest();
            assert!(f.paints("Col-3"), "{mode:?}");
            assert_eq!(f.paints("transparente"), transparent_slot, "{mode:?}");
            assert_eq!(f.paints("Col-9"), !transparent_slot, "{mode:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Row 4: gestures, tooltip and F1
// ---------------------------------------------------------------------------------------------

#[test]
#[ignore = "C4: pending the two bars"]
fn row_4_the_left_click_asks_for_the_dropper_the_right_one_for_the_editor_and_the_button_swaps() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        for (open, editing) in [(false, Role::Foreground), (true, Role::Background)] {
            let mut bench = Bench::with_panel_height(
                ppp,
                700 * i64::from(n),
                palette_image(),
                ColorChoice::Entry(3),
            );
            bench.background = ColorChoice::Entry(5);
            bench.view.editor_open = open;
            bench.editing = editing;
            bench.rest();
            for role in [Role::Foreground, Role::Background] {
                let at = in_tail(&bench, bar_cell_rect(role, PANEL_COLS, n));
                bench.history.clear();
                let f = bench.click(at);
                assert_eq!(
                    f.output.bar_click,
                    Some(BarClick::Dropper(role)),
                    "ppp {ppp}"
                );
                let f = bench.right_click(at);
                assert_eq!(
                    f.output.bar_click,
                    Some(BarClick::FocusEditor(role)),
                    "ppp {ppp}"
                );
            }
            let at = in_tail(&bench, swap_rect(PANEL_COLS, n));
            let f = bench.click(at);
            assert_eq!(f.output.bar_click, Some(BarClick::Swap), "ppp {ppp}");
            // The middle button, the gap before the button and the grid open nothing.
            bench.history.clear();
            let bg = bar_cell_rect(Role::Background, PANEL_COLS, n);
            let (x, y) = in_tail(&bench, bg);
            let middle = Event::PointerButton {
                pos: pos2(x as f32 / bench.ppp(), y as f32 / bench.ppp()),
                button: PointerButton::Middle,
                pressed: true,
                modifiers: Modifiers::NONE,
            };
            let f = bench.frame(vec![middle]);
            assert_eq!(f.output.bar_click, None);
            let gap = (bg.x + bg.width + i64::from(n) / 2 + 1, y);
            assert_eq!(bench.click(gap).output.bar_click, None);
            let f = bench.click(bench.cell_center(4));
            assert_eq!(f.output.bar_click, None);
            assert_eq!(
                f.output.choice,
                Some(ColorChoice::Entry(4)),
                "the grid still chooses"
            );
            // Disabled (a drawing gesture) or with a slider drag in progress: nothing answers.
            bench.gesture_drawing = true;
            let at = in_tail(&bench, bar_cell_rect(Role::Background, PANEL_COLS, n));
            assert_eq!(bench.click(at).output.bar_click, None);
            bench.gesture_drawing = false;
            if open {
                assert!(bench.editor.begin(Control::Red, 10));
                assert_eq!(bench.click(at).output.bar_click, None);
                bench.editor.cancel();
            }
        }
    }
}

#[test]
#[ignore = "C4: pending the two bars"]
fn row_4_the_right_click_asks_for_the_focus_of_the_field_unless_it_closes_the_editor() {
    let n = text_scale(1.0);
    let bar = |role| bar_cell_rect(role, PANEL_COLS, n);
    // Closed: a right click on the background bar opens the editor on it and the field gets the
    // focus in a frame without pointer (the session does the opening; the bench plays it).
    let mut bench = open_bench(1.0, palette_image(), ColorChoice::Entry(3));
    bench.view.editor_open = false;
    bench.rest();
    let at = in_tail(&bench, bar(Role::Background));
    bench.right_click(at);
    bench.editing = Role::Background;
    bench.view.editor_open = true;
    for _ in 0..3 {
        bench.rest();
    }
    assert!(
        bench.ctx.memory(|m| m.has_focus(hex_field_id())),
        "the field has the focus"
    );
    // A second right click on the edited bar closes the editor: nothing is focused.
    let mut bench = open_bench(1.0, palette_image(), ColorChoice::Entry(3));
    let at = in_tail(&bench, bar(Role::Foreground));
    let f = bench.right_click(at);
    assert_eq!(
        f.output.bar_click,
        Some(BarClick::FocusEditor(Role::Foreground))
    );
    for _ in 0..3 {
        bench.rest();
    }
    assert!(!bench.ctx.memory(|m| m.has_focus(hex_field_id())));
    // On the other bar, the role changes without closing and the field is focused.
    let at = in_tail(&bench, bar(Role::Background));
    bench.right_click(at);
    bench.editing = Role::Background;
    for _ in 0..3 {
        bench.rest();
    }
    assert!(bench.ctx.memory(|m| m.has_focus(hex_field_id())));
}

#[test]
#[ignore = "C4: pending the two bars"]
fn row_4_the_tooltips_have_two_lines_and_f1_opens_the_right_help() {
    let n = text_scale(1.0);
    let tip = "clic: cuentagotas; clic derecho: editar este color";
    for (role, name) in [
        (Role::Foreground, "Primer plano"),
        (Role::Background, "Fondo"),
    ] {
        let mut bench = open_bench(1.0, palette_image(), ColorChoice::Entry(3));
        let at = in_tail(&bench, bar_cell_rect(role, PANEL_COLS, n));
        let shown = bench.tooltip_texts(at);
        assert!(shown.iter().any(|t| t == name), "{name}: {shown:?}");
        assert!(shown.iter().any(|t| t == tip), "{name}: {shown:?}");
        assert_eq!(bench.rest().info.help, Some(HelpTarget::ColorBar));
    }
    let mut bench = open_bench(1.0, palette_image(), ColorChoice::Entry(3));
    let at = in_tail(&bench, swap_rect(PANEL_COLS, n));
    let shown = bench.tooltip_texts(at);
    assert!(
        shown
            .iter()
            .any(|t| t == "Intercambiar primer plano y fondo"),
        "{shown:?}"
    );
    assert_eq!(bench.rest().info.help, Some(HelpTarget::SwapColors));
    // The field keeps the help of the bars; the square, the bars, the selector and the sliders
    // have the one of the editor; the grid, none.
    let field = (
        px_ui::edit_layout::FIELD_X_U + 10,
        px_ui::edit_layout::FIELD_Y_U + 9,
    );
    let at = (field.0, bench.tail_top() + field.1);
    assert_eq!(bench.moves(at).info.help, Some(HelpTarget::ColorBar));
    for (x_u, y_u) in [
        (60, 60),
        (125, 60),
        (141, 60),
        (20, SELECTOR_Y_U + 5),
        (100, 144),
    ] {
        let at = in_editor(&bench, x_u, y_u);
        assert_eq!(
            bench.moves(at).info.help,
            Some(HelpTarget::ColorEditor),
            "({x_u}, {y_u})"
        );
    }
    let at = bench.cell_center(2);
    assert_eq!(bench.moves(at).info.help, None);
}

// ---------------------------------------------------------------------------------------------
// Row 6: the hexadecimal field writes to the role the editor edits
// ---------------------------------------------------------------------------------------------

fn write_hex(bench: &mut Bench, text: &str) {
    let (x, y) = (
        (px_ui::edit_layout::FIELD_X_U + 20) * bench.n(),
        bench.tail_top() + (px_ui::edit_layout::FIELD_Y_U + 9) * bench.n(),
    );
    bench.click((x, y));
    bench.rest();
    assert!(bench.wants_keyboard(), "a click on the field focuses it");
    bench.frame(vec![Event::Key {
        key: Key::A,
        physical_key: Some(Key::A),
        pressed: true,
        repeat: false,
        modifiers: Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        },
    }]);
    bench.frame(vec![Event::Text(text.to_owned())]);
    bench.history.clear();
    bench.key(Key::Enter);
    bench.rest();
}

#[test]
fn row_6_the_field_edits_the_foreground_closed_and_the_role_of_the_open_editor() {
    let mut bench = open_bench(1.0, palette_image(), ColorChoice::Entry(3));
    bench.background = ColorChoice::Entry(5);
    bench.view.editor_open = false;
    let f = bench.rest();
    assert!(
        f.paints(&hex_text(stored(&bench, 3))),
        "the field shows the foreground"
    );
    write_hex(&mut bench, "#ff8800");
    assert_eq!(
        bench.commits(),
        vec![Commit {
            target: EditTarget::Entry(3),
            color: Rgba::opaque(255, 136, 0)
        }]
    );
    // Open on the background (right click on its bar): the field shows and writes the background.
    bench.editing = Role::Background;
    bench.view.editor_open = true;
    let f = bench.rest();
    assert!(
        f.paints(&hex_text(stored(&bench, 5))),
        "the field shows the background"
    );
    write_hex(&mut bench, "#102030");
    assert_eq!(
        bench.commits(),
        vec![Commit {
            target: EditTarget::Entry(5),
            color: Rgba::opaque(16, 32, 48)
        }]
    );
    // Closed again: back to the foreground.
    bench.editing = Role::Foreground;
    bench.view.editor_open = false;
    let f = bench.rest();
    assert!(f.paints(&hex_text(stored(&bench, 3))));
}

#[test]
fn row_6_with_slots_the_alpha_the_invalid_text_and_escape_work_as_in_c3() {
    let image = Image::new(8, 4, Mode::Rgba).expect("image");
    let mut bench = Bench::new(1.0, bench::SCREEN_HIGH, image, ColorChoice::Slot(3));
    bench.view.editor_open = false;
    bench.rest();
    write_hex(&mut bench, "#ff8800");
    assert_eq!(
        bench.commits(),
        vec![Commit {
            target: EditTarget::Slot(3),
            color: Rgba::opaque(255, 136, 0)
        }]
    );
    // The background (slot 9, alpha 0): six digits keep the alpha and eight set it.
    bench.editing = Role::Background;
    bench.view.editor_open = true;
    bench.rest();
    write_hex(&mut bench, "#ff8800");
    assert_eq!(
        bench.commits(),
        vec![Commit {
            target: EditTarget::Slot(9),
            color: Rgba::new(255, 136, 0, 0)
        }]
    );
    write_hex(&mut bench, "#ff8800aa");
    assert_eq!(
        bench.commits(),
        vec![Commit {
            target: EditTarget::Slot(9),
            color: Rgba::new(255, 136, 0, 0xaa)
        }]
    );
    // Invalid: a notice and no commit.
    write_hex(&mut bench, "zzz");
    assert!(bench.commits().is_empty());
    assert_eq!(bench.notices().len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Row 7: the HSV model
// ---------------------------------------------------------------------------------------------

fn editor_on(color: Rgba) -> ColorEditor {
    let mut editor = ColorEditor::default();
    editor.sync(Some((EditTarget::Entry(0), color)));
    editor
}

/// What the editor shows: color, `Hsl` tone, and `Hsv`.
fn shown(editor: &ColorEditor) -> (Rgba, Hsv) {
    let (rgba, hsl) = editor.shown().expect("a target");
    let hsv = editor.shown_hsv().expect("a target");
    assert_eq!(hsv.to_rgba(rgba.a), rgba, "hsv and rgba agree");
    assert_eq!(hsl.to_rgba(rgba.a), rgba, "hsl and rgba agree");
    (rgba, hsv)
}

/// Releases the drag and brings the editor up to date with the color it committed, as the panel
/// and the session do.
fn finish(editor: &mut ColorEditor) -> Option<Commit> {
    let commit = editor.release();
    if let Some(commit) = commit {
        editor.sync(Some((commit.target, commit.color)));
    }
    commit
}

#[test]
fn row_7_hsv_converts_the_corners_of_the_cube_and_a_sample_and_normalizes() {
    for (color, h, s, v) in [
        (Rgba::opaque(0, 0, 0), 0.0, 0.0, 0.0),
        (Rgba::opaque(255, 255, 255), 0.0, 0.0, 1.0),
        (Rgba::opaque(255, 0, 0), 0.0, 1.0, 1.0),
        (Rgba::opaque(255, 255, 0), 60.0, 1.0, 1.0),
        (Rgba::opaque(0, 255, 0), 120.0, 1.0, 1.0),
        (Rgba::opaque(0, 255, 255), 180.0, 1.0, 1.0),
        (Rgba::opaque(0, 0, 255), 240.0, 1.0, 1.0),
        (Rgba::opaque(255, 0, 255), 300.0, 1.0, 1.0),
        (WARM, 7.5, 0.8, 200.0 / 255.0),
    ] {
        let hsv = Hsv::from_rgba(color);
        assert!((hsv.h() - h).abs() < 1e-3, "{color:?}: h = {}", hsv.h());
        assert!(
            (hsv.s() - s).abs() < 1e-4 && (hsv.v() - v).abs() < 1e-4,
            "{color:?}"
        );
        assert_eq!(hsv.to_rgba(color.a), color);
        assert_eq!(hsv.to_rgba(77).a, 77, "the alpha is passed apart");
    }
    assert_eq!(Hsv::new(-30.0, 2.0, -1.0), Hsv::new(330.0, 1.0, 0.0));
    assert_eq!(Hsv::new(360.0, f32::NAN, 0.5).h(), 0.0);
    assert_eq!(Hsv::new(f32::INFINITY, 0.5, 0.5).h(), 0.0);
    // After editing RGB: gray keeps the hue, black keeps hue and saturation, the rest is `from_rgba`.
    let previous = Hsv::new(210.0, 0.6, 0.4);
    assert_eq!(
        hsv_after_rgb(previous, Rgba::opaque(90, 90, 90)),
        Hsv::new(210.0, 0.0, 90.0 / 255.0)
    );
    assert_eq!(
        hsv_after_rgb(previous, Rgba::opaque(0, 0, 0)),
        Hsv::new(210.0, 0.6, 0.0)
    );
    assert_eq!(hsv_after_rgb(previous, WARM), Hsv::from_rgba(WARM));
}

#[test]
fn row_7_the_square_and_rgb_never_make_the_hue_jump_through_gray_or_black() {
    let (_, v0) = (0, 200.0f32 / 255.0);
    // The square: saturation to 0 and back.
    let mut editor = editor_on(WARM);
    assert!(editor.begin_square(0.0, v0));
    assert_eq!(shown(&editor).0, Rgba::opaque(200, 200, 200));
    assert!(
        (shown(&editor).1.h() - 7.5).abs() < 1e-3,
        "the hue stays while it is gray"
    );
    assert!(finish(&mut editor).is_some());
    assert!(editor.begin_square(0.8, v0));
    assert_eq!(shown(&editor).0, WARM, "back to the same color");
    editor.cancel();
    // Brightness to 0 keeps hue and saturation.
    let mut editor = editor_on(WARM);
    assert!(editor.begin_square(0.8, 0.0));
    let (rgba, hsv) = shown(&editor);
    assert_eq!(rgba, Rgba::opaque(0, 0, 0));
    assert!((hsv.h() - 7.5).abs() < 1e-3 && (hsv.s() - 0.8).abs() < 1e-4);
    // The square never changes the hue nor the alpha.
    let translucent = Rgba::new(30, 60, 90, 77);
    let mut editor = editor_on(translucent);
    let h = shown(&editor).1.h();
    assert!(editor.begin_square(0.3, 0.9));
    editor.drag_square(1.0, 1.0);
    let (rgba, hsv) = shown(&editor);
    assert_eq!((hsv.h(), rgba.a), (h, 77));
    assert!(editor.dragging_square());
    editor.cancel();
    // RGB that ends in gray keeps the hue it had just before; in black, the hue and the saturation.
    let mut editor = editor_on(Rgba::opaque(30, 60, 90));
    assert!(editor.begin(Control::Red, 60));
    finish(&mut editor);
    let (_, chromatic) = shown(&editor);
    assert!(editor.begin(Control::Blue, 60));
    finish(&mut editor);
    let (rgba, gray) = shown(&editor);
    assert_eq!(rgba, Rgba::opaque(60, 60, 60));
    assert_eq!(gray.h(), chromatic.h());
    let mut editor = editor_on(Rgba::opaque(30, 60, 90));
    for control in [Control::Red, Control::Green] {
        assert!(editor.begin(control, 0));
        finish(&mut editor);
    }
    let (_, blue) = shown(&editor);
    assert!(editor.begin(Control::Blue, 0));
    finish(&mut editor);
    let (rgba, black) = shown(&editor);
    assert_eq!(rgba, Rgba::opaque(0, 0, 0));
    assert_eq!((black.h(), black.s()), (blue.h(), blue.s()));
    // The hue from a gray: the edited one is in both models.
    let mut editor = editor_on(Rgba::opaque(90, 90, 90));
    assert!(editor.begin(Control::HueV, 120));
    let (rgba, hsv) = shown(&editor);
    assert_eq!((rgba, hsv.h()), (Rgba::opaque(90, 90, 90), 120.0));
    let (_, hsl) = editor.shown().unwrap();
    assert_eq!(hsl.h(), 120.0, "the same hue in the two models");
    finish(&mut editor);
    assert!(editor.begin_square(1.0, 1.0));
    assert_eq!(
        shown(&editor).0,
        Rgba::opaque(0, 255, 0),
        "and now the hue shows"
    );
}

#[test]
fn row_7_the_auxiliary_is_recomputed_only_when_the_target_or_the_stored_color_change_outside() {
    // After its own confirmation the model is not derived again: the gray keeps its hue.
    let mut editor = editor_on(Rgba::opaque(30, 60, 90));
    let hue = shown(&editor).1.h();
    assert!(editor.begin_square(0.0, 0.5));
    let commit = finish(&mut editor).expect("a step");
    editor.sync(Some((commit.target, commit.color)));
    assert_eq!(
        shown(&editor).1.h(),
        hue,
        "own confirmation: not recomputed"
    );
    // Another target, or a color changed from outside (undo): `from_rgba`, so gray gives hue 0.
    editor.sync(Some((EditTarget::Entry(1), commit.color)));
    assert_eq!(shown(&editor).1.h(), 0.0);
    let mut editor = editor_on(Rgba::opaque(30, 60, 90));
    editor.sync(Some((EditTarget::Entry(0), Rgba::opaque(60, 60, 60))));
    assert_eq!(shown(&editor).1.h(), 0.0);
    // A drag that is cancelled leaves everything as it was.
    let mut editor = editor_on(WARM);
    let before = shown(&editor);
    assert!(editor.begin_square(0.1, 0.1));
    editor.cancel();
    assert_eq!(shown(&editor), before);
    // The same color again (`sync` to the same color): nothing changes.
    editor.sync(Some((EditTarget::Entry(0), WARM)));
    assert_eq!(shown(&editor), before);
}

// ---------------------------------------------------------------------------------------------
// Row 8: the square
// ---------------------------------------------------------------------------------------------

fn s_v_at(bench: &Bench, dx: i64, dy: i64) -> (f32, f32) {
    let n = bench.n() as u32;
    square_value_at(
        SQUARE_X_U * i64::from(n) + dx,
        SQUARE_Y_U * i64::from(n) + dy,
        n,
    )
}

#[test]
#[ignore = "C4: pending the square"]
fn row_8_the_pointer_maps_to_saturation_and_brightness_and_one_drag_is_one_step() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let side = 112 * n;
        let mut bench = open_bench(ppp, palette_image(), ColorChoice::Entry(0));
        let square = piece_rect(Piece::Square, n as u32);
        let at = |bench: &Bench, dx: i64, dy: i64| editor_px(bench, square.x + dx, square.y + dy);
        // The four corners are reached exactly, the outside is clamped.
        for (dx, dy, expected) in [
            (0, 0, (0.0, 1.0)),
            (side - 1, 0, (1.0, 1.0)),
            (0, side - 1, (0.0, 0.0)),
            (side - 1, side - 1, (1.0, 0.0)),
            (-5 * n, -5 * n, (0.0, 1.0)),
            (side + 9 * n, side + 9 * n, (1.0, 0.0)),
        ] {
            assert_eq!(s_v_at(&bench, dx, dy), expected, "ppp {ppp}, ({dx}, {dy})");
        }
        // Press: the value under the pointer; move: the new value; release: one commit.
        let (s0, v0) = s_v_at(&bench, side / 4, side / 2);
        let h0 = Hsv::from_rgba(WARM).h();
        bench.presses(at(&bench, side / 4, side / 2));
        assert!(bench.editor.dragging_square());
        assert_eq!(bench.editor.shown_hsv(), Some(Hsv::new(h0, s0, v0)));
        let (s1, v1) = s_v_at(&bench, 3 * side / 4, side / 4);
        bench.moves(at(&bench, 3 * side / 4, side / 4));
        assert_eq!(bench.editor.shown_hsv(), Some(Hsv::new(h0, s1, v1)));
        assert!(
            bench.commits().is_empty(),
            "the document does not change until the release"
        );
        let f = bench.releases(at(&bench, 3 * side / 4, side / 4));
        assert_eq!(
            f.output.commit,
            Some(Commit {
                target: EditTarget::Entry(0),
                color: Hsv::new(h0, s1, v1).to_rgba(255)
            }),
            "ppp {ppp}"
        );
        assert!(!bench.editor.dragging());
    }
    // No change, no step: a click on the corner of a pure red that is already (1, 1).
    let mut bench = open_bench(1.0, palette_image(), ColorChoice::Entry(7));
    let square = piece_rect(Piece::Square, 1);
    let corner = editor_px(&bench, square.x + square.width - 1, square.y);
    bench.click(corner);
    bench.rest();
    assert!(bench.commits().is_empty());
}

#[test]
#[ignore = "C4: pending the square"]
fn row_8_escape_and_losing_the_window_focus_restore_everything() {
    for cancel in [Key::Escape, Key::F24] {
        let mut bench = open_bench(1.0, palette_image(), ColorChoice::Entry(0));
        let before = (bench.editor.shown(), bench.editor.shown_hsv());
        let state = bench.doc_state;
        let square = piece_rect(Piece::Square, 1);
        bench.presses(editor_px(&bench, square.x + 30, square.y + 80));
        bench.moves(editor_px(&bench, square.x + 90, square.y + 10));
        assert!(bench.editor.dragging_square());
        assert_ne!(bench.editor.shown_hsv(), before.1);
        if cancel == Key::Escape {
            bench.escape();
        } else {
            bench.frame(vec![Event::WindowFocused(false)]);
        }
        assert!(!bench.editor.dragging());
        assert_eq!((bench.editor.shown(), bench.editor.shown_hsv()), before);
        // The release that follows does nothing and the document was never touched.
        bench.releases(editor_px(&bench, square.x + 90, square.y + 10));
        assert!(bench.commits().is_empty());
        assert_eq!(bench.doc_state, state);
    }
}

#[test]
#[ignore = "C4: pending the square"]
fn row_8_the_texture_is_the_oracle_texel_by_texel_and_is_uploaded_only_when_the_hue_changes() {
    for hue in [0u32, 120, 211] {
        for y in 0..112u32 {
            for x in 0..112u32 {
                let expected =
                    Hsv::new(hue as f32, x as f32 / 111.0, 1.0 - y as f32 / 111.0).to_rgba(255);
                assert_eq!(sv_texel(hue, x, y), expected, "hue {hue}, texel ({x}, {y})");
            }
        }
    }
    let mut bench = open_bench(1.0, palette_image(), ColorChoice::Entry(0));
    let base = sv_uploads(&bench.ctx);
    assert!(base >= 1);
    for _ in 0..5 {
        bench.rest();
    }
    assert_eq!(
        sv_uploads(&bench.ctx),
        base,
        "five frames with the same hue upload nothing"
    );
    // Dragging inside the square regenerates nothing.
    let square = piece_rect(Piece::Square, 1);
    bench.presses(editor_px(&bench, square.x + 10, square.y + 10));
    bench.moves(editor_px(&bench, square.x + 100, square.y + 90));
    bench.releases(editor_px(&bench, square.x + 100, square.y + 90));
    assert_eq!(sv_uploads(&bench.ctx), base);
    // Dragging the hue bar: one upload per new whole degree seen in a frame.
    let hue = piece_rect(Piece::HueBar, 1);
    let x = hue.x + 6;
    bench.presses(editor_px(&bench, x, hue.y + 20));
    bench.moves(editor_px(&bench, x, hue.y + 21));
    bench.moves(editor_px(&bench, x, hue.y + 21));
    bench.moves(editor_px(&bench, x, hue.y + 60));
    let shown = bench.editor.shown_hsv().unwrap().h().round() as u32 % 360;
    assert_eq!(shown, bar_value_at(Piece::HueBar, hue.y + 60 - hue.y, 1));
    assert_eq!(
        sv_uploads(&bench.ctx),
        base + 3,
        "three whole degrees, three uploads"
    );
}

// ---------------------------------------------------------------------------------------------
// Row 9: the hue and alpha bars
// ---------------------------------------------------------------------------------------------

#[test]
#[ignore = "C4: pending the bars"]
fn row_9_the_bars_map_the_pointer_and_a_drag_is_one_step() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let len = 112 * n;
        let hue = piece_rect(Piece::HueBar, n as u32);
        let alpha = piece_rect(Piece::AlphaBar, n as u32);
        // Hue: 0 at the top, 359 at the bottom; alpha: 255 at the top, 0 at the bottom.
        assert_eq!(bar_value_at(Piece::HueBar, hue.y, n as u32), 0);
        assert_eq!(bar_value_at(Piece::HueBar, hue.y + len - 1, n as u32), 359);
        assert_eq!(bar_value_at(Piece::HueBar, hue.y - 9 * n, n as u32), 0);
        assert_eq!(
            bar_value_at(Piece::HueBar, hue.y + len + 9 * n, n as u32),
            359
        );
        assert_eq!(bar_value_at(Piece::AlphaBar, alpha.y, n as u32), 255);
        assert_eq!(
            bar_value_at(Piece::AlphaBar, alpha.y + len - 1, n as u32),
            0
        );
        let mut bench = open_bench(ppp, palette_image(), ColorChoice::Entry(0));
        let (rgba0, hsv0) = (WARM, Hsv::from_rgba(WARM));
        let top = bench.layout().top_px;
        let at = |y| (hue.x + hue.width / 2, top + y);
        bench.presses(at(hue.y + len / 2));
        let value = bar_value_at(Piece::HueBar, hue.y + len / 2, n as u32);
        let shown = bench.editor.shown_hsv().unwrap();
        assert_eq!(
            (shown.s(), shown.v()),
            (hsv0.s(), hsv0.v()),
            "the mark of the square stays"
        );
        assert_eq!(shown.h().round() as u32, value);
        bench.moves(at(hue.y + len - 1));
        let f = bench.releases(at(hue.y + len - 1));
        let expected = Hsv::new(359.0, hsv0.s(), hsv0.v()).to_rgba(rgba0.a);
        assert_eq!(
            f.output.commit.map(|c| c.color),
            Some(expected),
            "ppp {ppp}"
        );
        bench.editor.sync(Some((EditTarget::Entry(0), expected)));
        // Alpha: only the alpha changes.
        let at = |y| (alpha.x + alpha.width / 2, top + y);
        bench.presses(at(alpha.y + len / 2));
        bench.moves(at(alpha.y));
        let f = bench.releases(at(alpha.y));
        let commit = f.output.commit;
        assert!(
            commit.is_none(),
            "alpha 255 at the top is the stored alpha: no step"
        );
        bench.presses(at(alpha.y + len - 1));
        let f = bench.releases(at(alpha.y + len - 1));
        let commit = f.output.commit.expect("a step");
        assert_eq!(
            (commit.color.r, commit.color.g, commit.color.b),
            (expected.r, expected.g, expected.b)
        );
        assert_eq!(commit.color.a, 0);
    }
}

#[test]
#[ignore = "C4: pending the bars"]
fn row_9_the_meshes_have_the_vertices_of_the_spec_and_the_marks_follow_the_model() {
    let hues = hue_mesh();
    let expected = [
        Rgba::opaque(255, 0, 0),
        Rgba::opaque(255, 255, 0),
        Rgba::opaque(0, 255, 0),
        Rgba::opaque(0, 255, 255),
        Rgba::opaque(0, 0, 255),
        Rgba::opaque(255, 0, 255),
        Rgba::opaque(255, 0, 0),
    ];
    for (i, (position, color)) in hues.iter().enumerate() {
        assert!((position - i as f32 / 6.0).abs() < 1e-6);
        assert_eq!(*color, expected[i]);
    }
    let color = Rgba::new(200, 60, 40, 128);
    let ends = alpha_mesh(color);
    assert_eq!(ends[0], (0.0, Rgba::opaque(200, 60, 40)));
    assert_eq!(ends[1], (1.0, Rgba::TRANSPARENT));
    // The bench paints a mesh of 14 vertices for the hue bar, in the colors of the mesh.
    let mut bench = open_bench(1.0, palette_image(), ColorChoice::Entry(0));
    let f = bench.rest();
    let hue = f
        .meshes
        .iter()
        .find(|m| m.len() == 14)
        .expect("the hue bar is a mesh of 7 pairs");
    for color in expected {
        assert!(hue.iter().any(|(_, c)| *c == color32(color)));
    }
    // The mark of the hue bar is a 14 u wide strip of TEXT and moves with the hue of the model,
    // also for a gray, whose hue is the one of the auxiliary.
    let strip_at = |f: &bench::Frame| {
        f.fills
            .iter()
            .filter(|(r, c)| {
                *c == TEXT && (r.width() - 14.0).abs() < 0.1 && (r.height() - 2.0).abs() < 0.1
            })
            .map(|(r, _)| (r.min.x, r.min.y))
            .next()
    };
    let top = strip_at(&f).expect("the mark of the hue bar");
    assert!((top.0 - (120.0 - 1.0)).abs() < 0.1);
    let mut gray = open_bench(1.0, palette_image(), ColorChoice::Entry(4));
    assert!(gray.editor.begin(Control::HueV, 240));
    gray.editor.release();
    let moved = strip_at(&gray.rest()).expect("the mark of the hue bar");
    assert!(moved.1 > top.1, "a gray keeps the visible hue it was given");
}

// ---------------------------------------------------------------------------------------------
// Row 10: keyboard
// ---------------------------------------------------------------------------------------------

/// Gives the focus to `piece` with a click on it and rests; returns the center of the piece.
fn focus(bench: &mut Bench, piece: Piece) -> (i64, i64) {
    let rect = piece_rect(piece, bench.n() as u32);
    let at = editor_px(bench, rect.x + rect.width / 2, rect.y + rect.height / 2);
    bench.click(at);
    bench.rest();
    bench.history.clear();
    assert!(
        bench.ctx.memory(|m| m.has_focus(piece_id(piece))),
        "{piece:?} has the focus"
    );
    at
}

#[test]
#[ignore = "C4: pending the keyboard"]
fn row_10_the_arrows_of_the_square_move_one_or_ten_and_press_to_release_is_one_step() {
    let mut bench = open_bench(1.0, palette_image(), ColorChoice::Entry(3));
    focus(&mut bench, Piece::Square);
    let hsv = bench.editor.shown_hsv().unwrap();
    let (s, v) = (
        (hsv.s() * 100.0).round() as u32,
        (hsv.v() * 100.0).round() as u32,
    );
    let color_of =
        |s: u32, v: u32| Hsv::new(hsv.h(), s as f32 / 100.0, v as f32 / 100.0).to_rgba(255);
    // Right: saturation +1; with Shift, +10; up: brightness +1.
    bench.frame(vec![press(Key::ArrowRight, Modifiers::NONE, false)]);
    assert!(bench.editor.dragging_square());
    assert_eq!(bench.editor.shown().unwrap().0, color_of(s + 1, v));
    assert!(
        bench.commits().is_empty(),
        "nothing until the key is released"
    );
    let f = bench.frame(vec![release(Key::ArrowRight)]);
    assert_eq!(f.output.commit.map(|c| c.color), Some(color_of(s + 1, v)));
    bench
        .editor
        .sync(Some((EditTarget::Entry(3), color_of(s + 1, v))));
    // Held: the repeats are the same step.
    bench.frame(vec![press(Key::ArrowUp, Modifiers::NONE, false)]);
    for _ in 0..3 {
        bench.frame(vec![press(Key::ArrowUp, Modifiers::NONE, true)]);
    }
    let held = bench.frame(vec![release(Key::ArrowUp)]);
    assert_eq!(
        held.output.commit.map(|c| c.color),
        Some(color_of(s + 1, v + 4))
    );
    bench
        .editor
        .sync(Some((EditTarget::Entry(3), color_of(s + 1, v + 4))));
    // Shift multiplies by ten, left and down go the other way, and the ends are clamped.
    let shift = Modifiers::SHIFT;
    bench.frame(vec![press(Key::ArrowLeft, shift, false)]);
    let f = bench.frame(vec![release(Key::ArrowLeft)]);
    assert_eq!(
        f.output.commit.map(|c| c.color),
        Some(color_of(s + 1 - 10, v + 4))
    );
    assert_eq!(square_key(100, 50, Arrow::Right, false), (100, 50));
    assert_eq!(square_key(0, 50, Arrow::Left, true), (0, 50));
    assert_eq!(square_key(50, 95, Arrow::Up, true), (50, 100));
    assert_eq!(square_key(50, 3, Arrow::Down, true), (50, 0));
}

#[test]
#[ignore = "C4: pending the keyboard"]
fn row_10_the_bars_take_the_arrows_escape_cancels_and_nothing_moves_without_focus_or_enabled() {
    assert_eq!(bar_key(Piece::HueBar, 10, Arrow::Up, false), 9);
    assert_eq!(bar_key(Piece::HueBar, 10, Arrow::Down, false), 11);
    assert_eq!(bar_key(Piece::HueBar, 355, Arrow::Down, true), 359);
    assert_eq!(bar_key(Piece::HueBar, 5, Arrow::Up, true), 0);
    assert_eq!(bar_key(Piece::AlphaBar, 254, Arrow::Up, false), 255);
    assert_eq!(bar_key(Piece::AlphaBar, 5, Arrow::Down, true), 0);
    let mut bench = open_bench(1.0, palette_image(), ColorChoice::Entry(0));
    // Without focus nothing happens.
    bench.frame(vec![press(Key::ArrowRight, Modifiers::NONE, false)]);
    assert!(!bench.editor.dragging());
    // The hue bar: the mark goes down with the down arrow.
    focus(&mut bench, Piece::HueBar);
    let h0 = bench.editor.shown_hsv().unwrap().h().round() as u32;
    bench.frame(vec![press(Key::ArrowDown, Modifiers::NONE, false)]);
    assert_eq!(bench.editor.shown_hsv().unwrap().h().round() as u32, h0 + 1);
    // Escape in the middle of the press cancels it; the release gives nothing.
    bench.escape();
    assert!(!bench.editor.dragging());
    let f = bench.frame(vec![release(Key::ArrowDown)]);
    assert_eq!(f.output.commit, None);
    // The alpha bar: up is +1 (here the alpha is already 255 and nothing changes).
    focus(&mut bench, Piece::AlphaBar);
    bench.frame(vec![press(Key::ArrowDown, Modifiers::NONE, false)]);
    assert_eq!(bench.editor.shown().unwrap().0.a, 254);
    bench.frame(vec![release(Key::ArrowDown)]);
    // With the panel disabled the arrows do nothing.
    bench.history.clear();
    bench.editor.sync(Some((EditTarget::Entry(0), WARM)));
    bench.gesture_drawing = true;
    bench.frame(vec![press(Key::ArrowDown, Modifiers::NONE, false)]);
    assert!(!bench.editor.dragging());
    bench.gesture_drawing = false;
    assert!(bench.commits().is_empty());
}

#[test]
#[ignore = "C4: pending the keyboard"]
fn row_10_the_arrows_keep_the_focus_and_the_view_and_tab_goes_through_the_pieces() {
    let mut bench = open_bench(1.0, palette_image(), ColorChoice::Entry(3));
    focus(&mut bench, Piece::Square);
    let (offset, first_row) = (bench.view.offset_px, bench.scroll.first_row);
    for key in [
        Key::ArrowLeft,
        Key::ArrowRight,
        Key::ArrowUp,
        Key::ArrowDown,
    ] {
        bench.frame(vec![press(key, Modifiers::NONE, false)]);
        bench.frame(vec![release(key)]);
        bench.editor.cancel();
        assert!(
            bench.ctx.memory(|m| m.has_focus(piece_id(Piece::Square))),
            "{key:?} does not move the focus of egui"
        );
    }
    assert_eq!(
        (bench.view.offset_px, bench.scroll.first_row),
        (offset, first_row)
    );
    // Tab reaches the three pieces in order and leaves them.
    let mut seen = Vec::new();
    for _ in 0..16 {
        bench.key(Key::Tab);
        bench.rest();
        for piece in [Piece::Square, Piece::HueBar, Piece::AlphaBar] {
            if bench.ctx.memory(|m| m.has_focus(piece_id(piece))) && !seen.contains(&piece) {
                seen.push(piece);
            }
        }
    }
    assert_eq!(seen.len(), 3, "Tab enters every piece: {seen:?}");
    let now_in = [Piece::Square, Piece::HueBar, Piece::AlphaBar]
        .into_iter()
        .any(|p| bench.ctx.memory(|m| m.has_focus(piece_id(p))));
    assert!(!now_in || seen.len() == 3, "and it leaves them");
}

// ---------------------------------------------------------------------------------------------
// Row 11: selector and sliders
// ---------------------------------------------------------------------------------------------

#[test]
#[ignore = "C4: pending the selector"]
fn row_11_the_selector_picks_one_model_and_the_sliders_follow_it() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let mut bench = open_bench(ppp, palette_image(), ColorChoice::Entry(0));
        assert_eq!(bench.view.slider_mode, SliderMode::Rgb, "RGB by default");
        for (index, mode, letters, absent) in [
            (1usize, SliderMode::Hsl, ["H", "S", "L", "A"], "V"),
            (2, SliderMode::Hsv, ["H", "S", "V", "A"], "L"),
            (0, SliderMode::Rgb, ["R", "G", "B", "A"], "H"),
        ] {
            let option = selector_rect(index, n as u32);
            let at = editor_px(
                &bench,
                option.x + option.width / 2,
                option.y + option.height / 2,
            );
            bench.click(at);
            let f = bench.rest();
            assert_eq!(bench.view.slider_mode, mode, "ppp {ppp}");
            for letter in letters {
                assert!(f.paints(letter), "ppp {ppp}, {mode:?}: {letter}");
            }
            assert!(!f.paints(absent), "ppp {ppp}, {mode:?}: no {absent}");
            assert!(
                bench.commits().is_empty(),
                "the selector does not touch the document"
            );
            assert_eq!(SELECTOR_X_U[index], option.x / n);
            // The sliders are at 138, 152, 166 and 184.
            let controls = visible_controls(Mode::Indexed, mode);
            assert_eq!(controls.len(), 4);
            for (row, control) in [138, 152, 166, 184].into_iter().zip(controls) {
                assert_eq!(
                    control_row_u(Mode::Indexed, *control),
                    Some(row),
                    "{control:?}"
                );
                let at = in_editor(&bench, 100, row + 7);
                bench.presses(at);
                assert_eq!(
                    bench.editor.dragging_control(),
                    Some(*control),
                    "ppp {ppp}, {mode:?}"
                );
                bench.editor.cancel();
                bench.releases(at);
            }
        }
    }
}

#[test]
#[ignore = "C4: pending the selector"]
fn row_11_a_slider_of_each_model_moves_the_square_and_the_bars_and_the_gray_modes_have_none() {
    let mut bench = open_bench(1.0, palette_image(), ColorChoice::Entry(0));
    let option = selector_rect(2, 1);
    let at = editor_px(&bench, option.x + 5, option.y + 5);
    bench.click(at);
    bench.rest();
    // The value slider of HSV (row 166): the same commit as the equivalent control of 11c.
    let at = in_editor(&bench, 15 + 92, 166 + 7);
    bench.presses(at);
    assert_eq!(bench.editor.dragging_control(), Some(Control::Value));
    let value = bench.editor.shown_hsv().unwrap().v();
    let f = bench.releases(at);
    let base = (WARM, px_core::Hsl::from_rgba(WARM), Hsv::from_rgba(WARM));
    let expected = apply_control_aux(base, Control::Value, (value * 100.0).round() as u32).0;
    assert_eq!(f.output.commit.map(|c| c.color), Some(expected));
    // The hexadecimal stays exact and the gray modes show neither selector nor square nor bars.
    for mode in [Mode::Gray, Mode::GrayAlpha] {
        let image = Image::new(8, 4, mode).expect("image");
        let mut bench = open_bench(1.0, image, ColorChoice::Slot(3));
        let f = bench.rest();
        for word in ["RGB", "HSL", "HSV"] {
            assert!(!f.paints(word), "{mode:?}: {word}");
        }
        assert!(
            f.meshes.iter().all(|m| m.len() != 14),
            "{mode:?}: no hue bar"
        );
        assert!(f.paints("G"));
        assert_eq!(
            visible_controls(mode, SliderMode::Hsv).len(),
            if mode == Mode::Gray { 1 } else { 2 }
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Row 12: the whole panel
// ---------------------------------------------------------------------------------------------

#[test]
#[ignore = "C4: pending the two bars"]
fn row_12_nothing_is_painted_outside_the_panel_and_the_swap_button_fits_at_the_five_scales() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        let k = i64::from(n);
        for mode in [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
            for open in [false, true] {
                let image = if mode == Mode::Indexed {
                    palette_image()
                } else {
                    Image::new(8, 4, mode).expect("image")
                };
                let choice = if mode == Mode::Indexed {
                    ColorChoice::Entry(3)
                } else {
                    ColorChoice::Slot(3)
                };
                let mut bench = Bench::with_panel_height(ppp, 700 * k, image, choice);
                bench.view.editor_open = open;
                let f = bench.rest();
                let panel = Rect::from_min_size(
                    pos2(0.0, 0.0),
                    vec2((233 * k) as f32 / ppp, bench.panel_h_px() as f32 / ppp),
                )
                .expand(0.05);
                for (rect, _) in &f.fills {
                    assert!(
                        panel.contains_rect(*rect) || !rect.is_positive(),
                        "ppp {ppp}, {mode:?}, open {open}: {rect:?}"
                    );
                }
                // The button is whole inside the column, between the cells and the right edge.
                let swap = swap_rect(PANEL_COLS, n);
                assert!(swap.x + swap.width <= 233 * k - 2 * k);
                assert_eq!(
                    px_ui::color_block::bar_hit(PANEL_COLS, swap.x + 1, swap.y + 1, n),
                    Some(BarHit::Swap)
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Property 1: the models never disagree
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Step {
    Square(f32, f32),
    Bar(Control, u32),
    Slider(Control, u32),
    Cancelled(f32, f32),
    External(u8, u8, u8, u8),
    SameColor,
}

fn step() -> impl Strategy<Value = Step> {
    let any_control = prop::sample::select(vec![
        Control::Red,
        Control::Green,
        Control::Blue,
        Control::Hue,
        Control::Saturation,
        Control::Lightness,
        Control::HueV,
        Control::SaturationV,
        Control::Value,
        Control::Alpha,
    ]);
    prop_oneof![
        (0.0f32..=1.0, 0.0f32..=1.0).prop_map(|(s, v)| Step::Square(s, v)),
        (
            prop::sample::select(vec![Control::HueV, Control::Alpha]),
            0u32..=400
        )
            .prop_map(|(c, v)| Step::Bar(c, v)),
        (any_control, 0u32..=400).prop_map(|(c, v)| Step::Slider(c, v)),
        (0.0f32..=1.0, 0.0f32..=1.0).prop_map(|(s, v)| Step::Cancelled(s, v)),
        (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
            .prop_map(|(r, g, b, a)| Step::External(r, g, b, a)),
        Just(Step::SameColor),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
        fn property_1_the_models_never_disagree_and_the_alpha_only_moves_with_its_control(
        r in any::<u8>(), g in any::<u8>(), b in any::<u8>(), a in any::<u8>(),
        steps in prop::collection::vec(step(), 1..24),
    ) {
        let color = Rgba::new(r, g, b, a);
        prop_assert_eq!(Hsv::from_rgba(color).to_rgba(a), color, "the conversion is exact");
        let mut editor = editor_on(color);
        for step in steps {
            let (before, hsv_before) = shown(&editor);
            let began = match &step {
                Step::Square(s, v) | Step::Cancelled(s, v) => editor.begin_square(*s, *v),
                Step::Bar(c, v) | Step::Slider(c, v) => editor.begin(*c, *v),
                Step::External(r, g, b, a) => {
                    editor.sync(Some((EditTarget::Entry(0), Rgba::new(*r, *g, *b, *a))));
                    false
                }
                Step::SameColor => {
                    editor.sync(Some((EditTarget::Entry(0), before)));
                    false
                }
            };
            if began {
                let (rgba, hsv) = shown(&editor);
                let touches_alpha = matches!(&step, Step::Bar(Control::Alpha, _) | Step::Slider(Control::Alpha, _));
                if !touches_alpha {
                    prop_assert_eq!(rgba.a, before.a, "only an alpha control moves the alpha");
                }
                if matches!(&step, Step::Square(..) | Step::Cancelled(..)) {
                    prop_assert_eq!(hsv.h(), hsv_before.h(), "the square never moves the hue");
                }
                if matches!(&step, Step::Cancelled(..)) {
                    editor.cancel();
                    prop_assert_eq!(shown(&editor), (before, hsv_before));
                } else {
                    finish(&mut editor);
                }
            }
            let _ = shown(&editor);
        }
    }
}
