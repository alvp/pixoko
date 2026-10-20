//! Tests del cuentagotas y del gesto con las dos herramientas nuevas, sin ventana
//! (specs/hito-2-cuentagotas-borrador.md, «Casos límite» filas 5, 6, 7 y 8 en lo que toca a
//! `px-ui`, y propiedades 2 (parte de `px-ui`) y 4).
//!
//! Los eventos se dan a mano a `ToolState::canvas_event_with`; el cuentagotas no necesita color
//! activo, así que se le pasa `None`. Lo que elige sale por `ToolState::take_events`.
//!
//! La propiedad 4 amplía la propiedad 6 de la unidad 7 (`tests/tool.rs`) a las cinco herramientas
//! sin tocarla. El oráculo de la propiedad 2 mira `Image::get_pixel` y no usa `picked_at`, que
//! se compara aparte.
//!
//! El «banco de egui» de la fila 8 es un `Context` con `RawInput` sintético y un campo de texto
//! enfocado, para ver `gate_keys` con el `text_edit_focused()` real.

use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, TextEdit, vec2};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Command, Document, Image, ImageId, Mode, Palette, Pixel, PixelEdit, Rgba};
use px_ui::canvas::{CanvasEvent, CanvasInput};
use px_ui::color::pixel_for_slot;
use px_ui::pick::{Dropper, EraseError, Picked, eraser_pixel, picked_at};
use px_ui::shortcuts::gate_keys;
use px_ui::tool::{Tool, ToolAction, ToolEvent, ToolState, tool_action_for_key};

const MIN: i32 = i32::MIN;
const MAX: i32 = i32::MAX;
const MODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];
const TODAS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];

// Apoyo común.

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

fn begin(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeBegin((x, y))
}

fn mov(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeMove((x, y))
}

fn fin() -> CanvasEvent {
    CanvasEvent::StrokeEnd
}

fn picked(picked: Picked) -> ToolEvent {
    ToolEvent::Picked(picked)
}

fn color(r: u8, g: u8, b: u8, a: u8) -> ToolEvent {
    ToolEvent::Picked(Picked::Color(rgba(r, g, b, a)))
}

/// Imagen RGBA de 8×8 en la que cada píxel tiene un color propio: `(10x, 10y, 5, 255)`.
fn rgba_unica() -> Image {
    let mut image = Image::new(8, 8, Mode::Rgba).expect("imagen válida");
    for y in 0..8u8 {
        for x in 0..8u8 {
            image
                .set_pixel(
                    u32::from(x),
                    u32::from(y),
                    Pixel::Rgba(Rgba::opaque(10 * x, 10 * y, 5)),
                )
                .expect("dentro");
        }
    }
    image
}

/// El color de `(x, y)` en `rgba_unica`, como lo elige el cuentagotas.
fn de_rgba_unica(x: u8, y: u8) -> ToolEvent {
    color(10 * x, 10 * y, 5, 255)
}

/// Imagen RGBA de 8×8 azul opaca.
fn rgba_azul() -> Image {
    let mut image = Image::new(8, 8, Mode::Rgba).expect("imagen válida");
    for y in 0..8 {
        for x in 0..8 {
            image
                .set_pixel(x, y, Pixel::Rgba(Rgba::opaque(0, 0, 255)))
                .expect("dentro");
        }
    }
    image
}

/// Indexada de 8×8 con paleta de 6; la fila 0 lleva los índices `0…5` en `x = 0…5`.
fn indexada_con_fila_0() -> Image {
    let paleta =
        Palette::from_colors((0..6).map(|i| Rgba::opaque(40 * i, 10, 10))).expect("paleta válida");
    let mut image = Image::new_indexed(8, 8, paleta).expect("imagen válida");
    for x in 0..6u8 {
        image
            .set_pixel(u32::from(x), 0, Pixel::Indexed(x))
            .expect("dentro");
    }
    image
}

/// Documento y herramienta juntos.
struct Escena {
    doc: Document,
    id: ImageId,
    tool: ToolState,
}

impl Escena {
    fn nueva(image: Image, herramienta: Tool) -> Escena {
        let doc = Document::new(image);
        let id = doc.main_image();
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(herramienta));
        assert_eq!(tool.tool(), herramienta);
        Escena { doc, id, tool }
    }

    fn cuentagotas(image: Image) -> Escena {
        Escena::nueva(image, Tool::Dropper)
    }

    fn imagen(&self) -> &Image {
        self.doc.image(self.id).expect("hay imagen")
    }

    /// Da el evento a la herramienta sin color activo; un `Err` aquí es un fallo del test.
    fn ev(&mut self, evento: CanvasEvent) -> bool {
        self.tool
            .canvas_event_with(&mut self.doc, self.id, None, evento)
            .unwrap_or_else(|e| panic!("{evento:?} dio el error {e:?}"))
    }

    fn escape(&mut self) -> bool {
        self.tool
            .canvas_input_with(&mut self.doc, self.id, None, CanvasInput::Escape)
            .unwrap_or_else(|e| panic!("Escape dio el error {e:?}"))
    }

    fn eventos(&mut self) -> Vec<ToolEvent> {
        self.tool.take_events()
    }

    fn preview(&self) -> Vec<PixelEdit> {
        self.tool.preview().collect()
    }

    /// Imagen, `state_id`, `can_undo`, `can_redo` e `history_bytes`.
    fn estado(&self) -> (Image, u64, bool, bool, usize) {
        estado(&self.doc, self.id)
    }
}

