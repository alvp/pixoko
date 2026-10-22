//! Pruebas de `.aseprite`, etapa 1 (specs/hito-3-aseprite.md, unidad 3.8): `read_aseprite`,
//! `write_aseprite`, `aseprite_dimensions` y `detect_format`.
//!
//! Las filas 2 y 4 a 11 y las propiedades 1, 3 y 4 de «Casos» y «Propiedades». La fila 3 (la
//! composición de los 29 casos) y la parte de la fila 8 con el corpus viven en `px-golden`; la
//! bomba de la fila 9 (con medida de RSS) está en `aseprite_reserve.rs`.
//!
//! Los ficheros de entrada se construyen aquí, a mano, con `escritor.rs` (un escritor y un lector
//! mínimos hechos SOLO con las tablas de la especificación pública del formato); los datos de
//! `golden/aseprite/` se leen como datos. Los valores esperados salen de la spec y de las tablas
//! del formato, no del código que se prueba.
//!
//! Este fichero es la fase de pruebas en rojo: `read_aseprite` y compañía no existen todavía.
//! Necesita además `miniz_oxide = "0.8.9"` en `[dependencies]` de `px-formats` (zlib).

// Los patrones de variantes llevan `{ .. }` aunque hoy sean variantes unitarias: así la prueba no
// depende de si la lógica les añade campos (la API de la spec puede crecer sin romperla).
#![allow(clippy::unneeded_struct_pattern)]

use std::fmt::Display;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    BlendMode, Document, DocumentError, Image, Layer, LayerSpec, Mode, Palette, Pixel, Rgba,
};
use px_formats::{
    AsepriteError, AsepriteLimits, AsepriteRead, FileFormat, Ignored, Unsupported,
    aseprite_dimensions, detect_format, read_aseprite, read_png, write_aseprite, write_png,
};

#[path = "aseprite/escritor.rs"]
#[allow(dead_code)]
mod escritor;
use escritor::*;

// ---------------------------------------------------------------------------------------
// Ayudantes: corpus, lectura, documentos y la igualdad por contenido (condición 1).
// ---------------------------------------------------------------------------------------

/// Un fichero de `golden/aseprite/` (datos, no código).
fn golden(nombre: &str) -> Vec<u8> {
    let ruta = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../golden/aseprite")
        .join(nombre);
    fs::read(&ruta).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", ruta.display()))
}

fn lee(bytes: &[u8]) -> Result<AsepriteRead, AsepriteError> {
    read_aseprite(bytes, &AsepriteLimits::default())
}

fn lee_ok(contexto: &str, bytes: &[u8]) -> AsepriteRead {
    match lee(bytes) {
        Ok(r) => r,
        Err(e) => panic!("{contexto}: se esperaba Ok y salió {e:?}"),
    }
}

fn lee_err(contexto: &str, bytes: &[u8]) -> AsepriteError {
    match lee(bytes) {
        Ok(_) => panic!("{contexto}: se esperaba un error y salió Ok"),
        Err(e) => e,
    }
}

fn lee_corpus(caso: &str) -> AsepriteRead {
    lee_ok(caso, &golden(&format!("{caso}.aseprite")))
}

/// El `Unsupported` de un fichero que no se admite.
fn no_soportado(contexto: &str, bytes: &[u8]) -> Unsupported {
    match lee_err(contexto, bytes) {
        AsepriteError::Unsupported(u) => u,
        otro => panic!("{contexto}: se esperaba Unsupported y salió {otro:?}"),
    }
}

/// Cuántos elementos de `ignored` cumplen `es`.
fn cuenta(r: &AsepriteRead, es: fn(&Ignored) -> bool) -> u32 {
    r.report
        .ignored
        .iter()
        .filter(|(k, _)| es(k))
        .map(|(_, n)| *n)
        .sum()
}

fn imagen_rgba(ancho: u32, alto: u32, bytes: Vec<u8>) -> Image {
    Image::from_bytes(ancho, alto, Mode::Rgba, bytes).expect("imagen RGBA de la prueba")
}

fn paleta_de(colores: &[[u8; 4]], transparente: Option<u8>) -> Palette {
    let mut paleta =
        Palette::from_colors(colores.iter().map(|c| Rgba::new(c[0], c[1], c[2], c[3])))
            .expect("paleta de la prueba");
    paleta
        .set_transparent_index(transparente)
        .expect("índice transparente de la prueba");
    paleta
}

fn imagen_indexada(ancho: u32, alto: u32, paleta: &Palette, indices: Vec<u8>) -> Image {
    Image::from_bytes_indexed(ancho, alto, paleta.clone(), indices)
        .expect("imagen indexada de la prueba")
}

fn documento(
    ancho: u32,
    alto: u32,
    modo: Mode,
    paleta: Option<Palette>,
    capas: Vec<LayerSpec>,
) -> Document {
    Document::from_layers(ancho, alto, modo, paleta, capas).expect("documento de la prueba")
}

fn capa(
    nombre: &str,
    visible: bool,
    opacity: u8,
    blend: BlendMode,
    locked: bool,
    image: Option<Image>,
) -> LayerSpec {
    LayerSpec {
        name: nombre.to_string(),
        visible,
        opacity,
        blend,
        locked,
        image,
    }
}

/// Una capa visible, desbloqueada, opaca y Normal.
fn capa_simple(nombre: &str, image: Option<Image>) -> LayerSpec {
    capa(nombre, true, 255, BlendMode::Normal, false, image)
}

/// Lo que compara «escribir y leer devuelve el mismo documento» (condición 1): sin ids.
#[derive(Debug, PartialEq)]
enum Valor {
    Transparente,
    Pixel(Pixel),
}

/// El valor de un píxel de una capa: la celda vacía es transparente; «transparente» es alfa 0 en
/// RGBA y el índice transparente en indexado.
fn valor(doc: &Document, capa: &Layer, x: u32, y: u32) -> Valor {
    let Some(imagen) = doc.image(capa.cel()) else {
        return Valor::Transparente;
    };
    let Ok(pixel) = imagen.get_pixel(x, y) else {
        return Valor::Pixel(Pixel::Gray(0));
    };
    let transparente = match pixel {
        Pixel::Rgba(c) => c.a == 0,
        Pixel::Indexed(i) => doc.palette().and_then(Palette::transparent_index) == Some(i),
        Pixel::GrayAlpha { alpha, .. } => alpha == 0,
        _ => false,
    };
    if transparente {
        Valor::Transparente
    } else {
        Valor::Pixel(pixel)
    }
}

/// La igualdad de ida y vuelta de la spec: mismas capas en el mismo orden con nombre,
/// visibilidad, opacidad, mezcla y bloqueo iguales, mismo lienzo, misma paleta (colores, alfa e
/// índice transparente) y mismo contenido de píxeles por capa (una celda vacía y una
/// materializada y totalmente transparente son iguales; dos píxeles transparentes son iguales
/// aunque difiera su RGB; los demás, byte a byte).
fn mismo_contenido(a: &Document, b: &Document) -> Result<(), String> {
    if (a.width(), a.height(), a.mode()) != (b.width(), b.height(), b.mode()) {
        return Err("lienzo distinto".to_string());
    }
    if a.palette() != b.palette() {
        return Err(format!(
            "paleta distinta: {:?} y {:?}",
            a.palette(),
            b.palette()
        ));
    }
    if a.layers().len() != b.layers().len() {
        return Err(format!(
            "{} capas y {} capas",
            a.layers().len(),
            b.layers().len()
        ));
    }
    for (i, (ca, cb)) in a.layers().iter().zip(b.layers()).enumerate() {
        let (pa, pb) = (
            (
                ca.name(),
                ca.visible(),
                ca.opacity(),
                ca.blend(),
                ca.locked(),
            ),
            (
                cb.name(),
                cb.visible(),
                cb.opacity(),
                cb.blend(),
                cb.locked(),
            ),
        );
        if pa != pb {
            return Err(format!("capa {i}: propiedades {pa:?} y {pb:?}"));
        }
        for y in 0..a.height() {
            for x in 0..a.width() {
                let (va, vb) = (valor(a, ca, x, y), valor(b, cb, x, y));
                if va != vb {
                    return Err(format!("capa {i}, píxel ({x}, {y}): {va:?} y {vb:?}"));
                }
            }
        }
    }
    Ok(())
}

/// Colocación de una celda de `cw × ch` píxeles con origen `(x, y)` en un lienzo (Spec: Cel
/// Chunk, «X position»/«Y position»): el píxel `(px, py)` del lienzo es el de la celda en
/// `(px − x, py − y)` si cae dentro y `relleno` si no. `None` si lo que cae dentro es todo
/// transparente (o no cae nada): la celda del modelo queda vacía.
fn colocacion_esperada(
    ancho: usize,
    alto: usize,
    (x, y): (i32, i32),
    (cw, ch): (usize, usize),
    datos: &[u8],
    relleno: &[u8],
    transparente: impl Fn(&[u8]) -> bool,
) -> Option<Vec<u8>> {
    let bpp = relleno.len();
    let mut salida = relleno.repeat(ancho * alto);
    let mut visible = false;
    for py in 0..alto {
        for px in 0..ancho {
            let (cx, cy) = (px as i32 - x, py as i32 - y);
            if cx < 0 || cy < 0 || cx as usize >= cw || cy as usize >= ch {
                continue;
            }
            let k = (cy as usize * cw + cx as usize) * bpp;
            let pixel = &datos[k..k + bpp];
            let d = (py * ancho + px) * bpp;
            salida[d..d + bpp].copy_from_slice(pixel);
            visible |= !transparente(pixel);
        }
    }
    visible.then_some(salida)
}

fn rgba_transparente(p: &[u8]) -> bool {
    p[3] == 0
}

// ---------------------------------------------------------------------------------------
// Ficheros base construidos a mano.
// ---------------------------------------------------------------------------------------

const PALETA4: [[u8; 4]; 4] = [
    [0, 0, 0, 255],
    [255, 0, 0, 255],
    [0, 255, 0, 255],
    [0, 0, 255, 255],
];

/// RGBA de 4 × 3, perfil sRGB, dos capas y dos celdas (la segunda, de 2 × 2 en (1, 1)).
fn base_rgba() -> Fichero {
    Fichero::rgba(
        4,
        3,
        vec![
            perfil_srgb(),
            Capa::nueva("fondo").chunk(),
            Capa::nueva("arriba").chunk(),
            Celda::nueva(
                0,
                0,
                0,
                4,
                3,
                pixeles_rgba(4, 3, |x, y| [10 * x as u8 + 1, 20 * y as u8 + 1, 200, 255]),
            )
            .chunk(),
            Celda::nueva(
                1,
                1,
                1,
                2,
                2,
                pixeles_rgba(2, 2, |x, y| [255, 0, 0, if x == y { 255 } else { 0 }]),
            )
            .chunk(),
        ],
    )
}

