//! Tests de `session` (specs/hito-2-abrir-guardar.md): filas 2, 6, 7, 9, 10, 11, 16 y 17;
//! propiedades 1, 2 y 5 (con `Session::open`; la de `load_png` va en la misma propiedad). Y, de
//! specs/hito-2-panel-paletas.md, las filas 8 (abrir reinicia la selección y el desplazamiento del
//! panel; sustituye a los dos tests de `adjust_slot`) y 13 (guardar no los reinicia).
//!
//! Cada test usa su propio directorio temporal y nunca guarda dentro de `golden/`: el corpus se
//! copia al directorio temporal antes de editarlo y guardarlo. Ningún test depende del
//! directorio actual (`save` recibe `new_dir`).

use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;
use px_core::{Command, Document, Image, Mode, Palette, Pixel, PixelEdit, Rgba};
use px_formats::{Conversion, read_png, write_png};

use px_ui::color::{ColorChoice, initial_choice};
use px_ui::palette_grid::Scroll;

use crate::files::{FileError, MAX_FILE_BYTES, describe_open, describe_open_aux, load_png};

// ---------------------------------------------------------------------------------------
// Ayudantes: directorio temporal, corpus, imágenes y PNG RGB de 1×1 hecho a mano.
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
            std::env::temp_dir().join(format!("pixoko-px-app-session-{}-{n}", std::process::id()));
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

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/png")
}

/// Los PNG del corpus, ordenados por nombre.
fn corpus() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(corpus_dir())
        .expect("leer golden/png")
        .map(|entry| entry.expect("entrada").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
        .collect();
    files.sort();
    files
}

/// Copia un fichero del corpus a `dir` y devuelve la ruta de la copia.
fn copy_to(dir: &Path, source: &Path) -> PathBuf {
    let target = dir.join(source.file_name().expect("nombre"));
    fs::copy(source, &target).expect("copiar el fichero del corpus");
    target
}

/// Imagen pequeña y variada en el modo pedido.
fn sample_image(mode: Mode) -> Image {
    let (w, h) = (5u32, 4u32);
    let mut image = match mode {
        Mode::Indexed => {
            let palette = Palette::from_colors([
                Rgba::opaque(0, 0, 0),
                Rgba::opaque(255, 0, 0),
                Rgba::opaque(0, 255, 0),
                Rgba::new(0, 0, 255, 128),
            ])
            .expect("paleta");
            Image::new_indexed(w, h, palette).expect("imagen indexada")
        }
        other => Image::new(w, h, other).expect("imagen"),
    };
    for y in 0..h {
        for x in 0..w {
            let n = (y * w + x) as u8;
            let pixel = match mode {
                Mode::Rgba => {
                    Pixel::Rgba(Rgba::new(n.wrapping_mul(13), n.wrapping_mul(7), n, 255 - n))
                }
                Mode::Indexed => Pixel::Indexed(n % 4),
                Mode::Gray => Pixel::Gray(n.wrapping_mul(11)),
                Mode::GrayAlpha => Pixel::GrayAlpha {
                    gray: n.wrapping_mul(11),
                    alpha: 255 - n,
                },
            };
            image.set_pixel(x, y, pixel).expect("píxel");
        }
    }
    image
}

const MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Rgba => "RGBA",
        Mode::Indexed => "indexado",
        Mode::Gray => "gris",
        Mode::GrayAlpha => "gris con alfa",
    }
}

/// Un píxel del mismo modo y distinto del que hay en `(x, y)`.
fn other_pixel(image: &Image, x: u32, y: u32) -> Pixel {
    match image.get_pixel(x, y).expect("píxel") {
        Pixel::Rgba(c) => Pixel::Rgba(Rgba::new(!c.r, c.g, c.b, c.a)),
        Pixel::Indexed(i) => {
            let len = image.palette().expect("paleta").len();
            Pixel::Indexed(((usize::from(i) + 1) % len) as u8)
        }
        Pixel::Gray(g) => Pixel::Gray(g.wrapping_add(1)),
        Pixel::GrayAlpha { gray, alpha } => Pixel::GrayAlpha {
            gray: gray.wrapping_add(1),
            alpha,
        },
    }
}

/// Cambia el píxel `(x, y)` del documento por otro distinto; `true` si hubo cambio.
fn edit_one(document: &mut Document, x: u32, y: u32) -> bool {
    let id = ImageId::from_raw(0);
    let pixel = other_pixel(document.image(id).expect("imagen"), x, y);
    document
        .apply(Command::SetPixels {
            image: id,
            pixels: vec![PixelEdit { x, y, pixel }],
        })
        .expect("apply")
}

fn main_image(document: &Document) -> &Image {
    document
        .image(ImageId::from_raw(0))
        .expect("imagen principal")
}

