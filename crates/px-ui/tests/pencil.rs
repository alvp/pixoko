//! Tests del lápiz en el lienzo de egui sin ventana (specs/hito-2-lapiz-deshacer.md, secciones
//! 2, 3 y 6; filas 14 a 16 de «Casos límite»): el mismo banco que `tests/canvas.rs`, un
//! `egui::Context` con `RawInput` sintético, pero llamando a `Canvas::interact` y luego a
//! `Canvas::paint` como hará la app.
//!
//! Los eventos de trazo salen de `interact`. Cada cambio de estado del botón va en su propio
//! fotograma (egui necesita un fotograma por pulsación o soltado). Las posiciones se calculan con
//! el rectángulo de la imagen del primer fotograma, en el centro de cada píxel: 800×600 con
//! 64×48 da zoom ×12 y las posiciones son enteras.
//!
//! Además de las filas, aquí se prueba que `show` sigue pintando igual, que la superposición del
//! trazo (`set_overlay`, tras `interact` y antes de `paint`) se ve en la **textura** y no añade
//! ninguna forma, y que `refresh` conserva la vista y deja la textura al día. Desde la unidad 14a-2
//! el banco aplica en orden los `textures_delta` de cada fotograma con
//! `common::textures::Textures` (la textura compuesta) y el fotograma es el de la app: `interact`,
//! `set_overlay`, `sync` y `paint`.
//!
//! El soltar perdido de la fila 16 no se puede fabricar solo con `RawInput` (egui solo baja el
//! botón con un evento de soltar): se baja con `InputState::pointer` dentro del fotograma, antes
//! de `interact`, que es exactamente lo que ve el lienzo cuando egui «ya no ve el botón».

use px_core::ImageId;
use std::collections::HashSet;

use egui::epaint::Primitive;
use egui::{
    Color32, Context, Event, FullOutput, ImageData, Key, Modifiers, MouseWheelUnit, PointerButton,
    PointerState, Pos2, RawInput, Rect, TextEdit, Ui, Vec2, pos2, vec2,
};
use px_core::{Document, Image, Mode, Pixel, Rgba};
use px_ui::canvas::{Canvas, CanvasEvent, CanvasInfo, CanvasInput};
use px_ui::color::Role;
use px_ui::stroke::Stroke;
use px_ui::texture::to_rgba8;
use px_ui::view::Zoom;

mod common;
use common::textures::{Texture, Textures};

const SCREEN: Vec2 = vec2(800.0, 600.0);
const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;

fn red() -> Pixel {
    Pixel::Rgba(Rgba::opaque(255, 0, 0))
}

/// Imagen RGBA de 64×48 con un píxel de cada color en las esquinas.
fn image_of_test() -> Image {
    let mut image = Image::new(WIDTH, HEIGHT, Mode::Rgba).expect("imagen válida");
    image
        .set_pixel(0, 0, Pixel::Rgba(Rgba::opaque(255, 0, 0)))
        .expect("dentro");
    image
        .set_pixel(63, 47, Pixel::Rgba(Rgba::opaque(0, 0, 255)))
        .expect("dentro");
    image
}

struct Bench {
    ctx: Context,
    canvas: Canvas,
    /// El documento que el lienzo muestra (la imagen con la que se llama a `set_overlay` y
    /// `sync`). Las pruebas que cambian la imagen la actualizan aquí.
    image_2: Image,
    /// Contenido de cada textura, compuesto en orden con los `textures_delta` de cada fotograma.
    textures_2: Textures,
}

struct TestFrame {
    output: FullOutput,
    events_2: Vec<CanvasEvent>,
    info: CanvasInfo,
}

impl Bench {
    fn new_2() -> Bench {
        Bench {
            ctx: Context::default(),
            canvas: Canvas::new(),
            image_2: image_of_test(),
            textures_2: Textures::new(),
        }
    }

    /// La textura compuesta de la baldosa 0 (la única de estas imágenes).
    fn texture_2(&self) -> &Texture {
        let id = self.canvas.texture_id().expect("la textura está subida");
        self.textures_2.get(id).expect("se ha subido su contenido")
    }

