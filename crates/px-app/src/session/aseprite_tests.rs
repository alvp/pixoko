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

static CONTADOR: AtomicU64 = AtomicU64::new(0);

struct Tmp {
    ruta: PathBuf,
}

impl Tmp {
    fn nuevo() -> Tmp {
        let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
        let ruta =
            std::env::temp_dir().join(format!("pixoko-px-app-aseprite-{}-{n}", std::process::id()));
        fs::create_dir_all(&ruta).expect("directorio temporal");
        Tmp { ruta }
    }

    fn ruta(&self) -> &Path {
        &self.ruta
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.ruta);
    }
}

fn corpus(nombre: &str) -> Vec<u8> {
    let ruta = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../golden/aseprite")
        .join(format!("{nombre}.aseprite"));
    fs::read(&ruta).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", ruta.display()))
}

/// Copia un caso del corpus a `tmp` con el nombre dado y devuelve la ruta.
fn copia_del_corpus(tmp: &Tmp, caso: &str, como: &str) -> PathBuf {
    let destino = tmp.ruta().join(como);
    fs::write(&destino, corpus(caso)).expect("copiar el caso del corpus");
    destino
}

fn sesion_vacia() -> Session {
    Session::sample(Document::new(
        Image::filled(3, 3, Pixel::Rgba(Rgba::opaque(10, 20, 30))).expect("imagen"),
    ))
}

fn anade_capa(sesion: &mut Session, nombre: &str) {
    let at = sesion.document.layers().len();
    sesion
        .apply_layer(Command::AddLayer {
            name: nombre.to_string(),
            at,
        })
        .expect("añadir capa");
}

/// Spec: Header, tamaño del fichero (`DWORD` en el byte 0) y número de fotogramas (`WORD` en 6).
const OFF_TAM_FICHERO: usize = 0;
const OFF_FOTOGRAMAS: usize = 6;
/// Spec: Frames, campos del primer fotograma (sigue a la cabecera de 128 bytes): tamaño (`DWORD`),
/// número antiguo de chunks (`WORD`) y número nuevo (`DWORD`).
const OFF_FOT_TAM: usize = 128;
const OFF_FOT_CHUNKS_ANTIGUO: usize = 128 + 6;
const OFF_FOT_CHUNKS_NUEVO: usize = 128 + 12;

/// Añade un chunk al final del primer fotograma de un `.aseprite` de un fotograma y arregla los
/// tamaños y contadores (Spec: Frames, formato de un chunk).
fn con_chunk(mut bytes: Vec<u8>, tipo: u16, datos: &[u8]) -> Vec<u8> {
    bytes.extend(((datos.len() + 6) as u32).to_le_bytes());
    bytes.extend(tipo.to_le_bytes());
    bytes.extend(datos);
    let total = bytes.len() as u32;
    bytes[OFF_TAM_FICHERO..OFF_TAM_FICHERO + 4].copy_from_slice(&total.to_le_bytes());
    bytes[OFF_FOT_TAM..OFF_FOT_TAM + 4].copy_from_slice(&(total - 128).to_le_bytes());
    let a = OFF_FOT_CHUNKS_ANTIGUO;
    let antiguo = u16::from_le_bytes([bytes[a], bytes[a + 1]]) + 1;
    bytes[a..a + 2].copy_from_slice(&antiguo.to_le_bytes());
    let n = OFF_FOT_CHUNKS_NUEVO;
    let nuevo = u32::from_le_bytes([bytes[n], bytes[n + 1], bytes[n + 2], bytes[n + 3]]) + 1;
    bytes[n..n + 4].copy_from_slice(&nuevo.to_le_bytes());
    bytes
}

/// Declara `n` fotogramas y deja detrás del primero unos bytes que no se miran.
fn con_fotogramas(mut bytes: Vec<u8>, n: u16) -> Vec<u8> {
    bytes.extend([0xAB; 40]);
    let total = bytes.len() as u32;
    bytes[OFF_TAM_FICHERO..OFF_TAM_FICHERO + 4].copy_from_slice(&total.to_le_bytes());
    bytes[OFF_FOTOGRAMAS..OFF_FOTOGRAMAS + 2].copy_from_slice(&n.to_le_bytes());
    bytes
}

