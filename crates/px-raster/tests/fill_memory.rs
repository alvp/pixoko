//! Pruebas de memoria y de tiempo del relleno en lienzos de 8192 x 8192
//! (specs/hito-2-limites-relleno.md, filas 12 y 14).
//!
//! Todas son `#[ignore]` y solo existen en Linux: miden `VmHWM` (el RSS máximo del **proceso**,
//! de `/proc/self/status`), así que cada una debe ejecutarse sola, en release, en su propio
//! proceso. `--exact` y el nombre completo dejan fuera a las demás:
//!
//! ```text
//! cargo test -p px-raster --test fill_memory --release -- --ignored --exact --nocapture fila_12_peine_indexado_8192
//! cargo test -p px-raster --test fill_memory --release -- --ignored --exact --nocapture fila_14_uniforme_rgba_8192
//! cargo test -p px-raster --test fill_memory --release -- --ignored --exact --nocapture fila_14_uniforme_indexado_8192
//! ```
//!
//! Las tolerancias son generosas (detectan una regresión de cientos de MiB, no de unos pocos):
//! la fila 12 exige que el relleno rechazado crezca menos de 256 MiB (sin tope serían más de
//! 1 GiB; con él, unos 8 MiB) y la fila 14, menos de 128 MiB en indexado. Los tiempos se
//! imprimen y solo hacen fallar por encima de 30 s (fila 12) y de 60 s (fila 14).

#![cfg(target_os = "linux")]

use std::time::{Duration, Instant};

use px_core::{Command, Document, DocumentError, Image, Mode, Palette, Pixel, Rgba};
use px_raster::fill_command;

const LADO: u32 = 8192;
const MIB: u64 = 1024 * 1024;

/// `VmHWM` en bytes: el RSS máximo del proceso hasta ahora.
fn vm_hwm() -> u64 {
    let estado = std::fs::read_to_string("/proc/self/status").expect("hay /proc/self/status");
    let linea = estado
        .lines()
        .find(|l| l.starts_with("VmHWM:"))
        .expect("hay una línea VmHWM");
    let kib: u64 = linea
        .split_whitespace()
        .nth(1)
        .expect("VmHWM tiene un valor")
        .parse()
        .expect("VmHWM es un número");
    kib * 1024
}

fn paleta() -> Palette {
    Palette::from_colors([Rgba::opaque(10, 20, 30), Rgba::opaque(200, 100, 50)]).unwrap()
}

fn a_rgba() -> Pixel {
    Pixel::Rgba(Rgba::new(10, 20, 30, 255))
}

fn c_rgba() -> Pixel {
    Pixel::Rgba(Rgba::new(200, 100, 50, 255))
}

/// Peine de 8192 x 8192 indexado (índice 0 = `A`, 1 = `B`): fila 0 toda `A`; en el resto, `A`
/// en las columnas pares y `B` en las impares. `S = 1 + 8191 * 4096 = 33 550 337` tramos.
fn peine_indexado() -> Image {
    let lado = LADO as usize;
    let mut data = Vec::with_capacity(lado * lado);
    data.resize(lado, 0u8);
    for _ in 1..lado {
        data.extend((0..lado).map(|x| u8::from(x % 2 == 1)));
    }
    Image::from_bytes_indexed(LADO, LADO, paleta(), data).unwrap()
}

fn spans_de(comando: &Command) -> usize {
    match comando {
        Command::FillSpans { spans, .. } => spans.len(),
        otro => panic!("se esperaba FillSpans, salió {otro:?}"),
    }
}

/// Fila 12: el peine indexado se rechaza con `FillTooFragmented` sin disparar la memoria.
#[test]
#[ignore = "mide VmHWM: ejecutar sola y en release (ver la cabecera del fichero)"]
fn fila_12_peine_indexado_8192() {
    let doc = Document::new(peine_indexado());
    let id = doc.main_image();

    let base = vm_hwm();
    let inicio = Instant::now();
    let resultado = fill_command(&doc, id, 0, 0, Pixel::Indexed(1));
    let tiempo = inicio.elapsed();
    let crecimiento = vm_hwm().saturating_sub(base);

    println!(
        "fila 12: {resultado:?} en {tiempo:?}; VmHWM base {} MiB, crece {} MiB",
        base / MIB,
        crecimiento / MIB
    );
    assert!(
        matches!(
            resultado,
            Err(DocumentError::FillTooFragmented {
                limit: Document::MAX_FILL_SPANS
            })
        ),
        "se esperaba FillTooFragmented con el tope real, salió {resultado:?}"
    );
    assert!(
        crecimiento < 256 * MIB,
        "VmHWM creció {} MiB (el límite es 256)",
        crecimiento / MIB
    );
    assert!(tiempo < Duration::from_secs(30), "tardó {tiempo:?}");
}

