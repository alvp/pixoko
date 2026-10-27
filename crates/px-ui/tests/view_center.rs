//! Tests de «Zoom y centrar» (specs/interfaz-teclado.md, unidad I.1, fila 6): el zoom por las teclas
//! nuevas frente al de antes, `View::center_on` (parte pura) y `Canvas::request_view(
//! ViewOp::CenterOnCursor)` (bench de egui without window, with y without pointer over el canvas_2, with other
//! capa encima y con un trazo a medias). En ROJO hasta la lógica: `ViewOp::CenterOnCursor`,
//! `View::center_on` y la tabla de teclas no existen todavía.
//!
//! - Parte pura. Imagen de 64×48 en un lienzo de 700×500, zooms ×1, ×5 y ×64, con la vista
//!   desplazada o no, y el punto en el centro, sobre una esquina de la imagen y fuera de ella:
//!   el zoom no cambia, la vista queda con `clamp` y el píxel que había bajo el punto pasa a estar
//!   bajo el centro del lienzo. Fuera de la imagen no rompe nada y manda `clamp`.
//!   `View::apply_op(ViewOp::CenterOnCursor)` no la conoce y deja la vista como está.
//! - Zoom por teclas: `>`, `+` y `=` dan `ViewOp::ZoomIn`, `<` y `-` dan `ViewOp::ZoomOut`, `Start`
//!   da `Fit` y `Ctrl+1` da `ActualSize`, y aplicadas a las vistas dan lo mismo que el modelo de
//!   `view_ops.rs` (`set_zoom` en el centro y `clamp`).
//! - Banco (copiando el estilo de `view_ops.rs` y `canvas_layers.rs`): con el puntero sobre el
//!   lienzo, el píxel bajo el cursor pasa al centro y el zoom no cambia; sin puntero, con el
//!   puntero fuera del lienzo o con otra capa encima, la vista no cambia; con un trazo a medias
//!   mueve la vista y el trazo sigue.
//!
//! Suposiciones sobre lo que la spec no fija (anotadas en el informe):
//! - `View::center_on(point, viewport, image)` recibe `point` en **píxeles de pantalla relativos
//!   al lienzo** (como `screen_to_image` y `set_zoom(anchor)`), no en coordenadas de imagen.
//! - «El píxel bajo el punto queda en el centro» se comprueba por igualdad del índice de píxel
//!   bajo el centro del lienzo (antes: bajo el punto) cuando el punto cae sobre la imagen; la
//!   spec admite ±1 px y la igualdad lo cumple.
//! - `request_view(CenterOnCursor)` se resuelve en el siguiente `interact` con la última posición
//!   del puntero (`latest_pos`); sin ninguna posición no hace nada.

use egui::{Area, Context, Event, Id, Order, Pos2, Rect, Vec2, pos2, vec2};
use px_core::{Image, Mode};
use px_ui::actions::{AppAction, Platform};
use px_ui::canvas::{Canvas, CanvasEvent, CanvasInfo, CanvasInput};
use px_ui::keymap::{KeyId, Keymap, Press};
use px_ui::view::{Size, View, ViewOp, Zoom};

mod common;
use common::{PPPS, button_2, context, frame, move_pointer};

const CANVAS: Size = Size {
    width: 700,
    height: 500,
};
const IMAGE: Size = Size {
    width: 64,
    height: 48,
};
const CENTER: (i64, i64) = (350, 250);
const ZOOMS: [u32; 3] = [1, 5, 64];
const PANS: [(i64, i64); 5] = [(0, 0), (7, -5), (-20, 15), (40, 30), (-45, -40)];

fn zoom(n: u32) -> Zoom {
    Zoom::from_int(n).unwrap_or_else(|| panic!("el zoom {n} debería existir"))
}

/// Vista a ×`n` anclada en el centro, con `clamp`, y desplazada `(dx, dy)` (con `pan`).
fn view(n: u32, pan: (i64, i64)) -> View {
    let mut v = View::fit(CANVAS, IMAGE);
    v.set_zoom(zoom(n), CENTER);
    v.clamp(CANVAS, IMAGE);
    v.pan(pan.0, pan.1, CANVAS, IMAGE);
    v
}

fn with_clamp(mut v: View) -> View {
    v.clamp(CANVAS, IMAGE);
    v
}

