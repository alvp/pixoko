//! Tests de `Palette`, paleta de hasta 256 colores (specs/hito-1-palette.md).

use std::fmt::{Debug, Display};
use std::ops::RangeInclusive;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Palette, PaletteError, Rgba};

// Comprobaciones en tiempo de compilación de los traits exigidos.
fn assert_palette_traits<T: Clone + Debug + Default + PartialEq + Eq>() {}
fn assert_error_traits<T: Debug + Clone + PartialEq + Eq + Display + std::error::Error>() {}

#[test]
fn traits_derivados() {
    assert_palette_traits::<Palette>();
    assert_error_traits::<PaletteError>();
}

#[test]
fn max_colors_vale_256() {
    assert_eq!(Palette::MAX_COLORS, 256);
}

// Colores y paletas de apoyo.

fn c0() -> Rgba {
    Rgba::new(10, 20, 30, 255)
}

fn c1() -> Rgba {
    Rgba::new(40, 50, 60, 255)
}

fn c2() -> Rgba {
    Rgba::new(70, 80, 90, 255)
}

fn paleta(colores: &[Rgba]) -> Palette {
    Palette::from_colors(colores.iter().copied()).unwrap()
}

fn paleta_de(n: usize) -> Palette {
    Palette::from_colors((0..n).map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))).unwrap()
}

