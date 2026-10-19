//! Tests de `Session` con chunks auxiliares (specs/hito-2-chunks-auxiliares.md, unidad 13b): las
//! filas 9, 10 y 11, y la parte de `Session` de la 7 (APNG). En ROJO hasta la implementación.
//!
//! Cada test usa su propio directorio temporal y nunca guarda dentro de `golden/`: el corpus se
//! copia al directorio temporal antes de abrirlo y guardarlo. Los PNG salen de `write_png` con
//! chunks insertados a mano (CRC-32 y Adler-32 escritos aquí, sin dependencias nuevas); solo el
//! RGB de la fila 11 se construye entero, porque `write_png` no escribe RGB.
//!
//! **Suposiciones y ambigüedades:**
//! - `describe_saved_dialog` de la fila 11 (causa **y** conversión) se lee como «la forma larga de
//!   hoy»: `el original se convirtió al abrir; guardado como x-copia.png`.
//! - Los dos ficheros de Grafx2 se abren sin conversión (así lo dice la fila 10).

use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use px_core::{Command, Document, Image, Mode, Pixel, PixelEdit, Rgba};
use px_formats::{read_png, write_png};

use crate::dialogs::{DialogKind, request_for};

// ---------------------------------------------------------------------------------------
// Ayudantes: directorio temporal, CRC, chunks y ficheros PNG.
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
            std::env::temp_dir().join(format!("pixoko-px-app-aux-{}-{n}", std::process::id()));
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

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/png")
}

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

fn iccp() -> Vec<u8> {
    let mut data = b"perfil\0\0".to_vec();
    data.extend(zlib_stored(b"datos del perfil"));
    chunk(b"iCCP", &data)
}

fn text() -> Vec<u8> {
    chunk(b"tEXt", b"Title\0pixoko")
}

fn phys() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend(2835u32.to_be_bytes());
    data.extend(2835u32.to_be_bytes());
    data.push(1);
    chunk(b"pHYs", &data)
}

fn time() -> Vec<u8> {
    chunk(b"tIME", &[0x07, 0xEA, 10, 17, 12, 0, 0])
}

fn exif() -> Vec<u8> {
    chunk(b"eXIf", b"MM\0*\0\0\0\x08")
}

/// Inserta chunks en un PNG de `write_png` (un solo IDAT): `before` justo tras el IHDR (que
/// acaba en el byte 33) y `after` justo antes del IEND (los últimos 12 bytes).
fn insert(bytes: &[u8], before: &[Vec<u8>], after: &[Vec<u8>]) -> Vec<u8> {
    let iend = bytes.len() - 12;
    let mut v = bytes[..33].to_vec();
    for c in before {
        v.extend(c);
    }
    v.extend(&bytes[33..iend]);
    for c in after {
        v.extend(c);
    }
    v.extend(&bytes[iend..]);
    v
}

fn pixel_1x1() -> Image {
    let mut image = Image::new(1, 1, Mode::Rgba).expect("imagen");
    image
        .set_pixel(0, 0, Pixel::Rgba(Rgba::new(10, 20, 30, 40)))
        .expect("píxel");
    image
}

/// RGBA de 1×1 con esos chunks entre el IHDR y el IDAT.
fn rgba_with(chunks: &[Vec<u8>]) -> Vec<u8> {
    insert(&write_png(&pixel_1x1()).expect("write_png"), chunks, &[])
}

/// PNG RGB (tipo 2, 8 bits) de 1×1 con esos chunks entre el IHDR y el IDAT.
fn rgb_with(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut ihdr = Vec::new();
    ihdr.extend(1u32.to_be_bytes());
    ihdr.extend(1u32.to_be_bytes());
    ihdr.extend([8, 2, 0, 0, 0]);
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend(chunk(b"IHDR", &ihdr));
    for c in chunks {
        v.extend(c);
    }
    v.extend(chunk(b"IDAT", &zlib_stored(&[0, 10, 20, 30])));
    v.extend(chunk(b"IEND", &[]));
    v
}

/// `fcTL` de 2×1 sin desplazamiento (26 bytes) con ese número de secuencia.
fn fctl(sequence: u32) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend(sequence.to_be_bytes());
    d.extend(2u32.to_be_bytes());
    d.extend(1u32.to_be_bytes());
    d.extend([0u8; 8]);
    d.extend([0, 1, 0, 10, 0, 0]);
    chunk(b"fcTL", &d)
}

