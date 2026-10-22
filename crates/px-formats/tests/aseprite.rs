//! Pruebas de `.aseprite`, etapa 1 (specs/hito-3-aseprite.md, unidad 3.8): `read_aseprite`,
//! `write_aseprite`, `aseprite_dimensions` y `detect_format`.
//!
//! Las filas 2 y 4 a 11 y las propiedades 1, 3 y 4 de «Casos» y «Propiedades». La fila 3 (la
//! composición de los 29 casos) y la parte de la fila 8 con el corpus viven en `px-golden`; la
//! bomba de la fila 9 (con medida de RSS) está en `aseprite_reserve.rs`.
//!
//! Los ficheros de entrada se construyen aquí, a mano, con `writer.rs` (un escritor y un lector
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

#[path = "aseprite/writer.rs"]
#[allow(dead_code)]
mod writer;
use writer::*;

// ---------------------------------------------------------------------------------------
// Ayudantes: corpus, lectura, documentos y la igualdad por contenido (condición 1).
// ---------------------------------------------------------------------------------------

/// Un fichero de `golden/aseprite/` (datos, no código).
fn golden(name_2: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../golden/aseprite")
        .join(name_2);
    fs::read(&path).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path.display()))
}

fn lee(bytes: &[u8]) -> Result<AsepriteRead, AsepriteError> {
    read_aseprite(bytes, &AsepriteLimits::default())
}

fn lee_ok(context: &str, bytes: &[u8]) -> AsepriteRead {
    match lee(bytes) {
        Ok(r) => r,
        Err(e) => panic!("{context}: se esperaba Ok y salió {e:?}"),
    }
}

fn lee_err(context: &str, bytes: &[u8]) -> AsepriteError {
    match lee(bytes) {
        Ok(_) => panic!("{context}: se esperaba un error y salió Ok"),
        Err(e) => e,
    }
}

fn lee_corpus(case: &str) -> AsepriteRead {
    lee_ok(case, &golden(&format!("{case}.aseprite")))
}

/// El `Unsupported` de un fichero que no se admite.
fn not_supported(context: &str, bytes: &[u8]) -> Unsupported {
    match lee_err(context, bytes) {
        AsepriteError::Unsupported(u) => u,
        other_2 => panic!("{context}: se esperaba Unsupported y salió {other_2:?}"),
    }
}

/// Cuántos elementos de `ignored` cumplen `es`.
fn count(r: &AsepriteRead, es: fn(&Ignored) -> bool) -> u32 {
    r.report
        .ignored
        .iter()
        .filter(|(k, _)| es(k))
        .map(|(_, n)| *n)
        .sum()
}

fn image_rgba(width: u32, height: u32, bytes: Vec<u8>) -> Image {
    Image::from_bytes(width, height, Mode::Rgba, bytes).expect("imagen RGBA de la prueba")
}

fn palette_of(colors: &[[u8; 4]], transparent: Option<u8>) -> Palette {
    let mut palette =
        Palette::from_colors(colors.iter().map(|c| Rgba::new(c[0], c[1], c[2], c[3])))
            .expect("paleta de la prueba");
    palette
        .set_transparent_index(transparent)
        .expect("índice transparente de la prueba");
    palette
}

fn image_indexed(width: u32, height: u32, palette: &Palette, indices: Vec<u8>) -> Image {
    Image::from_bytes_indexed(width, height, palette.clone(), indices)
        .expect("imagen indexada de la prueba")
}

fn document(
    width: u32,
    height: u32,
    mode: Mode,
    palette: Option<Palette>,
    layers: Vec<LayerSpec>,
) -> Document {
    Document::from_layers(width, height, mode, palette, layers).expect("documento de la prueba")
}

fn layer_2(
    name_2: &str,
    visible: bool,
    opacity: u8,
    blend: BlendMode,
    locked: bool,
    image: Option<Image>,
) -> LayerSpec {
    LayerSpec {
        name: name_2.to_string(),
        visible,
        opacity,
        blend,
        locked,
        image,
    }
}

/// Una capa visible, desbloqueada, opaca y Normal.
fn layer_simple(name_2: &str, image: Option<Image>) -> LayerSpec {
    layer_2(name_2, true, 255, BlendMode::Normal, false, image)
}

/// Lo que compara «escribir y leer devuelve el mismo documento» (condición 1): sin ids.
#[derive(Debug, PartialEq)]
enum Value {
    Transparent,
    Pixel(Pixel),
}

/// El valor de un píxel de una capa: la celda vacía es transparente; «transparente» es alfa 0 en
/// RGBA y el índice transparente en indexado.
fn value(doc: &Document, layer_2: &Layer, x: u32, y: u32) -> Value {
    let Some(image) = doc.image(layer_2.cel()) else {
        return Value::Transparent;
    };
    let Ok(pixel) = image.get_pixel(x, y) else {
        return Value::Pixel(Pixel::Gray(0));
    };
    let transparent = match pixel {
        Pixel::Rgba(c) => c.a == 0,
        Pixel::Indexed(i) => doc.palette().and_then(Palette::transparent_index) == Some(i),
        Pixel::GrayAlpha { alpha, .. } => alpha == 0,
        _ => false,
    };
    if transparent {
        Value::Transparent
    } else {
        Value::Pixel(pixel)
    }
}

