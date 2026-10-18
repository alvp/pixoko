//! Tests de `edit_layout`: la maqueta del editor de color, sin tipos de `egui`, y
//! `visible_rows_reserved` (specs/hito-2-editar-color.md, unidad 11c) y la parte pura de la
//! spec del panel con la ventana baja (specs/hito-2-panel-ventana-baja.md).
//!
//! Cubre la fila 3 de «Casos límite» de la spec del editor en su parte de maqueta
//! (`editor_height_u`, `visible_rows_reserved == visible_rows` con reserva 0, `slider_hit`,
//! `control_row_u`, `field_y_u`, `notice_y_u` y `edges_u`: todo borde en puntos × `ppp` es un
//! entero, ±0,001, a `ppp` 1,0, 1,25, 1,5, 2,0 y 3,0 con `n = text_scale(ppp)`). De la spec de la
//! ventana baja, en su parte pura: las filas 1 (`min_panel_height_px`, `min_window_height_pt`),
//! 2 (una sola función para la altura del editor: `editor_height_u`, `editor_height_u_max`,
//! `block_height_px`) y 3 (`panel_layout` con 241·n, 240·n, 200·n y 100·n). `editor_layout`,
//! `EditorLayout` y `EDITOR_HIDDEN_NOTICE` desaparecieron: la fila 18 de la spec del editor la
//! sustituye la spec de la ventana baja.
//!
//! Los números de la tabla «Geometría» están escritos como literales en un solo test
//! (`las_constantes_valen_lo_que_dice_la_spec`); el resto usa las constantes de `edit_layout`,
//! que son el único sitio donde viven.

use proptest::prelude::*;
use px_core::Mode;
use px_ui::color_edit::{Control, controls};
use px_ui::edit_layout::{
    BLOCK_GAP_U, FIELD_H_U, FIELD_PAD_U, FIELD_W_U, FIELD_X_U, FIRST_ROW_Y_U, GROUP_GAP_U,
    HEADER_X_U, HEADER_Y_U, HIT_X0_U, HIT_X1_U, LABEL_W_U, LABEL_X_U, NOTICE_LINES, PanelLayout,
    ROW_H_U, SEPARATOR_Y_U, THUMB_H_U, THUMB_W_U, TRACK_H_U, TRACK_W_U, TRACK_X_U, VALUE_W_U,
    VALUE_X_U, block_height_px, control_row_u, edges_u, editor_height_u, editor_height_u_max,
    field_y_u, min_panel_height_px, min_window_height_pt, notice_y_u, panel_layout, slider_hit,
};
use px_ui::palette_grid::{GRID_Y_U, PITCH_U, visible_rows, visible_rows_reserved};
use px_ui::status::bar_height_px;
use px_ui::text::text_scale;

const MODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];
const ESCALAS: [u32; 3] = [1, 2, 3];
const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

// Constantes.

#[test]
fn las_constantes_valen_lo_que_dice_la_spec() {
    assert_eq!(SEPARATOR_Y_U, 0);
    assert_eq!(HEADER_X_U, 4);
    assert_eq!(HEADER_Y_U, 4);
    assert_eq!(ROW_H_U, 14);
    assert_eq!(FIRST_ROW_Y_U, 22);
    assert_eq!(GROUP_GAP_U, 4);
    assert_eq!(LABEL_X_U, 4);
    assert_eq!(LABEL_W_U, 7);
    assert_eq!(TRACK_X_U, 15);
    assert_eq!(TRACK_W_U, 184);
    assert_eq!(TRACK_H_U, 4);
    assert_eq!(THUMB_W_U, 3);
    assert_eq!(THUMB_H_U, 12);
    assert_eq!(VALUE_X_U, 203);
    assert_eq!(VALUE_W_U, 21);
    assert_eq!(HIT_X0_U, 13);
    assert_eq!(HIT_X1_U, 201);
    assert_eq!(FIELD_X_U, 4);
    assert_eq!(FIELD_W_U, 69);
    assert_eq!(FIELD_H_U, 18);
    assert_eq!(FIELD_PAD_U, 3);
    assert_eq!(BLOCK_GAP_U, 4);
    assert_eq!(NOTICE_LINES, 2);
}

#[test]
fn las_constantes_son_coherentes_entre_si() {
    // `#` + 8 dígitos = 9 columnas de 7 u, más 3 de relleno a cada lado.
    assert_eq!(FIELD_W_U, 9 * 7 + 2 * FIELD_PAD_U);
    const {
        // La zona activa envuelve la pista (con 2 u de margen) y deja fuera la letra y el valor.
        assert!(HIT_X0_U < TRACK_X_U);
        assert!(HIT_X1_U > TRACK_X_U + TRACK_W_U);
        assert!(HIT_X1_U <= VALUE_X_U);
        // La pista y la marca caben en la fila.
        assert!(TRACK_H_U < ROW_H_U && THUMB_H_U < ROW_H_U);
        // Letra, pista y valor van de izquierda a derecha sin pisarse.
        assert!(LABEL_X_U + LABEL_W_U <= TRACK_X_U);
        assert!(TRACK_X_U + TRACK_W_U <= VALUE_X_U);
    }
}

