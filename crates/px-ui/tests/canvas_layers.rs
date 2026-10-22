//! Tests de la guarda de capa de `Canvas` y del zoom a mitad de trazo (specs/hito-2-menu-herramientas.md,
//! «Casos» fila 9b y la parte de banco de la 5b, y decisión 6).
//!
//! Banco de egui sin ventana (`Context::run_ui` con `RawInput` sintético), a las cinco escalas.
//!
//! - **Fila 9b.** El lienzo con, encima, un `egui::Area` en `Order::Foreground` (y otra prueba con
//!   `Order::Middle`) con un botón que cae sobre él, como un desplegable del menú. Con el área
//!   abierta: el clic **no** produce `StrokeBegin` ni `StrokeMove` (y llega al botón), la rueda y
//!   el pellizco **no cambian el zoom** y `hover` es `None` sobre el área; un trazo empezado
//!   **antes** de abrir el área sigue y termina con `StrokeEnd` aunque el puntero pase por el área.
//!   Con el área cerrada todo funciona como antes (clic dibuja, rueda hace zoom, `hover` válido).
//!   Además, la misma prueba con el `menu_button` real de `show_menu` abierto sobre el lienzo.
//! - **Fila 5b (banco).** Un trazo a medias con `Canvas::request_view(ZoomIn)` y `ToolState`: el
//!   zoom cambia, **el trazo sigue** (el siguiente `StrokeMove` usa la vista nueva), el documento
//!   no cambia hasta soltar y el `StrokeEnd` crea **un solo paso**. La rueda no cambia.
//!
//! El área se muestra **antes** del lienzo en el fotograma (como el menú en la app) y, como egui
//! decide qué capa está bajo el puntero con las áreas del fotograma anterior, el banco deja pasar
//! dos fotogramas con el área abierta antes de pulsar.

use egui::{Area, Context, Event, Id, Order, Pos2, Rect, Vec2, pos2, vec2};
use px_core::ImageId;
use px_core::{Document, Image, Mode, Pixel, Rgba};
use px_ui::actions::{AppAction, Locks, Platform, UiState};
use px_ui::canvas::{Canvas, CanvasEvent, CanvasInfo, CanvasInput};
use px_ui::menu::show_menu;
use px_ui::tool::{Tool, ToolState};
use px_ui::view::{ViewOp, Zoom};

mod common;
use common::{PPPS, button_2, context, frame, in_points, move_pointer, n_of, wheel_line};

const TEST_SCREEN: Vec2 = vec2(800.0, 600.0);
const POS_OF_THE_AREA: Pos2 = pos2(300.0, 200.0);
const COMMANDS: [Order; 2] = [Order::Foreground, Order::Middle];

fn image_2() -> Image {
    Image::new(64, 48, Mode::Rgba).expect("imagen válida")
}

struct Step {
    entries: Vec<CanvasInput>,
    info: CanvasInfo,
    /// El botón del área recibió un clic en este fotograma.
    click_on_the_button: bool,
}

impl Step {
    fn there_is_stroke(&self) -> bool {
        self.entries
            .iter()
            .any(|e| matches!(e, CanvasInput::Stroke(_)))
    }

    fn starts(&self) -> bool {
        self.entries
            .iter()
            .any(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeBegin(_))))
    }

    fn ends(&self) -> bool {
        self.entries
            .iter()
            .any(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeEnd)))
    }

    fn movements(&self) -> Vec<(i32, i32)> {
        self.entries
            .iter()
            .filter_map(|e| match e {
                CanvasInput::Stroke(CanvasEvent::StrokeMove(p)) => Some(*p),
                _ => None,
            })
            .collect()
    }
}

struct Bench {
    ctx: Context,
    canvas: Canvas,
    id_of_the_area: Id,
    order: Order,
    open_2: bool,
    /// Rectángulo del botón del área en el último fotograma (puntos).
    button_2: Rect,
}

impl Bench {
    fn new_2(ppp: f32, order: Order, open_2: bool) -> Bench {
        let mut canvas = Canvas::new();
        canvas.set_image(&image_2());
        let mut bench = Bench {
            ctx: context(ppp),
            canvas,
            id_of_the_area: Id::unique("desplegable"),
            order,
            open_2,
            button_2: Rect::NOTHING,
        };
        // Reparte el espacio, sube la textura y deja el área en su sitio.
        for _ in 0..3 {
            bench.step(vec![]);
        }
        bench
    }

