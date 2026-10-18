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

const ROJO: Rgba = Rgba::opaque(255, 0, 0);
const VERDE: Rgba = Rgba::opaque(0, 255, 0);
const AZUL: Rgba = Rgba::opaque(0, 0, 255);
const NEGRO: Rgba = Rgba::opaque(0, 0, 0);
const AMARILLO: Rgba = Rgba::opaque(255, 255, 0);

const CONTROLES: [Control; 8] = [
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
fn imagen_f() -> Image {
    let paleta = Palette::from_colors([ROJO, VERDE, AZUL, NEGRO]).expect("paleta válida");
    let mut imagen = Image::new_indexed(4, 1, paleta).expect("imagen válida");
    for x in 0..4u8 {
        imagen
            .set_pixel(u32::from(x), 0, Pixel::Indexed(x))
            .expect("píxel válido");
    }
    imagen
}

fn imagen_modo(modo: Mode) -> Image {
    match modo {
        Mode::Indexed => imagen_f(),
        otro => Image::new(4, 4, otro).expect("imagen válida"),
    }
}

/// Editor ya sincronizado con ese destino y color guardado.
fn editor_con(target: EditTarget, color: Rgba) -> ColorEditor {
    let mut editor = ColorEditor::default();
    editor.sync(Some((target, color)));
    editor
}

fn cerca(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

/// Valor de `H` que muestra el editor.
fn matiz_mostrado(editor: &ColorEditor) -> u32 {
    let (color, hsl) = editor.shown().expect("hay destino");
    control_value(color, hsl, Control::Hue)
}

/// Lo que hace `px-app` tras un arrastre: el color guardado que ve el siguiente `sync` es el
/// que quedó en el editor (o el confirmado).
fn arrastrar(
    editor: &mut ColorEditor,
    target: EditTarget,
    control: Control,
    valores: &[u32],
) -> Option<Commit> {
    assert!(editor.begin(control, valores[0]), "begin aceptado");
    for valor in &valores[1..] {
        editor.drag(*valor);
    }
    let commit = editor.release();
    let guardado = match commit {
        Some(commit) => commit.color,
        None => editor.shown().expect("hay destino").0,
    };
    editor.sync(Some((target, guardado)));
    commit
}

/// El primer renglón del aviso sin el prefijo `aviso: ` (la spec dice que el editor muestra
/// `aviso: ` + motivo; este helper vale tanto si `notice` ya lo trae como si lo pone quien pinta).
fn sin_prefijo(linea: &str) -> &str {
    linea.strip_prefix(NOTICE_PREFIX).unwrap_or(linea)
}

fn vacia(linea: &str) -> bool {
    linea.trim().is_empty()
}

fn en_el_juego(texto: &str) -> bool {
    texto.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

// Fila 1: destino.

#[test]
fn fila_1_destino_sobre_la_indexada_f() {
    let f = imagen_f();
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
fn fila_1_destino_sobre_rgba() {
    let imagen = imagen_modo(Mode::Rgba);
    let de = |choice| EditTarget::of(&imagen, choice);
    assert_eq!(de(ColorChoice::Slot(3)), Some(EditTarget::Slot(3)));
    assert_eq!(de(ColorChoice::Slot(0)), None);
    assert_eq!(de(ColorChoice::Slot(10)), None);
    assert_eq!(de(ColorChoice::Entry(0)), None);
    for slot in 1..=9 {
        assert_eq!(de(ColorChoice::Slot(slot)), Some(EditTarget::Slot(slot)));
    }
}

#[test]
fn fila_1_destino_sobre_gray_y_gray_alpha() {
    let gris = imagen_modo(Mode::Gray);
    assert_eq!(
        EditTarget::of(&gris, ColorChoice::Slot(9)),
        Some(EditTarget::Slot(9))
    );
    assert_eq!(EditTarget::of(&gris, ColorChoice::Entry(0)), None);
    assert_eq!(EditTarget::of(&gris, ColorChoice::Slot(0)), None);
    let con_alfa = imagen_modo(Mode::GrayAlpha);
    assert_eq!(
        EditTarget::of(&con_alfa, ColorChoice::Slot(9)),
        Some(EditTarget::Slot(9))
    );
    assert_eq!(EditTarget::of(&con_alfa, ColorChoice::Slot(10)), None);
    assert_eq!(EditTarget::of(&con_alfa, ColorChoice::Entry(1)), None);
}

#[test]
fn el_destino_guardado_es_el_color_de_la_paleta_o_de_la_ranura() {
    let f = imagen_f();
    let ranuras = SlotColors::defaults(Mode::Rgba);
    assert_eq!(EditTarget::Entry(1).stored(&f, &ranuras), Some(VERDE));
    assert_eq!(EditTarget::Entry(3).stored(&f, &ranuras), Some(NEGRO));
    assert_eq!(EditTarget::Entry(4).stored(&f, &ranuras), None);

    let imagen = imagen_modo(Mode::Rgba);
    let mut ranuras = SlotColors::defaults(Mode::Rgba);
    assert!(ranuras.set(3, rgba(1, 2, 3, 4)));
    assert_eq!(
        EditTarget::Slot(3).stored(&imagen, &ranuras),
        Some(rgba(1, 2, 3, 4))
    );
    assert_eq!(
        EditTarget::Slot(2).stored(&imagen, &ranuras),
        ranuras.get(2)
    );
}

#[test]
fn se_edita_el_color_guardado_no_el_resuelto() {
    // Fila 13 (b), parte pura: la entrada transparente se ve con su color guardado.
    let mut paleta = Palette::from_colors([ROJO, VERDE, AZUL]).expect("paleta válida");
    paleta
        .set_transparent_index(Some(2))
        .expect("índice válido");
    let imagen = Image::new_indexed(3, 1, paleta).expect("imagen válida");
    let ranuras = SlotColors::defaults(Mode::Rgba);
    assert_eq!(EditTarget::Entry(2).stored(&imagen, &ranuras), Some(AZUL));
}

// Fila 2: valores, textos y cabeceras.

#[test]
fn fila_2_valores_de_un_naranja_opaco() {
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
fn fila_2_valores_del_transparente_negro_y_del_blanco() {
    let cero = Rgba::TRANSPARENT;
    let hsl = Hsl::from_rgba(cero);
    for control in [
        Control::Red,
        Control::Green,
        Control::Blue,
        Control::Alpha,
        Control::Hue,
        Control::Saturation,
        Control::Lightness,
    ] {
        assert_eq!(control_value(cero, hsl, control), 0, "{control:?}");
    }
    let blanco = Rgba::opaque(255, 255, 255);
    let hsl = Hsl::from_rgba(blanco);
    assert_eq!(control_value(blanco, hsl, Control::Lightness), 100);
    assert_eq!(control_value(blanco, hsl, Control::Saturation), 0);
}

#[test]
fn fila_2_gris_96_en_gray() {
    let color = Rgba::opaque(96, 96, 96);
    assert_eq!(
        control_value(color, Hsl::from_rgba(color), Control::Gray),
        96
    );
    assert_eq!(hex_text(color), "#606060");
}

#[test]
fn fila_2_naranja_con_alfa_128() {
    let color = rgba(255, 128, 0, 128);
    let hsl = Hsl::from_rgba(color);
    assert_eq!(control_value(color, hsl, Control::Alpha), 128);
    assert_eq!(hex_text(color), "#ff800080");
    assert_eq!(hex_text(Rgba::opaque(255, 128, 0)), "#ff8000");
    assert_eq!(hex_text(Rgba::TRANSPARENT), "#00000000");
}

#[test]
fn fila_2_el_matiz_se_redondea_y_envuelve_y_s_y_l_se_redondean() {
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
fn value_text_ocupa_tres_columnas_a_la_derecha() {
    assert_eq!(value_text(0), "  0");
    assert_eq!(value_text(7), "  7");
    assert_eq!(value_text(30), " 30");
    assert_eq!(value_text(100), "100");
    assert_eq!(value_text(255), "255");
    assert_eq!(value_text(359), "359");
    for valor in 0..=359u32 {
        let texto = value_text(valor);
        assert_eq!(texto.chars().count(), 3, "{valor}");
        assert_eq!(texto.trim().parse::<u32>().ok(), Some(valor));
        assert!(texto.starts_with(' ') || valor >= 100);
    }
}

#[test]
fn fila_2_cabeceras() {
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
fn fila_2_ninguna_cabecera_supera_las_32_columnas_ni_sale_del_juego_de_caracteres() {
    let mut cabeceras = Vec::new();
    for entrada in 0..=255u8 {
        cabeceras.push(EditTarget::Entry(entrada).header(Mode::Indexed));
    }
    for modo in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        for ranura in 1..=9u8 {
            cabeceras.push(EditTarget::Slot(ranura).header(modo));
        }
    }
    for cabecera in cabeceras {
        assert!(cabecera.chars().count() <= 32, "{cabecera:?}");
        assert!(en_el_juego(&cabecera), "{cabecera:?}");
    }
    assert_eq!(
        EditTarget::Slot(4).header(Mode::Gray).chars().count(),
        19,
        "`editar color 4 gris` son 19"
    );
}

#[test]
fn fila_2_los_avisos_caben_en_32_columnas_y_en_el_juego_de_caracteres() {
    // `hex_notice` va tras `aviso: `: 7 + 25 = 32.
    for error in [
        HexError::Empty,
        HexError::Length(5),
        HexError::Digit,
        HexError::NotGray,
        HexError::NoAlpha,
    ] {
        let motivo = hex_notice(&error);
        let linea = format!("{NOTICE_PREFIX}{motivo}");
        assert!(linea.chars().count() <= 32, "{linea:?}");
        assert!(en_el_juego(&linea), "{linea:?}");
    }
    let editor = ColorEditor::default();
    for linea in editor.notice(true) {
        assert!(linea.chars().count() <= 32, "{linea:?}");
        assert!(en_el_juego(&linea), "{linea:?}");
    }
    assert_eq!(
        format!("{NOTICE_PREFIX}entrada transparente")
            .chars()
            .count(),
        27
    );
}

#[test]
fn los_textos_de_hex_notice_son_los_de_la_spec() {
    assert_eq!(hex_notice(&HexError::Empty), "escribe un color");
    assert_eq!(hex_notice(&HexError::Length(5)), "usa 3, 4, 6 u 8 dígitos");
    assert_eq!(hex_notice(&HexError::Length(9)), "usa 3, 4, 6 u 8 dígitos");
    assert_eq!(hex_notice(&HexError::Digit), "solo dígitos 0-9 y a-f");
    assert_eq!(hex_notice(&HexError::NotGray), "en modo gris: r = g = b");
    assert_eq!(hex_notice(&HexError::NoAlpha), "este modo no tiene alfa");
}

#[test]
fn los_controles_de_cada_modo_en_orden_de_arriba_abajo() {
    let completos = [
        Control::Red,
        Control::Green,
        Control::Blue,
        Control::Hue,
        Control::Saturation,
        Control::Lightness,
        Control::Alpha,
    ];
    assert_eq!(controls(Mode::Rgba), completos);
    assert_eq!(controls(Mode::Indexed), completos);
    assert_eq!(controls(Mode::GrayAlpha), [Control::Gray, Control::Alpha]);
    assert_eq!(controls(Mode::Gray), [Control::Gray]);
}

#[test]
fn letras_y_maximos_de_los_controles() {
    let esperado = [
        (Control::Red, 'R', 255),
        (Control::Green, 'G', 255),
        (Control::Blue, 'B', 255),
        (Control::Hue, 'H', 359),
        (Control::Saturation, 'S', 100),
        (Control::Lightness, 'L', 100),
        (Control::Alpha, 'A', 255),
        (Control::Gray, 'G', 255),
    ];
    for (control, letra, maximo) in esperado {
        assert_eq!(control.label(), letra, "{control:?}");
        assert_eq!(control.max(), maximo, "{control:?}");
    }
}

// Fila 3: deslizadores (parte pura).

#[test]
fn fila_3_slider_value_at_con_maximo_255() {
    for n in [1i64, 2, 3] {
        let ancho = 184 * n;
        assert_eq!(slider_value_at(-5, ancho, 255), 0, "n = {n}");
        assert_eq!(slider_value_at(0, ancho, 255), 0, "n = {n}");
        assert_eq!(slider_value_at(ancho - 1, ancho, 255), 255, "n = {n}");
        assert_eq!(slider_value_at(ancho + 9, ancho, 255), 255, "n = {n}");
    }
    // `92·255/183` redondeado.
    assert_eq!(slider_value_at(92, 184, 255), 128);
    for n in [2i64, 3] {
        let medio = slider_value_at(92 * n, 184 * n, 255);
        assert!((127..=128).contains(&medio), "n = {n}: {medio}");
    }
}

#[test]
fn fila_3_slider_value_at_con_maximo_359_y_100_en_el_medio() {
    for n in [1i64, 2, 3] {
        assert_eq!(slider_value_at(92 * n, 184 * n, 359), 180, "n = {n}");
        assert_eq!(slider_value_at(92 * n, 184 * n, 100), 50, "n = {n}");
    }
}

#[test]
fn fila_3_slider_x_en_los_extremos() {
    for n in [1i64, 2, 3] {
        for maximo in [100u32, 255, 359] {
            assert_eq!(slider_x(0, 184 * n, maximo), 0, "n = {n}, máx. {maximo}");
            assert_eq!(
                slider_x(maximo, 184 * n, maximo),
                184 * n - 1,
                "n = {n}, máx. {maximo}"
            );
        }
    }
}

#[test]
fn fila_3_slider_x_y_slider_value_at_son_monotonas() {
    for n in [1i64, 2, 3] {
        let ancho = 184 * n;
        for maximo in [100u32, 255, 359] {
            let mut anterior = slider_x(0, ancho, maximo);
            for valor in 1..=maximo {
                let actual = slider_x(valor, ancho, maximo);
                assert!(actual >= anterior, "slider_x({valor}), n = {n}");
                assert!((0..ancho).contains(&actual));
                anterior = actual;
            }
            let mut anterior = slider_value_at(0, ancho, maximo);
            for x in 1..ancho {
                let actual = slider_value_at(x, ancho, maximo);
                assert!(actual >= anterior, "slider_value_at({x}), n = {n}");
                assert!(actual <= maximo);
                anterior = actual;
            }
        }
    }
}

#[test]
fn fila_3_ida_y_vuelta_con_maximo_100() {
    for n in [1i64, 2, 3] {
        let ancho = 184 * n;
        for valor in 0..=100u32 {
            let x = slider_x(valor, ancho, 100);
            assert_eq!(slider_value_at(x, ancho, 100), valor, "n = {n}");
        }
    }
}

// Fila 5: sin paso.

#[test]
fn fila_5_pulsar_y_soltar_sobre_el_valor_actual_no_da_paso() {
    let mut editor = editor_con(EditTarget::Entry(0), ROJO);
    assert!(editor.begin(Control::Red, 255));
    assert_eq!(editor.release(), None);
}

#[test]
fn fila_5_volver_al_valor_inicial_no_da_paso() {
    let mut editor = editor_con(EditTarget::Entry(0), ROJO);
    assert!(editor.begin(Control::Red, 0));
    editor.drag(200);
    editor.drag(255);
    assert_eq!(editor.release(), None);
}

#[test]
fn fila_5_cancelar_deja_el_editor_como_al_empezar_y_lo_siguiente_no_hace_nada() {
    let mut editor = editor_con(EditTarget::Entry(0), ROJO);
    let inicio = editor.shown().expect("hay destino");
    assert!(editor.begin(Control::Green, 90));
    assert_ne!(editor.shown(), Some(inicio), "el provisional se ve");
    editor.drag(10);
    editor.cancel();
    assert_eq!(editor.shown(), Some(inicio), "Rgba y Hsl exactos");
    assert_eq!(editor.preview(), None);
    editor.drag(200);
    assert_eq!(editor.shown(), Some(inicio), "no se reanuda");
    assert_eq!(editor.preview(), None);
    assert_eq!(editor.release(), None);
    assert_eq!(editor.shown(), Some(inicio));
    assert_eq!(editor.preview(), None);
    assert!(!editor.dragging());
}

#[test]
fn fila_5_perder_el_foco_de_la_ventana_es_cancelar() {
    // La app llama a `cancel` al perder el foco la ventana: igual que arriba, sin paso.
    let mut editor = editor_con(EditTarget::Entry(0), ROJO);
    let inicio = editor.shown();
    assert!(editor.begin(Control::Blue, 255));
    editor.drag(128);
    editor.cancel();
    assert_eq!(editor.release(), None);
    assert_eq!(editor.shown(), inicio);
}

#[test]
fn fila_5_begin_sin_destino_da_false() {
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
fn fila_5_begin_con_otro_arrastre_en_curso_da_false_y_no_lo_altera() {
    let mut editor = editor_con(EditTarget::Entry(0), ROJO);
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
fn fila_5_drag_y_release_sueltos_no_hacen_nada() {
    let mut editor = editor_con(EditTarget::Entry(0), ROJO);
    let inicio = editor.shown();
    editor.drag(77);
    assert_eq!(editor.shown(), inicio);
    assert_eq!(editor.preview(), None);
    assert_eq!(editor.release(), None);
    assert_eq!(editor.shown(), inicio);
    assert!(!editor.dragging());
}

#[test]
fn fila_5_el_matiz_sobre_un_gris_cambia_el_hsl_pero_no_da_paso() {
    let gris = Rgba::opaque(128, 128, 128);
    let mut editor = editor_con(EditTarget::Entry(0), gris);
    assert!(editor.begin(Control::Hue, 100));
    assert_eq!(editor.release(), None, "el Rgba no cambió");
    let (color, hsl) = editor.shown().expect("hay destino");
    assert_eq!(color, gris);
    assert!(cerca(hsl.h(), 100.0), "h = {}", hsl.h());
    assert_eq!(matiz_mostrado(&editor), 100);
    // Un `sync` con el mismo color no recalcula el `Hsl`.
    editor.sync(Some((EditTarget::Entry(0), gris)));
    assert_eq!(matiz_mostrado(&editor), 100);
}

// Fila 6: `Hsl` propio.

fn azul_claro() -> Rgba {
    Rgba::opaque(0, 128, 255)
}

#[test]
fn fila_6_saturacion_a_cero_y_vuelta_recupera_el_color() {
    // La spec dice H 208 para este color, pero su matiz es 210 (209,88): se compara con
    // `Hsl::from_rgba`, que es lo que quiere decir.
    let base = azul_claro();
    let matiz = control_value(base, Hsl::from_rgba(base), Control::Hue);
    let destino = EditTarget::Entry(0);
    let mut editor = editor_con(destino, base);

    let commit = arrastrar(&mut editor, destino, Control::Saturation, &[0]);
    assert_eq!(
        commit,
        Some(Commit {
            target: destino,
            color: Rgba::opaque(128, 128, 128)
        })
    );
    assert_eq!(matiz_mostrado(&editor), matiz, "el matiz se conserva");
    let (_, hsl) = editor.shown().expect("hay destino");
    assert_eq!(
        control_value(Rgba::opaque(128, 128, 128), hsl, Control::Saturation),
        0
    );

    let commit = arrastrar(&mut editor, destino, Control::Saturation, &[100]);
    assert_eq!(
        commit,
        Some(Commit {
            target: destino,
            color: base
        })
    );
}

#[test]
fn fila_6_luminosidad_a_los_extremos_y_vuelta_al_50() {
    let base = azul_claro();
    let destino = EditTarget::Entry(0);
    let mut editor = editor_con(destino, base);

    let negro = arrastrar(&mut editor, destino, Control::Lightness, &[0]);
    assert_eq!(negro.map(|c| c.color), Some(Rgba::opaque(0, 0, 0)));
    let de_vuelta = arrastrar(&mut editor, destino, Control::Lightness, &[50]);
    assert_eq!(de_vuelta.map(|c| c.color), Some(base));

    let blanco = arrastrar(&mut editor, destino, Control::Lightness, &[100]);
    assert_eq!(blanco.map(|c| c.color), Some(Rgba::opaque(255, 255, 255)));
    let de_vuelta = arrastrar(&mut editor, destino, Control::Lightness, &[50]);
    assert_eq!(de_vuelta.map(|c| c.color), Some(base));
}

#[test]
fn fila_6_r_y_b_a_128_dejan_un_gris_con_el_matiz_del_color_intermedio() {
    let base = azul_claro();
    let destino = EditTarget::Entry(0);
    let mut editor = editor_con(destino, base);

    let primero = arrastrar(&mut editor, destino, Control::Red, &[128]);
    assert_eq!(primero.map(|c| c.color), Some(Rgba::opaque(128, 128, 255)));
    let segundo = arrastrar(&mut editor, destino, Control::Blue, &[128]);
    assert_eq!(segundo.map(|c| c.color), Some(Rgba::opaque(128, 128, 128)));

    let (color, hsl) = editor.shown().expect("hay destino");
    assert_eq!(color, Rgba::opaque(128, 128, 128));
    // El matiz es el de (128, 128, 255), 240, no el del color original.
    assert!(cerca(hsl.h(), 240.0), "h = {}", hsl.h());
    assert_eq!(control_value(color, hsl, Control::Saturation), 0);
}

#[test]
fn fila_6_otro_destino_y_volver_recalcula_con_from_rgba() {
    let gris = Rgba::opaque(128, 128, 128);
    let mut editor = editor_con(EditTarget::Entry(0), azul_claro());
    arrastrar(&mut editor, EditTarget::Entry(0), Control::Saturation, &[0]);
    assert_ne!(matiz_mostrado(&editor), 0);
    editor.sync(Some((EditTarget::Entry(1), VERDE)));
    assert_eq!(editor.shown(), Some((VERDE, Hsl::from_rgba(VERDE))));
    editor.sync(Some((EditTarget::Entry(0), gris)));
    // Un gris da H 0: pérdida esperada (decisión 8).
    assert_eq!(editor.shown(), Some((gris, Hsl::from_rgba(gris))));
    assert_eq!(matiz_mostrado(&editor), 0);
}

#[test]
fn fila_6_deshacer_el_color_recalcula_con_from_rgba_del_restaurado() {
    let base = azul_claro();
    let destino = EditTarget::Entry(0);
    let mut editor = editor_con(destino, base);
    arrastrar(&mut editor, destino, Control::Red, &[128]);
    assert!(cerca(editor.shown().unwrap().1.h(), 240.0));
    // `undo`: el siguiente `sync` ve el color original.
    editor.sync(Some((destino, base)));
    assert_eq!(editor.shown(), Some((base, Hsl::from_rgba(base))));
}

#[test]
fn fila_6_un_sync_con_el_mismo_color_no_recalcula_el_hsl() {
    // Un trazo del lápiz y su `undo` no cambian el color de la entrada.
    let base = azul_claro();
    let destino = EditTarget::Entry(0);
    let mut editor = editor_con(destino, base);
    let commit = arrastrar(&mut editor, destino, Control::Saturation, &[0]);
    let gris = commit.expect("cambió").color;
    let antes = editor.shown();
    editor.sync(Some((destino, gris)));
    editor.sync(Some((destino, gris)));
    assert_eq!(editor.shown(), antes);
    assert_ne!(matiz_mostrado(&editor), 0, "no se recalculó (daría 0)");
}

#[test]
fn reset_obliga_a_recalcular_el_hsl_en_el_siguiente_sync() {
    let gris = Rgba::opaque(128, 128, 128);
    let mut editor = editor_con(EditTarget::Entry(0), gris);
    editor.begin(Control::Hue, 100);
    editor.release();
    assert_eq!(matiz_mostrado(&editor), 100);
    editor.reset();
    assert_eq!(editor.shown(), None, "sin destino tras reset");
    assert!(!editor.dragging());
    editor.sync(Some((EditTarget::Entry(0), gris)));
    assert_eq!(editor.shown(), Some((gris, Hsl::from_rgba(gris))));
}

#[test]
fn sync_durante_un_arrastre_no_cambia_nada() {
    let mut editor = editor_con(EditTarget::Entry(0), ROJO);
    assert!(editor.begin(Control::Green, 200));
    let durante = editor.shown();
    editor.sync(Some((EditTarget::Entry(3), NEGRO)));
    editor.sync(None);
    assert_eq!(editor.shown(), durante);
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
fn el_provisional_va_de_un_valor_a_otro_sin_depender_del_camino() {
    // Fila 4 (parte del editor): `begin(Red, 255)`, `drag(128)`, `drag(255)` desde la entrada 1
    // (verde) da (255,255,0), (128,255,0) y otra vez amarillo.
    let mut editor = editor_con(EditTarget::Entry(1), VERDE);
    let provisional = |editor: &ColorEditor| editor.preview();
    assert!(editor.begin(Control::Red, 255));
    assert_eq!(provisional(&editor), Some((EditTarget::Entry(1), AMARILLO)));
    assert_eq!(editor.shown().map(|(c, _)| c), Some(AMARILLO));
    editor.drag(128);
    assert_eq!(
        provisional(&editor),
        Some((EditTarget::Entry(1), Rgba::opaque(128, 255, 0)))
    );
    editor.drag(255);
    assert_eq!(provisional(&editor), Some((EditTarget::Entry(1), AMARILLO)));
    assert_eq!(
        editor.release(),
        Some(Commit {
            target: EditTarget::Entry(1),
            color: AMARILLO
        })
    );
    assert_eq!(editor.preview(), None, "ya no hay arrastre vivo");
    assert!(!editor.dragging());
}

// Fila 7: hexadecimal de 3 o 6 dígitos, conserva el alfa.

#[test]
fn fila_7_tres_y_seis_digitos_conservan_el_alfa_actual() {
    let actual = rgba(1, 2, 3, 128);
    for texto in ["#f80", "f80", "#ff8800", "ff8800", "  #FF8800  ", "#fF8800"] {
        assert_eq!(
            parse_hex(texto, Mode::Rgba, actual),
            Ok(rgba(255, 136, 0, 128)),
            "{texto:?}"
        );
    }
}

#[test]
fn fila_7_con_alfa_actual_255() {
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
fn fila_7_gray_y_gray_alpha() {
    let gris = rgba(50, 50, 50, 255);
    assert_eq!(
        parse_hex("#808080", Mode::Gray, gris),
        Ok(rgba(128, 128, 128, 255))
    );
    let con_alfa = rgba(50, 50, 50, 64);
    assert_eq!(
        parse_hex("#808080", Mode::GrayAlpha, con_alfa),
        Ok(rgba(128, 128, 128, 64))
    );
}

#[test]
fn fila_7_teclear_00ff00_sobre_una_entrada_con_alfa_128_lo_conserva() {
    let base = rgba(255, 0, 0, 128);
    let mut editor = editor_con(EditTarget::Entry(0), base);
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
fn fila_8_cuatro_y_ocho_digitos_fijan_el_alfa_escrito() {
    let actual = rgba(1, 2, 3, 128);
    let casos = [
        ("#f808", rgba(255, 136, 0, 136)),
        ("f80f", rgba(255, 136, 0, 255)),
        ("#ff880080", rgba(255, 136, 0, 128)),
        ("ff8800ff", rgba(255, 136, 0, 255)),
        ("#ff880000", rgba(255, 136, 0, 0)),
    ];
    for (texto, esperado) in casos {
        assert_eq!(
            parse_hex(texto, Mode::Rgba, actual),
            Ok(esperado),
            "{texto:?}"
        );
    }
}

#[test]
fn fila_8_gray_alpha_y_gray() {
    let con_alfa = rgba(50, 50, 50, 64);
    assert_eq!(
        parse_hex("#80808010", Mode::GrayAlpha, con_alfa),
        Ok(rgba(128, 128, 128, 16))
    );
    let gris = rgba(50, 50, 50, 255);
    assert_eq!(
        parse_hex("#808080ff", Mode::Gray, gris),
        Ok(rgba(128, 128, 128, 255))
    );
    assert_eq!(
        parse_hex("#80808080", Mode::Gray, gris),
        Err(HexError::NoAlpha)
    );
}

#[test]
fn fila_8_teclear_00ff00ff_sobre_una_entrada_con_alfa_128_lo_pasa_a_255() {
    let base = rgba(255, 0, 0, 128);
    let mut editor = editor_con(EditTarget::Entry(0), base);
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

fn teclear(editor: &mut ColorEditor, destino: EditTarget, base: Rgba, texto: &str) {
    editor.type_hex(destino, base, texto.to_string());
}

#[test]
fn fila_9_un_valor_valido_con_intro_da_un_commit_y_borra_lo_pendiente() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "#00ff00");
    assert_eq!(editor.hex_display().as_deref(), Some("#00ff00"));
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Commit(Commit {
            target: destino,
            color: VERDE
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
fn fila_9_el_color_actual_da_unchanged() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "#0000ff");
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Unchanged
    );
    assert_eq!(editor.hex_display(), None);
}

#[test]
fn fila_9_unchanged_tambien_con_otra_forma_de_escribir_el_mismo_color() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "  00F  ");
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, true),
        HexOutcome::Unchanged
    );
}

#[test]
fn fila_9_escape_borra_lo_pendiente_sin_aviso_ni_paso() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "#00ff00");
    editor.escape_hex();
    assert_eq!(editor.hex_display(), None, "el campo vuelve a `#0000ff`");
    assert_eq!(editor.confirm_hex(Mode::Indexed, true), HexOutcome::Nothing);
    assert!(editor.notice(false).iter().all(|l| vacia(l)));
    assert_eq!(editor.shown().map(|(c, _)| c), Some(AZUL));
}

#[test]
fn fila_9_cambiar_de_muestra_en_el_mismo_fotograma_aplica_al_destino_original() {
    let original = EditTarget::Entry(2);
    let mut editor = editor_con(original, AZUL);
    teclear(&mut editor, original, AZUL, "#00ff00");
    // La muestra 3 (entrada 2 → entrada 3) se elige antes de perder el foco.
    editor.sync(Some((EditTarget::Entry(3), NEGRO)));
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, true),
        HexOutcome::Commit(Commit {
            target: original,
            color: VERDE
        })
    );
}

#[test]
fn fila_9_el_alfa_pendiente_es_el_del_color_base_original() {
    let original = EditTarget::Entry(2);
    let base = rgba(0, 0, 255, 128);
    let mut editor = editor_con(original, base);
    teclear(&mut editor, original, base, "#00ff00");
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
fn fila_9_enfocar_y_salir_sin_teclear_da_nothing() {
    let mut editor = editor_con(EditTarget::Entry(2), AZUL);
    assert_eq!(editor.confirm_hex(Mode::Indexed, true), HexOutcome::Nothing);
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Nothing
    );
    assert_eq!(editor.hex_display(), None);
}

#[test]
fn confirmar_aplica_las_reglas_del_modo() {
    let destino = EditTarget::Slot(4);
    let gris = Rgba::opaque(96, 96, 96);
    let mut editor = editor_con(destino, gris);
    teclear(&mut editor, destino, gris, "#ff0000");
    assert_eq!(
        editor.confirm_hex(Mode::Gray, false),
        HexOutcome::Invalid(HexError::NotGray)
    );
    teclear(&mut editor, destino, gris, "#808080");
    assert_eq!(
        editor.confirm_hex(Mode::Gray, false),
        HexOutcome::Commit(Commit {
            target: destino,
            color: Rgba::opaque(128, 128, 128)
        })
    );
}

// Fila 11 (parte pura): inválido con Intro, con foco perdido y con Escape.

#[test]
fn fila_11_intro_con_un_valor_invalido_conserva_el_texto_y_el_aviso() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "zzz");
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Invalid(HexError::Digit)
    );
    assert_eq!(editor.hex_display().as_deref(), Some("zzz"));
    let aviso = editor.notice(false);
    assert_eq!(sin_prefijo(&aviso[0]), "solo dígitos 0-9 y a-f");
    assert!(vacia(&aviso[1]), "el aviso del hexadecimal es de una línea");
    // El color no cambió.
    assert_eq!(editor.shown().map(|(c, _)| c), Some(AZUL));
}

