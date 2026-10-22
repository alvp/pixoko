//! Chunks auxiliares del PNG en el informe de lectura (specs/hito-2-chunks-auxiliares.md,
//! unidad 13b): filas 1 a 7 (la parte de `read_png`) y propiedades 1 a 3. En ROJO hasta la
//! implementación de `AuxChunk`, `AuxReport` y `PngRead.aux`.
//!
//! Todos los PNG se construyen a mano con los ayudantes del fichero padre (`piece`, `Png`,
//! `zlib_stored`); el corpus de `golden/png` solo se lee. Ningún caso reserva más de unos pocos KiB.
//!
//! **Suposiciones y ambigüedades** (ver el informe de la unidad):
//! - D2 dice «cICP > iCCP > sRGB > gAMA/cHRM» en su cabecera y numera `iCCP` el primero con
//!   «siempre». El modelo de la propiedad 3 sigue la regla numerada: `iCCP` presente ⇒ `Icc` aunque
//!   haya `cICP`.
//! - La propiedad 3 genera cada tipo como mucho una vez (los repetidos van en la fila 3, con
//!   longitudes buenas) y `acTL` solo con sus 8 bytes: la spec no dice qué pasa con un `acTL`,
//!   `pHYs` o `tIME` de tamaño imposible.
//! - **El crate `png` rechaza un `cICP` de 4 bytes con matriz distinta de 0 o con rango distinto
//!   de 0 y 1** (`Decode("invalid data")`, error que no es benigno, antes de que se clasifique
//!   nada). Por eso la fila 4 usa matriz 0 donde la spec escribe (9,16,9,1) y (1,13,5,0), y las
//!   propiedades generan siempre matriz 0 y rango 0 o 1. Ver el informe de la unidad.

use super::*;

use std::fs;
use std::path::Path;

use px_formats::AuxReport;

// ---------------------------------------------------------------------------------------
// Ayudantes: chunks auxiliares con CRC válido, inserción en un fichero y comparación.
// ---------------------------------------------------------------------------------------

/// RGBA de 1×1 sin ningún auxiliar.
fn base() -> Png {
    Png::new(1, 1, 8, 6).swatches(&[10, 20, 30, 40])
}

/// `base()` con esos chunks entre IHDR e IDAT y después del IDAT.
fn with(antes: &[Vec<u8>], after: &[Vec<u8>]) -> Vec<u8> {
    let mut png = base();
    for c in antes {
        png = png.antes(c.clone());
    }
    for c in after {
        png = png.after(c.clone());
    }
    png.bytes()
}

/// Inserta chunks en un PNG de `write_png` (un solo IDAT): `antes` justo tras el IHDR (que
/// acaba en el byte 33) y `after` justo antes del IEND (los últimos 12 bytes).
fn inserts(bytes: &[u8], antes: &[Vec<u8>], after: &[Vec<u8>]) -> Vec<u8> {
    let iend = bytes.len() - 12;
    let mut v = bytes[..33].to_vec();
    for c in antes {
        v.extend(c);
    }
    v.extend(&bytes[33..iend]);
    for c in after {
        v.extend(c);
    }
    v.extend(&bytes[iend..]);
    v
}

fn aux_de(bytes: &[u8]) -> AuxReport {
    lee(bytes).aux
}

fn report(found: &[AuxChunk], appearance: Option<AuxChunk>) -> AuxReport {
    AuxReport {
        found: found.to_vec(),
        appearance,
    }
}

fn c_iccp() -> Vec<u8> {
    let mut d = b"perfil\0".to_vec();
    d.push(0);
    d.extend(zlib_stored(b"datos del perfil", 0));
    piece(b"iCCP", &d)
}

fn c_cicp(primary: u8, transfer: u8, matrix: u8, range: u8) -> Vec<u8> {
    piece(b"cICP", &[primary, transfer, matrix, range])
}

fn c_srgb() -> Vec<u8> {
    piece(b"sRGB", &[0])
}

fn c_gamma(valor: u32) -> Vec<u8> {
    piece(b"gAMA", &valor.to_be_bytes())
}

/// Blanco D65 y primarios de sRGB, por este orden: blanco x e y, rojo, verde y azul.
const CHRM_SRGB: [u32; 8] = [31270, 32900, 64000, 33000, 30000, 60000, 15000, 6000];
/// Blanco D65 y primarios de Adobe RGB (1998): solo cambia el verde.
const CHRM_ADOBE: [u32; 8] = [31270, 32900, 64000, 33000, 21000, 71000, 15000, 6000];

