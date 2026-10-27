//! Tests de la geometría de la ventana `px_ui::chrome` y de los mínimos de ventana
//! (specs/hito-2-menu-herramientas.md, «Casos» filas 7 y 8 y propiedad 3; C2,
//! specs/ui-single-column.md: una sola columna de ancho ajustable). Sin tipos de egui: `chrome`
//! es aritmética entera.
//!
//! - Fila 7 (parte pura): el ancho de la columna es `W(k) = 14·k + 9` (la rejilla de la paleta, con
//!   `k` columnas de muestras); los 9 botones caben en la columna sin tocar el borde y no se
//!   solapan; 20 herramientas al mínimo (10 columnas) caben en 277 u; el menú entero y el desplegable más
//!   ancho (208 u) caben en el ancho mínimo; los cinco rectángulos de `window_layout` recubren la
//!   ventana sin huecos ni solapes, a las cinco escalas y en la ventana por defecto y en la
//!   mínima. El banco de egui con la pila real está en `window_stack.rs`.
//! - Fila 8: `min_window_height_pt`, `min_window_width_u` (el mayor de cuatro términos: la columna
//!   mínima y el lienzo mínimo, la barra de menús, el diálogo y las 66 columnas de la barra de
//!   estado) y `min_window_width_pt` a las cinco escalas y con `ppp` inválido; `min_inner_size` es
//!   de `px-app` (`window_min/tests.rs`).
//! - Propiedad 3: geometría entera y recubrimiento con `ppp` de 1,0 a 4,0 en pasos de 0,05.

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_ui::actions::Platform;
use px_ui::chrome::{
    COLUMN_DEFAULT_COLS, COLUMN_MIN_COLS, MENU_BAR_W_U, MENU_H_U, TOOL_LABELS, WindowLayout,
    canvas_origin_px, column_cols, column_width_u, columns_per_row, head_u, tool_rows,
    window_layout, window_layout_cols,
};
use px_ui::dialog_layout::{DIALOG_W_U, MARGIN_U};
use px_ui::edit_layout::{
    MIN_CANVAS_WIDTH_U, block_height_closed_px, min_panel_height_px, min_width_terms_u,
    min_window_height_pt, min_window_width_pt, min_window_width_u,
};
use px_ui::icon_button::{BUTTON_U, GAP_U};
use px_ui::menu::{MENUS, shortcut_for, shortcut_text};
use px_ui::palette_grid::PANEL_WIDTH_U;
use px_ui::status::{ADVANCE_PX, bar_height_px};
use px_ui::text::text_scale;
use px_ui::toolbar::TOOL_BUTTONS;
use px_ui::view::ScreenRect;

const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];
const ALTOS_PT: [f32; 5] = [313.0, 250.4, 417.33, 313.0, 313.0];
const WIDTHS_PT: [f32; 5] = [468.0, 374.4, 624.0, 468.0, 468.0];
const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

fn n_of(ppp: f32) -> i64 {
    i64::from(text_scale(ppp))
}

fn near_pt(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.01
}

/// Herramientas de hoy.
fn tools() -> usize {
    TOOL_LABELS.len()
}

// ---------------------------------------------------------------------------------------------
// Fila 7: constantes y columna
// ---------------------------------------------------------------------------------------------

#[test]
fn row_7_the_constants_are_worth_what_the_spec_says() {
    assert_eq!(MENU_H_U, 16);
    assert_eq!(column_width_u(COLUMN_DEFAULT_COLS), 233);
    assert_eq!(head_u(tools(), COLUMN_DEFAULT_COLS), 84);
    assert_eq!(PANEL_WIDTH_U, 233);
    assert_eq!(bar_height_px(1), 20);
    assert_eq!(min_panel_height_px(1), 277);
}

