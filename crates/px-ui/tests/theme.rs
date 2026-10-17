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

fn cerca(a: f64, b: f64, tolerancia: f64) -> bool {
    (a - b).abs() <= tolerancia
}

const SUPERFICIES: [Color32; 4] = [SURFACE_0, SURFACE_1, SURFACE_2, SURFACE_3];

#[test]
fn fila_12_contraste_de_blanco_con_negro_es_21() {
    let c = contrast_ratio(Color32::WHITE, Color32::BLACK);
    assert!(cerca(c, 21.0, 1e-6), "{c}");
}

#[test]
fn fila_12_un_color_consigo_mismo_da_1() {
    for color in [
        Color32::WHITE,
        Color32::BLACK,
        Color32::from_rgb(0x39, 0xd0, 0xb2),
        Color32::from_gray(0x76),
    ] {
        let c = contrast_ratio(color, color);
        assert!(cerca(c, 1.0, 1e-9), "{color:?}: {c}");
    }
}

#[test]
fn fila_12_el_gris_767676_sobre_blanco_da_4_54_y_los_argumentos_conmutan() {
    let gris = Color32::from_rgb(0x76, 0x76, 0x76);
    let directo = contrast_ratio(gris, Color32::WHITE);
    let inverso = contrast_ratio(Color32::WHITE, gris);
    assert!(cerca(directo, 4.54, 0.01), "{directo}");
    assert!(cerca(directo, inverso, 1e-12), "{directo} y {inverso}");
    let (a, b) = (
        Color32::from_rgb(0x39, 0xd0, 0xb2),
        Color32::from_gray(0x24),
    );
    assert!(cerca(contrast_ratio(a, b), contrast_ratio(b, a), 1e-12));
}

#[test]
fn fila_12_las_superficies_y_el_texto_son_grises_con_los_valores_de_la_spec() {
    let esperados = [
        (SURFACE_0, 0x1a),
        (SURFACE_1, 0x24),
        (SURFACE_2, 0x30),
        (SURFACE_3, 0x42),
        (TEXT, 0xe0),
        (TEXT_DIM, 0xa3),
    ];
    for (color, gris) in esperados {
        assert_eq!(color, Color32::from_rgb(gris, gris, gris), "{color:?}");
        assert!(
            color.r() == color.g() && color.g() == color.b(),
            "{color:?} no es neutro"
        );
    }
    assert_eq!(ACCENT, Color32::from_rgb(0x39, 0xd0, 0xb2));
    assert_eq!(WARNING, Color32::from_rgb(0xef, 0x8e, 0x39));
}

#[test]
fn fila_12_todos_los_colores_son_opacos_y_distintos() {
    let colores = [
        SURFACE_0, SURFACE_1, SURFACE_2, SURFACE_3, TEXT, TEXT_DIM, ACCENT, WARNING,
    ];
    for (i, a) in colores.iter().enumerate() {
        assert_eq!(a.a(), 255, "{a:?} no es opaco");
        for b in &colores[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn fila_12_el_texto_tiene_contraste_de_al_menos_7_sobre_las_cuatro_superficies() {
    for superficie in SUPERFICIES {
        let c = contrast_ratio(TEXT, superficie);
        assert!(c >= 7.0, "TEXT sobre {superficie:?}: {c}");
    }
}

#[test]
fn fila_12_los_demas_textos_tienen_al_menos_4_5_sobre_las_tres_primeras_superficies() {
    for (nombre, texto) in [
        ("TEXT_DIM", TEXT_DIM),
        ("ACCENT", ACCENT),
        ("WARNING", WARNING),
    ] {
        for superficie in &SUPERFICIES[..3] {
            let c = contrast_ratio(texto, *superficie);
            assert!(c >= 4.5, "{nombre} sobre {superficie:?}: {c}");
        }
    }
}

#[test]
fn fila_12_los_contrastes_miden_lo_que_dice_la_tabla_de_la_spec() {
    // Valores de la tabla (un decimal), sobre las superficies 0, 1, 2 y 3.
    let tabla: [(&str, Color32, [f64; 4]); 4] = [
        ("TEXT", TEXT, [13.2, 11.8, 10.0, 7.6]),
        ("TEXT_DIM", TEXT_DIM, [6.9, 6.2, 5.2, 4.0]),
        ("ACCENT", ACCENT, [9.0, 8.0, 6.8, 5.2]),
        ("WARNING", WARNING, [7.1, 6.4, 5.4, 4.1]),
    ];
    for (nombre, color, esperados) in tabla {
        for (superficie, esperado) in SUPERFICIES.iter().zip(esperados) {
            let c = contrast_ratio(color, *superficie);
            assert!(
                cerca(c, esperado, 0.06),
                "{nombre} sobre {superficie:?}: {c}, tabla {esperado}"
            );
        }
    }
}

#[test]
fn fila_12_visuals_usa_las_superficies_en_modo_oscuro() {
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
fn fila_12_visuals_no_tiene_esquinas_redondeadas_ni_sombras() {
    for ppp in [1.0_f32, 1.5, 2.0, 3.0] {
        let v = visuals(ppp);
        assert_eq!(v.window_corner_radius, CornerRadius::ZERO, "ppp {ppp}");
        assert_eq!(v.menu_corner_radius, CornerRadius::ZERO, "ppp {ppp}");
        for (nombre, widget) in [
            ("noninteractive", &v.widgets.noninteractive),
            ("inactive", &v.widgets.inactive),
            ("hovered", &v.widgets.hovered),
            ("active", &v.widgets.active),
            ("open", &v.widgets.open),
        ] {
            assert_eq!(
                widget.corner_radius,
                CornerRadius::ZERO,
                "ppp {ppp}: {nombre}"
            );
        }
        assert_eq!(v.window_shadow, Shadow::NONE, "ppp {ppp}");
        assert_eq!(v.popup_shadow, Shadow::NONE, "ppp {ppp}");
    }
}

#[test]
fn fila_12_los_trazos_de_visuals_son_multiplos_de_la_unidad_de_interfaz() {
    for ppp in [1.0_f32, 1.25, 1.5, 2.0, 2.5, 3.0] {
        let u = ui_unit(ppp);
        let v = visuals(ppp);
        let trazos = [
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
        for (nombre, ancho) in trazos {
            let multiplos = f64::from(ancho) / f64::from(u);
            assert!(
                cerca(multiplos, multiplos.round(), 1e-3),
                "ppp {ppp}: {nombre} mide {ancho} pt, {multiplos} unidades de {u} pt"
            );
        }
    }
}

#[test]
fn fila_12_el_prefijo_de_aviso_es_aviso_dos_puntos_y_espacio() {
    assert_eq!(NOTICE_PREFIX, "aviso: ");
}
