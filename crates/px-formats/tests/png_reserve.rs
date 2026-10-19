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
const MENSAJE_COTA: &str = "tamaño declarado inconsistente con el fichero";

/// IDAT mínimo de 9 bytes: cabecera zlib, un bloque fijo con un byte y Adler-32 de un byte cero.
const IDAT_MINIMO: [u8; 9] = [0x78, 0x9c, 0x63, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01];

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

fn trozo(tipo: &[u8; 4], datos: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + datos.len());
    v.extend((datos.len() as u32).to_be_bytes());
    v.extend(tipo);
    v.extend(datos);
    let mut cuerpo = tipo.to_vec();
    cuerpo.extend(datos);
    v.extend(crc32(&cuerpo).to_be_bytes());
    v
}

/// Fichero falso: cabecera válida (con PLTE de 2 entradas si es indexado), IDAT mínimo e IEND.
fn falso(ancho: u32, alto: u32, color: u8, entrelazado: bool) -> Vec<u8> {
    let mut ihdr = Vec::new();
    ihdr.extend(ancho.to_be_bytes());
    ihdr.extend(alto.to_be_bytes());
    ihdr.extend([8, color, 0, 0, u8::from(entrelazado)]);
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend(trozo(b"IHDR", &ihdr));
    if color == 3 {
        v.extend(trozo(b"PLTE", &[0, 0, 0, 255, 255, 255]));
    }
    v.extend(trozo(b"IDAT", &IDAT_MINIMO));
    v.extend(trozo(b"IEND", &[]));
    v
}

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

fn es_cota(r: &Result<PngRead, PngError>) -> bool {
    matches!(r, Err(PngError::Decode(m)) if m.starts_with(MENSAJE_COTA))
}

#[test]
fn fila4_rechazo_antes_de_reservar_y_control() {
    // Sin poder reiniciar el pico no hay instrumento: se salta, salvo en el CI (variable `CI`
    // definida), donde un fallo aquí es una regresión del entorno y debe verse.
    if let Err(e) = reinicia_pico() {
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
    let casos: [(&str, Vec<u8>); 5] = [
        ("gris 8 de 32768x32768", falso(32768, 32768, 0, false)),
        ("RGBA 8 de 16384x16384", falso(16384, 16384, 6, false)),
        ("RGBA 8 de 8192x8192", falso(8192, 8192, 6, false)),
        ("indexado de 6000x6000", falso(6000, 6000, 3, false)),
        ("gris 8 de 32768x32768 Adam7", falso(32768, 32768, 0, true)),
    ];
    let mut fallos = Vec::new();
    for (nombre, bytes) in &casos {
        let (r, subida) = mide(|| read_png(bytes));
        if !es_cota(&r) {
            fallos.push(format!(
                "{nombre}: se esperaba Decode(\"{MENSAJE_COTA}...\") y salió {:?}",
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

    // Control: gris 8192² plano. La imagen se suelta antes de medir; solo queda el fichero.
    let fichero = {
        let img = Image::filled(8192, 8192, Pixel::Gray(77)).unwrap();
        write_png(&img).unwrap()
    };
    let (r, subida) = mide(|| read_png(&fichero));
    assert!(r.is_ok(), "el control debe ser Ok: {:?}", r.as_ref().err());
    assert!(
        subida >= 48 * MIB,
        "el instrumento no mide: el control subió solo {} MiB (se esperaban 48 o más)",
        subida / MIB
    );
    drop(r);

    assert!(fallos.is_empty(), "{}", fallos.join("\n"));
}
