//! Tests de `color_edit` (specs/hito-2-editar-color.md, unidad 11c): la lógica pura del editor de
//! color, sin tipos de egui y sin documento.
//!
//! Cubre las filas 1, 2 (solo la parte pura: valores, `value_text`, `hex_text`, cabeceras y
//! `hex_notice`), 3 (solo `slider_value_at` y `slider_x`), 5, 6, 7, 8, 9 (solo la parte pura de
//! `ColorEditor`: `type_hex`, `confirm_hex` y `escape_hex` con sus `HexOutcome`), 10 y 12 de
//! «Casos límite», y las propiedades 1 (conversión de controles), 2 (máquina del arrastre) y 3
//! (hexadecimal). Lo que necesita un `Document` (la fila 1 con `commit_edit`, la fila 4) no está
//! aquí.
//!
//! Del texto pendiente del campo (specs/hito-2-panel-ventana-baja.md) cubre, en su parte pura,
//! las filas 7, 8 y 9 de «Casos»: `ColorEditor::has_pending`, `confirm_hex_checked` con sus
//! `Resolved` y `StaleReason`, `stale_notice` y `DOCUMENT_CHANGED_NOTICE`. El flujo con ventana
//! está en `panel_baja.rs` y el de `Session::open` en `px-app`.
//!
//! Imagen `F`: indexada 4×1, paleta `[R, G, B, K]`, píxeles `[0, 1, 2, 3]`.
//!
//! Los números esperados salen de la spec. El oráculo de la propiedad 3 está escrito a mano y no
//! llama a `Rgba::from_hex`. El del arrastre (propiedad 2) es un modelo de las decisiones 6 y 8
//! que usa `apply_control` y `Hsl::from_rgba` (probados aparte) y compara exactamente.

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Hsl, Image, Mode, Palette, Pixel, Rgba};
use px_ui::color::{ColorChoice, SlotColors};
use px_ui::color_edit::{
    ColorEditor, Commit, Control, DOCUMENT_CHANGED_NOTICE, EditTarget, HexError, HexOutcome,
    Resolved, StaleReason, apply_control, control_value, controls, hex_notice, hex_text,
    hsl_after_rgb, parse_hex, slider_value_at, slider_x, stale_notice, value_text,
};
use px_ui::text::INTERFACE_CHARSET;
use px_ui::theme::NOTICE_PREFIX;
use std::cell::Cell;

// Apoyo común.

const RED: Rgba = Rgba::opaque(255, 0, 0);
const GREEN: Rgba = Rgba::opaque(0, 255, 0);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);
const NEGRO: Rgba = Rgba::opaque(0, 0, 0);
const YELLOW: Rgba = Rgba::opaque(255, 255, 0);

const CONTROLS: [Control; 8] = [
    Control::Red,
    Control::Green,
    Control::Blue,
    Control::Hue,
    Control::Saturation,
    Control::Lightness,
    Control::Alpha,
    Control::Gray,
];

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

/// `F`: indexada 4×1, paleta `[R, G, B, K]`, píxeles `[0, 1, 2, 3]`.
fn image_f() -> Image {
    let palette = Palette::from_colors([RED, GREEN, BLUE, NEGRO]).expect("paleta válida");
    let mut image_2 = Image::new_indexed(4, 1, palette).expect("imagen válida");
    for x in 0..4u8 {
        image_2
            .set_pixel(u32::from(x), 0, Pixel::Indexed(x))
            .expect("píxel válido");
    }
    image_2
}

fn image_mode(mode_2: Mode) -> Image {
    match mode_2 {
        Mode::Indexed => image_f(),
        other_2 => Image::new(4, 4, other_2).expect("imagen válida"),
    }
}

