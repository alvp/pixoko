//! La sesión con `.aseprite` (specs/hito-3-aseprite.md, unidad 3.8): fila 12 de «Casos», cinco
//! pruebas. Sin ventana: `Session` y ficheros en un directorio temporal propio; el corpus de
//! `golden/aseprite/` se copia allí antes de editarlo (nunca se guarda dentro de `golden/`).
//!
//! Contrato de API que estas pruebas fijan (la lógica debe ofrecerlo):
//! - `Loaded.document` (en lugar de `Loaded.image`) y `Session::load`/`open` que detectan el
//!   formato por contenido (`px_formats::detect_format`).
//! - `FileError::Aseprite(AsepriteError)`.
//! - `Session::save_route() -> SaveRoute` (`SaveRoute::{Direct, Dialog}`, visible por `super::*`)
//!   y, en `crate::dialogs`, `proposed_save_name(Option<&Path>, usize) -> String` y
//!   `dialog_title(DialogKind, usize, Option<&Path>) -> &'static str`.
//!
//! Los ficheros con etiquetas o varios fotogramas se fabrican añadiendo un chunk, o bytes tras el
//! primer fotograma, a un `.aseprite` del corpus (Spec: Frames, Tags Chunk); no hace falta zlib.

use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use px_core::Rgba;
use px_formats::{
    AsepriteError, AsepriteLimits, FileFormat, detect_format, read_aseprite, read_png, write_png,
};

use crate::dialogs::{DialogKind, dialog_title, proposed_save_name};

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
            std::env::temp_dir().join(format!("pixoko-px-app-aseprite-{}-{n}", std::process::id()));
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

fn corpus(name_2: &str) -> Vec<u8> {
    let path_2 = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../golden/aseprite")
        .join(format!("{name_2}.aseprite"));
    fs::read(&path_2).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path_2.display()))
}

/// Copia un caso del corpus a `tmp` con el nombre dado y devuelve la ruta.
fn copy_of_the_corpus(tmp: &Tmp, case: &str, as_word: &str) -> PathBuf {
    let target_2 = tmp.path_2().join(as_word);
    fs::write(&target_2, corpus(case)).expect("copiar el caso del corpus");
    target_2
}

fn session_empty() -> Session {
    Session::sample(Document::new(
        Image::filled(3, 3, Pixel::Rgba(Rgba::opaque(10, 20, 30))).expect("imagen"),
    ))
}

fn adds_layer(session_2: &mut Session, name_2: &str) {
    let at = session_2.document.layers().len();
    session_2
        .apply_layer(Command::AddLayer {
            name: name_2.to_string(),
            at,
        })
        .expect("añadir capa");
}

/// Spec: Header, tamaño del fichero (`DWORD` en el byte 0) y número de fotogramas (`WORD` en 6).
const OFF_TAM_FILE: usize = 0;
const OFF_FRAMES: usize = 6;
/// Spec: Frames, campos del primer fotograma (sigue a la cabecera de 128 bytes): tamaño (`DWORD`),
/// número antiguo de chunks (`WORD`) y número nuevo (`DWORD`).
const OFF_FRAME_TAM: usize = 128;
const OFF_FRAME_CHUNKS_OLD: usize = 128 + 6;
const OFF_FRAME_CHUNKS_NEW: usize = 128 + 12;

/// Añade un chunk al final del primer fotograma de un `.aseprite` de un fotograma y arregla los
/// tamaños y contadores (Spec: Frames, formato de un chunk).
fn with_chunk(mut bytes: Vec<u8>, kind: u16, data: &[u8]) -> Vec<u8> {
    bytes.extend(((data.len() + 6) as u32).to_le_bytes());
    bytes.extend(kind.to_le_bytes());
    bytes.extend(data);
    let total = bytes.len() as u32;
    bytes[OFF_TAM_FILE..OFF_TAM_FILE + 4].copy_from_slice(&total.to_le_bytes());
    bytes[OFF_FRAME_TAM..OFF_FRAME_TAM + 4].copy_from_slice(&(total - 128).to_le_bytes());
    let a = OFF_FRAME_CHUNKS_OLD;
    let old = u16::from_le_bytes([bytes[a], bytes[a + 1]]) + 1;
    bytes[a..a + 2].copy_from_slice(&old.to_le_bytes());
    let n = OFF_FRAME_CHUNKS_NEW;
    let new_2 = u32::from_le_bytes([bytes[n], bytes[n + 1], bytes[n + 2], bytes[n + 3]]) + 1;
    bytes[n..n + 4].copy_from_slice(&new_2.to_le_bytes());
    bytes
}

