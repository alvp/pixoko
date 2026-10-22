//! Tests de `Session::new_image` (specs/hito-2-nueva-imagen.md, unidad 19, fila 8 y D3): qué crea la
//! sesión al aceptar «Nueva imagen…».
//!
//! - Documento nuevo con `can_undo` y `can_redo` falsos, `state_id` 0, sin ruta y no modificado
//!   (cerrar y abrir no preguntan).
//! - Color activo: la entrada del negro (25) en indexado y `Slot(1)` en los demás modos.
//! - Se reinician como en `open`: desplazamiento y vista del panel, editor (con el aviso de texto
//!   descartado) y operaciones de paleta (con el aviso de quita cancelada). Las ranuras de color
//!   **no** (son de la sesión).
//! - Con cambios y sin `discard`: `Err(Unsaved)` y no cambia nada; con `discard` o sin cambios, `Ok`.
//! - `Guardar` sin ruta pide el diálogo con `sin-titulo.png`; `open` se comporta como antes.
//!
//! Los ayudantes de los demás ficheros de este directorio son privados de sus módulos: aquí se
//! duplican los mínimos (directorio temporal, `F`, selector falso). Cada test usa su propio directorio
//! temporal y nunca escribe dentro de `golden/`.
//!
//! Suposiciones donde la spec no fija un detalle:
//! - La «herramienta» de la fila 8 no es de `Session` (la lleva `ToolState` en `App`): aquí solo se
//!   comprueban las ranuras de color, que sí son de la sesión.
//! - `new_image` deja `converted()` en falso (no hay conversión que recordar), como una sesión nueva.
//! - El indexado de la prueba lleva la paleta real de D2 (`initial_palette()`): la entrada 25 es el
//!   negro de esa paleta.

use super::*;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use px_core::{Command, Document, Image, Mode, Palette, Pixel, PixelEdit, Rgba};
use px_formats::write_png;
use px_ui::color::{ColorChoice, initial_choice};
use px_ui::color_edit::EditTarget;
use px_ui::palette_grid::{PanelView, Scroll};

use crate::confirm::{Confirm, close_allowed};
use crate::dialogs::{DialogKind, DialogRequest, Dialogs, FilePicker, open_gate};
use crate::files::FileError;
use crate::new_image::initial_palette;

// ---------------------------------------------------------------------------------------
// Ayudantes: directorio temporal, imágenes, sesiones y selector falso.
// ---------------------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Directorio temporal propio del test; el guardián lo borra al soltarse.
struct TmpDir {
    path: PathBuf,
}

impl TmpDir {
    fn new() -> TmpDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "pixoko-px-app-new-image-{}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("crear el directorio temporal");
        TmpDir { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

const RED: Rgba = Rgba::opaque(255, 0, 0);
const GREEN: Rgba = Rgba::opaque(0, 255, 0);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);
const BLACK: Rgba = Rgba::opaque(0, 0, 0);

/// Indexada 4×1, paleta `[R, G, B, K]`, píxeles `[0, 1, 2, 3]`.
fn image_f() -> Image {
    let palette = Palette::from_colors([RED, GREEN, BLUE, BLACK]).expect("paleta F");
    let mut image = Image::new_indexed(4, 1, palette).expect("imagen indexada");
    for x in 0..4u8 {
        image
            .set_pixel(u32::from(x), 0, Pixel::Indexed(x))
            .expect("píxel");
    }
    image
}

/// La imagen nueva de cada modo, como la entregaría `build_new_image` (con la paleta de D2 en
/// indexado).
fn imagen_nueva(mode: Mode) -> Image {
    match mode {
        Mode::Indexed => Image::new_indexed(8, 6, initial_palette()).expect("indexada"),
        otro => Image::new(8, 6, otro).expect("imagen"),
    }
}

fn session_f() -> Session {
    Session::sample(Document::new(image_f()))
}

/// Una edición de la paleta que marca el documento como modificado (cada llamada cambia el color:
/// dos seguidas son dos pasos).
fn edita(session: &mut Session) {
    let image = ImageId::from_raw(0);
    let tono = (session.document.state_id() % 200) as u8 + 1;
    session
        .document
        .apply(Command::SetPaletteColor {
            image,
            index: 0,
            color: Rgba::opaque(9, 9, tono),
        })
        .expect("editar");
    assert!(session.modified());
}

/// Una edición de un píxel, distinta del contenido de una imagen nueva, en el modo que sea.
fn edita_pixel(session: &mut Session) {
    let image = ImageId::from_raw(0);
    let pixel = match imagen_de(session).mode() {
        Mode::Indexed => Pixel::Indexed(1),
        Mode::Rgba => Pixel::Rgba(Rgba::opaque(1, 2, 3)),
        Mode::Gray => Pixel::Gray(7),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: 7,
            alpha: 255,
        },
    };
    session
        .document
        .apply(Command::SetPixels {
            image,
            pixels: vec![PixelEdit { x: 0, y: 0, pixel }],
        })
        .expect("editar");
    assert!(session.modified());
}