// Alturas y filas.

#[test]
fn la_altura_del_editor_depende_del_modo() {
    assert_eq!(editor_height_u(Mode::Indexed), 186);
    assert_eq!(editor_height_u(Mode::Rgba), 186);
    assert_eq!(editor_height_u(Mode::GrayAlpha), 112);
    assert_eq!(editor_height_u(Mode::Gray), 94);
}

#[test]
fn las_filas_de_deslizadores_de_indexadas_y_rgba() {
    for modo in [Mode::Indexed, Mode::Rgba] {
        let esperado = [
            (Control::Red, 22),
            (Control::Green, 36),
            (Control::Blue, 50),
            (Control::Hue, 68),
            (Control::Saturation, 82),
            (Control::Lightness, 96),
            (Control::Alpha, 114),
        ];
        for (control, fila) in esperado {
            assert_eq!(
                control_row_u(modo, control),
                Some(fila),
                "{modo:?} {control:?}"
            );
        }
        assert_eq!(control_row_u(modo, Control::Gray), None);
    }
}

#[test]
fn las_filas_se_compactan_sin_huecos_en_gray_y_gray_alpha() {
    // Gray: solo «G», en la primera fila.
    assert_eq!(
        control_row_u(Mode::Gray, Control::Gray),
        Some(FIRST_ROW_Y_U)
    );
    for ausente in [
        Control::Red,
        Control::Green,
        Control::Blue,
        Control::Hue,
        Control::Saturation,
        Control::Lightness,
        Control::Alpha,
    ] {
        assert_eq!(control_row_u(Mode::Gray, ausente), None, "{ausente:?}");
    }
    // GrayAlpha: «G» y «A» con el hueco de 4 u entre grupos; la altura 112 lo exige (94 más una
    // fila de 14 más el hueco de 4).
    assert_eq!(
        control_row_u(Mode::GrayAlpha, Control::Gray),
        Some(FIRST_ROW_Y_U)
    );
    assert_eq!(
        control_row_u(Mode::GrayAlpha, Control::Alpha),
        Some(FIRST_ROW_Y_U + ROW_H_U + GROUP_GAP_U)
    );
    for ausente in [
        Control::Red,
        Control::Green,
        Control::Blue,
        Control::Hue,
        Control::Saturation,
        Control::Lightness,
    ] {
        assert_eq!(control_row_u(Mode::GrayAlpha, ausente), None, "{ausente:?}");
    }
}

#[test]
fn los_controles_de_cada_modo_tienen_fila_y_van_de_arriba_abajo() {
    for modo in MODOS {
        let mut anterior = None;
        for control in controls(modo) {
            let fila = control_row_u(modo, *control).expect("cada control tiene fila");
            if let Some(anterior) = anterior {
                assert!(fila >= anterior + ROW_H_U, "{modo:?} {control:?}");
            }
            anterior = Some(fila);
        }
    }
}

#[test]
fn el_campo_y_el_aviso_van_tras_los_controles() {
    // Tras el último control, 4 u; tras el campo, 4 u; el aviso son dos líneas y 4 u de margen.
    let esperado = [
        (Mode::Indexed, 132, 154),
        (Mode::Rgba, 132, 154),
        (Mode::GrayAlpha, 58, 80),
        (Mode::Gray, 40, 62),
    ];
    for (modo, campo, aviso) in esperado {
        assert_eq!(field_y_u(modo), campo, "{modo:?}");
        assert_eq!(notice_y_u(modo), aviso, "{modo:?}");
    }
}

#[test]
fn la_maqueta_se_deduce_de_las_constantes() {
    for modo in MODOS {
        let ultimo = controls(modo)
            .iter()
            .filter_map(|c| control_row_u(modo, *c))
            .max()
            .expect("hay controles");
        assert_eq!(field_y_u(modo), ultimo + ROW_H_U + BLOCK_GAP_U, "{modo:?}");
        assert_eq!(
            notice_y_u(modo),
            field_y_u(modo) + FIELD_H_U + BLOCK_GAP_U,
            "{modo:?}"
        );
        assert_eq!(
            editor_height_u(modo),
            notice_y_u(modo) + NOTICE_LINES * ROW_H_U + BLOCK_GAP_U,
            "{modo:?}"
        );
    }
}

// `editor_height_u_max`, `block_height_px`, `min_panel_height_px` y `min_window_height_pt`.

/// El bloque `[texto · rejilla de 1 fila · editor]` calculado a mano desde las constantes.
fn bloque_px(modo: Mode, n: i64) -> i64 {
    (GRID_Y_U + PITCH_U + editor_height_u(modo)) * n
}

#[test]
fn editor_height_u_max_es_el_mayor_de_los_modos() {
    let mayor = MODOS
        .iter()
        .map(|m| editor_height_u(*m))
        .max()
        .expect("hay modos");
    assert_eq!(editor_height_u_max(), mayor);
    assert_eq!(editor_height_u_max(), 186);
}