    /// Ejecuta un fotograma como el de la app: `before` (si lo hay) corre dentro del fotograma,
    /// antes de `interact`; luego `interact`, `set_overlay` con la superposición dada (el trazo
    /// pendiente entero; vacía la quita), `sync` y `paint`. Aplica los `textures_delta` a
    /// `textures` y no los limpia (quien llama los mira y los limpia).
    fn frame(
        &mut self,
        events: Vec<Event>,
        overlay: Vec<(u32, u32, Rgba)>,
        before: Option<fn(&mut Ui)>,
    ) -> TestFrame {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events,
            ..RawInput::default()
        };
        let mut result = None;
        let canvas = &mut self.canvas;
        let image_2 = &self.image_2;
        let output = self.ctx.run_ui(input, |ui| {
            if let Some(before) = before {
                before(ui);
            }
            let events_2: Vec<CanvasEvent> = canvas
                .interact_roles(ui)
                .into_iter()
                .filter_map(|(input, _)| match input {
                    CanvasInput::Stroke(event) => Some(event),
                    CanvasInput::Escape => None,
                })
                .collect();
            canvas.set_overlay(ui.ctx(), image_2, &overlay);
            canvas.sync(ui.ctx(), image_2).expect("sync");
            let info = canvas.paint(ui);
            result = Some((events_2, info));
        });
        self.textures_2.applies(&output.textures_delta);
        let (events_2, info) = result.expect("el lienzo se dibuja");
        TestFrame {
            output,
            events_2,
            info,
        }
    }

    /// Un fotograma sin superposición y con los cambios de textura ya aplicados y limpiados (egui
    /// entra en pánico si se sueltan sin aplicar).
    fn step(&mut self, events: Vec<Event>) -> TestFrame {
        let mut f = self.frame(events, vec![], None);
        f.output.textures_delta.clear();
        f
    }

    /// Un fotograma como el de la app (`interact_roles`, `sync` y `paint`), con la entrada
    /// ordenada: los eventos de trazo **y** `Escape`, cada uno con su rol.
    fn step_roles(&mut self, events: Vec<Event>) -> (Vec<(CanvasInput, Role)>, CanvasInfo) {
        self.step_roles_with(events, None)
    }

    /// Como `step_roles`, con una función que corre dentro del fotograma antes de `interact_roles`.
    fn step_roles_with(
        &mut self,
        events: Vec<Event>,
        before: Option<fn(&mut Ui)>,
    ) -> (Vec<(CanvasInput, Role)>, CanvasInfo) {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events,
            ..RawInput::default()
        };
        let mut result = None;
        let canvas = &mut self.canvas;
        let image_2 = &self.image_2;
        let mut output = self.ctx.run_ui(input, |ui| {
            if let Some(before) = before {
                before(ui);
            }
            let entries = canvas.interact_roles(ui);
            canvas.sync(ui.ctx(), image_2).expect("sync");
            result = Some((entries, canvas.paint(ui)));
        });
        self.textures_2.applies(&output.textures_delta);
        output.textures_delta.clear();
        result.expect("el lienzo se dibuja")
    }

    /// Un fotograma con `show`, que debe ser igual que `interact` y `paint` sin superposición.
    fn frame_show(&mut self, events: Vec<Event>) -> (FullOutput, CanvasInfo) {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events,
            ..RawInput::default()
        };
        let mut info = None;
        let canvas = &mut self.canvas;
        let output = self.ctx.run_ui(input, |ui| {
            info = Some(canvas.show(ui));
        });
        self.textures_2.applies(&output.textures_delta);
        (output, info.expect("el lienzo se dibuja"))
    }
}

// Entradas sintéticas.

fn move_pointer(pos: Pos2) -> Event {
    Event::PointerMoved(pos)
}

fn button_2(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn left_2(pos: Pos2, pressed: bool) -> Event {
    button_2(pos, PointerButton::Primary, pressed)
}

fn space(pressed: bool) -> Event {
    Event::Key {
        key: Key::Space,
        physical_key: Some(Key::Space),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn wheel(top: bool) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, if top { 1.0 } else { -1.0 }),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    }
}

/// Lo que ve el lienzo cuando egui ya no ve el botón: el estado del puntero sin pulsaciones.
fn lose_the_button(ui: &mut Ui) {
    ui.input_mut(|i| i.pointer = PointerState::default());
}

/// Centro del píxel `(x, y)` en una vista con el rectángulo y el zoom dados (puntos de egui).
fn center(rect: Rect, zoom: i64, x: i32, y: i32) -> Pos2 {
    let z = zoom as f32;
    pos2(
        rect.min.x + (x as f32 + 0.5) * z,
        rect.min.y + (y as f32 + 0.5) * z,
    )
}

/// Ningún evento de trazo.
const NOTHING: [CanvasEvent; 0] = [];

fn begin(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeBegin((x, y))
}

fn step_a(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeMove((x, y))
}

/// Quita los `StrokeMove` que repiten la posición del `Begin` o del `Move` anterior (soltar sin
/// moverse puede añadir otra vez la misma posición; la spec no lo fija y `Stroke` la ignora).
/// Nada más se toca.
fn without_moves_repeated(events_2: &[CanvasEvent]) -> Vec<CanvasEvent> {
    let mut output_2: Vec<CanvasEvent> = Vec::new();
    let mut last: Option<(i32, i32)> = None;
    for e in events_2 {
        match e {
            CanvasEvent::StrokeMove(p) if last == Some(*p) => continue,
            CanvasEvent::StrokeBegin(p) | CanvasEvent::StrokeMove(p) => last = Some(*p),
            CanvasEvent::StrokeEnd => last = None,
        }
        output_2.push(*e);
    }
    output_2
}

/// Un banco con la imagen subida, el primer fotograma ya ejecutado y los eventos acumulados.
struct Session {
    bench: Bench,
    rect: Rect,
    zoom: i64,
    info: CanvasInfo,
    events_2: Vec<CanvasEvent>,
}

impl Session {
    fn new() -> Session {
        let mut bench = Bench::new_2();
        bench.canvas.set_image(&image_of_test());
        let f = bench.step(vec![]);
        let rect = f.info.image_rect.expect("hay imagen");
        let zoom = f.info.zoom.scale(1);
        assert_eq!(zoom, 12, "800×600 con 64×48 da zoom ×12");
        Session {
            bench,
            rect,
            zoom,
            info: f.info,
            events_2: Vec::new(),
        }
    }

    /// Centro del píxel `(x, y)` con la vista del primer fotograma.
    fn pos(&self, x: i32, y: i32) -> Pos2 {
        center(self.rect, self.zoom, x, y)
    }

    /// Ejecuta un fotograma, acumula sus eventos y devuelve los de este fotograma.
    fn step(&mut self, events: Vec<Event>) -> Vec<CanvasEvent> {
        let f = self.bench.step(events);
        self.events_2.extend(f.events_2.iter().cloned());
        self.info = f.info;
        f.events_2
    }

