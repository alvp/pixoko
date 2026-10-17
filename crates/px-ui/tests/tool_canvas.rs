//! Tests de `Canvas` y `ToolState` juntos, con el banco de egui sin ventana
//! (specs/hito-2-linea-relleno-app.md, «Casos límite» filas 20 y 21 y la segunda parte de la 17).
//!
//! El banco es el de `tests/pencil.rs` (un `egui::Context` con `RawInput` sintético, copiado y no
//! modificado): `Canvas::interact` y `Canvas::paint` como hará la app. Cada cambio de estado del
//! botón va en su propio fotograma. Las posiciones se calculan con el rectángulo de la imagen del
//! primer fotograma, en el centro de cada píxel: 800×600 con 64×48 da zoom ×12.
//!
//! `Sesion` hace de `px-app`: pasa los eventos de `interact` a `ToolState::canvas_event` en el
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
const ANCHO: u32 = 64;
const ALTO: u32 = 48;

fn rojo() -> Pixel {
    Pixel::Rgba(Rgba::opaque(255, 0, 0))
}

/// Imagen RGBA de 64×48 vacía.
fn imagen_rgba() -> Image {
    Image::new(ANCHO, ALTO, Mode::Rgba).expect("imagen válida")
}

/// Imagen indexada de 64×48 con una paleta de 3 entradas: la ranura 5 no tiene color.
fn imagen_indexada() -> Image {
    let paleta = Palette::from_colors([
        Rgba::opaque(10, 10, 10),
        Rgba::opaque(20, 20, 20),
        Rgba::opaque(30, 30, 30),
    ])
    .expect("paleta válida");
    Image::new_indexed(ANCHO, ALTO, paleta).expect("imagen válida")
}

struct Banco {
    ctx: Context,
    canvas: Canvas,
    /// `interact_ordered` (con `Escape`) o `interact` (sin `Escape`).
    ordenado: bool,
}

struct Fotograma {
    output: FullOutput,
    eventos: Vec<CanvasInput>,
    info: CanvasInfo,
}

impl Banco {
    fn nuevo(ordenado: bool) -> Banco {
        Banco {
            ctx: Context::default(),
            canvas: Canvas::new(),
            ordenado,
        }
    }

    /// Ejecuta un fotograma: `antes` (si lo hay) corre dentro del fotograma, antes de `interact`;
    /// luego `interact_ordered` (o `interact`), y `paint` sin superposición. No limpia los cambios de textura.
    fn fotograma(&mut self, events: Vec<Event>, antes: Option<fn(&mut Ui)>) -> Fotograma {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events,
            ..RawInput::default()
        };
        let mut resultado = None;
        let canvas = &mut self.canvas;
        let ordenado = self.ordenado;
        let output = self.ctx.run_ui(input, |ui| {
            if let Some(antes) = antes {
                antes(ui);
            }
            let eventos = if ordenado {
                canvas.interact_ordered(ui)
            } else {
                canvas
                    .interact(ui)
                    .into_iter()
                    .map(CanvasInput::Stroke)
                    .collect()
            };
            let info = canvas.paint(ui, std::iter::empty());
            resultado = Some((eventos, info));
        });
        let (eventos, info) = resultado.expect("el lienzo se dibuja");
        Fotograma {
            output,
            eventos,
            info,
        }
    }

    /// Un fotograma con los cambios de textura ya limpiados (egui entra en pánico si se sueltan
    /// sin aplicar).
    fn paso(&mut self, events: Vec<Event>) -> Fotograma {
        let mut f = self.fotograma(events, None);
        f.output.textures_delta.clear();
        f
    }
}

// Entradas sintéticas.

fn mover(pos: Pos2) -> Event {
    Event::PointerMoved(pos)
}

fn izquierdo(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn tecla(key: Key, modifiers: Modifiers, pressed: bool) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers,
    }
}

