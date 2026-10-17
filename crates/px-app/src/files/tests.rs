//! Tests de `files` (specs/hito-2-abrir-guardar.md): filas 1, 3, 4, 5, 12 a 15 y la parte
//! `file_label` de la 21; propiedades 4 (`write_atomic`), 6 (`check_fits`) y 7.
//!
//! Cada test usa su propio directorio temporal (`TmpDir`, con el pid y un contador, borrado al
//! terminar) y no depende del directorio actual. Las filas 14 y 15 usan `Session` (la parte de
//! `modified` y del enlace pasa por `save`) y se saltan donde no apliquen: `root` y no Unix.

use super::*;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Command, Document, Image, Mode, Palette, Pixel, PixelEdit, Rgba};
use px_formats::read_png;
#[cfg(unix)]
use px_formats::write_png;

#[cfg(unix)]
use crate::session::Session;

// ---------------------------------------------------------------------------------------
// Ayudantes: directorio temporal, corpus, imágenes y PNG de solo cabecera.
// ---------------------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Directorio temporal propio del test; el guardián lo borra al soltarse (también si el test
/// falla), restaurando antes los permisos de los directorios que el test hubiera cerrado.
struct TmpDir {
    path: PathBuf,
}

impl TmpDir {
    fn new() -> TmpDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("pixoko-px-app-files-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).expect("crear el directorio temporal");
        TmpDir { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        make_writable(&self.path);
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Deja los directorios del árbol con permisos de escritura para poder borrarlos.
fn make_writable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::symlink_metadata(path)
            && meta.is_dir()
        {
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    make_writable(&entry.path());
                }
            }
        }
    }
    #[cfg(not(unix))]
    let _ = path;
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

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes).expect("escribir el fichero de prueba");
    path
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("cambiar permisos");
}

#[cfg(unix)]
fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).expect("metadatos").permissions().mode() & 0o777
}

/// `true` si el sistema respeta los permisos (no somos `root`): se intenta escribir en un
/// fichero de solo lectura y en un directorio sin permiso de escritura, y se mira si falla.
#[cfg(unix)]
fn permissions_enforced() -> bool {
    static ENFORCED: OnceLock<bool> = OnceLock::new();
    *ENFORCED.get_or_init(|| {
        let probe = TmpDir::new();
        let file = write_file(probe.path(), "sonda.txt", b"x");
        set_mode(&file, 0o444);
        let file_blocked = fs::OpenOptions::new().write(true).open(&file).is_err();
        let dir = probe.path().join("sonda-dir");
        fs::create_dir(&dir).expect("crear el directorio de la sonda");
        set_mode(&dir, 0o555);
        let dir_blocked = fs::File::create(dir.join("x")).is_err();
        file_blocked && dir_blocked
    })
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

/// Imagen pequeña y variada en el modo pedido.
#[cfg_attr(not(unix), allow(dead_code))]
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

/// Un píxel del mismo modo y distinto del que hay en `(x, y)`.
#[cfg_attr(not(unix), allow(dead_code))]
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
#[cfg_attr(not(unix), allow(dead_code))]
fn edit_one(document: &mut Document, x: u32, y: u32) -> bool {
    let id = document.main_image();
    let pixel = other_pixel(document.image(id).expect("imagen"), x, y);
    document
        .apply(Command::SetPixels {
            image: id,
            pixels: vec![PixelEdit { x, y, pixel }],
        })
        .expect("apply")
}

#[cfg_attr(not(unix), allow(dead_code))]
fn main_image(document: &Document) -> &Image {
    document
        .image(document.main_image())
        .expect("imagen principal")
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

/// Solo firma + `IHDR` (RGBA de 8 bits) que declara `width` × `height`: sin datos de imagen.
fn header_only(width: u32, height: u32) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend(width.to_be_bytes());
    data.extend(height.to_be_bytes());
    data.extend([8, 6, 0, 0, 0]);
    let mut v = SIGNATURE.to_vec();
    v.extend(chunk(b"IHDR", &data));
    v
}

fn load(path: &Path) -> Result<px_formats::PngRead, FileError> {
    load_png(path, MAX_FILE_BYTES, 16384)
}

// ---------------------------------------------------------------------------------------
// Fila 1: argumentos y ficheros soltados.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_1_parse_args() {
    assert_eq!(parse_args(&[]), Ok(None));
    assert_eq!(
        parse_args(&[OsString::from("a.png")]),
        Ok(Some(PathBuf::from("a.png")))
    );
    let uso = parse_args(&[OsString::from("a"), OsString::from("b")]);
    match uso {
        Err(text) => assert!(!text.is_empty(), "el uso no puede ir vacío"),
        Ok(other) => panic!("con dos argumentos se esperaba el uso, no {other:?}"),
    }
}

