//! Tests del zoom por operaciones: `View::apply_op` y `Canvas::request_view`
//! (specs/hito-2-menu-herramientas.md, «Casos» fila 6, y la decisión 4 en lo que toca al
//! interruptor `Canvas::set_view_keys`).
//!
//! - `View::apply_op` es pura: `ZoomIn` y `ZoomOut` anclados en el **centro** del lienzo, `Actual` =
//!   ×1 anclado en el centro, `Fit` = `View::fit`, y todas terminan en `clamp`. Se prueba sobre una
//!   imagen de 64×48 en un lienzo de 700×500, en ×1, ×5 y ×64 y con la vista desplazada, contra un
//!   modelo escrito con `set_zoom` y `clamp`, más una propiedad con zoom y desplazamientos
//!   cualesquiera.
//! - `Canvas::request_view` (banco de egui sin ventana, como `tests/canvas.rs`): se aplica en el
//!   siguiente `interact`, `Canvas::zoom()` es el zoom de la vista, y el nivel que enseña la
//!   columna es `Zoom::to_string()` de ese zoom.
//! - `Canvas::set_view_keys(false)`: `Home`, `+`, `=` y `-` dejan de actuar en el lienzo (la app los
//!   lleva por la capa de acciones), la rueda y el pellizco no cambian y `Escape` sigue saliendo
//!   ordenado con los eventos de trazo.

use egui::{Context, Event, Key, Modifiers, pos2, vec2};
use proptest::prelude::*;
use px_core::{Image, Mode};
use px_ui::canvas::{Canvas, CanvasInfo, CanvasInput};
use px_ui::view::{Size, View, ViewOp, Zoom};

mod common;
use common::{PPPS, boton, cerca, contexto, fotograma, modificadores, mover, rueda_linea, tecla};

const LIENZO: Size = Size {
    width: 700,
    height: 500,
};
const IMAGEN: Size = Size {
    width: 64,
    height: 48,
};
const CENTRO: (i64, i64) = (350, 250);
const OPERACIONES: [ViewOp; 4] = [ViewOp::ZoomIn, ViewOp::ZoomOut, ViewOp::Fit, ViewOp::Actual];

fn zoom(n: u32) -> Zoom {
    Zoom::from_int(n).unwrap_or_else(|| panic!("el zoom {n} debería existir"))
}

/// Vista a ×`n` anclada en el centro, ya con `clamp`, y desplazada `(dx, dy)` (con `pan`, que
/// también hace `clamp`).
fn vista(n: u32, desplazamiento: (i64, i64)) -> View {
    let mut v = View::fit(LIENZO, IMAGEN);
    v.set_zoom(zoom(n), CENTRO);
    v.clamp(LIENZO, IMAGEN);
    v.pan(desplazamiento.0, desplazamiento.1, LIENZO, IMAGEN);
    v
}

fn con_clamp(mut v: View) -> View {
    v.clamp(LIENZO, IMAGEN);
    v
}

fn aplica(mut v: View, op: ViewOp) -> View {
    v.apply_op(op, LIENZO, IMAGEN);
    v
}

/// Modelo de `apply_op`, escrito con las operaciones que ya existen.
fn modelo(v: View, op: ViewOp) -> View {
    let mut m = v;
    match op {
        ViewOp::ZoomIn => m.set_zoom(v.zoom().zoom_in(), CENTRO),
        ViewOp::ZoomOut => m.set_zoom(v.zoom().zoom_out(), CENTRO),
        ViewOp::Actual => m.set_zoom(Zoom::ONE, CENTRO),
        ViewOp::Fit => return View::fit(LIENZO, IMAGEN),
    }
    con_clamp(m)
}

/// Vistas de prueba: ×1, ×5 y ×64, sin desplazar y desplazadas, **con el centro del lienzo sobre la
/// imagen** (así el píxel bajo el centro existe).
fn vistas_con_el_centro_sobre_la_imagen() -> Vec<(String, View)> {
    let mut v = Vec::new();
    for n in [1, 5, 64] {
        for d in [(0, 0), (7, -5), (-20, 15), (40, 30), (-45, -40)] {
            let vista = vista(n, d);
            if vista.screen_to_image(CENTRO, IMAGEN).is_some() {
                v.push((format!("×{n} desplazada {d:?}"), vista));
            }
        }
    }
    v
}

#[test]
fn view_op_es_un_tipo_plano() {
    fn plano<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
    plano::<ViewOp>();
}