/// Sesión de muestra con una imagen cualquiera (la que se reemplaza al abrir).
fn fresh_session() -> Session {
    Session::sample(Document::new(sample_image(Mode::Indexed)))
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

/// Flujo zlib con un único bloque deflate «stored» (datos pequeños, menos de 65535 bytes).
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

/// PNG RGB (tipo 2, 8 bits) de 1×1 con ese píxel.
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

// ---------------------------------------------------------------------------------------
// Sesión de muestra.
// ---------------------------------------------------------------------------------------

#[test]
fn the_sample_session_has_no_path_or_changes() {
    let session = fresh_session();
    assert!(session.path().is_none());
    assert!(!session.modified());
    assert!(!session.document.can_undo());
    assert!(!session.document.can_redo());
    assert_eq!(session.document.state_id(), 0);
}

// ---------------------------------------------------------------------------------------
// Fila 2: abrir cada fichero del corpus.
// ---------------------------------------------------------------------------------------

#[test]
fn row_2_open_the_corpus() {
    let files = corpus();
    assert!(files.len() >= 4, "el corpus tiene cuatro ficheros");
    for path in files {
        let bytes = fs::read(&path).unwrap();
        let expected = read_png(&bytes).expect("el corpus es válido");
        let name = path.file_name().unwrap().to_string_lossy().into_owned();

        let mut session = fresh_session();
        let text = session
            .open(&path, 16384, false)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));

        assert_eq!(main_image(&session.document), &expected.image, "{name}");
        let opened = session.path().expect("ruta tras abrir");
        assert!(opened.is_absolute(), "{name}: {opened:?} no es absoluta");
        assert_eq!(opened.file_name(), path.file_name());
        assert!(!session.modified(), "{name}");
        assert!(!session.document.can_undo(), "{name}");
        assert!(!session.document.can_redo(), "{name}");
        assert_eq!(session.document.state_id(), 0, "{name}");

        let image = &expected.image;
        assert!(text.contains(&name), "{name}: {text}");
        assert!(
            text.contains(&format!("{}×{}", image.width(), image.height())),
            "{name}: {text}"
        );
        assert!(text.contains(mode_label(image.mode())), "{name}: {text}");
        assert_eq!(
            text,
            describe_open_aux(&name, image, &expected.report, &expected.aux),
            "{name}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 6: cambios sin guardar y `discard`.
// ---------------------------------------------------------------------------------------

#[test]
fn row_6_with_changes_without_discard_the_file_is_not_read() {
    let tmp = TmpDir::new();
    let mut session = fresh_session();
    assert!(edit_one(&mut session.document, 0, 0));
    let before = session.document.clone();
    let state = session.document.state_id();

    let result = session.open(&tmp.path().join("no-existe.png"), 16384, false);
    assert_eq!(result, Err(FileError::Unsaved), "no es Read: no se lee");

    assert_eq!(session.document, before);
    assert_eq!(session.document.state_id(), state);
    assert!(session.document.can_undo());
    assert!(session.modified());
    assert!(session.path().is_none());
}

#[test]
fn row_6_with_discard_it_opens_and_discards() {
    let mut session = fresh_session();
    assert!(edit_one(&mut session.document, 0, 0));
    let path = corpus().remove(0);
    let expected = read_png(&fs::read(&path).unwrap()).unwrap().image;

    session.open(&path, 16384, true).expect("abrir con discard");

    assert_eq!(main_image(&session.document), &expected);
    assert!(!session.document.can_undo(), "historia vacía");
    assert!(!session.document.can_redo());
    assert_eq!(session.path().and_then(Path::file_name), path.file_name());
    assert!(!session.modified());
    assert_eq!(session.document.state_id(), 0);
}

#[test]
fn row_6_an_error_on_opening_leaves_the_session_intact() {
    let tmp = TmpDir::new();
    let mut session = fresh_session();
    assert!(edit_one(&mut session.document, 1, 1));
    let before = session.document.clone();
    let result = session.open(&tmp.path().join("no-existe.png"), 16384, true);
    assert!(matches!(result, Err(FileError::Read(_))), "{result:?}");
    assert_eq!(session.document, before);
    assert!(session.modified());
    assert!(session.document.can_undo());
    assert!(session.path().is_none());
}

// ---------------------------------------------------------------------------------------
// Fila 7: tras guardar se puede abrir otro sin `discard`.
// ---------------------------------------------------------------------------------------

#[test]
fn row_7_save_and_open_another() {
    let tmp = TmpDir::new();
    let files = corpus();
    let first = copy_to(tmp.path(), &files[0]);
    let second = files[1].clone();
    let expected = read_png(&fs::read(&second).unwrap()).unwrap().image;

    let mut session = fresh_session();
    session
        .open(&first, 16384, false)
        .expect("abrir el primero");
    assert!(edit_one(&mut session.document, 0, 0));
    assert!(session.modified());
    session.save(tmp.path()).expect("save");
    assert!(!session.modified());

    session
        .open(&second, 16384, false)
        .expect("sin cambios, abrir sin discard funciona");
    assert_eq!(main_image(&session.document), &expected);
    assert!(!session.document.can_undo(), "historia vacía");
    assert_eq!(session.path().and_then(Path::file_name), second.file_name());
    assert!(!session.modified());
}

// ---------------------------------------------------------------------------------------
// Fila 8 (specs/hito-2-panel-paletas.md): abrir reinicia la selección y el desplazamiento.
// ---------------------------------------------------------------------------------------

/// Indexada de 3 colores, de lado `side` (para distinguir ficheros iguales en modo y paleta).
fn indexed_of_3(side: u32) -> Image {
    let palette = Palette::from_colors([
        Rgba::opaque(0, 0, 0),
        Rgba::opaque(255, 0, 0),
        Rgba::opaque(0, 255, 0),
    ])
    .expect("paleta de 3");
    Image::new_indexed(side, side, palette).expect("imagen indexada de 3")
}

/// Indexada de 256 colores distintos.
fn indexed_of_256() -> Image {
    let palette = Palette::from_colors((0..=255u8).map(|i| Rgba::opaque(i, 255 - i, i / 2)))
        .expect("paleta de 256");
    Image::new_indexed(6, 6, palette).expect("imagen indexada de 256")
}

/// Escribe `image` como PNG en `dir/name` y devuelve la ruta.
fn write_image(dir: &Path, name: &str, image: &Image) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, write_png(image).expect("write_png")).expect("escribir el PNG");
    path
}

