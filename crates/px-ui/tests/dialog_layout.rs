//! Tests de la geometría pura `px_ui::dialog_layout` (specs/hito-2-dialogos-modales.md, «Casos»
//! filas 7 y 8 y propiedad 3, y la sección «Geometría»). Sin egui: todo es aritmética entera en
//! píxeles físicos y en `u` (`n` px cada una, `n = text_scale(ppp)`).
//!
//! - Fila 7: `dialog_layout` en la ventana mínima real de cada escala (`468n × 333n` px: 468 × 333,
//!   468 × 333, 936 × 666, 936 × 666 y 1404 × 999 a `ppp` 1,0, 1,25, 1,5, 2,0 y 3,0) y en 960 × 720
//!   pt (px = `round(pt · ppp)`), con contenido de 228 u, de 5000 u y con `Fill` (con y sin
//!   `Cancel`); ventanas de 300 × 200, 1 × 1 y 0 × 0 px; todas las cifras de la tabla (330n, 20n,
//!   45n, botones de 68n × 16n con 4n de hueco, motivo de 308n, `x = 69n`, `y = 8n` o `20n`, cuerpo
//!   de 252n, 16 filas con `Fill` vía `zone_rows`, `max_scroll_u = content − 252`).
//! - Fila 8: un `Dialog` con valores escritos, un hexadecimal pendiente y `scroll_u = 100` pasa por
//!   las ventanas y escalas de la fila 7 sin perder nada salvo el desplazamiento recortado; y
//!   `ensure_visible` con un elemento por encima, por debajo, a medias, más alto que la vista y con
//!   el contenido encogido.
//! - Propiedad 3: geometría entera, dentro de la ventana y recubierta (`ppp` de 1,0 a 4,0 en pasos
//!   de 0,05 como índices enteros, para no acumular error de `f32`).
//!
//! Suposiciones sobre las firmas (la spec no las fija al detalle; anotadas también en el informe):
//! - `dialog_layout(window_px: (i64, i64), n: u32, content_h_u: i64, fill: bool, has_cancel: bool)
//!   -> DialogLayout`, as_word `chrome::window_layout(window_px, n)`; las constantes `DIALOG_W_U`,
//!   `MARGIN_U`, `TITLE_H_U`, `FOOTER_H_U` y `BUTTON_W_U` son `i64`.
//! - `DialogLayout { dialog, title, body, footer, reason, accept: ScreenRect, cancel:
//!   Option<ScreenRect>, max_scroll_u: i64 }`, with `Clone, Debug, PartialEq`.
//! - Los tamaños en `u` salen de dividir los píxeles de la ventana entre `n` **con división
//!   entera** (`⌊px / n⌋`): ancho `min(330, ⌊width_2/n⌋ − 16)`, alto `min(65 + content, ⌊height_2/n⌋ −
//!   16)`; la posición es `(ventana − diálogo).div_euclid(2)` at píxeles.
//! - Título, cuerpo y pie ocupan el ancho entero del diálogo. El motivo está en `x = 8n` del
//!   diálogo (el relleno) y `y = 5n` del pie (1 de línea + 4); los botones, en `y = 21n` del pie
//!   (1 + 4 + 14 + 2) y pegados a la derecha con un relleno de `8n`: `Cancel` el último y
//!   `Accept` a su izquierda con `4n` de hueco; sin `Cancel`, `Accept` ocupa su sitio.
//! - Con `Fill`, `content_h_u` es el natural (`330 = 16 + 6 + 22·14`, lo que da
//!   `Dialog::content_h_u()`) y `max_scroll_u` es 0 («sin desplazamiento del cuerpo»). Por eso las
//!   pruebas pasan siempre 330 con `fill = true` y no otro contenido.
//! - `zone_rows(box_h_u)` recibe el alto de la **caja** de la zona (el cuerpo menos `2·8` de
//!   relleno): con el cuerpo de 252 u la caja mide 236 u y `⌊(236 − 6)/14⌋ = 16` filas; con un alto
//!   menor que una fila (o negativo) da 0, sin pánico.
//! - `ensure_visible(scroll_u, top_u, bottom_u, view_h_u)`: el elemento ocupa `[top, bottom)` en
//!   coordenadas del contenido (`bottom` exclusivo) y la vista `[scroll, scroll + view_h)`. Da el
//!   desplazamiento mínimo que lo deja a la vista; si es más alto que la vista, `top_u`. No conoce
//!   el contenido: el llamador recorta antes el desplazamiento a `max_scroll_u` (con el contenido
//!   encogido) y, con un elemento dentro del contenido, el resultado ya cae en `0..=max`.
//! - `Dialog` es `Clone + PartialEq` (el resto de derives que anota la spec); el desplazamiento
//!   forma parte de su igualdad, por eso se compara con `set_scroll_u(0)` en ambos lados.
//! - Con `Dialog::set_scroll_u` se guarda el valor tal cual (en `u`); quien recorta a
//!   `max_scroll_u` es el llamador (`show_dialog`), que aquí se simula con `clamp(0, max)`.

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_ui::dialog::{Dialog, DialogKind, Event};
use px_ui::dialog_layout::{
    BUTTON_W_U, DIALOG_W_U, DialogLayout, FOOTER_H_U, MARGIN_U, TITLE_H_U, dialog_layout,
    ensure_visible, zone_rows,
};
use px_ui::licenses_view::licenses_form;
use px_ui::text::text_scale;
use px_ui::view::ScreenRect;

