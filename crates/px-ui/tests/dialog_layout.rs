//! Tests de la geometría pura `px_ui::dialog_layout` (specs/hito-2-dialogos-modales.md, «Casos»
//! filas 7 y 8 y propiedad 3, y la sección «Geometría»). Sin egui: todo es aritmética entera en
//! píxeles físicos y en `u` (`n` px cada una, `n = text_scale(ppp)`).
//!
//! - Fila 7: `dialog_layout` en la ventana mínima real de cada escala (`468n × 333n` px: 468 × 333,
//!   468 × 333, 936 × 666, 936 × 666 y 1404 × 999 a `ppp` 1,0, 1,25, 1,5, 2,0 y 3,0) y en 960 × 720
//!   pt (px = `round(pt · ppp)`), con contenido de 228 u, de 5000 u y con `Fill` (con y sin
//!   `Cancelar`); ventanas de 300 × 200, 1 × 1 y 0 × 0 px; todas las cifras de la tabla (330n, 20n,
//!   45n, botones de 68n × 16n con 4n de hueco, motivo de 308n, `x = 69n`, `y = 8n` o `20n`, cuerpo
//!   de 252n, 16 filas con `Fill` vía `zone_rows`, `max_scroll_u = contenido − 252`).
//! - Fila 8: un `Dialog` con valores escritos, un hexadecimal pendiente y `scroll_u = 100` pasa por
//!   las ventanas y escalas de la fila 7 sin perder nada salvo el desplazamiento recortado; y
//!   `ensure_visible` con un elemento por encima, por debajo, a medias, más alto que la vista y con
//!   el contenido encogido.
//! - Propiedad 3: geometría entera, dentro de la ventana y recubierta (`ppp` de 1,0 a 4,0 en pasos
//!   de 0,05 como índices enteros, para no acumular error de `f32`).
//!
//! Suposiciones sobre las firmas (la spec no las fija al detalle; anotadas también en el informe):
//! - `dialog_layout(window_px: (i64, i64), n: u32, content_h_u: i64, fill: bool, has_cancel: bool)
//!   -> DialogLayout`, como `chrome::window_layout(window_px, n)`; las constantes `DIALOG_W_U`,
//!   `MARGIN_U`, `TITLE_H_U`, `FOOTER_H_U` y `BUTTON_W_U` son `i64`.
//! - `DialogLayout { dialog, title, body, footer, reason, accept: ScreenRect, cancel:
//!   Option<ScreenRect>, max_scroll_u: i64 }`, con `Clone, Debug, PartialEq`.
//! - Los tamaños en `u` salen de dividir los píxeles de la ventana entre `n` **con división
//!   entera** (`⌊px / n⌋`): ancho `min(330, ⌊ancho/n⌋ − 16)`, alto `min(65 + contenido, ⌊alto/n⌋ −
//!   16)`; la posición es `(ventana − diálogo).div_euclid(2)` en píxeles.
//! - Título, cuerpo y pie ocupan el ancho entero del diálogo. El motivo está en `x = 8n` del
//!   diálogo (el relleno) y `y = 5n` del pie (1 de línea + 4); los botones, en `y = 21n` del pie
//!   (1 + 4 + 14 + 2) y pegados a la derecha con un relleno de `8n`: `Cancelar` el último y
//!   `Aceptar` a su izquierda con `4n` de hueco; sin `Cancelar`, `Aceptar` ocupa su sitio.
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
const MINIMAS: [(f32, (i64, i64)); 5] = [
    (1.0, (468, 333)),
    (1.25, (468, 333)),
    (1.5, (936, 666)),
    (2.0, (936, 666)),
    (3.0, (1404, 999)),
];
/// Contenido natural de un diálogo con `Fill`: `16 + 6 + 22·14`.
const FILL_U: i64 = 330;
/// Título más pie: lo que no es cuerpo.
const MARCO_U: i64 = TITLE_H_U + FOOTER_H_U;
const BOTON_H_U: i64 = 16;
const HUECO_U: i64 = 4;
const RELLENO_U: i64 = 8;

/// Un contenido de las pruebas: su alto en `u` y si es una zona de relleno.
#[derive(Clone, Copy, Debug)]
struct Contenido {
    nombre: &'static str,
    alto_u: i64,
    fill: bool,
}

const CONTENIDOS: [Contenido; 3] = [
    Contenido {
        nombre: "228 u",
        alto_u: 228,
        fill: false,
    },
    Contenido {
        nombre: "5000 u",
        alto_u: 5000,
        fill: false,
    },
    Contenido {
        nombre: "fill",
        alto_u: FILL_U,
        fill: true,
    },
];

fn k_de(n: u32) -> i64 {
    i64::from(n)
}

/// `ppp` de 1,0 a 4,0 en pasos de 0,05 sin acumular error de coma flotante.
fn ppp_del_paso(i: u32) -> f32 {
    (100 + 5 * i) as f32 / 100.0
}