/// Indexado de 4 × 3 con la paleta de 4 colores (índice transparente 0), dos capas y dos celdas.
fn base_indexada() -> Fichero {
    Fichero::indexado(
        4,
        3,
        0,
        4,
        vec![
            perfil_srgb(),
            paleta_completa(&PALETA4),
            Capa::nueva("fondo").chunk(),
            Capa::nueva("arriba").chunk(),
            Celda::nueva(0, 0, 0, 4, 3, (0..12).map(|i| (i % 4) as u8).collect()).chunk(),
            Celda::nueva(1, 1, 1, 2, 2, vec![1, 0, 2, 3]).chunk(),
        ],
    )
}

/// Las capas de `base_rgba` con `extra` añadido al final del primer fotograma.
fn base_con(extra: Vec<u8>) -> Vec<u8> {
    let mut f = base_rgba();
    f.chunks.push(extra);
    f.bytes()
}

// ---------------------------------------------------------------------------------------
// Fila 2: la estructura de los casos del corpus.
// ---------------------------------------------------------------------------------------

/// Nombre del caso `mezcla-*` y el modo de su capa `fuente` (tabla de Spec: Layer Chunk).
const MODOS_DEL_CORPUS: [(&str, BlendMode); 8] = [
    ("normal", BlendMode::Normal),
    ("multiplicar", BlendMode::Multiply),
    ("trama", BlendMode::Screen),
    ("superponer", BlendMode::Overlay),
    ("oscurecer", BlendMode::Darken),
    ("aclarar", BlendMode::Lighten),
    ("sumar", BlendMode::Add),
    ("diferencia", BlendMode::Difference),
];

#[test]
fn fila_02_a_capas_opacidad_tiene_seis_capas_con_sus_opacidades() {
    let r = lee_corpus("capas-opacidad");
    let doc = &r.document;
    assert_eq!(
        (doc.width(), doc.height(), doc.mode()),
        (32, 16, Mode::Rgba)
    );
    assert!(doc.palette().is_none());
    let esperadas = [
        ("fondo", 255u8),
        ("op-0", 0),
        ("op-64", 64),
        ("op-128", 128),
        ("op-200", 200),
        ("op-255", 255),
    ];
    assert_eq!(doc.layers().len(), 6);
    for (i, (nombre, opacidad)) in esperadas.iter().enumerate() {
        let capa = &doc.layers()[i];
        assert_eq!(capa.name(), *nombre, "nombre de la capa {i}");
        assert_eq!(capa.opacity(), *opacidad, "opacidad de {nombre}");
        assert!(capa.visible() && !capa.locked(), "{nombre}");
        assert_eq!(capa.blend(), BlendMode::Normal, "{nombre}");
        // Los ids son 0..n (el fichero no los guarda).
        assert_eq!(capa.id().raw() as usize, i);
        assert!(doc.image(capa.cel()).is_some(), "{nombre} tiene celda");
    }
    assert_eq!(r.report.frames_in_file, 1);
    assert!(!r.report.lossy, "el corpus no se descarta nada");

    // `detect_format` mira el contenido, no la extensión.
    let bytes = golden("capas-opacidad.aseprite");
    assert!(matches!(detect_format(&bytes), Some(FileFormat::Aseprite)));
    let png = write_png(&Image::new(2, 2, Mode::Rgba).expect("imagen")).expect("png");
    assert!(matches!(detect_format(&png), Some(FileFormat::Png)));
    assert!(detect_format(&[1, 2, 3]).is_none());
    assert_eq!(aseprite_dimensions(&bytes).ok(), Some((32, 16)));
}

#[test]
fn fila_02_b_los_ocho_mezcla_tienen_el_modo_de_su_capa_fuente() {
    for (nombre, modo) in MODOS_DEL_CORPUS {
        let caso = format!("mezcla-{nombre}");
        let r = lee_corpus(&caso);
        let capas = r.document.layers();
        assert_eq!(capas.len(), 2, "{caso}");
        assert_eq!(
            (capas[0].name(), capas[0].blend()),
            ("base", BlendMode::Normal)
        );
        assert_eq!(
            (capas[1].name(), capas[1].blend()),
            ("fuente", modo),
            "{caso}"
        );
        assert_eq!(
            (capas[1].opacity(), capas[1].visible()),
            (255, true),
            "{caso}"
        );
    }
    // Las ocho tiras, una capa por modo y todas con opacidad 128.
    let r = lee_corpus("mezcla-ocho-opacidad");
    let capas = r.document.layers();
    assert_eq!(capas.len(), 9);
    assert_eq!((capas[0].name(), capas[0].opacity()), ("base", 255));
    for (i, (nombre, modo)) in MODOS_DEL_CORPUS.iter().enumerate() {
        let capa = &capas[i + 1];
        assert_eq!(capa.name(), format!("m-{nombre}"));
        assert_eq!((capa.blend(), capa.opacity()), (*modo, 128), "{nombre}");
    }
}

#[test]
fn fila_02_c_capas_ocultas_y_capa_bloqueada_son_independientes() {
    let r = lee_corpus("capas-ocultas");
    let capas = r.document.layers();
    let datos: Vec<(&str, bool)> = capas.iter().map(|c| (c.name(), c.visible())).collect();
    assert_eq!(
        datos,
        [
            ("fondo", true),
            ("oculta-1", false),
            ("visible-1", true),
            ("oculta-2", false),
            ("visible-2", true)
        ]
    );
    assert_eq!(
        (capas[3].blend(), capas[3].opacity()),
        (BlendMode::Multiply, 128)
    );
    assert!(capas.iter().all(|c| !c.locked()), "ninguna está bloqueada");

    // Bit 1 = visible y bit 2 = editable (bloqueada = bit 2 apagado): banderas 3, 1, 3 y 0.
    let r = lee_corpus("capa-bloqueada");
    let datos: Vec<(&str, bool, bool)> = r
        .document
        .layers()
        .iter()
        .map(|c| (c.name(), c.visible(), c.locked()))
        .collect();
    assert_eq!(
        datos,
        [
            ("fondo", true, false),
            ("bloqueada", true, true),
            ("arriba", true, false),
            ("oculta-bloqueada", false, true)
        ]
    );
}

#[test]
fn fila_02_d_celda_vacia_e_indexado_con_transparente_7() {
    let r = lee_corpus("celda-vacia");
    let doc = &r.document;
    let capas = doc.layers();
    assert_eq!(capas.len(), 3);
    assert!(
        doc.image(capas[0].cel()).is_some(),
        "con-celda tiene imagen"
    );
    // `sin-celda` conserva sus propiedades aunque no haya celda, y `transparente` (una celda con
    // alfa 0 en todo) equivale a una vacía: sin imagen.
    assert_eq!(capas[1].name(), "sin-celda");
    assert_eq!(
        (capas[1].blend(), capas[1].opacity()),
        (BlendMode::Multiply, 100)
    );
    assert!(doc.image(capas[1].cel()).is_none());
    assert_eq!(capas[2].name(), "transparente");
    assert!(doc.image(capas[2].cel()).is_none());

    let r = lee_corpus("indexado-transparente-7");
    let doc = &r.document;
    assert_eq!(doc.mode(), Mode::Indexed);
    let paleta = doc.palette().expect("el indexado tiene paleta");
    assert_eq!(paleta.len(), 16);
    assert_eq!(paleta.transparent_index(), Some(7));
    assert_eq!(doc.layers().len(), 2);
}

// ---------------------------------------------------------------------------------------
// Fila 4: paleta.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_04_a_paleta_256_del_corpus_y_chunk_nuevo_con_alfa_nombres_y_rangos() {
    // El corpus: la paleta de 256 entradas coincide con la de su PNG, con alfa e índice.
    let r = lee_corpus("indexado-paleta-256");
    let paleta = r.document.palette().expect("paleta");
    let png = read_png(&golden("indexado-paleta-256.png"))
        .expect("png del corpus")
        .image;
    let paleta_png = png.palette().expect("el PNG es indexado");
    assert_eq!((paleta.len(), paleta_png.len()), (256, 256));
    for i in 0..=255u8 {
        assert_eq!(
            paleta.resolved(i).ok(),
            paleta_png.resolved(i).ok(),
            "entrada {i}"
        );
    }
    assert_eq!(paleta.transparent_index(), Some(0));

    // Chunk 0x2019: alfa propio, nombres que se saltan y dos rangos; entradas sin cubrir en
    // (0, 0, 0, 255). El chunk 0x0004 que va antes se ignora (gana el nuevo).
    let colores_antiguos = vec![[9u8, 9, 9]; 8];
    let f = Fichero::indexado(
        4,
        3,
        1,
        8,
        vec![
            perfil_srgb(),
            paleta_antigua(CHUNK_PALETA_ANTIGUA, &[(0, 8, colores_antiguos)]),
            paleta_nueva(
                8,
                0,
                3,
                &[
                    ([255, 0, 0, 255], None),
                    ([0, 255, 0, 128], Some("verde")),
                    ([0, 0, 255, 255], None),
                    ([10, 20, 30, 40], Some("x")),
                ],
            ),
            paleta_nueva(8, 6, 7, &[([1, 2, 3, 4], None), ([5, 6, 7, 8], Some("n"))]),
            Capa::nueva("a").chunk(),
            Celda::nueva(0, 0, 0, 4, 3, (0..12).map(|i| (i % 8) as u8).collect()).chunk(),
        ],
    );
    let r = lee_ok("0x2019", &f.bytes());
    let paleta = r.document.palette().expect("paleta");
    let esperados = [
        Rgba::new(255, 0, 0, 255),
        Rgba::new(0, 255, 0, 128),
        Rgba::new(0, 0, 255, 255),
        Rgba::new(10, 20, 30, 40),
        Rgba::new(0, 0, 0, 255),
        Rgba::new(0, 0, 0, 255),
        Rgba::new(1, 2, 3, 4),
        Rgba::new(5, 6, 7, 8),
    ];
    assert_eq!(paleta.colors(), esperados);
    assert_eq!(paleta.transparent_index(), Some(1));
}

