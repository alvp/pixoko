//! Puerta del hito 3, parte de `px-app` (specs/hito-3-puerta.md, unidad 3.9, filas 5 a 11 y
//! propiedades 1 a 3): una sesión con capas, mezclas, combinar y aplanar, convertir de modo,
//! deshacer y rehacer, guardar y reabrir en `.aseprite` y PNG aplanado, en RGBA y en indexado.
//!
//! No hay código de producción nuevo: estas pruebas ejercen lo que ya existe, así que **nacen en
//! verde**; la prueba de que saben fallar son los controles negativos (fila 11 aquí y fila 4 en
//! `px-golden`).
//!
//! Reparto:
//! - `model`: el modelo ingenuo de la pila (rejilla de celdas por capa, órdenes con código
//!   ingenuo); no reimplementa la composición (la fijan 3.2 y 3.5 con 46 goldens de Aseprite).
//! - `world`: la sesión real, las herramientas y el modelo, comprobados paso a paso.
//! - `scripts`: los guiones de punta a punta (RGBA e indexado), con vectores a mano de 3.2.
//! - `rows`: las filas 5 a 9 y 11; `large`: la fila 10, `#[ignore]`; `properties`: 1 a 3.
//!
//! Reglas que cumplen todas las pruebas:
//! - **Ninguna escritura sale de `target/pixoko-gate/`** (el directorio de trabajo de la puerta 2,
//!   que se borra siempre): los casos del corpus se copian allí y se abre la copia; toda ruta de
//!   escritura pasa por `outside_golden_aseprite` (y por `outside_of_golden`).
//! - **`golden/aseprite/` queda igual**: instantánea y SHA-256 de cada fichero al principio y al
//!   terminar cada prueba.

mod large;
mod model;
mod properties;
mod rows;
mod scripts;
mod world;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use px_core::{Image, Pixel, Rgba};

use super::common::sha256::sha256_hex;
use super::common::{
    TestSnapshot, Work, compare_snapshots, outside_of_golden, root_repo, snapshot_of,
};

pub fn golden_aseprite() -> PathBuf {
    root_repo().join("golden").join("aseprite")
}

/// La ruta con sus enlaces y `..` resueltos: se canonicaliza el ancestro más largo que exista y se
/// le añade el resto (la ruta puede no existir todavía).
fn resolve(path_2: &Path) -> PathBuf {
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
            (Some(parent), Some(name_2)) => {
                rest.push(name_2.to_os_string());
                actual = parent.to_path_buf();
            }
            _ => return absolute,
        }
    }
}

/// Pánico si `path_2` (de escritura) cuelga de `golden/aseprite/` o de `golden/png/`. Se le pasa
/// **toda** ruta de escritura de la puerta de capas.
pub fn outside_golden_aseprite(path_2: &Path) {
    let forbidden = resolve(&golden_aseprite());
    let target_2 = resolve(path_2);
    assert!(
        !target_2.starts_with(&forbidden),
        "escritura prohibida en golden/aseprite: {} (resuelta a {})",
        path_2.display(),
        target_2.display()
    );
    outside_of_golden(path_2);
}

/// SHA-256 de cada fichero de una carpeta (un subdirectorio, con barra final y sin huella).
pub fn fingerprints(dir: &Path) -> BTreeMap<String, String> {
    snapshot_of(dir)
        .into_iter()
        .map(|(name_2, bytes)| {
            let fingerprint = if name_2.ends_with('/') {
                String::new()
            } else {
                sha256_hex(&bytes)
            };
            (name_2, fingerprint)
        })
        .collect()
}

static INITIAL: OnceLock<(TestSnapshot, BTreeMap<String, String>)> = OnceLock::new();

