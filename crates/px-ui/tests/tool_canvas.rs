//! Tests de `Canvas` y `ToolState` juntos, con el banco de egui sin ventana
//! (specs/hito-2-linea-relleno-app.md, «Casos límite» filas 20 y 21 y la segunda parte de la 17).
//!
//! El banco es el de `tests/pencil.rs` (un `egui::Context` con `RawInput` sintético, copiado y no
//! modificado): `Canvas::interact` y `Canvas::paint` como hará la app. Cada cambio de estado del
//! botón va en su propio fotograma. Las posiciones se calculan con el rectángulo de la imagen del
//! primer fotograma, en el centro de cada píxel: 800×600 con 64×48 da zoom ×12.
//!
//! `Session` hace de `px-app`: pasa los eventos de `interact` a `ToolState::canvas_event` en el
//! orden en que salieron y guarda los resultados.
//!
//! El soltar perdido se fabrica igual que en `tests/pencil.rs`: se baja el botón con
//! `InputState::pointer` dentro del fotograma, antes de `interact`, que es lo que ve el lienzo
//! cuando egui «ya no ve el botón».

use std::collections::BTreeSet;

use egui::{
    Context, Event, FullOutput, Key, Modifiers, PointerButton, PointerState, Pos2, RawInput, Rect,
    Ui, Vec2, pos2, vec2,
};
use px_core::{Document, Image, ImageId, Mode, Palette, Pixel, Rgba};
use px_ui::canvas::{Canvas, CanvasEvent, CanvasInfo, CanvasInput};
use px_ui::tool::{Tool, ToolAction, ToolState};

const SCREEN: Vec2 = vec2(800.0, 600.0);
const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;

fn red() -> Pixel {
    Pixel::Rgba(Rgba::opaque(255, 0, 0))
}

/// Imagen RGBA de 64×48 vacía.
fn image_rgba() -> Image {
    Image::new(WIDTH, HEIGHT, Mode::Rgba).expect("imagen válida")
}

/// Imagen indexada de 64×48 con una paleta de 3 entradas: la ranura 5 no tiene color.
fn image_indexed() -> Image {
    let palette = Palette::from_colors([
        Rgba::opaque(10, 10, 10),
        Rgba::opaque(20, 20, 20),
        Rgba::opaque(30, 30, 30),
    ])
    .expect("paleta válida");
    Image::new_indexed(WIDTH, HEIGHT, palette).expect("imagen válida")
}

struct Bench {
    ctx: Context,
    canvas: Canvas,
}

struct TestFrame {
    output: FullOutput,
    events_2: Vec<CanvasInput>,
    info: CanvasInfo,
}

impl Bench {
    fn new_2() -> Bench {
        Bench {
            ctx: Context::default(),
            canvas: Canvas::new(),
        }
    }

    /// Ejecuta un fotograma: `antes` (si lo hay) corre dentro del fotograma, antes de `interact`;
    /// luego `interact_ordered` (o `interact`), y `paint` sin superposición. No limpia los cambios de textura.
    fn frame(&mut self, events: Vec<Event>, antes: Option<fn(&mut Ui)>) -> TestFrame {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events,
            ..RawInput::default()
        };
        let mut result = None;
        let canvas = &mut self.canvas;
        let output = self.ctx.run_ui(input, |ui| {
            if let Some(antes) = antes {
                antes(ui);
            }
            let events_2 = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            let info = canvas.paint(ui);
            result = Some((events_2, info));
        });
        let (events_2, info) = result.expect("el lienzo se dibuja");
        TestFrame {
            output,
            events_2,
            info,
        }
    }

    /// Un fotograma con los cambios de textura ya limpiados (egui entra en pánico si se sueltan
    /// sin aplicar).
    fn step(&mut self, events: Vec<Event>) -> TestFrame {
        let mut f = self.frame(events, None);
        f.output.textures_delta.clear();
        f
    }
}

// Entradas sintéticas.

fn mover(pos: Pos2) -> Event {
    Event::PointerMoved(pos)
}

fn left_2(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn key_2(key: Key, modifiers: Modifiers, pressed: bool) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers,
    }
}

