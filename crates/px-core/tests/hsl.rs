//! Tests de `Hsl` y su conversión con `Rgba` (specs/hito-2-hsl.md).

use std::fmt::Debug;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Hsl, Rgba};

// Comprobación en tiempo de compilación de los traits exigidos.
fn assert_hsl_traits<T: Copy + Clone + Debug + PartialEq + Default>() {}

#[test]
fn traits_derived() {
    assert_hsl_traits::<Hsl>();
}

// Utilidades de apoyo.

/// Tolerancia de las comparaciones de `f32` (salvo las marcadas «exacto»).
const TOL: f32 = 1e-4;

fn near(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

fn assert_near(real: f32, expected: f32, that: &str) {
    assert!(
        near(real, expected, TOL),
        "{that}: se esperaba {expected}, salió {real}"
    );
}

/// Comprueba `h`, `s` y `l` de un `Hsl` con la tolerancia por defecto.
fn assert_hsl(hsl: Hsl, h: f32, s: f32, l: f32) {
    assert_near(hsl.h(), h, "h");
    assert_near(hsl.s(), s, "s");
    assert_near(hsl.l(), l, "l");
}

fn opaque(r: u8, g: u8, b: u8) -> Rgba {
    Rgba::new(r, g, b, 255)
}

/// Distancia circular entre dos ángulos en grados.
fn distance_circular(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % 360.0;
    d.min(360.0 - d)
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn default_es_negro() {
    assert_eq!(Hsl::default(), Hsl::new(0.0, 0.0, 0.0));
    let d = Hsl::default();
    assert_eq!((d.h(), d.s(), d.l()), (0.0, 0.0, 0.0));
}

#[test]
fn hue_normalizes() {
    // 360 equivale a 0 (exacto).
    assert_eq!(Hsl::new(360.0, 1.0, 0.5).h(), 0.0);
    assert_near(Hsl::new(-60.0, 1.0, 0.5).h(), 300.0, "h de -60");
    assert_near(Hsl::new(720.5, 1.0, 0.5).h(), 0.5, "h de 720.5");
}

#[test]
fn non_finite_hue_goes_to_zero() {
    assert_eq!(Hsl::new(f32::NAN, 1.0, 0.5).h(), 0.0);
    assert_eq!(Hsl::new(f32::INFINITY, 1.0, 0.5).h(), 0.0);
    assert_eq!(Hsl::new(f32::NEG_INFINITY, 1.0, 0.5).h(), 0.0);
}

#[test]
fn tiny_negative_hue_is_never_360() {
    assert_eq!(Hsl::new(-1e-9, 1.0, 0.5).h(), 0.0);
}

#[test]
fn saturation_and_lightness_are_clamped() {
    let a = Hsl::new(0.0, 2.0, -3.0);
    assert_near(a.s(), 1.0, "s de 2");
    assert_near(a.l(), 0.0, "l de -3");

    let b = Hsl::new(0.0, f32::NAN, f32::NAN);
    assert_near(b.s(), 0.0, "s de NaN");
    assert_near(b.l(), 0.0, "l de NaN");

    let c = Hsl::new(0.0, f32::INFINITY, f32::NEG_INFINITY);
    assert_near(c.s(), 1.0, "s de +inf");
    assert_near(c.l(), 0.0, "l de -inf");
}

#[test]
fn from_rgba_primaries_and_secondaries() {
    let cases = [
        (opaque(255, 0, 0), 0.0),
        (opaque(255, 255, 0), 60.0),
        (opaque(0, 255, 0), 120.0),
        (opaque(0, 255, 255), 180.0),
        (opaque(0, 0, 255), 240.0),
        (opaque(255, 0, 255), 300.0),
    ];
    for (color, h) in cases {
        assert_hsl(Hsl::from_rgba(color), h, 1.0, 0.5);
    }
}

#[test]
fn from_rgba_black_white_and_mid_gray() {
    assert_hsl(Hsl::from_rgba(opaque(0, 0, 0)), 0.0, 0.0, 0.0);
    assert_hsl(Hsl::from_rgba(opaque(255, 255, 255)), 0.0, 0.0, 1.0);
    assert_hsl(
        Hsl::from_rgba(opaque(128, 128, 128)),
        0.0,
        0.0,
        128.0 / 255.0,
    );
}

#[test]
fn from_rgba_almost_gray_has_maximum_saturation() {
    assert_hsl(Hsl::from_rgba(opaque(1, 0, 0)), 0.0, 1.0, 1.0 / 510.0);
    assert_hsl(
        Hsl::from_rgba(opaque(255, 254, 254)),
        0.0,
        1.0,
        509.0 / 510.0,
    );
}

#[test]
fn from_rgba_orange() {
    // h = 60 * 128/255 ≈ 30.1176.
    assert_hsl(Hsl::from_rgba(opaque(255, 128, 0)), 30.1176, 1.0, 0.5);
}

#[test]
fn from_rgba_maximum_ties() {
    // Orden de prueba r, g, b.
    assert_near(
        Hsl::from_rgba(opaque(255, 255, 0)).h(),
        60.0,
        "h de (255,255,0)",
    );
    assert_near(
        Hsl::from_rgba(opaque(0, 255, 255)).h(),
        180.0,
        "h de (0,255,255)",
    );
    assert_near(
        Hsl::from_rgba(opaque(255, 0, 255)).h(),
        300.0,
        "h de (255,0,255)",
    );
}

#[test]
fn to_rgba_six_basic_hues() {
    let cases = [
        (0.0, opaque(255, 0, 0)),
        (60.0, opaque(255, 255, 0)),
        (120.0, opaque(0, 255, 0)),
        (180.0, opaque(0, 255, 255)),
        (240.0, opaque(0, 0, 255)),
        (300.0, opaque(255, 0, 255)),
    ];
    for (h, expected) in cases {
        assert_eq!(Hsl::new(h, 1.0, 0.5).to_rgba(255), expected, "h = {h}");
    }
}

#[test]
fn to_rgba_half_a_unit_rounds_up() {
    // 127.5 redondea a 128 (la mitad se aleja de cero).
    assert_eq!(Hsl::new(30.0, 1.0, 0.5).to_rgba(255), opaque(255, 128, 0));
}

#[test]
fn to_rgba_saturation_zero_is_gray_for_any_hue() {
    for h in [0.0, 90.0, 359.9] {
        assert_eq!(
            Hsl::new(h, 0.0, 0.5).to_rgba(255),
            opaque(128, 128, 128),
            "h = {h}"
        );
    }
}

#[test]
fn to_rgba_lightness_0_and_1_give_black_and_white() {
    let shades = [0.0, 45.0, 90.0, 180.0, 270.0, 359.9, -30.0, f32::NAN];
    let saturaciones = [0.0, 0.3, 0.5, 1.0];
    for h in shades {
        for s in saturaciones {
            assert_eq!(
                Hsl::new(h, s, 0.0).to_rgba(255),
                opaque(0, 0, 0),
                "h = {h}, s = {s}"
            );
            assert_eq!(
                Hsl::new(h, s, 1.0).to_rgba(255),
                opaque(255, 255, 255),
                "h = {h}, s = {s}"
            );
        }
    }
}

#[test]
fn from_rgba_ignores_alpha() {
    assert_eq!(
        Hsl::from_rgba(Rgba::new(10, 20, 30, 0)),
        Hsl::from_rgba(Rgba::new(10, 20, 30, 255))
    );
}

#[test]
fn to_rgba_copies_the_alpha_and_keeps_the_rgb() {
    let hsl = Hsl::new(210.0, 0.5, 0.4);
    let transparent = hsl.to_rgba(0);
    let partial = hsl.to_rgba(77);
    assert_eq!(transparent.a, 0);
    assert_eq!(partial.a, 77);
    assert_eq!(
        (transparent.r, transparent.g, transparent.b),
        (partial.r, partial.g, partial.b)
    );
}

#[test]
fn round_trip_of_a_fixed_set() {
    let mut colors_2 = Vec::new();
    // Las 8 esquinas del cubo RGB.
    for r in [0u8, 255] {
        for g in [0u8, 255] {
            for b in [0u8, 255] {
                colors_2.push((r, g, b));
            }
        }
    }
    colors_2.extend([
        (1, 0, 0),
        (0, 1, 0),
        (0, 0, 1),
        (254, 255, 255),
        (127, 128, 129),
    ]);
    for (r, g, b) in colors_2 {
        for a in [255u8, 0] {
            let c = Rgba::new(r, g, b, a);
            assert_eq!(Hsl::from_rgba(c).to_rgba(c.a), c);
        }
    }
}

// Exhaustivo de la ida y vuelta (comportamiento 4).

/// Recorre los colores dados y comprueba `from_rgba(c).to_rgba(c.a) == c`.
/// El alfa se deriva del color para comprobar de paso que se copia tal cual.
/// Si hay fallos, el pánico lista los primeros y cuántos son.
fn check_round_trip(colors_2: impl Iterator<Item = (u8, u8, u8)>) {
    const MAX_SHOWN: usize = 10;
    let mut total = 0usize;
    let mut first = Vec::new();
    for (r, g, b) in colors_2 {
        let c = Rgba::new(r, g, b, r ^ g ^ b);
        let ret = Hsl::from_rgba(c).to_rgba(c.a);
        if ret != c {
            total += 1;
            if first.len() < MAX_SHOWN {
                first.push((c, ret));
            }
        }
    }
    assert!(
        total == 0,
        "{total} colores no hacen la ida y vuelta exacta; los primeros (original, vuelta): {first:?}"
    );
}

/// Los 2^24 colores RGB.
fn all_the_rgb() -> impl Iterator<Item = (u8, u8, u8)> {
    (0..=255u8).flat_map(|r| (0..=255u8).flat_map(move |g| (0..=255u8).map(move |b| (r, g, b))))
}

#[test]
fn exhaustive_round_trip_all_rgb() {
    check_round_trip(all_the_rgb());
}

// Propiedades.

/// `f32` arbitrario, con más peso en los casos delicados: NaN, ±∞,
/// subnormales, negativos diminutos y los extremos.
fn f_32_any() -> impl Strategy<Value = f32> {
    prop_oneof![
        6 => proptest::num::f32::ANY,
        1 => Just(f32::NAN),
        1 => Just(f32::INFINITY),
        1 => Just(f32::NEG_INFINITY),
        1 => Just(f32::MIN_POSITIVE / 2.0),
        1 => Just(-1e-9f32),
        1 => Just(f32::MAX),
        1 => Just(f32::MIN),
    ]
}

/// `Hsl` construido con `new` a partir de tres `f32` arbitrarios.
fn hsl_any() -> impl Strategy<Value = Hsl> {
    (f_32_any(), f_32_any(), f_32_any()).prop_map(|(h, s, l)| Hsl::new(h, s, l))
}

/// Canal sesgado hacia valores que producen empates y empalmes de sector.
fn channel_biased() -> impl Strategy<Value = u8> {
    prop_oneof![
        1 => any::<u8>(),
        1 => proptest::sample::select(vec![0u8, 1, 127, 128, 254, 255]),
    ]
}

fn check_invariant(x: Hsl) -> Result<(), TestCaseError> {
    prop_assert!(x.h().is_finite() && x.s().is_finite() && x.l().is_finite());
    prop_assert!((0.0..360.0).contains(&x.h()), "h = {}", x.h());
    prop_assert!((0.0..=1.0).contains(&x.s()), "s = {}", x.s());
    prop_assert!((0.0..=1.0).contains(&x.l()), "l = {}", x.l());
    Ok(())
}

fn is_gray(c: Rgba) -> bool {
    c.r == c.g && c.g == c.b
}

proptest! {
    // 1. Rangos.
    #[test]
    fn from_rgba_respects_the_ranges(r: u8, g: u8, b: u8, a: u8) {
        check_invariant(Hsl::from_rgba(Rgba::new(r, g, b, a)))?;
    }

    #[test]
    fn new_respects_the_ranges_and_does_not_panic(
        h in f_32_any(),
        s in f_32_any(),
        l in f_32_any(),
    ) {
        let x = Hsl::new(h, s, l);
        check_invariant(x)?;
        // Tampoco hace pánico al convertir de vuelta.
        let _ = x.to_rgba(255);
    }

    // 2. Normalización.
    #[test]
    fn new_is_idempotent(
        h in f_32_any(),
        s in f_32_any(),
        l in f_32_any(),
    ) {
        let x = Hsl::new(h, s, l);
        prop_assert_eq!(Hsl::new(x.h(), x.s(), x.l()), x);
    }

    #[test]
    fn new_is_equivalent_to_adding_whole_turns(
        h in -1000i32..=1000,
        k in -3i32..=3,
        s in 0.0f32..=1.0,
        l in 0.0f32..=1.0,
    ) {
        let h0 = h as f32;
        let shifted = h0 + 360.0 * k as f32;
        prop_assert_eq!(Hsl::new(shifted, s, l), Hsl::new(h0, s, l));
    }

    // 3. Ida y vuelta.
    #[test]
    fn round_trip_exact(r: u8, g: u8, b: u8, a: u8) {
        let c = Rgba::new(r, g, b, a);
        prop_assert_eq!(Hsl::from_rgba(c).to_rgba(c.a), c);
    }

    // 4. Rotación de canales: (r, g, b) -> (b, r, g) suma 120 grados.
    #[test]
    fn rotating_channels_adds_120_degrees(
        r in channel_biased(),
        g in channel_biased(),
        b in channel_biased(),
        a: u8,
    ) {
        let c = Rgba::new(r, g, b, a);
        prop_assume!(!is_gray(c));
        let rotated = Rgba::new(c.b, c.r, c.g, c.a);
        let (h0, h1) = (Hsl::from_rgba(c), Hsl::from_rgba(rotated));
        // Exacto: salen de los mismos `max` y `min`.
        prop_assert_eq!(h1.s(), h0.s());
        prop_assert_eq!(h1.l(), h0.l());
        let expected = (h0.h() + 120.0) % 360.0;
        let d = distance_circular(h1.h(), expected);
        prop_assert!(d < 1e-3, "h = {}, rotado = {}, esperado = {}", h0.h(), h1.h(), expected);
    }

    // 5. Grises.
    #[test]
    fn grays_have_no_hue_or_saturation(v: u8, a: u8) {
        let hsl = Hsl::from_rgba(Rgba::new(v, v, v, a));
        prop_assert_eq!(hsl.h(), 0.0);
        prop_assert_eq!(hsl.s(), 0.0);
        prop_assert!(near(hsl.l(), v as f32 / 255.0, 1e-6), "l = {}", hsl.l());
    }

    // 6. Alfa.
    #[test]
    fn to_rgba_copies_the_alpha_and_the_rgb_does_not_depend_on_it(x in hsl_any(), alpha: u8) {
        let c = x.to_rgba(alpha);
        prop_assert_eq!(c.a, alpha);
        let reference = x.to_rgba(255);
        prop_assert_eq!((c.r, c.g, c.b), (reference.r, reference.g, reference.b));
    }

    // 7. Cromáticos.
    #[test]
    fn chromatic_colors_have_saturation_and_are_not_grays(
        r in channel_biased(),
        g in channel_biased(),
        b in channel_biased(),
        a: u8,
    ) {
        let c = Rgba::new(r, g, b, a);
        prop_assume!(!is_gray(c));
        let hsl = Hsl::from_rgba(c);
        prop_assert!(hsl.s() > 0.0, "s = {}", hsl.s());
        prop_assert!(!is_gray(hsl.to_rgba(c.a)), "{:?} da un gris", hsl);
    }
}
