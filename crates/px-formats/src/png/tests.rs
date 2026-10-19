//! Unidad 13 (specs/hito-2-png-flujo.md): filas 1 a 3 y propiedad 1. Prueban las dos funciones
//! privadas de la cota de lectura de `read_png`, que son aritmética pura:
//!
//! ```text
//! fn inflated_len(width: u32, height: u32, bits_per_pixel: u8, interlaced: bool) -> Option<u64>;
//! fn stream_is_plausible(needed: u64, idat_len: u64) -> bool;
//! ```
//!
//! En ROJO hasta la lógica: hoy ambas son `todo!()`.
//!
//! Los valores esperados de las filas 1 y 2 están verificados a mano contra las fórmulas de la
//! spec (`alto × (1 + ceil(ancho × bpp / 8))`, por pasada con Adam7), no contra el código.

use proptest::prelude::*;
use proptest::sample::select;

use super::{inflated_len, stream_is_plausible};

// ---------------------------------------------------------------------------------------
// Fila 1: sin entrelazar.
// ---------------------------------------------------------------------------------------

#[test]
fn fila1_inflated_len_sin_entrelazar() {
    // (ancho, alto, bits por píxel, esperado)
    let casos: [(u32, u32, u8, Option<u64>); 7] = [
        (1, 1, 8, Some(2)),                  // gris 8: 1 × (1 + 1)
        (3, 2, 8, Some(8)),                  // gris 8: 2 × (1 + 3)
        (1, 1, 32, Some(5)),                 // RGBA 8: 1 × (1 + 4)
        (9, 2, 1, Some(6)),                  // gris 1 bit: 2 × (1 + ceil(9 / 8) = 2)
        (2, 2, 48, Some(26)),                // RGB 16: 2 × (1 + 12)
        (8192, 8192, 32, Some(268_443_648)), // RGBA 8: 8192 × (1 + 32768)
        (u32::MAX, u32::MAX, 64, None),      // RGBA 16: desborda u64
    ];
    for (ancho, alto, bpp, esperado) in casos {
        assert_eq!(
            inflated_len(ancho, alto, bpp, false),
            esperado,
            "{ancho}x{alto} a {bpp} bpp"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 2: Adam7.
// ---------------------------------------------------------------------------------------

#[test]
fn fila2_inflated_len_adam7() {
    // (ancho, alto, bits por píxel, esperado); las pasadas vacías no cuentan.
    let casos: [(u32, u32, u8, u64); 8] = [
        (1, 1, 8, 2),
        (5, 1, 8, 9),
        (1, 5, 8, 10),
        (8, 8, 8, 79),
        (3, 3, 32, 42),
        (9, 9, 1, 42),
        (8192, 8192, 8, 67_124_224),
        (8192, 8192, 32, 268_450_816),
    ];
    for (ancho, alto, bpp, esperado) in casos {
        assert_eq!(
            inflated_len(ancho, alto, bpp, true),
            Some(esperado),
            "Adam7 {ancho}x{alto} a {bpp} bpp"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 3: stream_is_plausible.
// ---------------------------------------------------------------------------------------

#[test]
fn fila3_stream_is_plausible() {
    // (needed, idat_len, esperado)
    let casos: [(u64, u64, bool); 6] = [
        (1032, 1, true),  // justo en el límite
        (1033, 1, false), // la comparación es estricta
        (0, 0, true),
        (1, 0, false),
        (u64::MAX, u64::MAX, true),  // 1032 × idat desbordaría u64
        (1032 << 32, 1 << 32, true), // sin desbordar
    ];
    for (needed, idat, esperado) in casos {
        assert_eq!(
            stream_is_plausible(needed, idat),
            esperado,
            "needed {needed}, idat {idat}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 1: inflated_len contra un modelo que cuenta píxeles.
// ---------------------------------------------------------------------------------------

/// Matriz 8×8 de Adam7 de la especificación PNG: pasada (1 a 7) de cada píxel `[y % 8][x % 8]`.
const MATRIZ_ADAM7: [[u8; 8]; 8] = [
    [1, 6, 4, 6, 2, 6, 4, 6],
    [7, 7, 7, 7, 7, 7, 7, 7],
    [5, 6, 5, 6, 5, 6, 5, 6],
    [7, 7, 7, 7, 7, 7, 7, 7],
    [3, 6, 4, 6, 3, 6, 4, 6],
    [7, 7, 7, 7, 7, 7, 7, 7],
    [5, 6, 5, 6, 5, 6, 5, 6],
    [7, 7, 7, 7, 7, 7, 7, 7],
];

/// Bytes de una pasada de `filas` filas de `columnas` píxeles: `filas × (1 + ceil(columnas × bpp / 8))`.
fn bytes_de_pasada(filas: u64, columnas: u64, bpp: u64) -> u64 {
    if filas == 0 || columnas == 0 {
        return 0;
    }
    let bits = columnas * bpp;
    filas * (1 + bits.div_ceil(8))
}

/// Modelo: clasifica cada píxel en su pasada con la matriz y cuenta, por pasada, los píxeles y
/// las filas distintas con píxeles; las columnas son píxeles / filas.
fn modelo(ancho: u32, alto: u32, bpp: u8, entrelazado: bool) -> u64 {
    let bpp = u64::from(bpp);
    if !entrelazado {
        return bytes_de_pasada(u64::from(alto), u64::from(ancho), bpp);
    }
    let mut pixeles = [0u64; 8];
    let mut filas = [0u64; 8];
    for y in 0..alto as usize {
        let mut pasadas_de_la_fila = [false; 8];
        for x in 0..ancho as usize {
            let p = usize::from(MATRIZ_ADAM7[y % 8][x % 8]);
            pixeles[p] += 1;
            pasadas_de_la_fila[p] = true;
        }
        for (p, hay) in pasadas_de_la_fila.iter().enumerate() {
            if *hay {
                filas[p] += 1;
            }
        }
    }
    (1..=7)
        .map(|p| {
            if pixeles[p] == 0 {
                0
            } else {
                assert_eq!(
                    pixeles[p] % filas[p],
                    0,
                    "pasada {p}: píxeles no rectangulares"
                );
                bytes_de_pasada(filas[p], pixeles[p] / filas[p], bpp)
            }
        })
        .sum()
}

/// (canales, profundidad) válidos de PNG, como bits por píxel.
fn bits_por_pixel_validos() -> impl Strategy<Value = u8> {
    let mut v = Vec::new();
    for (canales, profundidades) in [
        (1u8, &[1u8, 2, 4, 8, 16][..]), // gris
        (3, &[8, 16][..]),              // RGB
        (1, &[1, 2, 4, 8][..]),         // indexado
        (2, &[8, 16][..]),              // gris con alfa
        (4, &[8, 16][..]),              // RGBA
    ] {
        for &p in profundidades {
            v.push(canales * p);
        }
    }
    select(v)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // Propiedad 1a: igual que el modelo para 1 a 40 de lado, con y sin Adam7.
    #[test]
    fn p1_inflated_len_coincide_con_el_modelo_que_cuenta_pixeles(
        ancho in 1u32..=40,
        alto in 1u32..=40,
        bpp in bits_por_pixel_validos(),
        entrelazado in any::<bool>(),
    ) {
        prop_assert_eq!(
            inflated_len(ancho, alto, bpp, entrelazado),
            Some(modelo(ancho, alto, bpp, entrelazado)),
            "{}x{} a {} bpp, entrelazado {}", ancho, alto, bpp, entrelazado
        );
    }

    // Propiedad 1b: con u32 arbitrarios nunca hay pánico (`Some` o `None`), y sin entrelazar el
    // resultado es el exacto en u128, con `None` solo si no cabe en u64.
    #[test]
    fn p1_inflated_len_con_u32_arbitrarios_no_hace_panico(
        ancho in 1u32..=u32::MAX,
        alto in 1u32..=u32::MAX,
        bpp in bits_por_pixel_validos(),
        entrelazado in any::<bool>(),
    ) {
        let r = inflated_len(ancho, alto, bpp, entrelazado);
        if !entrelazado {
            let bits = u128::from(ancho) * u128::from(bpp);
            let exacto = u128::from(alto) * (1 + bits.div_ceil(8));
            prop_assert_eq!(r, u64::try_from(exacto).ok());
        }
    }
}
