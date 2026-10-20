//! Tests de la lógica de herramientas, sin ventana (specs/hito-2-linea-relleno-app.md, «Casos
//! límite» filas 1 a 12, 15 (en `tests/notice.rs`), 16, 17 (primera parte), 18 y 19, y
//! propiedades 1 a 6).
//!
//! Los eventos se dan a mano a `ToolState::canvas_event`. El oráculo de la línea es
//! `px_raster::line_command` (propiedad 1) y el del relleno es un recorrido ingenuo pixel a pixel
//! con `VecDeque`, 4 vecinos y `Image::get_pixel` (propiedad 1 de specs/hito-2-relleno.md), que no
//! usa nada de `px-raster`.
//!
//! La propiedad 5 recorre todo `Key::ALL` por las 32 combinaciones de modificadores: se escribe
//! como `proptest!` (muestreo) y, además, como recorrido exhaustivo, que es más fuerte.

use std::collections::{BTreeSet, VecDeque};

use egui::{Key, Modifiers};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Command, Document, DocumentError, Image, ImageId, Mode, Palette, Pixel, PixelEdit, Rgba,
};
use px_raster::{flood_fill, line_command};
use px_ui::canvas::{CanvasEvent, CanvasInput};
use px_ui::color::pixel_for_slot;
use px_ui::shortcuts::action_for_key;
use px_ui::tool::{Tool, ToolAction, ToolState, status_text, tool_action_for_key};

const MIN: i32 = i32::MIN;
const MAX: i32 = i32::MAX;
const MODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];
const HERRAMIENTAS: [Tool; 3] = [Tool::Pencil, Tool::Line, Tool::Fill];

// Apoyo común.

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

/// Ranura 3: rojo opaco.
fn rojo() -> Pixel {
    Pixel::Rgba(Rgba::opaque(255, 0, 0))
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

/// Imagen RGBA de 8×8 vacía (todo `A` a cero).
fn imagen_8x8() -> Image {
    Image::new(8, 8, Mode::Rgba).expect("imagen válida")
}

/// Imagen indexada de 8×8 con una paleta de 3 entradas.
fn imagen_indexada_8x8() -> Image {
    let paleta = Palette::from_colors([
        Rgba::opaque(10, 10, 10),
        Rgba::opaque(20, 20, 20),
        Rgba::opaque(30, 30, 30),
    ])
    .expect("paleta válida");
    Image::new_indexed(8, 8, paleta).expect("imagen válida")
}

/// Documento, herramienta y ranura juntos: lo que hace `px-app` con los eventos del lienzo.
struct Escena {
    doc: Document,
    id: ImageId,
    tool: ToolState,
    slot: u8,
}

impl Escena {
    fn nueva(image: Image, herramienta: Tool, slot: u8) -> Escena {
        let doc = Document::new(image);
        let id = doc.main_image();
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(herramienta));
        Escena {
            doc,
            id,
            tool,
            slot,
        }
    }

    fn rgba_8x8(herramienta: Tool) -> Escena {
        Escena::nueva(imagen_8x8(), herramienta, 3)
    }

    fn imagen(&self) -> &Image {
        self.doc.image(self.id).expect("hay imagen")
    }

    /// Da el evento a la herramienta; un `Err` aquí es un fallo del test.
    fn ev(&mut self, evento: CanvasEvent) -> bool {
        self.tool
            .canvas_event(&mut self.doc, self.id, self.slot, evento)
            .unwrap_or_else(|e| panic!("{evento:?} dio el error {e:?}"))
    }

    /// `Escape` por la vía de la entrada ordenada (`canvas_input`).
    fn escape(&mut self) -> bool {
        self.tool
            .canvas_input(&mut self.doc, self.id, self.slot, CanvasInput::Escape)
            .unwrap_or_else(|e| panic!("Escape dio el error {e:?}"))
    }

    /// Un evento de trazo por la vía de la entrada ordenada (`canvas_input`).
    fn entrada(&mut self, evento: CanvasEvent) -> bool {
        self.tool
            .canvas_input(
                &mut self.doc,
                self.id,
                self.slot,
                CanvasInput::Stroke(evento),
            )
            .unwrap_or_else(|e| panic!("{evento:?} dio el error {e:?}"))
    }

    /// Lo que hay pendiente en la superposición.
    fn preview(&self) -> Vec<PixelEdit> {
        self.tool.preview().collect()
    }

    fn historia(&self) -> (bool, bool, usize) {
        historia(&self.doc)
    }
}

/// `can_undo`, `can_redo` e `history_bytes`.
fn historia(doc: &Document) -> (bool, bool, usize) {
    (doc.can_undo(), doc.can_redo(), doc.history_bytes())
}

fn conjunto(puntos: impl IntoIterator<Item = (u32, u32)>) -> BTreeSet<(u32, u32)> {
    puntos.into_iter().collect()
}