/// APNG de 2 cuadros y 2×1 en gris: el IDAT (`[10, 20]`) es el primer fotograma.
fn apng() -> Vec<u8> {
    let mut image = Image::new(2, 1, Mode::Gray).expect("imagen");
    image.set_pixel(0, 0, Pixel::Gray(10)).expect("píxel");
    image.set_pixel(1, 0, Pixel::Gray(20)).expect("píxel");
    let mut fdat = 2u32.to_be_bytes().to_vec();
    fdat.extend(zlib_stored(&[0, 30, 40]));
    insert(
        &write_png(&image).expect("write_png"),
        &[chunk(b"acTL", &[0, 0, 0, 2, 0, 0, 0, 0]), fctl(0)],
        &[fctl(1), chunk(b"fdAT", &fdat)],
    )
}

fn fresh_session() -> Session {
    Session::sample(Document::new(pixel_1x1()))
}

/// Escribe `bytes` como `x.png` en `dir` y devuelve la ruta.
fn write_x(dir: &Path, bytes: &[u8]) -> PathBuf {
    let path = dir.join("x.png");
    fs::write(&path, bytes).expect("escribir x.png");
    path
}

fn main_image(session: &Session) -> &Image {
    session
        .document
        .image(session.document.main_image())
        .expect("imagen principal")
}

/// Cambia el píxel (0, 0) del documento por otro distinto.
fn edit_first_pixel(session: &mut Session) {
    let id = session.document.main_image();
    let pixel = match main_image(session).get_pixel(0, 0).expect("píxel") {
        Pixel::Rgba(c) => Pixel::Rgba(Rgba::new(!c.r, c.g, c.b, c.a)),
        Pixel::Gray(g) => Pixel::Gray(g.wrapping_add(1)),
        other => other,
    };
    assert!(
        session
            .document
            .apply(Command::SetPixels {
                image: id,
                pixels: vec![PixelEdit { x: 0, y: 0, pixel }],
            })
            .expect("apply")
    );
}

const ICC_CAUSE: &str = "perfil de color ICC ignorado";
const ICC_SAVED: &str = "perfil de color ICC ignorado; guardado como x-copia.png";

/// Sesión que abre un `x.png` RGBA de 1×1 con `iCCP`; devuelve también los bytes originales.
fn open_with_iccp(dir: &Path) -> (Session, Vec<u8>, String) {
    let original = rgba_with(&[iccp()]);
    let path = write_x(dir, &original);
    let mut session = fresh_session();
    let opened = session.open(&path, 16384, false).expect("abrir");
    (session, original, opened)
}

// ---------------------------------------------------------------------------------------
// Fila 9: iCCP activa la copia.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_9_abrir_con_iccp_avisa_y_save_escribe_la_copia() {
    let tmp = TmpDir::new();
    let (mut session, original, opened) = open_with_iccp(tmp.path());

    assert!(session.converted());
    assert!(!session.modified());
    assert_eq!(
        opened,
        "perfil de color ICC ignorado; Ctrl+S guardará una copia · abierto x.png · 1×1 · RGBA"
    );
    assert!(opened.starts_with(ICC_CAUSE));

    let saved = session.save(tmp.path()).expect("save");

    let copy = tmp.path().join("x-copia.png");
    assert_eq!(saved.path, copy);
    assert_eq!(saved.copy_reason.as_deref(), Some(ICC_CAUSE));
    assert_eq!(describe_saved(&saved), ICC_SAVED);
    assert_eq!(describe_saved_dialog(&saved), ICC_SAVED);
    assert_eq!(
        fs::read(tmp.path().join("x.png")).unwrap(),
        original,
        "original intacto byte a byte"
    );
    assert_eq!(names(tmp.path()), ["x-copia.png", "x.png"]);
    let stored = read_png(&fs::read(&copy).unwrap()).unwrap();
    assert_eq!(&stored.image, main_image(&session));
    assert!(stored.aux.found.is_empty(), "la copia no lleva el iCCP");
    assert_eq!(session.path(), Some(copy.as_path()));
    assert!(!session.modified());
    assert!(!session.converted());
}

