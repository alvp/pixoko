//! Aspecto de un botón de icono (specs/interfaz-iconos.md, decisión 3, fila 7 y propiedad 2): el
//! aspecto sale de tres banderas, `pressed`, `enabled_2` y `ratón above`, en un módulo puro
//! (`icon_button::look`, sin egui); aquí se miden sus contrastes con `theme::contrast_ratio`.
//!
//! El borde oscuro no se mide contra la cara: ni el negro puro pasa de 1,59:1 contra una cara
//! oscura, así que el contorno lo da el lado claro (≥ 3:1 con la cara, con el fondo de la caja y
//! con el oscuro).

use egui::Color32;
use px_ui::icon_button::{BEVEL_U, BUTTON_U, GAP_U, ICON_U, Look, PAD_U, PITCH_U, look};
use px_ui::theme::{
    ACCENT, BEVEL_DARK, BEVEL_LIGHT, SURFACE_0, SURFACE_1, SURFACE_2, SURFACE_3, TEXT,
    TEXT_DISABLED, contrast_ratio,
};

/// Las ocho combinaciones `(pressed, enabled_2, ratón above)`.
fn combinations() -> impl Iterator<Item = (bool, bool, bool)> {
    (0..8).map(|i| (i & 4 != 0, i & 2 != 0, i & 1 != 0))
}

/// Lado claro y lado oscuro del relieve de un aspecto.
fn sides(l: &Look, pressed: bool) -> (Color32, Color32) {
    if pressed {
        (l.bottom_right, l.top_left)
    } else {
        (l.top_left, l.bottom_right)
    }
}

#[test]
fn the_button_measures_are_those_of_the_spec() {
    assert_eq!((BUTTON_U, BEVEL_U, PAD_U, ICON_U), (24, 1, 3, 16));
    assert_eq!(BEVEL_U + PAD_U + ICON_U + PAD_U + BEVEL_U, BUTTON_U);
    assert_eq!((GAP_U, PITCH_U), (1, 25));
}

#[test]
fn row_7_the_icon_over_the_face_meets_4_5_normal_hover_and_active_and_2_disabled() {
    let normal = look(false, true, false);
    let above = look(false, true, true);
    let active_2_2 = look(true, true, false);
    let active_above = look(true, true, true);
    for (name_2, l) in [
        ("normal", &normal),
        ("encima", &above),
        ("activo", &active_2_2),
        ("activo con el ratón encima", &active_above),
    ] {
        let c = contrast_ratio(l.tint, l.face);
        assert!(c >= 4.5, "{name_2}: icono sobre cara {c}");
    }
    // Las cifras de la spec (a dos decimales).
    assert!((contrast_ratio(normal.tint, normal.face) - 10.0).abs() < 0.05);
    assert!((contrast_ratio(above.tint, above.face) - 7.61).abs() < 0.05);
    assert!((contrast_ratio(active_2_2.tint, active_2_2.face) - 8.98).abs() < 0.05);
    for (pressed, hover) in [(false, false), (false, true), (true, false), (true, true)] {
        let l = look(pressed, false, hover);
        let c = contrast_ratio(l.tint, l.face);
        assert!(c >= 2.0, "desactivado (hundido {pressed}): {c}");
        assert_eq!(l.tint, TEXT_DISABLED);
    }
    // Colores del tema, no inventados.
    assert_eq!((normal.tint, active_2_2.tint), (TEXT, ACCENT));
    assert_eq!(
        (normal.face, above.face, active_2_2.face),
        (SURFACE_2, SURFACE_3, SURFACE_0)
    );
}

#[test]
fn row_7_the_light_border_meets_3_with_the_face_the_background_and_the_dark_one_and_the_dark_one_cannot()
 {
    for (pressed, enabled_2, above) in combinations() {
        let l = look(pressed, enabled_2, above);
        let (light, dark) = sides(&l, pressed);
        assert_eq!(light, BEVEL_LIGHT);
        assert_eq!(dark, BEVEL_DARK);
        assert_ne!(light, dark);
        for (against, name_2) in [
            (l.face, "la cara"),
            (SURFACE_1, "el fondo"),
            (dark, "el oscuro"),
        ] {
            let c = contrast_ratio(light, against);
            assert!(
                c >= 3.0,
                "{:?}: el borde claro contra {name_2} da {c}",
                (pressed, enabled_2, above)
            );
        }
    }
    // Por qué el oscuro no se mide contra la cara: ni el negro puro llega a 3:1.
    for face in [SURFACE_2, SURFACE_3, SURFACE_0] {
        assert!(contrast_ratio(Color32::BLACK, face) < 3.0);
    }
    assert!(contrast_ratio(Color32::BLACK, SURFACE_2) < 1.6);
}

#[test]
fn row_7_pressed_is_distinguished_from_normal_without_depending_on_the_color() {
    let normal = look(false, true, false);
    let pressed = look(true, true, false);
    // El relieve se invierte y el icono baja 1 u.
    assert_eq!(normal.offset_u, 0);
    assert_eq!(pressed.offset_u, 1);
    assert_eq!(pressed.top_left, normal.bottom_right);
    assert_eq!(pressed.bottom_right, normal.top_left);
    assert_ne!(normal.tint, pressed.tint);
}

#[test]
fn property_2_the_look_only_depends_on_the_three_flags_and_the_priority_is_pressed_over_normal() {
    for (pressed, enabled_2, above) in combinations() {
        let l = look(pressed, enabled_2, above);
        // Misma entrada, mismo aspecto.
        assert_eq!(l, look(pressed, enabled_2, above));
        let face = if pressed {
            SURFACE_0
        } else if enabled_2 && above {
            SURFACE_3
        } else {
            SURFACE_2
        };
        let ink = if !enabled_2 {
            TEXT_DISABLED
        } else if pressed {
            ACCENT
        } else {
            TEXT
        };
        assert_eq!(l.face, face, "{:?}", (pressed, enabled_2, above));
        assert_eq!(l.tint, ink, "{:?}", (pressed, enabled_2, above));
        assert_eq!(l.offset_u, i64::from(pressed));
        // Con el ratón encima solo cambia la cara de un botón habilitado y no hundido.
        if !pressed && !enabled_2 {
            assert_eq!(l, look(false, false, !above));
        }
    }
}
