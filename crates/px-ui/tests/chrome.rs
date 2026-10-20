//! Tests de la geometría de la ventana `px_ui::chrome` y de los mínimos de ventana
//! (specs/hito-2-menu-herramientas.md, «Casos» filas 7 y 8 y propiedad 3). Sin tipos de egui:
//! `chrome` es aritmética entera.
//!
//! - Fila 7 (parte pura): `toolbar_width_u() = 107 = 2·4 + 99` con 99 = `4 + (longest + 2)·7 + 4` y
//!   `longest` = el mayor `chars().count()` de `TOOL_BUTTONS` (11); los 9 botones caben en la
//!   columna sin tocar el borde y no se solapan; el contenido (335 u) cabe en 351 u; el menú entero
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
    MENU_H_U, TOOL_LABELS, TOOLBAR_CONTENT_H_U, WindowLayout, canvas_origin_px,
    toolbar_button_width_u, toolbar_width_u, window_layout,
};
use px_ui::edit_layout::{
    MIN_CANVAS_WIDTH_U, min_panel_height_px, min_window_height_pt, min_window_width_pt,
    min_window_width_u,
};
use px_ui::menu::{MENUS, shortcut_for, shortcut_text};
use px_ui::palette_grid::PANEL_WIDTH_U;
use px_ui::status::{ADVANCE_PX, bar_height_px};
use px_ui::text::text_scale;
use px_ui::toolbar::TOOL_BUTTONS;
use px_ui::view::ScreenRect;

const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];
const ALTOS_PT: [f32; 5] = [387.0, 309.6, 516.0, 387.0, 387.0];
const ANCHOS_PT: [f32; 5] = [468.0, 374.4, 624.0, 468.0, 468.0];
const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

fn n_de(ppp: f32) -> i64 {
    i64::from(text_scale(ppp))
}

fn cerca_pt(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.01
}

// ---------------------------------------------------------------------------------------------
// Fila 7: constantes y columna
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_7_las_constantes_valen_lo_que_dice_la_spec() {
    assert_eq!(MENU_H_U, 16);
    assert_eq!(TOOLBAR_CONTENT_H_U, 335);
    assert_eq!(toolbar_width_u(), 107);
    assert_eq!(toolbar_button_width_u(), 99);
    assert_eq!(PANEL_WIDTH_U, 233);
    assert_eq!(bar_height_px(1), 20);
    assert_eq!(min_panel_height_px(1), 351);
}

#[test]
fn fila_7_el_ancho_de_la_columna_se_deriva_del_rotulo_mas_largo() {
    let longest = TOOL_BUTTONS
        .iter()
        .map(|b| b.label.chars().count())
        .max()
        .expect("hay botones") as i64;
    assert_eq!(longest, 11, "Cuentagotas");
    let boton = 4 + (longest + 2) * i64::from(ADVANCE_PX) + 4;
    assert_eq!(boton, 99);
    assert_eq!(toolbar_button_width_u(), boton);
    assert_eq!(toolbar_width_u(), 2 * 4 + boton);
    assert_eq!(toolbar_width_u(), 107);
}

#[test]
fn fila_7_tool_labels_son_los_rotulos_de_tool_buttons_sin_la_letra() {
    let rotulos: Vec<&str> = TOOL_BUTTONS.iter().map(|b| b.label).collect();
    assert_eq!(rotulos, TOOL_LABELS);
    assert_eq!(
        TOOL_LABELS,
        ["Lápiz", "Línea", "Relleno", "Cuentagotas", "Borrador"]
    );
}

