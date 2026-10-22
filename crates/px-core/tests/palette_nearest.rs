//! Tests de `Palette::nearest`, color más cercano en una paleta
//! (specs/hito-2-color-cercano.md).
//!
//! Corren en el perfil de test (debug, con `overflow-checks`): un desbordamiento
//! de la implementación aparecería como pánico.

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Palette, Rgba};

// Utilidades de apoyo.

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

fn opaque(r: u8, g: u8, b: u8) -> Rgba {
    Rgba::opaque(r, g, b)
}

fn palette_2(colors_2: &[Rgba]) -> Palette {
    Palette::from_colors(colors_2.iter().copied()).unwrap()
}

fn palette_with_transparent(colors_2: &[Rgba], index: u8) -> Palette {
    let mut p = palette_2(colors_2);
    p.set_transparent_index(Some(index)).unwrap();
    p
}

/// Distancia del oráculo entre dos colores ya resueltos: forma EXPANDIDA de la
/// fórmula, `Σ_c (c_p·a_p − c_q·a_q)² + 255²·(a_p − a_q)²`, en `i128`. No usa el
/// vector premultiplicado ni comparte nada con la implementación.
fn distance(p: Rgba, q: Rgba) -> i128 {
    let canal = |cp: u8, cq: u8| {
        let d = i128::from(cp) * i128::from(p.a) - i128::from(cq) * i128::from(q.a);
        d * d
    };
    let gives = i128::from(p.a) - i128::from(q.a);
    canal(p.r, q.r) + canal(p.g, q.g) + canal(p.b, q.b) + 255 * 255 * gives * gives
}

/// Distancia de `query` a cada entrada de la paleta, por su valor resuelto.
fn distances(p: &Palette, query: Rgba) -> Vec<i128> {
    (0..p.len())
        .map(|i| distance(query, p.resolved(i as u8).unwrap()))
        .collect()
}

/// Oráculo ingenuo: mínimo por (distancia, índice) sobre `resolved(i)`.
fn oracle(p: &Palette, query: Rgba) -> Option<u8> {
    distances(p, query)
        .into_iter()
        .enumerate()
        .min_by_key(|&(i, d)| (d, i))
        .map(|(i, _)| i as u8)
}

/// Comprueba `nearest` contra el valor de la tabla y, de paso, que el oráculo
/// del test también da ese valor (así el oráculo queda anclado a la tabla).
fn waits(p: &Palette, query: Rgba, expected: Option<u8>) {
    assert_eq!(p.nearest(query), expected, "nearest de {query:?}");
    assert_eq!(
        oracle(p, query),
        expected,
        "el oráculo del test no coincide con la tabla para {query:?}"
    );
}

/// Tres primarios opacos: la paleta `R` de la tabla.
fn primary() -> Palette {
    palette_2(&[opaque(255, 0, 0), opaque(0, 255, 0), opaque(0, 0, 255)])
}