#[test]
fn block_height_px_es_texto_una_fila_y_editor() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        for modo in MODOS {
            assert_eq!(
                block_height_px(modo, n),
                bloque_px(modo, n_i),
                "{modo:?}, n = {n}"
            );
        }
        // Mientras no haya botones (11d): 241, 241, 167 y 149 u.
        assert_eq!(block_height_px(Mode::Indexed, n), 241 * n_i);
        assert_eq!(block_height_px(Mode::Rgba, n), 241 * n_i);
        assert_eq!(block_height_px(Mode::GrayAlpha, n), 167 * n_i);
        assert_eq!(block_height_px(Mode::Gray, n), 149 * n_i);
    }
}

// Fila 1 (parte pura de `edit_layout`).

#[test]
fn fila_1_min_panel_height_px_es_241n() {
    for n in [1u32, 2, 3] {
        let n_i = i64::from(n);
        assert_eq!(min_panel_height_px(n), 241 * n_i, "n = {n}");
        // Sin literales: una fila, el texto y el editor del modo de mayor editor.
        assert_eq!(
            min_panel_height_px(n),
            (GRID_Y_U + PITCH_U + editor_height_u_max()) * n_i
        );
    }
}

#[test]
fn fila_1_el_minimo_cubre_el_bloque_de_todos_los_modos() {
    // Abrir una imagen Gray no cambia el mínimo: es el del mayor editor, no el del modo actual.
    for n in ESCALAS {
        for modo in MODOS {
            assert!(
                block_height_px(modo, n) <= min_panel_height_px(n),
                "{modo:?}, n = {n}"
            );
        }
        assert_eq!(
            block_height_px(Mode::Indexed, n),
            min_panel_height_px(n),
            "n = {n}"
        );
        assert!(block_height_px(Mode::Gray, n) < min_panel_height_px(n));
    }
}

#[test]
fn fila_1_min_window_height_pt_a_los_cinco_ppp() {
    let esperado = [
        (1.0f32, 261.0f32),
        (1.25, 208.8),
        (1.5, 348.0),
        (2.0, 261.0),
        (3.0, 261.0),
    ];
    for (ppp, puntos) in esperado {
        let medido = min_window_height_pt(ppp);
        assert!(
            (medido - puntos).abs() <= 0.01,
            "ppp {ppp}: {medido} pt y se esperaban {puntos}"
        );
        // Barra de estado más panel mínimo, en píxeles físicos, entre `ppp`.
        let n = text_scale(ppp);
        let formula = (i64::from(bar_height_px(n)) + min_panel_height_px(n)) as f32 / ppp;
        assert!(
            (medido - formula).abs() <= 0.001,
            "ppp {ppp}: {medido} pt y la fórmula da {formula}"
        );
    }
}

#[test]
fn fila_1_el_minimo_cabe_en_la_mitad_de_768_y_deja_una_fila_con_el_editor() {
    let mut mayor = 0.0f32;
    for ppp in PPPS {
        let n = text_scale(ppp);
        let minimo = min_window_height_pt(ppp);
        mayor = mayor.max(minimo);
        assert!(
            minimo >= min_panel_height_px(n) as f32 / ppp,
            "ppp {ppp}: {minimo} pt no deja sitio a una fila y al editor"
        );
        // Y la barra de estado.
        assert!(minimo > min_panel_height_px(n) as f32 / ppp, "ppp {ppp}");
    }
    assert!(mayor <= 384.0, "el mayor de los cinco mide {mayor} pt");
}

// Fila 2: una sola función para la altura del editor.

#[test]
fn fila_2_editor_height_u_coincide_con_la_derivacion_de_la_maqueta() {
    let esperado = [
        (Mode::Indexed, 186),
        (Mode::Rgba, 186),
        (Mode::GrayAlpha, 112),
        (Mode::Gray, 94),
    ];
    for (modo, altura) in esperado {
        let derivada = notice_y_u(modo) + NOTICE_LINES * ROW_H_U + BLOCK_GAP_U;
        assert_eq!(editor_height_u(modo), derivada, "{modo:?}");
        assert_eq!(editor_height_u(modo), altura, "{modo:?}");
    }
}