fn escape() -> Event {
    key_2(Key::Escape, Modifiers::NONE, true)
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
const NOTHING: [CanvasInput; 0] = [];

fn begin(x: i32, y: i32) -> CanvasInput {
    CanvasInput::Stroke(CanvasEvent::StrokeBegin((x, y)))
}

fn step_a(x: i32, y: i32) -> CanvasInput {
    CanvasInput::Stroke(CanvasEvent::StrokeMove((x, y)))
}

fn fin() -> CanvasInput {
    CanvasInput::Stroke(CanvasEvent::StrokeEnd)
}

/// Los eventos de trazo de una lista ordenada, sin los `Escape`: lo que da `Canvas::interact`.
fn only_stroke(entries: &[CanvasInput]) -> Vec<CanvasEvent> {
    entries
        .iter()
        .filter_map(|e| match e {
            CanvasInput::Stroke(event) => Some(*event),
            CanvasInput::Escape => None,
        })
        .collect()
}

/// El banco con la imagen subida y el primer fotograma ya ejecutado, más el documento y la
/// herramienta que reciben los eventos.
struct Session {
    bench: Bench,
    /// Los eventos de trazo (sin `Escape`) del último fotograma.
    ultimo_interact: Vec<CanvasEvent>,
    rect: Rect,
    zoom: i64,
    /// Todos los eventos del lienzo (`interact_ordered`), en orden.
    events_2: Vec<CanvasInput>,
    /// El resultado de `canvas_event` para cada evento de `events_2`.
    results: Vec<bool>,
    doc: Document,
    id: ImageId,
    tool: ToolState,
    slot: u8,
    original: Image,
}

impl Session {
    fn new(image: Image, tool_2: Tool, slot: u8) -> Session {
        let mut bench = Bench::new_2();
        bench.canvas.set_image(&image);
        let f = bench.step(vec![]);
        let rect = f.info.image_rect.expect("hay imagen");
        let zoom = f.info.zoom.scale(1);
        assert_eq!(zoom, 12, "800×600 con 64×48 da zoom ×12");
        let doc = Document::new(image.clone());
        let id = ImageId::from_raw(0);
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(tool_2));
        Session {
            bench,
            ultimo_interact: Vec::new(),
            rect,
            zoom,
            events_2: Vec::new(),
            results: Vec::new(),
            doc,
            id,
            tool,
            slot,
            original: image,
        }
    }

    /// Centro del píxel `(x, y)` con la vista del primer fotograma.
    fn pos(&self, x: i32, y: i32) -> Pos2 {
        center(self.rect, self.zoom, x, y)
    }

    /// Pasa los eventos del fotograma a la herramienta, en orden.
    fn feed(&mut self, events_2: &[CanvasInput]) {
        for &event in events_2 {
            let r = self
                .tool
                .canvas_input(&mut self.doc, self.id, self.slot, event)
                .unwrap_or_else(|e| panic!("{event:?} dio el error {e:?}"));
            self.events_2.push(event);
            self.results.push(r);
        }
    }

    /// Ejecuta un fotograma y devuelve los eventos de este fotograma.
    fn step(&mut self, events: Vec<Event>) -> Vec<CanvasInput> {
        let f = self.bench.step(events);
        self.ultimo_interact = only_stroke(&f.events_2);
        self.feed(&f.events_2);
        f.events_2
    }

    /// Como `step`, pero con una función que corre dentro del fotograma antes de `interact`.
    fn step_with(&mut self, events: Vec<Event>, antes: fn(&mut Ui)) -> Vec<CanvasInput> {
        let mut f = self.bench.frame(events, Some(antes));
        f.output.textures_delta.clear();
        self.ultimo_interact = only_stroke(&f.events_2);
        self.feed(&f.events_2);
        f.events_2
    }

    fn image_2(&self) -> &Image {
        self.doc.image(self.id).expect("hay imagen")
    }

    /// Píxeles que difieren de la imagen original.
    fn changed(&self) -> BTreeSet<(u32, u32)> {
        let image_2 = self.image_2();
        (0..HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                image_2.get_pixel(x, y).unwrap() != self.original.get_pixel(x, y).unwrap()
            })
            .collect()
    }

    /// Los píxeles de la fila `y` de `x0` a `x1`, incluidos.
    fn row_2(y: u32, x0: u32, x1: u32) -> BTreeSet<(u32, u32)> {
        (x0..=x1).map(|x| (x, y)).collect()
    }

    fn unchanged(&self) -> bool {
        self.image_2() == &self.original && !self.doc.can_undo()
    }

    /// Cuántos `canvas_event` dieron `Ok(true)`.
    fn true_value(&self) -> usize {
        self.results.iter().filter(|&&r| r).count()
    }
}