/// Un desplazamiento que no es el de partida (ni fila 0 ni resto 0).
fn scrolled() -> Scroll {
    Scroll {
        first_row: 7,
        rest: 3,
    }
}

#[test]
fn row_8_open_resets_the_selection_and_the_scroll() {
    let tmp = TmpDir::new();
    // (imagen de la sesión, selección que se deja, fichero que se abre).
    let cases: Vec<(&str, Image, ColorChoice, Image)> = vec![
        (
            "indexada de 256 con Entry(200) -> RGBA",
            indexed_of_256(),
            ColorChoice::Entry(200),
            sample_image(Mode::Rgba),
        ),
        (
            "RGBA con Slot(9) -> indexada de 3",
            sample_image(Mode::Rgba),
            ColorChoice::Slot(9),
            indexed_of_3(4),
        ),
        (
            "indexada con Entry(2) -> otra indexada de 3 (mismo modo y tamaño)",
            indexed_of_3(3),
            ColorChoice::Entry(2),
            indexed_of_3(3),
        ),
        (
            "Gray",
            indexed_of_256(),
            ColorChoice::Entry(255),
            sample_image(Mode::Gray),
        ),
        (
            "GrayAlpha",
            sample_image(Mode::Rgba),
            ColorChoice::Slot(5),
            sample_image(Mode::GrayAlpha),
        ),
    ];
    for (index, (label, start, choice, opened)) in cases.into_iter().enumerate() {
        let path = write_image(tmp.path(), &format!("abrir-{index}.png"), &opened);
        let expected = read_png(&fs::read(&path).unwrap())
            .expect("PNG válido")
            .image;

        let mut session = Session::sample(Document::new(start));
        session.choice = choice;
        session.scroll = scrolled();
        session
            .open(&path, 16384, false)
            .unwrap_or_else(|e| panic!("{label}: {e:?}"));

        assert_eq!(main_image(&session.document), &expected, "{label}");
        assert_eq!(
            session.choice,
            initial_choice(&expected),
            "{label}: la selección vuelve a la inicial"
        );
        assert_eq!(
            session.scroll,
            Scroll::default(),
            "{label}: el desplazamiento vuelve a 0"
        );
    }
}

#[test]
fn row_8_the_initial_selection_depends_on_the_mode_of_the_new_image() {
    let tmp = TmpDir::new();
    for (mode, expected) in [
        (Mode::Rgba, ColorChoice::Slot(1)),
        (Mode::Indexed, ColorChoice::Entry(0)),
        (Mode::Gray, ColorChoice::Slot(1)),
        (Mode::GrayAlpha, ColorChoice::Slot(1)),
    ] {
        let path = write_image(
            tmp.path(),
            &format!("modo-{}.png", mode_label(mode).replace(' ', "-")),
            &sample_image(mode),
        );
        let mut session = fresh_session();
        session.choice = ColorChoice::Entry(3);
        session.scroll = scrolled();
        session.open(&path, 16384, false).expect("abrir");
        assert_eq!(session.choice, expected, "{mode:?}");
        assert_eq!(session.scroll, Scroll::default(), "{mode:?}");
    }
}

#[test]
fn row_8_the_sample_session_starts_with_the_initial_selection() {
    for mode in MODES {
        let image = sample_image(mode);
        let expected = initial_choice(&image);
        let session = Session::sample(Document::new(image));
        assert_eq!(session.choice, expected, "{mode:?}");
        assert_eq!(session.scroll, Scroll::default(), "{mode:?}");
    }
}

