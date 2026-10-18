//! Tests de `edit_layout` (specs/hito-2-editar-color.md, unidad 11c): la maqueta del editor de
//! color, sin tipos de `egui`, y `visible_rows_reserved`.
//!
//! Cubre la fila 3 de «Casos límite» en su parte de maqueta (`editor_layout`, `editor_height_u`,
//! `visible_rows_reserved == visible_rows` con reserva 0, `slider_hit`, `control_row_u`,
//! `field_y_u`, `notice_y_u` y `edges_u`: todo borde en puntos × `ppp` es un entero, ±0,001, a
//! `ppp` 1,0, 1,25, 1,5, 2,0 y 3,0 con `n = text_scale(ppp)`) y las medidas de la fila 18 que son
//! de geometría (240n y 241n).
//!
//! Los números de la tabla «Geometría» están escritos como literales en un solo test
//! (`las_constantes_valen_lo_que_dice_la_spec`); el resto usa las constantes de `edit_layout`,
//! que son el único sitio donde viven.

use proptest::prelude::*;
use px_core::Mode;
use px_ui::color_edit::{Control, controls};
use px_ui::edit_layout::{
    BLOCK_GAP_U, EDITOR_HIDDEN_NOTICE, EditorLayout, FIELD_H_U, FIELD_PAD_U, FIELD_W_U, FIELD_X_U,
    FIRST_ROW_Y_U, GROUP_GAP_U, HEADER_X_U, HEADER_Y_U, HIT_X0_U, HIT_X1_U, LABEL_W_U, LABEL_X_U,
    NOTICE_LINES, ROW_H_U, SEPARATOR_Y_U, THUMB_H_U, THUMB_W_U, TRACK_H_U, TRACK_W_U, TRACK_X_U,
    VALUE_W_U, VALUE_X_U, control_row_u, edges_u, editor_height_u, editor_layout, field_y_u,
    notice_y_u, slider_hit,
};
use px_ui::palette_grid::{GRID_Y_U, PITCH_U, visible_rows, visible_rows_reserved};
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
    assert_eq!(EDITOR_HIDDEN_NOTICE, "ventana baja: editor oculto");
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

// `editor_layout`.

#[test]
fn con_alto_700n_el_editor_se_ancla_al_fondo() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        let esperado = [
            (Mode::Indexed, 514, 186),
            (Mode::Rgba, 514, 186),
            (Mode::GrayAlpha, 588, 112),
            (Mode::Gray, 606, 94),
        ];
        for (modo, arriba, reservado) in esperado {
            assert_eq!(
                editor_layout(modo, 700 * n_i, n),
                Some(EditorLayout {
                    top_px: arriba * n_i,
                    reserved_px: reservado * n_i
                }),
                "{modo:?}, n = {n}"
            );
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
            let layout = editor_layout(modo, alto, n).expect("cabe");
            assert_eq!(layout.top_px % n_i, 0, "{modo:?}, resto {resto}");
            // `((alto − H·n) div n)·n`, con el alto redondeado a píxeles físicos.
            let h = editor_height_u(modo) * n_i;
            assert_eq!(layout.top_px, (alto - h).div_euclid(n_i) * n_i);
            assert_eq!(layout.reserved_px, alto - layout.top_px);
            assert!(layout.reserved_px >= h);
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
    let a = editor_layout(modo, 700 * n_i, n).expect("cabe");
    let b = editor_layout(modo, 700 * n_i + 1, n).expect("cabe");
    let c = editor_layout(modo, 701 * n_i, n).expect("cabe");
    assert_eq!(a.top_px, b.top_px);
    assert_eq!(c.top_px, a.top_px + n_i);
}

#[test]
fn no_cabe_con_240n_pero_si_con_241n_en_indexadas_y_rgba() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        for modo in [Mode::Indexed, Mode::Rgba] {
            assert_eq!(editor_layout(modo, 240 * n_i, n), None, "{modo:?}, n = {n}");
            let layout = editor_layout(modo, 241 * n_i, n).expect("cabe");
            assert_eq!(layout.reserved_px, 186 * n_i);
            // Queda justo una fila de rejilla.
            assert_eq!(
                visible_rows_reserved(241 * n_i, layout.reserved_px, n),
                1,
                "{modo:?}, n = {n}"
            );
        }
    }
}

#[test]
fn el_limite_es_una_fila_de_rejilla_mas_el_editor_en_cada_modo() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        for modo in MODOS {
            let limite = (GRID_Y_U + PITCH_U + editor_height_u(modo)) * n_i;
            assert_eq!(
                editor_layout(modo, limite - 1, n),
                None,
                "{modo:?}, n = {n}"
            );
            let layout = editor_layout(modo, limite, n).expect("cabe");
            assert_eq!(visible_rows_reserved(limite, layout.reserved_px, n), 1);
        }
    }
}

#[test]
fn alto_cero_o_negativo_no_tiene_editor() {
    for n in ESCALAS {
        for modo in MODOS {
            assert_eq!(editor_layout(modo, 0, n), None);
            assert_eq!(editor_layout(modo, -5, n), None);
        }
    }
}

#[test]
fn con_el_editor_visible_siempre_queda_una_fila_de_rejilla() {
    for n in ESCALAS {
        let n_i = i64::from(n);
        for modo in MODOS {
            for alto in 0..(900 * n_i) {
                if let Some(layout) = editor_layout(modo, alto, n) {
                    assert!(
                        visible_rows_reserved(alto, layout.reserved_px, n) >= 1,
                        "{modo:?}, n = {n}, alto {alto}"
                    );
                    assert_eq!(layout.top_px % n_i, 0);
                    assert!(layout.top_px >= (GRID_Y_U + PITCH_U) * n_i);
                }
            }
        }
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
                let layout = editor_layout(modo, alto, n).expect("cabe");
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