/// Una línea horizontal en la fila 20 desde el píxel 30 hasta el 40, con el botón abajo.
fn open_line_stroke() -> Session {
    stroke_open(image_rgba(), Tool::Line, 3)
}

fn stroke_open(image: Image, tool_2: Tool, slot: u8) -> Session {
    let mut s = Session::new(image, tool_2, slot);
    let (a, b) = (s.pos(30, 20), s.pos(40, 20));
    s.step(vec![mover(a)]);
    assert_eq!(s.step(vec![left_2(a, true)]), [begin(30, 20)]);
    assert_eq!(s.step(vec![mover(b)]), [step_a(40, 20)]);
    s
}

/// Tras terminar un gesto no queda ninguno abierto y se puede cambiar de herramienta.
fn check_that_nothing_is_left_open(s: &mut Session) {
    assert!(!s.tool.busy(), "busy() debe ser falso");
    assert_eq!(s.step(vec![mover(s.pos(45, 22))]), NOTHING);
    assert_eq!(s.step(vec![mover(s.pos(46, 23))]), NOTHING);
    assert!(!s.tool.busy());
    s.tool.apply_action(ToolAction::Select(Tool::Fill));
    assert_eq!(s.tool.tool(), Tool::Fill, "Select ya cambia de herramienta");
}

// Fila 20.

#[test]
fn row_20_escape_before_the_release_in_the_same_frame_cancels_the_line() {
    let mut s = open_line_stroke();
    let ev = s.step(vec![escape(), left_2(s.pos(41, 20), false)]);
    assert_eq!(ev, [CanvasInput::Escape, step_a(41, 20), fin()]);
    // `interact` da lo mismo que antes, sin el `Escape`.
    assert_eq!(
        s.ultimo_interact,
        [CanvasEvent::StrokeMove((41, 20)), CanvasEvent::StrokeEnd]
    );
    assert!(s.unchanged(), "la línea se cancela: ni imagen ni historia");
    assert_eq!(s.true_value(), 0);
    check_that_nothing_is_left_open(&mut s);
}

#[test]
fn row_20_escape_after_the_release_in_the_same_frame_leaves_the_line() {
    let mut s = open_line_stroke();
    let ev = s.step(vec![left_2(s.pos(41, 20), false), escape()]);
    assert_eq!(ev, [step_a(41, 20), fin(), CanvasInput::Escape]);
    assert_eq!(
        s.ultimo_interact,
        [CanvasEvent::StrokeMove((41, 20)), CanvasEvent::StrokeEnd],
        "interact: lo mismo que antes, sin el Escape"
    );
    // El soltado ya aplicó la línea hasta el último destino (30 a 41) y `Escape` no la deshace.
    assert_eq!(s.changed(), Session::row_2(20, 30, 41));
    assert_eq!(s.image_2().get_pixel(41, 20), Ok(red()));
    assert!(s.doc.can_undo());
    assert_eq!(s.true_value(), 1);
    assert_eq!(
        s.results.last(),
        Some(&false),
        "Escape da Ok(false) y no hace nada"
    );
    check_that_nothing_is_left_open(&mut s);
}

#[test]
fn row_20_shift_escape_emits_no_event_and_the_stroke_continues() {
    let mut s = open_line_stroke();
    let ev = s.step(vec![key_2(Key::Escape, Modifiers::SHIFT, true)]);
    assert_eq!(ev, NOTHING, "Escape con Shift no es un Escape");
    assert_eq!(s.ultimo_interact, []);
    assert!(s.tool.busy());
    // El trazo sigue vivo y se aplica al soltar.
    s.step(vec![left_2(s.pos(41, 20), false)]);
    assert_eq!(s.changed(), Session::row_2(20, 30, 41));
    check_that_nothing_is_left_open(&mut s);
}