#[test]
fn fila_1_pick_dropped() {
    let a = PathBuf::from("/d/a.png");
    let b = PathBuf::from("/d/b.png");
    assert_eq!(
        pick_dropped(&[None, Some(a.clone()), Some(b)]),
        Some((a.clone(), 1))
    );
    assert_eq!(pick_dropped(&[None]), None);
    // Casos derivados: sin nada, y uno solo (no se ignora ninguno).
    assert_eq!(pick_dropped(&[]), None);
    assert_eq!(pick_dropped(&[Some(a.clone())]), Some((a, 0)));
}

// ---------------------------------------------------------------------------------------
// Fila 3: load_png.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_3_ruta_inexistente_es_read() {
    let tmp = TmpDir::new();
    let result = load(&tmp.path().join("no-existe.png"));
    assert!(matches!(result, Err(FileError::Read(_))), "{result:?}");
}

#[test]
fn fila_3_directorio_es_not_a_file() {
    let tmp = TmpDir::new();
    let dir = tmp.path().join("carpeta.png");
    fs::create_dir(&dir).unwrap();
    assert_eq!(load(&dir).unwrap_err(), FileError::NotAFile);
}

#[cfg(unix)]
#[test]
fn fila_3_dispositivo_es_not_a_file() {
    assert_eq!(
        load(Path::new("/dev/null")).unwrap_err(),
        FileError::NotAFile
    );
}

#[test]
fn fila_3_vacio_basura_y_cabecera_sin_datos_son_png() {
    let tmp = TmpDir::new();
    let casos: [(&str, Vec<u8>); 3] = [
        ("vacio.png", Vec::new()),
        ("basura.png", b"esto no es un PNG, ni de lejos".to_vec()),
        ("sin-datos.png", header_only(8, 8)),
    ];
    for (name, bytes) in casos {
        let path = write_file(tmp.path(), name, &bytes);
        let result = load(&path);
        assert!(
            matches!(result, Err(FileError::Png(_))),
            "{name}: {result:?}"
        );
    }
}

#[test]
fn fila_3_fichero_mayor_que_max_bytes_es_too_big_file() {
    let tmp = TmpDir::new();
    let path = write_file(tmp.path(), "grande.png", &[0u8; 200]);
    assert_eq!(
        load_png(&path, 100, 16384).unwrap_err(),
        FileError::TooBigFile {
            bytes: 200,
            max: 100
        }
    );
}

#[test]
fn fila_3_el_tope_de_bytes_es_inclusivo() {
    // «de hasta MAX_FILE_BYTES»: con exactamente `max_bytes` no es TooBigFile (aquí, basura).
    let tmp = TmpDir::new();
    let path = write_file(tmp.path(), "justo.png", &[0u8; 100]);
    let result = load_png(&path, 100, 16384);
    assert!(matches!(result, Err(FileError::Png(_))), "{result:?}");
}

#[test]
fn fila_3_un_png_valido_del_corpus_se_lee_igual_que_read_png() {
    let files = corpus();
    assert!(files.len() >= 4, "el corpus tiene cuatro ficheros");
    for path in files {
        let bytes = fs::read(&path).unwrap();
        let expected = read_png(&bytes).expect("el corpus es válido");
        assert_eq!(load(&path).expect("load_png"), expected, "{path:?}");
    }
}

