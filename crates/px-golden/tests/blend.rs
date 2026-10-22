//! Composición contra el corpus `golden/aseprite/` (specs/hito-3-mezcla.md, unidad 3.2, filas 9,
//! 10 y 11 de «Casos»; specs/corpus-hito-3.md para los casos).
//!
//! Cada caso se reconstruye aquí como un `Document` de `px-core` a partir de las tablas `A` y
//! `B` y de las reglas de `tools/corpus-h3/generate.lua` (rectángulos, rejillas, tiras y el disco
//! de `background-with-opacity`, por desigualdad entera), y `px_raster::compose` se compara con el PNG
//! aplanado `golden/aseprite/<case>.png` leído con `px_formats::read_png`. El formato
//! `.aseprite` no se lee (eso es la 3.8): por eso solo entran los casos que se reconstruyen sin
//! formas de celda; los demás esperan a la 3.9.
//!
//! - Fila 9: las ocho matrices `blend-*` (2 capas, 32 × 32), **tolerancia 0**.
//! - Fila 10: `blend-eight-opacity`, `blend-eight-alpha`, `layers-opacity`, `layers-alpha-source` y
//!   `background-with-opacity`, **tolerancia 0**.
//! - Fila 11: `layers-alpha-background`, `blend-eight-background-translucent` y las dos rejillas de alfas.
//!   Fuera del dominio de la fórmula empírica del paso 4b (`0 < αb < 255` del fondo ya compuesto,
//!   `αs > 0` y modo distinto de Normal), tolerancia 0; dentro, techo `TOLERANCE_EMPIRICAL` y
//!   suelo 1 en cada caso agregado (las dos rejillas y cada una de las siete tiras).
//!
//! Este fichero es la fase de pruebas en rojo: `BlendMode`, `SetLayerBlend` y `px_raster::compose`
//! no existen todavía, así que no compila hasta la lógica A y B de la spec.

mod common;

use std::fs;
use std::path::Path;

use px_core::{BlendMode, Command, Document, Image, Mode, Pixel, PixelEdit, Rgba};
use px_formats::read_png;
use px_raster::{compose, composite_pixel, flatten_command, merge_down_command};

/// Techo del error, en niveles de canal, de la fórmula EMPÍRICA del paso 4b contra los PNG del
/// corpus, dentro de su dominio (fondo ya compuesto translúcido, `αs > 0` y modo distinto de
/// Normal). Es un techo, y además hay un suelo de 1 por caso agregado: si el error máximo baja a
/// 0, la exención sobra y ese caso debe pasar a tolerancia 0. Si algún día aparece la secuencia
/// entera exacta, esta constante desaparece.
const TOLERANCE_EMPIRICAL: i32 = 2;

// ---------------------------------------------------------------------------------------
// Los colores y las tablas de `generate.lua`.
// ---------------------------------------------------------------------------------------

const RED: [u8; 3] = [196, 46, 52];
const GREEN: [u8; 3] = [58, 168, 76];
const CYAN: [u8; 3] = [66, 196, 206];
const MAGENTA: [u8; 3] = [186, 64, 170];
const ORANGE: [u8; 3] = [236, 132, 44];
const WHITE: [u8; 3] = [255, 255, 255];
const BLACK: [u8; 3] = [0, 0, 0];
const GRAY: [u8; 3] = [128, 128, 128];
const BLUE: [u8; 3] = [44, 92, 200];

/// Colores de fondo (`A`) y de origen (`B`) de las matrices de mezcla.
const A: [[u8; 3]; 8] = [
    [255, 255, 255],
    [0, 0, 0],
    [196, 46, 52],
    [58, 168, 76],
    [44, 92, 200],
    [232, 198, 52],
    [128, 128, 128],
    [37, 201, 143],
];
const B: [[u8; 3]; 8] = [
    [255, 255, 255],
    [0, 0, 0],
    [90, 60, 220],
    [220, 120, 30],
    [20, 200, 200],
    [180, 180, 60],
    [100, 100, 100],
    [211, 17, 99],
];