#[test]
fn fila_11_teclear_otra_vez_quita_el_aviso() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "zzz");
    editor.confirm_hex(Mode::Indexed, false);
    teclear(&mut editor, destino, AZUL, "zzzz");
    assert!(editor.notice(false).iter().all(|l| vacia(l)));
    assert_eq!(editor.hex_display().as_deref(), Some("zzzz"));
    // Intro otra vez: el aviso vuelve.
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Invalid(HexError::Digit)
    );
    assert!(!vacia(&editor.notice(false)[0]));
}

#[test]
fn fila_11_perder_el_foco_con_un_valor_invalido_revierte_el_texto() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "zzz");
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
    assert!(editor.notice(false).iter().all(|l| vacia(l)));
    assert_eq!(editor.confirm_hex(Mode::Indexed, true), HexOutcome::Nothing);
}

#[test]
fn fila_11_perder_el_foco_sin_haber_pulsado_intro_tambien_revierte() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "zzz");
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, true),
        HexOutcome::Invalid(HexError::Digit)
    );
    assert_eq!(editor.hex_display(), None);
}

#[test]
fn fila_11_escape_revierte_el_texto_sin_aviso() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "zzz");
    editor.confirm_hex(Mode::Indexed, false);
    editor.escape_hex();
    assert_eq!(editor.hex_display(), None);
    assert!(editor.notice(false).iter().all(|l| vacia(l)));
    assert_eq!(editor.confirm_hex(Mode::Indexed, true), HexOutcome::Nothing);
}