fn c_chrm(values: [u32; 8]) -> Vec<u8> {
    let data: Vec<u8> = values.iter().flat_map(|v| v.to_be_bytes()).collect();
    piece(b"cHRM", &data)
}

fn c_phys() -> Vec<u8> {
    let mut d = Vec::new();
    d.extend(2835u32.to_be_bytes());
    d.extend(2835u32.to_be_bytes());
    d.push(1);
    piece(b"pHYs", &d)
}

fn c_text() -> Vec<u8> {
    piece(b"tEXt", b"Title\0pixoko")
}

fn c_ztxt() -> Vec<u8> {
    let mut d = b"Title\0\0".to_vec();
    d.extend(zlib_stored(b"pixoko", 0));
    piece(b"zTXt", &d)
}

fn c_itxt() -> Vec<u8> {
    // Palabra clave, indicador y método de compresión, etiqueta de idioma y palabra traducida
    // vacías, y el texto.
    piece(b"iTXt", b"Title\0\0\0\0\0pixoko")
}

fn c_time() -> Vec<u8> {
    piece(b"tIME", &[0x07, 0xEA, 10, 17, 12, 0, 0])
}

fn c_exif() -> Vec<u8> {
    piece(b"eXIf", b"MM\0*\0\0\0\x08")
}

fn c_actl() -> Vec<u8> {
    piece(b"acTL", &[0, 0, 0, 2, 0, 0, 0, 0])
}

fn c_unknown() -> Vec<u8> {
    piece(b"teSt", &[1, 2, 3])
}

/// El mismo chunk con el último byte del CRC cambiado.
fn crc_bad(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut v = piece(kind, data);
    let n = v.len();
    v[n - 1] ^= 0xFF;
    v
}

/// Cabecera de un chunk que declara 2³¹ − 1 bytes de datos y solo trae tres.
fn giant(kind: &[u8; 4]) -> Vec<u8> {
    let mut v = (u32::MAX >> 1).to_be_bytes().to_vec();
    v.extend(kind);
    v.extend([1, 2, 3]);
    v
}

// ---------------------------------------------------------------------------------------
// Fila 1: sin auxiliares y corpus.
// ---------------------------------------------------------------------------------------

#[test]
fn row_1_without_auxiliaries_the_report_is_empty() {
    let rgba = lee(&base().bytes());
    assert_eq!(rgba.aux, AuxReport::default());
    assert!(rgba.aux.found.is_empty());
    assert_eq!(rgba.aux.appearance, None);

    let indexed = Png::new(2, 1, 8, 3)
        .swatches(&[0, 1])
        .antes(plte(&[(0, 0, 0), (255, 0, 0)]))
        .bytes();
    let r = lee(&indexed);
    assert_eq!(r.aux, AuxReport::default());
    assert!(r.aux.found.is_empty());
    assert_eq!(r.aux.appearance, None);
}

#[test]
fn row_1_png_golden_corpus() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/png");
    let (mut grafx2, mut libresprite) = (0, 0);
    for input in fs::read_dir(&dir).expect("leer golden/png") {
        let path = input.expect("entrada").path();
        if path.extension().is_none_or(|e| e != "png") {
            continue;
        }
        let name_2 = path.file_name().unwrap().to_string_lossy().into_owned();
        let r = lee(&fs::read(&path).unwrap());
        if name_2.starts_with("grafx2") {
            grafx2 += 1;
            assert_eq!(
                r.aux,
                report(&[AuxChunk::Text, AuxChunk::Other], None),
                "{name_2}"
            );
        } else if name_2.starts_with("libresprite") {
            libresprite += 1;
            assert_eq!(r.aux, AuxReport::default(), "{name_2}");
        } else {
            panic!("fichero inesperado en el corpus: {name_2}");
        }
    }
    assert_eq!((grafx2, libresprite), (2, 2));
}

// ---------------------------------------------------------------------------------------
// Fila 2: cada tipo válido por separado.
// ---------------------------------------------------------------------------------------