    /// Como `step`, pero con una función que corre dentro del fotograma antes de `interact`.
    fn step_with(&mut self, events: Vec<Event>, before: fn(&mut Ui)) -> Vec<CanvasEvent> {
        let mut f = self.bench.frame(events, vec![], Some(before));
        f.output.textures_delta.clear();
        self.events_2.extend(f.events_2.iter().cloned());
        self.info = f.info;
        f.events_2
    }

    fn rect_actual(&self) -> Rect {
        self.info.image_rect.expect("hay imagen")
    }
}

/// La app mínima: aplica los eventos de trazo igual que lo hará `px-app`.
struct Mini {
    doc: Document,
    stroke: Option<Stroke>,
    original: Image,
}

impl Mini {
    fn new_2() -> Mini {
        let original = image_of_test();
        Mini {
            doc: Document::new(original.clone()),
            stroke: None,
            original,
        }
    }

    fn apply(&mut self, events_2: &[CanvasEvent]) {
        for e in events_2 {
            match e {
                CanvasEvent::StrokeBegin(p) => {
                    let mut stroke = Stroke::new(WIDTH, HEIGHT);
                    stroke.move_to(*p, red());
                    self.stroke = Some(stroke);
                }
                CanvasEvent::StrokeMove(p) => {
                    if let Some(stroke) = self.stroke.as_mut() {
                        stroke.move_to(*p, red());
                    }
                }
                CanvasEvent::StrokeEnd => {
                    if let Some(stroke) = self.stroke.take() {
                        let id = ImageId::from_raw(0);
                        stroke
                            .commit(&mut self.doc, id)
                            .expect("el trazo se aplica");
                    }
                }
            }
        }
    }

    fn image_2(&self) -> &Image {
        self.doc.image(ImageId::from_raw(0)).expect("hay imagen")
    }

    /// Píxeles que difieren de la imagen original.
    fn changed(&self) -> usize {
        let image_2 = self.image_2();
        (0..HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                image_2.get_pixel(x, y).unwrap() != self.original.get_pixel(x, y).unwrap()
            })
            .count()
    }
}

/// Arrastra el puntero cuatro pasos de `(10, 5)` puntos desde `start_2`; devuelve la última
/// posición.
fn drag(s: &mut Session, start_2: Pos2) -> Pos2 {
    let mut pos = start_2;
    for step in 1..=4 {
        pos = start_2 + vec2(10.0, 5.0) * step as f32;
        s.step(vec![move_pointer(pos)]);
    }
    pos
}

// Fila 14: clic sin mover.

#[test]
fn row_14_a_click_without_moving_emits_begin_and_end_and_paints_a_pixel() {
    let mut s = Session::new();
    let p = s.pos(10, 5);
    assert_eq!(
        s.step(vec![move_pointer(p)]),
        NOTHING,
        "pasar el ratón no hace nada"
    );
    s.step(vec![left_2(p, true)]);
    s.step(vec![left_2(p, false)]);
    assert_eq!(
        without_moves_repeated(&s.events_2),
        [begin(10, 5), CanvasEvent::StrokeEnd]
    );

    let mut app = Mini::new_2();
    app.apply(&s.events_2);
    assert_eq!(app.image_2().get_pixel(10, 5), Ok(red()));
    assert_eq!(app.changed(), 1);
    assert!(app.doc.can_undo());
    assert_eq!(app.doc.undo(), Ok(()));
    assert_eq!(app.image_2(), &app.original, "un solo paso");
    assert!(!app.doc.can_undo());
}

#[test]
fn a_dragged_stroke_emits_one_move_per_pointer_event() {
    let mut s = Session::new();
    let p = s.pos(10, 5);
    s.step(vec![move_pointer(p)]);
    assert_eq!(s.step(vec![left_2(p, true)]), [begin(10, 5)]);
    // Tres posiciones en un mismo fotograma: tres `Move`, en orden, sin perder ninguna.
    let ev = s.step(vec![
        move_pointer(s.pos(11, 5)),
        move_pointer(s.pos(12, 7)),
        move_pointer(s.pos(11, 9)),
    ]);
    assert_eq!(ev, [step_a(11, 5), step_a(12, 7), step_a(11, 9)]);
    // Y uno por fotograma cuando hay un evento por fotograma.
    assert_eq!(s.step(vec![move_pointer(s.pos(20, 9))]), [step_a(20, 9)]);
    s.step(vec![left_2(s.pos(20, 9), false)]);
    assert_eq!(s.events_2.last(), Some(&CanvasEvent::StrokeEnd));

    let mut app = Mini::new_2();
    app.apply(&s.events_2);
    assert!(app.doc.can_undo());
    assert!(app.changed() > 10, "las líneas entre eventos se cierran");
}

#[test]
fn a_stroke_can_be_started_over_the_background_outside_the_image() {
    let mut s = Session::new();
    // A la izquierda de la imagen, dentro del lienzo: el píxel (-1, 5).
    let outside = s.pos(-1, 5);
    assert!(outside.x > 0.0 && outside.x < s.rect.min.x);
    s.step(vec![move_pointer(outside)]);
    assert_eq!(s.step(vec![left_2(outside, true)]), [begin(-1, 5)]);
    s.step(vec![move_pointer(s.pos(0, 5))]);
    s.step(vec![move_pointer(s.pos(2, 5))]);
    s.step(vec![left_2(s.pos(2, 5), false)]);
    assert_eq!(
        without_moves_repeated(&s.events_2),
        [
            begin(-1, 5),
            step_a(0, 5),
            step_a(2, 5),
            CanvasEvent::StrokeEnd
        ]
    );

    // El trozo de fuera se recorta: solo se pintan (0,5), (1,5) y (2,5).
    let mut app = Mini::new_2();
    app.apply(&s.events_2);
    assert_eq!(app.changed(), 3);
    for x in 0..=2 {
        assert_eq!(app.image_2().get_pixel(x, 5), Ok(red()));
    }
}

