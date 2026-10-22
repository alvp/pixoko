//! Fila 9 de la unidad 3.8 (specs/hito-3-aseprite.md): `read_aseprite` rechaza **antes de reservar**
//! los ficheros que declaran mucho más de lo que sus datos pueden dar (una celda de 65 535 × 65 535
//! con unos bytes de zlib, una celda de 256 MiB con unos bytes, un lienzo de 65 535 × 65 535).
//!
//! Mismo método que `png_reserve.rs`: ejecutable propio y una sola prueba secuencial, porque mide
//! el pico de RSS del **proceso**. Solo Linux: lee `VmHWM` y `VmRSS` de `/proc/self/status` y
//! reinicia el máximo escribiendo `5` en `/proc/self/clear_refs` (sin `unsafe`). Cada fichero falso
//! sube el pico **menos de 32 MiB**, y el control (un lienzo indexado de 8192², cuya celda de 64 MiB
//! se rellena con un valor distinto de cero) lo sube **48 MiB o más**, para demostrar que el
//! instrumento mide.
//!
//! Regla de 1032× por celda (specs/hito-3-aseprite.md, «Límites del lector»): un flujo zlib
//! **válido** de 128 MiB de ceros (unos 130 KB) en una celda que declara 512 MiB. La razón
//! declarada (unos 4000×) pasa de lo que DEFLATE puede dar (1032×), así que el lector responde
//! `CelData` **antes de descomprimir**; sin la regla, la descompresión con tope también acabaría
//! en `CelData`, pero después de producir 128 MiB. Por eso lo que distingue las dos rutas es el
//! pico de RSS (menos de 32 MiB), y el control, la misma celda declarada con su tamaño real
//! (128 MiB), que se descomprime de verdad y sube el pico 64 MiB o más. Quitar la regla de
//! `read.rs` hace fallar esta prueba.
//!
//! Si `/proc/self/clear_refs` no se puede escribir (algunos contenedores), la prueba se omite con un
//! aviso en stderr, salvo que la variable de entorno `CI` esté definida: entonces falla.
//!
//! Necesita `miniz_oxide = "0.8.9"` en `[dependencies]` de `px-formats` (por `writer.rs`).

#![cfg(target_os = "linux")]

use std::fs;

use px_formats::{AsepriteError, AsepriteLimits, read_aseprite};

#[path = "aseprite/writer.rs"]
#[allow(dead_code)]
mod writer;
use writer::*;

const MIB: u64 = 1 << 20;

/// Valor en bytes de un campo de `/proc/self/status` (`VmHWM`, `VmRSS`; vienen en kB).
fn campo(nombre: &str) -> u64 {
    let status =
        fs::read_to_string("/proc/self/status").expect("no se pudo leer /proc/self/status");
    let linea = status
        .lines()
        .find_map(|l| l.strip_prefix(nombre)?.strip_prefix(':'))
        .unwrap_or_else(|| panic!("falta {nombre} en /proc/self/status"));
    let kb: u64 = linea
        .split_whitespace()
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("{nombre} ilegible: {linea:?}"));
    kb * 1024
}

/// Reinicia el máximo de RSS del proceso. Algunos contenedores no dejan escribir en
/// `/proc/self/clear_refs`.
fn reinicia_pico() -> std::io::Result<()> {
    fs::write("/proc/self/clear_refs", "5")
}

/// Ejecuta `f` y devuelve su resultado y cuánto subió el pico de RSS del proceso (en bytes) por
/// encima del RSS de antes de empezar.
fn mide<T>(f: impl FnOnce() -> T) -> (T, u64) {
    reinicia_pico().expect("no se pudo reiniciar el máximo de RSS (/proc/self/clear_refs)");
    let antes = campo("VmRSS");
    let resultado = f();
    let pico = campo("VmHWM");
    (resultado, pico.saturating_sub(antes))
}

/// Un lienzo de 8 × 8 con una capa y una celda que declara `ancho × alto` píxeles RGBA, con un
/// flujo zlib mínimo de un byte (unos 9 bytes).
fn celda_falsa(ancho: u16, alto: u16) -> Vec<u8> {
    let celda = Celda {
        ancho,
        alto,
        comprimido: Some(zlib(&[0])),
        ..Celda::nueva(0, 0, 0, 1, 1, vec![0])
    };
    Fichero::rgba(
        8,
        8,
        vec![perfil_srgb(), Capa::nueva("a").chunk(), celda.chunk()],
    )
    .bytes()
}

/// Un lienzo de 8 × 8 con una capa y una celda que declara `ancho × alto` píxeles RGBA con el flujo
/// zlib `flujo` (que puede ser válido y descomprimirse a mucho más de lo que cabría en el fichero).
fn celda_con_flujo(ancho: u16, alto: u16, flujo: Vec<u8>) -> Vec<u8> {
    let celda = Celda {
        ancho,
        alto,
        comprimido: Some(flujo),
        ..Celda::nueva(0, 0, 0, 1, 1, vec![0])
    };
    Fichero::rgba(
        8,
        8,
        vec![perfil_srgb(), Capa::nueva("a").chunk(), celda.chunk()],
    )
    .bytes()
}

