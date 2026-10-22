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
//! `common::texturas::Texturas` (la textura compuesta) y el fotograma es el de la app: `interact`,
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
use common::texturas::{Textura, Texturas};

const SCREEN: Vec2 = vec2(800.0, 600.0);
const ANCHO: u32 = 64;
const ALTO: u32 = 48;

fn rojo() -> Pixel {
    Pixel::Rgba(Rgba::opaque(255, 0, 0))
}

/// Imagen RGBA de 64×48 con un píxel de cada color en las esquinas.
fn imagen_de_prueba() -> Image {
    let mut image = Image::new(ANCHO, ALTO, Mode::Rgba).expect("imagen válida");
    image
        .set_pixel(0, 0, Pixel::Rgba(Rgba::opaque(255, 0, 0)))
        .expect("dentro");
    image
        .set_pixel(63, 47, Pixel::Rgba(Rgba::opaque(0, 0, 255)))
        .expect("dentro");
    image
}

struct Banco {
    ctx: Context,
    canvas: Canvas,
    /// El documento que el lienzo muestra (la imagen con la que se llama a `set_overlay` y
    /// `sync`). Las pruebas que cambian la imagen la actualizan aquí.
    imagen: Image,
    /// Contenido de cada textura, compuesto en orden con los `textures_delta` de cada fotograma.
    texturas: Texturas,
}

struct Fotograma {
    output: FullOutput,
    eventos: Vec<CanvasEvent>,
    info: CanvasInfo,
}

impl Banco {
    fn nuevo() -> Banco {
        Banco {
            ctx: Context::default(),
            canvas: Canvas::new(),
            imagen: imagen_de_prueba(),
            texturas: Texturas::new(),
        }
    }

    /// La textura compuesta de la baldosa 0 (la única de estas imágenes).
    fn textura(&self) -> &Textura {
        let id = self.canvas.texture_id().expect("la textura está subida");
        self.texturas.get(id).expect("se ha subido su contenido")
    }

    /// Ejecuta un fotograma como el de la app: `antes` (si lo hay) corre dentro del fotograma,
    /// antes de `interact`; luego `interact`, `set_overlay` con la superposición dada (el trazo
    /// pendiente entero; vacía la quita), `sync` y `paint`. Aplica los `textures_delta` a
    /// `texturas` y no los limpia (quien llama los mira y los limpia).
    fn fotograma(
        &mut self,
        events: Vec<Event>,
        overlay: Vec<(u32, u32, Rgba)>,
        antes: Option<fn(&mut Ui)>,
    ) -> Fotograma {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events,
            ..RawInput::default()
        };
        let mut resultado = None;
        let canvas = &mut self.canvas;
        let imagen = &self.imagen;
        let output = self.ctx.run_ui(input, |ui| {
            if let Some(antes) = antes {
                antes(ui);
            }
            let eventos: Vec<CanvasEvent> = canvas
                .interact_roles(ui)
                .into_iter()
                .filter_map(|(input, _)| match input {
                    CanvasInput::Stroke(event) => Some(event),
                    CanvasInput::Escape => None,
                })
                .collect();
            canvas.set_overlay(ui.ctx(), imagen, &overlay);
            canvas.sync(ui.ctx(), imagen).expect("sync");
            let info = canvas.paint(ui);
            resultado = Some((eventos, info));
        });
        self.texturas.aplica(&output.textures_delta);
        let (eventos, info) = resultado.expect("el lienzo se dibuja");
        Fotograma {
            output,
            eventos,
            info,
        }
    }

    /// Un fotograma sin superposición y con los cambios de textura ya aplicados y limpiados (egui
    /// entra en pánico si se sueltan sin aplicar).
    fn paso(&mut self, events: Vec<Event>) -> Fotograma {
        let mut f = self.fotograma(events, vec![], None);
        f.output.textures_delta.clear();
        f
    }

    /// Un fotograma como el de la app (`interact_roles`, `sync` y `paint`), con la entrada
    /// ordenada: los eventos de trazo **y** `Escape`, cada uno con su rol.
    fn paso_roles(&mut self, events: Vec<Event>) -> (Vec<(CanvasInput, Role)>, CanvasInfo) {
        self.paso_roles_con(events, None)
    }

    /// Como `paso_roles`, con una función que corre dentro del fotograma antes de `interact_roles`.
    fn paso_roles_con(
        &mut self,
        events: Vec<Event>,
        antes: Option<fn(&mut Ui)>,
    ) -> (Vec<(CanvasInput, Role)>, CanvasInfo) {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events,
            ..RawInput::default()
        };
        let mut resultado = None;
        let canvas = &mut self.canvas;
        let imagen = &self.imagen;
        let mut output = self.ctx.run_ui(input, |ui| {
            if let Some(antes) = antes {
                antes(ui);
            }
            let entradas = canvas.interact_roles(ui);
            canvas.sync(ui.ctx(), imagen).expect("sync");
            resultado = Some((entradas, canvas.paint(ui)));
        });
        self.texturas.aplica(&output.textures_delta);
        output.textures_delta.clear();
        resultado.expect("el lienzo se dibuja")
    }

    /// Un fotograma con `show`, que debe ser igual que `interact` y `paint` sin superposición.
    fn fotograma_show(&mut self, events: Vec<Event>) -> (FullOutput, CanvasInfo) {
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
        self.texturas.aplica(&output.textures_delta);
        (output, info.expect("el lienzo se dibuja"))
    }
}