#[test]
fn fila_9_el_segundo_save_sobrescribe_la_copia_sin_motivo() {
    let tmp = TmpDir::new();
    let (mut session, original, _) = open_with_iccp(tmp.path());
    session.save(tmp.path()).expect("primer save");
    edit_first_pixel(&mut session);

    let saved = session.save(tmp.path()).expect("segundo save");

    let copy = tmp.path().join("x-copia.png");
    assert_eq!(saved.path, copy);
    assert_eq!(saved.copy_reason, None);
    assert_eq!(describe_saved(&saved), "guardado x-copia.png");
    assert_eq!(names(tmp.path()), ["x-copia.png", "x.png"]);
    let stored = read_png(&fs::read(&copy).unwrap()).unwrap();
    assert_eq!(
        &stored.image,
        main_image(&session),
        "la copia se sobrescribió"
    );
    assert_eq!(fs::read(tmp.path().join("x.png")).unwrap(), original);
    assert!(!session.modified());
}

#[test]
fn fila_9_save_to_con_la_ruta_del_documento_o_una_equivalente_escribe_la_copia() {
    for variant in ["igual", "punto", "punto-punto"] {
        let tmp = TmpDir::new();
        fs::create_dir(tmp.path().join("sub")).unwrap();
        let (mut session, original, _) = open_with_iccp(tmp.path());
        let chosen = match variant {
            "igual" => tmp.path().join("x.png"),
            "punto" => tmp.path().join(".").join("x.png"),
            _ => tmp.path().join("sub").join("..").join("x.png"),
        };

        let saved = session.save_to(&chosen).expect("save_to");

        let copy = tmp.path().join("x-copia.png");
        assert_eq!(saved.path, copy, "{variant}");
        assert_eq!(saved.copy_reason.as_deref(), Some(ICC_CAUSE), "{variant}");
        assert_eq!(describe_saved_dialog(&saved), ICC_SAVED, "{variant}");
        assert_eq!(describe_saved(&saved), ICC_SAVED, "{variant}");
        assert_eq!(
            fs::read(tmp.path().join("x.png")).unwrap(),
            original,
            "{variant}: original intacto"
        );
        assert_eq!(
            names(tmp.path()),
            ["sub", "x-copia.png", "x.png"],
            "{variant}"
        );
        assert!(!session.converted(), "{variant}");
    }
}

// ---------------------------------------------------------------------------------------
// Fila 10: lo que solo se informa no activa la copia.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_10_texto_resolucion_fecha_y_exif_solo_se_informan() {
    let tmp = TmpDir::new();
    let original = rgba_with(&[text(), phys(), time(), exif()]);
    let path = write_x(tmp.path(), &original);
    let mut session = fresh_session();

    let opened = session.open(&path, 16384, false).expect("abrir");

    assert!(!session.converted());
    assert_eq!(
        opened,
        "abierto x.png · 1×1 · RGBA · ignorados: texto, Exif"
    );
    assert!(!opened.contains("Ctrl+S"));
    assert!(!opened.contains("resolución") && !opened.contains("fecha"));

    // El nombre propuesto es el del propio fichero, no el de una copia.
    for kind in [DialogKind::Save, DialogKind::SaveAs] {
        let request = request_for(kind, session.path(), session.converted(), None, None);
        assert_eq!(request.file_name.as_deref(), Some("x.png"), "{kind:?}");
    }

    edit_first_pixel(&mut session);
    let saved = session.save(tmp.path()).expect("save");
    assert_eq!(saved.path, path);
    assert_eq!(saved.copy_reason, None);
    assert_eq!(describe_saved(&saved), "guardado x.png");
    assert_eq!(names(tmp.path()), ["x.png"]);
    let stored = read_png(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        &stored.image,
        main_image(&session),
        "el original se sobrescribió"
    );
}

