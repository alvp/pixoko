//! Unidad I.2a, fila 2 (specs/interfaz-dos-colores.md): `Canvas::interact_roles` en el banco de
//! egui sin ventana. El botón que **empieza** el gesto da su rol (izquierdo: primer plano;
//! derecho: fondo) a todos los eventos del trazo; el central desplaza; el derecho no desplaza; un
//! segundo botón con un gesto en curso se ignora (decisión 2; `interact` e `interact_ordered`,
//! que no emitían nada con el derecho, se retiraron en la unidad L1).
//!
//! El banco es el de `tests/palette_panel.rs`, reducido al lienzo: 800×600 con una imagen de
//! 64×48 da zoom ×12 y los centros de los píxeles son enteros.

use egui::{
    Context, Event, Key, Modifiers, PointerButton, PointerState, Pos2, RawInput, Rect, Ui, pos2,
    vec2,
};
use px_core::{Image, Mode};
use px_ui::canvas::{Canvas, CanvasEvent, CanvasInput};
use px_ui::color::Role;

struct Bench {
    ctx: Context,
    canvas: Canvas,
    rect: Rect,
}

impl Bench {
    fn new_2() -> Bench {
        let mut canvas = Canvas::new();
        canvas.set_image(&Image::new(64, 48, Mode::Rgba).expect("imagen"));
        let mut bench = Bench {
            ctx: Context::default(),
            canvas,
            rect: Rect::NOTHING,
        };
        bench.rect = bench.step_roles(vec![], None).1;
        bench
    }

    fn input_2(events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            events,
            ..RawInput::default()
        }
    }

    /// Un fotograma con `interact_roles`; devuelve las entradas y el rectángulo de la imagen.
    fn step_roles(
        &mut self,
        events: Vec<Event>,
        before: Option<fn(&mut Ui)>,
    ) -> (Vec<(CanvasInput, Role)>, Rect) {
        let mut result = None;
        let canvas = &mut self.canvas;
        let mut output_2 = self.ctx.run_ui(Bench::input_2(events), |ui| {
            if let Some(before) = before {
                before(ui);
            }
            let entries = canvas.interact_roles(ui);
            let info = canvas.paint(ui);
            result = Some((entries, info.image_rect.expect("hay imagen")));
        });
        output_2.textures_delta.clear();
        result.expect("el lienzo se dibuja")
    }

    /// Centro del píxel `(x, y)` (zoom ×12).
    fn pos(&self, x: i32, y: i32) -> Pos2 {
        pos2(
            self.rect.min.x + (x as f32 + 0.5) * 12.0,
            self.rect.min.y + (y as f32 + 0.5) * 12.0,
        )
    }
}