    fn step(&mut self, events: Vec<Event>) -> Step {
        let mut result = None;
        let mut button_rect = self.button_2;
        let (order, open_2, id) = (self.order, self.open_2, self.id_of_the_area);
        let canvas = &mut self.canvas;
        frame(&self.ctx, TEST_SCREEN, events, |ui| {
            let mut click = false;
            if open_2 {
                let response = Area::new(id)
                    .order(order)
                    .fixed_pos(POS_OF_THE_AREA)
                    .show(ui.ctx(), |ui| ui.button("Guardar"));
                click = response.inner.clicked();
                button_rect = response.inner.rect;
            }
            let entries = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            let info = canvas.paint(ui);
            result = Some(Step {
                entries,
                info,
                click_on_the_button: click,
            });
        });
        self.button_2 = button_rect;
        result.expect("el lienzo se dibuja")
    }

    fn zoom(&mut self) -> Zoom {
        self.step(vec![]).info.zoom
    }

    /// Centro del botón del área (un punto sobre el área).
    fn over_the_area(&self) -> Pos2 {
        assert!(self.button_2.is_positive(), "el área no se ha pintado");
        self.button_2.center()
    }

    /// Un punto del lienzo (sobre la imagen) que no toca el área.
    fn outside_of_the_area(&self) -> Pos2 {
        pos2(100.0, 100.0)
    }