#[test]
fn row_8_an_open_with_an_error_changes_neither_the_selection_nor_the_scroll() {
    let tmp = TmpDir::new();

    // Fichero inexistente, con y sin `discard`.
    for discard in [false, true] {
        let mut session = Session::sample(Document::new(indexed_of_256()));
        session.choice = ColorChoice::Entry(200);
        session.scroll = scrolled();
        let result = session.open(&tmp.path().join("no-existe.png"), 16384, discard);
        assert!(matches!(result, Err(FileError::Read(_))), "{result:?}");
        assert_eq!(session.choice, ColorChoice::Entry(200), "discard {discard}");
        assert_eq!(session.scroll, scrolled(), "discard {discard}");
    }

    // Cambios sin guardar y sin `discard`: ni siquiera se lee el fichero (que existe y es
    // válido: el error es `Unsaved`).
    let valid = write_image(tmp.path(), "valido.png", &indexed_of_3(4));
    let mut session = Session::sample(Document::new(indexed_of_256()));
    assert!(edit_one(&mut session.document, 0, 0));
    session.choice = ColorChoice::Entry(200);
    session.scroll = scrolled();
    let result = session.open(&valid, 16384, false);
    assert_eq!(result, Err(FileError::Unsaved));
    assert_eq!(session.choice, ColorChoice::Entry(200));
    assert_eq!(session.scroll, scrolled());

    // Un fichero que no es PNG, con `discard`: error de lectura y todo igual.
    let garbage = tmp.path().join("basura.png");
    fs::write(&garbage, b"esto no es un PNG").unwrap();
    let result = session.open(&garbage, 16384, true);
    assert!(result.is_err(), "{result:?}");
    assert_eq!(session.choice, ColorChoice::Entry(200));
    assert_eq!(session.scroll, scrolled());

    // Y con `discard`, un fichero válido sí reinicia (contraste: el banco recoge el cambio).
    session
        .open(&valid, 16384, true)
        .expect("abrir con discard");
    assert_eq!(session.choice, ColorChoice::Entry(0));
    assert_eq!(session.scroll, Scroll::default());
}

// ---------------------------------------------------------------------------------------
// Fila 13 (specs/hito-2-panel-paletas.md): guardar y guardar como no reinician.
// ---------------------------------------------------------------------------------------

#[test]
fn row_13_save_does_not_reset_the_selection() {
    let tmp = TmpDir::new();
    let path = write_image(tmp.path(), "paleta.png", &indexed_of_256());

    let mut session = fresh_session();
    session.open(&path, 16384, false).expect("abrir");
    session.choice = ColorChoice::Entry(200);
    session.scroll = scrolled();
    let keep = |session: &Session, step: &str| {
        assert_eq!(session.choice, ColorChoice::Entry(200), "{step}: choice");
        assert_eq!(session.scroll, scrolled(), "{step}: scroll");
    };

    // Una edición del documento tampoco los toca.
    assert!(edit_one(&mut session.document, 0, 0));
    keep(&session, "apply");

    session.save(tmp.path()).expect("save");
    assert!(!session.modified());
    keep(&session, "save");

    assert!(edit_one(&mut session.document, 1, 1));
    session.save_as(tmp.path()).expect("save_as");
    keep(&session, "save_as");

    // Una sesión de muestra sin ruta: `save` escribe `without-title.png`.
    let mut sample = Session::sample(Document::new(sample_image(Mode::Indexed)));
    sample.choice = ColorChoice::Entry(2);
    sample.scroll = scrolled();
    sample.save(tmp.path()).expect("save sin ruta");
    assert_eq!(sample.choice, ColorChoice::Entry(2), "save sin ruta");
    assert_eq!(sample.scroll, scrolled(), "save sin ruta");
}

#[test]
fn row_13_if_saving_fails_they_do_not_change_either() {
    let tmp = TmpDir::new();
    let missing = tmp.path().join("no-existe").join("tampoco");

    let mut session = Session::sample(Document::new(sample_image(Mode::Indexed)));
    session.choice = ColorChoice::Entry(3);
    session.scroll = scrolled();
    assert!(edit_one(&mut session.document, 0, 0));

    let result = session.save(&missing);
    assert!(
        result.is_err(),
        "guardar en un directorio inexistente: {result:?}"
    );
    assert_eq!(session.choice, ColorChoice::Entry(3), "save fallido");
    assert_eq!(session.scroll, scrolled(), "save fallido");
    assert!(session.modified(), "el fallo no marca como guardado");

    let result = session.save_as(&missing);
    assert!(
        result.is_err(),
        "guardar como en un directorio inexistente: {result:?}"
    );
    assert_eq!(session.choice, ColorChoice::Entry(3), "save_as fallido");
    assert_eq!(session.scroll, scrolled(), "save_as fallido");
}

// ---------------------------------------------------------------------------------------
// Fila 9: describe_open (la función vive en `files`).
// ---------------------------------------------------------------------------------------

