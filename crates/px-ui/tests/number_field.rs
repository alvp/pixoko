//! Tests del campo numérico `px_ui::number` (specs/hito-2-dialogos-modales.md, unidad 18, decisión
//! 4, «Casos» filas 1 y 2 y propiedad 1). Todo puro: ni egui ni ventana. **En rojo** hasta que
//! exista el módulo.
//!
//! - Fila 1: `parse_number` con todas las entradas de la tabla (incluidas las cadenas de un millón
//!   de caracteres) y los textos de `number_error_text`.
//! - Fila 2: `step_number`, `step_enabled` y `bounded_text`.
//! - Propiedad 1: el analizador es total y exacto, contra un oráculo escrito aparte, y las
//!   invariantes de `step_number`, `step_enabled` y `bounded_text`.
//!
//! Decisiones donde la spec no fija un detalle:
//! - «No recorrer ni reservar más de 11 caracteres» se comprueba de forma indirecta: la
//!   capacidad de `bounded_text` de un millón de caracteres no pasa de 4096 bytes (una
//!   cota holgada que un `String` de un millón no cumpliría).
//! - `step_number` y `step_enabled` solo se prueban con `delta` ±1 (la spec no dice qué hacen
//!   otros pasos).
//! - Con `step_enabled` falso la propiedad exige que el **valor** no cambie, y que el texto sea
//!   idéntico solo si ya era canónico (sin ceros a la izquierda): la spec pide que `+` no deje
//!   ceros a la izquierda y no dice qué pasa con `007` en el límite (`007` o `7`).
//! - Los textos de `number_error_text` miden como mucho 23 caracteres (el motivo más largo de la
//!   decisión 8) y están en `INTERFACE_CHARSET`.
//! - En la propiedad, `default` está siempre dentro del rango (`min ≤ default ≤ max`).

use proptest::prelude::*;
use px_ui::number::{
    MAX_NUMBER_CHARS, NumberError, bounded_text, number_error_text, parse_number, step_enabled,
    step_number,
};
use px_ui::text::INTERFACE_CHARSET;

const MIN: u32 = 1;
const MAX: u32 = 8192;

fn p(text: &str) -> Result<u32, NumberError> {
    parse_number(text, MIN, MAX)
}

// ---------------------------------------------------------------------------------------------
// Fila 1: parse_number
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_1_la_constante_es_diez() {
    assert_eq!(MAX_NUMBER_CHARS, 10);
}

#[test]
fn fila_1_vacio_es_empty() {
    assert_eq!(p(""), Err(NumberError::Empty));
    assert_eq!(parse_number("", 0, u32::MAX), Err(NumberError::Empty));
}

#[test]
fn fila_1_espacios_no_se_recortan() {
    for texto in [" ", " 5", "5 ", "1 2", "\t5", "5\n", "  "] {
        assert_eq!(p(texto), Err(NumberError::NotDigit), "{texto:?}");
    }
}

#[test]
fn fila_1_los_limites_del_rango() {
    assert_eq!(p("0"), Err(NumberError::Below { min: 1 }));
    assert_eq!(p("1"), Ok(1));
    assert_eq!(p("8192"), Ok(8192));
    assert_eq!(p("8193"), Err(NumberError::Above { max: 8192 }));
}

#[test]
fn fila_1_los_ceros_a_la_izquierda_valen_pero_cuentan_para_las_diez() {
    assert_eq!(p("007"), Ok(7));
    assert_eq!(p("0000000064"), Ok(64));
    assert_eq!(p("00000000064"), Err(NumberError::TooLong));
    assert_eq!(p("0000000000"), Err(NumberError::Below { min: 1 }));
    assert_eq!(parse_number("0000000000", 0, 5), Ok(0));
}

#[test]
fn fila_1_signos_decimales_y_otros_alfabetos_son_not_digit() {
    for texto in [
        "-5", "+5", "5.0", "5,0", "1e3", "0x10", "５", "٣", "5\u{301}", "🙂",
    ] {
        assert_eq!(p(texto), Err(NumberError::NotDigit), "{texto:?}");
    }
}