/// Coordenadas de los píxeles que difieren entre dos imágenes del mismo tamaño.
fn cambiados(a: &Image, b: &Image) -> BTreeSet<(u32, u32)> {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    (0..a.height())
        .flat_map(|y| (0..a.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| a.get_pixel(x, y).unwrap() != b.get_pixel(x, y).unwrap())
        .collect()
}

/// `n` píxeles de la fila `y`, de `x0` a `x1` incluidos, con `pixel`.
fn fila_de(y: u32, x0: u32, x1: u32, pixel: Pixel) -> Vec<PixelEdit> {
    (x0..=x1).map(|x| PixelEdit { x, y, pixel }).collect()
}

/// La línea de la fila 5: `Begin(1,1)`, `Move(5,1)` y `End`, que da los píxeles `(1,1)…(5,1)`.
fn linea_de_la_fila_5(es: &mut Escena) -> bool {
    assert!(!es.ev(begin(1, 1)));
    assert!(!es.ev(mov(5, 1)));
    es.ev(fin())
}

// Fila 1.

#[test]
fn fila_1_p_l_f_y_escape_sin_modificadores() {
    let n = Modifiers::NONE;
    assert_eq!(
        tool_action_for_key(Key::P, n),
        Some(ToolAction::Select(Tool::Pencil))
    );
    assert_eq!(
        tool_action_for_key(Key::L, n),
        Some(ToolAction::Select(Tool::Line))
    );
    assert_eq!(
        tool_action_for_key(Key::F, n),
        Some(ToolAction::Select(Tool::Fill))
    );
    assert_eq!(
        tool_action_for_key(Key::Escape, n),
        Some(ToolAction::Cancel)
    );
}

// Fila 2.

#[test]
fn fila_2_con_shift_command_o_alt_no_hay_accion() {
    for modifiers in [Modifiers::SHIFT, Modifiers::COMMAND, Modifiers::ALT] {
        for key in [Key::P, Key::L, Key::F, Key::Escape] {
            assert_eq!(
                tool_action_for_key(key, modifiers),
                None,
                "{key:?} con {modifiers:?}"
            );
        }
    }
}

#[test]
fn fila_2_otras_teclas_sin_modificadores_no_hacen_nada() {
    for key in [Key::Z, Key::Num1, Key::Space, Key::Home] {
        assert_eq!(tool_action_for_key(key, Modifiers::NONE), None, "{key:?}");
    }
}

#[test]
fn fila_2_action_for_key_sigue_dando_none_para_p_l_f_y_escape() {
    for key in [Key::P, Key::L, Key::F, Key::Escape] {
        assert_eq!(action_for_key(key, Modifiers::NONE), None, "{key:?}");
    }
}

// Fila 3.

#[test]
fn fila_3_texto_de_estado() {
    assert_eq!(
        status_text(Tool::Line, Some("color 3 #ff0000")),
        "línea · color 3 #ff0000"
    );
    assert_eq!(status_text(Tool::Pencil, None), "lápiz");
    assert_eq!(
        status_text(Tool::Fill, Some("color 9 transparente")),
        "relleno · color 9 transparente"
    );
}

#[test]
fn los_nombres_de_las_herramientas() {
    assert_eq!(Tool::Pencil.label(), "lápiz");
    assert_eq!(Tool::Line.label(), "línea");
    assert_eq!(Tool::Fill.label(), "relleno");
}

#[test]
fn la_herramienta_inicial_es_el_lapiz_y_no_hay_nada_pendiente() {
    let tool = ToolState::new();
    assert_eq!(tool.tool(), Tool::Pencil);
    assert!(!tool.busy());
    assert_eq!(tool.preview().count(), 0);
}

// Fila 4.

#[test]
fn fila_4_la_linea_en_curso_solo_se_ve_en_la_vista_previa() {
    let mut es = Escena::rgba_8x8(Tool::Line);
    let copia = es.imagen().clone();
    assert!(!es.ev(begin(1, 1)), "Begin no cambia el documento");
    assert!(!es.ev(mov(5, 1)), "Move no cambia el documento");
    assert_eq!(es.imagen(), &copia);
    assert!(!es.doc.can_undo());
    assert_eq!(es.preview(), fila_de(1, 1, 5, rojo()));
    assert!(es.tool.busy());
}

// Fila 5.

#[test]
fn fila_5_end_aplica_la_linea_como_un_solo_paso() {
    let mut es = Escena::rgba_8x8(Tool::Line);
    let copia = es.imagen().clone();
    assert!(linea_de_la_fila_5(&mut es), "End da Ok(true)");
    assert_eq!(
        cambiados(es.imagen(), &copia),
        conjunto((1..=5).map(|x| (x, 1)))
    );
    for x in 1..=5 {
        assert_eq!(es.imagen().get_pixel(x, 1), Ok(rojo()));
    }
    assert!(!es.tool.busy());
    assert_eq!(es.preview(), vec![], "nada pendiente tras el End");

    let editada = es.imagen().clone();
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &copia, "un solo paso");
    assert!(!es.doc.can_undo());
    assert_eq!(es.doc.redo(), Ok(()));
    assert_eq!(es.imagen(), &editada);
}

// Fila 6.

#[test]
fn fila_6_los_destinos_intermedios_no_se_acumulan() {
    let mut es = Escena::rgba_8x8(Tool::Line);
    let copia = es.imagen().clone();
    assert!(!es.ev(begin(0, 0)));
    assert!(!es.ev(mov(7, 0)));
    assert!(!es.ev(mov(3, 3)));
    assert!(es.ev(fin()));
    assert_eq!(
        cambiados(es.imagen(), &copia),
        conjunto([(0, 0), (1, 1), (2, 2), (3, 3)])
    );
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &copia, "un solo paso");
}

// Fila 7.

#[test]
fn fila_7_un_clic_sin_movimiento_pinta_un_pixel() {
    let mut es = Escena::rgba_8x8(Tool::Line);
    let copia = es.imagen().clone();
    assert!(!es.ev(begin(2, 3)));
    assert!(es.ev(fin()));
    assert_eq!(cambiados(es.imagen(), &copia), conjunto([(2, 3)]));
    assert_eq!(es.imagen().get_pixel(2, 3), Ok(rojo()));
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &copia, "un solo paso");
    assert!(!es.doc.can_undo());
}

// Fila 8.

#[test]
fn fila_8_el_extremo_fuera_se_recorta() {
    let mut es = Escena::rgba_8x8(Tool::Line);
    let copia = es.imagen().clone();
    assert!(!es.ev(begin(-3, 2)));
    assert!(!es.ev(mov(3, 2)));
    assert_eq!(
        es.preview(),
        fila_de(2, 0, 3, rojo()),
        "la vista previa también va recortada"
    );
    assert!(es.ev(fin()));
    assert_eq!(
        cambiados(es.imagen(), &copia),
        conjunto((0..=3).map(|x| (x, 2)))
    );
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &copia, "un solo paso");
    assert!(!es.doc.can_undo());
}

#[test]
fn fila_8_una_linea_entera_fuera_no_da_paso_y_no_borra_el_rehacer() {
    let mut es = Escena::rgba_8x8(Tool::Line);
    let copia = es.imagen().clone();
    assert!(linea_de_la_fila_5(&mut es));
    assert_eq!(es.doc.undo(), Ok(()));
    assert!(es.doc.can_redo());
    let antes = es.historia();

    assert!(!es.ev(begin(-5, -5)));
    assert!(!es.ev(mov(-1, -1)));
    assert_eq!(es.preview(), vec![], "nada dentro del lienzo que mostrar");
    assert!(!es.ev(fin()), "Ok(false): fuera del lienzo");
    assert_eq!(es.imagen(), &copia);
    assert_eq!(es.historia(), antes);
    assert!(es.doc.can_redo(), "el rehacer sigue disponible");
    assert!(!es.doc.can_undo());
}