fn centered(mut v: View, point: (i64, i64)) -> View {
    v.center_on(point, CANVAS, IMAGE);
    v
}

/// Todas las vistas de prueba: ×1, ×5 y ×64, sin desplazar y desplazadas.
fn views() -> Vec<(String, View)> {
    let mut v = Vec::new();
    for n in ZOOMS {
        for d in PANS {
            v.push((format!("×{n} desplazada {d:?}"), view(n, d)));
        }
    }
    v
}

/// El centro del bloque del píxel `(x, y)` en pantalla.
fn center_of_the_pixel(v: &View, x: u32, y: u32) -> (i64, i64) {
    let (min, max) = v.pixel_block(x, y).expect("zoom entero: hay bloque");
    ((min.0 + max.0) / 2, (min.1 + max.1) / 2)
}

// ---------------------------------------------------------------------------------------------
// Zoom por las teclas nuevas frente al de antes
// ---------------------------------------------------------------------------------------------

fn press_of(key: KeyId, command: bool) -> Press {
    Press {
        key: Some(key),
        command,
        shift: false,
        alt: false,
        other_ctrl: false,
        repeat: false,
    }
}

fn op_of(action: AppAction) -> ViewOp {
    match action {
        AppAction::ZoomIn => ViewOp::ZoomIn,
        AppAction::ZoomOut => ViewOp::ZoomOut,
        AppAction::ZoomFit => ViewOp::Fit,
        AppAction::ZoomActualSize => ViewOp::ActualSize,
        other => panic!("{other:?} no es una acción de zoom"),
    }
}

/// Modelo de `apply_op` de antes de la unidad, escrito con `set_zoom` y `clamp`.
fn model(v: View, op: ViewOp) -> View {
    let mut m = v;
    match op {
        ViewOp::ZoomIn => m.set_zoom(v.zoom().zoom_in(), CENTER),
        ViewOp::ZoomOut => m.set_zoom(v.zoom().zoom_out(), CENTER),
        ViewOp::ActualSize => m.set_zoom(Zoom::ONE, CENTER),
        ViewOp::Fit => return View::fit(CANVAS, IMAGE),
        other => panic!("{other:?} no es una operación de zoom"),
    }
    with_clamp(m)
}

#[test]
fn row_6_greater_and_smaller_give_zoom_in_and_zoom_out_as_before() {
    let km = Keymap::default_scheme();
    for platform in [Platform::Linux, Platform::Windows, Platform::Mac] {
        for (key_2, expected) in [('>', ViewOp::ZoomIn), ('<', ViewOp::ZoomOut)] {
            let action = km
                .action_for(&press_of(KeyId::Char(key_2), false), platform)
                .unwrap_or_else(|| panic!("`{key_2}` no da acción en {platform:?}"));
            assert_eq!(op_of(action), expected, "`{key_2}` en {platform:?}");
        }
        let fit = km.action_for(&press_of(KeyId::Home, false), platform);
        assert_eq!(fit.map(op_of), Some(ViewOp::Fit), "{platform:?}");
        let actual = km.action_for(&press_of(KeyId::Char('1'), true), platform);
        assert_eq!(actual.map(op_of), Some(ViewOp::ActualSize), "{platform:?}");
    }
}

#[test]
fn row_6_the_zoom_keys_applied_to_the_view_give_the_same_as_the_previous_model() {
    let km = Keymap::default_scheme();
    let keys = [
        (KeyId::Char('>'), false),
        (KeyId::Char('<'), false),
        (KeyId::Home, false),
        (KeyId::Char('1'), true),
    ];
    for (name_2, before) in views() {
        for (key, command) in keys {
            let action = km
                .action_for(&press_of(key, command), Platform::Linux)
                .expect("la tecla da una acción");
            let op = op_of(action);
            let mut after_2 = before;
            after_2.apply_op(op, CANVAS, IMAGE);
            assert_eq!(after_2, model(before, op), "{name_2}: {key:?}");
        }
    }
}

