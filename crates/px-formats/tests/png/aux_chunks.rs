//! Chunks auxiliares del PNG en el informe de lectura (specs/hito-2-chunks-auxiliares.md,
//! unidad 13b): filas 1 a 7 (la parte de `read_png`) y propiedades 1 a 3. En ROJO hasta la
//! implementación de `AuxChunk`, `AuxReport` y `PngRead.aux`.
//!
//! Todos los PNG se construyen a mano con los ayudantes del fichero padre (`trozo`, `Png`,
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
    Png::new(1, 1, 8, 6).muestras(&[10, 20, 30, 40])
}

/// `base()` con esos chunks entre IHDR e IDAT y después del IDAT.
fn con(antes: &[Vec<u8>], despues: &[Vec<u8>]) -> Vec<u8> {
    let mut png = base();
    for c in antes {
        png = png.antes(c.clone());
    }
    for c in despues {
        png = png.despues(c.clone());
    }
    png.bytes()
}

/// Inserta chunks en un PNG de `write_png` (un solo IDAT): `antes` justo tras el IHDR (que
/// acaba en el byte 33) y `despues` justo antes del IEND (los últimos 12 bytes).
fn inserta(bytes: &[u8], antes: &[Vec<u8>], despues: &[Vec<u8>]) -> Vec<u8> {
    let iend = bytes.len() - 12;
    let mut v = bytes[..33].to_vec();
    for c in antes {
        v.extend(c);
    }
    v.extend(&bytes[33..iend]);
    for c in despues {
        v.extend(c);
    }
    v.extend(&bytes[iend..]);
    v
}

fn aux_de(bytes: &[u8]) -> AuxReport {
    lee(bytes).aux
}

fn informe(found: &[AuxChunk], appearance: Option<AuxChunk>) -> AuxReport {
    AuxReport {
        found: found.to_vec(),
        appearance,
    }
}

fn c_iccp() -> Vec<u8> {
    let mut d = b"perfil\0".to_vec();
    d.push(0);
    d.extend(zlib_stored(b"datos del perfil", 0));
    trozo(b"iCCP", &d)
}

fn c_cicp(primarios: u8, transferencia: u8, matriz: u8, rango: u8) -> Vec<u8> {
    trozo(b"cICP", &[primarios, transferencia, matriz, rango])
}

fn c_srgb() -> Vec<u8> {
    trozo(b"sRGB", &[0])
}

fn c_gama(valor: u32) -> Vec<u8> {
    trozo(b"gAMA", &valor.to_be_bytes())
}

/// Blanco D65 y primarios de sRGB, por este orden: blanco x e y, rojo, verde y azul.
const CHRM_SRGB: [u32; 8] = [31270, 32900, 64000, 33000, 30000, 60000, 15000, 6000];
/// Blanco D65 y primarios de Adobe RGB (1998): solo cambia el verde.
const CHRM_ADOBE: [u32; 8] = [31270, 32900, 64000, 33000, 21000, 71000, 15000, 6000];

fn c_chrm(valores: [u32; 8]) -> Vec<u8> {
    let datos: Vec<u8> = valores.iter().flat_map(|v| v.to_be_bytes()).collect();
    trozo(b"cHRM", &datos)
}

fn c_phys() -> Vec<u8> {
    let mut d = Vec::new();
    d.extend(2835u32.to_be_bytes());
    d.extend(2835u32.to_be_bytes());
    d.push(1);
    trozo(b"pHYs", &d)
}

fn c_text() -> Vec<u8> {
    trozo(b"tEXt", b"Title\0pixoko")
}

fn c_ztxt() -> Vec<u8> {
    let mut d = b"Title\0\0".to_vec();
    d.extend(zlib_stored(b"pixoko", 0));
    trozo(b"zTXt", &d)
}

fn c_itxt() -> Vec<u8> {
    // Palabra clave, indicador y método de compresión, etiqueta de idioma y palabra traducida
    // vacías, y el texto.
    trozo(b"iTXt", b"Title\0\0\0\0\0pixoko")
}

