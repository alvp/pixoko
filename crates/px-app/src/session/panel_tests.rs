//! Tests de `session` con el panel de la ventana baja (specs/hito-2-panel-ventana-baja.md): fila
//! 10 (texto pendiente y `open`) y la parte de `Session` de la fila 11 (`view.reveal`,
//! `set_choice_revealing` y `reveal_if_choice_changed`).
//!
//! Los ayudantes de `tests.rs` y `edit_tests.rs` son privados de esos módulos: aquí se duplican
//! los mínimos. Cada test usa su propio directorio temporal y nunca escribe dentro de `golden/`.

use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use px_core::{Image, Palette, Pixel, Rgba};
use px_formats::write_png;
use px_ui::color::ColorChoice;
use px_ui::color_edit::{EditTarget, HexOutcome};
use px_ui::palette_grid::{PanelView, Scroll};

// ---------------------------------------------------------------------------------------
// Ayudantes: directorio temporal, imágenes y sesiones.
// ---------------------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Directorio temporal propio del test; el guardián lo borra al soltarse.
struct TmpDir {
    path: PathBuf,
}

impl TmpDir {
    fn new() -> TmpDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("pixoko-px-app-panel-{}-{n}", std::process::id()));
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

/// `F` de la spec: indexada 4×1, paleta `[R, G, B, K]`, píxeles `[0, 1, 2, 3]`.
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

/// Indexada de 256 colores (16 filas de la rejilla).
fn image_256() -> Image {
    let colors: Vec<Rgba> = (0..256usize)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8))
        .collect();
    let palette = Palette::from_colors(colors).expect("paleta de 256 colores");
    Image::new_indexed(8, 8, palette).expect("imagen indexada")
}

fn session_f() -> Session {
    Session::sample(Document::new(image_f()))
}

/// Escribe `image` como PNG en `dir/name` y devuelve la ruta.
fn write_image(dir: &Path, name: &str, image: &Image) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, write_png(image).expect("write_png")).expect("escribir el PNG");
    path
}

/// Elige `choice` y pone al día el editor, como hace el fotograma siguiente.
fn select(session: &mut Session, choice: ColorChoice) {
    session.choice = choice;
    session.sync_editor();
}

/// Sesión sobre `F` con la entrada 2 elegida y `#00ff00` tecleado y sin confirmar.
fn session_with_pending() -> Session {
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(2));
    session
        .editor
        .type_hex(EditTarget::Entry(2), BLUE, "#00ff00".to_string());
    assert!(session.editor.has_pending());
    session
}

// ---------------------------------------------------------------------------------------
// Fila 10: texto pendiente y `open`.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_10_un_open_correcto_con_texto_pendiente_lo_descarta_y_lo_dice_una_vez() {
    let tmp = TmpDir::new();
    let path = write_image(tmp.path(), "otro.png", &image_256());
    let mut session = session_with_pending();
    assert!(!session.take_discarded_text(), "antes del open no hay nada");

    session.open(&path, 16384, false).expect("abrir");

    assert!(!session.editor.has_pending(), "el texto se descartó");
    assert_eq!(session.editor.hex_display(), None);
    assert!(session.take_discarded_text(), "se avisa");
    assert!(!session.take_discarded_text(), "se lee una sola vez");
    assert!(!session.take_discarded_text());
}

#[test]
fn fila_10_el_texto_descartado_no_se_aplica_a_nada() {
    let tmp = TmpDir::new();
    let path = write_image(tmp.path(), "otro.png", &image_f());
    let mut session = session_with_pending();
    session.open(&path, 16384, false).expect("abrir");
    // Lo que quedaba pendiente ya no resuelve a nada: ni commit ni aviso del hexadecimal.
    session.sync_editor();
    assert_eq!(
        session.editor.confirm_hex(px_core::Mode::Indexed, true),
        HexOutcome::Nothing
    );
    // El documento nuevo es el recién abierto, sin cambios.
    assert!(!session.modified());
    assert!(!session.document.can_undo());
}