#[test]
fn row_9_empty_report_and_modes() {
    let cases = [
        (Mode::Rgba, "RGBA"),
        (Mode::Gray, "gris"),
        (Mode::GrayAlpha, "gris con alfa"),
        (Mode::Indexed, "indexado"),
    ];
    for (mode, label) in cases {
        let image = match mode {
            Mode::Indexed => Image::new_indexed(
                16,
                16,
                Palette::from_colors([Rgba::opaque(0, 0, 0)]).unwrap(),
            )
            .unwrap(),
            other => Image::new(16, 16, other).unwrap(),
        };
        assert_eq!(
            describe_open("casa.png", &image, &[]),
            format!("abierto casa.png · 16×16 · {label}")
        );
    }
}

#[test]
fn row_9_with_a_report_it_warns_about_the_copy() {
    let image = Image::new(16, 16, Mode::Rgba).unwrap();
    assert_eq!(
        describe_open(
            "casa.png",
            &image,
            &[
                Conversion::DepthExpanded { from_bits: 4 },
                Conversion::ColorKeyToAlpha
            ]
        ),
        "abierto casa.png · 16×16 · RGBA · convertido: gris de 4 bits ampliado a 8, \
         color clave convertido en alfa · Ctrl+S guardará una copia"
    );
    assert_eq!(
        describe_open("casa.png", &image, &[Conversion::Depth16Reduced]),
        "abierto casa.png · 16×16 · RGBA · convertido: 16 bits reducido a 8 · Ctrl+S guardará una copia"
    );
    assert_eq!(
        describe_open("casa.png", &image, &[Conversion::RgbToRgba]),
        "abierto casa.png · 16×16 · RGBA · convertido: RGB convertido a RGBA · Ctrl+S guardará una copia"
    );
}

// ---------------------------------------------------------------------------------------
// Fila 10: el indicador `modified` con `state_id`.
// ---------------------------------------------------------------------------------------

#[test]
fn row_10_modified_indicator() {
    let tmp = TmpDir::new();
    let mut session = Session::sample(Document::new(sample_image(Mode::Rgba)));
    assert!(!session.modified());

    assert!(edit_one(&mut session.document, 0, 0));
    assert!(session.modified(), "apply");

    session.save(tmp.path()).expect("save");
    assert!(!session.modified(), "save");

    assert!(edit_one(&mut session.document, 1, 0));
    session.document.undo().unwrap();
    assert!(!session.modified(), "apply, undo (vuelve al guardado)");

    session.document.redo().unwrap();
    assert!(session.modified(), "redo");
}

#[test]
fn row_10_the_saved_state_stops_being_reachable() {
    let tmp = TmpDir::new();
    let mut session = Session::sample(Document::new(sample_image(Mode::Rgba)));
    assert!(edit_one(&mut session.document, 0, 0));
    session.save(tmp.path()).expect("save");
    assert!(!session.modified());

    session.document.undo().unwrap();
    assert!(session.modified(), "undo");
    assert!(edit_one(&mut session.document, 2, 2));
    assert!(session.modified(), "apply nuevo");
    session.document.undo().unwrap();
    assert!(session.modified(), "undo: el guardado ya no se alcanza");
    session.document.redo().unwrap();
    assert!(session.modified(), "redo: sigue sin alcanzarse");
}

// ---------------------------------------------------------------------------------------
// Fila 11: ida y vuelta en los cuatro modos.
// ---------------------------------------------------------------------------------------

/// Abre `source`, edita 3 píxeles, guarda, abre en otra sesión y comprueba la imagen; después
/// `undo` + `save` deja la imagen original exacta.
fn check_round_trip(tmp: &TmpDir, source: &Path) {
    let label = source.file_name().unwrap().to_string_lossy().into_owned();
    let original = read_png(&fs::read(source).unwrap())
        .expect("PNG válido")
        .image;

    let mut session = fresh_session();
    session.open(source, 16384, false).expect("abrir");
    let id = ImageId::from_raw(0);
    let pixels: Vec<PixelEdit> = [(0, 0), (1, 0), (2, 1)]
        .into_iter()
        .map(|(x, y)| PixelEdit {
            x,
            y,
            pixel: other_pixel(&original, x, y),
        })
        .collect();
    assert!(
        session
            .document
            .apply(Command::SetPixels { image: id, pixels })
            .unwrap(),
        "{label}: la edición cambia la imagen"
    );
    let edited = main_image(&session.document).clone();

    session.save(tmp.path()).expect("save");
    assert!(!session.modified(), "{label}");
    assert_no_temporaries(tmp.path());

    // Si el original se convierte al abrir, `save` escribe la copia: se abre la ruta de la sesión.
    let saved_path = session.path().expect("ruta tras guardar").to_path_buf();
    let mut other = fresh_session();
    other
        .open(&saved_path, 16384, false)
        .expect("abrir lo guardado");
    assert_eq!(
        main_image(&other.document),
        &edited,
        "{label}: imagen editada"
    );

    session.document.undo().unwrap();
    assert!(session.modified(), "{label}: tras undo");
    session.save(tmp.path()).expect("save tras undo");
    assert!(!session.modified(), "{label}");
    assert_no_temporaries(tmp.path());
    let mut third = fresh_session();
    third
        .open(session.path().unwrap(), 16384, false)
        .expect("abrir lo guardado tras undo");
    assert_eq!(
        main_image(&third.document),
        &original,
        "{label}: imagen original"
    );
}