/// No se puede simular de verdad una altura de botones de 20 u sin tocar la maqueta; lo que sí se
/// comprueba es que todo lo que depende de la altura del editor es función **solo** de
/// `editor_height_u`: dos modos con la misma altura dan lo mismo, y entre dos modos con alturas
/// distintas la diferencia de cada cantidad derivada es exactamente la diferencia de alturas por
/// `n`. Así, 20 u más en `editor_height_u` mueven `block_height_px`, el mínimo y `panel_layout` en
/// la misma cuantía y sin tocar nada más.
#[test]
fn fila_2_lo_derivado_solo_depende_de_editor_height_u() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        // Misma altura (186 u): Indexed y Rgba son indistinguibles.
        assert_eq!(
            block_height_px(Mode::Indexed, n),
            block_height_px(Mode::Rgba, n)
        );
        for alto in [0, 100 * n_i, 200 * n_i, 241 * n_i, 700 * n_i] {
            for pedido in [0, n_i, 50 * n_i, 10_000] {
                assert_eq!(
                    panel_layout(Mode::Indexed, alto, n, pedido),
                    panel_layout(Mode::Rgba, alto, n, pedido),
                    "alto {alto}, pedido {pedido}"
                );
            }
        }
        // Alturas distintas: la diferencia es `Δh · n`.
        for a in MODOS {
            for b in MODOS {
                let delta = (editor_height_u(a) - editor_height_u(b)) * n_i;
                assert_eq!(
                    block_height_px(a, n) - block_height_px(b, n),
                    delta,
                    "{a:?} frente a {b:?}"
                );
            }
            // El mínimo es el bloque del mayor editor: la diferencia con el bloque de `a` es lo
            // que le falta a `a` para llegar al mayor.
            assert_eq!(
                min_panel_height_px(n) - block_height_px(a, n),
                (editor_height_u_max() - editor_height_u(a)) * n_i,
                "{a:?}"
            );
            // `panel_layout` cambia de régimen exactamente en el bloque de cada modo.
            let bloque = block_height_px(a, n);
            assert!(!panel_layout(a, bloque, n, 0).scrolling, "{a:?}");
            assert!(panel_layout(a, bloque - 1, n, 0).scrolling, "{a:?}");
            // Y el editor, anclado al fondo, empieza siempre `editor_height_u · n` por encima.
            let pie = panel_layout(a, bloque, n, 0);
            assert_eq!(pie.top_px, bloque - editor_height_u(a) * n_i, "{a:?}");
        }
    }
}

// `panel_layout`.

/// `PanelLayout` del diseño normal (sin desplazamiento) con `alto` ≥ bloque.
fn normal(modo: Mode, alto: i64, n: u32) -> PanelLayout {
    let n_i = i64::from(n);
    let top = (alto - editor_height_u(modo) * n_i).div_euclid(n_i) * n_i;
    PanelLayout {
        scrolling: false,
        offset_px: 0,
        max_offset_px: 0,
        top_px: top,
        visible_rows: visible_rows_reserved(alto, alto - top, n),
    }
}

#[test]
fn con_alto_700n_el_editor_se_ancla_al_fondo() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        let esperado = [
            (Mode::Indexed, 514),
            (Mode::Rgba, 514),
            (Mode::GrayAlpha, 588),
            (Mode::Gray, 606),
        ];
        for (modo, arriba) in esperado {
            let layout = panel_layout(modo, 700 * n_i, n, 0);
            assert!(!layout.scrolling, "{modo:?}, n = {n}");
            assert_eq!(layout.offset_px, 0, "{modo:?}, n = {n}");
            assert_eq!(layout.top_px, arriba * n_i, "{modo:?}, n = {n}");
            assert_eq!(
                layout.visible_rows,
                visible_rows_reserved(700 * n_i, 700 * n_i - layout.top_px, n),
                "{modo:?}, n = {n}"
            );
        }
        // 700n con 186n reservados: (700 − 186 − 41) / 14 = 33 filas.
        assert_eq!(
            panel_layout(Mode::Indexed, 700 * n_i, n, 0).visible_rows,
            33
        );
    }
}

#[test]
fn en_el_diseno_normal_el_desplazamiento_pedido_se_ignora() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        for modo in MODOS {
            let alto = 700 * n_i;
            for pedido in [0, 1, n_i, 1000, -5] {
                let layout = panel_layout(modo, alto, n, pedido);
                assert!(!layout.scrolling, "{modo:?}, pedido {pedido}");
                assert_eq!(layout.offset_px, 0, "{modo:?}, pedido {pedido}");
                assert_eq!(layout.top_px, normal(modo, alto, n).top_px);
            }
        }
    }
}

#[test]
fn top_px_siempre_es_multiplo_de_n_aunque_el_alto_no_lo_sea() {
    // La spec propone `700n + 3` con n = 3, que sí es múltiplo de 3 (2103): se prueban los
    // restos 1 y 2 y también el 3 de la spec.
    let n = 3u32;
    let n_i = i64::from(n);
    for modo in MODOS {
        for resto in 0..=5 {
            let alto = 700 * n_i + resto;
            let layout = panel_layout(modo, alto, n, 0);
            assert!(!layout.scrolling, "{modo:?}, resto {resto}");
            assert_eq!(layout.top_px % n_i, 0, "{modo:?}, resto {resto}");
            // `((alto − H·n) div n)·n`, con el alto redondeado a píxeles físicos.
            let h = editor_height_u(modo) * n_i;
            assert_eq!(layout.top_px, (alto - h).div_euclid(n_i) * n_i);
            assert!(alto - layout.top_px >= h);
            assert_eq!(
                layout.visible_rows,
                visible_rows_reserved(alto, alto - layout.top_px, n)
            );
        }
    }
}

