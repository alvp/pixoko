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
fn traits_derived() {
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

fn palette_2(colors_2: &[Rgba]) -> Palette {
    Palette::from_colors(colors_2.iter().copied()).unwrap()
}

fn palette_of(n: usize) -> Palette {
    Palette::from_colors((0..n).map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))).unwrap()
}

fn out_of_range(index: u8, len: usize) -> PaletteError {
    PaletteError::IndexOutOfRange { index, len }
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn new_and_default_are_empty() {
    for p in [Palette::new(), Palette::default()] {
        assert_eq!(p.len(), 0);
        assert!(p.is_empty());
        assert_eq!(p.transparent_index(), None);
    }
    assert_eq!(Palette::new(), Palette::default());
}

#[test]
fn from_colors_with_0_1_and_256() {
    for n in [0usize, 1, 256] {
        let input: Vec<Rgba> = (0..n).map(|i| Rgba::new(i as u8, 1, 2, 255)).collect();
        let p = Palette::from_colors(input.iter().copied()).unwrap();
        assert_eq!(p.len(), n);
        assert_eq!(p.is_empty(), n == 0);
        assert_eq!(p.colors(), input.as_slice());
    }
    let p = Palette::from_colors((0..=255u8).map(|i| Rgba::opaque(i, i, i))).unwrap();
    assert_eq!(p.len(), 256);
}

#[test]
fn from_colors_with_257_and_infinite() {
    let r = Palette::from_colors((0..=256usize).map(|i| Rgba::opaque(i as u8, 0, 0)));
    assert_eq!(r, Err(PaletteError::TooManyColors));

    let r = Palette::from_colors(std::iter::repeat(c0()));
    assert_eq!(r, Err(PaletteError::TooManyColors));
}

#[test]
fn from_colors_does_not_read_more_than_257_elements() {
    let mut read = 0usize;
    let r = Palette::from_colors(std::iter::repeat(c0()).inspect(|_| read += 1));
    assert_eq!(r, Err(PaletteError::TooManyColors));
    assert!(read <= 257, "leyó {read} elementos");
}

#[test]
fn duplicates_are_kept_and_independent() {
    let mut p = palette_2(&[c0(), c0(), c0()]);
    assert_eq!(p.colors(), &[c0(), c0(), c0()]);
    assert_eq!(p.set(0, c2()), Ok(()));
    assert_eq!(p.colors(), &[c2(), c0(), c0()]);
}

#[test]
fn alpha_and_rgb_intact() {
    let p = palette_2(&[Rgba::new(1, 2, 3, 0), Rgba::new(1, 2, 3, 128)]);
    assert_eq!(p.get(0), Ok(Rgba::new(1, 2, 3, 0)));
    assert_eq!(p.get(1), Ok(Rgba::new(1, 2, 3, 128)));
}

#[test]
fn get_in_range_and_last_index() {
    assert_eq!(palette_2(&[c0(), c1()]).get(1), Ok(c1()));
    let p = palette_of(256);
    assert_eq!(p.get(255), Ok(p.colors()[255]));
}

#[test]
fn get_out_of_range() {
    let p = palette_2(&[c0(), c1()]);
    assert_eq!(p.get(2), Err(out_of_range(2, 2)));
    assert_eq!(p.get(255), Err(out_of_range(255, 2)));
    assert_eq!(Palette::new().get(0), Err(out_of_range(0, 0)));
}

#[test]
fn push_returns_the_new_index() {
    let mut empty = Palette::new();
    assert_eq!(empty.push(c0()), Ok(0));

    let mut p = palette_2(&[c0()]);
    assert_eq!(p.push(c1()), Ok(1));
    assert_eq!(p.colors(), &[c0(), c1()]);
}

#[test]
fn push_with_255_and_with_256() {
    let mut p = palette_of(255);
    assert_eq!(p.push(c0()), Ok(255));
    assert_eq!(p.len(), 256);

    let antes = p.clone();
    assert_eq!(p.push(c1()), Err(PaletteError::TooManyColors));
    assert_eq!(p, antes);
}

#[test]
fn set_replaces_or_fails_without_changes() {
    let mut p = palette_2(&[c0(), c1()]);
    assert_eq!(p.set(1, c2()), Ok(()));
    assert_eq!(p.colors(), &[c0(), c2()]);

    let antes = p.clone();
    assert_eq!(p.set(2, c2()), Err(out_of_range(2, 2)));
    assert_eq!(p, antes);
}

#[test]
fn set_transparent_index_valid() {
    let mut p = palette_2(&[c0(), c1()]);
    assert_eq!(p.set_transparent_index(Some(1)), Ok(()));
    assert_eq!(p.transparent_index(), Some(1));
}

#[test]
fn set_transparent_index_out_of_range_keeps_the_previous() {
    let mut p = palette_2(&[c0(), c1()]);
    assert_eq!(p.set_transparent_index(Some(2)), Err(out_of_range(2, 2)));
    assert_eq!(p.transparent_index(), None);

    p.set_transparent_index(Some(0)).unwrap();
    let antes = p.clone();
    assert_eq!(p.set_transparent_index(Some(2)), Err(out_of_range(2, 2)));
    assert_eq!(p.transparent_index(), Some(0));
    assert_eq!(p, antes);

    let mut empty = Palette::new();
    assert_eq!(
        empty.set_transparent_index(Some(0)),
        Err(out_of_range(0, 0))
    );
    assert_eq!(empty.transparent_index(), None);
}

#[test]
fn set_transparent_index_none_always_works() {
    let mut empty = Palette::new();
    assert_eq!(empty.set_transparent_index(None), Ok(()));
    assert_eq!(empty.transparent_index(), None);

    let mut p = palette_2(&[c0(), c1()]);
    p.set_transparent_index(Some(1)).unwrap();
    assert_eq!(p.set_transparent_index(None), Ok(()));
    assert_eq!(p.transparent_index(), None);
    for i in 0..2 {
        assert_eq!(p.resolved(i), p.get(i));
    }
}

#[test]
fn resolved_applies_the_transparent_index() {
    let mut p = palette_2(&[Rgba::new(9, 8, 7, 255), c1()]);
    p.set_transparent_index(Some(0)).unwrap();
    assert_eq!(p.resolved(0), Ok(Rgba::new(9, 8, 7, 0)));
    assert_eq!(p.get(0), Ok(Rgba::new(9, 8, 7, 255)));
    assert_eq!(p.resolved(1), Ok(c1()));
}

#[test]
fn resolved_does_not_touch_alpha_zero_and_fails_out_of_range() {
    let p = palette_2(&[Rgba::new(9, 8, 7, 0)]);
    assert_eq!(p.resolved(0), Ok(Rgba::new(9, 8, 7, 0)));
    for i in [1u8, 2, 255] {
        assert_eq!(p.resolved(i), Err(out_of_range(i, 1)));
    }
    assert_eq!(Palette::new().resolved(0), Err(out_of_range(0, 0)));
}

#[test]
fn push_and_set_do_not_change_the_transparent_index() {
    let mut p = palette_2(&[c0(), c1()]);
    p.set_transparent_index(Some(0)).unwrap();

    assert_eq!(p.push(c2()), Ok(2));
    assert_eq!(p.transparent_index(), Some(0));

    assert_eq!(p.set(0, Rgba::new(5, 6, 7, 200)), Ok(()));
    assert_eq!(p.transparent_index(), Some(0));
    assert_eq!(p.resolved(0), Ok(Rgba::new(5, 6, 7, 0)));
}

// Estrategias.

fn color_any() -> impl Strategy<Value = Rgba> {
    (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
        .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
}

// Mezcla colores arbitrarios con un conjunto de tres para forzar duplicados.
fn color() -> impl Strategy<Value = Rgba> {
    prop_oneof![
        color_any(),
        prop::sample::select(vec![c0(), c1(), Rgba::new(10, 20, 30, 0)]),
    ]
}

fn colors_2(size: RangeInclusive<usize>) -> impl Strategy<Value = Vec<Rgba>> {
    prop::collection::vec(color(), size)
}

// Índices cercanos al inicio (casi siempre válidos) o cualesquiera.
fn index() -> impl Strategy<Value = u8> {
    prop_oneof![3 => 0u8..8, 1 => any::<u8>()]
}

#[derive(Debug, Clone)]
enum Op {
    Push(Rgba),
    Set(u8, Rgba),
    Transparent(Option<u8>),
}

fn operation() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => color().prop_map(Op::Push),
        2 => (index(), color()).prop_map(|(i, c)| Op::Set(i, c)),
        2 => prop::option::of(index()).prop_map(Op::Transparent),
    ]
}

