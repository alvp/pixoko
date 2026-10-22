//! Piezas comunes de la puerta del hito 2 (specs/hito-2-puerta.md): directorio de trabajo,
//! fuentes, modelo independiente, comparador, instantánea de `golden/png/`, `FakePicker` propio y
//! la ayuda `outside_of_golden`.
//!
//! **Modelo independiente.** `Model` es una rejilla de celdas más una paleta; aplica las mismas
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

use px_core::ImageId;
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
// Rutas, directorio de trabajo y `outside_of_golden`.
// ---------------------------------------------------------------------------------------

/// Los cuatro PNG reales de `golden/png/`.
pub const REAL: [&str; 4] = [
    "grafx2-golden1.png",
    "grafx2-golden2.png",
    "libresprite-golden-indexed.png",
    "libresprite-golden-rgba.png",
];

pub fn root_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

pub fn golden_png() -> PathBuf {
    root_repo().join("golden").join("png")
}

/// `<repo>/target/pixoko-gate`: en disco, no en `/tmp` (tmpfs) y en `.gitignore` (`/target`).
fn base_work() -> PathBuf {
    root_repo().join("target").join("pixoko-gate")
}

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);
/// Crear y borrar directorios de trabajo no puede cruzarse con el borrado de la carpeta madre.
static LOCK: Mutex<()> = Mutex::new(());

/// Directorio de trabajo propio de una prueba (`<repo>/target/pixoko-gate/<pid>-<n>`). El guardián
/// lo borra al soltarse, también si la prueba falla, y comprueba entonces que `golden/png/` sigue
/// intacto (salvo que la prueba ya esté en pánico).
pub struct Work {
    path_2: PathBuf,
}