fn fuera_de_rango(index: u8, len: usize) -> PaletteError {
    PaletteError::IndexOutOfRange { index, len }
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn new_y_default_son_vacias() {
    for p in [Palette::new(), Palette::default()] {
        assert_eq!(p.len(), 0);
        assert!(p.is_empty());
        assert_eq!(p.transparent_index(), None);
    }
    assert_eq!(Palette::new(), Palette::default());
}

#[test]
fn from_colors_con_0_1_y_256() {
    for n in [0usize, 1, 256] {
        let entrada: Vec<Rgba> = (0..n).map(|i| Rgba::new(i as u8, 1, 2, 255)).collect();
        let p = Palette::from_colors(entrada.iter().copied()).unwrap();
        assert_eq!(p.len(), n);
        assert_eq!(p.is_empty(), n == 0);
        assert_eq!(p.colors(), entrada.as_slice());
    }
    let p = Palette::from_colors((0..=255u8).map(|i| Rgba::opaque(i, i, i))).unwrap();
    assert_eq!(p.len(), 256);
}

#[test]
fn from_colors_con_257_e_infinito() {
    let r = Palette::from_colors((0..=256usize).map(|i| Rgba::opaque(i as u8, 0, 0)));
    assert_eq!(r, Err(PaletteError::TooManyColors));

    let r = Palette::from_colors(std::iter::repeat(c0()));
    assert_eq!(r, Err(PaletteError::TooManyColors));
}

#[test]
fn from_colors_no_lee_mas_de_257_elementos() {
    let mut leidos = 0usize;
    let r = Palette::from_colors(std::iter::repeat(c0()).inspect(|_| leidos += 1));
    assert_eq!(r, Err(PaletteError::TooManyColors));
    assert!(leidos <= 257, "leyó {leidos} elementos");
}

#[test]
fn duplicados_se_conservan_e_independientes() {
    let mut p = paleta(&[c0(), c0(), c0()]);
    assert_eq!(p.colors(), &[c0(), c0(), c0()]);
    assert_eq!(p.set(0, c2()), Ok(()));
    assert_eq!(p.colors(), &[c2(), c0(), c0()]);
}

#[test]
fn alfa_y_rgb_intactos() {
    let p = paleta(&[Rgba::new(1, 2, 3, 0), Rgba::new(1, 2, 3, 128)]);
    assert_eq!(p.get(0), Ok(Rgba::new(1, 2, 3, 0)));
    assert_eq!(p.get(1), Ok(Rgba::new(1, 2, 3, 128)));
}

#[test]
fn get_en_rango_y_ultimo_indice() {
    assert_eq!(paleta(&[c0(), c1()]).get(1), Ok(c1()));
    let p = paleta_de(256);
    assert_eq!(p.get(255), Ok(p.colors()[255]));
}

#[test]
fn get_fuera_de_rango() {
    let p = paleta(&[c0(), c1()]);
    assert_eq!(p.get(2), Err(fuera_de_rango(2, 2)));
    assert_eq!(p.get(255), Err(fuera_de_rango(255, 2)));
    assert_eq!(Palette::new().get(0), Err(fuera_de_rango(0, 0)));
}

#[test]
fn push_devuelve_el_indice_nuevo() {
    let mut vacia = Palette::new();
    assert_eq!(vacia.push(c0()), Ok(0));

    let mut p = paleta(&[c0()]);
    assert_eq!(p.push(c1()), Ok(1));
    assert_eq!(p.colors(), &[c0(), c1()]);
}

#[test]
fn push_con_255_y_con_256() {
    let mut p = paleta_de(255);
    assert_eq!(p.push(c0()), Ok(255));
    assert_eq!(p.len(), 256);

    let antes = p.clone();
    assert_eq!(p.push(c1()), Err(PaletteError::TooManyColors));
    assert_eq!(p, antes);
}

#[test]
fn set_sustituye_o_falla_sin_cambios() {
    let mut p = paleta(&[c0(), c1()]);
    assert_eq!(p.set(1, c2()), Ok(()));
    assert_eq!(p.colors(), &[c0(), c2()]);

    let antes = p.clone();
    assert_eq!(p.set(2, c2()), Err(fuera_de_rango(2, 2)));
    assert_eq!(p, antes);
}

#[test]
fn set_transparent_index_valido() {
    let mut p = paleta(&[c0(), c1()]);
    assert_eq!(p.set_transparent_index(Some(1)), Ok(()));
    assert_eq!(p.transparent_index(), Some(1));
}

#[test]
fn set_transparent_index_fuera_de_rango_conserva_el_anterior() {
    let mut p = paleta(&[c0(), c1()]);
    assert_eq!(p.set_transparent_index(Some(2)), Err(fuera_de_rango(2, 2)));
    assert_eq!(p.transparent_index(), None);

    p.set_transparent_index(Some(0)).unwrap();
    let antes = p.clone();
    assert_eq!(p.set_transparent_index(Some(2)), Err(fuera_de_rango(2, 2)));
    assert_eq!(p.transparent_index(), Some(0));
    assert_eq!(p, antes);

    let mut vacia = Palette::new();
    assert_eq!(
        vacia.set_transparent_index(Some(0)),
        Err(fuera_de_rango(0, 0))
    );
    assert_eq!(vacia.transparent_index(), None);
}

#[test]
fn set_transparent_index_none_siempre_funciona() {
    let mut vacia = Palette::new();
    assert_eq!(vacia.set_transparent_index(None), Ok(()));
    assert_eq!(vacia.transparent_index(), None);

    let mut p = paleta(&[c0(), c1()]);
    p.set_transparent_index(Some(1)).unwrap();
    assert_eq!(p.set_transparent_index(None), Ok(()));
    assert_eq!(p.transparent_index(), None);
    for i in 0..2 {
        assert_eq!(p.resolved(i), p.get(i));
    }
}

#[test]
fn resolved_aplica_el_indice_transparente() {
    let mut p = paleta(&[Rgba::new(9, 8, 7, 255), c1()]);
    p.set_transparent_index(Some(0)).unwrap();
    assert_eq!(p.resolved(0), Ok(Rgba::new(9, 8, 7, 0)));
    assert_eq!(p.get(0), Ok(Rgba::new(9, 8, 7, 255)));
    assert_eq!(p.resolved(1), Ok(c1()));
}

#[test]
fn resolved_no_toca_alfa_cero_y_falla_fuera_de_rango() {
    let p = paleta(&[Rgba::new(9, 8, 7, 0)]);
    assert_eq!(p.resolved(0), Ok(Rgba::new(9, 8, 7, 0)));
    for i in [1u8, 2, 255] {
        assert_eq!(p.resolved(i), Err(fuera_de_rango(i, 1)));
    }
    assert_eq!(Palette::new().resolved(0), Err(fuera_de_rango(0, 0)));
}

#[test]
fn push_y_set_no_cambian_el_indice_transparente() {
    let mut p = paleta(&[c0(), c1()]);
    p.set_transparent_index(Some(0)).unwrap();

    assert_eq!(p.push(c2()), Ok(2));
    assert_eq!(p.transparent_index(), Some(0));

    assert_eq!(p.set(0, Rgba::new(5, 6, 7, 200)), Ok(()));
    assert_eq!(p.transparent_index(), Some(0));
    assert_eq!(p.resolved(0), Ok(Rgba::new(5, 6, 7, 0)));
}

// Estrategias.

fn color_cualquiera() -> impl Strategy<Value = Rgba> {
    (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
        .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
}

// Mezcla colores arbitrarios con un conjunto de tres para forzar duplicados.
fn color() -> impl Strategy<Value = Rgba> {
    prop_oneof![
        color_cualquiera(),
        prop::sample::select(vec![c0(), c1(), Rgba::new(10, 20, 30, 0)]),
    ]
}

fn colores(tamano: RangeInclusive<usize>) -> impl Strategy<Value = Vec<Rgba>> {
    prop::collection::vec(color(), tamano)
}

// Índices cercanos al inicio (casi siempre válidos) o cualesquiera.
fn indice() -> impl Strategy<Value = u8> {
    prop_oneof![3 => 0u8..8, 1 => any::<u8>()]
}

#[derive(Debug, Clone)]
enum Op {
    Push(Rgba),
    Set(u8, Rgba),
    Transparente(Option<u8>),
}

fn operacion() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => color().prop_map(Op::Push),
        2 => (indice(), color()).prop_map(|(i, c)| Op::Set(i, c)),
        2 => prop::option::of(indice()).prop_map(Op::Transparente),
    ]
}