/// Declara `n` fotogramas y deja detrás del primero unos bytes que no se miran.
fn with_frames(mut bytes: Vec<u8>, n: u16) -> Vec<u8> {
    bytes.extend([0xAB; 40]);
    let total = bytes.len() as u32;
    bytes[OFF_TAM_FILE..OFF_TAM_FILE + 4].copy_from_slice(&total.to_le_bytes());
    bytes[OFF_FRAMES..OFF_FRAMES + 2].copy_from_slice(&n.to_le_bytes());
    bytes
}

/// Spec: Tags Chunk (0x2018), con una etiqueta.
fn labels() -> Vec<u8> {
    let mut d = vec![1, 0];
    d.extend([0u8; 8]);
    d.extend([0, 0, 0, 0, 0, 0, 0]); // desde, hasta, dirección y repeticiones
    d.extend([0u8; 6]);
    d.extend([0u8; 3]);
    d.push(0);
    d.extend([4, 0]);
    d.extend(b"anda");
    d
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .expect("leer el directorio")
        .map(|e| {
            e.expect("entrada")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    v.sort();
    v
}

fn es_aseprite(bytes: &[u8]) -> bool {
    matches!(detect_format(bytes), Some(FileFormat::Aseprite))
}

fn es_png(bytes: &[u8]) -> bool {
    matches!(detect_format(bytes), Some(FileFormat::Png))
}

// ---------------------------------------------------------------------------------------------
// Fila 12
// ---------------------------------------------------------------------------------------------

#[test]
fn row_12_a_opening_an_aseprite_installs_the_document_with_layers_the_active_one_is_the_top_one_and_warns()
 {
    let tmp = Tmp::new_2();
    let path_2 = copy_of_the_corpus(&tmp, "mixed-rgba", "mixto.aseprite");
    let mut session_2 = session_empty();

    let loaded = session_2
        .load(&path_2, 16384, false)
        .expect("cargar el .aseprite");
    assert_eq!(loaded.document.layers().len(), 6);
    assert_eq!(
        (loaded.document.width(), loaded.document.height()),
        (48, 48)
    );
    for piece in ["mixto.aseprite", "48×48", "6 capas"] {
        assert!(
            loaded.text.contains(piece),
            "{piece:?} en {:?}",
            loaded.text
        );
    }
    assert!(!loaded.text.contains("se ignoran:"), "{}", loaded.text);
    assert!(!loaded.text.contains("fotograma"), "{}", loaded.text);
    // `load` no cambia la sesión.
    assert_eq!(session_2.document.layers().len(), 1);

    session_2.install(loaded);
    assert_eq!(session_2.document.layers().len(), 6);
    assert_eq!(
        session_2.active_layer(),
        session_2.document.layers()[5].id(),
        "la de arriba"
    );
    assert_eq!(session_2.path(), Some(path_2.as_path()));
    assert!(!session_2.modified());
    assert_eq!(session_2.save_route(), SaveRoute::Direct);
}

#[test]
fn row_12_b_an_aseprite_with_labels_and_two_frames_warns_and_ctrl_s_saves_a_copy() {
    let tmp = Tmp::new_2();
    let original = with_frames(
        with_chunk(corpus("layers-normal-three"), 0x2018, &labels()),
        2,
    );
    let path_2 = tmp.path_2().join("anim.aseprite");
    fs::write(&path_2, &original).expect("escribir el original");

    let mut session_2 = session_empty();
    let text_2 = session_2.open(&path_2, 16384, false).expect("abrir");
    for piece in [
        "3 capas",
        "se ignoran:",
        "Ctrl+S guardará una copia",
        "solo se abre el fotograma 1 de 2",
    ] {
        assert!(text_2.contains(piece), "{piece:?} en {text_2:?}");
    }
    assert_eq!(session_2.document.layers().len(), 3);

    // Ctrl+S: una copia `-copy.aseprite`, con el motivo, y el original intacto.
    let saved = session_2.save(tmp.path_2()).expect("Ctrl+S");
    assert_eq!(
        saved.path.file_name().expect("nombre"),
        "anim-copia.aseprite"
    );
    assert!(saved.copy_reason.is_some(), "la copia dice por qué");
    assert_eq!(
        fs::read(&path_2).expect("original"),
        original,
        "el original no se toca"
    );
    let copy = fs::read(&saved.path).expect("copia");
    assert!(es_aseprite(&copy));
    let read = read_aseprite(&copy, &AsepriteLimits::default()).expect("la copia se lee");
    assert_eq!(read.document.layers().len(), 3);
    assert!(!read.report.lossy && read.report.frames_in_file == 1);
    assert_eq!(session_2.path(), Some(saved.path.as_path()));
    assert!(!session_2.modified());

    // Ya es la copia, que se entiende entera: el siguiente Ctrl+S la reescribe sin otra copia.
    let other = session_2.save(tmp.path_2()).expect("segundo Ctrl+S");
    assert_eq!(
        (other.path.as_path(), other.copy_reason),
        (saved.path.as_path(), None)
    );
    assert_eq!(
        names(tmp.path_2()),
        ["anim-copia.aseprite", "anim.aseprite"]
    );
}

#[test]
fn row_12_c_ctrl_s_with_an_aseprite_path_writes_aseprite_and_save_as_decides_by_the_extension() {
    let tmp = Tmp::new_2();
    let path_2 = copy_of_the_corpus(&tmp, "layers-normal-three", "tres.aseprite");
    let mut session_2 = session_empty();
    session_2.open(&path_2, 16384, false).expect("abrir");
    adds_layer(&mut session_2, "nueva");
    assert!(session_2.modified());

    // Ctrl+S con ruta .aseprite: sin diálogo y escribe .aseprite en esa ruta.
    assert_eq!(session_2.save_route(), SaveRoute::Direct);
    let saved = session_2.save(tmp.path_2()).expect("Ctrl+S");
    assert_eq!(
        (saved.path.as_path(), saved.copy_reason),
        (path_2.as_path(), None)
    );
    let bytes = fs::read(&path_2).expect("leer");
    assert!(es_aseprite(&bytes));
    let read = read_aseprite(&bytes, &AsepriteLimits::default()).expect("se lee");
    assert_eq!(read.document.layers().len(), 4);
    assert!(!session_2.modified());

    // Guardar como: el formato lo decide la extensión (sin mirar mayúsculas).
    let cases = [
        ("a.aseprite", true, "a.aseprite"),
        ("B.ASEPRITE", true, "B.ASEPRITE"),
        ("c.png", false, "c.png"),
        ("d", false, "d.png"),
        ("e.jpg", false, "e.jpg.png"),
    ];
    for (chosen, aseprite, target_2) in cases {
        let saved = session_2.save_to(&tmp.path_2().join(chosen)).expect(chosen);
        assert_eq!(saved.path.file_name().expect("nombre"), target_2);
        let bytes = fs::read(&saved.path).expect("leer");
        assert_eq!(es_aseprite(&bytes), aseprite, "{chosen}");
        assert_eq!(es_png(&bytes), !aseprite, "{chosen}");
        if !aseprite {
            let png = read_png(&bytes).expect("PNG aplanado").image;
            assert_eq!((png.width(), png.height()), (24, 24));
        }
    }

    // Tras guardar como .aseprite, el documento pasa a ese fichero y no está modificado.
    adds_layer(&mut session_2, "otra");
    assert!(session_2.modified());
    let saved = session_2
        .save_to(&tmp.path_2().join("final.aseprite"))
        .expect("guardar como");
    assert_eq!(session_2.path(), Some(saved.path.as_path()));
    assert_eq!(saved.path.file_name().expect("nombre"), "final.aseprite");
    assert!(!session_2.modified());
    let read = read_aseprite(
        &fs::read(&saved.path).expect("leer"),
        &AsepriteLimits::default(),
    )
    .expect("se lee");
    assert_eq!(read.document.layers().len(), 5);
}

#[test]
fn row_12_d_open_goes_by_content_and_gray_is_not_saved_as_aseprite() {
    let tmp = Tmp::new_2();
    // Un PNG llamado `.aseprite`.
    let png = write_png(&Image::filled(3, 2, Pixel::Rgba(Rgba::opaque(1, 2, 3))).expect("imagen"))
        .expect("PNG");
    let false_value = tmp.path_2().join("falso.aseprite");
    fs::write(&false_value, png).expect("escribir");
    let mut session_2 = session_empty();
    session_2
        .open(&false_value, 16384, false)
        .expect("un PNG con extensión .aseprite se abre");
    assert_eq!(session_2.document.layers().len(), 1);
    assert_eq!(
        (session_2.document.width(), session_2.document.height()),
        (3, 2)
    );

    // Un `.aseprite` llamado `.png`.
    let odd = tmp.path_2().join("raro.png");
    fs::write(&odd, corpus("blend-normal")).expect("escribir");
    let text_2 = session_2
        .open(&odd, 16384, false)
        .expect("un .aseprite con extensión .png se abre");
    assert_eq!(session_2.document.layers().len(), 2);
    assert!(text_2.contains("2 capas"), "{text_2}");

    // Gris a `.aseprite`: error con motivo, nada escrito; el PNG sí se guarda.
    let mut gray = Session::sample(Document::new(Image::new(3, 3, Mode::Gray).expect("gris")));
    let target_2 = tmp.path_2().join("g.aseprite");
    match gray.save_to(&target_2) {
        Err(FileError::Aseprite(error)) => {
            assert!(matches!(error, AsepriteError::Unsupported(_)), "{error:?}");
            let text_2 = FileError::Aseprite(error).to_string();
            assert!(
                text_2.contains("gris") && text_2.contains("el fichero no se ha modificado"),
                "{text_2}"
            );
        }
        other_2 => panic!("se esperaba FileError::Aseprite(Unsupported) y salió {other_2:?}"),
    }
    assert!(!target_2.exists(), "no se escribe nada");
    assert_eq!(gray.path(), None);
    let saved = gray
        .save_to(&tmp.path_2().join("g.png"))
        .expect("el PNG de grises se guarda");
    let read_2 = read_png(&fs::read(&saved.path).expect("leer"))
        .expect("PNG")
        .image;
    assert_eq!(read_2.mode(), Mode::Gray);
}

#[test]
fn row_12_e_a_png_with_layers_asks_for_a_dialog_and_the_names_and_titles_say_the_format() {
    let tmp = Tmp::new_2();
    let path_2 = tmp.path_2().join("foto.png");
    let png = write_png(&Image::filled(4, 4, Pixel::Rgba(Rgba::opaque(9, 9, 9))).expect("imagen"))
        .expect("PNG");
    fs::write(&path_2, png).expect("escribir");
    let mut session_2 = session_empty();
    session_2.open(&path_2, 16384, false).expect("abrir");

    // Regla 2: con 2 o más capas y una ruta que no es `.aseprite`, Ctrl+S abre el diálogo.
    assert_eq!(
        session_2.save_route(),
        SaveRoute::Direct,
        "una capa y ruta .png"
    );
    adds_layer(&mut session_2, "dos");
    assert_eq!(
        session_2.save_route(),
        SaveRoute::Dialog,
        "dos capas y ruta .png"
    );
    let layers_2 = session_2.document.layers().len();
    assert_eq!(
        proposed_save_name(session_2.path(), layers_2),
        "foto.aseprite"
    );
    assert_eq!(
        dialog_title(DialogKind::Save, layers_2, session_2.path()),
        "Guardar como .aseprite (con capas) o PNG (aplanado)"
    );
    // Sin ruta, siempre diálogo; con ruta `.aseprite`, directo aunque haya muchas capas.
    assert_eq!(session_empty().save_route(), SaveRoute::Dialog);
    let aseprite = copy_of_the_corpus(&tmp, "layers-normal-three", "tres.ASEPRITE");
    let mut other = session_empty();
    other.open(&aseprite, 16384, false).expect("abrir");
    assert_eq!(
        other.save_route(),
        SaveRoute::Direct,
        "ruta .aseprite sin mirar mayúsculas"
    );

    // Los nombres propuestos y los títulos de los diálogos (todas las combinaciones de la spec).
    let path_png = Path::new("/d/x.png");
    let ase = Path::new("/d/x.aseprite");
    assert_eq!(proposed_save_name(None, 2), "sin-titulo.aseprite");
    assert_eq!(proposed_save_name(None, 1), "sin-titulo.png");
    assert_eq!(proposed_save_name(Some(path_png), 2), "x.aseprite");
    assert_eq!(proposed_save_name(Some(path_png), 1), "x.png");
    assert_eq!(proposed_save_name(Some(ase), 1), "x.aseprite");
    assert_eq!(
        dialog_title(DialogKind::Open, 1, None),
        "Abrir imagen (PNG o Aseprite)"
    );
    let with_layers = "Guardar como .aseprite (con capas) o PNG (aplanado)";
    assert_eq!(
        dialog_title(DialogKind::SaveAs, 3, Some(path_png)),
        with_layers
    );
    assert_eq!(dialog_title(DialogKind::Save, 2, None), with_layers);
    assert_eq!(
        dialog_title(DialogKind::Save, 1, Some(ase)),
        "Guardar .aseprite"
    );
    assert_eq!(
        dialog_title(DialogKind::SaveAs, 1, Some(path_png)),
        "Guardar PNG"
    );
    assert_eq!(dialog_title(DialogKind::Save, 1, None), "Guardar PNG");

    // Si en ese diálogo se elige un `.png`, se guarda aplanado y el aviso lo dice.
    let saved = session_2
        .save_to(&tmp.path_2().join("plano.png"))
        .expect("guardar el PNG");
    assert!(es_png(&fs::read(&saved.path).expect("leer")));
    let notice = with_flatten_notice(describe_saved_dialog(&saved), &session_2.document);
    assert!(notice.contains("se guarda aplanado"), "{notice}");
    assert!(notice.contains("conservas las capas"), "{notice}");
}
