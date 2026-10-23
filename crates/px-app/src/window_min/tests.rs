//! Tests de `window_min` (specs/hito-2-panel-ventana-baja.md, fila 1 en su parte de `px-app`):
//! `min_enabled` (la variable de entorno `PIXOKO_NO_MIN_SIZE`, solo para pruebas) y
//! `min_inner_size` (alto y ancho). Sin ventana: son funciones puras. La parte de `px-ui`
//! (`min_panel_height_px`, `min_window_height_pt`, `min_window_width_pt`) está en `crates/px-ui/tests/edit_layout.rs`.
//!
//! Los números esperados salen de la spec (C4): 313, 250,4, 417,33, 313 y 313 puntos de alto a `ppp`
//! 1,0, 1,25, 1,5, 2,0 y 3,0 (±0,01); el ancho mínimo es 468, 374,4, 624,0, 468 y 468 puntos (fila 1b,
//! C2: el mayor de la columna mínima más 128 u de lienzo, la barra de menús, el diálogo y las 66
//! columnas de la barra de estado, que son 468 u); el mayor alto no pasa de 468,0 (el 65 % de
//! 720; specs/hito-2-menu-herramientas.md, decisión 1) ni el mayor ancho de 683 (la mitad de 1366).

use super::*;

const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];
const ALTOS: [f32; 5] = [313.0, 250.4, 417.33, 313.0, 313.0];
const WIDTHS: [f32; 5] = [468.0, 374.4, 624.0, 468.0, 468.0];

// `min_enabled`.

#[test]
fn row_1_min_enabled_is_false_only_with_the_value_1() {
    assert!(!min_enabled(Some("1")));
}

#[test]
fn row_1_min_enabled_without_a_variable_or_with_any_other_value_is_true() {
    assert!(min_enabled(None));
    for value in [
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
        "PIXOKO_NO_MIN_SIZE",
    ] {
        assert!(min_enabled(Some(value)), "{value:?} deja el mínimo activo");
    }
}

// `min_inner_size`.

#[test]
fn row_1_min_inner_size_disabled_is_none_at_all_ppp() {
    for ppp in PPPS {
        assert_eq!(min_inner_size(ppp, false), None, "ppp {ppp}");
    }
}

#[test]
fn row_1_min_inner_size_active_gives_the_width_and_height_of_the_spec() {
    for ((ppp, height), width) in PPPS.into_iter().zip(ALTOS).zip(WIDTHS) {
        let [measured_width, measured] =
            min_inner_size(ppp, true).expect("activo: hay tamaño mínimo");
        assert!(
            (measured_width - width).abs() <= 0.01,
            "ppp {ppp}: {measured_width} pt de ancho y se esperaban {width}"
        );
        assert!(
            (measured - height).abs() <= 0.01,
            "ppp {ppp}: {measured} pt de alto y se esperaban {height}"
        );
    }
}

#[test]
fn row_1_min_inner_size_is_that_of_min_window_height_pt_and_width_pt() {
    for ppp in PPPS {
        let [width, height] = min_inner_size(ppp, true).expect("activo");
        assert!(
            (height - px_ui::edit_layout::min_window_height_pt(ppp)).abs() <= 0.001,
            "ppp {ppp}"
        );
        assert!(
            (width - px_ui::edit_layout::min_window_width_pt(ppp)).abs() <= 0.001,
            "ppp {ppp}"
        );
    }
}

#[test]
fn row_1_no_minimum_exceeds_60_percent_of_720_in_height_or_half_of_1366_in_width() {
    for ppp in PPPS {
        let [width, height] = min_inner_size(ppp, true).expect("activo");
        assert!(height <= 432.0 + 0.01, "ppp {ppp}: {height} pt");
        assert!(width <= 683.0, "ppp {ppp}: {width} pt");
        assert!(height > 0.0 && width > 0.0);
        assert!(height.is_finite() && width.is_finite());
    }
}

#[test]
fn row_1_min_inner_size_with_the_environment_variable_combines_the_two_functions() {
    // `PIXOKO_NO_MIN_SIZE=1` desactiva el mínimo; cualquier otro valor, o sin variable, no.
    for ppp in PPPS {
        assert_eq!(
            min_inner_size(ppp, min_enabled(Some("1"))),
            None,
            "ppp {ppp}"
        );
        for value in [None, Some("0"), Some("")] {
            assert!(
                min_inner_size(ppp, min_enabled(value)).is_some(),
                "ppp {ppp}, {value:?}"
            );
        }
    }
}