#[test]
fn row_7_the_column_width_is_derived_from_the_icon_grid() {
    // 4 u de margen, 14 u por columna de muestras, 3 u de pista y 1 u de línea, con un hueco de 1 u.
    assert_eq!((COLUMN_MIN_COLS, COLUMN_DEFAULT_COLS), (10, 16));
    for cols in COLUMN_MIN_COLS..=64 {
        assert_eq!(column_width_u(cols), 4 + 14 * cols + 5, "k = {cols}");
    }
    assert_eq!(column_width_u(10), 149);
    assert_eq!(column_width_u(14), 205);
    assert_eq!(column_width_u(16), 233);
    // Nueve botones de 24 u con un hueco de 1 u llenan justo las 16 columnas de muestras.
    assert_eq!(9 * BUTTON_U + 8 * GAP_U, 16 * 14);
    assert_eq!(column_width_u(16) - 8, 9 * BUTTON_U + 8 * GAP_U + 1);
}

#[test]
fn row_7_tool_labels_are_the_labels_of_tool_buttons_without_the_letter() {
    let labels: Vec<&str> = TOOL_BUTTONS.iter().map(|b| b.label).collect();
    assert_eq!(labels, TOOL_LABELS);
    assert_eq!(
        TOOL_LABELS,
        [
            "Lápiz",
            "Línea",
            "Relleno",
            "Cuentagotas",
            "Borrador",
            "Formas"
        ]
    );
}

#[test]
fn row_7_the_nine_buttons_fit_in_the_column_without_touching_the_edge_and_do_not_overlap() {
    use px_ui::chrome::toolbox_button_rect_u_at;
    // Escrito con los literales de la spec: `c(k) = ⌊(14k + 2)/25⌋` botones por fila; con
    // `r = ⌈5/c⌉` filas, el separador en `25r + 7`, el texto del zoom en `25r + 12` y la fila de zoom
    // en `25r + 30`; la caja mide `25r + 58`.
    for cols in COLUMN_MIN_COLS..=24 {
        let per_row = (14 * cols + 2) / 25;
        let rows = (5 + per_row - 1) / per_row;
        let width_2 = 14 * cols + 9;
        let mut pieces: Vec<(i64, i64, i64, i64)> = (0..5)
            .map(|i| (4 + 25 * (i % per_row), 4 + 25 * (i / per_row), 24, 24))
            .collect();
        pieces.extend((0..4).map(|i| (4 + 25 * i, 25 * rows + 30, 24, 24)));
        for (i, expected_2) in pieces.iter().enumerate() {
            assert_eq!(
                toolbox_button_rect_u_at(i, 5, cols),
                *expected_2,
                "k = {cols}, botón {i}"
            );
        }
        let separator = (4, 25 * rows + 7, width_2 - 8, 1);
        let level = (8, 25 * rows + 12, width_2 - 16, 14);
        pieces.push(separator);
        pieces.push(level);
        assert_eq!(25 * rows + 58, head_u(5, cols) - 1, "k = {cols}");
        for (i, a) in pieces.iter().enumerate() {
            // Margen de 4 u a cada lado y 4 u bajo la fila de zoom: sin tocar el borde.
            assert!(
                a.0 >= 4 && a.0 + a.2 <= width_2 - 4,
                "k = {cols}, pieza {i}: {a:?}"
            );
            assert!(
                a.1 >= 4 && a.1 + a.3 <= 25 * rows + 54,
                "k = {cols}, pieza {i}: {a:?}"
            );
            for b in pieces.iter().skip(i + 1) {
                let overlap =
                    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3;
                assert!(!overlap, "k = {cols}: {a:?} se solapa con {b:?}");
            }
        }
        // Dos botones seguidos de un mismo grupo dejan 1 u entre ellos.
        for i in 0..3 {
            let a = toolbox_button_rect_u_at(5 + i, 5, cols);
            let b = toolbox_button_rect_u_at(5 + i + 1, 5, cols);
            assert_eq!(b.0 - (a.0 + a.2), 1, "k = {cols}");
        }
    }
}