#[test]
fn el_aviso_del_hexadecimal_invalido_dura_hasta_que_cambia_el_destino_o_el_color() {
    let destino = EditTarget::Entry(2);
    for nuevo in [
        Some((EditTarget::Entry(3), NEGRO)),
        Some((destino, VERDE)),
        None,
    ] {
        let mut editor = editor_con(destino, AZUL);
        teclear(&mut editor, destino, AZUL, "zzz");
        editor.confirm_hex(Mode::Indexed, false);
        // Mismo destino y color: sigue.
        editor.sync(Some((destino, AZUL)));
        assert!(!vacia(&editor.notice(false)[0]), "sin cambios sigue");
        assert_eq!(editor.hex_display().as_deref(), Some("zzz"));
        editor.sync(nuevo);
        assert!(
            editor.notice(false).iter().all(|l| vacia(l)),
            "cambió el destino o el color: {nuevo:?}"
        );
        assert_eq!(editor.hex_display(), None, "{nuevo:?}");
    }
}

#[test]
fn confirmar_uno_valido_tras_uno_invalido_quita_el_aviso() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "zzz");
    editor.confirm_hex(Mode::Indexed, false);
    teclear(&mut editor, destino, AZUL, "#00ff00");
    assert_eq!(
        editor.confirm_hex(Mode::Indexed, false),
        HexOutcome::Commit(Commit {
            target: destino,
            color: VERDE
        })
    );
    assert!(editor.notice(false).iter().all(|l| vacia(l)));
}