// Entradas sintéticas.

fn mover(pos: Pos2) -> Event {
    Event::PointerMoved(pos)
}

fn boton(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn izquierdo(pos: Pos2, pressed: bool) -> Event {
    boton(pos, PointerButton::Primary, pressed)
}

fn espacio(pressed: bool) -> Event {
    Event::Key {
        key: Key::Space,
        physical_key: Some(Key::Space),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn rueda(arriba: bool) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, if arriba { 1.0 } else { -1.0 }),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    }
}

/// Lo que ve el lienzo cuando egui ya no ve el botón: el estado del puntero sin pulsaciones.
fn perder_el_boton(ui: &mut Ui) {
    ui.input_mut(|i| i.pointer = PointerState::default());
}

/// Centro del píxel `(x, y)` en una vista con el rectángulo y el zoom dados (puntos de egui).
fn centro(rect: Rect, zoom: i64, x: i32, y: i32) -> Pos2 {
    let z = zoom as f32;
    pos2(
        rect.min.x + (x as f32 + 0.5) * z,
        rect.min.y + (y as f32 + 0.5) * z,
    )
}

/// Ningún evento de trazo.
const NADA: [CanvasEvent; 0] = [];

fn begin(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeBegin((x, y))
}

fn paso_a(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeMove((x, y))
}

/// Quita los `StrokeMove` que repiten la posición del `Begin` o del `Move` anterior (soltar sin
/// moverse puede añadir otra vez la misma posición; la spec no lo fija y `Stroke` la ignora).
/// Nada más se toca.
fn sin_moves_repetidos(eventos: &[CanvasEvent]) -> Vec<CanvasEvent> {
    let mut salida: Vec<CanvasEvent> = Vec::new();
    let mut ultima: Option<(i32, i32)> = None;
    for e in eventos {
        match e {
            CanvasEvent::StrokeMove(p) if ultima == Some(*p) => continue,
            CanvasEvent::StrokeBegin(p) | CanvasEvent::StrokeMove(p) => ultima = Some(*p),
            CanvasEvent::StrokeEnd => ultima = None,
        }
        salida.push(*e);
    }
    salida
}

/// Un banco con la imagen subida, el primer fotograma ya ejecutado y los eventos acumulados.
struct Sesion {
    banco: Banco,
    rect: Rect,
    zoom: i64,
    info: CanvasInfo,
    eventos: Vec<CanvasEvent>,
}

impl Sesion {
    fn nueva() -> Sesion {
        let mut banco = Banco::nuevo();
        banco.canvas.set_image(&imagen_de_prueba());
        let f = banco.paso(vec![]);
        let rect = f.info.image_rect.expect("hay imagen");
        let zoom = f.info.zoom.scale(1);
        assert_eq!(zoom, 12, "800×600 con 64×48 da zoom ×12");
        Sesion {
            banco,
            rect,
            zoom,
            info: f.info,
            eventos: Vec::new(),
        }
    }

    /// Centro del píxel `(x, y)` con la vista del primer fotograma.
    fn pos(&self, x: i32, y: i32) -> Pos2 {
        centro(self.rect, self.zoom, x, y)
    }

    /// Ejecuta un fotograma, acumula sus eventos y devuelve los de este fotograma.
    fn paso(&mut self, events: Vec<Event>) -> Vec<CanvasEvent> {
        let f = self.banco.paso(events);
        self.eventos.extend(f.eventos.iter().cloned());
        self.info = f.info;
        f.eventos
    }

