//! Piezas comunes de la puerta del hito 2 (specs/hito-2-puerta.md): directorio de trabajo,
//! fuentes, modelo independiente, comparador, instantánea de `golden/png/`, `FakePicker` propio y
//! la ayuda `fuera_de_golden`.
//!
//! **Modelo independiente.** `Modelo` es una rejilla de celdas más una paleta; aplica las mismas
//! ediciones que la app con código ingenuo (relleno por cola, línea solo horizontal, vertical o a
//! 45°, donde cualquier algoritmo coincide). Lo esperado se calcula siempre con el modelo: nunca con
//! `Image`, `Session` ni `px-raster` (de `Image` solo se *lee* para comparar y para sembrar el modelo
//! con lo que se abrió).
//!
//! **Suposiciones y ambigüedades.**
//! - Las ranuras de color (1 a 9) y el valor del borrador se copian del comportamiento documentado en
//!   las specs del lápiz y del borrador: son parte del modelo, no se piden a `px-ui`.
//! - Una conversión de modo deja el transparente canónico `(0, 0, 0, 0)` en todo píxel de alfa 0 (spec
//!   de la conversión de modos); el modelo lo repite.
//! - «`converted()` coincide con que el informe o `aux` de `read_png` no esté vacío» se lee como
//!   «el informe no está vacío o `aux.appearance` es `Some`»: los chunks auxiliares inocuos (`pHYs`,
//!   `tEXt`…) no fuerzan copia.
//! - Los reales no tienen lectura independiente propia: la referencia independiente es el `.sha256`
//!   de su RGBA expandida (calculado con otra herramienta); después se siembra el modelo con lo
//!   abierto.

pub mod sha256;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use px_core::{Allowed, ConvertError, Document, DocumentError, Image, Mode, Palette, Rgba};
use px_formats::{read_png, write_png};
use px_ui::canvas::{Canvas, CanvasEvent};
use px_ui::color::ColorChoice;
use px_ui::color_edit::{Commit, EditTarget};
use px_ui::palette_ops::PaletteOp;
use px_ui::tool::{Tool, ToolAction, ToolEvent, ToolState};

use crate::dialogs::{DialogRequest, Dialogs, FilePicker};
use crate::files::FileError;
use crate::session::{Saved, Session};

use sha256::sha256_hex;

// ---------------------------------------------------------------------------------------
// Rutas, directorio de trabajo y `fuera_de_golden`.
// ---------------------------------------------------------------------------------------

/// Los cuatro PNG reales de `golden/png/`.
pub const REALES: [&str; 4] = [
    "grafx2-golden1.png",
    "grafx2-golden2.png",
    "libresprite-golden-indexed.png",
    "libresprite-golden-rgba.png",
];

pub fn raiz_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

pub fn golden_png() -> PathBuf {
    raiz_repo().join("golden").join("png")
}

/// `<repo>/target/pixoko-gate`: en disco, no en `/tmp` (tmpfs) y en `.gitignore` (`/target`).
fn base_trabajo() -> PathBuf {
    raiz_repo().join("target").join("pixoko-gate")
}

static CONTADOR: AtomicU64 = AtomicU64::new(0);
/// Crear y borrar directorios de trabajo no puede cruzarse con el borrado de la carpeta madre.
static CERROJO: Mutex<()> = Mutex::new(());

/// Directorio de trabajo propio de una prueba (`<repo>/target/pixoko-gate/<pid>-<n>`). El guardián
/// lo borra al soltarse, también si la prueba falla, y comprueba entonces que `golden/png/` sigue
/// intacto (salvo que la prueba ya esté en pánico).
pub struct Trabajo {
    ruta: PathBuf,
}