#[test]
fn fila_04_b_paleta_antigua_con_saltos_y_cero_es_256() {
    let a = [10u8, 11, 12];
    let b = [20u8, 21, 22];
    let c = [30u8, 31, 32];
    let fichero = |paquetes: Vec<(u8, u8, Vec<[u8; 3]>)>, colores: u16, transparente: u8| {
        Fichero::indexado(
            4,
            3,
            transparente,
            colores,
            vec![
                perfil_srgb(),
                paleta_antigua(CHUNK_PALETA_ANTIGUA, &paquetes),
                Capa::nueva("a").chunk(),
                Celda::nueva(0, 0, 0, 4, 3, vec![0; 12]).chunk(),
            ],
        )
        .bytes()
    };
    // «Saltar» cuenta desde el final del paquete anterior: cubre 1 y 2, y después 5.
    let r = lee_ok(
        "saltos",
        &fichero(vec![(1, 2, vec![a, b]), (2, 1, vec![c])], 8, 5),
    );
    let paleta = r.document.palette().expect("paleta");
    let defecto = Rgba::new(0, 0, 0, 255);
    let opaco = |t: [u8; 3]| Rgba::new(t[0], t[1], t[2], 255);
    assert_eq!(
        paleta.colors(),
        [
            defecto,
            opaco(a),
            opaco(b),
            defecto,
            defecto,
            opaco(c),
            defecto,
            defecto
        ]
    );
    assert_eq!(paleta.transparent_index(), Some(5));

    // Número de colores 0 en la cabecera y 0 colores en el paquete: 256 entradas.
    let todos: Vec<[u8; 3]> = (0..=255u8).map(|k| [k, 255 - k, 7]).collect();
    let r = lee_ok("256", &fichero(vec![(0, 0, todos)], 0, 255));
    let paleta = r.document.palette().expect("paleta");
    assert_eq!(paleta.len(), 256);
    assert_eq!(paleta.colors()[255], Rgba::new(255, 0, 7, 255));
    assert_eq!(paleta.transparent_index(), Some(255));

    // Un índice transparente fuera de la longitud no es ninguno.
    let r = lee_ok(
        "sin transparente",
        &fichero(vec![(0, 8, vec![a; 8])], 8, 200),
    );
    assert_eq!(
        r.document.palette().and_then(Palette::transparent_index),
        None
    );

    // Cubrir más allá de la longitud es un error.
    lee_err("fuera", &fichero(vec![(3, 2, vec![a, b])], 4, 0));
}

#[test]
fn fila_04_c_sin_paleta_solo_de_6_bits_y_paleta_en_rgba() {
    // Indexado sin ningún chunk de paleta.
    let f = Fichero::indexado(
        4,
        3,
        0,
        4,
        vec![
            perfil_srgb(),
            Capa::nueva("a").chunk(),
            Celda::nueva(0, 0, 0, 4, 3, vec![0; 12]).chunk(),
        ],
    );
    assert!(matches!(
        lee_err("sin paleta", &f.bytes()),
        AsepriteError::NoPalette
    ));

    // Solo el chunk 0x0011 (componentes de 0 a 63): no se inventa la escala.
    let f = Fichero::indexado(
        4,
        3,
        0,
        4,
        vec![
            perfil_srgb(),
            paleta_antigua(CHUNK_PALETA_ANTIGUA_6, &[(0, 4, vec![[1, 2, 3]; 4])]),
            Capa::nueva("a").chunk(),
            Celda::nueva(0, 0, 0, 4, 3, vec![0; 12]).chunk(),
        ],
    );
    let u = no_soportado("0x0011", &f.bytes());
    assert!(matches!(u, Unsupported::Palette6Bit { .. }), "{u:?}");

    // RGBA con un chunk de paleta: se ignora y se anota, sin `lossy`.
    let mut f = base_rgba();
    f.chunks.insert(1, paleta_completa(&PALETA4));
    let r = lee_ok("paleta en RGBA", &f.bytes());
    assert_eq!(cuenta(&r, |k| matches!(k, Ignored::RgbaPalette { .. })), 1);
    assert!(!r.report.lossy);
    assert!(r.document.palette().is_none());
}

// ---------------------------------------------------------------------------------------
// Fila 5: lo que se ignora.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_05_a_cada_chunk_descartable_se_anota_y_avisa() {
    #[allow(clippy::type_complexity)]
    let casos: Vec<(&str, Vec<u8>, fn(&Ignored) -> bool)> = vec![
        ("etiquetas", etiquetas(), |k| {
            matches!(k, Ignored::Tags { .. })
        }),
        ("datos de usuario", datos_usuario(), |k| {
            matches!(k, Ignored::UserData { .. })
        }),
        ("cortes", cortes(), |k| matches!(k, Ignored::Slices { .. })),
        ("ficheros externos", ficheros_externos(), |k| {
            matches!(k, Ignored::ExternalFiles { .. })
        }),
        ("extra de celda", celda_extra(), |k| {
            matches!(k, Ignored::CelExtra { .. })
        }),
        ("máscara", mascara(), |k| matches!(k, Ignored::Mask { .. })),
        ("trazo", trazo(), |k| matches!(k, Ignored::Path { .. })),
        ("conjunto de tiles", tileset(), |k| {
            matches!(k, Ignored::Tileset { .. })
        }),
        ("tipo desconocido", desconocido(0x7777), |k| {
            matches!(k, Ignored::Unknown { .. })
        }),
        ("perfil ICC", perfil(2, 0, 0, &[1, 2, 3, 4]), |k| {
            matches!(k, Ignored::ColorProfile { .. })
        }),
        ("gamma fijo", perfil(1, 1, 0x1_0000, &[]), |k| {
            matches!(k, Ignored::ColorProfile { .. })
        }),
    ];
    for (nombre, extra, es) in casos {
        let r = lee_ok(nombre, &base_con(extra));
        assert!(r.report.lossy, "{nombre}: debe avisar (lossy)");
        assert_eq!(cuenta(&r, es), 1, "{nombre}: una vez en `ignored`");
        assert_eq!(r.document.layers().len(), 2, "{nombre}: lo demás se lee");
    }

    // Cuentas: dos etiquetas, tres de usuario y dos tipos desconocidos distintos.
    let mut f = base_rgba();
    f.chunks.extend([
        etiquetas(),
        etiquetas(),
        datos_usuario(),
        datos_usuario(),
        datos_usuario(),
        desconocido(0x7777),
        desconocido(0x1234),
    ]);
    let r = lee_ok("cuentas", &f.bytes());
    assert_eq!(cuenta(&r, |k| matches!(k, Ignored::Tags { .. })), 2);
    assert_eq!(cuenta(&r, |k| matches!(k, Ignored::UserData { .. })), 3);
    assert_eq!(cuenta(&r, |k| matches!(k, Ignored::Unknown { .. })), 2);
    assert!(r.report.lossy);
}

#[test]
fn fila_05_b_lo_que_no_cambia_nada_no_avisa_y_el_fondo_indexado_si() {
    // sRGB (tipo 1 sin gamma) y «sin perfil» (tipo 0): nada.
    for perfil in [perfil_srgb(), perfil(0, 0, 0, &[])] {
        let mut f = base_rgba();
        f.chunks[0] = perfil;
        let r = lee_ok("perfil sin efecto", &f.bytes());
        assert!(!r.report.lossy);
        assert!(r.report.ignored.is_empty(), "{:?}", r.report.ignored);
        assert_eq!(r.report.frames_in_file, 1);
    }

    // Cabecera (velocidad, razón de píxel, cuadrícula), duración del fotograma y banderas de
    // capa 4, 16 y 32: se ignoran sin aviso.
    let mut f = base_rgba();
    f.chunks[1] = Capa {
        flags: CAPA_VISIBLE
            | CAPA_EDITABLE
            | CAPA_BLOQUEO_MOVIMIENTO
            | CAPA_ENLAZADAS
            | CAPA_COLAPSADA,
        ..Capa::nueva("fondo")
    }
    .chunk();
    let mut bytes = f.bytes();
    pon_u16(&mut bytes, OFF_VELOCIDAD, 7);
    bytes[OFF_RAZON_PIXEL] = 2;
    bytes[OFF_RAZON_PIXEL + 1] = 1;
    pon_u16(&mut bytes, OFF_REJILLA, 5);
    pon_u16(&mut bytes, OFF_REJILLA + 4, 8);
    pon_u16(&mut bytes, OFF_REJILLA + 6, 8);
    pon_u16(&mut bytes, OFF_FOT_DURACION, 250);
    let r = lee_ok("campos ignorados", &bytes);
    assert!(!r.report.lossy, "{:?}", r.report.ignored);
    assert!(r.report.ignored.is_empty());
    assert_eq!(r.document.layers().len(), 2);

    // Capa de fondo (bandera 8): RGBA no avisa; indexado sin píxeles del índice transparente,
    // tampoco; indexado con ellos, sí.
    let fondo = Capa {
        flags: CAPA_VISIBLE | CAPA_EDITABLE | CAPA_FONDO,
        ..Capa::nueva("fondo")
    }
    .chunk();
    let mut f = base_rgba();
    f.chunks[1] = fondo.clone();
    assert!(!lee_ok("fondo RGBA", &f.bytes()).report.lossy);
    let indexado = |indices: Vec<u8>| {
        Fichero::indexado(
            4,
            3,
            0,
            4,
            vec![
                perfil_srgb(),
                paleta_completa(&PALETA4),
                fondo.clone(),
                Celda::nueva(0, 0, 0, 4, 3, indices).chunk(),
            ],
        )
        .bytes()
    };
    let r = lee_ok(
        "fondo sin transparentes",
        &indexado(vec![1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3]),
    );
    assert!(!r.report.lossy);
    let r = lee_ok(
        "fondo con transparentes",
        &indexado(vec![0, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3]),
    );
    assert!(
        r.report.lossy,
        "el índice transparente se vería opaco en un fondo"
    );
    assert!(cuenta(&r, |k| matches!(k, Ignored::BackgroundTransparent { .. })) >= 1);
}

// ---------------------------------------------------------------------------------------
// Fila 6: lo que no se entiende.
// ---------------------------------------------------------------------------------------

const NO_MODIFICADO: &str = "el fichero no se ha modificado";

#[test]
fn fila_06_a_lo_no_soportado_da_unsupported_con_su_motivo() {
    let rgba = |chunks: Vec<Vec<u8>>| Fichero::rgba(4, 3, chunks).bytes();
    let celda = |c: Celda| rgba(vec![perfil_srgb(), Capa::nueva("a").chunk(), c.chunk()]);
    let base = Celda::nueva(0, 0, 0, 4, 3, vec![255; 48]);
    #[allow(clippy::type_complexity)]
    let casos: Vec<(&str, Vec<u8>, fn(&Unsupported) -> bool)> = vec![
        (
            "capa de grupo",
            rgba(vec![
                Capa {
                    tipo: CAPA_TIPO_GRUPO,
                    ..Capa::nueva("g")
                }
                .chunk(),
            ]),
            |u| matches!(u, Unsupported::Group { .. }),
        ),
        (
            "capa de mapa de tiles",
            rgba(vec![
                Capa {
                    tipo: CAPA_TIPO_MAPA_TILES,
                    tileset: Some(0),
                    ..Capa::nueva("t")
                }
                .chunk(),
            ]),
            |u| matches!(u, Unsupported::TilemapLayer { .. }),
        ),
        (
            "capa de referencia",
            rgba(vec![
                Capa {
                    flags: CAPA_VISIBLE | CAPA_EDITABLE | CAPA_REFERENCIA,
                    ..Capa::nueva("r")
                }
                .chunk(),
            ]),
            |u| matches!(u, Unsupported::ReferenceLayer { .. }),
        ),
        (
            "celda enlazada",
            celda(Celda {
                tipo: CELDA_ENLAZADA,
                ..base.clone()
            }),
            |u| matches!(u, Unsupported::LinkedCel { .. }),
        ),
        (
            "celda de mapa de tiles",
            celda(Celda {
                tipo: CELDA_MAPA_TILES,
                pixeles: vec![0; 16],
                ..base.clone()
            }),
            |u| matches!(u, Unsupported::TilemapCel { .. }),
        ),
        (
            "opacidad de celda",
            celda(Celda {
                opacidad: 128,
                ..base.clone()
            }),
            |u| matches!(u, Unsupported::CelOpacity { .. }),
        ),
        (
            "z-index de celda",
            celda(Celda {
                z: 1,
                ..base.clone()
            }),
            |u| matches!(u, Unsupported::CelZIndex { .. }),
        ),
        (
            "grises de 16 bits",
            Fichero {
                profundidad: PROFUNDIDAD_GRISES,
                ..Fichero::rgba(
                    4,
                    3,
                    vec![
                        perfil_srgb(),
                        Capa::nueva("a").chunk(),
                        Celda::nueva(0, 0, 0, 4, 3, vec![255; 24]).chunk(),
                    ],
                )
            }
            .bytes(),
            |u| matches!(u, Unsupported::Grayscale { .. }),
        ),
    ];
    for (nombre, bytes, es) in casos {
        let u = no_soportado(nombre, &bytes);
        assert!(es(&u), "{nombre}: salió {u:?}");
        let error = AsepriteError::Unsupported(u);
        assert!(
            error.to_string().contains(NO_MODIFICADO),
            "{nombre}: el mensaje debe decir que el fichero no se ha modificado: {error}"
        );
    }
}