#[test]
fn row_2_each_valid_type_separately() {
    use AuxChunk::*;
    let cases: Vec<(&str, Vec<u8>, AuxChunk, Option<AuxChunk>)> = vec![
        ("iCCP", c_iccp(), Icc, Some(Icc)),
        ("cICP (1,13,0,1)", c_cicp(1, 13, 0, 1), CodePoints, None),
        ("sRGB", c_srgb(), Srgb, None),
        ("gAMA 45455", c_gamma(45455), Gamma, None),
        ("cHRM de sRGB", c_chrm(CHRM_SRGB), Chromaticities, None),
        ("pHYs", c_phys(), Physical, None),
        ("tEXt", c_text(), Text, None),
        ("zTXt", c_ztxt(), Text, None),
        ("iTXt", c_itxt(), Text, None),
        ("tIME", c_time(), Time, None),
        ("eXIf", c_exif(), Exif, None),
        ("acTL", c_actl(), Animation, Some(Animation)),
        ("teSt", c_unknown(), Other, None),
    ];
    for (name_2, chunk, kind, appearance) in cases {
        let r = lee(&with(&[chunk], &[]));
        assert_eq!(r.aux, report(&[kind], appearance), "{name_2}");
        // Lo que se ignora no cambia ni la imagen ni el informe de conversiones.
        assert!(r.report.is_empty(), "{name_2}");
        assert_eq!(pixels(&r.image), [rgba(10, 20, 30, 40)], "{name_2}");
    }
}

// ---------------------------------------------------------------------------------------
// Fila 3: repetidos, orden, posición y sin tope.
// ---------------------------------------------------------------------------------------

#[test]
fn row_3_repeated_without_duplicates_and_in_declaration_order_before_or_after_idat() {
    let sequence = || vec![c_text(), c_iccp(), c_text(), c_gamma(45455), c_iccp()];
    let expected = report(
        &[AuxChunk::Icc, AuxChunk::Gamma, AuxChunk::Text],
        Some(AuxChunk::Icc),
    );
    assert_eq!(aux_de(&with(&sequence(), &[])), expected, "antes del IDAT");
    assert_eq!(
        aux_de(&with(&[], &sequence())),
        expected,
        "después del IDAT"
    );
}

#[test]
fn row_3_two_thousand_text_do_not_hide_an_iccp_at_the_end() {
    let mut chunks: Vec<Vec<u8>> = (0..2000)
        .map(|i| piece(b"tEXt", format!("clave{i}\0valor").as_bytes()))
        .collect();
    chunks.push(c_iccp());
    let r = lee(&with(&chunks, &[]));
    assert_eq!(
        r.aux,
        report(&[AuxChunk::Icc, AuxChunk::Text], Some(AuxChunk::Icc))
    );
    assert!(r.report.is_empty());
}

#[test]
fn row_3_of_a_repeated_type_the_first_one_counts() {
    // D3: «repetidos ⇒ vale el primero y el tipo se lista una vez».
    let gamma_without_cause_first = aux_de(&with(&[c_gamma(45455), c_gamma(100_000)], &[]));
    assert_eq!(gamma_without_cause_first, report(&[AuxChunk::Gamma], None));
    let gamma_with_cause_first = aux_de(&with(&[c_gamma(100_000), c_gamma(45455)], &[]));
    assert_eq!(
        gamma_with_cause_first,
        report(&[AuxChunk::Gamma], Some(AuxChunk::Gamma))
    );
}

// ---------------------------------------------------------------------------------------
// Fila 4: qué activa la copia (`appearance`).
// ---------------------------------------------------------------------------------------

