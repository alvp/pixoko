//! Tests del zoom por operaciones: `View::apply_op` y `Canvas::request_view`
//! (specs/hito-2-menu-herramientas.md, «Casos» fila 6, y la decisión 4 en lo que toca al
//! interruptor `Canvas::set_view_keys`).
//!
//! - `View::apply_op` es pura: `ZoomIn` y `ZoomOut` anclados en el **centro** del lienzo, `ActualSize` =
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
use common::{PPPS, button_2, context, frame, key_2, modifiers_2, move_pointer, near, wheel_line};

const CANVAS: Size = Size {
    width: 700,
    height: 500,
};
const IMAGE: Size = Size {
    width: 64,
    height: 48,
};
const CENTER: (i64, i64) = (350, 250);
const OPERATIONS: [ViewOp; 4] = [
    ViewOp::ZoomIn,
    ViewOp::ZoomOut,
    ViewOp::Fit,
    ViewOp::ActualSize,
];

fn zoom(n: u32) -> Zoom {
    Zoom::from_int(n).unwrap_or_else(|| panic!("el zoom {n} debería existir"))
}

/// Vista a ×`n` anclada en el centro, ya con `clamp`, y desplazada `(dx, dy)` (con `pan`, que
/// también hace `clamp`).
fn view(n: u32, pan: (i64, i64)) -> View {
    let mut v = View::fit(CANVAS, IMAGE);
    v.set_zoom(zoom(n), CENTER);
    v.clamp(CANVAS, IMAGE);
    v.pan(pan.0, pan.1, CANVAS, IMAGE);
    v
}

/// Vista en el zoom mínimo (1/64), anclada en el centro, con `clamp` y desplazada.
fn view_minimum(pan: (i64, i64)) -> View {
    let mut v = View::fit(CANVAS, IMAGE);
    v.set_zoom(Zoom::MIN, CENTER);
    v.clamp(CANVAS, IMAGE);
    v.pan(pan.0, pan.1, CANVAS, IMAGE);
    v
}

fn with_clamp(mut v: View) -> View {
    v.clamp(CANVAS, IMAGE);
    v
}

fn applies(mut v: View, op: ViewOp) -> View {
    v.apply_op(op, CANVAS, IMAGE);
    v
}

/// Modelo de `apply_op`, escrito con las operaciones que ya existen.
fn model(v: View, op: ViewOp) -> View {
    let mut m = v;
    match op {
        ViewOp::ZoomIn => m.set_zoom(v.zoom().zoom_in(), CENTER),
        ViewOp::ZoomOut => m.set_zoom(v.zoom().zoom_out(), CENTER),
        ViewOp::ActualSize => m.set_zoom(Zoom::ONE, CENTER),
        ViewOp::Fit => return View::fit(CANVAS, IMAGE),
        // Necesita el puntero: `apply_op` la deja como está (solo `clamp`).
        ViewOp::CenterOnCursor => {}
    }
    with_clamp(m)
}

/// Vistas de prueba: ×1, ×5 y ×64, sin desplazar y desplazadas, **con el centro del lienzo sobre la
/// imagen** (así el píxel bajo el centro existe).
fn views_with_the_center_over_the_image() -> Vec<(String, View)> {
    let mut v = Vec::new();
    for n in [1, 5, 64] {
        for d in [(0, 0), (7, -5), (-20, 15), (40, 30), (-45, -40)] {
            let view = view(n, d);
            if view.screen_to_image(CENTER, IMAGE).is_some() {
                v.push((format!("×{n} desplazada {d:?}"), view));
            }
        }
    }
    v
}

#[test]
fn view_op_is_a_plain_type() {
    fn plane<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
    plane::<ViewOp>();
}

#[test]
fn row_6_the_test_views_cover_x1_x5_and_x64_with_the_center_over_the_image() {
    let views = views_with_the_center_over_the_image();
    for n in [1, 5, 64] {
        assert!(
            views.iter().any(|(_, v)| v.zoom() == zoom(n)),
            "falta ×{n}; hay {:?}",
            views.iter().map(|(name_2, _)| name_2).collect::<Vec<_>>()
        );
    }
    assert!(
        views
            .iter()
            .any(|(_, v)| v.origin() != View::fit(CANVAS, IMAGE).origin())
    );
}