#[test]
fn fila_06_b_capa_indexada_con_opacidad_o_mezcla_es_un_error() {
    let indexado = |segunda: Capa| {
        Fichero::indexado(
            4,
            3,
            0,
            4,
            vec![
                perfil_srgb(),
                paleta_completa(&PALETA4),
                Capa::nueva("a").chunk(),
                segunda.chunk(),
                Celda::nueva(0, 0, 0, 4, 3, vec![1; 12]).chunk(),
                Celda::nueva(1, 0, 0, 4, 3, vec![2; 12]).chunk(),
            ],
        )
        .bytes()
    };
    let con_opacidad = indexado(Capa {
        opacidad: 128,
        ..Capa::nueva("b")
    });
    let e = lee_err("opacidad", &con_opacidad);
    assert!(
        matches!(e, AsepriteError::IndexedLayer { layer: 1 }),
        "{e:?}"
    );
    let con_mezcla = indexado(Capa {
        mezcla: MEZCLA_MULTIPLICAR,
        ..Capa::nueva("b")
    });
    let e = lee_err("mezcla", &con_mezcla);
    assert!(
        matches!(e, AsepriteError::IndexedLayer { layer: 1 }),
        "{e:?}"
    );
    // Con opacidad 255 y Normal, bien.
    lee_ok("normal", &indexado(Capa::nueva("b")));
}

#[test]
fn fila_06_c_los_codigos_de_mezcla_de_0_a_40_siguen_la_tabla() {
    // Spec: Layer Chunk, tabla «Blend mode».
    let valido = |codigo: u16| match codigo {
        MEZCLA_NORMAL => Some(BlendMode::Normal),
        MEZCLA_MULTIPLICAR => Some(BlendMode::Multiply),
        MEZCLA_TRAMA => Some(BlendMode::Screen),
        MEZCLA_SUPERPONER => Some(BlendMode::Overlay),
        MEZCLA_OSCURECER => Some(BlendMode::Darken),
        MEZCLA_ACLARAR => Some(BlendMode::Lighten),
        MEZCLA_DIFERENCIA => Some(BlendMode::Difference),
        MEZCLA_SUMA => Some(BlendMode::Add),
        _ => None,
    };
    let mut aceptados = 0;
    for codigo in 0..=40u16 {
        let bytes = Fichero::rgba(
            4,
            3,
            vec![
                perfil_srgb(),
                Capa {
                    mezcla: codigo,
                    ..Capa::nueva("m")
                }
                .chunk(),
                Celda::nueva(0, 0, 0, 4, 3, vec![255; 48]).chunk(),
            ],
        )
        .bytes();
        match valido(codigo) {
            Some(modo) => {
                let r = lee_ok(&format!("código {codigo}"), &bytes);
                assert_eq!(r.document.layers()[0].blend(), modo, "código {codigo}");
                aceptados += 1;
            }
            None => match no_soportado(&format!("código {codigo}"), &bytes) {
                Unsupported::BlendMode { code, .. } => assert_eq!(code, codigo),
                otro => panic!("código {codigo}: salió {otro:?}"),
            },
        }
    }
    assert_eq!(aceptados, 8);
}

// ---------------------------------------------------------------------------------------
// Fila 7: fotogramas.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_07_a_con_tres_fotogramas_se_lee_el_primero_y_se_avisa() {
    let mut f = base_rgba();
    f.fotogramas = 3;
    // Detrás del primero, basura: no se mira (ni se recorren sus cabeceras).
    f.resto = (0..90u8).map(|i| i.wrapping_mul(37) ^ 0x5C).collect();
    let r = lee_ok("3 fotogramas", &f.bytes());
    assert_eq!(r.report.frames_in_file, 3);
    assert!(r.report.lossy);
    assert_eq!(r.document.layers().len(), 2);
    let una = lee_ok("1 fotograma", &base_rgba().bytes());
    assert_eq!(una.report.frames_in_file, 1);
    assert!(mismo_contenido(&r.document, &una.document).is_ok());
}

#[test]
fn fila_07_b_cero_fotogramas_y_bytes_de_mas_son_malformados() {
    let mut f = base_rgba();
    f.fotogramas = 0;
    let e = lee_err("0 fotogramas", &f.bytes());
    assert!(matches!(e, AsepriteError::Malformed(_)), "{e:?}");

    // Un fotograma declarado, pero el fichero (con su tamaño al día) trae bytes detrás.
    let mut f = base_rgba();
    f.resto = vec![0; 16];
    let e = lee_err("bytes de más", &f.bytes());
    assert!(matches!(e, AsepriteError::Malformed(_)), "{e:?}");
}

// ---------------------------------------------------------------------------------------
// Fila 8: celdas (la parte con el corpus está en px-golden).
// ---------------------------------------------------------------------------------------

#[test]
fn fila_08_celdas_de_tipos_0_y_2_desplazadas_recortadas_y_fuera() {
    let (ancho, alto) = (6usize, 5usize);
    // Píxeles de una celda: color distinto por píxel; alfa 0 en uno de cada tres.
    let datos = |cw: usize, ch: usize| {
        pixeles_rgba(cw, ch, |x, y| {
            let k = (y * cw + x) as u8;
            [
                k.wrapping_add(1),
                50,
                60,
                if k.is_multiple_of(3) { 0 } else { 255 },
            ]
        })
    };
    let opaca = |cw: usize, ch: usize| pixeles_rgba(cw, ch, |x, _| [x as u8 + 1, 7, 8, 255]);
    let transparente = |cw: usize, ch: usize| pixeles_rgba(cw, ch, |x, _| [x as u8 + 9, 7, 8, 0]);
    // (nombre, tipo, x, y, ancho, alto, píxeles, ¿queda vacía?)
    #[allow(clippy::type_complexity)]
    let casos: Vec<(&str, u16, i16, i16, usize, usize, Vec<u8>, bool)> = vec![
        (
            "desplazada comprimida",
            CELDA_COMPRIMIDA,
            1,
            1,
            3,
            2,
            datos(3, 2),
            false,
        ),
        (
            "desplazada cruda",
            CELDA_CRUDA,
            1,
            1,
            3,
            2,
            datos(3, 2),
            false,
        ),
        (
            "recorte arriba-izquierda",
            CELDA_COMPRIMIDA,
            -2,
            -1,
            4,
            4,
            opaca(4, 4),
            false,
        ),
        (
            "recorte abajo-derecha",
            CELDA_COMPRIMIDA,
            4,
            3,
            4,
            4,
            opaca(4, 4),
            false,
        ),
        (
            "recorte por los cuatro lados",
            CELDA_COMPRIMIDA,
            -2,
            -2,
            10,
            9,
            datos(10, 9),
            false,
        ),
        (
            "de 0 píxeles",
            CELDA_COMPRIMIDA,
            1,
            1,
            0,
            0,
            Vec::new(),
            true,
        ),
        (
            "del todo fuera por la derecha",
            CELDA_COMPRIMIDA,
            10,
            10,
            3,
            3,
            opaca(3, 3),
            true,
        ),
        (
            "del todo fuera por la izquierda",
            CELDA_COMPRIMIDA,
            -5,
            -5,
            3,
            3,
            opaca(3, 3),
            true,
        ),
        (
            "toda transparente",
            CELDA_COMPRIMIDA,
            0,
            0,
            6,
            5,
            transparente(6, 5),
            true,
        ),
        // Lo único opaco queda fuera: dentro del lienzo todo es transparente.
        (
            "transparente dentro y opaca fuera",
            CELDA_COMPRIMIDA,
            -3,
            0,
            4,
            1,
            [opaca(3, 1), transparente(1, 1)].concat(),
            true,
        ),
    ];
    let mut chunks = vec![perfil_srgb()];
    for (nombre, ..) in &casos {
        chunks.push(Capa::nueva(nombre).chunk());
    }
    for (i, (_, tipo, x, y, cw, ch, pixeles, _)) in casos.iter().enumerate() {
        let celda = Celda {
            tipo: *tipo,
            ..Celda::nueva(i as u16, *x, *y, *cw as u16, *ch as u16, pixeles.clone())
        };
        chunks.push(celda.chunk());
    }
    let r = lee_ok(
        "celdas",
        &Fichero::rgba(ancho as u16, alto as u16, chunks).bytes(),
    );
    let doc = &r.document;
    assert_eq!(doc.layers().len(), casos.len());
    for (i, (nombre, _, x, y, cw, ch, pixeles, vacia)) in casos.iter().enumerate() {
        let esperada = colocacion_esperada(
            ancho,
            alto,
            (i32::from(*x), i32::from(*y)),
            (*cw, *ch),
            pixeles,
            &[0, 0, 0, 0],
            rgba_transparente,
        );
        assert_eq!(
            esperada.is_none(),
            *vacia,
            "{nombre}: la tabla de la prueba"
        );
        let imagen = doc.image(doc.layers()[i].cel());
        assert_eq!(imagen.map(Image::bytes), esperada.as_deref(), "{nombre}");
    }
    // Los tipos 0 y 2 de la misma celda dan la misma imagen.
    let (a, b) = (doc.layers()[0].cel(), doc.layers()[1].cel());
    assert_eq!(doc.image(a), doc.image(b));
}