#[test]
fn with_the_wheel_in_the_middle_of_a_stroke_each_position_uses_the_view_of_that_moment() {
    let mut s = Session::new();
    let p = s.pos(10, 5);
    s.step(vec![move_pointer(p)]);
    assert_eq!(s.step(vec![left_2(p, true)]), [begin(10, 5)]);
    // La rueda cambia el zoom (×13) anclado al cursor: el píxel bajo el cursor se conserva.
    s.step(vec![wheel(true)]);
    assert_eq!(Some(s.info.zoom), Zoom::from_int(13));
    let (rect, zoom) = (s.rect_actual(), s.info.zoom.scale(1));
    let ev = s.step(vec![move_pointer(center(rect, zoom, 12, 5))]);
    assert_eq!(ev, [step_a(12, 5)]);
    s.step(vec![left_2(center(rect, zoom, 12, 5), false)]);
    assert_eq!(
        without_moves_repeated(&s.events_2).last(),
        Some(&CanvasEvent::StrokeEnd)
    );
}

// Fila 15: el gesto se decide al pulsar.

#[test]
fn row_15_the_middle_button_pans_and_does_not_draw() {
    let mut s = Session::new();
    let before = s.rect_actual();
    let start_2 = pos2(400.0, 300.0);
    s.step(vec![move_pointer(start_2)]);
    s.step(vec![button_2(start_2, PointerButton::Middle, true)]);
    let fin = drag(&mut s, start_2);
    s.step(vec![button_2(fin, PointerButton::Middle, false)]);

    assert_eq!(s.events_2, NOTHING, "ningún evento de trazo");
    let after_2 = s.rect_actual();
    let (dx, dy) = (after_2.min.x - before.min.x, after_2.min.y - before.min.y);
    assert!(dx > 0.0 && dx <= 40.0 + 1e-3, "dx = {dx}");
    assert!(dy > 0.0 && dy <= 20.0 + 1e-3, "dy = {dy}");
}

#[test]
fn row_15_the_left_with_space_down_when_pressed_pans_and_does_not_draw() {
    let mut s = Session::new();
    let before = s.rect_actual();
    let start_2 = pos2(400.0, 300.0);
    s.step(vec![move_pointer(start_2)]);
    s.step(vec![space(true)]);
    s.step(vec![left_2(start_2, true)]);
    let fin = drag(&mut s, start_2);
    s.step(vec![left_2(fin, false)]);
    s.step(vec![space(false)]);

    assert_eq!(s.events_2, NOTHING, "ningún evento de trazo");
    let after_2 = s.rect_actual();
    let (dx, dy) = (after_2.min.x - before.min.x, after_2.min.y - before.min.y);
    assert!(dx > 0.0 && dx <= 40.0 + 1e-3, "dx = {dx}");
    assert!(dy > 0.0 && dy <= 20.0 + 1e-3, "dy = {dy}");
}

