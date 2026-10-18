//! Tests de `session` de la unidad 16 (specs/hito-2-dialogos-archivo.md): filas 5 y 6
//! (`Session::save_to`), 8 (el resultado de un diálogo llega a `Session::open`), 9 (la escritura
//! de `Session::save` y `Session::save_as` tras la segunda pulsación) y 12 (textos de
//! `Exists` y de `describe_saved_dialog`).
//!
//! Cada test usa su propio directorio temporal, que se borra al terminar. Ningún test depende
//! del directorio actual ni escribe dentro de `golden/`: el corpus solo se lee.

use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use px_core::{Command, Document, Image, Mode, Pixel, PixelEdit, Rgba};
use px_formats::{read_png, write_png};
use px_ui::text::INTERFACE_CHARSET;

use crate::confirm::{Confirm, Confirmation, Request};
use crate::dialogs::{DialogKind, DialogRequest, Dialogs, FilePicker, Finished, Outcome};
use crate::files::FileError;

// ---------------------------------------------------------------------------------------
// Ayudantes: directorio temporal, imágenes y PNG RGB de 1×1 hecho a mano.
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
            "pixoko-px-app-session-dialog-{}-{n}",
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

/// Nombres de lo que hay en `dir`, ordenados.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("leer el directorio")
        .map(|entry| {
            entry
                .expect("entrada")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// Ningún `.pixoko-tmp` suelto en `dir`.
fn assert_no_temporaries(dir: &Path) {
    let found: Vec<String> = names(dir)
        .into_iter()
        .filter(|name| name.contains("pixoko-tmp"))
        .collect();
    assert!(found.is_empty(), "quedan temporales: {found:?}");
}

fn corpus() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/png");
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .expect("leer golden/png")
        .map(|entry| entry.expect("entrada").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
        .collect();
    files.sort();
    files
}

/// Imagen RGBA de 5×4 con píxeles variados.
fn sample_image() -> Image {
    let (w, h) = (5u32, 4u32);
    let mut image = Image::new(w, h, Mode::Rgba).expect("imagen");
    for y in 0..h {
        for x in 0..w {
            let n = (y * w + x) as u8;
            let color = Rgba::new(n.wrapping_mul(13), n.wrapping_mul(7), n, 255 - n);
            image.set_pixel(x, y, Pixel::Rgba(color)).expect("píxel");
        }
    }
    image
}

/// Cambia el píxel `(0, 0)` por un color distinto.
fn edit(session: &mut Session) {
    let id = session.document.main_image();
    let changed = session
        .document
        .apply(Command::SetPixels {
            image: id,
            pixels: vec![PixelEdit {
                x: 0,
                y: 0,
                pixel: Pixel::Rgba(Rgba::new(200, 100, 50, 255)),
            }],
        })
        .expect("apply");
    assert!(changed, "el píxel (0, 0) cambia");
}

fn main_image(document: &Document) -> &Image {
    document
        .image(document.main_image())
        .expect("imagen principal")
}

/// Sesión sin ruta con cambios sin guardar.
fn modified_session() -> Session {
    let mut session = Session::sample(Document::new(sample_image()));
    edit(&mut session);
    assert!(session.modified());
    session
}

/// Escribe en `dir/name` un PNG válido que Pixoko no convierte al abrir, y devuelve sus bytes.
fn write_plain_png(dir: &Path, name: &str) -> Vec<u8> {
    let bytes = write_png(&sample_image()).expect("codificar");
    fs::write(dir.join(name), &bytes).expect("escribir el PNG");
    bytes
}

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// CRC-32 de la especificación PNG.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Adler-32 de la especificación zlib.
fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Flujo zlib con un único bloque deflate «stored» (datos pequeños).
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut v = vec![0x78, 0x01, 1];
    let n = data.len() as u16;
    v.extend(n.to_le_bytes());
    v.extend((!n).to_le_bytes());
    v.extend(data);
    v.extend(adler32(data).to_be_bytes());
    v
}

/// Chunk completo: longitud, tipo, datos y CRC.
fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + data.len());
    v.extend((data.len() as u32).to_be_bytes());
    v.extend(kind);
    v.extend(data);
    let mut body = kind.to_vec();
    body.extend(data);
    v.extend(crc32(&body).to_be_bytes());
    v
}