// ---------------------------------------------------------------------------------------
// Fila 9: el lector no se fía.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_09_a_cada_prefijo_de_un_fichero_valido_da_error() {
    let ficheros = [
        ("RGBA a mano", base_rgba().bytes()),
        ("indexado a mano", base_indexada().bytes()),
        ("mixto-rgba", golden("mixto-rgba.aseprite")),
    ];
    for (nombre, bytes) in &ficheros {
        lee_ok(nombre, bytes);
        for n in 0..bytes.len() {
            assert!(
                lee(&bytes[..n]).is_err(),
                "{nombre}: el prefijo de {n} bytes (de {}) debe dar error",
                bytes.len()
            );
        }
    }
    // La cabecera sola, truncada: nunca pánico.
    for n in 0..12 {
        assert!(
            aseprite_dimensions(&golden("mixto-rgba.aseprite")[..n]).is_err(),
            "{n}"
        );
    }
}

#[test]
fn fila_09_b_cabecera_fotograma_chunks_capas_y_lienzo_mal() {
    let es_cualquiera: fn(&AsepriteError) -> bool = |_| true;
    let base = base_rgba().bytes();
    let parcheado = |f: &dyn Fn(&mut Vec<u8>)| {
        let mut b = base.clone();
        f(&mut b);
        b
    };
    #[allow(clippy::type_complexity)]
    let casos: Vec<(&str, Vec<u8>, fn(&AsepriteError) -> bool)> = vec![
        (
            "tamaño de cabecera distinto",
            parcheado(&|b| {
                let largo = b.len() as u32;
                pon_u32(b, OFF_TAM_FICHERO, largo + 1);
            }),
            es_cualquiera,
        ),
        (
            "magia del fichero",
            parcheado(&|b| pon_u16(b, OFF_MAGIA, 0xA5E1)),
            |e| matches!(e, AsepriteError::NotAseprite | AsepriteError::Malformed(_)),
        ),
        (
            "magia del fotograma",
            parcheado(&|b| pon_u16(b, OFF_FOT_MAGIA, 0xF1FB)),
            es_cualquiera,
        ),
        (
            "ancho 0",
            parcheado(&|b| pon_u16(b, OFF_ANCHO, 0)),
            es_cualquiera,
        ),
        (
            "alto 0",
            parcheado(&|b| pon_u16(b, OFF_ALTO, 0)),
            es_cualquiera,
        ),
        (
            "profundidad de 24 bits",
            parcheado(&|b| pon_u16(b, OFF_PROFUNDIDAD, 24)),
            es_cualquiera,
        ),
        (
            "fotograma que sale del fichero",
            parcheado(&|b| {
                let largo = b.len() as u32;
                pon_u32(b, OFF_FOT_TAM, largo);
            }),
            es_cualquiera,
        ),
        (
            "65 537 chunks declarados con el fichero corto",
            parcheado(&|b| {
                pon_u16(b, OFF_FOT_CHUNKS_ANTIGUO, 0xFFFF);
                pon_u32(b, OFF_FOT_CHUNKS_NUEVO, 65_537);
            }),
            |e| matches!(e, AsepriteError::TooManyChunks),
        ),
        (
            "chunk de menos de 6 bytes",
            base_con(vec![5, 0, 0, 0, 0x19, 0x20]),
            es_cualquiera,
        ),
        (
            "chunk que sale del fotograma",
            base_con(vec![0xE8, 0x03, 0, 0, 0x19, 0x20, 1, 2, 3, 4]),
            es_cualquiera,
        ),
        (
            "257 capas",
            Fichero::rgba(
                4,
                3,
                (0..257)
                    .map(|i| Capa::nueva(&format!("c{i}")).chunk())
                    .collect(),
            )
            .bytes(),
            |e| matches!(e, AsepriteError::TooManyLayers),
        ),
        (
            "nombre de 256 bytes",
            Fichero::rgba(4, 3, vec![Capa::nueva(&"a".repeat(256)).chunk()]).bytes(),
            |e| matches!(e, AsepriteError::LayerName),
        ),
        (
            "nombre que no es UTF-8",
            Fichero::rgba(
                4,
                3,
                vec![
                    Capa {
                        nombre: vec![0xFF, 0xFE, 0x41],
                        ..Capa::nueva("x")
                    }
                    .chunk(),
                ],
            )
            .bytes(),
            es_cualquiera,
        ),
        (
            "capa con nivel hijo distinto de 0",
            Fichero::rgba(
                4,
                3,
                vec![
                    Capa {
                        nivel: 1,
                        ..Capa::nueva("x")
                    }
                    .chunk(),
                ],
            )
            .bytes(),
            es_cualquiera,
        ),
        (
            "sin ninguna capa",
            Fichero::rgba(4, 3, vec![perfil_srgb()]).bytes(),
            es_cualquiera,
        ),
        // `from_layers` es el único camino: su error sale como `Document(..)`, no como pánico.
        (
            "indexado con dos capas y sin índice transparente válido",
            Fichero::indexado(
                4,
                3,
                200,
                4,
                vec![
                    perfil_srgb(),
                    paleta_completa(&PALETA4),
                    Capa::nueva("a").chunk(),
                    Capa::nueva("b").chunk(),
                    Celda::nueva(0, 0, 0, 4, 3, vec![1; 12]).chunk(),
                ],
            )
            .bytes(),
            |e| {
                matches!(
                    e,
                    AsepriteError::Document(DocumentError::NoTransparency { .. })
                )
            },
        ),
    ];
    for (nombre, bytes, es) in casos {
        let e = lee_err(nombre, &bytes);
        assert!(es(&e), "{nombre}: salió {e:?}");
    }

    // Un lienzo de más de 2^26 píxeles: el error dice el límite y se rechaza antes de reservar.
    let grande = Fichero::rgba(8193, 8192, vec![Capa::nueva("x").chunk()]).bytes();
    let e = lee_err("lienzo grande", &grande);
    assert!(matches!(e, AsepriteError::TooLarge { .. }), "{e:?}");
    assert!(e.to_string().contains("67108864"), "{e}");
    assert_eq!(aseprite_dimensions(&grande).ok(), Some((8193, 8192)));
    // Y el límite que pasa el llamador manda.
    let pequeno = AsepriteLimits {
        max_pixels: 100,
        ..AsepriteLimits::default()
    };
    let bytes = Fichero::rgba(11, 10, vec![Capa::nueva("x").chunk()]).bytes();
    match read_aseprite(&bytes, &pequeno) {
        Err(e @ AsepriteError::TooLarge { .. }) => assert!(e.to_string().contains("100"), "{e}"),
        otro => panic!("lienzo sobre el límite del llamador: {:?}", otro.err()),
    }
}

#[test]
fn fila_09_c_celdas_mal_formadas() {
    let ok = || {
        Celda::nueva(
            0,
            0,
            0,
            4,
            3,
            pixeles_rgba(4, 3, |x, y| [x as u8, y as u8, 9, 255]),
        )
    };
    let con = |chunks: Vec<Vec<u8>>| {
        let mut v = vec![perfil_srgb(), Capa::nueva("a").chunk()];
        v.extend(chunks);
        Fichero::rgba(4, 3, v).bytes()
    };
    let zlib_ok = zlib(&ok().pixeles);
    let es_cel_data: fn(&AsepriteError) -> bool =
        |e| matches!(e, AsepriteError::CelData { layer: 0 });
    let es_cualquiera: fn(&AsepriteError) -> bool = |_| true;
    let mut adler = zlib_ok.clone();
    *adler.last_mut().expect("zlib no vacío") ^= 0xFF;
    #[allow(clippy::type_complexity)]
    let casos: Vec<(&str, Vec<u8>, fn(&AsepriteError) -> bool)> = vec![
        (
            "Adler-32 erróneo",
            con(vec![
                Celda {
                    comprimido: Some(adler),
                    ..ok()
                }
                .chunk(),
            ]),
            es_cel_data,
        ),
        (
            "zlib sin su Adler-32 (corto)",
            con(vec![
                Celda {
                    comprimido: Some(zlib_ok[..zlib_ok.len() - 4].to_vec()),
                    ..ok()
                }
                .chunk(),
            ]),
            es_cel_data,
        ),
        (
            "zlib cortado por la mitad",
            con(vec![
                Celda {
                    comprimido: Some(zlib_ok[..zlib_ok.len() / 2].to_vec()),
                    ..ok()
                }
                .chunk(),
            ]),
            es_cel_data,
        ),
        (
            "zlib con un byte de más (largo)",
            con(vec![
                Celda {
                    pixeles: vec![7; 49],
                    ..ok()
                }
                .chunk(),
            ]),
            es_cel_data,
        ),
        (
            "zlib con un byte de menos",
            con(vec![
                Celda {
                    pixeles: vec![7; 47],
                    ..ok()
                }
                .chunk(),
            ]),
            es_cel_data,
        ),
        (
            "1000 × 1000 con unos pocos bytes de zlib (más de 1032×)",
            con(vec![
                Celda {
                    ancho: 1000,
                    alto: 1000,
                    comprimido: Some(zlib(&[0])),
                    ..ok()
                }
                .chunk(),
            ]),
            es_cel_data,
        ),
        (
            "celda repetida",
            con(vec![ok().chunk(), ok().chunk()]),
            es_cualquiera,
        ),
        (
            "índice de capa fuera de las capas",
            con(vec![Celda { capa: 1, ..ok() }.chunk()]),
            es_cualquiera,
        ),
        (
            "celda cruda con un byte de menos",
            con(vec![
                Celda {
                    tipo: CELDA_CRUDA,
                    pixeles: vec![7; 47],
                    ..ok()
                }
                .chunk(),
            ]),
            es_cualquiera,
        ),
        (
            "celda cruda con un byte de más",
            con(vec![
                Celda {
                    tipo: CELDA_CRUDA,
                    pixeles: vec![7; 49],
                    ..ok()
                }
                .chunk(),
            ]),
            es_cualquiera,
        ),
        (
            "índice de píxel fuera de la paleta",
            Fichero::indexado(
                4,
                3,
                0,
                4,
                vec![
                    perfil_srgb(),
                    paleta_completa(&PALETA4),
                    Capa::nueva("a").chunk(),
                    Celda::nueva(0, 0, 0, 4, 3, vec![0, 1, 2, 3, 0, 1, 2, 4, 0, 1, 2, 3]).chunk(),
                ],
            )
            .bytes(),
            es_cualquiera,
        ),
    ];
    for (nombre, bytes, es) in casos {
        let e = lee_err(nombre, &bytes);
        assert!(es(&e), "{nombre}: salió {e:?}");
    }
}

/// Memoria: 8 capas con un píxel cada una en un lienzo de 8192 × 8192 son 8 celdas de 256 MiB
/// (2 GiB, lo que el núcleo deja crear) y se reabren; una novena pasa del tope y da `TooLarge`
/// con el límite en el mensaje. No es del CI.
#[test]
#[ignore = "reserva 2 GiB; se corre una vez al cerrar la unidad"]
fn fila_09_ocho_celdas_de_256_mib_se_reabren_y_la_novena_da_too_large() {
    let fichero = |capas: usize| {
        let mut chunks = vec![perfil_srgb()];
        chunks.extend((0..capas).map(|i| Capa::nueva(&format!("c{i}")).chunk()));
        chunks.extend(
            (0..capas).map(|i| Celda::nueva(i as u16, 0, 0, 1, 1, vec![1, 2, 3, 255]).chunk()),
        );
        Fichero::rgba(8192, 8192, chunks).bytes()
    };
    let r = lee_ok("8 capas", &fichero(8));
    assert_eq!(r.document.layers().len(), 8);
    assert_eq!(r.document.materialized_bytes(), 8 * 8192 * 8192 * 4);
    drop(r);
    let e = lee_err("9 capas", &fichero(9));
    assert!(
        matches!(&e, AsepriteError::TooLarge { limit } if *limit == 2u64 << 30),
        "{e:?}"
    );
    assert!(e.to_string().contains("2147483648"), "{e}");
}