#[path = "common/dialog_forms.rs"]
mod dialog_forms;

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];
/// La ventana mínima real de cada escala, en píxeles (`468n × 333n`).
const MINIMUM: [(f32, (i64, i64)); 5] = [
    (1.0, (468, 333)),
    (1.25, (468, 333)),
    (1.5, (936, 666)),
    (2.0, (936, 666)),
    (3.0, (1404, 999)),
];
/// Contenido natural de un diálogo con `Fill`: `16 + 6 + 22·14`.
const FILL_U: i64 = 330;
/// Título más pie: lo que no es cuerpo.
const FRAME_U: i64 = TITLE_H_U + FOOTER_H_U;
const TEST_BUTTON_H_U: i64 = 16;
const TEST_GAP_U: i64 = 4;
const FILL_U_2: i64 = 8;

/// Un contenido de las pruebas: su alto en `u` y si es una zona de relleno.
#[derive(Clone, Copy, Debug)]
struct Content {
    name_2: &'static str,
    height_u: i64,
    fill: bool,
}

const CONTENTS: [Content; 3] = [
    Content {
        name_2: "228 u",
        height_u: 228,
        fill: false,
    },
    Content {
        name_2: "5000 u",
        height_u: 5000,
        fill: false,
    },
    Content {
        name_2: "fill",
        height_u: FILL_U,
        fill: true,
    },
];

fn k_of(n: u32) -> i64 {
    i64::from(n)
}

/// `ppp` de 1,0 a 4,0 en pasos de 0,05 sin acumular error de coma flotante.
fn ppp_of_the_step(i: u32) -> f32 {
    (100 + 5 * i) as f32 / 100.0
}

/// La ventana mínima de `ppp` calculada con la fórmula (`468n × 333n`).
fn minimum(ppp: f32) -> (i64, i64) {
    let k = k_of(text_scale(ppp));
    (468 * k, 333 * k)
}

/// 960 × 720 pt en píxeles: `round(pt · ppp)`.
fn window_960x720(ppp: f32) -> (i64, i64) {
    ((960.0 * ppp).round() as i64, (720.0 * ppp).round() as i64)
}

fn designs(window: (i64, i64), n: u32, content: Content, has_cancel: bool) -> DialogLayout {
    dialog_layout(window, n, content.height_u, content.fill, has_cancel)
}

fn rect(x: i64, y: i64, width: i64, height: i64) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width,
        height,
    }
}

fn inside(a: ScreenRect, b: ScreenRect) -> bool {
    a.x >= b.x && a.y >= b.y && a.x + a.width <= b.x + b.width && a.y + a.height <= b.y + b.height
}