impl Work {
    pub fn new_2() -> Work {
        // La instantánea inicial de golden/png se toma antes de que nada pueda escribir.
        initial();
        let path_2 = {
            let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path_2 = base_work().join(format!("{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&path_2);
            fs::create_dir_all(&path_2).expect("crear el directorio de trabajo");
            fs::canonicalize(&path_2).expect("canonicalizar el directorio de trabajo")
        };
        outside_of_golden(&path_2);
        Work { path_2 }
    }

    pub fn path_2(&self) -> &Path {
        &self.path_2
    }

    /// Deja la fuente en el directorio de trabajo: los reales con `fs::copy`, el resto escritos.
    pub fn put(&self, source: &Source) -> PathBuf {
        let target_2 = self.path_2.join(&source.name_2);
        outside_of_golden(&target_2);
        if source.real {
            fs::copy(golden_png().join(&source.name_2), &target_2)
                .unwrap_or_else(|e| panic!("copiar {} al trabajo: {e}", source.name_2));
        } else {
            fs::write(&target_2, &source.bytes)
                .unwrap_or_else(|e| panic!("escribir {}: {e}", target_2.display()));
        }
        target_2
    }

    /// Escribe un fichero en el directorio de trabajo.
    pub fn write(&self, name_2: &str, bytes: &[u8]) -> PathBuf {
        let target_2 = self.path_2.join(name_2);
        outside_of_golden(&target_2);
        fs::write(&target_2, bytes)
            .unwrap_or_else(|e| panic!("escribir {}: {e}", target_2.display()));
        target_2
    }

    /// Nombres de lo que hay en el directorio de trabajo, ordenados.
    pub fn names(&self) -> Vec<String> {
        names_in(&self.path_2)
    }
}

impl Drop for Work {
    fn drop(&mut self) {
        {
            let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let _ = fs::remove_dir_all(&self.path_2);
            // Solo se borra si ya no queda ningún otro directorio de trabajo.
            let _ = fs::remove_dir(base_work());
        }
        if !std::thread::panicking() {
            golden_intact();
        }
    }
}

/// Nombres de lo que hay en `dir`, ordenados.
pub fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("leer {}: {e}", dir.display()))
        .map(|input| {
            input
                .expect("entrada de directorio")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// Que nada con `.pixoko-tmp` en el nombre quede en `names`.
pub fn without_temporary(names: &[String]) {
    assert!(
        names.iter().all(|n| !n.contains(".pixoko-tmp")),
        "quedó un temporal de write_atomic: {names:?}"
    );
}

/// La ruta con sus enlaces y `..` resueltos: se canonicaliza el ancestro más largo que exista y se
/// le añade el resto (la ruta puede no existir todavía).
fn resolver(path_2: &Path) -> PathBuf {
    let absolute = std::path::absolute(path_2).unwrap_or_else(|_| path_2.to_path_buf());
    let mut rest: Vec<OsString> = Vec::new();
    let mut actual = absolute.clone();
    loop {
        if let Ok(canonical) = fs::canonicalize(&actual) {
            let mut resolved = canonical;
            for component in rest.iter().rev() {
                resolved.push(component);
            }
            return resolved;
        }
        match (actual.parent(), actual.file_name()) {
            (Some(padre), Some(name_2)) => {
                rest.push(name_2.to_os_string());
                actual = padre.to_path_buf();
            }
            _ => return absolute,
        }
    }
}

/// Pánico si `path_2` (de escritura) cuelga de `golden/png/`. Se le pasa **toda** ruta de escritura.
pub fn outside_of_golden(path_2: &Path) {
    let forbidden = resolver(&golden_png());
    let target_2 = resolver(path_2);
    assert!(
        !target_2.starts_with(&forbidden),
        "escritura prohibida en golden/png: {} (resuelta a {})",
        path_2.display(),
        target_2.display()
    );
}

/// `Session::save` con la comprobación de `outside_of_golden` sobre el directorio y sobre la ruta del
/// documento (el destino es esa ruta o su copia, en el mismo directorio).
pub fn save(s: &mut Session, dir: &Path) -> Result<Saved, FileError> {
    outside_of_golden(dir);
    if let Some(path_2) = s.path() {
        outside_of_golden(path_2);
    }
    s.save(dir)
}

/// `Session::save_as` con la misma comprobación.
pub fn save_as(s: &mut Session, dir: &Path) -> Result<Saved, FileError> {
    outside_of_golden(dir);
    if let Some(path_2) = s.path() {
        outside_of_golden(path_2);
    }
    s.save_as(dir)
}

/// `Session::save_to` con la comprobación sobre la ruta elegida. La ruta del documento puede ser un
/// real abierto directamente (solo lectura): `save_to` solo escribe junto a ella si la elegida es
/// ella misma, y eso ya lo recoge la comprobación de la elegida.
pub fn save_in(s: &mut Session, chosen: &Path) -> Result<Saved, FileError> {
    outside_of_golden(chosen);
    s.save_to(chosen)
}

/// Primer nombre libre `{name_2}-copy.png`, `{name_2}-copy-2.png`… junto a `original`, calculado
/// aquí (independiente de `copy_path`).
pub fn free_copy(original: &Path) -> PathBuf {
    let stem = original
        .file_stem()
        .expect("el original tiene nombre")
        .to_string_lossy()
        .into_owned();
    let dir = original.parent().expect("el original tiene directorio");
    let mut n = 1u32;
    loop {
        let name_2 = if n == 1 {
            format!("{stem}-copia.png")
        } else {
            format!("{stem}-copia-{n}.png")
        };
        let candidate = dir.join(name_2);
        if fs::symlink_metadata(&candidate).is_err() {
            return candidate;
        }
        n += 1;
    }
}

/// Añade `.png` salvo que la extensión ya sea `png` (sin distinguir mayúsculas), como un diálogo.
pub fn with_png(path_2: &Path) -> PathBuf {
    let es_png = path_2
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if es_png {
        return path_2.to_path_buf();
    }
    let mut name_2 = path_2.as_os_str().to_os_string();
    name_2.push(".png");
    PathBuf::from(name_2)
}

// ---------------------------------------------------------------------------------------
// Instantánea de golden/png/.
// ---------------------------------------------------------------------------------------

/// Nombre y bytes de cada fichero de una carpeta (un subdirectorio, con barra final y sin bytes).
pub type TestSnapshot = BTreeMap<String, Vec<u8>>;

pub fn snapshot_of(dir: &Path) -> TestSnapshot {
    let mut map = TestSnapshot::new();
    for input in fs::read_dir(dir).unwrap_or_else(|e| panic!("leer {}: {e}", dir.display())) {
        let input = input.expect("entrada de directorio");
        let name_2 = input.file_name().to_string_lossy().into_owned();
        if input.path().is_dir() {
            map.insert(format!("{name_2}/"), Vec::new());
        } else {
            let bytes = fs::read(input.path())
                .unwrap_or_else(|e| panic!("leer {}: {e}", input.path().display()));
            map.insert(name_2, bytes);
        }
    }
    map
}

/// `Err` con todos los problemas si `after_2` no es `antes`: un fichero que falta, que aparece o
/// con algún byte distinto.
pub fn compare_snapshots(antes: &TestSnapshot, after_2: &TestSnapshot) -> Result<(), String> {
    let mut problems = Vec::new();
    for (name_2, bytes) in antes {
        match after_2.get(name_2) {
            None => problems.push(format!("falta {name_2}")),
            Some(other_2_2) if other_2_2 != bytes => {
                let first = bytes
                    .iter()
                    .zip(other_2_2)
                    .position(|(a, b)| a != b)
                    .unwrap_or(bytes.len().min(other_2_2.len()));
                problems.push(format!(
                    "{name_2} cambió ({} → {} bytes, primera diferencia en el byte {first})",
                    bytes.len(),
                    other_2_2.len()
                ));
            }
            Some(_) => {}
        }
    }
    for name_2 in after_2.keys() {
        if !antes.contains_key(name_2) {
            problems.push(format!("aparece {name_2}"));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

static INITIAL: OnceLock<TestSnapshot> = OnceLock::new();

/// La instantánea de `golden/png/` al empezar la puerta: la toma la primera prueba que la pide (todas
/// las que usan golden la piden antes de escribir nada, al crear su `Work`).
pub fn initial() -> &'static TestSnapshot {
    INITIAL.get_or_init(|| snapshot_of(&golden_png()))
}

/// Pánico si `golden/png/` ya no es la instantánea inicial.
pub fn golden_intact() {
    let now = snapshot_of(&golden_png());
    if let Err(problems) = compare_snapshots(initial(), &now) {
        panic!("golden/png/ ya no es el de antes de la puerta: {problems}");
    }
}

// ---------------------------------------------------------------------------------------
// Generador pseudoaleatorio propio y PNG a mano.
// ---------------------------------------------------------------------------------------

/// Congruencial lineal de 64 bits: bastan unos píxeles repetibles, sin dependencias.
pub struct Lcg(u64);

impl Lcg {
    pub fn new(seed: u64) -> Lcg {
        Lcg(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }

    /// Un valor en `0..n` (con `n > 0`).
    pub fn until(&mut self, n: u32) -> u32 {
        self.next() % n
    }
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

/// Flujo zlib con un único bloque deflate «stored» (datos pequeños, menos de 65 536 bytes).
pub fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut v = vec![0x78, 0x01, 1];
    let n = u16::try_from(data.len()).expect("datos pequeños");
    v.extend(n.to_le_bytes());
    v.extend((!n).to_le_bytes());
    v.extend(data);
    v.extend(adler32(data).to_be_bytes());
    v
}

/// Chunk completo: longitud, tipo, datos y CRC.
pub fn piece(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + data.len());
    v.extend((data.len() as u32).to_be_bytes());
    v.extend(kind);
    v.extend(data);
    let mut body = kind.to_vec();
    body.extend(data);
    v.extend(crc32(&body).to_be_bytes());
    v
}

/// Empaqueta las muestras de una fila: sub-8 bits de más a menos significativo con relleno a byte,
/// 8 bits tal cual y 16 bits en big-endian.
fn packs(row: &[u16], prof: u8) -> Vec<u8> {
    match prof {
        16 => row.iter().flat_map(|v| v.to_be_bytes()).collect(),
        8 => row.iter().map(|&v| v as u8).collect(),
        _ => {
            let p = usize::from(prof);
            row.chunks(8 / p)
                .map(|group| {
                    let mut byte = 0u8;
                    for (i, &v) in group.iter().enumerate() {
                        byte |= (v as u8) << (8 - p * (i + 1));
                    }
                    byte
                })
                .collect()
        }
    }
}

/// PNG entero sin entrelazar (chunks, CRC y deflate «stored» a mano): `swatches` va fila a fila y
/// píxel a píxel, con los canales del tipo de color; `antes` son chunks ya serializados entre IHDR e
/// IDAT. Es la forma de `session/aux_tests.rs`, sin importarla.
pub fn png_raw(
    width: u32,
    height: u32,
    prof: u8,
    color: u8,
    swatches: &[u16],
    antes: &[Vec<u8>],
) -> Vec<u8> {
    let channels = match color {
        2 => 3,
        4 => 2,
        6 => 4,
        _ => 1,
    };
    let per_row = width as usize * channels;
    let mut raw = Vec::new();
    for row in swatches.chunks(per_row) {
        raw.push(0);
        raw.extend(packs(row, prof));
    }
    let mut ihdr = Vec::new();
    ihdr.extend(width.to_be_bytes());
    ihdr.extend(height.to_be_bytes());
    ihdr.extend([prof, color, 0, 0, 0]);
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend(piece(b"IHDR", &ihdr));
    for c in antes {
        v.extend(c);
    }
    v.extend(piece(b"IDAT", &zlib_stored(&raw)));
    v.extend(piece(b"IEND", &[]));
    v
}

/// Inserta chunks justo tras el IHDR (que acaba en el byte 33) de un PNG de `write_png`.
pub fn with_chunks(bytes: &[u8], antes: &[Vec<u8>]) -> Vec<u8> {
    let mut v = bytes[..33].to_vec();
    for c in antes {
        v.extend(c);
    }
    v.extend(&bytes[33..]);
    v
}

/// Chunk `iCCP` con un perfil de mentira (`read_png` no lo interpreta: solo lo anota).
pub fn iccp() -> Vec<u8> {
    let mut data = b"perfil\0\0".to_vec();
    data.extend(zlib_stored(b"datos del perfil"));
    piece(b"iCCP", &data)
}

/// `(width, height, profundidad, kind de color)` del IHDR.
pub fn ihdr(bytes: &[u8]) -> (u32, u32, u8, u8) {
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("ancho"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("alto"));
    (width, height, bytes[24], bytes[25])
}

/// Tipos de los chunks de un fichero bien formado, en orden.
pub fn types_of_chunks(bytes: &[u8]) -> Vec<[u8; 4]> {
    let mut types = Vec::new();
    let mut pos = 8usize;
    while pos + 8 <= bytes.len() {
        let largo = u32::from_be_bytes(bytes[pos..pos + 4].try_into().expect("longitud")) as usize;
        let kind: [u8; 4] = bytes[pos + 4..pos + 8].try_into().expect("tipo");
        types.push(kind);
        pos += 12 + largo;
    }
    types
}

/// El modo que un lector debe dar al fichero (de su IHDR y de si lleva `tRNS`), sin leer los píxeles.
pub fn mode_of_the_file(bytes: &[u8]) -> Mode {
    let (_, _, _, color) = ihdr(bytes);
    match color {
        3 => Mode::Indexed,
        2 | 6 => Mode::Rgba,
        4 => Mode::GrayAlpha,
        0 => {
            if types_of_chunks(bytes).contains(b"tRNS") {
                Mode::GrayAlpha
            } else {
                Mode::Gray
            }
        }
        other_2 => panic!("tipo de color PNG inesperado: {other_2}"),
    }
}

// ---------------------------------------------------------------------------------------
// El modelo.
// ---------------------------------------------------------------------------------------

/// Lo que elige el color de un gesto: una ranura (1 a 9) o una entrada de la paleta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    TestSlot(u8),
    TestInput(u8),
}

fn a_color_choice(e: Choice) -> ColorChoice {
    match e {
        Choice::TestSlot(n) => ColorChoice::Slot(n),
        Choice::TestInput(i) => ColorChoice::Entry(i),
    }
}

/// Un paso del guion: un gesto de una herramienta, una operación de la paleta o una conversión.
#[derive(Clone, Debug)]
pub enum TestStep {
    /// Trayecto del puntero (cada tramo, horizontal, vertical o a 45°) y color.
    Pencil(Vec<(i32, i32)>, Choice),
    Line((i32, i32), (i32, i32), Choice),
    Fill((i32, i32), Choice),
    Eraser(Vec<(i32, i32)>),
    /// Edita el color de una entrada de la paleta.
    ColorInput(u8, [u8; 4]),
    Rotate {
        start_2: u8,
        largo: u16,
        steps_2: i32,
    },
    Mover {
        de: u8,
        a: u8,
    },
    /// Añade una copia del color resuelto de la entrada activa.
    Add {
        active_2: u8,
    },
    Transparent(Option<u8>),
    /// La conversión exacta del modo actual: indexado a RGBA, gris a gris con alfa, gris con alfa a
    /// RGBA y RGBA a indexado.
    Convert,
}

/// Lo que el modelo dice de un paso.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effect {
    /// El documento debe cambiar (y entonces hay un paso de historia nuevo).
    pub change: bool,
    /// El borrador debe rechazar el gesto (aviso `Refused`).
    pub rejected: bool,
    /// La conversión debe fallar con `TooManyColors`, sin cambiar nada.
    pub error: bool,
}

/// Rejilla de píxeles y paleta. Cada celda son 4 bytes con el significado del modo: RGBA `[r, g, b,
/// a]`, gray with alpha_2 `[g, a, 0, 0]`, gray `[g, 0, 0, 0]` e indexed_2 `[índice, 0, 0, 0]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    pub mode_2: Mode,
    pub width: usize,
    pub height: usize,
    pub cels: Vec<[u8; 4]>,
    /// Colores guardados de la paleta (solo indexado).
    pub palette: Vec<[u8; 4]>,
    pub transparent: Option<u8>,
}

fn bytes_per_pixel(mode_2: Mode) -> usize {
    match mode_2 {
        Mode::Rgba => 4,
        Mode::GrayAlpha => 2,
        Mode::Gray | Mode::Indexed => 1,
    }
}

/// El transparente canónico de una conversión: todo alfa 0 pasa a `(0, 0, 0, 0)`.
fn canonical_2(c: [u8; 4]) -> [u8; 4] {
    if c[3] == 0 { [0, 0, 0, 0] } else { c }
}

/// Segmento de `a` a `b` incluidos ambos extremos; solo horizontal, vertical o a 45°.
fn segment(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
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
fn stroke(path_2: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let Some(&first_2) = path_2.first() else {
        return points;
    };
    points.push(first_2);
    let mut ultimo = first_2;
    for &next in &path_2[1..] {
        if next == ultimo {
            continue;
        }
        points.extend(segment(ultimo, next));
        ultimo = next;
    }
    points
}

impl Model {
    fn index_2(&self, x: i32, y: i32) -> Option<usize> {
        let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
            return None;
        };
        (x < self.width && y < self.height).then_some(y * self.width + x)
    }

    /// Los bytes en el formato de `Image::bytes`.
    pub fn bytes(&self) -> Vec<u8> {
        let n = bytes_per_pixel(self.mode_2);
        self.cels
            .iter()
            .flat_map(|c| c[..n].iter().copied())
            .collect()
    }

    /// Color de una entrada de la paleta con el índice transparente resuelto (alfa 0).
    pub fn resolved_2(&self, i: u8) -> [u8; 4] {
        let mut color = self.palette[usize::from(i)];
        if self.transparent == Some(i) {
            color[3] = 0;
        }
        color
    }

    /// RGBA de 8 bits expandido, como el `.sha256` del corpus.
    pub fn rgba(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.cels.len() * 4);
        for c in &self.cels {
            match self.mode_2 {
                Mode::Rgba => v.extend_from_slice(c),
                Mode::GrayAlpha => v.extend([c[0], c[0], c[0], c[1]]),
                Mode::Gray => v.extend([c[0], c[0], c[0], 255]),
                Mode::Indexed => v.extend(self.resolved_2(c[0])),
            }
        }
        v
    }

    /// Lo que devuelve la relectura de un PNG guardado: un indexado con índice transparente vuelve
    /// con la entrada resuelta (alfa 0) y sin índice (comportamiento 11 de la spec de PNG).
    pub fn normalized(&self) -> Model {
        let mut m = self.clone();
        if let Some(t) = m.transparent.take() {
            m.palette[usize::from(t)][3] = 0;
        }
        m
    }

    /// La `Image` equivalente (solo como entrada de las pruebas, nunca como esperado).
    pub fn a_image(&self) -> Image {
        let (width, height) = (self.width as u32, self.height as u32);
        let data = self.bytes();
        if self.mode_2 == Mode::Indexed {
            let colors = self
                .palette
                .iter()
                .map(|c| Rgba::new(c[0], c[1], c[2], c[3]));
            let mut palette = Palette::from_colors(colors).expect("paleta del modelo");
            palette
                .set_transparent_index(self.transparent)
                .expect("índice transparente del modelo");
            Image::from_bytes_indexed(width, height, palette, data).expect("imagen indexada")
        } else {
            Image::from_bytes(width, height, self.mode_2, data).expect("imagen")
        }
    }

    /// Siembra el modelo con una imagen ya comprobada contra una referencia independiente.
    pub fn of_image(image_2: &Image) -> Model {
        let mode_2 = image_2.mode();
        let n = bytes_per_pixel(mode_2);
        let cels = image_2
            .bytes()
            .chunks_exact(n)
            .map(|piece| {
                let mut cel = [0u8; 4];
                cel[..n].copy_from_slice(piece);
                cel
            })
            .collect();
        let palette = image_2
            .palette()
            .map(|p| p.colors().iter().map(|c| [c.r, c.g, c.b, c.a]).collect())
            .unwrap_or_default();
        Model {
            mode_2,
            width: image_2.width() as usize,
            height: image_2.height() as usize,
            cels,
            palette,
            transparent: image_2.palette().and_then(Palette::transparent_index),
        }
    }

    /// La celda que escribe un color elegido; `None` si no hay color para esa elección.
    pub fn cel_of(&self, e: Choice) -> Option<[u8; 4]> {
        match (e, self.mode_2) {
            (Choice::TestInput(i), Mode::Indexed) => {
                (usize::from(i) < self.palette.len()).then_some([i, 0, 0, 0])
            }
            (Choice::TestInput(_), _) => None,
            (Choice::TestSlot(n), _) if !(1..=9).contains(&n) => None,
            (Choice::TestSlot(n), Mode::Indexed) => {
                (usize::from(n - 1) < self.palette.len()).then_some([n - 1, 0, 0, 0])
            }
            (Choice::TestSlot(n), Mode::Rgba) => Some(match n {
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
            (Choice::TestSlot(n), Mode::Gray) => {
                let gray = if n == 9 { 255 } else { (n - 1) * 32 };
                Some([gray, 0, 0, 0])
            }
            (Choice::TestSlot(n), Mode::GrayAlpha) => Some(if n == 9 {
                [0, 0, 0, 0]
            } else {
                [(n - 1) * 32, 255, 0, 0]
            }),
        }
    }

    /// Alfa resuelto de la celda `i`.
    fn alpha_2(&self, i: usize) -> u8 {
        let c = self.cels[i];
        match self.mode_2 {
            Mode::Rgba => c[3],
            Mode::GrayAlpha => c[1],
            Mode::Gray => 255,
            Mode::Indexed => self.resolved_2(c[0])[3],
        }
    }

    /// Lo que escribe el borrador, o `None` si no puede con la imagen: RGBA y gris con alfa, el
    /// transparente canónico; indexado, el índice transparente o, sin él, la primera entrada de alfa
    /// guardado 0; gris, nada.
    fn value_deleted(&self) -> Option<[u8; 4]> {
        match self.mode_2 {
            Mode::Rgba | Mode::GrayAlpha => Some([0, 0, 0, 0]),
            Mode::Gray => None,
            Mode::Indexed => match self.transparent {
                Some(t) => Some([t, 0, 0, 0]),
                None => self
                    .palette
                    .iter()
                    .position(|c| c[3] == 0)
                    .map(|i| [i as u8, 0, 0, 0]),
            },
        }
    }

    fn paint(&mut self, points: &[(i32, i32)], cel: [u8; 4]) -> bool {
        let mut change = false;
        for &(x, y) in points {
            if let Some(i) = self.index_2(x, y)
                && self.cels[i] != cel
            {
                self.cels[i] = cel;
                change = true;
            }
        }
        change
    }

    /// Relleno por cola de la región de 4 vecinos con el mismo valor guardado que la semilla.
    fn fill(&mut self, seed: (i32, i32), cel: [u8; 4]) -> bool {
        let Some(start_2) = self.index_2(seed.0, seed.1) else {
            return false;
        };
        let goal = self.cels[start_2];
        if goal == cel {
            return false;
        }
        self.cels[start_2] = cel;
        let mut queue = VecDeque::from([seed]);
        while let Some((x, y)) = queue.pop_front() {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let neighbor = (x + dx, y + dy);
                if let Some(i) = self.index_2(neighbor.0, neighbor.1)
                    && self.cels[i] == goal
                {
                    self.cels[i] = cel;
                    queue.push_back(neighbor);
                }
            }
        }
        true
    }

    fn delete(&mut self, path_2: &[(i32, i32)]) -> Effect {
        let Some(valor) = self.value_deleted() else {
            return Effect {
                rejected: true,
                ..Effect::default()
            };
        };
        let mut change = false;
        for (x, y) in stroke(path_2) {
            if let Some(i) = self.index_2(x, y)
                && self.alpha_2(i) != 0
            {
                self.cels[i] = valor;
                change = true;
            }
        }
        Effect {
            change,
            ..Effect::default()
        }
    }

    fn convert(&mut self) -> Effect {
        match self.mode_2 {
            Mode::Indexed => {
                let new_2_2: Vec<[u8; 4]> = self
                    .cels
                    .iter()
                    .map(|c| canonical_2(self.resolved_2(c[0])))
                    .collect();
                self.cels = new_2_2;
                self.mode_2 = Mode::Rgba;
                self.palette.clear();
                self.transparent = None;
            }
            Mode::Gray => {
                for c in &mut self.cels {
                    *c = [c[0], 255, 0, 0];
                }
                self.mode_2 = Mode::GrayAlpha;
            }
            Mode::GrayAlpha => {
                for c in &mut self.cels {
                    *c = canonical_2([c[0], c[0], c[0], c[1]]);
                }
                self.mode_2 = Mode::Rgba;
            }
            Mode::Rgba => {
                // El transparente, si lo hay, en el índice 0; el resto por primera aparición.
                let mut colors: Vec<[u8; 4]> = Vec::new();
                let mut there_is_transparent = false;
                for c in &self.cels {
                    if c[3] == 0 {
                        there_is_transparent = true;
                    } else if !colors.contains(c) {
                        colors.push(*c);
                    }
                }
                if colors.len() + usize::from(there_is_transparent) > 256 {
                    return Effect {
                        error: true,
                        ..Effect::default()
                    };
                }
                let pan = usize::from(there_is_transparent);
                let new_2_2: Vec<[u8; 4]> = self
                    .cels
                    .iter()
                    .map(|c| {
                        if c[3] == 0 {
                            [0, 0, 0, 0]
                        } else {
                            let at = colors.iter().position(|k| k == c).expect("registrado");
                            [(at + pan) as u8, 0, 0, 0]
                        }
                    })
                    .collect();
                let mut palette = Vec::new();
                if there_is_transparent {
                    palette.push([0, 0, 0, 0]);
                }
                palette.extend(colors);
                self.cels = new_2_2;
                self.mode_2 = Mode::Indexed;
                self.palette = palette;
                self.transparent = there_is_transparent.then_some(0);
            }
        }
        Effect {
            change: true,
            ..Effect::default()
        }
    }

    /// Aplica un paso al modelo y dice qué debe pasar en la sesión. Con un error de conversión, el
    /// modelo no cambia.
    pub fn apply(&mut self, step: &TestStep) -> Effect {
        let per_cel = |change: bool| Effect {
            change,
            ..Effect::default()
        };
        match step {
            TestStep::Pencil(path_2, e) => match self.cel_of(*e) {
                Some(cel) => per_cel(self.paint(&stroke(path_2), cel)),
                None => Effect::default(),
            },
            TestStep::Line(a, b, e) => match self.cel_of(*e) {
                Some(cel) => per_cel(self.paint(&segment(*a, *b), cel)),
                None => Effect::default(),
            },
            TestStep::Fill(seed, e) => match self.cel_of(*e) {
                Some(cel) => per_cel(self.fill(*seed, cel)),
                None => Effect::default(),
            },
            TestStep::Eraser(path_2) => self.delete(path_2),
            TestStep::ColorInput(i, color) => {
                let change = self.palette[usize::from(*i)] != *color;
                self.palette[usize::from(*i)] = *color;
                per_cel(change)
            }
            TestStep::Rotate {
                start_2,
                largo,
                steps_2,
            } => {
                let (start_2, largo) = (usize::from(*start_2), usize::from(*largo));
                assert!(
                    start_2 + largo <= self.palette.len(),
                    "rango fuera de la paleta"
                );
                if largo < 2 {
                    return Effect::default();
                }
                let k = steps_2.rem_euclid(largo as i32) as usize;
                let original = self.palette.clone();
                for i in 0..largo {
                    self.palette[start_2 + (i + k) % largo] = original[start_2 + i];
                }
                per_cel(self.palette != original)
            }
            TestStep::Mover { de, a } => {
                let (de, a) = (usize::from(*de), usize::from(*a));
                if de == a {
                    return Effect::default();
                }
                // `order[new_2] = old`, como quitar e insertar.
                let mut order: Vec<usize> = (0..self.palette.len()).collect();
                let moved = order.remove(de);
                order.insert(a, moved);
                let mut forward = vec![0u8; order.len()];
                for (new_2, &old) in order.iter().enumerate() {
                    forward[old] = new_2 as u8;
                }
                self.palette = order.iter().map(|&old| self.palette[old]).collect();
                for c in &mut self.cels {
                    c[0] = forward[usize::from(c[0])];
                }
                self.transparent = self.transparent.map(|t| forward[usize::from(t)]);
                per_cel(true)
            }
            TestStep::Add { active_2 } => {
                assert!(self.palette.len() < 256, "la paleta está llena");
                let color = self.resolved_2(*active_2);
                self.palette.push(color);
                per_cel(true)
            }
            TestStep::Transparent(index_2) => {
                let change = self.transparent != *index_2;
                self.transparent = *index_2;
                per_cel(change)
            }
            TestStep::Convert => self.convert(),
        }
    }
}

/// RGBA expandido de una `Image`, leído de sus bytes y su paleta (con el índice transparente
/// resuelto): es la «vista RGBA» del corpus.
pub fn expand(image_2: &Image) -> Vec<u8> {
    let bytes = image_2.bytes();
    let mut v = Vec::with_capacity(bytes.len() * 4);
    match image_2.mode() {
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
            let palette = image_2.palette().expect("una indexada tiene paleta");
            let transparent = palette.transparent_index();
            for &i in bytes {
                let c = palette.colors()[usize::from(i)];
                let alpha_2 = if transparent == Some(i) { 0 } else { c.a };
                v.extend([c.r, c.g, c.b, alpha_2]);
            }
        }
    }
    v
}

/// Pánico, con el primer píxel que difiere, si la imagen no es el modelo: modo, tamaño, bytes,
/// paleta, índice transparente y RGBA expandido.
pub fn assert_equal(image_2: &Image, model: &Model, ctx: &str) {
    assert_eq!(image_2.mode(), model.mode_2, "{ctx}: modo");
    assert_eq!(
        (image_2.width() as usize, image_2.height() as usize),
        (model.width, model.height),
        "{ctx}: tamaño"
    );
    let expected_2 = model.bytes();
    if image_2.bytes() != &expected_2[..] {
        let n = bytes_per_pixel(model.mode_2);
        let pos = image_2
            .bytes()
            .iter()
            .zip(&expected_2)
            .position(|(a, b)| a != b)
            .unwrap_or(0)
            / n;
        panic!(
            "{ctx}: el píxel ({}, {}) difiere: imagen {:?}, modelo {:?}",
            pos % model.width,
            pos / model.width,
            &image_2.bytes()[pos * n..(pos + 1) * n],
            &expected_2[pos * n..(pos + 1) * n]
        );
    }
    let palette: Vec<[u8; 4]> = image_2
        .palette()
        .map(|p| p.colors().iter().map(|c| [c.r, c.g, c.b, c.a]).collect())
        .unwrap_or_default();
    assert_eq!(palette, model.palette, "{ctx}: paleta");
    assert_eq!(
        image_2.palette().and_then(Palette::transparent_index),
        model.transparent,
        "{ctx}: índice transparente"
    );
    assert!(
        expand(image_2) == model.rgba(),
        "{ctx}: el RGBA expandido no coincide con el del modelo"
    );
}

/// SHA-256 de la RGBA expandida de una imagen (la técnica del corpus).
pub fn sha_rgba(image_2: &Image) -> String {
    sha256_hex(&expand(image_2))
}

// ---------------------------------------------------------------------------------------
// Fuentes.
// ---------------------------------------------------------------------------------------

/// Un PNG de partida de la puerta.
pub struct Source {
    pub name_2: String,
    /// Los bytes del fichero.
    pub bytes: Vec<u8>,
    /// Es uno de `golden/png/` (se copia con `fs::copy`).
    pub real: bool,
    /// Lo que debe dar su apertura (generadas y construidas); los reales, `None`.
    pub expected_2: Option<Model>,
    /// SHA-256 de la RGBA expandida que dice su `.sha256` (solo los reales).
    pub sha: Option<String>,
    /// Si Pixoko debe convertirlo al abrir, cuando se sabe por construcción.
    pub converted_2: Option<bool>,
}

/// Los 4 reales: su esperado es el `.sha256` que dejó otra herramienta.
pub fn real_2() -> Vec<Source> {
    REAL.iter()
        .map(|name_2| {
            let path_2 = golden_png().join(name_2);
            let bytes =
                fs::read(&path_2).unwrap_or_else(|e| panic!("leer {}: {e}", path_2.display()));
            let path_sha = golden_png().join(format!("{name_2}.sha256"));
            let sha = fs::read_to_string(&path_sha)
                .unwrap_or_else(|e| panic!("leer {}: {e}", path_sha.display()))
                .trim()
                .to_ascii_lowercase();
            Source {
                name_2: (*name_2).to_string(),
                bytes,
                real: true,
                expected_2: None,
                sha: Some(sha),
                converted_2: None,
            }
        })
        .collect()
}

/// Una fuente construida: sus bytes se escriben tal cual y se espera `expected_2` al abrir.
fn built(name_2: &str, bytes: Vec<u8>, expected_2: Model, converted_2: bool) -> Source {
    Source {
        name_2: name_2.to_string(),
        bytes,
        real: false,
        expected_2: Some(expected_2),
        sha: None,
        converted_2: Some(converted_2),
    }
}

const WIDTH_GENERATED: usize = 37;
const HEIGHT_GENERATED: usize = 23;

fn generate(mode_2: Mode, cel: impl Fn(usize, usize) -> [u8; 4]) -> Model {
    let mut cels = Vec::new();
    for y in 0..HEIGHT_GENERATED {
        for x in 0..WIDTH_GENERATED {
            cels.push(cel(x, y));
        }
    }
    Model {
        mode_2,
        width: WIDTH_GENERATED,
        height: HEIGHT_GENERATED,
        cels,
        palette: Vec::new(),
        transparent: None,
    }
}

/// Una fuente generada con `write_png` desde el modelo; al abrir debe dar el modelo normalizado.
fn generated(name_2: &str, model: Model) -> Source {
    let bytes = write_png(&model.a_image()).expect("write_png de la fuente generada");
    built(name_2, bytes, model.normalized(), false)
}

/// Las cuatro generadas de 37×23 (que no es múltiplo de nada).
pub fn generated_2() -> Vec<Source> {
    // RGBA con píxeles de alfa 0 (con RGB distinto de cero) y de alfa 128.
    let tabla = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 128],
        [250, 250, 10, 0],
        [10, 200, 200, 128],
        [90, 90, 90, 255],
    ];
    let rgba = generate(Mode::Rgba, |x, y| tabla[(x + 2 * y) % tabla.len()]);
    // Indexada de 5 colores con alfa propio y el índice 2 transparente.
    let mut indexed = generate(Mode::Indexed, |x, y| {
        [((x * 3 + y * 5 + x / 4) % 5) as u8, 0, 0, 0]
    });
    indexed.palette = vec![
        [10, 20, 30, 255],
        [200, 100, 50, 128],
        [0, 255, 0, 255],
        [255, 255, 0, 64],
        [255, 0, 255, 255],
    ];
    indexed.transparent = Some(2);
    let gray = generate(Mode::Gray, |x, y| [((x * 5 + y * 3) % 256) as u8, 0, 0, 0]);
    let gray_alpha = generate(Mode::GrayAlpha, |x, y| {
        let alpha_2 = [0, 100, 200, 255][(x + 2 * y) % 4];
        [((x * 7 + y * 2) % 256) as u8, alpha_2, 0, 0]
    });
    vec![
        generated("generada-rgba.png", rgba),
        generated("generada-indexada.png", indexed),
        generated("generada-gris.png", gray),
        generated("generada-gris-alfa.png", gray_alpha),
    ]
}