fn escape() -> Event {
    tecla(Key::Escape, Modifiers::NONE, true)
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
const NADA: [CanvasInput; 0] = [];

fn begin(x: i32, y: i32) -> CanvasInput {
    CanvasInput::Stroke(CanvasEvent::StrokeBegin((x, y)))
}

fn paso_a(x: i32, y: i32) -> CanvasInput {
    CanvasInput::Stroke(CanvasEvent::StrokeMove((x, y)))
}

fn fin() -> CanvasInput {
    CanvasInput::Stroke(CanvasEvent::StrokeEnd)
}

/// Los eventos de trazo de una lista ordenada, sin los `Escape`: lo que da `Canvas::interact`.
fn solo_trazo(entradas: &[CanvasInput]) -> Vec<CanvasEvent> {
    entradas
        .iter()
        .filter_map(|e| match e {
            CanvasInput::Stroke(evento) => Some(*evento),
            CanvasInput::Escape => None,
        })
        .collect()
}

/// El banco con la imagen subida y el primer fotograma ya ejecutado, más el documento y la
/// herramienta que reciben los eventos.
struct Sesion {
    banco: Banco,
    /// Un segundo lienzo que recibe los mismos fotogramas pero con `interact`: no emite `Escape`.
    paralelo: Banco,
    /// Lo que dio `interact` en el último fotograma.
    ultimo_interact: Vec<CanvasEvent>,
    rect: Rect,
    zoom: i64,
    /// Todos los eventos del lienzo (`interact_ordered`), en orden.
    eventos: Vec<CanvasInput>,
    /// El resultado de `canvas_event` para cada evento de `eventos`.
    resultados: Vec<bool>,
    doc: Document,
    id: ImageId,
    tool: ToolState,
    slot: u8,
    original: Image,
}

impl Sesion {
    fn nueva(image: Image, herramienta: Tool, slot: u8) -> Sesion {
        let mut banco = Banco::nuevo(true);
        banco.canvas.set_image(&image);
        let f = banco.paso(vec![]);
        let mut paralelo = Banco::nuevo(false);
        paralelo.canvas.set_image(&image);
        paralelo.paso(vec![]);
        let rect = f.info.image_rect.expect("hay imagen");
        let zoom = f.info.zoom.scale(1);
        assert_eq!(zoom, 12, "800×600 con 64×48 da zoom ×12");
        let doc = Document::new(image.clone());
        let id = doc.main_image();
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(herramienta));
        Sesion {
            banco,
            paralelo,
            ultimo_interact: Vec::new(),
            rect,
            zoom,
            eventos: Vec::new(),
            resultados: Vec::new(),
            doc,
            id,
            tool,
            slot,
            original: image,
        }
    }

    /// Centro del píxel `(x, y)` con la vista del primer fotograma.
    fn pos(&self, x: i32, y: i32) -> Pos2 {
        centro(self.rect, self.zoom, x, y)
    }

    /// Pasa los eventos del fotograma a la herramienta, en orden.
    fn alimentar(&mut self, eventos: &[CanvasInput]) {
        for &evento in eventos {
            let r = self
                .tool
                .canvas_input(&mut self.doc, self.id, self.slot, evento)
                .unwrap_or_else(|e| panic!("{evento:?} dio el error {e:?}"));
            self.eventos.push(evento);
            self.resultados.push(r);
        }
    }

    /// Ejecuta un fotograma y devuelve los eventos de este fotograma.
    fn paso(&mut self, events: Vec<Event>) -> Vec<CanvasInput> {
        let g = self.paralelo.paso(events.clone());
        let f = self.banco.paso(events);
        self.comparar_con_interact(&f.eventos, &g.eventos);
        self.alimentar(&f.eventos);
        f.eventos
    }

    /// `interact` da lo mismo que `interact_ordered` sin los `Escape`, en todos los fotogramas.
    fn comparar_con_interact(&mut self, ordenados: &[CanvasInput], de_interact: &[CanvasInput]) {
        self.ultimo_interact = solo_trazo(de_interact);
        assert_eq!(
            de_interact.len(),
            self.ultimo_interact.len(),
            "interact no emite Escape"
        );
        assert_eq!(solo_trazo(ordenados), self.ultimo_interact);
    }

    /// Como `paso`, pero con una función que corre dentro del fotograma antes de `interact`.
    fn paso_con(&mut self, events: Vec<Event>, antes: fn(&mut Ui)) -> Vec<CanvasInput> {
        let mut g = self.paralelo.fotograma(events.clone(), Some(antes));
        g.output.textures_delta.clear();
        let mut f = self.banco.fotograma(events, Some(antes));
        f.output.textures_delta.clear();
        self.comparar_con_interact(&f.eventos, &g.eventos);
        self.alimentar(&f.eventos);
        f.eventos
    }

    fn imagen(&self) -> &Image {
        self.doc.image(self.id).expect("hay imagen")
    }

    /// Píxeles que difieren de la imagen original.
    fn cambiados(&self) -> BTreeSet<(u32, u32)> {
        let imagen = self.imagen();
        (0..ALTO)
            .flat_map(|y| (0..ANCHO).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                imagen.get_pixel(x, y).unwrap() != self.original.get_pixel(x, y).unwrap()
            })
            .collect()
    }

    /// Los píxeles de la fila `y` de `x0` a `x1`, incluidos.
    fn fila(y: u32, x0: u32, x1: u32) -> BTreeSet<(u32, u32)> {
        (x0..=x1).map(|x| (x, y)).collect()
    }

    fn sin_cambios(&self) -> bool {
        self.imagen() == &self.original && !self.doc.can_undo()
    }

    /// Cuántos `canvas_event` dieron `Ok(true)`.
    fn verdaderos(&self) -> usize {
        self.resultados.iter().filter(|&&r| r).count()
    }
}