fn button_2(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn escape() -> Event {
    Event::Key {
        key: Key::Escape,
        physical_key: Some(Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn lose_the_button(ui: &mut Ui) {
    ui.input_mut(|i| i.pointer = PointerState::default());
}

/// Quita los `Move` que repiten la posición anterior (soltar sin moverse añade uno).
fn without_repeated(entries: &[(CanvasInput, Role)]) -> Vec<(CanvasInput, Role)> {
    let mut output_2: Vec<(CanvasInput, Role)> = Vec::new();
    let mut last = None;
    for input_2 in entries {
        match input_2.0 {
            CanvasInput::Stroke(CanvasEvent::StrokeMove(p)) if last == Some(p) => continue,
            CanvasInput::Stroke(CanvasEvent::StrokeBegin(p) | CanvasEvent::StrokeMove(p)) => {
                last = Some(p);
            }
            _ => last = None,
        }
        output_2.push(*input_2);
    }
    output_2
}

fn begin(x: i32, y: i32) -> CanvasInput {
    CanvasInput::Stroke(CanvasEvent::StrokeBegin((x, y)))
}

fn moves(x: i32, y: i32) -> CanvasInput {
    CanvasInput::Stroke(CanvasEvent::StrokeMove((x, y)))
}

fn fin() -> CanvasInput {
    CanvasInput::Stroke(CanvasEvent::StrokeEnd)
}

/// Un trazo de `(10, 5)` a `(14, 5)` con `button`; devuelve todo lo emitido.
fn stroke_2(bench: &mut Bench, button: PointerButton) -> Vec<(CanvasInput, Role)> {
    let (p0, p1, p2) = (bench.pos(10, 5), bench.pos(12, 5), bench.pos(14, 5));
    let mut all = Vec::new();
    for events in [
        vec![Event::PointerMoved(p0)],
        vec![button_2(p0, button, true)],
        vec![Event::PointerMoved(p1)],
        vec![Event::PointerMoved(p2)],
        vec![button_2(p2, button, false)],
    ] {
        all.extend(bench.step_roles(events, None).0);
    }
    without_repeated(&all)
}

#[test]
fn row_2_the_left_button_gives_the_foreground_in_all_the_stroke_events() {
    let mut bench = Bench::new_2();
    let r = Role::Foreground;
    assert_eq!(
        stroke_2(&mut bench, PointerButton::Primary),
        [
            (begin(10, 5), r),
            (moves(12, 5), r),
            (moves(14, 5), r),
            (fin(), r)
        ]
    );
}

#[test]
fn row_2_the_right_button_gives_the_background_in_all_the_stroke_events() {
    let mut bench = Bench::new_2();
    let r = Role::Background;
    assert_eq!(
        stroke_2(&mut bench, PointerButton::Secondary),
        [
            (begin(10, 5), r),
            (moves(12, 5), r),
            (moves(14, 5), r),
            (fin(), r)
        ]
    );
}

#[test]
fn row_2_the_right_one_does_not_pan_and_the_middle_one_pans_without_emitting_a_stroke() {
    let mut bench = Bench::new_2();
    let before = bench.rect;
    let right = stroke_2(&mut bench, PointerButton::Secondary);
    assert!(!right.is_empty());
    let (_, rect) = bench.step_roles(vec![], None);
    assert_eq!(rect, before, "el derecho no mueve la vista");

    let (p0, p1) = (pos2(400.0, 300.0), pos2(440.0, 330.0));
    let mut all = Vec::new();
    for events in [
        vec![Event::PointerMoved(p0)],
        vec![button_2(p0, PointerButton::Middle, true)],
        vec![Event::PointerMoved(p1)],
        vec![button_2(p1, PointerButton::Middle, false)],
    ] {
        all.extend(bench.step_roles(events, None).0);
    }
    assert_eq!(all, [], "el central no emite trazo");
    let (_, rect) = bench.step_roles(vec![], None);
    assert_ne!(rect, before, "el central desplaza");
}

#[test]
fn row_2_a_second_button_with_a_gesture_in_progress_and_releasing_the_other_do_nothing() {
    for (owner, other_2, role) in [
        (
            PointerButton::Primary,
            PointerButton::Secondary,
            Role::Foreground,
        ),
        (
            PointerButton::Secondary,
            PointerButton::Primary,
            Role::Background,
        ),
    ] {
        let mut bench = Bench::new_2();
        let (p0, p1, p2, p3) = (
            bench.pos(10, 5),
            bench.pos(12, 5),
            bench.pos(14, 5),
            bench.pos(16, 5),
        );
        let mut all = Vec::new();
        for events in [
            vec![Event::PointerMoved(p0)],
            vec![button_2(p0, owner, true)],
            vec![Event::PointerMoved(p1)],
            vec![button_2(p1, other_2, true)],
            vec![Event::PointerMoved(p2)],
            vec![button_2(p2, other_2, false)],
            vec![Event::PointerMoved(p3)],
            vec![button_2(p3, owner, false)],
        ] {
            all.extend(bench.step_roles(events, None).0);
        }
        assert_eq!(
            without_repeated(&all),
            [
                (begin(10, 5), role),
                (moves(12, 5), role),
                (moves(14, 5), role),
                (moves(16, 5), role),
                (fin(), role)
            ],
            "{owner:?} con {other_2:?}"
        );
    }
}

#[test]
fn row_2_escape_in_the_middle_of_a_stroke_with_the_right_one_stays_in_place() {
    let mut bench = Bench::new_2();
    let (p0, p1) = (bench.pos(10, 5), bench.pos(12, 5));
    let mut all = Vec::new();
    for events in [
        vec![Event::PointerMoved(p0)],
        vec![button_2(p0, PointerButton::Secondary, true)],
        vec![Event::PointerMoved(p1)],
        vec![escape()],
    ] {
        all.extend(bench.step_roles(events, None).0);
    }
    let all = without_repeated(&all);
    assert_eq!(
        all,
        [
            (begin(10, 5), Role::Background),
            (moves(12, 5), Role::Background),
            (CanvasInput::Escape, Role::Background),
        ]
    );
}

#[test]
fn row_2_a_lost_release_of_the_right_one_ends_the_stroke_with_the_background() {
    let mut bench = Bench::new_2();
    let p0 = bench.pos(10, 5);
    bench.step_roles(vec![Event::PointerMoved(p0)], None);
    bench.step_roles(vec![button_2(p0, PointerButton::Secondary, true)], None);
    let (entries, _) = bench.step_roles(vec![], Some(lose_the_button));
    assert_eq!(entries, [(fin(), Role::Background)]);
}
