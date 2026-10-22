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
//!   discard)` y `run_convert(&ctx, plan, analysis)` son los de `main.rs` (con `analysis: Option<&Analysis>`).
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

static CONTADOR: AtomicU64 = AtomicU64::new(0);

/// Directorio temporal propio de la prueba; se borra al soltarlo.
struct Tmp {
    ruta: PathBuf,
}

impl Tmp {
    fn nuevo() -> Tmp {
        let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
        let ruta =
            std::env::temp_dir().join(format!("pixoko-px-app-baldosas-{}-{n}", std::process::id()));
        fs::create_dir_all(&ruta).expect("directorio temporal");
        Tmp { ruta }
    }

    fn png(&self, nombre: &str, imagen: &Image) -> PathBuf {
        let ruta = self.ruta.join(nombre);
        fs::write(&ruta, write_png(imagen).expect("PNG")).expect("escribir el PNG");
        ruta
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.ruta);
    }
}

const ROJO: Rgba = Rgba::opaque(255, 0, 0);

fn rgba(ancho: u32, alto: u32) -> Image {
    Image::new(ancho, alto, Mode::Rgba).expect("imagen RGBA")
}

fn indexada(ancho: u32, alto: u32) -> Image {
    Image::new_indexed(ancho, alto, initial_palette()).expect("imagen indexada")
}

fn sesion_de(imagen: Image) -> Session {
    Session::sample(Document::new(imagen))
}

fn bloques_de(pixeles: &[(u32, u32)], ancho: u32, alto: u32) -> BlockSet {
    let mut b = BlockSet::new(ancho, alto);
    for &(x, y) in pixeles {
        b.add_pixel(x, y);
    }
    b
}