#[test]
fn fila_10_un_open_con_error_no_cambia_la_sesion_ni_descarta_el_texto() {
    let tmp = TmpDir::new();

    // Fichero inexistente, con y sin `discard`.
    for discard in [false, true] {
        let mut session = session_with_pending();
        let before = session.document.state_id();
        let result = session.open(&tmp.path().join("no-existe.png"), 16384, discard);
        assert!(result.is_err(), "discard {discard}: {result:?}");
        assert!(session.editor.has_pending(), "discard {discard}");
        assert_eq!(
            session.editor.hex_display().as_deref(),
            Some("#00ff00"),
            "discard {discard}"
        );
        assert!(!session.take_discarded_text(), "discard {discard}");
        assert_eq!(session.choice, ColorChoice::Entry(2), "discard {discard}");
        assert_eq!(session.document.state_id(), before, "discard {discard}");
    }

    // Un fichero que no es PNG, con `discard`.
    let garbage = tmp.path().join("basura.png");
    fs::write(&garbage, b"esto no es un PNG").expect("escribir");
    let mut session = session_with_pending();
    let result = session.open(&garbage, 16384, true);
    assert!(result.is_err(), "{result:?}");
    assert!(session.editor.has_pending());
    assert!(!session.take_discarded_text());

    // Cambios sin guardar y sin `discard`: `Unsaved`, sin leer el fichero válido que hay.
    let valid = write_image(tmp.path(), "valido.png", &image_f());
    let mut session = session_with_pending();
    let image = session.document.main_image();
    session
        .document
        .apply(Command::SetPaletteColor {
            image,
            index: 0,
            color: BLACK,
        })
        .expect("editar");
    assert!(session.modified());
    let result = session.open(&valid, 16384, false);
    assert_eq!(result, Err(FileError::Unsaved));
    assert!(session.editor.has_pending());
    assert!(!session.take_discarded_text());

    // Contraste: con `discard` el mismo fichero sí se abre y descarta.
    session.open(&valid, 16384, true).expect("abrir");
    assert!(!session.editor.has_pending());
    assert!(session.take_discarded_text());
}

#[test]
fn fila_10_sin_texto_pendiente_un_open_no_avisa() {
    let tmp = TmpDir::new();
    let path = write_image(tmp.path(), "otro.png", &image_256());
    let mut session = session_f();
    select(&mut session, ColorChoice::Entry(2));
    assert!(!session.editor.has_pending());
    session.open(&path, 16384, false).expect("abrir");
    assert!(!session.take_discarded_text());

    // Tampoco tras teclear y confirmar o cancelar: ya no queda nada pendiente.
    let mut session = session_with_pending();
    session.editor.escape_hex();
    session.open(&path, 16384, false).expect("abrir");
    assert!(!session.take_discarded_text(), "tras Escape");

    let mut session = session_with_pending();
    let _ = session.editor.confirm_hex(px_core::Mode::Indexed, false);
    session.open(&path, 16384, false).expect("abrir");
    assert!(!session.take_discarded_text(), "tras confirmar");
}

#[test]
fn fila_10_el_aviso_vale_para_cada_open_con_texto_pendiente() {
    let tmp = TmpDir::new();
    let path = write_image(tmp.path(), "otro.png", &image_f());
    let mut session = session_with_pending();
    session.open(&path, 16384, false).expect("abrir");
    assert!(session.take_discarded_text());

    // Un texto nuevo y otro `open`: vuelve a avisar.
    select(&mut session, ColorChoice::Entry(1));
    session
        .editor
        .type_hex(EditTarget::Entry(1), GREEN, "ff0000".to_string());
    session.open(&path, 16384, false).expect("abrir otra vez");
    assert!(session.take_discarded_text());
    assert!(!session.take_discarded_text());
}

// ---------------------------------------------------------------------------------------
// Fila 11: `view`, `set_choice_revealing` y `reveal_if_choice_changed`.
// ---------------------------------------------------------------------------------------

#[test]
fn una_sesion_nueva_parte_con_la_vista_por_defecto() {
    let session = session_f();
    assert_eq!(session.view, PanelView::default());
    assert!(!session.view.reveal);
    assert_eq!(session.view.offset_px, 0);
    assert_eq!(session.view.last_visible, None);
}

#[test]
fn fila_11_set_choice_revealing_con_otra_eleccion_la_asigna_y_pone_reveal() {
    let mut session = Session::sample(Document::new(image_256()));
    assert_eq!(session.choice, ColorChoice::Entry(0));
    assert!(!session.view.reveal);

    session.set_choice_revealing(ColorChoice::Entry(200));
    assert_eq!(session.choice, ColorChoice::Entry(200));
    assert!(
        session.view.reveal,
        "la elección cambió: el panel la muestra"
    );

    // Una elección de otra clase también cuenta.
    session.view.reveal = false;
    session.set_choice_revealing(ColorChoice::Slot(3));
    assert_eq!(session.choice, ColorChoice::Slot(3));
    assert!(session.view.reveal);
}

