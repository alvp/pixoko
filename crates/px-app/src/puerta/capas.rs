//! Puerta del hito 3, parte de `px-app` (specs/hito-3-puerta.md, unidad 3.9, filas 5 a 11 y
//! propiedades 1 a 3): una sesión con capas, mezclas, combinar y aplanar, convertir de modo,
//! deshacer y rehacer, guardar y reabrir en `.aseprite` y PNG aplanado, en RGBA y en indexado.
//!
//! No hay código de producción nuevo: estas pruebas ejercen lo que ya existe, así que **nacen en
//! verde**; la prueba de que saben fallar son los controles negativos (fila 11 aquí y fila 4 en
//! `px-golden`).
//!
//! Reparto:
//! - `modelo`: el modelo ingenuo de la pila (rejilla de celdas por capa, órdenes con código
//!   ingenuo); no reimplementa la composición (la fijan 3.2 y 3.5 con 46 goldens de Aseprite).
//! - `mundo`: la sesión real, las herramientas y el modelo, comprobados paso a paso.
//! - `guiones`: los guiones de punta a punta (RGBA e indexado), con vectores a mano de 3.2.
//! - `filas`: las filas 5 a 9 y 11; `grande`: la fila 10, `#[ignore]`; `propiedades`: 1 a 3.
//!
//! Reglas que cumplen todas las pruebas:
//! - **Ninguna escritura sale de `target/pixoko-gate/`** (el directorio de trabajo de la puerta 2,
//!   que se borra siempre): los casos del corpus se copian allí y se abre la copia; toda ruta de
//!   escritura pasa por `fuera_de_golden_aseprite` (y por `fuera_de_golden`).
//! - **`golden/aseprite/` queda igual**: instantánea y SHA-256 de cada fichero al principio y al
//!   terminar cada prueba.

mod filas;
mod grande;
mod guiones;
mod modelo;
mod mundo;
mod propiedades;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use px_core::{Image, Pixel, Rgba};

use super::comun::sha256::sha256_hex;
use super::comun::{
    Instantanea, Trabajo, comparar_instantaneas, fuera_de_golden, instantanea_de, raiz_repo,
};

pub fn golden_aseprite() -> PathBuf {
    raiz_repo().join("golden").join("aseprite")
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

/// Pánico si `ruta` (de escritura) cuelga de `golden/aseprite/` o de `golden/png/`. Se le pasa
/// **toda** ruta de escritura de la puerta de capas.
pub fn fuera_de_golden_aseprite(ruta: &Path) {
    let prohibida = resolver(&golden_aseprite());
    let destino = resolver(ruta);
    assert!(
        !destino.starts_with(&prohibida),
        "escritura prohibida en golden/aseprite: {} (resuelta a {})",
        ruta.display(),
        destino.display()
    );
    fuera_de_golden(ruta);
}

/// SHA-256 de cada fichero de una carpeta (un subdirectorio, con barra final y sin huella).
pub fn huellas(dir: &Path) -> BTreeMap<String, String> {
    instantanea_de(dir)
        .into_iter()
        .map(|(nombre, bytes)| {
            let huella = if nombre.ends_with('/') {
                String::new()
            } else {
                sha256_hex(&bytes)
            };
            (nombre, huella)
        })
        .collect()
}

static INICIAL: OnceLock<(Instantanea, BTreeMap<String, String>)> = OnceLock::new();

/// Instantánea y huellas de `golden/aseprite/` al empezar la puerta de capas: la toma la primera
/// prueba que la pide (todas lo hacen al crear su `Trabajo3`, antes de escribir nada).
pub fn inicial() -> &'static (Instantanea, BTreeMap<String, String>) {
    INICIAL.get_or_init(|| {
        let dir = golden_aseprite();
        (instantanea_de(&dir), huellas(&dir))
    })
}

/// `Err` con los problemas si `golden/aseprite/` ya no es la instantánea inicial, por bytes y por
/// SHA-256 de cada fichero.
pub fn comprobar_golden_aseprite() -> Result<(), String> {
    let dir = golden_aseprite();
    let (instantanea, hashes) = inicial();
    comparar_instantaneas(instantanea, &instantanea_de(&dir))?;
    let ahora = huellas(&dir);
    if &ahora != hashes {
        return Err("alguna huella SHA-256 de golden/aseprite cambió".to_string());
    }
    Ok(())
}

/// Directorio de trabajo de la puerta de capas: el de la puerta 2 (`target/pixoko-gate/…`, se borra
/// siempre) más la vigilancia de `golden/aseprite/` al soltarse.
pub struct Trabajo3 {
    pub t: Trabajo,
}

impl Trabajo3 {
    pub fn nuevo() -> Trabajo3 {
        // La instantánea inicial se toma antes de que nada pueda escribir.
        inicial();
        let t = Trabajo::nuevo();
        fuera_de_golden_aseprite(t.ruta());
        Trabajo3 { t }
    }

    pub fn ruta(&self) -> &Path {
        self.t.ruta()
    }

    /// Nombres de lo que hay en el directorio de trabajo, ordenados.
    pub fn nombres(&self) -> Vec<String> {
        self.t.nombres()
    }

    /// Copia un caso del corpus (solo lectura) al directorio de trabajo con el nombre dado.
    pub fn copia_del_corpus(&self, caso: &str, como: &str) -> PathBuf {
        let destino = self.ruta().join(como);
        fuera_de_golden_aseprite(&destino);
        fs::copy(golden_aseprite().join(format!("{caso}.aseprite")), &destino)
            .unwrap_or_else(|e| panic!("copiar {caso} al trabajo: {e}"));
        destino
    }
}

impl Drop for Trabajo3 {
    fn drop(&mut self) {
        if !std::thread::panicking()
            && let Err(problemas) = comprobar_golden_aseprite()
        {
            panic!("golden/aseprite/ ya no es el de antes de la puerta de capas: {problemas}");
        }
    }
}

/// El píxel como RGBA resuelto (los indexados, por su paleta).
pub fn rgba_de(imagen: &Image, x: u32, y: u32) -> Rgba {
    match imagen.get_pixel(x, y).expect("píxel dentro") {
        Pixel::Rgba(c) => c,
        Pixel::Indexed(i) => imagen
            .palette()
            .and_then(|p| p.resolved(i).ok())
            .unwrap_or_else(|| panic!("índice {i} sin entrada")),
        otro => panic!("se esperaba RGBA o indexado, hay {otro:?}"),
    }
}

/// Dos composiciones iguales con la exención **exacta** de la decisión 3: el alfa es exacto y un
/// píxel con alfa mayor que 0 que difiera en cualquier canal falla siempre; solo se exime el RGB
/// cuando los dos píxeles tienen alfa 0.
pub fn igual_salvo_rgb_con_alfa_0(a: &Image, b: &Image) -> Result<(), String> {
    if (a.width(), a.height()) != (b.width(), b.height()) {
        return Err("composiciones de distinto tamaño".to_string());
    }
    for y in 0..a.height() {
        for x in 0..a.width() {
            let (pa, pb) = (rgba_de(a, x, y), rgba_de(b, x, y));
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