impl Trabajo {
    pub fn nuevo() -> Trabajo {
        // La instantánea inicial de golden/png se toma antes de que nada pueda escribir.
        inicial();
        let ruta = {
            let _cerrojo = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
            let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
            let ruta = base_trabajo().join(format!("{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&ruta);
            fs::create_dir_all(&ruta).expect("crear el directorio de trabajo");
            fs::canonicalize(&ruta).expect("canonicalizar el directorio de trabajo")
        };
        fuera_de_golden(&ruta);
        Trabajo { ruta }
    }

    pub fn ruta(&self) -> &Path {
        &self.ruta
    }

    /// Deja la fuente en el directorio de trabajo: los reales con `fs::copy`, el resto escritos.
    pub fn poner(&self, fuente: &Fuente) -> PathBuf {
        let destino = self.ruta.join(&fuente.nombre);
        fuera_de_golden(&destino);
        if fuente.real {
            fs::copy(golden_png().join(&fuente.nombre), &destino)
                .unwrap_or_else(|e| panic!("copiar {} al trabajo: {e}", fuente.nombre));
        } else {
            fs::write(&destino, &fuente.bytes)
                .unwrap_or_else(|e| panic!("escribir {}: {e}", destino.display()));
        }
        destino
    }

    /// Escribe un fichero en el directorio de trabajo.
    pub fn escribir(&self, nombre: &str, bytes: &[u8]) -> PathBuf {
        let destino = self.ruta.join(nombre);
        fuera_de_golden(&destino);
        fs::write(&destino, bytes)
            .unwrap_or_else(|e| panic!("escribir {}: {e}", destino.display()));
        destino
    }

    /// Nombres de lo que hay en el directorio de trabajo, ordenados.
    pub fn nombres(&self) -> Vec<String> {
        nombres_en(&self.ruta)
    }
}

impl Drop for Trabajo {
    fn drop(&mut self) {
        {
            let _cerrojo = CERROJO.lock().unwrap_or_else(|e| e.into_inner());
            let _ = fs::remove_dir_all(&self.ruta);
            // Solo se borra si ya no queda ningún otro directorio de trabajo.
            let _ = fs::remove_dir(base_trabajo());
        }
        if !std::thread::panicking() {
            golden_intacto();
        }
    }
}

/// Nombres de lo que hay en `dir`, ordenados.
pub fn nombres_en(dir: &Path) -> Vec<String> {
    let mut nombres: Vec<String> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("leer {}: {e}", dir.display()))
        .map(|entrada| {
            entrada
                .expect("entrada de directorio")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    nombres.sort();
    nombres
}

/// Que nada con `.pixoko-tmp` en el nombre quede en `nombres`.
pub fn sin_temporales(nombres: &[String]) {
    assert!(
        nombres.iter().all(|n| !n.contains(".pixoko-tmp")),
        "quedó un temporal de write_atomic: {nombres:?}"
    );
}

/// La ruta con sus enlaces y `..` resueltos: se canonicaliza el ancestro más largo que exista y se
/// le añade el resto (la ruta puede no existir todavía).
fn resolver(ruta: &Path) -> PathBuf {
    let absoluta = std::path::absolute(ruta).unwrap_or_else(|_| ruta.to_path_buf());
    let mut resto: Vec<OsString> = Vec::new();
    let mut actual = absoluta.clone();
    loop {
        if let Ok(canonica) = fs::canonicalize(&actual) {
            let mut resuelta = canonica;
            for componente in resto.iter().rev() {
                resuelta.push(componente);
            }
            return resuelta;
        }
        match (actual.parent(), actual.file_name()) {
            (Some(padre), Some(nombre)) => {
                resto.push(nombre.to_os_string());
                actual = padre.to_path_buf();
            }
            _ => return absoluta,
        }
    }
}

/// Pánico si `ruta` (de escritura) cuelga de `golden/png/`. Se le pasa **toda** ruta de escritura.
pub fn fuera_de_golden(ruta: &Path) {
    let prohibida = resolver(&golden_png());
    let destino = resolver(ruta);
    assert!(
        !destino.starts_with(&prohibida),
        "escritura prohibida en golden/png: {} (resuelta a {})",
        ruta.display(),
        destino.display()
    );
}

/// `Session::save` con la comprobación de `fuera_de_golden` sobre el directorio y sobre la ruta del
/// documento (el destino es esa ruta o su copia, en el mismo directorio).
pub fn guardar(s: &mut Session, dir: &Path) -> Result<Saved, FileError> {
    fuera_de_golden(dir);
    if let Some(ruta) = s.path() {
        fuera_de_golden(ruta);
    }
    s.save(dir)
}

/// `Session::save_as` con la misma comprobación.
pub fn guardar_como(s: &mut Session, dir: &Path) -> Result<Saved, FileError> {
    fuera_de_golden(dir);
    if let Some(ruta) = s.path() {
        fuera_de_golden(ruta);
    }
    s.save_as(dir)
}

/// `Session::save_to` con la comprobación sobre la ruta elegida. La ruta del documento puede ser un
/// real abierto directamente (solo lectura): `save_to` solo escribe junto a ella si la elegida es
/// ella misma, y eso ya lo recoge la comprobación de la elegida.
pub fn guardar_en(s: &mut Session, elegido: &Path) -> Result<Saved, FileError> {
    fuera_de_golden(elegido);
    s.save_to(elegido)
}

/// Primer nombre libre `{nombre}-copia.png`, `{nombre}-copia-2.png`… junto a `original`, calculado
/// aquí (independiente de `copy_path`).
pub fn libre_copia(original: &Path) -> PathBuf {
    let stem = original
        .file_stem()
        .expect("el original tiene nombre")
        .to_string_lossy()
        .into_owned();
    let dir = original.parent().expect("el original tiene directorio");
    let mut n = 1u32;
    loop {
        let nombre = if n == 1 {
            format!("{stem}-copia.png")
        } else {
            format!("{stem}-copia-{n}.png")
        };
        let candidato = dir.join(nombre);
        if fs::symlink_metadata(&candidato).is_err() {
            return candidato;
        }
        n += 1;
    }
}

/// Añade `.png` salvo que la extensión ya sea `png` (sin distinguir mayúsculas), como un diálogo.
pub fn con_png(ruta: &Path) -> PathBuf {
    let es_png = ruta
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if es_png {
        return ruta.to_path_buf();
    }
    let mut nombre = ruta.as_os_str().to_os_string();
    nombre.push(".png");
    PathBuf::from(nombre)
}

// ---------------------------------------------------------------------------------------
// Instantánea de golden/png/.
// ---------------------------------------------------------------------------------------

/// Nombre y bytes de cada fichero de una carpeta (un subdirectorio, con barra final y sin bytes).
pub type Instantanea = BTreeMap<String, Vec<u8>>;

pub fn instantanea_de(dir: &Path) -> Instantanea {
    let mut mapa = Instantanea::new();
    for entrada in fs::read_dir(dir).unwrap_or_else(|e| panic!("leer {}: {e}", dir.display())) {
        let entrada = entrada.expect("entrada de directorio");
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        if entrada.path().is_dir() {
            mapa.insert(format!("{nombre}/"), Vec::new());
        } else {
            let bytes = fs::read(entrada.path())
                .unwrap_or_else(|e| panic!("leer {}: {e}", entrada.path().display()));
            mapa.insert(nombre, bytes);
        }
    }
    mapa
}

/// `Err` con todos los problemas si `despues` no es `antes`: un fichero que falta, que aparece o
/// con algún byte distinto.
pub fn comparar_instantaneas(antes: &Instantanea, despues: &Instantanea) -> Result<(), String> {
    let mut problemas = Vec::new();
    for (nombre, bytes) in antes {
        match despues.get(nombre) {
            None => problemas.push(format!("falta {nombre}")),
            Some(otros) if otros != bytes => {
                let primera = bytes
                    .iter()
                    .zip(otros)
                    .position(|(a, b)| a != b)
                    .unwrap_or(bytes.len().min(otros.len()));
                problemas.push(format!(
                    "{nombre} cambió ({} → {} bytes, primera diferencia en el byte {primera})",
                    bytes.len(),
                    otros.len()
                ));
            }
            Some(_) => {}
        }
    }
    for nombre in despues.keys() {
        if !antes.contains_key(nombre) {
            problemas.push(format!("aparece {nombre}"));
        }
    }
    if problemas.is_empty() {
        Ok(())
    } else {
        Err(problemas.join("; "))
    }
}

static INICIAL: OnceLock<Instantanea> = OnceLock::new();

/// La instantánea de `golden/png/` al empezar la puerta: la toma la primera prueba que la pide (todas
/// las que usan golden la piden antes de escribir nada, al crear su `Trabajo`).
pub fn inicial() -> &'static Instantanea {
    INICIAL.get_or_init(|| instantanea_de(&golden_png()))
}

/// Pánico si `golden/png/` ya no es la instantánea inicial.
pub fn golden_intacto() {
    let ahora = instantanea_de(&golden_png());
    if let Err(problemas) = comparar_instantaneas(inicial(), &ahora) {
        panic!("golden/png/ ya no es el de antes de la puerta: {problemas}");
    }
}

// ---------------------------------------------------------------------------------------
// Generador pseudoaleatorio propio y PNG a mano.
// ---------------------------------------------------------------------------------------

/// Congruencial lineal de 64 bits: bastan unos píxeles repetibles, sin dependencias.
pub struct Lcg(u64);

impl Lcg {
    pub fn new(semilla: u64) -> Lcg {
        Lcg(semilla ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn siguiente(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }

    /// Un valor en `0..n` (con `n > 0`).
    pub fn hasta(&mut self, n: u32) -> u32 {
        self.siguiente() % n
    }
}

/// CRC-32 de la especificación PNG.
fn crc32(datos: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in datos {
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
fn adler32(datos: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in datos {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Flujo zlib con un único bloque deflate «stored» (datos pequeños, menos de 65 536 bytes).
pub fn zlib_stored(datos: &[u8]) -> Vec<u8> {
    let mut v = vec![0x78, 0x01, 1];
    let n = u16::try_from(datos.len()).expect("datos pequeños");
    v.extend(n.to_le_bytes());
    v.extend((!n).to_le_bytes());
    v.extend(datos);
    v.extend(adler32(datos).to_be_bytes());
    v
}

/// Chunk completo: longitud, tipo, datos y CRC.
pub fn trozo(tipo: &[u8; 4], datos: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + datos.len());
    v.extend((datos.len() as u32).to_be_bytes());
    v.extend(tipo);
    v.extend(datos);
    let mut cuerpo = tipo.to_vec();
    cuerpo.extend(datos);
    v.extend(crc32(&cuerpo).to_be_bytes());
    v
}

/// Empaqueta las muestras de una fila: sub-8 bits de más a menos significativo con relleno a byte,
/// 8 bits tal cual y 16 bits en big-endian.
fn empaqueta(fila: &[u16], prof: u8) -> Vec<u8> {
    match prof {
        16 => fila.iter().flat_map(|v| v.to_be_bytes()).collect(),
        8 => fila.iter().map(|&v| v as u8).collect(),
        _ => {
            let p = usize::from(prof);
            fila.chunks(8 / p)
                .map(|grupo| {
                    let mut byte = 0u8;
                    for (i, &v) in grupo.iter().enumerate() {
                        byte |= (v as u8) << (8 - p * (i + 1));
                    }
                    byte
                })
                .collect()
        }
    }
}

/// PNG entero sin entrelazar (chunks, CRC y deflate «stored» a mano): `muestras` va fila a fila y
/// píxel a píxel, con los canales del tipo de color; `antes` son chunks ya serializados entre IHDR e
/// IDAT. Es la forma de `session/aux_tests.rs`, sin importarla.
pub fn png_crudo(
    ancho: u32,
    alto: u32,
    prof: u8,
    color: u8,
    muestras: &[u16],
    antes: &[Vec<u8>],
) -> Vec<u8> {
    let canales = match color {
        2 => 3,
        4 => 2,
        6 => 4,
        _ => 1,
    };
    let por_fila = ancho as usize * canales;
    let mut crudo = Vec::new();
    for fila in muestras.chunks(por_fila) {
        crudo.push(0);
        crudo.extend(empaqueta(fila, prof));
    }
    let mut ihdr = Vec::new();
    ihdr.extend(ancho.to_be_bytes());
    ihdr.extend(alto.to_be_bytes());
    ihdr.extend([prof, color, 0, 0, 0]);
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend(trozo(b"IHDR", &ihdr));
    for c in antes {
        v.extend(c);
    }
    v.extend(trozo(b"IDAT", &zlib_stored(&crudo)));
    v.extend(trozo(b"IEND", &[]));
    v
}

/// Inserta chunks justo tras el IHDR (que acaba en el byte 33) de un PNG de `write_png`.
pub fn con_chunks(bytes: &[u8], antes: &[Vec<u8>]) -> Vec<u8> {
    let mut v = bytes[..33].to_vec();
    for c in antes {
        v.extend(c);
    }
    v.extend(&bytes[33..]);
    v
}

/// Chunk `iCCP` con un perfil de mentira (`read_png` no lo interpreta: solo lo anota).
pub fn iccp() -> Vec<u8> {
    let mut datos = b"perfil\0\0".to_vec();
    datos.extend(zlib_stored(b"datos del perfil"));
    trozo(b"iCCP", &datos)
}

/// `(ancho, alto, profundidad, tipo de color)` del IHDR.
pub fn ihdr(bytes: &[u8]) -> (u32, u32, u8, u8) {
    let ancho = u32::from_be_bytes(bytes[16..20].try_into().expect("ancho"));
    let alto = u32::from_be_bytes(bytes[20..24].try_into().expect("alto"));
    (ancho, alto, bytes[24], bytes[25])
}

/// Tipos de los chunks de un fichero bien formado, en orden.
pub fn tipos_de_chunks(bytes: &[u8]) -> Vec<[u8; 4]> {
    let mut tipos = Vec::new();
    let mut pos = 8usize;
    while pos + 8 <= bytes.len() {
        let largo = u32::from_be_bytes(bytes[pos..pos + 4].try_into().expect("longitud")) as usize;
        let tipo: [u8; 4] = bytes[pos + 4..pos + 8].try_into().expect("tipo");
        tipos.push(tipo);
        pos += 12 + largo;
    }
    tipos
}

/// El modo que un lector debe dar al fichero (de su IHDR y de si lleva `tRNS`), sin leer los píxeles.
pub fn modo_del_fichero(bytes: &[u8]) -> Mode {
    let (_, _, _, color) = ihdr(bytes);
    match color {
        3 => Mode::Indexed,
        2 | 6 => Mode::Rgba,
        4 => Mode::GrayAlpha,
        0 => {
            if tipos_de_chunks(bytes).contains(b"tRNS") {
                Mode::GrayAlpha
            } else {
                Mode::Gray
            }
        }
        otro => panic!("tipo de color PNG inesperado: {otro}"),
    }
}

// ---------------------------------------------------------------------------------------
// El modelo.
// ---------------------------------------------------------------------------------------

/// Lo que elige el color de un gesto: una ranura (1 a 9) o una entrada de la paleta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eleccion {
    Ranura(u8),
    Entrada(u8),
}

fn a_color_choice(e: Eleccion) -> ColorChoice {
    match e {
        Eleccion::Ranura(n) => ColorChoice::Slot(n),
        Eleccion::Entrada(i) => ColorChoice::Entry(i),
    }
}

/// Un paso del guion: un gesto de una herramienta, una operación de la paleta o una conversión.
#[derive(Clone, Debug)]
pub enum Paso {
    /// Trayecto del puntero (cada tramo, horizontal, vertical o a 45°) y color.
    Lapiz(Vec<(i32, i32)>, Eleccion),
    Linea((i32, i32), (i32, i32), Eleccion),
    Relleno((i32, i32), Eleccion),
    Borrador(Vec<(i32, i32)>),
    /// Edita el color de una entrada de la paleta.
    ColorEntrada(u8, [u8; 4]),
    Rotar {
        inicio: u8,
        largo: u16,
        pasos: i32,
    },
    Mover {
        de: u8,
        a: u8,
    },
    /// Añade una copia del color resuelto de la entrada activa.
    Anadir {
        activa: u8,
    },
    Transparente(Option<u8>),
    /// La conversión exacta del modo actual: indexado a RGBA, gris a gris con alfa, gris con alfa a
    /// RGBA y RGBA a indexado.
    Convertir,
}

/// Lo que el modelo dice de un paso.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Efecto {
    /// El documento debe cambiar (y entonces hay un paso de historia nuevo).
    pub cambio: bool,
    /// El borrador debe rechazar el gesto (aviso `Refused`).
    pub rechazado: bool,
    /// La conversión debe fallar con `TooManyColors`, sin cambiar nada.
    pub error: bool,
}

/// Rejilla de píxeles y paleta. Cada celda son 4 bytes con el significado del modo: RGBA `[r, g, b,
/// a]`, gris con alfa `[g, a, 0, 0]`, gris `[g, 0, 0, 0]` e indexado `[índice, 0, 0, 0]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Modelo {
    pub modo: Mode,
    pub ancho: usize,
    pub alto: usize,
    pub celdas: Vec<[u8; 4]>,
    /// Colores guardados de la paleta (solo indexado).
    pub paleta: Vec<[u8; 4]>,
    pub transparente: Option<u8>,
}

fn bytes_por_pixel(modo: Mode) -> usize {
    match modo {
        Mode::Rgba => 4,
        Mode::GrayAlpha => 2,
        Mode::Gray | Mode::Indexed => 1,
    }
}

/// El transparente canónico de una conversión: todo alfa 0 pasa a `(0, 0, 0, 0)`.
fn canonico(c: [u8; 4]) -> [u8; 4] {
    if c[3] == 0 { [0, 0, 0, 0] } else { c }
}

/// Segmento de `a` a `b` incluidos ambos extremos; solo horizontal, vertical o a 45°.
fn segmento(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    assert!(
        dx == 0 || dy == 0 || dx.abs() == dy.abs(),
        "el modelo solo admite segmentos H, V o a 45°: {a:?} -> {b:?}"
    );
    let n = dx.abs().max(dy.abs());
    (0..=n)
        .map(|i| (a.0 + i * dx.signum(), a.1 + i * dy.signum()))
        .collect()
}

/// Píxeles de un trazo del lápiz: el primero y los segmentos entre posiciones sucesivas.
fn trazo(ruta: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut puntos = Vec::new();
    let Some(&primero) = ruta.first() else {
        return puntos;
    };
    puntos.push(primero);
    let mut ultimo = primero;
    for &siguiente in &ruta[1..] {
        if siguiente == ultimo {
            continue;
        }
        puntos.extend(segmento(ultimo, siguiente));
        ultimo = siguiente;
    }
    puntos
}

impl Modelo {
    fn indice(&self, x: i32, y: i32) -> Option<usize> {
        let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
            return None;
        };
        (x < self.ancho && y < self.alto).then_some(y * self.ancho + x)
    }

    /// Los bytes en el formato de `Image::bytes`.
    pub fn bytes(&self) -> Vec<u8> {
        let n = bytes_por_pixel(self.modo);
        self.celdas
            .iter()
            .flat_map(|c| c[..n].iter().copied())
            .collect()
    }

    /// Color de una entrada de la paleta con el índice transparente resuelto (alfa 0).
    pub fn resuelto(&self, i: u8) -> [u8; 4] {
        let mut color = self.paleta[usize::from(i)];
        if self.transparente == Some(i) {
            color[3] = 0;
        }
        color
    }

    /// RGBA de 8 bits expandido, como el `.sha256` del corpus.
    pub fn rgba(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.celdas.len() * 4);
        for c in &self.celdas {
            match self.modo {
                Mode::Rgba => v.extend_from_slice(c),
                Mode::GrayAlpha => v.extend([c[0], c[0], c[0], c[1]]),
                Mode::Gray => v.extend([c[0], c[0], c[0], 255]),
                Mode::Indexed => v.extend(self.resuelto(c[0])),
            }
        }
        v
    }

    /// Lo que devuelve la relectura de un PNG guardado: un indexado con índice transparente vuelve
    /// con la entrada resuelta (alfa 0) y sin índice (comportamiento 11 de la spec de PNG).
    pub fn normalizada(&self) -> Modelo {
        let mut m = self.clone();
        if let Some(t) = m.transparente.take() {
            m.paleta[usize::from(t)][3] = 0;
        }
        m
    }

    /// La `Image` equivalente (solo como entrada de las pruebas, nunca como esperado).
    pub fn a_imagen(&self) -> Image {
        let (ancho, alto) = (self.ancho as u32, self.alto as u32);
        let datos = self.bytes();
        if self.modo == Mode::Indexed {
            let colores = self
                .paleta
                .iter()
                .map(|c| Rgba::new(c[0], c[1], c[2], c[3]));
            let mut paleta = Palette::from_colors(colores).expect("paleta del modelo");
            paleta
                .set_transparent_index(self.transparente)
                .expect("índice transparente del modelo");
            Image::from_bytes_indexed(ancho, alto, paleta, datos).expect("imagen indexada")
        } else {
            Image::from_bytes(ancho, alto, self.modo, datos).expect("imagen")
        }
    }

    /// Siembra el modelo con una imagen ya comprobada contra una referencia independiente.
    pub fn de_imagen(imagen: &Image) -> Modelo {
        let modo = imagen.mode();
        let n = bytes_por_pixel(modo);
        let celdas = imagen
            .bytes()
            .chunks_exact(n)
            .map(|trozo| {
                let mut celda = [0u8; 4];
                celda[..n].copy_from_slice(trozo);
                celda
            })
            .collect();
        let paleta = imagen
            .palette()
            .map(|p| p.colors().iter().map(|c| [c.r, c.g, c.b, c.a]).collect())
            .unwrap_or_default();
        Modelo {
            modo,
            ancho: imagen.width() as usize,
            alto: imagen.height() as usize,
            celdas,
            paleta,
            transparente: imagen.palette().and_then(Palette::transparent_index),
        }
    }

    /// La celda que escribe un color elegido; `None` si no hay color para esa elección.
    pub fn celda_de(&self, e: Eleccion) -> Option<[u8; 4]> {
        match (e, self.modo) {
            (Eleccion::Entrada(i), Mode::Indexed) => {
                (usize::from(i) < self.paleta.len()).then_some([i, 0, 0, 0])
            }
            (Eleccion::Entrada(_), _) => None,
            (Eleccion::Ranura(n), _) if !(1..=9).contains(&n) => None,
            (Eleccion::Ranura(n), Mode::Indexed) => {
                (usize::from(n - 1) < self.paleta.len()).then_some([n - 1, 0, 0, 0])
            }
            (Eleccion::Ranura(n), Mode::Rgba) => Some(match n {
                1 => [0, 0, 0, 255],
                2 => [255, 255, 255, 255],
                3 => [255, 0, 0, 255],
                4 => [0, 255, 0, 255],
                5 => [0, 0, 255, 255],
                6 => [255, 255, 0, 255],
                7 => [255, 0, 255, 255],
                8 => [0, 255, 255, 255],
                _ => [0, 0, 0, 0],
            }),
            (Eleccion::Ranura(n), Mode::Gray) => {
                let gris = if n == 9 { 255 } else { (n - 1) * 32 };
                Some([gris, 0, 0, 0])
            }
            (Eleccion::Ranura(n), Mode::GrayAlpha) => Some(if n == 9 {
                [0, 0, 0, 0]
            } else {
                [(n - 1) * 32, 255, 0, 0]
            }),
        }
    }

    /// Alfa resuelto de la celda `i`.
    fn alfa(&self, i: usize) -> u8 {
        let c = self.celdas[i];
        match self.modo {
            Mode::Rgba => c[3],
            Mode::GrayAlpha => c[1],
            Mode::Gray => 255,
            Mode::Indexed => self.resuelto(c[0])[3],
        }
    }

    /// Lo que escribe el borrador, o `None` si no puede con la imagen: RGBA y gris con alfa, el
    /// transparente canónico; indexado, el índice transparente o, sin él, la primera entrada de alfa
    /// guardado 0; gris, nada.
    fn valor_borrado(&self) -> Option<[u8; 4]> {
        match self.modo {
            Mode::Rgba | Mode::GrayAlpha => Some([0, 0, 0, 0]),
            Mode::Gray => None,
            Mode::Indexed => match self.transparente {
                Some(t) => Some([t, 0, 0, 0]),
                None => self
                    .paleta
                    .iter()
                    .position(|c| c[3] == 0)
                    .map(|i| [i as u8, 0, 0, 0]),
            },
        }
    }

    fn pintar(&mut self, puntos: &[(i32, i32)], celda: [u8; 4]) -> bool {
        let mut cambio = false;
        for &(x, y) in puntos {
            if let Some(i) = self.indice(x, y)
                && self.celdas[i] != celda
            {
                self.celdas[i] = celda;
                cambio = true;
            }
        }
        cambio
    }

    /// Relleno por cola de la región de 4 vecinos con el mismo valor guardado que la semilla.
    fn rellenar(&mut self, semilla: (i32, i32), celda: [u8; 4]) -> bool {
        let Some(inicio) = self.indice(semilla.0, semilla.1) else {
            return false;
        };
        let objetivo = self.celdas[inicio];
        if objetivo == celda {
            return false;
        }
        self.celdas[inicio] = celda;
        let mut cola = VecDeque::from([semilla]);
        while let Some((x, y)) = cola.pop_front() {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let vecino = (x + dx, y + dy);
                if let Some(i) = self.indice(vecino.0, vecino.1)
                    && self.celdas[i] == objetivo
                {
                    self.celdas[i] = celda;
                    cola.push_back(vecino);
                }
            }
        }
        true
    }

    fn borrar(&mut self, ruta: &[(i32, i32)]) -> Efecto {
        let Some(valor) = self.valor_borrado() else {
            return Efecto {
                rechazado: true,
                ..Efecto::default()
            };
        };
        let mut cambio = false;
        for (x, y) in trazo(ruta) {
            if let Some(i) = self.indice(x, y)
                && self.alfa(i) != 0
            {
                self.celdas[i] = valor;
                cambio = true;
            }
        }
        Efecto {
            cambio,
            ..Efecto::default()
        }
    }

    fn convertir(&mut self) -> Efecto {
        match self.modo {
            Mode::Indexed => {
                let nuevas: Vec<[u8; 4]> = self
                    .celdas
                    .iter()
                    .map(|c| canonico(self.resuelto(c[0])))
                    .collect();
                self.celdas = nuevas;
                self.modo = Mode::Rgba;
                self.paleta.clear();
                self.transparente = None;
            }
            Mode::Gray => {
                for c in &mut self.celdas {
                    *c = [c[0], 255, 0, 0];
                }
                self.modo = Mode::GrayAlpha;
            }
            Mode::GrayAlpha => {
                for c in &mut self.celdas {
                    *c = canonico([c[0], c[0], c[0], c[1]]);
                }
                self.modo = Mode::Rgba;
            }
            Mode::Rgba => {
                // El transparente, si lo hay, en el índice 0; el resto por primera aparición.
                let mut colores: Vec<[u8; 4]> = Vec::new();
                let mut hay_transparente = false;
                for c in &self.celdas {
                    if c[3] == 0 {
                        hay_transparente = true;
                    } else if !colores.contains(c) {
                        colores.push(*c);
                    }
                }
                if colores.len() + usize::from(hay_transparente) > 256 {
                    return Efecto {
                        error: true,
                        ..Efecto::default()
                    };
                }
                let desplazamiento = usize::from(hay_transparente);
                let nuevas: Vec<[u8; 4]> = self
                    .celdas
                    .iter()
                    .map(|c| {
                        if c[3] == 0 {
                            [0, 0, 0, 0]
                        } else {
                            let at = colores.iter().position(|k| k == c).expect("registrado");
                            [(at + desplazamiento) as u8, 0, 0, 0]
                        }
                    })
                    .collect();
                let mut paleta = Vec::new();
                if hay_transparente {
                    paleta.push([0, 0, 0, 0]);
                }
                paleta.extend(colores);
                self.celdas = nuevas;
                self.modo = Mode::Indexed;
                self.paleta = paleta;
                self.transparente = hay_transparente.then_some(0);
            }
        }
        Efecto {
            cambio: true,
            ..Efecto::default()
        }
    }

    /// Aplica un paso al modelo y dice qué debe pasar en la sesión. Con un error de conversión, el
    /// modelo no cambia.
    pub fn aplicar(&mut self, paso: &Paso) -> Efecto {
        let por_celda = |cambio: bool| Efecto {
            cambio,
            ..Efecto::default()
        };
        match paso {
            Paso::Lapiz(ruta, e) => match self.celda_de(*e) {
                Some(celda) => por_celda(self.pintar(&trazo(ruta), celda)),
                None => Efecto::default(),
            },
            Paso::Linea(a, b, e) => match self.celda_de(*e) {
                Some(celda) => por_celda(self.pintar(&segmento(*a, *b), celda)),
                None => Efecto::default(),
            },
            Paso::Relleno(semilla, e) => match self.celda_de(*e) {
                Some(celda) => por_celda(self.rellenar(*semilla, celda)),
                None => Efecto::default(),
            },
            Paso::Borrador(ruta) => self.borrar(ruta),
            Paso::ColorEntrada(i, color) => {
                let cambio = self.paleta[usize::from(*i)] != *color;
                self.paleta[usize::from(*i)] = *color;
                por_celda(cambio)
            }
            Paso::Rotar {
                inicio,
                largo,
                pasos,
            } => {
                let (inicio, largo) = (usize::from(*inicio), usize::from(*largo));
                assert!(
                    inicio + largo <= self.paleta.len(),
                    "rango fuera de la paleta"
                );
                if largo < 2 {
                    return Efecto::default();
                }
                let k = pasos.rem_euclid(largo as i32) as usize;
                let original = self.paleta.clone();
                for i in 0..largo {
                    self.paleta[inicio + (i + k) % largo] = original[inicio + i];
                }
                por_celda(self.paleta != original)
            }
            Paso::Mover { de, a } => {
                let (de, a) = (usize::from(*de), usize::from(*a));
                if de == a {
                    return Efecto::default();
                }
                // `orden[nuevo] = antiguo`, como quitar e insertar.
                let mut orden: Vec<usize> = (0..self.paleta.len()).collect();
                let movida = orden.remove(de);
                orden.insert(a, movida);
                let mut adelante = vec![0u8; orden.len()];
                for (nuevo, &antiguo) in orden.iter().enumerate() {
                    adelante[antiguo] = nuevo as u8;
                }
                self.paleta = orden.iter().map(|&antiguo| self.paleta[antiguo]).collect();
                for c in &mut self.celdas {
                    c[0] = adelante[usize::from(c[0])];
                }
                self.transparente = self.transparente.map(|t| adelante[usize::from(t)]);
                por_celda(true)
            }
            Paso::Anadir { activa } => {
                assert!(self.paleta.len() < 256, "la paleta está llena");
                let color = self.resuelto(*activa);
                self.paleta.push(color);
                por_celda(true)
            }
            Paso::Transparente(indice) => {
                let cambio = self.transparente != *indice;
                self.transparente = *indice;
                por_celda(cambio)
            }
            Paso::Convertir => self.convertir(),
        }
    }
}

/// RGBA expandido de una `Image`, leído de sus bytes y su paleta (con el índice transparente
/// resuelto): es la «vista RGBA» del corpus.
pub fn expandir(imagen: &Image) -> Vec<u8> {
    let bytes = imagen.bytes();
    let mut v = Vec::with_capacity(bytes.len() * 4);
    match imagen.mode() {
        Mode::Rgba => v.extend_from_slice(bytes),
        Mode::Gray => {
            for &g in bytes {
                v.extend([g, g, g, 255]);
            }
        }
        Mode::GrayAlpha => {
            for p in bytes.as_chunks::<2>().0 {
                v.extend([p[0], p[0], p[0], p[1]]);
            }
        }
        Mode::Indexed => {
            let paleta = imagen.palette().expect("una indexada tiene paleta");
            let transparente = paleta.transparent_index();
            for &i in bytes {
                let c = paleta.colors()[usize::from(i)];
                let alfa = if transparente == Some(i) { 0 } else { c.a };
                v.extend([c.r, c.g, c.b, alfa]);
            }
        }
    }
    v
}

/// Pánico, con el primer píxel que difiere, si la imagen no es el modelo: modo, tamaño, bytes,
/// paleta, índice transparente y RGBA expandido.
pub fn assert_igual(imagen: &Image, modelo: &Modelo, ctx: &str) {
    assert_eq!(imagen.mode(), modelo.modo, "{ctx}: modo");
    assert_eq!(
        (imagen.width() as usize, imagen.height() as usize),
        (modelo.ancho, modelo.alto),
        "{ctx}: tamaño"
    );
    let esperado = modelo.bytes();
    if imagen.bytes() != &esperado[..] {
        let n = bytes_por_pixel(modelo.modo);
        let pos = imagen
            .bytes()
            .iter()
            .zip(&esperado)
            .position(|(a, b)| a != b)
            .unwrap_or(0)
            / n;
        panic!(
            "{ctx}: el píxel ({}, {}) difiere: imagen {:?}, modelo {:?}",
            pos % modelo.ancho,
            pos / modelo.ancho,
            &imagen.bytes()[pos * n..(pos + 1) * n],
            &esperado[pos * n..(pos + 1) * n]
        );
    }
    let paleta: Vec<[u8; 4]> = imagen
        .palette()
        .map(|p| p.colors().iter().map(|c| [c.r, c.g, c.b, c.a]).collect())
        .unwrap_or_default();
    assert_eq!(paleta, modelo.paleta, "{ctx}: paleta");
    assert_eq!(
        imagen.palette().and_then(Palette::transparent_index),
        modelo.transparente,
        "{ctx}: índice transparente"
    );
    assert!(
        expandir(imagen) == modelo.rgba(),
        "{ctx}: el RGBA expandido no coincide con el del modelo"
    );
}

/// SHA-256 de la RGBA expandida de una imagen (la técnica del corpus).
pub fn sha_rgba(imagen: &Image) -> String {
    sha256_hex(&expandir(imagen))
}

// ---------------------------------------------------------------------------------------
// Fuentes.
// ---------------------------------------------------------------------------------------

/// Un PNG de partida de la puerta.
pub struct Fuente {
    pub nombre: String,
    /// Los bytes del fichero.
    pub bytes: Vec<u8>,
    /// Es uno de `golden/png/` (se copia con `fs::copy`).
    pub real: bool,
    /// Lo que debe dar su apertura (generadas y construidas); los reales, `None`.
    pub esperado: Option<Modelo>,
    /// SHA-256 de la RGBA expandida que dice su `.sha256` (solo los reales).
    pub sha: Option<String>,
    /// Si Pixoko debe convertirlo al abrir, cuando se sabe por construcción.
    pub convertida: Option<bool>,
}

/// Los 4 reales: su esperado es el `.sha256` que dejó otra herramienta.
pub fn reales() -> Vec<Fuente> {
    REALES
        .iter()
        .map(|nombre| {
            let ruta = golden_png().join(nombre);
            let bytes = fs::read(&ruta).unwrap_or_else(|e| panic!("leer {}: {e}", ruta.display()));
            let ruta_sha = golden_png().join(format!("{nombre}.sha256"));
            let sha = fs::read_to_string(&ruta_sha)
                .unwrap_or_else(|e| panic!("leer {}: {e}", ruta_sha.display()))
                .trim()
                .to_ascii_lowercase();
            Fuente {
                nombre: (*nombre).to_string(),
                bytes,
                real: true,
                esperado: None,
                sha: Some(sha),
                convertida: None,
            }
        })
        .collect()
}

/// Una fuente construida: sus bytes se escriben tal cual y se espera `esperado` al abrir.
fn construida(nombre: &str, bytes: Vec<u8>, esperado: Modelo, convertida: bool) -> Fuente {
    Fuente {
        nombre: nombre.to_string(),
        bytes,
        real: false,
        esperado: Some(esperado),
        sha: None,
        convertida: Some(convertida),
    }
}

const ANCHO_GENERADO: usize = 37;
const ALTO_GENERADO: usize = 23;

fn generar(modo: Mode, celda: impl Fn(usize, usize) -> [u8; 4]) -> Modelo {
    let mut celdas = Vec::new();
    for y in 0..ALTO_GENERADO {
        for x in 0..ANCHO_GENERADO {
            celdas.push(celda(x, y));
        }
    }
    Modelo {
        modo,
        ancho: ANCHO_GENERADO,
        alto: ALTO_GENERADO,
        celdas,
        paleta: Vec::new(),
        transparente: None,
    }
}

/// Una fuente generada con `write_png` desde el modelo; al abrir debe dar el modelo normalizado.
fn generada(nombre: &str, modelo: Modelo) -> Fuente {
    let bytes = write_png(&modelo.a_imagen()).expect("write_png de la fuente generada");
    construida(nombre, bytes, modelo.normalizada(), false)
}

/// Las cuatro generadas de 37×23 (que no es múltiplo de nada).
pub fn generadas() -> Vec<Fuente> {
    // RGBA con píxeles de alfa 0 (con RGB distinto de cero) y de alfa 128.
    let tabla = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 128],
        [250, 250, 10, 0],
        [10, 200, 200, 128],
        [90, 90, 90, 255],
    ];
    let rgba = generar(Mode::Rgba, |x, y| tabla[(x + 2 * y) % tabla.len()]);
    // Indexada de 5 colores con alfa propio y el índice 2 transparente.
    let mut indexada = generar(Mode::Indexed, |x, y| {
        [((x * 3 + y * 5 + x / 4) % 5) as u8, 0, 0, 0]
    });
    indexada.paleta = vec![
        [10, 20, 30, 255],
        [200, 100, 50, 128],
        [0, 255, 0, 255],
        [255, 255, 0, 64],
        [255, 0, 255, 255],
    ];
    indexada.transparente = Some(2);
    let gris = generar(Mode::Gray, |x, y| [((x * 5 + y * 3) % 256) as u8, 0, 0, 0]);
    let gris_alfa = generar(Mode::GrayAlpha, |x, y| {
        let alfa = [0, 100, 200, 255][(x + 2 * y) % 4];
        [((x * 7 + y * 2) % 256) as u8, alfa, 0, 0]
    });
    vec![
        generada("generada-rgba.png", rgba),
        generada("generada-indexada.png", indexada),
        generada("generada-gris.png", gris),
        generada("generada-gris-alfa.png", gris_alfa),
    ]
}