/// Editor ya sincronizado con ese destino y color guardado.
fn editor_with(target: EditTarget, color: Rgba) -> ColorEditor {
    let mut editor = ColorEditor::default();
    editor.sync(Some((target, color)));
    editor
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

/// Valor de `H` que muestra el editor.
fn hue_shown(editor: &ColorEditor) -> u32 {
    let (color, hsl) = editor.shown().expect("hay destino");
    control_value(color, hsl, Control::Hue)
}

/// Lo que hace `px-app` tras un arrastre: el color guardado que ve el siguiente `sync` es el
/// que quedó en el editor (o el confirmado).
fn drag(
    editor: &mut ColorEditor,
    target: EditTarget,
    control: Control,
    values: &[u32],
) -> Option<Commit> {
    assert!(editor.begin(control, values[0]), "begin aceptado");
    for valor in &values[1..] {
        editor.drag(*valor);
    }
    let commit = editor.release();
    let saved = match commit {
        Some(commit) => commit.color,
        None => editor.shown().expect("hay destino").0,
    };
    editor.sync(Some((target, saved)));
    commit
}

/// El primer renglón del aviso sin el prefijo `notice: ` (la spec dice que el editor muestra
/// `notice: ` + motivo; este helper vale tanto si `notice` ya lo trae como si lo pone quien pinta).
fn without_prefix(line: &str) -> &str {
    line.strip_prefix(NOTICE_PREFIX).unwrap_or(line)
}

fn empty(line: &str) -> bool {
    line.trim().is_empty()
}

fn in_the_set(text_2: &str) -> bool {
    text_2.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

// Fila 1: destino.

#[test]
fn row_1_target_over_the_indexed_one_f() {
    let f = image_f();
    let de = |choice| EditTarget::of(&f, choice);
    assert_eq!(de(ColorChoice::Entry(1)), Some(EditTarget::Entry(1)));
    // `Slot(n)` vale como la entrada n − 1.
    assert_eq!(de(ColorChoice::Slot(3)), Some(EditTarget::Entry(2)));
    assert_eq!(de(ColorChoice::Entry(4)), None);
    assert_eq!(de(ColorChoice::Slot(5)), None);
    assert_eq!(de(ColorChoice::Slot(0)), None);
    assert_eq!(de(ColorChoice::Slot(10)), None);
    assert_eq!(de(ColorChoice::Entry(255)), None);
    assert_eq!(de(ColorChoice::Entry(3)), Some(EditTarget::Entry(3)));
}

#[test]
fn row_1_target_over_rgba() {
    let image_2 = image_mode(Mode::Rgba);
    let de = |choice| EditTarget::of(&image_2, choice);
    assert_eq!(de(ColorChoice::Slot(3)), Some(EditTarget::Slot(3)));
    assert_eq!(de(ColorChoice::Slot(0)), None);
    assert_eq!(de(ColorChoice::Slot(10)), None);
    assert_eq!(de(ColorChoice::Entry(0)), None);
    for slot in 1..=9 {
        assert_eq!(de(ColorChoice::Slot(slot)), Some(EditTarget::Slot(slot)));
    }
}

#[test]
fn row_1_target_over_gray_and_gray_alpha() {
    let gray_2 = image_mode(Mode::Gray);
    assert_eq!(
        EditTarget::of(&gray_2, ColorChoice::Slot(9)),
        Some(EditTarget::Slot(9))
    );
    assert_eq!(EditTarget::of(&gray_2, ColorChoice::Entry(0)), None);
    assert_eq!(EditTarget::of(&gray_2, ColorChoice::Slot(0)), None);
    let with_alpha = image_mode(Mode::GrayAlpha);
    assert_eq!(
        EditTarget::of(&with_alpha, ColorChoice::Slot(9)),
        Some(EditTarget::Slot(9))
    );
    assert_eq!(EditTarget::of(&with_alpha, ColorChoice::Slot(10)), None);
    assert_eq!(EditTarget::of(&with_alpha, ColorChoice::Entry(1)), None);
}

#[test]
fn the_saved_target_is_the_palette_or_the_slot_color() {
    let f = image_f();
    let slots = SlotColors::defaults(Mode::Rgba);
    assert_eq!(EditTarget::Entry(1).stored(&f, &slots), Some(GREEN));
    assert_eq!(EditTarget::Entry(3).stored(&f, &slots), Some(NEGRO));
    assert_eq!(EditTarget::Entry(4).stored(&f, &slots), None);

    let image_2 = image_mode(Mode::Rgba);
    let mut slots = SlotColors::defaults(Mode::Rgba);
    assert!(slots.set(3, rgba(1, 2, 3, 4)));
    assert_eq!(
        EditTarget::Slot(3).stored(&image_2, &slots),
        Some(rgba(1, 2, 3, 4))
    );
    assert_eq!(EditTarget::Slot(2).stored(&image_2, &slots), slots.get(2));
}

#[test]
fn the_stored_color_is_edited_not_the_resolved_one() {
    // Fila 13 (b), parte pura: la entrada transparente se ve con su color guardado.
    let mut palette = Palette::from_colors([RED, GREEN, BLUE]).expect("paleta válida");
    palette
        .set_transparent_index(Some(2))
        .expect("índice válido");
    let image_2 = Image::new_indexed(3, 1, palette).expect("imagen válida");
    let slots = SlotColors::defaults(Mode::Rgba);
    assert_eq!(EditTarget::Entry(2).stored(&image_2, &slots), Some(BLUE));
}

// Fila 2: valores, textos y cabeceras.

#[test]
fn row_2_values_of_an_opaque_orange() {
    let color = Rgba::opaque(255, 128, 0);
    let hsl = Hsl::from_rgba(color);
    let valor = |c| control_value(color, hsl, c);
    assert_eq!(valor(Control::Red), 255);
    assert_eq!(valor(Control::Green), 128);
    assert_eq!(valor(Control::Blue), 0);
    assert_eq!(valor(Control::Alpha), 255);
    assert_eq!(valor(Control::Hue), 30);
    assert_eq!(valor(Control::Saturation), 100);
    assert_eq!(valor(Control::Lightness), 50);
    assert_eq!(value_text(valor(Control::Blue)), "  0");
    assert_eq!(value_text(valor(Control::Red)), "255");
}

#[test]
fn row_2_values_of_the_transparent_black_and_white() {
    let zero = Rgba::TRANSPARENT;
    let hsl = Hsl::from_rgba(zero);
    for control in [
        Control::Red,
        Control::Green,
        Control::Blue,
        Control::Alpha,
        Control::Hue,
        Control::Saturation,
        Control::Lightness,
    ] {
        assert_eq!(control_value(zero, hsl, control), 0, "{control:?}");
    }
    let white = Rgba::opaque(255, 255, 255);
    let hsl = Hsl::from_rgba(white);
    assert_eq!(control_value(white, hsl, Control::Lightness), 100);
    assert_eq!(control_value(white, hsl, Control::Saturation), 0);
}

#[test]
fn row_2_gray_96_in_gray() {
    let color = Rgba::opaque(96, 96, 96);
    assert_eq!(
        control_value(color, Hsl::from_rgba(color), Control::Gray),
        96
    );
    assert_eq!(hex_text(color), "#606060");
}

#[test]
fn row_2_orange_with_alpha_128() {
    let color = rgba(255, 128, 0, 128);
    let hsl = Hsl::from_rgba(color);
    assert_eq!(control_value(color, hsl, Control::Alpha), 128);
    assert_eq!(hex_text(color), "#ff800080");
    assert_eq!(hex_text(Rgba::opaque(255, 128, 0)), "#ff8000");
    assert_eq!(hex_text(Rgba::TRANSPARENT), "#00000000");
}

#[test]
fn row_2_the_hue_is_rounded_and_wrapped_and_s_and_l_are_rounded() {
    // `round(h) mod 360`: 359,6 redondea a 360 y vuelve a 0.
    let color = Rgba::opaque(0, 0, 0);
    assert_eq!(
        control_value(color, Hsl::new(359.6, 1.0, 0.5), Control::Hue),
        0
    );
    assert_eq!(
        control_value(color, Hsl::new(359.4, 1.0, 0.5), Control::Hue),
        359
    );
    assert_eq!(
        control_value(color, Hsl::new(30.4, 1.0, 0.5), Control::Hue),
        30
    );
    assert_eq!(
        control_value(color, Hsl::new(30.6, 1.0, 0.5), Control::Hue),
        31
    );
    // `round(100·x)`.
    assert_eq!(
        control_value(color, Hsl::new(0.0, 0.504, 0.496), Control::Saturation),
        50
    );
    assert_eq!(
        control_value(color, Hsl::new(0.0, 0.504, 0.496), Control::Lightness),
        50
    );
    assert_eq!(
        control_value(color, Hsl::new(0.0, 0.506, 0.494), Control::Saturation),
        51
    );
    assert_eq!(
        control_value(color, Hsl::new(0.0, 0.506, 0.494), Control::Lightness),
        49
    );
}

#[test]
fn value_text_takes_three_columns_on_the_right() {
    assert_eq!(value_text(0), "  0");
    assert_eq!(value_text(7), "  7");
    assert_eq!(value_text(30), " 30");
    assert_eq!(value_text(100), "100");
    assert_eq!(value_text(255), "255");
    assert_eq!(value_text(359), "359");
    for valor in 0..=359u32 {
        let text_2 = value_text(valor);
        assert_eq!(text_2.chars().count(), 3, "{valor}");
        assert_eq!(text_2.trim().parse::<u32>().ok(), Some(valor));
        assert!(text_2.starts_with(' ') || valor >= 100);
    }
}

#[test]
fn row_2_headers() {
    assert_eq!(
        EditTarget::Entry(12).header(Mode::Indexed),
        "editar índice 12"
    );
    assert_eq!(
        EditTarget::Entry(0).header(Mode::Indexed),
        "editar índice 0"
    );
    assert_eq!(EditTarget::Slot(3).header(Mode::Rgba), "editar color 3");
    assert_eq!(
        EditTarget::Slot(4).header(Mode::Gray),
        "editar color 4 gris"
    );
}

#[test]
fn row_2_no_header_exceeds_32_columns_or_leaves_the_character_set() {
    let mut headers = Vec::new();
    for input_2 in 0..=255u8 {
        headers.push(EditTarget::Entry(input_2).header(Mode::Indexed));
    }
    for mode_2 in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        for slot in 1..=9u8 {
            headers.push(EditTarget::Slot(slot).header(mode_2));
        }
    }
    for header in headers {
        assert!(header.chars().count() <= 32, "{header:?}");
        assert!(in_the_set(&header), "{header:?}");
    }
    assert_eq!(
        EditTarget::Slot(4).header(Mode::Gray).chars().count(),
        19,
        "`editar color 4 gris` son 19"
    );
}

#[test]
fn row_2_the_notices_fit_in_32_columns_and_in_the_character_set() {
    // `hex_notice` va tras `notice: `: 7 + 25 = 32.
    for error in [
        HexError::Empty,
        HexError::Length(5),
        HexError::Digit,
        HexError::NotGray,
        HexError::NoAlpha,
    ] {
        let reason_2 = hex_notice(&error);
        let line = format!("{NOTICE_PREFIX}{reason_2}");
        assert!(line.chars().count() <= 32, "{line:?}");
        assert!(in_the_set(&line), "{line:?}");
    }
    let editor = ColorEditor::default();
    for line in editor.notice(true) {
        assert!(line.chars().count() <= 32, "{line:?}");
        assert!(in_the_set(&line), "{line:?}");
    }
    assert_eq!(
        format!("{NOTICE_PREFIX}entrada transparente")
            .chars()
            .count(),
        27
    );
}

#[test]
fn the_hex_notice_texts_are_those_of_the_spec() {
    assert_eq!(hex_notice(&HexError::Empty), "escribe un color");
    assert_eq!(hex_notice(&HexError::Length(5)), "usa 3, 4, 6 u 8 dígitos");
    assert_eq!(hex_notice(&HexError::Length(9)), "usa 3, 4, 6 u 8 dígitos");
    assert_eq!(hex_notice(&HexError::Digit), "solo dígitos 0-9 y a-f");
    assert_eq!(hex_notice(&HexError::NotGray), "en modo gris: r = g = b");
    assert_eq!(hex_notice(&HexError::NoAlpha), "este modo no tiene alfa");
}

#[test]
fn the_controls_of_each_mode_in_order_from_top_to_bottom() {
    let complete = [
        Control::Red,
        Control::Green,
        Control::Blue,
        Control::Hue,
        Control::Saturation,
        Control::Lightness,
        Control::Alpha,
    ];
    assert_eq!(controls(Mode::Rgba), complete);
    assert_eq!(controls(Mode::Indexed), complete);
    assert_eq!(controls(Mode::GrayAlpha), [Control::Gray, Control::Alpha]);
    assert_eq!(controls(Mode::Gray), [Control::Gray]);
}

#[test]
fn letters_and_maximums_of_the_controls() {
    let expected_2 = [
        (Control::Red, 'R', 255),
        (Control::Green, 'G', 255),
        (Control::Blue, 'B', 255),
        (Control::Hue, 'H', 359),
        (Control::Saturation, 'S', 100),
        (Control::Lightness, 'L', 100),
        (Control::Alpha, 'A', 255),
        (Control::Gray, 'G', 255),
    ];
    for (control, letter, maximum_2) in expected_2 {
        assert_eq!(control.label(), letter, "{control:?}");
        assert_eq!(control.max(), maximum_2, "{control:?}");
    }
}

// Fila 3: deslizadores (parte pura).

#[test]
fn row_3_slider_value_at_with_maximum_255() {
    for n in [1i64, 2, 3] {
        let width_2 = 184 * n;
        assert_eq!(slider_value_at(-5, width_2, 255), 0, "n = {n}");
        assert_eq!(slider_value_at(0, width_2, 255), 0, "n = {n}");
        assert_eq!(slider_value_at(width_2 - 1, width_2, 255), 255, "n = {n}");
        assert_eq!(slider_value_at(width_2 + 9, width_2, 255), 255, "n = {n}");
    }
    // `92·255/183` redondeado.
    assert_eq!(slider_value_at(92, 184, 255), 128);
    for n in [2i64, 3] {
        let middle = slider_value_at(92 * n, 184 * n, 255);
        assert!((127..=128).contains(&middle), "n = {n}: {middle}");
    }
}

#[test]
fn row_3_slider_value_at_with_maximum_359_and_100_in_the_middle() {
    for n in [1i64, 2, 3] {
        assert_eq!(slider_value_at(92 * n, 184 * n, 359), 180, "n = {n}");
        assert_eq!(slider_value_at(92 * n, 184 * n, 100), 50, "n = {n}");
    }
}

#[test]
fn row_3_slider_x_at_the_extremes() {
    for n in [1i64, 2, 3] {
        for maximum_2 in [100u32, 255, 359] {
            assert_eq!(
                slider_x(0, 184 * n, maximum_2),
                0,
                "n = {n}, máx. {maximum_2}"
            );
            assert_eq!(
                slider_x(maximum_2, 184 * n, maximum_2),
                184 * n - 1,
                "n = {n}, máx. {maximum_2}"
            );
        }
    }
}

#[test]
fn row_3_slider_x_and_slider_value_at_are_monotonic() {
    for n in [1i64, 2, 3] {
        let width_2 = 184 * n;
        for maximum_2 in [100u32, 255, 359] {
            let mut anterior = slider_x(0, width_2, maximum_2);
            for valor in 1..=maximum_2 {
                let actual = slider_x(valor, width_2, maximum_2);
                assert!(actual >= anterior, "slider_x({valor}), n = {n}");
                assert!((0..width_2).contains(&actual));
                anterior = actual;
            }
            let mut anterior = slider_value_at(0, width_2, maximum_2);
            for x in 1..width_2 {
                let actual = slider_value_at(x, width_2, maximum_2);
                assert!(actual >= anterior, "slider_value_at({x}), n = {n}");
                assert!(actual <= maximum_2);
                anterior = actual;
            }
        }
    }
}

#[test]
fn row_3_round_trip_with_maximum_100() {
    for n in [1i64, 2, 3] {
        let width_2 = 184 * n;
        for valor in 0..=100u32 {
            let x = slider_x(valor, width_2, 100);
            assert_eq!(slider_value_at(x, width_2, 100), valor, "n = {n}");
        }
    }
}

// Fila 5: sin paso.

#[test]
fn row_5_pressing_and_releasing_on_the_current_value_gives_no_step() {
    let mut editor = editor_with(EditTarget::Entry(0), RED);
    assert!(editor.begin(Control::Red, 255));
    assert_eq!(editor.release(), None);
}

#[test]
fn row_5_going_back_to_the_initial_value_gives_no_step() {
    let mut editor = editor_with(EditTarget::Entry(0), RED);
    assert!(editor.begin(Control::Red, 0));
    editor.drag(200);
    editor.drag(255);
    assert_eq!(editor.release(), None);
}

#[test]
fn row_5_cancel_leaves_the_editor_as_at_the_start_and_what_follows_does_nothing() {
    let mut editor = editor_with(EditTarget::Entry(0), RED);
    let start_2 = editor.shown().expect("hay destino");
    assert!(editor.begin(Control::Green, 90));
    assert_ne!(editor.shown(), Some(start_2), "el provisional se ve");
    editor.drag(10);
    editor.cancel();
    assert_eq!(editor.shown(), Some(start_2), "Rgba y Hsl exactos");
    assert_eq!(editor.preview(), None);
    editor.drag(200);
    assert_eq!(editor.shown(), Some(start_2), "no se reanuda");
    assert_eq!(editor.preview(), None);
    assert_eq!(editor.release(), None);
    assert_eq!(editor.shown(), Some(start_2));
    assert_eq!(editor.preview(), None);
    assert!(!editor.dragging());
}

#[test]
fn row_5_losing_the_window_focus_is_cancel() {
    // La app llama a `cancel` al perder el foco la ventana: igual que arriba, sin paso.
    let mut editor = editor_with(EditTarget::Entry(0), RED);
    let start_2 = editor.shown();
    assert!(editor.begin(Control::Blue, 255));
    editor.drag(128);
    editor.cancel();
    assert_eq!(editor.release(), None);
    assert_eq!(editor.shown(), start_2);
}

#[test]
fn row_5_begin_without_a_target_gives_false() {
    let mut editor = ColorEditor::default();
    assert!(!editor.begin(Control::Red, 10));
    assert!(!editor.dragging());
    assert_eq!(editor.shown(), None);
    assert_eq!(editor.preview(), None);
    editor.sync(None);
    assert!(!editor.begin(Control::Red, 10));
    assert_eq!(editor.release(), None);
}

#[test]
fn row_5_begin_with_another_drag_in_progress_gives_false_and_does_not_alter_it() {
    let mut editor = editor_with(EditTarget::Entry(0), RED);
    assert!(editor.begin(Control::Green, 255));
    assert!(!editor.begin(Control::Blue, 255));
    assert!(editor.dragging());
    // Sigue el primer arrastre.
    editor.drag(100);
    assert_eq!(
        editor.release(),
        Some(Commit {
            target: EditTarget::Entry(0),
            color: Rgba::opaque(255, 100, 0)
        })
    );
}

#[test]
fn row_5_loose_drag_and_release_do_nothing() {
    let mut editor = editor_with(EditTarget::Entry(0), RED);
    let start_2 = editor.shown();
    editor.drag(77);
    assert_eq!(editor.shown(), start_2);
    assert_eq!(editor.preview(), None);
    assert_eq!(editor.release(), None);
    assert_eq!(editor.shown(), start_2);
    assert!(!editor.dragging());
}

#[test]
fn row_5_the_hue_on_a_gray_changes_the_hsl_but_gives_no_step() {
    let gray_2 = Rgba::opaque(128, 128, 128);
    let mut editor = editor_with(EditTarget::Entry(0), gray_2);
    assert!(editor.begin(Control::Hue, 100));
    assert_eq!(editor.release(), None, "el Rgba no cambió");
    let (color, hsl) = editor.shown().expect("hay destino");
    assert_eq!(color, gray_2);
    assert!(near(hsl.h(), 100.0), "h = {}", hsl.h());
    assert_eq!(hue_shown(&editor), 100);
    // Un `sync` con el mismo color no recalcula el `Hsl`.
    editor.sync(Some((EditTarget::Entry(0), gray_2)));
    assert_eq!(hue_shown(&editor), 100);
}

// Fila 6: `Hsl` propio.

fn blue_light() -> Rgba {
    Rgba::opaque(0, 128, 255)
}

#[test]
fn row_6_saturation_to_zero_and_back_recovers_the_color() {
    // La spec dice H 208 para este color, pero su matiz es 210 (209,88): se compara con
    // `Hsl::from_rgba`, que es lo que quiere decir.
    let base = blue_light();
    let hue = control_value(base, Hsl::from_rgba(base), Control::Hue);
    let target_2 = EditTarget::Entry(0);
    let mut editor = editor_with(target_2, base);

    let commit = drag(&mut editor, target_2, Control::Saturation, &[0]);
    assert_eq!(
        commit,
        Some(Commit {
            target: target_2,
            color: Rgba::opaque(128, 128, 128)
        })
    );
    assert_eq!(hue_shown(&editor), hue, "el matiz se conserva");
    let (_, hsl) = editor.shown().expect("hay destino");
    assert_eq!(
        control_value(Rgba::opaque(128, 128, 128), hsl, Control::Saturation),
        0
    );

    let commit = drag(&mut editor, target_2, Control::Saturation, &[100]);
    assert_eq!(
        commit,
        Some(Commit {
            target: target_2,
            color: base
        })
    );
}

#[test]
fn row_6_lightness_at_the_extremes_and_back_to_50() {
    let base = blue_light();
    let target_2 = EditTarget::Entry(0);
    let mut editor = editor_with(target_2, base);

    let negro = drag(&mut editor, target_2, Control::Lightness, &[0]);
    assert_eq!(negro.map(|c| c.color), Some(Rgba::opaque(0, 0, 0)));
    let of_return = drag(&mut editor, target_2, Control::Lightness, &[50]);
    assert_eq!(of_return.map(|c| c.color), Some(base));

    let white = drag(&mut editor, target_2, Control::Lightness, &[100]);
    assert_eq!(white.map(|c| c.color), Some(Rgba::opaque(255, 255, 255)));
    let of_return = drag(&mut editor, target_2, Control::Lightness, &[50]);
    assert_eq!(of_return.map(|c| c.color), Some(base));
}

#[test]
fn row_6_r_and_b_at_128_leave_a_gray_with_the_hue_of_the_intermediate_color() {
    let base = blue_light();
    let target_2 = EditTarget::Entry(0);
    let mut editor = editor_with(target_2, base);

    let first = drag(&mut editor, target_2, Control::Red, &[128]);
    assert_eq!(first.map(|c| c.color), Some(Rgba::opaque(128, 128, 255)));
    let second = drag(&mut editor, target_2, Control::Blue, &[128]);
    assert_eq!(second.map(|c| c.color), Some(Rgba::opaque(128, 128, 128)));

    let (color, hsl) = editor.shown().expect("hay destino");
    assert_eq!(color, Rgba::opaque(128, 128, 128));
    // El matiz es el de (128, 128, 255), 240, no el del color original.
    assert!(near(hsl.h(), 240.0), "h = {}", hsl.h());
    assert_eq!(control_value(color, hsl, Control::Saturation), 0);
}

#[test]
fn row_6_another_target_and_going_back_recomputes_with_from_rgba() {
    let gray_2 = Rgba::opaque(128, 128, 128);
    let mut editor = editor_with(EditTarget::Entry(0), blue_light());
    drag(&mut editor, EditTarget::Entry(0), Control::Saturation, &[0]);
    assert_ne!(hue_shown(&editor), 0);
    editor.sync(Some((EditTarget::Entry(1), GREEN)));
    assert_eq!(editor.shown(), Some((GREEN, Hsl::from_rgba(GREEN))));
    editor.sync(Some((EditTarget::Entry(0), gray_2)));
    // Un gris da H 0: pérdida esperada (decisión 8).
    assert_eq!(editor.shown(), Some((gray_2, Hsl::from_rgba(gray_2))));
    assert_eq!(hue_shown(&editor), 0);
}

#[test]
fn row_6_undoing_the_color_recomputes_with_from_rgba_of_the_restored_one() {
    let base = blue_light();
    let target_2 = EditTarget::Entry(0);
    let mut editor = editor_with(target_2, base);
    drag(&mut editor, target_2, Control::Red, &[128]);
    assert!(near(editor.shown().unwrap().1.h(), 240.0));
    // `undo`: el siguiente `sync` ve el color original.
    editor.sync(Some((target_2, base)));
    assert_eq!(editor.shown(), Some((base, Hsl::from_rgba(base))));
}

#[test]
fn row_6_a_sync_with_the_same_color_does_not_recompute_the_hsl() {
    // Un trazo del lápiz y su `undo` no cambian el color de la entrada.
    let base = blue_light();
    let target_2 = EditTarget::Entry(0);
    let mut editor = editor_with(target_2, base);
    let commit = drag(&mut editor, target_2, Control::Saturation, &[0]);
    let gray_2 = commit.expect("cambió").color;
    let antes = editor.shown();
    editor.sync(Some((target_2, gray_2)));
    editor.sync(Some((target_2, gray_2)));
    assert_eq!(editor.shown(), antes);
    assert_ne!(hue_shown(&editor), 0, "no se recalculó (daría 0)");
}

#[test]
fn reset_forces_recomputing_the_hsl_in_the_next_sync() {
    let gray_2 = Rgba::opaque(128, 128, 128);
    let mut editor = editor_with(EditTarget::Entry(0), gray_2);
    editor.begin(Control::Hue, 100);
    editor.release();
    assert_eq!(hue_shown(&editor), 100);
    editor.reset();
    assert_eq!(editor.shown(), None, "sin destino tras reset");
    assert!(!editor.dragging());
    editor.sync(Some((EditTarget::Entry(0), gray_2)));
    assert_eq!(editor.shown(), Some((gray_2, Hsl::from_rgba(gray_2))));
}

#[test]
fn sync_during_a_drag_changes_nothing() {
    let mut editor = editor_with(EditTarget::Entry(0), RED);
    assert!(editor.begin(Control::Green, 200));
    let during = editor.shown();
    editor.sync(Some((EditTarget::Entry(3), NEGRO)));
    editor.sync(None);
    assert_eq!(editor.shown(), during);
    assert!(editor.dragging());
    assert_eq!(
        editor.release(),
        Some(Commit {
            target: EditTarget::Entry(0),
            color: Rgba::opaque(255, 200, 0)
        })
    );
}

#[test]
fn the_provisional_goes_from_one_value_to_another_without_depending_on_the_path() {
    // Fila 4 (parte del editor): `begin(Red, 255)`, `drag(128)`, `drag(255)` desde la entrada 1
    // (verde) da (255,255,0), (128,255,0) y otra vez amarillo.
    let mut editor = editor_with(EditTarget::Entry(1), GREEN);
    let provisional = |editor: &ColorEditor| editor.preview();
    assert!(editor.begin(Control::Red, 255));
    assert_eq!(provisional(&editor), Some((EditTarget::Entry(1), YELLOW)));
    assert_eq!(editor.shown().map(|(c, _)| c), Some(YELLOW));
    editor.drag(128);
    assert_eq!(
        provisional(&editor),
        Some((EditTarget::Entry(1), Rgba::opaque(128, 255, 0)))
    );
    editor.drag(255);
    assert_eq!(provisional(&editor), Some((EditTarget::Entry(1), YELLOW)));
    assert_eq!(
        editor.release(),
        Some(Commit {
            target: EditTarget::Entry(1),
            color: YELLOW
        })
    );
    assert_eq!(editor.preview(), None, "ya no hay arrastre vivo");
    assert!(!editor.dragging());
}

// Fila 7: hexadecimal de 3 o 6 dígitos, conserva el alfa.

#[test]
fn row_7_three_and_six_digits_keep_the_current_alpha() {
    let actual = rgba(1, 2, 3, 128);
    for text_2 in ["#f80", "f80", "#ff8800", "ff8800", "  #FF8800  ", "#fF8800"] {
        assert_eq!(
            parse_hex(text_2, Mode::Rgba, actual),
            Ok(rgba(255, 136, 0, 128)),
            "{text_2:?}"
        );
    }
}

#[test]
fn row_7_with_current_alpha_255() {
    let actual = rgba(1, 2, 3, 255);
    assert_eq!(
        parse_hex("#f80", Mode::Rgba, actual),
        Ok(rgba(255, 136, 0, 255))
    );
    assert_eq!(
        parse_hex("ff8800", Mode::Indexed, actual),
        Ok(rgba(255, 136, 0, 255))
    );
}

#[test]
fn row_7_gray_and_gray_alpha() {
    let gray_2 = rgba(50, 50, 50, 255);
    assert_eq!(
        parse_hex("#808080", Mode::Gray, gray_2),
        Ok(rgba(128, 128, 128, 255))
    );
    let with_alpha = rgba(50, 50, 50, 64);
    assert_eq!(
        parse_hex("#808080", Mode::GrayAlpha, with_alpha),
        Ok(rgba(128, 128, 128, 64))
    );
}

#[test]
fn row_7_typing_00ff00_over_an_entry_with_alpha_128_keeps_it() {
    let base = rgba(255, 0, 0, 128);
    let mut editor = editor_with(EditTarget::Entry(0), base);
    editor.type_hex(EditTarget::Entry(0), base, "#00ff00".to_string());
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Commit(Commit {
            target: EditTarget::Entry(0),
            color: rgba(0, 255, 0, 128)
        })
    );
}

