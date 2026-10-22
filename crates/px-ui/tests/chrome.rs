//! Tests de la geometría de la ventana `px_ui::chrome` y de los mínimos de ventana
//! (specs/hito-2-menu-herramientas.md, «Casos» filas 7 y 8 y propiedad 3). Sin tipos de egui:
//! `chrome` es aritmética entera.
//!
//! - Fila 7 (parte pura): `toolbar_width_u() = 107 = 2·4 + 99` con 99 = `4·24 + 3·1` (la rejilla de
//!   iconos de 4 columnas, specs/interfaz-iconos.md); los 9 botones caben en la
//!   columna sin tocar el borde y no se solapan; el contenido (250 u) cabe en 305 u; el menú entero
//!   y el desplegable más ancho (208 u) caben en el ancho mínimo; los cinco rectángulos de
//!   `window_layout` recubren la ventana sin huecos ni solapes, a las cinco escalas y en la ventana
//!   por defecto y en la mínima. El banco de egui con la pila real está en `window_stack.rs`.
//! - Fila 8: `min_window_height_pt`, `min_window_width_u` y `min_window_width_pt` a las cinco
//!   escalas y con `ppp` inválido; `min_inner_size` es de `px-app` (`window_min/tests.rs`).
//! - Propiedad 3: geometría entera y recubrimiento con `ppp` de 1,0 a 4,0 en pasos de 0,05.

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_ui::actions::Platform;
use px_ui::chrome::{
    MENU_H_U, TOOL_LABELS, TOOLBAR_CONTENT_H_U, TOOLBOX_COLUMNS, WindowLayout, canvas_origin_px,
    toolbar_button_width_u, toolbar_width_u, window_layout,
};
use px_ui::edit_layout::{
    MIN_CANVAS_WIDTH_U, min_panel_height_px, min_window_height_pt, min_window_width_pt,
    min_window_width_u,
};
use px_ui::icon_button::{BUTTON_U, GAP_U};
use px_ui::menu::{MENUS, shortcut_for, shortcut_text};
use px_ui::palette_grid::PANEL_WIDTH_U;
use px_ui::status::{ADVANCE_PX, bar_height_px};
use px_ui::text::text_scale;
use px_ui::toolbar::TOOL_BUTTONS;
use px_ui::view::ScreenRect;

const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];
const ALTOS_PT: [f32; 5] = [341.0, 272.8, 454.67, 341.0, 341.0];
const WIDTHS_PT: [f32; 5] = [468.0, 374.4, 624.0, 468.0, 468.0];
const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

fn n_de(ppp: f32) -> i64 {
    i64::from(text_scale(ppp))
}

fn near_pt(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.01
}

// ---------------------------------------------------------------------------------------------
// Fila 7: constantes y columna
// ---------------------------------------------------------------------------------------------

#[test]
fn row_7_the_constants_are_worth_what_the_spec_says() {
    assert_eq!(MENU_H_U, 16);
    assert_eq!(TOOLBAR_CONTENT_H_U, 250);
    assert_eq!(toolbar_width_u(), 107);
    assert_eq!(toolbar_button_width_u(), 99);
    assert_eq!(PANEL_WIDTH_U, 233);
    assert_eq!(bar_height_px(1), 20);
    assert_eq!(min_panel_height_px(1), 305);
}

#[test]
fn row_7_the_column_width_is_derived_from_the_icon_grid() {
    assert_eq!(TOOLBOX_COLUMNS, 4);
    let button_2 = TOOLBOX_COLUMNS * BUTTON_U + (TOOLBOX_COLUMNS - 1) * GAP_U;
    assert_eq!(button_2, 99);
    assert_eq!(toolbar_button_width_u(), button_2);
    assert_eq!(toolbar_width_u(), 2 * 4 + button_2);
    assert_eq!(toolbar_width_u(), 107);
}

#[test]
fn row_7_tool_labels_are_the_labels_of_tool_buttons_without_the_letter() {
    let labels: Vec<&str> = TOOL_BUTTONS.iter().map(|b| b.label).collect();
    assert_eq!(labels, TOOL_LABELS);
    assert_eq!(
        TOOL_LABELS,
        ["Lápiz", "Línea", "Relleno", "Cuentagotas", "Borrador"]
    );
}