/// Los ocho modos, en el orden de `generate.lua` (el de las tiras).
const MODES: [(&str, BlendMode); 8] = [
    ("normal", BlendMode::Normal),
    ("multiplicar", BlendMode::Multiply),
    ("trama", BlendMode::Screen),
    ("superponer", BlendMode::Overlay),
    ("oscurecer", BlendMode::Darken),
    ("aclarar", BlendMode::Lighten),
    ("sumar", BlendMode::Add),
    ("diferencia", BlendMode::Difference),
];

fn color(rgb: [u8; 3], alpha_2: u8) -> Rgba {
    Rgba::new(rgb[0], rgb[1], rgb[2], alpha_2)
}

// ---------------------------------------------------------------------------------------
// Lienzos de píxeles (las formas de `generate.lua`) y casos.
// ---------------------------------------------------------------------------------------

/// La imagen de una celda: `w × h` píxeles RGBA, fila a fila, transparentes al principio.
struct Canvas {
    w: i32,
    h: i32,
    pixels_2: Vec<Rgba>,
}

impl Canvas {
    fn empty(w: i32, h: i32) -> Canvas {
        Canvas::fill(w, h, Rgba::TRANSPARENT)
    }

    fn fill(w: i32, h: i32, c: Rgba) -> Canvas {
        Canvas {
            w,
            h,
            pixels_2: vec![c; (w * h) as usize],
        }
    }

    /// Único punto donde se escribe un píxel; lo de fuera se ignora.
    fn put(&mut self, x: i32, y: i32, c: Rgba) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.pixels_2[(y * self.w + x) as usize] = c;
        }
    }

    /// Rectángulo con los dos extremos incluidos.
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgba) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.put(x, y, c);
            }
        }
    }

    /// Disco por desigualdad entera: `(x − cx)² + (y − cy)² ≤ r²`.
    fn disk(&mut self, cx: i32, cy: i32, r: i32, c: Rgba) {
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                if (x - cx) * (x - cx) + (y - cy) * (y - cy) <= r * r {
                    self.put(x, y, c);
                }
            }
        }
    }
}

/// Una capa del caso con su celda del tamaño del lienzo.
struct TestLayer {
    visible: bool,
    opacity_2: u8,
    mode: BlendMode,
    cel_2: Canvas,
}

impl TestLayer {
    fn normal(cel_2: Canvas) -> TestLayer {
        TestLayer {
            visible: true,
            opacity_2: 255,
            mode: BlendMode::Normal,
            cel_2,
        }
    }

    fn with(mode: BlendMode, opacity_2: u8, cel_2: Canvas) -> TestLayer {
        TestLayer {
            visible: true,
            opacity_2,
            mode,
            cel_2,
        }
    }
}

/// La palabra inglesa del nombre de caso `blend-<word>` de un modo de mezcla; el nombre español es el
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

/// Un caso del corpus: su nombre (el del PNG), el tamaño y las capas de abajo arriba.
struct Case {
    name_2: String,
    w: i32,
    h: i32,
    layers: Vec<TestLayer>,
}

/// Matriz 8 × 8: columnas de 4 px con los colores `A` (fondo), filas de 4 px con los `B` (origen).
fn matrix(name_mode: &str, mode: BlendMode) -> Case {
    let mut a = Canvas::empty(32, 32);
    let mut b = Canvas::empty(32, 32);
    for y in 0..32 {
        for x in 0..32 {
            a.put(x, y, color(A[(x / 4) as usize], 255));
            b.put(x, y, color(B[(y / 4) as usize], 255));
        }
    }
    Case {
        name_2: format!("blend-{}", case_word(name_mode)),
        w: 32,
        h: 32,
        layers: vec![TestLayer::normal(a), TestLayer::with(mode, 255, b)],
    }
}