/// Una línea horizontal en la fila 20 desde el píxel 30 hasta el 40, con el botón abajo.
fn trazo_de_linea_abierto() -> Sesion {
    trazo_abierto(imagen_rgba(), Tool::Line, 3)
}

fn trazo_abierto(image: Image, herramienta: Tool, slot: u8) -> Sesion {
    let mut s = Sesion::nueva(image, herramienta, slot);
    let (a, b) = (s.pos(30, 20), s.pos(40, 20));
    s.paso(vec![mover(a)]);
    assert_eq!(s.paso(vec![izquierdo(a, true)]), [begin(30, 20)]);
    assert_eq!(s.paso(vec![mover(b)]), [paso_a(40, 20)]);
    s
}

/// Tras terminar un gesto no queda ninguno abierto y se puede cambiar de herramienta.
fn comprobar_que_no_queda_nada_abierto(s: &mut Sesion) {
    assert!(!s.tool.busy(), "busy() debe ser falso");
    assert_eq!(s.paso(vec![mover(s.pos(45, 22))]), NADA);
    assert_eq!(s.paso(vec![mover(s.pos(46, 23))]), NADA);
    assert!(!s.tool.busy());
    s.tool.apply_action(ToolAction::Select(Tool::Fill));
    assert_eq!(s.tool.tool(), Tool::Fill, "Select ya cambia de herramienta");
}

// Fila 20.

#[test]
fn fila_20_escape_antes_del_soltado_en_el_mismo_fotograma_cancela_la_linea() {
    let mut s = trazo_de_linea_abierto();
    let ev = s.paso(vec![escape(), izquierdo(s.pos(41, 20), false)]);
    assert_eq!(ev, [CanvasInput::Escape, paso_a(41, 20), fin()]);
    // `interact` da lo mismo que antes, sin el `Escape`.
    assert_eq!(
        s.ultimo_interact,
        [CanvasEvent::StrokeMove((41, 20)), CanvasEvent::StrokeEnd]
    );
    assert!(
        s.sin_cambios(),
        "la línea se cancela: ni imagen ni historia"
    );
    assert_eq!(s.verdaderos(), 0);
    comprobar_que_no_queda_nada_abierto(&mut s);
}

#[test]
fn fila_20_escape_despues_del_soltado_en_el_mismo_fotograma_deja_la_linea() {
    let mut s = trazo_de_linea_abierto();
    let ev = s.paso(vec![izquierdo(s.pos(41, 20), false), escape()]);
    assert_eq!(ev, [paso_a(41, 20), fin(), CanvasInput::Escape]);
    assert_eq!(
        s.ultimo_interact,
        [CanvasEvent::StrokeMove((41, 20)), CanvasEvent::StrokeEnd],
        "interact: lo mismo que antes, sin el Escape"
    );
    // El soltado ya aplicó la línea hasta el último destino (30 a 41) y `Escape` no la deshace.
    assert_eq!(s.cambiados(), Sesion::fila(20, 30, 41));
    assert_eq!(s.imagen().get_pixel(41, 20), Ok(rojo()));
    assert!(s.doc.can_undo());
    assert_eq!(s.verdaderos(), 1);
    assert_eq!(
        s.resultados.last(),
        Some(&false),
        "Escape da Ok(false) y no hace nada"
    );
    comprobar_que_no_queda_nada_abierto(&mut s);
}

#[test]
fn fila_20_shift_escape_no_emite_evento_y_el_trazo_sigue() {
    let mut s = trazo_de_linea_abierto();
    let ev = s.paso(vec![tecla(Key::Escape, Modifiers::SHIFT, true)]);
    assert_eq!(ev, NADA, "Escape con Shift no es un Escape");
    assert_eq!(s.ultimo_interact, []);
    assert!(s.tool.busy());
    // El trazo sigue vivo y se aplica al soltar.
    s.paso(vec![izquierdo(s.pos(41, 20), false)]);
    assert_eq!(s.cambiados(), Sesion::fila(20, 30, 41));
    comprobar_que_no_queda_nada_abierto(&mut s);
}