#[test]
fn un_pixel_mas_de_alto_no_mueve_el_editor_hasta_completar_n() {
    // `top_px` es múltiplo de `n`: un píxel físico más de alto no lo mueve; al completar `n`
    // píxeles baja exactamente `n`.
    let n = 2u32;
    let n_i = i64::from(n);
    let modo = Mode::Rgba;
    let a = panel_layout(modo, 700 * n_i, n, 0);
    let b = panel_layout(modo, 700 * n_i + 1, n, 0);
    let c = panel_layout(modo, 701 * n_i, n, 0);
    assert_eq!(a.top_px, b.top_px);
    assert_eq!(c.top_px, a.top_px + n_i);
}

#[test]
fn el_cambio_de_regimen_es_el_bloque_de_cada_modo() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        for modo in MODOS {
            let limite = bloque_px(modo, n_i);
            assert_eq!(limite, block_height_px(modo, n));
            let justo = panel_layout(modo, limite, n, 0);
            assert!(!justo.scrolling, "{modo:?}, n = {n}");
            assert_eq!(justo.visible_rows, 1, "{modo:?}, n = {n}");
            let debajo = panel_layout(modo, limite - 1, n, 0);
            assert!(debajo.scrolling, "{modo:?}, n = {n}");
            assert_eq!(debajo.visible_rows, 1, "{modo:?}, n = {n}");
            // Sin salto: con `offset = 0` el `top_px` es el mismo a un lado y otro.
            assert_eq!(debajo.top_px, justo.top_px, "{modo:?}, n = {n}");
            assert_eq!(debajo.top_px, (GRID_Y_U + PITCH_U) * n_i);
        }
    }
}

#[test]
fn alto_cero_o_negativo_no_da_panico_y_desplaza() {
    for n in ESCALAS {
        for modo in MODOS {
            for alto in [0, -5] {
                let layout = panel_layout(modo, alto, n, 0);
                assert!(layout.scrolling, "{modo:?}, alto {alto}");
                assert_eq!(layout.visible_rows, 1);
                assert_eq!(layout.offset_px, 0);
                // Con cualquier desplazamiento pedido, sigue en su rango.
                let enorme = panel_layout(modo, alto, n, i64::MAX / 4);
                assert_eq!(enorme.offset_px, enorme.max_offset_px);
            }
        }
    }
}

#[test]
fn con_el_editor_visible_siempre_queda_una_fila_de_rejilla() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        for modo in MODOS {
            for alto in 0..(900 * n_i) {
                let layout = panel_layout(modo, alto, n, 0);
                assert!(layout.visible_rows >= 1, "{modo:?}, n = {n}, alto {alto}");
                assert_eq!(layout.top_px % n_i, 0);
                assert!(layout.top_px >= (GRID_Y_U + PITCH_U) * n_i);
                assert_eq!(layout.scrolling, alto < block_height_px(modo, n));
            }
        }
    }
}

// Fila 3: alto del panel 241n, 240n, 200n y 100n.

#[test]
fn fila_3_con_241n_el_editor_cabe_y_la_rejilla_tiene_una_fila() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        for modo in [Mode::Indexed, Mode::Rgba] {
            let layout = panel_layout(modo, 241 * n_i, n, 0);
            assert!(!layout.scrolling, "{modo:?}, n = {n}");
            assert_eq!(layout.offset_px, 0);
            assert_eq!(layout.top_px, 55 * n_i, "{modo:?}, n = {n}");
            assert_eq!(layout.visible_rows, 1, "{modo:?}, n = {n}");
        }
    }
}

#[test]
fn fila_3_con_240n_y_offset_0_no_hay_salto_respecto_de_241n() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        for modo in [Mode::Indexed, Mode::Rgba] {
            let justo = panel_layout(modo, 241 * n_i, n, 0);
            let debajo = panel_layout(modo, 240 * n_i, n, 0);
            assert!(debajo.scrolling, "{modo:?}, n = {n}");
            assert_eq!(debajo.offset_px, 0);
            assert_eq!(debajo.top_px, 55 * n_i, "{modo:?}, n = {n}");
            assert_eq!(debajo.top_px, justo.top_px);
            assert_eq!(debajo.visible_rows, 1);
            // Un solo `n` de desplazamiento posible: el bloque excede el panel en `n`.
            assert_eq!(debajo.max_offset_px, n_i, "{modo:?}, n = {n}");
        }
    }
}