/// Un sprite de 64 × 16: base con los colores `A` por columna (`x mod 8`) y ocho capas, una por
/// modo, cada una en su tira de 8 columnas; el origen cambia de color por fila.
fn eight_modes(
    name_2: &str,
    opacity_2: u8,
    alpha_height: u8,
    alpha_under: u8,
    alpha_base: u8,
) -> Case {
    let mut base = Canvas::empty(64, 16);
    for y in 0..16 {
        for x in 0..64 {
            base.put(x, y, color(A[(x % 8) as usize], alpha_base));
        }
    }
    let mut layers = vec![TestLayer::normal(base)];
    for (k, (_, mode)) in MODES.iter().enumerate() {
        let mut cel_2 = Canvas::empty(64, 16);
        for y in 0..16 {
            let rgb = B[((y + if y >= 8 { 3 } else { 0 }) % 8) as usize];
            let alpha_2 = if y < 8 { alpha_height } else { alpha_under };
            for x in 8 * k as i32..8 * (k as i32 + 1) {
                cel_2.put(x, y, color(rgb, alpha_2));
            }
        }
        layers.push(TestLayer::with(*mode, opacity_2, cel_2));
    }
    Case {
        name_2: name_2.to_string(),
        w: 64,
        h: 16,
        layers,
    }
}

fn layers_opacity() -> Case {
    let mut base = Canvas::empty(32, 16);
    for y in 0..16 {
        for x in 0..32 {
            let rgb = if (x / 4 + y / 4) % 2 == 0 {
                WHITE
            } else {
                BLACK
            };
            base.put(x, y, color(rgb, 255));
        }
    }
    let mut layers = vec![TestLayer::normal(base)];
    for (k, op) in [0u8, 64, 128, 200, 255].into_iter().enumerate() {
        let mut t = Canvas::empty(32, 16);
        let x0 = 1 + 6 * k as i32;
        t.rect(x0, 2, x0 + 4, 13, color(RED, 255));
        layers.push(TestLayer::with(BlendMode::Normal, op, t));
    }
    Case {
        name_2: "layers-opacity".to_string(),
        w: 32,
        h: 16,
        layers,
    }
}

fn layers_alpha_source() -> Case {
    let mut background = Canvas::empty(32, 16);
    for (k, rgb) in [WHITE, GRAY, BLUE, BLACK].into_iter().enumerate() {
        background.rect(8 * k as i32, 0, 8 * (k as i32 + 1) - 1, 15, color(rgb, 255));
    }
    let mut source_2 = Canvas::empty(32, 16);
    for (k, a) in [0u8, 36, 73, 109, 146, 182, 219, 255]
        .into_iter()
        .enumerate()
    {
        let x0 = 4 * k as i32;
        source_2.rect(x0, 0, x0 + 3, 7, color(ORANGE, a));
        source_2.rect(x0, 8, x0 + 3, 15, color(CYAN, a));
    }
    Case {
        name_2: "layers-alpha-source".to_string(),
        w: 32,
        h: 16,
        layers: vec![TestLayer::normal(background), TestLayer::normal(source_2)],
    }
}

fn layers_alpha_background() -> Case {
    let alphas = [255u8, 200, 128, 64];
    let mut base = Canvas::empty(32, 16);
    let mut top = Canvas::empty(32, 16);
    for (k, &a) in alphas.iter().enumerate() {
        let k = k as i32;
        base.rect(8 * k, 0, 8 * (k + 1) - 1, 15, color(GREEN, a));
        top.rect(0, 4 * k, 31, 4 * (k + 1) - 1, color(MAGENTA, a));
    }
    Case {
        name_2: "layers-alpha-background".to_string(),
        w: 32,
        h: 16,
        layers: vec![TestLayer::normal(base), TestLayer::normal(top)],
    }
}