#[test]
fn row_6_greater_zooms_in_one_level_and_smaller_zooms_out_one_level() {
    let km = Keymap::default_scheme();
    for (name_2, before) in views() {
        let applies = |key_2: char| {
            let action = km
                .action_for(&press_of(KeyId::Char(key_2), false), Platform::Linux)
                .expect("acción");
            let mut v = before;
            v.apply_op(op_of(action), CANVAS, IMAGE);
            v
        };
        assert_eq!(
            applies('>').zoom(),
            before.zoom().zoom_in(),
            "{name_2}: `>` acerca"
        );
        assert_eq!(
            applies('<').zoom(),
            before.zoom().zoom_out(),
            "{name_2}: `<` aleja"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// View::center_on (parte pura)
// ---------------------------------------------------------------------------------------------

#[test]
fn view_op_center_on_cursor_is_a_plain_type() {
    fn plane<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
    plane::<ViewOp>();
    assert_ne!(ViewOp::CenterOnCursor, ViewOp::Fit);
}

#[test]
fn row_6_center_on_does_not_change_the_zoom() {
    let points = [CENTER, (0, 0), (699, 499), (-500, -500), (5000, 5000)];
    for (name_2, before) in views() {
        for point in points {
            assert_eq!(
                centered(before, point).zoom(),
                before.zoom(),
                "{name_2}: punto {point:?}"
            );
        }
    }
}

#[test]
fn row_6_center_on_leaves_the_view_with_clamp_at_all_the_points() {
    let points = [
        CENTER,
        (0, 0),
        (699, 499),
        (0, 499),
        (699, 0),
        (-500, -500),
        (5000, 5000),
        (-100_000, 100_000),
        (1_000_000_000, -1_000_000_000),
    ];
    for (name_2, before) in views() {
        for point in points {
            let after_2 = centered(before, point);
            assert_eq!(
                after_2,
                with_clamp(after_2),
                "{name_2}: punto {point:?} no deja la vista con clamp"
            );
        }
    }
}

#[test]
fn row_6_the_pixel_under_the_point_ends_up_under_the_center_of_the_canvas() {
    for (name_2, before) in views() {
        // El punto en el centro del lienzo, si cae sobre la imagen.
        let mut points: Vec<(String, (i64, i64))> = vec![("el centro".to_string(), CENTER)];
        // El centro de cada una de las cuatro esquinas de la imagen y del píxel del medio.
        for (x, y) in [(0, 0), (63, 0), (0, 47), (63, 47), (32, 24)] {
            points.push((
                format!("el píxel ({x}, {y})"),
                center_of_the_pixel(&before, x, y),
            ));
        }
        for (description, point) in points {
            let Some(pixel) = before.screen_to_image(point, IMAGE) else {
                // Sobre un punto fuera de la imagen no hay píxel que mantener (véase abajo).
                continue;
            };
            let after_2 = centered(before, point);
            assert_eq!(
                after_2.screen_to_image(CENTER, IMAGE),
                Some(pixel),
                "{name_2}: {description} ({point:?}) no quedó bajo el centro"
            );
        }
    }
}

#[test]
fn row_6_with_the_point_already_at_the_center_the_pixel_under_the_center_does_not_change() {
    for (name_2, before) in views() {
        if let Some(pixel) = before.screen_to_image(CENTER, IMAGE) {
            assert_eq!(
                centered(before, CENTER).screen_to_image(CENTER, IMAGE),
                Some(pixel),
                "{name_2}"
            );
        }
    }
}

#[test]
fn row_6_with_a_point_over_the_image_no_clamp_is_needed_and_the_centering_is_complete() {
    // Si el punto está sobre la imagen, centrarlo nunca saca la imagen del lienzo: el resultado
    // es el de desplazar la vista lo que falta para llevar el punto al centro, sin recorte
    // (±1 px de redondeo).
    for (name_2, before) in views() {
        let point = center_of_the_pixel(&before, 10, 10);
        if before.screen_to_image(point, IMAGE).is_none() {
            continue;
        }
        let after_2 = centered(before, point);
        let (bx, by) = center_of_the_pixel(&after_2, 10, 10);
        let z = before.zoom().scale(1);
        assert!(
            (bx - CENTER.0).abs() <= z.max(1) && (by - CENTER.1).abs() <= z.max(1),
            "{name_2}: el píxel (10, 10) quedó en ({bx}, {by}) y el centro es {CENTER:?}"
        );
    }
}

#[test]
fn row_6_outside_the_image_nothing_breaks_and_the_zoom_does_not_change() {
    let outside = [
        (-500, -500),
        (-1, -1),
        (699, 499),
        (5000, 5000),
        (-100_000, 7),
        (1_000_000_000, -1_000_000_000),
    ];
    for (name_2, before) in views() {
        for point in outside {
            let after_2 = centered(before, point);
            assert_eq!(after_2.zoom(), before.zoom(), "{name_2}: {point:?}");
            // `clamp` manda: la imagen sigue visible en el lienzo.
            assert_eq!(after_2, with_clamp(after_2), "{name_2}: {point:?}");
            assert!(
                after_2.image_rect(IMAGE).width > 0,
                "{name_2}: {point:?}: la imagen existe"
            );
        }
    }
}

#[test]
fn row_6_center_on_with_an_empty_canvas_or_image_does_not_panic() {
    for (viewport, image_2) in [
        (Size::new(0, 0), IMAGE),
        (CANVAS, Size::new(0, 0)),
        (Size::new(0, 0), Size::new(0, 0)),
        (Size::new(1, 1), Size::new(1, 1)),
    ] {
        let mut v = View::fit(viewport, image_2);
        v.center_on((10, 10), viewport, image_2);
        let mut clamped = v;
        clamped.clamp(viewport, image_2);
        assert_eq!(v, clamped, "{viewport:?} con {image_2:?}");
    }
}

#[test]
fn row_6_apply_op_does_not_know_center_on_cursor_and_leaves_the_view_as_it_is() {
    // La operación necesita el cursor: la resuelve el lienzo con `center_on`. `apply_op` la deja.
    for (name_2, before) in views() {
        let mut after_2 = before;
        after_2.apply_op(ViewOp::CenterOnCursor, CANVAS, IMAGE);
        assert_eq!(after_2, with_clamp(before), "{name_2}");
    }
}

// ---------------------------------------------------------------------------------------------
// Canvas::request_view(CenterOnCursor) en el banco de egui
// ---------------------------------------------------------------------------------------------

/// Pantalla de 800×600 puntos y imagen RGBA de 64×48: a `ppp` 1,0 el ajuste es ×12 y el lienzo es
/// la pantalla entera.
const TEST_SCREEN: Vec2 = vec2(800.0, 600.0);
const CENTER_OF_SCREEN: Pos2 = pos2(400.0, 300.0);
const POS_OF_THE_AREA: Pos2 = pos2(300.0, 200.0);

fn image_2() -> Image {
    Image::new(64, 48, Mode::Rgba).expect("imagen válida")
}

struct Bench {
    ctx: Context,
    canvas: Canvas,
    /// Con un `Area` encima del lienzo (como un desplegable del menú).
    area: Option<Id>,
    button_2: Rect,
}

impl Bench {
    fn new_2(ppp: f32) -> Bench {
        let mut canvas = Canvas::new();
        canvas.set_image(&image_2());
        let mut bench = Bench {
            ctx: context(ppp),
            canvas,
            area: None,
            button_2: Rect::NOTHING,
        };
        bench.rest_2_2();
        bench
    }

    fn with_area(ppp: f32) -> Bench {
        let mut bench = Bench::new_2(ppp);
        bench.area = Some(Id::unique("desplegable"));
        for _ in 0..3 {
            bench.frame(vec![]);
        }
        bench
    }

    fn frame(&mut self, events: Vec<Event>) -> (CanvasInfo, Vec<CanvasInput>) {
        let mut result = None;
        let canvas = &mut self.canvas;
        let area = self.area;
        let mut button_rect = self.button_2;
        frame(&self.ctx, TEST_SCREEN, events, |ui| {
            if let Some(id) = area {
                let response = Area::new(id)
                    .order(Order::Foreground)
                    .fixed_pos(POS_OF_THE_AREA)
                    .show(ui.ctx(), |ui| ui.button("Guardar"));
                button_rect = response.inner.rect;
            }
            let entries = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            let info = canvas.paint(ui);
            result = Some((info, entries));
        });
        self.button_2 = button_rect;
        result.expect("el lienzo se dibuja")
    }

    fn info(&mut self, events: Vec<Event>) -> CanvasInfo {
        self.frame(events).0
    }

    fn rest_2_2(&mut self) -> CanvasInfo {
        self.info(vec![]);
        self.info(vec![])
    }

    /// Mueve el puntero a `pos` y deja un fotograma de reposo.
    fn pointer_in(&mut self, pos: Pos2) -> CanvasInfo {
        self.info(vec![move_pointer(pos)]);
        self.info(vec![])
    }

    /// Pide centrar y deja pasar un fotograma.
    fn centers(&mut self) -> CanvasInfo {
        self.canvas.request_view(ViewOp::CenterOnCursor);
        self.info(vec![])
    }
}

#[test]
fn row_6_request_view_is_applied_in_the_next_interact_and_not_before() {
    let mut bench = Bench::new_2(1.0);
    bench.pointer_in(pos2(100.0, 100.0));
    let before = bench.info(vec![]);
    bench.canvas.request_view(ViewOp::CenterOnCursor);
    assert_eq!(
        bench.canvas.zoom(),
        Some(before.zoom),
        "pedirlo no cambia la vista: lo hace el siguiente interact"
    );
    let after_2 = bench.info(vec![]);
    assert_ne!(
        after_2.image_rect, before.image_rect,
        "se aplica en ese fotograma"
    );
    // Y una sola vez: otro fotograma no la mueve más.
    let other = bench.info(vec![]);
    assert_eq!(other.image_rect, after_2.image_rect);
}

#[test]
fn row_6_with_the_pointer_over_the_canvas_the_pixel_under_the_cursor_goes_to_the_center_and_the_zoom_does_not_change()
 {
    for ppp in PPPS {
        for cursor in [
            pos2(100.0, 100.0),
            pos2(250.0, 450.0),
            pos2(600.0, 150.0),
            pos2(400.0, 300.0),
        ] {
            let mut bench = Bench::new_2(ppp);
            let before = bench.pointer_in(cursor);
            let Some(pixel) = before.hover else {
                // El cursor cae fuera de la imagen a esta escala: nada que comprobar.
                continue;
            };
            let after_2 = bench.centers();
            assert_eq!(
                after_2.zoom, before.zoom,
                "ppp {ppp}, cursor {cursor:?}: mismo zoom"
            );
            // Lo que había bajo el cursor está ahora bajo el centro del lienzo: se comprueba
            // llevando el puntero allí.
            let in_the_center = bench.pointer_in(CENTER_OF_SCREEN);
            assert_eq!(
                in_the_center.hover,
                Some(pixel),
                "ppp {ppp}, cursor {cursor:?}: el píxel {pixel:?} no quedó en el centro"
            );
            assert_eq!(in_the_center.zoom, before.zoom, "ppp {ppp}");
        }
    }
}

#[test]
fn row_6_with_the_pointer_already_at_the_center_the_pixel_under_the_center_does_not_change() {
    let mut bench = Bench::new_2(1.0);
    let before = bench.pointer_in(CENTER_OF_SCREEN);
    let pixel = before.hover.expect("el centro está sobre la imagen");
    let after_2 = bench.centers();
    assert_eq!(after_2.zoom, before.zoom);
    assert_eq!(after_2.hover, Some(pixel));
}

#[test]
fn row_6_without_a_pointer_over_the_canvas_the_view_does_not_change() {
    for ppp in PPPS {
        // Nunca hubo puntero.
        let mut bench = Bench::new_2(ppp);
        let before = bench.rest_2_2();
        let after_2 = bench.centers();
        assert_eq!(
            after_2.image_rect, before.image_rect,
            "ppp {ppp}: sin puntero"
        );
        assert_eq!(after_2.zoom, before.zoom, "ppp {ppp}");

        // El puntero fuera del lienzo (a la derecha de la pantalla).
        let mut bench = Bench::new_2(ppp);
        bench.pointer_in(pos2(900.0, 300.0));
        let before = bench.rest_2_2();
        let after_2 = bench.centers();
        assert_eq!(
            after_2.image_rect, before.image_rect,
            "ppp {ppp}: puntero fuera del lienzo"
        );
        assert_eq!(after_2.zoom, before.zoom, "ppp {ppp}");
    }
}

#[test]
fn row_6_without_an_image_asking_to_center_breaks_nothing() {
    let mut bench = Bench {
        ctx: context(1.0),
        canvas: Canvas::new(),
        area: None,
        button_2: Rect::NOTHING,
    };
    bench.pointer_in(pos2(100.0, 100.0));
    bench.canvas.request_view(ViewOp::CenterOnCursor);
    bench.info(vec![]);
    assert_eq!(bench.canvas.zoom(), None);
}

#[test]
fn row_6_with_another_layer_above_the_cursor_the_view_does_not_change() {
    let mut bench = Bench::with_area(1.0);
    assert!(bench.button_2.is_positive(), "el área se pintó");
    let over_the_area = bench.button_2.center();
    bench.pointer_in(over_the_area);
    let before = bench.rest_2_2();
    let after_2 = bench.centers();
    assert_eq!(
        after_2.image_rect, before.image_rect,
        "con el cursor sobre un desplegable no se centra"
    );
    // Con el cursor fuera del área sí se centra (el control de la prueba).
    bench.pointer_in(pos2(100.0, 100.0));
    let before = bench.rest_2_2();
    let after_2 = bench.centers();
    assert_ne!(after_2.image_rect, before.image_rect);
}

#[test]
fn row_6_with_a_half_made_stroke_n_moves_the_view_and_the_stroke_continues() {
    let cursor = pos2(100.0, 100.0);
    let mut bench = Bench::new_2(1.0);
    bench.pointer_in(cursor);
    let before = bench.rest_2_2();

    // Empieza el trazo: pulsa sin soltar.
    let (_, start_2_2) = bench.frame(vec![button_2(cursor, true)]);
    let source_2 = match start_2_2.as_slice() {
        [CanvasInput::Stroke(CanvasEvent::StrokeBegin(p))] => *p,
        other => panic!("se esperaba un StrokeBegin y salió {other:?}"),
    };

    // Centra a mitad del trazo.
    bench.canvas.request_view(ViewOp::CenterOnCursor);
    let (info, entries) = bench.frame(vec![]);
    assert!(
        !entries
            .iter()
            .any(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeEnd))),
        "centrar no corta el trazo: {entries:?}"
    );
    assert_ne!(
        info.image_rect, before.image_rect,
        "la vista se movió con el trazo a medias"
    );
    assert_eq!(info.zoom, before.zoom);

    // El trazo sigue: el siguiente movimiento usa la vista nueva (otro píxel bajo el mismo punto).
    let (_, movement) = bench.frame(vec![move_pointer(pos2(101.0, 100.0))]);
    let target_2 = movement
        .iter()
        .find_map(|e| match e {
            CanvasInput::Stroke(CanvasEvent::StrokeMove(p)) => Some(*p),
            _ => None,
        })
        .expect("el trazo sigue y da un StrokeMove");
    assert_ne!(
        target_2, source_2,
        "el movimiento usa la vista nueva: {source_2:?} → {target_2:?}"
    );

    // Y termina como siempre, al soltar.
    let (_, release_2) = bench.frame(vec![button_2(pos2(101.0, 100.0), false)]);
    assert_eq!(
        release_2.last(),
        Some(&CanvasInput::Stroke(CanvasEvent::StrokeEnd))
    );
}

#[test]
fn row_6_centering_with_a_half_made_stroke_does_not_change_the_pixel_drawn_under_the_center_cursor()
{
    // Tras centrar, llevar el cursor al centro da el mismo píxel que había bajo el cursor antes.
    let cursor = pos2(100.0, 100.0);
    let mut bench = Bench::new_2(1.0);
    let before = bench.pointer_in(cursor);
    let pixel = before.hover.expect("sobre la imagen");
    bench.frame(vec![button_2(cursor, true)]);
    bench.canvas.request_view(ViewOp::CenterOnCursor);
    bench.frame(vec![]);
    let (info, _) = bench.frame(vec![move_pointer(CENTER_OF_SCREEN)]);
    assert_eq!(info.hover, Some(pixel));
    bench.frame(vec![button_2(CENTER_OF_SCREEN, false)]);
}

#[test]
fn row_6_the_other_zoom_operations_stay_the_same_with_the_new_one_in_the_queue() {
    // `CenterOnCursor` sin puntero no estropea las operaciones pedidas en el mismo fotograma.
    let mut bench = Bench::new_2(1.0);
    let fit = bench.rest_2_2().zoom;
    bench.canvas.request_view(ViewOp::CenterOnCursor);
    bench.canvas.request_view(ViewOp::ZoomIn);
    let after_2 = bench.info(vec![]);
    assert_eq!(after_2.zoom, fit.zoom_in());
}
