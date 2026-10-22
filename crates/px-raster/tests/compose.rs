//! Tests de la composición de capas de `px-raster` (specs/hito-3-mezcla.md, unidad 3.2, cambios
//! `R-01` a `R-04`): `Rect`, `compose`, `compose_rect`, `composite_pixel` y `ComposeError`.
//!
//! Una prueba por cada una de las filas 1 a 8 de «Casos» (`fila_NN_...`; la fila 4 se reparte en
//! sus valores a mano, el barrido que corre siempre y el barrido completo, que está `#[ignore]`)
//! y una por cada una de las 4 «Propiedades» (`propiedad_N_...`, con los casos por defecto de
//! `proptest`: 256, o los de `PROPTEST_CASES`). Las filas 9 a 11 (corpus) viven en
//! `crates/px-golden/tests/mezcla.rs` y la fila 12 en `crates/px-core/tests/blend.rs`.
//!
//! Los valores de las filas 1 a 4 son los de la spec, calculados a mano: no se recalculan con el
//! código. El oráculo de las propiedades y del barrido es un modelo racional exacto con enteros
//! (`i128`, sin `f64`) escrito aparte (módulo `model`): no repite las fórmulas en enteros de la
//! spec (`div255`, truncamientos), sino la forma real de la teoría y, en el dominio del paso 4b,
//! la forma observada `m = 1 − (1 − αb)(1 − αs·αb)`. Todo documento mide como mucho 6 × 5.
//!
//! Este fichero es la fase de pruebas en rojo: la API de composición y `BlendMode` no existen
//! todavía, así que no compila hasta la lógica A y B de la spec.
//!

use proptest::prelude::*;
use px_core::{BlendMode, Command, Document, Image, Mode, Palette, Pixel, PixelEdit, Rgba};
use px_raster::{ComposeError, Rect, compose, compose_rect, composite_pixel};

// ---------------------------------------------------------------------------------------
// El modelo real (oráculo): racionales con enteros, sin `f64`.
// ---------------------------------------------------------------------------------------

/// Techo del error, en niveles de canal, de la fórmula EMPÍRICA del paso 4b (fondo translúcido
/// `0 < αb < 255`, `αs > 0` y modo distinto de Normal) frente a la forma real observada con el
/// `αr` entero. Es un techo y no una igualdad: la fórmula es empírica (salió de mirar los PNG
/// del corpus como datos) y puede no ser lo que hace el programa fuera de los puntos medidos.
/// Si algún día aparece la secuencia exacta con 0 diferencias, esta constante desaparece.
const TOLERANCE_EMPIRICAL: i128 = 2;

mod model {
    use super::{BlendMode, TOLERANCE_EMPIRICAL};

    /// `255 · B(Cb, Cs)` de la teoría (W3C), con la mezcla sin redondear: un entero sobre 255.
    /// Superponer usa el umbral `Cb < 128` de la spec (lo fijó el corpus).
    pub fn blend_per_255(mode: BlendMode, cb: i128, cs: i128) -> i128 {
        match mode {
            BlendMode::Normal => 255 * cs,
            BlendMode::Multiply => cb * cs,
            BlendMode::Screen => 255 * (cb + cs) - cb * cs,
            BlendMode::Overlay => {
                if cb < 128 {
                    2 * cb * cs
                } else {
                    let t = 2 * cb - 255;
                    255 * (cs + t) - cs * t
                }
            }
            BlendMode::Darken => 255 * cb.min(cs),
            BlendMode::Lighten => 255 * cb.max(cs),
            BlendMode::Add => 255 * (cb + cs).min(255),
            BlendMode::Difference => 255 * (cb - cs).abs(),
            other => panic!("the model does not know the mode {other:?}"),
        }
    }

    /// `Cr` como fracción `(num, den)` con `den > 0`:
    /// `Cr = Cb + (Cm − Cb) · αs / αr` con `Cm = (1 − m)·Cs + m·B` y
    /// `m = 1 − (1 − αb)(1 − αs·αb)` (con la fuente opaca, `1 − (1 − αb)²`; para Normal, o con
    /// `αb = 255`, `Cm` es la de la teoría: `Cs` o `B`). `ab` y `as_` son alfas de 0..=255 y
    /// `αr = ar_num / ar_den` (el entero del paso 4: `(αr, 1)`; el real: `(255·αs + 255·αb −
    /// αs·αb, 255)`).
    pub fn result(
        mode: BlendMode,
        cb: u8,
        cs: u8,
        ab: i128,
        as_: i128,
        (ar_num, ar_den): (i128, i128),
    ) -> (i128, i128) {
        let (cb, cs) = (i128::from(cb), i128::from(cs));
        let b255 = blend_per_255(mode, cb, cs);
        // m = m_num / 255³; Cm = Cs + m · (B − Cs) con B = b255 / 255.
        let m_num = 255i128.pow(3) - (255 - ab) * (65025 - as_ * ab);
        let d = 255i128.pow(4);
        let cm_num = cs * d + m_num * (b255 - 255 * cs);
        (
            cb * d * ar_num + (cm_num - cb * d) * as_ * ar_den,
            d * ar_num,
        )
    }

    /// `|integer − num/den| ≤ tol_num/tol_den`.
    pub fn meets(integer: u8, (num, den): (i128, i128), (tol_num, tol_den): (i128, i128)) -> bool {
        (i128::from(integer) * den - num).abs() * tol_den <= tol_num * den
    }

    /// `|αr integer − αr real| ≤ 0,5` con `αr real = (255·αs + 255·αb − αs·αb) / 255`.
    pub fn alpha_meets(integer: u8, ab: i128, as_: i128) -> bool {
        let real_per_255 = 255 * as_ + 255 * ab - as_ * ab;
        2 * (i128::from(integer) * 255 - real_per_255).abs() <= 255
    }

    /// Dominio del paso 4b (con `ab > 0` y `as_ > 0`).
    pub fn in_domain(mode: BlendMode, ab: i128) -> bool {
        ab > 0 && ab < 255 && mode != BlendMode::Normal
    }

    /// Modos cuya función de mezcla redondea (`div255`): la mezcla ya está a ≤ 0,5 del real.
    fn blend_rounded(mode: BlendMode) -> bool {
        matches!(
            mode,
            BlendMode::Multiply | BlendMode::Screen | BlendMode::Overlay
        )
    }

    /// Techo de error con el `αr` entero: la tolerancia empírica dentro del dominio del paso 4b
    /// y fuera, `1` (la interpolación trunca: < 1) más, en los tres modos que redondean la
    /// mezcla, el ≤ 0,5 de esa mezcla multiplicado por `αs / 255`: 3/2.
    // Cotas de la spec (propiedad 2): 3/2 en Multiplicar, Trama y Superponer (la mezcla redondea:
    // medido 1,44) y 1 en el resto fuera del dominio de 4b.
    pub fn tolerance_whole(mode: BlendMode, ab: i128) -> (i128, i128) {
        if in_domain(mode, ab) {
            (TOLERANCE_EMPIRICAL, 1)
        } else if blend_rounded(mode) {
            (3, 2)
        } else {
            (1, 1)
        }
    }