#[test]
fn row_7_the_nine_buttons_fit_in_the_column_without_touching_the_edge_and_do_not_overlap() {
    // Herramientas: dos filas de botones de 24 u en y = 4 y 29 (hueco de 1 u); zoom: una fila en
    // y = 80.
    let height_2 = 24;
    let tools: Vec<(i64, i64)> = (0..2).map(|i| (4 + 25 * i, height_2)).collect();
    let zoom: Vec<(i64, i64)> = vec![(80, height_2)];
    assert_eq!(tools.last().map(|(y, h)| y + h), Some(53));
    assert_eq!(zoom.last().map(|(y, h)| y + h), Some(104));
    let separator = (57, 1);
    let level = (62, 14);

    let mut all_2: Vec<(i64, i64)> = tools.clone();
    all_2.push(separator);
    all_2.push(level);
    all_2.extend(zoom.iter().copied());
    for pair in all_2.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        assert!(a.0 + a.1 <= b.0, "{a:?} se solapa con {b:?}");
    }
    // Margen de 4 u a cada lado: 4 + 99 + 4 = 107, sin tocar el borde.
    let x0 = 4;
    let width_2 = toolbar_button_width_u();
    assert!(x0 > 0);
    assert!(x0 + width_2 < toolbar_width_u());
    assert_eq!(x0 + width_2 + 4, toolbar_width_u());
    // Dos filas seguidas de un mismo grupo dejan 1 u entre ellas.
    for pair in tools.windows(2).chain(zoom.windows(2)) {
        assert_eq!(pair[1].0 - (pair[0].0 + pair[0].1), 1);
    }
}