/// La igualdad de ida y vuelta de la spec: mismas capas en el mismo orden con nombre,
/// visibilidad, opacidad, mezcla y bloqueo iguales, mismo lienzo, misma paleta (colores, alfa e
/// índice transparente) y mismo contenido de píxeles por capa (una celda vacía y una
/// materializada y totalmente transparente son iguales; dos píxeles transparentes son iguales
/// aunque difiera su RGB; los demás, byte a byte).
fn same_content(a: &Document, b: &Document) -> Result<(), String> {
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
                let (va, vb) = (value(a, ca, x, y), value(b, cb, x, y));
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
/// `(px − x, py − y)` si cae dentro y `fill` si no. `None` si lo que cae dentro es todo
/// transparente (o no cae nada): la celda del modelo queda vacía.
fn placement_expected(
    width: usize,
    height: usize,
    (x, y): (i32, i32),
    (cw, ch): (usize, usize),
    data: &[u8],
    fill: &[u8],
    transparent: impl Fn(&[u8]) -> bool,
) -> Option<Vec<u8>> {
    let bpp = fill.len();
    let mut output = fill.repeat(width * height);
    let mut visible = false;
    for py in 0..height {
        for px in 0..width {
            let (cx, cy) = (px as i32 - x, py as i32 - y);
            if cx < 0 || cy < 0 || cx as usize >= cw || cy as usize >= ch {
                continue;
            }
            let k = (cy as usize * cw + cx as usize) * bpp;
            let pixel = &data[k..k + bpp];
            let d = (py * width + px) * bpp;
            output[d..d + bpp].copy_from_slice(pixel);
            visible |= !transparent(pixel);
        }
    }
    visible.then_some(output)
}

fn rgba_transparent(p: &[u8]) -> bool {
    p[3] == 0
}

// ---------------------------------------------------------------------------------------
// Ficheros base construidos a mano.
// ---------------------------------------------------------------------------------------

const PALETTE_4: [[u8; 4]; 4] = [
    [0, 0, 0, 255],
    [255, 0, 0, 255],
    [0, 255, 0, 255],
    [0, 0, 255, 255],
];

/// RGBA de 4 × 3, perfil sRGB, dos capas y dos celdas (la segunda, de 2 × 2 en (1, 1)).
fn base_rgba() -> File {
    File::rgba(
        4,
        3,
        vec![
            profile_srgb(),
            TestLayer::new("fondo").chunk(),
            TestLayer::new("arriba").chunk(),
            Cel::new(
                0,
                0,
                0,
                4,
                3,
                pixels_rgba(4, 3, |x, y| [10 * x as u8 + 1, 20 * y as u8 + 1, 200, 255]),
            )
            .chunk(),
            Cel::new(
                1,
                1,
                1,
                2,
                2,
                pixels_rgba(2, 2, |x, y| [255, 0, 0, if x == y { 255 } else { 0 }]),
            )
            .chunk(),
        ],
    )
}

/// Indexado de 4 × 3 con la paleta de 4 colores (índice transparente 0), dos capas y dos celdas.
fn base_indexed() -> File {
    File::indexed(
        4,
        3,
        0,
        4,
        vec![
            profile_srgb(),
            palette_complete(&PALETTE_4),
            TestLayer::new("fondo").chunk(),
            TestLayer::new("arriba").chunk(),
            Cel::new(0, 0, 0, 4, 3, (0..12).map(|i| (i % 4) as u8).collect()).chunk(),
            Cel::new(1, 1, 1, 2, 2, vec![1, 0, 2, 3]).chunk(),
        ],
    )
}

/// Las capas de `base_rgba` con `extra` añadido al final del primer fotograma.
fn base_with(extra: Vec<u8>) -> Vec<u8> {
    let mut f = base_rgba();
    f.chunks.push(extra);
    f.bytes()
}

// ---------------------------------------------------------------------------------------
// Fila 2: la estructura de los casos del corpus.
// ---------------------------------------------------------------------------------------

/// Nombre del caso `blend-*` y el modo de su capa `fuente` (tabla de Spec: Layer Chunk).
const MODES_OF_THE_CORPUS: [(&str, BlendMode); 8] = [
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
fn row_02_a_layers_opacity_has_six_layers_with_their_opacities() {
    let r = lee_corpus("layers-opacity");
    let doc = &r.document;
    assert_eq!(
        (doc.width(), doc.height(), doc.mode()),
        (32, 16, Mode::Rgba)
    );
    assert!(doc.palette().is_none());
    let expected_2 = [
        ("fondo", 255u8),
        ("op-0", 0),
        ("op-64", 64),
        ("op-128", 128),
        ("op-200", 200),
        ("op-255", 255),
    ];
    assert_eq!(doc.layers().len(), 6);
    for (i, (name_2, opacity)) in expected_2.iter().enumerate() {
        let layer_2 = &doc.layers()[i];
        assert_eq!(layer_2.name(), *name_2, "nombre de la capa {i}");
        assert_eq!(layer_2.opacity(), *opacity, "opacidad de {name_2}");
        assert!(layer_2.visible() && !layer_2.locked(), "{name_2}");
        assert_eq!(layer_2.blend(), BlendMode::Normal, "{name_2}");
        // Los ids son 0..n (el fichero no los guarda).
        assert_eq!(layer_2.id().raw() as usize, i);
        assert!(doc.image(layer_2.cel()).is_some(), "{name_2} tiene celda");
    }
    assert_eq!(r.report.frames_in_file, 1);
    assert!(!r.report.lossy, "el corpus no se descarta nada");

    // `detect_format` mira el contenido, no la extensión.
    let bytes = golden("layers-opacity.aseprite");
    assert!(matches!(detect_format(&bytes), Some(FileFormat::Aseprite)));
    let png = write_png(&Image::new(2, 2, Mode::Rgba).expect("imagen")).expect("png");
    assert!(matches!(detect_format(&png), Some(FileFormat::Png)));
    assert!(detect_format(&[1, 2, 3]).is_none());
    assert_eq!(aseprite_dimensions(&bytes).ok(), Some((32, 16)));
}

/// La palabra inglesa del nombre de caso `blend-<palabra>` de un modo de mezcla; el nombre español es el
/// de la capa dentro del fichero (el contenido del corpus no cambia).
fn case_word(name_2: &str) -> &'static str {
    match name_2 {
        "normal" => "normal",
        "multiplicar" => "multiply",
        "trama" => "pattern",
        "superponer" => "overlay",
        "oscurecer" => "darken",
        "aclarar" => "lighten",
        "sumar" => "add",
        "diferencia" => "difference",
        _ => panic!("modo desconocido: {name_2}"),
    }
}

#[test]
fn row_02_b_the_eight_blends_have_the_mode_of_their_source_layer() {
    for (name_2, mode) in MODES_OF_THE_CORPUS {
        let case = format!("blend-{}", case_word(name_2));
        let r = lee_corpus(&case);
        let layers = r.document.layers();
        assert_eq!(layers.len(), 2, "{case}");
        assert_eq!(
            (layers[0].name(), layers[0].blend()),
            ("base", BlendMode::Normal)
        );
        assert_eq!(
            (layers[1].name(), layers[1].blend()),
            ("fuente", mode),
            "{case}"
        );
        assert_eq!(
            (layers[1].opacity(), layers[1].visible()),
            (255, true),
            "{case}"
        );
    }
    // Las ocho tiras, una capa por modo y todas con opacidad 128.
    let r = lee_corpus("blend-eight-opacity");
    let layers = r.document.layers();
    assert_eq!(layers.len(), 9);
    assert_eq!((layers[0].name(), layers[0].opacity()), ("base", 255));
    for (i, (name_2, mode)) in MODES_OF_THE_CORPUS.iter().enumerate() {
        let layer_2 = &layers[i + 1];
        assert_eq!(layer_2.name(), format!("m-{name_2}"));
        assert_eq!(
            (layer_2.blend(), layer_2.opacity()),
            (*mode, 128),
            "{name_2}"
        );
    }
}

#[test]
fn row_02_c_hidden_layers_and_locked_layer_are_independent() {
    let r = lee_corpus("layers-hidden");
    let layers = r.document.layers();
    let data: Vec<(&str, bool)> = layers.iter().map(|c| (c.name(), c.visible())).collect();
    assert_eq!(
        data,
        [
            ("fondo", true),
            ("oculta-1", false),
            ("visible-1", true),
            ("oculta-2", false),
            ("visible-2", true)
        ]
    );
    assert_eq!(
        (layers[3].blend(), layers[3].opacity()),
        (BlendMode::Multiply, 128)
    );
    assert!(layers.iter().all(|c| !c.locked()), "ninguna está bloqueada");

    // Bit 1 = visible y bit 2 = editable (bloqueada = bit 2 apagado): banderas 3, 1, 3 y 0.
    let r = lee_corpus("layer-locked");
    let data: Vec<(&str, bool, bool)> = r
        .document
        .layers()
        .iter()
        .map(|c| (c.name(), c.visible(), c.locked()))
        .collect();
    assert_eq!(
        data,
        [
            ("fondo", true, false),
            ("bloqueada", true, true),
            ("arriba", true, false),
            ("oculta-bloqueada", false, true)
        ]
    );
}

#[test]
fn row_02_d_empty_cel_and_indexed_with_transparent_7() {
    let r = lee_corpus("cel-empty");
    let doc = &r.document;
    let layers = doc.layers();
    assert_eq!(layers.len(), 3);
    assert!(
        doc.image(layers[0].cel()).is_some(),
        "con-celda tiene imagen"
    );
    // `without-cel` conserva sus propiedades aunque no haya celda, y `transparent` (una celda con
    // alfa 0 en todo) equivale a una vacía: sin imagen.
    assert_eq!(layers[1].name(), "sin-celda");
    assert_eq!(
        (layers[1].blend(), layers[1].opacity()),
        (BlendMode::Multiply, 100)
    );
    assert!(doc.image(layers[1].cel()).is_none());
    assert_eq!(layers[2].name(), "transparente");
    assert!(doc.image(layers[2].cel()).is_none());

    let r = lee_corpus("indexed-transparent-7");
    let doc = &r.document;
    assert_eq!(doc.mode(), Mode::Indexed);
    let palette = doc.palette().expect("el indexado tiene paleta");
    assert_eq!(palette.len(), 16);
    assert_eq!(palette.transparent_index(), Some(7));
    assert_eq!(doc.layers().len(), 2);
}

// ---------------------------------------------------------------------------------------
// Fila 4: paleta.
// ---------------------------------------------------------------------------------------

#[test]
fn row_04_a_corpus_palette_256_and_new_chunk_with_alpha_names_and_ranges() {
    // El corpus: la paleta de 256 entradas coincide con la de su PNG, con alfa e índice.
    let r = lee_corpus("indexed-palette-256");
    let palette = r.document.palette().expect("paleta");
    let png = read_png(&golden("indexed-palette-256.png"))
        .expect("png del corpus")
        .image;
    let palette_png = png.palette().expect("el PNG es indexado");
    assert_eq!((palette.len(), palette_png.len()), (256, 256));
    for i in 0..=255u8 {
        assert_eq!(
            palette.resolved(i).ok(),
            palette_png.resolved(i).ok(),
            "entrada {i}"
        );
    }
    assert_eq!(palette.transparent_index(), Some(0));

    // Chunk 0x2019: alfa propio, nombres que se saltan y dos rangos; entradas sin cubrir en
    // (0, 0, 0, 255). El chunk 0x0004 que va antes se ignora (gana el nuevo).
    let colors_old = vec![[9u8, 9, 9]; 8];
    let f = File::indexed(
        4,
        3,
        1,
        8,
        vec![
            profile_srgb(),
            palette_old(CHUNK_PALETTE_OLD, &[(0, 8, colors_old)]),
            palette_new(
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
            palette_new(8, 6, 7, &[([1, 2, 3, 4], None), ([5, 6, 7, 8], Some("n"))]),
            TestLayer::new("a").chunk(),
            Cel::new(0, 0, 0, 4, 3, (0..12).map(|i| (i % 8) as u8).collect()).chunk(),
        ],
    );
    let r = lee_ok("0x2019", &f.bytes());
    let palette = r.document.palette().expect("paleta");
    let expected_2_2 = [
        Rgba::new(255, 0, 0, 255),
        Rgba::new(0, 255, 0, 128),
        Rgba::new(0, 0, 255, 255),
        Rgba::new(10, 20, 30, 40),
        Rgba::new(0, 0, 0, 255),
        Rgba::new(0, 0, 0, 255),
        Rgba::new(1, 2, 3, 4),
        Rgba::new(5, 6, 7, 8),
    ];
    assert_eq!(palette.colors(), expected_2_2);
    assert_eq!(palette.transparent_index(), Some(1));
}

#[test]
fn row_04_b_old_palette_with_jumps_and_zero_is_256() {
    let a = [10u8, 11, 12];
    let b = [20u8, 21, 22];
    let c = [30u8, 31, 32];
    let file = |packets: Vec<(u8, u8, Vec<[u8; 3]>)>, colors: u16, transparent: u8| {
        File::indexed(
            4,
            3,
            transparent,
            colors,
            vec![
                profile_srgb(),
                palette_old(CHUNK_PALETTE_OLD, &packets),
                TestLayer::new("a").chunk(),
                Cel::new(0, 0, 0, 4, 3, vec![0; 12]).chunk(),
            ],
        )
        .bytes()
    };
    // «Saltar» cuenta desde el final del paquete anterior: cubre 1 y 2, y después 5.
    let r = lee_ok(
        "saltos",
        &file(vec![(1, 2, vec![a, b]), (2, 1, vec![c])], 8, 5),
    );
    let palette = r.document.palette().expect("paleta");
    let default = Rgba::new(0, 0, 0, 255);
    let opaque_2 = |t: [u8; 3]| Rgba::new(t[0], t[1], t[2], 255);
    assert_eq!(
        palette.colors(),
        [
            default,
            opaque_2(a),
            opaque_2(b),
            default,
            default,
            opaque_2(c),
            default,
            default
        ]
    );
    assert_eq!(palette.transparent_index(), Some(5));

    // Número de colores 0 en la cabecera y 0 colores en el paquete: 256 entradas.
    let all: Vec<[u8; 3]> = (0..=255u8).map(|k| [k, 255 - k, 7]).collect();
    let r = lee_ok("256", &file(vec![(0, 0, all)], 0, 255));
    let palette = r.document.palette().expect("paleta");
    assert_eq!(palette.len(), 256);
    assert_eq!(palette.colors()[255], Rgba::new(255, 0, 7, 255));
    assert_eq!(palette.transparent_index(), Some(255));

    // Un índice transparente fuera de la longitud no es ninguno.
    let r = lee_ok("sin transparente", &file(vec![(0, 8, vec![a; 8])], 8, 200));
    assert_eq!(
        r.document.palette().and_then(Palette::transparent_index),
        None
    );

    // Cubrir más allá de la longitud es un error.
    lee_err("fuera", &file(vec![(3, 2, vec![a, b])], 4, 0));
}

#[test]
fn row_04_c_without_palette_only_6_bit_and_palette_in_rgba() {
    // Indexado sin ningún chunk de paleta.
    let f = File::indexed(
        4,
        3,
        0,
        4,
        vec![
            profile_srgb(),
            TestLayer::new("a").chunk(),
            Cel::new(0, 0, 0, 4, 3, vec![0; 12]).chunk(),
        ],
    );
    assert!(matches!(
        lee_err("sin paleta", &f.bytes()),
        AsepriteError::NoPalette
    ));

    // Solo el chunk 0x0011 (componentes de 0 a 63): no se inventa la escala.
    let f = File::indexed(
        4,
        3,
        0,
        4,
        vec![
            profile_srgb(),
            palette_old(CHUNK_OLD_PALETTE_6, &[(0, 4, vec![[1, 2, 3]; 4])]),
            TestLayer::new("a").chunk(),
            Cel::new(0, 0, 0, 4, 3, vec![0; 12]).chunk(),
        ],
    );
    let u = not_supported("0x0011", &f.bytes());
    assert!(matches!(u, Unsupported::Palette6Bit { .. }), "{u:?}");

    // RGBA con un chunk de paleta: se ignora y se anota, sin `lossy`.
    let mut f = base_rgba();
    f.chunks.insert(1, palette_complete(&PALETTE_4));
    let r = lee_ok("paleta en RGBA", &f.bytes());
    assert_eq!(count(&r, |k| matches!(k, Ignored::RgbaPalette { .. })), 1);
    assert!(!r.report.lossy);
    assert!(r.document.palette().is_none());
}

// ---------------------------------------------------------------------------------------
// Fila 5: lo que se ignora.
// ---------------------------------------------------------------------------------------

#[test]
fn row_05_a_each_discardable_chunk_is_noted_and_warned() {
    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, Vec<u8>, fn(&Ignored) -> bool)> = vec![
        ("etiquetas", labels(), |k| matches!(k, Ignored::Tags { .. })),
        ("datos de usuario", data_user(), |k| {
            matches!(k, Ignored::UserData { .. })
        }),
        ("cortes", cuts(), |k| matches!(k, Ignored::Slices { .. })),
        ("ficheros externos", files_external(), |k| {
            matches!(k, Ignored::ExternalFiles { .. })
        }),
        ("extra de celda", cel_extra(), |k| {
            matches!(k, Ignored::CelExtra { .. })
        }),
        ("máscara", mask(), |k| matches!(k, Ignored::Mask { .. })),
        ("trazo", stroke(), |k| matches!(k, Ignored::Path { .. })),
        ("conjunto de tiles", tileset(), |k| {
            matches!(k, Ignored::Tileset { .. })
        }),
        ("tipo desconocido", unknown(0x7777), |k| {
            matches!(k, Ignored::Unknown { .. })
        }),
        ("perfil ICC", profile(2, 0, 0, &[1, 2, 3, 4]), |k| {
            matches!(k, Ignored::ColorProfile { .. })
        }),
        ("gamma fijo", profile(1, 1, 0x1_0000, &[]), |k| {
            matches!(k, Ignored::ColorProfile { .. })
        }),
    ];
    for (name_2, extra, es) in cases {
        let r = lee_ok(name_2, &base_with(extra));
        assert!(r.report.lossy, "{name_2}: debe avisar (lossy)");
        assert_eq!(count(&r, es), 1, "{name_2}: una vez en `ignored`");
        assert_eq!(r.document.layers().len(), 2, "{name_2}: lo demás se lee");
    }

    // Cuentas: dos etiquetas, tres de usuario y dos tipos desconocidos distintos.
    let mut f = base_rgba();
    f.chunks.extend([
        labels(),
        labels(),
        data_user(),
        data_user(),
        data_user(),
        unknown(0x7777),
        unknown(0x1234),
    ]);
    let r = lee_ok("cuentas", &f.bytes());
    assert_eq!(count(&r, |k| matches!(k, Ignored::Tags { .. })), 2);
    assert_eq!(count(&r, |k| matches!(k, Ignored::UserData { .. })), 3);
    assert_eq!(count(&r, |k| matches!(k, Ignored::Unknown { .. })), 2);
    assert!(r.report.lossy);
}

#[test]
fn row_05_b_what_changes_nothing_does_not_warn_and_the_indexed_background_does() {
    // sRGB (tipo 1 sin gamma) y «sin perfil» (tipo 0): nada.
    for profile in [profile_srgb(), profile(0, 0, 0, &[])] {
        let mut f = base_rgba();
        f.chunks[0] = profile;
        let r = lee_ok("perfil sin efecto", &f.bytes());
        assert!(!r.report.lossy);
        assert!(r.report.ignored.is_empty(), "{:?}", r.report.ignored);
        assert_eq!(r.report.frames_in_file, 1);
    }

    // Cabecera (velocidad, razón de píxel, cuadrícula), duración del fotograma y banderas de
    // capa 4, 16 y 32: se ignoran sin aviso.
    let mut f = base_rgba();
    f.chunks[1] = TestLayer {
        flags: TEST_LAYER_VISIBLE
            | TEST_LAYER_EDITABLE
            | LAYER_LOCK_MOVEMENT
            | LAYER_LINKED
            | LAYER_COLLAPSED,
        ..TestLayer::new("fondo")
    }
    .chunk();
    let mut bytes = f.bytes();
    put_u_16(&mut bytes, OFF_SPEED, 7);
    bytes[OFF_REASON_PIXEL] = 2;
    bytes[OFF_REASON_PIXEL + 1] = 1;
    put_u_16(&mut bytes, OFF_GRID, 5);
    put_u_16(&mut bytes, OFF_GRID + 4, 8);
    put_u_16(&mut bytes, OFF_GRID + 6, 8);
    put_u_16(&mut bytes, OFF_FRAME_DURATION, 250);
    let r = lee_ok("campos ignorados", &bytes);
    assert!(!r.report.lossy, "{:?}", r.report.ignored);
    assert!(r.report.ignored.is_empty());
    assert_eq!(r.document.layers().len(), 2);

    // Capa de fondo (bandera 8): RGBA no avisa; indexado sin píxeles del índice transparente,
    // tampoco; indexado con ellos, sí.
    let background = TestLayer {
        flags: TEST_LAYER_VISIBLE | TEST_LAYER_EDITABLE | TEST_LAYER_BACKGROUND,
        ..TestLayer::new("fondo")
    }
    .chunk();
    let mut f = base_rgba();
    f.chunks[1] = background.clone();
    assert!(!lee_ok("fondo RGBA", &f.bytes()).report.lossy);
    let indexed = |indices: Vec<u8>| {
        File::indexed(
            4,
            3,
            0,
            4,
            vec![
                profile_srgb(),
                palette_complete(&PALETTE_4),
                background.clone(),
                Cel::new(0, 0, 0, 4, 3, indices).chunk(),
            ],
        )
        .bytes()
    };
    let r = lee_ok(
        "fondo sin transparentes",
        &indexed(vec![1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3]),
    );
    assert!(!r.report.lossy);
    let r = lee_ok(
        "fondo con transparentes",
        &indexed(vec![0, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3]),
    );
    assert!(
        r.report.lossy,
        "el índice transparente se vería opaco en un fondo"
    );
    assert!(count(&r, |k| matches!(k, Ignored::BackgroundTransparent { .. })) >= 1);
}

// ---------------------------------------------------------------------------------------
// Fila 6: lo que no se entiende.
// ---------------------------------------------------------------------------------------

const NOT_MODIFIED: &str = "el fichero no se ha modificado";

#[test]
fn row_06_a_the_unsupported_gives_unsupported_with_its_reason() {
    let rgba = |chunks: Vec<Vec<u8>>| File::rgba(4, 3, chunks).bytes();
    let cel = |c: Cel| rgba(vec![profile_srgb(), TestLayer::new("a").chunk(), c.chunk()]);
    let base = Cel::new(0, 0, 0, 4, 3, vec![255; 48]);
    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, Vec<u8>, fn(&Unsupported) -> bool)> = vec![
        (
            "capa de grupo",
            rgba(vec![
                TestLayer {
                    kind: TEST_LAYER_TYPE_GROUP,
                    ..TestLayer::new("g")
                }
                .chunk(),
            ]),
            |u| matches!(u, Unsupported::Group { .. }),
        ),
        (
            "capa de mapa de tiles",
            rgba(vec![
                TestLayer {
                    kind: TEST_LAYER_TYPE_TILEMAP,
                    tileset: Some(0),
                    ..TestLayer::new("t")
                }
                .chunk(),
            ]),
            |u| matches!(u, Unsupported::TilemapLayer { .. }),
        ),
        (
            "capa de referencia",
            rgba(vec![
                TestLayer {
                    flags: TEST_LAYER_VISIBLE | TEST_LAYER_EDITABLE | TEST_LAYER_REFERENCE,
                    ..TestLayer::new("r")
                }
                .chunk(),
            ]),
            |u| matches!(u, Unsupported::ReferenceLayer { .. }),
        ),
        (
            "celda enlazada",
            cel(Cel {
                kind: TEST_CEL_LINKED,
                ..base.clone()
            }),
            |u| matches!(u, Unsupported::LinkedCel { .. }),
        ),
        (
            "celda de mapa de tiles",
            cel(Cel {
                kind: CEL_MAP_TILES,
                pixels: vec![0; 16],
                ..base.clone()
            }),
            |u| matches!(u, Unsupported::TilemapCel { .. }),
        ),
        (
            "opacidad de celda",
            cel(Cel {
                opacity: 128,
                ..base.clone()
            }),
            |u| matches!(u, Unsupported::CelOpacity { .. }),
        ),
        (
            "z-index de celda",
            cel(Cel {
                z: 1,
                ..base.clone()
            }),
            |u| matches!(u, Unsupported::CelZIndex { .. }),
        ),
        (
            "grises de 16 bits",
            File {
                depth: DEPTH_GRAYS,
                ..File::rgba(
                    4,
                    3,
                    vec![
                        profile_srgb(),
                        TestLayer::new("a").chunk(),
                        Cel::new(0, 0, 0, 4, 3, vec![255; 24]).chunk(),
                    ],
                )
            }
            .bytes(),
            |u| matches!(u, Unsupported::Grayscale { .. }),
        ),
    ];
    for (name_2, bytes, es) in cases {
        let u = not_supported(name_2, &bytes);
        assert!(es(&u), "{name_2}: salió {u:?}");
        let error = AsepriteError::Unsupported(u);
        assert!(
            error.to_string().contains(NOT_MODIFIED),
            "{name_2}: el mensaje debe decir que el fichero no se ha modificado: {error}"
        );
    }
}