fn c_time() -> Vec<u8> {
    trozo(b"tIME", &[0x07, 0xEA, 10, 17, 12, 0, 0])
}

fn c_exif() -> Vec<u8> {
    trozo(b"eXIf", b"MM\0*\0\0\0\x08")
}

fn c_actl() -> Vec<u8> {
    trozo(b"acTL", &[0, 0, 0, 2, 0, 0, 0, 0])
}

fn c_desconocido() -> Vec<u8> {
    trozo(b"teSt", &[1, 2, 3])
}

/// El mismo chunk con el último byte del CRC cambiado.
fn crc_malo(tipo: &[u8; 4], datos: &[u8]) -> Vec<u8> {
    let mut v = trozo(tipo, datos);
    let n = v.len();
    v[n - 1] ^= 0xFF;
    v
}

/// Cabecera de un chunk que declara 2³¹ − 1 bytes de datos y solo trae tres.
fn gigante(tipo: &[u8; 4]) -> Vec<u8> {
    let mut v = (u32::MAX >> 1).to_be_bytes().to_vec();
    v.extend(tipo);
    v.extend([1, 2, 3]);
    v
}

// ---------------------------------------------------------------------------------------
// Fila 1: sin auxiliares y corpus.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_1_sin_auxiliares_el_informe_esta_vacio() {
    let rgba = lee(&base().bytes());
    assert_eq!(rgba.aux, AuxReport::default());
    assert!(rgba.aux.found.is_empty());
    assert_eq!(rgba.aux.appearance, None);

    let indexada = Png::new(2, 1, 8, 3)
        .muestras(&[0, 1])
        .antes(plte(&[(0, 0, 0), (255, 0, 0)]))
        .bytes();
    let r = lee(&indexada);
    assert_eq!(r.aux, AuxReport::default());
    assert!(r.aux.found.is_empty());
    assert_eq!(r.aux.appearance, None);
}

#[test]
fn fila_1_corpus_de_golden_png() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/png");
    let (mut grafx2, mut libresprite) = (0, 0);
    for entrada in fs::read_dir(&dir).expect("leer golden/png") {
        let ruta = entrada.expect("entrada").path();
        if ruta.extension().is_none_or(|e| e != "png") {
            continue;
        }
        let nombre = ruta.file_name().unwrap().to_string_lossy().into_owned();
        let r = lee(&fs::read(&ruta).unwrap());
        if nombre.starts_with("grafx2") {
            grafx2 += 1;
            assert_eq!(
                r.aux,
                informe(&[AuxChunk::Text, AuxChunk::Other], None),
                "{nombre}"
            );
        } else if nombre.starts_with("libresprite") {
            libresprite += 1;
            assert_eq!(r.aux, AuxReport::default(), "{nombre}");
        } else {
            panic!("fichero inesperado en el corpus: {nombre}");
        }
    }
    assert_eq!((grafx2, libresprite), (2, 2));
}

// ---------------------------------------------------------------------------------------
// Fila 2: cada tipo válido por separado.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_2_cada_tipo_valido_por_separado() {
    use AuxChunk::*;
    let casos: Vec<(&str, Vec<u8>, AuxChunk, Option<AuxChunk>)> = vec![
        ("iCCP", c_iccp(), Icc, Some(Icc)),
        ("cICP (1,13,0,1)", c_cicp(1, 13, 0, 1), CodePoints, None),
        ("sRGB", c_srgb(), Srgb, None),
        ("gAMA 45455", c_gama(45455), Gamma, None),
        ("cHRM de sRGB", c_chrm(CHRM_SRGB), Chromaticities, None),
        ("pHYs", c_phys(), Physical, None),
        ("tEXt", c_text(), Text, None),
        ("zTXt", c_ztxt(), Text, None),
        ("iTXt", c_itxt(), Text, None),
        ("tIME", c_time(), Time, None),
        ("eXIf", c_exif(), Exif, None),
        ("acTL", c_actl(), Animation, Some(Animation)),
        ("teSt", c_desconocido(), Other, None),
    ];
    for (nombre, chunk, tipo, apariencia) in casos {
        let r = lee(&con(&[chunk], &[]));
        assert_eq!(r.aux, informe(&[tipo], apariencia), "{nombre}");
        // Lo que se ignora no cambia ni la imagen ni el informe de conversiones.
        assert!(r.report.is_empty(), "{nombre}");
        assert_eq!(pixeles(&r.image), [rgba(10, 20, 30, 40)], "{nombre}");
    }
}