/// Instantánea y huellas de `golden/aseprite/` al empezar la puerta de capas: la toma la primera
/// prueba que la pide (todas lo hacen al crear su `Work3`, antes de escribir nada).
pub fn initial() -> &'static (TestSnapshot, BTreeMap<String, String>) {
    INITIAL.get_or_init(|| {
        let dir = golden_aseprite();
        (snapshot_of(&dir), fingerprints(&dir))
    })
}

/// `Err` con los problemas si `golden/aseprite/` ya no es la instantánea inicial, por bytes y por
/// SHA-256 de cada fichero.
pub fn check_golden_aseprite() -> Result<(), String> {
    let dir = golden_aseprite();
    let (snapshot, hashes) = initial();
    compare_snapshots(snapshot, &snapshot_of(&dir))?;
    let now = fingerprints(&dir);
    if &now != hashes {
        return Err("alguna huella SHA-256 de golden/aseprite cambió".to_string());
    }
    Ok(())
}

/// Directorio de trabajo de la puerta de capas: el de la puerta 2 (`target/pixoko-gate/…`, se borra
/// siempre) más la vigilancia de `golden/aseprite/` al soltarse.
pub struct Work3 {
    pub t: Work,
}

impl Work3 {
    pub fn new_2() -> Work3 {
        // La instantánea inicial se toma antes de que nada pueda escribir.
        initial();
        let t = Work::new_2();
        outside_golden_aseprite(t.path_2());
        Work3 { t }
    }

    pub fn path_2(&self) -> &Path {
        self.t.path_2()
    }

    /// Nombres de lo que hay en el directorio de trabajo, ordenados.
    pub fn names(&self) -> Vec<String> {
        self.t.names()
    }

    /// Copia un caso del corpus (solo lectura) al directorio de trabajo con el nombre dado.
    pub fn copy_of_the_corpus(&self, case: &str, as_word: &str) -> PathBuf {
        let target_2 = self.path_2().join(as_word);
        outside_golden_aseprite(&target_2);
        fs::copy(
            golden_aseprite().join(format!("{case}.aseprite")),
            &target_2,
        )
        .unwrap_or_else(|e| panic!("copiar {case} al trabajo: {e}"));
        target_2
    }
}

impl Drop for Work3 {
    fn drop(&mut self) {
        if !std::thread::panicking()
            && let Err(problems) = check_golden_aseprite()
        {
            panic!("golden/aseprite/ ya no es el de antes de la puerta de capas: {problems}");
        }
    }
}

/// El píxel como RGBA resuelto (los indexados, por su paleta).
pub fn rgba_of(image_2: &Image, x: u32, y: u32) -> Rgba {
    match image_2.get_pixel(x, y).expect("píxel dentro") {
        Pixel::Rgba(c) => c,
        Pixel::Indexed(i) => image_2
            .palette()
            .and_then(|p| p.resolved(i).ok())
            .unwrap_or_else(|| panic!("índice {i} sin entrada")),
        other_2 => panic!("se esperaba RGBA o indexado, hay {other_2:?}"),
    }
}

/// Dos composiciones iguales con la exención **exacta** de la decisión 3: el alfa es exacto y un
/// píxel con alfa mayor que 0 que difiera en cualquier canal falla siempre; solo se exime el RGB
/// cuando los dos píxeles tienen alfa 0.
pub fn equal_except_rgb_with_alpha_0(a: &Image, b: &Image) -> Result<(), String> {
    if (a.width(), a.height()) != (b.width(), b.height()) {
        return Err("composiciones de distinto tamaño".to_string());
    }
    for y in 0..a.height() {
        for x in 0..a.width() {
            let (pa, pb) = (rgba_of(a, x, y), rgba_of(b, x, y));
            if pa.a != pb.a {
                return Err(format!("píxel ({x}, {y}): alfa {} y {}", pa.a, pb.a));
            }
            if pa.a > 0 && pa != pb {
                return Err(format!("píxel ({x}, {y}) con alfa > 0: {pa:?} y {pb:?}"));
            }
        }
    }
    Ok(())
}