#[test]
fn row_06_b_indexed_layer_with_opacity_or_blend_is_an_error() {
    let indexed = |second: TestLayer| {
        File::indexed(
            4,
            3,
            0,
            4,
            vec![
                profile_srgb(),
                palette_complete(&PALETTE_4),
                TestLayer::new("a").chunk(),
                second.chunk(),
                Cel::new(0, 0, 0, 4, 3, vec![1; 12]).chunk(),
                Cel::new(1, 0, 0, 4, 3, vec![2; 12]).chunk(),
            ],
        )
        .bytes()
    };
    let with_opacity = indexed(TestLayer {
        opacity: 128,
        ..TestLayer::new("b")
    });
    let e = lee_err("opacidad", &with_opacity);
    assert!(
        matches!(e, AsepriteError::IndexedLayer { layer: 1 }),
        "{e:?}"
    );
    let with_blend = indexed(TestLayer {
        blend: BLEND_MULTIPLY,
        ..TestLayer::new("b")
    });
    let e = lee_err("mezcla", &with_blend);
    assert!(
        matches!(e, AsepriteError::IndexedLayer { layer: 1 }),
        "{e:?}"
    );
    // Con opacidad 255 y Normal, bien.
    lee_ok("normal", &indexed(TestLayer::new("b")));
}