/// PNG RGB (tipo 2, 8 bits) de 1×1: Pixoko lo convierte a RGBA al abrir.
fn rgb_1x1(r: u8, g: u8, b: u8) -> Vec<u8> {
    let mut ihdr = Vec::new();
    ihdr.extend(1u32.to_be_bytes());
    ihdr.extend(1u32.to_be_bytes());
    ihdr.extend([8, 2, 0, 0, 0]);
    let mut v = SIGNATURE.to_vec();
    v.extend(chunk(b"IHDR", &ihdr));
    v.extend(chunk(b"IDAT", &zlib_stored(&[0, r, g, b])));
    v.extend(chunk(b"IEND", &[]));
    v
}

/// Sesión abierta sobre `dir/x.png`, un RGB de 1×1 que se convierte al abrir. Devuelve los
/// bytes del original.
fn converted_session(dir: &Path) -> (Session, Vec<u8>) {
    let original = rgb_1x1(10, 20, 30);
    fs::write(dir.join("x.png"), &original).unwrap();
    let mut session = Session::sample(Document::new(sample_image()));
    session
        .open(&dir.join("x.png"), 16384, false)
        .expect("abrir el RGB de 1×1");
    assert!(session.converted());
    assert!(!session.modified());
    (session, original)
}

fn assert_in_charset(text: &str) {
    for c in text.chars() {
        assert!(
            INTERFACE_CHARSET.contains(c),
            "{text:?} lleva {c:?}, que no está en INTERFACE_CHARSET"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 5: Session::save_to.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_5_sin_extension_escribe_nombre_png() {
    let tmp = TmpDir::new();
    let mut session = modified_session();
    let expected = main_image(&session.document).clone();

    let saved = session.save_to(&tmp.path().join("nuevo")).expect("guardar");

    let target = tmp.path().join("nuevo.png");
    assert_eq!(saved.path, target);
    assert_eq!(saved.copy_reason, None);
    assert_eq!(names(tmp.path()), ["nuevo.png"]);
    let stored = read_png(&fs::read(&target).unwrap()).unwrap();
    assert_eq!(stored.image, expected);
    assert_eq!(session.path(), Some(target.as_path()), "la ruta nueva");
    assert!(!session.modified());
    assert!(!session.converted());
    assert_no_temporaries(tmp.path());
}

#[test]
fn fila_5_la_eleccion_sobrescribe_un_png_existente_entero() {
    let tmp = TmpDir::new();
    let old = tmp.path().join("viejo.png");
    fs::write(&old, vec![0xAAu8; 1024]).unwrap();
    let mut session = modified_session();
    let expected = write_png(main_image(&session.document)).unwrap();

    let saved = session.save_to(&old).expect("guardar sobre el existente");

    assert_eq!(saved.path, old);
    assert_eq!(saved.copy_reason, None);
    assert_eq!(
        fs::read(&old).unwrap(),
        expected,
        "contenido nuevo completo"
    );
    assert_eq!(names(tmp.path()), ["viejo.png"]);
    assert_no_temporaries(tmp.path());
    assert!(!session.modified());
    assert_eq!(session.path(), Some(old.as_path()));
}

#[test]
fn fila_5_si_se_anade_la_extension_y_existe_es_exists_y_no_escribe_nada() {
    let tmp = TmpDir::new();
    let existing = tmp.path().join("otro.png");
    fs::write(&existing, b"no tocar").unwrap();
    let mut session = modified_session();
    let before = session.document.clone();

    let result = session.save_to(&tmp.path().join("otro"));

    let error = result.expect_err("ya existe otro.png");
    assert_eq!(error, FileError::Exists(existing.clone()));
    assert_eq!(error.to_string(), "ya existe otro.png; elige otro nombre");
    assert_eq!(fs::read(&existing).unwrap(), b"no tocar");
    assert_eq!(names(tmp.path()), ["otro.png"]);
    assert_no_temporaries(tmp.path());
    assert!(session.modified(), "sigue habiendo cambios sin guardar");
    assert!(session.path().is_none());
    assert_eq!(session.document, before);
}

#[test]
fn fila_5_exists_tambien_con_otra_extension_distinta_de_png() {
    let tmp = TmpDir::new();
    fs::write(tmp.path().join("casa.bmp.png"), b"no tocar").unwrap();
    let mut session = modified_session();

    let error = session
        .save_to(&tmp.path().join("casa.bmp"))
        .expect_err("ya existe casa.bmp.png");

    assert_eq!(error, FileError::Exists(tmp.path().join("casa.bmp.png")));
    assert_eq!(names(tmp.path()), ["casa.bmp.png"]);
    assert!(session.modified());
}

#[test]
fn fila_5_extension_png_en_mayusculas_no_se_toca() {
    let tmp = TmpDir::new();
    let mut session = modified_session();

    let saved = session.save_to(&tmp.path().join("X.PNG")).expect("guardar");

    assert_eq!(saved.path, tmp.path().join("X.PNG"));
    assert_eq!(names(tmp.path()), ["X.PNG"]);
    assert!(!session.modified());
    assert_no_temporaries(tmp.path());
}

#[test]
fn fila_5_otra_extension_recibe_png_al_final() {
    let tmp = TmpDir::new();
    let mut session = modified_session();

    let saved = session.save_to(&tmp.path().join("x.bmp")).expect("guardar");

    assert_eq!(saved.path, tmp.path().join("x.bmp.png"));
    assert_eq!(names(tmp.path()), ["x.bmp.png"]);
}

#[test]
fn fila_5_un_directorio_que_no_existe_es_error_y_la_sesion_no_cambia() {
    let tmp = TmpDir::new();
    let mut session = modified_session();
    let before = session.document.clone();

    let result = session.save_to(&tmp.path().join("no-existe").join("a.png"));

    assert!(matches!(result, Err(FileError::Write(_))), "{result:?}");
    assert!(names(tmp.path()).is_empty());
    assert!(session.modified());
    assert!(session.path().is_none());
    assert_eq!(session.document, before);
}

#[test]
fn fila_5_la_ruta_del_documento_sin_conversion_se_sobrescribe_sin_motivo() {
    let tmp = TmpDir::new();
    write_plain_png(tmp.path(), "y.png");
    let mut session = Session::sample(Document::new(sample_image()));
    session
        .open(&tmp.path().join("y.png"), 16384, false)
        .unwrap();
    assert!(!session.converted());
    edit(&mut session);
    let expected = main_image(&session.document).clone();

    let saved = session
        .save_to(&tmp.path().join("y.png"))
        .expect("guardar sobre sí mismo");

    assert_eq!(saved.path, tmp.path().join("y.png"));
    assert_eq!(saved.copy_reason, None);
    assert_eq!(names(tmp.path()), ["y.png"]);
    let stored = read_png(&fs::read(tmp.path().join("y.png")).unwrap()).unwrap();
    assert_eq!(stored.image, expected);
    assert!(!session.modified());
}

// ---------------------------------------------------------------------------------------
// Fila 6: no se sobrescribe un PNG que se convierte al abrir.
// ---------------------------------------------------------------------------------------

/// Comprueba el resultado de elegir el propio fichero convertido, sea cual sea su escritura.
fn assert_copy_written(tmp: &Path, session: &Session, saved: &Saved, original: &[u8]) {
    let copy = tmp.join("x-copia.png");
    assert_eq!(saved.path, copy);
    assert_eq!(saved.copy_reason.as_deref(), Some("RGB convertido a RGBA"));
    assert_eq!(
        describe_saved_dialog(saved),
        "el original se convirtió al abrir; guardado como x-copia.png"
    );
    assert_eq!(
        fs::read(tmp.join("x.png")).unwrap(),
        original,
        "original intacto"
    );
    let stored = read_png(&fs::read(&copy).unwrap()).unwrap();
    assert_eq!(&stored.image, main_image(&session.document));
    assert_eq!(session.path(), Some(copy.as_path()));
    assert!(!session.modified());
    assert!(!session.converted());
    assert_no_temporaries(tmp);
}

#[test]
fn fila_6_elegir_la_ruta_del_documento_escribe_la_copia() {
    let tmp = TmpDir::new();
    let (mut session, original) = converted_session(tmp.path());

    let saved = session.save_to(&tmp.path().join("x.png")).expect("guardar");

    assert_copy_written(tmp.path(), &session, &saved, &original);
    assert_eq!(names(tmp.path()), ["x-copia.png", "x.png"]);
}

#[test]
fn fila_6_una_ruta_equivalente_con_punto_y_punto_punto_tambien() {
    for variant in ["dot", "dotdot"] {
        let tmp = TmpDir::new();
        let (mut session, original) = converted_session(tmp.path());
        fs::create_dir(tmp.path().join("sub")).unwrap();
        let chosen = match variant {
            "dot" => tmp.path().join(".").join("x.png"),
            _ => tmp.path().join("sub").join("..").join("x.png"),
        };

        let saved = session.save_to(&chosen).expect("guardar");

        assert_copy_written(tmp.path(), &session, &saved, &original);
        assert_eq!(
            names(tmp.path()),
            ["sub", "x-copia.png", "x.png"],
            "{variant}"
        );
    }
}

#[cfg(unix)]
#[test]
fn fila_6_un_enlace_simbolico_al_documento_tambien() {
    let tmp = TmpDir::new();
    let (mut session, original) = converted_session(tmp.path());
    std::os::unix::fs::symlink(tmp.path().join("x.png"), tmp.path().join("link.png")).unwrap();

    let saved = session
        .save_to(&tmp.path().join("link.png"))
        .expect("guardar");

    assert_copy_written(tmp.path(), &session, &saved, &original);
    assert_eq!(names(tmp.path()), ["link.png", "x-copia.png", "x.png"]);
    assert!(
        fs::symlink_metadata(tmp.path().join("link.png"))
            .unwrap()
            .file_type()
            .is_symlink(),
        "el enlace sigue siendo un enlace"
    );
}

#[test]
fn fila_6_si_el_original_ya_no_existe_se_compara_con_la_ruta_absoluta() {
    // `canonicalize` falla sin el fichero: se compara con `std::path::absolute`.
    let tmp = TmpDir::new();
    let (mut session, _original) = converted_session(tmp.path());
    fs::remove_file(tmp.path().join("x.png")).unwrap();

    let saved = session.save_to(&tmp.path().join("x.png")).expect("guardar");

    assert_eq!(saved.path, tmp.path().join("x-copia.png"));
    assert_eq!(saved.copy_reason.as_deref(), Some("RGB convertido a RGBA"));
    assert_eq!(
        names(tmp.path()),
        ["x-copia.png"],
        "no se recreó el original"
    );
}

#[test]
fn fila_6_con_otra_ruta_se_escribe_la_elegida_sin_motivo_y_se_borra_la_conversion() {
    let tmp = TmpDir::new();
    let (mut session, original) = converted_session(tmp.path());

    let saved = session
        .save_to(&tmp.path().join("otro.png"))
        .expect("guardar");

    let target = tmp.path().join("otro.png");
    assert_eq!(saved.path, target);
    assert_eq!(saved.copy_reason, None);
    assert_eq!(describe_saved_dialog(&saved), "guardado otro.png");
    assert_eq!(fs::read(tmp.path().join("x.png")).unwrap(), original);
    assert_eq!(names(tmp.path()), ["otro.png", "x.png"]);
    assert_eq!(session.path(), Some(target.as_path()));
    assert!(!session.modified());
    assert!(
        !session.converted(),
        "la conversión pendiente queda borrada"
    );

    // Un `save` posterior ya no hace copia: sobrescribe el fichero del documento.
    edit(&mut session);
    let again = session.save(tmp.path()).expect("guardar de nuevo");
    assert_eq!(again.path, target);
    assert_eq!(again.copy_reason, None);
    assert_eq!(names(tmp.path()), ["otro.png", "x.png"]);
}

// ---------------------------------------------------------------------------------------
// Fila 8: el resultado de un diálogo llega a Session::open con el descarte ya confirmado.
// ---------------------------------------------------------------------------------------

/// Selector que resuelve al instante con lo que se le dio.
struct InstantPicker(Option<Option<PathBuf>>);

impl FilePicker for InstantPicker {
    fn start(&mut self, _request: &DialogRequest, _parent: Option<&eframe::Frame>) {}

    fn poll(&mut self) -> Option<Option<PathBuf>> {
        self.0.take()
    }

    fn abandon(&mut self) {}
}

/// El `Finished` de un diálogo de abrir que el usuario resuelve con `result`.
fn finished_open(result: Option<PathBuf>, discard: bool) -> Finished {
    let mut dialogs = Dialogs::new(Box::new(InstantPicker(Some(result))), false, None);
    assert!(dialogs.begin(DialogKind::Open, discard, None, false, 0.0, None));
    dialogs.poll(1.0).expect("el selector ya resolvió")
}

#[test]
fn fila_8_chosen_con_discard_abre_y_descarta() {
    let path = corpus().remove(0);
    let expected = read_png(&fs::read(&path).unwrap()).unwrap().image;
    let mut session = modified_session();

    let finished = finished_open(Some(path.clone()), true);
    assert_eq!(finished.outcome, Outcome::Chosen(path.clone()));
    assert!(finished.discard);
    let Outcome::Chosen(chosen) = finished.outcome else {
        unreachable!()
    };
    session
        .open(&chosen, 16384, finished.discard)
        .expect("abrir con el descarte ya confirmado");

    assert_eq!(main_image(&session.document), &expected);
    assert!(!session.document.can_undo(), "historia vacía");
    assert!(!session.document.can_redo());
    assert!(!session.modified());
    assert_eq!(session.path().and_then(Path::file_name), path.file_name());
}

#[test]
fn fila_8_cancelled_deja_la_sesion_intacta() {
    let session = modified_session();
    let before = session.document.clone();

    let finished = finished_open(None, true);

    // Con `Cancelled` la app no llama a `open`: no pasa nada.
    assert_eq!(finished.outcome, Outcome::Cancelled);
    assert_eq!(session.document, before);
    assert!(session.modified());
    assert!(session.path().is_none());
}

#[test]
fn fila_8_chosen_de_un_fichero_que_no_es_png_es_error_y_deja_la_sesion_intacta() {
    let tmp = TmpDir::new();
    let text = tmp.path().join("nota.png");
    fs::write(&text, b"esto no es un PNG").unwrap();
    let mut session = modified_session();
    let before = session.document.clone();

    let finished = finished_open(Some(text.clone()), true);
    let Outcome::Chosen(chosen) = finished.outcome else {
        panic!("se esperaba Chosen, salió {:?}", finished.outcome);
    };
    let result = session.open(&chosen, 16384, finished.discard);

    assert!(matches!(result, Err(FileError::Png(_))), "{result:?}");
    assert_eq!(session.document, before);
    assert!(session.modified());
    assert!(session.path().is_none());
}

// ---------------------------------------------------------------------------------------
// Fila 9: la escritura tras la segunda pulsación.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_9_segunda_pulsacion_de_guardar_escribe_sin_titulo_en_el_directorio() {
    let tmp = TmpDir::new();
    let mut confirm = Confirm::new();
    let mut session = modified_session();
    let request = Request::SaveFallback { as_copy: false };

    // Primera pulsación: arma y no escribe nada.
    assert_eq!(confirm.request(request.clone(), 0.0), Confirmation::Armed);
    assert!(names(tmp.path()).is_empty());

    // Segunda, dentro de 5 s: se hace lo de la unidad 15.
    assert!(confirm.confirm_armed(&request, 2.0));
    let saved = session.save(tmp.path()).expect("guardar");

    assert_eq!(saved.path, tmp.path().join("sin-titulo.png"));
    assert_eq!(saved.copy_reason, None);
    assert_eq!(names(tmp.path()), ["sin-titulo.png"]);
    assert!(!session.modified());
    assert_eq!(
        crate::session::describe_saved(&saved),
        "guardado sin-titulo.png"
    );
}

#[test]
fn fila_9_la_segunda_pulsacion_no_pisa_un_sin_titulo_existente() {
    let tmp = TmpDir::new();
    fs::write(tmp.path().join("sin-titulo.png"), b"no tocar").unwrap();
    let mut confirm = Confirm::new();
    let mut session = modified_session();
    let request = Request::SaveFallback { as_copy: false };
    assert_eq!(confirm.request(request.clone(), 0.0), Confirmation::Armed);

    assert!(confirm.confirm_armed(&request, 1.0));
    let saved = session.save(tmp.path()).expect("guardar");

    assert_eq!(saved.path, tmp.path().join("sin-titulo-2.png"));
    assert_eq!(
        fs::read(tmp.path().join("sin-titulo.png")).unwrap(),
        b"no tocar"
    );
    assert_eq!(names(tmp.path()), ["sin-titulo-2.png", "sin-titulo.png"]);
}

#[test]
fn fila_9_segunda_pulsacion_de_guardar_como_escribe_la_copia() {
    let tmp = TmpDir::new();
    let original = write_plain_png(tmp.path(), "y.png");
    let mut session = Session::sample(Document::new(sample_image()));
    session
        .open(&tmp.path().join("y.png"), 16384, false)
        .unwrap();
    edit(&mut session);
    let mut confirm = Confirm::new();
    let request = Request::SaveFallback { as_copy: true };
    assert_eq!(confirm.request(request.clone(), 0.0), Confirmation::Armed);

    assert!(confirm.confirm_armed(&request, 3.0));
    let saved = session.save_as(tmp.path()).expect("guardar una copia");

    assert_eq!(saved.path, tmp.path().join("y-copia.png"));
    assert_eq!(fs::read(tmp.path().join("y.png")).unwrap(), original);
    assert_eq!(names(tmp.path()), ["y-copia.png", "y.png"]);
}

#[test]
fn fila_9_sin_segunda_pulsacion_no_se_escribe_nada() {
    // Expirada o de otra clase: `confirm_armed` es falso y la app abre el diálogo, no escribe.
    let tmp = TmpDir::new();
    let mut confirm = Confirm::new();
    assert_eq!(
        confirm.request(Request::SaveFallback { as_copy: false }, 0.0),
        Confirmation::Armed
    );
    assert!(!confirm.confirm_armed(&Request::SaveFallback { as_copy: false }, 5.0));
    assert!(!confirm.confirm_armed(&Request::SaveFallback { as_copy: true }, 1.0));
    assert!(names(tmp.path()).is_empty());
}

// ---------------------------------------------------------------------------------------
// Fila 12: textos de Exists y de describe_saved_dialog.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_12_display_de_exists_dice_solo_el_nombre() {
    let error = FileError::Exists(PathBuf::from("/a/b/casa.png"));
    let text = error.to_string();
    assert_eq!(text, "ya existe casa.png; elige otro nombre");
    assert!(!text.contains("/a/b"), "{text}");
    assert_in_charset(&text);
}

#[test]
fn fila_12_describe_saved_dialog_con_copia_dice_que_el_original_no_se_toco() {
    let saved = Saved {
        path: PathBuf::from("/a/b/casa-copia.png"),
        copy_reason: Some("RGB convertido a RGBA".to_string()),
    };
    let text = describe_saved_dialog(&saved);
    assert_eq!(
        text,
        "el original se convirtió al abrir; guardado como casa-copia.png"
    );
    assert_in_charset(&text);
}

#[test]
fn fila_12_describe_saved_dialog_sin_copia_dice_guardado() {
    let saved = Saved {
        path: PathBuf::from("/a/b/casa.png"),
        copy_reason: None,
    };
    let text = describe_saved_dialog(&saved);
    assert_eq!(text, "guardado casa.png");
    assert_in_charset(&text);
}