/// Reales y generadas.
pub fn all() -> Vec<Source> {
    let mut sources = real_2();
    sources.extend(generated_2());
    sources
}

/// Un modelo pequeño de celdas dadas.
pub fn model_of(mode_2: Mode, width: usize, height: usize, cels: Vec<[u8; 4]>) -> Model {
    assert_eq!(cels.len(), width * height);
    Model {
        mode_2,
        width,
        height,
        cels,
        palette: Vec::new(),
        transparent: None,
    }
}

/// PNG RGB de 8 bits de 4×2 (Pixoko lo lee como RGBA).
pub fn source_rgb_8() -> Source {
    let pixels_2: [[u8; 3]; 8] = [
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [10, 20, 30],
        [200, 100, 50],
        [1, 2, 3],
        [255, 255, 255],
        [0, 0, 0],
    ];
    let swatches: Vec<u16> = pixels_2.iter().flatten().map(|&v| u16::from(v)).collect();
    let cels = pixels_2.iter().map(|p| [p[0], p[1], p[2], 255]).collect();
    built(
        "rgb8.png",
        png_raw(4, 2, 8, 2, &swatches, &[]),
        model_of(Mode::Rgba, 4, 2, cels),
        true,
    )
}

/// PNG de gris de 4 bits de 4×2 (se amplía a 8: `v · 17`).
pub fn source_gray_4() -> Source {
    let swatches: [u16; 8] = [0, 1, 5, 15, 7, 8, 9, 10];
    let cels = swatches
        .iter()
        .map(|&v| [(v * 17) as u8, 0, 0, 0])
        .collect();
    built(
        "gris4.png",
        png_raw(4, 2, 4, 0, &swatches, &[]),
        model_of(Mode::Gray, 4, 2, cels),
        true,
    )
}

