//! Tests de `Hsl` y su conversión con `Rgba` (specs/hito-2-hsl.md).

use std::fmt::Debug;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Hsl, Rgba};

// Comprobación en tiempo de compilación de los traits exigidos.
fn assert_hsl_traits<T: Copy + Clone + Debug + PartialEq + Default>() {}

#[test]
fn traits_derivados() {
    assert_hsl_traits::<Hsl>();
}

// Utilidades de apoyo.

/// Tolerancia de las comparaciones de `f32` (salvo las marcadas «exacto»).
const TOL: f32 = 1e-4;

fn cerca(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

fn assert_cerca(real: f32, esperado: f32, que: &str) {
    assert!(
        cerca(real, esperado, TOL),
        "{que}: se esperaba {esperado}, salió {real}"
    );
}

/// Comprueba `h`, `s` y `l` de un `Hsl` con la tolerancia por defecto.
fn assert_hsl(hsl: Hsl, h: f32, s: f32, l: f32) {
    assert_cerca(hsl.h(), h, "h");
    assert_cerca(hsl.s(), s, "s");
    assert_cerca(hsl.l(), l, "l");
}

fn opaco(r: u8, g: u8, b: u8) -> Rgba {
    Rgba::new(r, g, b, 255)
}

/// Distancia circular entre dos ángulos en grados.
fn distancia_circular(a: f32, b: f32) -> f32 {
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
fn matiz_se_normaliza() {
    // 360 equivale a 0 (exacto).
    assert_eq!(Hsl::new(360.0, 1.0, 0.5).h(), 0.0);
    assert_cerca(Hsl::new(-60.0, 1.0, 0.5).h(), 300.0, "h de -60");
    assert_cerca(Hsl::new(720.5, 1.0, 0.5).h(), 0.5, "h de 720.5");
}

#[test]
fn matiz_no_finito_pasa_a_cero() {
    assert_eq!(Hsl::new(f32::NAN, 1.0, 0.5).h(), 0.0);
    assert_eq!(Hsl::new(f32::INFINITY, 1.0, 0.5).h(), 0.0);
    assert_eq!(Hsl::new(f32::NEG_INFINITY, 1.0, 0.5).h(), 0.0);
}

#[test]
fn matiz_negativo_diminuto_nunca_es_360() {
    assert_eq!(Hsl::new(-1e-9, 1.0, 0.5).h(), 0.0);
}

#[test]
fn saturacion_y_luminosidad_se_recortan() {
    let a = Hsl::new(0.0, 2.0, -3.0);
    assert_cerca(a.s(), 1.0, "s de 2");
    assert_cerca(a.l(), 0.0, "l de -3");

    let b = Hsl::new(0.0, f32::NAN, f32::NAN);
    assert_cerca(b.s(), 0.0, "s de NaN");
    assert_cerca(b.l(), 0.0, "l de NaN");

    let c = Hsl::new(0.0, f32::INFINITY, f32::NEG_INFINITY);
    assert_cerca(c.s(), 1.0, "s de +inf");
    assert_cerca(c.l(), 0.0, "l de -inf");
}

#[test]
fn from_rgba_primarios_y_secundarios() {
    let casos = [
        (opaco(255, 0, 0), 0.0),
        (opaco(255, 255, 0), 60.0),
        (opaco(0, 255, 0), 120.0),
        (opaco(0, 255, 255), 180.0),
        (opaco(0, 0, 255), 240.0),
        (opaco(255, 0, 255), 300.0),
    ];
    for (color, h) in casos {
        assert_hsl(Hsl::from_rgba(color), h, 1.0, 0.5);
    }
}

#[test]
fn from_rgba_negro_blanco_y_gris_medio() {
    assert_hsl(Hsl::from_rgba(opaco(0, 0, 0)), 0.0, 0.0, 0.0);
    assert_hsl(Hsl::from_rgba(opaco(255, 255, 255)), 0.0, 0.0, 1.0);
    assert_hsl(
        Hsl::from_rgba(opaco(128, 128, 128)),
        0.0,
        0.0,
        128.0 / 255.0,
    );
}

#[test]
fn from_rgba_casi_gris_tiene_saturacion_maxima() {
    assert_hsl(Hsl::from_rgba(opaco(1, 0, 0)), 0.0, 1.0, 1.0 / 510.0);
    assert_hsl(
        Hsl::from_rgba(opaco(255, 254, 254)),
        0.0,
        1.0,
        509.0 / 510.0,
    );
}

#[test]
fn from_rgba_naranja() {
    // h = 60 * 128/255 ≈ 30.1176.
    assert_hsl(Hsl::from_rgba(opaco(255, 128, 0)), 30.1176, 1.0, 0.5);
}

#[test]
fn from_rgba_empates_de_maximo() {
    // Orden de prueba r, g, b.
    assert_cerca(
        Hsl::from_rgba(opaco(255, 255, 0)).h(),
        60.0,
        "h de (255,255,0)",
    );
    assert_cerca(
        Hsl::from_rgba(opaco(0, 255, 255)).h(),
        180.0,
        "h de (0,255,255)",
    );
    assert_cerca(
        Hsl::from_rgba(opaco(255, 0, 255)).h(),
        300.0,
        "h de (255,0,255)",
    );
}

#[test]
fn to_rgba_seis_matices_basicos() {
    let casos = [
        (0.0, opaco(255, 0, 0)),
        (60.0, opaco(255, 255, 0)),
        (120.0, opaco(0, 255, 0)),
        (180.0, opaco(0, 255, 255)),
        (240.0, opaco(0, 0, 255)),
        (300.0, opaco(255, 0, 255)),
    ];
    for (h, esperado) in casos {
        assert_eq!(Hsl::new(h, 1.0, 0.5).to_rgba(255), esperado, "h = {h}");
    }
}

#[test]
fn to_rgba_media_unidad_redondea_hacia_arriba() {
    // 127.5 redondea a 128 (la mitad se aleja de cero).
    assert_eq!(Hsl::new(30.0, 1.0, 0.5).to_rgba(255), opaco(255, 128, 0));
}

#[test]
fn to_rgba_saturacion_cero_es_gris_para_cualquier_matiz() {
    for h in [0.0, 90.0, 359.9] {
        assert_eq!(
            Hsl::new(h, 0.0, 0.5).to_rgba(255),
            opaco(128, 128, 128),
            "h = {h}"
        );
    }
}

#[test]
fn to_rgba_luminosidad_0_y_1_dan_negro_y_blanco() {
    let matices = [0.0, 45.0, 90.0, 180.0, 270.0, 359.9, -30.0, f32::NAN];
    let saturaciones = [0.0, 0.3, 0.5, 1.0];
    for h in matices {
        for s in saturaciones {
            assert_eq!(
                Hsl::new(h, s, 0.0).to_rgba(255),
                opaco(0, 0, 0),
                "h = {h}, s = {s}"
            );
            assert_eq!(
                Hsl::new(h, s, 1.0).to_rgba(255),
                opaco(255, 255, 255),
                "h = {h}, s = {s}"
            );
        }
    }
}

#[test]
fn from_rgba_ignora_el_alfa() {
    assert_eq!(
        Hsl::from_rgba(Rgba::new(10, 20, 30, 0)),
        Hsl::from_rgba(Rgba::new(10, 20, 30, 255))
    );
}

#[test]
fn to_rgba_copia_el_alfa_y_conserva_el_rgb() {
    let hsl = Hsl::new(210.0, 0.5, 0.4);
    let transparente = hsl.to_rgba(0);
    let parcial = hsl.to_rgba(77);
    assert_eq!(transparente.a, 0);
    assert_eq!(parcial.a, 77);
    assert_eq!(
        (transparente.r, transparente.g, transparente.b),
        (parcial.r, parcial.g, parcial.b)
    );
}

#[test]
fn ida_y_vuelta_de_un_conjunto_fijo() {
    let mut colores = Vec::new();
    // Las 8 esquinas del cubo RGB.
    for r in [0u8, 255] {
        for g in [0u8, 255] {
            for b in [0u8, 255] {
                colores.push((r, g, b));
            }
        }
    }
    colores.extend([
        (1, 0, 0),
        (0, 1, 0),
        (0, 0, 1),
        (254, 255, 255),
        (127, 128, 129),
    ]);
    for (r, g, b) in colores {
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
fn comprobar_ida_y_vuelta(colores: impl Iterator<Item = (u8, u8, u8)>) {
    const MAX_MOSTRADOS: usize = 10;
    let mut total = 0usize;
    let mut primeros = Vec::new();
    for (r, g, b) in colores {
        let c = Rgba::new(r, g, b, r ^ g ^ b);
        let vuelta = Hsl::from_rgba(c).to_rgba(c.a);
        if vuelta != c {
            total += 1;
            if primeros.len() < MAX_MOSTRADOS {
                primeros.push((c, vuelta));
            }
        }
    }
    assert!(
        total == 0,
        "{total} colores no hacen la ida y vuelta exacta; los primeros (original, vuelta): {primeros:?}"
    );
}

/// Los 2^24 colores RGB.
fn todos_los_rgb() -> impl Iterator<Item = (u8, u8, u8)> {
    (0..=255u8).flat_map(|r| (0..=255u8).flat_map(move |g| (0..=255u8).map(move |b| (r, g, b))))
}

#[test]
fn ida_y_vuelta_exhaustiva_todos_los_rgb() {
    comprobar_ida_y_vuelta(todos_los_rgb());
}

// Propiedades.

/// `f32` arbitrario, con más peso en los casos delicados: NaN, ±∞,
/// subnormales, negativos diminutos y los extremos.
fn f32_cualquiera() -> impl Strategy<Value = f32> {
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
fn hsl_cualquiera() -> impl Strategy<Value = Hsl> {
    (f32_cualquiera(), f32_cualquiera(), f32_cualquiera()).prop_map(|(h, s, l)| Hsl::new(h, s, l))
}

/// Canal sesgado hacia valores que producen empates y empalmes de sector.
fn canal_sesgado() -> impl Strategy<Value = u8> {
    prop_oneof![
        1 => any::<u8>(),
        1 => proptest::sample::select(vec![0u8, 1, 127, 128, 254, 255]),
    ]
}

fn comprobar_invariante(x: Hsl) -> Result<(), TestCaseError> {
    prop_assert!(x.h().is_finite() && x.s().is_finite() && x.l().is_finite());
    prop_assert!((0.0..360.0).contains(&x.h()), "h = {}", x.h());
    prop_assert!((0.0..=1.0).contains(&x.s()), "s = {}", x.s());
    prop_assert!((0.0..=1.0).contains(&x.l()), "l = {}", x.l());
    Ok(())
}

fn es_gris(c: Rgba) -> bool {
    c.r == c.g && c.g == c.b
}

proptest! {
    // 1. Rangos.
    #[test]
    fn from_rgba_respeta_los_rangos(r: u8, g: u8, b: u8, a: u8) {
        comprobar_invariante(Hsl::from_rgba(Rgba::new(r, g, b, a)))?;
    }

    #[test]
    fn new_respeta_los_rangos_y_no_hace_panic(
        h in f32_cualquiera(),
        s in f32_cualquiera(),
        l in f32_cualquiera(),
    ) {
        let x = Hsl::new(h, s, l);
        comprobar_invariante(x)?;
        // Tampoco hace pánico al convertir de vuelta.
        let _ = x.to_rgba(255);
    }

    // 2. Normalización.
    #[test]
    fn new_es_idempotente(
        h in f32_cualquiera(),
        s in f32_cualquiera(),
        l in f32_cualquiera(),
    ) {
        let x = Hsl::new(h, s, l);
        prop_assert_eq!(Hsl::new(x.h(), x.s(), x.l()), x);
    }

    #[test]
    fn new_equivale_a_sumar_vueltas_enteras(
        h in -1000i32..=1000,
        k in -3i32..=3,
        s in 0.0f32..=1.0,
        l in 0.0f32..=1.0,
    ) {
        let h0 = h as f32;
        let desplazado = h0 + 360.0 * k as f32;
        prop_assert_eq!(Hsl::new(desplazado, s, l), Hsl::new(h0, s, l));
    }

    // 3. Ida y vuelta.
    #[test]
    fn ida_y_vuelta_exacta(r: u8, g: u8, b: u8, a: u8) {
        let c = Rgba::new(r, g, b, a);
        prop_assert_eq!(Hsl::from_rgba(c).to_rgba(c.a), c);
    }

    // 4. Rotación de canales: (r, g, b) -> (b, r, g) suma 120 grados.
    #[test]
    fn rotar_canales_suma_120_grados(
        r in canal_sesgado(),
        g in canal_sesgado(),
        b in canal_sesgado(),
        a: u8,
    ) {
        let c = Rgba::new(r, g, b, a);
        prop_assume!(!es_gris(c));
        let rotado = Rgba::new(c.b, c.r, c.g, c.a);
        let (h0, h1) = (Hsl::from_rgba(c), Hsl::from_rgba(rotado));
        // Exacto: salen de los mismos `max` y `min`.
        prop_assert_eq!(h1.s(), h0.s());
        prop_assert_eq!(h1.l(), h0.l());
        let esperado = (h0.h() + 120.0) % 360.0;
        let d = distancia_circular(h1.h(), esperado);
        prop_assert!(d < 1e-3, "h = {}, rotado = {}, esperado = {}", h0.h(), h1.h(), esperado);
    }

    // 5. Grises.
    #[test]
    fn los_grises_no_tienen_matiz_ni_saturacion(v: u8, a: u8) {
        let hsl = Hsl::from_rgba(Rgba::new(v, v, v, a));
        prop_assert_eq!(hsl.h(), 0.0);
        prop_assert_eq!(hsl.s(), 0.0);
        prop_assert!(cerca(hsl.l(), v as f32 / 255.0, 1e-6), "l = {}", hsl.l());
    }

    // 6. Alfa.
    #[test]
    fn to_rgba_copia_el_alfa_y_el_rgb_no_depende_de_el(x in hsl_cualquiera(), alpha: u8) {
        let c = x.to_rgba(alpha);
        prop_assert_eq!(c.a, alpha);
        let referencia = x.to_rgba(255);
        prop_assert_eq!((c.r, c.g, c.b), (referencia.r, referencia.g, referencia.b));
    }

    // 7. Cromáticos.
    #[test]
    fn los_cromaticos_tienen_saturacion_y_no_son_grises(
        r in canal_sesgado(),
        g in canal_sesgado(),
        b in canal_sesgado(),
        a: u8,
    ) {
        let c = Rgba::new(r, g, b, a);
        prop_assume!(!es_gris(c));
        let hsl = Hsl::from_rgba(c);
        prop_assert!(hsl.s() > 0.0, "s = {}", hsl.s());
        prop_assert!(!es_gris(hsl.to_rgba(c.a)), "{:?} da un gris", hsl);
    }
}