#[test]
fn row_7_20_tools_at_the_minimum_column_fit_in_the_277_u_of_the_minimum_panel() {
    // 20 herramientas al mínimo (10 columnas, `c = 5`) son 4 filas: la caja mide `25·4 + 59 = 159` u
    // y el bloque cerrado de la paleta indexada (hueco, una fila de rejilla y la cola), 106 u: 265 u,
    // con 12 u de holgura bajo los 277 u del mínimo (C4: el panel de capas vuelve a fijarlo).
    assert_eq!(columns_per_row(COLUMN_MIN_COLS), 5);
    assert_eq!(tool_rows(20, COLUMN_MIN_COLS), 4);
    assert_eq!(head_u(20, COLUMN_MIN_COLS), 159);
    assert_eq!(head_u(20, COLUMN_MIN_COLS) + 106, 265);
    for ppp in PPPS {
        let n = n_of(ppp);
        let closed = block_height_closed_px(px_core::Mode::Indexed, n as u32);
        assert_eq!(closed, 106 * n, "ppp {ppp}");
        assert!(
            head_u(20, COLUMN_MIN_COLS) * n + closed <= min_panel_height_px(n as u32),
            "ppp {ppp}: 20 herramientas al mínimo no caben"
        );
        // Con 21 (cinco filas) la regla falla en voz alta, en vez de recortar.
        assert!(
            head_u(21, COLUMN_MIN_COLS) * n + closed > min_panel_height_px(n as u32),
            "ppp {ppp}: 21 herramientas al mínimo"
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
    assert_eq!(MENU_BAR_W_U, total);
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

/// Invariantes de `window_layout_cols` con la ventana de `window` píxeles físicos, la escala `n` y
/// `wanted` columnas elegidas.
fn checks_layout_cols(window: (i64, i64), n: u32, wanted: i64) -> Result<(), TestCaseError> {
    let k = i64::from(n);
    let (w, h) = window;
    let layout = window_layout_cols(window, n, wanted);
    let band = h - (MENU_H_U + 20) * k;
    let cols = column_cols(w, n, wanted);
    let column_w = column_width_u(cols) * k;
    let head = (head_u(tools(), cols) * k).min(band);
    // Medidas de la spec: menú de 16·n, barra de 20·n y una sola columna de `W(k)·n` a la derecha
    // del lienzo, con la caja arriba y la paleta debajo, del mismo `x` y del mismo ancho.
    prop_assert_eq!(layout.menu, rect(0, 0, w, MENU_H_U * k));
    prop_assert_eq!(layout.status, rect(0, h - 20 * k, w, 20 * k));
    prop_assert_eq!(
        layout.toolbar,
        rect(w - column_w, MENU_H_U * k, column_w, head)
    );
    prop_assert_eq!(
        layout.panel,
        rect(w - column_w, MENU_H_U * k + head, column_w, band - head)
    );
    // El lienzo es lo que queda tras quitar menú, columna y barra, y nunca baja de su mínimo.
    prop_assert_eq!(layout.canvas, rect(0, MENU_H_U * k, w - column_w, band));
    if w >= (column_width_u(COLUMN_MIN_COLS) + MIN_CANVAS_WIDTH_U) * k {
        prop_assert!(layout.canvas.width >= MIN_CANVAS_WIDTH_U * k);
    }
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

/// Invariantes de `window_layout` (16 columnas elegidas).
fn checks_layout(window: (i64, i64), n: u32) -> Result<(), TestCaseError> {
    prop_assert_eq!(
        window_layout(window, n),
        window_layout_cols(window, n, COLUMN_DEFAULT_COLS)
    );
    checks_layout_cols(window, n, COLUMN_DEFAULT_COLS)
}

fn checks(window: (i64, i64), n: u32) {
    if let Err(e) = checks_layout(window, n) {
        panic!("{e}");
    }
}

/// Ventana por defecto (960 × 720 pt) y mínima, en píxeles físicos, a `ppp`.
fn windows(ppp: f32) -> [(i64, i64); 2] {
    let n = n_of(ppp);
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
        let n = n_of(ppp);
        for window in windows(ppp) {
            let layout = window_layout(window, n as u32);
            let cols = column_cols(window.0, n as u32, COLUMN_DEFAULT_COLS);
            // La caja y la paleta, apiladas, suman el alto de la franja central.
            assert_eq!(layout.toolbar.height, head_u(tools(), cols) * n);
            assert_eq!(layout.toolbar.y + layout.toolbar.height, layout.panel.y);
            assert_eq!(
                layout.toolbar.height + layout.panel.height,
                window.1 - 36 * n
            );
            assert_eq!(layout.canvas.height, window.1 - 36 * n);
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
    for (ppp, expected_2) in PPPS.iter().zip(WIDTHS_PT) {
        let real = min_window_width_pt(*ppp);
        assert!(
            near_pt(real, expected_2),
            "ppp {ppp}: {real} y no {expected_2}"
        );
    }
}

#[test]
fn row_8_minimum_width_term_the_minimum_column_and_the_minimum_canvas() {
    assert_eq!(column_width_u(COLUMN_MIN_COLS) + MIN_CANVAS_WIDTH_U, 277);
    assert!(min_window_width_u() >= 277);
}

#[test]
fn row_8_minimum_width_term_the_menu_bar_and_its_margin() {
    assert_eq!(MENU_BAR_W_U + 4, 293);
    assert!(min_window_width_u() >= 293);
}

#[test]
fn row_8_minimum_width_term_the_dialog_and_its_two_margins() {
    assert_eq!(DIALOG_W_U + 2 * MARGIN_U, 346);
    assert!(min_window_width_u() >= 346);
}

#[test]
fn row_8_minimum_width_term_the_status_bar_keeps_its_66_columns() {
    // 66 columnas de 7 u y el relleno de 3 u a cada lado (la barra de estado de siempre).
    assert_eq!(66 * i64::from(ADVANCE_PX) + 2 * 3, 468);
    assert_eq!(min_window_width_u(), 468);
    assert_eq!(min_width_terms_u(), [277, 293, 346, 468]);
}

#[test]
fn row_8_an_invalid_ppp_gives_the_minimum_of_1_0() {
    for ppp in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1.0, -0.5] {
        assert!(near_pt(min_window_height_pt(ppp), 313.0), "alto con {ppp}");
        assert!(near_pt(min_window_width_pt(ppp), 468.0), "ancho con {ppp}");
    }
}

#[test]
fn row_8_the_height_does_not_pass_60_percent_of_720_and_the_width_does_not_pass_683() {
    for ppp in PPPS {
        assert!(min_window_height_pt(ppp) <= 432.0 + 0.01, "alto con {ppp}");
        assert!(min_window_width_pt(ppp) <= 683.0, "ancho con {ppp}");
    }
}

#[test]
fn row_8_the_height_is_bar_plus_menu_plus_minimum_panel_and_the_width_the_widest_term() {
    for ppp in PPPS {
        let n = n_of(ppp);
        let height_2 = (i64::from(bar_height_px(n as u32))
            + MENU_H_U * n
            + min_panel_height_px(n as u32)) as f32
            / ppp;
        assert!(
            near_pt(min_window_height_pt(ppp), height_2),
            "alto con {ppp}"
        );
        let width_2 = (468 * n) as f32 / ppp;
        assert!(
            near_pt(min_window_width_pt(ppp), width_2),
            "ancho con {ppp}"
        );
    }
}

#[test]
fn row_8_the_minimum_canvas_measures_235n_by_277n_pixels() {
    // A 468 u caben las 16 columnas elegidas (233 u) y el lienzo queda en 235 u.
    for (ppp, expected_2) in
        PPPS.iter()
            .zip([(235, 277), (235, 277), (470, 554), (470, 554), (705, 831)])
    {
        let n = n_of(*ppp);
        let minimum = windows(*ppp)[1];
        let layout = window_layout(minimum, n as u32);
        assert_eq!(
            (layout.canvas.width, layout.canvas.height),
            expected_2,
            "ppp {ppp}"
        );
        assert_eq!(
            (layout.canvas.width, layout.canvas.height),
            (235 * n, 277 * n)
        );
        assert!(layout.canvas.width >= MIN_CANVAS_WIDTH_U * n);
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
        let n = n_of(ppp);
        let height_2 = ((20 + MENU_H_U + 277) * n) as f32 / ppp;
        let width_2 = (468 * n) as f32 / ppp;
        assert!(
            near_pt(min_window_height_pt(ppp), height_2),
            "alto con {ppp}"
        );
        assert!(
            near_pt(min_window_width_pt(ppp), width_2),
            "ancho con {ppp}"
        );
        assert!(min_window_height_pt(ppp) <= 432.0 + 0.01, "alto con {ppp}");
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
// Unidad I.3 (specs/interfaz-caja-derecha.md): orden de las zonas y mínimos
// ---------------------------------------------------------------------------------------------

mod toolbox_right {
    use super::*;

    const HEIGHT_MAX_PT: f32 = 432.0;
    const WIDTH_MAX_PT: f32 = 683.0;

    /// Fila 1: **la única prueba que conoce el orden de las zonas** (lienzo, columna).
    #[test]
    fn row_1_the_order_is_canvas_and_column_on_the_right_edge() {
        for ppp in PPPS {
            let n = n_of(ppp);
            for window in windows(ppp) {
                let l = window_layout(window, n as u32);
                let cols = column_cols(window.0, n as u32, COLUMN_DEFAULT_COLS);
                assert_eq!(l.canvas.x, 0, "ppp {ppp}");
                assert_eq!(l.canvas.x + l.canvas.width, l.panel.x, "ppp {ppp}");
                assert_eq!(l.panel.x, l.toolbar.x, "ppp {ppp}");
                assert_eq!(l.panel.width, l.toolbar.width, "ppp {ppp}");
                assert_eq!(l.panel.x + l.panel.width, window.0, "ppp {ppp}");
                assert_eq!(l.panel.width, column_width_u(cols) * n);
                assert_eq!(canvas_origin_px(n as u32), (0, 16 * n));
            }
        }
    }

    /// Fila 2: el alto y el ancho mínimos no cambian y los dos techos se cumplen; la caja cabe.
    #[test]
    fn row_2_the_minimums_do_not_change_and_they_meet_the_ceilings() {
        for ((ppp, height_2), width_2) in PPPS.iter().zip(ALTOS_PT).zip(WIDTHS_PT) {
            assert!(near_pt(min_window_height_pt(*ppp), height_2), "ppp {ppp}");
            assert!(near_pt(min_window_width_pt(*ppp), width_2), "ppp {ppp}");
            assert!(min_window_height_pt(*ppp) <= HEIGHT_MAX_PT, "ppp {ppp}");
            assert!(min_window_width_pt(*ppp) <= WIDTH_MAX_PT, "ppp {ppp}");
            let n = n_of(*ppp);
            assert!(
                head_u(20, COLUMN_MIN_COLS) * n + 106 * n <= min_panel_height_px(n as u32),
                "ppp {ppp}: la caja no cabe en el panel mínimo"
            );
        }
        for invalid in [f32::NAN, 0.0, -1.0, f32::INFINITY] {
            assert!(near_pt(min_window_height_pt(invalid), 313.0));
            assert!(near_pt(min_window_width_pt(invalid), 468.0));
        }
        assert_eq!(min_window_width_u(), 468);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Propiedad 1: reparto entero, adyacente y recubridor con cualquier ancho (también impar).
        #[test]
        fn property_1_the_split_with_the_column_on_the_right_is_integer_adjacent_and_fits(
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
            prop_assert_eq!(l.panel.x + l.panel.width, window.0);
            prop_assert_eq!(l.panel.x, l.toolbar.x);
            prop_assert_eq!(l.canvas.x, 0);
            prop_assert_eq!(l.canvas.x + l.canvas.width, l.panel.x);
            prop_assert_eq!(canvas_origin_px(n), (0, 16 * k));
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
        TOOLS_Y_U, toolbox_button_rect_u, toolbox_button_rect_u_at, toolbox_height_u, toolbox_hit,
        toolbox_hit_at, toolbox_separator_y_u, zoom_label_y_u, zoom_row_y_u,
    };
    use px_ui::edit_layout::block_height_px;

    type RectU = (i64, i64, i64, i64);

    fn overlap(a: RectU, b: RectU) -> bool {
        a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
    }

    /// Los botones (`tools` herramientas y cuatro de zoom) escritos con los literales de la spec:
    /// casillas de 24 × 24 u con un hueco de 1 u, `c(k) = ⌊(14k + 2)/25⌋` por fila (herramientas) y
    /// el zoom en una fila debajo.
    fn buttons_of_the_spec(cols: i64, tools: i64) -> Vec<RectU> {
        let per_row = (14 * cols + 2) / 25;
        let rows = (tools + per_row - 1) / per_row;
        let tools = (0..tools).map(|i| (4 + 25 * (i % per_row), 4 + 25 * (i / per_row), 24, 24));
        let zoom = (0..4).map(|i| (4 + 25 * i, 25 * rows + 30, 24, 24));
        tools.chain(zoom).collect()
    }

    #[test]
    fn row_3_the_toolbox_has_the_geometry_of_the_icon_grid() {
        assert_eq!((BUTTON_U, GAP_U), (24, 1));
        assert_eq!(TOOLS_Y_U, 4);
        // La tabla de la spec: 5 herramientas con k = 16 (9 por fila, 1 fila), 14 (7 por fila, 1
        // fila) y 8 (4 por fila, 2 filas): separador, texto del zoom, fila de zoom, alto y `head`.
        for (cols, rows, separator, label, zoom, height) in [
            (16, 1, 32, 37, 55, 83),
            (14, 1, 32, 37, 55, 83),
            (8, 2, 57, 62, 80, 108),
        ] {
            assert_eq!(tool_rows(5, cols), rows, "k = {cols}");
            assert_eq!(toolbox_separator_y_u(5, cols), separator, "k = {cols}");
            assert_eq!(zoom_label_y_u(5, cols), label, "k = {cols}");
            assert_eq!(zoom_row_y_u(5, cols), zoom, "k = {cols}");
            assert_eq!(toolbox_height_u(5, cols), height, "k = {cols}");
            assert_eq!(head_u(5, cols), height + 1, "k = {cols}");
            for (i, expected_2) in buttons_of_the_spec(cols, 5).iter().enumerate() {
                assert_eq!(
                    toolbox_button_rect_u_at(i, 5, cols),
                    *expected_2,
                    "k = {cols}, botón {i}"
                );
            }
        }
        // 12 y 20 herramientas (hito 4): el zoom sigue en una sola fila bajo las herramientas.
        for (tools, cols, rows) in [
            (12, 16, 2),
            (12, 8, 3),
            (20, 16, 3),
            (20, 14, 3),
            (20, 8, 5),
        ] {
            assert_eq!(tool_rows(tools, cols), rows, "{tools} a {cols}");
            assert_eq!(head_u(tools, cols), 25 * rows + 59, "{tools} a {cols}");
            let zoom_y = zoom_row_y_u(tools, cols);
            for i in 0..4 {
                let (x, y, w, h) = toolbox_button_rect_u_at(tools + i, tools, cols);
                assert_eq!((x, y, w, h), (4 + 25 * i as i64, zoom_y, 24, 24));
            }
        }
        // Los botones de hoy con el ancho por defecto son los de `toolbox_button_rect_u`.
        for (i, expected_2) in buttons_of_the_spec(16, 6).iter().enumerate() {
            assert_eq!(toolbox_button_rect_u(i), *expected_2, "botón {i}");
        }
        // Todo es entero y múltiplo de `n` desde la esquina de la caja, a las cinco escalas.
        for ppp in PPPS {
            let n = n_of(ppp);
            for cols in [COLUMN_MIN_COLS, 14, 16] {
                for (i, p) in buttons_of_the_spec(cols, 5).iter().enumerate() {
                    for u in [p.0, p.1, p.2, p.3] {
                        let physical = (u * n) as f32 / ppp * ppp;
                        assert!(
                            (physical - physical.round()).abs() <= 0.001,
                            "ppp {ppp}, k = {cols}, botón {i}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn row_5_the_minimums_and_the_toolbox_capacity_with_20_tools_at_the_minimum_column() {
        for ((ppp, height_2), width_2) in PPPS.iter().zip(ALTOS_PT).zip(WIDTHS_PT) {
            assert!(
                near_pt(min_window_height_pt(*ppp), height_2),
                "alto con {ppp}"
            );
            assert!(
                near_pt(min_window_width_pt(*ppp), width_2),
                "ancho con {ppp}"
            );
            assert!(min_window_height_pt(*ppp) <= 432.0 + 0.01, "alto con {ppp}");
            assert!(min_window_width_pt(*ppp) <= 683.0, "ancho con {ppp}");
            let n = n_of(*ppp);
            assert_eq!(block_height_px(Mode::Indexed, n as u32), 308 * n);
            assert_eq!(min_panel_height_px(n as u32), 277 * n);
            // Las 5 herramientas de hoy caben a cualquier ancho con holgura…
            for cols in [COLUMN_MIN_COLS, 14, 16] {
                assert!(
                    head_u(5, cols) * n + 106 * n <= min_panel_height_px(n as u32),
                    "ppp {ppp}, k = {cols}"
                );
            }
            // …y 20 al mínimo (diez columnas, cuatro filas) caben en la ventana mínima, con 12 u de sobra.
            assert_eq!(tool_rows(20, COLUMN_MIN_COLS), 4);
            assert_eq!(
                min_panel_height_px(n as u32) - (head_u(20, COLUMN_MIN_COLS) + 106) * n,
                12 * n,
                "ppp {ppp}"
            );
        }
        for invalid in [f32::NAN, f32::INFINITY, 0.0, -1.0] {
            assert!(near_pt(min_window_height_pt(invalid), 313.0));
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
            let cols = column_cols(window.0, n, COLUMN_DEFAULT_COLS);
            // Cada botón, el separador y el texto del zoom caben en la caja y no se solapan.
            let mut pieces: Vec<RectU> =
                (0..9).map(|i| toolbox_button_rect_u_at(i, 5, cols)).collect();
            pieces.push((4, toolbox_separator_y_u(5, cols), column_width_u(cols) - 8, 1));
            pieces.push((8, zoom_label_y_u(5, cols), column_width_u(cols) - 16, 14));
            for (i, a) in pieces.iter().enumerate() {
                prop_assert!(a.0 >= 0 && (a.0 + a.2) * k <= l.toolbar.width);
                prop_assert!((a.1 + a.3) * k <= l.toolbar.height);
                for b in pieces.iter().skip(i + 1) {
                    prop_assert!(!overlap(*a, *b));
                }
            }
            prop_assert!(
                block_height_closed_px(Mode::Indexed, n) <= l.panel.height + l.toolbar.height
            );
            let height_2 = ((20 + MENU_H_U + 277) * k) as f32 / ppp;
            prop_assert!((min_window_height_pt(ppp) - height_2).abs() <= 0.01);
            prop_assert!(min_window_height_pt(ppp) <= 432.0 + 0.01);
            let width_2 = (min_window_width_u() * k) as f32 / ppp;
            prop_assert!((min_window_width_pt(ppp) - width_2).abs() <= 0.01);
            prop_assert!(min_window_width_pt(ppp) <= 683.0);
        }

        /// Propiedad 3 (la caja): un punto golpea como mucho un botón y exactamente el de su
        /// rectángulo; en los huecos no golpea nada; es total.
        #[test]
        fn property_3_toolbox_hit_gives_the_button_of_its_rectangle_and_nothing_in_the_gaps(
            n in 1u32..=8,
            cols in 8i64..=24,
            x in -60i64..=1000,
            y in -60i64..=2200,
        ) {
            let k = i64::from(n);
            let at = |r: &RectU| x >= r.0 * k && x < (r.0 + r.2) * k && y >= r.1 * k && y < (r.1 + r.3) * k;
            let expected_2 = buttons_of_the_spec(cols, 5).iter().position(at);
            prop_assert_eq!(toolbox_hit_at(x, y, n, 5, cols), expected_2);
            if cols == 16 {
                // The legacy entry point is that of the 6 tools of today.
                prop_assert_eq!(toolbox_hit(x, y, n), buttons_of_the_spec(16, 6).iter().position(at));
            }
        }
    }
}