// ---------------------------------------------------------------------------------------
// Fila 3: repetidos, orden, posición y sin tope.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_3_repetidos_sin_duplicados_y_en_orden_de_declaracion_antes_o_despues_del_idat() {
    let secuencia = || vec![c_text(), c_iccp(), c_text(), c_gama(45455), c_iccp()];
    let esperado = informe(
        &[AuxChunk::Icc, AuxChunk::Gamma, AuxChunk::Text],
        Some(AuxChunk::Icc),
    );
    assert_eq!(aux_de(&con(&secuencia(), &[])), esperado, "antes del IDAT");
    assert_eq!(
        aux_de(&con(&[], &secuencia())),
        esperado,
        "después del IDAT"
    );
}

#[test]
fn fila_3_dos_mil_text_no_esconden_un_iccp_al_final() {
    let mut chunks: Vec<Vec<u8>> = (0..2000)
        .map(|i| trozo(b"tEXt", format!("clave{i}\0valor").as_bytes()))
        .collect();
    chunks.push(c_iccp());
    let r = lee(&con(&chunks, &[]));
    assert_eq!(
        r.aux,
        informe(&[AuxChunk::Icc, AuxChunk::Text], Some(AuxChunk::Icc))
    );
    assert!(r.report.is_empty());
}

#[test]
fn fila_3_de_un_tipo_repetido_vale_el_primero() {
    // D3: «repetidos ⇒ vale el primero y el tipo se lista una vez».
    let gamma_sin_causa_primero = aux_de(&con(&[c_gama(45455), c_gama(100_000)], &[]));
    assert_eq!(gamma_sin_causa_primero, informe(&[AuxChunk::Gamma], None));
    let gamma_con_causa_primero = aux_de(&con(&[c_gama(100_000), c_gama(45455)], &[]));
    assert_eq!(
        gamma_con_causa_primero,
        informe(&[AuxChunk::Gamma], Some(AuxChunk::Gamma))
    );
}

// ---------------------------------------------------------------------------------------
// Fila 4: qué activa la copia (`appearance`).
// ---------------------------------------------------------------------------------------

#[test]
fn fila_4_causas_segun_la_regla_de_d2() {
    use AuxChunk::*;
    let casos: Vec<(&str, Vec<Vec<u8>>, Option<AuxChunk>)> = vec![
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
            vec![c_cicp(1, 13, 0, 0), c_gama(100_000)],
            None,
        ),
        (
            "cICP (1,13,0,1) + cHRM Adobe",
            vec![c_cicp(1, 13, 0, 1), c_chrm(CHRM_ADOBE)],
            None,
        ),
        (
            "sRGB + gAMA 100000 + cHRM Adobe: sRGB acaba la búsqueda",
            vec![c_srgb(), c_gama(100_000), c_chrm(CHRM_ADOBE)],
            None,
        ),
        ("gAMA 100000", vec![c_gama(100_000)], Some(Gamma)),
        ("gAMA 45455", vec![c_gama(45455)], None),
        ("gAMA 44455 (−1000)", vec![c_gama(44455)], None),
        ("gAMA 46455 (+1000)", vec![c_gama(46455)], None),
        ("gAMA 46456 (+1001)", vec![c_gama(46456)], Some(Gamma)),
        ("gAMA 44454 (−1001)", vec![c_gama(44454)], Some(Gamma)),
        (
            "cHRM Adobe RGB",
            vec![c_chrm(CHRM_ADOBE)],
            Some(Chromaticities),
        ),
        ("cHRM de sRGB", vec![c_chrm(CHRM_SRGB)], None),
        (
            "gAMA lineal + cHRM Adobe: manda Gamma",
            vec![c_gama(100_000), c_chrm(CHRM_ADOBE)],
            Some(Gamma),
        ),
        (
            "gAMA de sRGB + cHRM Adobe",
            vec![c_gama(45455), c_chrm(CHRM_ADOBE)],
            Some(Chromaticities),
        ),
        (
            "gAMA lineal + cHRM de sRGB",
            vec![c_gama(100_000), c_chrm(CHRM_SRGB)],
            Some(Gamma),
        ),
    ];
    for (nombre, chunks, esperado) in casos {
        let r = aux_de(&con(&chunks, &[]));
        assert_eq!(r.appearance, esperado, "{nombre}: {r:?}");
        if let Some(a) = esperado {
            assert!(r.found.contains(&a), "{nombre}: {r:?}");
        }
    }
}