    /// Mueve, pulsa y suelta en `pos`; devuelve los tres pasos.
    fn click(&mut self, pos: Pos2) -> [Step; 3] {
        let a = self.step(vec![move_pointer(pos)]);
        let b = self.step(vec![button_2(pos, true)]);
        let c = self.step(vec![button_2(pos, false)]);
        [a, b, c]
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9b: con el área abierta
// ---------------------------------------------------------------------------------------------

#[test]
fn row_9b_the_area_is_painted_over_the_canvas_and_covers_part_of_the_image() {
    for ppp in PPPS {
        for order in COMMANDS {
            let bench = Bench::new_2(ppp, order, true);
            let rect = bench.button_2;
            assert!(rect.is_positive(), "ppp {ppp} {order:?}");
            let screen = Rect::from_min_size(Pos2::ZERO, TEST_SCREEN);
            assert!(screen.contains_rect(rect), "ppp {ppp} {order:?}: {rect:?}");
            // Cae sobre la imagen: el lienzo ocupa toda la pantalla.
            let image_2 = bench.canvas.zoom().is_some();
            assert!(image_2);
        }
    }
}

#[test]
fn row_9b_with_the_area_open_the_click_does_not_start_a_stroke_and_reaches_the_button() {
    for ppp in PPPS {
        for order in COMMANDS {
            let mut bench = Bench::new_2(ppp, order, true);
            let pos = bench.over_the_area();
            let steps_2 = bench.click(pos);
            for (i, step) in steps_2.iter().enumerate() {
                assert!(
                    !step.there_is_stroke(),
                    "ppp {ppp} {order:?}: el paso {i} produjo {:?}",
                    step.entries
                );
                assert_eq!(step.info.hover, None, "ppp {ppp} {order:?}: paso {i}");
            }
            assert!(
                steps_2.iter().any(|p| p.click_on_the_button),
                "ppp {ppp} {order:?}: el clic no llegó al botón"
            );
        }
    }
}

#[test]
fn row_9b_with_the_area_open_hover_is_none_over_it_and_valid_outside() {
    for ppp in PPPS {
        for order in COMMANDS {
            let mut bench = Bench::new_2(ppp, order, true);
            let over = bench.over_the_area();
            let outside = bench.outside_of_the_area();
            assert_eq!(bench.step(vec![move_pointer(over)]).info.hover, None);
            assert_eq!(bench.step(vec![]).info.hover, None);
            let step = bench.step(vec![move_pointer(outside)]);
            assert!(
                step.info.hover.is_some(),
                "ppp {ppp} {order:?}: fuera del área hay hover"
            );
            // Y vuelve a ser `None` al regresar al área.
            assert_eq!(bench.step(vec![move_pointer(over)]).info.hover, None);
        }
    }
}

#[test]
fn row_9b_with_the_area_open_the_wheel_does_not_change_the_zoom() {
    for ppp in PPPS {
        for order in COMMANDS {
            let mut bench = Bench::new_2(ppp, order, true);
            let before = bench.zoom();
            let over = bench.over_the_area();
            bench.step(vec![move_pointer(over)]);
            bench.step(vec![]);
            for top in [true, false] {
                let step = bench.step(vec![wheel_line(top), wheel_line(top)]);
                assert_eq!(
                    step.info.zoom, before,
                    "ppp {ppp} {order:?}: rueda arriba={top}"
                );
            }
            assert_eq!(bench.zoom(), before);
            // Fuera del área la misma rueda sí hace zoom.
            let outside = bench.outside_of_the_area();
            bench.step(vec![move_pointer(outside)]);
            let step = bench.step(vec![wheel_line(true)]);
            assert_eq!(
                step.info.zoom,
                before.zoom_in(),
                "ppp {ppp} {order:?}: fuera del área"
            );
        }
    }
}

#[test]
fn row_9b_with_the_area_open_the_pinch_does_not_change_the_zoom() {
    for ppp in PPPS {
        for order in COMMANDS {
            let mut bench = Bench::new_2(ppp, order, true);
            let before = bench.zoom();
            let over = bench.over_the_area();
            bench.step(vec![move_pointer(over)]);
            bench.step(vec![]);
            for factor in [1.3, 0.7] {
                let step = bench.step(vec![Event::Zoom(factor)]);
                assert_eq!(
                    step.info.zoom, before,
                    "ppp {ppp} {order:?}: pellizco {factor}"
                );
            }
            let outside = bench.outside_of_the_area();
            bench.step(vec![move_pointer(outside)]);
            let step = bench.step(vec![Event::Zoom(1.3)]);
            assert_eq!(
                step.info.zoom,
                before.zoom_in(),
                "ppp {ppp} {order:?}: fuera del área"
            );
        }
    }
}

#[test]
fn row_9b_a_stroke_started_before_opening_the_area_continues_and_ends_with_stroke_end() {
    for ppp in PPPS {
        for order in COMMANDS {
            let mut bench = Bench::new_2(ppp, order, false);
            let start_2 = bench.outside_of_the_area();
            bench.step(vec![move_pointer(start_2)]);
            let start_2_2 = bench.step(vec![button_2(start_2, true)]);
            assert!(
                start_2_2.starts(),
                "ppp {ppp} {order:?}: con el área cerrada el clic dibuja"
            );

            // Se abre el área a mitad del gesto y se deja asentar (el puntero no se mueve).
            bench.open_2 = true;
            bench.step(vec![]);
            bench.step(vec![]);
            let over = bench.over_the_area();

            // El puntero pasa por encima del área: el trazo sigue.
            let step = bench.step(vec![move_pointer(over)]);
            assert!(
                !step.movements().is_empty(),
                "ppp {ppp} {order:?}: el trazo debe seguir sobre el área: {:?}",
                step.entries
            );
            assert!(!step.starts() && !step.ends());
            // Soltar sobre el área termina el trazo (y no es un clic del botón).
            let release_2 = bench.step(vec![button_2(over, false)]);
            assert!(
                release_2.ends(),
                "ppp {ppp} {order:?}: {:?}",
                release_2.entries
            );
            assert_eq!(
                release_2
                    .entries
                    .iter()
                    .filter(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeEnd)))
                    .count(),
                1
            );
            assert!(
                !release_2.click_on_the_button,
                "ppp {ppp} {order:?}: no es un clic del botón"
            );
            // Y el gesto terminó: un movimiento más no dibuja.
            let after_2 = bench.step(vec![move_pointer(start_2)]);
            assert!(
                !after_2.there_is_stroke(),
                "ppp {ppp} {order:?}: {:?}",
                after_2.entries
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9b: con el área cerrada todo funciona como antes
// ---------------------------------------------------------------------------------------------

#[test]
fn row_9b_with_the_area_closed_the_click_draws_the_wheel_zooms_and_hover_is_valid() {
    for ppp in PPPS {
        // El área está cerrada: el mismo recorrido que en las pruebas de arriba.
        let mut bench = Bench::new_2(ppp, Order::Foreground, false);
        // El punto donde estaría el botón.
        let pos = pos2(POS_OF_THE_AREA.x + 10.0, POS_OF_THE_AREA.y + 8.0);

        let a = bench.step(vec![move_pointer(pos)]);
        assert!(a.info.hover.is_some(), "ppp {ppp}: hover válido");
        let b = bench.step(vec![button_2(pos, true)]);
        assert!(b.starts(), "ppp {ppp}: el clic dibuja: {:?}", b.entries);
        let c = bench.step(vec![button_2(pos, false)]);
        assert!(c.ends(), "ppp {ppp}");

        let before = bench.zoom();
        let step = bench.step(vec![wheel_line(true)]);
        assert_eq!(
            step.info.zoom,
            before.zoom_in(),
            "ppp {ppp}: la rueda hace zoom"
        );
        let before = bench.zoom();
        let step = bench.step(vec![Event::Zoom(1.3)]);
        assert_eq!(
            step.info.zoom,
            before.zoom_in(),
            "ppp {ppp}: el pellizco hace zoom"
        );
    }
}

#[test]
fn row_9b_when_closing_the_area_the_canvas_responds_again() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, Order::Foreground, true);
        let over = bench.over_the_area();
        let [_, b, _] = bench.click(over);
        assert!(!b.starts());
        bench.open_2 = false;
        bench.step(vec![]);
        bench.step(vec![]);
        let [a, b, c] = bench.click(over);
        assert!(a.info.hover.is_some(), "ppp {ppp}");
        assert!(b.starts(), "ppp {ppp}: {:?}", b.entries);
        assert!(c.ends(), "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9b: con el menu_button real de show_menu
// ---------------------------------------------------------------------------------------------

/// Banco con `show_menu` arriba y el lienzo debajo: el desplegable de `Archivo` cae sobre el lienzo.
struct BenchWithMenu {
    ctx: Context,
    canvas: Canvas,
    ppp: f32,
    state_2: UiState,
}

struct StepWithMenu {
    entries: Vec<CanvasInput>,
    info: CanvasInfo,
    action_2: Option<AppAction>,
    texts: common::Painted,
}

impl BenchWithMenu {
    fn new_2(ppp: f32) -> BenchWithMenu {
        let mut canvas = Canvas::new();
        canvas.set_image(&image_2());
        let mut bench = BenchWithMenu {
            ctx: context(ppp),
            canvas,
            ppp,
            state_2: UiState {
                can_undo: true,
                can_redo: true,
                zoom: Zoom::from_int(12),
                tool: Tool::Pencil,
                locks: Locks::default(),
            },
        };
        for _ in 0..3 {
            bench.step(vec![]);
        }
        bench
    }

    fn step(&mut self, events: Vec<Event>) -> StepWithMenu {
        let mut result = None;
        let canvas = &mut self.canvas;
        let state_2 = &self.state_2;
        let shapes_2 = frame(&self.ctx, TEST_SCREEN, events, |ui| {
            let action_2 = show_menu(ui, state_2, Platform::Linux);
            let entries = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            let info = canvas.paint(ui);
            result = Some((action_2, entries, info));
        });
        let (action_2, entries, info) = result.expect("se dibuja");
        StepWithMenu {
            entries,
            info,
            action_2,
            texts: common::painted(&shapes_2),
        }
    }

    /// Mueve, pulsa y suelta en `pos` y devuelve los tres pasos.
    fn click(&mut self, pos: Pos2) -> [StepWithMenu; 3] {
        let a = self.step(vec![move_pointer(pos)]);
        let b = self.step(vec![button_2(pos, true)]);
        let c = self.step(vec![button_2(pos, false)]);
        [a, b, c]
    }

    /// Abre `Archivo` con un clic en su título (a 4 u del borde, con 61 u de ancho).
    fn opens_file(&mut self) {
        let n = n_of(self.ppp);
        let title = in_points(self.ppp, (4 + 61 / 2) * n, 8 * n);
        self.click(title);
        self.step(vec![]);
        self.step(vec![]);
    }

    /// Centro de la entrada `Guardar` del desplegable abierto (no `Guardar as_word…`).
    fn center_of_save(&mut self) -> Pos2 {
        let step = self.step(vec![]);
        let text_2 = step
            .texts
            .texts
            .iter()
            .find(|t| t.text_2.starts_with("Guardar") && !t.text_2.starts_with("Guardar como"))
            .unwrap_or_else(|| {
                panic!(
                    "el desplegable de Archivo no se ve; hay {:?}",
                    step.texts.all_the_texts()
                )
            });
        text_2.rect().center()
    }
}

#[test]
fn row_9b_with_the_real_menu_open_a_click_on_save_does_not_draw_and_gives_save() {
    for ppp in PPPS {
        let mut bench = BenchWithMenu::new_2(ppp);
        bench.opens_file();
        let save = bench.center_of_save();
        // La entrada cae sobre el lienzo (el menú no ocupa más que su barra).
        let n = n_of(ppp);
        assert!(save.y * ppp > (16 * n) as f32, "ppp {ppp}: {save:?}");

        let steps_2 = bench.click(save);
        for (i, step) in steps_2.iter().enumerate() {
            assert!(
                !step
                    .entries
                    .iter()
                    .any(|e| matches!(e, CanvasInput::Stroke(_))),
                "ppp {ppp}: el paso {i} dibujó: {:?}",
                step.entries
            );
            assert_eq!(step.info.hover, None, "ppp {ppp}: paso {i}");
        }
        assert!(
            steps_2.iter().any(|p| p.action_2 == Some(AppAction::Save)),
            "ppp {ppp}: el clic debe dar Save; dio {:?}",
            steps_2.iter().map(|p| p.action_2).collect::<Vec<_>>()
        );
    }
}

#[test]
fn row_9b_with_the_real_menu_open_the_wheel_and_the_pinch_do_not_change_the_zoom() {
    for ppp in PPPS {
        let mut bench = BenchWithMenu::new_2(ppp);
        let before = bench.step(vec![]).info.zoom;
        bench.opens_file();
        let save = bench.center_of_save();
        bench.step(vec![move_pointer(save)]);
        bench.step(vec![]);
        assert_eq!(bench.step(vec![]).info.hover, None, "ppp {ppp}");
        let step = bench.step(vec![wheel_line(true)]);
        assert_eq!(
            step.info.zoom, before,
            "ppp {ppp}: rueda sobre el desplegable"
        );
        let step = bench.step(vec![Event::Zoom(1.3)]);
        assert_eq!(
            step.info.zoom, before,
            "ppp {ppp}: pellizco sobre el desplegable"
        );
    }
}

#[test]
fn row_9b_with_the_menu_closed_the_same_point_draws() {
    for ppp in PPPS {
        let mut bench = BenchWithMenu::new_2(ppp);
        bench.opens_file();
        let save = bench.center_of_save();
        // Elegir `Guardar` cierra el desplegable.
        bench.click(save);
        bench.step(vec![]);
        bench.step(vec![]);
        let [a, b, c] = bench.click(save);
        assert!(
            a.info.hover.is_some(),
            "ppp {ppp}: hover válido con el menú cerrado"
        );
        assert!(
            b.entries
                .iter()
                .any(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeBegin(_)))),
            "ppp {ppp}: con el menú cerrado el clic dibuja: {:?}",
            b.entries
        );
        assert!(
            c.entries
                .iter()
                .any(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeEnd))),
            "ppp {ppp}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5b (banco): el zoom a mitad de trazo
// ---------------------------------------------------------------------------------------------

fn red() -> Option<Pixel> {
    Some(Pixel::Rgba(Rgba::opaque(255, 0, 0)))
}

/// El píxel de imagen (con signo) bajo `pos` según el rectángulo de la imagen del fotograma.
fn pixel_under(info: &CanvasInfo, ppp: f32, pos: Pos2) -> (i32, i32) {
    let rect = info.image_rect.expect("hay imagen");
    let physical = |v: f32| (v * ppp).round() as i64;
    let x = info.zoom.unscale(physical(pos.x - rect.min.x));
    let y = info.zoom.unscale(physical(pos.y - rect.min.y));
    (x as i32, y as i32)
}

struct Session {
    bench: Bench,
    doc: Document,
    tools: ToolState,
}

impl Session {
    fn new(ppp: f32) -> Session {
        let bench = Bench::new_2(ppp, Order::Foreground, false);
        Session {
            bench,
            doc: Document::new(image_2()),
            tools: ToolState::new(),
        }
    }

    /// Un fotograma cuyos eventos de trazo pasan por `ToolState` (como `main`).
    fn step(&mut self, events: Vec<Event>) -> Step {
        let step = self.bench.step(events);
        let id = ImageId::from_raw(0);
        for input in &step.entries {
            self.tools
                .canvas_input_with(&mut self.doc, id, red(), *input)
                .expect("el evento es válido");
        }
        step
    }
}

#[test]
fn row_5b_a_half_made_stroke_with_request_view_zoom_in_changes_the_zoom_and_the_stroke_continues() {
    for ppp in PPPS {
        let mut s = Session::new(ppp);
        let state_initial = s.doc.state_id();
        let p0 = pos2(400.0, 300.0);
        let p1 = pos2(430.0, 300.0);
        let p2 = pos2(460.0, 320.0);

        s.step(vec![move_pointer(p0)]);
        let start_2_2 = s.step(vec![button_2(p0, true)]);
        assert!(start_2_2.starts(), "ppp {ppp}");
        assert!(s.tools.busy());
        s.step(vec![move_pointer(p1)]);
        assert!(s.tools.busy());

        let zoom_before = s.bench.zoom();
        s.bench.canvas.request_view(ViewOp::ZoomIn);
        let after = s.step(vec![]);
        assert_eq!(
            after.info.zoom,
            zoom_before.zoom_in(),
            "ppp {ppp}: el zoom cambia a mitad de trazo"
        );
        assert!(s.tools.busy(), "ppp {ppp}: el trazo sigue");
        assert!(
            !after.ends(),
            "ppp {ppp}: pedir la vista no termina el trazo"
        );
        assert_eq!(
            s.doc.state_id(),
            state_initial,
            "ppp {ppp}: el documento no cambia hasta soltar"
        );
        assert!(!s.doc.can_undo());

        // El siguiente StrokeMove usa la vista nueva.
        let step = s.step(vec![move_pointer(p2)]);
        let expected_2 = pixel_under(&step.info, ppp, p2);
        assert_eq!(step.movements(), vec![expected_2], "ppp {ppp}: vista nueva");
        assert!(s.tools.busy());
        assert_eq!(s.doc.state_id(), state_initial);

        // Soltar crea un solo paso.
        let release_2 = s.step(vec![button_2(p2, false)]);
        assert!(release_2.ends(), "ppp {ppp}");
        assert!(!s.tools.busy());
        assert_ne!(
            s.doc.state_id(),
            state_initial,
            "ppp {ppp}: el trazo se aplicó"
        );
        assert!(s.doc.can_undo());
        s.doc.undo().expect("un paso que deshacer");
        assert!(
            !s.doc.can_undo(),
            "ppp {ppp}: el trazo entero fue un solo paso"
        );
        assert_eq!(s.doc.state_id(), state_initial);
    }
}

#[test]
fn row_5b_several_view_requests_in_the_middle_of_a_stroke_are_still_a_single_step() {
    let mut s = Session::new(1.0);
    let p0 = pos2(400.0, 300.0);
    s.step(vec![move_pointer(p0)]);
    s.step(vec![button_2(p0, true)]);
    for (i, op) in [
        ViewOp::ZoomIn,
        ViewOp::ZoomOut,
        ViewOp::ActualSize,
        ViewOp::Fit,
    ]
    .into_iter()
    .enumerate()
    {
        s.bench.canvas.request_view(op);
        s.step(vec![]);
        assert!(s.tools.busy(), "tras {op:?}");
        s.step(vec![move_pointer(pos2(400.0 + 10.0 * i as f32, 310.0))]);
    }
    s.step(vec![button_2(p0, false)]);
    assert!(!s.tools.busy());
    assert!(s.doc.can_undo());
    s.doc.undo().expect("un paso");
    assert!(
        !s.doc.can_undo(),
        "un solo paso aunque el zoom cambió cuatro veces"
    );
}

#[test]
fn row_5b_the_wheel_in_the_middle_of_a_stroke_keeps_changing_the_zoom_as_before() {
    for ppp in PPPS {
        let mut s = Session::new(ppp);
        let p0 = pos2(400.0, 300.0);
        s.step(vec![move_pointer(p0)]);
        s.step(vec![button_2(p0, true)]);
        let before = s.bench.zoom();
        let step = s.step(vec![wheel_line(true)]);
        assert_eq!(
            step.info.zoom,
            before.zoom_in(),
            "ppp {ppp}: la rueda no cambia"
        );
        assert!(s.tools.busy(), "ppp {ppp}");
        s.step(vec![button_2(p0, false)]);
        assert!(!s.tools.busy());
        assert!(s.doc.can_undo());
    }
}