fn estado(doc: &Document, id: ImageId) -> (Image, u64, bool, bool, usize) {
    (
        doc.image(id).expect("hay imagen").clone(),
        doc.state_id(),
        doc.can_undo(),
        doc.can_redo(),
        doc.history_bytes(),
    )
}

/// Un paso previo en la historia: `0` nada, `1` un paso, `2` un paso deshecho (hay rehacer).
fn paso_previo(doc: &mut Document, id: ImageId, variante: u8) {
    if variante == 0 {
        return;
    }
    let image = doc.image(id).expect("hay imagen");
    let actual = image.get_pixel(0, 0).unwrap();
    let otro = pool(image.mode())
        .into_iter()
        .find(|&p| p != actual)
        .expect("el fondo tiene al menos dos valores");
    let comando = Command::SetPixels {
        image: id,
        pixels: vec![PixelEdit {
            x: 0,
            y: 0,
            pixel: otro,
        }],
    };
    assert_eq!(doc.apply(comando), Ok(true));
    if variante == 2 {
        assert_eq!(doc.undo(), Ok(()));
    }
}

// Fila 5.

/// La secuencia de la fila 5 y lo que debe dar cada paso: `(evento, eventos de herramienta,
/// busy después)`.
fn secuencia_de_la_fila_5() -> Vec<(CanvasEvent, Vec<ToolEvent>, bool)> {
    vec![
        (begin(3, 0), vec![picked(Picked::Entry(3))], true),
        (mov(3, 0), vec![], true),
        (mov(4, 0), vec![picked(Picked::Entry(4))], true),
        (mov(4, 0), vec![], true),
        (mov(5, 0), vec![picked(Picked::Entry(5))], true),
        (fin(), vec![], false),
        (mov(1, 0), vec![], false),
    ]
}

#[test]
fn fila_5_cuentagotas_indexado_un_evento_por_pixel_nuevo() {
    for previo in 0u8..=2 {
        let mut es = Escena::cuentagotas(indexada_con_fila_0());
        paso_previo(&mut es.doc, es.id, previo);
        let antes = es.estado();
        assert!(!es.tool.busy());

        for (evento, esperados, ocupado) in secuencia_de_la_fila_5() {
            assert!(!es.ev(evento), "{evento:?}: Ok(false) siempre");
            assert_eq!(es.eventos(), esperados, "{evento:?} (previo {previo})");
            assert_eq!(es.tool.busy(), ocupado, "{evento:?}");
            assert_eq!(es.preview(), vec![], "{evento:?}: sin vista previa");
            assert_eq!(es.estado(), antes, "{evento:?}: el documento no cambia");
        }
    }
}

#[test]
fn fila_5_todos_los_eventos_juntos_en_orden() {
    let mut es = Escena::cuentagotas(indexada_con_fila_0());
    let mut todos = Vec::new();
    for (evento, _, _) in secuencia_de_la_fila_5() {
        es.ev(evento);
        todos.extend(es.eventos());
    }
    assert_eq!(
        todos,
        vec![
            picked(Picked::Entry(3)),
            picked(Picked::Entry(4)),
            picked(Picked::Entry(5)),
        ]
    );
}

#[test]
fn take_events_vacia_la_cola() {
    let mut es = Escena::cuentagotas(indexada_con_fila_0());
    es.ev(begin(3, 0));
    es.ev(mov(4, 0));
    es.ev(mov(5, 0));
    assert_eq!(es.eventos().len(), 3, "los tres a la vez, en orden");
    assert_eq!(es.eventos(), vec![], "la segunda vez no queda nada");
}

#[test]
fn los_eventos_sin_recoger_se_acumulan_en_orden() {
    let mut es = Escena::cuentagotas(indexada_con_fila_0());
    es.ev(begin(1, 0));
    es.ev(mov(2, 0));
    es.ev(fin());
    es.ev(begin(4, 0));
    es.ev(fin());
    assert_eq!(
        es.eventos(),
        vec![
            picked(Picked::Entry(1)),
            picked(Picked::Entry(2)),
            picked(Picked::Entry(4)),
        ]
    );
}

#[test]
fn el_cuentagotas_no_hace_lineas_entre_posiciones() {
    // Cada `Move(q)` elige `q` y nada más: el hueco entre dos posiciones no se recorre.
    let mut es = Escena::cuentagotas(rgba_unica());
    es.ev(begin(0, 0));
    es.ev(mov(5, 0));
    es.ev(mov(5, 4));
    es.ev(fin());
    assert_eq!(
        es.eventos(),
        vec![
            de_rgba_unica(0, 0),
            de_rgba_unica(5, 0),
            de_rgba_unica(5, 4)
        ]
    );
}