// ---------------------------------------------------------------------------------------
// Fila 10: escritura, contra un lector mínimo escrito solo con las tablas.
// ---------------------------------------------------------------------------------------

fn codigo_mezcla(modo: BlendMode) -> u16 {
    match modo {
        BlendMode::Normal => MEZCLA_NORMAL,
        BlendMode::Multiply => MEZCLA_MULTIPLICAR,
        BlendMode::Screen => MEZCLA_TRAMA,
        BlendMode::Overlay => MEZCLA_SUPERPONER,
        BlendMode::Darken => MEZCLA_OSCURECER,
        BlendMode::Lighten => MEZCLA_ACLARAR,
        BlendMode::Difference => MEZCLA_DIFERENCIA,
        BlendMode::Add => MEZCLA_SUMA,
        _ => panic!("modo de mezcla sin código en la prueba"),
    }
}

/// Rectángulo mínimo `(x0, y0, x1, y1)` (extremo final excluido) de los píxeles no transparentes.
fn rectangulo_minimo(imagen: &Image, transparente: impl Fn(&[u8]) -> bool) -> Option<[usize; 4]> {
    let (ancho, bpp) = (imagen.width() as usize, imagen.mode().bytes_per_pixel());
    let mut rect: Option<[usize; 4]> = None;
    for (k, pixel) in imagen.bytes().chunks(bpp).enumerate() {
        if transparente(pixel) {
            continue;
        }
        let (x, y) = (k % ancho, k / ancho);
        rect = Some(match rect {
            None => [x, y, x + 1, y + 1],
            Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1)],
        });
    }
    rect
}

/// Comprueba campo a campo el fichero escrito contra el documento y devuelve lo leído.
fn comprueba_escritura(doc: &Document, bytes: &[u8]) -> FicheroLeido {
    let f = parsea(bytes);
    let (cab, fot) = (&f.cabecera, &f.fotograma);
    let indexado = doc.mode() == Mode::Indexed;
    // Spec: Header.
    assert_eq!(cab.tam_fichero as usize, bytes.len());
    assert_eq!(cab.magia, MAGIA_FICHERO);
    assert_eq!(cab.fotogramas, 1);
    assert_eq!(
        (u32::from(cab.ancho), u32::from(cab.alto)),
        (doc.width(), doc.height())
    );
    assert_eq!(
        cab.profundidad,
        if indexado {
            PROFUNDIDAD_INDEXADO
        } else {
            PROFUNDIDAD_RGBA
        }
    );
    assert_eq!(cab.flags, CABECERA_OPACIDAD_VALIDA);
    assert_eq!(cab.velocidad, 100);
    assert_eq!(cab.dwords_cero, [0, 0]);
    assert_eq!(cab.ignorados, [0, 0, 0]);
    assert_eq!(cab.pixel, (1, 1));
    assert_eq!(cab.rejilla, (0, 0, 16, 16));
    assert!(cab.futuro_a_cero);
    // Spec: Frames.
    assert_eq!(fot.tam as usize, bytes.len() - TAM_CABECERA);
    assert_eq!(fot.magia, MAGIA_FOTOGRAMA);
    assert_eq!(fot.duracion, 100);
    assert_eq!(fot.futuro, [0, 0]);
    assert_eq!(u32::from(fot.chunks_antiguo), fot.chunks_nuevo);

    // Spec: Chunk Types, orden: perfil, paleta (solo indexado), capas y celdas.
    let transparente = |p: &[u8]| -> bool {
        match doc.palette().and_then(Palette::transparent_index) {
            Some(t) if indexado => p[0] == t,
            _ => rgba_transparente(p),
        }
    };
    let rects: Vec<Option<[usize; 4]>> = doc
        .layers()
        .iter()
        .map(|l| {
            doc.image(l.cel())
                .and_then(|i| rectangulo_minimo(i, transparente))
        })
        .collect();
    let n = doc.layers().len();
    let celdas = rects.iter().flatten().count();
    let mut tipos = vec![CHUNK_PERFIL];
    if indexado {
        tipos.push(CHUNK_PALETA);
    }
    tipos.extend(std::iter::repeat_n(CHUNK_CAPA, n));
    tipos.extend(std::iter::repeat_n(CHUNK_CELDA, celdas));
    let reales: Vec<u16> = f.chunks.iter().map(|(t, _)| *t).collect();
    assert_eq!(reales, tipos, "orden de los chunks");

    // Perfil sRGB (Spec: Color Profile Chunk): tipo 1, sin gamma.
    assert_eq!(f.chunks[0].1[..4], [1, 0, 0, 0]);
    // Paleta (Spec: Palette Chunk).
    let mut siguiente = 1;
    if let Some(paleta) = doc.palette().filter(|_| indexado) {
        let datos = &f.chunks[1].1;
        let mut l = Lector::nuevo(datos);
        assert_eq!(l.u32() as usize, paleta.len());
        assert_eq!((l.u32(), l.u32() as usize), (0, paleta.len() - 1));
        assert_eq!(l.bytes(8), [0; 8]);
        for (i, c) in paleta.colors().iter().enumerate() {
            assert_eq!(l.u16(), 0, "entrada {i} sin nombre");
            assert_eq!(l.bytes(4), [c.r, c.g, c.b, c.a], "entrada {i}");
        }
        assert_eq!(l.resto(), 0);
        assert_eq!(usize::from(cab.colores), paleta.len());
        assert_eq!(Some(cab.transparente), paleta.transparent_index());
        siguiente = 2;
    }
    // Capas (Spec: Layer Chunk).
    for (i, capa) in doc.layers().iter().enumerate() {
        let c = decodifica_capa(&f.chunks[siguiente + i].1);
        let flags =
            u16::from(capa.visible()) * CAPA_VISIBLE + u16::from(!capa.locked()) * CAPA_EDITABLE;
        assert_eq!(c.flags, flags, "banderas de {}", capa.name());
        assert_eq!(
            (c.tipo, c.nivel, c.ancho, c.alto),
            (CAPA_TIPO_NORMAL, 0, 0, 0)
        );
        assert_eq!(
            c.mezcla,
            codigo_mezcla(capa.blend()),
            "mezcla de {}",
            capa.name()
        );
        assert_eq!(c.opacidad, capa.opacity());
        assert_eq!(c.futuro, [0, 0, 0]);
        assert_eq!(c.nombre, capa.name());
    }
    // Celdas (Spec: Cel Chunk): rectángulo mínimo, sin chunk para las vacías o transparentes.
    let bpp = doc.mode().bytes_per_pixel();
    let mut vistas = Vec::new();
    for (_, datos) in &f.chunks[siguiente + n..] {
        let c = decodifica_celda(datos);
        let capa = usize::from(c.capa);
        let [x0, y0, x1, y1] = rects[capa].expect("solo hay celda si hay píxeles visibles");
        assert_eq!(
            (c.x, c.y),
            (x0 as i16, y0 as i16),
            "origen de la celda {capa}"
        );
        assert_eq!(
            (usize::from(c.ancho), usize::from(c.alto)),
            (x1 - x0, y1 - y0)
        );
        assert_eq!(
            (c.opacidad, c.tipo, c.z, c.futuro),
            (255, CELDA_COMPRIMIDA, 0, [0; 5])
        );
        let imagen = doc
            .image(doc.layers()[capa].cel())
            .expect("celda con imagen");
        let ancho = imagen.width() as usize;
        let mut esperados = Vec::new();
        for y in y0..y1 {
            let desde = (y * ancho + x0) * bpp;
            esperados.extend(&imagen.bytes()[desde..desde + (x1 - x0) * bpp]);
        }
        assert_eq!(c.pixeles, esperados, "píxeles de la celda {capa}");
        vistas.push(capa);
    }
    let mut ordenadas = vistas.clone();
    ordenadas.sort_unstable();
    ordenadas.dedup();
    assert_eq!(ordenadas.len(), vistas.len(), "una celda por capa");
    f
}

#[test]
fn fila_10_a_escribe_rgba_campo_a_campo_con_los_ocho_modos() {
    let (ancho, alto) = (5u32, 4u32);
    let vacio = || vec![0u8; 5 * 4 * 4];
    let con = |pixeles: &[(usize, usize, [u8; 4])]| {
        let mut b = vacio();
        for &(x, y, p) in pixeles {
            b[(y * 5 + x) * 4..][..4].copy_from_slice(&p);
        }
        imagen_rgba(ancho, alto, b)
    };
    let punto = |x: usize, y: usize| con(&[(x, y, [200, 100, 50, 255])]);
    let capas = vec![
        // Rectángulo mínimo (1,1)-(3,2): un píxel transparente con RGB dentro y otro fuera.
        capa_simple(
            "uno",
            Some(con(&[
                (1, 1, [1, 2, 3, 255]),
                (2, 1, [7, 8, 9, 0]),
                (3, 2, [4, 5, 6, 128]),
                (0, 3, [4, 4, 4, 0]),
            ])),
        ),
        capa(
            "dos",
            false,
            128,
            BlendMode::Multiply,
            true,
            Some(imagen_rgba(ancho, alto, vec![9; 80])),
        ),
        capa_simple("tres", None),
        // Materializada y totalmente transparente (con RGB distinto de 0): no escribe celda.
        capa_simple(
            "cuatro",
            Some(con(&[(2, 2, [5, 5, 5, 0]), (0, 0, [6, 6, 6, 0])])),
        ),
        capa(
            "cinco",
            true,
            17,
            BlendMode::Screen,
            false,
            Some(punto(0, 0)),
        ),
        capa(
            "seis",
            true,
            255,
            BlendMode::Overlay,
            false,
            Some(punto(4, 3)),
        ),
        capa(
            "siete",
            true,
            255,
            BlendMode::Darken,
            false,
            Some(punto(2, 1)),
        ),
        capa(
            "ocho",
            true,
            255,
            BlendMode::Lighten,
            false,
            Some(punto(1, 3)),
        ),
        capa("nueve", true, 255, BlendMode::Add, false, Some(punto(3, 0))),
        capa(
            "diez",
            true,
            255,
            BlendMode::Difference,
            false,
            Some(punto(4, 0)),
        ),
    ];
    let doc = documento(ancho, alto, Mode::Rgba, None, capas);
    let bytes = match write_aseprite(&doc) {
        Ok(b) => b,
        Err(e) => panic!("escribir: {e:?}"),
    };
    let f = comprueba_escritura(&doc, &bytes);
    // Los códigos de la tabla de Spec: Layer Chunk, a mano: Normal 0, Multiply 1, Screen 2,
    // Overlay 3, Darken 4, Lighten 5, Addition 16 y Difference 10.
    let codigos: Vec<u16> = (0..10)
        .map(|i| decodifica_capa(&f.chunks[1 + i].1).mezcla)
        .collect();
    assert_eq!(codigos, [0, 1, 0, 0, 2, 3, 4, 5, 16, 10]);
    // Sin chunk de paleta en RGBA, y solo 8 celdas: «tres» y «cuatro» no escriben.
    assert!(
        f.chunks
            .iter()
            .all(|(t, _)| *t != CHUNK_PALETA && *t != CHUNK_PALETA_ANTIGUA)
    );
    assert_eq!(
        f.chunks.iter().filter(|(t, _)| *t == CHUNK_CELDA).count(),
        8
    );
}