#[test]
fn fila_4_chrm_de_srgb_mas_menos_1000_no_activa_y_mas_menos_1001_si() {
    for i in 0..8 {
        for (delta, esperado) in [
            (1000i64, None),
            (-1000, None),
            (1001, Some(AuxChunk::Chromaticities)),
            (-1001, Some(AuxChunk::Chromaticities)),
        ] {
            let mut v = CHRM_SRGB;
            v[i] = u32::try_from(i64::from(v[i]) + delta).unwrap();
            let r = aux_de(&con(&[c_chrm(v)], &[]));
            assert_eq!(r.appearance, esperado, "valor {i}, delta {delta}");
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
fn fila_5_longitud_imposible_cuenta_como_otro_y_no_activa_nada() {
    let casos: Vec<(&str, Vec<u8>)> = vec![
        ("gAMA de 3 bytes", trozo(b"gAMA", &[0, 1, 2])),
        ("cHRM de 31 bytes", trozo(b"cHRM", &[7; 31])),
        ("sRGB de 2 bytes", trozo(b"sRGB", &[0, 0])),
        ("cICP de 5 bytes", trozo(b"cICP", &[9, 16, 9, 1, 0])),
    ];
    for (nombre, chunk) in casos {
        let r = lee(&con(&[chunk], &[]));
        assert_eq!(r.aux, informe(&[AuxChunk::Other], None), "{nombre}");
        assert!(r.report.is_empty(), "{nombre}");
    }
}

#[test]
fn fila_5_iccp_vacio_activa_la_copia() {
    let r = lee(&con(&[trozo(b"iCCP", &[])], &[]));
    assert_eq!(r.aux, informe(&[AuxChunk::Icc], Some(AuxChunk::Icc)));
}

// ---------------------------------------------------------------------------------------
// Fila 6: basura.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_6_crc_erroneo_se_lista_como_si_fuera_bueno() {
    let casos: Vec<(&str, [u8; 4], Vec<u8>)> = vec![
        ("iCCP", *b"iCCP", {
            let mut d = b"perfil\0\0".to_vec();
            d.extend(zlib_stored(b"datos", 0));
            d
        }),
        ("gAMA", *b"gAMA", 100_000u32.to_be_bytes().to_vec()),
        ("tEXt", *b"tEXt", b"Title\0pixoko".to_vec()),
    ];
    for (nombre, tipo, datos) in casos {
        let bueno = lee(&con(&[trozo(&tipo, &datos)], &[]));
        let malo = lee(&con(&[crc_malo(&tipo, &datos)], &[]));
        assert_eq!(malo.aux, bueno.aux, "{nombre}");
        assert!(!malo.aux.found.is_empty(), "{nombre}: se lista igual");
        assert_eq!(malo.image, bueno.image, "{nombre}");
        assert_eq!(malo.report, bueno.report, "{nombre}");
        // Y después del IDAT.
        let malo_despues = lee(&con(&[], &[crc_malo(&tipo, &datos)]));
        assert_eq!(malo_despues.aux, bueno.aux, "{nombre} tras el IDAT");
    }
    // Lo que queda dicho en concreto: el iCCP de CRC malo activa la copia.
    let r = aux_de(&con(&[crc_malo(b"iCCP", b"x")], &[]));
    assert_eq!(r, informe(&[AuxChunk::Icc], Some(AuxChunk::Icc)));
}

#[test]
fn fila_6_tipos_raros_y_longitud_cero() {
    let casos: Vec<(&str, Vec<u8>, Vec<AuxChunk>)> = vec![
        ("zzZz", trozo(b"zzZz", &[1, 2]), vec![AuxChunk::Other]),
        (
            "bytes no alfabéticos con minúscula inicial",
            trozo(&[0x61, 0x01, 0x02, 0x03], &[9, 9]),
            vec![AuxChunk::Other],
        ),
        (
            "desconocido de longitud 0",
            trozo(b"teSt", &[]),
            vec![AuxChunk::Other],
        ),
        (
            "tEXt de longitud 0",
            trozo(b"tEXt", &[]),
            vec![AuxChunk::Text],
        ),
        (
            "eXIf de longitud 0",
            trozo(b"eXIf", &[]),
            vec![AuxChunk::Exif],
        ),
    ];
    for (nombre, chunk, found) in casos {
        for (donde, bytes) in [
            ("antes", con(std::slice::from_ref(&chunk), &[])),
            ("después", con(&[], std::slice::from_ref(&chunk))),
        ] {
            let r = read_png(&bytes);
            let r = r.unwrap_or_else(|e| panic!("{nombre} {donde} del IDAT: {e:?}"));
            assert_eq!(r.aux, informe(&found, None), "{nombre} {donde} del IDAT");
            assert_eq!(pixeles(&r.image), [rgba(10, 20, 30, 40)], "{nombre}");
        }
    }
}

#[test]
fn fila_6_longitud_declarada_de_2_31_menos_1_con_el_fichero_corto_es_decode() {
    let valido = base().bytes();
    let iend = valido.len() - 12;
    for tipo in [b"eXIf", b"iCCP", b"tEXt", b"teSt"] {
        let nombre = String::from_utf8_lossy(tipo).into_owned();
        // Antes del IDAT (tras el IHDR) y después (antes del IEND).
        let mut antes = valido[..33].to_vec();
        antes.extend(gigante(tipo));
        antes.extend(&valido[33..]);
        let mut despues = valido[..iend].to_vec();
        despues.extend(gigante(tipo));
        despues.extend(&valido[iend..]);
        for (donde, bytes) in [("antes", antes), ("después", despues)] {
            let r = read_png(&bytes);
            assert!(es_decode(&r), "{nombre} {donde} del IDAT: {r:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 7: APNG.
// ---------------------------------------------------------------------------------------

/// `fcTL` de 2×1 sin desplazamiento (26 bytes) con ese número de secuencia.
fn fctl(secuencia: u32) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend(secuencia.to_be_bytes());
    d.extend(2u32.to_be_bytes());
    d.extend(1u32.to_be_bytes());
    d.extend([0u8; 8]);
    d.extend([0, 1, 0, 10, 0, 0]);
    d
}

/// APNG de 2×1 en gris con el IDAT `[10, 20]` como imagen por defecto y un segundo cuadro
/// en un `fdAT`. Con `con_actl` falso falta el `acTL`; `extra` va entre el IHDR y el `acTL`.
fn apng(con_actl: bool, extra: &[Vec<u8>]) -> Vec<u8> {
    let mut fdat = 2u32.to_be_bytes().to_vec();
    fdat.extend(zlib_stored(&[0, 30, 40], 0));
    let mut png = Png::new(2, 1, 8, 0).muestras(&[10, 20]);
    for c in extra {
        png = png.antes(c.clone());
    }
    if con_actl {
        png = png.antes(c_actl());
    }
    png.antes(trozo(b"fcTL", &fctl(0)))
        .despues(trozo(b"fcTL", &fctl(1)))
        .despues(trozo(b"fdAT", &fdat))
        .bytes()
}

#[test]
fn fila_7_apng_solo_activa_la_copia_con_su_primer_fotograma() {
    let r = lee(&apng(true, &[]));
    assert_eq!(
        r.aux,
        informe(&[AuxChunk::Animation], Some(AuxChunk::Animation))
    );
    assert_eq!(r.image.mode(), Mode::Gray);
    assert_eq!(pixeles(&r.image), grises(&[10, 20]));
    assert!(r.report.is_empty());
}

#[test]
fn fila_7_apng_con_iccp_manda_animation() {
    let r = lee(&apng(true, &[c_iccp()]));
    assert_eq!(
        r.aux,
        informe(
            &[AuxChunk::Icc, AuxChunk::Animation],
            Some(AuxChunk::Animation)
        )
    );
    assert_eq!(pixeles(&r.image), grises(&[10, 20]));
}

#[test]
fn fila_7_fctl_sin_actl_se_lista_pero_no_activa_la_copia() {
    // Un APNG roto: `fcTL` (y `fdAT`) sin `acTL`.
    let sin_actl = Png::new(2, 1, 8, 0)
        .muestras(&[10, 20])
        .antes(trozo(b"fcTL", &fctl(0)))
        .bytes();
    let r = lee(&sin_actl);
    assert_eq!(r.aux, informe(&[AuxChunk::Animation], None));
    assert_eq!(pixeles(&r.image), grises(&[10, 20]));

    let r = lee(&apng(false, &[]));
    assert_eq!(r.aux, informe(&[AuxChunk::Animation], None));
    assert_eq!(pixeles(&r.image), grises(&[10, 20]));
}

// ---------------------------------------------------------------------------------------
// Propiedad 1: los auxiliares no cambian la imagen ni el informe.
// ---------------------------------------------------------------------------------------

/// Un auxiliar válido de los tipos de D2, con valores cualesquiera dentro de lo legal.
fn aux_valido() -> impl Strategy<Value = Vec<u8>> {
    let fijos = select(vec![
        c_iccp(),
        c_phys(),
        c_text(),
        c_ztxt(),
        c_itxt(),
        c_time(),
        c_exif(),
        c_actl(),
        c_desconocido(),
    ]);
    prop_oneof![
        5 => fijos,
        1 => (any::<u8>(), any::<u8>(), 0u8..=1)
            .prop_map(|(p, t, r)| c_cicp(p, t, 0, r)),
        1 => (0u8..=3).prop_map(|i| trozo(b"sRGB", &[i])),
        1 => (1u32..=200_000).prop_map(c_gama),
        1 => proptest::array::uniform8(0u32..=100_000).prop_map(c_chrm),
    ]
}

proptest! {
    #[test]
    fn propiedad_1_los_auxiliares_no_cambian_la_imagen_ni_el_informe(
        img in imagen_arbitraria(),
        chunks in vec((aux_valido(), any::<bool>()), 0..=8),
    ) {
        let sin_bytes = write_png(&img).unwrap();
        let antes: Vec<Vec<u8>> = chunks.iter().filter(|(_, d)| !d).map(|(c, _)| c.clone()).collect();
        let despues: Vec<Vec<u8>> = chunks.iter().filter(|(_, d)| *d).map(|(c, _)| c.clone()).collect();
        let con_bytes = inserta(&sin_bytes, &antes, &despues);

        let sin = read_png(&sin_bytes).unwrap();
        let con = read_png(&con_bytes).unwrap();
        prop_assert_eq!(&con.image, &sin.image);
        prop_assert_eq!(&con.report, &sin.report);
        prop_assert_eq!(con.is_exact(), sin.is_exact());
        prop_assert_eq!(
            write_png(&con.image).unwrap(),
            write_png(&sin.image).unwrap()
        );
        prop_assert_eq!(sin.aux, AuxReport::default());
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 2: nunca pánico y el informe cumple su forma.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Basura {
    tipo: [u8; 4],
    datos: Vec<u8>,
    crc_bueno: bool,
    /// Declara 2³¹ − 1 bytes de datos aunque lleve solo `datos`.
    declara_de_mas: bool,
    despues: bool,
}

impl Basura {
    fn bytes(&self) -> Vec<u8> {
        let largo = if self.declara_de_mas {
            u32::MAX >> 1
        } else {
            self.datos.len() as u32
        };
        let mut cuerpo = self.tipo.to_vec();
        cuerpo.extend(&self.datos);
        let mut crc = crc32(&cuerpo);
        if !self.crc_bueno {
            crc ^= 0xFF;
        }
        let mut v = largo.to_be_bytes().to_vec();
        v.extend(&cuerpo);
        v.extend(crc.to_be_bytes());
        v
    }
}

fn basura() -> impl Strategy<Value = Basura> {
    (
        b'a'..=b'z',
        any::<[u8; 3]>(),
        vec(any::<u8>(), 0..=64),
        any::<bool>(),
        prop_oneof![4 => Just(false), 1 => Just(true)],
        any::<bool>(),
    )
        .prop_map(
            |(inicial, resto, datos, crc_bueno, declara_de_mas, despues)| Basura {
                tipo: [inicial, resto[0], resto[1], resto[2]],
                datos,
                crc_bueno,
                declara_de_mas,
                despues,
            },
        )
}

/// `found` ordenado y sin repetidos, y `appearance` o `None` o un elemento de `found` entre
/// los cinco que activan la copia.
fn comprueba_forma(aux: &AuxReport) -> Result<(), TestCaseError> {
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
    fn propiedad_2_nunca_panico_y_el_informe_cumple_su_forma(
        img in imagen_arbitraria(),
        chunks in vec(basura(), 0..=8),
        cambios in vec((any::<usize>(), any::<u8>()), 1..=4),
    ) {
        let valido = write_png(&img).unwrap();
        let antes: Vec<Vec<u8>> = chunks.iter().filter(|c| !c.despues).map(Basura::bytes).collect();
        let despues: Vec<Vec<u8>> = chunks.iter().filter(|c| c.despues).map(Basura::bytes).collect();
        let mut bytes = inserta(&valido, &antes, &despues);
        for (donde, nuevo) in cambios {
            let i = donde % bytes.len();
            bytes[i] = nuevo;
        }
        if let Ok(leido) = read_png(&bytes) {
            comprueba_forma(&leido.aux)?;
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 3: `appearance` contra un modelo escrito aparte.
// ---------------------------------------------------------------------------------------

/// Los seis chunks que influyen en `appearance`, cada uno ausente o con sus bytes.
#[derive(Clone, Debug)]
struct CasoColor {
    actl: bool,
    iccp: Option<Vec<u8>>,
    cicp: Option<Vec<u8>>,
    srgb: Option<Vec<u8>>,
    gama: Option<Vec<u8>>,
    chrm: Option<Vec<u8>>,
}

impl CasoColor {
    /// Los chunks presentes, con CRC bueno, por el orden de los campos.
    fn chunks(&self) -> Vec<Vec<u8>> {
        let mut v = Vec::new();
        if self.actl {
            v.push(c_actl());
        }
        for (tipo, datos) in [
            (b"iCCP", &self.iccp),
            (b"cICP", &self.cicp),
            (b"sRGB", &self.srgb),
            (b"gAMA", &self.gama),
            (b"cHRM", &self.chrm),
        ] {
            if let Some(d) = datos {
                v.push(trozo(tipo, d));
            }
        }
        v
    }
}

/// Bytes de un chunk con la longitud pedida: los de `base` recortados o completados con ceros.
fn con_longitud(mut base: Vec<u8>, largo: usize) -> Vec<u8> {
    base.resize(largo, 0);
    base
}

fn caso_color() -> impl Strategy<Value = CasoColor> {
    let iccp = proptest::option::of(vec(any::<u8>(), 0..=40));
    let cicp = proptest::option::of(
        (
            prop_oneof![Just(1u8), any::<u8>()],
            prop_oneof![Just(13u8), any::<u8>()],
            0u8..=1,
            select(vec![4usize, 4, 4, 3, 5, 0]),
        )
            .prop_map(|(p, t, r, largo)| con_longitud(vec![p, t, 0, r], largo)),
    );
    let srgb = proptest::option::of(
        (any::<u8>(), select(vec![1usize, 1, 1, 0, 2]))
            .prop_map(|(v, largo)| con_longitud(vec![v], largo)),
    );
    let gama = proptest::option::of(
        (
            prop_oneof![
                select(vec![
                    45455u32, 44455, 46455, 44454, 46456, 45456, 100_000, 55000, 35000
                ]),
                1u32..=200_000
            ],
            select(vec![4usize, 4, 4, 3, 5]),
        )
            .prop_map(|(g, largo)| con_longitud(g.to_be_bytes().to_vec(), largo)),
    );
    let desvio = select(vec![0i64, 0, 0, 1000, -1000, 1001, -1001, 20_000, -20_000]);
    let chrm = proptest::option::of(
        (
            proptest::array::uniform8(desvio),
            select(vec![32usize, 32, 32, 31, 33]),
        )
            .prop_map(|(d, largo)| {
                let datos: Vec<u8> = CHRM_SRGB
                    .iter()
                    .zip(d)
                    .flat_map(|(&v, d)| {
                        let valor = u32::try_from((i64::from(v) + d).max(0)).unwrap();
                        valor.to_be_bytes()
                    })
                    .collect();
                con_longitud(datos, largo)
            }),
    );
    (any::<bool>(), iccp, cicp, srgb, gama, chrm).prop_map(
        |(actl, iccp, cicp, srgb, gama, chrm)| CasoColor {
            actl,
            iccp,
            cicp,
            srgb,
            gama,
            chrm,
        },
    )
}

/// La regla de D2, escrita aparte de la implementación.
fn apariencia_esperada(c: &CasoColor) -> Option<AuxChunk> {
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
    if let Some(d) = c.gama.as_ref().filter(|d| d.len() == 4)
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
fn encontrados_esperados(c: &CasoColor) -> Vec<AuxChunk> {
    let mut found = Vec::new();
    if c.actl {
        found.push(AuxChunk::Animation);
    }
    if c.iccp.is_some() {
        found.push(AuxChunk::Icc);
    }
    let mut con_tamano = |presente: &Option<Vec<u8>>, largo: usize, tipo: AuxChunk| {
        if let Some(d) = presente {
            found.push(if d.len() == largo {
                tipo
            } else {
                AuxChunk::Other
            });
        }
    };
    con_tamano(&c.cicp, 4, AuxChunk::CodePoints);
    con_tamano(&c.srgb, 1, AuxChunk::Srgb);
    con_tamano(&c.gama, 4, AuxChunk::Gamma);
    con_tamano(&c.chrm, 32, AuxChunk::Chromaticities);
    found.sort();
    found.dedup();
    found
}

proptest! {
    #[test]
    fn propiedad_3_appearance_coincide_con_el_modelo_de_d2(caso in caso_color()) {
        let r = read_png(&con(&caso.chunks(), &[])).unwrap();
        prop_assert_eq!(r.aux.appearance, apariencia_esperada(&caso), "{:?}", caso);
        prop_assert_eq!(&r.aux.found, &encontrados_esperados(&caso), "{:?}", caso);
        prop_assert!(r.report.is_empty());
        prop_assert_eq!(pixeles(&r.image), vec![rgba(10, 20, 30, 40)]);
    }
}