#[test]
fn row_11_indexed_and_rgba_corpus() {
    let files = corpus();
    let indexed = files
        .iter()
        .find(|p| {
            p.to_string_lossy()
                .ends_with("libresprite-golden-indexed.png")
        })
        .expect("corpus indexado");
    let rgba = files
        .iter()
        .find(|p| p.to_string_lossy().ends_with("libresprite-golden-rgba.png"))
        .expect("corpus RGBA");
    for source in [indexed, rgba] {
        let dir = TmpDir::new();
        let copy = copy_to(dir.path(), source);
        check_round_trip(&dir, &copy);
    }
}

#[test]
fn row_11_synthetic_gray_and_gray_alpha() {
    for mode in [Mode::Gray, Mode::GrayAlpha] {
        let dir = TmpDir::new();
        let path = dir.path().join("origen.png");
        fs::write(&path, write_png(&sample_image(mode)).unwrap()).unwrap();
        check_round_trip(&dir, &path);
    }
}

// ---------------------------------------------------------------------------------------
// Fila 16: nombres de guardado.
// ---------------------------------------------------------------------------------------

#[test]
fn row_16_without_a_path_it_never_overwrites() {
    let tmp = TmpDir::new();
    let first_image = sample_image(Mode::Rgba);
    let mut first = Session::sample(Document::new(first_image.clone()));
    let saved = first.save(tmp.path()).expect("primer save");
    assert_eq!(saved.path.file_name().unwrap(), "sin-titulo.png");
    assert_eq!(saved.copy_reason, None);
    assert_eq!(first.path(), Some(saved.path.as_path()));
    assert!(!first.modified());

    let mut second = Session::sample(Document::new(sample_image(Mode::Gray)));
    let saved_2 = second.save(tmp.path()).expect("segundo save");
    assert_eq!(saved_2.path.file_name().unwrap(), "sin-titulo-2.png");
    assert_eq!(saved_2.copy_reason, None);

    // El primero sigue con su imagen.
    let stored = read_png(&fs::read(&saved.path).unwrap()).unwrap().image;
    assert_eq!(stored, first_image);
    assert_eq!(names(tmp.path()), ["sin-titulo-2.png", "sin-titulo.png"]);
}

#[test]
fn row_16_save_as_in_two_sessions_that_open_the_same_file() {
    let tmp = TmpDir::new();
    let x = tmp.path().join("x.png");
    fs::write(&x, write_png(&sample_image(Mode::Indexed)).unwrap()).unwrap();
    let original_bytes = fs::read(&x).unwrap();

    let mut a = fresh_session();
    a.open(&x, 16384, false)
        .expect("abrir x.png en la sesión A");
    let mut b = fresh_session();
    b.open(&x, 16384, false)
        .expect("abrir x.png en la sesión B");

    let saved_a = a.save_as(tmp.path()).expect("save_as de A");
    assert_eq!(saved_a.path.file_name().unwrap(), "x-copia.png");
    assert_eq!(saved_a.copy_reason, None);
    let saved_b = b.save_as(tmp.path()).expect("save_as de B");
    assert_eq!(saved_b.path.file_name().unwrap(), "x-copia-2.png");
    assert_eq!(saved_b.copy_reason, None);

    assert_eq!(fs::read(&x).unwrap(), original_bytes, "x.png queda igual");
    assert_eq!(
        b.path(),
        Some(saved_b.path.as_path()),
        "la ruta es la última copia"
    );
    assert_eq!(a.path(), Some(saved_a.path.as_path()));
    assert!(!a.modified());
    assert!(!b.modified());
    assert_eq!(names(tmp.path()), ["x-copia-2.png", "x-copia.png", "x.png"]);
}

#[test]
fn row_16_repeated_save_as_in_the_same_session_copies_the_copy() {
    // Según la corrección de la spec: la ruta pasa a ser la copia, así que un segundo
    // `save_as` en la misma sesión parte de ella (`x-copy-copy.png`) y no pisa nada.
    let tmp = TmpDir::new();
    let x = tmp.path().join("x.png");
    fs::write(&x, write_png(&sample_image(Mode::Gray)).unwrap()).unwrap();
    let mut session = fresh_session();
    session.open(&x, 16384, false).unwrap();
    let first = session.save_as(tmp.path()).unwrap();
    assert_eq!(first.path.file_name().unwrap(), "x-copia.png");
    let second = session.save_as(tmp.path()).unwrap();
    assert_eq!(second.path.file_name().unwrap(), "x-copia-copia.png");
    assert_eq!(session.path(), Some(second.path.as_path()));
}