#[test]
fn fila_10_b_escribe_indexado_con_paleta_nueva_de_256_y_banderas_de_capa() {
    let colores: Vec<[u8; 4]> = (0..=255u8)
        .map(|i| {
            [
                i,
                255 - i,
                i.wrapping_mul(7),
                if i % 5 == 0 { 100 } else { 255 },
            ]
        })
        .collect();
    let paleta = paleta_de(&colores, Some(255));
    let (ancho, alto) = (6u32, 3u32);
    let indices =
        |f: &dyn Fn(usize) -> u8| imagen_indexada(ancho, alto, &paleta, (0..18).map(f).collect());
    let capas = vec![
        capa(
            "a",
            true,
            255,
            BlendMode::Normal,
            false,
            Some(indices(&|k| (k * 13 % 250) as u8)),
        ),
        capa(
            "b",
            true,
            255,
            BlendMode::Normal,
            true,
            Some(indices(&|k| if k == 4 || k == 9 { 7 } else { 255 })),
        ),
        capa("c", false, 255, BlendMode::Normal, false, None),
        capa(
            "d",
            false,
            255,
            BlendMode::Normal,
            true,
            Some(indices(&|_| 255)),
        ),
    ];
    let doc = documento(ancho, alto, Mode::Indexed, Some(paleta), capas);
    let bytes = match write_aseprite(&doc) {
        Ok(b) => b,
        Err(e) => panic!("escribir: {e:?}"),
    };
    let f = comprueba_escritura(&doc, &bytes);
    assert_eq!(f.cabecera.colores, 256);
    assert_eq!(f.cabecera.transparente, 255);
    // Bit 1 = visible y bit 2 = editable: visible y libre 3, visible y bloqueada 1, oculta y libre
    // 2, oculta y bloqueada 0.
    let flags: Vec<u16> = (0..4)
        .map(|i| decodifica_capa(&f.chunks[2 + i].1).flags)
        .collect();
    assert_eq!(flags, [3, 1, 2, 0]);
}