#[test]
fn dos_clics_seguidos_en_el_mismo_pixel_eligen_dos_veces() {
    // `Begin(p)` elige `p` siempre; el «último visitado» es de cada gesto.
    let mut es = Escena::cuentagotas(rgba_unica());
    for _ in 0..2 {
        assert!(!es.ev(begin(2, 3)));
        assert!(!es.ev(fin()));
        assert_eq!(es.eventos(), vec![de_rgba_unica(2, 3)]);
    }
}

#[test]
fn el_cuentagotas_no_cambia_el_documento_con_ningun_modo() {
    let mut gris = Image::new(8, 8, Mode::Gray).expect("imagen válida");
    gris.set_pixel(1, 1, Pixel::Gray(50)).expect("dentro");
    let mut gris_alfa = Image::new(8, 8, Mode::GrayAlpha).expect("imagen válida");
    gris_alfa
        .set_pixel(1, 1, Pixel::GrayAlpha { gray: 5, alpha: 0 })
        .expect("dentro");
    for image in [rgba_unica(), indexada_con_fila_0(), gris, gris_alfa] {
        let mut es = Escena::cuentagotas(image);
        paso_previo(&mut es.doc, es.id, 2);
        let antes = es.estado();
        for evento in [begin(1, 1), mov(2, 2), mov(0, 0), fin()] {
            assert!(!es.ev(evento));
            assert_eq!(es.estado(), antes, "{evento:?}");
        }
        assert_eq!(es.eventos().len(), 3);
        assert!(es.doc.can_redo(), "el rehacer sigue disponible");
    }
}

// `Dropper` suelto.

#[test]
fn dropper_visit_da_some_solo_dentro_y_en_un_pixel_distinto_del_ultimo() {
    let image = indexada_con_fila_0();
    let mut dropper = Dropper::new();
    assert_eq!(dropper.visit(&image, (3, 0)), Some(Picked::Entry(3)));
    assert_eq!(dropper.visit(&image, (3, 0)), None, "el mismo píxel");
    assert_eq!(dropper.visit(&image, (4, 0)), Some(Picked::Entry(4)));
    assert_eq!(dropper.visit(&image, (4, 0)), None);
    assert_eq!(dropper.visit(&image, (5, 0)), Some(Picked::Entry(5)));
}

#[test]
fn dropper_visit_fuera_de_la_imagen_no_elige_nada() {
    let image = indexada_con_fila_0();
    let mut dropper = Dropper::new();
    for pos in [(-1, 0), (8, 0), (0, 8), (0, -1), (MIN, MAX), (MAX, MIN)] {
        assert_eq!(dropper.visit(&image, pos), None, "{pos:?}");
    }
    // Y no estropea lo siguiente: un píxel de dentro sigue eligiéndose.
    assert_eq!(dropper.visit(&image, (2, 0)), Some(Picked::Entry(2)));
}

#[test]
fn dropper_visit_coincide_con_picked_at() {
    let image = rgba_unica();
    for y in 0..8u32 {
        for x in 0..8u32 {
            let mut dropper = Dropper::new();
            assert_eq!(
                dropper.visit(&image, (x as i32, y as i32)),
                picked_at(&image, x, y),
                "({x}, {y})"
            );
        }
    }
}

#[test]
fn dropper_visit_un_dropper_nuevo_no_recuerda_nada() {
    let image = indexada_con_fila_0();
    let mut a = Dropper::new();
    assert!(a.visit(&image, (3, 0)).is_some());
    let mut b = Dropper::new();
    assert_eq!(b.visit(&image, (3, 0)), Some(Picked::Entry(3)));
}

// Fila 6.

/// Un clic en `(2, 3)`: `Begin` y `End`. Devuelve lo que eligió.
fn clic_en_2_3(image: Image) -> Vec<ToolEvent> {
    let mut es = Escena::cuentagotas(image);
    assert!(!es.ev(begin(2, 3)));
    assert!(!es.ev(fin()));
    es.eventos()
}

fn con_pixel(mode: Mode, pixel: Pixel) -> Image {
    let mut image = Image::new(8, 8, mode).expect("imagen válida");
    image.set_pixel(2, 3, pixel).expect("dentro");
    image
}

#[test]
fn fila_6_rgba_con_alfa_128() {
    let image = con_pixel(Mode::Rgba, Pixel::Rgba(rgba(10, 20, 30, 128)));
    assert_eq!(clic_en_2_3(image), vec![color(10, 20, 30, 128)]);
}

#[test]
fn fila_6_rgba_con_alfa_0_se_copia_tal_cual() {
    let image = con_pixel(Mode::Rgba, Pixel::Rgba(rgba(10, 20, 30, 0)));
    assert_eq!(clic_en_2_3(image), vec![color(10, 20, 30, 0)]);
}

#[test]
fn fila_6_gris() {
    let image = con_pixel(Mode::Gray, Pixel::Gray(96));
    assert_eq!(clic_en_2_3(image), vec![color(96, 96, 96, 255)]);
}

#[test]
fn fila_6_gris_con_alfa_200() {
    let image = con_pixel(
        Mode::GrayAlpha,
        Pixel::GrayAlpha {
            gray: 7,
            alpha: 200,
        },
    );
    assert_eq!(clic_en_2_3(image), vec![color(7, 7, 7, 200)]);
}

