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

struct Banco {
    ctx: Context,
    canvas: Canvas,
    rect: Rect,
}

impl Banco {
    fn nuevo() -> Banco {
        let mut canvas = Canvas::new();
        canvas.set_image(&Image::new(64, 48, Mode::Rgba).expect("imagen"));
        let mut banco = Banco {
            ctx: Context::default(),
            canvas,
            rect: Rect::NOTHING,
        };
        banco.rect = banco.paso_roles(vec![], None).1;
        banco
    }

    fn entrada(events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            events,
            ..RawInput::default()
        }
    }

    /// Un fotograma con `interact_roles`; devuelve las entradas y el rectángulo de la imagen.
    fn paso_roles(
        &mut self,
        events: Vec<Event>,
        antes: Option<fn(&mut Ui)>,
    ) -> (Vec<(CanvasInput, Role)>, Rect) {
        let mut resultado = None;
        let canvas = &mut self.canvas;
        let mut salida = self.ctx.run_ui(Banco::entrada(events), |ui| {
            if let Some(antes) = antes {
                antes(ui);
            }
            let entradas = canvas.interact_roles(ui);
            let info = canvas.paint(ui);
            resultado = Some((entradas, info.image_rect.expect("hay imagen")));
        });
        salida.textures_delta.clear();
        resultado.expect("el lienzo se dibuja")
    }

    /// Centro del píxel `(x, y)` (zoom ×12).
    fn pos(&self, x: i32, y: i32) -> Pos2 {
        pos2(
            self.rect.min.x + (x as f32 + 0.5) * 12.0,
            self.rect.min.y + (y as f32 + 0.5) * 12.0,
        )
    }
}