/// La paleta `Q` de la tabla.
fn q_red_and_gray() -> [Rgba; 2] {
    [opaque(200, 0, 0), opaque(9, 8, 7)]
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn palette_empty_returns_none() {
    let p = Palette::new();
    for c in [
        rgba(0, 0, 0, 0),
        rgba(0, 0, 0, 255),
        rgba(255, 255, 255, 255),
        rgba(10, 20, 30, 128),
    ] {
        assert_eq!(p.nearest(c), None);
        assert_eq!(oracle(&p, c), None);
    }
    assert_eq!(Palette::default().nearest(rgba(0, 0, 0, 0)), None);
}

#[test]
fn an_entry_with_maximum_distance() {
    let p = palette_2(&[rgba(0, 0, 0, 0)]);
    let query = rgba(255, 255, 255, 255);
    assert_eq!(distances(&p, query), [16_913_002_500]);
    waits(&p, query, Some(0));
}

#[test]
fn primary_exact() {
    let p = primary();
    waits(&p, opaque(255, 0, 0), Some(0));
    waits(&p, opaque(0, 255, 0), Some(1));
    waits(&p, opaque(0, 0, 255), Some(2));
    // A mano: la de en medio.
    assert_eq!(
        distances(&p, opaque(0, 255, 0)),
        [8_456_501_250, 0, 8_456_501_250]
    );
}

#[test]
fn primary_with_opaque_intermediate() {
    let p = primary();
    // A mano.
    let query = opaque(200, 30, 30);
    assert_eq!(
        distances(&p, query),
        [313_745_625, 5_951_413_125, 5_951_413_125]
    );
    waits(&p, query, Some(0));
    waits(&p, opaque(10, 10, 240), Some(2));
}

#[test]
fn two_hundred_fifty_six_grays() {
    let p = Palette::from_colors((0..=255u8).map(|i| opaque(i, i, i))).unwrap();
    assert_eq!(p.len(), 256);
    for v in 0..=255u8 {
        waits(&p, opaque(v, v, v), Some(v));
    }
    waits(&p, opaque(254, 255, 255), Some(255));
}

#[test]
fn exact_tie_wins_the_low_index() {
    let a = palette_2(&[opaque(0, 0, 0), opaque(2, 0, 0)]);
    let b = palette_2(&[opaque(2, 0, 0), opaque(0, 0, 0)]);
    let query = opaque(1, 0, 0);
    assert_eq!(distances(&a, query)[0], distances(&a, query)[1]);
    assert_eq!(distances(&b, query)[0], distances(&b, query)[1]);
    waits(&a, query, Some(0));
    waits(&b, query, Some(0));
}

#[test]
fn duplicates_return_the_first() {
    let p = palette_2(&[opaque(9, 9, 9), opaque(5, 5, 5), opaque(5, 5, 5)]);
    waits(&p, opaque(5, 5, 5), Some(1));
}

#[test]
fn transparent_beats_opaque_black_with_a_transparent_query() {
    // A mano.
    let p = palette_2(&[opaque(0, 0, 0), rgba(255, 255, 255, 0)]);
    let query = rgba(0, 0, 0, 0);
    assert_eq!(distances(&p, query), [4_228_250_625, 0]);
    waits(&p, query, Some(1));
}

#[test]
fn alpha_zero_ignores_rgb_and_exact_does_not_return_its_index() {
    let p = palette_2(&[rgba(10, 20, 30, 0), rgba(10, 20, 30, 255)]);
    waits(&p, rgba(200, 100, 50, 0), Some(0));

    let q = palette_2(&[rgba(1, 2, 3, 0), rgba(9, 9, 9, 0)]);
    waits(&q, rgba(9, 9, 9, 0), Some(0));
}

#[test]
fn transparent_index_counts_as_a_transparent_entry() {
    let colors_2 = [opaque(1, 1, 1), rgba(5, 5, 5, 0), rgba(7, 7, 7, 0)];
    let query = rgba(0, 0, 0, 0);

    waits(&palette_2(&colors_2), query, Some(1));
    waits(&palette_with_transparent(&colors_2, 0), query, Some(0));
}

#[test]
fn alpha_zero_without_transparent_goes_to_the_darkest() {
    let p = palette_2(&[opaque(0, 0, 0), opaque(255, 255, 255)]);
    waits(&p, rgba(255, 255, 255, 0), Some(0));

    let q = palette_2(&q_red_and_gray());
    waits(&q, rgba(0, 0, 0, 0), Some(1));
}

#[test]
fn q_without_index_transparent() {
    let p = palette_2(&q_red_and_gray());
    waits(&p, opaque(200, 0, 0), Some(0));
    waits(&p, rgba(0, 0, 0, 0), Some(1));
}

#[test]
fn q_with_transparent_index_0_does_not_find_its_stored_value() {
    let p = palette_with_transparent(&q_red_and_gray(), 0);
    waits(&p, rgba(0, 0, 0, 0), Some(0));
    waits(&p, opaque(200, 0, 0), Some(1));
}

#[test]
fn q_with_transparent_index_1() {
    let p = palette_with_transparent(&q_red_and_gray(), 1);
    waits(&p, opaque(9, 8, 7), Some(0));
    waits(&p, rgba(0, 0, 0, 0), Some(1));
}

#[test]
fn alpha_threshold_between_127_and_128() {
    // A mano.
    let p = palette_2(&[opaque(255, 255, 255), rgba(255, 255, 255, 0)]);

    let c127 = rgba(255, 255, 255, 127);
    assert_eq!(distances(&p, c127), [4_261_478_400, 4_195_152_900]);
    waits(&p, c127, Some(1));

    let c128 = rgba(255, 255, 255, 128);
    assert_eq!(distances(&p, c128), [4_195_152_900, 4_261_478_400]);
    waits(&p, c128, Some(0));
}

#[test]
fn semitransparents_go_to_the_entry_with_similar_alpha() {
    let p = palette_2(&[rgba(255, 0, 0, 128), opaque(255, 0, 0), rgba(0, 0, 0, 0)]);
    waits(&p, rgba(255, 0, 0, 130), Some(0));
    waits(&p, rgba(255, 0, 0, 250), Some(1));
    waits(&p, rgba(0, 0, 255, 10), Some(2));
}

// Estrategias.

/// Valores por canal que fuerzan empates, duplicados y alfa 0.
const CHANNELS: [u8; 5] = [0, 1, 127, 128, 255];

fn canal() -> impl Strategy<Value = u8> {
    prop_oneof![any::<u8>(), prop::sample::select(CHANNELS.to_vec())]
}

fn color_uniform() -> impl Strategy<Value = Rgba> {
    (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()).prop_map(|(r, g, b, a)| rgba(r, g, b, a))
}

fn color_small() -> impl Strategy<Value = Rgba> {
    let c = || prop::sample::select(CHANNELS.to_vec());
    (c(), c(), c(), c()).prop_map(|(r, g, b, a)| rgba(r, g, b, a))
}

// Mezcla colores uniformes con colores del conjunto pequeño.
fn color() -> impl Strategy<Value = Rgba> {
    prop_oneof![color_uniform(), color_small()]
}

fn color_opaque() -> impl Strategy<Value = Rgba> {
    color().prop_map(|c| opaque(c.r, c.g, c.b))
}

/// Paleta con los colores dados y, si `t` es `Some`, índice transparente
/// `t % len` (si la paleta no está vacía).
fn builds(colors_2: &[Rgba], t: Option<u8>) -> Palette {
    let mut p = palette_2(colors_2);
    if let Some(t) = t
        && !p.is_empty()
    {
        p.set_transparent_index(Some((t as usize % p.len()) as u8))
            .unwrap();
    }
    p
}

// Paletas de `tam` entradas, con índice transparente o sin él.
fn palette_arbitrary(tam: std::ops::RangeInclusive<usize>) -> impl Strategy<Value = Palette> {
    (
        prop::collection::vec(color(), tam),
        prop::option::of(any::<u8>()),
    )
        .prop_map(|(v, t)| builds(&v, t))
}

// Paletas opacas, sin índice transparente.
fn palette_opaque(tam: std::ops::RangeInclusive<usize>) -> impl Strategy<Value = Palette> {
    prop::collection::vec(color_opaque(), tam).prop_map(|v| palette_2(&v))
}

// Propiedades, como funciones para compartirlas entre paletas pequeñas y grandes.

fn prop_oracle(p: &Palette, query: Rgba) -> Result<(), TestCaseError> {
    let expected = oracle(p, query);
    let real = p.nearest(query);
    prop_assert_eq!(real, expected);
    prop_assert_eq!(real.is_none(), p.is_empty());
    if let Some(i) = real {
        prop_assert!((i as usize) < p.len());
    }
    Ok(())
}

/// Dos entradas resueltas son equivalentes si son iguales o ambas tienen `a = 0`.
fn equivalent(a: Rgba, b: Rgba) -> bool {
    a == b || (a.a == 0 && b.a == 0)
}

fn prop_exact_returns_the_first_equivalent(p: &Palette) -> Result<(), TestCaseError> {
    let resolved: Vec<Rgba> = (0..p.len()).map(|i| p.resolved(i as u8).unwrap()).collect();
    let without_duplicates_nor_transparent = resolved
        .iter()
        .enumerate()
        .all(|(k, &c)| resolved[..k].iter().all(|&other_2| !equivalent(other_2, c)));
    for (i, &c) in resolved.iter().enumerate() {
        let j = resolved.iter().position(|&other_2| equivalent(other_2, c));
        prop_assert_eq!(p.nearest(c), j.map(|j| j as u8));
        prop_assert!(j.unwrap() <= i);
        if without_duplicates_nor_transparent {
            prop_assert_eq!(j, Some(i));
        }
    }
    Ok(())
}

fn prop_opaque_as_rgb(p: &Palette, query: Rgba) -> Result<(), TestCaseError> {
    let rgb = |c: Rgba, d: Rgba| {
        let delta = |x: u8, y: u8| (i64::from(x) - i64::from(y)).pow(2);
        delta(c.r, d.r) + delta(c.g, d.g) + delta(c.b, d.b)
    };
    let expected = p
        .colors()
        .iter()
        .enumerate()
        .min_by_key(|&(i, &c)| (rgb(query, c), i))
        .map(|(i, _)| i as u8);
    prop_assert_eq!(p.nearest(query), expected);
    Ok(())
}

fn prop_alpha_zero(p: &Palette, r: u8, g: u8, b: u8) -> Result<(), TestCaseError> {
    let first = (0..p.len()).find(|&i| p.resolved(i as u8).unwrap().a == 0);
    if let Some(i) = first {
        prop_assert_eq!(p.nearest(rgba(r, g, b, 0)), Some(i as u8));
    }
    Ok(())
}

fn prop_resolved_is_equivalent_to_substituting(
    p: &Palette,
    t: u8,
    query: Rgba,
) -> Result<(), TestCaseError> {
    if p.is_empty() {
        return Ok(());
    }
    let t = (t as usize % p.len()) as u8;

    let mut with_index = p.clone();
    with_index.set_transparent_index(Some(t)).unwrap();

    let mut replaced = p.clone();
    replaced.set_transparent_index(None).unwrap();
    replaced.set(t, with_index.resolved(t).unwrap()).unwrap();

    prop_assert_eq!(replaced.transparent_index(), None);
    prop_assert_eq!(with_index.nearest(query), replaced.nearest(query));
    Ok(())
}

fn prop_transparent_rgb_does_not_count(
    p: &Palette,
    queries: &[Rgba],
    (r, g, b): (u8, u8, u8),
    (r2, g2, b2): (u8, u8, u8),
) -> Result<(), TestCaseError> {
    // Consulta con a = 0: el RGB no influye.
    prop_assert_eq!(p.nearest(rgba(r, g, b, 0)), p.nearest(rgba(r2, g2, b2, 0)));

    // Entradas con a = 0 (guardada o por el índice transparente): su RGB no influye.
    let replaced_2: Vec<Rgba> = (0..p.len())
        .map(|i| {
            let saved = p.get(i as u8).unwrap();
            if p.resolved(i as u8).unwrap().a == 0 {
                rgba(r2, g2, b2, saved.a)
            } else {
                saved
            }
        })
        .collect();
    let mut other = palette_2(&replaced_2);
    other.set_transparent_index(p.transparent_index()).unwrap();

    let entries = (0..p.len()).map(|i| p.resolved(i as u8).unwrap());
    for c in queries.iter().copied().chain(entries) {
        prop_assert_eq!(p.nearest(c), other.nearest(c));
    }
    Ok(())
}

// Mismas propiedades para paletas pequeñas (0..=8) y grandes (0..=256, menos casos).
macro_rules! properties {
    ($modulo:ident, $tam:expr, $cases:expr) => {
        mod $modulo {
            use super::*;

            proptest! {
                #![proptest_config(ProptestConfig::with_cases($cases))]

                #[test]
                fn oracle_naive(p in palette_arbitrary($tam), query in color()) {
                    prop_oracle(&p, query)?;
                    // Las propias entradas, guardadas y resueltas, como consultas.
                    for i in 0..p.len() {
                        prop_oracle(&p, p.get(i as u8).unwrap())?;
                        prop_oracle(&p, p.resolved(i as u8).unwrap())?;
                    }
                }

                #[test]
                fn exact_returns_the_first_equivalent(p in palette_arbitrary($tam)) {
                    prop_exact_returns_the_first_equivalent(&p)?;
                }

                #[test]
                fn opaque_as_rgb(p in palette_opaque($tam), query in color_opaque()) {
                    prop_opaque_as_rgb(&p, query)?;
                }

                #[test]
                fn alpha_zero_goes_to_the_first_transparent(
                    p in palette_arbitrary($tam),
                    r in canal(),
                    g in canal(),
                    b in canal(),
                ) {
                    prop_alpha_zero(&p, r, g, b)?;
                }

                #[test]
                fn resolved_equals_to_replace(
                    p in palette_arbitrary($tam),
                    t: u8,
                    query in color(),
                ) {
                    prop_resolved_is_equivalent_to_substituting(&p, t, query)?;
                }

                #[test]
                fn rgb_transparent_not_count(
                    p in palette_arbitrary($tam),
                    queries in prop::collection::vec(color(), 1..=4),
                    rgb in (canal(), canal(), canal()),
                    other_rgb in (canal(), canal(), canal()),
                ) {
                    prop_transparent_rgb_does_not_count(&p, &queries, rgb, other_rgb)?;
                }
            }
        }
    };
}

properties!(palettes_small, 0..=8, 256);
properties!(palettes_large, 0..=256, 32);