#[test]
fn fila_6_las_vistas_de_prueba_cubren_x1_x5_y_x64_con_el_centro_sobre_la_imagen() {
    let vistas = vistas_con_el_centro_sobre_la_imagen();
    for n in [1, 5, 64] {
        assert!(
            vistas.iter().any(|(_, v)| v.zoom() == zoom(n)),
            "falta ×{n}; hay {:?}",
            vistas.iter().map(|(nombre, _)| nombre).collect::<Vec<_>>()
        );
    }
    assert!(
        vistas
            .iter()
            .any(|(_, v)| v.origin() != View::fit(LIENZO, IMAGEN).origin())
    );
}

#[test]
fn fila_6_acercar_alejar_y_actual_no_mueven_el_pixel_bajo_el_centro() {
    for (nombre, antes) in vistas_con_el_centro_sobre_la_imagen() {
        let pixel = antes.screen_to_image_i32(CENTRO, IMAGEN);
        for op in [ViewOp::ZoomIn, ViewOp::ZoomOut, ViewOp::Actual] {
            let despues = aplica(antes, op);
            assert_eq!(
                despues.screen_to_image_i32(CENTRO, IMAGEN),
                pixel,
                "{nombre}: {op:?} movió el píxel bajo el centro"
            );
        }
    }
}

#[test]
fn fila_6_acercar_y_alejar_dan_un_paso_de_zoom() {
    for (nombre, antes) in vistas_con_el_centro_sobre_la_imagen() {
        assert_eq!(
            aplica(antes, ViewOp::ZoomIn).zoom(),
            antes.zoom().zoom_in(),
            "{nombre}"
        );
        assert_eq!(
            aplica(antes, ViewOp::ZoomOut).zoom(),
            antes.zoom().zoom_out(),
            "{nombre}"
        );
        assert_eq!(aplica(antes, ViewOp::Actual).zoom(), Zoom::ONE, "{nombre}");
    }
}

#[test]
fn fila_6_fit_es_view_fit_centrado_y_el_mayor_entero_que_cabe() {
    let esperado = View::fit(LIENZO, IMAGEN);
    // 700×500 con 64×48: ×10 (640×480), centrada en (30, 10).
    assert_eq!(esperado.zoom(), zoom(10));
    assert_eq!(esperado.origin(), (30, 10));
    for n in [1, 5, 64] {
        for d in [(0, 0), (7, -5), (-45, -40), (300, 200)] {
            let v = vista(n, d);
            assert_eq!(aplica(v, ViewOp::Fit), esperado, "×{n} desplazada {d:?}");
        }
    }
}

#[test]
fn fila_6_x64_sigue_en_x64_con_acercar() {
    for d in [(0, 0), (7, -5), (-45, -40), (900, 700)] {
        let antes = vista(64, d);
        let despues = aplica(antes, ViewOp::ZoomIn);
        assert_eq!(despues.zoom(), Zoom::MAX, "{d:?}");
        assert_eq!(despues, antes, "{d:?}: ZoomIn en el máximo no cambia nada");
    }
}

#[test]
fn fila_6_alejar_en_zoom_min_no_cambia_nada() {
    for d in [(0, 0), (7, -5), (-45, -40), (900, 700)] {
        let antes = vista(1, d);
        assert_eq!(antes.zoom(), Zoom::MIN);
        assert_eq!(aplica(antes, ViewOp::ZoomOut), antes, "{d:?}");
    }
}

#[test]
fn fila_6_actual_en_x1_no_cambia_nada() {
    for d in [(0, 0), (7, -5), (-45, -40)] {
        let antes = vista(1, d);
        assert_eq!(aplica(antes, ViewOp::Actual), antes, "{d:?}");
    }
}

#[test]
fn fila_6_todas_las_operaciones_dejan_la_vista_con_clamp() {
    for n in [1, 5, 64] {
        for d in [(0, 0), (7, -5), (-45, -40), (900, 700), (-900, -700)] {
            let antes = vista(n, d);
            for op in OPERACIONES {
                let despues = aplica(antes, op);
                assert_eq!(
                    despues,
                    con_clamp(despues),
                    "×{n} desplazada {d:?}: {op:?} no deja la vista con clamp"
                );
            }
        }
    }
}

#[test]
fn fila_6_apply_op_coincide_con_el_modelo_set_zoom_en_el_centro_mas_clamp() {
    for n in [1, 2, 5, 10, 33, 64] {
        for d in [(0, 0), (7, -5), (-45, -40), (900, 700), (-900, -700)] {
            let antes = vista(n, d);
            for op in OPERACIONES {
                assert_eq!(
                    aplica(antes, op),
                    modelo(antes, op),
                    "×{n} desplazada {d:?}: {op:?}"
                );
            }
        }
    }
}

