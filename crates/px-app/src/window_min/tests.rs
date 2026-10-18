//! Tests de `window_min` (specs/hito-2-panel-ventana-baja.md, fila 1 en su parte de `px-app`):
//! `min_enabled` (la variable de entorno `PIXOKO_SIN_MINIMO`, solo para pruebas) y
//! `min_inner_size` (alto y ancho). Sin ventana: son funciones puras. La parte de `px-ui`
//! (`min_panel_height_px`, `min_window_height_pt`, `min_window_width_pt`) está en `crates/px-ui/tests/edit_layout.rs`.
//!
//! Los números esperados salen de la spec: 317, 253,6, 422,67, 317 y 317 puntos de alto a `ppp`
//! 1,0, 1,25, 1,5, 2,0 y 3,0 (±0,01); el ancho mínimo es 361, 288,8, 481,33, 361 y 361 puntos (fila 1b: panel
//! de 233 u más 128 u de lienzo); el mayor alto no pasa de 432 (el 60 % de 720; unidad 11d) ni el mayor
//! ancho de 683 (la mitad de 1366).

use super::*;

const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];
const ALTOS: [f32; 5] = [317.0, 253.6, 422.67, 317.0, 317.0];
const ANCHOS: [f32; 5] = [361.0, 288.8, 481.33, 361.0, 361.0];

// `min_enabled`.

#[test]
fn fila_1_min_enabled_solo_es_falso_con_el_valor_1() {
    assert!(!min_enabled(Some("1")));
}

#[test]
fn fila_1_min_enabled_sin_variable_o_con_cualquier_otro_valor_es_verdadero() {
    assert!(min_enabled(None));
    for valor in [
        "0",
        "",
        "2",
        "11",
        "01",
        " 1",
        "1 ",
        "true",
        "si",
        "sí",
        "yes",
        "on",
        "un",
        "\n",
        "PIXOKO_SIN_MINIMO",
    ] {
        assert!(min_enabled(Some(valor)), "{valor:?} deja el mínimo activo");
    }
}

// `min_inner_size`.

#[test]
fn fila_1_min_inner_size_desactivado_es_none_a_todos_los_ppp() {
    for ppp in PPPS {
        assert_eq!(min_inner_size(ppp, false), None, "ppp {ppp}");
    }
}

#[test]
fn fila_1_min_inner_size_activo_da_el_ancho_y_el_alto_de_la_spec() {
    for ((ppp, alto), ancho) in PPPS.into_iter().zip(ALTOS).zip(ANCHOS) {
        let [medido_ancho, medido] = min_inner_size(ppp, true).expect("activo: hay tamaño mínimo");
        assert!(
            (medido_ancho - ancho).abs() <= 0.01,
            "ppp {ppp}: {medido_ancho} pt de ancho y se esperaban {ancho}"
        );
        assert!(
            (medido - alto).abs() <= 0.01,
            "ppp {ppp}: {medido} pt de alto y se esperaban {alto}"
        );
    }
}

#[test]
fn fila_1_min_inner_size_es_el_de_min_window_height_pt_y_width_pt() {
    for ppp in PPPS {
        let [ancho, alto] = min_inner_size(ppp, true).expect("activo");
        assert!(
            (alto - px_ui::edit_layout::min_window_height_pt(ppp)).abs() <= 0.001,
            "ppp {ppp}"
        );
        assert!(
            (ancho - px_ui::edit_layout::min_window_width_pt(ppp)).abs() <= 0.001,
            "ppp {ppp}"
        );
    }
}

#[test]
fn fila_1_ningun_minimo_pasa_del_60_por_ciento_de_720_de_alto_ni_de_la_mitad_de_1366_de_ancho() {
    for ppp in PPPS {
        let [ancho, alto] = min_inner_size(ppp, true).expect("activo");
        assert!(alto <= 432.0, "ppp {ppp}: {alto} pt");
        assert!(ancho <= 683.0, "ppp {ppp}: {ancho} pt");
        assert!(alto > 0.0 && ancho > 0.0);
        assert!(alto.is_finite() && ancho.is_finite());
    }
}

#[test]
fn fila_1_min_inner_size_con_la_variable_de_entorno_combina_las_dos_funciones() {
    // `PIXOKO_SIN_MINIMO=1` desactiva el mínimo; cualquier otro valor, o sin variable, no.
    for ppp in PPPS {
        assert_eq!(
            min_inner_size(ppp, min_enabled(Some("1"))),
            None,
            "ppp {ppp}"
        );
        for valor in [None, Some("0"), Some("")] {
            assert!(
                min_inner_size(ppp, min_enabled(valor)).is_some(),
                "ppp {ppp}, {valor:?}"
            );
        }
    }
}