#[test]
fn row_6_zoom_in_zoom_out_and_actual_do_not_move_the_pixel_under_the_center() {
    for (name_2, before) in views_with_the_center_over_the_image() {
        let pixel = before.screen_to_image_i32(CENTER);
        for op in [ViewOp::ZoomIn, ViewOp::ZoomOut, ViewOp::ActualSize] {
            let after_2 = applies(before, op);
            // Con zoom ≥ ×1 el píxel es el mismo; a 1/K sube a la rejilla del nivel
            // (specs/hito-2-zoom-baldosas.md, decisión 4).
            let k = if after_2.zoom() < Zoom::ONE {
                i32::try_from(after_2.zoom().unscale(1)).expect("K pequeño")
            } else {
                1
            };
            let expected_2 = (pixel.0.div_euclid(k) * k, pixel.1.div_euclid(k) * k);
            assert_eq!(
                after_2.screen_to_image_i32(CENTER),
                expected_2,
                "{name_2}: {op:?} movió el píxel bajo el centro"
            );
        }
    }
}

#[test]
fn row_6_zoom_in_and_zoom_out_give_a_zoom_step() {
    for (name_2, before) in views_with_the_center_over_the_image() {
        assert_eq!(
            applies(before, ViewOp::ZoomIn).zoom(),
            before.zoom().zoom_in(),
            "{name_2}"
        );
        assert_eq!(
            applies(before, ViewOp::ZoomOut).zoom(),
            before.zoom().zoom_out(),
            "{name_2}"
        );
        assert_eq!(
            applies(before, ViewOp::ActualSize).zoom(),
            Zoom::ONE,
            "{name_2}"
        );
    }
}

#[test]
fn row_6_fit_is_view_fit_centered_and_the_largest_level_that_fits() {
    let expected_2 = View::fit(CANVAS, IMAGE);
    // 700×500 con 64×48: ×10 (640×480), centrada en (30, 10).
    assert_eq!(expected_2.zoom(), zoom(10));
    assert_eq!(expected_2.origin(), (30, 10));
    for n in [1, 5, 64] {
        for d in [(0, 0), (7, -5), (-45, -40), (300, 200)] {
            let v = view(n, d);
            assert_eq!(applies(v, ViewOp::Fit), expected_2, "×{n} desplazada {d:?}");
        }
    }
}

#[test]
fn row_6_x64_stays_at_x64_with_zoom_in() {
    for d in [(0, 0), (7, -5), (-45, -40), (900, 700)] {
        let before = view(64, d);
        let after_2 = applies(before, ViewOp::ZoomIn);
        assert_eq!(after_2.zoom(), Zoom::MAX, "{d:?}");
        assert_eq!(after_2, before, "{d:?}: ZoomIn en el máximo no cambia nada");
    }
}

#[test]
fn row_6_zoom_out_at_zoom_min_changes_nothing() {
    for d in [(0, 0), (7, -5), (-45, -40), (900, 700)] {
        let before = view_minimum(d);
        assert_eq!(before.zoom(), Zoom::MIN);
        assert_eq!(applies(before, ViewOp::ZoomOut), before, "{d:?}");
    }
}

#[test]
fn row_6_actual_at_x1_changes_nothing() {
    for d in [(0, 0), (7, -5), (-45, -40)] {
        let before = view(1, d);
        assert_eq!(applies(before, ViewOp::ActualSize), before, "{d:?}");
    }
}

#[test]
fn row_6_all_the_operations_leave_the_view_with_clamp() {
    for n in [1, 5, 64] {
        for d in [(0, 0), (7, -5), (-45, -40), (900, 700), (-900, -700)] {
            let before = view(n, d);
            for op in OPERATIONS {
                let after_2 = applies(before, op);
                assert_eq!(
                    after_2,
                    with_clamp(after_2),
                    "×{n} desplazada {d:?}: {op:?} no deja la vista con clamp"
                );
            }
        }
    }
}

#[test]
fn row_6_apply_op_matches_the_model_set_zoom_at_the_center_plus_clamp() {
    for n in [1, 2, 5, 10, 33, 64] {
        for d in [(0, 0), (7, -5), (-45, -40), (900, 700), (-900, -700)] {
            let before = view(n, d);
            for op in OPERATIONS {
                assert_eq!(
                    applies(before, op),
                    model(before, op),
                    "×{n} desplazada {d:?}: {op:?}"
                );
            }
        }
    }
}