#[test]
fn fila_1_diez_cifras_caben_sin_desbordar() {
    // 4294967295 con max 8192 y con 0..=u32::MAX.
    assert_eq!(p("4294967295"), Err(NumberError::Above { max: 8192 }));
    assert_eq!(parse_number("4294967295", 0, u32::MAX), Ok(u32::MAX));
    // 4294967296 no cabe en u32: `Above`, sin desbordar.
    assert_eq!(
        parse_number("4294967296", 0, u32::MAX),
        Err(NumberError::Above { max: u32::MAX })
    );
    // Las diez cifras más grandes caben en u64.
    assert_eq!(p("9999999999"), Err(NumberError::Above { max: 8192 }));
    assert_eq!(
        parse_number("9999999999", 0, u32::MAX),
        Err(NumberError::Above { max: u32::MAX })
    );
}

#[test]
fn fila_1_once_caracteres_son_too_long_salvo_que_haya_un_no_digito() {
    assert_eq!(p("99999999999"), Err(NumberError::TooLong));
    assert_eq!(
        parse_number("99999999999", 0, u32::MAX),
        Err(NumberError::TooLong)
    );
    // NotDigit gana a TooLong cuando el carácter malo está entre los 11 primeros.
    assert_eq!(p("1234567890x"), Err(NumberError::NotDigit));
    assert_eq!(p("x1234567890"), Err(NumberError::NotDigit));
    // Pero uno más allá del 11 no se mira.
    assert_eq!(p("12345678901x"), Err(NumberError::TooLong));
}

#[test]
fn fila_1_un_millon_de_nueves_es_too_long() {
    let texto = "9".repeat(1_000_000);
    assert_eq!(p(&texto), Err(NumberError::TooLong));
    assert_eq!(parse_number(&texto, 0, u32::MAX), Err(NumberError::TooLong));
}

#[test]
fn fila_1_un_millon_de_espacios_es_not_digit() {
    let texto = " ".repeat(1_000_000);
    assert_eq!(p(&texto), Err(NumberError::NotDigit));
}

#[test]
fn fila_1_un_millon_de_caracteres_multibyte_no_entra_en_panico() {
    let texto = "é".repeat(1_000_000);
    assert_eq!(p(&texto), Err(NumberError::NotDigit));
    let texto = "🙂".repeat(1_000_000);
    assert_eq!(p(&texto), Err(NumberError::NotDigit));
}

#[test]
fn fila_1_el_error_por_orden_empty_notdigit_toolong_below_above() {
    // Dentro de los 11 primeros: NotDigit antes que TooLong, TooLong antes que el rango.
    assert_eq!(parse_number("0", 5, 9), Err(NumberError::Below { min: 5 }));
    assert_eq!(parse_number("10", 5, 9), Err(NumberError::Above { max: 9 }));
    assert_eq!(parse_number("5", 5, 9), Ok(5));
    assert_eq!(parse_number("9", 5, 9), Ok(9));
    assert_eq!(parse_number("5", 5, 5), Ok(5));
    assert_eq!(parse_number("00000000000", 5, 9), Err(NumberError::TooLong));
}

#[test]
fn fila_1_los_textos_de_los_errores() {
    assert_eq!(number_error_text(&NumberError::Empty), "escribe un número");
    assert_eq!(
        number_error_text(&NumberError::NotDigit),
        "solo dígitos 0-9"
    );
    assert_eq!(number_error_text(&NumberError::TooLong), "máximo 10 cifras");
    assert_eq!(
        number_error_text(&NumberError::Below { min: 1 }),
        "mínimo 1"
    );
    assert_eq!(
        number_error_text(&NumberError::Above { max: 8192 }),
        "máximo 8192"
    );
    assert_eq!(
        number_error_text(&NumberError::Below { min: 0 }),
        "mínimo 0"
    );
    assert_eq!(
        number_error_text(&NumberError::Above { max: u32::MAX }),
        "máximo 4294967295"
    );
}