/// Reales y generadas.
pub fn todas() -> Vec<Fuente> {
    let mut fuentes = reales();
    fuentes.extend(generadas());
    fuentes
}

/// Un modelo pequeño de celdas dadas.
pub fn modelo_de(modo: Mode, ancho: usize, alto: usize, celdas: Vec<[u8; 4]>) -> Modelo {
    assert_eq!(celdas.len(), ancho * alto);
    Modelo {
        modo,
        ancho,
        alto,
        celdas,
        paleta: Vec::new(),
        transparente: None,
    }
}

/// PNG RGB de 8 bits de 4×2 (Pixoko lo lee como RGBA).
pub fn fuente_rgb8() -> Fuente {
    let pixeles: [[u8; 3]; 8] = [
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [10, 20, 30],
        [200, 100, 50],
        [1, 2, 3],
        [255, 255, 255],
        [0, 0, 0],
    ];
    let muestras: Vec<u16> = pixeles.iter().flatten().map(|&v| u16::from(v)).collect();
    let celdas = pixeles.iter().map(|p| [p[0], p[1], p[2], 255]).collect();
    construida(
        "rgb8.png",
        png_crudo(4, 2, 8, 2, &muestras, &[]),
        modelo_de(Mode::Rgba, 4, 2, celdas),
        true,
    )
}