#[test]
fn las_constantes_de_los_topes() {
    assert_eq!(MAX_FILE_BYTES, 256 << 20);
    assert_eq!(MAX_OPEN_PIXELS, 1 << 26);
}

// ---------------------------------------------------------------------------------------
// Fila 4: se rechaza antes de decodificar (solo firma + IHDR).
// ---------------------------------------------------------------------------------------

#[test]
fn fila_4_ancho_mayor_que_max_side() {
    let tmp = TmpDir::new();
    let path = write_file(tmp.path(), "ancho.png", &header_only(20000, 1));
    assert_eq!(
        load_png(&path, MAX_FILE_BYTES, 16384).unwrap_err(),
        FileError::TooLargeToShow {
            width: 20000,
            height: 1,
            max_side: 16384
        }
    );
}

#[test]
fn fila_4_demasiados_pixeles() {
    let tmp = TmpDir::new();
    let path = write_file(tmp.path(), "pixeles.png", &header_only(8193, 8192));
    assert_eq!(
        load_png(&path, MAX_FILE_BYTES, 16384).unwrap_err(),
        FileError::TooManyPixels {
            width: 8193,
            height: 8192,
            max: MAX_OPEN_PIXELS
        }
    );
}

#[test]
fn fila_4_el_max_side_es_el_que_se_pasa() {
    let tmp = TmpDir::new();
    let path = write_file(tmp.path(), "lado.png", &header_only(2049, 1));
    assert_eq!(
        load_png(&path, MAX_FILE_BYTES, 2048).unwrap_err(),
        FileError::TooLargeToShow {
            width: 2049,
            height: 1,
            max_side: 2048
        }
    );
}

#[test]
fn fila_4_justo_el_tope_pasa_check_fits_y_llega_a_read_png() {
    let tmp = TmpDir::new();
    let path = write_file(tmp.path(), "justo.png", &header_only(8192, 8192));
    // No hay datos: el error es de `read_png` (`Png`), no de `check_fits`.
    let result = load_png(&path, MAX_FILE_BYTES, 16384);
    assert!(matches!(result, Err(FileError::Png(_))), "{result:?}");
}

// ---------------------------------------------------------------------------------------
// Fila 5: check_fits.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_5_check_fits() {
    assert_eq!(check_fits(8192, 8192, 16384), Ok(()));
    assert_eq!(check_fits(16384, 4096, 16384), Ok(()));
    assert_eq!(
        check_fits(8193, 8192, 16384),
        Err(FileError::TooManyPixels {
            width: 8193,
            height: 8192,
            max: MAX_OPEN_PIXELS
        })
    );
    assert_eq!(
        check_fits(16385, 1, 16384),
        Err(FileError::TooLargeToShow {
            width: 16385,
            height: 1,
            max_side: 16384
        })
    );
    assert_eq!(
        check_fits(1, 16385, 16384),
        Err(FileError::TooLargeToShow {
            width: 1,
            height: 16385,
            max_side: 16384
        })
    );
    // Sin desbordar: u32::MAX × u32::MAX cabe en u64 pero el lado manda.
    assert_eq!(
        check_fits(u32::MAX, u32::MAX, 16384),
        Err(FileError::TooLargeToShow {
            width: u32::MAX,
            height: u32::MAX,
            max_side: 16384
        })
    );
}

// ---------------------------------------------------------------------------------------
// Fila 12: escritura atómica.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_12_exito_sobre_destino_nuevo() {
    let tmp = TmpDir::new();
    let target = tmp.path().join("nuevo.png");
    let bytes: Vec<u8> = (0..5000u32).map(|n| (n % 251) as u8).collect();
    write_atomic(&target, &bytes).expect("write_atomic");
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert_eq!(names(tmp.path()), ["nuevo.png"]);
}