#[test]
fn fila_6_gris_con_alfa_0() {
    let image = con_pixel(Mode::GrayAlpha, Pixel::GrayAlpha { gray: 7, alpha: 0 });
    assert_eq!(clic_en_2_3(image), vec![color(7, 7, 7, 0)]);
}

#[test]
fn fila_6_indexada_elige_la_entrada_tambien_la_transparente() {
    let mut paleta =
        Palette::from_colors((0..4).map(|i| Rgba::opaque(60 * i, 0, 0))).expect("paleta válida");
    paleta.set_transparent_index(Some(2)).expect("existe");
    let mut image = Image::new_indexed(8, 8, paleta).expect("imagen válida");
    image.set_pixel(2, 3, Pixel::Indexed(2)).expect("dentro");
    assert_eq!(clic_en_2_3(image), vec![picked(Picked::Entry(2))]);
}

// Fila 7.

#[test]
fn fila_7_empezar_fuera_no_elige_nada_pero_deja_el_gesto_ocupado() {
    for (x, y) in [(-1, 0), (8, 0), (0, 8), (0, -1), (MIN, MAX), (MAX, MAX)] {
        let mut es = Escena::cuentagotas(rgba_unica());
        let antes = es.estado();
        assert!(!es.ev(begin(x, y)));
        assert!(es.tool.busy(), "({x}, {y}): busy desde el Begin");
        assert_eq!(es.eventos(), vec![], "({x}, {y})");
        assert_eq!(es.preview(), vec![]);
        assert!(!es.ev(fin()));
        assert!(!es.tool.busy(), "({x}, {y}): libre tras el End");
        assert_eq!(es.eventos(), vec![]);
        assert_eq!(es.estado(), antes);
    }
}

#[test]
fn fila_7_un_arrastre_que_empieza_fuera_y_entra_elige_al_entrar() {
    let mut es = Escena::cuentagotas(rgba_unica());
    assert!(!es.ev(begin(-3, 2)));
    assert_eq!(es.eventos(), vec![]);
    assert!(!es.ev(mov(2, 2)));
    assert_eq!(es.eventos(), vec![de_rgba_unica(2, 2)]);
    assert!(es.tool.busy());
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());
    assert_eq!(es.eventos(), vec![]);
}

#[test]
fn fila_7_un_arrastre_que_sale_no_elige_ni_anula_y_al_volver_elige_otro() {
    let mut es = Escena::cuentagotas(rgba_unica());
    let antes = es.estado();
    assert!(!es.ev(begin(2, 2)));
    assert_eq!(es.eventos(), vec![de_rgba_unica(2, 2)]);
    assert!(!es.ev(mov(20, 2)));
    assert_eq!(es.eventos(), vec![], "fuera: ni elige ni anula");
    assert!(!es.ev(mov(3, 2)));
    assert_eq!(es.eventos(), vec![de_rgba_unica(3, 2)]);
    assert!(es.tool.busy());
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());
    assert_eq!(
        es.eventos(),
        vec![],
        "lo elegido se queda: nada más que decir"
    );
    assert_eq!(es.estado(), antes);
}

// Fila 8: gesto y teclado.

#[test]
fn fila_8_durante_un_trazo_del_borrador_select_y_escape_no_hacen_nada() {
    let copia = rgba_azul();
    let mut es = Escena::nueva(copia.clone(), Tool::Eraser);
    assert!(!es.ev(begin(1, 1)));
    assert!(!es.ev(mov(5, 1)));
    let pendiente = es.preview();
    assert_eq!(pendiente.len(), 5);

    es.tool.apply_action(ToolAction::Select(Tool::Pencil));
    assert_eq!(
        es.tool.tool(),
        Tool::Eraser,
        "Select se ignora con el gesto"
    );
    es.tool.apply_action(ToolAction::Select(Tool::Dropper));
    assert_eq!(es.tool.tool(), Tool::Eraser);
    assert!(!es.escape(), "Escape: Ok(false)");
    es.tool.apply_action(ToolAction::Cancel);
    assert!(es.tool.busy());
    assert_eq!(es.preview(), pendiente, "el trazo sigue ahí");
    assert_eq!(es.imagen(), &copia, "y el documento no cambió");

    assert!(es.ev(fin()), "el borrador se aplica al soltar");
    for x in 1..=5 {
        assert_eq!(
            es.imagen().get_pixel(x, 1),
            Ok(Pixel::Rgba(Rgba::TRANSPARENT))
        );
    }
    assert!(es.doc.can_undo());
    assert!(!es.tool.busy());
}