/// PNG de gris de 4 bits de 4×2 (se amplía a 8: `v · 17`).
pub fn fuente_gris4() -> Fuente {
    let muestras: [u16; 8] = [0, 1, 5, 15, 7, 8, 9, 10];
    let celdas = muestras
        .iter()
        .map(|&v| [(v * 17) as u8, 0, 0, 0])
        .collect();
    construida(
        "gris4.png",
        png_crudo(4, 2, 4, 0, &muestras, &[]),
        modelo_de(Mode::Gray, 4, 2, celdas),
        true,
    )
}

/// PNG de gris de 16 bits de 3×2 con `tRNS` de color clave: las muestras son múltiplos de 257, así
/// que la reducción a 8 bits es exacta (`v / 257`); el píxel de la clave pasa a alfa 0.
pub fn fuente_gris16_trns() -> Fuente {
    let clave = 0x8080u16;
    let muestras: [u16; 6] = [0x0000, clave, 0xFFFF, 0x1010, clave, 0x4242];
    let celdas = muestras
        .iter()
        .map(|&v| [(v / 257) as u8, if v == clave { 0 } else { 255 }, 0, 0])
        .collect();
    construida(
        "gris16-trns.png",
        png_crudo(
            3,
            2,
            16,
            0,
            &muestras,
            &[trozo(b"tRNS", &clave.to_be_bytes())],
        ),
        modelo_de(Mode::GrayAlpha, 3, 2, celdas),
        true,
    )
}