fn applies(p: &mut Palette, op: &Op) -> Result<(), TestCaseError> {
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
        Op::Transparent(t) => match p.set_transparent_index(t) {
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
    fn construction_until_256(v in colors_2(0..=256)) {
        let p = Palette::from_colors(v.iter().copied()).unwrap();
        prop_assert_eq!(p.len(), v.len());
        prop_assert_eq!(p.colors(), v.as_slice());
        for (i, c) in v.iter().enumerate() {
            prop_assert_eq!(p.get(i as u8), Ok(*c));
        }
    }

    #[test]
    fn construction_with_more_than_256_fails(v in colors_2(257..=400)) {
        prop_assert_eq!(
            Palette::from_colors(v),
            Err(PaletteError::TooManyColors)
        );
    }

    #[test]
    fn get_and_resolved_bounded(
        v in colors_2(0..=256),
        transparent in prop::option::of(any::<u8>()),
        index: u8,
    ) {
        let mut p = Palette::from_colors(v).unwrap();
        let _ = p.set_transparent_index(transparent);
        let len = p.len();
        let get = p.get(index);
        let resolved = p.resolved(index);
        if (index as usize) < len {
            prop_assert!(get.is_ok());
            prop_assert!(resolved.is_ok());
        } else {
            let expected = PaletteError::IndexOutOfRange { index, len };
            prop_assert_eq!(get, Err(expected.clone()));
            prop_assert_eq!(resolved, Err(expected));
        }
    }

    #[test]
    fn push_until_fill(v in colors_2(0..=300)) {
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
        v in colors_2(0..=256),
        transparent in prop::option::of(any::<u8>()),
        index: u8,
        new in color(),
    ) {
        let mut p = Palette::from_colors(v).unwrap();
        let _ = p.set_transparent_index(transparent);
        let antes = p.clone();
        let len = p.len();
        let r = p.set(index, new);
        if (index as usize) < len {
            prop_assert_eq!(r, Ok(()));
            prop_assert_eq!(p.get(index), Ok(new));
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
    fn index_transparent_and_resolved(
        v in colors_2(0..=256),
        previous in prop::option::of(any::<u8>()),
        index: u8,
    ) {
        let mut p = Palette::from_colors(v).unwrap();
        let _ = p.set_transparent_index(previous);
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
            let saved = p.get(i as u8).unwrap();
            let effective = p.resolved(i as u8).unwrap();
            if t == Some(i as u8) {
                prop_assert_eq!(effective, Rgba::new(saved.r, saved.g, saved.b, 0));
            } else {
                prop_assert_eq!(effective, saved);
            }
        }

        prop_assert_eq!(p.set_transparent_index(None), Ok(()));
        prop_assert_eq!(p.transparent_index(), None);
        for i in 0..len {
            prop_assert_eq!(p.resolved(i as u8), p.get(i as u8));
        }
    }

    #[test]
    fn sequences_random(ops in prop::collection::vec(operation(), 0..=60)) {
        let mut p = Palette::new();
        for op in &ops {
            applies(&mut p, op)?;
        }
    }
}