#[test]
fn fila_8_durante_un_arrastre_del_cuentagotas_select_y_escape_no_hacen_nada() {
    let mut es = Escena::cuentagotas(rgba_unica());
    let antes = es.estado();
    assert!(!es.ev(begin(2, 2)));
    assert!(!es.ev(mov(3, 2)));
    assert_eq!(es.eventos(), vec![de_rgba_unica(2, 2), de_rgba_unica(3, 2)]);

    es.tool.apply_action(ToolAction::Select(Tool::Pencil));
    assert_eq!(
        es.tool.tool(),
        Tool::Dropper,
        "Select se ignora con el gesto"
    );
    es.tool.apply_action(ToolAction::Select(Tool::Eraser));
    assert_eq!(es.tool.tool(), Tool::Dropper);
    assert!(!es.escape(), "Escape: Ok(false)");
    es.tool.apply_action(ToolAction::Cancel);
    assert!(es.tool.busy());
    assert_eq!(es.eventos(), vec![], "Escape no elige ni anula nada");

    // El arrastre sigue después de `Escape`, y lo elegido se queda.
    assert!(!es.ev(mov(4, 2)));
    assert_eq!(es.eventos(), vec![de_rgba_unica(4, 2)]);
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());
    assert_eq!(es.estado(), antes);
}

#[test]
fn fila_8_tras_el_end_select_si_cambia_de_herramienta() {
    for herramienta in [Tool::Eraser, Tool::Dropper] {
        let mut es = Escena::nueva(rgba_azul(), herramienta);
        es.ev(begin(1, 1));
        es.ev(mov(2, 1));
        es.ev(fin());
        assert!(!es.tool.busy());
        es.tool.apply_action(ToolAction::Select(Tool::Pencil));
        assert_eq!(es.tool.tool(), Tool::Pencil, "desde {herramienta:?}");
        es.tool.apply_action(ToolAction::Select(Tool::Eraser));
        assert_eq!(es.tool.tool(), Tool::Eraser);
        es.tool.apply_action(ToolAction::Select(Tool::Dropper));
        assert_eq!(es.tool.tool(), Tool::Dropper);
    }
}

#[test]
fn escape_en_reposo_con_las_herramientas_nuevas_no_hace_nada() {
    for herramienta in [Tool::Dropper, Tool::Eraser] {
        let mut es = Escena::nueva(rgba_azul(), herramienta);
        let antes = es.estado();
        assert!(!es.escape());
        assert_eq!(es.tool.tool(), herramienta);
        assert!(!es.tool.busy());
        assert_eq!(es.eventos(), vec![]);
        assert_eq!(es.estado(), antes);
    }
}

#[test]
fn busy_va_de_begin_a_end_con_y_sin_gesto_real_en_las_dos_herramientas() {
    // Con gesto real (RGBA), sin él (el borrador en gris, el cuentagotas fuera de la imagen).
    let casos = [
        (rgba_azul(), Tool::Eraser, (1, 1)),
        (rgba_azul(), Tool::Dropper, (1, 1)),
        (rgba_azul(), Tool::Dropper, (-4, -4)),
        (
            Image::new(8, 8, Mode::Gray).expect("imagen válida"),
            Tool::Eraser,
            (1, 1),
        ),
    ];
    for (image, herramienta, (x, y)) in casos {
        let mut es = Escena::nueva(image, herramienta);
        assert!(!es.tool.busy());
        es.ev(begin(x, y));
        assert!(es.tool.busy(), "{herramienta:?}: tras Begin");
        es.ev(mov(x + 1, y));
        assert!(es.tool.busy(), "{herramienta:?}: tras Move");
        es.ev(fin());
        assert!(!es.tool.busy(), "{herramienta:?}: tras End");
    }
}

fn tecla(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// Lo que haría la app con los eventos que le dejó pasar `gate_keys`.
fn aplica_teclas_de_herramienta(tool: &mut ToolState, eventos: &[Event]) {
    for evento in eventos {
        if let Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = evento
            && let Some(accion) = tool_action_for_key(*key, *modifiers)
        {
            tool.apply_action(accion);
        }
    }
}

fn teclas_coma_y_e() -> Vec<Event> {
    vec![
        tecla(Key::Comma, Modifiers::NONE),
        Event::Text(",".to_string()),
        tecla(Key::E, Modifiers::NONE),
        Event::Text("e".to_string()),
    ]
}

#[test]
fn fila_8_gate_keys_con_foco_de_texto_no_deja_pasar_ni_coma_ni_e() {
    let eventos = teclas_coma_y_e();
    assert!(gate_keys(&eventos, true).is_empty());
    for evento in &eventos {
        assert!(gate_keys(std::slice::from_ref(evento), true).is_empty());
    }
    let mut tool = ToolState::new();
    aplica_teclas_de_herramienta(&mut tool, &gate_keys(&eventos, true));
    assert_eq!(tool.tool(), Tool::Pencil, "la herramienta no cambia");
}

#[test]
fn fila_8_gate_keys_sin_foco_deja_pasar_coma_y_e() {
    let eventos = teclas_coma_y_e();
    assert_eq!(gate_keys(&eventos, false), eventos);
    let mut tool = ToolState::new();
    aplica_teclas_de_herramienta(&mut tool, &gate_keys(&tecla_sola(Key::Comma), false));
    assert_eq!(tool.tool(), Tool::Dropper);
    // La `E` ya no es atajo: el filtro la deja pasar y no cambia nada. El borrador se elige por
    // su acción (la del botón).
    aplica_teclas_de_herramienta(&mut tool, &gate_keys(&tecla_sola(Key::E), false));
    assert_eq!(tool.tool(), Tool::Dropper);
    tool.apply_action(ToolAction::Select(Tool::Eraser));
    assert_eq!(tool.tool(), Tool::Eraser);
}

fn tecla_sola(key: Key) -> Vec<Event> {
    vec![tecla(key, Modifiers::NONE)]
}

/// Un `Context` con un campo de texto que se puede enfocar o no.
struct BancoTexto {
    ctx: Context,
    texto: String,
}

impl BancoTexto {
    fn nuevo() -> BancoTexto {
        BancoTexto {
            ctx: Context::default(),
            texto: String::new(),
        }
    }

    fn fotograma(&mut self, events: Vec<Event>, enfocar: bool) {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            events,
            ..RawInput::default()
        };
        let texto = &mut self.texto;
        let mut salida = self.ctx.run_ui(input, |ui| {
            let respuesta = ui.add(TextEdit::singleline(texto));
            if enfocar {
                respuesta.request_focus();
            }
        });
        salida.textures_delta.clear();
    }
}