#[test]
fn row_06_c_blend_codes_from_0_to_40_follow_the_table() {
    // Spec: Layer Chunk, tabla «Blend mode».
    let valid = |code_2: u16| match code_2 {
        BLEND_NORMAL => Some(BlendMode::Normal),
        BLEND_MULTIPLY => Some(BlendMode::Multiply),
        BLEND_PATTERN => Some(BlendMode::Screen),
        BLEND_OVERLAY => Some(BlendMode::Overlay),
        BLEND_DARKEN => Some(BlendMode::Darken),
        BLEND_LIGHTEN => Some(BlendMode::Lighten),
        BLEND_DIFFERENCE => Some(BlendMode::Difference),
        BLEND_SUM => Some(BlendMode::Add),
        _ => None,
    };
    let mut accepted = 0;
    for code_2 in 0..=40u16 {
        let bytes = File::rgba(
            4,
            3,
            vec![
                profile_srgb(),
                TestLayer {
                    blend: code_2,
                    ..TestLayer::new("m")
                }
                .chunk(),
                Cel::new(0, 0, 0, 4, 3, vec![255; 48]).chunk(),
            ],
        )
        .bytes();
        match valid(code_2) {
            Some(mode) => {
                let r = lee_ok(&format!("código {code_2}"), &bytes);
                assert_eq!(r.document.layers()[0].blend(), mode, "código {code_2}");
                accepted += 1;
            }
            None => match not_supported(&format!("código {code_2}"), &bytes) {
                Unsupported::BlendMode { code, .. } => assert_eq!(code, code_2),
                other_2 => panic!("código {code_2}: salió {other_2:?}"),
            },
        }
    }
    assert_eq!(accepted, 8);
}

// ---------------------------------------------------------------------------------------
// Fila 7: fotogramas.
// ---------------------------------------------------------------------------------------

#[test]
fn row_07_a_with_three_frames_the_first_is_read_and_it_warns() {
    let mut f = base_rgba();
    f.frames = 3;
    // Detrás del primero, basura: no se mira (ni se recorren sus cabeceras).
    f.rest_2 = (0..90u8).map(|i| i.wrapping_mul(37) ^ 0x5C).collect();
    let r = lee_ok("3 fotogramas", &f.bytes());
    assert_eq!(r.report.frames_in_file, 3);
    assert!(r.report.lossy);
    assert_eq!(r.document.layers().len(), 2);
    let a = lee_ok("1 fotograma", &base_rgba().bytes());
    assert_eq!(a.report.frames_in_file, 1);
    assert!(same_content(&r.document, &a.document).is_ok());
}

#[test]
fn row_07_b_zero_frames_and_extra_bytes_are_malformed() {
    let mut f = base_rgba();
    f.frames = 0;
    let e = lee_err("0 fotogramas", &f.bytes());
    assert!(matches!(e, AsepriteError::Malformed(_)), "{e:?}");

    // Un fotograma declarado, pero el fichero (con su tamaño al día) trae bytes detrás.
    let mut f = base_rgba();
    f.rest_2 = vec![0; 16];
    let e = lee_err("bytes de más", &f.bytes());
    assert!(matches!(e, AsepriteError::Malformed(_)), "{e:?}");
}

// ---------------------------------------------------------------------------------------
// Fila 8: celdas (la parte con el corpus está en px-golden).
// ---------------------------------------------------------------------------------------

#[test]
fn row_08_cels_of_types_0_and_2_shifted_clipped_and_outside() {
    let (width, height) = (6usize, 5usize);
    // Píxeles de una celda: color distinto por píxel; alfa 0 en uno de cada tres.
    let data = |cw: usize, ch: usize| {
        pixels_rgba(cw, ch, |x, y| {
            let k = (y * cw + x) as u8;
            [
                k.wrapping_add(1),
                50,
                60,
                if k.is_multiple_of(3) { 0 } else { 255 },
            ]
        })
    };
    let opaque = |cw: usize, ch: usize| pixels_rgba(cw, ch, |x, _| [x as u8 + 1, 7, 8, 255]);
    let transparent = |cw: usize, ch: usize| pixels_rgba(cw, ch, |x, _| [x as u8 + 9, 7, 8, 0]);
    // (nombre, tipo, x, y, ancho, alto, píxeles, ¿queda vacía?)
    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, u16, i16, i16, usize, usize, Vec<u8>, bool)> = vec![
        (
            "desplazada comprimida",
            TEST_CEL_COMPRESSED,
            1,
            1,
            3,
            2,
            data(3, 2),
            false,
        ),
        (
            "desplazada cruda",
            TEST_CEL_RAW,
            1,
            1,
            3,
            2,
            data(3, 2),
            false,
        ),
        (
            "recorte arriba-izquierda",
            TEST_CEL_COMPRESSED,
            -2,
            -1,
            4,
            4,
            opaque(4, 4),
            false,
        ),
        (
            "recorte abajo-derecha",
            TEST_CEL_COMPRESSED,
            4,
            3,
            4,
            4,
            opaque(4, 4),
            false,
        ),
        (
            "recorte por los cuatro lados",
            TEST_CEL_COMPRESSED,
            -2,
            -2,
            10,
            9,
            data(10, 9),
            false,
        ),
        (
            "de 0 píxeles",
            TEST_CEL_COMPRESSED,
            1,
            1,
            0,
            0,
            Vec::new(),
            true,
        ),
        (
            "del todo fuera por la derecha",
            TEST_CEL_COMPRESSED,
            10,
            10,
            3,
            3,
            opaque(3, 3),
            true,
        ),
        (
            "del todo fuera por la izquierda",
            TEST_CEL_COMPRESSED,
            -5,
            -5,
            3,
            3,
            opaque(3, 3),
            true,
        ),
        (
            "toda transparente",
            TEST_CEL_COMPRESSED,
            0,
            0,
            6,
            5,
            transparent(6, 5),
            true,
        ),
        // Lo único opaco queda fuera: dentro del lienzo todo es transparente.
        (
            "transparente dentro y opaca fuera",
            TEST_CEL_COMPRESSED,
            -3,
            0,
            4,
            1,
            [opaque(3, 1), transparent(1, 1)].concat(),
            true,
        ),
    ];
    let mut chunks = vec![profile_srgb()];
    for (name_2, ..) in &cases {
        chunks.push(TestLayer::new(name_2).chunk());
    }
    for (i, (_, kind, x, y, cw, ch, pixels, _)) in cases.iter().enumerate() {
        let cel = Cel {
            kind: *kind,
            ..Cel::new(i as u16, *x, *y, *cw as u16, *ch as u16, pixels.clone())
        };
        chunks.push(cel.chunk());
    }
    let r = lee_ok(
        "celdas",
        &File::rgba(width as u16, height as u16, chunks).bytes(),
    );
    let doc = &r.document;
    assert_eq!(doc.layers().len(), cases.len());
    for (i, (name_2, _, x, y, cw, ch, pixels, empty)) in cases.iter().enumerate() {
        let expected = placement_expected(
            width,
            height,
            (i32::from(*x), i32::from(*y)),
            (*cw, *ch),
            pixels,
            &[0, 0, 0, 0],
            rgba_transparent,
        );
        assert_eq!(
            expected.is_none(),
            *empty,
            "{name_2}: la tabla de la prueba"
        );
        let image = doc.image(doc.layers()[i].cel());
        assert_eq!(image.map(Image::bytes), expected.as_deref(), "{name_2}");
    }
    // Los tipos 0 y 2 de la misma celda dan la misma imagen.
    let (a, b) = (doc.layers()[0].cel(), doc.layers()[1].cel());
    assert_eq!(doc.image(a), doc.image(b));
}

// ---------------------------------------------------------------------------------------
// Fila 9: el lector no se fía.
// ---------------------------------------------------------------------------------------

#[test]
fn row_09_a_each_prefix_of_a_valid_file_gives_an_error() {
    let files = [
        ("RGBA a mano", base_rgba().bytes()),
        ("indexado a mano", base_indexed().bytes()),
        ("mixed-rgba", golden("mixed-rgba.aseprite")),
    ];
    for (name_2, bytes) in &files {
        lee_ok(name_2, bytes);
        for n in 0..bytes.len() {
            assert!(
                lee(&bytes[..n]).is_err(),
                "{name_2}: el prefijo de {n} bytes (de {}) debe dar error",
                bytes.len()
            );
        }
    }
    // La cabecera sola, truncada: nunca pánico.
    for n in 0..12 {
        assert!(
            aseprite_dimensions(&golden("mixed-rgba.aseprite")[..n]).is_err(),
            "{n}"
        );
    }
}

