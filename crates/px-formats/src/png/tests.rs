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
//! spec (`height × (1 + ceil(width × bpp / 8))`, por pasada con Adam7), no contra el código.

use proptest::prelude::*;
use proptest::sample::select;

use super::{inflated_len, stream_is_plausible};

// ---------------------------------------------------------------------------------------
// Fila 1: sin entrelazar.
// ---------------------------------------------------------------------------------------

#[test]
fn row1_inflated_len_without_interlacing() {
    // (ancho, alto, bits por píxel, esperado)
    let cases: [(u32, u32, u8, Option<u64>); 7] = [
        (1, 1, 8, Some(2)),                  // gris 8: 1 × (1 + 1)
        (3, 2, 8, Some(8)),                  // gris 8: 2 × (1 + 3)
        (1, 1, 32, Some(5)),                 // RGBA 8: 1 × (1 + 4)
        (9, 2, 1, Some(6)),                  // gris 1 bit: 2 × (1 + ceil(9 / 8) = 2)
        (2, 2, 48, Some(26)),                // RGB 16: 2 × (1 + 12)
        (8192, 8192, 32, Some(268_443_648)), // RGBA 8: 8192 × (1 + 32768)
        (u32::MAX, u32::MAX, 64, None),      // RGBA 16: desborda u64
    ];
    for (width, height, bpp, expected) in cases {
        assert_eq!(
            inflated_len(width, height, bpp, false),
            expected,
            "{width}x{height} a {bpp} bpp"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 2: Adam7.
// ---------------------------------------------------------------------------------------

#[test]
fn row2_inflated_len_adam7() {
    // (ancho, alto, bits por píxel, esperado); las pasadas vacías no cuentan.
    let cases: [(u32, u32, u8, u64); 8] = [
        (1, 1, 8, 2),
        (5, 1, 8, 9),
        (1, 5, 8, 10),
        (8, 8, 8, 79),
        (3, 3, 32, 42),
        (9, 9, 1, 42),
        (8192, 8192, 8, 67_124_224),
        (8192, 8192, 32, 268_450_816),
    ];
    for (width, height, bpp, expected) in cases {
        assert_eq!(
            inflated_len(width, height, bpp, true),
            Some(expected),
            "Adam7 {width}x{height} a {bpp} bpp"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 3: stream_is_plausible.
// ---------------------------------------------------------------------------------------

#[test]
fn row3_stream_is_plausible() {
    // (needed, idat_len, esperado)
    let cases: [(u64, u64, bool); 6] = [
        (1032, 1, true),  // justo en el límite
        (1033, 1, false), // la comparación es estricta
        (0, 0, true),
        (1, 0, false),
        (u64::MAX, u64::MAX, true),  // 1032 × idat desbordaría u64
        (1032 << 32, 1 << 32, true), // sin desbordar
    ];
    for (needed, idat, expected) in cases {
        assert_eq!(
            stream_is_plausible(needed, idat),
            expected,
            "needed {needed}, idat {idat}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 1: inflated_len contra un modelo que cuenta píxeles.
// ---------------------------------------------------------------------------------------

/// Matriz 8×8 de Adam7 de la especificación PNG: pasada (1 a 7) de cada píxel `[y % 8][x % 8]`.
const MATRIX_ADAM_7: [[u8; 8]; 8] = [
    [1, 6, 4, 6, 2, 6, 4, 6],
    [7, 7, 7, 7, 7, 7, 7, 7],
    [5, 6, 5, 6, 5, 6, 5, 6],
    [7, 7, 7, 7, 7, 7, 7, 7],
    [3, 6, 4, 6, 3, 6, 4, 6],
    [7, 7, 7, 7, 7, 7, 7, 7],
    [5, 6, 5, 6, 5, 6, 5, 6],
    [7, 7, 7, 7, 7, 7, 7, 7],
];

/// Bytes de una pasada de `rows` filas de `columns` píxeles: `rows × (1 + ceil(columns × bpp / 8))`.
fn bytes_of_pass(rows: u64, columns: u64, bpp: u64) -> u64 {
    if rows == 0 || columns == 0 {
        return 0;
    }
    let bits = columns * bpp;
    rows * (1 + bits.div_ceil(8))
}

/// Modelo: clasifica cada píxel en su pasada con la matriz y cuenta, por pasada, los píxeles y
/// las filas distintas con píxeles; las columnas son píxeles / filas.
fn model(width: u32, height: u32, bpp: u8, interlaced_2: bool) -> u64 {
    let bpp = u64::from(bpp);
    if !interlaced_2 {
        return bytes_of_pass(u64::from(height), u64::from(width), bpp);
    }
    let mut pixels = [0u64; 8];
    let mut rows = [0u64; 8];
    for y in 0..height as usize {
        let mut passes_of_the_row = [false; 8];
        for x in 0..width as usize {
            let p = usize::from(MATRIX_ADAM_7[y % 8][x % 8]);
            pixels[p] += 1;
            passes_of_the_row[p] = true;
        }
        for (p, there_is) in passes_of_the_row.iter().enumerate() {
            if *there_is {
                rows[p] += 1;
            }
        }
    }
    (1..=7)
        .map(|p| {
            if pixels[p] == 0 {
                0
            } else {
                assert_eq!(
                    pixels[p] % rows[p],
                    0,
                    "pasada {p}: píxeles no rectangulares"
                );
                bytes_of_pass(rows[p], pixels[p] / rows[p], bpp)
            }
        })
        .sum()
}

/// (canales, profundidad) válidos de PNG, como bits por píxel.
fn valid_bits_per_pixel() -> impl Strategy<Value = u8> {
    let mut v = Vec::new();
    for (channels, depths) in [
        (1u8, &[1u8, 2, 4, 8, 16][..]), // gris
        (3, &[8, 16][..]),              // RGB
        (1, &[1, 2, 4, 8][..]),         // indexado
        (2, &[8, 16][..]),              // gris con alfa
        (4, &[8, 16][..]),              // RGBA
    ] {
        for &p in depths {
            v.push(channels * p);
        }
    }
    select(v)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // Propiedad 1a: igual que el modelo para 1 a 40 de lado, con y sin Adam7.
    #[test]
    fn p1_inflated_len_matches_the_model_that_counts_pixels(
        width in 1u32..=40,
        height in 1u32..=40,
        bpp in valid_bits_per_pixel(),
        interlaced_2 in any::<bool>(),
    ) {
        prop_assert_eq!(
            inflated_len(width, height, bpp, interlaced_2),
            Some(model(width, height, bpp, interlaced_2)),
            "{}x{} a {} bpp, entrelazado {}", width, height, bpp, interlaced_2
        );
    }

    // Propiedad 1b: con u32 arbitrarios nunca hay pánico (`Some` o `None`), y sin entrelazar el
    // resultado es el exacto en u128, con `None` solo si no cabe en u64.
    #[test]
    fn p1_inflated_len_with_arbitrary_u32_does_not_panic(
        width in 1u32..=u32::MAX,
        height in 1u32..=u32::MAX,
        bpp in valid_bits_per_pixel(),
        interlaced_2 in any::<bool>(),
    ) {
        let r = inflated_len(width, height, bpp, interlaced_2);
        if !interlaced_2 {
            let bits = u128::from(width) * u128::from(bpp);
            let exact = u128::from(height) * (1 + bits.div_ceil(8));
            prop_assert_eq!(r, u64::try_from(exact).ok());
        }
    }
}