/// PNG RGBA de 5×3 de `write_png` con un `iCCP` (la apariencia cambiaría al guardar sobre él).
pub fn fuente_con_iccp() -> Fuente {
    let celdas = (0..15u8)
        .map(|i| {
            [
                i * 16,
                255 - i * 10,
                i * 3,
                if i % 4 == 0 { 128 } else { 255 },
            ]
        })
        .collect();
    let modelo = modelo_de(Mode::Rgba, 5, 3, celdas);
    let bytes = con_chunks(
        &write_png(&modelo.a_imagen()).expect("write_png"),
        &[iccp()],
    );
    construida("con-iccp.png", bytes, modelo, true)
}

/// Los PNG construidos en la prueba que Pixoko convierte al abrir.
pub fn convertidas() -> Vec<Fuente> {
    vec![
        fuente_rgb8(),
        fuente_gris4(),
        fuente_gris16_trns(),
        fuente_con_iccp(),
    ]
}

/// Un modelo RGBA de 37×23 con más de 256 colores distintos (para `TooManyColors`).
pub fn rgba_de_muchos_colores() -> Modelo {
    generar(Mode::Rgba, |x, y| {
        [(x * 6) as u8, (y * 10) as u8, ((x + y) * 3) as u8, 255]
    })
}

// ---------------------------------------------------------------------------------------
// Sesión, mundo (sesión + modelo + historia anotada) y guiones.
// ---------------------------------------------------------------------------------------