/// La capa de abajo con opacidad 128 y modo Multiplicar sobre el lienzo transparente y una capa
/// Normal encima (con un disco y un cuadrado con alfa 128). La fila de alfa 0 lleva RGB.
fn background_with_opacity() -> Case {
    let alphas = [255u8, 223, 191, 159, 127, 95, 63, 0];
    let mut base = Canvas::empty(32, 32);
    for y in 0..32 {
        for x in 0..32 {
            base.put(x, y, color(A[(x / 4) as usize], alphas[(y / 4) as usize]));
        }
    }
    let mut top = Canvas::empty(32, 32);
    top.disk(16, 16, 9, color(ORANGE, 255));
    top.rect(2, 2, 13, 13, color(CYAN, 128));
    Case {
        name_2: "background-with-opacity".to_string(),
        w: 32,
        h: 32,
        layers: vec![
            TestLayer::with(BlendMode::Multiply, 128, base),
            TestLayer::normal(top),
        ],
    }
}

/// Alfas de las rejillas (el 127 al final, fuera de orden, a propósito).
const ALPHAS_GRID: [u8; 8] = [0, 40, 80, 120, 160, 200, 255, 127];
/// Los dos pares de colores de las rejillas: `(x0, source, background)`; el segundo cambia fuente y
/// fondo de sitio.
const PAIRS_GRID: [(i32, [u8; 3], [u8; 3]); 2] = [
    (4, [200, 60, 20], [30, 120, 220]),
    (36, [30, 120, 220], [200, 60, 20]),
];

/// Dos rejillas de 8 × 8 celdas de 3 × 3 px: columnas = alfa del fondo, filas = alfa de la
/// fuente. La base es gris 90 opaco fuera de las rejillas y translúcida dentro; la capa de
/// fuente solo existe dentro.
fn grid(name_2: &str, mode: BlendMode, opacity_2: u8) -> Case {
    let mut base = Canvas::fill(64, 32, color([90, 90, 90], 255));
    let mut source = Canvas::empty(64, 32);
    for (x_initial, c_source, c_background) in PAIRS_GRID {
        for (i, &alpha_background) in ALPHAS_GRID.iter().enumerate() {
            for (j, &alpha_source) in ALPHAS_GRID.iter().enumerate() {
                let x0 = x_initial + 3 * i as i32;
                let y0 = 4 + 3 * j as i32;
                base.rect(
                    x0,
                    y0,
                    x0 + 2,
                    y0 + 2,
                    color(c_background, alpha_background),
                );
                source.rect(x0, y0, x0 + 2, y0 + 2, color(c_source, alpha_source));
            }
        }
    }
    Case {
        name_2: name_2.to_string(),
        w: 64,
        h: 32,
        layers: vec![
            TestLayer::normal(base),
            TestLayer::with(mode, opacity_2, source),
        ],
    }
}

// ---------------------------------------------------------------------------------------
// Del caso al documento, el PNG y la comparación.
// ---------------------------------------------------------------------------------------

/// El documento del caso, solo con los comandos del núcleo.
fn document(case: &Case) -> Document {
    let name_2 = &case.name_2;
    let image_2 = Image::new(case.w as u32, case.h as u32, Mode::Rgba).expect("imagen RGBA");
    let mut doc = Document::new(image_2);
    for i in 1..case.layers.len() {
        let cmd = Command::AddLayer {
            name: format!("capa {i}"),
            at: i,
        };
        assert_eq!(doc.apply(cmd), Ok(true), "{name_2}: add layer {i}");
    }
    for (i, layer_2) in case.layers.iter().enumerate() {
        let (id, cel) = (doc.layers()[i].id(), doc.layers()[i].cel());
        // Solo los píxeles que no son `(0, 0, 0, 0)`: lo demás ya es así en una celda nueva.
        let pixels: Vec<PixelEdit> = layer_2
            .cel_2
            .pixels_2
            .iter()
            .enumerate()
            .filter(|(_, c)| **c != Rgba::TRANSPARENT)
            .map(|(k, &c)| PixelEdit {
                x: k as u32 % case.w as u32,
                y: k as u32 / case.w as u32,
                pixel: Pixel::Rgba(c),
            })
            .collect();
        if !pixels.is_empty() {
            let cmd = Command::SetPixels { image: cel, pixels };
            assert!(doc.apply(cmd).is_ok(), "{name_2}: write layer {i}");
        }
        if layer_2.opacity_2 != 255 {
            let cmd = Command::SetLayerOpacity {
                layer: id,
                opacity: layer_2.opacity_2,
            };
            assert!(doc.apply(cmd).is_ok(), "{name_2}: opacity of layer {i}");
        }
        if layer_2.mode != BlendMode::Normal {
            let cmd = Command::SetLayerBlend {
                layer: id,
                blend: layer_2.mode,
            };
            assert!(doc.apply(cmd).is_ok(), "{name_2}: mode of layer {i}");
        }
        if !layer_2.visible {
            let cmd = Command::SetLayerVisible {
                layer: id,
                visible: false,
            };
            assert!(doc.apply(cmd).is_ok(), "{name_2}: hide layer {i}");
        }
    }
    doc
}