fn overlap(a: ScreenRect, b: ScreenRect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

/// Todo lo que vale para cualquier ventana igual o mayor que la mínima (sección «Geometría»):
/// medidas de la tabla, recubrimiento de título, cuerpo y pie, botones, motivo, margen, centrado
/// y `max_scroll_u`. Devuelve el diseño para que quien llama compruebe lo suyo.
fn checks_geometry(
    window: (i64, i64),
    n: u32,
    content: Content,
    has_cancel: bool,
) -> Result<DialogLayout, TestCaseError> {
    let k = k_of(n);
    let (w, h) = window;
    let l = designs(window, n, content, has_cancel);

    let width_u = DIALOG_W_U.min(w.div_euclid(k) - 2 * MARGIN_U);
    let height_u = (FRAME_U + content.height_u).min(h.div_euclid(k) - 2 * MARGIN_U);
    let (dw, dh) = (width_u * k, height_u * k);
    let d = l.dialog;
    prop_assert_eq!(
        d,
        rect((w - dw).div_euclid(2), (h - dh).div_euclid(2), dw, dh),
        "el diálogo en {:?} con n {} y {:?}",
        window,
        n,
        content
    );

    // Dentro de la ventana, con margen de 8n como mínimo, y centrado a ±1 px.
    prop_assert!(d.x >= MARGIN_U * k, "margen izquierdo {:?}", d);
    prop_assert!(d.y >= MARGIN_U * k, "margen superior {:?}", d);
    prop_assert!(
        w - (d.x + d.width) >= MARGIN_U * k,
        "margen derecho {:?}",
        d
    );
    prop_assert!(
        h - (d.y + d.height) >= MARGIN_U * k,
        "margen inferior {:?}",
        d
    );
    prop_assert!(
        (2 * d.x - (w - d.width)).abs() <= 1,
        "centrado en x {:?}",
        d
    );
    prop_assert!(
        (2 * d.y - (h - d.height)).abs() <= 1,
        "centrado en y {:?}",
        d
    );

    // Título, cuerpo y pie recubren el diálogo sin huecos ni solapes.
    prop_assert_eq!(l.title, rect(d.x, d.y, d.width, TITLE_H_U * k));
    prop_assert_eq!(
        l.body,
        rect(d.x, d.y + TITLE_H_U * k, d.width, d.height - FRAME_U * k)
    );
    prop_assert_eq!(
        l.footer,
        rect(
            d.x,
            d.y + d.height - FOOTER_H_U * k,
            d.width,
            FOOTER_H_U * k
        )
    );
    prop_assert!(l.body.height >= 0);
    prop_assert_eq!(
        l.title.y + l.title.height,
        l.body.y,
        "sin hueco bajo el título"
    );
    prop_assert_eq!(
        l.body.y + l.body.height,
        l.footer.y,
        "sin hueco sobre el pie"
    );
    prop_assert_eq!(l.footer.y + l.footer.height, d.y + d.height);

    // El motivo (308n × 14n, siempre reservado) y los botones caben en el pie, sin tocarse.
    prop_assert_eq!(
        l.reason,
        rect(d.x + FILL_U_2 * k, l.footer.y + 5 * k, 308 * k, 14 * k)
    );
    let button_y = l.footer.y + (1 + 4 + 14 + 2) * k;
    let right = d.x + d.width - FILL_U_2 * k;
    let width_button = BUTTON_W_U * k;
    prop_assert_eq!(l.accept.width, width_button);
    prop_assert_eq!(l.accept.height, TEST_BUTTON_H_U * k);
    prop_assert_eq!(l.accept.y, button_y);
    match (has_cancel, l.cancel) {
        (true, Some(cancel)) => {
            prop_assert_eq!(
                cancel,
                rect(
                    right - width_button,
                    button_y,
                    width_button,
                    TEST_BUTTON_H_U * k
                )
            );
            prop_assert_eq!(
                l.accept.x,
                cancel.x - TEST_GAP_U * k - width_button,
                "4n de hueco entre los botones"
            );
            prop_assert!(inside(cancel, l.footer));
            prop_assert!(!overlap(cancel, l.accept));
            prop_assert!(!overlap(cancel, l.reason));
        }
        (false, None) => prop_assert_eq!(l.accept.x, right - width_button),
        (expected_2, real) => {
            prop_assert!(false, "has_cancel {} pero cancel es {:?}", expected_2, real);
        }
    }
    prop_assert!(inside(l.accept, l.footer));
    prop_assert!(inside(l.reason, l.footer));
    prop_assert!(!overlap(l.accept, l.reason));

    // Las dimensiones son múltiplos de n.
    for value in [
        d.width,
        d.height,
        l.title.height,
        l.body.height,
        l.footer.height,
        l.reason.width,
        l.reason.height,
        l.accept.width,
        l.accept.height,
    ] {
        prop_assert_eq!(value % k, 0, "{} no es múltiplo de {}", value, k);
    }

    // `max_scroll_u`: el contenido menos el cuerpo, 0 si cabe; con `Fill`, siempre 0.
    let body_u = l.body.height / k;
    if content.fill {
        prop_assert_eq!(l.max_scroll_u, 0, "Fill no desplaza el cuerpo");
    } else {
        prop_assert_eq!(l.max_scroll_u, (content.height_u - body_u).max(0));
        prop_assert_eq!(l.max_scroll_u == 0, content.height_u <= body_u);
    }
    Ok(l)
}

fn ok<T>(result: Result<T, TestCaseError>, label_2: &str) -> T {
    result.unwrap_or_else(|error| panic!("{label_2}: {error}"))
}

// ---------------------------------------------------------------------------------------------
// Fila 7: constantes y ventana mínima
// ---------------------------------------------------------------------------------------------

#[test]
fn row_7_the_constants_are_worth_what_the_spec_says() {
    assert_eq!(DIALOG_W_U, 330);
    assert_eq!(MARGIN_U, 8);
    assert_eq!(TITLE_H_U, 20);
    assert_eq!(FOOTER_H_U, 45);
    assert_eq!(BUTTON_W_U, 68);
    // 330 = 8 + 308 + 2 + 4 + 8 y 68 = 2·6 + 8·7.
    assert_eq!(DIALOG_W_U, 8 + 308 + 2 + 4 + 8);
    assert_eq!(BUTTON_W_U, 2 * 6 + 8 * 7);
    // 45 = 1 + 4 + 14 + 2 + 16 + 8.
    assert_eq!(FOOTER_H_U, 1 + 4 + 14 + 2 + 16 + 8);
}

#[test]
fn row_7_the_table_of_minimum_windows_is_468n_by_333n() {
    for (ppp, (width_2, height_2)) in MINIMUM {
        assert_eq!(minimum(ppp), (width_2, height_2), "ppp {ppp}");
    }
    let scales: Vec<u32> = PPPS.iter().map(|ppp| text_scale(*ppp)).collect();
    assert_eq!(scales, [1, 1, 2, 2, 3]);
}

#[test]
fn row_7_the_minimum_window_meets_the_geometry_with_all_the_contents_and_with_and_without_cancel() {
    for (ppp, window) in MINIMUM {
        let n = text_scale(ppp);
        for content in CONTENTS {
            for has_cancel in [true, false] {
                ok(
                    checks_geometry(window, n, content, has_cancel),
                    &format!("ppp {ppp}, {}, cancelar {has_cancel}", content.name_2),
                );
            }
        }
    }
}

#[test]
fn row_7_with_228_u_it_fits_without_scrolling_and_is_centered_vertically() {
    for (ppp, window) in MINIMUM {
        let n = text_scale(ppp);
        let k = k_of(n);
        for has_cancel in [true, false] {
            let l = designs(window, n, CONTENTS[0], has_cancel);
            let label_2 = format!("ppp {ppp}, cancelar {has_cancel}");
            assert_eq!(l.dialog.width, 330 * k, "{label_2}");
            assert_eq!(l.dialog.height, (65 + 228) * k, "{label_2}");
            assert_eq!(l.dialog.x, 69 * k, "{label_2}");
            assert_eq!(l.dialog.y, 20 * k, "{label_2}");
            assert_eq!(l.body.height, 228 * k, "{label_2}");
            assert_eq!(l.max_scroll_u, 0, "{label_2}");
        }
    }
}

#[test]
fn row_7_with_5000_u_the_height_is_the_maximum_and_the_body_measures_252n() {
    for (ppp, window) in MINIMUM {
        let n = text_scale(ppp);
        let k = k_of(n);
        for has_cancel in [true, false] {
            let l = designs(window, n, CONTENTS[1], has_cancel);
            let label_2 = format!("ppp {ppp}, cancelar {has_cancel}");
            assert_eq!(l.dialog.width, 330 * k, "{label_2}");
            assert_eq!(l.dialog.height, 317 * k, "{label_2}");
            assert_eq!(l.dialog.x, 69 * k, "{label_2}");
            assert_eq!(l.dialog.y, 8 * k, "{label_2}");
            assert_eq!(l.title.height, 20 * k, "{label_2}");
            assert_eq!(l.footer.height, 45 * k, "{label_2}");
            assert_eq!(l.body.height, 252 * k, "{label_2}");
            assert_eq!(l.max_scroll_u, 5000 - 252, "{label_2}");
        }
    }
}

#[test]
fn row_7_with_fill_at_the_minimum_window_there_are_16_rows_and_it_does_not_scroll() {
    for (ppp, window) in MINIMUM {
        let n = text_scale(ppp);
        let k = k_of(n);
        for has_cancel in [true, false] {
            let l = designs(window, n, CONTENTS[2], has_cancel);
            let label_2 = format!("ppp {ppp}, cancelar {has_cancel}");
            assert_eq!(l.dialog.height, 317 * k, "{label_2}");
            assert_eq!(l.dialog.x, 69 * k, "{label_2}");
            assert_eq!(l.dialog.y, 8 * k, "{label_2}");
            assert_eq!(l.body.height, 252 * k, "{label_2}");
            assert_eq!(l.max_scroll_u, 0, "{label_2}");
            let toolbox_u = l.body.height / k - 2 * FILL_U_2;
            assert_eq!(toolbox_u, 236, "{label_2}");
            assert_eq!(zone_rows(toolbox_u), 16, "{label_2}");
        }
    }
}

#[test]
fn row_7_the_dialog_measures_the_same_in_u_at_the_five_scales_of_the_minimum_window() {
    for content in CONTENTS {
        let reference = designs(minimum(1.0), 1, content, true);
        for (ppp, window) in MINIMUM {
            let n = text_scale(ppp);
            let k = k_of(n);
            let l = designs(window, n, content, true);
            assert_eq!(l.dialog.width / k, reference.dialog.width, "{ppp}");
            assert_eq!(l.dialog.height / k, reference.dialog.height, "{ppp}");
            assert_eq!(l.dialog.x / k, reference.dialog.x, "{ppp}");
            assert_eq!(l.dialog.y / k, reference.dialog.y, "{ppp}");
            assert_eq!(l.max_scroll_u, reference.max_scroll_u, "{ppp}");
        }
    }
}

#[test]
fn row_7_the_height_is_min_65_plus_content_and_317_at_the_minimum() {
    for (ppp, window) in MINIMUM {
        let n = text_scale(ppp);
        let k = k_of(n);
        for content_u in [0, 1, 100, 227, 228, 251, 252, 253, 300, 1000, 5000, 10_000] {
            let l = dialog_layout(window, n, content_u, false, true);
            assert_eq!(
                l.dialog.height,
                (65 + content_u).min(317) * k,
                "ppp {ppp}, contenido {content_u}"
            );
            assert_eq!(
                l.max_scroll_u,
                (content_u - (l.dialog.height / k - 65)).max(0),
                "ppp {ppp}, contenido {content_u}"
            );
        }
    }
}

#[test]
fn row_7_cancel_only_moves_accept_and_the_rest_stays_the_same() {
    for (ppp, window) in MINIMUM {
        let n = text_scale(ppp);
        for content in CONTENTS {
            let with = designs(window, n, content, true);
            let without = designs(window, n, content, false);
            assert_eq!(with.dialog, without.dialog, "ppp {ppp}");
            assert_eq!(with.title, without.title, "ppp {ppp}");
            assert_eq!(with.body, without.body, "ppp {ppp}");
            assert_eq!(with.footer, without.footer, "ppp {ppp}");
            assert_eq!(with.reason, without.reason, "ppp {ppp}");
            assert_eq!(with.max_scroll_u, without.max_scroll_u, "ppp {ppp}");
            assert!(with.cancel.is_some(), "ppp {ppp}");
            assert!(without.cancel.is_none(), "ppp {ppp}");
        }
    }
}

#[test]
fn row_7_the_layout_is_a_pure_function() {
    for (ppp, window) in MINIMUM {
        let n = text_scale(ppp);
        for content in CONTENTS {
            assert_eq!(
                designs(window, n, content, true),
                designs(window, n, content, true),
                "ppp {ppp}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: 960 × 720 pt
// ---------------------------------------------------------------------------------------------

#[test]
fn row_7_at_960_by_720_pt_the_geometry_is_met_at_the_five_scales() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        let window = window_960x720(ppp);
        for content in CONTENTS {
            for has_cancel in [true, false] {
                ok(
                    checks_geometry(window, n, content, has_cancel),
                    &format!(
                        "ppp {ppp}, {}, cancelar {has_cancel}, ventana {window:?}",
                        content.name_2
                    ),
                );
            }
        }
    }
}

#[test]
fn row_7_960_by_720_pt_in_pixels_is_the_rounded_product() {
    let expected_2 = [
        (1.0, (960, 720)),
        (1.25, (1200, 900)),
        (1.5, (1440, 1080)),
        (2.0, (1920, 1440)),
        (3.0, (2880, 2160)),
    ];
    for (ppp, pixels_2) in expected_2 {
        assert_eq!(window_960x720(ppp), pixels_2, "ppp {ppp}");
    }
}

#[test]
fn row_7_at_960_by_720_pt_the_height_follows_65_plus_the_content_capped_at_the_window() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        let k = k_of(n);
        let window = window_960x720(ppp);
        let height_max_u = window.1.div_euclid(k) - 16;
        for has_cancel in [true, false] {
            let short = designs(window, n, CONTENTS[0], has_cancel);
            assert_eq!(short.dialog.width, 330 * k, "ppp {ppp}");
            assert_eq!(short.dialog.height, 293 * k, "ppp {ppp}");
            assert_eq!(short.max_scroll_u, 0, "ppp {ppp}");

            let length_2 = designs(window, n, CONTENTS[1], has_cancel);
            assert_eq!(length_2.dialog.height, height_max_u * k, "ppp {ppp}");
            assert_eq!(
                length_2.max_scroll_u,
                5000 - (height_max_u - 65),
                "ppp {ppp}"
            );

            // Con la ventana grande el `Fill` ocupa su contenido natural: 22 filas.
            let fill = designs(window, n, CONTENTS[2], has_cancel);
            assert_eq!(fill.dialog.height, 395 * k, "ppp {ppp}");
            assert_eq!(fill.max_scroll_u, 0, "ppp {ppp}");
            assert_eq!(
                zone_rows(fill.body.height / k - 2 * FILL_U_2),
                22,
                "ppp {ppp}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: ventanas menores que la mínima
// ---------------------------------------------------------------------------------------------

fn checks_inside_the_window(l: &DialogLayout, window: (i64, i64)) -> Result<(), TestCaseError> {
    let d = l.dialog;
    prop_assert!(d.width >= 0 && d.height >= 0, "medidas negativas {:?}", d);
    prop_assert!(
        d.x >= 0 && d.y >= 0,
        "fuera por arriba o la izquierda {:?}",
        d
    );
    prop_assert!(d.x + d.width <= window.0, "fuera por la derecha {:?}", d);
    prop_assert!(d.y + d.height <= window.1, "fuera por abajo {:?}", d);
    Ok(())
}

#[test]
fn row_7_with_300_by_200_px_the_dialog_shrinks_and_stays_inside() {
    // A n = 1 la fórmula sigue valiendo: ancho min(330, 300 − 16) y alto min(65 + c, 200 − 16).
    for content in CONTENTS {
        for has_cancel in [true, false] {
            let l = designs((300, 200), 1, content, has_cancel);
            assert_eq!(l.dialog.width, 284, "{}", content.name_2);
            assert_eq!(
                l.dialog.height,
                (65 + content.height_u).min(184),
                "{}",
                content.name_2
            );
            assert_eq!(l.dialog.x, 8, "{}", content.name_2);
            assert_eq!(
                l.dialog.y,
                (200 - l.dialog.height).div_euclid(2),
                "{}",
                content.name_2
            );
            ok(checks_inside_the_window(&l, (300, 200)), content.name_2);
            // Título, cuerpo y pie siguen apilados y recubriendo el diálogo.
            assert_eq!(l.title.height, 20);
            assert_eq!(l.footer.height, 45);
            assert_eq!(l.body.height, l.dialog.height - 65);
        }
    }
}

#[test]
fn row_7_with_windows_smaller_than_the_minimum_the_dialog_stays_inside_and_there_is_no_panic() {
    let windows = [
        (300, 200),
        (1, 1),
        (0, 0),
        (468, 100),
        (100, 333),
        (467, 332),
    ];
    for ppp in PPPS {
        let n = text_scale(ppp);
        for window in windows {
            for content in CONTENTS {
                for has_cancel in [true, false] {
                    let l = designs(window, n, content, has_cancel);
                    ok(
                        checks_inside_the_window(&l, window),
                        &format!(
                            "ppp {ppp}, ventana {window:?}, {}, cancelar {has_cancel}",
                            content.name_2
                        ),
                    );
                }
            }
        }
    }
}

#[test]
fn row_7_a_window_of_0_by_0_leaves_an_empty_dialog_at_the_origin() {
    for n in [1u32, 2, 3, 8] {
        let l = designs((0, 0), n, CONTENTS[1], true);
        assert_eq!((l.dialog.width, l.dialog.height), (0, 0), "n {n}");
        assert_eq!((l.dialog.x, l.dialog.y), (0, 0), "n {n}");
    }
}

// ---------------------------------------------------------------------------------------------
// `zone_rows`
// ---------------------------------------------------------------------------------------------

#[test]
fn zone_rows_is_the_floor_of_height_minus_6_divided_by_14() {
    let cases = [
        (6, 0),
        (19, 0),
        (20, 1),
        (33, 1),
        (34, 2),
        (6 + 14 * 4, 4),
        (6 + 14 * 4 - 1, 3),
        (236, 16),
        (237, 16),
        (6 + 14 * 17 - 1, 16),
        (6 + 14 * 17, 17),
        (314, 22),
        (6 + 14 * 1000, 1000),
    ];
    for (height_2, rows_2) in cases {
        assert_eq!(zone_rows(height_2), rows_2, "caja de {height_2} u");
    }
}

#[test]
fn zone_rows_with_boxes_smaller_than_a_row_or_negative_gives_zero_without_panic() {
    for height_2 in [-1_000_000, -100, -1, 0, 1, 5, 6, 7, 19] {
        assert_eq!(zone_rows(height_2), 0, "caja de {height_2} u");
    }
}

#[test]
fn zone_rows_does_not_go_down_when_the_box_grows() {
    let mut previous = 0;
    for height_2 in 0..2000 {
        let rows_2 = zone_rows(height_2);
        assert!(rows_2 >= previous, "caja de {height_2} u");
        assert!(rows_2 - previous <= 1, "caja de {height_2} u");
        previous = rows_2;
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: recolocar sin perder valores
// ---------------------------------------------------------------------------------------------

/// El orden de la fila 8: 468×333 (`n` 1), 936×666 (2), 1404×999 (3), 960×720 y otra vez 468×333.
const STEPS_ROW_8: [((i64, i64), u32); 5] = [
    ((468, 333), 1),
    ((936, 666), 2),
    ((1404, 999), 3),
    ((960, 720), 1),
    ((468, 333), 1),
];

/// Un `Dialog` igual al de `d` salvo el desplazamiento, que se pone a 0.
fn without_pan(d: &Dialog) -> Dialog {
    let mut copy = d.clone();
    copy.set_scroll_u(0);
    copy
}

/// Un `Dialog` con valores escritos (ancho 100, alto 200, modo 2, opaco) y un hexadecimal
/// pendiente de confirmar, con `scroll_u = 100`.
fn dialog_with_values() -> Dialog {
    let mut dialog = dialog_forms::open(dialog_forms::all_kinds());
    for event in [
        Event::Number {
            field: "ancho",
            text: "100".to_string(),
        },
        Event::Number {
            field: "alto",
            text: "200".to_string(),
        },
        Event::Choice {
            field: "modo",
            index: 2,
        },
        Event::Check {
            field: "opaco",
            on: true,
        },
        Event::Hex {
            field: "color",
            text: "#12ab34".to_string(),
        },
    ] {
        dialog.event(event);
    }
    assert!(
        dialog.hex_pending("color"),
        "el hexadecimal queda pendiente"
    );
    dialog.set_scroll_u(100);
    assert_eq!(dialog.scroll_u(), 100);
    dialog
}

/// Un `Dialog` de contenido enorme con varias casillas marcadas y `scroll_u = 100`.
fn dialog_huge() -> Dialog {
    let mut dialog = dialog_forms::open(dialog_forms::huge(250));
    assert!(
        dialog.content_h_u() > 1000,
        "el contenido enorme mide {} u",
        dialog.content_h_u()
    );
    let ids: Vec<_> = dialog.spec().fields.iter().map(|f| f.id).collect();
    for id in ids.into_iter().step_by(3) {
        dialog.event(Event::Check {
            field: id,
            on: true,
        });
    }
    dialog.set_scroll_u(100);
    assert_eq!(dialog.scroll_u(), 100);
    dialog
}

/// Pasa `dialog` por `steps_2` como lo hace `show_dialog`: pide el diseño con el contenido y recorta
/// el desplazamiento a `0..=max_scroll_u`. Comprueba que, salvo el desplazamiento, el `Dialog` no
/// cambia, y devuelve el `max_scroll_u` y el desplazamiento tras cada paso.
fn repositions(dialog: &mut Dialog, steps_2: &[((i64, i64), u32)]) -> Vec<(i64, i64)> {
    let original = without_pan(dialog);
    let values = dialog.values();
    let pending = dialog.hex_pending("color");
    let text_2 = dialog.text("color");
    let has_cancel = dialog.spec().cancel.is_some();
    let mut output_2 = Vec::new();
    for (i, (window, n)) in steps_2.iter().copied().enumerate() {
        let l = dialog_layout(
            window,
            n,
            dialog.content_h_u(),
            dialog.is_fill(),
            has_cancel,
        );
        let expected_2 = dialog.scroll_u().clamp(0, l.max_scroll_u);
        dialog.set_scroll_u(expected_2);
        assert_eq!(dialog.scroll_u(), expected_2, "paso {i}");
        assert_eq!(without_pan(dialog), original, "paso {i}: el Dialog cambió");
        assert_eq!(dialog.values(), values, "paso {i}: valores");
        assert_eq!(dialog.hex_pending("color"), pending, "paso {i}: pendiente");
        assert_eq!(
            dialog.text("color"),
            text_2,
            "paso {i}: texto del hexadecimal"
        );
        output_2.push((l.max_scroll_u, dialog.scroll_u()));
    }
    output_2
}

#[test]
fn row_8_with_huge_content_the_scroll_of_100_u_survives_all_the_changes() {
    let mut dialog = dialog_huge();
    let output_2 = repositions(&mut dialog, &STEPS_ROW_8);
    for (i, (max, scroll)) in output_2.into_iter().enumerate() {
        assert!(max > 100, "paso {i}: max_scroll_u {max}");
        assert_eq!(scroll, 100, "paso {i}");
    }
}

#[test]
fn row_8_the_scroll_is_clipped_to_the_maximum_and_the_values_and_the_pending_one_do_not_change() {
    let mut dialog = dialog_with_values();
    let content = dialog.content_h_u();
    assert!(
        content > 252,
        "con el modo 2 el formulario de all_kinds debe superar el cuerpo mínimo: {content} u"
    );
    let output_2 = repositions(&mut dialog, &STEPS_ROW_8);

    // A la ventana mínima (n = 1, 2 y 3) el cuerpo mide 252 u: se recorta a contenido − 252.
    for (i, (max, scroll)) in output_2.iter().take(3).copied().enumerate() {
        assert_eq!(max, content - 252, "paso {i}");
        assert_eq!(scroll, 100i64.min(content - 252), "paso {i}");
    }
    // En 960 × 720 el formulario cabe: no hay desplazamiento y no vuelve al volver a la mínima.
    assert_eq!(output_2[3], (0, 0));
    assert_eq!(output_2[4].1, 0);
}

#[test]
fn row_8_the_same_dialog_at_the_five_scales_in_the_minimum_window_and_in_960_by_720() {
    let mut steps_2 = Vec::new();
    for (ppp, window) in MINIMUM {
        steps_2.push((window, text_scale(ppp)));
        steps_2.push((window_960x720(ppp), text_scale(ppp)));
    }
    for creates in [dialog_huge as fn() -> Dialog, dialog_with_values] {
        let mut dialog = creates();
        repositions(&mut dialog, &steps_2);
    }
}

#[test]
fn row_8_with_the_content_shrunk_the_scroll_is_clipped_and_what_was_written_is_kept() {
    let mut dialog = dialog_with_values();
    let height_with_hex = dialog.content_h_u();
    let (min_px, n) = ((468, 333), 1);
    let l = dialog_layout(min_px, n, height_with_hex, false, true);
    dialog.set_scroll_u(dialog.scroll_u().clamp(0, l.max_scroll_u));
    assert_eq!(
        dialog.scroll_u(),
        100i64.min(l.max_scroll_u),
        "recortado al máximo"
    );
    assert!(l.max_scroll_u > 0);

    // Se oculta el hexadecimal (modo 0): el contenido encoge y el máximo baja.
    dialog.event(Event::Choice {
        field: "modo",
        index: 0,
    });
    assert!(!dialog.is_shown("color"));
    let height_without_hex = dialog.content_h_u();
    assert!(
        height_without_hex < height_with_hex,
        "un campo oculto no ocupa sitio: {height_without_hex} < {height_with_hex}"
    );
    let l = dialog_layout(min_px, n, height_without_hex, false, true);
    assert_eq!(l.max_scroll_u, (height_without_hex - 252).max(0));
    let before = dialog.scroll_u();
    dialog.set_scroll_u(before.clamp(0, l.max_scroll_u));
    assert_eq!(dialog.scroll_u(), before.min(l.max_scroll_u));
    assert!((0..=l.max_scroll_u).contains(&dialog.scroll_u()));

    // Al mostrarlo otra vez conserva lo escrito (el pendiente).
    dialog.event(Event::Choice {
        field: "modo",
        index: 2,
    });
    assert!(dialog.is_shown("color"));
    assert!(dialog.hex_pending("color"));
    assert_eq!(dialog.content_h_u(), height_with_hex);
}

#[test]
fn row_8_licenses_with_fill_takes_its_natural_content_and_at_the_minimum_gives_16_rows() {
    let dialog = Dialog::new(DialogKind::Licenses, licenses_form("texto de prueba"))
        .expect("licenses_form valida");
    assert!(dialog.is_fill());
    assert_eq!(dialog.content_h_u(), FILL_U);
    for (ppp, window) in MINIMUM {
        let n = text_scale(ppp);
        let k = k_of(n);
        let l = dialog_layout(
            window,
            n,
            dialog.content_h_u(),
            dialog.is_fill(),
            dialog.spec().cancel.is_some(),
        );
        assert!(l.cancel.is_none(), "Licencias no tiene Cancelar");
        assert_eq!(l.max_scroll_u, 0, "ppp {ppp}");
        assert_eq!(zone_rows(l.body.height / k - 2 * FILL_U_2), 16, "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: ensure_visible
// ---------------------------------------------------------------------------------------------

const VIEW_U: i64 = 252;

#[test]
fn row_8_ensure_visible_an_already_visible_element_moves_nothing() {
    assert_eq!(ensure_visible(0, 0, 20, VIEW_U), 0);
    assert_eq!(ensure_visible(10, 20, 40, VIEW_U), 10);
    assert_eq!(ensure_visible(100, 100, 120, VIEW_U), 100);
    assert_eq!(ensure_visible(100, 332, 352, VIEW_U), 100);
    // Justo en el borde inferior: el último `u` de la vista.
    assert_eq!(ensure_visible(100, 332, 352, VIEW_U), 100);
    assert_eq!(ensure_visible(0, 232, 252, VIEW_U), 0);
}

#[test]
fn row_8_ensure_visible_from_above_raises_just_enough_to_the_edge_of_the_element() {
    assert_eq!(ensure_visible(100, 50, 70, VIEW_U), 50);
    assert_eq!(ensure_visible(500, 0, 20, VIEW_U), 0);
    assert_eq!(ensure_visible(1000, 999, 1019, VIEW_U), 999);
    // Un solo `u` por encima.
    assert_eq!(ensure_visible(100, 99, 119, VIEW_U), 99);
}

#[test]
fn row_8_ensure_visible_from_below_lowers_just_enough_to_the_foot_of_the_element() {
    assert_eq!(ensure_visible(0, 300, 320, VIEW_U), 320 - VIEW_U);
    assert_eq!(ensure_visible(100, 400, 440, VIEW_U), 440 - VIEW_U);
    // Un solo `u` por debajo.
    assert_eq!(ensure_visible(0, 233, 253, VIEW_U), 1);
}

#[test]
fn row_8_ensure_visible_half_way_leaves_the_whole_element() {
    // Cortado por arriba.
    assert_eq!(ensure_visible(100, 90, 130, VIEW_U), 90);
    // Cortado por abajo.
    assert_eq!(ensure_visible(0, 240, 260, VIEW_U), 8);
    assert_eq!(ensure_visible(100, 340, 380, VIEW_U), 380 - VIEW_U);
}

#[test]
fn row_8_ensure_visible_with_an_element_taller_than_the_view_puts_its_top_edge_on_top() {
    for scroll in [0, 50, 100, 300, 5000] {
        assert_eq!(
            ensure_visible(scroll, 100, 500, VIEW_U),
            100,
            "scroll {scroll}"
        );
    }
    // Un `u` más alto que la vista.
    assert_eq!(ensure_visible(0, 50, 50 + VIEW_U + 1, VIEW_U), 50);
    // Del alto exacto de la vista cabe: el borde superior arriba (o lo justo).
    assert_eq!(ensure_visible(0, 100, 100 + VIEW_U, VIEW_U), 100);
    assert_eq!(ensure_visible(500, 100, 100 + VIEW_U, VIEW_U), 100);
}

#[test]
fn row_8_ensure_visible_with_the_content_shrunk_after_clipping_stays_inside_zero_to_max() {
    // El contenido pasa de 5000 u a 300 u con el desplazamiento en 500: el llamador lo recorta al
    // nuevo máximo (300 − 252 = 48) y después pide la vista del elemento enfocado.
    let max = 300 - VIEW_U;
    let clipped = 500i64.clamp(0, max);
    assert_eq!(clipped, 48);
    // El último elemento (280..300) ya se ve desde 48.
    assert_eq!(ensure_visible(clipped, 280, 300, VIEW_U), 48);
    // Uno de arriba (10..30) lleva el desplazamiento al principio.
    assert_eq!(ensure_visible(clipped, 10, 30, VIEW_U), 10);
    for (top, bottom) in [(0, 20), (10, 30), (100, 120), (280, 300)] {
        let r = ensure_visible(clipped, top, bottom, VIEW_U);
        assert!((0..=max).contains(&r), "{top}..{bottom} da {r}");
        assert!(r <= top && bottom <= r + VIEW_U, "{top}..{bottom} da {r}");
    }
    // El contenido cabe entero (200 u): máximo 0.
    let max = (200 - VIEW_U).max(0);
    assert_eq!(max, 0);
    for (top, bottom) in [(0, 20), (150, 170), (180, 200)] {
        assert_eq!(ensure_visible(0, top, bottom, VIEW_U), 0, "{top}..{bottom}");
    }
}

#[test]
fn row_8_ensure_visible_matches_its_rule_in_a_grid() {
    // Regla: más alto que la vista, arriba; si no, el mínimo movimiento (sin salir de 0).
    for view in [1, 14, 252] {
        for scroll in (0..=600).step_by(7) {
            for top in (0..=600).step_by(11) {
                for height_2 in [0, 1, 14, 16, 251, 252, 253, 400] {
                    let bottom = top + height_2;
                    let expected_2 = if height_2 > view || top < scroll {
                        top
                    } else if bottom > scroll + view {
                        bottom - view
                    } else {
                        scroll
                    };
                    assert_eq!(
                        ensure_visible(scroll, top, bottom, view),
                        expected_2,
                        "scroll {scroll}, elemento {top}..{bottom}, vista {view}"
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3
// ---------------------------------------------------------------------------------------------

#[test]
fn property_3_the_ppp_steps_go_from_1_0_to_4_0() {
    assert_eq!(ppp_of_the_step(0), 1.0);
    assert_eq!(ppp_of_the_step(5), 1.25);
    assert_eq!(ppp_of_the_step(10), 1.5);
    assert_eq!(ppp_of_the_step(20), 2.0);
    assert_eq!(ppp_of_the_step(40), 3.0);
    assert_eq!(ppp_of_the_step(60), 4.0);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Todas las medidas enteras, el diálogo dentro con margen y centrado, título, cuerpo y pie
    /// recubriéndolo, botones y motivo en el pie y `max_scroll_u` coherente, para cualquier
    /// ventana igual o mayor que la mínima y cualquier contenido de 0 a 10 000 u.
    #[test]
    fn property_3_the_split_is_integer_is_inside_and_covers(
        step in 0u32..=60,
        extra_width in 0i64..=3000,
        extra_height in 0i64..=3000,
        content_u in 0i64..=10_000,
        fill: bool,
        has_cancel: bool,
    ) {
        let ppp = ppp_of_the_step(step);
        let n = text_scale(ppp);
        let (width_2, height_2) = minimum(ppp);
        let window = (width_2 + extra_width, height_2 + extra_height);
        let content = Content {
            name_2: "propiedad",
            height_u: if fill { FILL_U } else { content_u },
            fill,
        };
        checks_geometry(window, n, content, has_cancel)?;
    }

    /// En la ventana mínima exacta, con cualquier contenido: ancho `330n`, `x = 69n`, alto
    /// `min(65 + content, 317)n` y cuerpo de 252n como máximo.
    #[test]
    fn property_3_at_the_exact_minimum_window_the_table_is_met(
        step in 0u32..=60,
        content_u in 0i64..=10_000,
        has_cancel: bool,
    ) {
        let ppp = ppp_of_the_step(step);
        let n = text_scale(ppp);
        let k = k_of(n);
        let content = Content { name_2: "minima", height_u: content_u, fill: false };
        let l = checks_geometry(minimum(ppp), n, content, has_cancel)?;
        prop_assert_eq!(l.dialog.width, 330 * k);
        prop_assert_eq!(l.dialog.x, 69 * k);
        prop_assert_eq!(l.dialog.height, (65 + content_u).min(317) * k);
        prop_assert!(l.body.height <= 252 * k);
        prop_assert_eq!(l.max_scroll_u, (content_u - 252).max(0));
    }

    /// `ensure_visible` deja a la vista un elemento que cabe en la vista, y el resultado cae en
    /// `0..=max_scroll_u` si el elemento está dentro del contenido.
    #[test]
    fn property_3_ensure_visible_leaves_the_element_in_view_and_inside_zero_to_max(
        step in 0u32..=60,
        extra_width in 0i64..=2000,
        extra_height in 0i64..=2000,
        content_u in 0i64..=10_000,
        seed_scroll: u32,
        seed_top in 0i64..=10_000,
        height_element in 0i64..=700,
        has_cancel: bool,
    ) {
        let ppp = ppp_of_the_step(step);
        let n = text_scale(ppp);
        let k = k_of(n);
        let (width_2, height_2) = minimum(ppp);
        let window = (width_2 + extra_width, height_2 + extra_height);
        let l = dialog_layout(window, n, content_u, false, has_cancel);
        let view = l.body.height / k;
        let max = l.max_scroll_u;
        let scroll = i64::from(seed_scroll) % (max + 1);
        let top = seed_top % (content_u + 1);
        let bottom = (top + height_element).min(content_u);
        let r = ensure_visible(scroll, top, bottom, view);

        prop_assert!((0..=max).contains(&r), "resultado {} fuera de 0..={}", r, max);
        if bottom - top > view {
            prop_assert_eq!(r, top, "más alto que la vista: arriba");
        } else {
            prop_assert!(r <= top && bottom <= r + view, "el elemento no se ve");
            if scroll <= top && bottom <= scroll + view {
                prop_assert_eq!(r, scroll, "ya visible: no se mueve");
            }
        }
    }

    /// Con ventanas menores que la mínima, el diálogo sigue dentro de la ventana, sin pánico.
    #[test]
    fn property_3_with_windows_smaller_than_the_minimum_the_dialog_stays_inside(
        step in 0u32..=60,
        width_2 in 0i64..=1500,
        height_2 in 0i64..=1100,
        content_u in 0i64..=10_000,
        fill: bool,
        has_cancel: bool,
    ) {
        let ppp = ppp_of_the_step(step);
        let n = text_scale(ppp);
        let l = dialog_layout((width_2, height_2), n, content_u, fill, has_cancel);
        checks_inside_the_window(&l, (width_2, height_2))?;
    }
}