/// Fila 15 (adaptada en I.2b): `Espacio` a mitad de un trazo **lo cancela**, igual que `Escape`.
/// Antes de I.2b la prueba afirmaba lo contrario (`…_no_desplaza_y_el_trazo_sigue`: el trazo seguía
/// y dejaba los píxeles 10 a 14); ahora la pulsación de `Espacio` emite el mismo `Escape` que la
/// tecla `Escape`, el trazo no deja píxeles y la vista sigue sin moverse (esa mitad se conserva).
/// La repetición de la tecla (`repeat: true`) no cancela.
#[test]
fn row_15_space_in_the_middle_of_a_stroke_cancels_it() {
    fn walk(key_2: Event) -> (Vec<(CanvasInput, Role)>, Rect, Rect, Mini) {
        let mut s = Session::new();
        let before = s.rect_actual();
        let (p0, p1, p2) = (s.pos(10, 5), s.pos(12, 5), s.pos(14, 5));
        let mut all = Vec::new();
        let mut last_2 = before;
        let mut step = |s: &mut Session, events: Vec<Event>| {
            let (entries, info) = s.bench.step_roles(events);
            last_2 = info.image_rect.expect("hay imagen");
            all.extend(entries.clone());
            entries
        };
        step(&mut s, vec![move_pointer(p0)]);
        let entries = step(&mut s, vec![left_2(p0, true)]);
        assert_eq!(
            entries,
            [(CanvasInput::Stroke(begin(10, 5)), Role::Foreground)]
        );
        step(&mut s, vec![move_pointer(p1)]);
        step(&mut s, vec![key_2]);
        step(&mut s, vec![move_pointer(p2)]);
        step(&mut s, vec![left_2(p2, false)]);
        let mut app = Mini::new_2();
        for (input_2, _) in &all {
            match input_2 {
                CanvasInput::Stroke(e) => app.apply(&[*e]),
                CanvasInput::Escape => app.stroke = None,
            }
        }
        (all, before, last_2, app)
    }

    let (with_space, before, after_2, app) = walk(space(true));
    let (with_escape, _, _, _) = walk(Event::Key {
        key: Key::Escape,
        physical_key: Some(Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    assert_eq!(with_space, with_escape, "Espacio hace lo mismo que Escape");
    let escapes = with_space
        .iter()
        .filter(|(e, _)| *e == CanvasInput::Escape)
        .count();
    assert_eq!(escapes, 1, "una sola cancelación");
    assert_eq!(app.changed(), 0, "el trazo cancelado no deja píxeles");
    assert_eq!(after_2, before, "la vista no se mueve");

    // Con la tecla mantenida, las repeticiones no cancelan otra vez: una sola cancelación.
    // (egui marca `repeat` solo si la tecla ya estaba abajo, por eso va después de la pulsación.)
    let repetition = Event::Key {
        key: Key::Space,
        physical_key: Some(Key::Space),
        pressed: true,
        repeat: true,
        modifiers: Modifiers::NONE,
    };
    let mut s = Session::new();
    let (p0, p1) = (s.pos(10, 5), s.pos(12, 5));
    s.step(vec![move_pointer(p0)]);
    s.step(vec![left_2(p0, true)]);
    s.step(vec![move_pointer(p1)]);
    let (first, _) = s.bench.step_roles(vec![space(true)]);
    let (second, _) = s.bench.step_roles(vec![repetition.clone()]);
    let (third, _) = s.bench.step_roles(vec![repetition]);
    let total = [first, second, third]
        .iter()
        .map(|e| e.iter().filter(|(e, _)| *e == CanvasInput::Escape).count())
        .sum::<usize>();
    assert_eq!(total, 1, "la repetición de la tecla no cancela");
}

#[test]
fn row_15_releasing_space_in_the_middle_of_a_pan_does_not_start_drawing() {
    let mut s = Session::new();
    let start_2 = pos2(400.0, 300.0);
    s.step(vec![move_pointer(start_2)]);
    s.step(vec![space(true)]);
    s.step(vec![left_2(start_2, true)]);
    let mut pos = start_2;
    for step in 1..=2 {
        pos = start_2 + vec2(10.0, 5.0) * step as f32;
        s.step(vec![move_pointer(pos)]);
    }
    let a_half = s.rect_actual();
    s.step(vec![space(false)]);
    // El gesto no cambia hasta soltar: sigue desplazando y no hay `Begin`.
    for step in 3..=5 {
        pos = start_2 + vec2(10.0, 5.0) * step as f32;
        s.step(vec![move_pointer(pos)]);
    }
    s.step(vec![left_2(pos, false)]);
    assert_eq!(s.events_2, NOTHING, "no se emite Begin ni nada");
    assert!(
        s.rect_actual().min.x > a_half.min.x,
        "el desplazamiento continúa tras soltar el espacio"
    );
}

#[test]
fn the_extra_buttons_do_nothing() {
    let mut s = Session::new();
    let before = s.rect_actual();
    let start_2 = pos2(400.0, 300.0);
    s.step(vec![move_pointer(start_2)]);
    s.step(vec![button_2(start_2, PointerButton::Extra1, true)]);
    let fin = drag(&mut s, start_2);
    s.step(vec![button_2(fin, PointerButton::Extra1, false)]);
    assert_eq!(s.events_2, NOTHING);
    assert_eq!(s.rect_actual(), before);
}

#[test]
fn a_second_button_during_a_stroke_is_ignored() {
    let mut s = Session::new();
    let (p0, p1, p2, p3) = (s.pos(10, 5), s.pos(12, 5), s.pos(14, 5), s.pos(16, 5));
    s.step(vec![move_pointer(p0)]);
    s.step(vec![left_2(p0, true)]);
    s.step(vec![move_pointer(p1)]);
    s.step(vec![button_2(p1, PointerButton::Secondary, true)]);
    s.step(vec![move_pointer(p2)]);
    s.step(vec![button_2(p2, PointerButton::Secondary, false)]);
    s.step(vec![move_pointer(p3)]);
    s.step(vec![left_2(p3, false)]);
    assert_eq!(
        without_moves_repeated(&s.events_2),
        [
            begin(10, 5),
            step_a(12, 5),
            step_a(14, 5),
            step_a(16, 5),
            CanvasEvent::StrokeEnd
        ]
    );
}

// Fila 16: cuándo termina un trazo.

/// Un trazo horizontal en la fila 20 desde el píxel 30 hasta el 40, listo para terminar de
/// alguna de las formas de la fila 16. Devuelve la sesión con el botón abajo.
fn stroke_open() -> Session {
    let mut s = Session::new();
    let (a, b) = (s.pos(30, 20), s.pos(40, 20));
    s.step(vec![move_pointer(a)]);
    assert_eq!(s.step(vec![left_2(a, true)]), [begin(30, 20)]);
    assert_eq!(s.step(vec![move_pointer(b)]), [step_a(40, 20)]);
    s
}

/// Tras terminar un trazo no queda ninguno abierto: mover el ratón no emite nada y una pulsación
/// nueva empieza uno nuevo.
fn check_that_nothing_is_left_open(s: &mut Session) {
    assert_eq!(s.step(vec![move_pointer(s.pos(45, 22))]), NOTHING);
    assert_eq!(s.step(vec![move_pointer(s.pos(46, 23))]), NOTHING);
    let p = s.pos(5, 40);
    let from_2 = s.events_2.len();
    s.step(vec![move_pointer(p)]);
    assert_eq!(s.step(vec![left_2(p, true)]), [begin(5, 40)]);
    s.step(vec![left_2(p, false)]);
    assert_eq!(
        without_moves_repeated(&s.events_2[from_2..]),
        [begin(5, 40), CanvasEvent::StrokeEnd]
    );
}

#[test]
fn row_16_leaving_with_pointer_gone_and_releasing_outside_includes_the_last_point() {
    let mut s = stroke_open();
    // El ratón sale de la ventana con el botón abajo: el trazo no termina.
    assert_eq!(
        s.step(vec![Event::PointerGone]),
        NOTHING,
        "PointerGone no termina"
    );
    assert_eq!(s.step(vec![]), NOTHING, "ni el fotograma siguiente");
    // Se suelta fuera del lienzo, con posición: píxel (73, 20), fuera de la imagen y de la
    // ventana (x = 898 > 800).
    let outside = s.pos(73, 20);
    assert!(outside.x > SCREEN.x);
    let ev = s.step(vec![left_2(outside, false)]);
    assert_eq!(
        without_moves_repeated(&ev),
        [step_a(73, 20), CanvasEvent::StrokeEnd],
        "la posición de soltado va antes de End"
    );
    assert_eq!(
        without_moves_repeated(&s.events_2),
        [
            begin(30, 20),
            step_a(40, 20),
            step_a(73, 20),
            CanvasEvent::StrokeEnd
        ]
    );

    // Aplicado: la fila 20 desde el píxel 30 hasta el borde derecho (63).
    let mut app = Mini::new_2();
    app.apply(&s.events_2);
    assert_eq!(app.changed(), 34);
    assert_eq!(app.image_2().get_pixel(63, 20), Ok(red()));
    assert!(app.doc.can_undo());

    check_that_nothing_is_left_open(&mut s);
}

#[test]
fn row_16_when_losing_focus_the_stroke_ends_with_what_there_is() {
    let mut s = stroke_open();
    assert_eq!(
        s.step(vec![Event::WindowFocused(false)]),
        [CanvasEvent::StrokeEnd]
    );
    // egui no suelta el botón al perder el foco: lo que venga después no es un trazo, ni
    // siquiera al soltar.
    let (c, d) = (s.pos(45, 22), s.pos(46, 23));
    assert_eq!(s.step(vec![move_pointer(c)]), NOTHING);
    assert_eq!(s.step(vec![left_2(d, false)]), NOTHING);
    assert_eq!(
        s.events_2,
        [begin(30, 20), step_a(40, 20), CanvasEvent::StrokeEnd]
    );

    let mut app = Mini::new_2();
    app.apply(&s.events_2);
    assert_eq!(app.changed(), 11, "los píxeles 30 a 40 de la fila 20");
    assert!(app.doc.can_undo());

    check_that_nothing_is_left_open(&mut s);
}

#[test]
fn row_16_a_lost_release_ends_the_stroke_in_the_next_frame() {
    let mut s = stroke_open();
    // Un fotograma en el que egui ya no ve el botón y no llega ningún evento de soltar.
    assert_eq!(
        s.step_with(vec![], lose_the_button),
        [CanvasEvent::StrokeEnd],
        "la red de seguridad cierra el trazo"
    );
    assert_eq!(
        s.events_2,
        [begin(30, 20), step_a(40, 20), CanvasEvent::StrokeEnd]
    );
    // Y no se emite un segundo `End` en los fotogramas siguientes.
    assert_eq!(s.step(vec![]), NOTHING);

    let mut app = Mini::new_2();
    app.apply(&s.events_2);
    assert_eq!(app.changed(), 11);

    check_that_nothing_is_left_open(&mut s);
}

// `show`, `paint` con superposición y `refresh`.

#[test]
fn show_keeps_painting_the_same_as_interact_and_paint_without_an_overlay() {
    let mut with_show = Bench::new_2();
    let mut split = Bench::new_2();
    with_show.canvas.set_image(&image_of_test());
    split.canvas.set_image(&image_of_test());

    let sequence = [
        vec![],
        vec![move_pointer(pos2(300.0, 200.0))],
        vec![wheel(true)],
        vec![left_2(pos2(300.0, 200.0), true)],
        vec![left_2(pos2(300.0, 200.0), false)],
    ];
    for events in sequence {
        let (mut a, info_a) = with_show.frame_show(events.clone());
        let mut b = split.frame(events, vec![], None);
        assert_eq!(info_a, b.info);
        assert_eq!(a.shapes, b.output.shapes, "mismas formas");
        assert_eq!(
            a.textures_delta.set.keys().collect::<HashSet<_>>(),
            b.output.textures_delta.set.keys().collect::<HashSet<_>>()
        );
        a.textures_delta.clear();
        b.output.textures_delta.clear();
    }
}

/// Formas y vértices teselados de un fotograma.
fn shapes_and_vertices(ctx: &Context, output: &mut FullOutput) -> (usize, usize) {
    let shapes_2 = output.shapes.len();
    let shapes = std::mem::take(&mut output.shapes);
    let vertices = ctx
        .tessellate(shapes, output.pixels_per_point)
        .into_iter()
        .map(|clipped| match clipped.primitive {
            Primitive::Mesh(mesh) => mesh.vertices.len(),
            Primitive::Callback(_) => 0,
        })
        .sum();
    (shapes_2, vertices)
}

/// Lo que la textura debe contener para `image_2` (el oráculo de la unidad 14a: el documento
/// convertido y premultiplicado), con los píxeles `pending_2` ya escritos encima.
fn texture_expected(image_2: &Image, pending_2: &[(u32, u32, Rgba)]) -> Vec<Color32> {
    let mut texels: Vec<Color32> = to_rgba8(image_2)
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    for &(x, y, c) in pending_2 {
        texels[(y * WIDTH + x) as usize] = Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
    }
    texels
}

#[test]
fn paint_with_an_overlay_sees_it_in_the_texture_without_extra_shapes_or_changing_the_image() {
    // Antes: `paint` con superposición añadía rectángulos y no subía la textura. Ahora la
    // superposición va por `set_overlay`: lo que se conserva es que es visible mientras dura el
    // trazo, que la imagen (el documento) no cambia y que al quitarla todo vuelve a lo de antes.
    let mut s = Session::new();
    let texture = s.bench.canvas.texture_id().expect("la textura está subida");
    let original = image_of_test();
    assert_eq!(
        s.bench.texture_2().texels,
        texture_expected(&original, &[]),
        "antes del trazo, la textura es el documento"
    );

    let mut without = s.bench.frame(vec![], vec![], None);
    let (shapes_without, vertices_without) = shapes_and_vertices(&s.bench.ctx, &mut without.output);
    assert!(
        without.output.textures_delta.set.is_empty(),
        "en reposo, nada se sube"
    );
    without.output.textures_delta.clear();

    // Dos píxeles opacos y uno semitransparente (que además lleva el fondo debajo).
    let overlay = vec![
        (1, 1, Rgba::opaque(255, 0, 0)),
        (2, 1, Rgba::opaque(255, 0, 0)),
        (3, 1, Rgba::new(255, 0, 0, 128)),
    ];
    let mut with = s.bench.frame(vec![], overlay.clone(), None);
    let (shapes_with, vertices_with) = shapes_and_vertices(&s.bench.ctx, &mut with.output);

    // Ninguna forma extra: ni rectángulos ni mallas de superposición.
    assert_eq!(
        (shapes_with, vertices_with),
        (shapes_without, vertices_without),
        "la superposición no añade formas"
    );
    // Se sube solo por bloques (deltas parciales) y solo la textura del lienzo.
    assert!(
        with.output
            .textures_delta
            .set
            .keys()
            .all(|id| *id == texture),
        "solo se sube la textura del lienzo"
    );
    let deltas = with
        .output
        .textures_delta
        .set
        .get(&texture)
        .expect("el fotograma en que cambia la superposición sube sus bloques");
    for delta in deltas {
        let ImageData::Color(color) = &delta.image;
        let [x0, y0] = delta.pos.expect("un delta parcial, no la textura entera");
        assert!(
            x0 + color.size[0] <= WIDTH as usize && y0 + color.size[1] <= HEIGHT as usize,
            "el delta {:?} en {:?} cabe en la baldosa",
            color.size,
            (x0, y0)
        );
    }
    with.output.textures_delta.clear();

    // La superposición se ve: la textura compuesta lleva los píxeles pendientes (premultiplicados)
    // y el resto de la imagen es el documento.
    assert_eq!(
        s.bench.texture_2().texels,
        texture_expected(&original, &overlay)
    );
    let texel = |x: usize, y: usize| s.bench.texture_2().texel(x, y).expect("dentro");
    assert_eq!(texel(1, 1), Color32::from_rgba_unmultiplied(255, 0, 0, 255));
    assert_eq!(texel(3, 1), Color32::from_rgba_unmultiplied(255, 0, 0, 128));
    assert_eq!(texel(0, 0), Color32::from_rgb(255, 0, 0), "el resto sigue");
    assert_eq!(texel(63, 47), Color32::from_rgb(0, 0, 255));
    // La imagen no cambia: la superposición no toca el documento.
    assert_eq!(s.bench.image_2, original);
    // La vista y la información son las mismas con y sin superposición.
    assert_eq!(with.info, without.info);

    // El mismo trazo en el fotograma siguiente: no hay píxeles nuevos y no se sube nada.
    let mut equal = s.bench.frame(vec![], overlay, None);
    assert!(
        equal.output.textures_delta.set.is_empty(),
        "solo se sube en el fotograma en que cambia"
    );
    equal.output.textures_delta.clear();

    // Sin superposición vuelve a lo de antes: las mismas formas y la textura es el documento.
    let mut other = s.bench.frame(vec![], vec![], None);
    let (shapes_other, vertices_other) = shapes_and_vertices(&s.bench.ctx, &mut other.output);
    other.output.textures_delta.clear();
    assert_eq!(
        (shapes_other, vertices_other),
        (shapes_without, vertices_without)
    );
    assert_eq!(s.bench.texture_2().texels, texture_expected(&original, &[]));
    assert_eq!(s.bench.image_2, original);
}

#[test]
fn refresh_keeps_the_view_and_brings_the_texture_up_to_date() {
    let mut bench = Bench::new_2();
    let mut image_2 = image_of_test();
    bench.canvas.set_image(&image_2);
    bench.step(vec![]);
    let texture = bench.canvas.texture_id().expect("la textura está subida");

    // Un zoom previo, para que la vista no sea la de ajuste.
    bench.step(vec![move_pointer(pos2(300.0, 200.0))]);
    let f = bench.step(vec![wheel(true)]);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(13));
    let before = f.info;

    image_2
        .set_pixel(5, 5, Pixel::Rgba(Rgba::opaque(0, 255, 0)))
        .expect("dentro");
    bench.image_2 = image_2.clone();
    bench.canvas.refresh(&bench.ctx, &image_2);

    // El fotograma siguiente lleva el contenido nuevo de la misma textura.
    let mut f = bench.frame(vec![], vec![], None);
    assert_eq!(bench.canvas.texture_id(), Some(texture), "misma textura");
    assert_eq!(f.info.image_rect, before.image_rect, "la vista se conserva");
    assert_eq!(f.info.zoom, before.zoom);
    let deltas = f
        .output
        .textures_delta
        .set
        .get(&texture)
        .expect("la textura se actualiza");
    let delta = deltas.iter().last().expect("un delta");
    assert!(delta.is_whole(), "se regenera la textura completa");
    let ImageData::Color(color) = &delta.image;
    assert_eq!(color.size, [WIDTH as usize, HEIGHT as usize]);
    let en = |x: usize, y: usize| color.pixels[y * WIDTH as usize + x];
    assert_eq!(en(5, 5), Color32::from_rgb(0, 255, 0), "el píxel nuevo");
    assert_eq!(en(0, 0), Color32::from_rgb(255, 0, 0), "el resto sigue");
    assert_eq!(en(63, 47), Color32::from_rgb(0, 0, 255));
    f.output.textures_delta.clear();
    assert_eq!(
        bench.texture_2().texels,
        texture_expected(&image_2, &[]),
        "la textura compuesta es el documento"
    );

    // Y los fotogramas posteriores ya no la vuelven a subir.
    let mut f = bench.frame(vec![], vec![], None);
    assert!(!f.output.textures_delta.set.contains_key(&texture));
    assert_eq!(f.info.image_rect, before.image_rect);
    f.output.textures_delta.clear();
}

#[test]
fn refresh_with_another_size_behaves_like_set_image() {
    let mut bench = Bench::new_2();
    bench.canvas.set_image(&image_of_test());
    bench.step(vec![]);
    bench.step(vec![move_pointer(pos2(300.0, 200.0))]);
    let f = bench.step(vec![wheel(true)]);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(13));

    // 32×24 en 800×600: el ajuste es ×25 y la vista vuelve a él.
    let small = Image::new(32, 24, Mode::Rgba).expect("imagen válida");
    bench.image_2 = small.clone();
    bench.canvas.refresh(&bench.ctx, &small);
    let f = bench.step(vec![]);
    assert!(bench.canvas.texture_id().is_some());
    assert_eq!(f.info.error, None);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(25));
    let rect = f.info.image_rect.expect("hay imagen");
    assert!((rect.width() - 800.0).abs() < 1e-3 && (rect.height() - 600.0).abs() < 1e-3);
}