#[test]
fn fila_11_set_choice_revealing_con_la_misma_eleccion_no_pone_reveal() {
    let mut session = Session::sample(Document::new(image_256()));
    session.choice = ColorChoice::Entry(200);
    assert!(!session.view.reveal);
    session.set_choice_revealing(ColorChoice::Entry(200));
    assert_eq!(session.choice, ColorChoice::Entry(200));
    assert!(!session.view.reveal, "no cambió: no hay nada que mostrar");
}

#[test]
fn fila_11_set_choice_revealing_con_la_misma_eleccion_no_apaga_un_reveal_pendiente() {
    let mut session = Session::sample(Document::new(image_256()));
    session.choice = ColorChoice::Entry(7);
    session.view.reveal = true;
    session.set_choice_revealing(ColorChoice::Entry(7));
    assert!(session.view.reveal, "solo el panel lo apaga");
}

#[test]
fn fila_11_set_choice_revealing_no_toca_el_resto_de_la_vista_ni_del_desplazamiento() {
    let mut session = Session::sample(Document::new(image_256()));
    session.view = PanelView {
        offset_px: 12,
        rest_px: 1,
        last_visible: Some(5),
        reveal: false,
    };
    session.scroll = Scroll {
        first_row: 3,
        rest: 4,
    };
    session.set_choice_revealing(ColorChoice::Entry(100));
    assert_eq!(
        session.view,
        PanelView {
            offset_px: 12,
            rest_px: 1,
            last_visible: Some(5),
            reveal: true,
        }
    );
    // Mostrar la elegida es cosa del panel: la sesión no mueve la rejilla.
    assert_eq!(
        session.scroll,
        Scroll {
            first_row: 3,
            rest: 4
        }
    );
}

#[test]
fn fila_11_reveal_if_choice_changed_solo_pone_reveal_si_la_eleccion_cambio() {
    let mut session = Session::sample(Document::new(image_256()));
    session.choice = ColorChoice::Entry(10);

    // Sin cambio: nada.
    session.reveal_if_choice_changed(ColorChoice::Entry(10));
    assert!(!session.view.reveal);
    assert_eq!(session.choice, ColorChoice::Entry(10));

    // Con cambio: reveal, y la elección no se toca.
    session.reveal_if_choice_changed(ColorChoice::Entry(11));
    assert!(session.view.reveal);
    assert_eq!(session.choice, ColorChoice::Entry(10));

    // Un reveal pendiente no se apaga si no hubo cambio.
    session.reveal_if_choice_changed(ColorChoice::Entry(10));
    assert!(session.view.reveal);

    // Y de otra clase también cuenta como cambio.
    session.view.reveal = false;
    session.reveal_if_choice_changed(ColorChoice::Slot(1));
    assert!(session.view.reveal);
}

#[test]
fn fila_11_abrir_un_fichero_reinicia_la_vista() {
    let tmp = TmpDir::new();
    let path = write_image(tmp.path(), "otro.png", &image_f());
    let mut session = Session::sample(Document::new(image_256()));
    session.view = PanelView {
        offset_px: 30,
        rest_px: 1,
        last_visible: Some(3),
        reveal: true,
    };
    session.open(&path, 16384, false).expect("abrir");
    assert_eq!(session.view, PanelView::default());
    assert!(!session.view.reveal);
    assert_eq!(session.scroll, Scroll::default());
}

#[test]
fn fila_11_un_open_con_error_no_cambia_la_vista() {
    let tmp = TmpDir::new();
    let vista = PanelView {
        offset_px: 30,
        rest_px: 1,
        last_visible: Some(3),
        reveal: true,
    };
    for discard in [false, true] {
        let mut session = Session::sample(Document::new(image_256()));
        session.view = vista;
        let result = session.open(&tmp.path().join("no-existe.png"), 16384, discard);
        assert!(result.is_err(), "discard {discard}");
        assert_eq!(session.view, vista, "discard {discard}");
    }
}