#[test]
fn fila_8_repetir_la_linea_sobre_pixeles_ya_rojos_no_da_paso() {
    let mut es = Escena::rgba_8x8(Tool::Line);
    assert!(linea_de_la_fila_5(&mut es));
    let editada = es.imagen().clone();
    let antes = es.historia();

    assert!(!linea_de_la_fila_5(&mut es), "Ok(false): sin cambios");
    assert_eq!(es.imagen(), &editada);
    assert_eq!(es.historia(), antes);
    // Solo hay un paso: un `undo` lo deja todo como al principio.
    assert_eq!(es.doc.undo(), Ok(()));
    assert!(!es.doc.can_undo());
}

#[test]
fn la_linea_usa_el_color_de_la_ranura_del_begin() {
    // Spec, sección 2: `Begin` guarda el `Pixel` de la ranura activa en ese momento.
    let mut es = Escena::rgba_8x8(Tool::Line);
    es.slot = 3;
    assert!(!es.ev(begin(1, 1)));
    es.slot = 5;
    assert!(!es.ev(mov(5, 1)));
    assert_eq!(es.preview(), fila_de(1, 1, 5, rojo()));
    es.slot = 1;
    assert!(es.ev(fin()));
    for x in 1..=5 {
        assert_eq!(es.imagen().get_pixel(x, 1), Ok(rojo()));
    }
}

// Fila 9.

#[test]
fn fila_9_cancelar_descarta_la_linea_y_el_gesto_sigue_ocupado_hasta_soltar() {
    let mut es = Escena::rgba_8x8(Tool::Line);
    let copia = es.imagen().clone();
    let antes = es.historia();
    assert!(!es.ev(begin(1, 1)));
    assert!(!es.ev(mov(5, 1)));
    es.tool.apply_action(ToolAction::Cancel);
    assert_eq!(es.preview(), vec![], "la línea pendiente se descarta");
    assert!(es.tool.busy(), "ocupado hasta el End");
    assert!(!es.ev(mov(6, 1)));
    assert!(es.tool.busy());
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());
    assert_eq!(es.imagen(), &copia);
    assert_eq!(es.historia(), antes);
}

#[test]
fn fila_9_cancelar_en_reposo_no_hace_nada() {
    for herramienta in HERRAMIENTAS {
        let mut es = Escena::rgba_8x8(herramienta);
        let copia = es.imagen().clone();
        let antes = es.historia();
        es.tool.apply_action(ToolAction::Cancel);
        assert_eq!(es.tool.tool(), herramienta);
        assert!(!es.tool.busy());
        assert_eq!(es.preview(), vec![]);
        assert_eq!(es.imagen(), &copia);
        assert_eq!(es.historia(), antes);
    }
}

#[test]
fn fila_9_cancelar_con_el_lapiz_no_cancela_el_trazo() {
    let mut es = Escena::rgba_8x8(Tool::Pencil);
    let copia = es.imagen().clone();
    assert!(!es.ev(begin(1, 1)));
    assert!(!es.ev(mov(5, 1)));
    es.tool.apply_action(ToolAction::Cancel);
    assert!(es.tool.busy());
    assert_eq!(es.preview(), fila_de(1, 1, 5, rojo()), "el trazo sigue ahí");
    assert!(es.ev(fin()), "el trazo del lápiz se aplica");
    assert_eq!(
        cambiados(es.imagen(), &copia),
        conjunto((1..=5).map(|x| (x, 1)))
    );
    assert!(es.doc.can_undo());
}

#[test]
fn fila_9_cancelar_con_el_relleno_no_deshace_el_relleno() {
    let mut es = Escena::rgba_8x8(Tool::Fill);
    let copia = es.imagen().clone();
    assert!(es.ev(begin(3, 3)), "el relleno se hace al pulsar");
    let rellena = es.imagen().clone();
    let despues = es.historia();
    es.tool.apply_action(ToolAction::Cancel);
    assert!(es.tool.busy(), "ocupado hasta el End");
    assert!(!es.ev(mov(0, 0)));
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());
    assert_eq!(es.imagen(), &rellena);
    assert_eq!(es.historia(), despues);
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &copia);
}

// Fila 10.

#[test]
fn fila_10_cambiar_de_herramienta_durante_un_gesto_se_ignora() {
    let mut es = Escena::rgba_8x8(Tool::Line);
    assert!(!es.ev(begin(1, 1)));
    es.tool.apply_action(ToolAction::Select(Tool::Fill));
    assert_eq!(es.tool.tool(), Tool::Line, "ignorado con el gesto en curso");
    assert!(!es.ev(mov(5, 1)));
    es.tool.apply_action(ToolAction::Select(Tool::Pencil));
    assert_eq!(es.tool.tool(), Tool::Line);
    assert!(es.ev(fin()), "y la línea sigue siendo una línea");
    assert_eq!(es.preview(), vec![]);

    es.tool.apply_action(ToolAction::Select(Tool::Fill));
    assert_eq!(es.tool.tool(), Tool::Fill, "tras el End sí cambia");
}

#[test]
fn fila_10_una_ranura_sin_color_deja_el_gesto_ocupado_pero_no_cambia_nada() {
    for herramienta in HERRAMIENTAS {
        // Paleta de 3 entradas: la ranura 5 no tiene color.
        let mut es = Escena::nueva(imagen_indexada_8x8(), herramienta, 5);
        assert_eq!(pixel_for_slot(es.imagen(), 5), None);
        let copia = es.imagen().clone();
        let antes = es.historia();

        assert!(!es.ev(begin(2, 2)), "{herramienta:?}");
        assert!(es.tool.busy(), "{herramienta:?}: ocupado entre Begin y End");
        es.tool.apply_action(ToolAction::Select(Tool::Line));
        es.tool.apply_action(ToolAction::Select(Tool::Fill));
        assert_eq!(es.tool.tool(), herramienta, "{herramienta:?}: sin cambio");
        assert_eq!(es.preview(), vec![], "{herramienta:?}: no hay gesto real");
        assert!(!es.ev(mov(4, 2)));
        assert!(es.tool.busy());
        assert!(!es.ev(fin()));
        assert!(!es.tool.busy(), "{herramienta:?}: no queda bloqueado");

        assert_eq!(es.imagen(), &copia);
        assert_eq!(es.historia(), antes);
        es.tool.apply_action(ToolAction::Select(Tool::Fill));
        assert_eq!(es.tool.tool(), Tool::Fill, "{herramienta:?}: ya se puede");
    }
}