fn aplica(p: &mut Palette, op: &Op) -> Result<(), TestCaseError> {
    let antes = p.clone();
    let len_antes = antes.len();
    match *op {
        Op::Push(c) => match p.push(c) {
            Ok(i) => {
                prop_assert_eq!(i as usize, len_antes);
                prop_assert_eq!(&p.colors()[..len_antes], antes.colors());
                prop_assert_eq!(p.colors()[len_antes], c);
                prop_assert_eq!(p.transparent_index(), antes.transparent_index());
            }
            Err(_) => prop_assert_eq!(&*p, &antes),
        },
        Op::Set(i, c) => match p.set(i, c) {
            Ok(()) => {
                prop_assert_eq!(p.len(), len_antes);
                for k in (0..len_antes).filter(|&k| k != i as usize) {
                    prop_assert_eq!(p.colors()[k], antes.colors()[k]);
                }
                prop_assert_eq!(p.get(i), Ok(c));
                prop_assert_eq!(p.transparent_index(), antes.transparent_index());
            }
            Err(_) => prop_assert_eq!(&*p, &antes),
        },
        Op::Transparente(t) => match p.set_transparent_index(t) {
            Ok(()) => {
                prop_assert_eq!(p.colors(), antes.colors());
                prop_assert_eq!(p.transparent_index(), t);
            }
            Err(_) => prop_assert_eq!(&*p, &antes),
        },
    }
    prop_assert!(p.len() <= Palette::MAX_COLORS);
    if let Some(t) = p.transparent_index() {
        prop_assert!((t as usize) < p.len());
    }
    Ok(())
}

// Propiedades.