/// **[DEPENDE DE EGUI]** Con un campo de texto enfocado de verdad, `text_edit_focused()` es cierto
/// y `gate_keys` no deja pasar ni `,` ni `E`; sin él, los deja pasar.
#[test]
fn fila_8_banco_de_egui_con_un_campo_enfocado() {
    let eventos = teclas_coma_y_e();

    let mut con_foco = BancoTexto::nuevo();
    con_foco.fotograma(vec![], true);
    con_foco.fotograma(vec![], true);
    assert!(con_foco.ctx.text_edit_focused(), "el campo tiene el foco");
    con_foco.fotograma(eventos.clone(), false);
    assert!(con_foco.ctx.text_edit_focused());
    let vistos = gate_keys(&eventos, con_foco.ctx.text_edit_focused());
    assert!(vistos.is_empty());
    let mut tool = ToolState::new();
    aplica_teclas_de_herramienta(&mut tool, &vistos);
    assert_eq!(tool.tool(), Tool::Pencil);

    let mut sin_foco = BancoTexto::nuevo();
    sin_foco.fotograma(vec![], false);
    sin_foco.fotograma(vec![], false);
    assert!(!sin_foco.ctx.text_edit_focused());
    let vistos = gate_keys(&eventos, sin_foco.ctx.text_edit_focused());
    assert_eq!(vistos, eventos);
    let mut tool = ToolState::new();
    aplica_teclas_de_herramienta(&mut tool, &vistos);
    assert_eq!(tool.tool(), Tool::Dropper, "`,` y luego E (sin atajo)");
    tool.apply_action(ToolAction::Select(Tool::Eraser));
    assert_eq!(tool.tool(), Tool::Eraser, "el borrador, por su acción");
}

// Estrategias de las propiedades 2 y 4.

fn modo() -> impl Strategy<Value = Mode> {
    prop::sample::select(MODOS.to_vec())
}

/// Paleta de seis entradas; con `transparente`, la 5 es el índice transparente.
fn paleta_seis(transparente: bool) -> Palette {
    let mut paleta = Palette::from_colors([
        rgba(200, 0, 0, 255),
        rgba(200, 0, 0, 255),
        rgba(200, 0, 0, 128),
        rgba(0, 0, 200, 255),
        rgba(0, 200, 0, 255),
        rgba(5, 5, 5, 0),
    ])
    .unwrap();
    if transparente {
        paleta.set_transparent_index(Some(5)).unwrap();
    }
    paleta
}

/// Valores posibles del fondo, con transparentes y casi duplicados.
fn pool(mode: Mode) -> Vec<Pixel> {
    match mode {
        Mode::Rgba => vec![
            Pixel::Rgba(rgba(10, 20, 30, 255)),
            Pixel::Rgba(rgba(10, 20, 30, 128)),
            Pixel::Rgba(rgba(10, 20, 30, 0)),
            Pixel::Rgba(rgba(0, 0, 0, 0)),
            Pixel::Rgba(rgba(40, 50, 60, 255)),
        ],
        Mode::Indexed => (0..6).map(Pixel::Indexed).collect(),
        Mode::Gray => vec![
            Pixel::Gray(5),
            Pixel::Gray(6),
            Pixel::Gray(7),
            Pixel::Gray(8),
        ],
        Mode::GrayAlpha => vec![
            Pixel::GrayAlpha {
                gray: 5,
                alpha: 255,
            },
            Pixel::GrayAlpha { gray: 5, alpha: 0 },
            Pixel::GrayAlpha { gray: 6, alpha: 0 },
            Pixel::GrayAlpha {
                gray: 7,
                alpha: 128,
            },
        ],
    }
}

/// Cualquier `i32`, con el interior de un lienzo de hasta 16 de lado y los extremos siempre
/// presentes.
fn coordenada() -> impl Strategy<Value = i32> {
    prop_oneof![
        6 => -2i32..=17,
        2 => any::<i32>(),
        1 => Just(MIN),
        1 => Just(MAX),
        1 => Just(MIN + 1),
        1 => Just(MAX - 1),
    ]
}

fn posicion() -> impl Strategy<Value = (i32, i32)> {
    (coordenada(), coordenada())
}