#[test]
fn row_4_causes_per_the_d2_rule() {
    use AuxChunk::*;
    type Case = (&'static str, Vec<Vec<u8>>, Option<AuxChunk>);
    let cases: Vec<Case> = vec![
        ("iCCP", vec![c_iccp()], Some(Icc)),
        ("iCCP + sRGB", vec![c_iccp(), c_srgb()], Some(Icc)),
        ("sRGB + iCCP", vec![c_srgb(), c_iccp()], Some(Icc)),
        (
            "cICP (9,16,0,1)",
            vec![c_cicp(9, 16, 0, 1)],
            Some(CodePoints),
        ),
        (
            "cICP (1,16,0,1)",
            vec![c_cicp(1, 16, 0, 1)],
            Some(CodePoints),
        ),
        (
            "cICP (9,13,0,1)",
            vec![c_cicp(9, 13, 0, 1)],
            Some(CodePoints),
        ),
        (
            "cICP no sRGB + sRGB: manda cICP",
            vec![c_cicp(9, 16, 0, 1), c_srgb()],
            Some(CodePoints),
        ),
        (
            "cICP (1,13,0,0): el rango no cuenta",
            vec![c_cicp(1, 13, 0, 0)],
            None,
        ),
        (
            "cICP (1,13,0,0) + gAMA 100000: cICP sRGB acaba la búsqueda",
            vec![c_cicp(1, 13, 0, 0), c_gamma(100_000)],
            None,
        ),
        (
            "cICP (1,13,0,1) + cHRM Adobe",
            vec![c_cicp(1, 13, 0, 1), c_chrm(CHRM_ADOBE)],
            None,
        ),
        (
            "sRGB + gAMA 100000 + cHRM Adobe: sRGB acaba la búsqueda",
            vec![c_srgb(), c_gamma(100_000), c_chrm(CHRM_ADOBE)],
            None,
        ),
        ("gAMA 100000", vec![c_gamma(100_000)], Some(Gamma)),
        ("gAMA 45455", vec![c_gamma(45455)], None),
        ("gAMA 44455 (−1000)", vec![c_gamma(44455)], None),
        ("gAMA 46455 (+1000)", vec![c_gamma(46455)], None),
        ("gAMA 46456 (+1001)", vec![c_gamma(46456)], Some(Gamma)),
        ("gAMA 44454 (−1001)", vec![c_gamma(44454)], Some(Gamma)),
        (
            "cHRM Adobe RGB",
            vec![c_chrm(CHRM_ADOBE)],
            Some(Chromaticities),
        ),
        ("cHRM de sRGB", vec![c_chrm(CHRM_SRGB)], None),
        (
            "gAMA lineal + cHRM Adobe: manda Gamma",
            vec![c_gamma(100_000), c_chrm(CHRM_ADOBE)],
            Some(Gamma),
        ),
        (
            "gAMA de sRGB + cHRM Adobe",
            vec![c_gamma(45455), c_chrm(CHRM_ADOBE)],
            Some(Chromaticities),
        ),
        (
            "gAMA lineal + cHRM de sRGB",
            vec![c_gamma(100_000), c_chrm(CHRM_SRGB)],
            Some(Gamma),
        ),
    ];
    for (name_2, chunks, expected) in cases {
        let r = aux_de(&with(&chunks, &[]));
        assert_eq!(r.appearance, expected, "{name_2}: {r:?}");
        if let Some(a) = expected {
            assert!(r.found.contains(&a), "{name_2}: {r:?}");
        }
    }
}

#[test]
fn row_4_chrm_of_srgb_plus_minus_1000_does_not_trigger_and_plus_minus_1001_does() {
    for i in 0..8 {
        for (delta, expected) in [
            (1000i64, None),
            (-1000, None),
            (1001, Some(AuxChunk::Chromaticities)),
            (-1001, Some(AuxChunk::Chromaticities)),
        ] {
            let mut v = CHRM_SRGB;
            v[i] = u32::try_from(i64::from(v[i]) + delta).unwrap();
            let r = aux_de(&with(&[c_chrm(v)], &[]));
            assert_eq!(r.appearance, expected, "valor {i}, delta {delta}");
            assert_eq!(
                r.found,
                [AuxChunk::Chromaticities],
                "valor {i}, delta {delta}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 5: longitud imposible.
// ---------------------------------------------------------------------------------------

#[test]
fn row_5_impossible_length_counts_as_other_and_triggers_nothing() {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("gAMA de 3 bytes", piece(b"gAMA", &[0, 1, 2])),
        ("cHRM de 31 bytes", piece(b"cHRM", &[7; 31])),
        ("sRGB de 2 bytes", piece(b"sRGB", &[0, 0])),
        ("cICP de 5 bytes", piece(b"cICP", &[9, 16, 9, 1, 0])),
    ];
    for (name_2, chunk) in cases {
        let r = lee(&with(&[chunk], &[]));
        assert_eq!(r.aux, report(&[AuxChunk::Other], None), "{name_2}");
        assert!(r.report.is_empty(), "{name_2}");
    }
}

#[test]
fn row_5_empty_iccp_triggers_the_copy() {
    let r = lee(&with(&[piece(b"iCCP", &[])], &[]));
    assert_eq!(r.aux, report(&[AuxChunk::Icc], Some(AuxChunk::Icc)));
}

// ---------------------------------------------------------------------------------------
// Fila 6: basura.
// ---------------------------------------------------------------------------------------

#[test]
fn row_6_wrong_crc_is_listed_as_if_it_were_good() {
    let cases: Vec<(&str, [u8; 4], Vec<u8>)> = vec![
        ("iCCP", *b"iCCP", {
            let mut d = b"perfil\0\0".to_vec();
            d.extend(zlib_stored(b"datos", 0));
            d
        }),
        ("gAMA", *b"gAMA", 100_000u32.to_be_bytes().to_vec()),
        ("tEXt", *b"tEXt", b"Title\0pixoko".to_vec()),
    ];
    for (name_2, kind, data) in cases {
        let good = lee(&with(&[piece(&kind, &data)], &[]));
        let bad = lee(&with(&[crc_bad(&kind, &data)], &[]));
        assert_eq!(bad.aux, good.aux, "{name_2}");
        assert!(!bad.aux.found.is_empty(), "{name_2}: se lista igual");
        assert_eq!(bad.image, good.image, "{name_2}");
        assert_eq!(bad.report, good.report, "{name_2}");
        // Y después del IDAT.
        let bad_after = lee(&with(&[], &[crc_bad(&kind, &data)]));
        assert_eq!(bad_after.aux, good.aux, "{name_2} tras el IDAT");
    }
    // Lo que queda dicho en concreto: el iCCP de CRC malo activa la copia.
    let r = aux_de(&with(&[crc_bad(b"iCCP", b"x")], &[]));
    assert_eq!(r, report(&[AuxChunk::Icc], Some(AuxChunk::Icc)));
}

#[test]
fn row_6_odd_types_and_zero_length() {
    let cases: Vec<(&str, Vec<u8>, Vec<AuxChunk>)> = vec![
        ("zzZz", piece(b"zzZz", &[1, 2]), vec![AuxChunk::Other]),
        (
            "bytes no alfabéticos con minúscula inicial",
            piece(&[0x61, 0x01, 0x02, 0x03], &[9, 9]),
            vec![AuxChunk::Other],
        ),
        (
            "desconocido de longitud 0",
            piece(b"teSt", &[]),
            vec![AuxChunk::Other],
        ),
        (
            "tEXt de longitud 0",
            piece(b"tEXt", &[]),
            vec![AuxChunk::Text],
        ),
        (
            "eXIf de longitud 0",
            piece(b"eXIf", &[]),
            vec![AuxChunk::Exif],
        ),
    ];
    for (name_2, chunk, found) in cases {
        for (location, bytes) in [
            ("antes", with(std::slice::from_ref(&chunk), &[])),
            ("después", with(&[], std::slice::from_ref(&chunk))),
        ] {
            let r = read_png(&bytes);
            let r = r.unwrap_or_else(|e| panic!("{name_2} {location} del IDAT: {e:?}"));
            assert_eq!(r.aux, report(&found, None), "{name_2} {location} del IDAT");
            assert_eq!(pixels(&r.image), [rgba(10, 20, 30, 40)], "{name_2}");
        }
    }
}

#[test]
fn row_6_declared_length_of_2_31_minus_1_with_a_short_file_is_decode() {
    let valid = base().bytes();
    let iend = valid.len() - 12;
    for kind in [b"eXIf", b"iCCP", b"tEXt", b"teSt"] {
        let name_2 = String::from_utf8_lossy(kind).into_owned();
        // Antes del IDAT (tras el IHDR) y después (antes del IEND).
        let mut antes = valid[..33].to_vec();
        antes.extend(giant(kind));
        antes.extend(&valid[33..]);
        let mut after = valid[..iend].to_vec();
        after.extend(giant(kind));
        after.extend(&valid[iend..]);
        for (location, bytes) in [("antes", antes), ("después", after)] {
            let r = read_png(&bytes);
            assert!(es_decode(&r), "{name_2} {location} del IDAT: {r:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 7: APNG.
// ---------------------------------------------------------------------------------------

/// `fcTL` de 2×1 sin desplazamiento (26 bytes) con ese número de secuencia.
fn fctl(sequence: u32) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend(sequence.to_be_bytes());
    d.extend(2u32.to_be_bytes());
    d.extend(1u32.to_be_bytes());
    d.extend([0u8; 8]);
    d.extend([0, 1, 0, 10, 0, 0]);
    d
}

/// APNG de 2×1 en gris con el IDAT `[10, 20]` como imagen por defecto y un segundo cuadro
/// en un `fdAT`. Con `with_actl` falso falta el `acTL`; `extra` va entre el IHDR y el `acTL`.
fn apng(with_actl: bool, extra: &[Vec<u8>]) -> Vec<u8> {
    let mut fdat = 2u32.to_be_bytes().to_vec();
    fdat.extend(zlib_stored(&[0, 30, 40], 0));
    let mut png = Png::new(2, 1, 8, 0).swatches(&[10, 20]);
    for c in extra {
        png = png.antes(c.clone());
    }
    if with_actl {
        png = png.antes(c_actl());
    }
    png.antes(piece(b"fcTL", &fctl(0)))
        .after(piece(b"fcTL", &fctl(1)))
        .after(piece(b"fdAT", &fdat))
        .bytes()
}

#[test]
fn row_7_apng_only_triggers_the_copy_with_its_first_frame() {
    let r = lee(&apng(true, &[]));
    assert_eq!(
        r.aux,
        report(&[AuxChunk::Animation], Some(AuxChunk::Animation))
    );
    assert_eq!(r.image.mode(), Mode::Gray);
    assert_eq!(pixels(&r.image), grays(&[10, 20]));
    assert!(r.report.is_empty());
}

#[test]
fn row_7_apng_with_iccp_wins_animation() {
    let r = lee(&apng(true, &[c_iccp()]));
    assert_eq!(
        r.aux,
        report(
            &[AuxChunk::Icc, AuxChunk::Animation],
            Some(AuxChunk::Animation)
        )
    );
    assert_eq!(pixels(&r.image), grays(&[10, 20]));
}

#[test]
fn row_7_fctl_without_actl_is_listed_but_does_not_trigger_the_copy() {
    // Un APNG roto: `fcTL` (y `fdAT`) sin `acTL`.
    let without_actl = Png::new(2, 1, 8, 0)
        .swatches(&[10, 20])
        .antes(piece(b"fcTL", &fctl(0)))
        .bytes();
    let r = lee(&without_actl);
    assert_eq!(r.aux, report(&[AuxChunk::Animation], None));
    assert_eq!(pixels(&r.image), grays(&[10, 20]));

    let r = lee(&apng(false, &[]));
    assert_eq!(r.aux, report(&[AuxChunk::Animation], None));
    assert_eq!(pixels(&r.image), grays(&[10, 20]));
}

// ---------------------------------------------------------------------------------------
// Propiedad 1: los auxiliares no cambian la imagen ni el informe.
// ---------------------------------------------------------------------------------------

/// Un auxiliar válido de los tipos de D2, con valores cualesquiera dentro de lo legal.
fn aux_valid() -> impl Strategy<Value = Vec<u8>> {
    let fixed = select(vec![
        c_iccp(),
        c_phys(),
        c_text(),
        c_ztxt(),
        c_itxt(),
        c_time(),
        c_exif(),
        c_actl(),
        c_unknown(),
    ]);
    prop_oneof![
        5 => fixed,
        1 => (any::<u8>(), any::<u8>(), 0u8..=1)
            .prop_map(|(p, t, r)| c_cicp(p, t, 0, r)),
        1 => (0u8..=3).prop_map(|i| piece(b"sRGB", &[i])),
        1 => (1u32..=200_000).prop_map(c_gamma),
        1 => proptest::array::uniform8(0u32..=100_000).prop_map(c_chrm),
    ]
}

proptest! {
    #[test]
    fn property_1_auxiliaries_change_neither_the_image_nor_the_report(
        img in image_arbitrary(),
        chunks in vec((aux_valid(), any::<bool>()), 0..=8),
    ) {
        let without_bytes = write_png(&img).unwrap();
        let antes: Vec<Vec<u8>> = chunks.iter().filter(|(_, d)| !d).map(|(c, _)| c.clone()).collect();
        let after: Vec<Vec<u8>> = chunks.iter().filter(|(_, d)| *d).map(|(c, _)| c.clone()).collect();
        let with_bytes = inserts(&without_bytes, &antes, &after);

        let without = read_png(&without_bytes).unwrap();
        let with = read_png(&with_bytes).unwrap();
        prop_assert_eq!(&with.image, &without.image);
        prop_assert_eq!(&with.report, &without.report);
        prop_assert_eq!(with.is_exact(), without.is_exact());
        prop_assert_eq!(
            write_png(&with.image).unwrap(),
            write_png(&without.image).unwrap()
        );
        prop_assert_eq!(without.aux, AuxReport::default());
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 2: nunca pánico y el informe cumple su forma.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Garbage {
    kind: [u8; 4],
    data: Vec<u8>,
    crc_good: bool,
    /// Declara 2³¹ − 1 bytes de datos aunque lleve solo `data`.
    declares_of_more: bool,
    after: bool,
}

impl Garbage {
    fn bytes(&self) -> Vec<u8> {
        let largo = if self.declares_of_more {
            u32::MAX >> 1
        } else {
            self.data.len() as u32
        };
        let mut body = self.kind.to_vec();
        body.extend(&self.data);
        let mut crc = crc32(&body);
        if !self.crc_good {
            crc ^= 0xFF;
        }
        let mut v = largo.to_be_bytes().to_vec();
        v.extend(&body);
        v.extend(crc.to_be_bytes());
        v
    }
}

fn garbage() -> impl Strategy<Value = Garbage> {
    (
        b'a'..=b'z',
        any::<[u8; 3]>(),
        vec(any::<u8>(), 0..=64),
        any::<bool>(),
        prop_oneof![4 => Just(false), 1 => Just(true)],
        any::<bool>(),
    )
        .prop_map(
            |(initial, rest_2, data, crc_good, declares_of_more, after)| Garbage {
                kind: [initial, rest_2[0], rest_2[1], rest_2[2]],
                data,
                crc_good,
                declares_of_more,
                after,
            },
        )
}

/// `found` ordenado y sin repetidos, y `appearance` o `None` o un elemento de `found` entre
/// los cinco que activan la copia.
fn checks_shape(aux: &AuxReport) -> Result<(), TestCaseError> {
    prop_assert!(
        aux.found.windows(2).all(|w| w[0] < w[1]),
        "found desordenado o con repetidos: {:?}",
        aux.found
    );
    prop_assert!(aux.found.len() <= 11);
    if let Some(a) = aux.appearance {
        prop_assert!(
            matches!(
                a,
                AuxChunk::Animation
                    | AuxChunk::Icc
                    | AuxChunk::CodePoints
                    | AuxChunk::Gamma
                    | AuxChunk::Chromaticities
            ),
            "appearance no admitida: {:?}",
            a
        );
        prop_assert!(aux.found.contains(&a), "{:?} no está en {:?}", a, aux.found);
    }
    Ok(())
}

proptest! {
    #[test]
    fn property_2_never_a_panic_and_the_report_meets_its_shape(
        img in image_arbitrary(),
        chunks in vec(garbage(), 0..=8),
        changes in vec((any::<usize>(), any::<u8>()), 1..=4),
    ) {
        let valid = write_png(&img).unwrap();
        let antes: Vec<Vec<u8>> = chunks.iter().filter(|c| !c.after).map(Garbage::bytes).collect();
        let after: Vec<Vec<u8>> = chunks.iter().filter(|c| c.after).map(Garbage::bytes).collect();
        let mut bytes = inserts(&valid, &antes, &after);
        for (location, new_2) in changes {
            let i = location % bytes.len();
            bytes[i] = new_2;
        }
        if let Ok(read) = read_png(&bytes) {
            checks_shape(&read.aux)?;
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 3: `appearance` contra un modelo escrito aparte.
// ---------------------------------------------------------------------------------------

/// Los seis chunks que influyen en `appearance`, cada uno ausente o con sus bytes.
#[derive(Clone, Debug)]
struct CaseColor {
    actl: bool,
    iccp: Option<Vec<u8>>,
    cicp: Option<Vec<u8>>,
    srgb: Option<Vec<u8>>,
    gamma: Option<Vec<u8>>,
    chrm: Option<Vec<u8>>,
}

impl CaseColor {
    /// Los chunks presentes, con CRC bueno, por el orden de los campos.
    fn chunks(&self) -> Vec<Vec<u8>> {
        let mut v = Vec::new();
        if self.actl {
            v.push(c_actl());
        }
        for (kind, data) in [
            (b"iCCP", &self.iccp),
            (b"cICP", &self.cicp),
            (b"sRGB", &self.srgb),
            (b"gAMA", &self.gamma),
            (b"cHRM", &self.chrm),
        ] {
            if let Some(d) = data {
                v.push(piece(kind, d));
            }
        }
        v
    }
}

/// Bytes de un chunk con la longitud pedida: los de `base` recortados o completados con ceros.
fn with_length(mut base: Vec<u8>, largo: usize) -> Vec<u8> {
    base.resize(largo, 0);
    base
}

fn case_color() -> impl Strategy<Value = CaseColor> {
    let iccp = proptest::option::of(vec(any::<u8>(), 0..=40));
    let cicp = proptest::option::of(
        (
            prop_oneof![Just(1u8), any::<u8>()],
            prop_oneof![Just(13u8), any::<u8>()],
            0u8..=1,
            select(vec![4usize, 4, 4, 3, 5, 0]),
        )
            .prop_map(|(p, t, r, largo)| with_length(vec![p, t, 0, r], largo)),
    );
    let srgb = proptest::option::of(
        (any::<u8>(), select(vec![1usize, 1, 1, 0, 2]))
            .prop_map(|(v, largo)| with_length(vec![v], largo)),
    );
    let gamma = proptest::option::of(
        (
            prop_oneof![
                select(vec![
                    45455u32, 44455, 46455, 44454, 46456, 45456, 100_000, 55000, 35000
                ]),
                1u32..=200_000
            ],
            select(vec![4usize, 4, 4, 3, 5]),
        )
            .prop_map(|(g, largo)| with_length(g.to_be_bytes().to_vec(), largo)),
    );
    let deviation = select(vec![0i64, 0, 0, 1000, -1000, 1001, -1001, 20_000, -20_000]);
    let chrm = proptest::option::of(
        (
            proptest::array::uniform8(deviation),
            select(vec![32usize, 32, 32, 31, 33]),
        )
            .prop_map(|(d, largo)| {
                let data: Vec<u8> = CHRM_SRGB
                    .iter()
                    .zip(d)
                    .flat_map(|(&v, d)| {
                        let valor = u32::try_from((i64::from(v) + d).max(0)).unwrap();
                        valor.to_be_bytes()
                    })
                    .collect();
                with_length(data, largo)
            }),
    );
    (any::<bool>(), iccp, cicp, srgb, gamma, chrm).prop_map(
        |(actl, iccp, cicp, srgb, gamma, chrm)| CaseColor {
            actl,
            iccp,
            cicp,
            srgb,
            gamma,
            chrm,
        },
    )
}

/// La regla de D2, escrita aparte de la implementación.
fn appearance_expected(c: &CaseColor) -> Option<AuxChunk> {
    let be = |d: &[u8], i: usize| u32::from_be_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]]);
    if c.actl {
        return Some(AuxChunk::Animation);
    }
    if c.iccp.is_some() {
        return Some(AuxChunk::Icc);
    }
    if let Some(d) = c.cicp.as_ref().filter(|d| d.len() == 4) {
        return (d[0] != 1 || d[1] != 13).then_some(AuxChunk::CodePoints);
    }
    if c.srgb.as_ref().is_some_and(|d| d.len() == 1) {
        return None;
    }
    if let Some(d) = c.gamma.as_ref().filter(|d| d.len() == 4)
        && be(d, 0).abs_diff(45455) > 1000
    {
        return Some(AuxChunk::Gamma);
    }
    if let Some(d) = c.chrm.as_ref().filter(|d| d.len() == 32)
        && (0..8).any(|i| be(d, 4 * i).abs_diff(CHRM_SRGB[i]) > 1000)
    {
        return Some(AuxChunk::Chromaticities);
    }
    None
}

/// `found` esperado: el tipo de cada chunk presente, o `Other` si su tamaño es imposible.
fn found_expected(c: &CaseColor) -> Vec<AuxChunk> {
    let mut found = Vec::new();
    if c.actl {
        found.push(AuxChunk::Animation);
    }
    if c.iccp.is_some() {
        found.push(AuxChunk::Icc);
    }
    let mut with_size = |present: &Option<Vec<u8>>, largo: usize, kind: AuxChunk| {
        if let Some(d) = present {
            found.push(if d.len() == largo {
                kind
            } else {
                AuxChunk::Other
            });
        }
    };
    with_size(&c.cicp, 4, AuxChunk::CodePoints);
    with_size(&c.srgb, 1, AuxChunk::Srgb);
    with_size(&c.gamma, 4, AuxChunk::Gamma);
    with_size(&c.chrm, 32, AuxChunk::Chromaticities);
    found.sort();
    found.dedup();
    found
}

proptest! {
    #[test]
    fn property_3_appearance_matches_the_d2_model(case in case_color()) {
        let r = read_png(&with(&case.chunks(), &[])).unwrap();
        prop_assert_eq!(r.aux.appearance, appearance_expected(&case), "{:?}", case);
        prop_assert_eq!(&r.aux.found, &found_expected(&case), "{:?}", case);
        prop_assert!(r.report.is_empty());
        prop_assert_eq!(pixels(&r.image), vec![rgba(10, 20, 30, 40)]);
    }
}