proptest! {
    #[test]
    fn construccion_hasta_256(v in colores(0..=256)) {
        let p = Palette::from_colors(v.iter().copied()).unwrap();
        prop_assert_eq!(p.len(), v.len());
        prop_assert_eq!(p.colors(), v.as_slice());
        for (i, c) in v.iter().enumerate() {
            prop_assert_eq!(p.get(i as u8), Ok(*c));
        }
    }

    #[test]
    fn construccion_con_mas_de_256_falla(v in colores(257..=400)) {
        prop_assert_eq!(
            Palette::from_colors(v),
            Err(PaletteError::TooManyColors)
        );
    }

    #[test]
    fn get_y_resolved_acotados(
        v in colores(0..=256),
        transparente in prop::option::of(any::<u8>()),
        index: u8,
    ) {
        let mut p = Palette::from_colors(v).unwrap();
        let _ = p.set_transparent_index(transparente);
        let len = p.len();
        let get = p.get(index);
        let resolved = p.resolved(index);
        if (index as usize) < len {
            prop_assert!(get.is_ok());
            prop_assert!(resolved.is_ok());
        } else {
            let esperado = PaletteError::IndexOutOfRange { index, len };
            prop_assert_eq!(get, Err(esperado.clone()));
            prop_assert_eq!(resolved, Err(esperado));
        }
    }

    #[test]
    fn push_hasta_llenar(v in colores(0..=300)) {
        let mut p = Palette::new();
        for (k, c) in v.iter().enumerate() {
            let antes = p.clone();
            let r = p.push(*c);
            if k < Palette::MAX_COLORS {
                prop_assert_eq!(r, Ok(k as u8));
                prop_assert_eq!(p.colors()[k], *c);
            } else {
                prop_assert_eq!(r, Err(PaletteError::TooManyColors));
                prop_assert_eq!(&p, &antes);
            }
        }
        prop_assert_eq!(p.len(), v.len().min(Palette::MAX_COLORS));
    }

    #[test]
    fn set_es_local(
        v in colores(0..=256),
        transparente in prop::option::of(any::<u8>()),
        index: u8,
        nuevo in color(),
    ) {
        let mut p = Palette::from_colors(v).unwrap();
        let _ = p.set_transparent_index(transparente);
        let antes = p.clone();
        let len = p.len();
        let r = p.set(index, nuevo);
        if (index as usize) < len {
            prop_assert_eq!(r, Ok(()));
            prop_assert_eq!(p.get(index), Ok(nuevo));
            prop_assert_eq!(p.len(), len);
            prop_assert_eq!(p.transparent_index(), antes.transparent_index());
            for k in (0..len).filter(|&k| k != index as usize) {
                prop_assert_eq!(p.colors()[k], antes.colors()[k]);
            }
        } else {
            prop_assert_eq!(r, Err(PaletteError::IndexOutOfRange { index, len }));
            prop_assert_eq!(&p, &antes);
        }
    }

    #[test]
    fn indice_transparente_y_resolved(
        v in colores(0..=256),
        previo in prop::option::of(any::<u8>()),
        index: u8,
    ) {
        let mut p = Palette::from_colors(v).unwrap();
        let _ = p.set_transparent_index(previo);
        let antes = p.clone();
        let len = p.len();

        let r = p.set_transparent_index(Some(index));
        if (index as usize) < len {
            prop_assert_eq!(r, Ok(()));
            prop_assert_eq!(p.transparent_index(), Some(index));
        } else {
            prop_assert_eq!(r, Err(PaletteError::IndexOutOfRange { index, len }));
            prop_assert_eq!(p.transparent_index(), antes.transparent_index());
            prop_assert_eq!(&p, &antes);
        }

        let t = p.transparent_index();
        for i in 0..len {
            let guardado = p.get(i as u8).unwrap();
            let efectivo = p.resolved(i as u8).unwrap();
            if t == Some(i as u8) {
                prop_assert_eq!(efectivo, Rgba::new(guardado.r, guardado.g, guardado.b, 0));
            } else {
                prop_assert_eq!(efectivo, guardado);
            }
        }

        prop_assert_eq!(p.set_transparent_index(None), Ok(()));
        prop_assert_eq!(p.transparent_index(), None);
        for i in 0..len {
            prop_assert_eq!(p.resolved(i as u8), p.get(i as u8));
        }
    }

    #[test]
    fn secuencias_aleatorias(ops in prop::collection::vec(operacion(), 0..=60)) {
        let mut p = Palette::new();
        for op in &ops {
            aplica(&mut p, op)?;
        }
    }
}