#[test]
fn row_09_b_header_frame_chunks_layers_and_canvas_wrong() {
    let is_any: fn(&AsepriteError) -> bool = |_| true;
    let base = base_rgba().bytes();
    let patched = |f: &dyn Fn(&mut Vec<u8>)| {
        let mut b = base.clone();
        f(&mut b);
        b
    };
    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, Vec<u8>, fn(&AsepriteError) -> bool)> = vec![
        (
            "tamaño de cabecera distinto",
            patched(&|b| {
                let length = b.len() as u32;
                put_u_32(b, OFF_TAM_FILE, length + 1);
            }),
            is_any,
        ),
        (
            "magia del fichero",
            patched(&|b| put_u_16(b, OFF_MAGIC, 0xA5E1)),
            |e| matches!(e, AsepriteError::NotAseprite | AsepriteError::Malformed(_)),
        ),
        (
            "magia del fotograma",
            patched(&|b| put_u_16(b, OFF_FRAME_MAGIC, 0xF1FB)),
            is_any,
        ),
        ("ancho 0", patched(&|b| put_u_16(b, OFF_WIDTH, 0)), is_any),
        ("alto 0", patched(&|b| put_u_16(b, OFF_HEIGHT, 0)), is_any),
        (
            "profundidad de 24 bits",
            patched(&|b| put_u_16(b, OFF_DEPTH, 24)),
            is_any,
        ),
        (
            "fotograma que sale del fichero",
            patched(&|b| {
                let length = b.len() as u32;
                put_u_32(b, OFF_FRAME_TAM, length);
            }),
            is_any,
        ),
        (
            "65 537 chunks declarados con el fichero corto",
            patched(&|b| {
                put_u_16(b, OFF_FRAME_CHUNKS_OLD, 0xFFFF);
                put_u_32(b, OFF_FRAME_CHUNKS_NEW, 65_537);
            }),
            |e| matches!(e, AsepriteError::TooManyChunks),
        ),
        (
            "chunk de menos de 6 bytes",
            base_with(vec![5, 0, 0, 0, 0x19, 0x20]),
            is_any,
        ),
        (
            "chunk que sale del fotograma",
            base_with(vec![0xE8, 0x03, 0, 0, 0x19, 0x20, 1, 2, 3, 4]),
            is_any,
        ),
        (
            "257 capas",
            File::rgba(
                4,
                3,
                (0..257)
                    .map(|i| TestLayer::new(&format!("c{i}")).chunk())
                    .collect(),
            )
            .bytes(),
            |e| matches!(e, AsepriteError::TooManyLayers),
        ),
        (
            "nombre de 256 bytes",
            File::rgba(4, 3, vec![TestLayer::new(&"a".repeat(256)).chunk()]).bytes(),
            |e| matches!(e, AsepriteError::LayerName),
        ),
        (
            "nombre que no es UTF-8",
            File::rgba(
                4,
                3,
                vec![
                    TestLayer {
                        name_2: vec![0xFF, 0xFE, 0x41],
                        ..TestLayer::new("x")
                    }
                    .chunk(),
                ],
            )
            .bytes(),
            is_any,
        ),
        (
            "capa con nivel hijo distinto de 0",
            File::rgba(
                4,
                3,
                vec![
                    TestLayer {
                        level: 1,
                        ..TestLayer::new("x")
                    }
                    .chunk(),
                ],
            )
            .bytes(),
            is_any,
        ),
        (
            "sin ninguna capa",
            File::rgba(4, 3, vec![profile_srgb()]).bytes(),
            is_any,
        ),
        // `from_layers` es el único camino: su error sale como `Document(..)`, no como pánico.
        (
            "indexado con dos capas y sin índice transparente válido",
            File::indexed(
                4,
                3,
                200,
                4,
                vec![
                    profile_srgb(),
                    palette_complete(&PALETTE_4),
                    TestLayer::new("a").chunk(),
                    TestLayer::new("b").chunk(),
                    Cel::new(0, 0, 0, 4, 3, vec![1; 12]).chunk(),
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
    for (name_2, bytes, es) in cases {
        let e = lee_err(name_2, &bytes);
        assert!(es(&e), "{name_2}: salió {e:?}");
    }

    // Un lienzo de más de 2^26 píxeles: el error dice el límite y se rechaza antes de reservar.
    let large = File::rgba(8193, 8192, vec![TestLayer::new("x").chunk()]).bytes();
    let e = lee_err("lienzo grande", &large);
    assert!(matches!(e, AsepriteError::TooLarge { .. }), "{e:?}");
    assert!(e.to_string().contains("67108864"), "{e}");
    assert_eq!(aseprite_dimensions(&large).ok(), Some((8193, 8192)));
    // Y el límite que pasa el llamador manda.
    let small = AsepriteLimits {
        max_pixels: 100,
        ..AsepriteLimits::default()
    };
    let bytes = File::rgba(11, 10, vec![TestLayer::new("x").chunk()]).bytes();
    match read_aseprite(&bytes, &small) {
        Err(e @ AsepriteError::TooLarge { .. }) => assert!(e.to_string().contains("100"), "{e}"),
        other_2 => panic!("lienzo sobre el límite del llamador: {:?}", other_2.err()),
    }
}

#[test]
fn row_09_c_malformed_cels() {
    let ok = || {
        Cel::new(
            0,
            0,
            0,
            4,
            3,
            pixels_rgba(4, 3, |x, y| [x as u8, y as u8, 9, 255]),
        )
    };
    let with = |chunks: Vec<Vec<u8>>| {
        let mut v = vec![profile_srgb(), TestLayer::new("a").chunk()];
        v.extend(chunks);
        File::rgba(4, 3, v).bytes()
    };
    let zlib_ok = zlib(&ok().pixels);
    let es_cel_data: fn(&AsepriteError) -> bool =
        |e| matches!(e, AsepriteError::CelData { layer: 0 });
    let is_any: fn(&AsepriteError) -> bool = |_| true;
    let mut adler = zlib_ok.clone();
    *adler.last_mut().expect("zlib no vacío") ^= 0xFF;
    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, Vec<u8>, fn(&AsepriteError) -> bool)> = vec![
        (
            "Adler-32 erróneo",
            with(vec![
                Cel {
                    compressed: Some(adler),
                    ..ok()
                }
                .chunk(),
            ]),
            es_cel_data,
        ),
        (
            "zlib sin su Adler-32 (corto)",
            with(vec![
                Cel {
                    compressed: Some(zlib_ok[..zlib_ok.len() - 4].to_vec()),
                    ..ok()
                }
                .chunk(),
            ]),
            es_cel_data,
        ),
        (
            "zlib cortado por la mitad",
            with(vec![
                Cel {
                    compressed: Some(zlib_ok[..zlib_ok.len() / 2].to_vec()),
                    ..ok()
                }
                .chunk(),
            ]),
            es_cel_data,
        ),
        (
            "zlib con un byte de más (largo)",
            with(vec![
                Cel {
                    pixels: vec![7; 49],
                    ..ok()
                }
                .chunk(),
            ]),
            es_cel_data,
        ),
        (
            "zlib con un byte de menos",
            with(vec![
                Cel {
                    pixels: vec![7; 47],
                    ..ok()
                }
                .chunk(),
            ]),
            es_cel_data,
        ),
        (
            "1000 × 1000 con unos pocos bytes de zlib (más de 1032×)",
            with(vec![
                Cel {
                    width: 1000,
                    height: 1000,
                    compressed: Some(zlib(&[0])),
                    ..ok()
                }
                .chunk(),
            ]),
            es_cel_data,
        ),
        (
            "celda repetida",
            with(vec![ok().chunk(), ok().chunk()]),
            is_any,
        ),
        (
            "índice de capa fuera de las capas",
            with(vec![Cel { layer_2: 1, ..ok() }.chunk()]),
            is_any,
        ),
        (
            "celda cruda con un byte de menos",
            with(vec![
                Cel {
                    kind: TEST_CEL_RAW,
                    pixels: vec![7; 47],
                    ..ok()
                }
                .chunk(),
            ]),
            is_any,
        ),
        (
            "celda cruda con un byte de más",
            with(vec![
                Cel {
                    kind: TEST_CEL_RAW,
                    pixels: vec![7; 49],
                    ..ok()
                }
                .chunk(),
            ]),
            is_any,
        ),
        (
            "índice de píxel fuera de la paleta",
            File::indexed(
                4,
                3,
                0,
                4,
                vec![
                    profile_srgb(),
                    palette_complete(&PALETTE_4),
                    TestLayer::new("a").chunk(),
                    Cel::new(0, 0, 0, 4, 3, vec![0, 1, 2, 3, 0, 1, 2, 4, 0, 1, 2, 3]).chunk(),
                ],
            )
            .bytes(),
            is_any,
        ),
    ];
    for (name_2, bytes, es) in cases {
        let e = lee_err(name_2, &bytes);
        assert!(es(&e), "{name_2}: salió {e:?}");
    }
}

/// Memoria: 8 capas con un píxel cada una en un lienzo de 8192 × 8192 son 8 celdas de 256 MiB
/// (2 GiB, lo que el núcleo deja crear) y se reabren; una novena pasa del tope y da `TooLarge`
/// con el límite en el mensaje. No es del CI.
#[test]
#[ignore = "reserva 2 GiB; se corre una vez al cerrar la unidad"]
fn row_09_eight_cels_of_256_mib_reopen_and_the_ninth_gives_too_large() {
    let file = |layers: usize| {
        let mut chunks = vec![profile_srgb()];
        chunks.extend((0..layers).map(|i| TestLayer::new(&format!("c{i}")).chunk()));
        chunks.extend(
            (0..layers).map(|i| Cel::new(i as u16, 0, 0, 1, 1, vec![1, 2, 3, 255]).chunk()),
        );
        File::rgba(8192, 8192, chunks).bytes()
    };
    let r = lee_ok("8 capas", &file(8));
    assert_eq!(r.document.layers().len(), 8);
    assert_eq!(r.document.materialized_bytes(), 8 * 8192 * 8192 * 4);
    drop(r);
    let e = lee_err("9 capas", &file(9));
    assert!(
        matches!(&e, AsepriteError::TooLarge { limit } if *limit == 2u64 << 30),
        "{e:?}"
    );
    assert!(e.to_string().contains("2147483648"), "{e}");
}

// ---------------------------------------------------------------------------------------
// Fila 10: escritura, contra un lector mínimo escrito solo con las tablas.
// ---------------------------------------------------------------------------------------

fn code_blend(mode: BlendMode) -> u16 {
    match mode {
        BlendMode::Normal => BLEND_NORMAL,
        BlendMode::Multiply => BLEND_MULTIPLY,
        BlendMode::Screen => BLEND_PATTERN,
        BlendMode::Overlay => BLEND_OVERLAY,
        BlendMode::Darken => BLEND_DARKEN,
        BlendMode::Lighten => BLEND_LIGHTEN,
        BlendMode::Difference => BLEND_DIFFERENCE,
        BlendMode::Add => BLEND_SUM,
        _ => panic!("modo de mezcla sin código en la prueba"),
    }
}

/// Rectángulo mínimo `(x0, y0, x1, y1)` (extremo final excluido) de los píxeles no transparentes.
fn rectangle_minimum(image: &Image, transparent: impl Fn(&[u8]) -> bool) -> Option<[usize; 4]> {
    let (width, bpp) = (image.width() as usize, image.mode().bytes_per_pixel());
    let mut rect: Option<[usize; 4]> = None;
    for (k, pixel) in image.bytes().chunks(bpp).enumerate() {
        if transparent(pixel) {
            continue;
        }
        let (x, y) = (k % width, k / width);
        rect = Some(match rect {
            None => [x, y, x + 1, y + 1],
            Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1)],
        });
    }
    rect
}