/// Fila 14, comprobaciones comunes: relleno de toda la imagen con 8192 tramos, `apply`, `undo` y
/// `redo` restauran los bytes. Devuelve el crecimiento de `VmHWM` sobre `base`.
fn rellena_toda_la_imagen(
    mut doc: Document,
    original: &Image,
    rellena: &Image,
    pixel: Pixel,
    etiqueta: &str,
) -> u64 {
    let id = doc.main_image();
    let base = vm_hwm();
    let inicio = Instant::now();

    let t0 = Instant::now();
    let comando = fill_command(&doc, id, 4096, 4096, pixel);
    let t_fill = t0.elapsed();
    let comando = match comando {
        Ok(Some(c)) => c,
        otro => panic!("{etiqueta}: se esperaba Ok(Some(..)), salió {otro:?}"),
    };
    assert_eq!(spans_de(&comando), 8192, "{etiqueta}: un tramo por fila");

    let t1 = Instant::now();
    assert_eq!(doc.apply(comando), Ok(true), "{etiqueta}: apply");
    let t_apply = t1.elapsed();
    assert_eq!(
        doc.image(id).unwrap().bytes(),
        rellena.bytes(),
        "{etiqueta}: la imagen rellena"
    );

    let t2 = Instant::now();
    assert_eq!(doc.undo(), Ok(()));
    let t_undo = t2.elapsed();
    assert_eq!(
        doc.image(id).unwrap().bytes(),
        original.bytes(),
        "{etiqueta}: undo restaura los bytes"
    );

    let t3 = Instant::now();
    assert_eq!(doc.redo(), Ok(()));
    let t_redo = t3.elapsed();
    assert_eq!(
        doc.image(id).unwrap().bytes(),
        rellena.bytes(),
        "{etiqueta}: redo vuelve a rellenar"
    );

    let total = inicio.elapsed();
    let crecimiento = vm_hwm().saturating_sub(base);
    println!(
        "fila 14 {etiqueta}: fill_command {t_fill:?}, apply {t_apply:?}, undo {t_undo:?}, \
         redo {t_redo:?}, total {total:?}; VmHWM base {} MiB, crece {} MiB",
        base / MIB,
        crecimiento / MIB
    );
    assert!(
        total < Duration::from_secs(60),
        "{etiqueta}: tardó {total:?}"
    );
    crecimiento
}

/// Fila 14: toda la imagen RGBA de un color sigue rellenándose (la memoria solo se imprime).
#[test]
#[ignore = "mide VmHWM: ejecutar sola y en release (ver la cabecera del fichero)"]
fn fila_14_uniforme_rgba_8192() {
    let original = Image::filled(LADO, LADO, a_rgba()).unwrap();
    let rellena = Image::filled(LADO, LADO, c_rgba()).unwrap();
    let doc = Document::new(original.clone());
    rellena_toda_la_imagen(doc, &original, &rellena, c_rgba(), "RGBA");
}

/// Fila 14: lo mismo en indexado, con `VmHWM` creciendo menos de 128 MiB.
#[test]
#[ignore = "mide VmHWM: ejecutar sola y en release (ver la cabecera del fichero)"]
fn fila_14_uniforme_indexado_8192() {
    let original = Image::filled_indexed(LADO, LADO, paleta(), 0).unwrap();
    let rellena = Image::filled_indexed(LADO, LADO, paleta(), 1).unwrap();
    assert_eq!(original.mode(), Mode::Indexed);
    let doc = Document::new(original.clone());
    let crecimiento =
        rellena_toda_la_imagen(doc, &original, &rellena, Pixel::Indexed(1), "indexado");
    assert!(
        crecimiento < 128 * MIB,
        "VmHWM creció {} MiB (el límite es 128)",
        crecimiento / MIB
    );
}