#[test]
fn fila_1_los_textos_caben_en_el_juego_de_caracteres_y_en_23() {
    let errores = [
        NumberError::Empty,
        NumberError::NotDigit,
        NumberError::TooLong,
        NumberError::Below { min: 0 },
        NumberError::Below { min: u32::MAX },
        NumberError::Above { max: 0 },
        NumberError::Above { max: u32::MAX },
    ];
    for error in &errores {
        let texto = number_error_text(error);
        assert!(texto.chars().count() <= 23, "{error:?}: {texto:?}");
        assert!(
            texto.chars().all(|c| INTERFACE_CHARSET.contains(c)),
            "{error:?}: {texto:?}"
        );
    }
}

#[test]
fn fila_1_number_error_es_clone_debug_y_eq() {
    let e = NumberError::Below { min: 3 };
    let copia = e.clone();
    assert_eq!(e, copia);
    assert_ne!(e, NumberError::Below { min: 4 });
    assert_ne!(NumberError::Below { min: 3 }, NumberError::Above { max: 3 });
    assert!(!format!("{e:?}").is_empty());
}

// ---------------------------------------------------------------------------------------------
// Fila 2: step_number, step_enabled y bounded_text
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_2_suma_y_resta_uno_sin_ceros_a_la_izquierda() {
    assert_eq!(step_number("5", 1, 8192, 64, 1), "6");
    assert_eq!(step_number("5", 1, 8192, 64, -1), "4");
    assert_eq!(step_number("007", 1, 8192, 64, 1), "8");
    assert_eq!(step_number("007", 1, 8192, 64, -1), "6");
    assert_eq!(step_number("1", 1, 8192, 64, 1), "2");
    assert_eq!(step_number("8192", 1, 8192, 64, -1), "8191");
}

#[test]
fn fila_2_en_el_limite_no_se_sale_del_rango() {
    assert_eq!(step_number("1", 1, 8192, 64, -1), "1");
    assert_eq!(step_number("8192", 1, 8192, 64, 1), "8192");
    assert_eq!(step_number("0", 0, 8192, 64, -1), "0");
    assert_eq!(step_number("4294967295", 0, u32::MAX, 64, 1), "4294967295");
    assert_eq!(step_number("4294967295", 0, u32::MAX, 64, -1), "4294967294");
    assert_eq!(step_number("0", 0, u32::MAX, 64, 1), "1");
}

#[test]
fn fila_2_con_texto_invalido_o_fuera_de_rango_pone_el_valor_por_defecto() {
    let invalidos = [
        "",
        "x",
        "9000",
        "0",
        "99999999999",
        " 5",
        "-1",
        "12345678901234",
    ];
    for texto in invalidos {
        for delta in [1i8, -1] {
            assert_eq!(
                step_number(texto, 1, 8192, 64, delta),
                "64",
                "{texto:?} delta {delta}"
            );
        }
    }
    // Un millón de caracteres: sin pánico y el valor por defecto.
    let largo = "9".repeat(1_000_000);
    assert_eq!(step_number(&largo, 1, 8192, 7, 1), "7");
    assert_eq!(step_number(&largo, 1, 8192, 7, -1), "7");
}

#[test]
fn fila_2_con_texto_invalido_los_dos_botones_estan_habilitados() {
    for texto in ["", "x", "9000", "99999999999", "0", " 5"] {
        for delta in [1i8, -1] {
            assert!(
                step_enabled(texto, 1, 8192, delta),
                "{texto:?} delta {delta}"
            );
        }
    }
}