/// Una sesión sin fichero, de la que se parte para abrir.
pub fn sesion_vacia() -> Session {
    let imagen = Image::new(1, 1, Mode::Rgba).expect("imagen de 1×1");
    Session::sample(Document::new(imagen))
}

/// Sesión, herramientas, modelo y la historia anotada: el `state_id` y el modelo de cada paso.
pub struct Mundo {
    pub s: Session,
    pub tools: ToolState,
    pub m: Modelo,
    /// `snaps[n]`: `state_id` y modelo tras `n` pasos de la rama actual.
    pub snaps: Vec<(u64, Modelo)>,
    /// Posición actual en `snaps` (cambia con `undo` y `redo`).
    pub pos: usize,
    /// Todos los `state_id` que ha tenido el documento: no se reutiliza ninguno.
    pub usados: HashSet<u64>,
    /// `state_id` y modelo del último estado guardado (el de apertura, al principio).
    pub guardado: (u64, Modelo),
    /// Cuántos gestos del borrador se rechazaron.
    pub rechazos: usize,
    pub etiqueta: String,
}

impl Mundo {
    pub fn de_sesion(s: Session, etiqueta: &str) -> Mundo {
        let imagen = s
            .document
            .image(s.document.main_image())
            .expect("el documento tiene imagen");
        let m = Modelo::de_imagen(imagen);
        let id = s.document.state_id();
        Mundo {
            s,
            tools: ToolState::new(),
            snaps: vec![(id, m.clone())],
            pos: 0,
            usados: HashSet::from([id]),
            guardado: (id, m.clone()),
            m,
            rechazos: 0,
            etiqueta: etiqueta.to_string(),
        }
    }

    pub fn imagen(&self) -> &Image {
        self.s
            .document
            .image(self.s.document.main_image())
            .expect("el documento tiene imagen")
    }

    /// Pasos de historia de la rama actual.
    pub fn pasos(&self) -> usize {
        self.snaps.len() - 1
    }

    fn gesto(
        &mut self,
        herramienta: Tool,
        eleccion: Eleccion,
        eventos: Vec<CanvasEvent>,
    ) -> (bool, bool) {
        let id = self.s.document.main_image();
        self.tools.apply_action(ToolAction::Select(herramienta));
        let mut cambio = false;
        for evento in eventos {
            cambio |= self
                .tools
                .canvas_event(&mut self.s.document, id, a_color_choice(eleccion), evento)
                .unwrap_or_else(|e| panic!("{}: el gesto dio error: {e}", self.etiqueta));
        }
        let cambios = self.tools.take_changes();
        self.s.absorb(cambios);
        let rechazado = self
            .tools
            .take_events()
            .iter()
            .any(|e| matches!(e, ToolEvent::Refused(_)));
        (cambio, rechazado)
    }