#[test]
fn fila_6_el_nivel_que_enseña_la_columna_es_el_display_del_zoom() {
    assert_eq!(zoom(12).to_string(), "×12");
    assert_eq!(zoom(1).to_string(), "×1");
    assert_eq!(Zoom::MAX.to_string(), "×64");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn fila_6_apply_op_sobre_vistas_cualesquiera(
        n in 1u32..=64,
        dx in -3000i64..=3000,
        dy in -3000i64..=3000,
        ancho in 1u32..=300,
        alto in 1u32..=300,
        lienzo_w in 16u32..=1500,
        lienzo_h in 16u32..=1500,
        op_i in 0usize..4,
    ) {
        let lienzo = Size::new(lienzo_w, lienzo_h);
        let imagen = Size::new(ancho, alto);
        let centro = (i64::from(lienzo_w) / 2, i64::from(lienzo_h) / 2);
        let mut antes = View::fit(lienzo, imagen);
        antes.set_zoom(zoom(n), centro);
        antes.pan(dx, dy, lienzo, imagen);
        let op = OPERACIONES[op_i];
        let mut despues = antes;
        despues.apply_op(op, lienzo, imagen);

        // Termina en `clamp`.
        let mut clamped = despues;
        clamped.clamp(lienzo, imagen);
        prop_assert_eq!(despues, clamped);

        match op {
            ViewOp::Fit => prop_assert_eq!(despues, View::fit(lienzo, imagen)),
            ViewOp::ZoomIn => prop_assert_eq!(despues.zoom(), antes.zoom().zoom_in()),
            ViewOp::ZoomOut => prop_assert_eq!(despues.zoom(), antes.zoom().zoom_out()),
            ViewOp::Actual => prop_assert_eq!(despues.zoom(), Zoom::ONE),
        }

        // Modelo: `set_zoom` anclado en el centro y `clamp`.
        if op != ViewOp::Fit {
            let objetivo = match op {
                ViewOp::ZoomIn => antes.zoom().zoom_in(),
                ViewOp::ZoomOut => antes.zoom().zoom_out(),
                _ => Zoom::ONE,
            };
            let mut esperado = antes;
            esperado.set_zoom(objetivo, centro);
            esperado.clamp(lienzo, imagen);
            prop_assert_eq!(despues, esperado);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Canvas::request_view y Canvas::zoom (banco de egui)
// ---------------------------------------------------------------------------------------------

/// Pantalla de 800×600 puntos y imagen RGBA de 64×48: a `ppp` 1,0 el ajuste es ×12.
const PANTALLA: egui::Vec2 = vec2(800.0, 600.0);

fn imagen() -> Image {
    Image::new(64, 48, Mode::Rgba).expect("imagen válida")
}

struct Banco {
    ctx: Context,
    canvas: Canvas,
}

impl Banco {
    fn nuevo(ppp: f32) -> Banco {
        let mut canvas = Canvas::new();
        canvas.set_image(&imagen());
        Banco {
            ctx: contexto(ppp),
            canvas,
        }
    }

    fn fotograma(&mut self, events: Vec<Event>) -> (CanvasInfo, Vec<CanvasInput>) {
        let mut resultado = None;
        let canvas = &mut self.canvas;
        fotograma(&self.ctx, PANTALLA, events, |ui| {
            let entradas = canvas.interact_ordered(ui);
            let info = canvas.paint(ui, std::iter::empty());
            resultado = Some((info, entradas));
        });
        resultado.expect("el lienzo se dibuja")
    }

    fn info(&mut self, events: Vec<Event>) -> CanvasInfo {
        self.fotograma(events).0
    }

    /// Un fotograma de reposo con el puntero en el centro del lienzo.
    fn con_puntero_en_el_centro(&mut self) -> CanvasInfo {
        self.info(vec![mover(pos2(400.0, 300.0))]);
        self.info(vec![])
    }
}

#[test]
fn fila_6_zoom_es_none_sin_imagen_y_el_del_ajuste_con_ella() {
    let mut sin_imagen = Banco {
        ctx: contexto(1.0),
        canvas: Canvas::new(),
    };
    assert_eq!(sin_imagen.canvas.zoom(), None);
    sin_imagen.info(vec![]);
    sin_imagen.info(vec![]);
    assert_eq!(
        sin_imagen.canvas.zoom(),
        None,
        "sin imagen no hay zoom que mostrar"
    );

    let mut banco = Banco::nuevo(1.0);
    let info = banco.info(vec![]);
    assert_eq!(banco.canvas.zoom(), Some(zoom(12)));
    assert_eq!(banco.canvas.zoom(), Some(info.zoom));
    // El nivel de la columna es el `Display` de ese zoom.
    assert_eq!(
        banco.canvas.zoom().map(|z| z.to_string()).as_deref(),
        Some("×12")
    );
}

#[test]
fn fila_6_request_view_se_aplica_en_el_siguiente_interact_y_no_antes() {
    let mut banco = Banco::nuevo(1.0);
    banco.info(vec![]);
    banco.canvas.request_view(ViewOp::ZoomIn);
    assert_eq!(
        banco.canvas.zoom(),
        Some(zoom(12)),
        "pedir la operación no cambia la vista: lo hace el siguiente interact"
    );
    let info = banco.info(vec![]);
    assert_eq!(info.zoom, zoom(13), "se aplica en ese fotograma");
    assert_eq!(banco.canvas.zoom(), Some(zoom(13)));
    // Y se aplica una sola vez.
    let info = banco.info(vec![]);
    assert_eq!(info.zoom, zoom(13));
}

#[test]
fn fila_6_request_view_acerca_aleja_ajusta_y_pone_1_a_1_a_las_cinco_escalas() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp);
        let ajuste = banco.info(vec![]).zoom;
        banco.canvas.request_view(ViewOp::ZoomIn);
        assert_eq!(
            banco.info(vec![]).zoom,
            ajuste.zoom_in(),
            "ppp {ppp}: acercar"
        );
        banco.canvas.request_view(ViewOp::ZoomOut);
        assert_eq!(banco.info(vec![]).zoom, ajuste, "ppp {ppp}: alejar");
        banco.canvas.request_view(ViewOp::Fit);
        assert_eq!(banco.info(vec![]).zoom, ajuste, "ppp {ppp}: ajustar");
        banco.canvas.request_view(ViewOp::Actual);
        assert_eq!(banco.info(vec![]).zoom, Zoom::ONE, "ppp {ppp}: 1:1");
    }
}