#[test]
fn fila_3_max_offset_px_y_top_con_offset_maximo() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        for modo in MODOS {
            let bloque = block_height_px(modo, n);
            for alto in [241 * n_i, 240 * n_i, 200 * n_i, 100 * n_i] {
                let sin = panel_layout(modo, alto, n, 0);
                if alto >= bloque {
                    assert!(!sin.scrolling, "{modo:?}, n = {n}, alto {alto}");
                    continue;
                }
                let max = (bloque - alto).div_euclid(n_i) * n_i;
                assert!(sin.scrolling, "{modo:?}, n = {n}, alto {alto}");
                assert_eq!(sin.max_offset_px, max, "{modo:?}, n = {n}, alto {alto}");
                assert_eq!(sin.visible_rows, 1);
                assert_eq!(sin.top_px, (GRID_Y_U + PITCH_U) * n_i);

                let abajo = panel_layout(modo, alto, n, max);
                assert_eq!(abajo.offset_px, max);
                assert_eq!(abajo.max_offset_px, max);
                assert_eq!(abajo.top_px, (GRID_Y_U + PITCH_U) * n_i - max);
                // Con el alto múltiplo de `n`, coincide con el editor anclado al fondo.
                assert_eq!(
                    abajo.top_px,
                    alto - editor_height_u(modo) * n_i,
                    "{modo:?}, n = {n}, alto {alto}"
                );
                assert_eq!(abajo.visible_rows, 1);
            }
        }
    }
}

#[test]
fn fila_3_el_desplazamiento_se_recorta_y_baja_a_multiplo_de_n() {
    let n = 3u32;
    let n_i = i64::from(n);
    let modo = Mode::Indexed;
    let alto = 200 * n_i;
    let max = panel_layout(modo, alto, n, 0).max_offset_px;
    assert_eq!(max, 41 * n_i);
    for (pedido, esperado) in [
        (-100, 0),
        (0, 0),
        (1, 0),
        (n_i - 1, 0),
        (n_i, n_i),
        (n_i + 1, n_i),
        (2 * n_i + 2, 2 * n_i),
        (max - 1, max - n_i),
        (max, max),
        (max + 1, max),
        (10_000, max),
    ] {
        let layout = panel_layout(modo, alto, n, pedido);
        assert_eq!(layout.offset_px, esperado, "pedido {pedido}");
        assert_eq!(
            layout.top_px,
            (GRID_Y_U + PITCH_U) * n_i - esperado,
            "pedido {pedido}"
        );
    }
}

#[test]
fn fila_3_el_editor_siempre_tiene_sitio_con_offset_maximo() {
    // Con `offset = max` el editor entero cae dentro del panel (alto múltiplo de `n`).
    for n in ESCALAS {
        let n_i = i64::from(n);
        for modo in MODOS {
            for alto_n in [60, 100, 120, 200, 240] {
                let alto = alto_n * n_i;
                let sin = panel_layout(modo, alto, n, 0);
                let layout = panel_layout(modo, alto, n, sin.max_offset_px.max(0));
                if layout.scrolling {
                    let fin = layout.top_px + editor_height_u(modo) * n_i;
                    assert_eq!(fin, alto, "{modo:?}, n = {n}, alto {alto}");
                }
            }
        }
    }
}

proptest! {
    #[test]
    fn panel_layout_cumple_sus_invariantes(
        modo in prop::sample::select(MODOS.to_vec()),
        alto in -50i64..6000,
        n in 1u32..=8,
        pedido in -200i64..9000,
    ) {
        let n_i = i64::from(n);
        let bloque = block_height_px(modo, n);
        let layout = panel_layout(modo, alto, n, pedido);
        if alto >= bloque {
            prop_assert!(!layout.scrolling);
            prop_assert_eq!(layout.offset_px, 0);
            prop_assert_eq!(layout.top_px, normal(modo, alto, n).top_px);
            prop_assert_eq!(layout.visible_rows, normal(modo, alto, n).visible_rows);
        } else {
            let max = (bloque - alto).div_euclid(n_i) * n_i;
            prop_assert!(layout.scrolling);
            prop_assert_eq!(layout.max_offset_px, max);
            prop_assert!(layout.offset_px >= 0 && layout.offset_px <= max);
            prop_assert_eq!(layout.offset_px % n_i, 0);
            prop_assert_eq!(layout.offset_px, pedido.clamp(0, max).div_euclid(n_i) * n_i);
            prop_assert_eq!(layout.top_px, (GRID_Y_U + PITCH_U) * n_i - layout.offset_px);
            prop_assert_eq!(layout.visible_rows, 1);
        }
        prop_assert_eq!(layout.top_px % n_i, 0);
    }

    #[test]
    fn el_editor_no_salta_al_cruzar_el_bloque(
        modo in prop::sample::select(MODOS.to_vec()),
        n in 1u32..=8,
        extra in 0i64..40,
    ) {
        // Un píxel por encima o por debajo del bloque, con offset 0, el top es el mismo.
        let bloque = block_height_px(modo, n);
        let a = panel_layout(modo, bloque - 1 - extra.min(i64::from(n) - 1), n, 0);
        let b = panel_layout(modo, bloque, n, 0);
        prop_assert_eq!(a.top_px, b.top_px);
        prop_assert_eq!(a.visible_rows, b.visible_rows);
    }
}

// `visible_rows_reserved`.

