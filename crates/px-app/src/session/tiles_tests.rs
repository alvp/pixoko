//! Región sucia, límites y avisos, y fallo de reserva de la unidad 14a-2
//! (specs/hito-2-zoom-baldosas.md: filas B3, B11 y B12; contrato de API en «Contrato de API de la
//! 14a-2»). Sin ventana: `egui::Context::run_ui` con `RawInput` sintético, sin GPU ni capturas.
//!
//! - B3: `dirty_of` de cada comando, el registro `ChangeLog` (apply, `undo`, `redo`, id desconocido, tope
//!   de 4 MiB) y su uso desde `Session` (`absorb`, `take_dirty`, `undo` y `redo` invalidan lo anotado,
//!   clave ausente o desbordada → toda la imagen, `open` / `new_image` / `install` vacían el registro, y
//!   `load` no cambia nada); `ToolState::take_changes` de la línea, el relleno y el lápiz.
//! - B11: `check_fits` con `Canvas::MAX_SIDE`, el texto nuevo de `TooLargeToShow`, `new_image_form`
//!   con tope 8192 y sin la nota de la tarjeta gráfica, y un PNG de 1025×2 con una GPU de 1024 que se
//!   abre y se pinta en 2 baldosas.
//! - B12: con un asignador falso (`App::tile_alloc`), abrir, nueva imagen y convertir avisan 5 s y
//!   dejan la sesión, el documento y el lienzo como estaban (convertir: `undo`, `can_redo` verdadero).
//!
//! Costuras que la spec no da y estas pruebas usan tal cual (la lógica debe ofrecerlas):
//! - `App::new(document, cli_path)` es invocable en prueba (no abre ventana: `RfdPicker::new()` y
//!   `Dialogs::new` solo guardan estado) y `App` tiene los campos `session`, `canvas`, `notice` y
//!   `tile_alloc: Box<dyn TileAlloc>`.
//! - `open_file(&ctx, &path, ignored, from_cli, confirmed)`, `finish_new_image(&ctx, &values,
//!   discard)` y `run_convert(&ctx, plan, analysis)` son los de `main.rs` (with `analysis: Option<&Analysis>`).
//! - Los `Values` de «Nueva imagen» salen de `Dialog::new(NewImage, new_image_form(..)).values()`
//!   (64×64 indexado por defecto) y el `Plan` es el de `convert_mode` (sus campos son públicos).
//! - El fotograma de la app (`eframe::App::ui`) no se puede ejecutar sin `eframe::Frame`: el banco
//!   repite su parte del lienzo (`interact_ordered`, `sync`, `paint`), no el orden de `main.rs`.
//!
//! Supuestos: `Loaded` tiene `document` y `text` públicos; `install(Loaded)` no devuelve nada que
//! importe; `take_dirty()` devuelve **todo** si nada se anotó desde la última vez y lo anotado (aunque
//! sea el conjunto vacío) si algo se anotó.

use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use eframe::egui::{Context, Pos2, RawInput, Rect, TextureId, vec2};
use px_core::{Allowed, Command, Document, Image, Mode, Pixel, PixelEdit, Rgba, Span};
use px_formats::write_png;
use px_ui::actions::Locks;
use px_ui::canvas::{Canvas, CanvasEvent, CanvasInfo};
use px_ui::color_edit::{Commit, EditTarget};
use px_ui::dialog::{Dialog, DialogKind as ModalKind, Event, FieldKind, Modal, Validity};
use px_ui::dirty::{BlockSet, Change, ChangeLog, dirty_of};
use px_ui::palette_ops::PaletteOp;
use px_ui::tiles::TileAlloc;
use px_ui::tool::{Tool, ToolAction, ToolState};
use px_ui::view::Size;

use crate::App;
use crate::convert_mode::Plan;
use crate::files::{FileError, MAX_OPEN_PIXELS, check_fits};
use crate::new_image::{
    FIELD_HEIGHT, FIELD_WIDTH, GPU_LIMIT_NOTE, initial_palette, new_image_form, open_new_image,
};

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Directorio temporal propio de la prueba; se borra al soltarlo.
struct Tmp {
    path_2: PathBuf,
}

impl Tmp {
    fn new_2() -> Tmp {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path_2 =
            std::env::temp_dir().join(format!("pixoko-px-app-baldosas-{}-{n}", std::process::id()));
        fs::create_dir_all(&path_2).expect("directorio temporal");
        Tmp { path_2 }
    }

    fn png(&self, name_2: &str, image_2: &Image) -> PathBuf {
        let path_2 = self.path_2.join(name_2);
        fs::write(&path_2, write_png(image_2).expect("PNG")).expect("escribir el PNG");
        path_2
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path_2);
    }
}

const TEST_RED: Rgba = Rgba::opaque(255, 0, 0);

fn rgba(width: u32, height: u32) -> Image {
    Image::new(width, height, Mode::Rgba).expect("imagen RGBA")
}

fn indexed(width: u32, height: u32) -> Image {
    Image::new_indexed(width, height, initial_palette()).expect("imagen indexada")
}

fn session_of(image_2: Image) -> Session {
    Session::sample(Document::new(image_2))
}

fn blocks_of(pixels_2: &[(u32, u32)], width: u32, height: u32) -> BlockSet {
    let mut b = BlockSet::new(width, height);
    for &(x, y) in pixels_2 {
        b.add_pixel(x, y);
    }
    b
}