// Fila 11.

#[test]
fn fila_11_el_relleno_se_hace_al_pulsar_y_el_arrastre_no_hace_nada_mas() {
    let mut es = Escena::rgba_8x8(Tool::Fill);
    let copia = es.imagen().clone();
    assert!(es.ev(begin(3, 3)), "Ok(true) en el Begin");
    assert!(!es.ev(mov(0, 0)));
    assert!(!es.ev(mov(7, 7)));
    assert!(es.tool.busy());
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());

    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(es.imagen().get_pixel(x, y), Ok(rojo()), "({x}, {y})");
        }
    }
    // Un solo `FillSpans`: lo mismo que `flood_fill` directamente sobre otro documento.
    let mut referencia = Document::new(copia.clone());
    let id = referencia.main_image();
    assert_eq!(flood_fill(&mut referencia, id, 3, 3, rojo()), Ok(true));
    assert_eq!(es.doc.history_bytes(), referencia.history_bytes());
    assert_eq!(es.imagen(), referencia.image(id).unwrap());

    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &copia, "un solo paso");
    assert!(!es.doc.can_undo());
}

// Fila 12.

#[test]
fn fila_12_semillas_fuera_del_lienzo_no_hacen_nada() {
    for (x, y) in [(-1, 0), (8, 0), (0, 8), (0, -1), (MIN, MAX)] {
        let mut es = Escena::rgba_8x8(Tool::Fill);
        let copia = es.imagen().clone();
        let antes = es.historia();
        assert!(!es.ev(begin(x, y)), "semilla ({x}, {y})");
        assert!(es.tool.busy());
        assert!(!es.ev(fin()));
        assert!(!es.tool.busy());
        assert_eq!(es.imagen(), &copia);
        assert_eq!(es.historia(), antes);
    }
}

#[test]
fn fila_12_una_semilla_ya_roja_no_da_paso() {
    let mut es = Escena::rgba_8x8(Tool::Fill);
    assert!(es.ev(begin(3, 3)));
    assert!(!es.ev(fin()));
    let rellena = es.imagen().clone();
    let antes = es.historia();

    assert!(!es.ev(begin(3, 3)), "segundo clic igual: Ok(false)");
    assert!(!es.ev(fin()));
    assert_eq!(es.imagen(), &rellena);
    assert_eq!(es.historia(), antes);
}

#[test]
fn fila_12_una_semilla_ya_roja_tras_un_undo_no_borra_el_rehacer() {
    // (0, 0) ya es rojo; el relleno verde (ranura 4) toma todo lo demás y se deshace.
    let mut imagen = imagen_8x8();
    imagen.set_pixel(0, 0, rojo()).unwrap();
    let mut es = Escena::nueva(imagen.clone(), Tool::Fill, 4);
    assert!(es.ev(begin(5, 5)));
    assert!(!es.ev(fin()));
    assert_eq!(es.doc.undo(), Ok(()));
    assert!(es.doc.can_redo());
    assert_eq!(es.imagen(), &imagen);
    let antes = es.historia();

    es.slot = 3;
    assert!(!es.ev(begin(0, 0)), "la semilla ya vale el color activo");
    assert!(!es.ev(fin()));
    assert_eq!(es.imagen(), &imagen);
    assert_eq!(es.historia(), antes);
    assert!(es.doc.can_redo(), "el rehacer sigue disponible");
}

// Fila 16.

#[test]
fn fila_16_ctrl_p_l_f_no_cambian_de_herramienta() {
    // En Linux egui da `ctrl` y `command` a la vez.
    let ctrl_y_command = Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    };
    let combinaciones = [
        Modifiers::CTRL,
        Modifiers::COMMAND,
        Modifiers::MAC_CMD,
        ctrl_y_command,
    ];
    for modifiers in combinaciones {
        for key in [Key::P, Key::L, Key::F] {
            assert_eq!(
                tool_action_for_key(key, modifiers),
                None,
                "{key:?} con {modifiers:?}"
            );
        }
    }
}

// Fila 17, primera parte.

#[test]
fn fila_17_el_relleno_mantenido_no_se_repite() {
    let mut es = Escena::rgba_8x8(Tool::Fill);
    let copia = es.imagen().clone();
    let mut verdaderos = usize::from(es.ev(begin(3, 3)));
    for _ in 0..100 {
        verdaderos += usize::from(es.ev(mov(3, 3)));
    }
    assert!(es.tool.busy());
    verdaderos += usize::from(es.ev(fin()));
    assert_eq!(verdaderos, 1, "un solo Ok(true)");
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &copia, "un solo paso");
    assert!(!es.doc.can_undo());
}

#[test]
fn canvas_input_con_eventos_de_trazo_equivale_a_canvas_event() {
    // La línea de la fila 5 por la vía ordenada: mismos resultados y mismo documento.
    let mut ordenada = Escena::rgba_8x8(Tool::Line);
    let mut directa = Escena::rgba_8x8(Tool::Line);
    for evento in [begin(1, 1), mov(5, 1), fin()] {
        assert_eq!(ordenada.entrada(evento), directa.ev(evento), "{evento:?}");
        assert_eq!(ordenada.preview(), directa.preview());
        assert_eq!(ordenada.tool.busy(), directa.tool.busy());
    }
    assert_eq!(ordenada.imagen(), directa.imagen());
    assert_eq!(ordenada.historia(), directa.historia());
}

// Fila 18.

#[test]
fn fila_18_escape_antes_del_soltado_cancela_la_linea() {
    let mut es = Escena::rgba_8x8(Tool::Line);
    let copia = es.imagen().clone();
    let antes = es.historia();
    assert!(!es.ev(begin(1, 1)));
    assert!(!es.ev(mov(5, 1)));
    assert!(!es.escape(), "Escape: Ok(false)");
    assert!(es.tool.busy(), "ocupado hasta el End");
    assert_eq!(es.preview(), vec![]);
    assert!(!es.ev(mov(5, 1)));
    assert!(es.tool.busy());
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());
    assert_eq!(es.imagen(), &copia);
    assert_eq!(es.historia(), antes);
}

// Fila 19.