/// El PNG aplanado del caso (`golden/aseprite/<case>.png`), RGBA de 8 bits.
fn read_golden_png(name_2: &str) -> Image {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../golden/aseprite")
        .join(format!("{name_2}.png"));
    let bytes =
        fs::read(&path).unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    let read = read_png(&bytes).unwrap_or_else(|e| panic!("{name_2}.png: read_png dio error: {e}"));
    assert_eq!(read.image.mode(), Mode::Rgba, "{name_2}.png debe ser RGBA");
    read.image
}

fn pixel_of(img: &Image, x: u32, y: u32) -> Rgba {
    match img.get_pixel(x, y).expect("píxel dentro") {
        Pixel::Rgba(c) => c,
        other_2 => panic!("se esperaba RGBA, hay {other_2:?}"),
    }
}

/// `compose` del caso y el PNG, con el tamaño ya comprobado.
fn composite_and_png(case: &Case) -> (Image, Image) {
    let name_2 = &case.name_2;
    let composite = compose(&document(case)).unwrap_or_else(|e| panic!("{name_2}: compose: {e}"));
    let png = read_golden_png(name_2);
    assert_eq!(
        (composite.width(), composite.height(), composite.mode()),
        (png.width(), png.height(), Mode::Rgba),
        "{name_2}: size or mode of the composition differs from the PNG"
    );
    (composite, png)
}

/// Tolerancia 0: todos los bytes de la composición (RGB bajo alfa 0 incluido) son los del PNG.
/// Falla con el nombre del caso y el primer píxel distinto.
fn requires_equal_to_the_png(case: &Case) {
    let (composite, png) = composite_and_png(case);
    requires_equal_image(case, &composite, &png);
}

