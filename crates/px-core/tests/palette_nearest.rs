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

fn opaco(r: u8, g: u8, b: u8) -> Rgba {
    Rgba::opaque(r, g, b)
}

fn paleta(colores: &[Rgba]) -> Palette {
    Palette::from_colors(colores.iter().copied()).unwrap()
}

fn paleta_con_transparente(colores: &[Rgba], indice: u8) -> Palette {
    let mut p = paleta(colores);
    p.set_transparent_index(Some(indice)).unwrap();
    p
}

/// Distancia del oráculo entre dos colores ya resueltos: forma EXPANDIDA de la
/// fórmula, `Σ_c (c_p·a_p − c_q·a_q)² + 255²·(a_p − a_q)²`, en `i128`. No usa el
/// vector premultiplicado ni comparte nada con la implementación.
fn distancia(p: Rgba, q: Rgba) -> i128 {
    let canal = |cp: u8, cq: u8| {
        let d = i128::from(cp) * i128::from(p.a) - i128::from(cq) * i128::from(q.a);
        d * d
    };
    let da = i128::from(p.a) - i128::from(q.a);
    canal(p.r, q.r) + canal(p.g, q.g) + canal(p.b, q.b) + 255 * 255 * da * da
}

/// Distancia de `consulta` a cada entrada de la paleta, por su valor resuelto.
fn distancias(p: &Palette, consulta: Rgba) -> Vec<i128> {
    (0..p.len())
        .map(|i| distancia(consulta, p.resolved(i as u8).unwrap()))
        .collect()
}

/// Oráculo ingenuo: mínimo por (distancia, índice) sobre `resolved(i)`.
fn oraculo(p: &Palette, consulta: Rgba) -> Option<u8> {
    distancias(p, consulta)
        .into_iter()
        .enumerate()
        .min_by_key(|&(i, d)| (d, i))
        .map(|(i, _)| i as u8)
}

/// Comprueba `nearest` contra el valor de la tabla y, de paso, que el oráculo
/// del test también da ese valor (así el oráculo queda anclado a la tabla).
fn espera(p: &Palette, consulta: Rgba, esperado: Option<u8>) {
    assert_eq!(p.nearest(consulta), esperado, "nearest de {consulta:?}");
    assert_eq!(
        oraculo(p, consulta),
        esperado,
        "el oráculo del test no coincide con la tabla para {consulta:?}"
    );
}

/// Tres primarios opacos: la paleta `R` de la tabla.
fn primarios() -> Palette {
    paleta(&[opaco(255, 0, 0), opaco(0, 255, 0), opaco(0, 0, 255)])
}