#[test]
fn sin_nada_que_avisar_el_aviso_esta_vacio() {
    let editor = editor_con(EditTarget::Entry(2), AZUL);
    assert!(editor.notice(false).iter().all(|l| vacia(l)));
}

// Fila 13 (b), parte pura: el aviso de la entrada transparente.

#[test]
fn fila_13_el_aviso_de_la_entrada_transparente_tiene_dos_lineas() {
    let editor = editor_con(EditTarget::Entry(2), AZUL);
    let aviso = editor.notice(true);
    assert_eq!(sin_prefijo(&aviso[0]), "entrada transparente");
    assert_eq!(aviso[1].trim(), "se guarda con alfa 0");
}

#[test]
fn el_aviso_de_hex_invalido_tapa_el_de_la_entrada_transparente() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "zzz");
    editor.confirm_hex(Mode::Indexed, false);
    let aviso = editor.notice(true);
    assert_eq!(sin_prefijo(&aviso[0]), "solo dígitos 0-9 y a-f");
    assert!(vacia(&aviso[1]));
}

#[test]
fn los_cinco_avisos_de_hexadecimal_caben_en_32_columnas() {
    let casos = [
        ("", Mode::Rgba, HexError::Empty),
        ("12345", Mode::Rgba, HexError::Length(5)),
        ("zzz", Mode::Rgba, HexError::Digit),
        ("#ff0000", Mode::Gray, HexError::NotGray),
        ("#80808080", Mode::Gray, HexError::NoAlpha),
    ];
    for (texto, modo, error) in casos {
        let destino = EditTarget::Slot(4);
        let base = Rgba::opaque(96, 96, 96);
        let mut editor = editor_con(destino, base);
        teclear(&mut editor, destino, base, texto);
        assert_eq!(
            editor.confirm_hex(modo, false),
            HexOutcome::Invalid(error.clone())
        );
        for linea in editor.notice(false) {
            assert!(linea.chars().count() <= 32, "{linea:?}");
            assert!(en_el_juego(&linea), "{linea:?}");
        }
        assert_eq!(sin_prefijo(&editor.notice(false)[0]), hex_notice(&error));
    }
}

// Fila 10: hexadecimal inválido (análisis).

#[test]
fn fila_10_vacio_es_empty() {
    let actual = Rgba::opaque(1, 2, 3);
    for texto in ["", "   ", "#", "  #  "] {
        assert_eq!(
            parse_hex(texto, Mode::Rgba, actual),
            Err(HexError::Empty),
            "{texto:?}"
        );
    }
}