/// Comprueba campo a campo el fichero escrito contra el documento y devuelve lo leído.
fn checks_writing(doc: &Document, bytes: &[u8]) -> FileRead {
    let f = parsea(bytes);
    let (cab, frame) = (&f.header, &f.frame_2);
    let indexed = doc.mode() == Mode::Indexed;
    // Spec: Header.
    assert_eq!(cab.tam_file as usize, bytes.len());
    assert_eq!(cab.magic, TEST_MAGIC_FILE);
    assert_eq!(cab.frames, 1);
    assert_eq!(
        (u32::from(cab.width), u32::from(cab.height)),
        (doc.width(), doc.height())
    );
    assert_eq!(
        cab.depth,
        if indexed {
            TEST_DEPTH_INDEXED
        } else {
            TEST_DEPTH_RGBA
        }
    );
    assert_eq!(cab.flags, HEADER_OPACITY_VALID);
    assert_eq!(cab.speed, 100);
    assert_eq!(cab.dwords_zero, [0, 0]);
    assert_eq!(cab.ignored, [0, 0, 0]);
    assert_eq!(cab.pixel, (1, 1));
    assert_eq!(cab.grid, (0, 0, 16, 16));
    assert!(cab.future_to_zero);
    // Spec: Frames.
    assert_eq!(frame.tam as usize, bytes.len() - TAM_HEADER);
    assert_eq!(frame.magic, TEST_MAGIC_FRAME);
    assert_eq!(frame.duration, 100);
    assert_eq!(frame.future, [0, 0]);
    assert_eq!(u32::from(frame.chunks_old), frame.chunks_new);

    // Spec: Chunk Types, orden: perfil, paleta (solo indexado), capas y celdas.
    let transparent = |p: &[u8]| -> bool {
        match doc.palette().and_then(Palette::transparent_index) {
            Some(t) if indexed => p[0] == t,
            _ => rgba_transparent(p),
        }
    };
    let rects: Vec<Option<[usize; 4]>> = doc
        .layers()
        .iter()
        .map(|l| {
            doc.image(l.cel())
                .and_then(|i| rectangle_minimum(i, transparent))
        })
        .collect();
    let n = doc.layers().len();
    let cels = rects.iter().flatten().count();
    let mut types = vec![CHUNK_PROFILE];
    if indexed {
        types.push(TEST_CHUNK_PALETTE);
    }
    types.extend(std::iter::repeat_n(TEST_CHUNK_LAYER, n));
    types.extend(std::iter::repeat_n(TEST_CHUNK_CEL, cels));
    let real: Vec<u16> = f.chunks.iter().map(|(t, _)| *t).collect();
    assert_eq!(real, types, "orden de los chunks");

    // Perfil sRGB (Spec: Color Profile Chunk): tipo 1, sin gamma.
    assert_eq!(f.chunks[0].1[..4], [1, 0, 0, 0]);
    // Paleta (Spec: Palette Chunk).
    let mut next = 1;
    if let Some(palette) = doc.palette().filter(|_| indexed) {
        let data = &f.chunks[1].1;
        let mut l = Reader::new_2(data);
        assert_eq!(l.u32() as usize, palette.len());
        assert_eq!((l.u32(), l.u32() as usize), (0, palette.len() - 1));
        assert_eq!(l.bytes(8), [0; 8]);
        for (i, c) in palette.colors().iter().enumerate() {
            assert_eq!(l.u16(), 0, "entrada {i} sin nombre");
            assert_eq!(l.bytes(4), [c.r, c.g, c.b, c.a], "entrada {i}");
        }
        assert_eq!(l.rest_2(), 0);
        assert_eq!(usize::from(cab.colors), palette.len());
        assert_eq!(Some(cab.transparent), palette.transparent_index());
        next = 2;
    }
    // Capas (Spec: Layer Chunk).
    for (i, layer_2) in doc.layers().iter().enumerate() {
        let c = decodes_layer(&f.chunks[next + i].1);
        let flags = u16::from(layer_2.visible()) * TEST_LAYER_VISIBLE
            + u16::from(!layer_2.locked()) * TEST_LAYER_EDITABLE;
        assert_eq!(c.flags, flags, "banderas de {}", layer_2.name());
        assert_eq!(
            (c.kind, c.level, c.width, c.height),
            (TEST_LAYER_TYPE_NORMAL, 0, 0, 0)
        );
        assert_eq!(
            c.blend,
            code_blend(layer_2.blend()),
            "mezcla de {}",
            layer_2.name()
        );
        assert_eq!(c.opacity, layer_2.opacity());
        assert_eq!(c.future, [0, 0, 0]);
        assert_eq!(c.name_2, layer_2.name());
    }
    // Celdas (Spec: Cel Chunk): rectángulo mínimo, sin chunk para las vacías o transparentes.
    let bpp = doc.mode().bytes_per_pixel();
    let mut views = Vec::new();
    for (_, data) in &f.chunks[next + n..] {
        let c = decodes_cel(data);
        let layer_2 = usize::from(c.layer_2);
        let [x0, y0, x1, y1] = rects[layer_2].expect("solo hay celda si hay píxeles visibles");
        assert_eq!(
            (c.x, c.y),
            (x0 as i16, y0 as i16),
            "origen de la celda {layer_2}"
        );
        assert_eq!(
            (usize::from(c.width), usize::from(c.height)),
            (x1 - x0, y1 - y0)
        );
        assert_eq!(
            (c.opacity, c.kind, c.z, c.future),
            (255, TEST_CEL_COMPRESSED, 0, [0; 5])
        );
        let image = doc
            .image(doc.layers()[layer_2].cel())
            .expect("celda con imagen");
        let width = image.width() as usize;
        let mut expected_2_2 = Vec::new();
        for y in y0..y1 {
            let from = (y * width + x0) * bpp;
            expected_2_2.extend(&image.bytes()[from..from + (x1 - x0) * bpp]);
        }
        assert_eq!(c.pixels, expected_2_2, "píxeles de la celda {layer_2}");
        views.push(layer_2);
    }
    let mut ordered = views.clone();
    ordered.sort_unstable();
    ordered.dedup();
    assert_eq!(ordered.len(), views.len(), "una celda por capa");
    f
}

#[test]
fn row_10_a_writes_rgba_field_by_field_with_the_eight_blends() {
    let (width, height) = (5u32, 4u32);
    let empty_2 = || vec![0u8; 5 * 4 * 4];
    let with = |pixels: &[(usize, usize, [u8; 4])]| {
        let mut b = empty_2();
        for &(x, y, p) in pixels {
            b[(y * 5 + x) * 4..][..4].copy_from_slice(&p);
        }
        image_rgba(width, height, b)
    };
    let point = |x: usize, y: usize| with(&[(x, y, [200, 100, 50, 255])]);
    let layers = vec![
        // Rectángulo mínimo (1,1)-(3,2): un píxel transparente con RGB dentro y otro fuera.
        layer_simple(
            "uno",
            Some(with(&[
                (1, 1, [1, 2, 3, 255]),
                (2, 1, [7, 8, 9, 0]),
                (3, 2, [4, 5, 6, 128]),
                (0, 3, [4, 4, 4, 0]),
            ])),
        ),
        layer_2(
            "dos",
            false,
            128,
            BlendMode::Multiply,
            true,
            Some(image_rgba(width, height, vec![9; 80])),
        ),
        layer_simple("tres", None),
        // Materializada y totalmente transparente (con RGB distinto de 0): no escribe celda.
        layer_simple(
            "cuatro",
            Some(with(&[(2, 2, [5, 5, 5, 0]), (0, 0, [6, 6, 6, 0])])),
        ),
        layer_2(
            "cinco",
            true,
            17,
            BlendMode::Screen,
            false,
            Some(point(0, 0)),
        ),
        layer_2(
            "seis",
            true,
            255,
            BlendMode::Overlay,
            false,
            Some(point(4, 3)),
        ),
        layer_2(
            "siete",
            true,
            255,
            BlendMode::Darken,
            false,
            Some(point(2, 1)),
        ),
        layer_2(
            "ocho",
            true,
            255,
            BlendMode::Lighten,
            false,
            Some(point(1, 3)),
        ),
        layer_2("nueve", true, 255, BlendMode::Add, false, Some(point(3, 0))),
        layer_2(
            "diez",
            true,
            255,
            BlendMode::Difference,
            false,
            Some(point(4, 0)),
        ),
    ];
    let doc = document(width, height, Mode::Rgba, None, layers);
    let bytes = match write_aseprite(&doc) {
        Ok(b) => b,
        Err(e) => panic!("escribir: {e:?}"),
    };
    let f = checks_writing(&doc, &bytes);
    // Los códigos de la tabla de Spec: Layer Chunk, a mano: Normal 0, Multiply 1, Screen 2,
    // Overlay 3, Darken 4, Lighten 5, Addition 16 y Difference 10.
    let codes: Vec<u16> = (0..10)
        .map(|i| decodes_layer(&f.chunks[1 + i].1).blend)
        .collect();
    assert_eq!(codes, [0, 1, 0, 0, 2, 3, 4, 5, 16, 10]);
    // Sin chunk de paleta en RGBA, y solo 8 celdas: «tres» y «cuatro» no escriben.
    assert!(
        f.chunks
            .iter()
            .all(|(t, _)| *t != TEST_CHUNK_PALETTE && *t != CHUNK_PALETTE_OLD)
    );
    assert_eq!(
        f.chunks
            .iter()
            .filter(|(t, _)| *t == TEST_CHUNK_CEL)
            .count(),
        8
    );
}