#[test]
fn visible_rows_reserved_con_reserva_cero_es_visible_rows() {
    for n in [1u32, 2, 3, 4] {
        for alto in -50i64..3000 {
            assert_eq!(
                visible_rows_reserved(alto, 0, n),
                visible_rows(alto, n),
                "alto {alto}, n = {n}"
            );
        }
    }
}

#[test]
fn visible_rows_reserved_usa_la_altura_restante() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        // 700n con 186n reservados: (700 − 186 − 41) / 14 = 33.
        assert_eq!(visible_rows_reserved(700 * n_i, 186 * n_i, n), 33);
        // Una reserva mayor que el alto no da pánico ni filas.
        assert_eq!(visible_rows_reserved(100 * n_i, 500 * n_i, n), 0);
        assert_eq!(visible_rows_reserved(0, 0, n), 0);
    }
}

proptest! {
    #[test]
    fn visible_rows_reserved_es_visible_rows_de_lo_que_queda(
        alto in 0i64..6000,
        reservado in 0i64..6000,
        n in 1u32..=8,
    ) {
        prop_assert_eq!(
            visible_rows_reserved(alto, reservado, n),
            visible_rows(alto - reservado.min(alto), n)
        );
    }

    #[test]
    fn reservar_mas_nunca_da_mas_filas(
        alto in 0i64..6000,
        reservado in 0i64..6000,
        extra in 0i64..500,
        n in 1u32..=8,
    ) {
        prop_assert!(
            visible_rows_reserved(alto, reservado + extra, n)
                <= visible_rows_reserved(alto, reservado, n)
        );
    }
}

// `slider_hit`.

/// Control cuya zona activa contiene el punto, calculado con `control_row_u` (probado arriba).
fn control_esperado(modo: Mode, x: i64, y: i64, n: i64) -> Option<Control> {
    if x < HIT_X0_U * n || x >= HIT_X1_U * n {
        return None;
    }
    controls(modo).iter().copied().find(|control| {
        let fila = control_row_u(modo, *control).expect("tiene fila");
        y >= fila * n && y < (fila + ROW_H_U) * n
    })
}

#[test]
fn slider_hit_recorre_toda_la_zona_de_cada_modo() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        for modo in MODOS {
            let xs = [
                -1,
                0,
                HIT_X0_U * n_i - 1,
                HIT_X0_U * n_i,
                HIT_X0_U * n_i + 1,
                100 * n_i,
                HIT_X1_U * n_i - 1,
                HIT_X1_U * n_i,
                233 * n_i,
            ];
            for y in -2..(editor_height_u(modo) * n_i + 2) {
                for x in xs {
                    assert_eq!(
                        slider_hit(modo, x, y, n),
                        control_esperado(modo, x, y, n_i),
                        "{modo:?}, n = {n}, ({x}, {y})"
                    );
                }
            }
        }
    }
}

#[test]
fn slider_hit_en_las_esquinas_de_cada_fila() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        for modo in MODOS {
            for control in controls(modo) {
                let fila = control_row_u(modo, *control).expect("tiene fila");
                let (x0, x1) = (HIT_X0_U * n_i, HIT_X1_U * n_i);
                let (y0, y1) = (fila * n_i, (fila + ROW_H_U) * n_i);
                // Dentro: las cuatro esquinas incluidas y las excluidas.
                assert_eq!(slider_hit(modo, x0, y0, n), Some(*control));
                assert_eq!(slider_hit(modo, x1 - 1, y0, n), Some(*control));
                assert_eq!(slider_hit(modo, x0, y1 - 1, n), Some(*control));
                assert_eq!(slider_hit(modo, x1 - 1, y1 - 1, n), Some(*control));
                // Fuera por la x.
                assert_eq!(slider_hit(modo, x0 - 1, y0, n), None);
                assert_eq!(slider_hit(modo, x1, y0, n), None);
            }
        }
    }
}

#[test]
fn slider_hit_no_acierta_en_la_cabecera_el_campo_ni_el_aviso() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        for modo in MODOS {
            let x = 100 * n_i;
            assert_eq!(slider_hit(modo, x, 0, n), None, "separador");
            assert_eq!(slider_hit(modo, x, HEADER_Y_U * n_i, n), None, "cabecera");
            assert_eq!(slider_hit(modo, x, field_y_u(modo) * n_i, n), None, "campo");
            assert_eq!(
                slider_hit(modo, x, notice_y_u(modo) * n_i, n),
                None,
                "aviso"
            );
            assert_eq!(
                slider_hit(modo, x, editor_height_u(modo) * n_i, n),
                None,
                "bajo el editor"
            );
        }
    }
}