#[test]
fn row_20_escape_without_a_stroke_emits_only_escape_and_the_tool_does_nothing() {
    for tool_2 in [Tool::Pencil, Tool::Line, Tool::Fill] {
        let mut s = Session::new(image_rgba(), tool_2, 3);
        let ev = s.step(vec![escape()]);
        assert_eq!(ev, [CanvasInput::Escape], "{tool_2:?}");
        assert_eq!(
            s.ultimo_interact,
            [],
            "{tool_2:?}: interact no emite Escape"
        );
        assert_eq!(s.results, [false], "{tool_2:?}: ToolState no hace nada");
        assert!(s.unchanged());
        assert!(!s.tool.busy());
        assert_eq!(s.tool.tool(), tool_2);
    }
}

// Fila 20, ampliaciones que no están en la tabla (sacadas de «`Escape` sin modificadores» y de que
// `Canvas::interact` solo mira las teclas pulsadas).

#[test]
fn escape_with_any_modifier_emits_no_event() {
    let with_ctrl_and_command = Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    };
    for modifiers in [
        Modifiers::SHIFT,
        Modifiers::ALT,
        Modifiers::CTRL,
        Modifiers::COMMAND,
        Modifiers::MAC_CMD,
        with_ctrl_and_command,
    ] {
        let mut s = Session::new(image_rgba(), Tool::Line, 3);
        let ev = s.step(vec![key_2(Key::Escape, modifiers, true)]);
        assert_eq!(ev, NOTHING, "Escape con {modifiers:?}");
    }
}

#[test]
fn releasing_the_escape_key_emits_no_event() {
    let mut s = open_line_stroke();
    let ev = s.step(vec![key_2(Key::Escape, Modifiers::NONE, false)]);
    assert_eq!(ev, NOTHING, "solo la pulsación es un Escape");
    assert!(s.tool.busy());
}

// Fila 21.

#[test]
fn row_21_losing_focus_applies_the_line_and_does_not_leave_the_gesture_hanging() {
    let mut s = open_line_stroke();
    assert_eq!(s.step(vec![Event::WindowFocused(false)]), [fin()]);
    assert!(!s.tool.busy());
    assert_eq!(
        s.changed(),
        Session::row_2(20, 30, 40),
        "hasta el último destino"
    );
    assert!(s.doc.can_undo());
    check_that_nothing_is_left_open(&mut s);
}

#[test]
fn row_21_pointer_gone_and_releasing_outside_applies_the_line_up_to_the_release_point() {
    let mut s = open_line_stroke();
    assert_eq!(
        s.step(vec![Event::PointerGone]),
        NOTHING,
        "PointerGone no termina"
    );
    assert!(s.tool.busy());
    assert_eq!(s.step(vec![]), NOTHING);
    assert!(s.tool.busy());
    // Se suelta fuera de la ventana: píxel (73, 20), con x = 898 > 800.
    let outside = s.pos(73, 20);
    assert!(outside.x > SCREEN.x);
    assert_eq!(
        s.step(vec![left_2(outside, false)]),
        [step_a(73, 20), fin()]
    );
    assert!(!s.tool.busy());
    // De 30 al borde derecho (63): 34 píxeles.
    assert_eq!(s.changed(), Session::row_2(20, 30, 63));
    assert!(s.doc.can_undo());
    check_that_nothing_is_left_open(&mut s);
}

#[test]
fn row_21_a_lost_release_applies_the_line_in_the_next_frame() {
    let mut s = open_line_stroke();
    assert_eq!(
        s.step_with(vec![], lose_the_button),
        [fin()],
        "la red de seguridad cierra el trazo"
    );
    assert!(!s.tool.busy());
    assert_eq!(s.changed(), Session::row_2(20, 30, 40));
    assert!(s.doc.can_undo());
    assert_eq!(s.step(vec![]), NOTHING, "ni un segundo End");
    check_that_nothing_is_left_open(&mut s);
}

/// Cómo termina un gesto en la fila 21, con el botón ya abajo.
#[derive(Clone, Copy, Debug)]
enum Final {
    Release,
    LoseTheFocus,
    ReleaseLost,
    PointerGoneAndReleaseOutside,
}