    fn operacion(&mut self, op: PaletteOp, activa: u8) -> bool {
        self.s.choice = ColorChoice::Entry(activa);
        self.s
            .apply_palette_op(op, false)
            .unwrap_or_else(|e| panic!("{}: {op:?} dio error: {e}", self.etiqueta))
            .changed
    }

    /// Aplica el paso a la sesión y al modelo y comprueba que coinciden: un paso de historia por
    /// gesto con cambio, ninguno sin él, `state_id` nuevo, rechazo del borrador y estado de
    /// `modified`. Devuelve si hubo cambio.
    pub fn aplicar(&mut self, paso: &Paso) -> bool {
        let ctx = format!("{}: {paso:?}", self.etiqueta);
        let antes_id = self.s.document.state_id();
        let antes_imagen = self.imagen().try_clone().expect("copia de la imagen");
        let puede_deshacer = self.s.document.can_undo();
        let mut nuevo = self.m.clone();
        let efecto = nuevo.aplicar(paso);

        let (cambio, rechazado) = match paso {
            Paso::Lapiz(ruta, e) => {
                let mut eventos = vec![CanvasEvent::StrokeBegin(ruta[0])];
                eventos.extend(ruta[1..].iter().map(|&p| CanvasEvent::StrokeMove(p)));
                eventos.push(CanvasEvent::StrokeEnd);
                self.gesto(Tool::Pencil, *e, eventos)
            }
            Paso::Linea(a, b, e) => self.gesto(
                Tool::Line,
                *e,
                vec![
                    CanvasEvent::StrokeBegin(*a),
                    CanvasEvent::StrokeMove(*b),
                    CanvasEvent::StrokeEnd,
                ],
            ),
            Paso::Relleno(semilla, e) => self.gesto(
                Tool::Fill,
                *e,
                vec![CanvasEvent::StrokeBegin(*semilla), CanvasEvent::StrokeEnd],
            ),
            Paso::Borrador(ruta) => {
                let mut eventos = vec![CanvasEvent::StrokeBegin(ruta[0])];
                eventos.extend(ruta[1..].iter().map(|&p| CanvasEvent::StrokeMove(p)));
                eventos.push(CanvasEvent::StrokeEnd);
                self.gesto(Tool::Eraser, Eleccion::Ranura(1), eventos)
            }
            Paso::ColorEntrada(i, c) => {
                let commit = Commit {
                    target: EditTarget::Entry(*i),
                    color: Rgba::new(c[0], c[1], c[2], c[3]),
                };
                let cambio = self
                    .s
                    .commit_edit(commit)
                    .unwrap_or_else(|e| panic!("{ctx}: dio error: {e}"));
                (cambio, false)
            }
            Paso::Rotar {
                inicio,
                largo,
                pasos,
            } => (
                self.operacion(
                    PaletteOp::Rotate {
                        start: *inicio,
                        len: *largo,
                        steps: *pasos,
                    },
                    0,
                ),
                false,
            ),
            Paso::Mover { de, a } => (
                self.operacion(PaletteOp::Move { from: *de, to: *a }, 0),
                false,
            ),
            Paso::Anadir { activa } => (self.operacion(PaletteOp::Add, *activa), false),
            Paso::Transparente(indice) => (
                self.operacion(PaletteOp::SetTransparent { index: *indice }, 0),
                false,
            ),
            Paso::Convertir => {
                let destino = match self.m.modo {
                    Mode::Indexed | Mode::GrayAlpha => Mode::Rgba,
                    Mode::Gray => Mode::GrayAlpha,
                    Mode::Rgba => Mode::Indexed,
                };
                match self.s.convert_mode(destino, Allowed::NOTHING) {
                    Ok(cambio) => {
                        assert!(!efecto.error, "{ctx}: se esperaba TooManyColors y dio Ok");
                        (cambio, false)
                    }
                    Err(DocumentError::Convert(ConvertError::TooManyColors { .. }))
                        if efecto.error =>
                    {
                        (false, false)
                    }
                    Err(e) => panic!("{ctx}: la conversión dio {e:?}"),
                }
            }
        };

        assert_eq!(cambio, efecto.cambio, "{ctx}: ¿hubo cambio?");
        assert_eq!(
            rechazado, efecto.rechazado,
            "{ctx}: ¿se rechazó el borrador?"
        );
        let despues_id = self.s.document.state_id();
        if efecto.cambio {
            assert_ne!(antes_id, despues_id, "{ctx}: el state_id no cambió");
            assert!(
                self.usados.insert(despues_id),
                "{ctx}: el state_id {despues_id} ya se había usado"
            );
            assert!(self.s.document.can_undo(), "{ctx}: sin paso de historia");
            self.snaps.truncate(self.pos + 1);
            self.snaps.push((despues_id, nuevo.clone()));
            self.pos += 1;
        } else {
            assert_eq!(antes_id, despues_id, "{ctx}: el state_id cambió sin cambio");
            assert_eq!(
                self.s.document.can_undo(),
                puede_deshacer,
                "{ctx}: cambió la historia sin cambio"
            );
            assert!(
                self.imagen() == &antes_imagen,
                "{ctx}: la imagen cambió sin cambio"
            );
        }
        if rechazado {
            self.rechazos += 1;
        }
        self.m = nuevo;
        self.comprobar();
        efecto.cambio
    }

    /// Imagen igual al modelo y `modified` coherente con el estado guardado: es falso si y solo si el
    /// `state_id` es el guardado, y nunca lo es con una imagen distinta de la guardada.
    pub fn comprobar(&self) {
        let ctx = &self.etiqueta;
        assert_igual(self.imagen(), &self.m, ctx);
        let (id_guardado, modelo_guardado) = &self.guardado;
        assert_eq!(
            self.s.modified(),
            self.s.document.state_id() != *id_guardado,
            "{ctx}: modified no corresponde al state_id guardado"
        );
        if !self.s.modified() {
            assert!(
                &self.m == modelo_guardado,
                "{ctx}: modified es falso con una imagen distinta de la guardada"
            );
        }
    }

    /// Anota el estado actual como el guardado (tras un `save` que salió bien).
    pub fn marcar_guardado(&mut self) {
        self.guardado = (self.s.document.state_id(), self.m.clone());
        assert!(
            !self.s.modified(),
            "{}: modified tras guardar",
            self.etiqueta
        );
        self.comprobar();
    }

    /// `undo`: da la instantánea de un paso atrás, o `NothingToUndo` al principio.
    pub fn deshacer(&mut self) {
        let resultado = self.s.undo();
        if self.pos == 0 {
            assert_eq!(
                resultado,
                Err(DocumentError::NothingToUndo),
                "{}: undo al principio",
                self.etiqueta
            );
        } else {
            resultado.unwrap_or_else(|e| panic!("{}: undo dio {e:?}", self.etiqueta));
            self.pos -= 1;
            self.m = self.snaps[self.pos].1.clone();
            assert_eq!(
                self.s.document.state_id(),
                self.snaps[self.pos].0,
                "{}: state_id tras undo",
                self.etiqueta
            );
        }
        self.comprobar();
    }

    /// `redo`: da la instantánea de un paso adelante, o `NothingToRedo` al final de la rama.
    pub fn rehacer(&mut self) {
        let resultado = self.s.redo();
        if self.pos + 1 == self.snaps.len() {
            assert_eq!(
                resultado,
                Err(DocumentError::NothingToRedo),
                "{}: redo al final",
                self.etiqueta
            );
        } else {
            resultado.unwrap_or_else(|e| panic!("{}: redo dio {e:?}", self.etiqueta));
            self.pos += 1;
            self.m = self.snaps[self.pos].1.clone();
            assert_eq!(
                self.s.document.state_id(),
                self.snaps[self.pos].0,
                "{}: state_id tras redo",
                self.etiqueta
            );
        }
        self.comprobar();
    }

    /// `undo` hasta el principio (y uno más, que debe fallar); devuelve cuántos hizo.
    pub fn deshacer_todo(&mut self) -> usize {
        let mut hechos = 0;
        while self.pos > 0 {
            self.deshacer();
            hechos += 1;
        }
        self.deshacer();
        hechos
    }