#[test]
fn fila_12_destino_directorio_no_vacio_falla_sin_dejar_rastro() {
    let tmp = TmpDir::new();
    let target = tmp.path().join("destino");
    fs::create_dir(&target).unwrap();
    write_file(&target, "dentro.bin", b"contenido de dentro");
    let result = write_atomic(&target, b"nuevo contenido");
    assert!(matches!(result, Err(FileError::Write(_))), "{result:?}");
    assert_eq!(names(tmp.path()), ["destino"]);
    assert_eq!(names(&target), ["dentro.bin"]);
    assert_eq!(
        fs::read(target.join("dentro.bin")).unwrap(),
        b"contenido de dentro"
    );
}

// ---------------------------------------------------------------------------------------
// Fila 13: permisos del destino existente.
// ---------------------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn fila_13_se_conservan_los_permisos() {
    let tmp = TmpDir::new();
    let target = write_file(tmp.path(), "existente.png", &[7u8; 1024]);
    set_mode(&target, 0o640);
    let bytes: Vec<u8> = (0..3000u32).map(|n| (n % 241) as u8).collect();
    write_atomic(&target, &bytes).expect("write_atomic");
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert_eq!(mode_of(&target), 0o640);
    assert_eq!(names(tmp.path()), ["existente.png"]);
}

// ---------------------------------------------------------------------------------------
// Fila 14: solo lectura (se salta como root).
// ---------------------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn fila_14_write_atomic_rechaza_un_fichero_de_solo_lectura() {
    if !permissions_enforced() {
        return;
    }
    let tmp = TmpDir::new();
    let target = write_file(tmp.path(), "ro.png", b"contenido original");
    set_mode(&target, 0o444);
    let result = write_atomic(&target, b"nuevo");
    assert!(matches!(result, Err(FileError::ReadOnly(_))), "{result:?}");
    assert_eq!(fs::read(&target).unwrap(), b"contenido original");
    assert_eq!(names(tmp.path()), ["ro.png"]);
}

#[cfg(unix)]
#[test]
fn fila_14_write_atomic_en_un_directorio_sin_permiso_da_write() {
    if !permissions_enforced() {
        return;
    }
    let tmp = TmpDir::new();
    let dir = tmp.path().join("ro-dir");
    fs::create_dir(&dir).unwrap();
    set_mode(&dir, 0o555);
    let result = write_atomic(&dir.join("nuevo.png"), b"x");
    assert!(matches!(result, Err(FileError::Write(_))), "{result:?}");
    assert!(
        names(&dir).is_empty(),
        "no debe crearse nada: {:?}",
        names(&dir)
    );
}

#[cfg(unix)]
#[test]
fn fila_14_sesion_con_fichero_de_solo_lectura() {
    if !permissions_enforced() {
        return;
    }
    let tmp = TmpDir::new();
    let path = write_file(
        tmp.path(),
        "x.png",
        &write_png(&sample_image(Mode::Rgba)).unwrap(),
    );
    let mut session = Session::sample(Document::new(sample_image(Mode::Gray)));
    session.open(&path, 16384, false).expect("abrir x.png");
    assert!(edit_one(&mut session.document, 0, 0));
    assert!(session.modified());

    let before = fs::read(&path).unwrap();
    set_mode(&path, 0o444);
    let result = session.save(tmp.path());
    assert!(matches!(result, Err(FileError::ReadOnly(_))), "{result:?}");
    assert!(session.modified(), "tras el fallo sigue modificado");
    assert_eq!(fs::read(&path).unwrap(), before, "contenido intacto");
    assert_eq!(
        names(tmp.path()),
        ["x.png"],
        "ni temporales ni ficheros nuevos"
    );

    // Después, un guardado correcto en otro sitio (la copia, junto al original).
    let saved = session.save_as(tmp.path()).expect("save_as");
    assert_eq!(saved.path.file_name().unwrap(), "x-copia.png");
    assert!(!session.modified());
    let after = names(tmp.path());
    assert_eq!(after.len(), 2, "{after:?}");
    assert!(after.contains(&"x.png".to_string()) && after.contains(&"x-copia.png".to_string()));
}

