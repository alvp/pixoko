//! Tests de `Rgba` y su notación hexadecimal (specs/hito-1-rgba.md).

use std::fmt::{Debug, Display};
use std::hash::Hash;

use proptest::prelude::*;
use px_core::{ParseHexError, Rgba};

// Comprobaciones en tiempo de compilación de los traits exigidos.
fn assert_rgba_traits<T: Copy + Clone + Debug + PartialEq + Eq + Hash>() {}
fn assert_error_traits<T: Debug + Clone + PartialEq + Eq + Display + std::error::Error>() {}

#[test]
fn traits_derived() {
    assert_rgba_traits::<Rgba>();
    assert_error_traits::<ParseHexError>();
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn black_with_hash() {
    assert_eq!(Rgba::from_hex("#000000"), Ok(Rgba::new(0, 0, 0, 255)));
}

#[test]
fn black_without_hash() {
    assert_eq!(Rgba::from_hex("000000"), Ok(Rgba::new(0, 0, 0, 255)));
}

#[test]
fn short_white_upper_and_lower_case() {
    let white = Rgba::new(255, 255, 255, 255);
    assert_eq!(Rgba::from_hex("#FFF"), Ok(white));
    assert_eq!(Rgba::from_hex("#fff"), Ok(white));
}

#[test]
fn short_with_alpha() {
    assert_eq!(Rgba::from_hex("#f80a"), Ok(Rgba::new(255, 136, 0, 170)));
}

#[test]
fn alpha_zero_keeps_rgb() {
    assert_eq!(Rgba::from_hex("#ff880000"), Ok(Rgba::new(255, 136, 0, 0)));
}

#[test]
fn empty_and_only_hash() {
    assert_eq!(Rgba::from_hex(""), Err(ParseHexError::Length(0)));
    assert_eq!(Rgba::from_hex("#"), Err(ParseHexError::Length(0)));
}

#[test]
fn length_2() {
    assert_eq!(Rgba::from_hex("#ff"), Err(ParseHexError::Length(2)));
}

#[test]
fn length_5() {
    assert_eq!(Rgba::from_hex("#fffff"), Err(ParseHexError::Length(5)));
}

#[test]
fn length_9() {
    assert_eq!(Rgba::from_hex("#fffffffff"), Err(ParseHexError::Length(9)));
}

#[test]
fn digit_invalid_g() {
    assert_eq!(
        Rgba::from_hex("#ggg"),
        Err(ParseHexError::InvalidDigit('g'))
    );
}

#[test]
fn double_hash() {
    // Se quita una sola `#`; quedan 4 caracteres, longitud válida.
    assert_eq!(
        Rgba::from_hex("##fff"),
        Err(ParseHexError::InvalidDigit('#'))
    );
}

#[test]
fn unicode_digit_counts_as_one_character() {
    // 3 caracteres (4 bytes): la longitud es válida y falla el dígito.
    assert_eq!(
        Rgba::from_hex("#ffé"),
        Err(ParseHexError::InvalidDigit('é'))
    );
}

#[test]
fn plus_sign_is_not_a_digit() {
    assert_eq!(
        Rgba::from_hex("+fff"),
        Err(ParseHexError::InvalidDigit('+'))
    );
}

// Valores básicos.

#[test]
fn default_is_transparent() {
    assert_eq!(Rgba::default(), Rgba::new(0, 0, 0, 0));
    assert_eq!(Rgba::default(), Rgba::TRANSPARENT);
}

#[test]
fn transparent_is_zero() {
    let t = Rgba::TRANSPARENT;
    assert_eq!((t.r, t.g, t.b, t.a), (0, 0, 0, 0));
}

#[test]
fn opaque_sets_alpha_255() {
    let c = Rgba::opaque(1, 2, 3);
    assert_eq!((c.r, c.g, c.b, c.a), (1, 2, 3, 255));
    assert_eq!(c, Rgba::new(1, 2, 3, 255));
}

#[test]
fn direct_alpha_round_trip() {
    let c = Rgba::new(10, 20, 30, 0);
    let read = Rgba::from_hex(&c.to_string()).unwrap();
    assert_eq!((read.r, read.g, read.b, read.a), (10, 20, 30, 0));
}

#[test]
fn display_canonical() {
    assert_eq!(Rgba::new(255, 136, 0, 255).to_string(), "#ff8800");
    assert_eq!(Rgba::new(255, 136, 0, 170).to_string(), "#ff8800aa");
    assert_eq!(Rgba::TRANSPARENT.to_string(), "#00000000");
}

#[test]
fn from_str_delegates_to_from_hex() {
    assert_eq!("#f80".parse::<Rgba>(), Ok(Rgba::new(255, 136, 0, 255)));
    assert_eq!(
        "#ggg".parse::<Rgba>(),
        Err(ParseHexError::InvalidDigit('g'))
    );
}

// Propiedades.

proptest! {
    #[test]
    fn round_trip(r: u8, g: u8, b: u8, a: u8) {
        let c = Rgba::new(r, g, b, a);
        prop_assert_eq!(Rgba::from_hex(&c.to_string()), Ok(c));
    }

    #[test]
    fn display_has_shape_canonical(r: u8, g: u8, b: u8, a: u8) {
        let s = Rgba::new(r, g, b, a).to_string();
        prop_assert!(s.starts_with('#'));
        prop_assert_eq!(&s, &s.to_lowercase());
        let n = s.chars().count();
        prop_assert!(n == 7 || n == 9);
    }

    #[test]
    fn arbitrary_strings_do_not_panic(s in any::<String>()) {
        let _ = Rgba::from_hex(&s);
        let _ = s.parse::<Rgba>();
    }

    #[test]
    fn unicode_strings_do_not_panic(s in "\\PC{0,12}") {
        let _ = Rgba::from_hex(&s);
        let _ = s.parse::<Rgba>();
    }
}