/// PNG de gris de 16 bits de 3×2 con `tRNS` de color clave: las muestras son múltiplos de 257, así
/// que la reducción a 8 bits es exacta (`v / 257`); el píxel de la clave pasa a alfa 0.
pub fn source_gray16_trns() -> Source {
    let key = 0x8080u16;
    let swatches: [u16; 6] = [0x0000, key, 0xFFFF, 0x1010, key, 0x4242];
    let cels = swatches
        .iter()
        .map(|&v| [(v / 257) as u8, if v == key { 0 } else { 255 }, 0, 0])
        .collect();
    built(
        "gris16-trns.png",
        png_raw(
            3,
            2,
            16,
            0,
            &swatches,
            &[piece(b"tRNS", &key.to_be_bytes())],
        ),
        model_of(Mode::GrayAlpha, 3, 2, cels),
        true,
    )
}

/// PNG RGBA de 5×3 de `write_png` con un `iCCP` (la apariencia cambiaría al guardar sobre él).
pub fn source_with_iccp() -> Source {
    let cels = (0..15u8)
        .map(|i| {
            [
                i * 16,
                255 - i * 10,
                i * 3,
                if i % 4 == 0 { 128 } else { 255 },
            ]
        })
        .collect();
    let model = model_of(Mode::Rgba, 5, 3, cels);
    let bytes = with_chunks(&write_png(&model.a_image()).expect("write_png"), &[iccp()]);
    built("con-iccp.png", bytes, model, true)
}

