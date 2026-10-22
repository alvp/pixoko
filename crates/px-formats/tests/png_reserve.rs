//! Fila 4 de la unidad 13 (specs/hito-2-png-flujo.md): `read_png` rechaza **antes de reservar**
//! los ficheros cuyos datos comprimidos no pueden dar los bytes que declara la cabecera.
//!
//! Ejecutable propio y una sola prueba secuencial: mide el pico de RSS del **proceso**, y otras
//! pruebas en hilos del mismo proceso lo ensuciarían. Solo Linux: lee `VmHWM` y `VmRSS` de
//! `/proc/self/status` y reinicia el máximo escribiendo `5` en `/proc/self/clear_refs` (sin
//! `unsafe`). Tolerancia generosa: el fichero falso sube el pico **menos de 32 MiB** (hoy 35 a
//! 1024 MiB) y el control (gris 8192² plano escrito con `write_png`, con la imagen soltada antes
//! de medir) lo sube **48 MiB o más**, para demostrar que el instrumento mide.
//!
//! Si `/proc/self/clear_refs` no se puede escribir (algunos contenedores), la prueba se omite con
//! un aviso en stderr, salvo que la variable de entorno `CI` esté definida: entonces falla.

#![cfg(target_os = "linux")]

use std::fs;

use px_core::{Image, Pixel};
use px_formats::{PngError, PngRead, read_png, write_png};

const MIB: u64 = 1 << 20;
const MESSAGE_BOUND: &str = "tamaño declarado inconsistente con el fichero";

/// IDAT mínimo de 9 bytes: cabecera zlib, un bloque fijo con un byte y Adler-32 de un byte cero.
const IDAT_MINIMUM: [u8; 9] = [0x78, 0x9c, 0x63, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01];

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

fn piece(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + data.len());
    v.extend((data.len() as u32).to_be_bytes());
    v.extend(kind);
    v.extend(data);
    let mut body = kind.to_vec();
    body.extend(data);
    v.extend(crc32(&body).to_be_bytes());
    v
}

/// Fichero falso: cabecera válida (con PLTE de 2 entradas si es indexado), IDAT mínimo e IEND.
fn false_value(width: u32, height: u32, color: u8, interlaced_2: bool) -> Vec<u8> {
    let mut ihdr = Vec::new();
    ihdr.extend(width.to_be_bytes());
    ihdr.extend(height.to_be_bytes());
    ihdr.extend([8, color, 0, 0, u8::from(interlaced_2)]);
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend(piece(b"IHDR", &ihdr));
    if color == 3 {
        v.extend(piece(b"PLTE", &[0, 0, 0, 255, 255, 255]));
    }
    v.extend(piece(b"IDAT", &IDAT_MINIMUM));
    v.extend(piece(b"IEND", &[]));
    v
}

/// Valor en bytes de un campo de `/proc/self/status` (`VmHWM`, `VmRSS`; vienen en kB).
fn field(name_2: &str) -> u64 {
    let status =
        fs::read_to_string("/proc/self/status").expect("no se pudo leer /proc/self/status");
    let line = status
        .lines()
        .find_map(|l| l.strip_prefix(name_2)?.strip_prefix(':'))
        .unwrap_or_else(|| panic!("falta {name_2} en /proc/self/status"));
    let kb: u64 = line
        .split_whitespace()
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("{name_2} ilegible: {line:?}"));
    kb * 1024
}

/// Reinicia el máximo de RSS del proceso. Algunos contenedores no dejan escribir en
/// `/proc/self/clear_refs`.
fn resets_peak() -> std::io::Result<()> {
    fs::write("/proc/self/clear_refs", "5")
}

/// Ejecuta `f` y devuelve su resultado y cuánto subió el pico de RSS del proceso (en bytes) por
/// encima del RSS de antes de empezar.
fn measures<T>(f: impl FnOnce() -> T) -> (T, u64) {
    resets_peak().expect("no se pudo reiniciar el máximo de RSS (/proc/self/clear_refs)");
    let antes = field("VmRSS");
    let result = f();
    let peak = field("VmHWM");
    (result, peak.saturating_sub(antes))
}

fn is_bound(r: &Result<PngRead, PngError>) -> bool {
    matches!(r, Err(PngError::Decode(m)) if m.starts_with(MESSAGE_BOUND))
}

#[test]
fn row4_rejection_before_reserving_and_control() {
    // Sin poder reiniciar el pico no hay instrumento: se salta, salvo en el CI (variable `CI`
    // definida), donde un fallo aquí es una regresión del entorno y debe verse.
    if let Err(e) = resets_peak() {
        assert!(
            std::env::var_os("CI").is_none(),
            "no se pudo escribir /proc/self/clear_refs en el CI: {e}"
        );
        eprintln!(
            "OMITIDA fila4_rechazo_antes_de_reservar_y_control: no se puede escribir \
             /proc/self/clear_refs ({e}); sin él no se mide el pico de RSS"
        );
        return;
    }
    // (nombre, fichero falso): gris 8 de 32768², RGBA 8 de 16384², RGBA 8 de 8192², indexado de
    // 6000² y gris 32768² entrelazado, todos con IDAT mínimo de 9 bytes.
    let cases: [(&str, Vec<u8>); 5] = [
        ("gris 8 de 32768x32768", false_value(32768, 32768, 0, false)),
        ("RGBA 8 de 16384x16384", false_value(16384, 16384, 6, false)),
        ("RGBA 8 de 8192x8192", false_value(8192, 8192, 6, false)),
        ("indexado de 6000x6000", false_value(6000, 6000, 3, false)),
        (
            "gris 8 de 32768x32768 Adam7",
            false_value(32768, 32768, 0, true),
        ),
    ];
    let mut failures = Vec::new();
    for (name_2, bytes) in &cases {
        let (r, upload) = measures(|| read_png(bytes));
        if !is_bound(&r) {
            failures.push(format!(
                "{name_2}: se esperaba Decode(\"{MESSAGE_BOUND}...\") y salió {:?}",
                r.as_ref().err()
            ));
        }
        drop(r);
        if upload >= 32 * MIB {
            failures.push(format!(
                "{name_2}: el pico de RSS subió {} MiB (límite: menos de 32)",
                upload / MIB
            ));
        }
    }

    // Control: gris 8192² plano. La imagen se suelta antes de medir; solo queda el fichero.
    let file = {
        let img = Image::filled(8192, 8192, Pixel::Gray(77)).unwrap();
        write_png(&img).unwrap()
    };
    let (r, upload) = measures(|| read_png(&file));
    assert!(r.is_ok(), "el control debe ser Ok: {:?}", r.as_ref().err());
    assert!(
        upload >= 48 * MIB,
        "el instrumento no mide: el control subió solo {} MiB (se esperaban 48 o más)",
        upload / MIB
    );
    drop(r);

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