/// Fila 7 (I.2b): `Espacio` sin gesto, repetido, con un desplazamiento en curso o con un campo de
/// texto enfocado no cancela nada. (El diálogo no es del lienzo: `Espacio` sigue activando el botón
/// enfocado o marcando la casilla en `new_image/dialog_tests.rs` y `convert_mode/dialog_tests.rs`,
/// que no cambian, y un diálogo no puede coexistir con un gesto.)
mod space {
    use super::*;

    fn escapes(entries: &[(CanvasInput, Role)]) -> usize {
        entries
            .iter()
            .filter(|(e, _)| *e == CanvasInput::Escape)
            .count()
    }

    #[test]
    fn row_7_without_a_gesture_space_emits_nothing_and_is_still_the_hand() {
        let mut s = Session::new();
        let p = s.pos(10, 5);
        s.step(vec![move_pointer(p)]);
        let (entries, _) = s.bench.step_roles(vec![space(true)]);
        assert!(entries.is_empty(), "{entries:?}");
        // Sigue siendo la tecla de la mano: con ella abajo, el izquierdo desplaza.
        let before = s.rect_actual();
        s.step(vec![left_2(p, true)]);
        s.step(vec![move_pointer(p + vec2(10.0, 5.0))]);
        s.step(vec![move_pointer(p + vec2(20.0, 10.0))]);
        s.step(vec![left_2(p + vec2(20.0, 10.0), false)]);
        assert_eq!(s.events_2, NOTHING);
        assert!(s.rect_actual().min.x > before.min.x);
    }