// Fila 8: hexadecimal de 4 u 8 dígitos, fija el alfa escrito.

#[test]
fn row_8_four_and_eight_digits_set_the_written_alpha() {
    let actual = rgba(1, 2, 3, 128);
    let cases = [
        ("#f808", rgba(255, 136, 0, 136)),
        ("f80f", rgba(255, 136, 0, 255)),
        ("#ff880080", rgba(255, 136, 0, 128)),
        ("ff8800ff", rgba(255, 136, 0, 255)),
        ("#ff880000", rgba(255, 136, 0, 0)),
    ];
    for (text_2, expected_2) in cases {
        assert_eq!(
            parse_hex(text_2, Mode::Rgba, actual),
            Ok(expected_2),
            "{text_2:?}"
        );
    }
}

#[test]
fn row_8_gray_alpha_and_gray() {
    let with_alpha = rgba(50, 50, 50, 64);
    assert_eq!(
        parse_hex("#80808010", Mode::GrayAlpha, with_alpha),
        Ok(rgba(128, 128, 128, 16))
    );
    let gray_2 = rgba(50, 50, 50, 255);
    assert_eq!(
        parse_hex("#808080ff", Mode::Gray, gray_2),
        Ok(rgba(128, 128, 128, 255))
    );
    assert_eq!(
        parse_hex("#80808080", Mode::Gray, gray_2),
        Err(HexError::NoAlpha)
    );
}