#[test]
fn fila_7_los_nueve_botones_caben_en_la_columna_sin_tocar_el_borde_y_no_se_solapan() {
    // Herramientas: 5 botones de 16 u en y = 4 + 18·i; zoom: 4 botones en y = 119 + 18·i.
    let alto = 16;
    let herramientas: Vec<(i64, i64)> = (0..5).map(|i| (4 + 18 * i, alto)).collect();
    let zoom: Vec<(i64, i64)> = (0..4).map(|i| (119 + 18 * i, alto)).collect();
    assert_eq!(herramientas.last().map(|(y, h)| y + h), Some(92));
    assert_eq!(zoom.last().map(|(y, h)| y + h), Some(189));
    let separador = (96, 1);
    let nivel = (101, 14);

    let mut todos: Vec<(i64, i64)> = herramientas.clone();
    todos.push(separador);
    todos.push(nivel);
    todos.extend(zoom.iter().copied());
    for pareja in todos.windows(2) {
        let (a, b) = (pareja[0], pareja[1]);
        assert!(a.0 + a.1 <= b.0, "{a:?} se solapa con {b:?}");
    }
    // Margen de 4 u a cada lado: 4 + 99 + 4 = 107, sin tocar el borde.
    let x0 = 4;
    let ancho = toolbar_button_width_u();
    assert!(x0 > 0);
    assert!(x0 + ancho < toolbar_width_u());
    assert_eq!(x0 + ancho + 4, toolbar_width_u());
    // Dos botones seguidos de un mismo grupo dejan 2 u entre ellos.
    for pareja in herramientas.windows(2).chain(zoom.windows(2)) {
        assert_eq!(pareja[1].0 - (pareja[0].0 + pareja[0].1), 2);
    }
}