    /// `redo` hasta el final de la rama (y uno más, que debe fallar); devuelve cuántos hizo.
    pub fn rehacer_todo(&mut self) -> usize {
        let mut hechos = 0;
        while self.pos + 1 < self.snaps.len() {
            self.rehacer();
            hechos += 1;
        }
        self.rehacer();
        hechos
    }
}

/// Abre la fuente en un directorio de trabajo (copia de los reales) y comprueba la imagen contra su
/// referencia independiente: el modelo (generadas y construidas) o el `.sha256` (reales).
pub fn abrir(t: &Trabajo, fuente: &Fuente) -> Mundo {
    let ruta = t.poner(fuente);
    let mut s = sesion_vacia();
    s.open(&ruta, Canvas::MAX_SIDE, false)
        .unwrap_or_else(|e| panic!("{}: open dio error: {e}", fuente.nombre));
    let imagen = s
        .document
        .image(s.document.main_image())
        .expect("el documento tiene imagen");
    comprobar_apertura(fuente, imagen);
    Mundo::de_sesion(s, &fuente.nombre)
}

/// La imagen abierta es la referencia independiente de la fuente.
pub fn comprobar_apertura(fuente: &Fuente, imagen: &Image) {
    if let Some(esperado) = &fuente.esperado {
        assert_igual(imagen, esperado, &fuente.nombre);
    }
    if let Some(sha) = &fuente.sha {
        assert_eq!(
            &sha_rgba(imagen),
            sha,
            "{}: el SHA-256 de la RGBA expandida no es el de su .sha256",
            fuente.nombre
        );
    }
}

/// Elección número `k` del color: una entrada de la paleta (módulo su longitud) o una ranura.
pub fn eleccion_n(m: &Modelo, k: usize) -> Eleccion {
    if m.modo == Mode::Indexed {
        Eleccion::Entrada((k % m.paleta.len()) as u8)
    } else {
        Eleccion::Ranura(1 + (k % 9) as u8)
    }
}

/// Lápiz, línea, relleno y borrador: con repeticiones que no deben dar paso (el mismo trazo con el
/// mismo color y el mismo relleno).
pub fn guion_herramientas(m: &Modelo) -> Vec<Paso> {
    let (c1, c2, c3, c4) = (
        eleccion_n(m, 1),
        eleccion_n(m, 2),
        eleccion_n(m, 3),
        eleccion_n(m, 4),
    );
    vec![
        Paso::Lapiz(vec![(2, 2), (9, 2), (9, 9), (5, 5)], c1),
        Paso::Lapiz(vec![(2, 2), (9, 2)], c1),
        Paso::Linea((1, 12), (14, 12), c2),
        Paso::Linea((12, 1), (12, 10), c3),
        Paso::Linea((0, 15), (10, 5), c2),
        Paso::Relleno((0, 0), c4),
        Paso::Relleno((0, 0), c4),
        Paso::Borrador(vec![(3, 3), (8, 3)]),
        Paso::Borrador(vec![(3, 3), (8, 3)]),
    ]
}

/// Edición de color, `Rotate`, `Move`, `Add` y `SetTransparent` (de una indexada), con un par de
/// repeticiones que no deben dar paso.
pub fn guion_paleta(m: &Modelo) -> Vec<Paso> {
    assert_eq!(m.modo, Mode::Indexed);
    let largo = m.paleta.len();
    let mut pasos = vec![
        Paso::ColorEntrada(0, [1, 2, 3, 255]),
        Paso::ColorEntrada(0, [1, 2, 3, 255]),
        Paso::Rotar {
            inicio: 0,
            largo: largo as u16,
            pasos: 1,
        },
    ];
    if largo >= 2 {
        pasos.push(Paso::Mover {
            de: 0,
            a: (largo - 1) as u8,
        });
    }
    if largo < 256 {
        pasos.push(Paso::Anadir { activa: 0 });
        pasos.push(Paso::ColorEntrada(largo as u8, [9, 8, 7, 200]));
    }
    pasos.push(Paso::Transparente(Some(1.min(largo - 1) as u8)));
    pasos.push(Paso::Rotar {
        inicio: 0,
        largo: largo.min(3) as u16,
        pasos: -1,
    });
    // Con el índice transparente fijado, el borrador ya escribe en él.
    pasos.push(Paso::Borrador(vec![(3, 3), (8, 3)]));
    pasos.push(Paso::Transparente(Some(0)));
    pasos.push(Paso::Transparente(Some(0)));
    pasos.push(Paso::Transparente(None));
    pasos
}

/// El guion entero: herramientas, paleta (si es indexada) y, al final, la conversión exacta.
pub fn guion_completo(m: &Modelo) -> Vec<Paso> {
    let mut pasos = guion_herramientas(m);
    if m.modo == Mode::Indexed {
        pasos.extend(guion_paleta(m));
    }
    pasos.push(Paso::Convertir);
    pasos
}

/// Un lápiz de un píxel en (0, 0) con un color que cambia ese píxel, o `None` si ninguno lo hace.
pub fn edicion_que_cambia_opt(m: &Modelo) -> Option<Paso> {
    let elecciones: Vec<Eleccion> = if m.modo == Mode::Indexed {
        (0..m.paleta.len().min(256))
            .map(|i| Eleccion::Entrada(i as u8))
            .collect()
    } else {
        (1..=9).map(Eleccion::Ranura).collect()
    };
    elecciones.into_iter().find_map(|e| {
        let paso = Paso::Lapiz(vec![(0, 0)], e);
        let mut copia = m.clone();
        copia.aplicar(&paso).cambio.then_some(paso)
    })
}

/// Como `edicion_que_cambia_opt`, con pánico si no hay ninguna.
pub fn edicion_que_cambia(m: &Modelo) -> Paso {
    edicion_que_cambia_opt(m).expect("alguna edición cambia el píxel (0, 0)")
}

/// Lee un PNG del disco con `read_png`, con pánico si falla.
pub fn leer(ruta: &Path) -> px_formats::PngRead {
    let bytes = fs::read(ruta).unwrap_or_else(|e| panic!("leer {}: {e}", ruta.display()));
    read_png(&bytes).unwrap_or_else(|e| panic!("read_png de {}: {e}", ruta.display()))
}

// ---------------------------------------------------------------------------------------
// El selector de archivos falso.
// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct EstadoSelector {
    arranques: usize,
    ultima: Option<DialogRequest>,
    /// `None`: sigue abierto. `Some(r)`: lo que dará el próximo `poll`.
    resultado: Option<Option<PathBuf>>,
}

/// Mando del test sobre el selector que se entregó a `Dialogs`: resuelve cuando el test lo dice.
#[derive(Clone, Default)]
pub struct Mando(Rc<RefCell<EstadoSelector>>);

impl Mando {
    /// El usuario elige `ruta`.
    pub fn elegir(&self, ruta: &Path) {
        self.0.borrow_mut().resultado = Some(Some(ruta.to_path_buf()));
    }

    /// El diálogo se cierra sin ruta.
    pub fn cerrar(&self) {
        self.0.borrow_mut().resultado = Some(None);
    }

    /// La última petición que recibió el selector.
    pub fn ultima(&self) -> DialogRequest {
        self.0
            .borrow()
            .ultima
            .clone()
            .expect("el selector recibió alguna petición")
    }

    pub fn arranques(&self) -> usize {
        self.0.borrow().arranques
    }
}

struct SelectorFalso(Mando);

impl FilePicker for SelectorFalso {
    fn start(&mut self, request: &DialogRequest, parent: Option<&eframe::Frame>) {
        assert!(parent.is_none(), "las pruebas no tienen ventana");
        let mut estado = self.0.0.borrow_mut();
        estado.arranques += 1;
        estado.ultima = Some(request.clone());
        estado.resultado = None;
    }

    fn poll(&mut self) -> Option<Option<PathBuf>> {
        self.0.0.borrow_mut().resultado.take()
    }

    fn abandon(&mut self) {
        self.0.0.borrow_mut().resultado = None;
    }
}

/// `Dialogs` como los monta la app (fallo rápido solo en Linux), sobre un selector falso.
pub fn dialogos(cwd: &Path) -> (Dialogs, Mando) {
    let mando = Mando::default();
    let dialogos = Dialogs::new(
        Box::new(SelectorFalso(mando.clone())),
        cfg!(target_os = "linux"),
        Some(cwd.to_path_buf()),
    );
    (dialogos, mando)
}