#[test]
fn row_6_the_level_the_column_shows_is_the_zoom_display() {
    assert_eq!(zoom(12).to_string(), "×12");
    assert_eq!(zoom(1).to_string(), "×1");
    assert_eq!(Zoom::MAX.to_string(), "×64");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn row_6_apply_op_over_any_views(
        n in 1u32..=64,
        dx in -3000i64..=3000,
        dy in -3000i64..=3000,
        width_2 in 1u32..=300,
        height_2 in 1u32..=300,
        canvas_w in 16u32..=1500,
        canvas_h in 16u32..=1500,
        op_i in 0usize..4,
    ) {
        let canvas_2 = Size::new(canvas_w, canvas_h);
        let image_2 = Size::new(width_2, height_2);
        let center = (i64::from(canvas_w) / 2, i64::from(canvas_h) / 2);
        let mut before = View::fit(canvas_2, image_2);
        before.set_zoom(zoom(n), center);
        before.pan(dx, dy, canvas_2, image_2);
        let op = OPERATIONS[op_i];
        let mut after_2 = before;
        after_2.apply_op(op, canvas_2, image_2);

        // Termina en `clamp`.
        let mut clamped = after_2;
        clamped.clamp(canvas_2, image_2);
        prop_assert_eq!(after_2, clamped);

        match op {
            ViewOp::Fit => prop_assert_eq!(after_2, View::fit(canvas_2, image_2)),
            ViewOp::ZoomIn => prop_assert_eq!(after_2.zoom(), before.zoom().zoom_in()),
            ViewOp::ZoomOut => prop_assert_eq!(after_2.zoom(), before.zoom().zoom_out()),
            ViewOp::ActualSize => prop_assert_eq!(after_2.zoom(), Zoom::ONE),
            ViewOp::CenterOnCursor => prop_assert_eq!(after_2.zoom(), before.zoom()),
        }

        // Modelo: `set_zoom` anclado en el centro y `clamp`.
        if op != ViewOp::Fit {
            let goal = match op {
                ViewOp::ZoomIn => before.zoom().zoom_in(),
                ViewOp::ZoomOut => before.zoom().zoom_out(),
                _ => Zoom::ONE,
            };
            let mut expected_2 = before;
            expected_2.set_zoom(goal, center);
            expected_2.clamp(canvas_2, image_2);
            prop_assert_eq!(after_2, expected_2);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Canvas::request_view y Canvas::zoom (banco de egui)
// ---------------------------------------------------------------------------------------------

/// Pantalla de 800×600 puntos y imagen RGBA de 64×48: a `ppp` 1,0 el ajuste es ×12.
const TEST_SCREEN: egui::Vec2 = vec2(800.0, 600.0);

fn image_2() -> Image {
    Image::new(64, 48, Mode::Rgba).expect("imagen válida")
}

struct Bench {
    ctx: Context,
    canvas: Canvas,
}

impl Bench {
    fn new_2(ppp: f32) -> Bench {
        let mut canvas = Canvas::new();
        canvas.set_image(&image_2());
        Bench {
            ctx: context(ppp),
            canvas,
        }
    }

    fn frame(&mut self, events: Vec<Event>) -> (CanvasInfo, Vec<CanvasInput>) {
        let mut result = None;
        let canvas = &mut self.canvas;
        frame(&self.ctx, TEST_SCREEN, events, |ui| {
            let entries = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            let info = canvas.paint(ui);
            result = Some((info, entries));
        });
        result.expect("el lienzo se dibuja")
    }

    fn info(&mut self, events: Vec<Event>) -> CanvasInfo {
        self.frame(events).0
    }

    /// Un fotograma de reposo con el puntero en el centro del lienzo.
    fn with_the_pointer_at_the_center(&mut self) -> CanvasInfo {
        self.info(vec![move_pointer(pos2(400.0, 300.0))]);
        self.info(vec![])
    }
}

#[test]
fn row_6_zoom_is_none_without_an_image_and_the_fit_one_with_it() {
    let mut without_image = Bench {
        ctx: context(1.0),
        canvas: Canvas::new(),
    };
    assert_eq!(without_image.canvas.zoom(), None);
    without_image.info(vec![]);
    without_image.info(vec![]);
    assert_eq!(
        without_image.canvas.zoom(),
        None,
        "sin imagen no hay zoom que mostrar"
    );

    let mut bench = Bench::new_2(1.0);
    let info = bench.info(vec![]);
    assert_eq!(bench.canvas.zoom(), Some(zoom(12)));
    assert_eq!(bench.canvas.zoom(), Some(info.zoom));
    // El nivel de la columna es el `Display` de ese zoom.
    assert_eq!(
        bench.canvas.zoom().map(|z| z.to_string()).as_deref(),
        Some("×12")
    );
}

#[test]
fn row_6_request_view_is_applied_in_the_next_interact_and_not_before() {
    let mut bench = Bench::new_2(1.0);
    bench.info(vec![]);
    bench.canvas.request_view(ViewOp::ZoomIn);
    assert_eq!(
        bench.canvas.zoom(),
        Some(zoom(12)),
        "pedir la operación no cambia la vista: lo hace el siguiente interact"
    );
    let info = bench.info(vec![]);
    assert_eq!(info.zoom, zoom(13), "se aplica en ese fotograma");
    assert_eq!(bench.canvas.zoom(), Some(zoom(13)));
    // Y se aplica una sola vez.
    let info = bench.info(vec![]);
    assert_eq!(info.zoom, zoom(13));
}

#[test]
fn row_6_request_view_zooms_in_out_fits_and_sets_1_to_1_at_the_five_scales() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp);
        let fit = bench.info(vec![]).zoom;
        bench.canvas.request_view(ViewOp::ZoomIn);
        assert_eq!(bench.info(vec![]).zoom, fit.zoom_in(), "ppp {ppp}: acercar");
        bench.canvas.request_view(ViewOp::ZoomOut);
        assert_eq!(bench.info(vec![]).zoom, fit, "ppp {ppp}: alejar");
        bench.canvas.request_view(ViewOp::Fit);
        assert_eq!(bench.info(vec![]).zoom, fit, "ppp {ppp}: ajustar");
        bench.canvas.request_view(ViewOp::ActualSize);
        assert_eq!(bench.info(vec![]).zoom, Zoom::ONE, "ppp {ppp}: 1:1");
    }
}