/// La ventana mínima de `ppp` calculada con la fórmula (`468n × 333n`).
fn minima(ppp: f32) -> (i64, i64) {
    let k = k_de(text_scale(ppp));
    (468 * k, 333 * k)
}

/// 960 × 720 pt en píxeles: `round(pt · ppp)`.
fn ventana_960x720(ppp: f32) -> (i64, i64) {
    ((960.0 * ppp).round() as i64, (720.0 * ppp).round() as i64)
}

fn disena(window: (i64, i64), n: u32, contenido: Contenido, has_cancel: bool) -> DialogLayout {
    dialog_layout(window, n, contenido.alto_u, contenido.fill, has_cancel)
}

fn rect(x: i64, y: i64, width: i64, height: i64) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width,
        height,
    }
}

fn dentro(a: ScreenRect, b: ScreenRect) -> bool {
    a.x >= b.x && a.y >= b.y && a.x + a.width <= b.x + b.width && a.y + a.height <= b.y + b.height
}

fn solapan(a: ScreenRect, b: ScreenRect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

/// Todo lo que vale para cualquier ventana igual o mayor que la mínima (sección «Geometría»):
/// medidas de la tabla, recubrimiento de título, cuerpo y pie, botones, motivo, margen, centrado
/// y `max_scroll_u`. Devuelve el diseño para que quien llama compruebe lo suyo.
fn comprueba_geometria(
    window: (i64, i64),
    n: u32,
    contenido: Contenido,
    has_cancel: bool,
) -> Result<DialogLayout, TestCaseError> {
    let k = k_de(n);
    let (w, h) = window;
    let l = disena(window, n, contenido, has_cancel);

    let ancho_u = DIALOG_W_U.min(w.div_euclid(k) - 2 * MARGIN_U);
    let alto_u = (MARCO_U + contenido.alto_u).min(h.div_euclid(k) - 2 * MARGIN_U);
    let (dw, dh) = (ancho_u * k, alto_u * k);
    let d = l.dialog;
    prop_assert_eq!(
        d,
        rect((w - dw).div_euclid(2), (h - dh).div_euclid(2), dw, dh),
        "el diálogo en {:?} con n {} y {:?}",
        window,
        n,
        contenido
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
        rect(d.x, d.y + TITLE_H_U * k, d.width, d.height - MARCO_U * k)
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
        rect(d.x + RELLENO_U * k, l.footer.y + 5 * k, 308 * k, 14 * k)
    );
    let boton_y = l.footer.y + (1 + 4 + 14 + 2) * k;
    let derecha = d.x + d.width - RELLENO_U * k;
    let ancho_boton = BUTTON_W_U * k;
    prop_assert_eq!(l.accept.width, ancho_boton);
    prop_assert_eq!(l.accept.height, BOTON_H_U * k);
    prop_assert_eq!(l.accept.y, boton_y);
    match (has_cancel, l.cancel) {
        (true, Some(cancel)) => {
            prop_assert_eq!(
                cancel,
                rect(derecha - ancho_boton, boton_y, ancho_boton, BOTON_H_U * k)
            );
            prop_assert_eq!(
                l.accept.x,
                cancel.x - HUECO_U * k - ancho_boton,
                "4n de hueco entre los botones"
            );
            prop_assert!(dentro(cancel, l.footer));
            prop_assert!(!solapan(cancel, l.accept));
            prop_assert!(!solapan(cancel, l.reason));
        }
        (false, None) => prop_assert_eq!(l.accept.x, derecha - ancho_boton),
        (esperado, real) => {
            prop_assert!(false, "has_cancel {} pero cancel es {:?}", esperado, real);
        }
    }
    prop_assert!(dentro(l.accept, l.footer));
    prop_assert!(dentro(l.reason, l.footer));
    prop_assert!(!solapan(l.accept, l.reason));

    // Las dimensiones son múltiplos de n.
    for valor in [
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
        prop_assert_eq!(valor % k, 0, "{} no es múltiplo de {}", valor, k);
    }

    // `max_scroll_u`: el contenido menos el cuerpo, 0 si cabe; con `Fill`, siempre 0.
    let cuerpo_u = l.body.height / k;
    if contenido.fill {
        prop_assert_eq!(l.max_scroll_u, 0, "Fill no desplaza el cuerpo");
    } else {
        prop_assert_eq!(l.max_scroll_u, (contenido.alto_u - cuerpo_u).max(0));
        prop_assert_eq!(l.max_scroll_u == 0, contenido.alto_u <= cuerpo_u);
    }
    Ok(l)
}

fn ok<T>(resultado: Result<T, TestCaseError>, etiqueta: &str) -> T {
    resultado.unwrap_or_else(|error| panic!("{etiqueta}: {error}"))
}

// ---------------------------------------------------------------------------------------------
// Fila 7: constantes y ventana mínima
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_7_las_constantes_valen_lo_que_dice_la_spec() {
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
fn fila_7_la_tabla_de_ventanas_minimas_es_468n_por_333n() {
    for (ppp, (ancho, alto)) in MINIMAS {
        assert_eq!(minima(ppp), (ancho, alto), "ppp {ppp}");
    }
    let escalas: Vec<u32> = PPPS.iter().map(|ppp| text_scale(*ppp)).collect();
    assert_eq!(escalas, [1, 1, 2, 2, 3]);
}

#[test]
fn fila_7_la_ventana_minima_cumple_la_geometria_con_todos_los_contenidos_y_con_y_sin_cancelar() {
    for (ppp, window) in MINIMAS {
        let n = text_scale(ppp);
        for contenido in CONTENIDOS {
            for has_cancel in [true, false] {
                ok(
                    comprueba_geometria(window, n, contenido, has_cancel),
                    &format!("ppp {ppp}, {}, cancelar {has_cancel}", contenido.nombre),
                );
            }
        }
    }
}

#[test]
fn fila_7_con_228_u_cabe_sin_desplazar_y_se_centra_en_vertical() {
    for (ppp, window) in MINIMAS {
        let n = text_scale(ppp);
        let k = k_de(n);
        for has_cancel in [true, false] {
            let l = disena(window, n, CONTENIDOS[0], has_cancel);
            let etiqueta = format!("ppp {ppp}, cancelar {has_cancel}");
            assert_eq!(l.dialog.width, 330 * k, "{etiqueta}");
            assert_eq!(l.dialog.height, (65 + 228) * k, "{etiqueta}");
            assert_eq!(l.dialog.x, 69 * k, "{etiqueta}");
            assert_eq!(l.dialog.y, 20 * k, "{etiqueta}");
            assert_eq!(l.body.height, 228 * k, "{etiqueta}");
            assert_eq!(l.max_scroll_u, 0, "{etiqueta}");
        }
    }
}

#[test]
fn fila_7_con_5000_u_el_alto_es_el_maximo_y_el_cuerpo_mide_252n() {
    for (ppp, window) in MINIMAS {
        let n = text_scale(ppp);
        let k = k_de(n);
        for has_cancel in [true, false] {
            let l = disena(window, n, CONTENIDOS[1], has_cancel);
            let etiqueta = format!("ppp {ppp}, cancelar {has_cancel}");
            assert_eq!(l.dialog.width, 330 * k, "{etiqueta}");
            assert_eq!(l.dialog.height, 317 * k, "{etiqueta}");
            assert_eq!(l.dialog.x, 69 * k, "{etiqueta}");
            assert_eq!(l.dialog.y, 8 * k, "{etiqueta}");
            assert_eq!(l.title.height, 20 * k, "{etiqueta}");
            assert_eq!(l.footer.height, 45 * k, "{etiqueta}");
            assert_eq!(l.body.height, 252 * k, "{etiqueta}");
            assert_eq!(l.max_scroll_u, 5000 - 252, "{etiqueta}");
        }
    }
}

#[test]
fn fila_7_con_fill_a_la_ventana_minima_hay_16_filas_y_no_se_desplaza() {
    for (ppp, window) in MINIMAS {
        let n = text_scale(ppp);
        let k = k_de(n);
        for has_cancel in [true, false] {
            let l = disena(window, n, CONTENIDOS[2], has_cancel);
            let etiqueta = format!("ppp {ppp}, cancelar {has_cancel}");
            assert_eq!(l.dialog.height, 317 * k, "{etiqueta}");
            assert_eq!(l.dialog.x, 69 * k, "{etiqueta}");
            assert_eq!(l.dialog.y, 8 * k, "{etiqueta}");
            assert_eq!(l.body.height, 252 * k, "{etiqueta}");
            assert_eq!(l.max_scroll_u, 0, "{etiqueta}");
            let caja_u = l.body.height / k - 2 * RELLENO_U;
            assert_eq!(caja_u, 236, "{etiqueta}");
            assert_eq!(zone_rows(caja_u), 16, "{etiqueta}");
        }
    }
}

#[test]
fn fila_7_el_dialogo_mide_lo_mismo_en_u_a_las_cinco_escalas_de_la_ventana_minima() {
    for contenido in CONTENIDOS {
        let referencia = disena(minima(1.0), 1, contenido, true);
        for (ppp, window) in MINIMAS {
            let n = text_scale(ppp);
            let k = k_de(n);
            let l = disena(window, n, contenido, true);
            assert_eq!(l.dialog.width / k, referencia.dialog.width, "{ppp}");
            assert_eq!(l.dialog.height / k, referencia.dialog.height, "{ppp}");
            assert_eq!(l.dialog.x / k, referencia.dialog.x, "{ppp}");
            assert_eq!(l.dialog.y / k, referencia.dialog.y, "{ppp}");
            assert_eq!(l.max_scroll_u, referencia.max_scroll_u, "{ppp}");
        }
    }
}

#[test]
fn fila_7_el_alto_es_min_65_mas_contenido_y_317_en_la_minima() {
    for (ppp, window) in MINIMAS {
        let n = text_scale(ppp);
        let k = k_de(n);
        for contenido_u in [0, 1, 100, 227, 228, 251, 252, 253, 300, 1000, 5000, 10_000] {
            let l = dialog_layout(window, n, contenido_u, false, true);
            assert_eq!(
                l.dialog.height,
                (65 + contenido_u).min(317) * k,
                "ppp {ppp}, contenido {contenido_u}"
            );
            assert_eq!(
                l.max_scroll_u,
                (contenido_u - (l.dialog.height / k - 65)).max(0),
                "ppp {ppp}, contenido {contenido_u}"
            );
        }
    }
}

#[test]
fn fila_7_cancelar_solo_mueve_aceptar_y_lo_demas_queda_igual() {
    for (ppp, window) in MINIMAS {
        let n = text_scale(ppp);
        for contenido in CONTENIDOS {
            let con = disena(window, n, contenido, true);
            let sin = disena(window, n, contenido, false);
            assert_eq!(con.dialog, sin.dialog, "ppp {ppp}");
            assert_eq!(con.title, sin.title, "ppp {ppp}");
            assert_eq!(con.body, sin.body, "ppp {ppp}");
            assert_eq!(con.footer, sin.footer, "ppp {ppp}");
            assert_eq!(con.reason, sin.reason, "ppp {ppp}");
            assert_eq!(con.max_scroll_u, sin.max_scroll_u, "ppp {ppp}");
            assert!(con.cancel.is_some(), "ppp {ppp}");
            assert!(sin.cancel.is_none(), "ppp {ppp}");
        }
    }
}

#[test]
fn fila_7_el_diseno_es_una_funcion_pura() {
    for (ppp, window) in MINIMAS {
        let n = text_scale(ppp);
        for contenido in CONTENIDOS {
            assert_eq!(
                disena(window, n, contenido, true),
                disena(window, n, contenido, true),
                "ppp {ppp}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: 960 × 720 pt
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_7_en_960_por_720_pt_se_cumple_la_geometria_a_las_cinco_escalas() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        let window = ventana_960x720(ppp);
        for contenido in CONTENIDOS {
            for has_cancel in [true, false] {
                ok(
                    comprueba_geometria(window, n, contenido, has_cancel),
                    &format!(
                        "ppp {ppp}, {}, cancelar {has_cancel}, ventana {window:?}",
                        contenido.nombre
                    ),
                );
            }
        }
    }
}

#[test]
fn fila_7_960_por_720_pt_en_pixeles_es_el_producto_redondeado() {
    let esperado = [
        (1.0, (960, 720)),
        (1.25, (1200, 900)),
        (1.5, (1440, 1080)),
        (2.0, (1920, 1440)),
        (3.0, (2880, 2160)),
    ];
    for (ppp, pixeles) in esperado {
        assert_eq!(ventana_960x720(ppp), pixeles, "ppp {ppp}");
    }
}

#[test]
fn fila_7_en_960_por_720_pt_el_alto_sigue_a_65_mas_el_contenido_con_tope_en_la_ventana() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        let k = k_de(n);
        let window = ventana_960x720(ppp);
        let alto_max_u = window.1.div_euclid(k) - 16;
        for has_cancel in [true, false] {
            let corto = disena(window, n, CONTENIDOS[0], has_cancel);
            assert_eq!(corto.dialog.width, 330 * k, "ppp {ppp}");
            assert_eq!(corto.dialog.height, 293 * k, "ppp {ppp}");
            assert_eq!(corto.max_scroll_u, 0, "ppp {ppp}");

            let largo = disena(window, n, CONTENIDOS[1], has_cancel);
            assert_eq!(largo.dialog.height, alto_max_u * k, "ppp {ppp}");
            assert_eq!(largo.max_scroll_u, 5000 - (alto_max_u - 65), "ppp {ppp}");

            // Con la ventana grande el `Fill` ocupa su contenido natural: 22 filas.
            let fill = disena(window, n, CONTENIDOS[2], has_cancel);
            assert_eq!(fill.dialog.height, 395 * k, "ppp {ppp}");
            assert_eq!(fill.max_scroll_u, 0, "ppp {ppp}");
            assert_eq!(
                zone_rows(fill.body.height / k - 2 * RELLENO_U),
                22,
                "ppp {ppp}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: ventanas menores que la mínima
// ---------------------------------------------------------------------------------------------

fn comprueba_dentro_de_la_ventana(
    l: &DialogLayout,
    window: (i64, i64),
) -> Result<(), TestCaseError> {
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
fn fila_7_con_300_por_200_px_el_dialogo_se_encoge_y_queda_dentro() {
    // A n = 1 la fórmula sigue valiendo: ancho min(330, 300 − 16) y alto min(65 + c, 200 − 16).
    for contenido in CONTENIDOS {
        for has_cancel in [true, false] {
            let l = disena((300, 200), 1, contenido, has_cancel);
            assert_eq!(l.dialog.width, 284, "{}", contenido.nombre);
            assert_eq!(
                l.dialog.height,
                (65 + contenido.alto_u).min(184),
                "{}",
                contenido.nombre
            );
            assert_eq!(l.dialog.x, 8, "{}", contenido.nombre);
            assert_eq!(
                l.dialog.y,
                (200 - l.dialog.height).div_euclid(2),
                "{}",
                contenido.nombre
            );
            ok(
                comprueba_dentro_de_la_ventana(&l, (300, 200)),
                contenido.nombre,
            );
            // Título, cuerpo y pie siguen apilados y recubriendo el diálogo.
            assert_eq!(l.title.height, 20);
            assert_eq!(l.footer.height, 45);
            assert_eq!(l.body.height, l.dialog.height - 65);
        }
    }
}

#[test]
fn fila_7_con_ventanas_menores_que_la_minima_el_dialogo_queda_dentro_y_no_hay_panico() {
    let ventanas = [
        (300, 200),
        (1, 1),
        (0, 0),
        (468, 100),
        (100, 333),
        (467, 332),
    ];
    for ppp in PPPS {
        let n = text_scale(ppp);
        for window in ventanas {
            for contenido in CONTENIDOS {
                for has_cancel in [true, false] {
                    let l = disena(window, n, contenido, has_cancel);
                    ok(
                        comprueba_dentro_de_la_ventana(&l, window),
                        &format!(
                            "ppp {ppp}, ventana {window:?}, {}, cancelar {has_cancel}",
                            contenido.nombre
                        ),
                    );
                }
            }
        }
    }
}

#[test]
fn fila_7_una_ventana_de_0_por_0_deja_un_dialogo_vacio_en_el_origen() {
    for n in [1u32, 2, 3, 8] {
        let l = disena((0, 0), n, CONTENIDOS[1], true);
        assert_eq!((l.dialog.width, l.dialog.height), (0, 0), "n {n}");
        assert_eq!((l.dialog.x, l.dialog.y), (0, 0), "n {n}");
    }
}

// ---------------------------------------------------------------------------------------------
// `zone_rows`
// ---------------------------------------------------------------------------------------------

#[test]
fn zone_rows_es_el_suelo_de_alto_menos_6_entre_14() {
    let casos = [
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
    for (alto, filas) in casos {
        assert_eq!(zone_rows(alto), filas, "caja de {alto} u");
    }
}

#[test]
fn zone_rows_con_cajas_menores_que_una_fila_o_negativas_da_cero_sin_panico() {
    for alto in [-1_000_000, -100, -1, 0, 1, 5, 6, 7, 19] {
        assert_eq!(zone_rows(alto), 0, "caja de {alto} u");
    }
}

#[test]
fn zone_rows_no_baja_al_crecer_la_caja() {
    let mut anterior = 0;
    for alto in 0..2000 {
        let filas = zone_rows(alto);
        assert!(filas >= anterior, "caja de {alto} u");
        assert!(filas - anterior <= 1, "caja de {alto} u");
        anterior = filas;
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: recolocar sin perder valores
// ---------------------------------------------------------------------------------------------

/// El orden de la fila 8: 468×333 (`n` 1), 936×666 (2), 1404×999 (3), 960×720 y otra vez 468×333.
const PASOS_FILA_8: [((i64, i64), u32); 5] = [
    ((468, 333), 1),
    ((936, 666), 2),
    ((1404, 999), 3),
    ((960, 720), 1),
    ((468, 333), 1),
];

/// Un `Dialog` igual al de `d` salvo el desplazamiento, que se pone a 0.
fn sin_desplazamiento(d: &Dialog) -> Dialog {
    let mut copia = d.clone();
    copia.set_scroll_u(0);
    copia
}

/// Un `Dialog` con valores escritos (ancho 100, alto 200, modo 2, opaco) y un hexadecimal
/// pendiente de confirmar, con `scroll_u = 100`.
fn dialogo_con_valores() -> Dialog {
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
fn dialogo_enorme() -> Dialog {
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

/// Pasa `dialog` por `pasos` como lo hace `show_dialog`: pide el diseño con el contenido y recorta
/// el desplazamiento a `0..=max_scroll_u`. Comprueba que, salvo el desplazamiento, el `Dialog` no
/// cambia, y devuelve el `max_scroll_u` y el desplazamiento tras cada paso.
fn recoloca(dialog: &mut Dialog, pasos: &[((i64, i64), u32)]) -> Vec<(i64, i64)> {
    let original = sin_desplazamiento(dialog);
    let valores = dialog.values();
    let pendiente = dialog.hex_pending("color");
    let texto = dialog.text("color");
    let has_cancel = dialog.spec().cancel.is_some();
    let mut salida = Vec::new();
    for (i, (window, n)) in pasos.iter().copied().enumerate() {
        let l = dialog_layout(
            window,
            n,
            dialog.content_h_u(),
            dialog.is_fill(),
            has_cancel,
        );
        let esperado = dialog.scroll_u().clamp(0, l.max_scroll_u);
        dialog.set_scroll_u(esperado);
        assert_eq!(dialog.scroll_u(), esperado, "paso {i}");
        assert_eq!(
            sin_desplazamiento(dialog),
            original,
            "paso {i}: el Dialog cambió"
        );
        assert_eq!(dialog.values(), valores, "paso {i}: valores");
        assert_eq!(
            dialog.hex_pending("color"),
            pendiente,
            "paso {i}: pendiente"
        );
        assert_eq!(
            dialog.text("color"),
            texto,
            "paso {i}: texto del hexadecimal"
        );
        salida.push((l.max_scroll_u, dialog.scroll_u()));
    }
    salida
}

#[test]
fn fila_8_con_contenido_enorme_el_desplazamiento_de_100_u_sobrevive_a_todos_los_cambios() {
    let mut dialog = dialogo_enorme();
    let salida = recoloca(&mut dialog, &PASOS_FILA_8);
    for (i, (max, scroll)) in salida.into_iter().enumerate() {
        assert!(max > 100, "paso {i}: max_scroll_u {max}");
        assert_eq!(scroll, 100, "paso {i}");
    }
}

#[test]
fn fila_8_el_desplazamiento_se_recorta_al_maximo_y_los_valores_y_el_pendiente_no_cambian() {
    let mut dialog = dialogo_con_valores();
    let contenido = dialog.content_h_u();
    assert!(
        contenido > 252,
        "con el modo 2 el formulario de all_kinds debe superar el cuerpo mínimo: {contenido} u"
    );
    let salida = recoloca(&mut dialog, &PASOS_FILA_8);

    // A la ventana mínima (n = 1, 2 y 3) el cuerpo mide 252 u: se recorta a contenido − 252.
    for (i, (max, scroll)) in salida.iter().take(3).copied().enumerate() {
        assert_eq!(max, contenido - 252, "paso {i}");
        assert_eq!(scroll, 100i64.min(contenido - 252), "paso {i}");
    }
    // En 960 × 720 el formulario cabe: no hay desplazamiento y no vuelve al volver a la mínima.
    assert_eq!(salida[3], (0, 0));
    assert_eq!(salida[4].1, 0);
}

#[test]
fn fila_8_el_mismo_dialogo_por_las_cinco_escalas_en_la_ventana_minima_y_en_960_por_720() {
    let mut pasos = Vec::new();
    for (ppp, window) in MINIMAS {
        pasos.push((window, text_scale(ppp)));
        pasos.push((ventana_960x720(ppp), text_scale(ppp)));
    }
    for crea in [dialogo_enorme as fn() -> Dialog, dialogo_con_valores] {
        let mut dialog = crea();
        recoloca(&mut dialog, &pasos);
    }
}

#[test]
fn fila_8_con_el_contenido_encogido_se_recorta_el_desplazamiento_y_se_conserva_lo_escrito() {
    let mut dialog = dialogo_con_valores();
    let alto_con_hex = dialog.content_h_u();
    let (min_px, n) = ((468, 333), 1);
    let l = dialog_layout(min_px, n, alto_con_hex, false, true);
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
    let alto_sin_hex = dialog.content_h_u();
    assert!(
        alto_sin_hex < alto_con_hex,
        "un campo oculto no ocupa sitio: {alto_sin_hex} < {alto_con_hex}"
    );
    let l = dialog_layout(min_px, n, alto_sin_hex, false, true);
    assert_eq!(l.max_scroll_u, (alto_sin_hex - 252).max(0));
    let antes = dialog.scroll_u();
    dialog.set_scroll_u(antes.clamp(0, l.max_scroll_u));
    assert_eq!(dialog.scroll_u(), antes.min(l.max_scroll_u));
    assert!((0..=l.max_scroll_u).contains(&dialog.scroll_u()));

    // Al mostrarlo otra vez conserva lo escrito (el pendiente).
    dialog.event(Event::Choice {
        field: "modo",
        index: 2,
    });
    assert!(dialog.is_shown("color"));
    assert!(dialog.hex_pending("color"));
    assert_eq!(dialog.content_h_u(), alto_con_hex);
}

#[test]
fn fila_8_licencias_con_fill_ocupa_su_contenido_natural_y_a_la_minima_da_16_filas() {
    let dialog = Dialog::new(DialogKind::Licenses, licenses_form("texto de prueba"))
        .expect("licenses_form valida");
    assert!(dialog.is_fill());
    assert_eq!(dialog.content_h_u(), FILL_U);
    for (ppp, window) in MINIMAS {
        let n = text_scale(ppp);
        let k = k_de(n);
        let l = dialog_layout(
            window,
            n,
            dialog.content_h_u(),
            dialog.is_fill(),
            dialog.spec().cancel.is_some(),
        );
        assert!(l.cancel.is_none(), "Licencias no tiene Cancelar");
        assert_eq!(l.max_scroll_u, 0, "ppp {ppp}");
        assert_eq!(
            zone_rows(l.body.height / k - 2 * RELLENO_U),
            16,
            "ppp {ppp}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: ensure_visible
// ---------------------------------------------------------------------------------------------

const VISTA_U: i64 = 252;

#[test]
fn fila_8_ensure_visible_un_elemento_ya_visible_no_mueve_nada() {
    assert_eq!(ensure_visible(0, 0, 20, VISTA_U), 0);
    assert_eq!(ensure_visible(10, 20, 40, VISTA_U), 10);
    assert_eq!(ensure_visible(100, 100, 120, VISTA_U), 100);
    assert_eq!(ensure_visible(100, 332, 352, VISTA_U), 100);
    // Justo en el borde inferior: el último `u` de la vista.
    assert_eq!(ensure_visible(100, 332, 352, VISTA_U), 100);
    assert_eq!(ensure_visible(0, 232, 252, VISTA_U), 0);
}

#[test]
fn fila_8_ensure_visible_por_encima_sube_lo_justo_hasta_el_borde_del_elemento() {
    assert_eq!(ensure_visible(100, 50, 70, VISTA_U), 50);
    assert_eq!(ensure_visible(500, 0, 20, VISTA_U), 0);
    assert_eq!(ensure_visible(1000, 999, 1019, VISTA_U), 999);
    // Un solo `u` por encima.
    assert_eq!(ensure_visible(100, 99, 119, VISTA_U), 99);
}

#[test]
fn fila_8_ensure_visible_por_debajo_baja_lo_justo_hasta_el_pie_del_elemento() {
    assert_eq!(ensure_visible(0, 300, 320, VISTA_U), 320 - VISTA_U);
    assert_eq!(ensure_visible(100, 400, 440, VISTA_U), 440 - VISTA_U);
    // Un solo `u` por debajo.
    assert_eq!(ensure_visible(0, 233, 253, VISTA_U), 1);
}

#[test]
fn fila_8_ensure_visible_a_medias_deja_el_elemento_entero() {
    // Cortado por arriba.
    assert_eq!(ensure_visible(100, 90, 130, VISTA_U), 90);
    // Cortado por abajo.
    assert_eq!(ensure_visible(0, 240, 260, VISTA_U), 8);
    assert_eq!(ensure_visible(100, 340, 380, VISTA_U), 380 - VISTA_U);
}

#[test]
fn fila_8_ensure_visible_con_un_elemento_mas_alto_que_la_vista_pone_su_borde_superior_arriba() {
    for scroll in [0, 50, 100, 300, 5000] {
        assert_eq!(
            ensure_visible(scroll, 100, 500, VISTA_U),
            100,
            "scroll {scroll}"
        );
    }
    // Un `u` más alto que la vista.
    assert_eq!(ensure_visible(0, 50, 50 + VISTA_U + 1, VISTA_U), 50);
    // Del alto exacto de la vista cabe: el borde superior arriba (o lo justo).
    assert_eq!(ensure_visible(0, 100, 100 + VISTA_U, VISTA_U), 100);
    assert_eq!(ensure_visible(500, 100, 100 + VISTA_U, VISTA_U), 100);
}

#[test]
fn fila_8_ensure_visible_con_el_contenido_encogido_tras_recortar_queda_dentro_de_cero_a_max() {
    // El contenido pasa de 5000 u a 300 u con el desplazamiento en 500: el llamador lo recorta al
    // nuevo máximo (300 − 252 = 48) y después pide la vista del elemento enfocado.
    let max = 300 - VISTA_U;
    let recortado = 500i64.clamp(0, max);
    assert_eq!(recortado, 48);
    // El último elemento (280..300) ya se ve desde 48.
    assert_eq!(ensure_visible(recortado, 280, 300, VISTA_U), 48);
    // Uno de arriba (10..30) lleva el desplazamiento al principio.
    assert_eq!(ensure_visible(recortado, 10, 30, VISTA_U), 10);
    for (top, bottom) in [(0, 20), (10, 30), (100, 120), (280, 300)] {
        let r = ensure_visible(recortado, top, bottom, VISTA_U);
        assert!((0..=max).contains(&r), "{top}..{bottom} da {r}");
        assert!(r <= top && bottom <= r + VISTA_U, "{top}..{bottom} da {r}");
    }
    // El contenido cabe entero (200 u): máximo 0.
    let max = (200 - VISTA_U).max(0);
    assert_eq!(max, 0);
    for (top, bottom) in [(0, 20), (150, 170), (180, 200)] {
        assert_eq!(
            ensure_visible(0, top, bottom, VISTA_U),
            0,
            "{top}..{bottom}"
        );
    }
}

#[test]
fn fila_8_ensure_visible_coincide_con_su_regla_en_una_rejilla() {
    // Regla: más alto que la vista, arriba; si no, el mínimo movimiento (sin salir de 0).
    for view in [1, 14, 252] {
        for scroll in (0..=600).step_by(7) {
            for top in (0..=600).step_by(11) {
                for alto in [0, 1, 14, 16, 251, 252, 253, 400] {
                    let bottom = top + alto;
                    let esperado = if alto > view || top < scroll {
                        top
                    } else if bottom > scroll + view {
                        bottom - view
                    } else {
                        scroll
                    };
                    assert_eq!(
                        ensure_visible(scroll, top, bottom, view),
                        esperado,
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
fn propiedad_3_los_pasos_de_ppp_van_de_1_0_a_4_0() {
    assert_eq!(ppp_del_paso(0), 1.0);
    assert_eq!(ppp_del_paso(5), 1.25);
    assert_eq!(ppp_del_paso(10), 1.5);
    assert_eq!(ppp_del_paso(20), 2.0);
    assert_eq!(ppp_del_paso(40), 3.0);
    assert_eq!(ppp_del_paso(60), 4.0);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Todas las medidas enteras, el diálogo dentro con margen y centrado, título, cuerpo y pie
    /// recubriéndolo, botones y motivo en el pie y `max_scroll_u` coherente, para cualquier
    /// ventana igual o mayor que la mínima y cualquier contenido de 0 a 10 000 u.
    #[test]
    fn propiedad_3_el_reparto_es_entero_esta_dentro_y_recubre(
        paso in 0u32..=60,
        extra_ancho in 0i64..=3000,
        extra_alto in 0i64..=3000,
        contenido_u in 0i64..=10_000,
        fill: bool,
        has_cancel: bool,
    ) {
        let ppp = ppp_del_paso(paso);
        let n = text_scale(ppp);
        let (ancho, alto) = minima(ppp);
        let ventana = (ancho + extra_ancho, alto + extra_alto);
        let contenido = Contenido {
            nombre: "propiedad",
            alto_u: if fill { FILL_U } else { contenido_u },
            fill,
        };
        comprueba_geometria(ventana, n, contenido, has_cancel)?;
    }

    /// En la ventana mínima exacta, con cualquier contenido: ancho `330n`, `x = 69n`, alto
    /// `min(65 + contenido, 317)n` y cuerpo de 252n como máximo.
    #[test]
    fn propiedad_3_a_la_ventana_minima_exacta_se_cumple_la_tabla(
        paso in 0u32..=60,
        contenido_u in 0i64..=10_000,
        has_cancel: bool,
    ) {
        let ppp = ppp_del_paso(paso);
        let n = text_scale(ppp);
        let k = k_de(n);
        let contenido = Contenido { nombre: "minima", alto_u: contenido_u, fill: false };
        let l = comprueba_geometria(minima(ppp), n, contenido, has_cancel)?;
        prop_assert_eq!(l.dialog.width, 330 * k);
        prop_assert_eq!(l.dialog.x, 69 * k);
        prop_assert_eq!(l.dialog.height, (65 + contenido_u).min(317) * k);
        prop_assert!(l.body.height <= 252 * k);
        prop_assert_eq!(l.max_scroll_u, (contenido_u - 252).max(0));
    }

    /// `ensure_visible` deja a la vista un elemento que cabe en la vista, y el resultado cae en
    /// `0..=max_scroll_u` si el elemento está dentro del contenido.
    #[test]
    fn propiedad_3_ensure_visible_deja_el_elemento_a_la_vista_y_dentro_de_cero_a_max(
        paso in 0u32..=60,
        extra_ancho in 0i64..=2000,
        extra_alto in 0i64..=2000,
        contenido_u in 0i64..=10_000,
        semilla_scroll: u32,
        semilla_top in 0i64..=10_000,
        alto_elemento in 0i64..=700,
        has_cancel: bool,
    ) {
        let ppp = ppp_del_paso(paso);
        let n = text_scale(ppp);
        let k = k_de(n);
        let (ancho, alto) = minima(ppp);
        let ventana = (ancho + extra_ancho, alto + extra_alto);
        let l = dialog_layout(ventana, n, contenido_u, false, has_cancel);
        let vista = l.body.height / k;
        let max = l.max_scroll_u;
        let scroll = i64::from(semilla_scroll) % (max + 1);
        let top = semilla_top % (contenido_u + 1);
        let bottom = (top + alto_elemento).min(contenido_u);
        let r = ensure_visible(scroll, top, bottom, vista);

        prop_assert!((0..=max).contains(&r), "resultado {} fuera de 0..={}", r, max);
        if bottom - top > vista {
            prop_assert_eq!(r, top, "más alto que la vista: arriba");
        } else {
            prop_assert!(r <= top && bottom <= r + vista, "el elemento no se ve");
            if scroll <= top && bottom <= scroll + vista {
                prop_assert_eq!(r, scroll, "ya visible: no se mueve");
            }
        }
    }

    /// Con ventanas menores que la mínima, el diálogo sigue dentro de la ventana, sin pánico.
    #[test]
    fn propiedad_3_con_ventanas_menores_que_la_minima_el_dialogo_sigue_dentro(
        paso in 0u32..=60,
        ancho in 0i64..=1500,
        alto in 0i64..=1100,
        contenido_u in 0i64..=10_000,
        fill: bool,
        has_cancel: bool,
    ) {
        let ppp = ppp_del_paso(paso);
        let n = text_scale(ppp);
        let l = dialog_layout((ancho, alto), n, contenido_u, fill, has_cancel);
        comprueba_dentro_de_la_ventana(&l, (ancho, alto))?;
    }
}