#[test]
fn fila_10_longitud_antes_que_digitos() {
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
fn fila_10_digitos_invalidos() {
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
fn fila_10_reglas_de_gray_y_gray_alpha() {
    let gris = rgba(50, 50, 50, 255);
    assert_eq!(
        parse_hex("#ff0000", Mode::Gray, gris),
        Err(HexError::NotGray)
    );
    assert_eq!(
        parse_hex("#80808080", Mode::Gray, gris),
        Err(HexError::NoAlpha)
    );
    assert_eq!(
        parse_hex("#ff0000", Mode::GrayAlpha, rgba(50, 50, 50, 64)),
        Err(HexError::NotGray)
    );
    // `NoAlpha` va antes que `NotGray`.
    assert_eq!(
        parse_hex("#ff000080", Mode::Gray, gris),
        Err(HexError::NoAlpha)
    );
}

#[test]
fn fila_10_en_indexadas_cualquier_color_vale() {
    assert_eq!(
        parse_hex("#ff0000", Mode::Indexed, Rgba::opaque(1, 2, 3)),
        Ok(Rgba::opaque(255, 0, 0))
    );
}

#[test]
fn fila_10_los_avisos_caben_en_25_caracteres_y_en_el_juego_de_caracteres() {
    for error in [
        HexError::Empty,
        HexError::Length(0),
        HexError::Length(5),
        HexError::Length(1000),
        HexError::Digit,
        HexError::NotGray,
        HexError::NoAlpha,
    ] {
        let aviso = hex_notice(&error);
        assert!(aviso.chars().count() <= 25, "{aviso:?}");
        assert!(en_el_juego(aviso), "{aviso:?}");
        assert!(!aviso.is_empty());
    }
}

// Fila 12: alfa y extremos.

#[test]
fn fila_12_alfa_a_cero_se_confirma_y_se_ve_con_hex_de_ocho_digitos() {
    let destino = EditTarget::Entry(0);
    let mut editor = editor_con(destino, ROJO);
    assert!(editor.begin(Control::Alpha, 0));
    let (provisional, _) = editor.shown().expect("hay destino");
    assert_eq!(provisional, rgba(255, 0, 0, 0));
    assert_eq!(hex_text(provisional), "#ff000000");
    assert_eq!(
        editor.release(),
        Some(Commit {
            target: destino,
            color: rgba(255, 0, 0, 0)
        })
    );
}

#[test]
fn fila_12_alfa_255_es_el_valor_de_partida_y_no_da_paso() {
    let mut editor = editor_con(EditTarget::Entry(0), ROJO);
    assert!(editor.begin(Control::Alpha, 255));
    assert_eq!(editor.release(), None);
}

#[test]
fn fila_12_alfa_128() {
    let mut editor = editor_con(EditTarget::Entry(0), ROJO);
    assert!(editor.begin(Control::Alpha, 128));
    let (provisional, _) = editor.shown().expect("hay destino");
    assert_eq!(hex_text(provisional), "#ff000080");
    assert_eq!(
        editor.release().map(|c| c.color),
        Some(rgba(255, 0, 0, 128))
    );
}

#[test]
fn fila_12_un_valor_por_encima_del_maximo_se_limita() {
    let bases = [
        (ROJO, Hsl::from_rgba(ROJO)),
        (azul_claro(), Hsl::from_rgba(azul_claro())),
        (rgba(10, 200, 30, 90), Hsl::from_rgba(rgba(10, 200, 30, 90))),
    ];
    for base in bases {
        for control in CONTROLES {
            let esperado = apply_control(base, control, control.max());
            assert_eq!(apply_control(base, control, 999), esperado, "{control:?}");
            assert_eq!(
                apply_control(base, control, u32::MAX),
                esperado,
                "{control:?}"
            );
        }
    }
    // Los máximos de la spec: 255, 359 y 100.
    let base = (ROJO, Hsl::from_rgba(ROJO));
    assert_eq!(apply_control(base, Control::Green, 999).0.g, 255);
    let (_, hsl) = apply_control(base, Control::Hue, 999);
    assert!(cerca(hsl.h(), 359.0), "h = {}", hsl.h());
    let (_, hsl) = apply_control(base, Control::Saturation, 999);
    assert!(cerca(hsl.s(), 1.0));
    let (_, hsl) = apply_control(base, Control::Lightness, 999);
    assert!(cerca(hsl.l(), 1.0));
}

fn extremo(control: Control, valor: u32) -> (Rgba, Hsl) {
    let rojo = (ROJO, Hsl::from_rgba(ROJO));
    let (color, hsl) = apply_control(rojo, control, valor);
    assert_eq!(hsl.to_rgba(color.a), color, "{control:?} {valor}");
    (color, hsl)
}

#[test]
fn fila_12_extremos_desde_rojo_opaco() {
    assert_eq!(extremo(Control::Red, 0).0, Rgba::opaque(0, 0, 0));
    assert_eq!(extremo(Control::Green, 255).0, Rgba::opaque(255, 255, 0));
    assert_eq!(extremo(Control::Blue, 255).0, Rgba::opaque(255, 0, 255));
    assert_eq!(
        extremo(Control::Saturation, 0).0,
        Rgba::opaque(128, 128, 128)
    );
    assert_eq!(extremo(Control::Lightness, 0).0, Rgba::opaque(0, 0, 0));
    assert_eq!(
        extremo(Control::Lightness, 100).0,
        Rgba::opaque(255, 255, 255)
    );
    // Extremos que no cambian el color pero sí el campo: H 0 y 359, S 100.
    assert_eq!(extremo(Control::Hue, 0).0, ROJO);
    let (color, hsl) = extremo(Control::Hue, 359);
    assert!(cerca(hsl.h(), 359.0));
    assert_eq!((color.r, color.g, color.a), (255, 0, 255));
    assert_eq!(extremo(Control::Saturation, 100).0, ROJO);
    // Cada control (salvo el de alfa) a su máximo y a 0 conserva el alfa.
    for control in CONTROLES.into_iter().filter(|c| *c != Control::Alpha) {
        for valor in [0, control.max()] {
            assert_eq!(extremo(control, valor).0.a, 255, "{control:?}");
        }
    }
}

#[test]
fn fila_12_gray_alpha() {
    let gris = Rgba::opaque(128, 128, 128);
    let base = (gris, Hsl::from_rgba(gris));
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
fn hsl_after_rgb_de_un_color_cromatico_es_from_rgba() {
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
fn hsl_after_rgb_de_un_gris_conserva_el_matiz_anterior() {
    let anterior = Hsl::new(120.0, 0.5, 0.5);
    let gris = Rgba::opaque(128, 128, 128);
    let nuevo = hsl_after_rgb(anterior, gris);
    assert!(cerca(nuevo.h(), 120.0), "h = {}", nuevo.h());
    assert!(cerca(nuevo.s(), 0.0));
    assert!(cerca(nuevo.l(), Hsl::from_rgba(gris).l()));
    assert_eq!(nuevo.to_rgba(255), gris);
}

#[test]
fn hsl_after_rgb_de_negro_y_blanco_conserva_matiz_y_saturacion() {
    let anterior = Hsl::new(120.0, 0.5, 0.5);
    for (color, luz) in [
        (Rgba::opaque(0, 0, 0), 0.0),
        (Rgba::opaque(255, 255, 255), 1.0),
    ] {
        let nuevo = hsl_after_rgb(anterior, color);
        assert!(cerca(nuevo.h(), 120.0), "h = {}", nuevo.h());
        assert!(cerca(nuevo.s(), 0.5), "s = {}", nuevo.s());
        assert!(cerca(nuevo.l(), luz));
        assert_eq!(nuevo.to_rgba(255), color);
    }
}

// Propiedad 1: conversión de controles.

/// Base coherente: `hsl.to_rgba(a) == rgba` y, en Gray y GrayAlpha, `r == g == b` (en Gray, `a`
/// 255).
fn construir_base(
    modo: Mode,
    canales: (u8, u8, u8, u8),
    hsl_raw: (f32, f32, f32),
    variante: bool,
) -> Option<(Rgba, Hsl)> {
    let (r, g, b, a) = canales;
    let alfa = if modo == Mode::Gray { 255 } else { a };
    let gris = matches!(modo, Mode::Gray | Mode::GrayAlpha);
    let (matiz, s, l) = hsl_raw;
    let (rgba, hsl) = if variante {
        let saturacion = if gris { 0.0 } else { s };
        let hsl = Hsl::new(matiz, saturacion, l);
        (hsl.to_rgba(alfa), hsl)
    } else {
        let color = if gris {
            Rgba::new(r, r, r, alfa)
        } else {
            Rgba::new(r, g, b, alfa)
        };
        let hsl = Hsl::from_rgba(color);
        // Un gris conserva un matiz cualquiera, como tras editar un color.
        let hsl = if color.r == color.g && color.g == color.b {
            Hsl::new(matiz, 0.0, hsl.l())
        } else {
            hsl
        };
        (color, hsl)
    };
    (hsl.to_rgba(rgba.a) == rgba).then_some((rgba, hsl))
}

fn modo_s() -> impl Strategy<Value = Mode> {
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

fn comprobar_control(
    modo: Mode,
    base: (Rgba, Hsl),
    control: Control,
    valor: u32,
) -> Result<(), TestCaseError> {
    let (rgba0, hsl0) = base;
    let (nuevo, hsl) = apply_control(base, control, valor);
    let limitado = valor.min(control.max());
    let canal = limitado as u8;
    // El resultado solo depende de `(base, control, valor)`.
    prop_assert_eq!(apply_control(base, control, valor), (nuevo, hsl));
    // El alfa solo cambia con el control de alfa.
    if control != Control::Alpha {
        prop_assert_eq!(nuevo.a, rgba0.a);
    }
    match control {
        Control::Red => {
            prop_assert_eq!((nuevo.r, nuevo.g, nuevo.b), (canal, rgba0.g, rgba0.b));
        }
        Control::Green => {
            prop_assert_eq!((nuevo.r, nuevo.g, nuevo.b), (rgba0.r, canal, rgba0.b));
        }
        Control::Blue => {
            prop_assert_eq!((nuevo.r, nuevo.g, nuevo.b), (rgba0.r, rgba0.g, canal));
        }
        Control::Gray => {
            prop_assert_eq!((nuevo.r, nuevo.g, nuevo.b), (canal, canal, canal));
        }
        Control::Alpha => {
            prop_assert_eq!((nuevo.r, nuevo.g, nuevo.b), (rgba0.r, rgba0.g, rgba0.b));
            prop_assert_eq!(nuevo.a, canal);
            // El `Hsl` no se toca con el alfa.
            prop_assert_eq!(hsl, hsl0);
        }
        Control::Hue => {
            prop_assert!(cerca(hsl.h(), limitado as f32), "h = {}", hsl.h());
            prop_assert_eq!(hsl.s(), hsl0.s());
            prop_assert_eq!(hsl.l(), hsl0.l());
        }
        Control::Saturation => {
            prop_assert!(cerca(hsl.s(), limitado as f32 / 100.0), "s = {}", hsl.s());
            prop_assert_eq!(hsl.h(), hsl0.h());
            prop_assert_eq!(hsl.l(), hsl0.l());
        }
        Control::Lightness => {
            prop_assert!(cerca(hsl.l(), limitado as f32 / 100.0), "l = {}", hsl.l());
            prop_assert_eq!(hsl.h(), hsl0.h());
            prop_assert_eq!(hsl.s(), hsl0.s());
        }
    }
    // El `Hsl` siempre es coherente con el `Rgba`.
    prop_assert_eq!(hsl.to_rgba(nuevo.a), nuevo);
    if matches!(
        control,
        Control::Hue | Control::Saturation | Control::Lightness
    ) {
        // `Hsl::to_rgba(alfa)`: el `Rgba` sale del `Hsl` nuevo.
        prop_assert_eq!(nuevo, hsl.to_rgba(rgba0.a));
    }
    if matches!(modo, Mode::Gray | Mode::GrayAlpha) {
        prop_assert!(nuevo.r == nuevo.g && nuevo.g == nuevo.b);
    }
    Ok(())
}

proptest! {
    #[test]
    fn prop_1_conversion_de_controles(
        modo in modo_s(),
        canales in (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()),
        hsl_raw in (0.0f32..360.0, 0.0f32..=1.0, 0.0f32..=1.0),
        variante in any::<bool>(),
        indice in 0usize..8,
        valor in valor_s(),
    ) {
        let Some(base) = construir_base(modo, canales, hsl_raw, variante) else {
            return Ok(());
        };
        let disponibles = controls(modo);
        let control = disponibles[indice % disponibles.len()];
        comprobar_control(modo, base, control, valor)?;
    }

    #[test]
    fn prop_1_cualquier_control_sin_panico_aunque_el_modo_no_lo_tenga(
        canales in (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()),
        indice in 0usize..8,
        valor in valor_s(),
    ) {
        let color = Rgba::new(canales.0, canales.1, canales.2, canales.3);
        let base = (color, Hsl::from_rgba(color));
        let control = CONTROLES[indice];
        let (nuevo, hsl) = apply_control(base, control, valor);
        prop_assert_eq!(hsl.to_rgba(nuevo.a), nuevo);
    }

    #[test]
    fn prop_1_slider_value_at_es_monotona_y_llega_a_los_extremos(
        ancho in 2i64..2000,
        maximo in prop_oneof![Just(100u32), Just(255u32), Just(359u32)],
        x0 in -100i64..2100,
        x1 in -100i64..2100,
    ) {
        let (menor, mayor) = (x0.min(x1), x0.max(x1));
        let a = slider_value_at(menor, ancho, maximo);
        let b = slider_value_at(mayor, ancho, maximo);
        prop_assert!(a <= b);
        prop_assert!(b <= maximo);
        prop_assert_eq!(slider_value_at(0, ancho, maximo), 0);
        prop_assert_eq!(slider_value_at(ancho - 1, ancho, maximo), maximo);
        prop_assert_eq!(slider_value_at(i64::MIN / 4, ancho, maximo), 0);
        prop_assert_eq!(slider_value_at(i64::MAX / 4, ancho, maximo), maximo);
    }

    #[test]
    fn prop_1_slider_x_de_slider_value_at_queda_cerca_de_x(
        ancho in 2i64..2000,
        maximo in prop_oneof![Just(100u32), Just(255u32), Just(359u32)],
        x_raw in 0i64..2000,
    ) {
        let x = x_raw % ancho;
        let valor = slider_value_at(x, ancho, maximo);
        let vuelta = slider_x(valor, ancho, maximo);
        // `⌈(w − 1)/max⌉`.
        let tolerancia = (ancho - 1 + i64::from(maximo) - 1) / i64::from(maximo);
        prop_assert!(
            (vuelta - x).abs() <= tolerancia,
            "x = {x}, valor = {valor}, vuelta = {vuelta}, tolerancia = {tolerancia}"
        );
    }

    /// Estabilidad: bajar S (o L) al extremo, soltar y volver al valor inicial devuelve el color de
    /// partida. Los colores de partida tienen matiz, S y L enteros (los del deslizador).
    #[test]
    fn prop_1_estabilidad_de_saturacion_y_luminosidad(
        matiz in 0u32..360,
        s in 0u32..=100,
        l in 0u32..=100,
        alfa in any::<u8>(),
        extremo in prop_oneof![
            Just((Control::Saturation, 0u32)),
            Just((Control::Lightness, 0u32)),
            Just((Control::Lightness, 100u32)),
        ],
    ) {
        let hsl = Hsl::new(matiz as f32, s as f32 / 100.0, l as f32 / 100.0);
        let color = hsl.to_rgba(alfa);
        let base = (color, hsl);
        let (control, valor) = extremo;
        let inicial = control_value(color, hsl, control);
        let (bajado, hsl_bajado) = apply_control(base, control, valor);
        let (vuelto, _) = apply_control((bajado, hsl_bajado), control, inicial);
        prop_assert_eq!(vuelto, color);
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
enum Estado {
    Reposo,
    Vivo {
        control: Control,
        base: (Rgba, Hsl),
        provisional: (Rgba, Hsl),
    },
    Cancelado,
}

struct Modelo {
    destino: Option<EditTarget>,
    rgba: Rgba,
    hsl: Hsl,
    estado: Estado,
}

impl Modelo {
    fn shown(&self) -> Option<(Rgba, Hsl)> {
        self.destino?;
        Some(match self.estado {
            Estado::Vivo { provisional, .. } => provisional,
            _ => (self.rgba, self.hsl),
        })
    }

    fn preview(&self) -> Option<(EditTarget, Rgba)> {
        match self.estado {
            Estado::Vivo { provisional, .. } => Some((self.destino?, provisional.0)),
            _ => None,
        }
    }

    fn sync(&mut self, actual: Option<(EditTarget, Rgba)>) {
        match actual {
            None => self.destino = None,
            Some((destino, color)) => {
                if self.destino != Some(destino) || self.rgba != color {
                    self.hsl = Hsl::from_rgba(color);
                    self.rgba = color;
                }
                self.destino = Some(destino);
            }
        }
    }
}

fn destino_s() -> impl Strategy<Value = EditTarget> {
    prop_oneof![
        (0u8..3).prop_map(EditTarget::Entry),
        (1u8..3).prop_map(EditTarget::Slot)
    ]
}

fn color_s() -> impl Strategy<Value = Rgba> {
    prop_oneof![
        Just(ROJO),
        Just(Rgba::opaque(128, 128, 128)),
        Just(azul_claro()),
        Just(rgba(10, 20, 30, 40)),
        (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
            .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a)),
    ]
}

fn op_s() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0usize..8, valor_s()).prop_map(|(i, v)| Op::Begin(CONTROLES[i], v)),
        3 => valor_s().prop_map(Op::Drag),
        3 => Just(Op::Release),
        2 => Just(Op::Cancel),
        2 => Just(Op::SyncActual),
        2 => proptest::option::weighted(0.8, (destino_s(), color_s())).prop_map(Op::Sync),
    ]
}

proptest! {
    #[test]
    fn prop_2_maquina_del_arrastre(ops in vec(op_s(), 1..60)) {
        let mut editor = ColorEditor::default();
        let mut modelo = Modelo {
            destino: None,
            rgba: Rgba::TRANSPARENT,
            hsl: Hsl::default(),
            estado: Estado::Reposo,
        };
        for op in ops {
            let cancelado = matches!(modelo.estado, Estado::Cancelado);
            match op {
                // Con un arrastre cancelado a medias solo se admiten `drag` y `release`.
                Op::Begin(..) | Op::Cancel | Op::SyncActual | Op::Sync(_) if cancelado => {
                    continue;
                }
                Op::Begin(control, valor) => {
                    let aceptado = editor.begin(control, valor);
                    let esperado = modelo.destino.is_some()
                        && matches!(modelo.estado, Estado::Reposo);
                    prop_assert_eq!(aceptado, esperado);
                    if esperado {
                        let base = (modelo.rgba, modelo.hsl);
                        modelo.estado = Estado::Vivo {
                            control,
                            base,
                            provisional: apply_control(base, control, valor),
                        };
                    }
                }
                Op::Drag(valor) => {
                    editor.drag(valor);
                    if let Estado::Vivo { control, base, .. } = modelo.estado {
                        modelo.estado = Estado::Vivo {
                            control,
                            base,
                            provisional: apply_control(base, control, valor),
                        };
                    }
                }
                Op::Release => {
                    let commit = editor.release();
                    let esperado = match modelo.estado {
                        Estado::Vivo { base, provisional, .. } => {
                            modelo.rgba = provisional.0;
                            modelo.hsl = provisional.1;
                            (provisional.0 != base.0).then(|| Commit {
                                target: modelo.destino.expect("hay destino"),
                                color: provisional.0,
                            })
                        }
                        _ => None,
                    };
                    modelo.estado = Estado::Reposo;
                    prop_assert_eq!(commit, esperado);
                }
                Op::Cancel => {
                    editor.cancel();
                    if matches!(modelo.estado, Estado::Vivo { .. }) {
                        modelo.estado = Estado::Cancelado;
                    }
                }
                Op::SyncActual => {
                    let actual = modelo.destino.map(|d| (d, modelo.rgba));
                    let antes = editor.shown();
                    editor.sync(actual);
                    modelo.sync(actual);
                    if matches!(modelo.estado, Estado::Vivo { .. }) {
                        prop_assert_eq!(editor.shown(), antes, "sync durante el arrastre");
                    }
                }
                Op::Sync(actual) => {
                    let antes = editor.shown();
                    editor.sync(actual);
                    if matches!(modelo.estado, Estado::Vivo { .. }) {
                        // No hace nada con arrastre en curso.
                        prop_assert_eq!(editor.shown(), antes, "sync durante el arrastre");
                    } else {
                        modelo.sync(actual);
                    }
                }
            }
            prop_assert_eq!(editor.shown(), modelo.shown());
            prop_assert_eq!(editor.preview(), modelo.preview());
            match modelo.estado {
                Estado::Vivo { .. } => prop_assert!(editor.dragging()),
                Estado::Reposo => prop_assert!(!editor.dragging()),
                Estado::Cancelado => {}
            }
        }
    }
}

// Propiedad 3: hexadecimal.

proptest! {
    #[test]
    fn prop_3_ida_y_vuelta_con_hex_text(
        r in any::<u8>(),
        g in any::<u8>(),
        b in any::<u8>(),
        a in any::<u8>(),
        gris in any::<u8>(),
    ) {
        let color = Rgba::new(r, g, b, a);
        for modo in [Mode::Rgba, Mode::Indexed] {
            prop_assert_eq!(parse_hex(&hex_text(color), modo, color), Ok(color));
        }
        let gris_con_alfa = Rgba::new(gris, gris, gris, a);
        prop_assert_eq!(
            parse_hex(&hex_text(gris_con_alfa), Mode::GrayAlpha, gris_con_alfa),
            Ok(gris_con_alfa)
        );
        let gris_opaco = Rgba::opaque(gris, gris, gris);
        prop_assert_eq!(
            parse_hex(&hex_text(gris_opaco), Mode::Gray, gris_opaco),
            Ok(gris_opaco)
        );
    }
}

/// Oráculo del hexadecimal, escrito a mano (sin `Rgba::from_hex`): recorta espacios, quita un `#`,
/// comprueba 3, 4, 6 u 8 dígitos ASCII, los lee, y aplica las reglas del modo.
fn oraculo(texto: &str, modo: Mode, actual: Rgba) -> Result<Rgba, HexError> {
    let recortado = texto.trim_matches(' ');
    let digitos = recortado.strip_prefix('#').unwrap_or(recortado);
    let caracteres: Vec<char> = digitos.chars().collect();
    if caracteres.is_empty() {
        return Err(HexError::Empty);
    }
    let largo = caracteres.len();
    if !matches!(largo, 3 | 4 | 6 | 8) {
        return Err(HexError::Length(largo));
    }
    let mut valores = Vec::new();
    for c in &caracteres {
        let valor = match c {
            '0'..='9' => *c as u8 - b'0',
            'a'..='f' => *c as u8 - b'a' + 10,
            'A'..='F' => *c as u8 - b'A' + 10,
            _ => return Err(HexError::Digit),
        };
        valores.push(valor);
    }
    let canal = |i: usize| -> u8 {
        if largo <= 4 {
            valores[i] * 17
        } else {
            valores[2 * i] * 16 + valores[2 * i + 1]
        }
    };
    let escribe_alfa = largo == 4 || largo == 8;
    let alfa = if escribe_alfa { canal(3) } else { actual.a };
    let color = Rgba::new(canal(0), canal(1), canal(2), alfa);
    let es_gris = color.r == color.g && color.g == color.b;
    match modo {
        Mode::Gray => {
            if escribe_alfa && alfa != 255 {
                return Err(HexError::NoAlpha);
            }
            if !es_gris {
                return Err(HexError::NotGray);
            }
        }
        Mode::GrayAlpha if !es_gris => return Err(HexError::NotGray),
        _ => {}
    }
    Ok(color)
}

fn caracter_s() -> impl Strategy<Value = char> {
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

fn texto_hex_s() -> impl Strategy<Value = String> {
    let cuerpo = vec(caracter_s(), 0..=10).prop_map(|cs| {
        let texto: String = cs.into_iter().collect();
        texto.trim_matches(' ').to_string()
    });
    let valido = (
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
        prop_oneof![cuerpo, valido],
        0usize..=1,
    )
        .prop_map(|(izquierda, almohadilla, cuerpo, derecha)| {
            format!(
                "{}{}{}{}",
                " ".repeat(izquierda),
                if almohadilla { "#" } else { "" },
                cuerpo,
                " ".repeat(derecha)
            )
        })
}

proptest! {
    #[test]
    fn prop_3_parse_hex_coincide_con_el_oraculo_y_los_avisos_caben(
        texto in texto_hex_s(),
        modo in modo_s(),
        canales in (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()),
    ) {
        prop_assume!(texto.chars().count() <= 12);
        // En Gray el color que se edita es opaco.
        let alfa = if modo == Mode::Gray { 255 } else { canales.3 };
        let actual = Rgba::new(canales.0, canales.1, canales.2, alfa);
        let obtenido = parse_hex(&texto, modo, actual);
        prop_assert_eq!(&obtenido, &oraculo(&texto, modo, actual), "{:?}", texto);
        if let Err(error) = obtenido {
            let aviso = hex_notice(&error);
            prop_assert!(aviso.chars().count() <= 25, "{aviso:?}");
            prop_assert!(en_el_juego(aviso), "{aviso:?}");
        }
    }

    #[test]
    fn prop_3_cualquier_texto_unicode_no_da_panico(
        texto in "\\PC{0,12}",
        modo in modo_s(),
        canales in (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()),
    ) {
        let actual = Rgba::new(canales.0, canales.1, canales.2, canales.3);
        if let Err(error) = parse_hex(&texto, modo, actual) {
            let aviso = hex_notice(&error);
            prop_assert!(aviso.chars().count() <= 25, "{aviso:?}");
            prop_assert!(en_el_juego(aviso), "{aviso:?}");
        }
    }
}

#[test]
fn el_oraculo_de_la_propiedad_3_acierta_los_ejemplos_de_la_spec() {
    // Guarda del propio oráculo: que no se separe de la spec.
    let actual = rgba(1, 2, 3, 128);
    assert_eq!(
        oraculo("#f80", Mode::Rgba, actual),
        Ok(rgba(255, 136, 0, 128))
    );
    assert_eq!(
        oraculo("f80f", Mode::Rgba, actual),
        Ok(rgba(255, 136, 0, 255))
    );
    assert_eq!(oraculo("ñññ", Mode::Rgba, actual), Err(HexError::Digit));
    assert_eq!(oraculo("#", Mode::Rgba, actual), Err(HexError::Empty));
    assert_eq!(
        oraculo("#80808080", Mode::Gray, Rgba::opaque(0, 0, 0)),
        Err(HexError::NoAlpha)
    );
}

// Texto pendiente del campo: `has_pending`, `confirm_hex_checked` y los avisos
// (specs/hito-2-panel-ventana-baja.md, filas 7, 8 y 9 en su parte pura).

fn hecho(resuelto: Resolved) -> HexOutcome {
    match resuelto {
        Resolved::Done(resultado) => resultado,
        Resolved::Stale(motivo) => panic!("se esperaba Done y fue Stale({motivo:?})"),
    }
}

fn descartado(resuelto: Resolved) -> StaleReason {
    match resuelto {
        Resolved::Stale(motivo) => motivo,
        Resolved::Done(_) => panic!("se esperaba Stale y fue Done"),
    }
}

fn commit_a(target: EditTarget, color: Rgba) -> HexOutcome {
    HexOutcome::Commit(Commit { target, color })
}

#[test]
fn has_pending_sigue_a_type_hex_y_a_lo_que_lo_borra() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    assert!(!editor.has_pending(), "recién creado");

    // Teclear lo deja pendiente, aunque el texto aún no sea válido.
    teclear(&mut editor, destino, AZUL, "#00");
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
    teclear(&mut editor, destino, AZUL, "#00ff00");
    assert!(editor.has_pending());
    editor.confirm_hex(Mode::Indexed, false);
    assert!(!editor.has_pending());

    teclear(&mut editor, destino, AZUL, "#00ff00");
    editor.escape_hex();
    assert!(!editor.has_pending());

    teclear(&mut editor, destino, AZUL, "#00ff00");
    editor.reset();
    assert!(!editor.has_pending());
    assert_eq!(editor.hex_display(), None);
}

#[test]
fn has_pending_coincide_con_hex_display() {
    let destino = EditTarget::Entry(1);
    let mut editor = editor_con(destino, VERDE);
    assert_eq!(editor.has_pending(), editor.hex_display().is_some());
    teclear(&mut editor, destino, VERDE, "f");
    assert_eq!(editor.has_pending(), editor.hex_display().is_some());
    assert!(editor.has_pending());
}

// Fila 7: el texto válido sobre una entrada que sigue igual se aplica.

#[test]
fn fila_7_una_entrada_que_sigue_igual_aplica_el_texto_al_destino_original() {
    for blur in [true, false] {
        let destino = EditTarget::Entry(2);
        let mut editor = editor_con(destino, AZUL);
        teclear(&mut editor, destino, AZUL, "#00ff00");
        let resuelto = editor.confirm_hex_checked(Mode::Indexed, blur, |_| Some(AZUL));
        assert_eq!(hecho(resuelto), commit_a(destino, VERDE), "blur {blur}");
        assert!(!editor.has_pending(), "blur {blur}");
        assert_eq!(editor.hex_display(), None);
        assert!(editor.notice(false).iter().all(|l| vacia(l)));
    }
}

#[test]
fn fila_7_consulta_el_guardado_del_destino_original_no_el_actual() {
    // Entre teclear y perder el foco la elección pasó a la entrada 3.
    let original = EditTarget::Entry(2);
    let mut editor = editor_con(original, AZUL);
    teclear(&mut editor, original, AZUL, "#00ff00");
    editor.sync(Some((EditTarget::Entry(3), NEGRO)));
    let preguntado = Cell::new(None);
    let resuelto = editor.confirm_hex_checked(Mode::Indexed, true, |destino| {
        preguntado.set(Some(destino));
        Some(AZUL)
    });
    assert_eq!(hecho(resuelto), commit_a(original, VERDE));
    assert_eq!(
        preguntado.get(),
        Some(original),
        "se pregunta por el destino original"
    );
}

#[test]
fn fila_7_con_el_mismo_color_dice_unchanged_y_sin_pendiente_nothing() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "#0000ff");
    let resuelto = editor.confirm_hex_checked(Mode::Indexed, true, |_| Some(AZUL));
    assert_eq!(hecho(resuelto), HexOutcome::Unchanged);
    assert!(!editor.has_pending());

    // Sin nada pendiente no hay nada que descartar.
    let resuelto = editor.confirm_hex_checked(Mode::Indexed, true, |_| Some(AZUL));
    assert_eq!(hecho(resuelto), HexOutcome::Nothing);
    let resuelto = editor.confirm_hex_checked(Mode::Indexed, false, |_| None);
    assert_eq!(hecho(resuelto), HexOutcome::Nothing);
}

#[test]
fn fila_7_conserva_el_alfa_del_color_base_y_vale_con_una_ranura() {
    let destino = EditTarget::Slot(4);
    let base = rgba(10, 20, 30, 128);
    let mut editor = editor_con(destino, base);
    teclear(&mut editor, destino, base, "#00ff00");
    let resuelto = editor.confirm_hex_checked(Mode::Rgba, true, |d| {
        assert_eq!(d, destino);
        Some(base)
    });
    assert_eq!(hecho(resuelto), commit_a(destino, rgba(0, 255, 0, 128)));
}

#[test]
fn fila_7_un_texto_invalido_sobre_una_entrada_que_sigue_igual_es_el_de_confirm_hex() {
    let destino = EditTarget::Entry(2);
    // Perder el foco: invalido, revierte.
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "zzz");
    let resuelto = editor.confirm_hex_checked(Mode::Indexed, true, |_| Some(AZUL));
    assert_eq!(hecho(resuelto), HexOutcome::Invalid(HexError::Digit));
    assert!(!editor.has_pending());
    // Intro: conserva el texto y el aviso.
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "zzz");
    let resuelto = editor.confirm_hex_checked(Mode::Indexed, false, |_| Some(AZUL));
    assert_eq!(hecho(resuelto), HexOutcome::Invalid(HexError::Digit));
    assert!(editor.has_pending());
    assert_eq!(editor.hex_display().as_deref(), Some("zzz"));
}