#[test]
fn row_8_typing_00ff00ff_over_an_entry_with_alpha_128_makes_it_255() {
    let base = rgba(255, 0, 0, 128);
    let mut editor = editor_with(EditTarget::Entry(0), base);
    editor.type_hex(EditTarget::Entry(0), base, "#00ff00ff".to_string());
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Commit(Commit {
            target: EditTarget::Entry(0),
            color: rgba(0, 255, 0, 255)
        })
    );
}

// Fila 9: confirmar.

fn kind(editor: &mut ColorEditor, target_2: EditTarget, base: Rgba, text_2: &str) {
    editor.type_hex(target_2, base, text_2.to_string());
}

#[test]
fn row_9_a_valid_value_with_enter_gives_a_commit_and_clears_the_pending() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "#00ff00");
    assert_eq!(editor.hex_display().as_deref(), Some("#00ff00"));
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Commit(Commit {
            target: target_2,
            color: GREEN
        })
    );
    assert_eq!(editor.hex_display(), None);
    // Ya no hay nada pendiente.
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Nothing
    );
}

#[test]
fn row_9_the_current_color_gives_unchanged() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "#0000ff");
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Unchanged
    );
    assert_eq!(editor.hex_display(), None);
}

#[test]
fn row_9_unchanged_also_with_another_way_of_writing_the_same_color() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "  00F  ");
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, true),
        HexOutcome::Unchanged
    );
}

#[test]
fn row_9_escape_clears_what_is_pending_without_a_notice_or_step() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "#00ff00");
    editor.escape_hex();
    assert_eq!(editor.hex_display(), None, "el campo vuelve a `#0000ff`");
    assert_eq!(editor.confirm_hex(Mode::Indexed, true), HexOutcome::Nothing);
    assert!(editor.notice(false).iter().all(|l| empty(l)));
    assert_eq!(editor.shown().map(|(c, _)| c), Some(BLUE));
}

#[test]
fn row_9_changing_swatch_in_the_same_frame_applies_to_the_original_target() {
    let original = EditTarget::Entry(2);
    let mut editor = editor_with(original, BLUE);
    kind(&mut editor, original, BLUE, "#00ff00");
    // La muestra 3 (entrada 2 → entrada 3) se elige antes de perder el foco.
    editor.sync(Some((EditTarget::Entry(3), NEGRO)));
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, true),
        HexOutcome::Commit(Commit {
            target: original,
            color: GREEN
        })
    );
}

#[test]
fn row_9_the_pending_alpha_is_that_of_the_original_base_color() {
    let original = EditTarget::Entry(2);
    let base = rgba(0, 0, 255, 128);
    let mut editor = editor_with(original, base);
    kind(&mut editor, original, base, "#00ff00");
    editor.sync(Some((EditTarget::Entry(3), NEGRO)));
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, true),
        HexOutcome::Commit(Commit {
            target: original,
            color: rgba(0, 255, 0, 128)
        })
    );
}

#[test]
fn row_9_focusing_and_leaving_without_typing_gives_nothing() {
    let mut editor = editor_with(EditTarget::Entry(2), BLUE);
    assert_eq!(editor.confirm_hex(Mode::Indexed, true), HexOutcome::Nothing);
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Nothing
    );
    assert_eq!(editor.hex_display(), None);
}

#[test]
fn confirming_applies_the_rules_of_the_mode() {
    let target_2 = EditTarget::Slot(4);
    let gray_2 = Rgba::opaque(96, 96, 96);
    let mut editor = editor_with(target_2, gray_2);
    kind(&mut editor, target_2, gray_2, "#ff0000");
    assert_eq!(
        editor.confirm_hex(Mode::Gray, false),
        HexOutcome::Invalid(HexError::NotGray)
    );
    kind(&mut editor, target_2, gray_2, "#808080");
    assert_eq!(
        editor.confirm_hex(Mode::Gray, false),
        HexOutcome::Commit(Commit {
            target: target_2,
            color: Rgba::opaque(128, 128, 128)
        })
    );
}

// Fila 11 (parte pura): inválido con Intro, con foco perdido y con Escape.

#[test]
fn row_11_enter_with_an_invalid_value_keeps_the_text_and_the_notice() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "zzz");
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Invalid(HexError::Digit)
    );
    assert_eq!(editor.hex_display().as_deref(), Some("zzz"));
    let notice = editor.notice(false);
    assert_eq!(without_prefix(&notice[0]), "solo dígitos 0-9 y a-f");
    assert!(
        empty(&notice[1]),
        "el aviso del hexadecimal es de una línea"
    );
    // El color no cambió.
    assert_eq!(editor.shown().map(|(c, _)| c), Some(BLUE));
}

#[test]
fn row_11_typing_again_removes_the_notice() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "zzz");
    editor.confirm_hex(Mode::Indexed, false);
    kind(&mut editor, target_2, BLUE, "zzzz");
    assert!(editor.notice(false).iter().all(|l| empty(l)));
    assert_eq!(editor.hex_display().as_deref(), Some("zzzz"));
    // Intro otra vez: el aviso vuelve.
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Invalid(HexError::Digit)
    );
    assert!(!empty(&editor.notice(false)[0]));
}

#[test]
fn row_11_losing_focus_with_an_invalid_value_reverts_the_text() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "zzz");
    editor.confirm_hex(Mode::Indexed, false);
    // Un clic fuera: el motivo sale en `Invalid` (la app lo lleva a la barra de estado).
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, true),
        HexOutcome::Invalid(HexError::Digit)
    );
    assert_eq!(
        editor.hex_display(),
        None,
        "el texto vuelve al color actual"
    );
    assert!(editor.notice(false).iter().all(|l| empty(l)));
    assert_eq!(editor.confirm_hex(Mode::Indexed, true), HexOutcome::Nothing);
}

#[test]
fn row_11_losing_focus_without_having_pressed_enter_also_reverts() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "zzz");
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, true),
        HexOutcome::Invalid(HexError::Digit)
    );
    assert_eq!(editor.hex_display(), None);
}

#[test]
fn row_11_escape_reverts_the_text_without_a_notice() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "zzz");
    editor.confirm_hex(Mode::Indexed, false);
    editor.escape_hex();
    assert_eq!(editor.hex_display(), None);
    assert!(editor.notice(false).iter().all(|l| empty(l)));
    assert_eq!(editor.confirm_hex(Mode::Indexed, true), HexOutcome::Nothing);
}

#[test]
fn the_invalid_hexadecimal_notice_lasts_until_the_target_or_the_color_changes() {
    let target_2 = EditTarget::Entry(2);
    for new_2 in [
        Some((EditTarget::Entry(3), NEGRO)),
        Some((target_2, GREEN)),
        None,
    ] {
        let mut editor = editor_with(target_2, BLUE);
        kind(&mut editor, target_2, BLUE, "zzz");
        editor.confirm_hex(Mode::Indexed, false);
        // Mismo destino y color: sigue.
        editor.sync(Some((target_2, BLUE)));
        assert!(!empty(&editor.notice(false)[0]), "sin cambios sigue");
        assert_eq!(editor.hex_display().as_deref(), Some("zzz"));
        editor.sync(new_2);
        assert!(
            editor.notice(false).iter().all(|l| empty(l)),
            "cambió el destino o el color: {new_2:?}"
        );
        assert_eq!(editor.hex_display(), None, "{new_2:?}");
    }
}

#[test]
fn confirming_a_valid_one_after_an_invalid_one_removes_the_notice() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "zzz");
    editor.confirm_hex(Mode::Indexed, false);
    kind(&mut editor, target_2, BLUE, "#00ff00");
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Commit(Commit {
            target: target_2,
            color: GREEN
        })
    );
    assert!(editor.notice(false).iter().all(|l| empty(l)));
}

#[test]
fn with_nothing_to_warn_about_the_notice_is_empty() {
    let editor = editor_with(EditTarget::Entry(2), BLUE);
    assert!(editor.notice(false).iter().all(|l| empty(l)));
}

// Fila 13 (b), parte pura: el aviso de la entrada transparente.

#[test]
fn row_13_the_notice_of_the_transparent_entry_has_two_lines() {
    let editor = editor_with(EditTarget::Entry(2), BLUE);
    let notice = editor.notice(true);
    assert_eq!(without_prefix(&notice[0]), "entrada transparente");
    assert_eq!(notice[1].trim(), "se guarda con alfa 0");
}

#[test]
fn the_invalid_hex_notice_covers_the_transparent_entry_one() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "zzz");
    editor.confirm_hex(Mode::Indexed, false);
    let notice = editor.notice(true);
    assert_eq!(without_prefix(&notice[0]), "solo dígitos 0-9 y a-f");
    assert!(empty(&notice[1]));
}