#[test]
fn fila_6_request_view_no_mueve_el_pixel_bajo_el_centro_del_lienzo() {
    for ppp in PPPS {
        for op in [ViewOp::ZoomIn, ViewOp::ZoomOut, ViewOp::Actual] {
            let mut banco = Banco::nuevo(ppp);
            // El puntero queda en el centro del lienzo (la pantalla entera).
            let antes = banco.con_puntero_en_el_centro();
            let pixel = antes.hover.expect("el centro está sobre la imagen");
            banco.canvas.request_view(op);
            let despues = banco.info(vec![]);
            assert_eq!(
                despues.hover,
                Some(pixel),
                "ppp {ppp}: {op:?} movió el píxel bajo el centro"
            );
        }
    }
}

#[test]
fn fila_6_request_view_fit_deja_la_imagen_centrada_en_el_lienzo() {
    let mut banco = Banco::nuevo(1.0);
    banco.info(vec![mover(pos2(100.0, 100.0))]);
    // Aleja la vista: rueda hacia arriba dos veces con el puntero encima.
    banco.info(vec![rueda_linea(true), rueda_linea(true)]);
    assert_ne!(banco.canvas.zoom(), Some(zoom(12)));
    banco.canvas.request_view(ViewOp::Fit);
    let info = banco.info(vec![]);
    assert_eq!(info.zoom, zoom(12));
    let rect = info.image_rect.expect("hay imagen");
    assert!(cerca(rect.center().x, 400.0) && cerca(rect.center().y, 300.0));
}

#[test]
fn fila_6_en_zoom_min_y_max_las_peticiones_que_no_caben_no_cambian_nada() {
    let mut banco = Banco::nuevo(1.0);
    banco.info(vec![]);
    for _ in 0..5 {
        banco.canvas.request_view(ViewOp::Actual);
        banco.info(vec![]);
    }
    assert_eq!(banco.canvas.zoom(), Some(Zoom::MIN));
    let antes = banco.info(vec![]).image_rect;
    banco.canvas.request_view(ViewOp::ZoomOut);
    let despues = banco.info(vec![]);
    assert_eq!(despues.zoom, Zoom::MIN);
    assert_eq!(despues.image_rect, antes);

    for _ in 0..80 {
        banco.canvas.request_view(ViewOp::ZoomIn);
        banco.info(vec![]);
    }
    assert_eq!(banco.canvas.zoom(), Some(Zoom::MAX));
    let antes = banco.info(vec![]).image_rect;
    banco.canvas.request_view(ViewOp::ZoomIn);
    let despues = banco.info(vec![]);
    assert_eq!(despues.zoom, Zoom::MAX);
    assert_eq!(despues.image_rect, antes);
}