// Fila 8: la entrada ya no existe.

#[test]
fn fila_8_si_la_entrada_ya_no_existe_se_descarta_sin_commit() {
    for blur in [true, false] {
        let destino = EditTarget::Entry(9);
        let mut editor = editor_con(destino, AZUL);
        teclear(&mut editor, destino, AZUL, "#00ff00");
        let resuelto = editor.confirm_hex_checked(Mode::Indexed, blur, |_| None);
        assert_eq!(descartado(resuelto), StaleReason::Gone, "blur {blur}");
        assert!(!editor.has_pending(), "blur {blur}");
        assert_eq!(editor.hex_display(), None, "blur {blur}");
        // No deja aviso de hexadecimal en el editor: el aviso va a la barra.
        assert!(editor.notice(false).iter().all(|l| vacia(l)), "blur {blur}");
    }
}

#[test]
fn fila_8_la_entrada_ausente_gana_a_un_texto_invalido() {
    let destino = EditTarget::Entry(9);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "zzz");
    let resuelto = editor.confirm_hex_checked(Mode::Indexed, false, |_| None);
    assert_eq!(descartado(resuelto), StaleReason::Gone);
    assert!(!editor.has_pending(), "ni con Intro se conserva");
    assert!(editor.notice(false).iter().all(|l| vacia(l)));
}

