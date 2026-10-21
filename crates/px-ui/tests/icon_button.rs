//! Aspecto de un botón de icono (specs/interfaz-iconos.md, decisión 3, fila 7 y propiedad 2): el
//! aspecto sale de tres banderas, `hundido`, `habilitado` y `ratón encima`, en un módulo puro
//! (`icon_button::look`, sin egui); aquí se miden sus contrastes con `theme::contrast_ratio`.
//!
//! El borde oscuro no se mide contra la cara: ni el negro puro pasa de 1,59:1 contra una cara
//! oscura, así que el contorno lo da el lado claro (≥ 3:1 con la cara, con el fondo de la caja y
//! con el oscuro).

use egui::Color32;
use px_ui::icon_button::{BEVEL_U, BUTTON_U, GAP_U, ICON_U, PAD_U, PITCH_U, Look, look};
use px_ui::theme::{
    ACCENT, BEVEL_DARK, BEVEL_LIGHT, SURFACE_0, SURFACE_1, SURFACE_2, SURFACE_3, TEXT,
    TEXT_DISABLED, contrast_ratio,
};

/// Las ocho combinaciones `(hundido, habilitado, ratón encima)`.
fn combinaciones() -> impl Iterator<Item = (bool, bool, bool)> {
    (0..8).map(|i| (i & 4 != 0, i & 2 != 0, i & 1 != 0))
}

/// Lado claro y lado oscuro del relieve de un aspecto.
fn lados(l: &Look, hundido: bool) -> (Color32, Color32) {
    if hundido {
        (l.bottom_right, l.top_left)
    } else {
        (l.top_left, l.bottom_right)
    }
}

#[test]
fn las_medidas_del_boton_son_las_de_la_spec() {
    assert_eq!((BUTTON_U, BEVEL_U, PAD_U, ICON_U), (24, 1, 3, 16));
    assert_eq!(BEVEL_U + PAD_U + ICON_U + PAD_U + BEVEL_U, BUTTON_U);
    assert_eq!((GAP_U, PITCH_U), (1, 25));
}

#[test]
fn fila_7_el_icono_sobre_la_cara_cumple_4_5_normal_encima_y_activo_y_2_desactivado() {
    let normal = look(false, true, false);
    let encima = look(false, true, true);
    let activo = look(true, true, false);
    let activo_encima = look(true, true, true);
    for (nombre, l) in [
        ("normal", &normal),
        ("encima", &encima),
        ("activo", &activo),
        ("activo con el ratón encima", &activo_encima),
    ] {
        let c = contrast_ratio(l.tint, l.face);
        assert!(c >= 4.5, "{nombre}: icono sobre cara {c}");
    }
    // Las cifras de la spec (a dos decimales).
    assert!((contrast_ratio(normal.tint, normal.face) - 10.0).abs() < 0.05);
    assert!((contrast_ratio(encima.tint, encima.face) - 7.61).abs() < 0.05);
    assert!((contrast_ratio(activo.tint, activo.face) - 8.98).abs() < 0.05);
    for (hundido, hover) in [(false, false), (false, true), (true, false), (true, true)] {
        let l = look(hundido, false, hover);
        let c = contrast_ratio(l.tint, l.face);
        assert!(c >= 2.0, "desactivado (hundido {hundido}): {c}");
        assert_eq!(l.tint, TEXT_DISABLED);
    }
    // Colores del tema, no inventados.
    assert_eq!((normal.tint, activo.tint), (TEXT, ACCENT));
    assert_eq!((normal.face, encima.face, activo.face), (SURFACE_2, SURFACE_3, SURFACE_0));
}

#[test]
fn fila_7_el_borde_claro_cumple_3_con_la_cara_el_fondo_y_el_oscuro_y_el_oscuro_no_puede() {
    for (hundido, habilitado, encima) in combinaciones() {
        let l = look(hundido, habilitado, encima);
        let (claro, oscuro) = lados(&l, hundido);
        assert_eq!(claro, BEVEL_LIGHT);
        assert_eq!(oscuro, BEVEL_DARK);
        assert_ne!(claro, oscuro);
        for (contra, nombre) in [(l.face, "la cara"), (SURFACE_1, "el fondo"), (oscuro, "el oscuro")]
        {
            let c = contrast_ratio(claro, contra);
            assert!(
                c >= 3.0,
                "{:?}: el borde claro contra {nombre} da {c}",
                (hundido, habilitado, encima)
            );
        }
    }
    // Por qué el oscuro no se mide contra la cara: ni el negro puro llega a 3:1.
    for cara in [SURFACE_2, SURFACE_3, SURFACE_0] {
        assert!(contrast_ratio(Color32::BLACK, cara) < 3.0);
    }
    assert!(contrast_ratio(Color32::BLACK, SURFACE_2) < 1.6);
}

#[test]
fn fila_7_hundido_se_distingue_de_normal_sin_depender_del_color() {
    let normal = look(false, true, false);
    let hundido = look(true, true, false);
    // El relieve se invierte y el icono baja 1 u.
    assert_eq!(normal.offset_u, 0);
    assert_eq!(hundido.offset_u, 1);
    assert_eq!(hundido.top_left, normal.bottom_right);
    assert_eq!(hundido.bottom_right, normal.top_left);
    assert_ne!(normal.tint, hundido.tint);
}

#[test]
fn propiedad_2_el_aspecto_solo_depende_de_las_tres_banderas_y_la_prioridad_es_hundido_encima_normal()
{
    for (hundido, habilitado, encima) in combinaciones() {
        let l = look(hundido, habilitado, encima);
        // Misma entrada, mismo aspecto.
        assert_eq!(l, look(hundido, habilitado, encima));
        let cara = if hundido {
            SURFACE_0
        } else if habilitado && encima {
            SURFACE_3
        } else {
            SURFACE_2
        };
        let tinta = if !habilitado {
            TEXT_DISABLED
        } else if hundido {
            ACCENT
        } else {
            TEXT
        };
        assert_eq!(l.face, cara, "{:?}", (hundido, habilitado, encima));
        assert_eq!(l.tint, tinta, "{:?}", (hundido, habilitado, encima));
        assert_eq!(l.offset_u, i64::from(hundido));
        // Con el ratón encima solo cambia la cara de un botón habilitado y no hundido.
        if !hundido && !habilitado {
            assert_eq!(l, look(false, false, !encima));
        }
    }
}