/// Sesión con historia, sin guardar, con elección, desplazamiento y vista del panel no iniciales,
/// texto pendiente en el editor y una quita pendiente.
fn session_con_de_todo() -> Session {
    let mut session = session_f();
    edita(&mut session);
    session.choice = ColorChoice::Entry(2);
    session.sync_editor();
    session.scroll = Scroll {
        first_row: 3,
        rest: 5,
    };
    session.view = PanelView {
        offset_px: 40,
        rest_px: 2,
        last_visible: Some(3),
        reveal: true,
        ..PanelView::default()
    };
    session
        .editor
        .type_hex(EditTarget::Entry(2), BLUE, "#00ff00".to_string());
    assert!(session.editor.has_pending());
    session.ops.begin_removal(3, Some(1));
    assert!(session.ops.removal().is_some());
    session
}

/// Todo lo observable que `new_image` con error no debe tocar.
#[derive(Clone, Debug, PartialEq)]
struct Foto {
    image: Image,
    state_id: u64,
    can_undo: bool,
    can_redo: bool,
    path: Option<PathBuf>,
    modified: bool,
    choice: ColorChoice,
    scroll: Scroll,
    view: PanelView,
    pending_hex: Option<String>,
    removal: bool,
    slots: px_ui::color::SlotBank,
    converted: bool,
}

fn foto(session: &Session) -> Foto {
    Foto {
        image: session
            .document
            .image(ImageId::from_raw(0))
            .expect("imagen")
            .clone(),
        state_id: session.document.state_id(),
        can_undo: session.document.can_undo(),
        can_redo: session.document.can_redo(),
        path: session.path().map(Path::to_path_buf),
        modified: session.modified(),
        choice: session.choice,
        scroll: session.scroll,
        view: session.view,
        pending_hex: session.editor.hex_display(),
        removal: session.ops.removal().is_some(),
        slots: session.slots.clone(),
        converted: session.converted(),
    }
}

fn imagen_de(session: &Session) -> &Image {
    session
        .document
        .image(ImageId::from_raw(0))
        .expect("imagen")
}

#[derive(Default)]
struct FakeState {
    starts: usize,
    last: Option<DialogRequest>,
}

#[derive(Clone, Default)]
struct Handle(Rc<RefCell<FakeState>>);

struct FakePicker(Handle);

impl FilePicker for FakePicker {
    fn start(&mut self, request: &DialogRequest, parent: Option<&eframe::Frame>) {
        assert!(parent.is_none(), "los tests no tienen ventana");
        let mut state = self.0.0.borrow_mut();
        state.starts += 1;
        state.last = Some(request.clone());
    }

    fn poll(&mut self) -> Option<Option<PathBuf>> {
        None
    }

    fn abandon(&mut self) {}
}

fn dialogs() -> (Dialogs, Handle) {
    let handle = Handle::default();
    let dialogs = Dialogs::new(Box::new(FakePicker(handle.clone())), false, None);
    (dialogs, handle)
}

// ---------------------------------------------------------------------------------------
// Fila 8: el documento nuevo.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_8_el_documento_nuevo_no_tiene_historia_ni_ruta_ni_cambios() {
    let tmp = TmpDir::new();
    let mut session = session_con_de_todo();
    // Con ruta (guardada una vez) y de nuevo con cambios.
    session.save(tmp.path()).expect("guardar");
    assert!(session.path().is_some());
    edita(&mut session);

    let nueva = imagen_nueva(Mode::Rgba);
    session.new_image(nueva.clone(), true).expect("crear");

    assert_eq!(
        imagen_de(&session),
        &nueva,
        "el documento es la imagen dada"
    );
    assert!(!session.document.can_undo());
    assert!(!session.document.can_redo());
    assert_eq!(session.document.state_id(), 0);
    assert!(session.path().is_none(), "sin ruta");
    assert!(!session.modified(), "no modificado");
    assert!(!session.converted());
}