#[test]
fn row_7_the_250_u_content_fits_in_the_305_u_of_the_minimum_panel() {
    // Fila de zoom (termina en 104), 4 u, el recuadro de opciones de 138 u y 4 u de margen.
    assert_eq!(80 + 24 + 4 + 138 + 4, TOOLBAR_CONTENT_H_U);
    assert_eq!(TOOLBAR_CONTENT_H_U, 250);
    for ppp in PPPS {
        let n = n_de(ppp);
        assert!(TOOLBAR_CONTENT_H_U * n <= min_panel_height_px(n as u32));
        assert_eq!(
            min_panel_height_px(n as u32) - TOOLBAR_CONTENT_H_U * n,
            55 * n
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: el menú cabe en el ancho mínimo
// ---------------------------------------------------------------------------------------------

/// Ancho de un título como botón: `chars·7 + 12` (6 u de margen a cada lado del texto).
fn title_width_u(title: &str) -> i64 {
    title.chars().count() as i64 * i64::from(ADVANCE_PX) + 12
}

/// Ancho del desplegable: `12 + max(rótulo + 2 + shortcut_2)·7`.
fn dropdown_width_u(items: &[px_ui::menu::MenuItem], platform: Platform) -> i64 {
    let greater = items
        .iter()
        .map(|item| {
            let shortcut_2 = shortcut_for(item.action, platform)
                .map_or(0, |s| shortcut_text(s, platform).chars().count());
            item.label.chars().count() + 2 + shortcut_2
        })
        .max()
        .unwrap_or(0) as i64;
    12 + greater * i64::from(ADVANCE_PX)
}

#[test]
fn row_7_the_menu_titles_add_249_u_and_fit_with_their_margin_in_the_minimum_width() {
    let widths: Vec<i64> = MENUS.iter().map(|m| title_width_u(m.title)).collect();
    assert_eq!(widths, [61, 54, 54, 40, 33, 47]);
    let total: i64 = widths.iter().sum();
    assert_eq!(total, 289);
    assert!(4 + total < min_window_width_u());
}

#[test]
fn row_7_the_view_dropdown_measures_208_u_and_is_the_widest() {
    for platform in PLATFORMS {
        let widths: Vec<(&str, i64)> = MENUS
            .iter()
            .map(|m| (m.title, dropdown_width_u(m.items, platform)))
            .collect();
        let see = widths
            .iter()
            .find(|(t, _)| *t == "Ver")
            .expect("menú Ver")
            .1;
        assert_eq!(see, 12 + (20 + 2 + 6) * 7, "{platform:?}");
        assert_eq!(see, 208);
        for (title, width_2) in widths {
            assert!(width_2 <= 208, "{title} en {platform:?}: {width_2}");
            assert!(width_2 < min_window_width_u(), "{title} en {platform:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: window_layout
// ---------------------------------------------------------------------------------------------

fn rect(x: i64, y: i64, width: i64, height: i64) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width,
        height,
    }
}

fn area(r: ScreenRect) -> i64 {
    r.width * r.height
}

fn overlap_2(a: ScreenRect, b: ScreenRect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

fn inside(r: ScreenRect, window: (i64, i64)) -> bool {
    r.x >= 0 && r.y >= 0 && r.x + r.width <= window.0 && r.y + r.height <= window.1
}

fn five(layout: &WindowLayout) -> [(&'static str, ScreenRect); 5] {
    [
        ("menu", layout.menu),
        ("toolbar", layout.toolbar),
        ("panel", layout.panel),
        ("canvas", layout.canvas),
        ("status", layout.status),
    ]
}

/// Invariantes de `window_layout` con la ventana de `window` píxeles físicos y la escala `n`.
fn checks_layout(window: (i64, i64), n: u32) -> Result<(), TestCaseError> {
    let k = i64::from(n);
    let (w, h) = window;
    let layout = window_layout(window, n);
    let band = h - (MENU_H_U + 20) * k;
    // Medidas de la spec: menú de 16·n, barra de 20·n, columna de 107·n y panel de 233·n, a la derecha del lienzo.
    prop_assert_eq!(layout.menu, rect(0, 0, w, MENU_H_U * k));
    prop_assert_eq!(layout.status, rect(0, h - 20 * k, w, 20 * k));
    prop_assert_eq!(
        layout.toolbar,
        rect(
            w - toolbar_width_u() * k,
            MENU_H_U * k,
            toolbar_width_u() * k,
            band
        )
    );
    prop_assert_eq!(
        layout.panel,
        rect(
            w - (toolbar_width_u() + PANEL_WIDTH_U) * k,
            MENU_H_U * k,
            PANEL_WIDTH_U * k,
            band
        )
    );
    // El lienzo es lo que queda tras quitar menú, columna, panel y barra.
    prop_assert_eq!(
        layout.canvas,
        rect(
            0,
            MENU_H_U * k,
            w - (toolbar_width_u() + PANEL_WIDTH_U) * k,
            band
        )
    );
    prop_assert_eq!(canvas_origin_px(n), (0, 16 * k));
    prop_assert_eq!(canvas_origin_px(n), (layout.canvas.x, layout.canvas.y));

    // Recubren la ventana sin huecos ni solapes: dentro, disjuntos y con el área total.
    let rects = five(&layout);
    let mut sum = 0;
    for (name_2, r) in rects {
        prop_assert!(r.width >= 0 && r.height >= 0, "{}: {:?}", name_2, r);
        prop_assert!(inside(r, window), "{} fuera de la ventana: {:?}", name_2, r);
        sum += area(r);
    }
    prop_assert_eq!(sum, w * h, "huecos o solapes");
    for (i, (na, a)) in rects.iter().enumerate() {
        for (nb, b) in &rects[i + 1..] {
            prop_assert!(!overlap_2(*a, *b), "{} y {} se solapan", na, nb);
        }
    }
    Ok(())
}

fn checks(window: (i64, i64), n: u32) {
    if let Err(e) = checks_layout(window, n) {
        panic!("{e}");
    }
}

/// Ventana por defecto (960 × 720 pt) y mínima, en píxeles físicos, a `ppp`.
fn windows(ppp: f32) -> [(i64, i64); 2] {
    let n = n_de(ppp);
    let minimum = (
        min_window_width_u() * n,
        i64::from(bar_height_px(n as u32)) + MENU_H_U * n + min_panel_height_px(n as u32),
    );
    let default = ((960.0 * ppp).round() as i64, (720.0 * ppp).round() as i64);
    [default, minimum]
}

#[test]
fn row_7_window_layout_meets_the_geometry_at_the_five_scales() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        for window in windows(ppp) {
            checks(window, n);
        }
    }
}

#[test]
fn row_7_the_canvas_origin_is_0_16n() {
    for (ppp, source_2) in PPPS
        .iter()
        .zip([(0, 16), (0, 16), (0, 32), (0, 32), (0, 48)])
    {
        assert_eq!(canvas_origin_px(text_scale(*ppp)), source_2, "ppp {ppp}");
    }
}

#[test]
fn row_7_the_column_and_the_panel_take_the_height_of_the_central_strip() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for window in windows(ppp) {
            let layout = window_layout(window, n as u32);
            assert_eq!(layout.toolbar.height, window.1 - 36 * n);
            assert_eq!(layout.panel.height, layout.toolbar.height);
            assert_eq!(layout.canvas.height, layout.toolbar.height);
            assert!(layout.toolbar.height >= TOOLBAR_CONTENT_H_U * n);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: mínimos de ventana
// ---------------------------------------------------------------------------------------------

#[test]
fn row_8_minimum_height_at_the_five_scales() {
    for (ppp, expected_2) in PPPS.iter().zip(ALTOS_PT) {
        let real = min_window_height_pt(*ppp);
        assert!(
            near_pt(real, expected_2),
            "ppp {ppp}: {real} y no {expected_2}"
        );
    }
}

#[test]
fn row_8_minimum_width_in_u_and_in_pt_at_the_five_scales() {
    assert_eq!(min_window_width_u(), 468);
    assert_eq!(
        min_window_width_u(),
        toolbar_width_u() + PANEL_WIDTH_U + MIN_CANVAS_WIDTH_U
    );
    for (ppp, expected_2) in PPPS.iter().zip(WIDTHS_PT) {
        let real = min_window_width_pt(*ppp);
        assert!(
            near_pt(real, expected_2),
            "ppp {ppp}: {real} y no {expected_2}"
        );
    }
}

#[test]
fn row_8_an_invalid_ppp_gives_the_minimum_of_1_0() {
    for ppp in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1.0, -0.5] {
        assert!(near_pt(min_window_height_pt(ppp), 341.0), "alto con {ppp}");
        assert!(near_pt(min_window_width_pt(ppp), 468.0), "ancho con {ppp}");
    }
}

#[test]
fn row_8_the_height_does_not_pass_65_percent_of_720_and_the_width_does_not_pass_683() {
    for ppp in PPPS {
        assert!(min_window_height_pt(ppp) <= 468.0 + 0.01, "alto con {ppp}");
        assert!(min_window_width_pt(ppp) <= 683.0, "ancho con {ppp}");
    }
}

#[test]
fn row_8_the_height_is_bar_plus_menu_plus_minimum_panel_and_the_width_column_panel_and_128_u() {
    for ppp in PPPS {
        let n = n_de(ppp);
        let height_2 = (i64::from(bar_height_px(n as u32))
            + MENU_H_U * n
            + min_panel_height_px(n as u32)) as f32
            / ppp;
        assert!(
            near_pt(min_window_height_pt(ppp), height_2),
            "alto con {ppp}"
        );
        let width_2 = ((toolbar_width_u() + PANEL_WIDTH_U + 128) * n) as f32 / ppp;
        assert!(
            near_pt(min_window_width_pt(ppp), width_2),
            "ancho con {ppp}"
        );
    }
}

#[test]
fn row_8_the_minimum_canvas_measures_128n_by_305n_pixels() {
    for (ppp, expected_2) in
        PPPS.iter()
            .zip([(128, 305), (128, 305), (256, 610), (256, 610), (384, 915)])
    {
        let n = n_de(*ppp);
        let minimum = windows(*ppp)[1];
        let layout = window_layout(minimum, n as u32);
        assert_eq!(
            (layout.canvas.width, layout.canvas.height),
            expected_2,
            "ppp {ppp}"
        );
        assert_eq!(
            (layout.canvas.width, layout.canvas.height),
            (128 * n, 305 * n)
        );
    }
}

#[test]
fn row_8_the_minimum_window_in_physical_pixels_is_whole_at_the_five_scales() {
    for ppp in PPPS {
        let width_2 = min_window_width_pt(ppp) * ppp;
        let height_2 = min_window_height_pt(ppp) * ppp;
        assert!(
            (width_2 - width_2.round()).abs() <= 0.001,
            "ancho con {ppp}"
        );
        assert!(
            (height_2 - height_2.round()).abs() <= 0.001,
            "alto con {ppp}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3
// ---------------------------------------------------------------------------------------------

/// `ppp` de 1,0 a 4,0 en pasos de 0,05 sin acumular error de coma flotante.
fn ppp_of_the_step(i: u32) -> f32 {
    (100 + 5 * i) as f32 / 100.0
}

#[test]
fn property_3_the_minimums_follow_the_formula_and_meet_the_two_rules_at_all_ppp() {
    for i in 0..=60 {
        let ppp = ppp_of_the_step(i);
        let n = n_de(ppp);
        let height_2 = ((20 + MENU_H_U + 305) * n) as f32 / ppp;
        let width_2 = ((toolbar_width_u() + PANEL_WIDTH_U + 128) * n) as f32 / ppp;
        assert!(
            near_pt(min_window_height_pt(ppp), height_2),
            "alto con {ppp}"
        );
        assert!(
            near_pt(min_window_width_pt(ppp), width_2),
            "ancho con {ppp}"
        );
        assert!(min_window_height_pt(ppp) <= 468.0 + 0.01, "alto con {ppp}");
        assert!(min_window_width_pt(ppp) <= 683.0, "ancho con {ppp}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_3_the_split_is_integer_and_covers_any_window_from_the_minimum(
        step in 0u32..=60,
        extra_width in 0i64..=3000,
        extra_height in 0i64..=3000,
    ) {
        let ppp = ppp_of_the_step(step);
        let n = text_scale(ppp);
        let k = i64::from(n);
        let minimum = (
            min_window_width_u() * k,
            i64::from(bar_height_px(n)) + MENU_H_U * k + min_panel_height_px(n),
        );
        let window = (minimum.0 + extra_width, minimum.1 + extra_height);
        checks_layout(window, n)?;
        // El lienzo nunca baja de su mínimo.
        let layout = window_layout(window, n);
        prop_assert!(layout.canvas.width >= MIN_CANVAS_WIDTH_U * k);
        prop_assert!(layout.canvas.height >= min_panel_height_px(n));
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad I.3 (specs/interfaz-caja-derecha.md): orden de las zonas, mínimos y geometría de la caja
// ---------------------------------------------------------------------------------------------

mod toolbox_right {
    use super::*;
    use px_ui::chrome::{
        OPTIONS_BODY_H_U, OPTIONS_BODY_ROWS, OPTIONS_BODY_W_U, OPTIONS_BODY_X_U, OPTIONS_BODY_Y_U,
        OPTIONS_BORDER_U, OPTIONS_FOCUS_BORDER_U, OPTIONS_H_U, OPTIONS_KEY_Y_U, OPTIONS_ROW_H_U,
        OPTIONS_TITLE_X_U, OPTIONS_TITLE_Y_U, OPTIONS_W_U, OPTIONS_X_U, OPTIONS_Y_U, ZOOM_Y_U,
    };

    const HEIGHT_MAX_PT: f32 = 468.0;
    const WIDTH_MAX_PT: f32 = 683.0;

    /// Fila 1: **la única prueba que conoce el orden de las zonas** (lienzo, panel, caja).
    #[test]
    fn row_1_the_order_is_canvas_panel_and_toolbox_on_the_right_edge() {
        for ppp in PPPS {
            let n = n_de(ppp);
            for window in windows(ppp) {
                let l = window_layout(window, n as u32);
                assert_eq!(l.canvas.x, 0, "ppp {ppp}");
                assert_eq!(l.canvas.x + l.canvas.width, l.panel.x, "ppp {ppp}");
                assert_eq!(l.panel.x + l.panel.width, l.toolbar.x, "ppp {ppp}");
                assert_eq!(l.toolbar.x + l.toolbar.width, window.0, "ppp {ppp}");
                assert_eq!(l.panel.width, 233 * n);
                assert_eq!(l.toolbar.width, 107 * n);
                assert_eq!(canvas_origin_px(n as u32), (0, 16 * n));
            }
        }
    }

    /// Fila 2: el alto mínimo baja (I.4), el ancho no cambia y los dos techos se cumplen; el
    /// contenido de la caja cabe.
    #[test]
    fn row_2_the_minimum_height_goes_down_the_width_does_not_and_they_meet_the_ceilings() {
        for ((ppp, height_2), width_2) in PPPS.iter().zip(ALTOS_PT).zip(WIDTHS_PT) {
            assert!(near_pt(min_window_height_pt(*ppp), height_2), "ppp {ppp}");
            assert!(near_pt(min_window_width_pt(*ppp), width_2), "ppp {ppp}");
            assert!(min_window_height_pt(*ppp) <= HEIGHT_MAX_PT, "ppp {ppp}");
            assert!(min_window_width_pt(*ppp) <= WIDTH_MAX_PT, "ppp {ppp}");
            let n = n_de(*ppp);
            assert!(
                TOOLBAR_CONTENT_H_U * n <= min_panel_height_px(n as u32),
                "ppp {ppp}: la caja no cabe en el panel mínimo"
            );
        }
        for invalid in [f32::NAN, 0.0, -1.0, f32::INFINITY] {
            assert!(near_pt(min_window_height_pt(invalid), 341.0));
            assert!(near_pt(min_window_width_pt(invalid), 468.0));
        }
        assert_eq!(min_window_width_u(), 468);
    }

    /// Fila 3: las constantes y las relaciones del recuadro de opciones.
    #[test]
    fn row_3_the_options_box_has_the_geometry_of_the_spec() {
        assert_eq!(
            (OPTIONS_X_U, OPTIONS_Y_U, OPTIONS_W_U, OPTIONS_H_U),
            (4, 108, 99, 138)
        );
        assert_eq!(OPTIONS_W_U, toolbar_button_width_u());
        assert_eq!((OPTIONS_BORDER_U, OPTIONS_FOCUS_BORDER_U), (1, 2));
        assert_eq!((OPTIONS_TITLE_X_U, OPTIONS_TITLE_Y_U), (8, 112));
        assert_eq!(OPTIONS_KEY_Y_U, 126);
        assert_eq!(
            (OPTIONS_BODY_X_U, OPTIONS_BODY_Y_U, OPTIONS_BODY_W_U),
            (8, 144, 91)
        );
        assert_eq!(OPTIONS_BODY_ROWS, 7);
        assert_eq!(OPTIONS_ROW_H_U, 14);
        assert_eq!(OPTIONS_BODY_H_U, 7 * 14);
        // 4 u bajo la fila de zoom (termina en 104) y 4 u de margen inferior.
        assert_eq!(OPTIONS_Y_U, ZOOM_Y_U + 24 + 4);
        assert_eq!(OPTIONS_Y_U + OPTIONS_H_U + 4, TOOLBAR_CONTENT_H_U);
        // Dentro del recuadro: margen de 4 u (borde y relleno) en los cuatro lados.
        assert_eq!(OPTIONS_TITLE_X_U, OPTIONS_X_U + 4);
        assert_eq!(OPTIONS_TITLE_Y_U, OPTIONS_Y_U + 4);
        assert_eq!(OPTIONS_KEY_Y_U, OPTIONS_TITLE_Y_U + 14);
        assert_eq!(OPTIONS_BODY_Y_U, OPTIONS_KEY_Y_U + 14 + 4);
        assert_eq!(
            OPTIONS_BODY_X_U + OPTIONS_BODY_W_U + 4,
            OPTIONS_X_U + OPTIONS_W_U
        );
        assert_eq!(
            OPTIONS_BODY_Y_U + OPTIONS_BODY_H_U + 4,
            OPTIONS_Y_U + OPTIONS_H_U
        );
        // Las 13 columnas de 7 u del cuerpo.
        assert_eq!(OPTIONS_BODY_W_U, 13 * 7);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Propiedad 1: reparto entero, adyacente y recubridor con cualquier ancho (también impar).
        #[test]
        fn property_1_the_split_with_the_toolbox_on_the_right_is_integer_adjacent_and_fits(
            step in 0u32..=60,
            extra_w in 0i64..=700,
            extra_h in 0i64..=500,
        ) {
            let ppp = 1.0 + step as f32 * 0.05;
            let n = text_scale(ppp);
            let k = i64::from(n);
            let minimum = (
                min_window_width_u() * k,
                i64::from(bar_height_px(n)) + MENU_H_U * k + min_panel_height_px(n),
            );
            let window = (minimum.0 + extra_w, minimum.1 + extra_h);
            checks_layout(window, n)?;
            let l = window_layout(window, n);
            prop_assert_eq!(l.toolbar.x + l.toolbar.width, window.0);
            prop_assert_eq!(l.panel.x + l.panel.width, l.toolbar.x);
            prop_assert_eq!(l.canvas.x, 0);
            prop_assert_eq!(l.canvas.x + l.canvas.width, l.panel.x);
            prop_assert_eq!(canvas_origin_px(n), (0, 16 * k));
            prop_assert!(l.toolbar.height >= TOOLBAR_CONTENT_H_U * k);
            // Alto y ancho mínimos: la fórmula y los dos techos.
            let height_2 = (i64::from(bar_height_px(n)) + MENU_H_U * k + min_panel_height_px(n)) as f32 / ppp;
            prop_assert!((min_window_height_pt(ppp) - height_2).abs() <= 0.01);
            prop_assert!(min_window_height_pt(ppp) <= HEIGHT_MAX_PT);
            let width_2 = (min_window_width_u() * k) as f32 / ppp;
            prop_assert!((min_window_width_pt(ppp) - width_2).abs() <= 0.01);
            prop_assert!(min_window_width_pt(ppp) <= WIDTH_MAX_PT);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad I.4 (specs/interfaz-iconos.md): rejilla de iconos de la caja, mínimos y regla del alto
// ---------------------------------------------------------------------------------------------

mod icons {
    use super::*;
    use px_core::Mode;
    use px_ui::chrome::{
        OPTIONS_BODY_ROWS, OPTIONS_BODY_X_U, OPTIONS_BODY_Y_U, OPTIONS_H_U, OPTIONS_KEY_Y_U,
        OPTIONS_TITLE_Y_U, OPTIONS_W_U, OPTIONS_X_U, OPTIONS_Y_U, TOOL_ROWS, TOOL_SLOTS,
        TOOLBAR_SEPARATOR_Y_U, TOOLS_Y_U, ZOOM_LABEL_Y_U, ZOOM_Y_U, tool_slot_u,
        toolbox_button_rect_u, toolbox_hit,
    };
    use px_ui::edit_layout::block_height_px;

    type RectU = (i64, i64, i64, i64);

    fn overlap(a: RectU, b: RectU) -> bool {
        a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
    }

    /// Los nueve botones escritos con los literales de la spec: casillas de 24 × 24 u con un hueco
    /// de 1 u (herramientas en las casillas 0 a 4; zoom en una fila en y = 80).
    fn buttons_of_the_spec() -> Vec<RectU> {
        let tools = (0..5).map(|i| (4 + 25 * (i % 4), 4 + 25 * (i / 4), 24, 24));
        let zoom = (0..4).map(|i| (4 + 25 * i, 80, 24, 24));
        tools.chain(zoom).collect()
    }

    #[test]
    fn row_3_the_toolbox_has_the_geometry_of_the_icon_grid() {
        assert_eq!((TOOLBOX_COLUMNS, TOOL_ROWS, TOOL_SLOTS), (4, 2, 8));
        assert_eq!((BUTTON_U, GAP_U), (24, 1));
        for slot in 0..TOOL_SLOTS {
            let slot = slot as i64;
            assert_eq!(
                tool_slot_u(slot as usize),
                (4 + 25 * (slot % 4), 4 + 25 * (slot / 4)),
                "casilla {slot}"
            );
        }
        let expected_2_2 = buttons_of_the_spec();
        for (i, expected_2) in expected_2_2.iter().enumerate() {
            assert_eq!(toolbox_button_rect_u(i), *expected_2, "botón {i}");
        }
        assert_eq!(TOOLS_Y_U, 4);
        assert_eq!(TOOLBAR_SEPARATOR_Y_U, 57);
        assert_eq!(ZOOM_LABEL_Y_U, 62);
        assert_eq!(ZOOM_Y_U, 80);
        // El recuadro de opciones: el mismo de I.3 en y = 108.
        assert_eq!(
            (OPTIONS_X_U, OPTIONS_Y_U, OPTIONS_W_U, OPTIONS_H_U),
            (4, 108, 99, 138)
        );
        assert_eq!((OPTIONS_TITLE_Y_U, OPTIONS_KEY_Y_U), (112, 126));
        assert_eq!(
            (OPTIONS_BODY_X_U, OPTIONS_BODY_Y_U, OPTIONS_BODY_ROWS),
            (8, 144, 7)
        );
        assert_eq!(TOOLBAR_CONTENT_H_U, 108 + 138 + 4);
        assert_eq!(TOOLBAR_CONTENT_H_U, 250);
        assert_eq!(toolbar_button_width_u(), 99);
        assert_eq!(toolbar_width_u(), 107);
        // Capacidad para el pincel del hito 4: 5 herramientas y el pincel caben en las casillas.
        assert!(TOOL_SLOTS > TOOL_BUTTONS.len());
        // Ninguna pieza se solapa ni toca el borde de la caja (4 u de margen a cada lado).
        let mut pieces = expected_2_2;
        pieces.push((4, 57, 99, 1)); // separador
        pieces.push((8, 62, 91, 14)); // texto del zoom
        pieces.push((OPTIONS_X_U, OPTIONS_Y_U, OPTIONS_W_U, OPTIONS_H_U));
        for (i, a) in pieces.iter().enumerate() {
            assert!(a.0 >= 4 && a.0 + a.2 <= 107 - 4, "pieza {i}: {a:?}");
            assert!(
                a.1 >= 4 && a.1 + a.3 <= TOOLBAR_CONTENT_H_U - 4,
                "pieza {i}: {a:?}"
            );
            for (j, b) in pieces.iter().enumerate().skip(i + 1) {
                assert!(
                    !overlap(*a, *b),
                    "las piezas {i} y {j} se solapan: {a:?} {b:?}"
                );
            }
        }
        // Todo es entero y múltiplo de `n` desde la esquina de la caja, a las cinco escalas.
        for ppp in PPPS {
            let n = n_de(ppp);
            for (i, p) in pieces.iter().enumerate() {
                for u in [p.0, p.1, p.2, p.3] {
                    let physical = (u * n) as f32 / ppp * ppp;
                    assert!(
                        (physical - physical.round()).abs() <= 0.001,
                        "ppp {ppp}, pieza {i}"
                    );
                }
            }
        }
    }

    #[test]
    fn row_5_the_minimums_and_the_toolbox_height_rule_with_four_and_with_five_rows() {
        for ((ppp, height_2), width_2) in PPPS.iter().zip(ALTOS_PT).zip(WIDTHS_PT) {
            assert!(
                near_pt(min_window_height_pt(*ppp), height_2),
                "alto con {ppp}"
            );
            assert!(
                near_pt(min_window_width_pt(*ppp), width_2),
                "ancho con {ppp}"
            );
            assert!(min_window_height_pt(*ppp) <= 468.0 + 0.01, "alto con {ppp}");
            assert!(min_window_width_pt(*ppp) <= 683.0, "ancho con {ppp}");
            let n = n_de(*ppp);
            assert_eq!(block_height_px(Mode::Indexed, n as u32), 305 * n);
            assert_eq!(min_panel_height_px(n as u32), 305 * n);
            // La caja de hoy cabe con holgura (55 u)…
            assert!(
                TOOLBAR_CONTENT_H_U * n <= min_panel_height_px(n as u32),
                "ppp {ppp}"
            );
            // …con 4 filas de herramientas (16) aún cabe, con 5 filas (17 a 20) la regla falla:
            // lo que guarda el recuadro de 138 u hasta que el hito 4 decida.
            let with_rows = |rows_2: i64| (TOOLBAR_CONTENT_H_U + (rows_2 - TOOL_ROWS) * 25) * n;
            assert!(
                with_rows(4) <= min_panel_height_px(n as u32),
                "ppp {ppp}: 4 filas"
            );
            assert!(
                with_rows(5) > min_panel_height_px(n as u32),
                "ppp {ppp}: 5 filas"
            );
        }
        for invalid in [f32::NAN, f32::INFINITY, 0.0, -1.0] {
            assert!(near_pt(min_window_height_pt(invalid), 341.0));
            assert!(near_pt(min_window_width_pt(invalid), 468.0));
        }
        assert_eq!(min_window_width_u(), 468);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Propiedad 1: geometría entera, sin solapes, que cabe, y mínimos con su fórmula y techos.
        #[test]
        fn property_1_the_toolbox_and_the_minimums_are_integers_fit_and_meet_the_ceilings(
            step in 0u32..=60,
            extra_w in 0i64..=700,
            extra_h in 0i64..=500,
        ) {
            let ppp = ppp_of_the_step(step);
            let n = text_scale(ppp);
            let k = i64::from(n);
            let minimum = (
                min_window_width_u() * k,
                i64::from(bar_height_px(n)) + MENU_H_U * k + min_panel_height_px(n),
            );
            let window = (minimum.0 + extra_w, minimum.1 + extra_h);
            checks_layout(window, n)?;
            let l = window_layout(window, n);
            // Cada botón, el separador, el texto del zoom y el recuadro caben en la caja y no se
            // solapan; el contenido cabe en el alto de la caja.
            let mut pieces: Vec<RectU> = (0..9).map(toolbox_button_rect_u).collect();
            pieces.push((4, 57, 99, 1));
            pieces.push((8, 62, 91, 14));
            pieces.push((OPTIONS_X_U, OPTIONS_Y_U, OPTIONS_W_U, OPTIONS_H_U));
            for (i, a) in pieces.iter().enumerate() {
                prop_assert!(a.0 >= 0 && (a.0 + a.2) * k <= l.toolbar.width);
                prop_assert!((a.1 + a.3) * k <= l.toolbar.height);
                for b in pieces.iter().skip(i + 1) {
                    prop_assert!(!overlap(*a, *b));
                }
            }
            prop_assert!(TOOLBAR_CONTENT_H_U * k <= l.toolbar.height);
            prop_assert!(block_height_px(Mode::Indexed, n) <= l.panel.height);
            let height_2 = ((20 + MENU_H_U + 305) * k) as f32 / ppp;
            prop_assert!((min_window_height_pt(ppp) - height_2).abs() <= 0.01);
            prop_assert!(min_window_height_pt(ppp) <= 468.0 + 0.01);
            let width_2 = (min_window_width_u() * k) as f32 / ppp;
            prop_assert!((min_window_width_pt(ppp) - width_2).abs() <= 0.01);
            prop_assert!(min_window_width_pt(ppp) <= 683.0);
        }

        /// Propiedad 3 (la caja): un punto golpea como mucho un botón y exactamente el de su
        /// rectángulo; en los huecos no golpea nada; es total.
        #[test]
        fn property_3_toolbox_hit_gives_the_button_of_its_rectangle_and_nothing_in_the_gaps(
            n in 1u32..=8,
            x in -60i64..=1000,
            y in -60i64..=2200,
        ) {
            let k = i64::from(n);
            let rects = buttons_of_the_spec();
            let expected_2 = rects
                .iter()
                .position(|r| x >= r.0 * k && x < (r.0 + r.2) * k && y >= r.1 * k && y < (r.1 + r.3) * k);
            prop_assert_eq!(toolbox_hit(x, y, n), expected_2);
        }
    }
}