fn boton(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
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

fn perder_el_boton(ui: &mut Ui) {
    ui.input_mut(|i| i.pointer = PointerState::default());
}

/// Quita los `Move` que repiten la posición anterior (soltar sin moverse añade uno).
fn sin_repetidos(entradas: &[(CanvasInput, Role)]) -> Vec<(CanvasInput, Role)> {
    let mut salida: Vec<(CanvasInput, Role)> = Vec::new();
    let mut ultima = None;
    for entrada in entradas {
        match entrada.0 {
            CanvasInput::Stroke(CanvasEvent::StrokeMove(p)) if ultima == Some(p) => continue,
            CanvasInput::Stroke(CanvasEvent::StrokeBegin(p) | CanvasEvent::StrokeMove(p)) => {
                ultima = Some(p);
            }
            _ => ultima = None,
        }
        salida.push(*entrada);
    }
    salida
}

fn begin(x: i32, y: i32) -> CanvasInput {
    CanvasInput::Stroke(CanvasEvent::StrokeBegin((x, y)))
}

fn mueve(x: i32, y: i32) -> CanvasInput {
    CanvasInput::Stroke(CanvasEvent::StrokeMove((x, y)))
}

fn fin() -> CanvasInput {
    CanvasInput::Stroke(CanvasEvent::StrokeEnd)
}

/// Un trazo de `(10, 5)` a `(14, 5)` con `button`; devuelve todo lo emitido.
fn trazo(banco: &mut Banco, button: PointerButton) -> Vec<(CanvasInput, Role)> {
    let (p0, p1, p2) = (banco.pos(10, 5), banco.pos(12, 5), banco.pos(14, 5));
    let mut todo = Vec::new();
    for events in [
        vec![Event::PointerMoved(p0)],
        vec![boton(p0, button, true)],
        vec![Event::PointerMoved(p1)],
        vec![Event::PointerMoved(p2)],
        vec![boton(p2, button, false)],
    ] {
        todo.extend(banco.paso_roles(events, None).0);
    }
    sin_repetidos(&todo)
}

#[test]
fn fila_2_el_boton_izquierdo_da_el_primer_plano_en_todos_los_eventos_del_trazo() {
    let mut banco = Banco::nuevo();
    let r = Role::Foreground;
    assert_eq!(
        trazo(&mut banco, PointerButton::Primary),
        [
            (begin(10, 5), r),
            (mueve(12, 5), r),
            (mueve(14, 5), r),
            (fin(), r)
        ]
    );
}

#[test]
fn fila_2_el_boton_derecho_da_el_fondo_en_todos_los_eventos_del_trazo() {
    let mut banco = Banco::nuevo();
    let r = Role::Background;
    assert_eq!(
        trazo(&mut banco, PointerButton::Secondary),
        [
            (begin(10, 5), r),
            (mueve(12, 5), r),
            (mueve(14, 5), r),
            (fin(), r)
        ]
    );
}

#[test]
fn fila_2_el_derecho_no_desplaza_y_el_central_desplaza_sin_emitir_trazo() {
    let mut banco = Banco::nuevo();
    let antes = banco.rect;
    let derecho = trazo(&mut banco, PointerButton::Secondary);
    assert!(!derecho.is_empty());
    let (_, rect) = banco.paso_roles(vec![], None);
    assert_eq!(rect, antes, "el derecho no mueve la vista");

    let (p0, p1) = (pos2(400.0, 300.0), pos2(440.0, 330.0));
    let mut todo = Vec::new();
    for events in [
        vec![Event::PointerMoved(p0)],
        vec![boton(p0, PointerButton::Middle, true)],
        vec![Event::PointerMoved(p1)],
        vec![boton(p1, PointerButton::Middle, false)],
    ] {
        todo.extend(banco.paso_roles(events, None).0);
    }
    assert_eq!(todo, [], "el central no emite trazo");
    let (_, rect) = banco.paso_roles(vec![], None);
    assert_ne!(rect, antes, "el central desplaza");
}

#[test]
fn fila_2_un_segundo_boton_con_un_gesto_en_curso_y_soltar_el_ajeno_no_hacen_nada() {
    for (dueno, otro, rol) in [
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
        let mut banco = Banco::nuevo();
        let (p0, p1, p2, p3) = (
            banco.pos(10, 5),
            banco.pos(12, 5),
            banco.pos(14, 5),
            banco.pos(16, 5),
        );
        let mut todo = Vec::new();
        for events in [
            vec![Event::PointerMoved(p0)],
            vec![boton(p0, dueno, true)],
            vec![Event::PointerMoved(p1)],
            vec![boton(p1, otro, true)],
            vec![Event::PointerMoved(p2)],
            vec![boton(p2, otro, false)],
            vec![Event::PointerMoved(p3)],
            vec![boton(p3, dueno, false)],
        ] {
            todo.extend(banco.paso_roles(events, None).0);
        }
        assert_eq!(
            sin_repetidos(&todo),
            [
                (begin(10, 5), rol),
                (mueve(12, 5), rol),
                (mueve(14, 5), rol),
                (mueve(16, 5), rol),
                (fin(), rol)
            ],
            "{dueno:?} con {otro:?}"
        );
    }
}

#[test]
fn fila_2_escape_a_mitad_de_un_trazo_con_el_derecho_sigue_en_su_sitio() {
    let mut banco = Banco::nuevo();
    let (p0, p1) = (banco.pos(10, 5), banco.pos(12, 5));
    let mut todo = Vec::new();
    for events in [
        vec![Event::PointerMoved(p0)],
        vec![boton(p0, PointerButton::Secondary, true)],
        vec![Event::PointerMoved(p1)],
        vec![escape()],
    ] {
        todo.extend(banco.paso_roles(events, None).0);
    }
    let todo = sin_repetidos(&todo);
    assert_eq!(
        todo,
        [
            (begin(10, 5), Role::Background),
            (mueve(12, 5), Role::Background),
            (CanvasInput::Escape, Role::Background),
        ]
    );
}

#[test]
fn fila_2_un_soltar_perdido_del_derecho_termina_el_trazo_con_el_fondo() {
    let mut banco = Banco::nuevo();
    let p0 = banco.pos(10, 5);
    banco.paso_roles(vec![Event::PointerMoved(p0)], None);
    banco.paso_roles(vec![boton(p0, PointerButton::Secondary, true)], None);
    let (entradas, _) = banco.paso_roles(vec![], Some(perder_el_boton));
    assert_eq!(entradas, [(fin(), Role::Background)]);
}