#[test]
fn fila_20_escape_sin_trazo_emite_solo_escape_y_la_herramienta_no_hace_nada() {
    for herramienta in [Tool::Pencil, Tool::Line, Tool::Fill] {
        let mut s = Sesion::nueva(imagen_rgba(), herramienta, 3);
        let ev = s.paso(vec![escape()]);
        assert_eq!(ev, [CanvasInput::Escape], "{herramienta:?}");
        assert_eq!(
            s.ultimo_interact,
            [],
            "{herramienta:?}: interact no emite Escape"
        );
        assert_eq!(
            s.resultados,
            [false],
            "{herramienta:?}: ToolState no hace nada"
        );
        assert!(s.sin_cambios());
        assert!(!s.tool.busy());
        assert_eq!(s.tool.tool(), herramienta);
    }
}

// Fila 20, ampliaciones que no están en la tabla (sacadas de «`Escape` sin modificadores» y de que
// `Canvas::interact` solo mira las teclas pulsadas).

#[test]
fn escape_con_cualquier_modificador_no_emite_evento() {
    let con_ctrl_y_command = Modifiers {
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
        con_ctrl_y_command,
    ] {
        let mut s = Sesion::nueva(imagen_rgba(), Tool::Line, 3);
        let ev = s.paso(vec![tecla(Key::Escape, modifiers, true)]);
        assert_eq!(ev, NADA, "Escape con {modifiers:?}");
    }
}

#[test]
fn soltar_la_tecla_escape_no_emite_evento() {
    let mut s = trazo_de_linea_abierto();
    let ev = s.paso(vec![tecla(Key::Escape, Modifiers::NONE, false)]);
    assert_eq!(ev, NADA, "solo la pulsación es un Escape");
    assert!(s.tool.busy());
}

// Fila 21.

#[test]
fn fila_21_perder_el_foco_aplica_la_linea_y_no_deja_el_gesto_colgado() {
    let mut s = trazo_de_linea_abierto();
    assert_eq!(s.paso(vec![Event::WindowFocused(false)]), [fin()]);
    assert!(!s.tool.busy());
    assert_eq!(
        s.cambiados(),
        Sesion::fila(20, 30, 40),
        "hasta el último destino"
    );
    assert!(s.doc.can_undo());
    comprobar_que_no_queda_nada_abierto(&mut s);
}

#[test]
fn fila_21_pointer_gone_y_soltar_fuera_aplica_la_linea_hasta_el_punto_de_soltado() {
    let mut s = trazo_de_linea_abierto();
    assert_eq!(
        s.paso(vec![Event::PointerGone]),
        NADA,
        "PointerGone no termina"
    );
    assert!(s.tool.busy());
    assert_eq!(s.paso(vec![]), NADA);
    assert!(s.tool.busy());
    // Se suelta fuera de la ventana: píxel (73, 20), con x = 898 > 800.
    let fuera = s.pos(73, 20);
    assert!(fuera.x > SCREEN.x);
    assert_eq!(
        s.paso(vec![izquierdo(fuera, false)]),
        [paso_a(73, 20), fin()]
    );
    assert!(!s.tool.busy());
    // De 30 al borde derecho (63): 34 píxeles.
    assert_eq!(s.cambiados(), Sesion::fila(20, 30, 63));
    assert!(s.doc.can_undo());
    comprobar_que_no_queda_nada_abierto(&mut s);
}

#[test]
fn fila_21_un_soltar_perdido_aplica_la_linea_en_el_fotograma_siguiente() {
    let mut s = trazo_de_linea_abierto();
    assert_eq!(
        s.paso_con(vec![], perder_el_boton),
        [fin()],
        "la red de seguridad cierra el trazo"
    );
    assert!(!s.tool.busy());
    assert_eq!(s.cambiados(), Sesion::fila(20, 30, 40));
    assert!(s.doc.can_undo());
    assert_eq!(s.paso(vec![]), NADA, "ni un segundo End");
    comprobar_que_no_queda_nada_abierto(&mut s);
}

/// Cómo termina un gesto en la fila 21, con el botón ya abajo.
#[derive(Clone, Copy, Debug)]
enum Final {
    Soltar,
    PerderElFoco,
    SoltarPerdido,
    PointerGoneYSoltarFuera,
}