#[test]
fn fila_19_escape_despues_del_soltado_no_deshace_la_linea() {
    let mut es = Escena::rgba_8x8(Tool::Line);
    let copia = es.imagen().clone();
    assert!(!es.ev(begin(1, 1)));
    assert!(!es.ev(mov(5, 1)));
    assert!(!es.ev(mov(5, 1)));
    assert!(es.ev(fin()), "End aplica la línea");
    assert_eq!(
        cambiados(es.imagen(), &copia),
        conjunto((1..=5).map(|x| (x, 1)))
    );
    for x in 1..=5 {
        assert_eq!(es.imagen().get_pixel(x, 1), Ok(rojo()));
    }
    let editada = es.imagen().clone();
    let despues = es.historia();

    assert!(!es.escape(), "Escape: Ok(false)");
    assert!(es.doc.can_undo(), "no deshace");
    assert_eq!(es.imagen(), &editada);
    assert_eq!(es.historia(), despues);
    assert!(!es.tool.busy());
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &copia, "un solo paso");
}

#[test]
fn escape_en_reposo_con_cualquier_herramienta_no_hace_nada() {
    for herramienta in HERRAMIENTAS {
        let mut es = Escena::rgba_8x8(herramienta);
        let copia = es.imagen().clone();
        let antes = es.historia();
        assert!(!es.escape());
        assert!(!es.tool.busy());
        assert_eq!(es.tool.tool(), herramienta);
        assert_eq!(es.imagen(), &copia);
        assert_eq!(es.historia(), antes);
    }
}

// Oráculo del relleno y estrategias de las propiedades.

/// Recorrido ingenuo pixel a pixel con cola: 4 vecinos y valor exacto (`get_pixel`).
fn region_ingenua(img: &Image, sx: u32, sy: u32) -> BTreeSet<(u32, u32)> {
    let valor = img.get_pixel(sx, sy).unwrap();
    let mut region = BTreeSet::new();
    let mut cola = VecDeque::new();
    region.insert((sx, sy));
    cola.push_back((sx, sy));
    while let Some((x, y)) = cola.pop_front() {
        let (xi, yi) = (i64::from(x), i64::from(y));
        for (nx, ny) in [(xi - 1, yi), (xi + 1, yi), (xi, yi - 1), (xi, yi + 1)] {
            let (Ok(nx), Ok(ny)) = (u32::try_from(nx), u32::try_from(ny)) else {
                continue;
            };
            if !region.contains(&(nx, ny)) && img.get_pixel(nx, ny) == Ok(valor) {
                region.insert((nx, ny));
                cola.push_back((nx, ny));
            }
        }
    }
    region
}

/// Imagen original con los píxeles de `puntos` escritos con `pixel`, solo con `set_pixel`.
fn con_puntos_pintados(
    original: &Image,
    puntos: impl IntoIterator<Item = (u32, u32)>,
    pixel: Pixel,
) -> Image {
    let mut img = original.clone();
    for (x, y) in puntos {
        img.set_pixel(x, y, pixel).unwrap();
    }
    img
}

fn modo() -> impl Strategy<Value = Mode> {
    prop::sample::select(MODOS.to_vec())
}

/// Paleta de seis entradas con casi duplicados: 0 y 1 igual color, 2 mismo RGB con otro alfa.
fn paleta_seis() -> Palette {
    Palette::from_colors([
        rgba(200, 0, 0, 255),
        rgba(200, 0, 0, 255),
        rgba(200, 0, 0, 128),
        rgba(0, 0, 200, 255),
        rgba(0, 200, 0, 255),
        rgba(5, 5, 5, 0),
    ])
    .unwrap()
}

fn imagen_vacia(mode: Mode, width: u32, height: u32) -> Image {
    match mode {
        Mode::Indexed => Image::new_indexed(width, height, paleta_seis()).unwrap(),
        _ => Image::new(width, height, mode).unwrap(),
    }
}

/// Valores posibles del fondo, con casi duplicados (hay al menos cuatro por modo).
fn pool_fondo(mode: Mode) -> Vec<Pixel> {
    match mode {
        Mode::Rgba => vec![
            Pixel::Rgba(rgba(10, 20, 30, 255)),
            Pixel::Rgba(rgba(10, 20, 30, 254)),
            Pixel::Rgba(rgba(10, 20, 30, 0)),
            Pixel::Rgba(rgba(9, 9, 9, 0)),
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
            Pixel::GrayAlpha {
                gray: 5,
                alpha: 254,
            },
            Pixel::GrayAlpha { gray: 5, alpha: 0 },
            Pixel::GrayAlpha { gray: 6, alpha: 0 },
            Pixel::GrayAlpha {
                gray: 6,
                alpha: 255,
            },
        ],
    }
}

/// Valores del fondo más el color de la ranura (si lo tiene): así la herramienta encuentra a
/// veces su propio color ya en la imagen.
fn pool_con_ranura(mode: Mode, slot: u8) -> Vec<Pixel> {
    let mut pool = pool_fondo(mode);
    if let Some(pixel) = pixel_for_slot(&imagen_vacia(mode, 1, 1), slot) {
        pool.push(pixel);
    }
    pool
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
    prop::sample::select(HERRAMIENTAS.to_vec())
}

/// Imagen de 1 a 16 de lado en cualquier modo, con fondo variado, y una ranura de 1 a 9.
fn imagenes() -> impl Strategy<Value = (Image, u8)> {
    (modo(), 1u32..=16, 1u32..=16, 1u8..=9)
        .prop_flat_map(|(mode, w, h, slot)| {
            let celdas = usize::try_from(w * h).unwrap();
            (
                Just((mode, w, h, slot)),
                prop::collection::vec(any::<u8>(), celdas),
            )
        })
        .prop_map(|((mode, w, h, slot), celdas)| {
            let pool = pool_con_ranura(mode, slot);
            let mut image = imagen_vacia(mode, w, h);
            for y in 0..h {
                for x in 0..w {
                    let byte = celdas[usize::try_from(y * w + x).unwrap()];
                    // La mitad de las casillas valen el primer valor: ganan las regiones grandes.
                    let k = if byte < 128 {
                        0
                    } else {
                        usize::from(byte) % pool.len()
                    };
                    image.set_pixel(x, y, pool[k]).unwrap();
                }
            }
            (image, slot)
        })
}