fn pixeles_rojos(imagen_id: px_core::ImageId, puntos: &[(u32, u32)]) -> Command {
    Command::SetPixels {
        image: imagen_id,
        pixels: puntos
            .iter()
            .map(|&(x, y)| PixelEdit {
                x,
                y,
                pixel: Pixel::Rgba(ROJO),
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------------------------
// B3 · Región de un comando
// ---------------------------------------------------------------------------------------------

const ANCHO: u32 = 200;
const ALTO: u32 = 130;

fn tamano() -> Size {
    Size::new(ANCHO, ALTO)
}

fn lleno() -> BlockSet {
    BlockSet::full(ANCHO, ALTO)
}

fn vacio() -> BlockSet {
    BlockSet::new(ANCHO, ALTO)
}

#[test]
fn fila_b3_dirty_of_de_set_pixels_son_los_bloques_de_sus_pixeles() {
    let id = Document::new(rgba(ANCHO, ALTO)).main_image();
    let comando = pixeles_rojos(id, &[(63, 0), (64, 0), (199, 129), (0, 0)]);
    assert_eq!(
        dirty_of(&comando, tamano()),
        bloques_de(&[(63, 0), (64, 0), (199, 129)], ANCHO, ALTO)
    );
    assert_eq!(dirty_of(&comando, tamano()).len(), 3);
    // Un solo píxel: un solo bloque, en el borde de bloque.
    for (x, y, bloque) in [
        (63u32, 63u32, (0u32, 0u32)),
        (64, 64, (1, 1)),
        (128, 128, (2, 2)),
    ] {
        let b = dirty_of(&pixeles_rojos(id, &[(x, y)]), tamano());
        assert_eq!(b.len(), 1, "({x}, {y})");
        assert!(b.contains(bloque.0, bloque.1), "({x}, {y})");
    }
}

#[test]
fn fila_b3_dirty_of_de_fill_spans_son_los_bloques_de_sus_tramos() {
    let id = Document::new(rgba(ANCHO, ALTO)).main_image();
    let tramos = |spans: Vec<Span>| Command::FillSpans {
        image: id,
        spans,
        pixel: Pixel::Rgba(ROJO),
    };
    let comando = tramos(vec![
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
    let mut esperado = vacio();
    esperado.add_rect(0, 5, 130, 1);
    esperado.add_rect(60, 70, 10, 1);
    let obtenido = dirty_of(&comando, tamano());
    assert_eq!(obtenido, esperado);
    assert_eq!(
        obtenido.len(),
        3 + 2,
        "tres bloques en la fila 0 y dos en la 1"
    );
    // `x_end` es exclusivo: 0..64 es un solo bloque y 0..65 son dos.
    let uno = tramos(vec![Span {
        y: 0,
        x_start: 0,
        x_end: 64,
    }]);
    assert_eq!(dirty_of(&uno, tamano()).len(), 1);
    let dos = tramos(vec![Span {
        y: 0,
        x_start: 0,
        x_end: 65,
    }]);
    assert_eq!(dirty_of(&dos, tamano()).len(), 2);
}

#[test]
fn fila_b3_dirty_of_de_un_lote_es_la_union_de_sus_comandos() {
    let id = Document::new(rgba(ANCHO, ALTO)).main_image();
    let a = pixeles_rojos(id, &[(1, 1)]);
    let b = Command::FillSpans {
        image: id,
        spans: vec![Span {
            y: 100,
            x_start: 130,
            x_end: 140,
        }],
        pixel: Pixel::Rgba(ROJO),
    };
    let c = pixeles_rojos(id, &[(70, 70)]);
    let lote = Command::Batch(vec![a.clone(), b.clone(), c.clone()]);
    let mut union = dirty_of(&a, tamano());
    union.union(&dirty_of(&b, tamano()));
    union.union(&dirty_of(&c, tamano()));
    assert_eq!(dirty_of(&lote, tamano()), union);
    assert_eq!(union.len(), 3);
}

#[test]
fn fila_b3_dirty_of_de_los_comandos_de_paleta_que_no_cambian_la_imagen_es_vacio() {
    let id = Document::new(indexada(ANCHO, ALTO)).main_image();
    for comando in [
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
            color: ROJO,
        },
    ] {
        let region = dirty_of(&comando, tamano());
        assert!(region.is_empty(), "{comando:?}");
        assert_eq!(region.size(), tamano());
    }
}

#[test]
fn fila_b3_dirty_of_de_conversion_color_quitar_transparente_y_rotar_es_toda_la_imagen() {
    let id = Document::new(indexada(ANCHO, ALTO)).main_image();
    for comando in [
        Command::ConvertMode {
            image: id,
            target: Mode::Rgba,
            allowed: Allowed::NOTHING,
        },
        Command::SetPaletteColor {
            image: id,
            index: 2,
            color: ROJO,
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
        assert_eq!(dirty_of(&comando, tamano()), lleno(), "{comando:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// B3 · Registro
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_b3_el_registro_guarda_consulta_y_se_vacia() {
    let mut log = ChangeLog::new();
    assert_eq!(log.len(), 0);
    assert_eq!(log.bytes(), 0);
    assert_eq!(ChangeLog::LIMIT_BYTES, 4 << 20);
    let a = bloques_de(&[(1, 1)], ANCHO, ALTO);
    let b = bloques_de(&[(70, 70)], ANCHO, ALTO);
    log.record(5, a.clone());
    log.record(9, b.clone());
    assert_eq!(log.len(), 2);
    assert_eq!(log.get(5), Some(&a));
    assert_eq!(log.get(9), Some(&b));
    // Un id desconocido no está.
    for id in [0, 1, 4, 6, 10, u64::MAX] {
        assert_eq!(log.get(id), None, "id {id}");
    }
    // El coste de una entrada es `8 · palabras del mapa + 32`: la rejilla de 4×3 bloques cabe en
    // una palabra.
    assert_eq!(log.bytes(), 2 * (8 + 32));
    log.clear();
    assert_eq!(log.len(), 0);
    assert_eq!(log.bytes(), 0);
    assert_eq!(log.get(5), None);
}

#[test]
fn fila_b3_el_registro_olvida_los_pasos_mas_viejos_al_pasar_el_tope_de_4_mib() {
    // Cada mapa de 8192² ocupa 128·128 bits = 256 palabras: 8·256 + 32 = 2080 bytes por entrada.
    let por_entrada = 8 * 256 + 32;
    let caben = ChangeLog::LIMIT_BYTES / por_entrada;
    assert_eq!(caben, 2016);
    let mut log = ChangeLog::new();
    for estado in 0..2100u64 {
        log.record(estado, BlockSet::new(8192, 8192));
        assert!(
            log.bytes() <= ChangeLog::LIMIT_BYTES,
            "estado {estado}: {} bytes",
            log.bytes()
        );
    }
    assert_eq!(log.len(), caben);
    assert_eq!(log.bytes(), caben * por_entrada);
    // Se olvidan los más viejos y quedan los más recientes.
    for viejo in [0u64, 1, 83] {
        assert_eq!(log.get(viejo), None, "estado {viejo}");
    }
    for reciente in [84u64, 1000, 2099] {
        assert!(log.get(reciente).is_some(), "estado {reciente}");
    }
}

// ---------------------------------------------------------------------------------------------
// B3 · Session
// ---------------------------------------------------------------------------------------------

/// Aplica un trazo de lápiz con `ToolState` y devuelve lo que anota `take_changes`.
fn trazo_de_lapiz(
    sesion: &mut Session,
    herramientas: &mut ToolState,
    desde: (i32, i32),
    hasta: (i32, i32),
    pixel: Pixel,
) -> Vec<Change> {
    let id = sesion.document.main_image();
    for evento in [
        CanvasEvent::StrokeBegin(desde),
        CanvasEvent::StrokeMove(hasta),
        CanvasEvent::StrokeEnd,
    ] {
        herramientas
            .canvas_event_with(&mut sesion.document, id, Some(pixel), evento)
            .expect("sin errores");
    }
    herramientas.take_changes()
}

#[test]
fn fila_b3_take_changes_del_lapiz_da_un_cambio_con_el_state_id_y_la_region_del_trazo() {
    let mut s = sesion_de(rgba(ANCHO, ALTO));
    let mut herramientas = ToolState::new();
    let cambios = trazo_de_lapiz(
        &mut s,
        &mut herramientas,
        (10, 10),
        (90, 10),
        Pixel::Rgba(ROJO),
    );
    assert_eq!(cambios.len(), 1, "un solo paso");
    assert_eq!(cambios[0].state, s.document.state_id());
    let pixeles: Vec<(u32, u32)> = (10..=90).map(|x| (x, 10)).collect();
    assert_eq!(cambios[0].blocks, bloques_de(&pixeles, ANCHO, ALTO));
    assert_eq!(cambios[0].blocks.len(), 2);
    assert!(herramientas.take_changes().is_empty(), "se vacía al leerlo");
    // El mismo trazo con el mismo color no cambia nada: `Ok(false)` y ningún cambio.
    let otra = trazo_de_lapiz(
        &mut s,
        &mut herramientas,
        (10, 10),
        (90, 10),
        Pixel::Rgba(ROJO),
    );
    assert!(otra.is_empty());
}

#[test]
fn fila_b3_take_changes_de_la_linea_y_del_relleno() {
    let mut s = sesion_de(rgba(ANCHO, ALTO));
    let id = s.document.main_image();
    let mut herramientas = ToolState::new();
    // Línea de (5, 5) a (130, 70): sus píxeles son los que cambian, y su región son sus bloques.
    herramientas.apply_action(ToolAction::Select(Tool::Line));
    let antes = s.document.image(id).expect("imagen").clone();
    let cambios = trazo_de_lapiz(
        &mut s,
        &mut herramientas,
        (5, 5),
        (130, 70),
        Pixel::Rgba(ROJO),
    );
    assert_eq!(cambios.len(), 1);
    assert_eq!(cambios[0].state, s.document.state_id());
    let despues = s.document.image(id).expect("imagen");
    let mut cambiados = Vec::new();
    for y in 0..ALTO {
        for x in 0..ANCHO {
            if antes.get_pixel(x, y) != despues.get_pixel(x, y) {
                cambiados.push((x, y));
            }
        }
    }
    assert!(
        cambiados.len() >= 126,
        "la línea tiene al menos 126 píxeles"
    );
    assert_eq!(cambios[0].blocks, bloques_de(&cambiados, ANCHO, ALTO));

    // Relleno: la región pintada de rojo es una sola en el resto de la imagen; rellena de azul toda la
    // imagen vacía menos la línea: toca todos los bloques.
    herramientas.apply_action(ToolAction::Select(Tool::Fill));
    let azul = Pixel::Rgba(Rgba::opaque(0, 0, 255));
    let id = s.document.main_image();
    for evento in [CanvasEvent::StrokeBegin((199, 129)), CanvasEvent::StrokeEnd] {
        herramientas
            .canvas_event_with(&mut s.document, id, Some(azul), evento)
            .expect("sin errores");
    }
    let cambios = herramientas.take_changes();
    assert_eq!(cambios.len(), 1);
    assert_eq!(cambios[0].state, s.document.state_id());
    assert_eq!(cambios[0].blocks, lleno());
    // Rellenar otra vez con el mismo color no cambia nada y no anota nada.
    for evento in [CanvasEvent::StrokeBegin((199, 129)), CanvasEvent::StrokeEnd] {
        herramientas
            .canvas_event_with(&mut s.document, id, Some(azul), evento)
            .expect("sin errores");
    }
    assert!(herramientas.take_changes().is_empty());
}

#[test]
fn fila_b3_session_absorb_take_dirty_undo_y_redo_invalidan_lo_anotado() {
    let mut s = sesion_de(rgba(ANCHO, ALTO));
    let mut herramientas = ToolState::new();
    // Nada anotado: toda la imagen.
    assert_eq!(s.take_dirty(), lleno());

    let cambios = trazo_de_lapiz(
        &mut s,
        &mut herramientas,
        (10, 10),
        (90, 10),
        Pixel::Rgba(ROJO),
    );
    let region = cambios[0].blocks.clone();
    let estado = cambios[0].state;
    s.absorb(cambios);
    assert_eq!(s.change_log().get(estado), Some(&region));
    assert_eq!(s.take_dirty(), region, "la unión de lo anotado");
    assert_eq!(s.take_dirty(), lleno(), "sin nada nuevo, toda la imagen");

    // `undo` desde `s` invalida `log[s]`; `redo` hacia `s'` invalida `log[s']` (el mismo paso).
    s.undo().expect("undo");
    assert_eq!(s.take_dirty(), region);
    s.redo().expect("redo");
    assert_eq!(s.document.state_id(), estado);
    assert_eq!(s.take_dirty(), region);

    // Dos trazos en bloques distintos: cada deshacer invalida solo el suyo.
    let mut cambios = trazo_de_lapiz(
        &mut s,
        &mut herramientas,
        (150, 100),
        (190, 100),
        Pixel::Rgba(Rgba::opaque(0, 255, 0)),
    );
    let segundo = cambios.remove(0);
    let (region2, estado2) = (segundo.blocks.clone(), segundo.state);
    s.absorb(vec![segundo]);
    assert_eq!(s.take_dirty(), region2);
    assert_ne!(region2, region);
    s.undo().expect("undo del segundo");
    assert_eq!(s.take_dirty(), region2);
    s.undo().expect("undo del primero");
    assert_eq!(s.take_dirty(), region);
    s.redo().expect("redo del primero");
    assert_eq!(s.take_dirty(), region);
    s.redo().expect("redo del segundo");
    assert_eq!(s.document.state_id(), estado2);
    assert_eq!(s.take_dirty(), region2);
}

#[test]
fn fila_b3_session_con_la_clave_ausente_o_el_registro_desbordado_invalida_toda_la_imagen() {
    // Clave ausente: el paso se aplicó sin anotarlo (por ejemplo, un paso de otro origen).
    let mut s = sesion_de(rgba(ANCHO, ALTO));
    let id = s.document.main_image();
    assert_eq!(s.document.apply(pixeles_rojos(id, &[(1, 1)])), Ok(true));
    assert_eq!(s.change_log().get(s.document.state_id()), None);
    s.undo().expect("undo");
    assert_eq!(s.take_dirty(), lleno(), "id desconocido: toda la imagen");
    s.redo().expect("redo");
    assert_eq!(s.take_dirty(), lleno(), "también al rehacer");

    // Registro desbordado: el paso anotado se olvida cuando entran demasiados más.
    let mut s = sesion_de(rgba(ANCHO, ALTO));
    let mut herramientas = ToolState::new();
    let cambios = trazo_de_lapiz(
        &mut s,
        &mut herramientas,
        (10, 10),
        (20, 10),
        Pixel::Rgba(ROJO),
    );
    let estado = cambios[0].state;
    s.absorb(cambios);
    assert!(s.change_log().get(estado).is_some());
    // Cada mapa de 200×130 cuesta 40 bytes: 4 MiB son 104 857 entradas.
    let muchos: Vec<Change> = (0..110_000u64)
        .map(|n| Change {
            state: 1_000_000 + n,
            blocks: BlockSet::new(ANCHO, ALTO),
        })
        .collect();
    s.absorb(muchos);
    assert!(s.change_log().bytes() <= ChangeLog::LIMIT_BYTES);
    assert_eq!(s.change_log().get(estado), None, "el más viejo se olvidó");
    let _ = s.take_dirty();
    s.undo().expect("undo");
    assert_eq!(
        s.take_dirty(),
        lleno(),
        "registro desbordado: toda la imagen"
    );
}

#[test]
fn fila_b3_session_anota_sola_la_conversion_la_paleta_y_la_edicion_de_color() {
    let mut s = sesion_de(indexada(ANCHO, ALTO));
    let _ = s.take_dirty();

    // Mover una entrada no cambia lo que se ve: región vacía.
    let resultado = s
        .apply_palette_op(PaletteOp::Move { from: 1, to: 5 }, false)
        .expect("mover");
    assert!(resultado.changed);
    assert_eq!(s.change_log().get(s.document.state_id()), Some(&vacio()));
    assert!(s.take_dirty().is_empty());

    // Añadir una entrada tampoco.
    let resultado = s.apply_palette_op(PaletteOp::Add, false).expect("añadir");
    assert!(resultado.changed);
    assert_eq!(s.change_log().get(s.document.state_id()), Some(&vacio()));
    assert!(s.take_dirty().is_empty());

    // Quitar una entrada, rotar colores y editar un color: toda la imagen.
    let resultado = s
        .apply_palette_op(PaletteOp::Remove { index: 3, into: 2 }, false)
        .expect("quitar");
    assert!(resultado.changed);
    assert_eq!(s.change_log().get(s.document.state_id()), Some(&lleno()));
    assert_eq!(s.take_dirty(), lleno());

    let resultado = s
        .apply_palette_op(
            PaletteOp::Rotate {
                start: 1,
                len: 4,
                steps: 1,
            },
            false,
        )
        .expect("rotar");
    assert!(resultado.changed);
    assert_eq!(s.change_log().get(s.document.state_id()), Some(&lleno()));
    let _ = s.take_dirty();

    assert_eq!(
        s.commit_edit(Commit {
            target: EditTarget::Entry(2),
            color: Rgba::opaque(9, 99, 199),
        }),
        Ok(true)
    );
    assert_eq!(s.change_log().get(s.document.state_id()), Some(&lleno()));
    assert_eq!(s.take_dirty(), lleno());

    // Deshacer lo anotado como vacío (mover) no invalida nada; deshacer la edición, todo.
    s.undo().expect("undo de la edición");
    assert_eq!(s.take_dirty(), lleno());

    // Conversión de modo: toda la imagen, y su deshacer también.
    assert_eq!(s.convert_mode(Mode::Rgba, Allowed::NOTHING), Ok(true));
    assert_eq!(s.change_log().get(s.document.state_id()), Some(&lleno()));
    assert_eq!(s.take_dirty(), lleno());
    s.undo().expect("undo de la conversión");
    assert_eq!(s.take_dirty(), lleno());
    s.redo().expect("redo de la conversión");
    assert_eq!(s.take_dirty(), lleno());
}

#[test]
fn fila_b3_deshacer_un_mover_de_entrada_no_invalida_nada() {
    let mut s = sesion_de(indexada(ANCHO, ALTO));
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
fn fila_b3_new_image_vacia_el_registro_y_lo_siguiente_es_toda_la_imagen_nueva() {
    let mut s = sesion_de(rgba(ANCHO, ALTO));
    let mut herramientas = ToolState::new();
    let cambios = trazo_de_lapiz(
        &mut s,
        &mut herramientas,
        (10, 10),
        (20, 10),
        Pixel::Rgba(ROJO),
    );
    s.absorb(cambios);
    assert_eq!(s.change_log().len(), 1);
    s.new_image(rgba(70, 40), true).expect("nueva imagen");
    assert_eq!(s.change_log().len(), 0, "crear vacía el registro");
    assert_eq!(s.take_dirty(), BlockSet::full(70, 40));
}

fn png_pequeno(tmp: &Tmp, nombre: &str, ancho: u32, alto: u32) -> (PathBuf, Image) {
    let imagen = rgba(ancho, alto);
    (tmp.png(nombre, &imagen), imagen)
}

#[test]
fn fila_b3_load_no_cambia_nada_e_install_reemplaza_y_vacia_el_registro_como_open() {
    let tmp = Tmp::nuevo();
    let (ruta, _) = png_pequeno(&tmp, "nueva.png", 90, 70);
    let mut s = sesion_de(rgba(ANCHO, ALTO));
    let mut herramientas = ToolState::new();
    let cambios = trazo_de_lapiz(
        &mut s,
        &mut herramientas,
        (10, 10),
        (20, 10),
        Pixel::Rgba(ROJO),
    );
    s.absorb(cambios);
    let (estado, imagen_antes) = (
        s.document.state_id(),
        s.document
            .image(s.document.main_image())
            .expect("imagen")
            .clone(),
    );

    // Con cambios sin guardar y sin `discard`, `load` también se niega.
    assert_eq!(
        s.load(&ruta, Canvas::MAX_SIDE, false).err(),
        Some(FileError::Unsaved)
    );

    // `load` lee y no cambia nada: ni documento, ni ruta, ni registro.
    let cargado = s.load(&ruta, Canvas::MAX_SIDE, true).expect("load");
    assert_eq!((cargado.document.width(), cargado.document.height()), (90, 70));
    assert_eq!(s.document.state_id(), estado);
    assert_eq!(
        s.document.image(s.document.main_image()),
        Some(&imagen_antes)
    );
    assert_eq!(s.path(), None);
    assert_eq!(s.change_log().len(), 1);
    assert!(s.modified());

    // `install` reemplaza el documento (historia vacía), pone la ruta y vacía el registro.
    s.install(cargado);
    assert_eq!(s.document.state_id(), 0);
    assert!(!s.document.can_undo());
    assert_eq!(
        s.document
            .image(s.document.main_image())
            .map(|i| (i.width(), i.height())),
        Some((90, 70))
    );
    assert!(s.path().is_some());
    assert!(!s.modified());
    assert_eq!(s.change_log().len(), 0);
    assert_eq!(s.take_dirty(), BlockSet::full(90, 70));
}

#[test]
fn fila_b3_open_es_load_mas_install() {
    let tmp = Tmp::nuevo();
    let (ruta, _) = png_pequeno(&tmp, "abierta.png", 90, 70);
    let mut con_open = sesion_de(rgba(ANCHO, ALTO));
    let mut con_load = sesion_de(rgba(ANCHO, ALTO));
    let mut herramientas = ToolState::new();
    let cambios = trazo_de_lapiz(
        &mut con_open,
        &mut herramientas,
        (10, 10),
        (20, 10),
        Pixel::Rgba(ROJO),
    );
    con_open.absorb(cambios);

    let texto = con_open.open(&ruta, Canvas::MAX_SIDE, true).expect("open");
    let cargado = con_load.load(&ruta, Canvas::MAX_SIDE, false).expect("load");
    assert_eq!(cargado.text, texto, "el aviso de apertura es el mismo");
    con_load.install(cargado);
    assert_eq!(con_open.path(), con_load.path());
    assert_eq!(con_open.document, con_load.document);
    assert_eq!(con_open.change_log().len(), 0);
    assert_eq!(con_open.take_dirty(), BlockSet::full(90, 70));
    assert_eq!(con_load.take_dirty(), BlockSet::full(90, 70));
}

// ---------------------------------------------------------------------------------------------
// B11 · Límites y avisos
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_b11_check_fits_con_el_lado_maximo_mostrable() {
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
fn fila_b11_mas_de_2_a_la_26_pixeles_es_too_many_pixels() {
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
fn fila_b11_nueva_imagen_con_max_side_mantiene_el_tope_de_8192_y_no_nombra_la_tarjeta_grafica() {
    // El tope de los campos sigue siendo 8192 aunque el lado mostrable sea 16384.
    let spec = new_image_form(Canvas::MAX_SIDE);
    for id in [FIELD_WIDTH, FIELD_HEIGHT] {
        let campo = spec.fields.iter().find(|f| f.id == id).expect("campo");
        let FieldKind::Number { min, max, default } = campo.kind else {
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
    let Modal::Open(dialogo) = &mut modal else {
        panic!("el diálogo está cerrado");
    };
    for campo in [FIELD_WIDTH, FIELD_HEIGHT] {
        for (texto, valido) in [("8192", true), ("8193", false), ("100", true)] {
            assert_eq!(
                dialogo.event(Event::Number {
                    field: campo,
                    text: texto.to_string()
                }),
                px_ui::dialog::Reaction::None
            );
            let validez = dialogo.validity(campo);
            if valido {
                assert_eq!(validez, Some(Validity::Valid), "{campo} {texto}");
            } else {
                assert_eq!(
                    validez,
                    Some(Validity::Invalid("máximo 8192".to_string())),
                    "{campo} {texto}"
                );
            }
        }
    }
    assert!(!"máximo 8192".contains(GPU_LIMIT_NOTE));
}

/// Un fotograma del lienzo de la app (la parte que no necesita `eframe::Frame`): `interact_ordered`,
/// `sync` y `paint`, con una GPU de lado máximo 1024.
fn fotograma(app: &mut App, ctx: &Context) -> CanvasInfo {
    let entrada = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(160.0, 120.0))),
        max_texture_side: Some(1024),
        ..RawInput::default()
    };
    let mut info = None;
    let mut salida = ctx.run_ui(entrada, |ui| {
        app.canvas.interact_ordered(ui);
        let id = app.session.document.main_image();
        let imagen = app.session.document.image(id).expect("imagen");
        let _ = app.canvas.sync(ui.ctx(), imagen);
        info = Some(app.canvas.paint(ui));
    });
    salida.textures_delta.clear();
    info.expect("el lienzo se dibuja")
}

fn app_con(imagen: Image) -> (App, Context) {
    let mut app = App::new(Document::new(imagen), None);
    let ctx = Context::default();
    fotograma(&mut app, &ctx);
    fotograma(&mut app, &ctx);
    (app, ctx)
}

#[test]
fn fila_b11_un_png_de_1025_por_2_con_una_gpu_de_1024_se_abre_y_se_pinta_en_2_baldosas() {
    let tmp = Tmp::nuevo();
    let grande = rgba(1025, 2);
    let ruta = tmp.png("ancha.png", &grande);
    let (mut app, ctx) = app_con(rgba(20, 20));
    assert_eq!(app.canvas.tile_count(), 1);

    app.open_file(&ctx, &ruta, 0, false, false);
    // Se abrió: el documento es el PNG y el aviso es el de apertura, no un error de textura.
    let imagen = app
        .session
        .document
        .image(app.session.document.main_image())
        .expect("imagen");
    assert_eq!((imagen.width(), imagen.height()), (1025, 2));
    let ahora = ctx.input(|i| i.time);
    let texto = app
        .notice
        .as_ref()
        .and_then(|n| n.visible(ahora))
        .expect("hay un aviso");
    assert!(texto.contains("1025×2"), "{texto}");
    assert!(!texto.contains("textura"), "{texto}");

    // Y se pinta, en dos baldosas de 1024 y 1 de ancho.
    let info = fotograma(&mut app, &ctx);
    assert_eq!(info.error, None);
    assert!(info.image_rect.is_some());
    assert_eq!(app.canvas.tile_count(), 2);
    let ids: Vec<TextureId> = app.canvas.texture_ids();
    assert_eq!(ids.len(), 2);
    let manager = ctx.tex_manager();
    let tamanos: Vec<[usize; 2]> = ids
        .iter()
        .map(|id| manager.read().meta(*id).expect("textura viva").size)
        .collect();
    assert_eq!(tamanos, vec![[1024, 2], [1, 2]]);
}

// ---------------------------------------------------------------------------------------------
// B12 · Fallo de reserva
// ---------------------------------------------------------------------------------------------

/// Asignador falso: da memoria las primeras `buenas` veces y después falla.
struct Falla {
    buenas: usize,
    llamadas: usize,
}

impl Falla {
    fn tras(buenas: usize) -> Box<Falla> {
        Box::new(Falla {
            buenas,
            llamadas: 0,
        })
    }
}

impl TileAlloc for Falla {
    fn pixels(&mut self, len: usize) -> Option<Vec<eframe::egui::Color32>> {
        self.llamadas += 1;
        (self.llamadas <= self.buenas).then(|| Vec::with_capacity(len))
    }
}

/// Lo que debe quedar igual tras un fallo: el documento (estado, historia, imagen), la ruta, y el
/// lienzo (texturas vivas, nivel de zoom y rectángulo de la imagen).
#[derive(Debug, PartialEq)]
struct Instantanea {
    estado: u64,
    puede_deshacer: bool,
    puede_rehacer: bool,
    imagen: Image,
    ruta: Option<PathBuf>,
    modificado: bool,
    texturas: Vec<TextureId>,
    vivas: usize,
    zoom: Option<px_ui::view::Zoom>,
}

fn instantanea(app: &App, ctx: &Context) -> Instantanea {
    let doc = &app.session.document;
    Instantanea {
        estado: doc.state_id(),
        puede_deshacer: doc.can_undo(),
        puede_rehacer: doc.can_redo(),
        imagen: doc.image(doc.main_image()).expect("imagen").clone(),
        ruta: app.session.path().map(Path::to_path_buf),
        modificado: app.session.modified(),
        texturas: app.canvas.texture_ids(),
        vivas: ctx.tex_manager().read().num_allocated(),
        zoom: app.canvas.zoom(),
    }
}

/// El aviso está, dura 5 s y es de un fallo (no vacío).
fn aviso_de_5_segundos(app: &App, ctx: &Context) -> String {
    let ahora = ctx.input(|i| i.time);
    let aviso = app.notice.as_ref().expect("hay un aviso");
    let texto = aviso.visible(ahora).expect("visible ahora").to_string();
    assert!(!texto.is_empty());
    assert!(aviso.visible(ahora + 4.9).is_some(), "sigue a los 4,9 s");
    assert_eq!(aviso.visible(ahora + 5.1), None, "caduca a los 5 s");
    texto
}

/// Una app con una imagen abierta de un fichero (con ruta) y un paso de historia.
fn app_con_historia(tmp: &Tmp) -> (App, Context) {
    let ruta = tmp.png("origen.png", &rgba(20, 20));
    let (mut app, ctx) = app_con(rgba(8, 8));
    app.open_file(&ctx, &ruta, 0, false, false);
    fotograma(&mut app, &ctx);
    let id = app.session.document.main_image();
    assert_eq!(
        app.session
            .document
            .apply(pixeles_rojos(id, &[(3, 3), (4, 4)])),
        Ok(true)
    );
    app.canvas.invalidate_all();
    fotograma(&mut app, &ctx);
    assert!(app.session.path().is_some());
    assert!(app.session.document.can_undo());
    (app, ctx)
}

#[test]
fn fila_b12_abrir_con_el_fallo_en_la_baldosa_3_avisa_5_segundos_y_deja_todo_como_estaba() {
    let tmp = Tmp::nuevo();
    // 2049×1 con una GPU de 1024: tres baldosas; la tercera reserva falla.
    let ruta = tmp.png(
        "tres.png",
        &Image::new(2049, 1, Mode::Gray).expect("imagen"),
    );
    let (mut app, ctx) = app_con_historia(&tmp);
    let antes = instantanea(&app, &ctx);
    app.tile_alloc = Falla::tras(2);
    app.notice = None;
    // Con cambios sin guardar, abrir sin confirmar pide repetir: `confirmed` descarta de verdad.
    app.open_file(&ctx, &ruta, 0, false, true);

    let texto = aviso_de_5_segundos(&app, &ctx);
    assert_ne!(texto, "", "hay un motivo");
    assert_eq!(
        instantanea(&app, &ctx),
        antes,
        "sesión, documento y lienzo intactos"
    );
    // El lienzo sigue pintando lo de antes, sin errores.
    let info = fotograma(&mut app, &ctx);
    assert_eq!(info.error, None);
    assert_eq!(app.canvas.tile_count(), 1);
    assert_eq!(instantanea(&app, &ctx), antes);
}

#[test]
fn fila_b12_abrir_sin_fallo_con_el_mismo_fichero_si_funciona() {
    // Control: sin el fallo, el mismo fichero se abre y se pinta en 3 baldosas.
    let tmp = Tmp::nuevo();
    let ruta = tmp.png(
        "tres.png",
        &Image::new(2049, 1, Mode::Gray).expect("imagen"),
    );
    let (mut app, ctx) = app_con_historia(&tmp);
    app.open_file(&ctx, &ruta, 0, false, true);
    fotograma(&mut app, &ctx);
    assert_eq!(app.canvas.tile_count(), 3);
    assert_eq!(app.session.document.state_id(), 0);
    assert!(!app.session.document.can_undo());
}

#[test]
fn fila_b12_nueva_imagen_con_el_fallo_avisa_5_segundos_y_deja_todo_como_estaba() {
    let tmp = Tmp::nuevo();
    let (mut app, ctx) = app_con_historia(&tmp);
    let antes = instantanea(&app, &ctx);
    let valores = Dialog::new(ModalKind::NewImage, new_image_form(Canvas::MAX_SIDE))
        .expect("el formulario es válido")
        .values()
        .expect("valores por defecto válidos");
    app.tile_alloc = Falla::tras(0);
    app.notice = None;
    app.finish_new_image(&ctx, &valores, true);

    aviso_de_5_segundos(&app, &ctx);
    assert_eq!(
        instantanea(&app, &ctx),
        antes,
        "sesión, documento y lienzo intactos"
    );
    let info = fotograma(&mut app, &ctx);
    assert_eq!(info.error, None);
    assert_eq!(instantanea(&app, &ctx), antes);

    // Control: con memoria, la nueva imagen de 64×64 sí reemplaza el documento.
    app.tile_alloc = Box::new(px_ui::tiles::SystemTileAlloc);
    app.notice = None;
    app.finish_new_image(&ctx, &valores, true);
    let imagen = app
        .session
        .document
        .image(app.session.document.main_image())
        .expect("imagen");
    assert_eq!((imagen.width(), imagen.height()), (64, 64));
    assert_eq!(app.session.document.state_id(), 0);
    assert_eq!(app.session.path(), None);
    fotograma(&mut app, &ctx);
    assert_eq!(app.canvas.tile_count(), 1);
}

#[test]
fn fila_b12_convertir_con_el_fallo_avisa_5_segundos_hace_undo_y_deja_rehacer_disponible() {
    let tmp = Tmp::nuevo();
    let (mut app, ctx) = app_con_historia(&tmp);
    let antes = instantanea(&app, &ctx);
    assert!(antes.puede_deshacer && !antes.puede_rehacer);
    let (zoom, rect) = {
        let info = fotograma(&mut app, &ctx);
        (info.zoom, info.image_rect)
    };
    let plan = Plan {
        target: Mode::GrayAlpha,
        allowed: Allowed {
            luma: true,
            drop_alpha: None,
        },
    };
    app.tile_alloc = Falla::tras(0);
    app.notice = None;
    app.run_convert(&ctx, plan, None);

    aviso_de_5_segundos(&app, &ctx);
    // El documento es el de antes (la conversión se deshizo) y queda disponible rehacer.
    let despues = instantanea(&app, &ctx);
    assert!(despues.puede_rehacer, "rehacer disponible");
    assert!(despues.puede_deshacer, "los pasos de antes siguen");
    assert_eq!(despues.estado, antes.estado);
    assert_eq!(despues.imagen, antes.imagen, "el documento es el de antes");
    assert_eq!(despues.imagen.mode(), Mode::Rgba);
    assert_eq!(despues.ruta, antes.ruta);
    // La vista del lienzo se conserva.
    assert_eq!(despues.zoom, antes.zoom);
    assert_eq!(despues.texturas, antes.texturas);
    let info = fotograma(&mut app, &ctx);
    assert_eq!((info.zoom, info.image_rect), (zoom, rect));
    assert_eq!(info.error, None);
    // Con memoria otra vez, la textura es la del documento (nada quedó a medias).
    app.tile_alloc = Box::new(px_ui::tiles::SystemTileAlloc);
    app.canvas.invalidate_all();
    fotograma(&mut app, &ctx);
    assert_eq!(app.session.document.state_id(), antes.estado);
}

#[test]
fn fila_b12_convertir_sin_fallo_aplica_la_conversion_y_refresca_el_lienzo() {
    // Control: sin fallo, el documento pasa a gris con alfa y la historia crece un paso.
    let tmp = Tmp::nuevo();
    let (mut app, ctx) = app_con_historia(&tmp);
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
    let imagen = app
        .session
        .document
        .image(app.session.document.main_image())
        .expect("imagen");
    assert_eq!(imagen.mode(), Mode::GrayAlpha);
    assert_ne!(app.session.document.state_id(), antes);
    assert!(!app.session.document.can_redo());
    fotograma(&mut app, &ctx);
}
