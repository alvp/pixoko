//! La sesión con capas (specs/hito-3-capas-vista.md, unidad 3.3): filas 1, 2, 6, 10, 11 y 12 y
//! propiedades 3, 4 y 5. Sin ventana: `Session`, `ToolState` con eventos a mano y ficheros en un
//! directorio temporal propio.
//!
//! Contrato de API que estas pruebas fijan (la lógica debe ofrecerlo):
//! - `Session`: `active_layer`, `active_cel`, `set_active_layer`, `apply_layer`, `paint_refusal`,
//!   `convert_all`, `layers_info`, `removal_needs_destination`, `begin_removal_for`,
//!   `mark_unmodified`; `ClearPageError::HiddenLayer`; `with_flatten_notice`.
//! - `crate::save_wait` (`SAVING_NOTICE`, `SAVE_SYNC_PIXELS`, `compose_pixels`, `start_save`,
//!   `StartSave`, `PendingSave::tick`).
//! - `TitleInfo.layers` pinta ` · layer_2 {posición}/{total}` tras las coordenadas.

use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;
use px_core::{Allowed, ImageId, LayerId, Mode, Palette, PixelEdit, Rgba};
use px_formats::read_png;
use px_ui::canvas::CanvasEvent;
use px_ui::dirty::BlockSet;
use px_ui::tool::{Tool, ToolAction, ToolState};
use px_ui::view::Zoom;

use crate::save_wait::{SAVE_SYNC_PIXELS, SAVING_NOTICE, StartSave, compose_pixels, start_save};
use crate::title::{TitleInfo, title_text};

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct Tmp {
    path_2: PathBuf,
}

impl Tmp {
    fn new_2() -> Tmp {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path_2 =
            std::env::temp_dir().join(format!("pixoko-px-app-capas-{}-{n}", std::process::id()));
        fs::create_dir_all(&path_2).expect("directorio temporal");
        Tmp { path_2 }
    }

    fn path_2(&self) -> &Path {
        &self.path_2
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path_2);
    }
}

const TEST_RED: Rgba = Rgba::opaque(255, 0, 0);
const TEST_GREEN: Rgba = Rgba::opaque(0, 255, 0);
const TEST_BLUE: Rgba = Rgba::opaque(0, 0, 255);
const TEST_BACKGROUND: Rgba = Rgba::opaque(10, 20, 30);
const HIDDEN: &str = "la capa activa está oculta";

fn rgba_full(width: u32, height: u32, color: Rgba) -> Image {
    let mut image_2 = Image::new(width, height, Mode::Rgba).expect("imagen");
    for y in 0..height {
        for x in 0..width {
            image_2.set_pixel(x, y, Pixel::Rgba(color)).expect("dentro");
        }
    }
    image_2
}

fn session_rgba(width: u32, height: u32) -> Session {
    Session::sample(Document::new(rgba_full(width, height, TEST_BACKGROUND)))
}

/// Añade una capa encima con `apply_layer` y devuelve su `LayerId` y su celda.
fn adds(session_2: &mut Session, name_2: &str) -> (LayerId, ImageId) {
    let at = session_2.document.layers().len();
    session_2
        .apply_layer(Command::AddLayer {
            name: name_2.to_string(),
            at,
        })
        .expect("añadir capa");
    let layer_2 = &session_2.document.layers()[at];
    (layer_2.id(), layer_2.cel())
}

fn paints(document: &mut Document, cel: ImageId, points: &[(u32, u32)], pixel: Pixel) {
    document
        .apply(Command::SetPixels {
            image: cel,
            pixels: points
                .iter()
                .map(|&(x, y)| PixelEdit { x, y, pixel })
                .collect(),
        })
        .expect("pintar");
}

/// Un gesto de `tool_2` con el color de primer plano sobre la capa activa, como la app:
/// primero la guardia (`paint_refusal`), después los eventos y el registro de lo cambiado.
fn gesture(
    session_2: &mut Session,
    tools: &mut ToolState,
    tool_2: Tool,
    points: &[(i32, i32)],
) -> Result<bool, DocumentError> {
    if session_2.paint_refusal(tool_2).is_some() {
        return Ok(false);
    }
    let cel = session_2.active_cel();
    let pixel = session_2.pixel_of(Role::Foreground);
    tools.apply_action(ToolAction::Select(tool_2));
    let mut change = false;
    let mut events = vec![CanvasEvent::StrokeBegin(points[0])];
    events.extend(points[1..].iter().map(|&p| CanvasEvent::StrokeMove(p)));
    events.push(CanvasEvent::StrokeEnd);
    let mut result = Ok(());
    for event_2 in events {
        match tools.canvas_event_with(&mut session_2.document, cel, pixel, event_2) {
            Ok(c) => change |= c,
            Err(error) => {
                result = Err(error);
                break;
            }
        }
    }
    session_2.absorb(tools.take_changes());
    result.map(|()| change)
}

// ---------------------------------------------------------------------------------------------
// Fila 1: la capa activa
// ---------------------------------------------------------------------------------------------

#[test]
fn row_1_by_default_the_active_one_is_layer_0_and_its_cel() {
    let mut session_2 = session_rgba(8, 6);
    assert_eq!(
        session_2.active_layer(),
        session_2.document.layers()[0].id()
    );
    assert_eq!(session_2.active_cel(), session_2.document.layers()[0].cel());
    let (layer_c, cel_c) = {
        adds(&mut session_2, "B");
        adds(&mut session_2, "C")
    };
    session_2.set_active_layer(layer_c).expect("existe");
    assert_eq!(session_2.active_cel(), cel_c);
    assert_ne!(session_2.active_cel(), ImageId::from_raw(0));
}