#[test]
fn the_five_hexadecimal_notices_fit_in_32_columns() {
    let cases = [
        ("", Mode::Rgba, HexError::Empty),
        ("12345", Mode::Rgba, HexError::Length(5)),
        ("zzz", Mode::Rgba, HexError::Digit),
        ("#ff0000", Mode::Gray, HexError::NotGray),
        ("#80808080", Mode::Gray, HexError::NoAlpha),
    ];
    for (text_2, mode_2, error) in cases {
        let target_2 = EditTarget::Slot(4);
        let base = Rgba::opaque(96, 96, 96);
        let mut editor = editor_with(target_2, base);
        kind(&mut editor, target_2, base, text_2);
        assert_eq!(
            editor.confirm_hex(mode_2, false),
            HexOutcome::Invalid(error.clone())
        );
        for line in editor.notice(false) {
            assert!(line.chars().count() <= 32, "{line:?}");
            assert!(in_the_set(&line), "{line:?}");
        }
        assert_eq!(without_prefix(&editor.notice(false)[0]), hex_notice(&error));
    }
}

// Fila 10: hexadecimal inválido (análisis).

#[test]
fn row_10_empty_is_empty() {
    let actual = Rgba::opaque(1, 2, 3);
    for text_2 in ["", "   ", "#", "  #  "] {
        assert_eq!(
            parse_hex(text_2, Mode::Rgba, actual),
            Err(HexError::Empty),
            "{text_2:?}"
        );
    }
}

#[test]
fn row_10_length_before_digits() {
    let actual = Rgba::opaque(1, 2, 3);
    assert_eq!(
        parse_hex("#12345", Mode::Rgba, actual),
        Err(HexError::Length(5))
    );
    assert_eq!(
        parse_hex("#ff0000ff0", Mode::Rgba, actual),
        Err(HexError::Length(9))
    );
    // Longitud mala y dígito malo a la vez: gana la longitud.
    assert_eq!(
        parse_hex("#zz", Mode::Rgba, actual),
        Err(HexError::Length(2))
    );
}

#[test]
fn row_10_invalid_digits() {
    let actual = Rgba::opaque(1, 2, 3);
    assert_eq!(
        parse_hex("#12g45f", Mode::Rgba, actual),
        Err(HexError::Digit)
    );
    // Tres caracteres (no tres bytes).
    assert_eq!(parse_hex("ñññ", Mode::Rgba, actual), Err(HexError::Digit));
    // Ocho caracteres con espacios dentro.
    assert_eq!(
        parse_hex("ff 00 00", Mode::Rgba, actual),
        Err(HexError::Digit)
    );
}

#[test]
fn row_10_rules_of_gray_and_gray_alpha() {
    let gray_2 = rgba(50, 50, 50, 255);
    assert_eq!(
        parse_hex("#ff0000", Mode::Gray, gray_2),
        Err(HexError::NotGray)
    );
    assert_eq!(
        parse_hex("#80808080", Mode::Gray, gray_2),
        Err(HexError::NoAlpha)
    );
    assert_eq!(
        parse_hex("#ff0000", Mode::GrayAlpha, rgba(50, 50, 50, 64)),
        Err(HexError::NotGray)
    );
    // `NoAlpha` va antes que `NotGray`.
    assert_eq!(
        parse_hex("#ff000080", Mode::Gray, gray_2),
        Err(HexError::NoAlpha)
    );
}

#[test]
fn row_10_in_indexed_any_color_is_valid() {
    assert_eq!(
        parse_hex("#ff0000", Mode::Indexed, Rgba::opaque(1, 2, 3)),
        Ok(Rgba::opaque(255, 0, 0))
    );
}

#[test]
fn row_10_the_notices_fit_in_25_characters_and_in_the_character_set() {
    for error in [
        HexError::Empty,
        HexError::Length(0),
        HexError::Length(5),
        HexError::Length(1000),
        HexError::Digit,
        HexError::NotGray,
        HexError::NoAlpha,
    ] {
        let notice = hex_notice(&error);
        assert!(notice.chars().count() <= 25, "{notice:?}");
        assert!(in_the_set(notice), "{notice:?}");
        assert!(!notice.is_empty());
    }
}

// Fila 12: alfa y extremos.

#[test]
fn row_12_alpha_to_zero_is_confirmed_and_seen_with_an_eight_digit_hex() {
    let target_2 = EditTarget::Entry(0);
    let mut editor = editor_with(target_2, RED);
    assert!(editor.begin(Control::Alpha, 0));
    let (provisional, _) = editor.shown().expect("hay destino");
    assert_eq!(provisional, rgba(255, 0, 0, 0));
    assert_eq!(hex_text(provisional), "#ff000000");
    assert_eq!(
        editor.release(),
        Some(Commit {
            target: target_2,
            color: rgba(255, 0, 0, 0)
        })
    );
}

#[test]
fn row_12_alpha_255_is_the_starting_value_and_gives_no_step() {
    let mut editor = editor_with(EditTarget::Entry(0), RED);
    assert!(editor.begin(Control::Alpha, 255));
    assert_eq!(editor.release(), None);
}

#[test]
fn row_12_alpha_128() {
    let mut editor = editor_with(EditTarget::Entry(0), RED);
    assert!(editor.begin(Control::Alpha, 128));
    let (provisional, _) = editor.shown().expect("hay destino");
    assert_eq!(hex_text(provisional), "#ff000080");
    assert_eq!(
        editor.release().map(|c| c.color),
        Some(rgba(255, 0, 0, 128))
    );
}

#[test]
fn row_12_a_value_above_the_maximum_is_limited() {
    let bases = [
        (RED, Hsl::from_rgba(RED)),
        (blue_light(), Hsl::from_rgba(blue_light())),
        (rgba(10, 200, 30, 90), Hsl::from_rgba(rgba(10, 200, 30, 90))),
    ];
    for base in bases {
        for control in CONTROLS {
            let expected_2 = apply_control(base, control, control.max());
            assert_eq!(apply_control(base, control, 999), expected_2, "{control:?}");
            assert_eq!(
                apply_control(base, control, u32::MAX),
                expected_2,
                "{control:?}"
            );
        }
    }
    // Los máximos de la spec: 255, 359 y 100.
    let base = (RED, Hsl::from_rgba(RED));
    assert_eq!(apply_control(base, Control::Green, 999).0.g, 255);
    let (_, hsl) = apply_control(base, Control::Hue, 999);
    assert!(near(hsl.h(), 359.0), "h = {}", hsl.h());
    let (_, hsl) = apply_control(base, Control::Saturation, 999);
    assert!(near(hsl.s(), 1.0));
    let (_, hsl) = apply_control(base, Control::Lightness, 999);
    assert!(near(hsl.l(), 1.0));
}

fn end(control: Control, valor: u32) -> (Rgba, Hsl) {
    let red = (RED, Hsl::from_rgba(RED));
    let (color, hsl) = apply_control(red, control, valor);
    assert_eq!(hsl.to_rgba(color.a), color, "{control:?} {valor}");
    (color, hsl)
}

#[test]
fn row_12_extremes_from_opaque_red() {
    assert_eq!(end(Control::Red, 0).0, Rgba::opaque(0, 0, 0));
    assert_eq!(end(Control::Green, 255).0, Rgba::opaque(255, 255, 0));
    assert_eq!(end(Control::Blue, 255).0, Rgba::opaque(255, 0, 255));
    assert_eq!(end(Control::Saturation, 0).0, Rgba::opaque(128, 128, 128));
    assert_eq!(end(Control::Lightness, 0).0, Rgba::opaque(0, 0, 0));
    assert_eq!(end(Control::Lightness, 100).0, Rgba::opaque(255, 255, 255));
    // Extremos que no cambian el color pero sí el campo: H 0 y 359, S 100.
    assert_eq!(end(Control::Hue, 0).0, RED);
    let (color, hsl) = end(Control::Hue, 359);
    assert!(near(hsl.h(), 359.0));
    assert_eq!((color.r, color.g, color.a), (255, 0, 255));
    assert_eq!(end(Control::Saturation, 100).0, RED);
    // Cada control (salvo el de alfa) a su máximo y a 0 conserva el alfa.
    for control in CONTROLS.into_iter().filter(|c| *c != Control::Alpha) {
        for valor in [0, control.max()] {
            assert_eq!(end(control, valor).0.a, 255, "{control:?}");
        }
    }
}

#[test]
fn row_12_gray_alpha() {
    let gray_2 = Rgba::opaque(128, 128, 128);
    let base = (gray_2, Hsl::from_rgba(gray_2));
    assert_eq!(
        apply_control(base, Control::Gray, 0).0,
        Rgba::opaque(0, 0, 0)
    );
    assert_eq!(
        apply_control(base, Control::Gray, 255).0,
        Rgba::opaque(255, 255, 255)
    );
    assert_eq!(
        apply_control(base, Control::Alpha, 0).0,
        rgba(128, 128, 128, 0),
        "A 0 no cambia el gris"
    );
}

// hsl_after_rgb.

#[test]
fn hsl_after_rgb_of_a_chromatic_color_is_from_rgba() {
    let anterior = Hsl::new(120.0, 0.5, 0.5);
    for color in [
        Rgba::opaque(128, 128, 255),
        rgba(10, 200, 30, 7),
        Rgba::opaque(255, 0, 0),
    ] {
        assert_eq!(hsl_after_rgb(anterior, color), Hsl::from_rgba(color));
    }
}

#[test]
fn hsl_after_rgb_of_a_gray_keeps_the_previous_hue() {
    let anterior = Hsl::new(120.0, 0.5, 0.5);
    let gray_2 = Rgba::opaque(128, 128, 128);
    let new_2 = hsl_after_rgb(anterior, gray_2);
    assert!(near(new_2.h(), 120.0), "h = {}", new_2.h());
    assert!(near(new_2.s(), 0.0));
    assert!(near(new_2.l(), Hsl::from_rgba(gray_2).l()));
    assert_eq!(new_2.to_rgba(255), gray_2);
}

#[test]
fn hsl_after_rgb_of_black_and_white_keeps_hue_and_saturation() {
    let anterior = Hsl::new(120.0, 0.5, 0.5);
    for (color, light) in [
        (Rgba::opaque(0, 0, 0), 0.0),
        (Rgba::opaque(255, 255, 255), 1.0),
    ] {
        let new_2 = hsl_after_rgb(anterior, color);
        assert!(near(new_2.h(), 120.0), "h = {}", new_2.h());
        assert!(near(new_2.s(), 0.5), "s = {}", new_2.s());
        assert!(near(new_2.l(), light));
        assert_eq!(new_2.to_rgba(255), color);
    }
}

// Propiedad 1: conversión de controles.