fn pixels_reds(image_id: px_core::ImageId, points: &[(u32, u32)]) -> Command {
    Command::SetPixels {
        image: image_id,
        pixels: points
            .iter()
            .map(|&(x, y)| PixelEdit {
                x,
                y,
                pixel: Pixel::Rgba(TEST_RED),
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------------------------
// B3 · Región de un comando
// ---------------------------------------------------------------------------------------------

const TEST_WIDTH: u32 = 200;
const TEST_HEIGHT: u32 = 130;

fn size() -> Size {
    Size::new(TEST_WIDTH, TEST_HEIGHT)
}

fn full() -> BlockSet {
    BlockSet::full(TEST_WIDTH, TEST_HEIGHT)
}

fn empty() -> BlockSet {
    BlockSet::new(TEST_WIDTH, TEST_HEIGHT)
}

#[test]
fn row_b3_dirty_of_set_pixels_are_the_blocks_of_its_pixels() {
    let id = ImageId::from_raw(0);
    let command = pixels_reds(id, &[(63, 0), (64, 0), (199, 129), (0, 0)]);
    assert_eq!(
        dirty_of(&command, size()),
        blocks_of(&[(63, 0), (64, 0), (199, 129)], TEST_WIDTH, TEST_HEIGHT)
    );
    assert_eq!(dirty_of(&command, size()).len(), 3);
    // Un solo píxel: un solo bloque, en el borde de bloque.
    for (x, y, block) in [
        (63u32, 63u32, (0u32, 0u32)),
        (64, 64, (1, 1)),
        (128, 128, (2, 2)),
    ] {
        let b = dirty_of(&pixels_reds(id, &[(x, y)]), size());
        assert_eq!(b.len(), 1, "({x}, {y})");
        assert!(b.contains(block.0, block.1), "({x}, {y})");
    }
}

#[test]
fn row_b3_dirty_of_fill_spans_are_the_blocks_of_its_spans() {
    let id = ImageId::from_raw(0);
    let spans_2 = |spans: Vec<Span>| Command::FillSpans {
        image: id,
        spans,
        pixel: Pixel::Rgba(TEST_RED),
    };
    let command = spans_2(vec![
        Span {
            y: 5,
            x_start: 0,
            x_end: 130,
        },
        Span {
            y: 70,
            x_start: 60,
            x_end: 70,
        },
    ]);
    let mut expected_2 = empty();
    expected_2.add_rect(0, 5, 130, 1);
    expected_2.add_rect(60, 70, 10, 1);
    let obtained = dirty_of(&command, size());
    assert_eq!(obtained, expected_2);
    assert_eq!(
        obtained.len(),
        3 + 2,
        "tres bloques en la fila 0 y dos en la 1"
    );
    // `x_end` es exclusivo: 0..64 es un solo bloque y 0..65 son dos.
    let one = spans_2(vec![Span {
        y: 0,
        x_start: 0,
        x_end: 64,
    }]);
    assert_eq!(dirty_of(&one, size()).len(), 1);
    let two = spans_2(vec![Span {
        y: 0,
        x_start: 0,
        x_end: 65,
    }]);
    assert_eq!(dirty_of(&two, size()).len(), 2);
}

#[test]
fn row_b3_dirty_of_a_batch_is_the_union_of_its_commands() {
    let id = ImageId::from_raw(0);
    let a = pixels_reds(id, &[(1, 1)]);
    let b = Command::FillSpans {
        image: id,
        spans: vec![Span {
            y: 100,
            x_start: 130,
            x_end: 140,
        }],
        pixel: Pixel::Rgba(TEST_RED),
    };
    let c = pixels_reds(id, &[(70, 70)]);
    let batch = Command::Batch(vec![a.clone(), b.clone(), c.clone()]);
    let mut union = dirty_of(&a, size());
    union.union(&dirty_of(&b, size()));
    union.union(&dirty_of(&c, size()));
    assert_eq!(dirty_of(&batch, size()), union);
    assert_eq!(union.len(), 3);
}

#[test]
fn row_b3_dirty_of_the_palette_commands_that_do_not_change_the_image_is_empty() {
    let id = ImageId::from_raw(0);
    for command in [
        Command::MovePaletteEntry {
            image: id,
            from: 1,
            to: 5,
        },
        Command::PermutePalette {
            image: id,
            order: (0..32).rev().collect(),
        },
        Command::AddPaletteEntry {
            image: id,
            color: TEST_RED,
        },
    ] {
        let region = dirty_of(&command, size());
        assert!(region.is_empty(), "{command:?}");
        assert_eq!(region.size(), size());
    }
}

#[test]
fn row_b3_dirty_of_color_conversion_remove_transparent_and_rotate_is_the_whole_image() {
    let id = ImageId::from_raw(0);
    for command in [
        Command::ConvertMode {
            image: id,
            target: Mode::Rgba,
            allowed: Allowed::NOTHING,
        },
        Command::SetPaletteColor {
            image: id,
            index: 2,
            color: TEST_RED,
        },
        Command::RemovePaletteEntry {
            image: id,
            index: 3,
            into: 2,
        },
        Command::SetTransparentIndex {
            image: id,
            index: Some(4),
        },
        Command::RotateColors {
            image: id,
            start: 1,
            len: 3,
            steps: 1,
        },
    ] {
        assert_eq!(dirty_of(&command, size()), full(), "{command:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// B3 · Registro
// ---------------------------------------------------------------------------------------------

#[test]
fn row_b3_the_log_stores_queries_and_empties() {
    let mut log = ChangeLog::new();
    assert_eq!(log.len(), 0);
    assert_eq!(log.bytes(), 0);
    assert_eq!(ChangeLog::LIMIT_BYTES, 4 << 20);
    let a = blocks_of(&[(1, 1)], TEST_WIDTH, TEST_HEIGHT);
    let b = blocks_of(&[(70, 70)], TEST_WIDTH, TEST_HEIGHT);
    log.record(5, a.clone());
    log.record(9, b.clone());
    assert_eq!(log.len(), 2);
    assert_eq!(log.get(5), Some(&a));
    assert_eq!(log.get(9), Some(&b));
    // Un id desconocido no está.
    for id in [0, 1, 4, 6, 10, u64::MAX] {
        assert_eq!(log.get(id), None, "id {id}");
    }
    // El coste de una entrada es `8 · palabras del map + 32`: la rejilla de 4×3 bloques cabe en
    // una palabra.
    assert_eq!(log.bytes(), 2 * (8 + 32));
    log.clear();
    assert_eq!(log.len(), 0);
    assert_eq!(log.bytes(), 0);
    assert_eq!(log.get(5), None);
}

#[test]
fn row_b3_the_log_forgets_the_oldest_steps_when_passing_the_4_mib_cap() {
    // Cada mapa de 8192² ocupa 128·128 bits = 256 palabras: 8·256 + 32 = 2080 bytes por entrada.
    let per_input = 8 * 256 + 32;
    let fit = ChangeLog::LIMIT_BYTES / per_input;
    assert_eq!(fit, 2016);
    let mut log = ChangeLog::new();
    for state_2 in 0..2100u64 {
        log.record(state_2, BlockSet::new(8192, 8192));
        assert!(
            log.bytes() <= ChangeLog::LIMIT_BYTES,
            "estado {state_2}: {} bytes",
            log.bytes()
        );
    }
    assert_eq!(log.len(), fit);
    assert_eq!(log.bytes(), fit * per_input);
    // Se olvidan los más viejos y quedan los más recientes.
    for old in [0u64, 1, 83] {
        assert_eq!(log.get(old), None, "estado {old}");
    }
    for recent in [84u64, 1000, 2099] {
        assert!(log.get(recent).is_some(), "estado {recent}");
    }
}

// ---------------------------------------------------------------------------------------------
// B3 · Session
// ---------------------------------------------------------------------------------------------

/// Aplica un trazo de lápiz con `ToolState` y devuelve lo que anota `take_changes`.
fn stroke_of_pencil(
    session_2: &mut Session,
    tools_2: &mut ToolState,
    from_2: (i32, i32),
    until: (i32, i32),
    pixel: Pixel,
) -> Vec<Change> {
    let id = ImageId::from_raw(0);
    for event_2 in [
        CanvasEvent::StrokeBegin(from_2),
        CanvasEvent::StrokeMove(until),
        CanvasEvent::StrokeEnd,
    ] {
        tools_2
            .canvas_event_with(&mut session_2.document, id, Some(pixel), event_2)
            .expect("sin errores");
    }
    tools_2.take_changes()
}

#[test]
fn row_b3_take_changes_of_the_pencil_gives_one_change_with_the_state_id_and_the_region_of_the_stroke()
 {
    let mut s = session_of(rgba(TEST_WIDTH, TEST_HEIGHT));
    let mut tools_2 = ToolState::new();
    let changes_2 = stroke_of_pencil(
        &mut s,
        &mut tools_2,
        (10, 10),
        (90, 10),
        Pixel::Rgba(TEST_RED),
    );
    assert_eq!(changes_2.len(), 1, "un solo paso");
    assert_eq!(changes_2[0].state, s.document.state_id());
    let pixels_2: Vec<(u32, u32)> = (10..=90).map(|x| (x, 10)).collect();
    assert_eq!(
        changes_2[0].blocks,
        blocks_of(&pixels_2, TEST_WIDTH, TEST_HEIGHT)
    );
    assert_eq!(changes_2[0].blocks.len(), 2);
    assert!(tools_2.take_changes().is_empty(), "se vacía al leerlo");
    // El mismo trazo con el mismo color no cambia nada: `Ok(false)` y ningún cambio.
    let other = stroke_of_pencil(
        &mut s,
        &mut tools_2,
        (10, 10),
        (90, 10),
        Pixel::Rgba(TEST_RED),
    );
    assert!(other.is_empty());
}

#[test]
fn row_b3_take_changes_of_the_line_and_the_fill() {
    let mut s = session_of(rgba(TEST_WIDTH, TEST_HEIGHT));
    let id = ImageId::from_raw(0);
    let mut tools_2 = ToolState::new();
    // Línea de (5, 5) a (130, 70): sus píxeles son los que cambian, y su región son sus bloques.
    tools_2.apply_action(ToolAction::Select(Tool::Line));
    let antes = s.document.image(id).expect("imagen").clone();
    let changes_2 = stroke_of_pencil(
        &mut s,
        &mut tools_2,
        (5, 5),
        (130, 70),
        Pixel::Rgba(TEST_RED),
    );
    assert_eq!(changes_2.len(), 1);
    assert_eq!(changes_2[0].state, s.document.state_id());
    let after_2 = s.document.image(id).expect("imagen");
    let mut changed = Vec::new();
    for y in 0..TEST_HEIGHT {
        for x in 0..TEST_WIDTH {
            if antes.get_pixel(x, y) != after_2.get_pixel(x, y) {
                changed.push((x, y));
            }
        }
    }
    assert!(changed.len() >= 126, "la línea tiene al menos 126 píxeles");
    assert_eq!(
        changes_2[0].blocks,
        blocks_of(&changed, TEST_WIDTH, TEST_HEIGHT)
    );

    // Relleno: la región pintada de rojo es una sola en el resto de la imagen; rellena de azul toda la
    // imagen vacía menos la línea: toca todos los bloques.
    tools_2.apply_action(ToolAction::Select(Tool::Fill));
    let blue = Pixel::Rgba(Rgba::opaque(0, 0, 255));
    let id = ImageId::from_raw(0);
    for event_2 in [CanvasEvent::StrokeBegin((199, 129)), CanvasEvent::StrokeEnd] {
        tools_2
            .canvas_event_with(&mut s.document, id, Some(blue), event_2)
            .expect("sin errores");
    }
    let changes_2 = tools_2.take_changes();
    assert_eq!(changes_2.len(), 1);
    assert_eq!(changes_2[0].state, s.document.state_id());
    assert_eq!(changes_2[0].blocks, full());
    // Rellenar otra vez con el mismo color no cambia nada y no anota nada.
    for event_2 in [CanvasEvent::StrokeBegin((199, 129)), CanvasEvent::StrokeEnd] {
        tools_2
            .canvas_event_with(&mut s.document, id, Some(blue), event_2)
            .expect("sin errores");
    }
    assert!(tools_2.take_changes().is_empty());
}

#[test]
fn row_b3_session_absorb_take_dirty_undo_and_redo_invalidate_what_was_noted() {
    let mut s = session_of(rgba(TEST_WIDTH, TEST_HEIGHT));
    let mut tools_2 = ToolState::new();
    // Nada anotado: toda la imagen.
    assert_eq!(s.take_dirty(), full());

    let changes_2 = stroke_of_pencil(
        &mut s,
        &mut tools_2,
        (10, 10),
        (90, 10),
        Pixel::Rgba(TEST_RED),
    );
    let region = changes_2[0].blocks.clone();
    let state_2 = changes_2[0].state;
    s.absorb(changes_2);
    assert_eq!(s.change_log().get(state_2), Some(&region));
    assert_eq!(s.take_dirty(), region, "la unión de lo anotado");
    assert_eq!(s.take_dirty(), full(), "sin nada nuevo, toda la imagen");

    // `undo` desde `s` invalida `log[s]`; `redo` hacia `s'` invalida `log[s']` (el mismo paso).
    s.undo().expect("undo");
    assert_eq!(s.take_dirty(), region);
    s.redo().expect("redo");
    assert_eq!(s.document.state_id(), state_2);
    assert_eq!(s.take_dirty(), region);

    // Dos trazos en bloques distintos: cada deshacer invalida solo el suyo.
    let mut changes_2 = stroke_of_pencil(
        &mut s,
        &mut tools_2,
        (150, 100),
        (190, 100),
        Pixel::Rgba(Rgba::opaque(0, 255, 0)),
    );
    let second = changes_2.remove(0);
    let (region2, state_2_2) = (second.blocks.clone(), second.state);
    s.absorb(vec![second]);
    assert_eq!(s.take_dirty(), region2);
    assert_ne!(region2, region);
    s.undo().expect("undo del segundo");
    assert_eq!(s.take_dirty(), region2);
    s.undo().expect("undo del primero");
    assert_eq!(s.take_dirty(), region);
    s.redo().expect("redo del primero");
    assert_eq!(s.take_dirty(), region);
    s.redo().expect("redo del segundo");
    assert_eq!(s.document.state_id(), state_2_2);
    assert_eq!(s.take_dirty(), region2);
}

#[test]
fn row_b3_session_with_the_key_absent_or_the_log_overflowed_invalidates_the_whole_image() {
    // Clave ausente: el paso se aplicó sin anotarlo (por ejemplo, un paso de otro origen).
    let mut s = session_of(rgba(TEST_WIDTH, TEST_HEIGHT));
    let id = ImageId::from_raw(0);
    assert_eq!(s.document.apply(pixels_reds(id, &[(1, 1)])), Ok(true));
    assert_eq!(s.change_log().get(s.document.state_id()), None);
    s.undo().expect("undo");
    assert_eq!(s.take_dirty(), full(), "id desconocido: toda la imagen");
    s.redo().expect("redo");
    assert_eq!(s.take_dirty(), full(), "también al rehacer");

    // Registro desbordado: el paso anotado se olvida cuando entran demasiados más.
    let mut s = session_of(rgba(TEST_WIDTH, TEST_HEIGHT));
    let mut tools_2 = ToolState::new();
    let changes_2 = stroke_of_pencil(
        &mut s,
        &mut tools_2,
        (10, 10),
        (20, 10),
        Pixel::Rgba(TEST_RED),
    );
    let state_2 = changes_2[0].state;
    s.absorb(changes_2);
    assert!(s.change_log().get(state_2).is_some());
    // Cada mapa de 200×130 cuesta 40 bytes: 4 MiB son 104 857 entradas.
    let many: Vec<Change> = (0..110_000u64)
        .map(|n| Change {
            state: 1_000_000 + n,
            blocks: BlockSet::new(TEST_WIDTH, TEST_HEIGHT),
        })
        .collect();
    s.absorb(many);
    assert!(s.change_log().bytes() <= ChangeLog::LIMIT_BYTES);
    assert_eq!(s.change_log().get(state_2), None, "el más viejo se olvidó");
    let _ = s.take_dirty();
    s.undo().expect("undo");
    assert_eq!(
        s.take_dirty(),
        full(),
        "registro desbordado: toda la imagen"
    );
}

#[test]
fn row_b3_session_notes_by_itself_the_conversion_the_palette_and_the_color_edit() {
    let mut s = session_of(indexed(TEST_WIDTH, TEST_HEIGHT));
    let _ = s.take_dirty();

    // Mover una entrada no cambia lo que se ve: región vacía.
    let result = s
        .apply_palette_op(PaletteOp::Move { from: 1, to: 5 }, false)
        .expect("mover");
    assert!(result.changed);
    assert_eq!(s.change_log().get(s.document.state_id()), Some(&empty()));
    assert!(s.take_dirty().is_empty());

    // Añadir una entrada tampoco.
    let result = s.apply_palette_op(PaletteOp::Add, false).expect("añadir");
    assert!(result.changed);
    assert_eq!(s.change_log().get(s.document.state_id()), Some(&empty()));
    assert!(s.take_dirty().is_empty());

    // Quitar una entrada, rotar colores y editar un color: toda la imagen.
    let result = s
        .apply_palette_op(PaletteOp::Remove { index: 3, into: 2 }, false)
        .expect("quitar");
    assert!(result.changed);
    assert_eq!(s.change_log().get(s.document.state_id()), Some(&full()));
    assert_eq!(s.take_dirty(), full());

    let result = s
        .apply_palette_op(
            PaletteOp::Rotate {
                start: 1,
                len: 4,
                steps: 1,
            },
            false,
        )
        .expect("rotar");
    assert!(result.changed);
    assert_eq!(s.change_log().get(s.document.state_id()), Some(&full()));
    let _ = s.take_dirty();

    assert_eq!(
        s.commit_edit(Commit {
            target: EditTarget::Entry(2),
            color: Rgba::opaque(9, 99, 199),
        }),
        Ok(true)
    );
    assert_eq!(s.change_log().get(s.document.state_id()), Some(&full()));
    assert_eq!(s.take_dirty(), full());

    // Deshacer lo anotado como vacío (mover) no invalida nada; deshacer la edición, todo.
    s.undo().expect("undo de la edición");
    assert_eq!(s.take_dirty(), full());

    // Conversión de modo: toda la imagen, y su deshacer también.
    assert_eq!(s.convert_mode(Mode::Rgba, Allowed::NOTHING), Ok(true));
    assert_eq!(s.change_log().get(s.document.state_id()), Some(&full()));
    assert_eq!(s.take_dirty(), full());
    s.undo().expect("undo de la conversión");
    assert_eq!(s.take_dirty(), full());
    s.redo().expect("redo de la conversión");
    assert_eq!(s.take_dirty(), full());
}

#[test]
fn row_b3_undoing_an_entry_move_invalidates_nothing() {
    let mut s = session_of(indexed(TEST_WIDTH, TEST_HEIGHT));
    let _ = s.take_dirty();
    s.apply_palette_op(PaletteOp::Move { from: 1, to: 5 }, false)
        .expect("mover");
    let _ = s.take_dirty();
    s.undo().expect("undo");
    assert!(s.take_dirty().is_empty(), "undo de lo vacío");
    s.redo().expect("redo");
    assert!(s.take_dirty().is_empty(), "redo de lo vacío");
}

#[test]
fn row_b3_new_image_empties_the_log_and_what_follows_is_the_whole_new_image() {
    let mut s = session_of(rgba(TEST_WIDTH, TEST_HEIGHT));
    let mut tools_2 = ToolState::new();
    let changes_2 = stroke_of_pencil(
        &mut s,
        &mut tools_2,
        (10, 10),
        (20, 10),
        Pixel::Rgba(TEST_RED),
    );
    s.absorb(changes_2);
    assert_eq!(s.change_log().len(), 1);
    s.new_image(rgba(70, 40), true).expect("nueva imagen");
    assert_eq!(s.change_log().len(), 0, "crear vacía el registro");
    assert_eq!(s.take_dirty(), BlockSet::full(70, 40));
}

fn png_small(tmp: &Tmp, name_2: &str, width: u32, height: u32) -> (PathBuf, Image) {
    let image_2 = rgba(width, height);
    (tmp.png(name_2, &image_2), image_2)
}

#[test]
fn row_b3_load_changes_nothing_and_install_replaces_and_empties_the_log_like_open() {
    let tmp = Tmp::new_2();
    let (path_2, _) = png_small(&tmp, "nueva.png", 90, 70);
    let mut s = session_of(rgba(TEST_WIDTH, TEST_HEIGHT));
    let mut tools_2 = ToolState::new();
    let changes_2 = stroke_of_pencil(
        &mut s,
        &mut tools_2,
        (10, 10),
        (20, 10),
        Pixel::Rgba(TEST_RED),
    );
    s.absorb(changes_2);
    let (state_2, image_before) = (
        s.document.state_id(),
        s.document
            .image(ImageId::from_raw(0))
            .expect("imagen")
            .clone(),
    );

    // Con cambios sin guardar y sin `discard`, `load` también se niega.
    assert_eq!(
        s.load(&path_2, Canvas::MAX_SIDE, false).err(),
        Some(FileError::Unsaved)
    );

    // `load` lee y no cambia nada: ni documento, ni ruta, ni registro.
    let loaded = s.load(&path_2, Canvas::MAX_SIDE, true).expect("load");
    assert_eq!(
        (loaded.document.width(), loaded.document.height()),
        (90, 70)
    );
    assert_eq!(s.document.state_id(), state_2);
    assert_eq!(s.document.image(ImageId::from_raw(0)), Some(&image_before));
    assert_eq!(s.path(), None);
    assert_eq!(s.change_log().len(), 1);
    assert!(s.modified());

    // `install` reemplaza el documento (historia vacía), pone la ruta y vacía el registro.
    s.install(loaded);
    assert_eq!(s.document.state_id(), 0);
    assert!(!s.document.can_undo());
    assert_eq!(
        s.document
            .image(ImageId::from_raw(0))
            .map(|i| (i.width(), i.height())),
        Some((90, 70))
    );
    assert!(s.path().is_some());
    assert!(!s.modified());
    assert_eq!(s.change_log().len(), 0);
    assert_eq!(s.take_dirty(), BlockSet::full(90, 70));
}

#[test]
fn row_b3_open_is_load_plus_install() {
    let tmp = Tmp::new_2();
    let (path_2, _) = png_small(&tmp, "abierta.png", 90, 70);
    let mut with_open = session_of(rgba(TEST_WIDTH, TEST_HEIGHT));
    let mut with_load = session_of(rgba(TEST_WIDTH, TEST_HEIGHT));
    let mut tools_2 = ToolState::new();
    let changes_2 = stroke_of_pencil(
        &mut with_open,
        &mut tools_2,
        (10, 10),
        (20, 10),
        Pixel::Rgba(TEST_RED),
    );
    with_open.absorb(changes_2);

    let text_2 = with_open
        .open(&path_2, Canvas::MAX_SIDE, true)
        .expect("open");
    let loaded = with_load
        .load(&path_2, Canvas::MAX_SIDE, false)
        .expect("load");
    assert_eq!(loaded.text, text_2, "el aviso de apertura es el mismo");
    with_load.install(loaded);
    assert_eq!(with_open.path(), with_load.path());
    assert_eq!(with_open.document, with_load.document);
    assert_eq!(with_open.change_log().len(), 0);
    assert_eq!(with_open.take_dirty(), BlockSet::full(90, 70));
    assert_eq!(with_load.take_dirty(), BlockSet::full(90, 70));
}

// ---------------------------------------------------------------------------------------------
// B11 · Límites y avisos
// ---------------------------------------------------------------------------------------------

#[test]
fn row_b11_check_fits_with_the_maximum_displayable_side() {
    assert_eq!(Canvas::MAX_SIDE, 16384);
    // 16384×4096 y 8192² valen (2²⁶ píxeles justos).
    for (w, h) in [
        (16384u32, 4096u32),
        (4096, 16384),
        (8192, 8192),
        (1, 1),
        (16384, 1),
    ] {
        assert_eq!(check_fits(w, h, Canvas::MAX_SIDE), Ok(()), "{w}×{h}");
    }
    // Un lado de más: el texto nuevo, sin hablar de una textura.
    for (w, h) in [(16385u32, 1u32), (1, 16385), (u32::MAX, 1)] {
        assert_eq!(
            check_fits(w, h, Canvas::MAX_SIDE),
            Err(FileError::TooLargeToShow {
                width: w,
                height: h,
                max_side: 16384
            }),
            "{w}×{h}"
        );
    }
    assert_eq!(
        check_fits(16385, 1, Canvas::MAX_SIDE)
            .expect_err("16385 de ancho")
            .to_string(),
        "la imagen de 16385×1 supera el lado máximo mostrable (16384)"
    );
    assert_eq!(
        check_fits(1, 16385, Canvas::MAX_SIDE)
            .expect_err("16385 de alto")
            .to_string(),
        "la imagen de 1×16385 supera el lado máximo mostrable (16384)"
    );
}

#[test]
fn row_b11_more_than_2_to_the_26_pixels_is_too_many_pixels() {
    assert_eq!(MAX_OPEN_PIXELS, 1 << 26);
    // Con lados ≤ 16384 el producto no puede ser 2²⁶ + 1; se usan los que se pasan por poco.
    for (w, h) in [
        (16384u32, 4097u32),
        (8193, 8193),
        (4097, 16384),
        (8192, 8193),
    ] {
        assert!(u64::from(w) * u64::from(h) > 1 << 26);
        assert_eq!(
            check_fits(w, h, Canvas::MAX_SIDE),
            Err(FileError::TooManyPixels {
                width: w,
                height: h,
                max: 1 << 26
            }),
            "{w}×{h}"
        );
    }
    // Un lado de más gana al producto.
    assert!(matches!(
        check_fits(16385, 4097, Canvas::MAX_SIDE),
        Err(FileError::TooLargeToShow { .. })
    ));
}

#[test]
fn row_b11_new_image_with_max_side_keeps_the_8192_cap_and_does_not_name_the_graphics_card() {
    // El tope de los campos sigue siendo 8192 aunque el lado mostrable sea 16384.
    let spec = new_image_form(Canvas::MAX_SIDE);
    for id in [FIELD_WIDTH, FIELD_HEIGHT] {
        let field_2 = spec.fields.iter().find(|f| f.id == id).expect("campo");
        let FieldKind::Number { min, max, default } = field_2.kind else {
            panic!("{id} no es numérico");
        };
        assert_eq!((min, max, default), (1, 8192, 64), "{id}");
    }
    // El diálogo abierto: 8192 vale, 8193 da «máximo 8192» a secas, sin la nota de la GPU.
    let mut modal = Modal::default();
    assert!(open_new_image(
        &mut modal,
        Canvas::MAX_SIDE,
        Locks::default()
    ));
    let Modal::Open(dialog) = &mut modal else {
        panic!("el diálogo está cerrado");
    };
    for field_2 in [FIELD_WIDTH, FIELD_HEIGHT] {
        for (text_2, valid) in [("8192", true), ("8193", false), ("100", true)] {
            assert_eq!(
                dialog.event(Event::Number {
                    field: field_2,
                    text: text_2.to_string()
                }),
                px_ui::dialog::Reaction::None
            );
            let validity = dialog.validity(field_2);
            if valid {
                assert_eq!(validity, Some(Validity::Valid), "{field_2} {text_2}");
            } else {
                assert_eq!(
                    validity,
                    Some(Validity::Invalid("máximo 8192".to_string())),
                    "{field_2} {text_2}"
                );
            }
        }
    }
    assert!(!"máximo 8192".contains(GPU_LIMIT_NOTE));
}

/// Un fotograma del lienzo de la app (la parte que no necesita `eframe::Frame`): `interact_ordered`,
/// `sync` y `paint`, con una GPU de lado máximo 1024.
fn frame(app: &mut App, ctx: &Context) -> CanvasInfo {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(160.0, 120.0))),
        max_texture_side: Some(1024),
        ..RawInput::default()
    };
    let mut info = None;
    let mut output = ctx.run_ui(input, |ui| {
        app.canvas.interact_roles(ui);
        let id = ImageId::from_raw(0);
        let image_2 = app.session.document.image(id).expect("imagen");
        let _ = app.canvas.sync(ui.ctx(), image_2);
        info = Some(app.canvas.paint(ui));
    });
    output.textures_delta.clear();
    info.expect("el lienzo se dibuja")
}

fn app_with(image_2: Image) -> (App, Context) {
    let mut app = App::new(Document::new(image_2), None);
    let ctx = Context::default();
    frame(&mut app, &ctx);
    frame(&mut app, &ctx);
    (app, ctx)
}

#[test]
fn row_b11_a_1025_by_2_png_with_a_1024_gpu_opens_and_is_painted_in_2_tiles() {
    let tmp = Tmp::new_2();
    let large = rgba(1025, 2);
    let path_2 = tmp.png("ancha.png", &large);
    let (mut app, ctx) = app_with(rgba(20, 20));
    assert_eq!(app.canvas.tile_count(), 1);

    app.open_file(&ctx, &path_2, 0, false, false);
    // Se abrió: el documento es el PNG y el aviso es el de apertura, no un error de textura.
    let image_2 = app
        .session
        .document
        .image(ImageId::from_raw(0))
        .expect("imagen");
    assert_eq!((image_2.width(), image_2.height()), (1025, 2));
    let now = ctx.input(|i| i.time);
    let text_2 = app
        .notice
        .as_ref()
        .and_then(|n| n.visible(now))
        .expect("hay un aviso");
    assert!(text_2.contains("1025×2"), "{text_2}");
    assert!(!text_2.contains("textura"), "{text_2}");

    // Y se pinta, en dos baldosas de 1024 y 1 de ancho.
    let info = frame(&mut app, &ctx);
    assert_eq!(info.error, None);
    assert!(info.image_rect.is_some());
    assert_eq!(app.canvas.tile_count(), 2);
    let ids: Vec<TextureId> = app.canvas.texture_ids();
    assert_eq!(ids.len(), 2);
    let manager = ctx.tex_manager();
    let sizes: Vec<[usize; 2]> = ids
        .iter()
        .map(|id| manager.read().meta(*id).expect("textura viva").size)
        .collect();
    assert_eq!(sizes, vec![[1024, 2], [1, 2]]);
}

// ---------------------------------------------------------------------------------------------
// B12 · Fallo de reserva
// ---------------------------------------------------------------------------------------------

/// Asignador falso: da memoria las primeras `good` veces y después falla.
struct Fail {
    good: usize,
    calls: usize,
}

impl Fail {
    fn after(good: usize) -> Box<Fail> {
        Box::new(Fail { good, calls: 0 })
    }
}

impl TileAlloc for Fail {
    fn pixels(&mut self, len: usize) -> Option<Vec<eframe::egui::Color32>> {
        self.calls += 1;
        (self.calls <= self.good).then(|| Vec::with_capacity(len))
    }
}

/// Lo que debe quedar igual tras un fallo: el documento (estado, historia, imagen), la ruta, y el
/// lienzo (texturas vivas, nivel de zoom y rectángulo de la imagen).
#[derive(Debug, PartialEq)]
struct TestSnapshot {
    state_2: u64,
    can_undo: bool,
    can_redo: bool,
    image_2: Image,
    path_2: Option<PathBuf>,
    modified_2: bool,
    textures: Vec<TextureId>,
    vivas: usize,
    zoom: Option<px_ui::view::Zoom>,
}

fn snapshot(app: &App, ctx: &Context) -> TestSnapshot {
    let doc = &app.session.document;
    TestSnapshot {
        state_2: doc.state_id(),
        can_undo: doc.can_undo(),
        can_redo: doc.can_redo(),
        image_2: doc.image(ImageId::from_raw(0)).expect("imagen").clone(),
        path_2: app.session.path().map(Path::to_path_buf),
        modified_2: app.session.modified(),
        textures: app.canvas.texture_ids(),
        vivas: ctx.tex_manager().read().num_allocated(),
        zoom: app.canvas.zoom(),
    }
}

/// El aviso está, dura 5 s y es de un fallo (no vacío).
fn five_second_notice(app: &App, ctx: &Context) -> String {
    let now = ctx.input(|i| i.time);
    let notice = app.notice.as_ref().expect("hay un aviso");
    let text_2 = notice.visible(now).expect("visible ahora").to_string();
    assert!(!text_2.is_empty());
    assert!(notice.visible(now + 4.9).is_some(), "sigue a los 4,9 s");
    assert_eq!(notice.visible(now + 5.1), None, "caduca a los 5 s");
    text_2
}

/// Una app con una imagen abierta de un fichero (con ruta) y un paso de historia.
fn app_with_history(tmp: &Tmp) -> (App, Context) {
    let path_2 = tmp.png("origen.png", &rgba(20, 20));
    let (mut app, ctx) = app_with(rgba(8, 8));
    app.open_file(&ctx, &path_2, 0, false, false);
    frame(&mut app, &ctx);
    let id = ImageId::from_raw(0);
    assert_eq!(
        app.session
            .document
            .apply(pixels_reds(id, &[(3, 3), (4, 4)])),
        Ok(true)
    );
    app.canvas.invalidate_all();
    frame(&mut app, &ctx);
    assert!(app.session.path().is_some());
    assert!(app.session.document.can_undo());
    (app, ctx)
}

#[test]
fn row_b12_opening_with_the_failure_at_tile_3_warns_5_seconds_and_leaves_everything_as_it_was() {
    let tmp = Tmp::new_2();
    // 2049×1 con una GPU de 1024: tres baldosas; la tercera reserva falla.
    let path_2 = tmp.png(
        "tres.png",
        &Image::new(2049, 1, Mode::Gray).expect("imagen"),
    );
    let (mut app, ctx) = app_with_history(&tmp);
    let antes = snapshot(&app, &ctx);
    app.tile_alloc = Fail::after(2);
    app.notice = None;
    // Con cambios sin guardar, abrir sin confirmar pide repetir: `confirmed` descarta de verdad.
    app.open_file(&ctx, &path_2, 0, false, true);

    let text_2 = five_second_notice(&app, &ctx);
    assert_ne!(text_2, "", "hay un motivo");
    assert_eq!(
        snapshot(&app, &ctx),
        antes,
        "sesión, documento y lienzo intactos"
    );
    // El lienzo sigue pintando lo de antes, sin errores.
    let info = frame(&mut app, &ctx);
    assert_eq!(info.error, None);
    assert_eq!(app.canvas.tile_count(), 1);
    assert_eq!(snapshot(&app, &ctx), antes);
}

#[test]
fn row_b12_opening_without_a_failure_the_same_file_does_work() {
    // Control: sin el fallo, el mismo fichero se abre y se pinta en 3 baldosas.
    let tmp = Tmp::new_2();
    let path_2 = tmp.png(
        "tres.png",
        &Image::new(2049, 1, Mode::Gray).expect("imagen"),
    );
    let (mut app, ctx) = app_with_history(&tmp);
    app.open_file(&ctx, &path_2, 0, false, true);
    frame(&mut app, &ctx);
    assert_eq!(app.canvas.tile_count(), 3);
    assert_eq!(app.session.document.state_id(), 0);
    assert!(!app.session.document.can_undo());
}

#[test]
fn row_b12_new_image_with_the_failure_warns_5_seconds_and_leaves_everything_as_it_was() {
    let tmp = Tmp::new_2();
    let (mut app, ctx) = app_with_history(&tmp);
    let antes = snapshot(&app, &ctx);
    let values_2 = Dialog::new(ModalKind::NewImage, new_image_form(Canvas::MAX_SIDE))
        .expect("el formulario es válido")
        .values()
        .expect("valores por defecto válidos");
    app.tile_alloc = Fail::after(0);
    app.notice = None;
    app.finish_new_image(&ctx, &values_2, true);

    five_second_notice(&app, &ctx);
    assert_eq!(
        snapshot(&app, &ctx),
        antes,
        "sesión, documento y lienzo intactos"
    );
    let info = frame(&mut app, &ctx);
    assert_eq!(info.error, None);
    assert_eq!(snapshot(&app, &ctx), antes);

    // Control: con memoria, la nueva imagen de 64×64 sí reemplaza el documento.
    app.tile_alloc = Box::new(px_ui::tiles::SystemTileAlloc);
    app.notice = None;
    app.finish_new_image(&ctx, &values_2, true);
    let image_2 = app
        .session
        .document
        .image(ImageId::from_raw(0))
        .expect("imagen");
    assert_eq!((image_2.width(), image_2.height()), (64, 64));
    assert_eq!(app.session.document.state_id(), 0);
    assert_eq!(app.session.path(), None);
    frame(&mut app, &ctx);
    assert_eq!(app.canvas.tile_count(), 1);
}

#[test]
fn row_b12_converting_with_the_failure_warns_5_seconds_undoes_and_leaves_redo_available() {
    let tmp = Tmp::new_2();
    let (mut app, ctx) = app_with_history(&tmp);
    let antes = snapshot(&app, &ctx);
    assert!(antes.can_undo && !antes.can_redo);
    let (zoom, rect) = {
        let info = frame(&mut app, &ctx);
        (info.zoom, info.image_rect)
    };
    let plan = Plan {
        target: Mode::GrayAlpha,
        allowed: Allowed {
            luma: true,
            drop_alpha: None,
        },
    };
    app.tile_alloc = Fail::after(0);
    app.notice = None;
    app.run_convert(&ctx, plan, None);

    five_second_notice(&app, &ctx);
    // El documento es el de antes (la conversión se deshizo) y queda disponible rehacer.
    let after_2 = snapshot(&app, &ctx);
    assert!(after_2.can_redo, "rehacer disponible");
    assert!(after_2.can_undo, "los pasos de antes siguen");
    assert_eq!(after_2.state_2, antes.state_2);
    assert_eq!(
        after_2.image_2, antes.image_2,
        "el documento es el de antes"
    );
    assert_eq!(after_2.image_2.mode(), Mode::Rgba);
    assert_eq!(after_2.path_2, antes.path_2);
    // La vista del lienzo se conserva.
    assert_eq!(after_2.zoom, antes.zoom);
    assert_eq!(after_2.textures, antes.textures);
    let info = frame(&mut app, &ctx);
    assert_eq!((info.zoom, info.image_rect), (zoom, rect));
    assert_eq!(info.error, None);
    // Con memoria otra vez, la textura es la del documento (nada quedó a medias).
    app.tile_alloc = Box::new(px_ui::tiles::SystemTileAlloc);
    app.canvas.invalidate_all();
    frame(&mut app, &ctx);
    assert_eq!(app.session.document.state_id(), antes.state_2);
}

#[test]
fn row_b12_converting_without_a_failure_applies_the_conversion_and_refreshes_the_canvas() {
    // Control: sin fallo, el documento pasa a gris con alfa y la historia crece un paso.
    let tmp = Tmp::new_2();
    let (mut app, ctx) = app_with_history(&tmp);
    let antes = app.session.document.state_id();
    let plan = Plan {
        target: Mode::GrayAlpha,
        allowed: Allowed {
            luma: true,
            drop_alpha: None,
        },
    };
    app.notice = None;
    app.run_convert(&ctx, plan, None);
    let image_2 = app
        .session
        .document
        .image(ImageId::from_raw(0))
        .expect("imagen");
    assert_eq!(image_2.mode(), Mode::GrayAlpha);
    assert_ne!(app.session.document.state_id(), antes);
    assert!(!app.session.document.can_redo());
    frame(&mut app, &ctx);
}