#[test]
fn fila_8_vale_tambien_para_una_ranura_sin_color_guardado() {
    let destino = EditTarget::Slot(4);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "#00ff00");
    let resuelto = editor.confirm_hex_checked(Mode::Rgba, true, |_| None);
    assert_eq!(descartado(resuelto), StaleReason::Gone);
    assert!(!editor.has_pending());
}

// Fila 9: el color guardado cambió.

#[test]
fn fila_9_si_el_color_guardado_cambio_se_descarta_sin_commit() {
    for blur in [true, false] {
        let destino = EditTarget::Entry(2);
        let mut editor = editor_con(destino, AZUL);
        teclear(&mut editor, destino, AZUL, "#00ff00");
        let resuelto = editor.confirm_hex_checked(Mode::Indexed, blur, |_| Some(ROJO));
        assert_eq!(descartado(resuelto), StaleReason::Changed, "blur {blur}");
        assert!(!editor.has_pending(), "blur {blur}");
        assert_eq!(editor.hex_display(), None, "blur {blur}");
    }
}

#[test]
fn fila_9_cambia_el_color_pero_tambien_si_solo_cambia_el_alfa() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "#00ff00");
    let cambiado = rgba(0, 0, 255, 128);
    let resuelto = editor.confirm_hex_checked(Mode::Indexed, true, |_| Some(cambiado));
    assert_eq!(descartado(resuelto), StaleReason::Changed);
}