#[test]
fn row_1_changing_layer_does_not_touch_the_document_the_history_or_the_modified_flag() {
    let mut session_2 = session_rgba(8, 6);
    let (layer_b, _) = adds(&mut session_2, "B");
    session_2.mark_unmodified();
    let (state_2, copy) = (session_2.document.state_id(), session_2.document.clone());
    let (undo, redo) = (session_2.document.can_undo(), session_2.document.can_redo());
    session_2.set_active_layer(layer_b).expect("existe");
    let unknown = LayerId::from_raw(4242);
    assert_eq!(
        session_2.set_active_layer(unknown),
        Err(DocumentError::UnknownLayer(unknown))
    );
    assert_eq!(
        session_2.active_layer(),
        layer_b,
        "un error no cambia la activa"
    );
    assert_eq!(session_2.document.state_id(), state_2);
    assert_eq!(session_2.document, copy);
    assert_eq!(
        (session_2.document.can_undo(), session_2.document.can_redo()),
        (undo, redo)
    );
    assert!(!session_2.modified());
}

#[test]
fn row_1_opening_or_creating_a_document_resets_the_active_one() {
    let mut session_2 = session_rgba(8, 6);
    let (layer_b, _) = adds(&mut session_2, "B");
    session_2.set_active_layer(layer_b).expect("existe");
    session_2
        .new_image(rgba_full(4, 4, TEST_BACKGROUND), true)
        .expect("nueva imagen");
    assert_eq!(
        session_2.active_layer(),
        session_2.document.layers()[0].id()
    );
    assert_eq!(session_2.document.layers().len(), 1);
}

#[test]
fn row_1_if_the_active_one_disappears_it_moves_to_the_neighbor() {
    // [A, B, C] con B activa: al quitarla pasa a la que ocupa su posición (C).
    let mut session_2 = session_rgba(8, 6);
    let (layer_b, _) = adds(&mut session_2, "B");
    let (layer_c, _) = adds(&mut session_2, "C");
    session_2.set_active_layer(layer_b).expect("existe");
    session_2
        .apply_layer(Command::RemoveLayer { layer: layer_b })
        .expect("quitar");
    assert_eq!(session_2.active_layer(), layer_c);
    // La última activa: pasa a la nueva última.
    session_2.set_active_layer(layer_c).expect("existe");
    session_2
        .apply_layer(Command::RemoveLayer { layer: layer_c })
        .expect("quitar");
    assert_eq!(
        session_2.active_layer(),
        session_2.document.layers()[0].id()
    );
    // La de abajo con otra encima: la que queda en su posición.
    let (layer_d, _) = adds(&mut session_2, "D");
    let below = session_2.document.layers()[0].id();
    session_2.set_active_layer(below).expect("existe");
    session_2
        .apply_layer(Command::RemoveLayer { layer: below })
        .expect("quitar");
    assert_eq!(session_2.active_layer(), layer_d);
}