#[test]
fn fila_2_deshabilitado_exactamente_donde_el_valor_valido_ya_esta_en_el_limite() {
    // `−` en el mínimo.
    assert!(!step_enabled("1", 1, 8192, -1));
    assert!(step_enabled("1", 1, 8192, 1));
    // `+` en el máximo.
    assert!(!step_enabled("8192", 1, 8192, 1));
    assert!(step_enabled("8192", 1, 8192, -1));
    // Con min 0.
    assert!(!step_enabled("0", 0, 8192, -1));
    assert!(step_enabled("0", 0, 8192, 1));
    // Con max u32::MAX.
    assert!(!step_enabled("4294967295", 0, u32::MAX, 1));
    assert!(step_enabled("4294967295", 0, u32::MAX, -1));
    // Por dentro, los dos.
    for texto in ["5", "007", "8191", "2"] {
        assert!(step_enabled(texto, 1, 8192, 1), "{texto}");
        assert!(step_enabled(texto, 1, 8192, -1), "{texto}");
    }
    // Un rango de un solo valor: ninguno.
    assert!(!step_enabled("5", 5, 5, 1));
    assert!(!step_enabled("5", 5, 5, -1));
    // Ceros a la izquierda en el límite.
    assert!(!step_enabled("0001", 1, 8192, -1));
}

#[test]
fn fila_2_bounded_text_acota_a_once_caracteres() {
    assert_eq!(bounded_text(""), "");
    assert_eq!(bounded_text("12345678901"), "12345678901");
    assert_eq!(bounded_text("123456789012"), "12345678901");
    assert_eq!(bounded_text("abc"), "abc");
    let largo = bounded_text(&"9".repeat(1_000_000));
    assert_eq!(largo, "99999999999");
    assert!(largo.capacity() <= 4096, "capacidad {}", largo.capacity());
}

#[test]
fn fila_2_bounded_text_corta_en_frontera_de_caracter() {
    let texto = "é".repeat(13);
    let acotado = bounded_text(&texto);
    assert_eq!(acotado.chars().count(), 11);
    assert_eq!(acotado, "é".repeat(11));
    assert!(texto.starts_with(&acotado));
    // Otros anchos de carácter, sin pánico.
    for c in ['日', '🙂', 'a', '\u{301}'] {
        let t: String = std::iter::repeat_n(c, 40).collect();
        let a = bounded_text(&t);
        assert_eq!(a.chars().count(), 11, "{c:?}");
    }
    let largo = bounded_text(&"🙂".repeat(1_000_000));
    assert_eq!(largo.chars().count(), 11);
    assert!(largo.capacity() <= 4096, "capacidad {}", largo.capacity());
}

// ---------------------------------------------------------------------------------------------
// Propiedad 1
// ---------------------------------------------------------------------------------------------

/// El oráculo, escrito aparte: los 11 primeros caracteres, en `u64`.
fn oraculo(text: &str, min: u32, max: u32) -> Result<u32, NumberError> {
    let primeros: Vec<char> = text.chars().take(11).collect();
    if primeros.is_empty() {
        return Err(NumberError::Empty);
    }
    if primeros.iter().any(|c| !c.is_ascii_digit()) {
        return Err(NumberError::NotDigit);
    }
    if primeros.len() == 11 {
        return Err(NumberError::TooLong);
    }
    let valor = primeros
        .iter()
        .fold(0u64, |acc, c| acc * 10 + (u64::from(*c) - u64::from('0')));
    if valor < u64::from(min) {
        Err(NumberError::Below { min })
    } else if valor > u64::from(max) {
        Err(NumberError::Above { max })
    } else {
        Ok(valor as u32)
    }
}

/// Textos: Unicode de hasta 64 caracteres, cadenas de dígitos de hasta 13 (donde se decide
/// todo) y números bien formados.
fn textos_cortos() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => prop::collection::vec(any::<char>(), 0..=64).prop_map(|v| v.into_iter().collect::<String>()),
        4 => "[0-9]{0,13}",
        2 => "[0-9]{0,11}[ x+\\-.é٣５]?[0-9]{0,3}",
        2 => (0u64..20_000).prop_map(|n| n.to_string()),
        1 => (0u64..=u64::from(u32::MAX) + 2).prop_map(|n| n.to_string()),
    ]
}