#[test]
fn fila_8_cerrar_y_abrir_no_preguntan_en_la_imagen_nueva_y_dibujar_lo_marca_modificado() {
    let mut session = session_con_de_todo();
    session
        .new_image(imagen_nueva(Mode::Indexed), true)
        .expect("crear");

    let mut confirm = Confirm::new();
    assert!(close_allowed(session.modified(), &mut confirm, 0.0));
    assert_eq!(
        open_gate(session.modified(), &mut confirm, 0.0),
        Some(false)
    );

    // La base de «modificado» es el estado nuevo: una edición sí lo marca.
    edita_pixel(&mut session);
    assert!(session.modified());
}

// ---------------------------------------------------------------------------------------
// Fila 8: color activo.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_8_el_color_activo_es_el_negro_25_en_indexado_y_slot_1_en_los_demas() {
    for (mode, esperado) in [
        (Mode::Indexed, ColorChoice::Entry(25)),
        (Mode::Rgba, ColorChoice::Slot(1)),
        (Mode::GrayAlpha, ColorChoice::Slot(1)),
        (Mode::Gray, ColorChoice::Slot(1)),
    ] {
        let mut session = session_f();
        session.choice = ColorChoice::Entry(2);
        let nueva = imagen_nueva(mode);
        session.new_image(nueva.clone(), false).expect("crear");
        assert_eq!(session.choice, esperado, "{mode:?}");
        // La elección es válida para la imagen y su color es el esperado.
        if mode == Mode::Indexed {
            let palette = nueva.palette().expect("paleta");
            assert_eq!(palette.get(25), Ok(BLACK), "la 25 es el negro");
        }
        assert!(
            session.active_pixel().is_some(),
            "{mode:?}: hay color activo"
        );
    }
}

#[test]
fn fila_8_el_color_activo_de_indexado_no_es_initial_choice() {
    let nueva = imagen_nueva(Mode::Indexed);
    assert_eq!(
        initial_choice(&nueva),
        ColorChoice::Entry(0),
        "initial_choice da el transparente (la spec lo descarta para la imagen nueva)"
    );
    let mut session = session_f();
    session.new_image(nueva, false).expect("crear");
    assert_ne!(session.choice, ColorChoice::Entry(0));
}

// ---------------------------------------------------------------------------------------
// Fila 8: lo que se reinicia y lo que no.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_8_se_reinician_el_desplazamiento_la_vista_el_editor_y_las_operaciones() {
    let mut session = session_con_de_todo();
    session
        .new_image(imagen_nueva(Mode::Indexed), true)
        .expect("crear");

    assert_eq!(session.scroll, Scroll::default());
    assert_eq!(session.view, PanelView::default());
    assert!(!session.editor.has_pending(), "el texto se descartó");
    assert_eq!(session.editor.hex_display(), None);
    assert!(session.ops.removal().is_none(), "la quita se canceló");
    assert!(session.ops.drag_state().is_none());
}

#[test]
fn fila_8_el_texto_pendiente_descartado_se_dice_una_sola_vez() {
    let mut session = session_con_de_todo();
    assert!(!session.take_discarded_text(), "antes no hay nada");
    session
        .new_image(imagen_nueva(Mode::Rgba), true)
        .expect("crear");
    assert!(session.take_discarded_text(), "se avisa");
    assert!(!session.take_discarded_text(), "se lee una sola vez");
}

#[test]
fn fila_8_la_quita_pendiente_cancelada_se_dice_una_sola_vez() {
    let mut session = session_con_de_todo();
    assert!(!session.take_removal_cancelled(), "antes no hay nada");
    session
        .new_image(imagen_nueva(Mode::Rgba), true)
        .expect("crear");
    assert!(session.take_removal_cancelled(), "se avisa");
    assert!(!session.take_removal_cancelled(), "se lee una sola vez");
}