#[test]
fn row_6_request_view_does_not_move_the_pixel_under_the_center_of_the_canvas() {
    for ppp in PPPS {
        for op in [ViewOp::ZoomIn, ViewOp::ZoomOut, ViewOp::ActualSize] {
            let mut bench = Bench::new_2(ppp);
            // El puntero queda en el centro del lienzo (la pantalla entera).
            let before = bench.with_the_pointer_at_the_center();
            let pixel = before.hover.expect("el centro está sobre la imagen");
            bench.canvas.request_view(op);
            let after_2 = bench.info(vec![]);
            assert_eq!(
                after_2.hover,
                Some(pixel),
                "ppp {ppp}: {op:?} movió el píxel bajo el centro"
            );
        }
    }
}

#[test]
fn row_6_request_view_fit_leaves_the_image_centered_in_the_canvas() {
    let mut bench = Bench::new_2(1.0);
    bench.info(vec![move_pointer(pos2(100.0, 100.0))]);
    // Aleja la vista: rueda hacia arriba dos veces con el puntero encima.
    bench.info(vec![wheel_line(true), wheel_line(true)]);
    assert_ne!(bench.canvas.zoom(), Some(zoom(12)));
    bench.canvas.request_view(ViewOp::Fit);
    let info = bench.info(vec![]);
    assert_eq!(info.zoom, zoom(12));
    let rect = info.image_rect.expect("hay imagen");
    assert!(near(rect.center().x, 400.0) && near(rect.center().y, 300.0));
}

#[test]
fn row_6_at_zoom_min_and_max_the_requests_that_do_not_fit_change_nothing() {
    let mut bench = Bench::new_2(1.0);
    bench.info(vec![]);
    for _ in 0..80 {
        bench.canvas.request_view(ViewOp::ZoomOut);
        bench.info(vec![]);
    }
    assert_eq!(bench.canvas.zoom(), Some(Zoom::MIN));
    let before = bench.info(vec![]).image_rect;
    bench.canvas.request_view(ViewOp::ZoomOut);
    let after_2 = bench.info(vec![]);
    assert_eq!(after_2.zoom, Zoom::MIN);
    assert_eq!(after_2.image_rect, before);

    for _ in 0..80 {
        bench.canvas.request_view(ViewOp::ZoomIn);
        bench.info(vec![]);
    }
    assert_eq!(bench.canvas.zoom(), Some(Zoom::MAX));
    let before = bench.info(vec![]).image_rect;
    bench.canvas.request_view(ViewOp::ZoomIn);
    let after_2 = bench.info(vec![]);
    assert_eq!(after_2.zoom, Zoom::MAX);
    assert_eq!(after_2.image_rect, before);
}