/// Los PNG construidos en la prueba que Pixoko convierte al abrir.
pub fn converted_2_2() -> Vec<Source> {
    vec![
        source_rgb_8(),
        source_gray_4(),
        source_gray16_trns(),
        source_with_iccp(),
    ]
}

/// Un modelo RGBA de 37×23 con más de 256 colores distintos (para `TooManyColors`).
pub fn rgba_of_many_colors() -> Model {
    generate(Mode::Rgba, |x, y| {
        [(x * 6) as u8, (y * 10) as u8, ((x + y) * 3) as u8, 255]
    })
}

// ---------------------------------------------------------------------------------------
// Sesión, mundo (sesión + modelo + historia anotada) y guiones.
// ---------------------------------------------------------------------------------------

/// Una sesión sin fichero, de la que se parte para abrir.
pub fn session_empty() -> Session {
    let image_2 = Image::new(1, 1, Mode::Rgba).expect("imagen de 1×1");
    Session::sample(Document::new(image_2))
}

/// Sesión, herramientas, modelo y la historia anotada: el `state_id` y el modelo de cada paso.
pub struct World {
    pub s: Session,
    pub tools: ToolState,
    pub m: Model,
    /// `snaps[n]`: `state_id` y modelo tras `n` pasos de la rama actual.
    pub snaps: Vec<(u64, Model)>,
    /// Posición actual en `snaps` (cambia con `undo` y `redo`).
    pub pos: usize,
    /// Todos los `state_id` que ha tenido el documento: no se reutiliza ninguno.
    pub used: HashSet<u64>,
    /// `state_id` y modelo del último estado guardado (el de apertura, al principio).
    pub saved: (u64, Model),
    /// Cuántos gestos del borrador se rechazaron.
    pub rejections: usize,
    pub label_2: String,
}