#[cfg(unix)]
#[test]
fn fila_14_sesion_con_directorio_de_solo_lectura() {
    if !permissions_enforced() {
        return;
    }
    let tmp = TmpDir::new();
    let closed = tmp.path().join("cerrado");
    let open = tmp.path().join("abierto");
    fs::create_dir(&closed).unwrap();
    fs::create_dir(&open).unwrap();
    set_mode(&closed, 0o555);

    let mut session = Session::sample(Document::new(sample_image(Mode::Indexed)));
    assert!(edit_one(&mut session.document, 1, 1));
    let result = session.save(&closed);
    assert!(matches!(result, Err(FileError::Write(_))), "{result:?}");
    assert!(session.modified(), "tras el fallo sigue modificado");
    assert!(session.path().is_none(), "sin ruta nueva");
    assert!(
        names(&closed).is_empty(),
        "nada creado: {:?}",
        names(&closed)
    );

    let saved = session.save(&open).expect("save en otro sitio");
    assert_eq!(saved.path.file_name().unwrap(), "sin-titulo.png");
    assert!(!session.modified());
    assert_eq!(names(&open), ["sin-titulo.png"]);
}

// ---------------------------------------------------------------------------------------
// Fila 15: enlace simbólico (solo Unix).
// ---------------------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn fila_15_save_escribe_en_el_destino_real_y_el_enlace_sigue_siendo_enlace() {
    let tmp = TmpDir::new();
    let real = write_file(
        tmp.path(),
        "real.png",
        &write_png(&sample_image(Mode::Rgba)).unwrap(),
    );
    let link = tmp.path().join("l.png");
    std::os::unix::fs::symlink("real.png", &link).expect("crear el enlace");

    let mut session = Session::sample(Document::new(sample_image(Mode::Gray)));
    session
        .open(&link, 16384, false)
        .expect("abrir por el enlace");
    assert!(edit_one(&mut session.document, 2, 1));
    let edited = main_image(&session.document).clone();
    session.save(tmp.path()).expect("save");

    let stored = read_png(&fs::read(&real).unwrap()).expect("real.png es un PNG");
    assert_eq!(stored.image, edited, "real.png lleva la imagen nueva");
    let meta = fs::symlink_metadata(&link).unwrap();
    assert!(
        meta.file_type().is_symlink(),
        "l.png sigue siendo un enlace"
    );
    assert_eq!(fs::read_link(&link).unwrap(), Path::new("real.png"));
    let listed = names(tmp.path());
    assert_eq!(listed.len(), 2, "sin temporales: {listed:?}");
}

#[cfg(unix)]
#[test]
fn fila_15_write_atomic_directo_sobre_un_enlace() {
    let tmp = TmpDir::new();
    let real = write_file(tmp.path(), "real.bin", b"viejo");
    let link = tmp.path().join("l.bin");
    std::os::unix::fs::symlink("real.bin", &link).unwrap();
    write_atomic(&link, b"nuevo contenido").expect("write_atomic");
    assert_eq!(fs::read(&real).unwrap(), b"nuevo contenido");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(names(tmp.path()).len(), 2);
}

// ---------------------------------------------------------------------------------------
// Fila 21: file_label. Y nombres de copia (copy_path y untitled_path).
// ---------------------------------------------------------------------------------------

#[test]
fn fila_21_file_label() {
    assert_eq!(file_label(Path::new("/a/b/casa.png")), "casa.png");
    assert_eq!(file_label(Path::new("casa.png")), "casa.png");
    assert_eq!(file_label(Path::new("/")), "/");
    assert_eq!(file_label(Path::new("a/b/")), "b");
}