#[test]
fn row_10_b_writes_indexed_with_a_new_palette_of_256_and_layer_flags() {
    let colors: Vec<[u8; 4]> = (0..=255u8)
        .map(|i| {
            [
                i,
                255 - i,
                i.wrapping_mul(7),
                if i % 5 == 0 { 100 } else { 255 },
            ]
        })
        .collect();
    let palette = palette_of(&colors, Some(255));
    let (width, height) = (6u32, 3u32);
    let indices =
        |f: &dyn Fn(usize) -> u8| image_indexed(width, height, &palette, (0..18).map(f).collect());
    let layers = vec![
        layer_2(
            "a",
            true,
            255,
            BlendMode::Normal,
            false,
            Some(indices(&|k| (k * 13 % 250) as u8)),
        ),
        layer_2(
            "b",
            true,
            255,
            BlendMode::Normal,
            true,
            Some(indices(&|k| if k == 4 || k == 9 { 7 } else { 255 })),
        ),
        layer_2("c", false, 255, BlendMode::Normal, false, None),
        layer_2(
            "d",
            false,
            255,
            BlendMode::Normal,
            true,
            Some(indices(&|_| 255)),
        ),
    ];
    let doc = document(width, height, Mode::Indexed, Some(palette), layers);
    let bytes = match write_aseprite(&doc) {
        Ok(b) => b,
        Err(e) => panic!("escribir: {e:?}"),
    };
    let f = checks_writing(&doc, &bytes);
    assert_eq!(f.header.colors, 256);
    assert_eq!(f.header.transparent, 255);
    // Bit 1 = visible y bit 2 = editable: visible y libre 3, visible y bloqueada 1, oculta y libre
    // 2, oculta y bloqueada 0.
    let flags: Vec<u16> = (0..4)
        .map(|i| decodes_layer(&f.chunks[2 + i].1).flags)
        .collect();
    assert_eq!(flags, [3, 1, 2, 0]);
}