/// Como [`requires_equal_to_the_png`] con la imagen ya calculada (la composición, o lo que dejan las
/// operaciones de la unidad 3.5).
fn requires_equal_image(case: &Case, composite: &Image, png: &Image) {
    let (mut different, mut first) = (0usize, None);
    for y in 0..png.height() {
        for x in 0..png.width() {
            let (our, expected) = (pixel_of(composite, x, y), pixel_of(png, x, y));
            if our != expected {
                different += 1;
                first.get_or_insert((x, y, our, expected));
            }
        }
    }
    if let Some((x, y, our, expected)) = first {
        panic!(
            "{}: {different} pixels differ from the PNG; the first, ({x}, {y}): \
             compose gives {our:?} and the PNG {expected:?}",
            case.name_2
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 9: las ocho matrices de 64 combinaciones.
// ---------------------------------------------------------------------------------------

#[test]
fn row_09_eight_blend_matrices_with_tolerance_0() {
    for (name_2, mode) in MODES {
        requires_equal_to_the_png(&matrix(name_2, mode));
    }
}

// ---------------------------------------------------------------------------------------
// Fila 10: opacidad, alfa de origen y capa de abajo con opacidad.
// ---------------------------------------------------------------------------------------

#[test]
fn row_10_opacity_source_alpha_and_background_with_opacity_with_tolerance_0() {
    let cases = [
        eight_modes("blend-eight-opacity", 128, 255, 255, 255),
        eight_modes("blend-eight-alpha", 255, 200, 77, 255),
        layers_opacity(),
        layers_alpha_source(),
        // La capa de arriba lleva `(0, 0, 0, 0)` sobre la de abajo con alfa 0 y RGB ≠ 0: el paso 0
        // de la spec (un origen `(0, 0, 0, 0)` no aporta nada) conserva ese RGB, como el PNG.
        background_with_opacity(),
    ];
    for case in &cases {
        requires_equal_to_the_png(case);
    }
}

// ---------------------------------------------------------------------------------------
// Fila 11: fondo translúcido (dominio de la fórmula empírica).
// ---------------------------------------------------------------------------------------

/// `x / 255` al entero más cercano (sin empates posibles), a partir de la definición.
fn rounds_255(x: u32) -> u32 {
    (2 * x + 255) / 510
}

/// `true` si el píxel `(x, y)` está en el dominio de la fórmula empírica: en alguna capa visible,
/// el fondo ya compuesto debajo tiene `0 < αb < 255`, el origen tiene `αs = div255(a ·
/// opacidad) > 0` y el mode no es Normal. El background se pliega with `composite_pixel` layer_2 a layer_2.
fn in_domain(case: &Case, x: u32, y: u32) -> bool {
    let k = (y as i32 * case.w + x as i32) as usize;
    let mut background = Rgba::TRANSPARENT;
    let mut inside = false;
    for layer_2 in case.layers.iter().filter(|c| c.visible) {
        let source_2 = layer_2.cel_2.pixels_2[k];
        let alpha_source_2 = rounds_255(u32::from(source_2.a) * u32::from(layer_2.opacity_2));
        if background.a > 0
            && background.a < 255
            && alpha_source_2 > 0
            && layer_2.mode != BlendMode::Normal
        {
            inside = true;
        }
        background = composite_pixel(background, source_2, layer_2.opacity_2, layer_2.mode);
    }
    inside
}

/// Lo que da evaluar un caso contra su PNG, agrupado (un grupo por tira, o uno solo).
struct Report {
    /// Error máximo de canal en el dominio, por grupo.
    max_error: Vec<i32>,
    /// Píxeles del dominio, por grupo.
    in_domain: Vec<usize>,
    /// El primer incumplimiento (nombre, píxel y valores), si lo hay.
    failure: Option<String>,
}

/// Compara la imagen de un caso (`compose`, o lo que deja Aplanar) con su PNG: fuera del dominio, tolerancia 0 (el RGB bajo alfa 0 no
/// se exige cuando el alfa de la composición y el del PNG son 0); dentro, el alfa es exacto y el
/// error de cada canal RGB se acumula en el máximo de su grupo (`group(x, y)`) y debe ser como
/// mucho `TOLERANCE_EMPIRICAL`.
fn evaluates_image(
    case: &Case,
    composite: &Image,
    png: &Image,
    group: impl Fn(u32, u32) -> usize,
    groups: usize,
) -> Report {
    let mut report = Report {
        max_error: vec![0; groups],
        in_domain: vec![0; groups],
        failure: None,
    };
    for y in 0..png.height() {
        for x in 0..png.width() {
            let (our, expected) = (pixel_of(composite, x, y), pixel_of(png, x, y));
            let inside = in_domain(case, x, y);
            let mut error_pixel = 0;
            let mut reason = None;
            if our.a != expected.a {
                reason = Some("el alfa no es exacto");
            } else if inside {
                let differences = [
                    i32::from(our.r) - i32::from(expected.r),
                    i32::from(our.g) - i32::from(expected.g),
                    i32::from(our.b) - i32::from(expected.b),
                ];
                error_pixel = differences.iter().map(|d| d.abs()).max().unwrap_or(0);
                if error_pixel > TOLERANCE_EMPIRICAL {
                    reason = Some("el error pasa del techo de la fórmula empírica");
                }
            } else if !(our.a == 0 && expected.a == 0) && our != expected {
                reason = Some("fuera del dominio la tolerancia es 0");
            }
            if inside {
                let g = group(x, y);
                report.in_domain[g] += 1;
                report.max_error[g] = report.max_error[g].max(error_pixel);
            }
            if let Some(reason) = reason
                && report.failure.is_none()
            {
                report.failure = Some(format!(
                    "{}: píxel ({x}, {y}) {}: {reason}; compose da {our:?} y el PNG \
                     {expected:?}",
                    case.name_2,
                    if inside {
                        "del dominio"
                    } else {
                        "fuera del dominio"
                    },
                ));
            }
        }
    }
    report
}

/// Que no haya ningún incumplimiento.
fn requires_without_failure(report: &Report) {
    if let Some(failure) = &report.failure {
        panic!("{failure}");
    }
}

/// El error máximo del grupo `that` debe estar en `[1, TOLERANCE_EMPIRICAL]`: el techo vigila la
/// fórmula y el suelo, que la exención no sobre.
fn requires_ceiling_and_floor(case: &str, that: &str, maximum: i32) {
    assert!(
        maximum != 0,
        "{case} ({that}): the maximum error inside the domain is 0: the exemption is unnecessary: \
         use tolerance 0 in this case"
    );
    assert!(
        maximum <= TOLERANCE_EMPIRICAL,
        "{case} ({that}): the maximum error inside the domain is {maximum}, more than the ceiling \
         {TOLERANCE_EMPIRICAL}"
    );
}

#[test]
fn row_11_translucent_background_domain_of_the_empirical_formula() {
    row_11_with(|case| composite_and_png(case).0);
}

/// El cuerpo de la fila 11 con la imagen que `produce` da para cada caso (la composición, en 3.2;
/// la celda que deja `Aplanar`, en 3.5).
fn row_11_with(produce: impl Fn(&Case) -> Image) {
    let evaluates = |case: &Case, group: &dyn Fn(u32, u32) -> usize, groups: usize| {
        let image_2 = produce(case);
        evaluates_image(
            case,
            &image_2,
            &read_golden_png(&case.name_2),
            group,
            groups,
        )
    };
    // `layers-alpha-background`: dos translúcidos a la vez con Normal, que no es del dominio: todo con
    // tolerancia 0.
    let report = evaluates(&layers_alpha_background(), &|_, _| 0, 1);
    requires_without_failure(&report);
    assert_eq!(
        report.in_domain,
        [0],
        "layers-alpha-background: Normal is not in the domain"
    );

    // `blend-eight-background-translucent`: fondo con alfa 150 y origen con 255 (arriba) y 100
    // (abajo). La tira Normal (la primera) está fuera del dominio: tolerancia 0. Cada una de las
    // otras siete es del dominio entera (8 columnas × 16 filas) y su error máximo está en [1, 2].
    let strips = eight_modes("blend-eight-background-translucent", 255, 255, 100, 150);
    let report = evaluates(&strips, &|x, _| (x / 8) as usize, 8);
    requires_without_failure(&report);
    assert_eq!(
        report.in_domain[0], 0,
        "the Normal strip is not in the domain"
    );
    assert_eq!(report.max_error[0], 0);
    for (k, (name_mode, _)) in MODES.iter().enumerate().skip(1) {
        assert_eq!(
            report.in_domain[k], 128,
            "the {name_mode} strip is entirely in the domain"
        );
        requires_ceiling_and_floor(
            "blend-eight-background-translucent",
            &format!("tira {name_mode}"),
            report.max_error[k],
        );
    }

    // Las dos rejillas de alfas: de las 128 celdas de 3 × 3 de cada una, 84 son del dominio
    // (fondo translúcido, fuente visible, modo distinto de Normal) y el resto, no; el error
    // máximo de las 84 celdas está en [1, 2].
    for (name_2, mode, opacity_2) in [
        ("blend-alpha-grid-multiply", BlendMode::Multiply, 255),
        ("blend-alpha-grid-overlay", BlendMode::Overlay, 128),
    ] {
        let report = evaluates(&grid(name_2, mode, opacity_2), &|_, _| 0, 1);
        requires_without_failure(&report);
        assert_eq!(report.in_domain, [84 * 9], "{name_2}: 84 celdas de 3 × 3");
        requires_ceiling_and_floor(name_2, "rejilla", report.max_error[0]);
    }
}

// ---------------------------------------------------------------------------------------
// Fila 8 de la unidad 3.5 (specs/hito-3-combinar.md): aplanar y combinar abajo contra los PNG.
// ---------------------------------------------------------------------------------------

/// Aplana el documento del caso con `flatten_command`: comprueba que queda una sola capa y que
/// su celda es la composición de antes, byte a byte, y devuelve esa celda.
fn flattens(case: &Case) -> Image {
    let name_2 = &case.name_2;
    let mut doc = document(case);
    let before = compose(&doc).unwrap_or_else(|e| panic!("{name_2}: compose: {e:?}"));
    let order = flatten_command(&doc).unwrap_or_else(|e| panic!("{name_2}: aplanar: {e:?}"));
    assert_eq!(doc.apply(order), Ok(true), "{name_2}: apply the flatten");
    assert_eq!(doc.layers().len(), 1, "{name_2}: a single layer remains");
    let cel_2 = doc
        .image(doc.layers()[0].cel())
        .unwrap_or_else(|| panic!("{name_2}: the flattened cel has an image"))
        .clone();
    assert_eq!(
        cel_2, before,
        "{name_2}: the cel is the earlier composition"
    );
    assert_eq!(
        compose(&doc).expect("compose tras aplanar"),
        before,
        "{name_2}: composing after flattening gives the same as before"
    );
    cel_2
}

/// Los casos de las filas 9 y 10 de 3.2 (tolerancia 0).
fn cases_exact() -> Vec<Case> {
    let mut cases: Vec<Case> = MODES
        .iter()
        .map(|(name_2, mode)| matrix(name_2, *mode))
        .collect();
    cases.extend([
        eight_modes("blend-eight-opacity", 128, 255, 255, 255),
        eight_modes("blend-eight-alpha", 255, 200, 77, 255),
        layers_opacity(),
        layers_alpha_source(),
        background_with_opacity(),
    ]);
    cases
}

#[test]
fn row_08_flatten_the_17_cases_of_3_2_gives_the_composition_and_meets_its_tolerances() {
    // Filas 9 y 10 de 3.2: tolerancia 0 contra el PNG.
    for case in &cases_exact() {
        let cel_2 = flattens(case);
        requires_equal_image(case, &cel_2, &read_golden_png(&case.name_2));
    }
    // Fila 11 de 3.2: tolerancia 0 fuera del dominio empírico y techo 2 dentro (suelo 1).
    row_11_with(flattens);
}

#[test]
fn row_08_merge_down_in_the_eight_matrices_at_tolerance_0() {
    for (name_2, mode) in MODES {
        let case = matrix(name_2, mode);
        let mut doc = document(&case);
        let before = compose(&doc).expect("compose");
        let top = doc.layers()[1].id();
        let order = merge_down_command(&doc, top)
            .unwrap_or_else(|e| panic!("{name_2}: combinar abajo: {e:?}"));
        assert_eq!(doc.apply(order), Ok(true), "{name_2}");
        assert_eq!(doc.layers().len(), 1, "{name_2}");
        let cel_2 = doc
            .image(doc.layers()[0].cel())
            .expect("celda combinada")
            .clone();
        assert_eq!(
            cel_2, before,
            "{name_2}: the two-layer matrix merges without loss"
        );
        requires_equal_image(&case, &cel_2, &read_golden_png(&case.name_2));
    }
}