/// Imagen con un fondo de 2 a 4 valores (casi duplicados incluidos, y a veces el color de la
/// ranura) y la ranura.
fn imagenes_de_fondo_corto() -> impl Strategy<Value = (Image, u8)> {
    (modo(), 1u32..=16, 1u32..=16, 1u8..=9)
        .prop_flat_map(|(mode, w, h, slot)| {
            let celdas = usize::try_from(w * h).unwrap();
            (
                Just((mode, w, h, slot)),
                prop::sample::subsequence(pool_con_ranura(mode, slot), 2..=4),
                prop::collection::vec(any::<u8>(), celdas),
            )
        })
        .prop_map(|((mode, w, h, slot), fondo, celdas)| {
            let mut image = imagen_vacia(mode, w, h);
            for y in 0..h {
                for x in 0..w {
                    let byte = celdas[usize::try_from(y * w + x).unwrap()];
                    let k = if byte < 128 {
                        0
                    } else {
                        usize::from(byte) % fondo.len()
                    };
                    image.set_pixel(x, y, fondo[k]).unwrap();
                }
            }
            (image, slot)
        })
}

/// Un paso previo en la historia: `0` nada, `1` un paso, `2` un paso deshecho (hay rehacer).
/// Tras `0` y `2` la imagen vuelve a ser la original.
fn paso_previo(es: &mut Escena, variante: u8) {
    if variante == 0 {
        return;
    }
    let actual = es.imagen().get_pixel(0, 0).unwrap();
    let otro = pool_fondo(es.imagen().mode())
        .into_iter()
        .find(|&p| p != actual)
        .expect("el fondo tiene al menos dos valores");
    let comando = Command::SetPixels {
        image: es.id,
        pixels: vec![PixelEdit {
            x: 0,
            y: 0,
            pixel: otro,
        }],
    };
    assert_eq!(es.doc.apply(comando), Ok(true));
    if variante == 2 {
        assert_eq!(es.doc.undo(), Ok(()));
    }
}

/// Los píxeles de `line_command(a, b)`, o vacío si no cae ninguno.
fn linea_esperada(
    es: &Escena,
    a: (i32, i32),
    b: (i32, i32),
    pixel: Pixel,
) -> Result<Vec<PixelEdit>, TestCaseError> {
    match line_command(&es.doc, es.id, a, b, pixel) {
        Ok(Some(Command::SetPixels { pixels, .. })) => Ok(pixels),
        Ok(None) => Ok(vec![]),
        otro => Err(TestCaseError::fail(format!(
            "line_command dio {otro:?} con {a:?} y {b:?}"
        ))),
    }
}

/// `preview` es el conjunto de `esperado`, sin repetidos y todos con `pixel`.
fn comprueba_vista_previa(
    es: &Escena,
    esperado: &[PixelEdit],
    pixel: Pixel,
) -> Result<(), TestCaseError> {
    let vista = es.preview();
    prop_assert_eq!(vista.len(), esperado.len(), "sin repetidos ni faltas");
    for edicion in &vista {
        prop_assert_eq!(edicion.pixel, pixel);
    }
    let a = conjunto(vista.iter().map(|e| (e.x, e.y)));
    let b = conjunto(esperado.iter().map(|e| (e.x, e.y)));
    prop_assert_eq!(a, b);
    // Perezosa en cada fotograma: pedirla dos veces da lo mismo.
    prop_assert_eq!(es.preview(), vista);
    Ok(())
}

// Propiedades.