fn herramienta() -> impl Strategy<Value = Tool> {
    prop::sample::select(TODAS.to_vec())
}

/// Imagen de 1 a 16 de lado en cualquier modo, con fondo variado, y una ranura de 1 a 9. Las
/// indexadas llevan a veces índice transparente.
fn imagenes() -> impl Strategy<Value = (Image, u8)> {
    (modo(), 1u32..=16, 1u32..=16, 1u8..=9, any::<bool>())
        .prop_flat_map(|(mode, w, h, slot, transparente)| {
            let celdas = usize::try_from(w * h).unwrap();
            (
                Just((mode, w, h, slot, transparente)),
                prop::collection::vec(any::<u8>(), celdas),
            )
        })
        .prop_map(|((mode, w, h, slot, transparente), celdas)| {
            let pool = pool(mode);
            let mut image = match mode {
                Mode::Indexed => Image::new_indexed(w, h, paleta_seis(transparente)).unwrap(),
                _ => Image::new(w, h, mode).unwrap(),
            };
            for y in 0..h {
                for x in 0..w {
                    let byte = celdas[usize::try_from(y * w + x).unwrap()];
                    let k = usize::from(byte) % pool.len();
                    image.set_pixel(x, y, pool[k]).unwrap();
                }
            }
            (image, slot)
        })
}

/// Lo que debe elegir el cuentagotas con un píxel, escrito aparte de `picked_at`.
fn elegido_esperado(pixel: Pixel) -> Picked {
    match pixel {
        Pixel::Indexed(i) => Picked::Entry(i),
        Pixel::Rgba(c) => Picked::Color(c),
        Pixel::Gray(g) => Picked::Color(Rgba::opaque(g, g, g)),
        Pixel::GrayAlpha { gray, alpha } => Picked::Color(rgba(gray, gray, gray, alpha)),
    }
}

/// El píxel de la posición, si cae dentro de la imagen.
fn dentro(image: &Image, pos: (i32, i32)) -> Option<(u32, u32)> {
    let (x, y) = (u32::try_from(pos.0).ok()?, u32::try_from(pos.1).ok()?);
    (x < image.width() && y < image.height()).then_some((x, y))
}

/// El oráculo del cuentagotas: elige un píxel de dentro distinto del último de dentro que eligió
/// en este gesto.
#[derive(Default)]
struct Oraculo {
    ultimo: Option<(u32, u32)>,
}

impl Oraculo {
    fn visita(&mut self, image: &Image, pos: (i32, i32)) -> Option<Picked> {
        let (x, y) = dentro(image, pos)?;
        if self.ultimo == Some((x, y)) {
            return None;
        }
        self.ultimo = Some((x, y));
        Some(elegido_esperado(image.get_pixel(x, y).unwrap()))
    }
}

fn coincide_con_picked_at(
    image: &Image,
    pos: (i32, i32),
    esperado: Picked,
) -> Result<(), TestCaseError> {
    let (x, y) = dentro(image, pos).expect("el píxel elegido está dentro");
    prop_assert_eq!(
        picked_at(image, x, y),
        Some(esperado),
        "picked_at ({}, {})",
        x,
        y
    );
    Ok(())
}

// Propiedad 2: el cuentagotas no escribe y es exacto (parte de `px-ui`).

proptest! {
    #[test]
    fn propiedad_2_el_cuentagotas_no_escribe_y_elige_cada_pixel_nuevo_y_dentro(
        (image, _slot) in imagenes(),
        previo in 0u8..=2,
        gestos in prop::collection::vec(
            (posicion(), prop::collection::vec(posicion(), 0..=6)),
            1..=4,
        ),
    ) {
        let mut es = Escena::cuentagotas(image);
        paso_previo(&mut es.doc, es.id, previo);
        let antes = es.estado();

        for (origen, movimientos) in gestos {
            let mut oraculo = Oraculo::default();
            let mut eventos = vec![(begin(origen.0, origen.1), origen)];
            eventos.extend(movimientos.iter().map(|&p| (mov(p.0, p.1), p)));
            for (evento, pos) in eventos {
                let esperado = oraculo.visita(es.imagen(), pos);
                prop_assert!(!es.ev(evento), "{:?}: Ok(false)", evento);
                let recibido = es.eventos();
                let esperados: Vec<ToolEvent> = esperado.map(ToolEvent::Picked).into_iter().collect();
                prop_assert_eq!(&recibido, &esperados, "{:?}", evento);
                if let Some(picked) = esperado {
                    coincide_con_picked_at(es.imagen(), pos, picked)?;
                }
                prop_assert_eq!(es.estado(), antes.clone(), "{:?}: el documento no cambia", evento);
                prop_assert!(es.tool.busy());
                prop_assert!(es.preview().is_empty());
            }
            prop_assert!(!es.ev(fin()));
            prop_assert!(!es.tool.busy());
            prop_assert!(es.eventos().is_empty());
            prop_assert_eq!(es.estado(), antes.clone());
        }
    }
}

// Propiedad 4: herramienta estable con las cinco.