/// La paleta `Q` de la tabla.
fn q_rojo_y_gris() -> [Rgba; 2] {
    [opaco(200, 0, 0), opaco(9, 8, 7)]
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn paleta_vacia_devuelve_none() {
    let p = Palette::new();
    for c in [
        rgba(0, 0, 0, 0),
        rgba(0, 0, 0, 255),
        rgba(255, 255, 255, 255),
        rgba(10, 20, 30, 128),
    ] {
        assert_eq!(p.nearest(c), None);
        assert_eq!(oraculo(&p, c), None);
    }
    assert_eq!(Palette::default().nearest(rgba(0, 0, 0, 0)), None);
}

#[test]
fn una_entrada_con_distancia_maxima() {
    let p = paleta(&[rgba(0, 0, 0, 0)]);
    let consulta = rgba(255, 255, 255, 255);
    assert_eq!(distancias(&p, consulta), [16_913_002_500]);
    espera(&p, consulta, Some(0));
}

#[test]
fn primarios_exactos() {
    let p = primarios();
    espera(&p, opaco(255, 0, 0), Some(0));
    espera(&p, opaco(0, 255, 0), Some(1));
    espera(&p, opaco(0, 0, 255), Some(2));
    // A mano: la de en medio.
    assert_eq!(
        distancias(&p, opaco(0, 255, 0)),
        [8_456_501_250, 0, 8_456_501_250]
    );
}

#[test]
fn primarios_con_opacos_intermedios() {
    let p = primarios();
    // A mano.
    let consulta = opaco(200, 30, 30);
    assert_eq!(
        distancias(&p, consulta),
        [313_745_625, 5_951_413_125, 5_951_413_125]
    );
    espera(&p, consulta, Some(0));
    espera(&p, opaco(10, 10, 240), Some(2));
}

#[test]
fn doscientos_cincuenta_y_seis_grises() {
    let p = Palette::from_colors((0..=255u8).map(|i| opaco(i, i, i))).unwrap();
    assert_eq!(p.len(), 256);
    for v in 0..=255u8 {
        espera(&p, opaco(v, v, v), Some(v));
    }
    espera(&p, opaco(254, 255, 255), Some(255));
}

#[test]
fn empate_exacto_gana_el_indice_bajo() {
    let a = paleta(&[opaco(0, 0, 0), opaco(2, 0, 0)]);
    let b = paleta(&[opaco(2, 0, 0), opaco(0, 0, 0)]);
    let consulta = opaco(1, 0, 0);
    assert_eq!(distancias(&a, consulta)[0], distancias(&a, consulta)[1]);
    assert_eq!(distancias(&b, consulta)[0], distancias(&b, consulta)[1]);
    espera(&a, consulta, Some(0));
    espera(&b, consulta, Some(0));
}

#[test]
fn duplicados_devuelven_el_primero() {
    let p = paleta(&[opaco(9, 9, 9), opaco(5, 5, 5), opaco(5, 5, 5)]);
    espera(&p, opaco(5, 5, 5), Some(1));
}

#[test]
fn transparente_gana_al_negro_opaco_con_consulta_transparente() {
    // A mano.
    let p = paleta(&[opaco(0, 0, 0), rgba(255, 255, 255, 0)]);
    let consulta = rgba(0, 0, 0, 0);
    assert_eq!(distancias(&p, consulta), [4_228_250_625, 0]);
    espera(&p, consulta, Some(1));
}

#[test]
fn alfa_cero_ignora_el_rgb_y_el_exacto_no_devuelve_su_indice() {
    let p = paleta(&[rgba(10, 20, 30, 0), rgba(10, 20, 30, 255)]);
    espera(&p, rgba(200, 100, 50, 0), Some(0));

    let q = paleta(&[rgba(1, 2, 3, 0), rgba(9, 9, 9, 0)]);
    espera(&q, rgba(9, 9, 9, 0), Some(0));
}

#[test]
fn indice_transparente_cuenta_como_entrada_transparente() {
    let colores = [opaco(1, 1, 1), rgba(5, 5, 5, 0), rgba(7, 7, 7, 0)];
    let consulta = rgba(0, 0, 0, 0);

    espera(&paleta(&colores), consulta, Some(1));
    espera(&paleta_con_transparente(&colores, 0), consulta, Some(0));
}

#[test]
fn alfa_cero_sin_transparente_va_a_la_mas_oscura() {
    let p = paleta(&[opaco(0, 0, 0), opaco(255, 255, 255)]);
    espera(&p, rgba(255, 255, 255, 0), Some(0));

    let q = paleta(&q_rojo_y_gris());
    espera(&q, rgba(0, 0, 0, 0), Some(1));
}

#[test]
fn q_sin_indice_transparente() {
    let p = paleta(&q_rojo_y_gris());
    espera(&p, opaco(200, 0, 0), Some(0));
    espera(&p, rgba(0, 0, 0, 0), Some(1));
}

#[test]
fn q_con_indice_transparente_0_no_encuentra_su_valor_guardado() {
    let p = paleta_con_transparente(&q_rojo_y_gris(), 0);
    espera(&p, rgba(0, 0, 0, 0), Some(0));
    espera(&p, opaco(200, 0, 0), Some(1));
}

#[test]
fn q_con_indice_transparente_1() {
    let p = paleta_con_transparente(&q_rojo_y_gris(), 1);
    espera(&p, opaco(9, 8, 7), Some(0));
    espera(&p, rgba(0, 0, 0, 0), Some(1));
}

#[test]
fn umbral_de_alfa_entre_127_y_128() {
    // A mano.
    let p = paleta(&[opaco(255, 255, 255), rgba(255, 255, 255, 0)]);

    let c127 = rgba(255, 255, 255, 127);
    assert_eq!(distancias(&p, c127), [4_261_478_400, 4_195_152_900]);
    espera(&p, c127, Some(1));

    let c128 = rgba(255, 255, 255, 128);
    assert_eq!(distancias(&p, c128), [4_195_152_900, 4_261_478_400]);
    espera(&p, c128, Some(0));
}

#[test]
fn semitransparentes_van_a_la_entrada_de_alfa_parecido() {
    let p = paleta(&[rgba(255, 0, 0, 128), opaco(255, 0, 0), rgba(0, 0, 0, 0)]);
    espera(&p, rgba(255, 0, 0, 130), Some(0));
    espera(&p, rgba(255, 0, 0, 250), Some(1));
    espera(&p, rgba(0, 0, 255, 10), Some(2));
}

// Estrategias.

/// Valores por canal que fuerzan empates, duplicados y alfa 0.
const CANALES: [u8; 5] = [0, 1, 127, 128, 255];

fn canal() -> impl Strategy<Value = u8> {
    prop_oneof![any::<u8>(), prop::sample::select(CANALES.to_vec())]
}

fn color_uniforme() -> impl Strategy<Value = Rgba> {
    (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()).prop_map(|(r, g, b, a)| rgba(r, g, b, a))
}

fn color_pequeno() -> impl Strategy<Value = Rgba> {
    let c = || prop::sample::select(CANALES.to_vec());
    (c(), c(), c(), c()).prop_map(|(r, g, b, a)| rgba(r, g, b, a))
}

// Mezcla colores uniformes con colores del conjunto pequeño.
fn color() -> impl Strategy<Value = Rgba> {
    prop_oneof![color_uniforme(), color_pequeno()]
}

fn color_opaco() -> impl Strategy<Value = Rgba> {
    color().prop_map(|c| opaco(c.r, c.g, c.b))
}

/// Paleta con los colores dados y, si `t` es `Some`, índice transparente
/// `t % len` (si la paleta no está vacía).
fn construye(colores: &[Rgba], t: Option<u8>) -> Palette {
    let mut p = paleta(colores);
    if let Some(t) = t
        && !p.is_empty()
    {
        p.set_transparent_index(Some((t as usize % p.len()) as u8))
            .unwrap();
    }
    p
}

// Paletas de `tam` entradas, con índice transparente o sin él.
fn paleta_arbitraria(tam: std::ops::RangeInclusive<usize>) -> impl Strategy<Value = Palette> {
    (
        prop::collection::vec(color(), tam),
        prop::option::of(any::<u8>()),
    )
        .prop_map(|(v, t)| construye(&v, t))
}

// Paletas opacas, sin índice transparente.
fn paleta_opaca(tam: std::ops::RangeInclusive<usize>) -> impl Strategy<Value = Palette> {
    prop::collection::vec(color_opaco(), tam).prop_map(|v| paleta(&v))
}

// Propiedades, como funciones para compartirlas entre paletas pequeñas y grandes.

fn prop_oraculo(p: &Palette, consulta: Rgba) -> Result<(), TestCaseError> {
    let esperado = oraculo(p, consulta);
    let real = p.nearest(consulta);
    prop_assert_eq!(real, esperado);
    prop_assert_eq!(real.is_none(), p.is_empty());
    if let Some(i) = real {
        prop_assert!((i as usize) < p.len());
    }
    Ok(())
}

/// Dos entradas resueltas son equivalentes si son iguales o ambas tienen `a = 0`.
fn equivalentes(a: Rgba, b: Rgba) -> bool {
    a == b || (a.a == 0 && b.a == 0)
}

fn prop_exacto_devuelve_el_primero_equivalente(p: &Palette) -> Result<(), TestCaseError> {
    let resueltos: Vec<Rgba> = (0..p.len()).map(|i| p.resolved(i as u8).unwrap()).collect();
    let sin_duplicados_ni_transparentes = resueltos
        .iter()
        .enumerate()
        .all(|(k, &c)| resueltos[..k].iter().all(|&otro| !equivalentes(otro, c)));
    for (i, &c) in resueltos.iter().enumerate() {
        let j = resueltos.iter().position(|&otro| equivalentes(otro, c));
        prop_assert_eq!(p.nearest(c), j.map(|j| j as u8));
        prop_assert!(j.unwrap() <= i);
        if sin_duplicados_ni_transparentes {
            prop_assert_eq!(j, Some(i));
        }
    }
    Ok(())
}

fn prop_opacos_como_rgb(p: &Palette, consulta: Rgba) -> Result<(), TestCaseError> {
    let rgb = |c: Rgba, d: Rgba| {
        let delta = |x: u8, y: u8| (i64::from(x) - i64::from(y)).pow(2);
        delta(c.r, d.r) + delta(c.g, d.g) + delta(c.b, d.b)
    };
    let esperado = p
        .colors()
        .iter()
        .enumerate()
        .min_by_key(|&(i, &c)| (rgb(consulta, c), i))
        .map(|(i, _)| i as u8);
    prop_assert_eq!(p.nearest(consulta), esperado);
    Ok(())
}

fn prop_alfa_cero(p: &Palette, r: u8, g: u8, b: u8) -> Result<(), TestCaseError> {
    let primera = (0..p.len()).find(|&i| p.resolved(i as u8).unwrap().a == 0);
    if let Some(i) = primera {
        prop_assert_eq!(p.nearest(rgba(r, g, b, 0)), Some(i as u8));
    }
    Ok(())
}

fn prop_resuelto_equivale_a_sustituir(
    p: &Palette,
    t: u8,
    consulta: Rgba,
) -> Result<(), TestCaseError> {
    if p.is_empty() {
        return Ok(());
    }
    let t = (t as usize % p.len()) as u8;

    let mut con_indice = p.clone();
    con_indice.set_transparent_index(Some(t)).unwrap();

    let mut sustituida = p.clone();
    sustituida.set_transparent_index(None).unwrap();
    sustituida.set(t, con_indice.resolved(t).unwrap()).unwrap();

    prop_assert_eq!(sustituida.transparent_index(), None);
    prop_assert_eq!(con_indice.nearest(consulta), sustituida.nearest(consulta));
    Ok(())
}

fn prop_rgb_transparente_no_cuenta(
    p: &Palette,
    consultas: &[Rgba],
    (r, g, b): (u8, u8, u8),
    (r2, g2, b2): (u8, u8, u8),
) -> Result<(), TestCaseError> {
    // Consulta con a = 0: el RGB no influye.
    prop_assert_eq!(p.nearest(rgba(r, g, b, 0)), p.nearest(rgba(r2, g2, b2, 0)));

    // Entradas con a = 0 (guardada o por el índice transparente): su RGB no influye.
    let sustituidas: Vec<Rgba> = (0..p.len())
        .map(|i| {
            let guardado = p.get(i as u8).unwrap();
            if p.resolved(i as u8).unwrap().a == 0 {
                rgba(r2, g2, b2, guardado.a)
            } else {
                guardado
            }
        })
        .collect();
    let mut otra = paleta(&sustituidas);
    otra.set_transparent_index(p.transparent_index()).unwrap();

    let entradas = (0..p.len()).map(|i| p.resolved(i as u8).unwrap());
    for c in consultas.iter().copied().chain(entradas) {
        prop_assert_eq!(p.nearest(c), otra.nearest(c));
    }
    Ok(())
}

// Mismas propiedades para paletas pequeñas (0..=8) y grandes (0..=256, menos casos).
macro_rules! propiedades {
    ($modulo:ident, $tam:expr, $casos:expr) => {
        mod $modulo {
            use super::*;

            proptest! {
                #![proptest_config(ProptestConfig::with_cases($casos))]

                #[test]
                fn oraculo_ingenuo(p in paleta_arbitraria($tam), consulta in color()) {
                    prop_oraculo(&p, consulta)?;
                    // Las propias entradas, guardadas y resueltas, como consultas.
                    for i in 0..p.len() {
                        prop_oraculo(&p, p.get(i as u8).unwrap())?;
                        prop_oraculo(&p, p.resolved(i as u8).unwrap())?;
                    }
                }

                #[test]
                fn exacto_devuelve_el_primero_equivalente(p in paleta_arbitraria($tam)) {
                    prop_exacto_devuelve_el_primero_equivalente(&p)?;
                }

                #[test]
                fn opacos_como_rgb(p in paleta_opaca($tam), consulta in color_opaco()) {
                    prop_opacos_como_rgb(&p, consulta)?;
                }

                #[test]
                fn alfa_cero_va_a_la_primera_transparente(
                    p in paleta_arbitraria($tam),
                    r in canal(),
                    g in canal(),
                    b in canal(),
                ) {
                    prop_alfa_cero(&p, r, g, b)?;
                }

                #[test]
                fn resuelto_equivale_a_sustituir(
                    p in paleta_arbitraria($tam),
                    t: u8,
                    consulta in color(),
                ) {
                    prop_resuelto_equivale_a_sustituir(&p, t, consulta)?;
                }

                #[test]
                fn rgb_transparente_no_cuenta(
                    p in paleta_arbitraria($tam),
                    consultas in prop::collection::vec(color(), 1..=4),
                    rgb in (canal(), canal(), canal()),
                    otro_rgb in (canal(), canal(), canal()),
                ) {
                    prop_rgb_transparente_no_cuenta(&p, &consultas, rgb, otro_rgb)?;
                }
            }
        }
    };
}

propiedades!(paletas_pequenas, 0..=8, 256);
propiedades!(paletas_grandes, 0..=256, 32);