proptest! {
    // Propiedad 1: la vista previa, lo aplicado y `draw_line` coinciden.
    #[test]
    fn propiedad_1_previsualizacion_igual_a_aplicado_igual_a_line_command(
        (image, slot) in imagenes(),
        a in posicion(),
        b in posicion(),
    ) {
        let original = image.clone();
        let pixel = pixel_for_slot(&original, slot);
        let mut es = Escena::nueva(image, Tool::Line, slot);

        // Tras `Begin`: la línea de `a` a `a`.
        prop_assert!(!es.ev(begin(a.0, a.1)));
        prop_assert_eq!(es.imagen(), &original);
        prop_assert!(!es.doc.can_undo());
        let (esperado_a, esperado_ab) = match pixel {
            Some(pixel) => (
                linea_esperada(&es, a, a, pixel)?,
                linea_esperada(&es, a, b, pixel)?,
            ),
            None => (vec![], vec![]),
        };
        comprueba_vista_previa(&es, &esperado_a, pixel.unwrap_or(Pixel::Gray(0)))?;

        // Tras `Move(b)`: la línea de `a` a `b`; el documento no cambia.
        prop_assert!(!es.ev(mov(b.0, b.1)));
        prop_assert_eq!(es.imagen(), &original);
        prop_assert!(!es.doc.can_undo());
        comprueba_vista_previa(&es, &esperado_ab, pixel.unwrap_or(Pixel::Gray(0)))?;

        // Tras `End`: la original con esos píxeles en `pixel` y nada más.
        let cambio = es.ev(fin());
        let esperada = match pixel {
            Some(pixel) => con_puntos_pintados(
                &original,
                esperado_ab.iter().map(|e| (e.x, e.y)),
                pixel,
            ),
            None => original.clone(),
        };
        prop_assert_eq!(es.imagen(), &esperada);
        prop_assert_eq!(cambio, esperada != original, "Ok(true) si y solo si cambió");
        prop_assert_eq!(es.doc.can_undo(), cambio);
        prop_assert!(!es.tool.busy());
        prop_assert!(es.preview().is_empty());
    }

    // Propiedad 2: un gesto, un paso.
    #[test]
    fn propiedad_2_un_gesto_un_paso(
        (image, slot) in imagenes(),
        gestos in prop::collection::vec(
            (herramienta(), posicion(), prop::collection::vec(posicion(), 0..=4)),
            1..=6,
        ),
    ) {
        let inicial = image.clone();
        let mut es = Escena::nueva(image, Tool::Pencil, slot);
        let mut aplicados = 0usize;
        for (herramienta, origen, movimientos) in gestos {
            es.tool.apply_action(ToolAction::Select(herramienta));
            prop_assert_eq!(es.tool.tool(), herramienta);
            let antes = es.imagen().clone();
            let mut verdaderos = usize::from(es.ev(begin(origen.0, origen.1)));
            for (x, y) in movimientos {
                verdaderos += usize::from(es.ev(mov(x, y)));
            }
            verdaderos += usize::from(es.ev(fin()));
            prop_assert!(verdaderos <= 1, "{:?}: {} pasos en un gesto", herramienta, verdaderos);
            prop_assert_eq!(
                verdaderos == 1,
                es.imagen() != &antes,
                "{:?}: Ok(true) si y solo si cambió la imagen",
                herramienta
            );
            prop_assert!(!es.tool.busy());
            aplicados += verdaderos;
        }
        let final_ = es.imagen().clone();
        for _ in 0..aplicados {
            prop_assert_eq!(es.doc.undo(), Ok(()));
        }
        prop_assert_eq!(
            es.doc.undo(),
            Err(DocumentError::NothingToUndo),
            "ni un paso de más"
        );
        prop_assert_eq!(es.imagen(), &inicial, "k deshacer dejan la imagen como al principio");
        for _ in 0..aplicados {
            prop_assert_eq!(es.doc.redo(), Ok(()));
        }
        prop_assert_eq!(es.doc.redo(), Err(DocumentError::NothingToRedo));
        prop_assert_eq!(es.imagen(), &final_, "k rehacer la dejan como al final");
    }

    // Propiedad 3: cancelar.
    #[test]
    fn propiedad_3_cancelar_una_linea(
        (image, slot) in imagenes(),
        previo in 0u8..=2,
        a in posicion(),
        movimientos in prop::collection::vec(posicion(), 0..=4),
        corte in 0usize..=4,
        con_escape in any::<bool>(),
    ) {
        let corte = corte.min(movimientos.len());
        let mut es = Escena::nueva(image, Tool::Line, slot);
        paso_previo(&mut es, previo);
        let imagen0 = es.imagen().clone();
        let historia0 = es.historia();

        let cancelar = |es: &mut Escena| -> bool {
            if con_escape {
                es.escape()
            } else {
                es.tool.apply_action(ToolAction::Cancel);
                false
            }
        };

        prop_assert!(!es.ev(begin(a.0, a.1)));
        for &(x, y) in &movimientos[..corte] {
            prop_assert!(!es.ev(mov(x, y)));
        }
        prop_assert!(!cancelar(&mut es), "cancelar no cambia el documento");
        prop_assert!(es.tool.busy(), "ocupado hasta el End");
        prop_assert!(es.preview().is_empty(), "la línea pendiente se descarta");
        for &(x, y) in &movimientos[corte..] {
            prop_assert!(!es.ev(mov(x, y)), "los Move posteriores no aplican nada");
            prop_assert!(es.tool.busy());
            prop_assert!(es.preview().is_empty());
        }
        prop_assert!(!es.ev(fin()), "el End posterior no aplica nada");
        prop_assert!(!es.tool.busy());
        prop_assert_eq!(es.imagen(), &imagen0);
        prop_assert_eq!(es.historia(), historia0);

        // Cancelar después del End: nada de lo aplicado cambia.
        prop_assert!(!es.ev(begin(a.0, a.1)));
        for &(x, y) in &movimientos {
            prop_assert!(!es.ev(mov(x, y)));
        }
        es.ev(fin());
        let imagen1 = es.imagen().clone();
        let historia1 = es.historia();
        prop_assert!(!cancelar(&mut es));
        prop_assert!(!es.tool.busy());
        prop_assert_eq!(es.imagen(), &imagen1);
        prop_assert_eq!(es.historia(), historia1);
    }

    // Propiedad 4: el relleno contra el oráculo ingenuo.
    #[test]
    fn propiedad_4_el_relleno_es_el_del_oraculo_ingenuo(
        (image, slot) in imagenes_de_fondo_corto(),
        semilla in posicion(),
        con_rehacer in any::<bool>(),
    ) {
        let mut es = Escena::nueva(image, Tool::Fill, slot);
        paso_previo(&mut es, if con_rehacer { 2 } else { 0 });
        let base = es.imagen().clone();
        let historia0 = es.historia();
        let pixel = pixel_for_slot(&base, slot);

        // Qué debe pasar, calculado sin la herramienta.
        let dentro = match (u32::try_from(semilla.0), u32::try_from(semilla.1)) {
            (Ok(x), Ok(y)) if x < base.width() && y < base.height() => Some((x, y)),
            _ => None,
        };
        let esperada = match (pixel, dentro) {
            (Some(pixel), Some((x, y))) if base.get_pixel(x, y).unwrap() != pixel => {
                let region = region_ingenua(&base, x, y);
                Some(con_puntos_pintados(&base, region, pixel))
            }
            _ => None,
        };

        let cambio = es.ev(begin(semilla.0, semilla.1));
        prop_assert_eq!(cambio, esperada.is_some(), "Ok(true) si y solo si dentro y distinto");
        match &esperada {
            Some(esperada) => {
                prop_assert_eq!(es.imagen(), esperada);
                prop_assert!(es.doc.can_undo());
            }
            None => {
                prop_assert_eq!(es.imagen(), &base);
                prop_assert_eq!(es.historia(), historia0, "historia intacta sin cambio");
            }
        }
        prop_assert!(!es.ev(mov(semilla.0, semilla.1)));
        prop_assert!(!es.ev(fin()));
        prop_assert!(!es.tool.busy());
        if let Some(esperada) = esperada {
            // Un solo paso, exacto.
            prop_assert_eq!(es.imagen(), &esperada);
            prop_assert_eq!(es.doc.undo(), Ok(()));
            prop_assert_eq!(es.imagen(), &base);
            prop_assert_eq!(es.doc.redo(), Ok(()));
            prop_assert_eq!(es.imagen(), &esperada);
        }
    }

    // Propiedad 6: herramienta estable y `busy` exacto.
    #[test]
    fn propiedad_6_select_solo_cambia_la_herramienta_en_reposo(
        (image, slot) in imagenes(),
        operaciones in prop::collection::vec(operacion(), 0..=40),
    ) {
        let mut es = Escena::nueva(image, Tool::Pencil, slot);
        prop_assert!(!es.tool.busy());
        prop_assert!(es.preview().is_empty(), "vacía en reposo");
        let mut ocupado = false;
        for operacion in operaciones {
            let herramienta_antes = es.tool.tool();
            match operacion {
                Operacion::Begin(p) if !ocupado => {
                    es.ev(begin(p.0, p.1));
                    ocupado = true;
                }
                Operacion::Move(p) if ocupado => {
                    es.ev(mov(p.0, p.1));
                }
                Operacion::End if ocupado => {
                    es.ev(fin());
                    ocupado = false;
                }
                Operacion::Escape => {
                    es.escape();
                }
                Operacion::Cancel => es.tool.apply_action(ToolAction::Cancel),
                Operacion::Select(herramienta) => {
                    es.tool.apply_action(ToolAction::Select(herramienta));
                    let esperada = if ocupado { herramienta_antes } else { herramienta };
                    prop_assert_eq!(es.tool.tool(), esperada, "Select con busy = {}", ocupado);
                }
                _ => {}
            }
            if !matches!(operacion, Operacion::Select(_)) {
                prop_assert_eq!(es.tool.tool(), herramienta_antes, "solo Select cambia la herramienta");
            }
            prop_assert_eq!(es.tool.busy(), ocupado, "busy es cierto entre Begin y End");
            if !ocupado {
                prop_assert!(es.preview().is_empty(), "vacía en reposo");
            }
        }
    }
}