#[test]
fn row_16_save_as_without_a_path_writes_untitled() {
    let tmp = TmpDir::new();
    let mut session = Session::sample(Document::new(sample_image(Mode::GrayAlpha)));
    let saved = session.save_as(tmp.path()).expect("save_as sin ruta");
    assert_eq!(saved.path.file_name().unwrap(), "sin-titulo.png");
    assert_eq!(saved.copy_reason, None);
    assert_eq!(session.path(), Some(saved.path.as_path()));
    assert!(!session.modified());
    // Con ruta, `save_as` ya no usa `without-title`: copia la del documento y no pisa nada.
    let other = session.save_as(tmp.path()).expect("segundo save_as");
    assert_ne!(other.path, saved.path);
    assert!(saved.path.exists() && other.path.exists());
}

// ---------------------------------------------------------------------------------------
// Fila 17: un PNG que se convierte no se sobrescribe.
// ---------------------------------------------------------------------------------------

#[test]
fn row_17_rgb_is_saved_as_a_copy_and_then_overwritten() {
    let tmp = TmpDir::new();
    let x = tmp.path().join("x.png");
    let original_bytes = rgb_1x1(10, 20, 30);
    fs::write(&x, &original_bytes).unwrap();

    let mut session = fresh_session();
    let text = session.open(&x, 16384, false).expect("abrir el RGB de 1×1");
    assert!(text.contains("RGB convertido a RGBA"), "{text}");
    assert!(text.contains("Ctrl+S guardará una copia"), "{text}");
    assert_eq!(
        main_image(&session.document).get_pixel(0, 0).unwrap(),
        Pixel::Rgba(Rgba::new(10, 20, 30, 255))
    );

    // Primer save: la copia, con la razón.
    let first = session.save(tmp.path()).expect("primer save");
    assert_eq!(first.path.file_name().unwrap(), "x-copia.png");
    assert_eq!(first.copy_reason.as_deref(), Some("RGB convertido a RGBA"));
    assert_eq!(
        fs::read(&x).unwrap(),
        original_bytes,
        "el original no cambia"
    );
    assert_eq!(
        session.path(),
        Some(first.path.as_path()),
        "la ruta es la copia"
    );
    assert!(!session.modified());
    let described = describe_saved(&first);
    assert!(described.contains("x-copia.png"), "{described}");
    assert!(described.contains("RGB convertido a RGBA"), "{described}");
    assert!(described.contains("no se sobrescribe"), "{described}");

    // Segundo save: sobrescribe la copia y ya no hay razón.
    assert!(edit_one(&mut session.document, 0, 0));
    let edited = main_image(&session.document).clone();
    let second = session.save(tmp.path()).expect("segundo save");
    assert_eq!(second.path, first.path);
    assert_eq!(second.copy_reason, None);
    let stored = read_png(&fs::read(&second.path).unwrap()).unwrap();
    assert_eq!(stored.image, edited, "la copia lleva la imagen nueva");
    assert!(stored.report.is_empty());
    assert_eq!(
        fs::read(&x).unwrap(),
        original_bytes,
        "el original sigue igual"
    );
    assert_eq!(names(tmp.path()), ["x-copia.png", "x.png"]);
    assert_eq!(describe_saved(&second), "guardado x-copia.png");
}

// ---------------------------------------------------------------------------------------
// Propiedad 1: ida y vuelta por disco.
// ---------------------------------------------------------------------------------------