    /// Como `paso`, pero con una función que corre dentro del fotograma antes de `interact`.
    fn paso_con(&mut self, events: Vec<Event>, antes: fn(&mut Ui)) -> Vec<CanvasEvent> {
        let mut f = self.banco.fotograma(events, vec![], Some(antes));
        f.output.textures_delta.clear();
        self.eventos.extend(f.eventos.iter().cloned());
        self.info = f.info;
        f.eventos
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
    fn nuevo() -> Mini {
        let original = imagen_de_prueba();
        Mini {
            doc: Document::new(original.clone()),
            stroke: None,
            original,
        }
    }

    fn aplicar(&mut self, eventos: &[CanvasEvent]) {
        for e in eventos {
            match e {
                CanvasEvent::StrokeBegin(p) => {
                    let mut stroke = Stroke::new(ANCHO, ALTO);
                    stroke.move_to(*p, rojo());
                    self.stroke = Some(stroke);
                }
                CanvasEvent::StrokeMove(p) => {
                    if let Some(stroke) = self.stroke.as_mut() {
                        stroke.move_to(*p, rojo());
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

    fn imagen(&self) -> &Image {
        self.doc.image(ImageId::from_raw(0)).expect("hay imagen")
    }

    /// Píxeles que difieren de la imagen original.
    fn cambiados(&self) -> usize {
        let imagen = self.imagen();
        (0..ALTO)
            .flat_map(|y| (0..ANCHO).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                imagen.get_pixel(x, y).unwrap() != self.original.get_pixel(x, y).unwrap()
            })
            .count()
    }
}

/// Arrastra el puntero cuatro pasos de `(10, 5)` puntos desde `inicio`; devuelve la última
/// posición.
fn arrastrar(s: &mut Sesion, inicio: Pos2) -> Pos2 {
    let mut pos = inicio;
    for paso in 1..=4 {
        pos = inicio + vec2(10.0, 5.0) * paso as f32;
        s.paso(vec![mover(pos)]);
    }
    pos
}

// Fila 14: clic sin mover.

#[test]
fn fila_14_un_clic_sin_mover_emite_begin_y_end_y_pinta_un_pixel() {
    let mut s = Sesion::nueva();
    let p = s.pos(10, 5);
    assert_eq!(s.paso(vec![mover(p)]), NADA, "pasar el ratón no hace nada");
    s.paso(vec![izquierdo(p, true)]);
    s.paso(vec![izquierdo(p, false)]);
    assert_eq!(
        sin_moves_repetidos(&s.eventos),
        [begin(10, 5), CanvasEvent::StrokeEnd]
    );

    let mut app = Mini::nuevo();
    app.aplicar(&s.eventos);
    assert_eq!(app.imagen().get_pixel(10, 5), Ok(rojo()));
    assert_eq!(app.cambiados(), 1);
    assert!(app.doc.can_undo());
    assert_eq!(app.doc.undo(), Ok(()));
    assert_eq!(app.imagen(), &app.original, "un solo paso");
    assert!(!app.doc.can_undo());
}

#[test]
fn un_trazo_arrastrado_emite_un_move_por_cada_evento_de_puntero() {
    let mut s = Sesion::nueva();
    let p = s.pos(10, 5);
    s.paso(vec![mover(p)]);
    assert_eq!(s.paso(vec![izquierdo(p, true)]), [begin(10, 5)]);
    // Tres posiciones en un mismo fotograma: tres `Move`, en orden, sin perder ninguna.
    let ev = s.paso(vec![
        mover(s.pos(11, 5)),
        mover(s.pos(12, 7)),
        mover(s.pos(11, 9)),
    ]);
    assert_eq!(ev, [paso_a(11, 5), paso_a(12, 7), paso_a(11, 9)]);
    // Y uno por fotograma cuando hay un evento por fotograma.
    assert_eq!(s.paso(vec![mover(s.pos(20, 9))]), [paso_a(20, 9)]);
    s.paso(vec![izquierdo(s.pos(20, 9), false)]);
    assert_eq!(s.eventos.last(), Some(&CanvasEvent::StrokeEnd));

    let mut app = Mini::nuevo();
    app.aplicar(&s.eventos);
    assert!(app.doc.can_undo());
    assert!(app.cambiados() > 10, "las líneas entre eventos se cierran");
}

#[test]
fn se_puede_empezar_un_trazo_sobre_el_fondo_fuera_de_la_imagen() {
    let mut s = Sesion::nueva();
    // A la izquierda de la imagen, dentro del lienzo: el píxel (-1, 5).
    let fuera = s.pos(-1, 5);
    assert!(fuera.x > 0.0 && fuera.x < s.rect.min.x);
    s.paso(vec![mover(fuera)]);
    assert_eq!(s.paso(vec![izquierdo(fuera, true)]), [begin(-1, 5)]);
    s.paso(vec![mover(s.pos(0, 5))]);
    s.paso(vec![mover(s.pos(2, 5))]);
    s.paso(vec![izquierdo(s.pos(2, 5), false)]);
    assert_eq!(
        sin_moves_repetidos(&s.eventos),
        [
            begin(-1, 5),
            paso_a(0, 5),
            paso_a(2, 5),
            CanvasEvent::StrokeEnd
        ]
    );

    // El trozo de fuera se recorta: solo se pintan (0,5), (1,5) y (2,5).
    let mut app = Mini::nuevo();
    app.aplicar(&s.eventos);
    assert_eq!(app.cambiados(), 3);
    for x in 0..=2 {
        assert_eq!(app.imagen().get_pixel(x, 5), Ok(rojo()));
    }
}

#[test]
fn con_la_rueda_a_mitad_de_trazo_cada_posicion_usa_la_vista_de_ese_momento() {
    let mut s = Sesion::nueva();
    let p = s.pos(10, 5);
    s.paso(vec![mover(p)]);
    assert_eq!(s.paso(vec![izquierdo(p, true)]), [begin(10, 5)]);
    // La rueda cambia el zoom (×13) anclado al cursor: el píxel bajo el cursor se conserva.
    s.paso(vec![rueda(true)]);
    assert_eq!(Some(s.info.zoom), Zoom::from_int(13));
    let (rect, zoom) = (s.rect_actual(), s.info.zoom.scale(1));
    let ev = s.paso(vec![mover(centro(rect, zoom, 12, 5))]);
    assert_eq!(ev, [paso_a(12, 5)]);
    s.paso(vec![izquierdo(centro(rect, zoom, 12, 5), false)]);
    assert_eq!(
        sin_moves_repetidos(&s.eventos).last(),
        Some(&CanvasEvent::StrokeEnd)
    );
}

// Fila 15: el gesto se decide al pulsar.

#[test]
fn fila_15_el_boton_central_desplaza_y_no_dibuja() {
    let mut s = Sesion::nueva();
    let antes = s.rect_actual();
    let inicio = pos2(400.0, 300.0);
    s.paso(vec![mover(inicio)]);
    s.paso(vec![boton(inicio, PointerButton::Middle, true)]);
    let fin = arrastrar(&mut s, inicio);
    s.paso(vec![boton(fin, PointerButton::Middle, false)]);

    assert_eq!(s.eventos, NADA, "ningún evento de trazo");
    let despues = s.rect_actual();
    let (dx, dy) = (despues.min.x - antes.min.x, despues.min.y - antes.min.y);
    assert!(dx > 0.0 && dx <= 40.0 + 1e-3, "dx = {dx}");
    assert!(dy > 0.0 && dy <= 20.0 + 1e-3, "dy = {dy}");
}

#[test]
fn fila_15_el_izquierdo_con_espacio_abajo_al_pulsar_desplaza_y_no_dibuja() {
    let mut s = Sesion::nueva();
    let antes = s.rect_actual();
    let inicio = pos2(400.0, 300.0);
    s.paso(vec![mover(inicio)]);
    s.paso(vec![espacio(true)]);
    s.paso(vec![izquierdo(inicio, true)]);
    let fin = arrastrar(&mut s, inicio);
    s.paso(vec![izquierdo(fin, false)]);
    s.paso(vec![espacio(false)]);

    assert_eq!(s.eventos, NADA, "ningún evento de trazo");
    let despues = s.rect_actual();
    let (dx, dy) = (despues.min.x - antes.min.x, despues.min.y - antes.min.y);
    assert!(dx > 0.0 && dx <= 40.0 + 1e-3, "dx = {dx}");
    assert!(dy > 0.0 && dy <= 20.0 + 1e-3, "dy = {dy}");
}

/// Fila 15 (adaptada en I.2b): `Espacio` a mitad de un trazo **lo cancela**, igual que `Escape`.
/// Antes de I.2b la prueba afirmaba lo contrario (`…_no_desplaza_y_el_trazo_sigue`: el trazo seguía
/// y dejaba los píxeles 10 a 14); ahora la pulsación de `Espacio` emite el mismo `Escape` que la
/// tecla `Escape`, el trazo no deja píxeles y la vista sigue sin moverse (esa mitad se conserva).
/// La repetición de la tecla (`repeat: true`) no cancela.
#[test]
fn fila_15_espacio_a_mitad_de_un_trazo_lo_cancela() {
    fn recorrido(tecla: Event) -> (Vec<(CanvasInput, Role)>, Rect, Rect, Mini) {
        let mut s = Sesion::nueva();
        let antes = s.rect_actual();
        let (p0, p1, p2) = (s.pos(10, 5), s.pos(12, 5), s.pos(14, 5));
        let mut todo = Vec::new();
        let mut ultimo = antes;
        let mut paso = |s: &mut Sesion, events: Vec<Event>| {
            let (entradas, info) = s.banco.paso_roles(events);
            ultimo = info.image_rect.expect("hay imagen");
            todo.extend(entradas.clone());
            entradas
        };
        paso(&mut s, vec![mover(p0)]);
        let entradas = paso(&mut s, vec![izquierdo(p0, true)]);
        assert_eq!(
            entradas,
            [(CanvasInput::Stroke(begin(10, 5)), Role::Foreground)]
        );
        paso(&mut s, vec![mover(p1)]);
        paso(&mut s, vec![tecla]);
        paso(&mut s, vec![mover(p2)]);
        paso(&mut s, vec![izquierdo(p2, false)]);
        let mut app = Mini::nuevo();
        for (entrada, _) in &todo {
            match entrada {
                CanvasInput::Stroke(e) => app.aplicar(&[*e]),
                CanvasInput::Escape => app.stroke = None,
            }
        }
        (todo, antes, ultimo, app)
    }

    let (con_espacio, antes, despues, app) = recorrido(espacio(true));
    let (con_escape, _, _, _) = recorrido(Event::Key {
        key: Key::Escape,
        physical_key: Some(Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    assert_eq!(con_espacio, con_escape, "Espacio hace lo mismo que Escape");
    let escapes = con_espacio
        .iter()
        .filter(|(e, _)| *e == CanvasInput::Escape)
        .count();
    assert_eq!(escapes, 1, "una sola cancelación");
    assert_eq!(app.cambiados(), 0, "el trazo cancelado no deja píxeles");
    assert_eq!(despues, antes, "la vista no se mueve");

    // Con la tecla mantenida, las repeticiones no cancelan otra vez: una sola cancelación.
    // (egui marca `repeat` solo si la tecla ya estaba abajo, por eso va después de la pulsación.)
    let repeticion = Event::Key {
        key: Key::Space,
        physical_key: Some(Key::Space),
        pressed: true,
        repeat: true,
        modifiers: Modifiers::NONE,
    };
    let mut s = Sesion::nueva();
    let (p0, p1) = (s.pos(10, 5), s.pos(12, 5));
    s.paso(vec![mover(p0)]);
    s.paso(vec![izquierdo(p0, true)]);
    s.paso(vec![mover(p1)]);
    let (primera, _) = s.banco.paso_roles(vec![espacio(true)]);
    let (segunda, _) = s.banco.paso_roles(vec![repeticion.clone()]);
    let (tercera, _) = s.banco.paso_roles(vec![repeticion]);
    let total = [primera, segunda, tercera]
        .iter()
        .map(|e| e.iter().filter(|(e, _)| *e == CanvasInput::Escape).count())
        .sum::<usize>();
    assert_eq!(total, 1, "la repetición de la tecla no cancela");
}

#[test]
fn fila_15_soltar_espacio_a_mitad_de_un_desplazamiento_no_empieza_a_dibujar() {
    let mut s = Sesion::nueva();
    let inicio = pos2(400.0, 300.0);
    s.paso(vec![mover(inicio)]);
    s.paso(vec![espacio(true)]);
    s.paso(vec![izquierdo(inicio, true)]);
    let mut pos = inicio;
    for paso in 1..=2 {
        pos = inicio + vec2(10.0, 5.0) * paso as f32;
        s.paso(vec![mover(pos)]);
    }
    let a_mitad = s.rect_actual();
    s.paso(vec![espacio(false)]);
    // El gesto no cambia hasta soltar: sigue desplazando y no hay `Begin`.
    for paso in 3..=5 {
        pos = inicio + vec2(10.0, 5.0) * paso as f32;
        s.paso(vec![mover(pos)]);
    }
    s.paso(vec![izquierdo(pos, false)]);
    assert_eq!(s.eventos, NADA, "no se emite Begin ni nada");
    assert!(
        s.rect_actual().min.x > a_mitad.min.x,
        "el desplazamiento continúa tras soltar el espacio"
    );
}

#[test]
fn los_botones_extra_no_hacen_nada() {
    let mut s = Sesion::nueva();
    let antes = s.rect_actual();
    let inicio = pos2(400.0, 300.0);
    s.paso(vec![mover(inicio)]);
    s.paso(vec![boton(inicio, PointerButton::Extra1, true)]);
    let fin = arrastrar(&mut s, inicio);
    s.paso(vec![boton(fin, PointerButton::Extra1, false)]);
    assert_eq!(s.eventos, NADA);
    assert_eq!(s.rect_actual(), antes);
}

#[test]
fn un_segundo_boton_durante_un_trazo_se_ignora() {
    let mut s = Sesion::nueva();
    let (p0, p1, p2, p3) = (s.pos(10, 5), s.pos(12, 5), s.pos(14, 5), s.pos(16, 5));
    s.paso(vec![mover(p0)]);
    s.paso(vec![izquierdo(p0, true)]);
    s.paso(vec![mover(p1)]);
    s.paso(vec![boton(p1, PointerButton::Secondary, true)]);
    s.paso(vec![mover(p2)]);
    s.paso(vec![boton(p2, PointerButton::Secondary, false)]);
    s.paso(vec![mover(p3)]);
    s.paso(vec![izquierdo(p3, false)]);
    assert_eq!(
        sin_moves_repetidos(&s.eventos),
        [
            begin(10, 5),
            paso_a(12, 5),
            paso_a(14, 5),
            paso_a(16, 5),
            CanvasEvent::StrokeEnd
        ]
    );
}

// Fila 16: cuándo termina un trazo.

/// Un trazo horizontal en la fila 20 desde el píxel 30 hasta el 40, listo para terminar de
/// alguna de las formas de la fila 16. Devuelve la sesión con el botón abajo.
fn trazo_abierto() -> Sesion {
    let mut s = Sesion::nueva();
    let (a, b) = (s.pos(30, 20), s.pos(40, 20));
    s.paso(vec![mover(a)]);
    assert_eq!(s.paso(vec![izquierdo(a, true)]), [begin(30, 20)]);
    assert_eq!(s.paso(vec![mover(b)]), [paso_a(40, 20)]);
    s
}

/// Tras terminar un trazo no queda ninguno abierto: mover el ratón no emite nada y una pulsación
/// nueva empieza uno nuevo.
fn comprobar_que_no_queda_nada_abierto(s: &mut Sesion) {
    assert_eq!(s.paso(vec![mover(s.pos(45, 22))]), NADA);
    assert_eq!(s.paso(vec![mover(s.pos(46, 23))]), NADA);
    let p = s.pos(5, 40);
    let desde = s.eventos.len();
    s.paso(vec![mover(p)]);
    assert_eq!(s.paso(vec![izquierdo(p, true)]), [begin(5, 40)]);
    s.paso(vec![izquierdo(p, false)]);
    assert_eq!(
        sin_moves_repetidos(&s.eventos[desde..]),
        [begin(5, 40), CanvasEvent::StrokeEnd]
    );
}

#[test]
fn fila_16_salir_con_pointer_gone_y_soltar_fuera_incluye_el_ultimo_punto() {
    let mut s = trazo_abierto();
    // El ratón sale de la ventana con el botón abajo: el trazo no termina.
    assert_eq!(
        s.paso(vec![Event::PointerGone]),
        NADA,
        "PointerGone no termina"
    );
    assert_eq!(s.paso(vec![]), NADA, "ni el fotograma siguiente");
    // Se suelta fuera del lienzo, con posición: píxel (73, 20), fuera de la imagen y de la
    // ventana (x = 898 > 800).
    let fuera = s.pos(73, 20);
    assert!(fuera.x > SCREEN.x);
    let ev = s.paso(vec![izquierdo(fuera, false)]);
    assert_eq!(
        sin_moves_repetidos(&ev),
        [paso_a(73, 20), CanvasEvent::StrokeEnd],
        "la posición de soltado va antes de End"
    );
    assert_eq!(
        sin_moves_repetidos(&s.eventos),
        [
            begin(30, 20),
            paso_a(40, 20),
            paso_a(73, 20),
            CanvasEvent::StrokeEnd
        ]
    );

    // Aplicado: la fila 20 desde el píxel 30 hasta el borde derecho (63).
    let mut app = Mini::nuevo();
    app.aplicar(&s.eventos);
    assert_eq!(app.cambiados(), 34);
    assert_eq!(app.imagen().get_pixel(63, 20), Ok(rojo()));
    assert!(app.doc.can_undo());

    comprobar_que_no_queda_nada_abierto(&mut s);
}

#[test]
fn fila_16_al_perder_el_foco_el_trazo_termina_con_lo_que_hay() {
    let mut s = trazo_abierto();
    assert_eq!(
        s.paso(vec![Event::WindowFocused(false)]),
        [CanvasEvent::StrokeEnd]
    );
    // egui no suelta el botón al perder el foco: lo que venga después no es un trazo, ni
    // siquiera al soltar.
    let (c, d) = (s.pos(45, 22), s.pos(46, 23));
    assert_eq!(s.paso(vec![mover(c)]), NADA);
    assert_eq!(s.paso(vec![izquierdo(d, false)]), NADA);
    assert_eq!(
        s.eventos,
        [begin(30, 20), paso_a(40, 20), CanvasEvent::StrokeEnd]
    );

    let mut app = Mini::nuevo();
    app.aplicar(&s.eventos);
    assert_eq!(app.cambiados(), 11, "los píxeles 30 a 40 de la fila 20");
    assert!(app.doc.can_undo());

    comprobar_que_no_queda_nada_abierto(&mut s);
}

#[test]
fn fila_16_un_soltar_perdido_termina_el_trazo_en_el_fotograma_siguiente() {
    let mut s = trazo_abierto();
    // Un fotograma en el que egui ya no ve el botón y no llega ningún evento de soltar.
    assert_eq!(
        s.paso_con(vec![], perder_el_boton),
        [CanvasEvent::StrokeEnd],
        "la red de seguridad cierra el trazo"
    );
    assert_eq!(
        s.eventos,
        [begin(30, 20), paso_a(40, 20), CanvasEvent::StrokeEnd]
    );
    // Y no se emite un segundo `End` en los fotogramas siguientes.
    assert_eq!(s.paso(vec![]), NADA);

    let mut app = Mini::nuevo();
    app.aplicar(&s.eventos);
    assert_eq!(app.cambiados(), 11);

    comprobar_que_no_queda_nada_abierto(&mut s);
}

// `show`, `paint` con superposición y `refresh`.

#[test]
fn show_sigue_pintando_lo_mismo_que_interact_y_paint_sin_superposicion() {
    let mut con_show = Banco::nuevo();
    let mut partido = Banco::nuevo();
    con_show.canvas.set_image(&imagen_de_prueba());
    partido.canvas.set_image(&imagen_de_prueba());

    let secuencia = [
        vec![],
        vec![mover(pos2(300.0, 200.0))],
        vec![rueda(true)],
        vec![izquierdo(pos2(300.0, 200.0), true)],
        vec![izquierdo(pos2(300.0, 200.0), false)],
    ];
    for events in secuencia {
        let (mut a, info_a) = con_show.fotograma_show(events.clone());
        let mut b = partido.fotograma(events, vec![], None);
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
fn formas_y_vertices(ctx: &Context, output: &mut FullOutput) -> (usize, usize) {
    let formas = output.shapes.len();
    let shapes = std::mem::take(&mut output.shapes);
    let vertices = ctx
        .tessellate(shapes, output.pixels_per_point)
        .into_iter()
        .map(|clipped| match clipped.primitive {
            Primitive::Mesh(mesh) => mesh.vertices.len(),
            Primitive::Callback(_) => 0,
        })
        .sum();
    (formas, vertices)
}

/// Lo que la textura debe contener para `imagen` (el oráculo de la unidad 14a: el documento
/// convertido y premultiplicado), con los píxeles `pendientes` ya escritos encima.
fn textura_esperada(imagen: &Image, pendientes: &[(u32, u32, Rgba)]) -> Vec<Color32> {
    let mut texels: Vec<Color32> = to_rgba8(imagen)
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    for &(x, y, c) in pendientes {
        texels[(y * ANCHO + x) as usize] = Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
    }
    texels
}

#[test]
fn paint_con_superposicion_la_ve_en_la_textura_sin_formas_extra_ni_cambiar_la_imagen() {
    // Antes: `paint` con superposición añadía rectángulos y no subía la textura. Ahora la
    // superposición va por `set_overlay`: lo que se conserva es que es visible mientras dura el
    // trazo, que la imagen (el documento) no cambia y que al quitarla todo vuelve a lo de antes.
    let mut s = Sesion::nueva();
    let texture = s.banco.canvas.texture_id().expect("la textura está subida");
    let original = imagen_de_prueba();
    assert_eq!(
        s.banco.textura().texels,
        textura_esperada(&original, &[]),
        "antes del trazo, la textura es el documento"
    );

    let mut sin = s.banco.fotograma(vec![], vec![], None);
    let (formas_sin, vertices_sin) = formas_y_vertices(&s.banco.ctx, &mut sin.output);
    assert!(
        sin.output.textures_delta.set.is_empty(),
        "en reposo, nada se sube"
    );
    sin.output.textures_delta.clear();

    // Dos píxeles opacos y uno semitransparente (que además lleva el fondo debajo).
    let overlay = vec![
        (1, 1, Rgba::opaque(255, 0, 0)),
        (2, 1, Rgba::opaque(255, 0, 0)),
        (3, 1, Rgba::new(255, 0, 0, 128)),
    ];
    let mut con = s.banco.fotograma(vec![], overlay.clone(), None);
    let (formas_con, vertices_con) = formas_y_vertices(&s.banco.ctx, &mut con.output);

    // Ninguna forma extra: ni rectángulos ni mallas de superposición.
    assert_eq!(
        (formas_con, vertices_con),
        (formas_sin, vertices_sin),
        "la superposición no añade formas"
    );
    // Se sube solo por bloques (deltas parciales) y solo la textura del lienzo.
    assert!(
        con.output
            .textures_delta
            .set
            .keys()
            .all(|id| *id == texture),
        "solo se sube la textura del lienzo"
    );
    let deltas = con
        .output
        .textures_delta
        .set
        .get(&texture)
        .expect("el fotograma en que cambia la superposición sube sus bloques");
    for delta in deltas {
        let ImageData::Color(color) = &delta.image;
        let [x0, y0] = delta.pos.expect("un delta parcial, no la textura entera");
        assert!(
            x0 + color.size[0] <= ANCHO as usize && y0 + color.size[1] <= ALTO as usize,
            "el delta {:?} en {:?} cabe en la baldosa",
            color.size,
            (x0, y0)
        );
    }
    con.output.textures_delta.clear();

    // La superposición se ve: la textura compuesta lleva los píxeles pendientes (premultiplicados)
    // y el resto de la imagen es el documento.
    assert_eq!(
        s.banco.textura().texels,
        textura_esperada(&original, &overlay)
    );
    let texel = |x: usize, y: usize| s.banco.textura().texel(x, y).expect("dentro");
    assert_eq!(texel(1, 1), Color32::from_rgba_unmultiplied(255, 0, 0, 255));
    assert_eq!(texel(3, 1), Color32::from_rgba_unmultiplied(255, 0, 0, 128));
    assert_eq!(texel(0, 0), Color32::from_rgb(255, 0, 0), "el resto sigue");
    assert_eq!(texel(63, 47), Color32::from_rgb(0, 0, 255));
    // La imagen no cambia: la superposición no toca el documento.
    assert_eq!(s.banco.imagen, original);
    // La vista y la información son las mismas con y sin superposición.
    assert_eq!(con.info, sin.info);

    // El mismo trazo en el fotograma siguiente: no hay píxeles nuevos y no se sube nada.
    let mut igual = s.banco.fotograma(vec![], overlay, None);
    assert!(
        igual.output.textures_delta.set.is_empty(),
        "solo se sube en el fotograma en que cambia"
    );
    igual.output.textures_delta.clear();

    // Sin superposición vuelve a lo de antes: las mismas formas y la textura es el documento.
    let mut otra = s.banco.fotograma(vec![], vec![], None);
    let (formas_otra, vertices_otra) = formas_y_vertices(&s.banco.ctx, &mut otra.output);
    otra.output.textures_delta.clear();
    assert_eq!((formas_otra, vertices_otra), (formas_sin, vertices_sin));
    assert_eq!(s.banco.textura().texels, textura_esperada(&original, &[]));
    assert_eq!(s.banco.imagen, original);
}

#[test]
fn refresh_conserva_la_vista_y_deja_la_textura_al_dia() {
    let mut banco = Banco::nuevo();
    let mut imagen = imagen_de_prueba();
    banco.canvas.set_image(&imagen);
    banco.paso(vec![]);
    let texture = banco.canvas.texture_id().expect("la textura está subida");

    // Un zoom previo, para que la vista no sea la de ajuste.
    banco.paso(vec![mover(pos2(300.0, 200.0))]);
    let f = banco.paso(vec![rueda(true)]);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(13));
    let antes = f.info;

    imagen
        .set_pixel(5, 5, Pixel::Rgba(Rgba::opaque(0, 255, 0)))
        .expect("dentro");
    banco.imagen = imagen.clone();
    banco.canvas.refresh(&banco.ctx, &imagen);

    // El fotograma siguiente lleva el contenido nuevo de la misma textura.
    let mut f = banco.fotograma(vec![], vec![], None);
    assert_eq!(banco.canvas.texture_id(), Some(texture), "misma textura");
    assert_eq!(f.info.image_rect, antes.image_rect, "la vista se conserva");
    assert_eq!(f.info.zoom, antes.zoom);
    let deltas = f
        .output
        .textures_delta
        .set
        .get(&texture)
        .expect("la textura se actualiza");
    let delta = deltas.iter().last().expect("un delta");
    assert!(delta.is_whole(), "se regenera la textura completa");
    let ImageData::Color(color) = &delta.image;
    assert_eq!(color.size, [ANCHO as usize, ALTO as usize]);
    let en = |x: usize, y: usize| color.pixels[y * ANCHO as usize + x];
    assert_eq!(en(5, 5), Color32::from_rgb(0, 255, 0), "el píxel nuevo");
    assert_eq!(en(0, 0), Color32::from_rgb(255, 0, 0), "el resto sigue");
    assert_eq!(en(63, 47), Color32::from_rgb(0, 0, 255));
    f.output.textures_delta.clear();
    assert_eq!(
        banco.textura().texels,
        textura_esperada(&imagen, &[]),
        "la textura compuesta es el documento"
    );

    // Y los fotogramas posteriores ya no la vuelven a subir.
    let mut f = banco.fotograma(vec![], vec![], None);
    assert!(!f.output.textures_delta.set.contains_key(&texture));
    assert_eq!(f.info.image_rect, antes.image_rect);
    f.output.textures_delta.clear();
}

#[test]
fn refresh_con_otro_tamano_se_comporta_como_set_image() {
    let mut banco = Banco::nuevo();
    banco.canvas.set_image(&imagen_de_prueba());
    banco.paso(vec![]);
    banco.paso(vec![mover(pos2(300.0, 200.0))]);
    let f = banco.paso(vec![rueda(true)]);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(13));

    // 32×24 en 800×600: el ajuste es ×25 y la vista vuelve a él.
    let pequena = Image::new(32, 24, Mode::Rgba).expect("imagen válida");
    banco.imagen = pequena.clone();
    banco.canvas.refresh(&banco.ctx, &pequena);
    let f = banco.paso(vec![]);
    assert!(banco.canvas.texture_id().is_some());
    assert_eq!(f.info.error, None);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(25));
    let rect = f.info.image_rect.expect("hay imagen");
    assert!((rect.width() - 800.0).abs() < 1e-3 && (rect.height() - 600.0).abs() < 1e-3);
}

/// Fila 7 (I.2b): `Espacio` sin gesto, repetido, con un desplazamiento en curso o con un campo de
/// texto enfocado no cancela nada. (El diálogo no es del lienzo: `Espacio` sigue activando el botón
/// enfocado o marcando la casilla en `new_image/dialog_tests.rs` y `convert_mode/dialog_tests.rs`,
/// que no cambian, y un diálogo no puede coexistir con un gesto.)
mod espacio {
    use super::*;

    fn escapes(entradas: &[(CanvasInput, Role)]) -> usize {
        entradas
            .iter()
            .filter(|(e, _)| *e == CanvasInput::Escape)
            .count()
    }

    #[test]
    fn fila_7_sin_gesto_espacio_no_emite_nada_y_sigue_siendo_la_mano() {
        let mut s = Sesion::nueva();
        let p = s.pos(10, 5);
        s.paso(vec![mover(p)]);
        let (entradas, _) = s.banco.paso_roles(vec![espacio(true)]);
        assert!(entradas.is_empty(), "{entradas:?}");
        // Sigue siendo la tecla de la mano: con ella abajo, el izquierdo desplaza.
        let antes = s.rect_actual();
        s.paso(vec![izquierdo(p, true)]);
        s.paso(vec![mover(p + vec2(10.0, 5.0))]);
        s.paso(vec![mover(p + vec2(20.0, 10.0))]);
        s.paso(vec![izquierdo(p + vec2(20.0, 10.0), false)]);
        assert_eq!(s.eventos, NADA);
        assert!(s.rect_actual().min.x > antes.min.x);
    }

    #[test]
    fn fila_7_un_desplazamiento_en_curso_no_se_cancela_con_espacio() {
        let mut s = Sesion::nueva();
        let p = s.pos(10, 5);
        s.paso(vec![mover(p)]);
        s.paso(vec![boton(p, PointerButton::Middle, true)]);
        s.paso(vec![mover(p + vec2(10.0, 5.0))]);
        let a_mitad = s.rect_actual();
        let (entradas, _) = s.banco.paso_roles(vec![espacio(true)]);
        assert_eq!(escapes(&entradas), 0);
        s.paso(vec![mover(p + vec2(20.0, 10.0))]);
        assert!(
            s.rect_actual().min.x > a_mitad.min.x,
            "el desplazamiento continúa"
        );
        s.paso(vec![boton(p, PointerButton::Middle, false)]);
    }

    #[test]
    fn fila_7_con_un_campo_de_texto_enfocado_espacio_no_cancela_el_trazo() {
        fn campo(ui: &mut Ui) {
            egui::Area::new(egui::Id::unique("campo-de-prueba"))
                .fixed_pos(pos2(700.0, 550.0))
                .show(ui.ctx(), |ui| {
                    let mut texto = String::new();
                    let r = ui.add(TextEdit::singleline(&mut texto).id(egui::Id::unique("campo")));
                    r.request_focus();
                });
        }
        let mut s = Sesion::nueva();
        let (p0, p1) = (s.pos(10, 5), s.pos(12, 5));
        s.paso(vec![mover(p0)]);
        s.paso(vec![izquierdo(p0, true)]);
        s.banco.paso_roles_con(vec![mover(p1)], Some(campo));
        s.banco.paso_roles_con(vec![], Some(campo));
        assert!(s.banco.ctx.text_edit_focused(), "el campo tiene el foco");
        let (entradas, _) = s.banco.paso_roles_con(vec![espacio(true)], Some(campo));
        assert_eq!(
            escapes(&entradas),
            0,
            "con el foco en el campo, Espacio no cancela"
        );
        let (entradas, _) = s.banco.paso_roles_con(vec![mover(p0)], Some(campo));
        assert!(
            entradas
                .iter()
                .any(|(e, _)| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeMove(_)))),
            "el trazo sigue"
        );
    }
}