#[test]
fn fila_7_el_contenido_de_335_u_cabe_en_los_351_u_del_panel_minimo() {
    // Último botón de zoom (termina en 189), 4 u, el recuadro de opciones de 138 u y 4 u de margen.
    assert_eq!(119 + 18 * 3 + 16 + 4 + 138 + 4, TOOLBAR_CONTENT_H_U);
    assert_eq!(TOOLBAR_CONTENT_H_U, 335);
    for ppp in PPPS {
        let n = n_de(ppp);
        assert!(TOOLBAR_CONTENT_H_U * n <= min_panel_height_px(n as u32));
        assert_eq!(
            min_panel_height_px(n as u32) - TOOLBAR_CONTENT_H_U * n,
            16 * n
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: el menú cabe en el ancho mínimo
// ---------------------------------------------------------------------------------------------

/// Ancho de un título como botón: `chars·7 + 12` (6 u de margen a cada lado del texto).
fn ancho_de_titulo_u(titulo: &str) -> i64 {
    titulo.chars().count() as i64 * i64::from(ADVANCE_PX) + 12
}

/// Ancho del desplegable: `12 + max(rótulo + 2 + atajo)·7`.
fn ancho_de_desplegable_u(items: &[px_ui::menu::MenuItem], platform: Platform) -> i64 {
    let mayor = items
        .iter()
        .map(|item| {
            let atajo = shortcut_for(item.action, platform)
                .map_or(0, |s| shortcut_text(s, platform).chars().count());
            item.label.chars().count() + 2 + atajo
        })
        .max()
        .unwrap_or(0) as i64;
    12 + mayor * i64::from(ADVANCE_PX)
}

#[test]
fn fila_7_los_titulos_del_menu_suman_249_u_y_caben_con_su_margen_en_el_ancho_minimo() {
    let anchos: Vec<i64> = MENUS.iter().map(|m| ancho_de_titulo_u(m.title)).collect();
    assert_eq!(anchos, [61, 54, 54, 33, 47]);
    let total: i64 = anchos.iter().sum();
    assert_eq!(total, 249);
    assert!(4 + total < min_window_width_u());
}

#[test]
fn fila_7_el_desplegable_de_ver_mide_208_u_y_es_el_mas_ancho() {
    for platform in PLATAFORMAS {
        let anchos: Vec<(&str, i64)> = MENUS
            .iter()
            .map(|m| (m.title, ancho_de_desplegable_u(m.items, platform)))
            .collect();
        let ver = anchos
            .iter()
            .find(|(t, _)| *t == "Ver")
            .expect("menú Ver")
            .1;
        assert_eq!(ver, 12 + (20 + 2 + 6) * 7, "{platform:?}");
        assert_eq!(ver, 208);
        for (titulo, ancho) in anchos {
            assert!(ancho <= 208, "{titulo} en {platform:?}: {ancho}");
            assert!(ancho < min_window_width_u(), "{titulo} en {platform:?}");
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

fn se_solapan(a: ScreenRect, b: ScreenRect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

fn dentro(r: ScreenRect, ventana: (i64, i64)) -> bool {
    r.x >= 0 && r.y >= 0 && r.x + r.width <= ventana.0 && r.y + r.height <= ventana.1
}

fn cinco(layout: &WindowLayout) -> [(&'static str, ScreenRect); 5] {
    [
        ("menu", layout.menu),
        ("toolbar", layout.toolbar),
        ("panel", layout.panel),
        ("canvas", layout.canvas),
        ("status", layout.status),
    ]
}

/// Invariantes de `window_layout` con la ventana de `ventana` píxeles físicos y la escala `n`.
fn comprueba_layout(ventana: (i64, i64), n: u32) -> Result<(), TestCaseError> {
    let k = i64::from(n);
    let (w, h) = ventana;
    let layout = window_layout(ventana, n);
    let franja = h - (MENU_H_U + 20) * k;
    // Medidas de la spec: menú de 16·n, barra de 20·n, columna de 107·n y panel de 233·n, a la derecha del lienzo.
    prop_assert_eq!(layout.menu, rect(0, 0, w, MENU_H_U * k));
    prop_assert_eq!(layout.status, rect(0, h - 20 * k, w, 20 * k));
    prop_assert_eq!(
        layout.toolbar,
        rect(
            w - toolbar_width_u() * k,
            MENU_H_U * k,
            toolbar_width_u() * k,
            franja
        )
    );
    prop_assert_eq!(
        layout.panel,
        rect(
            w - (toolbar_width_u() + PANEL_WIDTH_U) * k,
            MENU_H_U * k,
            PANEL_WIDTH_U * k,
            franja
        )
    );
    // El lienzo es lo que queda tras quitar menú, columna, panel y barra.
    prop_assert_eq!(
        layout.canvas,
        rect(
            0,
            MENU_H_U * k,
            w - (toolbar_width_u() + PANEL_WIDTH_U) * k,
            franja
        )
    );
    prop_assert_eq!(canvas_origin_px(n), (0, 16 * k));
    prop_assert_eq!(canvas_origin_px(n), (layout.canvas.x, layout.canvas.y));

    // Recubren la ventana sin huecos ni solapes: dentro, disjuntos y con el área total.
    let rects = cinco(&layout);
    let mut suma = 0;
    for (nombre, r) in rects {
        prop_assert!(r.width >= 0 && r.height >= 0, "{}: {:?}", nombre, r);
        prop_assert!(
            dentro(r, ventana),
            "{} fuera de la ventana: {:?}",
            nombre,
            r
        );
        suma += area(r);
    }
    prop_assert_eq!(suma, w * h, "huecos o solapes");
    for (i, (na, a)) in rects.iter().enumerate() {
        for (nb, b) in &rects[i + 1..] {
            prop_assert!(!se_solapan(*a, *b), "{} y {} se solapan", na, nb);
        }
    }
    Ok(())
}

fn comprueba(ventana: (i64, i64), n: u32) {
    if let Err(e) = comprueba_layout(ventana, n) {
        panic!("{e}");
    }
}

/// Ventana por defecto (960 × 720 pt) y mínima, en píxeles físicos, a `ppp`.
fn ventanas(ppp: f32) -> [(i64, i64); 2] {
    let n = n_de(ppp);
    let minima = (
        min_window_width_u() * n,
        i64::from(bar_height_px(n as u32)) + MENU_H_U * n + min_panel_height_px(n as u32),
    );
    let defecto = ((960.0 * ppp).round() as i64, (720.0 * ppp).round() as i64);
    [defecto, minima]
}

#[test]
fn fila_7_window_layout_cumple_la_geometria_a_las_cinco_escalas() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        for ventana in ventanas(ppp) {
            comprueba(ventana, n);
        }
    }
}

#[test]
fn fila_7_el_origen_del_lienzo_es_0_16n() {
    for (ppp, origen) in PPPS
        .iter()
        .zip([(0, 16), (0, 16), (0, 32), (0, 32), (0, 48)])
    {
        assert_eq!(canvas_origin_px(text_scale(*ppp)), origen, "ppp {ppp}");
    }
}

#[test]
fn fila_7_la_columna_y_el_panel_ocupan_el_alto_de_la_franja_central() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for ventana in ventanas(ppp) {
            let layout = window_layout(ventana, n as u32);
            assert_eq!(layout.toolbar.height, ventana.1 - 36 * n);
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
fn fila_8_alto_minimo_a_las_cinco_escalas() {
    for (ppp, esperado) in PPPS.iter().zip(ALTOS_PT) {
        let real = min_window_height_pt(*ppp);
        assert!(
            cerca_pt(real, esperado),
            "ppp {ppp}: {real} y no {esperado}"
        );
    }
}

#[test]
fn fila_8_ancho_minimo_en_u_y_en_pt_a_las_cinco_escalas() {
    assert_eq!(min_window_width_u(), 468);
    assert_eq!(
        min_window_width_u(),
        toolbar_width_u() + PANEL_WIDTH_U + MIN_CANVAS_WIDTH_U
    );
    for (ppp, esperado) in PPPS.iter().zip(ANCHOS_PT) {
        let real = min_window_width_pt(*ppp);
        assert!(
            cerca_pt(real, esperado),
            "ppp {ppp}: {real} y no {esperado}"
        );
    }
}

#[test]
fn fila_8_un_ppp_invalido_da_el_minimo_de_1_0() {
    for ppp in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1.0, -0.5] {
        assert!(cerca_pt(min_window_height_pt(ppp), 387.0), "alto con {ppp}");
        assert!(cerca_pt(min_window_width_pt(ppp), 468.0), "ancho con {ppp}");
    }
}

#[test]
fn fila_8_el_alto_no_pasa_del_72_por_ciento_de_720_y_el_ancho_no_pasa_de_683() {
    for ppp in PPPS {
        assert!(min_window_height_pt(ppp) <= 518.4 + 0.01, "alto con {ppp}");
        assert!(min_window_width_pt(ppp) <= 683.0, "ancho con {ppp}");
    }
}

#[test]
fn fila_8_el_alto_es_barra_mas_menu_mas_panel_minimo_y_el_ancho_columna_panel_y_128_u() {
    for ppp in PPPS {
        let n = n_de(ppp);
        let alto = (i64::from(bar_height_px(n as u32))
            + MENU_H_U * n
            + min_panel_height_px(n as u32)) as f32
            / ppp;
        assert!(cerca_pt(min_window_height_pt(ppp), alto), "alto con {ppp}");
        let ancho = ((toolbar_width_u() + PANEL_WIDTH_U + 128) * n) as f32 / ppp;
        assert!(cerca_pt(min_window_width_pt(ppp), ancho), "ancho con {ppp}");
    }
}

#[test]
fn fila_8_el_lienzo_minimo_mide_128n_por_351n_pixeles() {
    for (ppp, esperado) in
        PPPS.iter()
            .zip([(128, 351), (128, 351), (256, 702), (256, 702), (384, 1053)])
    {
        let n = n_de(*ppp);
        let minima = ventanas(*ppp)[1];
        let layout = window_layout(minima, n as u32);
        assert_eq!(
            (layout.canvas.width, layout.canvas.height),
            esperado,
            "ppp {ppp}"
        );
        assert_eq!(
            (layout.canvas.width, layout.canvas.height),
            (128 * n, 351 * n)
        );
    }
}

#[test]
fn fila_8_la_ventana_minima_en_pixeles_fisicos_es_entera_a_las_cinco_escalas() {
    for ppp in PPPS {
        let ancho = min_window_width_pt(ppp) * ppp;
        let alto = min_window_height_pt(ppp) * ppp;
        assert!((ancho - ancho.round()).abs() <= 0.001, "ancho con {ppp}");
        assert!((alto - alto.round()).abs() <= 0.001, "alto con {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3
// ---------------------------------------------------------------------------------------------

/// `ppp` de 1,0 a 4,0 en pasos de 0,05 sin acumular error de coma flotante.
fn ppp_del_paso(i: u32) -> f32 {
    (100 + 5 * i) as f32 / 100.0
}

#[test]
fn propiedad_3_los_minimos_siguen_la_formula_y_cumplen_las_dos_reglas_en_todos_los_ppp() {
    for i in 0..=60 {
        let ppp = ppp_del_paso(i);
        let n = n_de(ppp);
        let alto = ((20 + MENU_H_U + 351) * n) as f32 / ppp;
        let ancho = ((toolbar_width_u() + PANEL_WIDTH_U + 128) * n) as f32 / ppp;
        assert!(cerca_pt(min_window_height_pt(ppp), alto), "alto con {ppp}");
        assert!(cerca_pt(min_window_width_pt(ppp), ancho), "ancho con {ppp}");
        assert!(min_window_height_pt(ppp) <= 518.4 + 0.01, "alto con {ppp}");
        assert!(min_window_width_pt(ppp) <= 683.0, "ancho con {ppp}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn propiedad_3_el_reparto_es_entero_y_recubre_cualquier_ventana_desde_el_minimo(
        paso in 0u32..=60,
        extra_ancho in 0i64..=3000,
        extra_alto in 0i64..=3000,
    ) {
        let ppp = ppp_del_paso(paso);
        let n = text_scale(ppp);
        let k = i64::from(n);
        let minima = (
            min_window_width_u() * k,
            i64::from(bar_height_px(n)) + MENU_H_U * k + min_panel_height_px(n),
        );
        let ventana = (minima.0 + extra_ancho, minima.1 + extra_alto);
        comprueba_layout(ventana, n)?;
        // El lienzo nunca baja de su mínimo.
        let layout = window_layout(ventana, n);
        prop_assert!(layout.canvas.width >= MIN_CANVAS_WIDTH_U * k);
        prop_assert!(layout.canvas.height >= min_panel_height_px(n));
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad I.3 (specs/interfaz-caja-derecha.md): orden de las zonas, mínimos y geometría de la caja
// ---------------------------------------------------------------------------------------------

mod caja_derecha {
    use super::*;
    use px_ui::chrome::{
        OPTIONS_BODY_H_U, OPTIONS_BODY_ROWS, OPTIONS_BODY_W_U, OPTIONS_BODY_X_U, OPTIONS_BODY_Y_U,
        OPTIONS_BORDER_U, OPTIONS_FOCUS_BORDER_U, OPTIONS_H_U, OPTIONS_KEY_Y_U, OPTIONS_ROW_H_U,
        OPTIONS_TITLE_X_U, OPTIONS_TITLE_Y_U, OPTIONS_W_U, OPTIONS_X_U, OPTIONS_Y_U, ZOOM_Y_U,
    };

    const ALTO_MAX_PT: f32 = 518.4;
    const ANCHO_MAX_PT: f32 = 683.0;

    /// Fila 1: **la única prueba que conoce el orden de las zonas** (lienzo, panel, caja).
    #[test]
    fn fila_1_el_orden_es_lienzo_panel_y_caja_en_el_borde_derecho() {
        for ppp in PPPS {
            let n = n_de(ppp);
            for ventana in ventanas(ppp) {
                let l = window_layout(ventana, n as u32);
                assert_eq!(l.canvas.x, 0, "ppp {ppp}");
                assert_eq!(l.canvas.x + l.canvas.width, l.panel.x, "ppp {ppp}");
                assert_eq!(l.panel.x + l.panel.width, l.toolbar.x, "ppp {ppp}");
                assert_eq!(l.toolbar.x + l.toolbar.width, ventana.0, "ppp {ppp}");
                assert_eq!(l.panel.width, 233 * n);
                assert_eq!(l.toolbar.width, 107 * n);
                assert_eq!(canvas_origin_px(n as u32), (0, 16 * n));
            }
        }
    }

    /// Fila 2: los mínimos no cambian y cumplen los dos techos; el contenido de la caja cabe.
    #[test]
    fn fila_2_los_minimos_no_cambian_y_cumplen_los_techos() {
        for ((ppp, alto), ancho) in PPPS.iter().zip(ALTOS_PT).zip(ANCHOS_PT) {
            assert!(cerca_pt(min_window_height_pt(*ppp), alto), "ppp {ppp}");
            assert!(cerca_pt(min_window_width_pt(*ppp), ancho), "ppp {ppp}");
            assert!(min_window_height_pt(*ppp) <= ALTO_MAX_PT, "ppp {ppp}");
            assert!(min_window_width_pt(*ppp) <= ANCHO_MAX_PT, "ppp {ppp}");
            let n = n_de(*ppp);
            assert!(
                TOOLBAR_CONTENT_H_U * n <= min_panel_height_px(n as u32),
                "ppp {ppp}: la caja no cabe en el panel mínimo"
            );
        }
        for invalido in [f32::NAN, 0.0, -1.0, f32::INFINITY] {
            assert!(cerca_pt(min_window_height_pt(invalido), 387.0));
            assert!(cerca_pt(min_window_width_pt(invalido), 468.0));
        }
        assert_eq!(min_window_width_u(), 468);
    }

    /// Fila 3: las constantes y las relaciones del recuadro de opciones.
    #[test]
    fn fila_3_el_recuadro_de_opciones_tiene_la_geometria_de_la_spec() {
        assert_eq!(
            (OPTIONS_X_U, OPTIONS_Y_U, OPTIONS_W_U, OPTIONS_H_U),
            (4, 193, 99, 138)
        );
        assert_eq!(OPTIONS_W_U, toolbar_button_width_u());
        assert_eq!((OPTIONS_BORDER_U, OPTIONS_FOCUS_BORDER_U), (1, 2));
        assert_eq!((OPTIONS_TITLE_X_U, OPTIONS_TITLE_Y_U), (8, 197));
        assert_eq!(OPTIONS_KEY_Y_U, 211);
        assert_eq!(
            (OPTIONS_BODY_X_U, OPTIONS_BODY_Y_U, OPTIONS_BODY_W_U),
            (8, 229, 91)
        );
        assert_eq!(OPTIONS_BODY_ROWS, 7);
        assert_eq!(OPTIONS_ROW_H_U, 14);
        assert_eq!(OPTIONS_BODY_H_U, 7 * 14);
        // 4 u bajo el último botón de zoom (termina en 189) y 4 u de margen inferior.
        assert_eq!(OPTIONS_Y_U, ZOOM_Y_U + 3 * 18 + 16 + 4);
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
        fn propiedad_1_el_reparto_con_la_caja_a_la_derecha_es_entero_adyacente_y_cabe(
            paso in 0u32..=60,
            extra_w in 0i64..=700,
            extra_h in 0i64..=500,
        ) {
            let ppp = 1.0 + paso as f32 * 0.05;
            let n = text_scale(ppp);
            let k = i64::from(n);
            let minima = (
                min_window_width_u() * k,
                i64::from(bar_height_px(n)) + MENU_H_U * k + min_panel_height_px(n),
            );
            let ventana = (minima.0 + extra_w, minima.1 + extra_h);
            comprueba_layout(ventana, n)?;
            let l = window_layout(ventana, n);
            prop_assert_eq!(l.toolbar.x + l.toolbar.width, ventana.0);
            prop_assert_eq!(l.panel.x + l.panel.width, l.toolbar.x);
            prop_assert_eq!(l.canvas.x, 0);
            prop_assert_eq!(l.canvas.x + l.canvas.width, l.panel.x);
            prop_assert_eq!(canvas_origin_px(n), (0, 16 * k));
            prop_assert!(l.toolbar.height >= TOOLBAR_CONTENT_H_U * k);
            // Alto y ancho mínimos: la fórmula y los dos techos.
            let alto = (i64::from(bar_height_px(n)) + MENU_H_U * k + min_panel_height_px(n)) as f32 / ppp;
            prop_assert!((min_window_height_pt(ppp) - alto).abs() <= 0.01);
            prop_assert!(min_window_height_pt(ppp) <= ALTO_MAX_PT);
            let ancho = (min_window_width_u() * k) as f32 / ppp;
            prop_assert!((min_window_width_pt(ppp) - ancho).abs() <= 0.01);
            prop_assert!(min_window_width_pt(ppp) <= ANCHO_MAX_PT);
        }
    }
}