#[test]
fn fila_8_sin_texto_ni_quita_pendientes_no_hay_avisos() {
    let mut session = session_f();
    session
        .new_image(imagen_nueva(Mode::Gray), false)
        .expect("crear");
    assert!(!session.take_discarded_text());
    assert!(!session.take_removal_cancelled());
}

#[test]
fn fila_8_las_ranuras_de_color_no_se_tocan() {
    let mut session = session_con_de_todo();
    session
        .slots
        .get_mut(Mode::Rgba)
        .set(2, Rgba::opaque(10, 20, 30));
    session
        .slots
        .get_mut(Mode::Gray)
        .set(3, Rgba::opaque(50, 50, 50));
    let antes = session.slots.clone();
    session
        .new_image(imagen_nueva(Mode::Rgba), true)
        .expect("crear");
    assert_eq!(session.slots, antes);
}

// ---------------------------------------------------------------------------------------
// Fila 8: cambios sin guardar y `discard`.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_8_con_cambios_y_sin_discard_da_unsaved_y_no_cambia_nada() {
    let mut session = session_con_de_todo();
    let antes = foto(&session);
    let result = session.new_image(imagen_nueva(Mode::Rgba), false);
    assert_eq!(result, Err(FileError::Unsaved));
    assert_eq!(foto(&session), antes, "nada cambió");
    assert!(!session.take_discarded_text(), "tampoco los avisos");
    assert!(!session.take_removal_cancelled());
}

#[test]
fn fila_8_con_cambios_y_discard_reemplaza() {
    let mut session = session_con_de_todo();
    let nueva = imagen_nueva(Mode::Gray);
    assert_eq!(session.new_image(nueva.clone(), true), Ok(()));
    assert_eq!(imagen_de(&session), &nueva);
}

#[test]
fn fila_8_sin_cambios_reemplaza_con_o_sin_discard() {
    for discard in [false, true] {
        let mut session = session_f();
        assert!(!session.modified());
        let nueva = imagen_nueva(Mode::GrayAlpha);
        assert_eq!(session.new_image(nueva.clone(), discard), Ok(()));
        assert_eq!(imagen_de(&session), &nueva, "discard {discard}");
    }
}

// ---------------------------------------------------------------------------------------
// Fila 8: `Guardar` y `open`.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_8_guardar_sin_ruta_pide_el_dialogo_con_sin_titulo_png() {
    let mut session = session_con_de_todo();
    session
        .new_image(imagen_nueva(Mode::Rgba), true)
        .expect("crear");
    assert!(session.path().is_none());

    for kind in [DialogKind::Save, DialogKind::SaveAs] {
        let (mut dialogs, handle) = dialogs();
        assert!(dialogs.begin(kind, false, session.path(), session.converted(), 0.0, None));
        let request = handle.0.borrow().last.clone().expect("se pidió un diálogo");
        assert_eq!(request.kind, kind);
        assert_eq!(
            request.file_name.as_deref(),
            Some("sin-titulo.png"),
            "{kind:?}"
        );
    }
}

#[test]
fn fila_8_guardar_la_imagen_nueva_escribe_sin_titulo_png_y_deja_de_estar_modificada() {
    let tmp = TmpDir::new();
    let mut session = session_f();
    session
        .new_image(imagen_nueva(Mode::Rgba), false)
        .expect("crear");
    edita_pixel(&mut session);
    let saved = session.save(tmp.path()).expect("guardar");
    assert_eq!(
        saved.path.file_name().and_then(|n| n.to_str()),
        Some("sin-titulo.png")
    );
    assert!(!session.modified());
    assert_eq!(session.path(), Some(saved.path.as_path()));
}

#[test]
fn fila_8_open_se_comporta_como_antes_tras_crear_una_imagen_nueva() {
    let tmp = TmpDir::new();
    let otra = image_f();
    let path = tmp.path().join("otra.png");
    fs::write(&path, write_png(&otra).expect("write_png")).expect("escribir el PNG");

    let mut session = session_f();
    session
        .new_image(imagen_nueva(Mode::Rgba), false)
        .expect("crear");
    // La imagen nueva no tiene cambios: `open` no pregunta.
    let text = session.open(&path, 16384, false).expect("abrir");
    assert!(text.contains("otra.png"), "{text}");
    assert!(session.path().is_some());
    assert_eq!(session.choice, initial_choice(&otra));
    assert!(!session.modified());
    assert_eq!(imagen_de(&session), &otra);
}