#[test]
fn row_1_undoing_the_add_of_the_active_layer_moves_to_the_last() {
    let mut session_2 = session_rgba(8, 6);
    let (layer_b, _) = adds(&mut session_2, "B");
    session_2.set_active_layer(layer_b).expect("existe");
    session_2.undo().expect("deshacer");
    assert_eq!(session_2.document.layers().len(), 1);
    assert_eq!(
        session_2.active_layer(),
        session_2.document.layers()[0].id()
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 2: pintar sobre la celda activa
// ---------------------------------------------------------------------------------------------

#[test]
fn row_2_the_pencil_on_an_empty_cel_materializes_it_in_one_step_and_undo_empties_it() {
    let mut session_2 = session_rgba(8, 6);
    let (_, cel_b) = adds(&mut session_2, "B");
    let (layer_c, _) = adds(&mut session_2, "C");
    let layer_b = session_2.document.layers()[1].id();
    let _ = layer_c;
    session_2.set_active_layer(layer_b).expect("existe");
    session_2.slots.get_mut(Mode::Rgba).set(1, TEST_RED);
    session_2.choice = ColorChoice::Slot(1);
    let before = session_2.document.clone();
    let cel_a = session_2.document.layers()[0].cel();
    let mut tools = ToolState::new();
    assert_eq!(
        gesture(&mut session_2, &mut tools, Tool::Pencil, &[(1, 1), (4, 1)]),
        Ok(true)
    );
    let image_2 = session_2.document.image(cel_b).expect("materializada");
    assert_eq!(image_2.get_pixel(3, 1), Ok(Pixel::Rgba(TEST_RED)));
    assert_eq!(session_2.document.image(cel_a), before.image(cel_a));
    session_2.undo().expect("un solo paso");
    assert_eq!(session_2.document, before);
    assert!(session_2.document.image(cel_b).is_none());
    session_2.redo().expect("rehacer");
    assert!(session_2.document.image(cel_b).is_some());
}

#[test]
fn row_2_without_memory_for_the_cel_the_document_stays_intact() {
    let mut session_2 = session_rgba(8, 6);
    adds(&mut session_2, "B");
    let layer_b = session_2.document.layers()[1].id();
    session_2.set_active_layer(layer_b).expect("existe");
    session_2.slots.get_mut(Mode::Rgba).set(1, TEST_RED);
    session_2.choice = ColorChoice::Slot(1);
    let limit_2 = session_2.document.cel_bytes();
    session_2.document.set_cells_limit(limit_2);
    let before = session_2.document.clone();
    let mut tools = ToolState::new();
    let result = gesture(&mut session_2, &mut tools, Tool::Pencil, &[(1, 1), (4, 1)]);
    assert!(
        matches!(result, Err(DocumentError::CelsTooLarge { .. })),
        "{result:?}"
    );
    assert_eq!(session_2.document, before);
    assert_eq!(session_2.document.state_id(), before.state_id());
    assert!(!session_2.document.can_undo() || before.can_undo());
}

#[test]
fn row_2_clear_page_acts_on_the_active_one() {
    let mut session_2 = session_rgba(6, 4);
    let (_, cel_b) = adds(&mut session_2, "B");
    let layer_b = session_2.document.layers()[1].id();
    session_2.set_active_layer(layer_b).expect("existe");
    // Con el fondo transparente por defecto sobre una celda vacía no hay nada que cambiar.
    let before = session_2.document.clone();
    assert_eq!(session_2.clear_page(), Ok(false));
    assert_eq!(session_2.document, before);
    assert!(session_2.document.image(cel_b).is_none());
    // Con fondo rojo la rellena (se materializa) en un paso.
    session_2.slots.get_mut(Mode::Rgba).set(3, TEST_RED);
    session_2.background = ColorChoice::Slot(3);
    assert_eq!(session_2.clear_page(), Ok(true));
    assert_eq!(
        session_2
            .document
            .image(cel_b)
            .expect("materializada")
            .get_pixel(5, 3),
        Ok(Pixel::Rgba(TEST_RED))
    );
    session_2.undo().expect("un paso");
    assert_eq!(session_2.document, before);
}

// ---------------------------------------------------------------------------------------------
// Fila 6: la capa activa oculta no se pinta
// ---------------------------------------------------------------------------------------------

#[test]
fn row_6_with_the_active_one_hidden_nothing_paints_and_the_dropper_works() {
    let mut session_2 = session_rgba(8, 6);
    let (layer_b, _) = adds(&mut session_2, "B");
    session_2.set_active_layer(layer_b).expect("existe");
    session_2
        .apply_layer(Command::SetLayerVisible {
            layer: layer_b,
            visible: false,
        })
        .expect("ocultar");
    for tool_2 in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Eraser] {
        assert_eq!(session_2.paint_refusal(tool_2), Some(HIDDEN), "{tool_2:?}");
    }
    assert_eq!(session_2.paint_refusal(Tool::Dropper), None);
    let before = session_2.document.clone();
    let mut tools = ToolState::new();
    assert_eq!(
        gesture(&mut session_2, &mut tools, Tool::Pencil, &[(1, 1), (3, 1)]),
        Ok(false)
    );
    session_2.slots.get_mut(Mode::Rgba).set(3, TEST_RED);
    session_2.background = ColorChoice::Slot(3);
    assert_eq!(session_2.clear_page(), Err(ClearPageError::HiddenLayer));
    assert_eq!(ClearPageError::HiddenLayer.to_string(), HIDDEN);
    assert_eq!(session_2.document, before);
    assert_eq!(session_2.document.state_id(), before.state_id());
    // Al mostrarla se vuelve a pintar.
    session_2
        .apply_layer(Command::SetLayerVisible {
            layer: layer_b,
            visible: true,
        })
        .expect("mostrar");
    assert_eq!(session_2.paint_refusal(Tool::Pencil), None);
    assert_eq!(session_2.clear_page(), Ok(true));
}

// ---------------------------------------------------------------------------------------------
// Fila 8 (sesión): `apply_layer` anota la región sucia
// ---------------------------------------------------------------------------------------------

#[test]
fn row_8_apply_layer_notes_the_dirty_region_and_undo_invalidates_it() {
    let mut session_2 = session_rgba(70, 70);
    let _ = session_2.take_dirty();
    let (layer_b, _) = adds(&mut session_2, "B");
    assert!(
        session_2.take_dirty().is_empty(),
        "añadir una capa vacía no cambia lo que se ve"
    );
    session_2
        .apply_layer(Command::RenameLayer {
            layer: layer_b,
            name: "Z".to_string(),
        })
        .expect("renombrar");
    assert!(session_2.take_dirty().is_empty());
    session_2
        .apply_layer(Command::SetLayerVisible {
            layer: layer_b,
            visible: false,
        })
        .expect("ocultar");
    assert_eq!(session_2.take_dirty(), BlockSet::full(70, 70));
    // Un comando sin cambios no ensucia ni deja paso.
    let state_2 = session_2.document.state_id();
    assert_eq!(
        session_2.apply_layer(Command::SetLayerVisible {
            layer: layer_b,
            visible: false
        }),
        Ok(false)
    );
    assert_eq!(session_2.document.state_id(), state_2);
    // Deshacer lo que ensució todo lo vuelve a ensuciar todo.
    session_2.undo().expect("deshacer");
    assert_eq!(session_2.take_dirty(), BlockSet::full(70, 70));
    session_2.redo().expect("rehacer");
    assert_eq!(session_2.take_dirty(), BlockSet::full(70, 70));
}

// ---------------------------------------------------------------------------------------------
// Fila 10: indexado, paleta y conversión con varias capas
// ---------------------------------------------------------------------------------------------

fn palette_of_six() -> Palette {
    let mut palette = Palette::from_colors([
        Rgba::opaque(0, 0, 0),
        Rgba::opaque(200, 20, 20),
        Rgba::opaque(20, 200, 20),
        Rgba::opaque(20, 20, 200),
        Rgba::opaque(200, 200, 20),
        Rgba::opaque(255, 255, 255),
    ])
    .expect("paleta");
    palette.set_transparent_index(Some(5)).expect("existe");
    palette
}

fn indexed_of(width: u32, height: u32, index_2: u8) -> Image {
    let mut image_2 = Image::new_indexed(width, height, palette_of_six()).expect("imagen");
    for y in 0..height {
        for x in 0..width {
            image_2
                .set_pixel(x, y, Pixel::Indexed(index_2))
                .expect("dentro");
        }
    }
    image_2
}

#[test]
fn row_10_with_the_active_cel_empty_the_color_comes_from_the_canvas() {
    let image_2 = indexed_of(6, 4, 1);
    let one = Session::sample(Document::new(image_2.clone()));
    let mut two = Session::sample(Document::new(image_2));
    adds(&mut two, "B");
    let layer_b = two.document.layers()[1].id();
    two.set_active_layer(layer_b).expect("existe");
    assert!(two.document.image(two.active_cel()).is_none());
    assert_eq!(two.choice, one.choice);
    assert_eq!(two.background, one.background);
    assert_eq!(
        two.pixel_of(Role::Foreground),
        one.pixel_of(Role::Foreground)
    );
    assert_eq!(
        two.pixel_of(Role::Background),
        one.pixel_of(Role::Background)
    );
    let mut one = one;
    one.cycle_choice(1);
    two.cycle_choice(1);
    assert_eq!(two.choice, one.choice);
    two.sync_editor();
    one.sync_editor();
    assert_eq!(two.editor.pending_target(), one.editor.pending_target());
}

#[test]
fn row_10_removing_an_entry_another_cel_uses_asks_for_a_target() {
    let mut base = indexed_of(6, 4, 1);
    base.set_pixel(2, 2, Pixel::Indexed(3)).expect("dentro");
    let mut session_2 = Session::sample(Document::new(base));
    adds(&mut session_2, "B");
    let layer_b = session_2.document.layers()[1].id();
    session_2.set_active_layer(layer_b).expect("existe");
    assert!(session_2.document.image(session_2.active_cel()).is_none());
    // La 3 solo la usa la celda de abajo, no la activa.
    assert!(session_2.removal_needs_destination(3));
    assert!(!session_2.removal_needs_destination(4));
    session_2.choice = ColorChoice::Entry(3);
    let before = session_2.document.clone();
    assert!(session_2.begin_removal_for(3));
    assert_eq!(session_2.ops.removal().map(|r| r.index), Some(3));
    assert_eq!(
        session_2.document, before,
        "pedir el destino no toca el documento"
    );
    // Confirmar con destino la quita de todas las celdas en un paso.
    let output = session_2
        .apply_palette_op(PaletteOp::Remove { index: 3, into: 1 }, false)
        .expect("quitar");
    assert!(output.changed);
    let cel_a = session_2.document.layers()[0].cel();
    let image_2 = session_2.document.image(cel_a).expect("imagen");
    assert_eq!(image_2.get_pixel(2, 2), Ok(Pixel::Indexed(1)));
    assert_eq!(session_2.document.palette().expect("paleta").len(), 5);
    session_2.undo().expect("un paso");
    assert_eq!(session_2.document, before);
}

#[test]
fn row_10_removing_an_entry_nobody_uses_removes_it_outright() {
    let mut session_2 = Session::sample(Document::new(indexed_of(6, 4, 1)));
    adds(&mut session_2, "B");
    session_2.choice = ColorChoice::Entry(4);
    assert!(!session_2.removal_needs_destination(4));
    let output = session_2
        .apply_palette_op(PaletteOp::Remove { index: 4, into: 0 }, true)
        .expect("quitar");
    assert!(output.changed);
    assert!(session_2.ops.removal().is_none());
}

#[test]
fn row_10_converting_the_mode_with_several_layers_is_no_longer_rejected() {
    // Antes (3.3) esta prueba afirmaba que con dos capas `convert_refusal()` daba el aviso «no se
    // puede convertir el modo con varias capas: deja una sola o espera a la unidad 3.5». Desde
    // 3.5 existe `ConvertAll`: ya no hay rechazo ni aviso y la sesión convierte todas las capas.
    let mut session_2 = session_rgba(4, 4);
    adds(&mut session_2, "B");
    let allowed = Allowed {
        luma: true,
        drop_alpha: None,
    };
    assert_eq!(session_2.convert_all(Mode::GrayAlpha, allowed), Ok(true));
    assert_eq!(session_2.document.mode(), Mode::GrayAlpha);
    assert_eq!(session_2.document.layers().len(), 2);
}

#[test]
fn row_10_digits_and_tools_work_with_the_active_cel_empty() {
    use crate::dispatch::{Dispatched, Targets, dispatch};
    use px_ui::actions::{AppAction, Locks, Origin};
    fn dispatches(session_2: &mut Session, tools: &mut ToolState, action: AppAction) -> Dispatched {
        dispatch(
            action,
            Origin::Key,
            &mut Targets {
                session: session_2,
                tools,
                locks: Locks::default(),
            },
        )
    }
    let mut session_2 = Session::sample(Document::new(indexed_of(6, 4, 1)));
    adds(&mut session_2, "B");
    let layer_b = session_2.document.layers()[1].id();
    session_2.set_active_layer(layer_b).expect("existe");
    assert!(session_2.document.image(session_2.active_cel()).is_none());
    let mut tools = ToolState::new();
    // La cifra 3 elige la entrada 2 aunque la celda activa no tenga imagen.
    let done = dispatches(&mut session_2, &mut tools, AppAction::SelectColor(3));
    assert!(matches!(done, Dispatched::Done { .. }));
    assert_eq!(session_2.choice, ColorChoice::Entry(2));
    // El borrador se elige y su aviso sale del lienzo (la paleta tiene índice transparente).
    let Dispatched::Done { notice, .. } = dispatches(
        &mut session_2,
        &mut tools,
        AppAction::SelectTool(Tool::Eraser),
    ) else {
        panic!("se despacha");
    };
    assert_eq!(notice, None);
    assert_eq!(tools.tool(), Tool::Eraser);
}

// ---------------------------------------------------------------------------------------------
// Fila 11: el guardado aplanado
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_with_one_layer_the_cel_is_saved_as_is() {
    let mut image_2 = rgba_full(5, 4, TEST_BACKGROUND);
    image_2
        .set_pixel(2, 2, Pixel::Rgba(Rgba::new(9, 8, 7, 0)))
        .expect("dentro");
    let mut session_2 = Session::sample(Document::new(image_2.clone()));
    let tmp = Tmp::new_2();
    let saved = session_2.save_as(tmp.path_2()).expect("guardar");
    assert_eq!(
        fs::read(&saved.path).expect("leer"),
        px_formats::write_png(&image_2).expect("PNG")
    );
}

#[test]
fn row_11_with_three_layers_the_visible_is_saved_and_the_document_does_not_change() {
    let mut session_2 = session_rgba(6, 4);
    let (_, cel_b) = adds(&mut session_2, "B");
    let (layer_c, cel_c) = adds(&mut session_2, "C");
    paints(
        &mut session_2.document,
        cel_b,
        &[(1, 1)],
        Pixel::Rgba(TEST_RED),
    );
    paints(
        &mut session_2.document,
        cel_c,
        &[(2, 2)],
        Pixel::Rgba(TEST_BLUE),
    );
    session_2
        .apply_layer(Command::SetLayerVisible {
            layer: layer_c,
            visible: false,
        })
        .expect("ocultar");
    let copy = session_2.document.clone();
    let tmp = Tmp::new_2();
    let saved = session_2.save_as(tmp.path_2()).expect("guardar");
    let read = read_png(&fs::read(&saved.path).expect("leer"))
        .expect("PNG")
        .image;
    let mut expected = rgba_full(6, 4, TEST_BACKGROUND);
    expected
        .set_pixel(1, 1, Pixel::Rgba(TEST_RED))
        .expect("dentro");
    assert_eq!(read, expected, "la capa oculta no entra");
    assert_eq!(session_2.document, copy);
    assert!(!session_2.modified());
}

#[test]
fn row_11_a_gray_with_everything_hidden_is_not_saved_and_stays_modified() {
    let mut session_2 =
        Session::sample(Document::new(Image::new(3, 3, Mode::Gray).expect("imagen")));
    let layer_2 = session_2.document.layers()[0].id();
    session_2
        .apply_layer(Command::SetLayerVisible {
            layer: layer_2,
            visible: false,
        })
        .expect("ocultar");
    assert!(session_2.modified());
    let tmp = Tmp::new_2();
    let result = session_2.save_as(tmp.path_2());
    assert!(matches!(result, Err(FileError::Write(_))), "{result:?}");
    assert!(session_2.modified());
    assert_eq!(fs::read_dir(tmp.path_2()).expect("directorio").count(), 0);
}

/// Una sesión de una sola capa, oculta: gris, indexado sin índice transparente o RGBA.
fn session_with_the_only_layer_hidden(mode_2: Mode) -> Session {
    let image_2 = match mode_2 {
        Mode::Indexed => Image::new_indexed(
            3,
            3,
            Palette::from_colors([Rgba::opaque(1, 2, 3), Rgba::opaque(4, 5, 6)]).expect("paleta"),
        )
        .expect("imagen"),
        _ => Image::new(3, 3, mode_2).expect("imagen"),
    };
    let mut session_2 = Session::sample(Document::new(image_2));
    let layer_2 = session_2.document.layers()[0].id();
    session_2
        .apply_layer(Command::SetLayerVisible {
            layer: layer_2,
            visible: false,
        })
        .expect("ocultar");
    session_2
}

#[test]
fn with_nothing_visible_save_and_export_png_give_an_error_that_says_what_to_do() {
    for (mode_2, remedy) in [
        (Mode::Gray, "muestra una capa"),
        (
            Mode::Indexed,
            "muestra una capa o fija una entrada transparente",
        ),
    ] {
        let tmp = Tmp::new_2();
        let mut session_2 = session_with_the_only_layer_hidden(mode_2);
        let save = session_2
            .save_as(tmp.path_2())
            .expect_err("guardar")
            .to_string();
        let export = session_2
            .export_flat(&tmp.path_2().join("plano.png"))
            .expect_err("exportar")
            .to_string();
        for text_2 in [save, export] {
            assert!(
                text_2.contains(&format!("no hay nada visible que guardar: {remedy}")),
                "{mode_2:?}: {text_2}"
            );
            assert!(!text_2.contains("NoTransparency"), "{mode_2:?}: {text_2}");
        }
        assert!(session_2.modified(), "{mode_2:?}");
        assert_eq!(fs::read_dir(tmp.path_2()).expect("directorio").count(), 0);
    }
}

#[test]
fn with_nothing_visible_an_rgba_is_saved_as_a_transparent_png() {
    let tmp = Tmp::new_2();
    let mut session_2 = session_with_the_only_layer_hidden(Mode::Rgba);
    let saved = session_2.save_as(tmp.path_2()).expect("guardar");
    let read = read_png(&fs::read(&saved.path).expect("leer")).expect("PNG");
    let transparent = (0..3).all(|y| {
        (0..3).all(|x| matches!(read.image.get_pixel(x, y), Ok(Pixel::Rgba(c)) if c.a == 0))
    });
    assert!(transparent);
}

#[test]
fn row_11_the_notice_says_flattened_only_with_more_than_one_layer() {
    let mut session_2 = session_rgba(4, 4);
    let text_2 = || "guardado a.png".to_string();
    assert_eq!(with_flatten_notice(text_2(), &session_2.document), text_2());
    let (_, _) = adds(&mut session_2, "B");
    let (layer_c, _) = adds(&mut session_2, "C");
    let plane = " · se guarda aplanado; con Guardar como… .aseprite conservas las capas";
    assert_eq!(
        with_flatten_notice(text_2(), &session_2.document),
        format!("{}{plane}", text_2())
    );
    session_2
        .apply_layer(Command::SetLayerVisible {
            layer: layer_c,
            visible: false,
        })
        .expect("ocultar");
    assert_eq!(
        with_flatten_notice(text_2(), &session_2.document),
        format!("{}{plane} · 1 capa oculta no se guarda", text_2())
    );
    let layer_b = session_2.document.layers()[1].id();
    session_2
        .apply_layer(Command::SetLayerVisible {
            layer: layer_b,
            visible: false,
        })
        .expect("ocultar");
    assert_eq!(
        with_flatten_notice(text_2(), &session_2.document),
        format!("{}{plane} · 2 capas ocultas no se guardan", text_2())
    );
}

#[test]
fn row_11_saving_asks_for_two_frames_first_if_there_is_a_lot_to_compose() {
    assert_eq!(SAVING_NOTICE, "Guardando…");
    assert_eq!(SAVE_SYNC_PIXELS, 1 << 24);
    // Hasta 2^24 píxeles de capa visible se guarda en el acto.
    assert_eq!(start_save(0), StartSave::Now);
    assert_eq!(start_save(SAVE_SYNC_PIXELS), StartSave::Now);
    // Con más, el aviso se ve antes: la acción llega en el segundo fotograma, una sola vez.
    let StartSave::Later(mut waits) = start_save(SAVE_SYNC_PIXELS + 1) else {
        panic!("más de 2^24 píxeles esperan");
    };
    assert!(!waits.tick(), "fotograma 1: solo el aviso");
    assert!(waits.tick(), "fotograma 2: guarda");
    assert!(!waits.tick(), "nunca más");
    assert!(!waits.tick());
}

#[test]
fn row_11_the_pixels_to_compose_are_width_times_height_times_visible_layers_except_the_identity() {
    let mut session_2 = session_rgba(4, 4);
    assert_eq!(
        compose_pixels(&session_2.document),
        0,
        "una capa visible y opaca: nada que componer"
    );
    let (_, cel_b) = adds(&mut session_2, "B");
    let (layer_c, cel_c) = adds(&mut session_2, "C");
    // B vacía: no cuenta; A y C sí.
    paints(
        &mut session_2.document,
        cel_c,
        &[(0, 0)],
        Pixel::Rgba(TEST_RED),
    );
    assert_eq!(compose_pixels(&session_2.document), 4 * 4 * 2);
    paints(
        &mut session_2.document,
        cel_b,
        &[(0, 0)],
        Pixel::Rgba(TEST_RED),
    );
    assert_eq!(compose_pixels(&session_2.document), 4 * 4 * 3);
    session_2
        .apply_layer(Command::SetLayerVisible {
            layer: layer_c,
            visible: false,
        })
        .expect("ocultar");
    assert_eq!(
        compose_pixels(&session_2.document),
        4 * 4 * 2,
        "la oculta no cuenta"
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 12: título y variable de depuración
// ---------------------------------------------------------------------------------------------

fn information(layers_2: Option<(usize, usize)>) -> TitleInfo {
    TitleInfo {
        size: Some((64, 48)),
        zoom: Zoom::from_int(8).expect("zoom"),
        hover: Some((3, 4)),
        tool: Some(Tool::Pencil),
        color: Some("color 1 #000000".to_string()),
        can_undo: false,
        can_redo: false,
        error: None,
        layers: layers_2,
    }
}

#[test]
fn row_12_the_title_carries_the_layer_only_with_more_than_one() {
    assert_eq!(
        title_text(&information(None)),
        "Pixoko: 64×48 · ×8 · (3, 4) · lápiz · color 1 #000000"
    );
    assert_eq!(
        title_text(&information(Some((2, 3)))),
        "Pixoko: 64×48 · ×8 · (3, 4) · capa 2/3 · lápiz · color 1 #000000"
    );
    let mut session_2 = session_rgba(4, 4);
    assert_eq!(session_2.layers_info(), None);
    adds(&mut session_2, "B");
    adds(&mut session_2, "C");
    let layer_b = session_2.document.layers()[1].id();
    session_2.set_active_layer(layer_b).expect("existe");
    assert_eq!(session_2.layers_info(), Some((2, 3)));
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3: un gesto solo toca la capa activa y se deshace exactamente
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_3_a_gesture_only_touches_the_active_layer_and_is_undone_exactly(
        layers_2 in prop::collection::vec((0u8..2, 0u8..3), 0..3),
        steps_2 in prop::collection::vec((0u8..7, 0u8..4, any::<u32>(), any::<u32>(), any::<u32>()), 1..8),
    ) {
        let mut session_2 = session_rgba(12, 9);
        session_2.slots.get_mut(Mode::Rgba).set(1, TEST_RED);
        session_2.slots.get_mut(Mode::Rgba).set(3, TEST_GREEN);
        for (i, &(fill, _)) in layers_2.iter().enumerate() {
            let (_, cel) = adds(&mut session_2, &format!("C{i}"));
            if fill != 0 {
                paints(&mut session_2.document, cel, &[(i as u32, 1), (2, i as u32)], Pixel::Rgba(TEST_BLUE));
            }
        }
        let mut tools = ToolState::new();
        for &(kind, layer_2, a, b, c) in &steps_2 {
            let total = session_2.document.layers().len();
            let id = session_2.document.layers()[usize::from(layer_2) % total].id();
            session_2.set_active_layer(id).expect("existe");
            match kind {
                5 => {
                    let visible = a % 2 == 0;
                    let _ = session_2.apply_layer(Command::SetLayerVisible { layer: id, visible });
                    continue;
                }
                6 => {
                    let limit_2 = if a % 2 == 0 { session_2.document.cel_bytes() } else { usize::MAX };
                    session_2.document.set_cells_limit(limit_2);
                    continue;
                }
                _ => {}
            }
            let before = session_2.document.clone();
            let cels: Vec<ImageId> = before.layers().iter().map(|l| l.cel()).collect();
            let active_2 = session_2.active_cel();
            let points = [
                ((a % 14) as i32 - 1, (b % 11) as i32 - 1),
                ((b % 14) as i32 - 1, (c % 11) as i32 - 1),
            ];
            let result = match kind {
                0 => gesture(&mut session_2, &mut tools, Tool::Pencil, &points),
                1 => gesture(&mut session_2, &mut tools, Tool::Line, &points),
                2 => gesture(&mut session_2, &mut tools, Tool::Fill, &points[..1]),
                3 => gesture(&mut session_2, &mut tools, Tool::Eraser, &points),
                _ => session_2.clear_page().map_err(|e| match e {
                    ClearPageError::Document(error) => error,
                    ClearPageError::OutOfMemory => DocumentError::OutOfMemory,
                    ClearPageError::HiddenLayer
                    | ClearPageError::LockedLayer
                    | ClearPageError::HiddenAndLockedLayer => DocumentError::EmptyCommand,
                }),
            };
            match result {
                Ok(true) => {
                    for cel in cels.iter().filter(|&&cel| cel != active_2) {
                        prop_assert_eq!(session_2.document.image(*cel), before.image(*cel));
                    }
                    let after_2 = session_2.document.clone();
                    session_2.undo().expect("deshacer");
                    prop_assert_eq!(&session_2.document, &before);
                    prop_assert_eq!(session_2.document.state_id(), before.state_id());
                    session_2.redo().expect("rehacer");
                    prop_assert_eq!(&session_2.document, &after_2);
                }
                Ok(false) | Err(_) => {
                    prop_assert_eq!(&session_2.document, &before);
                    prop_assert_eq!(session_2.document.state_id(), before.state_id());
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 4: el PNG guardado es la composición
// ---------------------------------------------------------------------------------------------

/// El oráculo de la composición en el dominio de la prueba (capas a opacidad 255 y Normal, con
/// píxeles opacos o exactamente transparentes): gana la capa visible más alta con un píxel que
/// no es transparente. La mezcla con alfa y modos se verifica contra `compose` en `px-ui`.
fn oracle(document: &Document) -> Image {
    let (width, height) = (document.width(), document.height());
    let transparent = document.palette().and_then(|p| p.transparent_index());
    let mut output = match document.palette() {
        Some(palette) => Image::new_indexed(width, height, palette.clone()).expect("imagen"),
        None => Image::new(width, height, document.mode()).expect("imagen"),
    };
    let empty = match document.mode() {
        Mode::Indexed => Pixel::Indexed(transparent.expect("transparente")),
        _ => Pixel::Rgba(Rgba::TRANSPARENT),
    };
    for y in 0..height {
        for x in 0..width {
            let mut pixel = empty;
            for layer_2 in document.layers() {
                if !layer_2.visible() {
                    continue;
                }
                let Some(cel) = document.image(layer_2.cel()) else {
                    continue;
                };
                let value = cel.get_pixel(x, y).expect("dentro");
                let count = match value {
                    Pixel::Rgba(color) => color.a != 0,
                    Pixel::Indexed(i) => Some(i) != transparent,
                    _ => true,
                };
                if count {
                    pixel = value;
                }
            }
            output.set_pixel(x, y, pixel).expect("dentro");
        }
    }
    output
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_4_the_saved_png_is_the_composition(
        indexed_2 in any::<bool>(),
        layers_2 in prop::collection::vec((any::<bool>(), any::<u32>(), any::<u32>()), 0..4),
    ) {
        let base = if indexed_2 { indexed_of(5, 4, 1) } else { rgba_full(5, 4, TEST_BACKGROUND) };
        let mut session_2 = Session::sample(Document::new(base));
        for (i, &(visible, a, b)) in layers_2.iter().enumerate() {
            let (id, cel) = adds(&mut session_2, &format!("C{i}"));
            if a % 3 != 0 {
                let pixel = if indexed_2 {
                    Pixel::Indexed((a % 6) as u8)
                } else {
                    Pixel::Rgba([TEST_RED, TEST_GREEN, TEST_BLUE, Rgba::TRANSPARENT][(a % 4) as usize])
                };
                paints(&mut session_2.document, cel, &[(a % 5, b % 4), ((a / 5) % 5, (b / 4) % 4)], pixel);
            }
            if !visible {
                session_2.apply_layer(Command::SetLayerVisible { layer: id, visible: false }).expect("ocultar");
            }
        }
        let copy = session_2.document.clone();
        let tmp = Tmp::new_2();
        let saved = session_2.save_as(tmp.path_2()).expect("guardar");
        let read = read_png(&fs::read(&saved.path).expect("leer")).expect("PNG").image;
        // El PNG guarda el índice transparente como alfa 0 de la entrada: se compara lo que se ve.
        let expected = oracle(&session_2.document);
        prop_assert_eq!((read.width(), read.height()), (expected.width(), expected.height()));
        prop_assert_eq!(px_ui::texture::to_rgba8(&read), px_ui::texture::to_rgba8(&expected));
        prop_assert_eq!(&session_2.document, &copy);
        prop_assert!(!session_2.modified());
        let notice = with_flatten_notice("guardado".to_string(), &session_2.document);
        prop_assert_eq!(notice.contains("aplanado"), session_2.document.layers().len() > 1);
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 5: ninguna celda referencia una entrada de paleta que no existe
// ---------------------------------------------------------------------------------------------

fn consistent(document: &Document) -> Result<(), String> {
    let palette = document.palette().expect("indexado");
    for layer_2 in document.layers() {
        let Some(cel) = document.image(layer_2.cel()) else {
            continue;
        };
        if cel.palette() != Some(palette) {
            return Err("una celda tiene otra paleta".to_string());
        }
        for y in 0..cel.height() {
            for x in 0..cel.width() {
                match cel.get_pixel(x, y) {
                    Ok(Pixel::Indexed(i)) if usize::from(i) < palette.len() => {}
                    other_2 => {
                        return Err(format!("({x}, {y}) de {:?}: {other_2:?}", layer_2.id()));
                    }
                }
            }
        }
    }
    Ok(())
}

fn uses_some_cel(document: &Document, index_2: u8) -> bool {
    document.layers().iter().any(|layer_2| {
        document.image(layer_2.cel()).is_some_and(|cel| {
            (0..cel.height()).any(|y| {
                (0..cel.width()).any(|x| cel.get_pixel(x, y) == Ok(Pixel::Indexed(index_2)))
            })
        })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_5_no_cel_references_an_entry_that_does_not_exist(
        layers_2 in prop::collection::vec((any::<bool>(), any::<u32>()), 0..4),
        steps_2 in prop::collection::vec((0u8..9, any::<u32>(), any::<u32>(), any::<u32>()), 0..20),
    ) {
        let mut session_2 = Session::sample(Document::new(indexed_of(4, 4, 1)));
        for (i, &(content, a)) in layers_2.iter().enumerate() {
            let (_, cel) = adds(&mut session_2, &format!("C{i}"));
            if content {
                paints(&mut session_2.document, cel, &[(a % 4, (a / 4) % 4)], Pixel::Indexed((a % 6) as u8));
            }
        }
        prop_assert_eq!(consistent(&session_2.document), Ok(()));
        for &(kind, a, b, c) in &steps_2 {
            let length = session_2.document.palette().expect("paleta").len();
            let total = session_2.document.layers().len();
            let id = session_2.document.layers()[(a as usize) % total].id();
            session_2.set_active_layer(id).expect("existe");
            match kind {
                0 => { let _ = session_2.apply_palette_op(PaletteOp::Add, false); }
                1 | 2 if length > 1 => {
                    let index_2 = (b as usize % length) as u8;
                    let target_2 = ((usize::from(index_2) + 1 + c as usize % (length - 1)) % length) as u8;
                    session_2.choice = ColorChoice::Entry(index_2);
                    // Pedir destino ⇔ alguna celda (no solo la activa) usa la entrada.
                    prop_assert_eq!(
                        session_2.removal_needs_destination(index_2),
                        uses_some_cel(&session_2.document, index_2)
                    );
                    let _ = session_2.apply_palette_op(PaletteOp::Remove { index: index_2, into: target_2 }, false);
                }
                3 if length > 1 => {
                    let _ = session_2.apply_palette_op(
                        PaletteOp::Move { from: (b as usize % length) as u8, to: (c as usize % length) as u8 },
                        false,
                    );
                }
                4 => {
                    let start_2 = (b as usize % length) as u8;
                    let _ = session_2.apply_palette_op(
                        PaletteOp::Rotate { start: start_2, len: 1 + (c as usize % (length - usize::from(start_2))) as u16, steps: (c % 5) as i32 - 2 },
                        false,
                    );
                }
                5 => {
                    let _ = session_2.apply_palette_op(
                        PaletteOp::SetTransparent { index: (b % 4 != 0).then(|| (c as usize % length) as u8) },
                        false,
                    );
                }
                6 => {
                    let cel = session_2.active_cel();
                    let _ = session_2.document.apply(Command::SetPixels {
                        image: cel,
                        pixels: vec![PixelEdit { x: b % 4, y: c % 4, pixel: Pixel::Indexed((b as usize % length) as u8) }],
                    });
                }
                7 => { let _ = session_2.undo(); }
                8 => { let _ = session_2.redo(); }
                _ => {}
            }
            session_2.normalize();
            prop_assert_eq!(consistent(&session_2.document), Ok(()), "tras el paso {}", kind);
        }
    }
}