/// Base coherente: `hsl.to_rgba(a) == rgba` y, en Gray y GrayAlpha, `r == g == b` (en Gray, `a`
/// 255).
fn build_base(
    mode_2: Mode,
    channels: (u8, u8, u8, u8),
    hsl_raw: (f32, f32, f32),
    variant: bool,
) -> Option<(Rgba, Hsl)> {
    let (r, g, b, a) = channels;
    let alpha = if mode_2 == Mode::Gray { 255 } else { a };
    let gray_2 = matches!(mode_2, Mode::Gray | Mode::GrayAlpha);
    let (hue, s, l) = hsl_raw;
    let (rgba, hsl) = if variant {
        let saturation = if gray_2 { 0.0 } else { s };
        let hsl = Hsl::new(hue, saturation, l);
        (hsl.to_rgba(alpha), hsl)
    } else {
        let color = if gray_2 {
            Rgba::new(r, r, r, alpha)
        } else {
            Rgba::new(r, g, b, alpha)
        };
        let hsl = Hsl::from_rgba(color);
        // Un gris conserva un matiz cualquiera, como tras editar un color.
        let hsl = if color.r == color.g && color.g == color.b {
            Hsl::new(hue, 0.0, hsl.l())
        } else {
            hsl
        };
        (color, hsl)
    };
    (hsl.to_rgba(rgba.a) == rgba).then_some((rgba, hsl))
}

fn mode_s() -> impl Strategy<Value = Mode> {
    prop_oneof![
        Just(Mode::Rgba),
        Just(Mode::Indexed),
        Just(Mode::Gray),
        Just(Mode::GrayAlpha)
    ]
}

fn valor_s() -> impl Strategy<Value = u32> {
    prop_oneof![
        any::<u32>(),
        0u32..=400,
        Just(u32::MAX),
        Just(255u32),
        Just(359u32),
        Just(100u32)
    ]
}

fn check_control(
    mode_2: Mode,
    base: (Rgba, Hsl),
    control: Control,
    valor: u32,
) -> Result<(), TestCaseError> {
    let (rgba0, hsl0) = base;
    let (new_2, hsl) = apply_control(base, control, valor);
    let limited = valor.min(control.max());
    let canal = limited as u8;
    // El resultado solo depende de `(base, control, valor)`.
    prop_assert_eq!(apply_control(base, control, valor), (new_2, hsl));
    // El alfa solo cambia con el control de alfa.
    if control != Control::Alpha {
        prop_assert_eq!(new_2.a, rgba0.a);
    }
    match control {
        Control::Red => {
            prop_assert_eq!((new_2.r, new_2.g, new_2.b), (canal, rgba0.g, rgba0.b));
        }
        Control::Green => {
            prop_assert_eq!((new_2.r, new_2.g, new_2.b), (rgba0.r, canal, rgba0.b));
        }
        Control::Blue => {
            prop_assert_eq!((new_2.r, new_2.g, new_2.b), (rgba0.r, rgba0.g, canal));
        }
        Control::Gray => {
            prop_assert_eq!((new_2.r, new_2.g, new_2.b), (canal, canal, canal));
        }
        Control::Alpha => {
            prop_assert_eq!((new_2.r, new_2.g, new_2.b), (rgba0.r, rgba0.g, rgba0.b));
            prop_assert_eq!(new_2.a, canal);
            // El `Hsl` no se toca con el alfa.
            prop_assert_eq!(hsl, hsl0);
        }
        Control::Hue => {
            prop_assert!(near(hsl.h(), limited as f32), "h = {}", hsl.h());
            prop_assert_eq!(hsl.s(), hsl0.s());
            prop_assert_eq!(hsl.l(), hsl0.l());
        }
        Control::Saturation => {
            prop_assert!(near(hsl.s(), limited as f32 / 100.0), "s = {}", hsl.s());
            prop_assert_eq!(hsl.h(), hsl0.h());
            prop_assert_eq!(hsl.l(), hsl0.l());
        }
        Control::Lightness => {
            prop_assert!(near(hsl.l(), limited as f32 / 100.0), "l = {}", hsl.l());
            prop_assert_eq!(hsl.h(), hsl0.h());
            prop_assert_eq!(hsl.s(), hsl0.s());
        }
    }
    // El `Hsl` siempre es coherente con el `Rgba`.
    prop_assert_eq!(hsl.to_rgba(new_2.a), new_2);
    if matches!(
        control,
        Control::Hue | Control::Saturation | Control::Lightness
    ) {
        // `Hsl::to_rgba(alpha)`: el `Rgba` sale del `Hsl` nuevo.
        prop_assert_eq!(new_2, hsl.to_rgba(rgba0.a));
    }
    if matches!(mode_2, Mode::Gray | Mode::GrayAlpha) {
        prop_assert!(new_2.r == new_2.g && new_2.g == new_2.b);
    }
    Ok(())
}

proptest! {
    #[test]
    fn prop_1_conversion_of_controls(
        mode_2 in mode_s(),
        channels in (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()),
        hsl_raw in (0.0f32..360.0, 0.0f32..=1.0, 0.0f32..=1.0),
        variant in any::<bool>(),
        index_2 in 0usize..8,
        valor in valor_s(),
    ) {
        let Some(base) = build_base(mode_2, channels, hsl_raw, variant) else {
            return Ok(());
        };
        let available = controls(mode_2);
        let control = available[index_2 % available.len()];
        check_control(mode_2, base, control, valor)?;
    }

    #[test]
    fn prop_1_any_control_without_panic_even_if_the_mode_does_not_have_it(
        channels in (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()),
        index_2 in 0usize..8,
        valor in valor_s(),
    ) {
        let color = Rgba::new(channels.0, channels.1, channels.2, channels.3);
        let base = (color, Hsl::from_rgba(color));
        let control = CONTROLS[index_2];
        let (new_2, hsl) = apply_control(base, control, valor);
        prop_assert_eq!(hsl.to_rgba(new_2.a), new_2);
    }

    #[test]
    fn prop_1_slider_value_at_is_monotonic_and_reaches_the_extremes(
        width_2 in 2i64..2000,
        maximum_2 in prop_oneof![Just(100u32), Just(255u32), Just(359u32)],
        x0 in -100i64..2100,
        x1 in -100i64..2100,
    ) {
        let (smaller, mayor) = (x0.min(x1), x0.max(x1));
        let a = slider_value_at(smaller, width_2, maximum_2);
        let b = slider_value_at(mayor, width_2, maximum_2);
        prop_assert!(a <= b);
        prop_assert!(b <= maximum_2);
        prop_assert_eq!(slider_value_at(0, width_2, maximum_2), 0);
        prop_assert_eq!(slider_value_at(width_2 - 1, width_2, maximum_2), maximum_2);
        prop_assert_eq!(slider_value_at(i64::MIN / 4, width_2, maximum_2), 0);
        prop_assert_eq!(slider_value_at(i64::MAX / 4, width_2, maximum_2), maximum_2);
    }

    #[test]
    fn prop_1_slider_x_of_slider_value_at_stays_close_to_x(
        width_2 in 2i64..2000,
        maximum_2 in prop_oneof![Just(100u32), Just(255u32), Just(359u32)],
        x_raw in 0i64..2000,
    ) {
        let x = x_raw % width_2;
        let valor = slider_value_at(x, width_2, maximum_2);
        let ret = slider_x(valor, width_2, maximum_2);
        // `⌈(w − 1)/max⌉`.
        let tolerance = (width_2 - 1 + i64::from(maximum_2) - 1) / i64::from(maximum_2);
        prop_assert!(
            (ret - x).abs() <= tolerance,
            "x = {x}, valor = {valor}, vuelta = {ret}, tolerancia = {tolerance}"
        );
    }

    /// Estabilidad: bajar S (o L) al extremo, soltar y volver al valor inicial devuelve el color de
    /// partida. Los colores de partida tienen matiz, S y L enteros (los del deslizador).
    #[test]
    fn prop_1_stability_of_saturation_and_lightness(
        hue in 0u32..360,
        s in 0u32..=100,
        l in 0u32..=100,
        alpha in any::<u8>(),
        end in prop_oneof![
            Just((Control::Saturation, 0u32)),
            Just((Control::Lightness, 0u32)),
            Just((Control::Lightness, 100u32)),
        ],
    ) {
        let hsl = Hsl::new(hue as f32, s as f32 / 100.0, l as f32 / 100.0);
        let color = hsl.to_rgba(alpha);
        let base = (color, hsl);
        let (control, valor) = end;
        let initial = control_value(color, hsl, control);
        let (lowered, hsl_lowered) = apply_control(base, control, valor);
        let (returned, _) = apply_control((lowered, hsl_lowered), control, initial);
        prop_assert_eq!(returned, color);
    }
}

// Propiedad 2: máquina del arrastre.

#[derive(Clone, Debug)]
enum Op {
    Begin(Control, u32),
    Drag(u32),
    Release,
    Cancel,
    /// `sync` con el destino y el color que el editor ya tiene (si tiene).
    SyncActual,
    Sync(Option<(EditTarget, Rgba)>),
}

#[derive(Clone, Copy)]
enum State {
    Rest,
    Live {
        control: Control,
        base: (Rgba, Hsl),
        provisional: (Rgba, Hsl),
    },
    Cancelled,
}

struct Model {
    target_2: Option<EditTarget>,
    rgba: Rgba,
    hsl: Hsl,
    state_2: State,
}

impl Model {
    fn shown(&self) -> Option<(Rgba, Hsl)> {
        self.target_2?;
        Some(match self.state_2 {
            State::Live { provisional, .. } => provisional,
            _ => (self.rgba, self.hsl),
        })
    }

    fn preview(&self) -> Option<(EditTarget, Rgba)> {
        match self.state_2 {
            State::Live { provisional, .. } => Some((self.target_2?, provisional.0)),
            _ => None,
        }
    }

    fn sync(&mut self, actual: Option<(EditTarget, Rgba)>) {
        match actual {
            None => self.target_2 = None,
            Some((target_2, color)) => {
                if self.target_2 != Some(target_2) || self.rgba != color {
                    self.hsl = Hsl::from_rgba(color);
                    self.rgba = color;
                }
                self.target_2 = Some(target_2);
            }
        }
    }
}

fn target_s() -> impl Strategy<Value = EditTarget> {
    prop_oneof![
        (0u8..3).prop_map(EditTarget::Entry),
        (1u8..3).prop_map(EditTarget::Slot)
    ]
}

fn color_s() -> impl Strategy<Value = Rgba> {
    prop_oneof![
        Just(RED),
        Just(Rgba::opaque(128, 128, 128)),
        Just(blue_light()),
        Just(rgba(10, 20, 30, 40)),
        (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
            .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a)),
    ]
}

fn op_s() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0usize..8, valor_s()).prop_map(|(i, v)| Op::Begin(CONTROLS[i], v)),
        3 => valor_s().prop_map(Op::Drag),
        3 => Just(Op::Release),
        2 => Just(Op::Cancel),
        2 => Just(Op::SyncActual),
        2 => proptest::option::weighted(0.8, (target_s(), color_s())).prop_map(Op::Sync),
    ]
}