// ---------------------------------------------------------------------------------------------
// Canvas::set_view_keys
// ---------------------------------------------------------------------------------------------

#[test]
fn with_the_view_keys_disabled_home_plus_equals_and_minus_do_not_act_on_the_canvas() {
    let mut bench = Bench::new_2(1.0);
    bench.canvas.set_view_keys(false);
    bench.info(vec![]);
    for key in [Key::Plus, Key::Equals, Key::Minus, Key::Home] {
        let info = bench.info(vec![key_2(key, Modifiers::NONE)]);
        assert_eq!(
            info.zoom,
            zoom(12),
            "{key:?} no debe actuar con las teclas apagadas"
        );
    }
    // Con Mayús (así se teclea `+` en muchas distribuciones) tampoco.
    let info = bench.info(vec![key_2(Key::Plus, Modifiers::SHIFT)]);
    assert_eq!(info.zoom, zoom(12));
}

#[test]
fn with_the_view_keys_disabled_the_wheel_and_the_pinch_keep_working() {
    let mut bench = Bench::new_2(1.0);
    bench.canvas.set_view_keys(false);
    bench.info(vec![move_pointer(pos2(400.0, 300.0))]);
    let info = bench.info(vec![wheel_line(true)]);
    assert_eq!(info.zoom, zoom(13), "la rueda no cambia");
    let info = bench.info(vec![Event::Zoom(1.2)]);
    assert_eq!(info.zoom, zoom(14), "el pellizco no cambia");
    // Y `request_view` tampoco depende del interruptor.
    bench.canvas.request_view(ViewOp::Fit);
    assert_eq!(bench.info(vec![]).zoom, zoom(12));
}

#[test]
fn the_view_keys_are_active_by_default_and_can_be_reactivated() {
    // 4.3: `+` y `-` ya no son zoom; la tecla de vista del lienzo es `Home` (reencuadrar).
    let mut bench = Bench::new_2(1.0);
    bench.info(vec![]);
    bench.canvas.request_view(ViewOp::ZoomOut);
    assert_eq!(bench.info(vec![]).zoom, zoom(11));
    let info = bench.info(vec![key_2(Key::Home, Modifiers::NONE)]);
    assert_eq!(
        info.zoom,
        zoom(12),
        "por defecto actúan (pruebas del lienzo sin cambios)"
    );
    bench.canvas.request_view(ViewOp::ZoomOut);
    bench.canvas.set_view_keys(false);
    let info = bench.info(vec![key_2(Key::Home, Modifiers::NONE)]);
    assert_eq!(info.zoom, zoom(11));
    bench.canvas.set_view_keys(true);
    let info = bench.info(vec![key_2(Key::Home, Modifiers::NONE)]);
    assert_eq!(info.zoom, zoom(12));
}

#[test]
fn with_the_view_keys_disabled_escape_still_comes_out_ordered() {
    let mut bench = Bench::new_2(1.0);
    bench.canvas.set_view_keys(false);
    bench.info(vec![]);
    let (_, entries) = bench.frame(vec![key_2(Key::Escape, Modifiers::NONE)]);
    assert_eq!(entries, vec![CanvasInput::Escape]);
    // Con modificadores, `Escape` no es de trazo (como antes).
    for bits in 1..32 {
        let (_, entries) = bench.frame(vec![key_2(Key::Escape, modifiers_2(bits))]);
        assert!(entries.is_empty(), "bits {bits:#07b}");
    }
}

#[test]
fn with_the_view_keys_disabled_a_stroke_keeps_working() {
    use px_ui::canvas::CanvasEvent;
    let mut bench = Bench::new_2(1.0);
    bench.canvas.set_view_keys(false);
    bench.info(vec![move_pointer(pos2(400.0, 300.0))]);
    let (_, start_2_2) = bench.frame(vec![button_2(pos2(400.0, 300.0), true)]);
    assert!(
        matches!(
            start_2_2.as_slice(),
            [CanvasInput::Stroke(CanvasEvent::StrokeBegin(_))]
        ),
        "{start_2_2:?}"
    );
    let (_, finish) = bench.frame(vec![button_2(pos2(400.0, 300.0), false)]);
    assert_eq!(
        finish.last(),
        Some(&CanvasInput::Stroke(CanvasEvent::StrokeEnd))
    );
}