/// Imagen de 1 a 16 de lado en uno de los cuatro modos, con paletas de 1 a 256 colores.
fn any_image() -> impl Strategy<Value = Image> {
    (
        0usize..4,
        1u32..=16,
        1u32..=16,
        prop::collection::vec(any::<(u8, u8, u8, u8)>(), 1..=256),
        prop::collection::vec(any::<u32>(), 256),
    )
        .prop_map(|(mode, width, height, colors, seeds)| {
            let mut image = match MODES[mode] {
                Mode::Indexed => {
                    let palette = Palette::from_colors(
                        colors.iter().map(|&(r, g, b, a)| Rgba::new(r, g, b, a)),
                    )
                    .expect("paleta de 1 a 256 colores");
                    Image::new_indexed(width, height, palette).expect("imagen indexada")
                }
                other => Image::new(width, height, other).expect("imagen"),
            };
            for y in 0..height {
                for x in 0..width {
                    let seed = seeds[(y * 16 + x) as usize];
                    let [b0, b1, b2, b3] = seed.to_le_bytes();
                    let pixel = match image.mode() {
                        Mode::Rgba => Pixel::Rgba(Rgba::new(b0, b1, b2, b3)),
                        Mode::Indexed => Pixel::Indexed((seed as usize % colors.len()) as u8),
                        Mode::Gray => Pixel::Gray(b0),
                        Mode::GrayAlpha => Pixel::GrayAlpha {
                            gray: b0,
                            alpha: b1,
                        },
                    };
                    image.set_pixel(x, y, pixel).expect("píxel");
                }
            }
            image
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn property_1_round_trip_through_disk(image in any_image()) {
        let tmp = TmpDir::new();
        let mut session = Session::sample(Document::new(image.clone()));
        let saved = session.save(tmp.path()).expect("save");
        prop_assert_eq!(saved.copy_reason, None);

        let loaded = load_png(&saved.path, MAX_FILE_BYTES, 16384).expect("load_png");
        prop_assert!(loaded.report.is_empty(), "informe: {:?}", loaded.report);
        prop_assert_eq!(&loaded.image, &image);

        let mut other = fresh_session();
        other.open(&saved.path, 16384, false).expect("abrir en otra sesión");
        prop_assert_eq!(main_image(&other.document), &image);
        let listed = names(tmp.path());
        prop_assert!(listed.iter().all(|n| !n.contains("pixoko-tmp")), "{:?}", listed);
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 2: el indicador contra un modelo.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Op {
    Edit(u32, u32, u8),
    Undo,
    Redo,
    Save,
}

fn any_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (0u32..4, 0u32..4, 0u8..3).prop_map(|(x, y, v)| Op::Edit(x, y, v)),
        1 => Just(Op::Undo),
        1 => Just(Op::Redo),
        1 => Just(Op::Save),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn property_2_the_indicator_has_no_false_negatives(
        ops in prop::collection::vec(any_op(), 0..30)
    ) {
        let tmp = TmpDir::new();
        let initial = Image::new(4, 4, Mode::Gray).unwrap();
        let mut session = Session::sample(Document::new(initial.clone()));
        // Copia de la imagen guardada (la de partida cuenta como guardada).
        let mut saved = initial;

        for op in ops {
            match op {
                Op::Edit(x, y, v) => {
                    let id = ImageId::from_raw(0);
                    let changed = session
                        .document
                        .apply(Command::SetPixels {
                            image: id,
                            pixels: vec![PixelEdit { x, y, pixel: Pixel::Gray(v) }],
                        })
                        .unwrap();
                    if changed {
                        prop_assert!(session.modified(), "tras una edición con cambio");
                    }
                }
                Op::Undo => {
                    let _ = session.document.undo();
                }
                Op::Redo => {
                    let _ = session.document.redo();
                }
                Op::Save => {
                    session.save(tmp.path()).expect("save");
                    saved = main_image(&session.document).clone();
                    prop_assert!(!session.modified(), "tras save");
                }
            }
            if !session.modified() {
                prop_assert_eq!(
                    main_image(&session.document),
                    &saved,
                    "modified falso pero la imagen difiere de la guardada"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 5: abrir no entra en pánico, y un error deja la sesión igual.
// ---------------------------------------------------------------------------------------

/// PNG válidos para mutar: el corpus y los cuatro modos sintéticos.
fn valid_pngs() -> Vec<Vec<u8>> {
    let mut all: Vec<Vec<u8>> = corpus()
        .iter()
        .map(|path| fs::read(path).expect("leer el corpus"))
        .collect();
    for mode in MODES {
        all.push(write_png(&sample_image(mode)).expect("write_png"));
    }
    all.push(rgb_1x1(1, 2, 3));
    all
}

/// Bytes arbitrarios, o un PNG válido con bytes cambiados y, a veces, truncado.
fn input_bytes() -> impl Strategy<Value = Vec<u8>> {
    let valid = valid_pngs();
    let count = valid.len();
    prop_oneof![
        prop::collection::vec(any::<u8>(), 0..300),
        (
            0..count,
            prop::collection::vec((any::<usize>(), any::<u8>()), 0..8),
            any::<Option<usize>>(),
        )
            .prop_map(move |(which, mutations, cut)| {
                let mut bytes = valid[which].clone();
                for (at, value) in mutations {
                    let len = bytes.len();
                    bytes[at % len] = value;
                }
                if let Some(cut) = cut {
                    let len = bytes.len();
                    bytes.truncate(cut % (len + 1));
                }
                bytes
            }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn property_5_opening_does_not_panic(bytes in input_bytes()) {
        let tmp = TmpDir::new();
        let path = tmp.path().join("entrada.png");
        fs::write(&path, &bytes).unwrap();

        // `load_png`: Ok o Err, sin pánico (el lado máximo pequeño rechaza cabeceras enormes).
        let _ = load_png(&path, MAX_FILE_BYTES, 64);

        // La sesión tiene cambios y se abre con `discard`: si falla, queda igual.
        let mut session = fresh_session();
        prop_assert!(edit_one(&mut session.document, 1, 1));
        let before = session.document.clone();
        let state = session.document.state_id();
        let result = session.open(&path, 64, true);
        if result.is_err() {
            prop_assert_eq!(&session.document, &before);
            prop_assert_eq!(session.document.state_id(), state);
            prop_assert!(session.document.can_undo());
            prop_assert!(session.modified());
            prop_assert!(session.path().is_none());
        }
    }
}