/// Repeticiones de un carácter de hasta 10⁶ veces.
fn textos_largos() -> impl Strategy<Value = String> {
    (
        prop::sample::select(vec!['9', '0', '1', ' ', 'é', '🙂', 'x']),
        0usize..=1_000_000,
    )
        .prop_map(|(c, n)| std::iter::repeat_n(c, n).collect())
}

/// `min ≤ default ≤ max`.
fn rangos() -> impl Strategy<Value = (u32, u32, u32)> {
    let triple = |a: u32, b: u32, d: u32| {
        let (min, max) = (a.min(b), a.max(b));
        let default = min + (u64::from(d) % (u64::from(max - min) + 1)) as u32;
        (min, max, default)
    };
    prop_oneof![
        3 => (any::<u32>(), any::<u32>(), any::<u32>()).prop_map(move |(a, b, d)| triple(a, b, d)),
        4 => (0u32..10_000, 0u32..10_000, any::<u32>()).prop_map(move |(a, b, d)| triple(a, b, d)),
        1 => Just((1, 8192, 64)),
        1 => Just((0, u32::MAX, 64)),
    ]
}

fn comprueba(text: &str, min: u32, max: u32, default: u32) -> Result<(), TestCaseError> {
    // Total y exacto.
    let r = parse_number(text, min, max);
    prop_assert_eq!(&r, &oraculo(text, min, max));

    // `bounded_text`: como mucho 11 caracteres y es un prefijo.
    let acotado = bounded_text(text);
    prop_assert!(acotado.chars().count() <= 11);
    prop_assert!(text.starts_with(&acotado));
    prop_assert_eq!(acotado.chars().count(), text.chars().count().min(11));

    // `Ok(v)`: de 1 a 10 dígitos ASCII, valor exacto y en rango.
    if let Ok(v) = r {
        prop_assert!((1..=10).contains(&text.chars().count()));
        prop_assert!(text.chars().all(|c| c.is_ascii_digit()));
        prop_assert_eq!(u64::from(v), text.parse::<u64>().unwrap());
        prop_assert!(min <= v && v <= max);
    }

    // `number_error_text` total.
    if let Err(e) = &r {
        prop_assert!(!number_error_text(e).is_empty());
    }

    for delta in [1i8, -1] {
        let siguiente = step_number(text, min, max, default, delta);
        let habilitado = step_enabled(text, min, max, delta);
        // Siempre válido y en rango.
        let nuevo = parse_number(&siguiente, min, max);
        prop_assert!(nuevo.is_ok(), "{:?} -> {:?}", text, siguiente);
        let nuevo = nuevo.unwrap();
        // Sin ceros a la izquierda.
        prop_assert_eq!(siguiente.clone(), nuevo.to_string());
        match r {
            Ok(v) => {
                if delta > 0 {
                    prop_assert!(nuevo >= v);
                } else {
                    prop_assert!(nuevo <= v);
                }
                if habilitado {
                    prop_assert_eq!(i64::from(nuevo), i64::from(v) + i64::from(delta));
                } else {
                    prop_assert_eq!(nuevo, v);
                    if text == v.to_string() {
                        prop_assert_eq!(siguiente.as_str(), text);
                    }
                }
                // Deshabilitado exactamente en el límite que toca.
                let en_limite = if delta > 0 { v == max } else { v == min };
                prop_assert_eq!(habilitado, !en_limite);
            }
            Err(_) => {
                // Texto inválido o fuera de rango: el valor por defecto y botones habilitados.
                prop_assert_eq!(nuevo, default);
                prop_assert!(habilitado);
            }
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn propiedad_1_el_analizador_es_total_y_exacto(
        text in textos_cortos(),
        (min, max, default) in rangos(),
    ) {
        comprueba(&text, min, max, default)?;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn propiedad_1_con_repeticiones_de_hasta_un_millon_de_caracteres(
        text in textos_largos(),
        (min, max, default) in rangos(),
    ) {
        comprueba(&text, min, max, default)?;
    }
}