impl World {
    pub fn of_session(s: Session, label_2: &str) -> World {
        let image_2 = s
            .document
            .image(ImageId::from_raw(0))
            .expect("el documento tiene imagen");
        let m = Model::of_image(image_2);
        let id = s.document.state_id();
        World {
            s,
            tools: ToolState::new(),
            snaps: vec![(id, m.clone())],
            pos: 0,
            used: HashSet::from([id]),
            saved: (id, m.clone()),
            m,
            rejections: 0,
            label_2: label_2.to_string(),
        }
    }

    pub fn image_2(&self) -> &Image {
        self.s
            .document
            .image(ImageId::from_raw(0))
            .expect("el documento tiene imagen")
    }

    /// Pasos de historia de la rama actual.
    pub fn steps_2(&self) -> usize {
        self.snaps.len() - 1
    }

    fn gesture(&mut self, tool_2: Tool, choice: Choice, events: Vec<CanvasEvent>) -> (bool, bool) {
        let id = ImageId::from_raw(0);
        self.tools.apply_action(ToolAction::Select(tool_2));
        let mut change = false;
        for event_2 in events {
            change |= self
                .tools
                .canvas_event(&mut self.s.document, id, a_color_choice(choice), event_2)
                .unwrap_or_else(|e| panic!("{}: el gesto dio error: {e}", self.label_2));
        }
        let changes_2 = self.tools.take_changes();
        self.s.absorb(changes_2);
        let rejected = self
            .tools
            .take_events()
            .iter()
            .any(|e| matches!(e, ToolEvent::Refused(_)));
        (change, rejected)
    }

    fn operation(&mut self, op: PaletteOp, active_2: u8) -> bool {
        self.s.choice = ColorChoice::Entry(active_2);
        self.s
            .apply_palette_op(op, false)
            .unwrap_or_else(|e| panic!("{}: {op:?} dio error: {e}", self.label_2))
            .changed
    }