proptest! {
    #[test]
    fn prop_2_drag_machine(ops in vec(op_s(), 1..60)) {
        let mut editor = ColorEditor::default();
        let mut model = Model {
            target_2: None,
            rgba: Rgba::TRANSPARENT,
            hsl: Hsl::default(),
            state_2: State::Rest,
        };
        for op in ops {
            let cancelled = matches!(model.state_2, State::Cancelled);
            match op {
                // Con un arrastre cancelado a medias solo se admiten `drag` y `release`.
                Op::Begin(..) | Op::Cancel | Op::SyncActual | Op::Sync(_) if cancelled => {
                    continue;
                }
                Op::Begin(control, valor) => {
                    let accepted = editor.begin(control, valor);
                    let expected_2 = model.target_2.is_some()
                        && matches!(model.state_2, State::Rest);
                    prop_assert_eq!(accepted, expected_2);
                    if expected_2 {
                        let base = (model.rgba, model.hsl);
                        model.state_2 = State::Live {
                            control,
                            base,
                            provisional: apply_control(base, control, valor),
                        };
                    }
                }
                Op::Drag(valor) => {
                    editor.drag(valor);
                    if let State::Live { control, base, .. } = model.state_2 {
                        model.state_2 = State::Live {
                            control,
                            base,
                            provisional: apply_control(base, control, valor),
                        };
                    }
                }
                Op::Release => {
                    let commit = editor.release();
                    let expected_2 = match model.state_2 {
                        State::Live { base, provisional, .. } => {
                            model.rgba = provisional.0;
                            model.hsl = provisional.1;
                            (provisional.0 != base.0).then(|| Commit {
                                target: model.target_2.expect("hay destino"),
                                color: provisional.0,
                            })
                        }
                        _ => None,
                    };
                    model.state_2 = State::Rest;
                    prop_assert_eq!(commit, expected_2);
                }
                Op::Cancel => {
                    editor.cancel();
                    if matches!(model.state_2, State::Live { .. }) {
                        model.state_2 = State::Cancelled;
                    }
                }
                Op::SyncActual => {
                    let actual = model.target_2.map(|d| (d, model.rgba));
                    let antes = editor.shown();
                    editor.sync(actual);
                    model.sync(actual);
                    if matches!(model.state_2, State::Live { .. }) {
                        prop_assert_eq!(editor.shown(), antes, "sync durante el arrastre");
                    }
                }
                Op::Sync(actual) => {
                    let antes = editor.shown();
                    editor.sync(actual);
                    if matches!(model.state_2, State::Live { .. }) {
                        // No hace nada con arrastre en curso.
                        prop_assert_eq!(editor.shown(), antes, "sync durante el arrastre");
                    } else {
                        model.sync(actual);
                    }
                }
            }
            prop_assert_eq!(editor.shown(), model.shown());
            prop_assert_eq!(editor.preview(), model.preview());
            match model.state_2 {
                State::Live { .. } => prop_assert!(editor.dragging()),
                State::Rest => prop_assert!(!editor.dragging()),
                State::Cancelled => {}
            }
        }
    }
}

// Propiedad 3: hexadecimal.

proptest! {
    #[test]
    fn prop_3_round_trip_with_hex_text(
        r in any::<u8>(),
        g in any::<u8>(),
        b in any::<u8>(),
        a in any::<u8>(),
        gray_2 in any::<u8>(),
    ) {
        let color = Rgba::new(r, g, b, a);
        for mode_2 in [Mode::Rgba, Mode::Indexed] {
            prop_assert_eq!(parse_hex(&hex_text(color), mode_2, color), Ok(color));
        }
        let gray_with_alpha = Rgba::new(gray_2, gray_2, gray_2, a);
        prop_assert_eq!(
            parse_hex(&hex_text(gray_with_alpha), Mode::GrayAlpha, gray_with_alpha),
            Ok(gray_with_alpha)
        );
        let gray_opaque = Rgba::opaque(gray_2, gray_2, gray_2);
        prop_assert_eq!(
            parse_hex(&hex_text(gray_opaque), Mode::Gray, gray_opaque),
            Ok(gray_opaque)
        );
    }
}

/// Oráculo del hexadecimal, escrito a mano (sin `Rgba::from_hex`): recorta espacios, quita un `#`,
/// comprueba 3, 4, 6 u 8 dígitos ASCII, los lee, y aplica las reglas del modo.
fn oracle(text_2: &str, mode_2: Mode, actual: Rgba) -> Result<Rgba, HexError> {
    let clipped = text_2.trim_matches(' ');
    let digits = clipped.strip_prefix('#').unwrap_or(clipped);
    let characters: Vec<char> = digits.chars().collect();
    if characters.is_empty() {
        return Err(HexError::Empty);
    }
    let largo = characters.len();
    if !matches!(largo, 3 | 4 | 6 | 8) {
        return Err(HexError::Length(largo));
    }
    let mut values = Vec::new();
    for c in &characters {
        let valor = match c {
            '0'..='9' => *c as u8 - b'0',
            'a'..='f' => *c as u8 - b'a' + 10,
            'A'..='F' => *c as u8 - b'A' + 10,
            _ => return Err(HexError::Digit),
        };
        values.push(valor);
    }
    let canal = |i: usize| -> u8 {
        if largo <= 4 {
            values[i] * 17
        } else {
            values[2 * i] * 16 + values[2 * i + 1]
        }
    };
    let writes_alpha = largo == 4 || largo == 8;
    let alpha = if writes_alpha { canal(3) } else { actual.a };
    let color = Rgba::new(canal(0), canal(1), canal(2), alpha);
    let is_gray = color.r == color.g && color.g == color.b;
    match mode_2 {
        Mode::Gray => {
            if writes_alpha && alpha != 255 {
                return Err(HexError::NoAlpha);
            }
            if !is_gray {
                return Err(HexError::NotGray);
            }
        }
        Mode::GrayAlpha if !is_gray => return Err(HexError::NotGray),
        _ => {}
    }
    Ok(color)
}

fn character_s() -> impl Strategy<Value = char> {
    prop_oneof![
        6 => prop_oneof![
            proptest::char::range('0', '9'),
            proptest::char::range('a', 'f'),
            proptest::char::range('A', 'F'),
        ],
        1 => prop_oneof![
            proptest::char::range('g', 'z'),
            Just('ñ'),
            Just(' '),
            Just('-'),
            Just('é'),
            Just('😀'),
            // Cualquier carácter que no sea un espacio ni `#`.
            any::<char>().prop_filter("sin espacios ni almohadilla", |c| {
                !c.is_whitespace() && *c != '#'
            }),
        ],
    ]
}

fn text_hex_s() -> impl Strategy<Value = String> {
    let body = vec(character_s(), 0..=10).prop_map(|cs| {
        let text_2: String = cs.into_iter().collect();
        text_2.trim_matches(' ').to_string()
    });
    let valid = (
        prop_oneof![Just(3usize), Just(4), Just(6), Just(8)],
        vec(
            prop_oneof![
                proptest::char::range('0', '9'),
                proptest::char::range('a', 'f'),
                proptest::char::range('A', 'F'),
            ],
            8,
        ),
    )
        .prop_map(|(largo, cs)| cs.into_iter().take(largo).collect::<String>());
    (
        0usize..=1,
        any::<bool>(),
        prop_oneof![body, valid],
        0usize..=1,
    )
        .prop_map(|(left, hash, body, right)| {
            format!(
                "{}{}{}{}",
                " ".repeat(left),
                if hash { "#" } else { "" },
                body,
                " ".repeat(right)
            )
        })
}

proptest! {
    #[test]
    fn prop_3_parse_hex_matches_the_oracle_and_the_notices_fit(
        text_2 in text_hex_s(),
        mode_2 in mode_s(),
        channels in (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()),
    ) {
        prop_assume!(text_2.chars().count() <= 12);
        // En Gray el color que se edita es opaco.
        let alpha = if mode_2 == Mode::Gray { 255 } else { channels.3 };
        let actual = Rgba::new(channels.0, channels.1, channels.2, alpha);
        let obtained = parse_hex(&text_2, mode_2, actual);
        prop_assert_eq!(&obtained, &oracle(&text_2, mode_2, actual), "{:?}", text_2);
        if let Err(error) = obtained {
            let notice = hex_notice(&error);
            prop_assert!(notice.chars().count() <= 25, "{notice:?}");
            prop_assert!(in_the_set(notice), "{notice:?}");
        }
    }

    #[test]
    fn prop_3_any_unicode_text_does_not_panic(
        text_2 in "\\PC{0,12}",
        mode_2 in mode_s(),
        channels in (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()),
    ) {
        let actual = Rgba::new(channels.0, channels.1, channels.2, channels.3);
        if let Err(error) = parse_hex(&text_2, mode_2, actual) {
            let notice = hex_notice(&error);
            prop_assert!(notice.chars().count() <= 25, "{notice:?}");
            prop_assert!(in_the_set(notice), "{notice:?}");
        }
    }
}

#[test]
fn the_oracle_of_property_3_gets_the_examples_of_the_spec_right() {
    // Guarda del propio oráculo: que no se separe de la spec.
    let actual = rgba(1, 2, 3, 128);
    assert_eq!(
        oracle("#f80", Mode::Rgba, actual),
        Ok(rgba(255, 136, 0, 128))
    );
    assert_eq!(
        oracle("f80f", Mode::Rgba, actual),
        Ok(rgba(255, 136, 0, 255))
    );
    assert_eq!(oracle("ñññ", Mode::Rgba, actual), Err(HexError::Digit));
    assert_eq!(oracle("#", Mode::Rgba, actual), Err(HexError::Empty));
    assert_eq!(
        oracle("#80808080", Mode::Gray, Rgba::opaque(0, 0, 0)),
        Err(HexError::NoAlpha)
    );
}

// Texto pendiente del campo: `has_pending`, `confirm_hex_checked` y los avisos
// (specs/hito-2-panel-ventana-baja.md, filas 7, 8 y 9 en su parte pura).

fn done(resolved: Resolved) -> HexOutcome {
    match resolved {
        Resolved::Done(result) => result,
        Resolved::Stale(reason_2) => panic!("se esperaba Done y fue Stale({reason_2:?})"),
    }
}

fn discarded(resolved: Resolved) -> StaleReason {
    match resolved {
        Resolved::Stale(reason_2) => reason_2,
        Resolved::Done(_) => panic!("se esperaba Stale y fue Done"),
    }
}

fn commit_a(target: EditTarget, color: Rgba) -> HexOutcome {
    HexOutcome::Commit(Commit { target, color })
}