const FINALES: [Final; 4] = [
    Final::Release,
    Final::LoseTheFocus,
    Final::ReleaseLost,
    Final::PointerGoneAndReleaseOutside,
];

fn finish(s: &mut Session, final_: Final) {
    match final_ {
        Final::Release => {
            let p = s.pos(41, 20);
            s.step(vec![left_2(p, false)]);
        }
        Final::LoseTheFocus => {
            s.step(vec![Event::WindowFocused(false)]);
        }
        Final::ReleaseLost => {
            s.step_with(vec![], lose_the_button);
        }
        Final::PointerGoneAndReleaseOutside => {
            s.step(vec![Event::PointerGone]);
            let p = s.pos(73, 20);
            s.step(vec![left_2(p, false)]);
        }
    }
}

#[test]
fn row_21_a_slot_without_color_does_not_leave_the_shortcuts_blocked() {
    // Paleta de 3 entradas: la ranura 5 no tiene color. Pulsar y soltar; lo mismo con las otras
    // formas de terminar el gesto y con las tres herramientas.
    for tool_2 in [Tool::Pencil, Tool::Line, Tool::Fill] {
        for final_ in FINALES {
            let mut s = Session::new(image_indexed(), tool_2, 5);
            let a = s.pos(30, 20);
            s.step(vec![mover(a)]);
            assert_eq!(s.step(vec![left_2(a, true)]), [begin(30, 20)]);
            assert!(s.tool.busy(), "{tool_2:?}/{final_:?}: ocupado tras Begin");
            // Con el botón abajo los atajos no valen.
            s.tool.apply_action(ToolAction::Select(Tool::Fill));
            assert_eq!(s.tool.tool(), tool_2);

            finish(&mut s, final_);
            assert!(
                !s.tool.busy(),
                "{tool_2:?}/{final_:?}: busy() debe volver a falso"
            );
            assert!(s.unchanged(), "{tool_2:?}/{final_:?}: sin cambios");
            assert_eq!(s.true_value(), 0);
            s.tool.apply_action(ToolAction::Select(Tool::Fill));
            assert_eq!(
                s.tool.tool(),
                Tool::Fill,
                "{tool_2:?}/{final_:?}: Select ya cambia de herramienta"
            );
        }
    }
}

#[test]
fn row_21_a_slot_without_color_press_and_release_without_moving() {
    // El caso literal de la fila: pulsar y soltar, con la herramienta de línea.
    let mut s = Session::new(image_indexed(), Tool::Line, 5);
    let p = s.pos(10, 10);
    s.step(vec![mover(p)]);
    s.step(vec![left_2(p, true)]);
    assert!(s.tool.busy());
    s.step(vec![left_2(p, false)]);
    assert!(!s.tool.busy());
    assert!(s.unchanged());
    s.tool.apply_action(ToolAction::Select(Tool::Fill));
    assert_eq!(s.tool.tool(), Tool::Fill);
}

// Fila 17, segunda parte.

#[test]
fn row_17_with_the_button_down_30_frames_the_fill_does_not_repeat() {
    let mut s = Session::new(image_rgba(), Tool::Fill, 3);
    let p = s.pos(30, 20);
    s.step(vec![mover(p)]);
    assert_eq!(
        s.step(vec![left_2(p, true)]),
        [begin(30, 20)],
        "Canvas emite solo Begin"
    );
    for frame in 0..30 {
        assert_eq!(s.step(vec![]), NOTHING, "fotograma {frame} sin eventos");
    }
    assert_eq!(s.events_2, [begin(30, 20)], "Canvas solo emitió Begin");
    assert_eq!(s.results, [true], "un solo Ok(true)");
    assert!(s.tool.busy(), "ocupado hasta soltar");
    // El relleno ya está hecho (todo el lienzo, que era uniforme).
    assert_eq!(s.changed().len(), (WIDTH * HEIGHT) as usize);

    // Soltar no rellena otra vez.
    s.step(vec![left_2(p, false)]);
    assert!(!s.tool.busy());
    assert_eq!(s.true_value(), 1, "ningún relleno más");
    assert_eq!(s.doc.undo(), Ok(()));
    assert!(s.unchanged(), "un solo paso");
}