/// Spec: Tags Chunk (0x2018), con una etiqueta.
fn etiquetas() -> Vec<u8> {
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

fn nombres(dir: &Path) -> Vec<String> {
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
fn fila_12_a_abrir_un_aseprite_instala_el_documento_con_capas_la_activa_es_la_de_arriba_y_avisa() {
    let tmp = Tmp::nuevo();
    let ruta = copia_del_corpus(&tmp, "mixto-rgba", "mixto.aseprite");
    let mut sesion = sesion_vacia();

    let cargado = sesion
        .load(&ruta, 16384, false)
        .expect("cargar el .aseprite");
    assert_eq!(cargado.document.layers().len(), 6);
    assert_eq!(
        (cargado.document.width(), cargado.document.height()),
        (48, 48)
    );
    for trozo in ["mixto.aseprite", "48×48", "6 capas"] {
        assert!(
            cargado.text.contains(trozo),
            "{trozo:?} en {:?}",
            cargado.text
        );
    }
    assert!(!cargado.text.contains("se ignoran:"), "{}", cargado.text);
    assert!(!cargado.text.contains("fotograma"), "{}", cargado.text);
    // `load` no cambia la sesión.
    assert_eq!(sesion.document.layers().len(), 1);

    sesion.install(cargado);
    assert_eq!(sesion.document.layers().len(), 6);
    assert_eq!(
        sesion.active_layer(),
        sesion.document.layers()[5].id(),
        "la de arriba"
    );
    assert_eq!(sesion.path(), Some(ruta.as_path()));
    assert!(!sesion.modified());
    assert_eq!(sesion.save_route(), SaveRoute::Direct);
}

#[test]
fn fila_12_b_un_aseprite_con_etiquetas_y_dos_fotogramas_avisa_y_ctrl_s_guarda_una_copia() {
    let tmp = Tmp::nuevo();
    let original = con_fotogramas(
        con_chunk(corpus("capas-normal-tres"), 0x2018, &etiquetas()),
        2,
    );
    let ruta = tmp.ruta().join("anim.aseprite");
    fs::write(&ruta, &original).expect("escribir el original");

    let mut sesion = sesion_vacia();
    let texto = sesion.open(&ruta, 16384, false).expect("abrir");
    for trozo in [
        "3 capas",
        "se ignoran:",
        "Ctrl+S guardará una copia",
        "solo se abre el fotograma 1 de 2",
    ] {
        assert!(texto.contains(trozo), "{trozo:?} en {texto:?}");
    }
    assert_eq!(sesion.document.layers().len(), 3);

    // Ctrl+S: una copia `-copia.aseprite`, con el motivo, y el original intacto.
    let guardado = sesion.save(tmp.ruta()).expect("Ctrl+S");
    assert_eq!(
        guardado.path.file_name().expect("nombre"),
        "anim-copia.aseprite"
    );
    assert!(guardado.copy_reason.is_some(), "la copia dice por qué");
    assert_eq!(
        fs::read(&ruta).expect("original"),
        original,
        "el original no se toca"
    );
    let copia = fs::read(&guardado.path).expect("copia");
    assert!(es_aseprite(&copia));
    let leida = read_aseprite(&copia, &AsepriteLimits::default()).expect("la copia se lee");
    assert_eq!(leida.document.layers().len(), 3);
    assert!(!leida.report.lossy && leida.report.frames_in_file == 1);
    assert_eq!(sesion.path(), Some(guardado.path.as_path()));
    assert!(!sesion.modified());

    // Ya es la copia, que se entiende entera: el siguiente Ctrl+S la reescribe sin otra copia.
    let otra = sesion.save(tmp.ruta()).expect("segundo Ctrl+S");
    assert_eq!(
        (otra.path.as_path(), otra.copy_reason),
        (guardado.path.as_path(), None)
    );
    assert_eq!(
        nombres(tmp.ruta()),
        ["anim-copia.aseprite", "anim.aseprite"]
    );
}

#[test]
fn fila_12_c_ctrl_s_con_ruta_aseprite_escribe_aseprite_y_guardar_como_decide_por_la_extension() {
    let tmp = Tmp::nuevo();
    let ruta = copia_del_corpus(&tmp, "capas-normal-tres", "tres.aseprite");
    let mut sesion = sesion_vacia();
    sesion.open(&ruta, 16384, false).expect("abrir");
    anade_capa(&mut sesion, "nueva");
    assert!(sesion.modified());

    // Ctrl+S con ruta .aseprite: sin diálogo y escribe .aseprite en esa ruta.
    assert_eq!(sesion.save_route(), SaveRoute::Direct);
    let guardado = sesion.save(tmp.ruta()).expect("Ctrl+S");
    assert_eq!(
        (guardado.path.as_path(), guardado.copy_reason),
        (ruta.as_path(), None)
    );
    let bytes = fs::read(&ruta).expect("leer");
    assert!(es_aseprite(&bytes));
    let leida = read_aseprite(&bytes, &AsepriteLimits::default()).expect("se lee");
    assert_eq!(leida.document.layers().len(), 4);
    assert!(!sesion.modified());

    // Guardar como: el formato lo decide la extensión (sin mirar mayúsculas).
    let casos = [
        ("a.aseprite", true, "a.aseprite"),
        ("B.ASEPRITE", true, "B.ASEPRITE"),
        ("c.png", false, "c.png"),
        ("d", false, "d.png"),
        ("e.jpg", false, "e.jpg.png"),
    ];
    for (elegido, aseprite, destino) in casos {
        let guardado = sesion.save_to(&tmp.ruta().join(elegido)).expect(elegido);
        assert_eq!(guardado.path.file_name().expect("nombre"), destino);
        let bytes = fs::read(&guardado.path).expect("leer");
        assert_eq!(es_aseprite(&bytes), aseprite, "{elegido}");
        assert_eq!(es_png(&bytes), !aseprite, "{elegido}");
        if !aseprite {
            let png = read_png(&bytes).expect("PNG aplanado").image;
            assert_eq!((png.width(), png.height()), (24, 24));
        }
    }

    // Tras guardar como .aseprite, el documento pasa a ese fichero y no está modificado.
    anade_capa(&mut sesion, "otra");
    assert!(sesion.modified());
    let guardado = sesion
        .save_to(&tmp.ruta().join("final.aseprite"))
        .expect("guardar como");
    assert_eq!(sesion.path(), Some(guardado.path.as_path()));
    assert_eq!(guardado.path.file_name().expect("nombre"), "final.aseprite");
    assert!(!sesion.modified());
    let leida = read_aseprite(
        &fs::read(&guardado.path).expect("leer"),
        &AsepriteLimits::default(),
    )
    .expect("se lee");
    assert_eq!(leida.document.layers().len(), 5);
}

#[test]
fn fila_12_d_abrir_va_por_el_contenido_y_el_gris_no_se_guarda_como_aseprite() {
    let tmp = Tmp::nuevo();
    // Un PNG llamado `.aseprite`.
    let png = write_png(&Image::filled(3, 2, Pixel::Rgba(Rgba::opaque(1, 2, 3))).expect("imagen"))
        .expect("PNG");
    let falso = tmp.ruta().join("falso.aseprite");
    fs::write(&falso, png).expect("escribir");
    let mut sesion = sesion_vacia();
    sesion
        .open(&falso, 16384, false)
        .expect("un PNG con extensión .aseprite se abre");
    assert_eq!(sesion.document.layers().len(), 1);
    assert_eq!((sesion.document.width(), sesion.document.height()), (3, 2));

    // Un `.aseprite` llamado `.png`.
    let raro = tmp.ruta().join("raro.png");
    fs::write(&raro, corpus("mezcla-normal")).expect("escribir");
    let texto = sesion
        .open(&raro, 16384, false)
        .expect("un .aseprite con extensión .png se abre");
    assert_eq!(sesion.document.layers().len(), 2);
    assert!(texto.contains("2 capas"), "{texto}");

    // Gris a `.aseprite`: error con motivo, nada escrito; el PNG sí se guarda.
    let mut gris = Session::sample(Document::new(Image::new(3, 3, Mode::Gray).expect("gris")));
    let destino = tmp.ruta().join("g.aseprite");
    match gris.save_to(&destino) {
        Err(FileError::Aseprite(error)) => {
            assert!(matches!(error, AsepriteError::Unsupported(_)), "{error:?}");
            let texto = FileError::Aseprite(error).to_string();
            assert!(
                texto.contains("gris") && texto.contains("el fichero no se ha modificado"),
                "{texto}"
            );
        }
        otro => panic!("se esperaba FileError::Aseprite(Unsupported) y salió {otro:?}"),
    }
    assert!(!destino.exists(), "no se escribe nada");
    assert_eq!(gris.path(), None);
    let guardado = gris
        .save_to(&tmp.ruta().join("g.png"))
        .expect("el PNG de grises se guarda");
    let leido = read_png(&fs::read(&guardado.path).expect("leer"))
        .expect("PNG")
        .image;
    assert_eq!(leido.mode(), Mode::Gray);
}

#[test]
fn fila_12_e_un_png_con_capas_pide_dialogo_y_los_nombres_y_titulos_dicen_el_formato() {
    let tmp = Tmp::nuevo();
    let ruta = tmp.ruta().join("foto.png");
    let png = write_png(&Image::filled(4, 4, Pixel::Rgba(Rgba::opaque(9, 9, 9))).expect("imagen"))
        .expect("PNG");
    fs::write(&ruta, png).expect("escribir");
    let mut sesion = sesion_vacia();
    sesion.open(&ruta, 16384, false).expect("abrir");

    // Regla 2: con 2 o más capas y una ruta que no es `.aseprite`, Ctrl+S abre el diálogo.
    assert_eq!(
        sesion.save_route(),
        SaveRoute::Direct,
        "una capa y ruta .png"
    );
    anade_capa(&mut sesion, "dos");
    assert_eq!(
        sesion.save_route(),
        SaveRoute::Dialog,
        "dos capas y ruta .png"
    );
    let capas = sesion.document.layers().len();
    assert_eq!(proposed_save_name(sesion.path(), capas), "foto.aseprite");
    assert_eq!(
        dialog_title(DialogKind::Save, capas, sesion.path()),
        "Guardar como .aseprite (con capas) o PNG (aplanado)"
    );
    // Sin ruta, siempre diálogo; con ruta `.aseprite`, directo aunque haya muchas capas.
    assert_eq!(sesion_vacia().save_route(), SaveRoute::Dialog);
    let aseprite = copia_del_corpus(&tmp, "capas-normal-tres", "tres.ASEPRITE");
    let mut otra = sesion_vacia();
    otra.open(&aseprite, 16384, false).expect("abrir");
    assert_eq!(
        otra.save_route(),
        SaveRoute::Direct,
        "ruta .aseprite sin mirar mayúsculas"
    );

    // Los nombres propuestos y los títulos de los diálogos (todas las combinaciones de la spec).
    let ruta_png = Path::new("/d/x.png");
    let ase = Path::new("/d/x.aseprite");
    assert_eq!(proposed_save_name(None, 2), "sin-titulo.aseprite");
    assert_eq!(proposed_save_name(None, 1), "sin-titulo.png");
    assert_eq!(proposed_save_name(Some(ruta_png), 2), "x.aseprite");
    assert_eq!(proposed_save_name(Some(ruta_png), 1), "x.png");
    assert_eq!(proposed_save_name(Some(ase), 1), "x.aseprite");
    assert_eq!(
        dialog_title(DialogKind::Open, 1, None),
        "Abrir imagen (PNG o Aseprite)"
    );
    let con_capas = "Guardar como .aseprite (con capas) o PNG (aplanado)";
    assert_eq!(
        dialog_title(DialogKind::SaveAs, 3, Some(ruta_png)),
        con_capas
    );
    assert_eq!(dialog_title(DialogKind::Save, 2, None), con_capas);
    assert_eq!(
        dialog_title(DialogKind::Save, 1, Some(ase)),
        "Guardar .aseprite"
    );
    assert_eq!(
        dialog_title(DialogKind::SaveAs, 1, Some(ruta_png)),
        "Guardar PNG"
    );
    assert_eq!(dialog_title(DialogKind::Save, 1, None), "Guardar PNG");

    // Si en ese diálogo se elige un `.png`, se guarda aplanado y el aviso lo dice.
    let guardado = sesion
        .save_to(&tmp.ruta().join("plano.png"))
        .expect("guardar el PNG");
    assert!(es_png(&fs::read(&guardado.path).expect("leer")));
    let aviso = with_flatten_notice(describe_saved_dialog(&guardado), &sesion.document);
    assert!(aviso.contains("se guarda aplanado"), "{aviso}");
    assert!(aviso.contains("conservas las capas"), "{aviso}");
}
