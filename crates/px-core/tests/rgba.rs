//! Tests de `Rgba` y su notación hexadecimal (specs/hito-1-rgba.md).

use std::fmt::{Debug, Display};
use std::hash::Hash;

use proptest::prelude::*;
use px_core::{ParseHexError, Rgba};

// Comprobaciones en tiempo de compilación de los traits exigidos.
fn assert_rgba_traits<T: Copy + Clone + Debug + PartialEq + Eq + Hash>() {}
fn assert_error_traits<T: Debug + Clone + PartialEq + Eq + Display + std::error::Error>() {}

#[test]
fn traits_derivados() {
    assert_rgba_traits::<Rgba>();
    assert_error_traits::<ParseHexError>();
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn negro_con_almohadilla() {
    assert_eq!(Rgba::from_hex("#000000"), Ok(Rgba::new(0, 0, 0, 255)));
}

#[test]
fn negro_sin_almohadilla() {
    assert_eq!(Rgba::from_hex("000000"), Ok(Rgba::new(0, 0, 0, 255)));
}

#[test]
fn blanco_corto_mayusculas_y_minusculas() {
    let blanco = Rgba::new(255, 255, 255, 255);
    assert_eq!(Rgba::from_hex("#FFF"), Ok(blanco));
    assert_eq!(Rgba::from_hex("#fff"), Ok(blanco));
}

#[test]
fn corto_con_alfa() {
    assert_eq!(Rgba::from_hex("#f80a"), Ok(Rgba::new(255, 136, 0, 170)));
}

#[test]
fn alfa_cero_conserva_rgb() {
    assert_eq!(Rgba::from_hex("#ff880000"), Ok(Rgba::new(255, 136, 0, 0)));
}

#[test]
fn vacio_y_solo_almohadilla() {
    assert_eq!(Rgba::from_hex(""), Err(ParseHexError::Length(0)));
    assert_eq!(Rgba::from_hex("#"), Err(ParseHexError::Length(0)));
}

#[test]
fn longitud_2() {
    assert_eq!(Rgba::from_hex("#ff"), Err(ParseHexError::Length(2)));
}

#[test]
fn longitud_5() {
    assert_eq!(Rgba::from_hex("#fffff"), Err(ParseHexError::Length(5)));
}

#[test]
fn longitud_9() {
    assert_eq!(Rgba::from_hex("#fffffffff"), Err(ParseHexError::Length(9)));
}

#[test]
fn digito_invalido_g() {
    assert_eq!(
        Rgba::from_hex("#ggg"),
        Err(ParseHexError::InvalidDigit('g'))
    );
}

#[test]
fn doble_almohadilla() {
    // Se quita una sola `#`; quedan 4 caracteres, longitud válida.
    assert_eq!(
        Rgba::from_hex("##fff"),
        Err(ParseHexError::InvalidDigit('#'))
    );
}

#[test]
fn digito_unicode_cuenta_como_un_caracter() {
    // 3 caracteres (4 bytes): la longitud es válida y falla el dígito.
    assert_eq!(
        Rgba::from_hex("#ffé"),
        Err(ParseHexError::InvalidDigit('é'))
    );
}

#[test]
fn signo_mas_no_es_digito() {
    assert_eq!(
        Rgba::from_hex("+fff"),
        Err(ParseHexError::InvalidDigit('+'))
    );
}

// Valores básicos.

#[test]
fn default_es_transparente() {
    assert_eq!(Rgba::default(), Rgba::new(0, 0, 0, 0));
    assert_eq!(Rgba::default(), Rgba::TRANSPARENT);
}

#[test]
fn transparent_vale_cero() {
    let t = Rgba::TRANSPARENT;
    assert_eq!((t.r, t.g, t.b, t.a), (0, 0, 0, 0));
}

#[test]
fn opaque_pone_alfa_255() {
    let c = Rgba::opaque(1, 2, 3);
    assert_eq!((c.r, c.g, c.b, c.a), (1, 2, 3, 255));
    assert_eq!(c, Rgba::new(1, 2, 3, 255));
}

#[test]
fn alfa_directo_ida_y_vuelta() {
    let c = Rgba::new(10, 20, 30, 0);
    let leido = Rgba::from_hex(&c.to_string()).unwrap();
    assert_eq!((leido.r, leido.g, leido.b, leido.a), (10, 20, 30, 0));
}

#[test]
fn display_canonico() {
    assert_eq!(Rgba::new(255, 136, 0, 255).to_string(), "#ff8800");
    assert_eq!(Rgba::new(255, 136, 0, 170).to_string(), "#ff8800aa");
    assert_eq!(Rgba::TRANSPARENT.to_string(), "#00000000");
}

#[test]
fn from_str_delega_en_from_hex() {
    assert_eq!("#f80".parse::<Rgba>(), Ok(Rgba::new(255, 136, 0, 255)));
    assert_eq!(
        "#ggg".parse::<Rgba>(),
        Err(ParseHexError::InvalidDigit('g'))
    );
}

// Propiedades.

proptest! {
    #[test]
    fn ida_y_vuelta(r: u8, g: u8, b: u8, a: u8) {
        let c = Rgba::new(r, g, b, a);
        prop_assert_eq!(Rgba::from_hex(&c.to_string()), Ok(c));
    }

    #[test]
    fn display_tiene_forma_canonica(r: u8, g: u8, b: u8, a: u8) {
        let s = Rgba::new(r, g, b, a).to_string();
        prop_assert!(s.starts_with('#'));
        prop_assert_eq!(&s, &s.to_lowercase());
        let n = s.chars().count();
        prop_assert!(n == 7 || n == 9);
    }

    #[test]
    fn cadenas_arbitrarias_no_hacen_panic(s in any::<String>()) {
        let _ = Rgba::from_hex(&s);
        let _ = s.parse::<Rgba>();
    }

    #[test]
    fn cadenas_unicode_no_hacen_panic(s in "\\PC{0,12}") {
        let _ = Rgba::from_hex(&s);
        let _ = s.parse::<Rgba>();
    }
}