fn es_cel_data(r: &Result<px_formats::AsepriteRead, AsepriteError>) -> bool {
    matches!(r, Err(AsepriteError::CelData { .. }))
}

#[test]
fn fila_09_bombas_se_rechazan_antes_de_reservar_y_el_control_mide() {
    // Sin poder reiniciar el pico no hay instrumento: se salta, salvo en el CI (variable `CI`
    // definida), donde un fallo aquí es una regresión del entorno y debe verse.
    if let Err(e) = reinicia_pico() {
        assert!(
            std::env::var_os("CI").is_none(),
            "no se pudo escribir /proc/self/clear_refs en el CI: {e}"
        );
        eprintln!(
            "OMITIDA fila_09_bombas_se_rechazan_antes_de_reservar_y_el_control_mide: no se puede \
             escribir /proc/self/clear_refs ({e}); sin él no se mide el pico de RSS"
        );
        return;
    }
    let limites = AsepriteLimits::default();
    // (nombre, fichero falso, ¿el error esperado?): celdas de 65 535², de 8192² y un lienzo de
    // 65 535² con la capa y nada más.
    let lienzo_enorme = Fichero::rgba(65_535, 65_535, vec![Capa::nueva("a").chunk()]).bytes();
    // Un flujo zlib válido de 128 MiB de ceros; el búfer se suelta antes de medir.
    let flujo = zlib(&vec![0u8; (128 * MIB) as usize]);
    assert!(
        flujo.len() as u64 * 1032 < 512 * MIB,
        "el flujo de ceros ({} bytes) debe quedar muy por debajo de 512 MiB / 1032",
        flujo.len()
    );
    let casos: [(&str, Vec<u8>, bool); 4] = [
        (
            "celda de 65535x65535 con 9 bytes de zlib",
            celda_falsa(65_535, 65_535),
            true,
        ),
        (
            "celda de 8192x8192 con 9 bytes de zlib",
            celda_falsa(8192, 8192),
            true,
        ),
        ("lienzo de 65535x65535", lienzo_enorme, false),
        (
            "celda de 512 MiB declarada con un flujo válido de 128 MiB de ceros (razón > 1032)",
            celda_con_flujo(8192, 16_384, flujo.clone()),
            true,
        ),
    ];
    let mut fallos = Vec::new();
    for (nombre, bytes, es_celda) in &casos {
        let (r, subida) = mide(|| read_aseprite(bytes, &limites));
        let bien = if *es_celda {
            es_cel_data(&r)
        } else {
            matches!(r, Err(AsepriteError::TooLarge { .. }))
        };
        if !bien {
            fallos.push(format!(
                "{nombre}: error distinto del esperado: {:?}",
                r.as_ref().err()
            ));
        }
        drop(r);
        if subida >= 32 * MIB {
            fallos.push(format!(
                "{nombre}: el pico de RSS subió {} MiB (límite: menos de 32)",
                subida / MIB
            ));
        }
    }

    // Control: un indexado de 8192² (64 MiB de celda, rellena con el índice transparente 5, que
    // no es cero, así que se escribe de verdad) y una celda de un píxel.
    let control = Fichero::indexado(
        8192,
        8192,
        5,
        8,
        vec![
            perfil_srgb(),
            paleta_completa(&[[0, 0, 0, 255]; 8]),
            Capa::nueva("a").chunk(),
            Celda::nueva(0, 0, 0, 1, 1, vec![0]).chunk(),
        ],
    )
    .bytes();
    let (r, subida) = mide(|| read_aseprite(&control, &limites));
    assert!(r.is_ok(), "el control debe ser Ok: {:?}", r.as_ref().err());
    assert!(
        subida >= 48 * MIB,
        "el instrumento no mide: el control subió solo {} MiB (se esperaban 48 o más)",
        subida / MIB
    );
    drop(r);

    // Control de la regla de 1032×: el mismo flujo en una celda que declara su tamaño real
    // (128 MiB, razón por debajo de 1032) se descomprime de verdad: es `Ok` (todo ceros, la celda
    // queda vacía) y el pico sube, lo que prueba que la descompresión se vería en la medida.
    let real = celda_con_flujo(8192, 4096, flujo);
    let (r, subida) = mide(|| read_aseprite(&real, &limites));
    assert!(r.is_ok(), "el control debe ser Ok: {:?}", r.as_ref().err());
    assert!(
        subida >= 64 * MIB,
        "el instrumento no ve la descompresión: el control subió solo {} MiB (se esperaban 64 o más)",
        subida / MIB
    );
    drop(r);

    assert!(fallos.is_empty(), "{}", fallos.join("\n"));
}