#[test]
fn slider_hit_en_el_hueco_entre_grupos_no_acierta() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        let x = 100 * n_i;
        for modo in [Mode::Indexed, Mode::Rgba] {
            // Entre B (50..64) y H (68..82), y entre L (96..110) y A (114..128).
            let tras_b = control_row_u(modo, Control::Blue).expect("fila") + ROW_H_U;
            let tras_l = control_row_u(modo, Control::Lightness).expect("fila") + ROW_H_U;
            for y in [tras_b * n_i, (tras_b + GROUP_GAP_U) * n_i - 1] {
                assert_eq!(slider_hit(modo, x, y, n), None, "{modo:?}, y = {y}");
            }
            for y in [tras_l * n_i, (tras_l + GROUP_GAP_U) * n_i - 1] {
                assert_eq!(slider_hit(modo, x, y, n), None, "{modo:?}, y = {y}");
            }
            // Las tres filas de un grupo son contiguas.
            let verde = control_row_u(modo, Control::Green).expect("fila") * n_i;
            assert_eq!(slider_hit(modo, x, verde - 1, n), Some(Control::Red));
            assert_eq!(slider_hit(modo, x, verde, n), Some(Control::Green));
        }
    }
}

#[test]
fn slider_hit_no_ve_controles_que_el_modo_no_tiene() {
    let n = 2i64;
    let x = 100 * n;
    // Gray no tiene segunda fila de deslizadores.
    let segunda = (FIRST_ROW_Y_U + ROW_H_U) * n;
    assert_eq!(slider_hit(Mode::Gray, x, segunda, 2), None);
    assert_eq!(
        slider_hit(Mode::Gray, x, FIRST_ROW_Y_U * n, 2),
        Some(Control::Gray)
    );
}

// Bordes en píxeles físicos enteros.

#[test]
fn todo_borde_en_puntos_por_ppp_es_entero() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        let n_i = i64::from(n);
        for modo in MODOS {
            let bordes = edges_u(modo);
            assert!(!bordes.is_empty());
            for borde in bordes {
                let fisico = (borde * n_i) as f32;
                let puntos = fisico / ppp;
                let de_vuelta = puntos * ppp;
                assert!(
                    (de_vuelta - de_vuelta.round()).abs() <= 0.001,
                    "{modo:?}, ppp {ppp}: borde {borde} u = {puntos} pt = {de_vuelta} px"
                );
                assert!(
                    (de_vuelta - fisico).abs() <= 0.001,
                    "{modo:?}, ppp {ppp}: borde {borde} u cae en {de_vuelta} px, no en {fisico}"
                );
            }
        }
    }
}

#[test]
fn el_anclaje_del_editor_cae_en_pixeles_fisicos_enteros() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        for modo in MODOS {
            for alto in [700, 701, 777, 1080, 1441] {
                let alto = alto * i64::from(n) + i64::from(n) / 2;
                let layout = panel_layout(modo, alto, n, 0);
                assert!(!layout.scrolling, "{modo:?}, ppp {ppp}, alto {alto}");
                let puntos = layout.top_px as f32 / ppp;
                let fisico = puntos * ppp;
                assert!(
                    (fisico - layout.top_px as f32).abs() <= 0.001,
                    "{modo:?}, ppp {ppp}, alto {alto}"
                );
            }
        }
    }
}

#[test]
fn edges_u_contiene_los_bordes_de_la_maqueta() {
    for modo in MODOS {
        let bordes = edges_u(modo);
        assert!(bordes.iter().all(|b| *b >= 0), "{modo:?}: {bordes:?}");
        let mut esperados = vec![
            SEPARATOR_Y_U,
            SEPARATOR_Y_U + 1,
            HEADER_Y_U,
            HEADER_Y_U + ROW_H_U,
            TRACK_X_U,
            TRACK_X_U + TRACK_W_U,
            VALUE_X_U,
            VALUE_X_U + VALUE_W_U,
            FIELD_X_U,
            FIELD_X_U + FIELD_W_U,
            field_y_u(modo),
            field_y_u(modo) + FIELD_H_U,
            notice_y_u(modo),
            notice_y_u(modo) + ROW_H_U,
            notice_y_u(modo) + NOTICE_LINES * ROW_H_U,
            editor_height_u(modo),
        ];
        for control in controls(modo) {
            let fila = control_row_u(modo, *control).expect("tiene fila");
            esperados.extend([
                fila,
                fila + ROW_H_U,
                // La pista, centrada en la fila.
                fila + (ROW_H_U - TRACK_H_U) / 2,
                fila + (ROW_H_U - TRACK_H_U) / 2 + TRACK_H_U,
                // La marca, centrada en la fila.
                fila + (ROW_H_U - THUMB_H_U) / 2,
                fila + (ROW_H_U - THUMB_H_U) / 2 + THUMB_H_U,
            ]);
        }
        for esperado in esperados {
            assert!(
                bordes.contains(&esperado),
                "{modo:?}: falta el borde {esperado} u en {bordes:?}"
            );
        }
    }
}

#[test]
fn edges_u_no_incluye_las_filas_que_el_modo_no_tiene() {
    let gray = edges_u(Mode::Gray);
    let completo = edges_u(Mode::Rgba);
    // La fila de B de RGBA (y = 50) no existe en Gray.
    let tercera_fila = FIRST_ROW_Y_U + 2 * ROW_H_U;
    assert!(completo.contains(&tercera_fila));
    assert!(!gray.contains(&tercera_fila));
}