// ---------------------------------------------------------------------------------------------
// Canvas::set_view_keys
// ---------------------------------------------------------------------------------------------

#[test]
fn con_las_teclas_de_vista_desactivadas_home_mas_igual_y_menos_no_actuan_en_el_lienzo() {
    let mut banco = Banco::nuevo(1.0);
    banco.canvas.set_view_keys(false);
    banco.info(vec![]);
    for key in [Key::Plus, Key::Equals, Key::Minus, Key::Home] {
        let info = banco.info(vec![tecla(key, Modifiers::NONE)]);
        assert_eq!(
            info.zoom,
            zoom(12),
            "{key:?} no debe actuar con las teclas apagadas"
        );
    }
    // Con Mayús (así se teclea `+` en muchas distribuciones) tampoco.
    let info = banco.info(vec![tecla(Key::Plus, Modifiers::SHIFT)]);
    assert_eq!(info.zoom, zoom(12));
}

#[test]
fn con_las_teclas_de_vista_desactivadas_la_rueda_y_el_pellizco_siguen_funcionando() {
    let mut banco = Banco::nuevo(1.0);
    banco.canvas.set_view_keys(false);
    banco.info(vec![mover(pos2(400.0, 300.0))]);
    let info = banco.info(vec![rueda_linea(true)]);
    assert_eq!(info.zoom, zoom(13), "la rueda no cambia");
    let info = banco.info(vec![Event::Zoom(1.2)]);
    assert_eq!(info.zoom, zoom(14), "el pellizco no cambia");
    // Y `request_view` tampoco depende del interruptor.
    banco.canvas.request_view(ViewOp::Fit);
    assert_eq!(banco.info(vec![]).zoom, zoom(12));
}

#[test]
fn las_teclas_de_vista_estan_activas_por_defecto_y_se_pueden_reactivar() {
    let mut banco = Banco::nuevo(1.0);
    banco.info(vec![]);
    let info = banco.info(vec![tecla(Key::Minus, Modifiers::NONE)]);
    assert_eq!(
        info.zoom,
        zoom(11),
        "por defecto actúan (pruebas del lienzo sin cambios)"
    );
    banco.canvas.set_view_keys(false);
    let info = banco.info(vec![tecla(Key::Minus, Modifiers::NONE)]);
    assert_eq!(info.zoom, zoom(11));
    banco.canvas.set_view_keys(true);
    let info = banco.info(vec![tecla(Key::Minus, Modifiers::NONE)]);
    assert_eq!(info.zoom, zoom(10));
}

#[test]
fn con_las_teclas_de_vista_desactivadas_escape_sigue_saliendo_ordenado() {
    let mut banco = Banco::nuevo(1.0);
    banco.canvas.set_view_keys(false);
    banco.info(vec![]);
    let (_, entradas) = banco.fotograma(vec![tecla(Key::Escape, Modifiers::NONE)]);
    assert_eq!(entradas, vec![CanvasInput::Escape]);
    // Con modificadores, `Escape` no es de trazo (como antes).
    for bits in 1..32 {
        let (_, entradas) = banco.fotograma(vec![tecla(Key::Escape, modificadores(bits))]);
        assert!(entradas.is_empty(), "bits {bits:#07b}");
    }
}

#[test]
fn con_las_teclas_de_vista_desactivadas_un_trazo_sigue_funcionando() {
    use px_ui::canvas::CanvasEvent;
    let mut banco = Banco::nuevo(1.0);
    banco.canvas.set_view_keys(false);
    banco.info(vec![mover(pos2(400.0, 300.0))]);
    let (_, empezar) = banco.fotograma(vec![boton(pos2(400.0, 300.0), true)]);
    assert!(
        matches!(
            empezar.as_slice(),
            [CanvasInput::Stroke(CanvasEvent::StrokeBegin(_))]
        ),
        "{empezar:?}"
    );
    let (_, terminar) = banco.fotograma(vec![boton(pos2(400.0, 300.0), false)]);
    assert_eq!(
        terminar.last(),
        Some(&CanvasInput::Stroke(CanvasEvent::StrokeEnd))
    );
}