#[test]
fn fila_10_c_errores_de_escritura_gris_sin_transparente_y_lienzo_enorme() {
    for modo in [Mode::Gray, Mode::GrayAlpha] {
        let doc = Document::new(Image::new(3, 3, modo).expect("imagen"));
        match write_aseprite(&doc) {
            Err(AsepriteError::Unsupported(u)) => {
                assert!(
                    matches!(u, Unsupported::Grayscale { .. }),
                    "{modo:?}: {u:?}"
                );
                assert!(
                    AsepriteError::Unsupported(u)
                        .to_string()
                        .contains(NO_MODIFICADO)
                );
            }
            otro => panic!(
                "{modo:?}: se esperaba Unsupported(Grayscale) y salió {:?}",
                otro.err()
            ),
        }
    }

    let paleta = paleta_de(&PALETA4, None);
    let doc = Document::new(imagen_indexada(2, 2, &paleta, vec![0; 4]));
    match write_aseprite(&doc) {
        Err(e @ AsepriteError::NoTransparentIndex { .. }) => assert_eq!(
            e.to_string(),
            "esta imagen indexada no tiene entrada transparente: fíjala con Fijar transparente o convierte a RGBA"
        ),
        otro => panic!("indexado sin transparente: {:?}", otro.err()),
    }

    for (ancho, alto) in [(65_536u32, 1u32), (1, 65_536)] {
        let doc = Document::new(Image::new(ancho, alto, Mode::Rgba).expect("imagen"));
        let e = write_aseprite(&doc).err();
        assert!(
            matches!(e, Some(AsepriteError::TooLarge { .. })),
            "{ancho}×{alto}: {e:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 11: ida y vuelta.
// ---------------------------------------------------------------------------------------

fn escribe(doc: &Document) -> Vec<u8> {
    match write_aseprite(doc) {
        Ok(b) => b,
        Err(e) => panic!("escribir: {e:?}"),
    }
}

/// Escribe, lee y exige `mismo_contenido`; devuelve el documento leído y los bytes.
fn ida_y_vuelta(contexto: &str, doc: &Document) -> (Document, Vec<u8>) {
    let bytes = escribe(doc);
    let leido = lee_ok(contexto, &bytes);
    assert!(
        !leido.report.lossy,
        "{contexto}: lo que escribe Pixoko no avisa"
    );
    assert_eq!(leido.report.frames_in_file, 1);
    if let Err(motivo) = mismo_contenido(doc, &leido.document) {
        panic!("{contexto}: {motivo}");
    }
    (leido.document, bytes)
}

#[test]
fn fila_11_a_rgba_de_tres_capas_con_celda_vacia_y_rgb_bajo_alfa_0() {
    let (ancho, alto) = (6u32, 5u32);
    let fondo = pixeles_rgba(6, 5, |x, y| {
        [
            x as u8 * 40,
            y as u8 * 50,
            99,
            [255, 128, 0, 77][(x + y) % 4],
        ]
    });
    // Dibujo disperso: el RGB bajo alfa 0 dentro de lo dibujado debe volver tal cual.
    let mut dibujo = vec![0u8; 6 * 5 * 4];
    for (x, y, p) in [
        (1usize, 1usize, [10u8, 20, 30, 255]),
        (2, 1, [11, 12, 13, 0]),
        (4, 3, [200, 100, 50, 90]),
    ] {
        dibujo[(y * 6 + x) * 4..][..4].copy_from_slice(&p);
    }
    let transparente = pixeles_rgba(6, 5, |x, _| [x as u8 + 1, 2, 3, 0]);
    let doc = documento(
        ancho,
        alto,
        Mode::Rgba,
        None,
        vec![
            capa(
                "fondo",
                true,
                200,
                BlendMode::Multiply,
                true,
                Some(imagen_rgba(ancho, alto, fondo)),
            ),
            capa("vacía", false, 90, BlendMode::Screen, false, None),
            capa(
                "transparente",
                true,
                255,
                BlendMode::Overlay,
                false,
                Some(imagen_rgba(ancho, alto, transparente)),
            ),
            capa(
                "dibujo",
                true,
                255,
                BlendMode::Normal,
                false,
                Some(imagen_rgba(ancho, alto, dibujo)),
            ),
        ],
    );
    let (leido, _) = ida_y_vuelta("RGBA de capas", &doc);
    // La materializada y transparente vuelve vacía (equivalentes); la vacía, también.
    assert!(leido.image(leido.layers()[1].cel()).is_none());
    assert!(leido.image(leido.layers()[2].cel()).is_none());
    // El RGB bajo alfa 0 dentro del rectángulo mínimo se conserva byte a byte.
    let dibujo = leido
        .image(leido.layers()[3].cel())
        .expect("el dibujo tiene celda");
    assert_eq!(
        dibujo.get_pixel(2, 1),
        Ok(Pixel::Rgba(Rgba::new(11, 12, 13, 0)))
    );
    assert_eq!(
        dibujo.get_pixel(4, 3),
        Ok(Pixel::Rgba(Rgba::new(200, 100, 50, 90)))
    );
}

#[test]
fn fila_11_b_indexado_de_256_colores_con_alfa_y_nombre_de_255_bytes_multibyte() {
    let colores: Vec<[u8; 4]> = (0..=255u8)
        .map(|i| [i, i ^ 0x5A, i.wrapping_mul(3), 255 - i / 2])
        .collect();
    let paleta = paleta_de(&colores, Some(255));
    let (ancho, alto) = (7u32, 4u32);
    // 127 veces «ñ» (2 bytes) y una «a»: 255 bytes.
    let nombre = format!("{}a", "ñ".repeat(127));
    assert_eq!(nombre.len(), 255);
    let capa_a = imagen_indexada(
        ancho,
        alto,
        &paleta,
        (0..28).map(|k| (k * 9 % 256) as u8).collect(),
    );
    let capa_b = imagen_indexada(
        ancho,
        alto,
        &paleta,
        (0..28)
            .map(|k| if k % 4 == 0 { 255 } else { k as u8 })
            .collect(),
    );
    let doc = documento(
        ancho,
        alto,
        Mode::Indexed,
        Some(paleta),
        vec![
            capa_simple(&nombre, Some(capa_a)),
            capa("arriba", false, 255, BlendMode::Normal, true, Some(capa_b)),
            capa_simple("vacía", None),
        ],
    );
    let (leido, _) = ida_y_vuelta("indexado de 256", &doc);
    assert_eq!(leido.layers()[0].name(), nombre);
    assert_eq!(
        leido.palette().and_then(Palette::transparent_index),
        Some(255)
    );
    assert_eq!(leido.palette().map(Palette::len), Some(256));
}

#[test]
fn fila_11_c_extremos_y_segunda_escritura_con_los_mismos_bytes() {
    // 256 capas de 1 × 1, la última encima.
    let capas: Vec<LayerSpec> = (0..256)
        .map(|i| {
            let imagen = (i % 3 != 0).then(|| imagen_rgba(1, 1, vec![i as u8, 2, 3, 255]));
            capa(
                &format!("c{i}"),
                i % 2 == 0,
                255 - i as u8 % 7,
                BlendMode::ALL[i % 8],
                i % 5 == 0,
                imagen,
            )
        })
        .collect();
    let doc = documento(1, 1, Mode::Rgba, None, capas);
    let (leido, bytes) = ida_y_vuelta("256 capas", &doc);
    assert_eq!(leido.layers().len(), 256);
    assert_eq!(
        escribe(&leido),
        bytes,
        "la segunda escritura da los mismos bytes"
    );

    // 1 × 1 y 65 535 × 1.
    let uno = documento(
        1,
        1,
        Mode::Rgba,
        None,
        vec![capa_simple("a", Some(imagen_rgba(1, 1, vec![9, 8, 7, 6])))],
    );
    ida_y_vuelta("1×1", &uno);
    let ancho = 65_535u32;
    let tira = pixeles_rgba(ancho as usize, 1, |x, _| {
        [x as u8, (x >> 8) as u8, 1, if x % 7 == 0 { 0 } else { 255 }]
    });
    let doc = documento(
        ancho,
        1,
        Mode::Rgba,
        None,
        vec![capa_simple("tira", Some(imagen_rgba(ancho, 1, tira)))],
    );
    let (leido, bytes) = ida_y_vuelta("65535×1", &doc);
    assert_eq!(escribe(&leido), bytes);

    // Y el de la fila 11 a/b, escrito dos veces.
    let paleta = paleta_de(&PALETA4, Some(0));
    let doc = documento(
        4,
        3,
        Mode::Indexed,
        Some(paleta.clone()),
        vec![
            capa_simple(
                "a",
                Some(imagen_indexada(
                    4,
                    3,
                    &paleta,
                    (0..12).map(|k| (k % 4) as u8).collect(),
                )),
            ),
            capa_simple("b", None),
        ],
    );
    let (leido, bytes) = ida_y_vuelta("indexado", &doc);
    assert_eq!(escribe(&leido), bytes);
}

// ---------------------------------------------------------------------------------------
// Propiedades (256 casos).
// ---------------------------------------------------------------------------------------

fn nombre_de_capa() -> impl Strategy<Value = String> {
    "[a-zñ0-9]{1,12}"
}

/// Píxel RGBA con la mitad de las veces alfa 0 y RGB aleatorio.
fn pixel_rgba() -> impl Strategy<Value = [u8; 4]> {
    prop_oneof![
        3 => any::<[u8; 3]>().prop_map(|c| [c[0], c[1], c[2], 0]),
        2 => any::<[u8; 4]>(),
    ]
}

/// Celda RGBA: vacía, materializada y totalmente transparente (con RGB), o con píxeles.
fn celda_rgba(n: usize) -> impl Strategy<Value = Option<Vec<u8>>> {
    prop_oneof![
        1 => Just(None),
        1 => any::<[u8; 3]>().prop_map(move |c| Some([c[0], c[1], c[2], 0].repeat(n))),
        4 => vec(pixel_rgba(), n).prop_map(|px| Some(px.concat())),
    ]
}

fn documento_rgba(ancho: u32, alto: u32) -> BoxedStrategy<Document> {
    let n = (ancho * alto) as usize;
    let capa_rgba = (
        nombre_de_capa(),
        any::<bool>(),
        any::<u8>(),
        0usize..8,
        any::<bool>(),
        celda_rgba(n),
    );
    vec(capa_rgba, 1..=6)
        .prop_map(move |capas| {
            let specs = capas
                .into_iter()
                .map(|(nombre, visible, opacidad, modo, bloqueada, celda)| {
                    let imagen = celda.map(|b| imagen_rgba(ancho, alto, b));
                    capa(
                        &nombre,
                        visible,
                        opacidad,
                        BlendMode::ALL[modo],
                        bloqueada,
                        imagen,
                    )
                })
                .collect();
            documento(ancho, alto, Mode::Rgba, None, specs)
        })
        .boxed()
}

fn documento_indexado(ancho: u32, alto: u32) -> BoxedStrategy<Document> {
    let n = (ancho * alto) as usize;
    (vec(any::<[u8; 4]>(), 2..=256), any::<u8>())
        .prop_flat_map(move |(colores, t)| {
            let largo = colores.len();
            let transparente = (usize::from(t) % largo) as u8;
            let celda = prop_oneof![1 => Just(None), 5 => vec(any::<u8>(), n).prop_map(Some)];
            let capa_idx = (nombre_de_capa(), any::<bool>(), any::<bool>(), celda);
            vec(capa_idx, 1..=6).prop_map(move |capas| {
                let paleta = paleta_de(&colores, Some(transparente));
                let specs = capas
                    .into_iter()
                    .map(|(nombre, visible, bloqueada, celda)| {
                        // Un tercio de los píxeles es el índice transparente.
                        let imagen = celda.map(|b| {
                            let indices = b
                                .iter()
                                .map(|&v| {
                                    if v % 3 == 0 {
                                        transparente
                                    } else {
                                        (usize::from(v) % largo) as u8
                                    }
                                })
                                .collect();
                            imagen_indexada(ancho, alto, &paleta, indices)
                        });
                        capa(&nombre, visible, 255, BlendMode::Normal, bloqueada, imagen)
                    })
                    .collect();
                documento(ancho, alto, Mode::Indexed, Some(paleta.clone()), specs)
            })
        })
        .boxed()
}

fn documento_cualquiera() -> impl Strategy<Value = Document> {
    (1u32..=12, 1u32..=12, any::<bool>()).prop_flat_map(|(ancho, alto, indexado)| {
        if indexado {
            documento_indexado(ancho, alto)
        } else {
            documento_rgba(ancho, alto)
        }
    })
}

fn falla<T, E: Display>(r: Result<T, E>) -> Result<T, TestCaseError> {
    r.map_err(|e| TestCaseError::fail(e.to_string()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// P1. Ida y vuelta: `read(write(d))` tiene el mismo contenido que `d`, y los bytes de una
    /// segunda escritura son los mismos.
    #[test]
    fn propiedad_1_ida_y_vuelta_y_bytes_estables(doc in documento_cualquiera()) {
        let bytes = falla(write_aseprite(&doc))?;
        let leido = falla(read_aseprite(&bytes, &AsepriteLimits::default()))?;
        prop_assert!(!leido.report.lossy);
        prop_assert_eq!(leido.report.frames_in_file, 1);
        falla(mismo_contenido(&doc, &leido.document))?;
        let otra = falla(write_aseprite(&leido.document))?;
        prop_assert!(otra == bytes, "la segunda escritura cambia los bytes");
    }
}

/// Bases para P3: dos ficheros hechos a mano y tres del corpus.
fn bases() -> &'static Vec<Vec<u8>> {
    static BASES: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
    BASES.get_or_init(|| {
        vec![
            base_rgba().bytes(),
            base_indexada().bytes(),
            golden("mixto-rgba.aseprite"),
            golden("indexado-celdas.aseprite"),
            golden("celda-recortada.aseprite"),
        ]
    })
}

/// Lo que debe cumplir todo documento que el lector entrega (P3).
fn cumple_invariantes(doc: &Document, limites: &AsepriteLimits) -> Result<(), String> {
    let n = doc.layers().len();
    if !(1..=Document::MAX_LAYERS).contains(&n) {
        return Err(format!("{n} capas"));
    }
    if doc.materialized_bytes() > limites.max_cells_bytes {
        return Err("celdas por encima del tope".to_string());
    }
    for capa in doc.layers() {
        if capa.name().len() > Document::MAX_LAYER_NAME_BYTES {
            return Err("nombre demasiado largo".to_string());
        }
        let Some(imagen) = doc.image(capa.cel()) else {
            continue;
        };
        if (imagen.width(), imagen.height(), imagen.mode())
            != (doc.width(), doc.height(), doc.mode())
        {
            return Err("celda que no es del tamaño del lienzo".to_string());
        }
        if imagen.palette() != doc.palette() {
            return Err("celda con otra paleta".to_string());
        }
        if let Some(paleta) = imagen.palette()
            && imagen
                .bytes()
                .iter()
                .any(|&i| usize::from(i) >= paleta.len())
        {
            return Err("índice de píxel fuera de la paleta".to_string());
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// P3. Robustez: bytes arbitrarios y ficheros válidos truncados o con bytes cambiados no
    /// dan pánico, y si dan `Ok` el documento cumple sus invariantes.
    #[test]
    fn propiedad_3_robustez_ante_bytes_arbitrarios_truncados_y_alterados(
        basura in vec(any::<u8>(), 0..400),
        base in 0usize..5,
        cambios in vec((any::<usize>(), any::<u8>()), 0..=4),
        corte in proptest::option::of(any::<usize>()),
    ) {
        let limites = AsepriteLimits { max_pixels: 1 << 16, max_cells_bytes: 1 << 20 };
        let mut candidatos = vec![basura];
        let mut valido = bases()[base].clone();
        for (pos, nuevo) in cambios {
            let i = pos % valido.len();
            valido[i] = nuevo;
        }
        if let Some(corte) = corte {
            valido.truncate(corte % (valido.len() + 1));
        }
        candidatos.push(valido);
        for bytes in candidatos {
            if let Ok(leido) = read_aseprite(&bytes, &limites) {
                falla(cumple_invariantes(&leido.document, &limites))?;
            }
        }
    }

    /// P4. Colocación de la celda: el píxel `(px, py)` del documento es el de la celda en
    /// `(px − x, py − y)` si cae dentro y el relleno si no; vacía si lo de dentro es transparente.
    #[test]
    fn propiedad_4_colocacion_de_la_celda(
        (ancho, alto) in (1u16..=12, 1u16..=12),
        (x, y) in (-20i16..=20, -20i16..=20),
        (cw, ch) in (0u16..=16, 0u16..=16),
        indexado in any::<bool>(),
        cruda in any::<bool>(),
        todo_transparente in prop_oneof![1 => Just(true), 3 => Just(false)],
        semilla in vec(any::<u8>(), 256 * 4),
        ceros in vec(any::<bool>(), 256),
    ) {
        let n = usize::from(cw) * usize::from(ch);
        let transparente_k = |k: usize| todo_transparente || ceros[k];
        let datos: Vec<u8> = if indexado {
            // Índice transparente 3; los demás píxeles usan los índices 0 a 2.
            (0..n).map(|k| if transparente_k(k) { 3 } else { semilla[k] % 3 }).collect()
        } else {
            (0..n)
                .flat_map(|k| {
                    let alfa = if transparente_k(k) { 0 } else { semilla[4 * k + 3].max(1) };
                    [semilla[4 * k], semilla[4 * k + 1], semilla[4 * k + 2], alfa]
                })
                .collect()
        };
        let celda = Celda {
            tipo: if cruda { CELDA_CRUDA } else { CELDA_COMPRIMIDA },
            ..Celda::nueva(0, x, y, cw, ch, datos.clone())
        };
        let capa_chunk = Capa::nueva("c").chunk();
        let fichero = if indexado {
            Fichero::indexado(ancho, alto, 3, 4, vec![perfil_srgb(), paleta_completa(&PALETA4), capa_chunk, celda.chunk()])
        } else {
            Fichero::rgba(ancho, alto, vec![perfil_srgb(), capa_chunk, celda.chunk()])
        };
        let leido = falla(lee(&fichero.bytes()))?;
        let doc = &leido.document;
        prop_assert_eq!(doc.layers().len(), 1);
        let relleno: &[u8] = if indexado { &[3] } else { &[0, 0, 0, 0] };
        let transparente: fn(&[u8]) -> bool = if indexado { |p| p[0] == 3 } else { rgba_transparente };
        let esperada = colocacion_esperada(
            usize::from(ancho),
            usize::from(alto),
            (i32::from(x), i32::from(y)),
            (usize::from(cw), usize::from(ch)),
            &datos,
            relleno,
            transparente,
        );
        let imagen = doc.image(doc.layers()[0].cel());
        prop_assert_eq!(imagen.map(Image::bytes), esperada.as_deref());
    }
}