#[test]
fn fila_10_los_dos_grafx2_dicen_texto_y_otros_y_se_sobrescriben() {
    let mut grafx2: Vec<PathBuf> = fs::read_dir(corpus_dir())
        .expect("leer golden/png")
        .map(|entry| entry.expect("entrada").path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "png")
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("grafx2"))
        })
        .collect();
    grafx2.sort();
    assert_eq!(grafx2.len(), 2, "el corpus tiene dos de Grafx2");

    for source in grafx2 {
        let tmp = TmpDir::new();
        let name = source.file_name().unwrap().to_string_lossy().into_owned();
        let path = tmp.path().join(&name);
        fs::copy(&source, &path).expect("copiar el fichero del corpus");
        let mut session = fresh_session();

        let opened = session.open(&path, 16384, false).expect("abrir");

        assert!(!session.converted(), "{name}");
        assert!(
            opened.ends_with(" · ignorados: texto, otros"),
            "{name}: {opened}"
        );
        assert!(opened.contains(&name), "{name}: {opened}");
        assert!(!opened.contains("Ctrl+S"), "{name}: {opened}");
        let request = request_for(
            DialogKind::Save,
            session.path(),
            session.converted(),
            None,
            None,
        );
        assert_eq!(request.file_name.as_deref(), Some(name.as_str()), "{name}");

        let saved = session.save(tmp.path()).expect("save");
        assert_eq!(saved.path, path, "{name}");
        assert_eq!(saved.copy_reason, None, "{name}");
        assert_eq!(
            names(tmp.path()),
            std::slice::from_ref(&name),
            "{name}: sin copia"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 11: conversión y causa a la vez.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_11_rgb_con_iccp_une_conversion_y_causa() {
    let tmp = TmpDir::new();
    let original = rgb_with(&[iccp()]);
    let path = write_x(tmp.path(), &original);
    let mut session = fresh_session();

    let opened = session.open(&path, 16384, false).expect("abrir");

    assert!(session.converted());
    // Una sola coletilla, con la causa delante y las conversiones detrás.
    assert_eq!(
        opened,
        "perfil de color ICC ignorado; Ctrl+S guardará una copia · abierto x.png · 1×1 · RGBA · convertido: RGB convertido a RGBA"
    );
    assert_eq!(opened.matches("Ctrl+S guardará una copia").count(), 1);

    let saved = session.save(tmp.path()).expect("save");

    let reason = "RGB convertido a RGBA, perfil de color ICC ignorado";
    assert_eq!(saved.path, tmp.path().join("x-copia.png"));
    assert_eq!(saved.copy_reason.as_deref(), Some(reason));
    assert_eq!(
        describe_saved(&saved),
        format!(
            "guardado como x-copia.png: el original se convierte al abrir ({reason}) y no se sobrescribe"
        )
    );
    assert_eq!(fs::read(&path).unwrap(), original, "original intacto");
    assert_eq!(names(tmp.path()), ["x-copia.png", "x.png"]);
}

#[test]
fn fila_11_el_dialogo_usa_la_forma_larga_de_hoy_cuando_hay_conversion() {
    let tmp = TmpDir::new();
    let path = write_x(tmp.path(), &rgb_with(&[iccp()]));
    let mut session = fresh_session();
    session.open(&path, 16384, false).expect("abrir");

    let saved = session.save_to(&path).expect("save_to");

    assert_eq!(
        saved.copy_reason.as_deref(),
        Some("RGB convertido a RGBA, perfil de color ICC ignorado")
    );
    assert_eq!(
        describe_saved_dialog(&saved),
        "el original se convirtió al abrir; guardado como x-copia.png"
    );
}

// ---------------------------------------------------------------------------------------
// Fila 7 (parte de `Session`): APNG de 2 cuadros.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_7_apng_se_abre_como_su_primer_fotograma_y_save_escribe_la_copia() {
    let tmp = TmpDir::new();
    let original = apng();
    let path = write_x(tmp.path(), &original);
    let mut session = fresh_session();

    let opened = session.open(&path, 16384, false).expect("abrir");

    // La imagen es la del IDAT (el primer fotograma).
    let image = main_image(&session);
    assert_eq!(
        (image.width(), image.height(), image.mode()),
        (2, 1, Mode::Gray)
    );
    assert_eq!(image.get_pixel(0, 0).unwrap(), Pixel::Gray(10));
    assert_eq!(image.get_pixel(1, 0).unwrap(), Pixel::Gray(20));
    assert!(session.converted());
    assert!(
        opened.starts_with("APNG: solo el 1.er fotograma; Ctrl+S guardará una copia"),
        "{opened}"
    );

    let saved = session.save(tmp.path()).expect("save");

    let copy = tmp.path().join("x-copia.png");
    assert_eq!(saved.path, copy);
    assert_eq!(
        saved.copy_reason.as_deref(),
        Some("APNG: solo el 1.er fotograma")
    );
    assert_eq!(
        describe_saved(&saved),
        "APNG: solo el 1.er fotograma; guardado como x-copia.png"
    );
    assert_eq!(
        describe_saved_dialog(&saved),
        "APNG: solo el 1.er fotograma; guardado como x-copia.png"
    );
    assert_eq!(fs::read(&path).unwrap(), original, "original intacto");
    assert_eq!(names(tmp.path()), ["x-copia.png", "x.png"]);
    let stored = read_png(&fs::read(&copy).unwrap()).unwrap();
    assert_eq!(&stored.image, main_image(&session));
    assert!(stored.aux.found.is_empty(), "la copia ya no es un APNG");
    assert!(!session.converted());
}