/// Un paso de la secuencia. Solo se entregan los que `Canvas` puede emitir: `Begin` en reposo y
/// `Move` y `End` con un gesto abierto; el resto se descarta.
#[derive(Clone, Debug)]
enum Operacion {
    Begin((i32, i32)),
    Move((i32, i32)),
    End,
    Escape,
    Select(Tool),
    Cancel,
}

fn operacion() -> impl Strategy<Value = Operacion> {
    prop_oneof![
        3 => posicion().prop_map(Operacion::Begin),
        3 => posicion().prop_map(Operacion::Move),
        3 => Just(Operacion::End),
        1 => Just(Operacion::Escape),
        4 => herramienta().prop_map(Operacion::Select),
        1 => Just(Operacion::Cancel),
    ]
}

proptest! {
    #[test]
    fn propiedad_4_herramienta_estable_con_las_cinco(
        (image, slot) in imagenes(),
        operaciones in prop::collection::vec(operacion(), 0..=50),
    ) {
        let mut es = Escena::nueva(image, Tool::Pencil);
        let activo = pixel_for_slot(es.imagen(), slot);
        let error_de_borrado = eraser_pixel(es.imagen()).err();
        prop_assert!(!es.tool.busy());
        prop_assert!(es.preview().is_empty(), "vacía en reposo");
        prop_assert!(es.tool.take_events().is_empty());

        let mut ocupado = false;
        for operacion in operaciones {
            let herramienta_antes = es.tool.tool();
            let mut esperados: Vec<ToolEvent> = Vec::new();
            match operacion {
                Operacion::Begin(p) if !ocupado => {
                    let ocupada_con = es.tool.tool();
                    es.tool
                        .canvas_event_with(&mut es.doc, es.id, activo, begin(p.0, p.1))
                        .unwrap();
                    ocupado = true;
                    if ocupada_con == Tool::Eraser
                        && let Some(error) = error_de_borrado
                    {
                        esperados.push(ToolEvent::Refused(error));
                    }
                    if ocupada_con == Tool::Dropper && dentro(es.imagen(), p).is_some() {
                        let recibidos = es.tool.take_events();
                        prop_assert_eq!(recibidos.len(), 1, "el Begin dentro elige un píxel");
                        prop_assert!(matches!(recibidos[0], ToolEvent::Picked(_)));
                    } else {
                        prop_assert_eq!(es.tool.take_events(), esperados, "Begin con {:?}", ocupada_con);
                    }
                }
                Operacion::Move(p) if ocupado => {
                    es.tool
                        .canvas_event_with(&mut es.doc, es.id, activo, mov(p.0, p.1))
                        .unwrap();
                    let recibidos = es.tool.take_events();
                    if es.tool.tool() == Tool::Dropper {
                        prop_assert!(recibidos.len() <= 1);
                        prop_assert!(recibidos.iter().all(|e| matches!(e, ToolEvent::Picked(_))));
                    } else {
                        prop_assert!(recibidos.is_empty(), "Move con {:?}: {:?}", es.tool.tool(), recibidos);
                    }
                }
                Operacion::End if ocupado => {
                    es.tool
                        .canvas_event_with(&mut es.doc, es.id, activo, fin())
                        .unwrap();
                    ocupado = false;
                    let recibidos = es.tool.take_events();
                    prop_assert!(recibidos.is_empty(), "End no emite eventos: {:?}", recibidos);
                }
                Operacion::Escape => {
                    es.tool
                        .canvas_input_with(&mut es.doc, es.id, activo, CanvasInput::Escape)
                        .unwrap();
                    prop_assert!(es.tool.take_events().is_empty());
                }
                Operacion::Cancel => {
                    es.tool.apply_action(ToolAction::Cancel);
                    prop_assert!(es.tool.take_events().is_empty());
                }
                Operacion::Select(herramienta) => {
                    es.tool.apply_action(ToolAction::Select(herramienta));
                    let esperada = if ocupado { herramienta_antes } else { herramienta };
                    prop_assert_eq!(es.tool.tool(), esperada, "Select con busy = {}", ocupado);
                    prop_assert!(es.tool.take_events().is_empty());
                }
                _ => {}
            }
            if !matches!(operacion, Operacion::Select(_)) {
                prop_assert_eq!(es.tool.tool(), herramienta_antes, "solo Select cambia la herramienta");
            }
            prop_assert_eq!(es.tool.busy(), ocupado, "busy es cierto entre Begin y End");
            if es.tool.tool() == Tool::Dropper {
                prop_assert!(es.preview().is_empty(), "el cuentagotas nunca tiene vista previa");
            }
            if !ocupado {
                prop_assert!(es.preview().is_empty(), "vacía en reposo");
                prop_assert!(es.tool.take_events().is_empty(), "sin eventos en reposo");
            }
        }
    }
}

/// `EraseError` solo se usa en la propiedad 4 a través de `ToolEvent::Refused`.
#[test]
fn el_error_de_borrado_viaja_en_el_evento() {
    let evento = ToolEvent::Refused(EraseError::NoTransparency);
    assert_eq!(evento.clone(), evento);
    assert_ne!(evento, ToolEvent::Refused(EraseError::NoTransparentIndex));
    assert_ne!(evento, ToolEvent::Picked(Picked::Entry(0)));
}