    /// Aplica el paso a la sesión y al modelo y comprueba que coinciden: un paso de historia por
    /// gesto con cambio, ninguno sin él, `state_id` nuevo, rechazo del borrador y estado de
    /// `modified`. Devuelve si hubo cambio.
    pub fn apply(&mut self, step: &TestStep) -> bool {
        let ctx = format!("{}: {step:?}", self.label_2);
        let antes_id = self.s.document.state_id();
        let before_image = self.image_2().try_clone().expect("copia de la imagen");
        let can_undo = self.s.document.can_undo();
        let mut new_2 = self.m.clone();
        let effect = new_2.apply(step);

        let (change, rejected) = match step {
            TestStep::Pencil(path_2, e) => {
                let mut events = vec![CanvasEvent::StrokeBegin(path_2[0])];
                events.extend(path_2[1..].iter().map(|&p| CanvasEvent::StrokeMove(p)));
                events.push(CanvasEvent::StrokeEnd);
                self.gesture(Tool::Pencil, *e, events)
            }
            TestStep::Line(a, b, e) => self.gesture(
                Tool::Line,
                *e,
                vec![
                    CanvasEvent::StrokeBegin(*a),
                    CanvasEvent::StrokeMove(*b),
                    CanvasEvent::StrokeEnd,
                ],
            ),
            TestStep::Fill(seed, e) => self.gesture(
                Tool::Fill,
                *e,
                vec![CanvasEvent::StrokeBegin(*seed), CanvasEvent::StrokeEnd],
            ),
            TestStep::Eraser(path_2) => {
                let mut events = vec![CanvasEvent::StrokeBegin(path_2[0])];
                events.extend(path_2[1..].iter().map(|&p| CanvasEvent::StrokeMove(p)));
                events.push(CanvasEvent::StrokeEnd);
                self.gesture(Tool::Eraser, Choice::TestSlot(1), events)
            }
            TestStep::ColorInput(i, c) => {
                let commit = Commit {
                    target: EditTarget::Entry(*i),
                    color: Rgba::new(c[0], c[1], c[2], c[3]),
                };
                let change = self
                    .s
                    .commit_edit(commit)
                    .unwrap_or_else(|e| panic!("{ctx}: dio error: {e}"));
                (change, false)
            }
            TestStep::Rotate {
                start_2,
                largo,
                steps_2,
            } => (
                self.operation(
                    PaletteOp::Rotate {
                        start: *start_2,
                        len: *largo,
                        steps: *steps_2,
                    },
                    0,
                ),
                false,
            ),
            TestStep::Mover { de, a } => (
                self.operation(PaletteOp::Move { from: *de, to: *a }, 0),
                false,
            ),
            TestStep::Add { active_2 } => (self.operation(PaletteOp::Add, *active_2), false),
            TestStep::Transparent(index_2) => (
                self.operation(PaletteOp::SetTransparent { index: *index_2 }, 0),
                false,
            ),
            TestStep::Convert => {
                let target_2 = match self.m.mode_2 {
                    Mode::Indexed | Mode::GrayAlpha => Mode::Rgba,
                    Mode::Gray => Mode::GrayAlpha,
                    Mode::Rgba => Mode::Indexed,
                };
                match self.s.convert_mode(target_2, Allowed::NOTHING) {
                    Ok(change) => {
                        assert!(!effect.error, "{ctx}: se esperaba TooManyColors y dio Ok");
                        (change, false)
                    }
                    Err(DocumentError::Convert(ConvertError::TooManyColors { .. }))
                        if effect.error =>
                    {
                        (false, false)
                    }
                    Err(e) => panic!("{ctx}: la conversión dio {e:?}"),
                }
            }
        };

        assert_eq!(change, effect.change, "{ctx}: ¿hubo cambio?");
        assert_eq!(rejected, effect.rejected, "{ctx}: ¿se rechazó el borrador?");
        let after_id = self.s.document.state_id();
        if effect.change {
            assert_ne!(antes_id, after_id, "{ctx}: el state_id no cambió");
            assert!(
                self.used.insert(after_id),
                "{ctx}: el state_id {after_id} ya se había usado"
            );
            assert!(self.s.document.can_undo(), "{ctx}: sin paso de historia");
            self.snaps.truncate(self.pos + 1);
            self.snaps.push((after_id, new_2.clone()));
            self.pos += 1;
        } else {
            assert_eq!(antes_id, after_id, "{ctx}: el state_id cambió sin cambio");
            assert_eq!(
                self.s.document.can_undo(),
                can_undo,
                "{ctx}: cambió la historia sin cambio"
            );
            assert!(
                self.image_2() == &before_image,
                "{ctx}: la imagen cambió sin cambio"
            );
        }
        if rejected {
            self.rejections += 1;
        }
        self.m = new_2;
        self.check();
        effect.change
    }

    /// Imagen igual al modelo y `modified` coherente con el estado guardado: es falso si y solo si el
    /// `state_id` es el guardado, y nunca lo es con una imagen distinta de la guardada.
    pub fn check(&self) {
        let ctx = &self.label_2;
        assert_equal(self.image_2(), &self.m, ctx);
        let (id_saved, model_saved) = &self.saved;
        assert_eq!(
            self.s.modified(),
            self.s.document.state_id() != *id_saved,
            "{ctx}: modified no corresponde al state_id guardado"
        );
        if !self.s.modified() {
            assert!(
                &self.m == model_saved,
                "{ctx}: modified es falso con una imagen distinta de la guardada"
            );
        }
    }

    /// Anota el estado actual como el guardado (tras un `save` que salió bien).
    pub fn mark_saved(&mut self) {
        self.saved = (self.s.document.state_id(), self.m.clone());
        assert!(
            !self.s.modified(),
            "{}: modified tras guardar",
            self.label_2
        );
        self.check();
    }

    /// `undo`: da la instantánea de un paso atrás, o `NothingToUndo` al principio.
    pub fn undo(&mut self) {
        let result = self.s.undo();
        if self.pos == 0 {
            assert_eq!(
                result,
                Err(DocumentError::NothingToUndo),
                "{}: undo al principio",
                self.label_2
            );
        } else {
            result.unwrap_or_else(|e| panic!("{}: undo dio {e:?}", self.label_2));
            self.pos -= 1;
            self.m = self.snaps[self.pos].1.clone();
            assert_eq!(
                self.s.document.state_id(),
                self.snaps[self.pos].0,
                "{}: state_id tras undo",
                self.label_2
            );
        }
        self.check();
    }

    /// `redo`: da la instantánea de un paso adelante, o `NothingToRedo` al final de la rama.
    pub fn redo(&mut self) {
        let result = self.s.redo();
        if self.pos + 1 == self.snaps.len() {
            assert_eq!(
                result,
                Err(DocumentError::NothingToRedo),
                "{}: redo al final",
                self.label_2
            );
        } else {
            result.unwrap_or_else(|e| panic!("{}: redo dio {e:?}", self.label_2));
            self.pos += 1;
            self.m = self.snaps[self.pos].1.clone();
            assert_eq!(
                self.s.document.state_id(),
                self.snaps[self.pos].0,
                "{}: state_id tras redo",
                self.label_2
            );
        }
        self.check();
    }

    /// `undo` hasta el principio (y uno más, que debe fallar); devuelve cuántos hizo.
    pub fn undo_all(&mut self) -> usize {
        let mut facts = 0;
        while self.pos > 0 {
            self.undo();
            facts += 1;
        }
        self.undo();
        facts
    }

    /// `redo` hasta el final de la rama (y uno más, que debe fallar); devuelve cuántos hizo.
    pub fn redo_all(&mut self) -> usize {
        let mut facts = 0;
        while self.pos + 1 < self.snaps.len() {
            self.redo();
            facts += 1;
        }
        self.redo();
        facts
    }
}