const FINALES: [Final; 4] = [
    Final::Soltar,
    Final::PerderElFoco,
    Final::SoltarPerdido,
    Final::PointerGoneYSoltarFuera,
];

fn terminar(s: &mut Sesion, final_: Final) {
    match final_ {
        Final::Soltar => {
            let p = s.pos(41, 20);
            s.paso(vec![izquierdo(p, false)]);
        }
        Final::PerderElFoco => {
            s.paso(vec![Event::WindowFocused(false)]);
        }
        Final::SoltarPerdido => {
            s.paso_con(vec![], perder_el_boton);
        }
        Final::PointerGoneYSoltarFuera => {
            s.paso(vec![Event::PointerGone]);
            let p = s.pos(73, 20);
            s.paso(vec![izquierdo(p, false)]);
        }
    }
}

#[test]
fn fila_21_una_ranura_sin_color_no_deja_los_atajos_bloqueados() {
    // Paleta de 3 entradas: la ranura 5 no tiene color. Pulsar y soltar; lo mismo con las otras
    // formas de terminar el gesto y con las tres herramientas.
    for herramienta in [Tool::Pencil, Tool::Line, Tool::Fill] {
        for final_ in FINALES {
            let mut s = Sesion::nueva(imagen_indexada(), herramienta, 5);
            let a = s.pos(30, 20);
            s.paso(vec![mover(a)]);
            assert_eq!(s.paso(vec![izquierdo(a, true)]), [begin(30, 20)]);
            assert!(
                s.tool.busy(),
                "{herramienta:?}/{final_:?}: ocupado tras Begin"
            );
            // Con el botón abajo los atajos no valen.
            s.tool.apply_action(ToolAction::Select(Tool::Fill));
            assert_eq!(s.tool.tool(), herramienta);

            terminar(&mut s, final_);
            assert!(
                !s.tool.busy(),
                "{herramienta:?}/{final_:?}: busy() debe volver a falso"
            );
            assert!(s.sin_cambios(), "{herramienta:?}/{final_:?}: sin cambios");
            assert_eq!(s.verdaderos(), 0);
            s.tool.apply_action(ToolAction::Select(Tool::Fill));
            assert_eq!(
                s.tool.tool(),
                Tool::Fill,
                "{herramienta:?}/{final_:?}: Select ya cambia de herramienta"
            );
        }
    }
}

#[test]
fn fila_21_una_ranura_sin_color_pulsar_y_soltar_sin_mover() {
    // El caso literal de la fila: pulsar y soltar, con la herramienta de línea.
    let mut s = Sesion::nueva(imagen_indexada(), Tool::Line, 5);
    let p = s.pos(10, 10);
    s.paso(vec![mover(p)]);
    s.paso(vec![izquierdo(p, true)]);
    assert!(s.tool.busy());
    s.paso(vec![izquierdo(p, false)]);
    assert!(!s.tool.busy());
    assert!(s.sin_cambios());
    s.tool.apply_action(ToolAction::Select(Tool::Fill));
    assert_eq!(s.tool.tool(), Tool::Fill);
}

// Fila 17, segunda parte.

#[test]
fn fila_17_con_el_boton_abajo_30_fotogramas_el_relleno_no_se_repite() {
    let mut s = Sesion::nueva(imagen_rgba(), Tool::Fill, 3);
    let p = s.pos(30, 20);
    s.paso(vec![mover(p)]);
    assert_eq!(
        s.paso(vec![izquierdo(p, true)]),
        [begin(30, 20)],
        "Canvas emite solo Begin"
    );
    for fotograma in 0..30 {
        assert_eq!(s.paso(vec![]), NADA, "fotograma {fotograma} sin eventos");
    }
    assert_eq!(s.eventos, [begin(30, 20)], "Canvas solo emitió Begin");
    assert_eq!(s.resultados, [true], "un solo Ok(true)");
    assert!(s.tool.busy(), "ocupado hasta soltar");
    // El relleno ya está hecho (todo el lienzo, que era uniforme).
    assert_eq!(s.cambiados().len(), (ANCHO * ALTO) as usize);

    // Soltar no rellena otra vez.
    s.paso(vec![izquierdo(p, false)]);
    assert!(!s.tool.busy());
    assert_eq!(s.verdaderos(), 1, "ningún relleno más");
    assert_eq!(s.doc.undo(), Ok(()));
    assert!(s.sin_cambios(), "un solo paso");
}