#[test]
fn fila_9_un_texto_invalido_sobre_un_color_cambiado_tambien_se_descarta() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "zzz");
    let resuelto = editor.confirm_hex_checked(Mode::Indexed, false, |_| Some(ROJO));
    assert_eq!(descartado(resuelto), StaleReason::Changed);
    assert!(!editor.has_pending());
    assert!(editor.notice(false).iter().all(|l| vacia(l)));
}

#[test]
fn fila_9_cambia_la_ranura_tras_teclear_y_el_pendiente_sobrevive_al_sync_hasta_resolverlo() {
    // Deshacer o rehacer, o cambiar la ranura, hacen que `sync` vea otro color; el texto sin
    // error sigue pendiente hasta que el campo lo resuelva.
    let destino = EditTarget::Slot(4);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "#00ff00");
    editor.sync(Some((destino, ROJO)));
    assert!(
        editor.has_pending(),
        "el sync no resuelve el texto pendiente"
    );
    let resuelto = editor.confirm_hex_checked(Mode::Rgba, true, |_| Some(ROJO));
    assert_eq!(descartado(resuelto), StaleReason::Changed);
    assert!(!editor.has_pending());
}

#[test]
fn fila_9_si_el_color_vuelve_a_ser_el_base_se_aplica() {
    // Deshacer (cambia) y rehacer (vuelve): al resolver, el guardado es otra vez el base.
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "#00ff00");
    editor.sync(Some((destino, ROJO)));
    editor.sync(Some((destino, AZUL)));
    let resuelto = editor.confirm_hex_checked(Mode::Indexed, true, |_| Some(AZUL));
    assert_eq!(hecho(resuelto), commit_a(destino, VERDE));
    assert!(!editor.has_pending());
}

#[test]
fn tras_un_stale_el_editor_acepta_texto_nuevo() {
    let destino = EditTarget::Entry(2);
    let mut editor = editor_con(destino, AZUL);
    teclear(&mut editor, destino, AZUL, "#00ff00");
    editor.confirm_hex_checked(Mode::Indexed, true, |_| None);
    teclear(&mut editor, destino, AZUL, "#ffff00");
    assert!(editor.has_pending());
    let resuelto = editor.confirm_hex_checked(Mode::Indexed, true, |_| Some(AZUL));
    assert_eq!(hecho(resuelto), commit_a(destino, AMARILLO));
}

// Avisos fijos.

#[test]
fn los_avisos_de_texto_descartado_son_los_de_la_spec() {
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
fn los_avisos_de_texto_descartado_solo_usan_el_juego_de_caracteres() {
    for aviso in [
        stale_notice(StaleReason::Gone),
        stale_notice(StaleReason::Changed),
        DOCUMENT_CHANGED_NOTICE,
    ] {
        assert!(
            en_el_juego(aviso),
            "{aviso:?} lleva un carácter fuera del juego"
        );
        assert!(!aviso.is_empty());
        assert!(!aviso.contains('\n'), "{aviso:?}: una sola línea");
    }
    assert_ne!(
        stale_notice(StaleReason::Gone),
        stale_notice(StaleReason::Changed)
    );
}

#[test]
fn stale_reason_es_copiable_y_comparable() {
    let motivo = StaleReason::Gone;
    let copia = motivo;
    assert_eq!(motivo, copia);
    assert_ne!(StaleReason::Gone, StaleReason::Changed);
}