#[test]
fn has_pending_follows_type_hex_and_what_clears_it() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    assert!(!editor.has_pending(), "recién creado");

    // Teclear lo deja pendiente, aunque el texto aún no sea válido.
    kind(&mut editor, target_2, BLUE, "#00");
    assert!(editor.has_pending());

    // Intro con un valor inválido conserva el texto: sigue pendiente.
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Invalid(HexError::Length(2))
    );
    assert!(
        editor.has_pending(),
        "con Intro el texto inválido se conserva"
    );

    // Perder el foco con un valor inválido lo revierte.
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, true),
        HexOutcome::Invalid(HexError::Length(2))
    );
    assert!(!editor.has_pending());

    // Confirmar uno válido, Escape y `reset` lo borran.
    kind(&mut editor, target_2, BLUE, "#00ff00");
    assert!(editor.has_pending());
    editor.confirm_hex(Mode::Indexed, false);
    assert!(!editor.has_pending());

    kind(&mut editor, target_2, BLUE, "#00ff00");
    editor.escape_hex();
    assert!(!editor.has_pending());

    kind(&mut editor, target_2, BLUE, "#00ff00");
    editor.reset();
    assert!(!editor.has_pending());
    assert_eq!(editor.hex_display(), None);
}

#[test]
fn has_pending_matches_hex_display() {
    let target_2 = EditTarget::Entry(1);
    let mut editor = editor_with(target_2, GREEN);
    assert_eq!(editor.has_pending(), editor.hex_display().is_some());
    kind(&mut editor, target_2, GREEN, "f");
    assert_eq!(editor.has_pending(), editor.hex_display().is_some());
    assert!(editor.has_pending());
}

// Fila 7: el texto válido sobre una entrada que sigue igual se aplica.

#[test]
fn row_7_an_entry_that_stays_the_same_applies_the_text_to_the_original_target() {
    for blur in [true, false] {
        let target_2 = EditTarget::Entry(2);
        let mut editor = editor_with(target_2, BLUE);
        kind(&mut editor, target_2, BLUE, "#00ff00");
        let resolved = editor.confirm_hex_checked(Mode::Indexed, blur, |_| Some(BLUE));
        assert_eq!(done(resolved), commit_a(target_2, GREEN), "blur {blur}");
        assert!(!editor.has_pending(), "blur {blur}");
        assert_eq!(editor.hex_display(), None);
        assert!(editor.notice(false).iter().all(|l| empty(l)));
    }
}

#[test]
fn row_7_it_queries_the_stored_one_of_the_original_target_not_the_current() {
    // Entre teclear y perder el foco la elección pasó a la entrada 3.
    let original = EditTarget::Entry(2);
    let mut editor = editor_with(original, BLUE);
    kind(&mut editor, original, BLUE, "#00ff00");
    editor.sync(Some((EditTarget::Entry(3), NEGRO)));
    let asked = Cell::new(None);
    let resolved = editor.confirm_hex_checked(Mode::Indexed, true, |target_2| {
        asked.set(Some(target_2));
        Some(BLUE)
    });
    assert_eq!(done(resolved), commit_a(original, GREEN));
    assert_eq!(
        asked.get(),
        Some(original),
        "se pregunta por el destino original"
    );
}

#[test]
fn row_7_with_the_same_color_it_says_unchanged_and_without_a_pending_one_nothing() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "#0000ff");
    let resolved = editor.confirm_hex_checked(Mode::Indexed, true, |_| Some(BLUE));
    assert_eq!(done(resolved), HexOutcome::Unchanged);
    assert!(!editor.has_pending());

    // Sin nada pendiente no hay nada que descartar.
    let resolved = editor.confirm_hex_checked(Mode::Indexed, true, |_| Some(BLUE));
    assert_eq!(done(resolved), HexOutcome::Nothing);
    let resolved = editor.confirm_hex_checked(Mode::Indexed, false, |_| None);
    assert_eq!(done(resolved), HexOutcome::Nothing);
}

#[test]
fn row_7_keeps_the_alpha_of_the_base_color_and_works_with_a_slot() {
    let target_2 = EditTarget::Slot(4);
    let base = rgba(10, 20, 30, 128);
    let mut editor = editor_with(target_2, base);
    kind(&mut editor, target_2, base, "#00ff00");
    let resolved = editor.confirm_hex_checked(Mode::Rgba, true, |d| {
        assert_eq!(d, target_2);
        Some(base)
    });
    assert_eq!(done(resolved), commit_a(target_2, rgba(0, 255, 0, 128)));
}

#[test]
fn row_7_an_invalid_text_over_an_entry_that_stays_the_same_is_that_of_confirm_hex() {
    let target_2 = EditTarget::Entry(2);
    // Perder el foco: invalido, revierte.
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "zzz");
    let resolved = editor.confirm_hex_checked(Mode::Indexed, true, |_| Some(BLUE));
    assert_eq!(done(resolved), HexOutcome::Invalid(HexError::Digit));
    assert!(!editor.has_pending());
    // Intro: conserva el texto y el aviso.
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "zzz");
    let resolved = editor.confirm_hex_checked(Mode::Indexed, false, |_| Some(BLUE));
    assert_eq!(done(resolved), HexOutcome::Invalid(HexError::Digit));
    assert!(editor.has_pending());
    assert_eq!(editor.hex_display().as_deref(), Some("zzz"));
}

// Fila 8: la entrada ya no existe.

#[test]
fn row_8_if_the_entry_no_longer_exists_it_is_discarded_without_a_commit() {
    for blur in [true, false] {
        let target_2 = EditTarget::Entry(9);
        let mut editor = editor_with(target_2, BLUE);
        kind(&mut editor, target_2, BLUE, "#00ff00");
        let resolved = editor.confirm_hex_checked(Mode::Indexed, blur, |_| None);
        assert_eq!(discarded(resolved), StaleReason::Gone, "blur {blur}");
        assert!(!editor.has_pending(), "blur {blur}");
        assert_eq!(editor.hex_display(), None, "blur {blur}");
        // No deja aviso de hexadecimal en el editor: el aviso va a la barra.
        assert!(editor.notice(false).iter().all(|l| empty(l)), "blur {blur}");
    }
}

#[test]
fn row_8_the_absent_entry_beats_an_invalid_text() {
    let target_2 = EditTarget::Entry(9);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "zzz");
    let resolved = editor.confirm_hex_checked(Mode::Indexed, false, |_| None);
    assert_eq!(discarded(resolved), StaleReason::Gone);
    assert!(!editor.has_pending(), "ni con Intro se conserva");
    assert!(editor.notice(false).iter().all(|l| empty(l)));
}

#[test]
fn row_8_it_also_applies_to_a_slot_without_a_stored_color() {
    let target_2 = EditTarget::Slot(4);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "#00ff00");
    let resolved = editor.confirm_hex_checked(Mode::Rgba, true, |_| None);
    assert_eq!(discarded(resolved), StaleReason::Gone);
    assert!(!editor.has_pending());
}

// Fila 9: el color guardado cambió.

#[test]
fn row_9_if_the_stored_color_changed_it_is_discarded_without_a_commit() {
    for blur in [true, false] {
        let target_2 = EditTarget::Entry(2);
        let mut editor = editor_with(target_2, BLUE);
        kind(&mut editor, target_2, BLUE, "#00ff00");
        let resolved = editor.confirm_hex_checked(Mode::Indexed, blur, |_| Some(RED));
        assert_eq!(discarded(resolved), StaleReason::Changed, "blur {blur}");
        assert!(!editor.has_pending(), "blur {blur}");
        assert_eq!(editor.hex_display(), None, "blur {blur}");
    }
}

#[test]
fn row_9_the_color_changes_but_also_if_only_the_alpha_changes() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "#00ff00");
    let changed = rgba(0, 0, 255, 128);
    let resolved = editor.confirm_hex_checked(Mode::Indexed, true, |_| Some(changed));
    assert_eq!(discarded(resolved), StaleReason::Changed);
}

#[test]
fn row_9_an_invalid_text_over_a_changed_color_is_also_discarded() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "zzz");
    let resolved = editor.confirm_hex_checked(Mode::Indexed, false, |_| Some(RED));
    assert_eq!(discarded(resolved), StaleReason::Changed);
    assert!(!editor.has_pending());
    assert!(editor.notice(false).iter().all(|l| empty(l)));
}

#[test]
fn row_9_the_slot_changes_after_typing_and_the_pending_one_survives_the_sync_until_resolved() {
    // Deshacer o rehacer, o cambiar la ranura, hacen que `sync` vea otro color; el texto sin
    // error sigue pendiente hasta que el campo lo resuelva.
    let target_2 = EditTarget::Slot(4);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "#00ff00");
    editor.sync(Some((target_2, RED)));
    assert!(
        editor.has_pending(),
        "el sync no resuelve el texto pendiente"
    );
    let resolved = editor.confirm_hex_checked(Mode::Rgba, true, |_| Some(RED));
    assert_eq!(discarded(resolved), StaleReason::Changed);
    assert!(!editor.has_pending());
}

#[test]
fn row_9_if_the_color_goes_back_to_the_base_one_it_is_applied() {
    // Deshacer (cambia) y rehacer (vuelve): al resolver, el guardado es otra vez el base.
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "#00ff00");
    editor.sync(Some((target_2, RED)));
    editor.sync(Some((target_2, BLUE)));
    let resolved = editor.confirm_hex_checked(Mode::Indexed, true, |_| Some(BLUE));
    assert_eq!(done(resolved), commit_a(target_2, GREEN));
    assert!(!editor.has_pending());
}

#[test]
fn after_a_stale_the_editor_accepts_new_text() {
    let target_2 = EditTarget::Entry(2);
    let mut editor = editor_with(target_2, BLUE);
    kind(&mut editor, target_2, BLUE, "#00ff00");
    editor.confirm_hex_checked(Mode::Indexed, true, |_| None);
    kind(&mut editor, target_2, BLUE, "#ffff00");
    assert!(editor.has_pending());
    let resolved = editor.confirm_hex_checked(Mode::Indexed, true, |_| Some(BLUE));
    assert_eq!(done(resolved), commit_a(target_2, YELLOW));
}

// Avisos fijos.

#[test]
fn the_discarded_text_notices_are_those_of_the_spec() {
    assert_eq!(
        stale_notice(StaleReason::Gone),
        "texto descartado: la entrada ya no existe"
    );
    assert_eq!(
        stale_notice(StaleReason::Changed),
        "texto descartado: el color cambió"
    );
    assert_eq!(
        DOCUMENT_CHANGED_NOTICE,
        "texto descartado: se abrió otro documento"
    );
}

#[test]
fn the_discarded_text_notices_only_use_the_character_set() {
    for notice in [
        stale_notice(StaleReason::Gone),
        stale_notice(StaleReason::Changed),
        DOCUMENT_CHANGED_NOTICE,
    ] {
        assert!(
            in_the_set(notice),
            "{notice:?} lleva un carácter fuera del juego"
        );
        assert!(!notice.is_empty());
        assert!(!notice.contains('\n'), "{notice:?}: una sola línea");
    }
    assert_ne!(
        stale_notice(StaleReason::Gone),
        stale_notice(StaleReason::Changed)
    );
}

#[test]
fn stale_reason_is_copyable_and_comparable() {
    let reason_2 = StaleReason::Gone;
    let copy = reason_2;
    assert_eq!(reason_2, copy);
    assert_ne!(StaleReason::Gone, StaleReason::Changed);
}