#[test]
fn copy_path_y_untitled_path_numeran_sin_pisar() {
    let tmp = TmpDir::new();
    let original = tmp.path().join("x.png");
    assert_eq!(copy_path(&original), tmp.path().join("x-copia.png"));
    write_file(tmp.path(), "x-copia.png", b"1");
    assert_eq!(copy_path(&original), tmp.path().join("x-copia-2.png"));
    write_file(tmp.path(), "x-copia-2.png", b"2");
    assert_eq!(copy_path(&original), tmp.path().join("x-copia-3.png"));

    assert_eq!(untitled_path(tmp.path()), tmp.path().join("sin-titulo.png"));
    write_file(tmp.path(), "sin-titulo.png", b"1");
    assert_eq!(
        untitled_path(tmp.path()),
        tmp.path().join("sin-titulo-2.png")
    );
    write_file(tmp.path(), "sin-titulo-2.png", b"2");
    assert_eq!(
        untitled_path(tmp.path()),
        tmp.path().join("sin-titulo-3.png")
    );
}

#[test]
fn los_errores_se_muestran_en_texto_no_vacio() {
    let errors = [
        FileError::NotAFile,
        FileError::TooBigFile { bytes: 2, max: 1 },
        FileError::Read("fallo".to_string()),
        FileError::TooLargeToShow {
            width: 3,
            height: 1,
            max_side: 2,
        },
        FileError::TooManyPixels {
            width: 3,
            height: 3,
            max: 4,
        },
        FileError::Unsaved,
        FileError::ReadOnly(PathBuf::from("/d/a.png")),
        FileError::Write("fallo".to_string()),
    ];
    for error in errors {
        assert!(!error.to_string().is_empty(), "{error:?}");
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 4: un fallo de write_atomic no destruye el contenido anterior.
// ---------------------------------------------------------------------------------------

/// Destino que es un directorio no vacío con `previous` dentro.
fn failure_with_directory(previous: &[u8], new: &[u8]) -> Result<(), TestCaseError> {
    let tmp = TmpDir::new();
    let target = tmp.path().join("destino");
    fs::create_dir(&target).unwrap();
    write_file(&target, "dentro.bin", previous);
    let result = write_atomic(&target, new);
    prop_assert!(matches!(result, Err(FileError::Write(_))), "{result:?}");
    prop_assert_eq!(
        fs::read(target.join("dentro.bin")).unwrap(),
        previous.to_vec()
    );
    prop_assert_eq!(names(&target), vec!["dentro.bin".to_string()]);
    prop_assert_eq!(names(tmp.path()), vec!["destino".to_string()]);
    Ok(())
}

/// Destino de solo lectura con `previous` dentro (se salta como `root` y fuera de Unix).
#[cfg(unix)]
fn failure_read_only(previous: &[u8], new: &[u8]) -> Result<(), TestCaseError> {
    if !permissions_enforced() {
        return Ok(());
    }
    let tmp = TmpDir::new();
    let target = write_file(tmp.path(), "ro.png", previous);
    set_mode(&target, 0o444);
    let result = write_atomic(&target, new);
    prop_assert!(matches!(result, Err(FileError::ReadOnly(_))), "{result:?}");
    prop_assert_eq!(fs::read(&target).unwrap(), previous.to_vec());
    prop_assert_eq!(names(tmp.path()), vec!["ro.png".to_string()]);
    Ok(())
}

#[cfg(not(unix))]
fn failure_read_only(_previous: &[u8], _new: &[u8]) -> Result<(), TestCaseError> {
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn propiedad_4_un_fallo_no_destruye(
        previous in prop::collection::vec(any::<u8>(), 0..2048),
        new in prop::collection::vec(any::<u8>(), 0..512),
        directory in any::<bool>(),
    ) {
        if directory {
            failure_with_directory(&previous, &new)?;
        } else {
            failure_read_only(&previous, &new)?;
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 6 (check_fits): Ok si y solo si ambos lados <= m y w × h <= 2^26.
// ---------------------------------------------------------------------------------------

fn side() -> impl Strategy<Value = u32> {
    prop_oneof![any::<u32>(), 0u32..20000, 8190u32..8195, 16380u32..16390]
}

fn max_side() -> impl Strategy<Value = usize> {
    prop_oneof![any::<usize>(), 0usize..40000, 16380usize..16390]
}

proptest! {
    #[test]
    fn propiedad_6_check_fits_es_ok_si_y_solo_si_cabe(w in side(), h in side(), m in max_side()) {
        let m64 = m as u64;
        let fits_side = u64::from(w) <= m64 && u64::from(h) <= m64;
        let fits_pixels = u64::from(w) * u64::from(h) <= (1u64 << 26);
        let result = check_fits(w, h, m);
        prop_assert_eq!(result.is_ok(), fits_side && fits_pixels, "{}×{} con {}: {:?}", w, h, m, result);
        match result {
            Ok(()) => {}
            Err(FileError::TooLargeToShow { width, height, max_side }) => {
                prop_assert!(!fits_side);
                prop_assert_eq!((width, height, max_side), (w, h, m));
            }
            Err(FileError::TooManyPixels { width, height, max }) => {
                prop_assert!(fits_side && !fits_pixels);
                prop_assert_eq!((width, height, max), (w, h, MAX_OPEN_PIXELS));
            }
            Err(other) => prop_assert!(false, "error inesperado: {:?}", other),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 7: nombres de copia.
// ---------------------------------------------------------------------------------------

/// Nombres de fichero válidos en Unix: espacios, Unicode, sin extensión, `.PNG`, con punto al
/// principio...
fn file_name() -> impl Strategy<Value = String> {
    (
        "[^/\\x00]{0,20}",
        prop_oneof![
            Just(""),
            Just(".png"),
            Just(".PNG"),
            Just(".tar.gz"),
            Just("."),
            Just(".png.png"),
        ],
    )
        .prop_map(|(stem, ext)| format!("{stem}{ext}"))
        .prop_filter("nombre de fichero válido", |name| {
            !name.is_empty() && name != "." && name != ".."
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn propiedad_7_copy_path(name in file_name(), existing in 0usize..=5) {
        let tmp = TmpDir::new();
        let original = tmp.path().join(&name);
        // Un sistema de ficheros puede rechazar un nombre generado (macOS devuelve `EILSEQ`
        // con algunos Unicode): eso no dice nada de `copy_path`, así que se descarta el caso.
        prop_assume!(fs::write(&original, b"original").is_ok());
        let mut seen: Vec<PathBuf> = Vec::new();
        for i in 0..=existing {
            let copy = copy_path(&original);
            prop_assert_eq!(copy.parent(), Some(tmp.path()), "directorio de {:?}", name);
            prop_assert!(copy.to_string_lossy().ends_with(".png"), "{:?}", copy);
            prop_assert!(!copy.exists(), "{:?} ya existe", copy);
            prop_assert_ne!(&copy, &original);
            prop_assert!(!seen.contains(&copy), "{:?} repetido", copy);
            if i < existing {
                prop_assume!(fs::write(&copy, b"copia").is_ok());
            }
            seen.push(copy);
        }
        prop_assert_eq!(fs::read(&original).unwrap(), b"original".to_vec());
    }

    #[test]
    fn propiedad_7_untitled_path(existing in 0usize..=5) {
        let tmp = TmpDir::new();
        let mut seen: Vec<PathBuf> = Vec::new();
        for i in 0..=existing {
            let path = untitled_path(tmp.path());
            prop_assert_eq!(path.parent(), Some(tmp.path()));
            prop_assert!(path.to_string_lossy().ends_with(".png"), "{:?}", path);
            prop_assert!(!path.exists(), "{:?} ya existe", path);
            prop_assert!(!seen.contains(&path), "{:?} repetido", path);
            if i < existing {
                fs::write(&path, b"x").unwrap();
            }
            seen.push(path);
        }
    }
}