/// Abre la fuente en un directorio de trabajo (copia de los reales) y comprueba la imagen contra su
/// referencia independiente: el modelo (generadas y construidas) o el `.sha256` (reales).
pub fn open(t: &Work, source: &Source) -> World {
    let path_2 = t.put(source);
    let mut s = session_empty();
    s.open(&path_2, Canvas::MAX_SIDE, false)
        .unwrap_or_else(|e| panic!("{}: open dio error: {e}", source.name_2));
    let image_2 = s
        .document
        .image(ImageId::from_raw(0))
        .expect("el documento tiene imagen");
    check_opening(source, image_2);
    World::of_session(s, &source.name_2)
}

/// La imagen abierta es la referencia independiente de la fuente.
pub fn check_opening(source: &Source, image_2: &Image) {
    if let Some(expected_2) = &source.expected_2 {
        assert_equal(image_2, expected_2, &source.name_2);
    }
    if let Some(sha) = &source.sha {
        assert_eq!(
            &sha_rgba(image_2),
            sha,
            "{}: el SHA-256 de la RGBA expandida no es el de su .sha256",
            source.name_2
        );
    }
}

/// Elección número `k` del color: una entrada de la paleta (módulo su longitud) o una ranura.
pub fn choice_n(m: &Model, k: usize) -> Choice {
    if m.mode_2 == Mode::Indexed {
        Choice::TestInput((k % m.palette.len()) as u8)
    } else {
        Choice::TestSlot(1 + (k % 9) as u8)
    }
}

/// Lápiz, línea, relleno y borrador: con repeticiones que no deben dar paso (el mismo trazo con el
/// mismo color y el mismo relleno).
pub fn script_tools(m: &Model) -> Vec<TestStep> {
    let (c1, c2, c3, c4) = (
        choice_n(m, 1),
        choice_n(m, 2),
        choice_n(m, 3),
        choice_n(m, 4),
    );
    vec![
        TestStep::Pencil(vec![(2, 2), (9, 2), (9, 9), (5, 5)], c1),
        TestStep::Pencil(vec![(2, 2), (9, 2)], c1),
        TestStep::Line((1, 12), (14, 12), c2),
        TestStep::Line((12, 1), (12, 10), c3),
        TestStep::Line((0, 15), (10, 5), c2),
        TestStep::Fill((0, 0), c4),
        TestStep::Fill((0, 0), c4),
        TestStep::Eraser(vec![(3, 3), (8, 3)]),
        TestStep::Eraser(vec![(3, 3), (8, 3)]),
    ]
}

/// Edición de color, `Rotate`, `Move`, `Add` y `SetTransparent` (de una indexada), con un par de
/// repeticiones que no deben dar paso.
pub fn script_palette(m: &Model) -> Vec<TestStep> {
    assert_eq!(m.mode_2, Mode::Indexed);
    let largo = m.palette.len();
    let mut steps_2 = vec![
        TestStep::ColorInput(0, [1, 2, 3, 255]),
        TestStep::ColorInput(0, [1, 2, 3, 255]),
        TestStep::Rotate {
            start_2: 0,
            largo: largo as u16,
            steps_2: 1,
        },
    ];
    if largo >= 2 {
        steps_2.push(TestStep::Mover {
            de: 0,
            a: (largo - 1) as u8,
        });
    }
    if largo < 256 {
        steps_2.push(TestStep::Add { active_2: 0 });
        steps_2.push(TestStep::ColorInput(largo as u8, [9, 8, 7, 200]));
    }
    steps_2.push(TestStep::Transparent(Some(1.min(largo - 1) as u8)));
    steps_2.push(TestStep::Rotate {
        start_2: 0,
        largo: largo.min(3) as u16,
        steps_2: -1,
    });
    // Con el índice transparente fijado, el borrador ya escribe en él.
    steps_2.push(TestStep::Eraser(vec![(3, 3), (8, 3)]));
    steps_2.push(TestStep::Transparent(Some(0)));
    steps_2.push(TestStep::Transparent(Some(0)));
    steps_2.push(TestStep::Transparent(None));
    steps_2
}

/// El guion entero: herramientas, paleta (si es indexada) y, al final, la conversión exacta.
pub fn script_complete(m: &Model) -> Vec<TestStep> {
    let mut steps_2 = script_tools(m);
    if m.mode_2 == Mode::Indexed {
        steps_2.extend(script_palette(m));
    }
    steps_2.push(TestStep::Convert);
    steps_2
}

/// Un lápiz de un píxel en (0, 0) con un color que cambia ese píxel, o `None` si ninguno lo hace.
pub fn edit_that_changes_opt(m: &Model) -> Option<TestStep> {
    let choices: Vec<Choice> = if m.mode_2 == Mode::Indexed {
        (0..m.palette.len().min(256))
            .map(|i| Choice::TestInput(i as u8))
            .collect()
    } else {
        (1..=9).map(Choice::TestSlot).collect()
    };
    choices.into_iter().find_map(|e| {
        let step = TestStep::Pencil(vec![(0, 0)], e);
        let mut copy = m.clone();
        copy.apply(&step).change.then_some(step)
    })
}

/// Como `edit_that_changes_opt`, con pánico si no hay ninguna.
pub fn editing_that_changes(m: &Model) -> TestStep {
    edit_that_changes_opt(m).expect("alguna edición cambia el píxel (0, 0)")
}

/// Lee un PNG del disco con `read_png`, con pánico si falla.
pub fn leer(path_2: &Path) -> px_formats::PngRead {
    let bytes = fs::read(path_2).unwrap_or_else(|e| panic!("leer {}: {e}", path_2.display()));
    read_png(&bytes).unwrap_or_else(|e| panic!("read_png de {}: {e}", path_2.display()))
}

// ---------------------------------------------------------------------------------------
// El selector de archivos falso.
// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct StateSelector {
    startups: usize,
    last_2: Option<DialogRequest>,
    /// `None`: sigue abierto. `Some(r)`: lo que dará el próximo `poll`.
    result: Option<Option<PathBuf>>,
}

/// Mando del test sobre el selector que se entregó a `Dialogs`: resuelve cuando el test lo dice.
#[derive(Clone, Default)]
pub struct TestControl(Rc<RefCell<StateSelector>>);

impl TestControl {
    /// El usuario elige `path_2`.
    pub fn choose(&self, path_2: &Path) {
        self.0.borrow_mut().result = Some(Some(path_2.to_path_buf()));
    }

    /// El diálogo se cierra sin ruta.
    pub fn close(&self) {
        self.0.borrow_mut().result = Some(None);
    }

    /// La última petición que recibió el selector.
    pub fn last_2(&self) -> DialogRequest {
        self.0
            .borrow()
            .last_2
            .clone()
            .expect("el selector recibió alguna petición")
    }

    pub fn startups(&self) -> usize {
        self.0.borrow().startups
    }
}

struct SelectorFalse(TestControl);

impl FilePicker for SelectorFalse {
    fn start(&mut self, request: &DialogRequest, parent: Option<&eframe::Frame>) {
        assert!(parent.is_none(), "las pruebas no tienen ventana");
        let mut state_2 = self.0.0.borrow_mut();
        state_2.startups += 1;
        state_2.last_2 = Some(request.clone());
        state_2.result = None;
    }

    fn poll(&mut self) -> Option<Option<PathBuf>> {
        self.0.0.borrow_mut().result.take()
    }

    fn abandon(&mut self) {
        self.0.0.borrow_mut().result = None;
    }
}

/// `Dialogs` como los monta la app (fallo rápido solo en Linux), sobre un selector falso.
pub fn dialogs_2(cwd: &Path) -> (Dialogs, TestControl) {
    let control = TestControl::default();
    let dialogs_2 = Dialogs::new(
        Box::new(SelectorFalse(control.clone())),
        cfg!(target_os = "linux"),
        Some(cwd.to_path_buf()),
    );
    (dialogs_2, control)
}
