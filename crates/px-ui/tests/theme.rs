//! Tests del tema mínimo (specs/hito-2-texto.md, «Tema mínimo» y fila 12 de «Casos límite»):
//! `contrast_ratio` es una función pura (WCAG 2.x) y las constantes de color cumplen el contraste
//! mínimo con cada fondo con el que se usan; `visuals(ppp)` da el aspecto de píxeles (modo oscuro,
//! superficies neutras, esquinas y sombras a 0, trazos en múltiplos de `u = n/ppp`).
//!
//! Sin `egui::Context`: todo es aritmética y datos.

use egui::epaint::Shadow;
use egui::{Color32, CornerRadius};
use px_ui::text::ui_unit;
use px_ui::theme::{
    ACCENT, NOTICE_PREFIX, SURFACE_0, SURFACE_1, SURFACE_2, SURFACE_3, TEXT, TEXT_DIM, WARNING,
    contrast_ratio, visuals,
};

fn near(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

const SURFACES: [Color32; 4] = [SURFACE_0, SURFACE_1, SURFACE_2, SURFACE_3];

#[test]
fn row_12_contrast_of_white_with_black_is_21() {
    let c = contrast_ratio(Color32::WHITE, Color32::BLACK);
    assert!(near(c, 21.0, 1e-6), "{c}");
}

#[test]
fn row_12_a_color_with_itself_gives_1() {
    for color in [
        Color32::WHITE,
        Color32::BLACK,
        Color32::from_rgb(0x39, 0xd0, 0xb2),
        Color32::from_gray(0x76),
    ] {
        let c = contrast_ratio(color, color);
        assert!(near(c, 1.0, 1e-9), "{color:?}: {c}");
    }
}

#[test]
fn row_12_gray_767676_over_white_gives_4_54_and_the_arguments_commute() {
    let gray_2 = Color32::from_rgb(0x76, 0x76, 0x76);
    let direct = contrast_ratio(gray_2, Color32::WHITE);
    let inverse = contrast_ratio(Color32::WHITE, gray_2);
    assert!(near(direct, 4.54, 0.01), "{direct}");
    assert!(near(direct, inverse, 1e-12), "{direct} y {inverse}");
    let (a, b) = (
        Color32::from_rgb(0x39, 0xd0, 0xb2),
        Color32::from_gray(0x24),
    );
    assert!(near(contrast_ratio(a, b), contrast_ratio(b, a), 1e-12));
}

#[test]
fn row_12_the_surfaces_and_the_text_are_grays_with_the_values_of_the_spec() {
    let expected_2_2 = [
        (SURFACE_0, 0x1a),
        (SURFACE_1, 0x24),
        (SURFACE_2, 0x30),
        (SURFACE_3, 0x42),
        (TEXT, 0xe0),
        (TEXT_DIM, 0xa3),
    ];
    for (color, gray_2) in expected_2_2 {
        assert_eq!(
            color,
            Color32::from_rgb(gray_2, gray_2, gray_2),
            "{color:?}"
        );
        assert!(
            color.r() == color.g() && color.g() == color.b(),
            "{color:?} no es neutro"
        );
    }
    assert_eq!(ACCENT, Color32::from_rgb(0x39, 0xd0, 0xb2));
    assert_eq!(WARNING, Color32::from_rgb(0xef, 0x8e, 0x39));
}

#[test]
fn row_12_all_the_colors_are_opaque_and_different() {
    let colors_2 = [
        SURFACE_0, SURFACE_1, SURFACE_2, SURFACE_3, TEXT, TEXT_DIM, ACCENT, WARNING,
    ];
    for (i, a) in colors_2.iter().enumerate() {
        assert_eq!(a.a(), 255, "{a:?} no es opaco");
        for b in &colors_2[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn row_12_the_text_has_contrast_of_at_least_7_over_the_four_surfaces() {
    for surface in SURFACES {
        let c = contrast_ratio(TEXT, surface);
        assert!(c >= 7.0, "TEXT sobre {surface:?}: {c}");
    }
}

#[test]
fn row_12_the_other_texts_have_at_least_4_5_over_the_first_three_surfaces() {
    for (name_2, text_2) in [
        ("TEXT_DIM", TEXT_DIM),
        ("ACCENT", ACCENT),
        ("WARNING", WARNING),
    ] {
        for surface in &SURFACES[..3] {
            let c = contrast_ratio(text_2, *surface);
            assert!(c >= 4.5, "{name_2} sobre {surface:?}: {c}");
        }
    }
}

#[test]
fn row_12_the_contrasts_measure_what_the_table_of_the_spec_says() {
    // Valores de la tabla (un decimal), sobre las superficies 0, 1, 2 y 3.
    let table: [(&str, Color32, [f64; 4]); 4] = [
        ("TEXT", TEXT, [13.2, 11.8, 10.0, 7.6]),
        ("TEXT_DIM", TEXT_DIM, [6.9, 6.2, 5.2, 4.0]),
        ("ACCENT", ACCENT, [9.0, 8.0, 6.8, 5.2]),
        ("WARNING", WARNING, [7.1, 6.4, 5.4, 4.1]),
    ];
    for (name_2, color, expected_2_2) in table {
        for (surface, expected_2) in SURFACES.iter().zip(expected_2_2) {
            let c = contrast_ratio(color, *surface);
            assert!(
                near(c, expected_2, 0.06),
                "{name_2} sobre {surface:?}: {c}, tabla {expected_2}"
            );
        }
    }
}

#[test]
fn row_12_visuals_uses_the_surfaces_in_dark_mode() {
    for ppp in [1.0_f32, 1.5, 2.0, 3.0] {
        let v = visuals(ppp);
        assert!(v.dark_mode, "ppp {ppp}");
        assert_eq!(v.panel_fill, SURFACE_1, "ppp {ppp}");
        assert_eq!(v.window_fill, SURFACE_2, "ppp {ppp}");
        assert_eq!(v.extreme_bg_color, SURFACE_0, "ppp {ppp}");
        assert_eq!(v.faint_bg_color, SURFACE_2, "ppp {ppp}");
        assert!(
            v.selection.bg_fill == ACCENT || v.selection.stroke.color == ACCENT,
            "ppp {ppp}: la selección usa ACCENT (en el relleno o en el trazo)"
        );
    }
}

#[test]
fn row_12_visuals_has_no_rounded_corners_or_shadows() {
    for ppp in [1.0_f32, 1.5, 2.0, 3.0] {
        let v = visuals(ppp);
        assert_eq!(v.window_corner_radius, CornerRadius::ZERO, "ppp {ppp}");
        assert_eq!(v.menu_corner_radius, CornerRadius::ZERO, "ppp {ppp}");
        for (name_2, widget) in [
            ("noninteractive", &v.widgets.noninteractive),
            ("inactive", &v.widgets.inactive),
            ("hovered", &v.widgets.hovered),
            ("active", &v.widgets.active),
            ("open", &v.widgets.open),
        ] {
            assert_eq!(
                widget.corner_radius,
                CornerRadius::ZERO,
                "ppp {ppp}: {name_2}"
            );
        }
        assert_eq!(v.window_shadow, Shadow::NONE, "ppp {ppp}");
        assert_eq!(v.popup_shadow, Shadow::NONE, "ppp {ppp}");
    }
}

#[test]
fn row_12_the_strokes_of_visuals_are_multiples_of_the_interface_unit() {
    for ppp in [1.0_f32, 1.25, 1.5, 2.0, 2.5, 3.0] {
        let u = ui_unit(ppp);
        let v = visuals(ppp);
        let strokes = [
            ("window_stroke", v.window_stroke.width),
            (
                "noninteractive.bg",
                v.widgets.noninteractive.bg_stroke.width,
            ),
            ("inactive.bg", v.widgets.inactive.bg_stroke.width),
            ("hovered.bg", v.widgets.hovered.bg_stroke.width),
            ("active.bg", v.widgets.active.bg_stroke.width),
            ("open.bg", v.widgets.open.bg_stroke.width),
            (
                "noninteractive.fg",
                v.widgets.noninteractive.fg_stroke.width,
            ),
            ("inactive.fg", v.widgets.inactive.fg_stroke.width),
            ("hovered.fg", v.widgets.hovered.fg_stroke.width),
            ("active.fg", v.widgets.active.fg_stroke.width),
            ("open.fg", v.widgets.open.fg_stroke.width),
        ];
        for (name_2, width_2) in strokes {
            let multiples = f64::from(width_2) / f64::from(u);
            assert!(
                near(multiples, multiples.round(), 1e-3),
                "ppp {ppp}: {name_2} mide {width_2} pt, {multiples} unidades de {u} pt"
            );
        }
    }
}

#[test]
fn row_12_the_notice_prefix_is_notice_colon_and_space() {
    assert_eq!(NOTICE_PREFIX, "aviso: ");
}