    /// Techo de error con el `αr` real: con fondo opaco, el mismo que con el `αr` entero (son el
    /// mismo); con fondo translúcido, 5, también con Normal (el `αr` entero redondeado se
    /// amplifica con alfas pequeñas: máximo medido por la spec 4,57).
    // Cotas de la spec (propiedad 2): con fondo translúcido, 5 frente al `αr` real (el `αr`
    // entero se amplifica con alfas pequeñas); con `αb = 255` vale lo de `tolerance_whole`.
    pub fn tolerance_real(mode: BlendMode, ab: i128) -> (i128, i128) {
        if ab == 255 {
            tolerance_whole(mode, ab)
        } else {
            (5, 1)
        }
    }
}

/// `x / 255` al entero más cercano (sin empates posibles), a partir de la definición y no del
/// truco de bits de la spec.
fn rounds_255(x: i128) -> i128 {
    (2 * x + 255).div_euclid(510)
}

// ---------------------------------------------------------------------------------------
// Colores, documentos y comprobaciones de apoyo.
// ---------------------------------------------------------------------------------------

const T: Rgba = Rgba::TRANSPARENT;

const fn c(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

/// Una capa de un documento de prueba. `cel_2: None` es una celda vacía (sin imagen); la capa 0
/// siempre tiene celda, así que en ella `None` deja la imagen inicial (toda a ceros).
#[derive(Clone, Debug)]
struct Layer2 {
    visible: bool,
    opacity_2: u8,
    mode: BlendMode,
    cel_2: Option<Vec<Pixel>>,
}

impl Layer2 {
    fn new(cel_2: Vec<Pixel>) -> Layer2 {
        Layer2 {
            visible: true,
            opacity_2: 255,
            mode: BlendMode::Normal,
            cel_2: Some(cel_2),
        }
    }

    fn rgba(cel_2: &[Rgba]) -> Layer2 {
        Layer2::new(cel_2.iter().copied().map(Pixel::Rgba).collect())
    }

    fn empty() -> Layer2 {
        Layer2 {
            visible: true,
            opacity_2: 255,
            mode: BlendMode::Normal,
            cel_2: None,
        }
    }

    fn mode(mut self, mode: BlendMode) -> Layer2 {
        self.mode = mode;
        self
    }

    fn opacity_2(mut self, opacity_2: u8) -> Layer2 {
        self.opacity_2 = opacity_2;
        self
    }

    fn hidden(mut self) -> Layer2 {
        self.visible = false;
        self
    }
}

/// Documento con `layers` de abajo arriba, a partir de la imagen `template` (que es la celda de
/// la capa 0 y fija el tamaño y el modo). Solo usa los comandos del núcleo.
fn builds(template: Image, layers: &[Layer2]) -> Document {
    let mut doc = Document::new(template);
    let (w, h) = (doc.width(), doc.height());
    for i in 1..layers.len() {
        let cmd = Command::AddLayer {
            name: format!("C{i}"),
            at: i,
        };
        assert_eq!(doc.apply(cmd), Ok(true), "add layer {i}");
    }
    for (i, layer_2) in layers.iter().enumerate() {
        let (id, cel) = (doc.layers()[i].id(), doc.layers()[i].cel());
        if let Some(pixels_2) = &layer_2.cel_2 {
            assert_eq!(pixels_2.len(), (w * h) as usize, "pixels of layer {i}");
            let pixels = pixels_2
                .iter()
                .enumerate()
                .map(|(k, &pixel)| PixelEdit {
                    x: k as u32 % w,
                    y: k as u32 / w,
                    pixel,
                })
                .collect();
            doc.apply(Command::SetPixels { image: cel, pixels })
                .expect("escribir la celda");
        }
        if layer_2.opacity_2 != 255 {
            let cmd = Command::SetLayerOpacity {
                layer: id,
                opacity: layer_2.opacity_2,
            };
            assert!(doc.apply(cmd).is_ok(), "opacity of layer {i}");
        }
        if layer_2.mode != BlendMode::Normal {
            let cmd = Command::SetLayerBlend {
                layer: id,
                blend: layer_2.mode,
            };
            assert!(doc.apply(cmd).is_ok(), "mode of layer {i}");
        }
        if !layer_2.visible {
            let cmd = Command::SetLayerVisible {
                layer: id,
                visible: false,
            };
            assert!(doc.apply(cmd).is_ok(), "hide layer {i}");
        }
    }
    doc
}

fn image_2(w: u32, h: u32, mode: Mode) -> Image {
    Image::new(w, h, mode).expect("imagen")
}

fn doc_rgba(w: u32, h: u32, layers: &[Layer2]) -> Document {
    builds(image_2(w, h, Mode::Rgba), layers)
}

/// Paleta de `n` colores opacos distintos con el índice transparente `t`.
fn palette(n: u8, t: Option<u8>) -> Palette {
    let mut p = Palette::from_colors(
        (0..n).map(|i| Rgba::new(i.wrapping_mul(30), 255 - i.wrapping_mul(30), 7, 255)),
    )
    .expect("paleta");
    p.set_transparent_index(t).expect("índice transparente");
    p
}

fn template_indexed(w: u32, h: u32, n: u8, t: Option<u8>) -> Image {
    Image::new_indexed(w, h, palette(n, t)).expect("imagen indexada")
}

fn idx(values: &[u8]) -> Vec<Pixel> {
    values.iter().copied().map(Pixel::Indexed).collect()
}

/// Los píxeles de una imagen RGBA, fila a fila.
fn rgba_of(img: &Image) -> Vec<Rgba> {
    assert_eq!(img.mode(), Mode::Rgba, "an RGBA image was expected");
    (0..img.height())
        .flat_map(|y| (0..img.width()).map(move |x| (x, y)))
        .map(|(x, y)| match img.get_pixel(x, y).expect("píxel dentro") {
            Pixel::Rgba(color) => color,
            other => panic!("se esperaba RGBA, hay {other:?}"),
        })
        .collect()
}

/// Los píxeles de una imagen en cualquier modo, fila a fila.
fn pixels_of(img: &Image) -> Vec<Pixel> {
    (0..img.height())
        .flat_map(|y| (0..img.width()).map(move |x| (x, y)))
        .map(|(x, y)| img.get_pixel(x, y).expect("píxel dentro"))
        .collect()
}

/// Los índices de una imagen indexada, fila a fila.
fn indices_of(img: &Image) -> Vec<u8> {
    pixels_of(img)
        .into_iter()
        .map(|p| match p {
            Pixel::Indexed(i) => i,
            other => panic!("se esperaba indexado, hay {other:?}"),
        })
        .collect()
}

/// `compose` que no debe fallar.
fn compose_2(doc: &Document) -> Image {
    compose(doc).expect("la composición no debe fallar")
}

/// El recorte de `img` en `r` (mismo modo y misma paleta).
fn clip(img: &Image, r: Rect) -> Image {
    let mut output = match img.palette() {
        Some(palette) => Image::new_indexed(r.width, r.height, palette.clone()),
        None => Image::new(r.width, r.height, img.mode()),
    }
    .expect("imagen del recorte");
    for y in 0..r.height {
        for x in 0..r.width {
            let pixel = img.get_pixel(r.x + x, r.y + y).expect("el recorte cabe");
            output.set_pixel(x, y, pixel).expect("escribir el recorte");
        }
    }
    output
}

// ---------------------------------------------------------------------------------------
// Fila 1: composite_pixel, identidades y bordes.
// ---------------------------------------------------------------------------------------

#[test]
fn row_01_identities_and_borders() {
    let background = c(196, 46, 52, 255);
    let source = c(90, 60, 220, 255);

    for mode in BlendMode::ALL {
        // Origen con alfa 0 sobre fondo opaco: el fondo, sin cambios.
        assert_eq!(
            composite_pixel(background, c(10, 20, 30, 0), 255, mode),
            background,
            "αs = 0, {mode:?}"
        );
        // Opacidad 0: lo mismo.
        assert_eq!(
            composite_pixel(background, source, 0, mode),
            background,
            "opacidad 0, {mode:?}"
        );
        // Contra el fondo vacío no hay con qué mezclar: el RGB del origen se conserva, también
        // con alfa 0.
        assert_eq!(
            composite_pixel(T, c(232, 198, 52, 0), 255, mode),
            c(232, 198, 52, 0),
            "empty background, αs = 0, {mode:?}"
        );
        // Un origen exactamente `(0, 0, 0, 0)` es «nada» (como una celda vacía): no pisa el RGB
        // de un fondo con alfa 0.
        assert_eq!(
            composite_pixel(c(255, 255, 255, 0), T, 255, mode),
            c(255, 255, 255, 0),
            "source (0, 0, 0, 0) over a background with alpha 0 and RGB, {mode:?}"
        );
    }

    // Origen opaco con Normal: el origen exacto.
    assert_eq!(
        composite_pixel(background, source, 255, BlendMode::Normal),
        source
    );

    // Fondo vacío y origen con alfa: sin mezcla, y la opacidad escala el alfa (αs = div255(200
    // · 100) = 78).
    assert_eq!(
        composite_pixel(T, c(10, 20, 30, 200), 255, BlendMode::Multiply),
        c(10, 20, 30, 200)
    );
    assert_eq!(
        composite_pixel(T, c(10, 20, 30, 200), 100, BlendMode::Multiply),
        c(10, 20, 30, 78)
    );
}

// ---------------------------------------------------------------------------------------
// Fila 2: los ocho modos con fondo y origen opacos.
// ---------------------------------------------------------------------------------------

#[test]
fn row_02_the_eight_modes_over_an_opaque_background() {
    let background = c(196, 46, 52, 255);
    let source = c(90, 60, 220, 255);
    // Valores de la sección 5 de specs/corpus-hito-3.md, hechos a mano.
    let expected_2 = [
        (BlendMode::Normal, (90, 60, 220)),
        (BlendMode::Multiply, (69, 11, 45)),
        (BlendMode::Screen, (217, 95, 227)),
        (BlendMode::Overlay, (179, 22, 90)),
        (BlendMode::Darken, (90, 46, 52)),
        (BlendMode::Lighten, (196, 60, 220)),
        (BlendMode::Add, (255, 106, 255)),
        (BlendMode::Difference, (106, 14, 168)),
    ];
    for (mode, (r, g, b)) in expected_2 {
        assert_eq!(
            composite_pixel(background, source, 255, mode),
            c(r, g, b, 255),
            "{mode:?}"
        );
    }

    // Superponer en el umbral: con el fondo en 127 se usa la rama de multiplicar y con 128, la de
    // trama (lo fijó el corpus). Origen 100.
    let gray_2 = |v: u8| c(v, v, v, 255);
    assert_eq!(
        composite_pixel(gray_2(127), gray_2(100), 255, BlendMode::Overlay),
        gray_2(100)
    );
    assert_eq!(
        composite_pixel(gray_2(128), gray_2(100), 255, BlendMode::Overlay),
        gray_2(101)
    );
}

// ---------------------------------------------------------------------------------------
// Fila 3: opacidad sobre fondo opaco.
// ---------------------------------------------------------------------------------------

#[test]
fn row_03_opacity_over_an_opaque_background() {
    let normal = BlendMode::Normal;
    // Rojo sobre negro con 64.
    assert_eq!(
        composite_pixel(c(0, 0, 0, 255), c(196, 46, 52, 255), 64, normal),
        c(49, 11, 13, 255)
    );
    // Blanco con 128 sobre negro y sobre gris 128.
    assert_eq!(
        composite_pixel(c(0, 0, 0, 255), c(255, 255, 255, 255), 128, normal),
        c(128, 128, 128, 255)
    );
    assert_eq!(
        composite_pixel(c(128, 128, 128, 255), c(255, 255, 255, 255), 128, normal),
        c(191, 191, 191, 255)
    );
    // Diferencia con opacidad 90 de (66, 196, 206) sobre gris 128: la interpolación trunca hacia
    // cero (el tercer canal, −17,65, da −17 y no −18: 111 y no 110).
    assert_eq!(
        composite_pixel(
            c(128, 128, 128, 255),
            c(66, 196, 206, 255),
            90,
            BlendMode::Difference
        ),
        c(105, 107, 111, 255)
    );
}

// ---------------------------------------------------------------------------------------
// Fila 4: fondo translúcido (valores a mano y barrido).
// ---------------------------------------------------------------------------------------

#[test]
fn row_04_a_translucent_background_values() {
    // Normal sobre un fondo con alfa 150 (dato del corpus).
    let background = c(58, 168, 76, 150);
    assert_eq!(
        composite_pixel(background, c(211, 17, 99, 100), 255, BlendMode::Normal),
        c(138, 89, 88, 191)
    );
    assert_eq!(
        composite_pixel(background, c(211, 17, 99, 255), 255, BlendMode::Normal),
        c(211, 17, 99, 255)
    );

    // Empírica (4b). Los dos primeros son iguales al PNG de `blend-eight-background-translucent`.
    let black_150 = c(0, 0, 0, 150);
    assert_eq!(
        composite_pixel(black_150, c(255, 255, 255, 255), 255, BlendMode::Multiply),
        c(43, 43, 43, 255)
    );
    assert_eq!(
        composite_pixel(black_150, c(255, 255, 255, 100), 255, BlendMode::Multiply),
        c(42, 42, 42, 191)
    );
    assert_eq!(
        composite_pixel(black_150, c(255, 255, 255, 255), 255, BlendMode::Difference),
        c(255, 255, 255, 255)
    );
    // Superponer con opacidad 128 (αs = 128): el alfa de salida es 128 + 100 − 50 = 178.
    assert_eq!(
        composite_pixel(
            c(128, 128, 128, 100),
            c(200, 200, 200, 255),
            128,
            BlendMode::Overlay
        ),
        c(180, 180, 180, 178)
    );
}

/// Barrido de los siete modos distintos de Normal con fondo translúcido: para cada combinación,
/// el alfa de salida está a ≤ 0,5 del real y los tres canales (grises: `R = G = B`, los modos
/// son separables) a ≤ `TOLERANCE_EMPIRICAL` de la forma real observada con el `αr` entero.
fn sweep(alphas_background: &[u8], alphas_source: &[u8], colors: &[u8]) {
    for mode in BlendMode::ALL {
        if mode == BlendMode::Normal {
            continue;
        }
        for &ab in alphas_background {
            for &a_s in alphas_source {
                let (abi, so) = (i128::from(ab), i128::from(a_s));
                for &cb in colors {
                    for &cs in colors {
                        let output =
                            composite_pixel(c(cb, cb, cb, ab), c(cs, cs, cs, a_s), 255, mode);
                        assert!(
                            model::alpha_meets(output.a, abi, so),
                            "{mode:?}: αb {ab}, αs {a_s}: the output alpha {} is more than 0.5 away from the real one",
                            output.a
                        );
                        let expected =
                            model::result(mode, cb, cs, abi, so, (i128::from(output.a), 1));
                        // Grises: los tres canales iguales (los modos son separables).
                        assert!(
                            output.r == output.g && output.g == output.b,
                            "{mode:?}: αb {ab}, αs {a_s}, Cb {cb}, Cs {cs}: the channels of a gray \
                             differ: {output:?}"
                        );
                        assert!(
                            model::meets(output.r, expected, (TOLERANCE_EMPIRICAL, 1)),
                            "{mode:?}: αb {ab}, αs {a_s}, Cb {cb}, Cs {cs}: the channel gives {} and the \
                             model {}/{} (tolerance {TOLERANCE_EMPIRICAL})",
                            output.r,
                            expected.0,
                            expected.1
                        );
                    }
                }
            }
        }
    }
}

/// Rejilla de colores por lado: 0, 17, …, 255 (16 valores) y 1, 2, 127, 128, 129, 254.
// La lista de colores de la spec (0, 17, …, 255 más 1, 2, 127, 128, 129, 254) tiene 22 valores: 484 parejas.
fn grid_of_colors() -> Vec<u8> {
    let mut colors: Vec<u8> = (0..=255u8).step_by(17).collect();
    colors.extend([1, 2, 127, 128, 129, 254]);
    colors
}

#[test]
fn row_04_b_fast_sweep_of_alphas() {
    // αb en 1..=254 con paso 5 más 127; αs en 1..=255 con paso 5 más 127 y 255.
    let mut alphas_background: Vec<u8> = (1..=254u8).step_by(5).collect();
    alphas_background.push(127);
    let mut alphas_source: Vec<u8> = (1..=255u8).step_by(5).collect();
    alphas_source.extend([127, 255]);
    sweep(&alphas_background, &alphas_source, &grid_of_colors());
}

#[test]
#[ignore = "barrido completo: cargo test --release -p px-raster --test compose -- --ignored"]
fn row_04_c_full_sweep_of_alphas() {
    // Todas las combinaciones: 254 alfas de fondo por 255 de origen.
    let alphas_background: Vec<u8> = (1..=254).collect();
    let alphas_source: Vec<u8> = (1..=255).collect();
    sweep(&alphas_background, &alphas_source, &grid_of_colors());
}

// ---------------------------------------------------------------------------------------
// Fila 5: pila de capas (RGBA).
// ---------------------------------------------------------------------------------------

const WHITE: Rgba = c(255, 255, 255, 255);
const GRAY_128: Rgba = c(128, 128, 128, 255);

/// `D(3)` de la fila 5, de 2 × 1: el fondo opaco `A = (200, 100, 50)`; encima una capa Normal
/// con `(10, 20, 30)` solo en el primer píxel; y encima una capa Multiplicar con blanco en el
/// primero y gris 128 en el segundo.
fn stack_by_hand() -> Vec<Layer2> {
    let a = c(200, 100, 50, 255);
    vec![
        Layer2::rgba(&[a, a]),
        Layer2::rgba(&[c(10, 20, 30, 255), T]),
        Layer2::rgba(&[WHITE, GRAY_128]).mode(BlendMode::Multiply),
    ]
}

/// El compuesto a mano de `stack_by_hand`: el primer píxel es `(10, 20, 30)` (multiplicar por
/// blanco no cambia nada) y el segundo, `A` multiplicado por 128: div255(200·128) = 100,
/// div255(100·128) = 50 y div255(50·128) = 25.
const COMPOSITE_TO_HAND: [Rgba; 2] = [c(10, 20, 30, 255), c(100, 50, 25, 255)];

#[test]
fn row_05_layer_stack() {
    // El compuesto a mano.
    assert_eq!(
        rgba_of(&compose_2(&doc_rgba(2, 1, &stack_by_hand()))),
        COMPOSITE_TO_HAND
    );

    // Orden de abajo arriba: el mismo par de capas opacas en un orden y en el otro.
    let (a, b) = (c(200, 100, 50, 255), c(10, 20, 30, 255));
    let below_a = doc_rgba(2, 1, &[Layer2::rgba(&[a, a]), Layer2::rgba(&[b, b])]);
    let below_b = doc_rgba(2, 1, &[Layer2::rgba(&[b, b]), Layer2::rgba(&[a, a])]);
    assert_eq!(rgba_of(&compose_2(&below_a)), [b, b]);
    assert_eq!(rgba_of(&compose_2(&below_b)), [a, a]);

    // Una capa oculta no cuenta, aunque cubra todo y sea opaca.
    let mut with_hidden = stack_by_hand();
    with_hidden.push(Layer2::rgba(&[c(1, 2, 3, 255), c(1, 2, 3, 255)]).hidden());
    assert_eq!(
        rgba_of(&compose_2(&doc_rgba(2, 1, &with_hidden))),
        COMPOSITE_TO_HAND
    );

    // Una capa con la celda vacía no aporta nada (ni siquiera con modo y opacidad raros).
    let mut with_empty = stack_by_hand();
    with_empty.push(Layer2::empty().mode(BlendMode::Multiply).opacity_2(100));
    assert_eq!(
        rgba_of(&compose_2(&doc_rgba(2, 1, &with_empty))),
        COMPOSITE_TO_HAND
    );

    // Ni siquiera el RGB: contra el fondo vacío, una celda con RGB bajo alfa 0 lo conserva y una
    // celda vacía, no.
    let empty_2 = [T, T];
    let with_rgb = doc_rgba(
        2,
        1,
        &[
            Layer2::rgba(&empty_2),
            Layer2::rgba(&[c(232, 198, 52, 0), T]),
        ],
    );
    assert_eq!(rgba_of(&compose_2(&with_rgb)), [c(232, 198, 52, 0), T]);
    let without_cel = doc_rgba(2, 1, &[Layer2::rgba(&empty_2), Layer2::empty()]);
    assert_eq!(rgba_of(&compose_2(&without_cel)), [T, T]);

    // Opacidad 0: igual que oculta sobre un fondo opaco...
    let base = Layer2::rgba(&[a, a]);
    let covers = [b, b];
    let op0 = doc_rgba(2, 1, &[base.clone(), Layer2::rgba(&covers).opacity_2(0)]);
    let hidden = doc_rgba(2, 1, &[base, Layer2::rgba(&covers).hidden()]);
    assert_eq!(rgba_of(&compose_2(&op0)), [a, a]);
    assert_eq!(compose_2(&op0), compose_2(&hidden));
    // ...y salvo el RGB si el fondo es vacío (αs = 0 contra el vacío conserva el origen).
    let source = [c(232, 198, 52, 200), T];
    let op_0_empty = doc_rgba(
        2,
        1,
        &[Layer2::rgba(&empty_2), Layer2::rgba(&source).opacity_2(0)],
    );
    let hidden_empty = doc_rgba(
        2,
        1,
        &[Layer2::rgba(&empty_2), Layer2::rgba(&source).hidden()],
    );
    assert_eq!(rgba_of(&compose_2(&op_0_empty)), [c(232, 198, 52, 0), T]);
    assert_eq!(rgba_of(&compose_2(&hidden_empty)), [T, T]);

    // Capa de fondo con opacidad 128 y modo Multiplicar: el modo no tiene efecto y el alfa se
    // escala (αs = 128 y div255(100 · 128) = 50).
    let background = doc_rgba(
        2,
        1,
        &[Layer2::rgba(&[a, c(10, 20, 30, 100)])
            .mode(BlendMode::Multiply)
            .opacity_2(128)],
    );
    assert_eq!(
        rgba_of(&compose_2(&background)),
        [c(200, 100, 50, 128), c(10, 20, 30, 50)]
    );

    // Todas ocultas: transparente en todo el lienzo, y del tamaño del documento.
    let all: Vec<Layer2> = stack_by_hand().into_iter().map(Layer2::hidden).collect();
    let output = compose_2(&doc_rgba(2, 1, &all));
    assert_eq!(
        (output.width(), output.height(), output.mode()),
        (2, 1, Mode::Rgba)
    );
    assert_eq!(rgba_of(&output), [T, T]);
}

// ---------------------------------------------------------------------------------------
// Fila 6: grises con alfa y grises sin alfa.
// ---------------------------------------------------------------------------------------

/// Una pila de 3 × 2 en grises con alfa `(gray_2, alpha_2)`, con capa oculta, vacía, opacidad,
/// modos, alfas intermedios y RGB bajo alfa 0. `pixel` da el píxel del modo que se prueba.
fn stack_of_grays(pixel: impl Fn(u8, u8) -> Pixel) -> Vec<Layer2> {
    let cel_2 = |values: [(u8, u8); 6]| -> Vec<Pixel> {
        values.iter().map(|&(g, a)| pixel(g, a)).collect()
    };
    vec![
        Layer2::new(cel_2([
            (200, 255),
            (100, 255),
            (50, 128),
            (0, 0),
            (255, 60),
            (30, 0),
        ])),
        Layer2::new(cel_2([
            (128, 255),
            (255, 100),
            (10, 255),
            (90, 200),
            (0, 0),
            (77, 0),
        ]))
        .mode(BlendMode::Multiply)
        .opacity_2(200),
        Layer2::new(cel_2([(1, 255); 6])).hidden(),
        Layer2::new(cel_2([
            (100, 150),
            (0, 0),
            (255, 255),
            (40, 40),
            (200, 200),
            (60, 60),
        ]))
        .mode(BlendMode::Screen)
        .opacity_2(128),
        Layer2::empty().mode(BlendMode::Overlay),
        Layer2::new(cel_2([(9, 99); 6]))
            .mode(BlendMode::Difference)
            .opacity_2(0),
    ]
}

#[test]
fn row_06_grays_with_and_without_alpha() {
    // GrayAlpha: el canal rojo del RGBA equivalente (R = G = B) y su alfa.
    let rgba = rgba_doc_of_grays();
    let grays = builds(
        image_2(3, 2, Mode::GrayAlpha),
        &stack_of_grays(|gray, alpha| Pixel::GrayAlpha { gray, alpha }),
    );
    let expected = rgba_of(&compose_2(&rgba));
    let output = compose_2(&grays);
    assert_eq!(output.mode(), Mode::GrayAlpha);
    assert_eq!((output.width(), output.height()), (3, 2));
    let obtained = pixels_of(&output);
    assert_eq!(obtained.len(), expected.len());
    for (k, (pixel, rgb)) in obtained.iter().zip(&expected).enumerate() {
        assert_eq!(
            *pixel,
            Pixel::GrayAlpha {
                gray: rgb.r,
                alpha: rgb.a
            },
            "pixel {k}: the equivalent RGBA is {rgb:?}"
        );
        assert!(
            rgb.r == rgb.g && rgb.g == rgb.b,
            "the equivalent RGBA is gray"
        );
    }

    // GrayAlpha con todas las capas ocultas: transparente (gris 0, alfa 0) en todo el lienzo.
    let all: Vec<Layer2> = stack_of_grays(|gray, alpha| Pixel::GrayAlpha { gray, alpha })
        .into_iter()
        .map(Layer2::hidden)
        .collect();
    let hidden_2 = compose_2(&builds(image_2(3, 2, Mode::GrayAlpha), &all));
    assert!(
        pixels_of(&hidden_2)
            .iter()
            .all(|p| *p == Pixel::GrayAlpha { gray: 0, alpha: 0 })
    );

    // Gray (sin alfa) con una capa: la celda, tal cual.
    let mut cel_2 = image_2(3, 2, Mode::Gray);
    for (k, value) in [0u8, 10, 20, 30, 250, 255].into_iter().enumerate() {
        cel_2
            .set_pixel(k as u32 % 3, k as u32 / 3, Pixel::Gray(value))
            .expect("píxel");
    }
    let mut gray_2 = Document::new(cel_2.clone());
    assert_eq!(compose(&gray_2), Ok(cel_2));

    // Gray con la única capa oculta: no queda nada que dar y no hay valor transparente.
    let layer_2 = gray_2.layers()[0].id();
    let cmd = Command::SetLayerVisible {
        layer: layer_2,
        visible: false,
    };
    assert_eq!(gray_2.apply(cmd), Ok(true));
    assert_eq!(
        compose(&gray_2),
        Err(ComposeError::NoTransparency { mode: Mode::Gray })
    );
}

/// La pila de `stack_of_grays` en RGBA con `R = G = B`.
fn rgba_doc_of_grays() -> Document {
    doc_rgba(
        3,
        2,
        &stack_of_grays(|g, a| Pixel::Rgba(Rgba::new(g, g, g, a))),
    )
}

// ---------------------------------------------------------------------------------------
// Fila 7: indexado.
// ---------------------------------------------------------------------------------------

#[test]
fn row_07_indexed_the_highest_visible_non_transparent_layer_wins() {
    // Tres capas, transparente 0, lienzo de 4 × 1.
    let base = || template_indexed(4, 1, 8, Some(0));
    let l0 = || Layer2::new(idx(&[1, 2, 3, 0]));
    let l1 = || Layer2::new(idx(&[0, 4, 0, 0]));
    let l2 = || Layer2::new(idx(&[5, 0, 0, 0]));

    let three = builds(base(), &[l0(), l1(), l2()]);
    let output = compose_2(&three);
    assert_eq!(indices_of(&output), [5, 4, 3, 0]);
    assert_eq!(output.mode(), Mode::Indexed);
    assert_eq!(
        output.palette(),
        three.palette(),
        "the palette is the canvas one"
    );
    assert_eq!((output.width(), output.height()), (4, 1));

    // Capa oculta: no cuenta.
    let hidden = builds(base(), &[l0(), l1(), l2().hidden()]);
    assert_eq!(indices_of(&compose_2(&hidden)), [1, 4, 3, 0]);

    // Celda vacía: no aporta nada (encima y en medio).
    let empty_top = builds(base(), &[l0(), l1(), l2(), Layer2::empty()]);
    assert_eq!(indices_of(&compose_2(&empty_top)), [5, 4, 3, 0]);
    let empty_in_middle = builds(base(), &[l0(), Layer2::empty(), l2().hidden()]);
    assert_eq!(indices_of(&compose_2(&empty_in_middle)), [1, 2, 3, 0]);

    // Todo transparente: el índice transparente.
    let nothing = builds(base(), &[Layer2::new(idx(&[0, 0, 0, 0])), Layer2::empty()]);
    assert_eq!(indices_of(&compose_2(&nothing)), [0, 0, 0, 0]);

    // Con el transparente 7, el índice 0 sí es opaco y tapa a lo de debajo.
    let base7 = || template_indexed(4, 1, 8, Some(7));
    let with_7 = builds(
        base7(),
        &[
            Layer2::new(idx(&[3, 3, 3, 3])),
            Layer2::new(idx(&[7, 2, 0, 7])),
        ],
    );
    let output_7 = compose_2(&with_7);
    assert_eq!(indices_of(&output_7), [3, 2, 0, 3]);
    assert_eq!(output_7.palette(), with_7.palette());

    // Y donde no hay nada, el 7.
    let nothing_7 = builds(base7(), &[Layer2::new(idx(&[7, 7, 7, 7]))]);
    assert_eq!(indices_of(&compose_2(&nothing_7)), [7, 7, 7, 7]);
}

#[test]
fn row_07_indexed_without_transparent_index() {
    // Una sola capa llena: la celda tal cual.
    let full = Document::new({
        let mut img = template_indexed(4, 1, 8, None);
        for (x, i) in [1u8, 2, 3, 0].into_iter().enumerate() {
            img.set_pixel(x as u32, 0, Pixel::Indexed(i))
                .expect("píxel");
        }
        img
    });
    let cel_2 = full.image(full.layers()[0].cel()).expect("celda").clone();
    assert_eq!(compose(&full), Ok(cel_2));

    // Con una celda vacía hace falta una segunda capa: se añade con un transparente y luego se
    // quita (`SetTransparentIndex`).
    let mut doc = builds(
        template_indexed(4, 1, 8, Some(0)),
        &[Layer2::new(idx(&[1, 2, 3, 0])), Layer2::empty()],
    );
    let cel = doc.layers()[0].cel();
    let removes = Command::SetTransparentIndex {
        image: cel,
        index: None,
    };
    assert_eq!(doc.apply(removes), Ok(true));
    assert_eq!(doc.palette().and_then(Palette::transparent_index), None);

    // La capa de abajo llena y visible: gana, también donde vale 0 (sin transparente, el 0 es
    // un color como los demás).
    assert_eq!(indices_of(&compose_2(&doc)), [1, 2, 3, 0]);

    // La de abajo oculta y la de arriba vacía: no queda ningún píxel que dar.
    let hidden = Command::SetLayerVisible {
        layer: doc.layers()[0].id(),
        visible: false,
    };
    assert_eq!(doc.apply(hidden), Ok(true));
    assert_eq!(
        compose(&doc),
        Err(ComposeError::NoTransparency {
            mode: Mode::Indexed
        })
    );
}

// ---------------------------------------------------------------------------------------
// Fila 8: compose_rect.
// ---------------------------------------------------------------------------------------

/// Documento RGBA de 6 × 5 con dos capas que dan un píxel distinto en cada posición: un fondo
/// opaco y encima una capa Trama con alfa y opacidad 200.
fn doc_6x5() -> Document {
    let background: Vec<Rgba> = (0..30u32)
        .map(|k| {
            let (x, y) = ((k % 6) as u8, (k / 6) as u8);
            c(10 + x * 40, 5 + y * 50, 20 * (x + y), 255)
        })
        .collect();
    let top: Vec<Rgba> = (0..30u32)
        .map(|k| {
            let (x, y) = ((k % 6) as u8, (k / 6) as u8);
            c(200 - 20 * x, 100 + 10 * y, 30 + 15 * x, 128 + 20 * y)
        })
        .collect();
    doc_rgba(
        6,
        5,
        &[
            Layer2::rgba(&background),
            Layer2::rgba(&top).mode(BlendMode::Screen).opacity_2(200),
        ],
    )
}

fn rect(x: u32, y: u32, width: u32, height: u32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn row_08_compose_rect() {
    let doc = doc_6x5();
    let integer = compose_2(&doc);
    assert_eq!((integer.width(), integer.height()), (6, 5));

    // Interior, borde, una fila y todo el lienzo: el tamaño del rectángulo y el recorte exacto.
    for r in [
        rect(1, 1, 3, 2),
        rect(3, 0, 3, 5),
        rect(0, 4, 6, 1),
        rect(2, 0, 1, 1),
        rect(0, 0, 6, 5),
    ] {
        let part = compose_rect(&doc, r).unwrap_or_else(|e| panic!("{r:?}: {e}"));
        assert_eq!((part.width(), part.height()), (r.width, r.height), "{r:?}");
        assert_eq!(part, clip(&integer, r), "{r:?}");
        // La memoria reservada es la del rectángulo.
        assert_eq!(
            Some(part.bytes().len()),
            Image::byte_len(r.width, r.height, Mode::Rgba).ok(),
            "{r:?}"
        );
    }
    assert_eq!(compose_rect(&doc, rect(0, 0, 6, 5)), Ok(integer));

    // Vacío: ancho o alto 0.
    assert_eq!(
        compose_rect(&doc, rect(1, 1, 0, 2)),
        Err(ComposeError::EmptyRect)
    );
    assert_eq!(
        compose_rect(&doc, rect(1, 1, 2, 0)),
        Err(ComposeError::EmptyRect)
    );

    // Fuera por la derecha, por abajo, entero fuera y más grande que el lienzo; con los datos
    // pedidos (el rectángulo y el tamaño del lienzo).
    for r in [
        rect(4, 0, 3, 2),
        rect(0, 4, 2, 2),
        rect(10, 0, 1, 1),
        rect(0, 0, 7, 5),
        rect(0, 0, 6, 6),
        rect(0, 0, 100, 100),
        // La suma `x + ancho` desborda `u32`: es «no cabe», no un pánico.
        rect(u32::MAX, 0, 2, 1),
        rect(0, u32::MAX - 1, 1, 3),
    ] {
        assert_eq!(
            compose_rect(&doc, r),
            Err(ComposeError::RectOutside {
                rect: r,
                width: 6,
                height: 5
            }),
            "{r:?}"
        );
    }

    // `Rect` es `Copy`, `Eq` y `Debug`, y el error se muestra en español (no vacío).
    let r = rect(1, 2, 3, 4);
    let copy = r;
    assert_eq!(r, copy);
    assert!(!format!("{r:?}").is_empty());
    assert!(!ComposeError::EmptyRect.to_string().is_empty());
}

// ---------------------------------------------------------------------------------------
// Generadores de documentos para las propiedades.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Type {
    Rgba,
    GrayAlpha,
    /// Indexado con ese índice transparente (paleta de 4 colores).
    Indexed(u8),
    /// Indexado sin índice transparente (se quita después de añadir las capas).
    IndexedWithoutT,
    Gray,
}

#[derive(Clone, Debug)]
struct DocGen {
    kind: Type,
    w: u32,
    h: u32,
    layers: Vec<Layer2>,
}

/// Alfa o opacidad con los extremos más frecuentes que en el uniforme.
fn alpha_2() -> impl Strategy<Value = u8> {
    prop_oneof![1 => Just(0u8), 1 => Just(255u8), 2 => any::<u8>()]
}

fn mode_any() -> impl Strategy<Value = BlendMode> {
    proptest::sample::select(BlendMode::ALL.to_vec())
}

fn type_strategy(complete: bool) -> BoxedStrategy<Type> {
    let basic = prop_oneof![
        Just(Type::Rgba),
        Just(Type::GrayAlpha),
        (0u8..4).prop_map(Type::Indexed),
    ];
    if complete {
        prop_oneof![
            3 => basic,
            1 => Just(Type::IndexedWithoutT),
            1 => Just(Type::Gray),
        ]
        .boxed()
    } else {
        basic.boxed()
    }
}

fn pixel_strategy(kind: Type) -> BoxedStrategy<Pixel> {
    match kind {
        Type::Rgba => (any::<u8>(), any::<u8>(), any::<u8>(), alpha_2())
            .prop_map(|(r, g, b, a)| Pixel::Rgba(Rgba::new(r, g, b, a)))
            .boxed(),
        Type::GrayAlpha => (any::<u8>(), alpha_2())
            .prop_map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
            .boxed(),
        Type::Indexed(_) | Type::IndexedWithoutT => (0u8..4).prop_map(Pixel::Indexed).boxed(),
        Type::Gray => any::<u8>().prop_map(Pixel::Gray).boxed(),
    }
}

fn layer_strategy(kind: Type, pixels_2: usize) -> impl Strategy<Value = Layer2> {
    let indexed = matches!(kind, Type::Indexed(_) | Type::IndexedWithoutT);
    (
        proptest::bool::weighted(0.85),
        alpha_2(),
        mode_any(),
        proptest::option::weighted(
            0.8,
            proptest::collection::vec(pixel_strategy(kind), pixels_2),
        ),
    )
        .prop_map(move |(visible, opacity_2, mode, cel_2)| {
            // En indexado el núcleo solo admite opacidad 255 y modo Normal.
            let (opacity_2, mode) = if indexed {
                (255, BlendMode::Normal)
            } else {
                (opacity_2, mode)
            };
            Layer2 {
                visible,
                opacity_2,
                mode,
                cel_2,
            }
        })
}

/// Documentos de hasta 6 × 5 con 1 a 4 capas (una sola en grises sin alfa). Con `complete`
/// entran también los grises sin alfa y el indexado sin transparente.
fn doc_strategy(complete: bool) -> impl Strategy<Value = DocGen> {
    (type_strategy(complete), 1u32..=6, 1u32..=5).prop_flat_map(|(kind, w, h)| {
        let layers = if matches!(kind, Type::Gray) {
            1..=1
        } else {
            1..=4
        };
        proptest::collection::vec(layer_strategy(kind, (w * h) as usize), layers)
            .prop_map(move |layers| DocGen { kind, w, h, layers })
    })
}

fn document(g: &DocGen) -> Document {
    let template = match g.kind {
        Type::Rgba => image_2(g.w, g.h, Mode::Rgba),
        Type::GrayAlpha => image_2(g.w, g.h, Mode::GrayAlpha),
        Type::Gray => image_2(g.w, g.h, Mode::Gray),
        Type::Indexed(t) => template_indexed(g.w, g.h, 4, Some(t)),
        Type::IndexedWithoutT => template_indexed(g.w, g.h, 4, Some(0)),
    };
    let mut doc = builds(template, &g.layers);
    if matches!(g.kind, Type::IndexedWithoutT) {
        let cmd = Command::SetTransparentIndex {
            image: doc.layers()[0].cel(),
            index: None,
        };
        assert!(doc.apply(cmd).is_ok(), "remove the transparent index");
    }
    doc
}

/// Plegado sencillo de `composite_pixel` sobre las capas visibles con celda (solo RGBA).
fn fold(doc: &Document, x: u32, y: u32) -> Rgba {
    let mut accumulated = T;
    for layer_2 in doc.layers() {
        if !layer_2.visible() {
            continue;
        }
        let Some(image_2) = doc.image(layer_2.cel()) else {
            continue;
        };
        let Pixel::Rgba(source) = image_2.get_pixel(x, y).expect("píxel dentro") else {
            panic!("se esperaba RGBA");
        };
        accumulated = composite_pixel(accumulated, source, layer_2.opacity(), layer_2.blend());
    }
    accumulated
}

// ---------------------------------------------------------------------------------------
// Propiedades.
// ---------------------------------------------------------------------------------------

proptest! {
    // 1. Recortar es componer, y la versión rápida coincide con el plegado sencillo.
    #[test]
    fn property_1_clipping_is_composing(
        g in doc_strategy(true),
        (fx, fy, fw, fh) in (0u32..64, 0u32..64, 0u32..64, 0u32..64),
    ) {
        let doc = document(&g);
        // Rectángulo válido cualquiera dentro del lienzo.
        let x = fx % g.w;
        let y = fy % g.h;
        let r = Rect { x, y, width: 1 + fw % (g.w - x), height: 1 + fh % (g.h - y) };

        let integer = compose(&doc);
        let part = compose_rect(&doc, r);
        // Si `compose` da `NoTransparency`, `compose_rect` de un rectángulo donde sí hay algo que
        // dar puede dar `Ok` (la spec decide `NoTransparency` por los píxeles del rectángulo):
        // solo se compara cuando el lienzo entero se compone.
        if let Ok(img) = &integer {
            prop_assert_eq!(part, Ok(clip(img, r)), "{:?}", r);
        }

        if let (Type::Rgba, Ok(img)) = (g.kind, &integer) {
            for yy in 0..g.h {
                for xx in 0..g.w {
                    let expected = Pixel::Rgba(fold(&doc, xx, yy));
                    prop_assert_eq!(
                        img.get_pixel(xx, yy).expect("píxel dentro"),
                        expected,
                        "pixel ({}, {}): `compose` is not the fold of `composite_pixel`", xx, yy
                    );
                }
            }
        }
    }

    // 2. Contra el modelo real independiente (RGBA, con racionales enteros).
    #[test]
    fn property_2_against_the_real_model(
        background in (any::<u8>(), any::<u8>(), any::<u8>(),
                  prop_oneof![1 => Just(0u8), 1 => Just(255u8), 3 => any::<u8>()]),
        source in (any::<u8>(), any::<u8>(), any::<u8>(), alpha_2()),
        opacity_2 in alpha_2(),
        mode in mode_any(),
    ) {
        let back = Rgba::new(background.0, background.1, background.2, background.3);
        let src = Rgba::new(source.0, source.1, source.2, source.3);
        let output = composite_pixel(back, src, opacity_2, mode);

        // Paso 1 (convención de redondeo): el modelo parte del αs entero.
        let a_s = rounds_255(i128::from(src.a) * i128::from(opacity_2));
        let ab = i128::from(back.a);
        let a_s_u8 = u8::try_from(a_s).expect("αs cabe en u8");

        // Paso 0: un origen exactamente `(0, 0, 0, 0)` es «nada» y deja el fondo.
        if src == Rgba::TRANSPARENT {
            prop_assert_eq!(output, back);
            return Ok(());
        }
        // Sin fondo: el origen y su αs; sin origen: el fondo.
        if back.a == 0 {
            prop_assert_eq!(output, Rgba::new(src.r, src.g, src.b, a_s_u8));
            return Ok(());
        }
        if a_s == 0 {
            prop_assert_eq!(output, back);
            return Ok(());
        }

        // Invariantes exactas.
        prop_assert!(i128::from(output.a) >= a_s && output.a >= back.a, "αr ≥ αs y αr ≥ αb");
        if back.a == 255 {
            prop_assert_eq!(output.a, 255, "with an opaque background, αr = 255");
        }
        if mode == BlendMode::Normal && a_s == 255 {
            prop_assert_eq!(output, Rgba::new(src.r, src.g, src.b, 255), "opaque Normal gives the source");
        }

        // αr: a ≤ 0,5 del real.
        prop_assert!(model::alpha_meets(output.a, ab, a_s), "αr = {}", output.a);

        let tol_real = model::tolerance_real(mode, ab);
        let tol_whole = model::tolerance_whole(mode, ab);
        let alpha_real = (255 * a_s + 255 * ab - a_s * ab, 255);
        for (channel, cb, cs, cr) in [
            ("R", back.r, src.r, output.r),
            ("G", back.g, src.g, output.g),
            ("B", back.b, src.b, output.b),
        ] {
            // Con el αr real: ver `tolerance_real`.
            let real = model::result(mode, cb, cs, ab, a_s, alpha_real);
            prop_assert!(
                model::meets(cr, real, tol_real),
                "{:?} {}: Cr = {} and the model (real αr) {}/{} (tolerance {:?})",
                mode, channel, cr, real.0, real.1, tol_real
            );
            // Con el αr entero: ≤ TOLERANCIA_EMPIRICA dentro del dominio de 4b (ver
            // `tolerance_whole` fuera).
            let integer = model::result(mode, cb, cs, ab, a_s, (i128::from(output.a), 1));
            prop_assert!(
                model::meets(cr, integer, tol_whole),
                "{:?} {}: Cr = {} and the model (integer αr) {}/{} (tolerance {:?})",
                mode, channel, cr, integer.0, integer.1, tol_whole
            );
        }
    }

    // 3. Monotonía y simetrías exactas.
    #[test]
    fn property_3_monotonicity_and_symmetries(
        cb in (any::<u8>(), any::<u8>(), any::<u8>()),
        cs in (any::<u8>(), any::<u8>(), any::<u8>()),
        alpha_background in 1u8..=255,
        alpha_source in any::<u8>(),
        mode in mode_any(),
    ) {
        let back = Rgba::new(cb.0, cb.1, cb.2, 255);
        let src = Rgba::new(cs.0, cs.1, cs.2, 255);
        let with_mode = |m: BlendMode| composite_pixel(back, src, 255, m);
        let (dark, light) = (with_mode(BlendMode::Darken), with_mode(BlendMode::Lighten));
        let (mult, pattern, sum) = (with_mode(BlendMode::Multiply), with_mode(BlendMode::Screen), with_mode(BlendMode::Add));

        for (k, (b, s)) in [(cb.0, cs.0), (cb.1, cs.1), (cb.2, cs.2)].into_iter().enumerate() {
            let channel = |p: Rgba| [p.r, p.g, p.b][k];
            prop_assert!(channel(dark) <= b && b <= channel(light), "Oscurecer ≤ Cb ≤ Aclarar");
            prop_assert!(channel(mult) <= b.min(s), "Multiplicar ≤ min(Cb, Cs)");
            prop_assert!(channel(pattern) >= b.max(s), "Trama ≥ max(Cb, Cs)");
            prop_assert!(channel(sum) >= b.max(s), "Sumar ≥ max(Cb, Cs)");
        }

        // Diferencia es simétrica en Cb y Cs.
        prop_assert_eq!(
            with_mode(BlendMode::Difference),
            composite_pixel(src, back, 255, BlendMode::Difference)
        );

        // Multiplicar por 255 (blanco) y trama con 0 (negro) son identidades.
        prop_assert_eq!(composite_pixel(back, WHITE, 255, BlendMode::Multiply), back);
        prop_assert_eq!(composite_pixel(back, c(0, 0, 0, 255), 255, BlendMode::Screen), back);

        // Opacidad 0 igual a capa oculta cuando hay fondo: el fondo, sin cambios.
        let with_background = Rgba::new(cb.0, cb.1, cb.2, alpha_background);
        let source = Rgba::new(cs.0, cs.1, cs.2, alpha_source);
        prop_assert_eq!(composite_pixel(with_background, source, 0, mode), with_background);
    }

    // 4. Totalidad y determinismo.
    #[test]
    fn property_4_totality_and_determinism(
        g in doc_strategy(true),
        (rx, ry, rw, rh) in (0u32..9, 0u32..9, 0u32..9, 0u32..9),
    ) {
        let doc = document(&g);
        let copy = doc.clone();
        let state = doc.state_id();
        let r = Rect { x: rx, y: ry, width: rw, height: rh };

        let integer = compose(&doc);
        let part = compose_rect(&doc, r);

        // Dos llamadas seguidas dan el mismo resultado byte a byte.
        prop_assert_eq!(&integer, &compose(&doc));
        prop_assert_eq!(&part, &compose_rect(&doc, r));

        // Ni `compose` ni `compose_rect` modifican el documento.
        prop_assert_eq!(&doc, &copy);
        prop_assert_eq!(doc.state_id(), state);

        // `compose`: `Ok` del tamaño y el modo del documento, o `NoTransparency` de su modo.
        match &integer {
            Ok(img) => {
                prop_assert_eq!((img.width(), img.height(), img.mode()), (g.w, g.h, doc.mode()));
                prop_assert_eq!(img.palette(), doc.palette());
            }
            Err(ComposeError::NoTransparency { mode }) => prop_assert_eq!(*mode, doc.mode()),
            Err(other) => prop_assert!(false, "`compose` dio {:?}", other),
        }

        // `compose_rect`: `Ok` del tamaño del rectángulo o un error de la lista, y cada error
        // solo cuando corresponde.
        let empty_2 = rw == 0 || rh == 0;
        let fits = u64::from(rx) + u64::from(rw) <= u64::from(g.w)
            && u64::from(ry) + u64::from(rh) <= u64::from(g.h);
        match &part {
            Ok(img) => {
                prop_assert!(!empty_2 && fits, "`compose_rect` gave Ok with {:?}", r);
                prop_assert_eq!((img.width(), img.height(), img.mode()), (rw, rh, doc.mode()));
            }
            Err(ComposeError::EmptyRect) => prop_assert!(empty_2),
            Err(ComposeError::RectOutside { rect, width, height }) => {
                prop_assert!(!fits);
                prop_assert_eq!((*rect, *width, *height), (r, g.w, g.h));
            }
            Err(ComposeError::NoTransparency { mode }) => {
                prop_assert!(!empty_2 && fits);
                prop_assert_eq!(*mode, doc.mode());
            }
            Err(other) => prop_assert!(false, "`compose_rect` dio {:?}", other),
        }
    }
}