/// Un paso de la secuencia de la propiedad 6. Solo se entregan los que `Canvas` puede emitir:
/// `Begin` en reposo y `Move` y `End` con un gesto abierto; el resto se descarta.
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
        3 => herramienta().prop_map(Operacion::Select),
        1 => Just(Operacion::Cancel),
    ]
}

// Propiedad 5: teclas.

fn modificadores(bits: u8) -> Modifiers {
    Modifiers {
        alt: bits & 1 != 0,
        ctrl: bits & 2 != 0,
        shift: bits & 4 != 0,
        mac_cmd: bits & 8 != 0,
        command: bits & 16 != 0,
    }
}

/// Lo que debe dar `tool_action_for_key`, escrito aparte de la implementación.
fn accion_esperada(key: Key, modifiers: Modifiers) -> Option<ToolAction> {
    let sin_modificadores = !(modifiers.alt
        || modifiers.ctrl
        || modifiers.shift
        || modifiers.mac_cmd
        || modifiers.command);
    if !sin_modificadores {
        return None;
    }
    match key {
        Key::P => Some(ToolAction::Select(Tool::Pencil)),
        Key::L => Some(ToolAction::Select(Tool::Line)),
        Key::F => Some(ToolAction::Select(Tool::Fill)),
        Key::I => Some(ToolAction::Select(Tool::Dropper)),
        Key::E => Some(ToolAction::Select(Tool::Eraser)),
        Key::Escape => Some(ToolAction::Cancel),
        _ => None,
    }
}

fn comprueba_teclas(key: Key, modifiers: Modifiers) -> Result<(), TestCaseError> {
    let accion = tool_action_for_key(key, modifiers);
    prop_assert_eq!(
        accion,
        accion_esperada(key, modifiers),
        "{:?} con {:?}",
        key,
        modifiers
    );
    if matches!(
        key,
        Key::P | Key::L | Key::F | Key::I | Key::E | Key::Escape
    ) {
        prop_assert_eq!(
            action_for_key(key, modifiers),
            None,
            "action_for_key {:?} con {:?}",
            key,
            modifiers
        );
    }
    Ok(())
}

proptest! {
    #[test]
    fn propiedad_5_teclas_por_muestreo(
        key in prop::sample::select(Key::ALL.to_vec()),
        bits in 0u8..32,
    ) {
        comprueba_teclas(key, modificadores(bits))?;
    }
}

#[test]
fn propiedad_5_teclas_exhaustiva_key_all_por_las_32_combinaciones() {
    assert!(!Key::ALL.is_empty());
    for &key in Key::ALL {
        for bits in 0..32 {
            let modifiers = modificadores(bits);
            if let Err(e) = comprueba_teclas(key, modifiers) {
                panic!("{e}");
            }
        }
    }
}

// Unidad 14c: límites del relleno (specs/hito-2-limites-relleno.md, fila 11).

mod limites_relleno {
    use super::*;

    /// Peine RGBA de 16384 x 368 (`S = 3 006 465`, por encima del tope real): fila 0 toda `A`
    /// y, en el resto, `A` en las columnas pares y `B` en las impares.
    fn peine() -> Image {
        let (ancho, alto) = (16384usize, 368usize);
        let a = [10u8, 20, 30, 255];
        let b = [40u8, 50, 60, 255];
        let mut data = Vec::with_capacity(ancho * alto * 4);
        for y in 0..alto {
            for x in 0..ancho {
                data.extend_from_slice(if y == 0 || x % 2 == 0 { &a } else { &b });
            }
        }
        Image::from_bytes(16384, 368, Mode::Rgba, data).expect("imagen válida")
    }

    #[test]
    fn fila_11_un_relleno_demasiado_fragmentado_da_error_y_el_siguiente_clic_funciona() {
        let mut es = Escena::nueva(peine(), Tool::Fill, 3);
        let imagen = es.imagen().clone();
        let antes = es.historia();
        let estado = es.doc.state_id();

        // Clic en (0, 0): el relleno se rechaza.
        let resultado = es
            .tool
            .canvas_event(&mut es.doc, es.id, es.slot, begin(0, 0));
        assert!(
            matches!(resultado, Err(DocumentError::FillTooFragmented { .. })),
            "salió {resultado:?}"
        );
        // Soltar termina el gesto (tras el `Err` queda en espera de su `End`).
        assert!(!es.ev(fin()));
        assert!(!es.tool.busy());

        assert!(
            es.tool.take_changes().is_empty(),
            "no hay cambios que registrar"
        );
        assert!(es.imagen() == &imagen, "la imagen no cambia");
        assert_eq!(es.historia(), antes);
        assert_eq!(es.doc.state_id(), estado);
        assert!(!es.doc.can_undo());

        // El mismo estado de la herramienta rellena un documento de 8 x 8 con el mismo clic.
        let mut pequeno = Document::new(imagen_8x8());
        let id = pequeno.main_image();
        assert_eq!(
            es.tool.canvas_event(&mut pequeno, id, 3, begin(0, 0)),
            Ok(true)
        );
        assert!(!es.ev(fin()));
        assert!(!es.tool.busy());
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(
                    pequeno.image(id).unwrap().get_pixel(x, y),
                    Ok(rojo()),
                    "({x}, {y})"
                );
            }
        }
        assert!(
            !es.tool.take_changes().is_empty(),
            "el relleno sí se registra"
        );
    }
}