    #[test]
    fn row_7_a_scroll_in_progress_is_not_cancelled_with_space() {
        let mut s = Session::new();
        let p = s.pos(10, 5);
        s.step(vec![move_pointer(p)]);
        s.step(vec![button_2(p, PointerButton::Middle, true)]);
        s.step(vec![move_pointer(p + vec2(10.0, 5.0))]);
        let a_half = s.rect_actual();
        let (entries, _) = s.bench.step_roles(vec![space(true)]);
        assert_eq!(escapes(&entries), 0);
        s.step(vec![move_pointer(p + vec2(20.0, 10.0))]);
        assert!(
            s.rect_actual().min.x > a_half.min.x,
            "el desplazamiento continúa"
        );
        s.step(vec![button_2(p, PointerButton::Middle, false)]);
    }

    #[test]
    fn row_7_with_a_focused_text_field_space_does_not_cancel_the_stroke() {
        fn field_2(ui: &mut Ui) {
            egui::Area::new(egui::Id::unique("campo-de-prueba"))
                .fixed_pos(pos2(700.0, 550.0))
                .show(ui.ctx(), |ui| {
                    let mut text_2 = String::new();
                    let r = ui.add(TextEdit::singleline(&mut text_2).id(egui::Id::unique("campo")));
                    r.request_focus();
                });
        }
        let mut s = Session::new();
        let (p0, p1) = (s.pos(10, 5), s.pos(12, 5));
        s.step(vec![move_pointer(p0)]);
        s.step(vec![left_2(p0, true)]);
        s.bench
            .step_roles_with(vec![move_pointer(p1)], Some(field_2));
        s.bench.step_roles_with(vec![], Some(field_2));
        assert!(s.bench.ctx.text_edit_focused(), "el campo tiene el foco");
        let (entries, _) = s.bench.step_roles_with(vec![space(true)], Some(field_2));
        assert_eq!(
            escapes(&entries),
            0,
            "con el foco en el campo, Espacio no cancela"
        );
        let (entries, _) = s
            .bench
            .step_roles_with(vec![move_pointer(p0)], Some(field_2));
        assert!(
            entries
                .iter()
                .any(|(e, _)| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeMove(_)))),
            "el trazo sigue"
        );
    }
}