#[test]
fn row_10_c_write_errors_gray_without_transparent_and_huge_canvas() {
    for mode in [Mode::Gray, Mode::GrayAlpha] {
        let doc = Document::new(Image::new(3, 3, mode).expect("imagen"));
        match write_aseprite(&doc) {
            Err(AsepriteError::Unsupported(u)) => {
                assert!(
                    matches!(u, Unsupported::Grayscale { .. }),
                    "{mode:?}: {u:?}"
                );
                assert!(
                    AsepriteError::Unsupported(u)
                        .to_string()
                        .contains(NOT_MODIFIED)
                );
            }
            other_2 => panic!(
                "{mode:?}: se esperaba Unsupported(Grayscale) y salió {:?}",
                other_2.err()
            ),
        }
    }

    let palette = palette_of(&PALETTE_4, None);
    let doc = Document::new(image_indexed(2, 2, &palette, vec![0; 4]));
    match write_aseprite(&doc) {
        Err(e @ AsepriteError::NoTransparentIndex { .. }) => assert_eq!(
            e.to_string(),
            "esta imagen indexada no tiene entrada transparente: fíjala con Fijar transparente o convierte a RGBA"
        ),
        other_2 => panic!("indexado sin transparente: {:?}", other_2.err()),
    }

    for (width, height) in [(65_536u32, 1u32), (1, 65_536)] {
        let doc = Document::new(Image::new(width, height, Mode::Rgba).expect("imagen"));
        let e = write_aseprite(&doc).err();
        assert!(
            matches!(e, Some(AsepriteError::TooLarge { .. })),
            "{width}×{height}: {e:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 11: ida y vuelta.
// ---------------------------------------------------------------------------------------

fn writes(doc: &Document) -> Vec<u8> {
    match write_aseprite(doc) {
        Ok(b) => b,
        Err(e) => panic!("escribir: {e:?}"),
    }
}

/// Escribe, lee y exige `same_content`; devuelve el documento leído y los bytes.
fn round_trip(context: &str, doc: &Document) -> (Document, Vec<u8>) {
    let bytes = writes(doc);
    let read = lee_ok(context, &bytes);
    assert!(
        !read.report.lossy,
        "{context}: lo que escribe Pixoko no avisa"
    );
    assert_eq!(read.report.frames_in_file, 1);
    if let Err(reason) = same_content(doc, &read.document) {
        panic!("{context}: {reason}");
    }
    (read.document, bytes)
}

#[test]
fn row_11_a_rgba_of_three_layers_with_empty_cel_and_rgb_under_alpha_0() {
    let (width, height) = (6u32, 5u32);
    let background = pixels_rgba(6, 5, |x, y| {
        [
            x as u8 * 40,
            y as u8 * 50,
            99,
            [255, 128, 0, 77][(x + y) % 4],
        ]
    });
    // Dibujo disperso: el RGB bajo alfa 0 dentro de lo dibujado debe volver tal cual.
    let mut drawing = vec![0u8; 6 * 5 * 4];
    for (x, y, p) in [
        (1usize, 1usize, [10u8, 20, 30, 255]),
        (2, 1, [11, 12, 13, 0]),
        (4, 3, [200, 100, 50, 90]),
    ] {
        drawing[(y * 6 + x) * 4..][..4].copy_from_slice(&p);
    }
    let transparent = pixels_rgba(6, 5, |x, _| [x as u8 + 1, 2, 3, 0]);
    let doc = document(
        width,
        height,
        Mode::Rgba,
        None,
        vec![
            layer_2(
                "fondo",
                true,
                200,
                BlendMode::Multiply,
                true,
                Some(image_rgba(width, height, background)),
            ),
            layer_2("vacía", false, 90, BlendMode::Screen, false, None),
            layer_2(
                "transparente",
                true,
                255,
                BlendMode::Overlay,
                false,
                Some(image_rgba(width, height, transparent)),
            ),
            layer_2(
                "dibujo",
                true,
                255,
                BlendMode::Normal,
                false,
                Some(image_rgba(width, height, drawing)),
            ),
        ],
    );
    let (read, _) = round_trip("RGBA de capas", &doc);
    // La materializada y transparente vuelve vacía (equivalentes); la vacía, también.
    assert!(read.image(read.layers()[1].cel()).is_none());
    assert!(read.image(read.layers()[2].cel()).is_none());
    // El RGB bajo alfa 0 dentro del rectángulo mínimo se conserva byte a byte.
    let drawing = read
        .image(read.layers()[3].cel())
        .expect("el dibujo tiene celda");
    assert_eq!(
        drawing.get_pixel(2, 1),
        Ok(Pixel::Rgba(Rgba::new(11, 12, 13, 0)))
    );
    assert_eq!(
        drawing.get_pixel(4, 3),
        Ok(Pixel::Rgba(Rgba::new(200, 100, 50, 90)))
    );
}

#[test]
fn row_11_b_indexed_of_256_colors_with_alpha_and_a_255_byte_multibyte_name() {
    let colors: Vec<[u8; 4]> = (0..=255u8)
        .map(|i| [i, i ^ 0x5A, i.wrapping_mul(3), 255 - i / 2])
        .collect();
    let palette = palette_of(&colors, Some(255));
    let (width, height) = (7u32, 4u32);
    // 127 veces «ñ» (2 bytes) y una «a»: 255 bytes.
    let name_2 = format!("{}a", "ñ".repeat(127));
    assert_eq!(name_2.len(), 255);
    let layer_a = image_indexed(
        width,
        height,
        &palette,
        (0..28).map(|k| (k * 9 % 256) as u8).collect(),
    );
    let layer_b = image_indexed(
        width,
        height,
        &palette,
        (0..28)
            .map(|k| if k % 4 == 0 { 255 } else { k as u8 })
            .collect(),
    );
    let doc = document(
        width,
        height,
        Mode::Indexed,
        Some(palette),
        vec![
            layer_simple(&name_2, Some(layer_a)),
            layer_2("arriba", false, 255, BlendMode::Normal, true, Some(layer_b)),
            layer_simple("vacía", None),
        ],
    );
    let (read, _) = round_trip("indexado de 256", &doc);
    assert_eq!(read.layers()[0].name(), name_2);
    assert_eq!(
        read.palette().and_then(Palette::transparent_index),
        Some(255)
    );
    assert_eq!(read.palette().map(Palette::len), Some(256));
}

#[test]
fn row_11_c_extremes_and_second_write_with_the_same_bytes() {
    // 256 capas de 1 × 1, la última encima.
    let layers: Vec<LayerSpec> = (0..256)
        .map(|i| {
            let image = (i % 3 != 0).then(|| image_rgba(1, 1, vec![i as u8, 2, 3, 255]));
            layer_2(
                &format!("c{i}"),
                i % 2 == 0,
                255 - i as u8 % 7,
                BlendMode::ALL[i % 8],
                i % 5 == 0,
                image,
            )
        })
        .collect();
    let doc = document(1, 1, Mode::Rgba, None, layers);
    let (read, bytes) = round_trip("256 capas", &doc);
    assert_eq!(read.layers().len(), 256);
    assert_eq!(
        writes(&read),
        bytes,
        "la segunda escritura da los mismos bytes"
    );

    // 1 × 1 y 65 535 × 1.
    let one = document(
        1,
        1,
        Mode::Rgba,
        None,
        vec![layer_simple("a", Some(image_rgba(1, 1, vec![9, 8, 7, 6])))],
    );
    round_trip("1×1", &one);
    let width = 65_535u32;
    let strip = pixels_rgba(width as usize, 1, |x, _| {
        [x as u8, (x >> 8) as u8, 1, if x % 7 == 0 { 0 } else { 255 }]
    });
    let doc = document(
        width,
        1,
        Mode::Rgba,
        None,
        vec![layer_simple("tira", Some(image_rgba(width, 1, strip)))],
    );
    let (read, bytes) = round_trip("65535×1", &doc);
    assert_eq!(writes(&read), bytes);

    // Y el de la fila 11 a/b, escrito dos veces.
    let palette = palette_of(&PALETTE_4, Some(0));
    let doc = document(
        4,
        3,
        Mode::Indexed,
        Some(palette.clone()),
        vec![
            layer_simple(
                "a",
                Some(image_indexed(
                    4,
                    3,
                    &palette,
                    (0..12).map(|k| (k % 4) as u8).collect(),
                )),
            ),
            layer_simple("b", None),
        ],
    );
    let (read, bytes) = round_trip("indexado", &doc);
    assert_eq!(writes(&read), bytes);
}

// ---------------------------------------------------------------------------------------
// Propiedades (256 casos).
// ---------------------------------------------------------------------------------------

fn name_of_layer() -> impl Strategy<Value = String> {
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
fn cel_rgba(n: usize) -> impl Strategy<Value = Option<Vec<u8>>> {
    prop_oneof![
        1 => Just(None),
        1 => any::<[u8; 3]>().prop_map(move |c| Some([c[0], c[1], c[2], 0].repeat(n))),
        4 => vec(pixel_rgba(), n).prop_map(|px| Some(px.concat())),
    ]
}

fn document_rgba(width: u32, height: u32) -> BoxedStrategy<Document> {
    let n = (width * height) as usize;
    let layer_rgba = (
        name_of_layer(),
        any::<bool>(),
        any::<u8>(),
        0usize..8,
        any::<bool>(),
        cel_rgba(n),
    );
    vec(layer_rgba, 1..=6)
        .prop_map(move |layers| {
            let specs = layers
                .into_iter()
                .map(|(name_2, visible, opacity, mode, locked, cel)| {
                    let image = cel.map(|b| image_rgba(width, height, b));
                    layer_2(
                        &name_2,
                        visible,
                        opacity,
                        BlendMode::ALL[mode],
                        locked,
                        image,
                    )
                })
                .collect();
            document(width, height, Mode::Rgba, None, specs)
        })
        .boxed()
}

fn document_indexed(width: u32, height: u32) -> BoxedStrategy<Document> {
    let n = (width * height) as usize;
    (vec(any::<[u8; 4]>(), 2..=256), any::<u8>())
        .prop_flat_map(move |(colors, t)| {
            let length = colors.len();
            let transparent = (usize::from(t) % length) as u8;
            let cel = prop_oneof![1 => Just(None), 5 => vec(any::<u8>(), n).prop_map(Some)];
            let layer_idx = (name_of_layer(), any::<bool>(), any::<bool>(), cel);
            vec(layer_idx, 1..=6).prop_map(move |layers| {
                let palette = palette_of(&colors, Some(transparent));
                let specs = layers
                    .into_iter()
                    .map(|(name_2, visible, locked, cel)| {
                        // Un tercio de los píxeles es el índice transparente.
                        let image = cel.map(|b| {
                            let indices = b
                                .iter()
                                .map(|&v| {
                                    if v % 3 == 0 {
                                        transparent
                                    } else {
                                        (usize::from(v) % length) as u8
                                    }
                                })
                                .collect();
                            image_indexed(width, height, &palette, indices)
                        });
                        layer_2(&name_2, visible, 255, BlendMode::Normal, locked, image)
                    })
                    .collect();
                document(width, height, Mode::Indexed, Some(palette.clone()), specs)
            })
        })
        .boxed()
}

fn document_any() -> impl Strategy<Value = Document> {
    (1u32..=12, 1u32..=12, any::<bool>()).prop_flat_map(|(width, height, indexed)| {
        if indexed {
            document_indexed(width, height)
        } else {
            document_rgba(width, height)
        }
    })
}

fn fail<T, E: Display>(r: Result<T, E>) -> Result<T, TestCaseError> {
    r.map_err(|e| TestCaseError::fail(e.to_string()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// P1. Ida y vuelta: `read(write(d))` tiene el mismo contenido que `d`, y los bytes de una
    /// segunda escritura son los mismos.
    #[test]
    fn property_1_round_trip_and_stable_bytes(doc in document_any()) {
        let bytes = fail(write_aseprite(&doc))?;
        let read = fail(read_aseprite(&bytes, &AsepriteLimits::default()))?;
        prop_assert!(!read.report.lossy);
        prop_assert_eq!(read.report.frames_in_file, 1);
        fail(same_content(&doc, &read.document))?;
        let other = fail(write_aseprite(&read.document))?;
        prop_assert!(other == bytes, "la segunda escritura cambia los bytes");
    }
}

/// Bases para P3: dos ficheros hechos a mano y tres del corpus.
fn bases() -> &'static Vec<Vec<u8>> {
    static BASES: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
    BASES.get_or_init(|| {
        vec![
            base_rgba().bytes(),
            base_indexed().bytes(),
            golden("mixed-rgba.aseprite"),
            golden("indexed-cels.aseprite"),
            golden("cel-clipped.aseprite"),
        ]
    })
}

/// Lo que debe cumplir todo documento que el lector entrega (P3).
fn meets_invariants(doc: &Document, limits: &AsepriteLimits) -> Result<(), String> {
    let n = doc.layers().len();
    if !(1..=Document::MAX_LAYERS).contains(&n) {
        return Err(format!("{n} capas"));
    }
    if doc.materialized_bytes() > limits.max_cells_bytes {
        return Err("celdas por encima del tope".to_string());
    }
    for layer_2 in doc.layers() {
        if layer_2.name().len() > Document::MAX_LAYER_NAME_BYTES {
            return Err("nombre demasiado largo".to_string());
        }
        let Some(image) = doc.image(layer_2.cel()) else {
            continue;
        };
        if (image.width(), image.height(), image.mode()) != (doc.width(), doc.height(), doc.mode())
        {
            return Err("celda que no es del tamaño del lienzo".to_string());
        }
        if image.palette() != doc.palette() {
            return Err("celda con otra paleta".to_string());
        }
        if let Some(palette) = image.palette()
            && image
                .bytes()
                .iter()
                .any(|&i| usize::from(i) >= palette.len())
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
    fn property_3_robustness_against_arbitrary_truncated_and_altered_bytes(
        garbage in vec(any::<u8>(), 0..400),
        base in 0usize..5,
        changes in vec((any::<usize>(), any::<u8>()), 0..=4),
        cut in proptest::option::of(any::<usize>()),
    ) {
        let limits = AsepriteLimits { max_pixels: 1 << 16, max_cells_bytes: 1 << 20 };
        let mut candidates = vec![garbage];
        let mut valid = bases()[base].clone();
        for (pos, new_2) in changes {
            let i = pos % valid.len();
            valid[i] = new_2;
        }
        if let Some(cut) = cut {
            valid.truncate(cut % (valid.len() + 1));
        }
        candidates.push(valid);
        for bytes in candidates {
            if let Ok(read) = read_aseprite(&bytes, &limits) {
                fail(meets_invariants(&read.document, &limits))?;
            }
        }
    }

    /// P4. Colocación de la celda: el píxel `(px, py)` del documento es el de la celda en
    /// `(px − x, py − y)` si cae dentro y el relleno si no; vacía si lo de dentro es transparente.
    #[test]
    fn property_4_cel_placement(
        (width, height) in (1u16..=12, 1u16..=12),
        (x, y) in (-20i16..=20, -20i16..=20),
        (cw, ch) in (0u16..=16, 0u16..=16),
        indexed in any::<bool>(),
        raw in any::<bool>(),
        all_transparent in prop_oneof![1 => Just(true), 3 => Just(false)],
        seed in vec(any::<u8>(), 256 * 4),
        zeros in vec(any::<bool>(), 256),
    ) {
        let n = usize::from(cw) * usize::from(ch);
        let transparent_k = |k: usize| all_transparent || zeros[k];
        let data: Vec<u8> = if indexed {
            // Índice transparente 3; los demás píxeles usan los índices 0 a 2.
            (0..n).map(|k| if transparent_k(k) { 3 } else { seed[k] % 3 }).collect()
        } else {
            (0..n)
                .flat_map(|k| {
                    let alpha = if transparent_k(k) { 0 } else { seed[4 * k + 3].max(1) };
                    [seed[4 * k], seed[4 * k + 1], seed[4 * k + 2], alpha]
                })
                .collect()
        };
        let cel = Cel {
            kind: if raw { TEST_CEL_RAW } else { TEST_CEL_COMPRESSED },
            ..Cel::new(0, x, y, cw, ch, data.clone())
        };
        let layer_chunk = TestLayer::new("c").chunk();
        let file = if indexed {
            File::indexed(width, height, 3, 4, vec![profile_srgb(), palette_complete(&PALETTE_4), layer_chunk, cel.chunk()])
        } else {
            File::rgba(width, height, vec![profile_srgb(), layer_chunk, cel.chunk()])
        };
        let read = fail(lee(&file.bytes()))?;
        let doc = &read.document;
        prop_assert_eq!(doc.layers().len(), 1);
        let fill: &[u8] = if indexed { &[3] } else { &[0, 0, 0, 0] };
        let transparent: fn(&[u8]) -> bool = if indexed { |p| p[0] == 3 } else { rgba_transparent };
        let expected = placement_expected(
            usize::from(width),
            usize::from(height),
            (i32::from(x), i32::from(y)),
            (usize::from(cw), usize::from(ch)),
            &data,
            fill,
            transparent,
        );
        let image = doc.image(doc.layers()[0].cel());
        prop_assert_eq!(image.map(Image::bytes), expected.as_deref());
    }
}
