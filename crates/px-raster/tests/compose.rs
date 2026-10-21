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
//! (`i128`, sin `f64`) escrito aparte (módulo `modelo`): no repite las fórmulas en enteros de la
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
const TOLERANCIA_EMPIRICA: i128 = 2;

mod modelo {
    use super::{BlendMode, TOLERANCIA_EMPIRICA};

    /// `255 · B(Cb, Cs)` de la teoría (W3C), con la mezcla sin redondear: un entero sobre 255.
    /// Superponer usa el umbral `Cb < 128` de la spec (lo fijó el corpus).
    pub fn mezcla_por_255(modo: BlendMode, cb: i128, cs: i128) -> i128 {
        match modo {
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
            otro => panic!("el modelo no conoce el modo {otro:?}"),
        }
    }

    /// `Cr` como fracción `(num, den)` con `den > 0`:
    /// `Cr = Cb + (Cm − Cb) · αs / αr` con `Cm = (1 − m)·Cs + m·B` y
    /// `m = 1 − (1 − αb)(1 − αs·αb)` (con la fuente opaca, `1 − (1 − αb)²`; para Normal, o con
    /// `αb = 255`, `Cm` es la de la teoría: `Cs` o `B`). `ab` y `as_` son alfas de 0..=255 y
    /// `αr = ar_num / ar_den` (el entero del paso 4: `(αr, 1)`; el real: `(255·αs + 255·αb −
    /// αs·αb, 255)`).
    pub fn resultado(
        modo: BlendMode,
        cb: u8,
        cs: u8,
        ab: i128,
        as_: i128,
        (ar_num, ar_den): (i128, i128),
    ) -> (i128, i128) {
        let (cb, cs) = (i128::from(cb), i128::from(cs));
        let b255 = mezcla_por_255(modo, cb, cs);
        // m = m_num / 255³; Cm = Cs + m · (B − Cs) con B = b255 / 255.
        let m_num = 255i128.pow(3) - (255 - ab) * (65025 - as_ * ab);
        let d = 255i128.pow(4);
        let cm_num = cs * d + m_num * (b255 - 255 * cs);
        (
            cb * d * ar_num + (cm_num - cb * d) * as_ * ar_den,
            d * ar_num,
        )
    }

    /// `|entero − num/den| ≤ tol_num/tol_den`.
    pub fn cumple(entero: u8, (num, den): (i128, i128), (tol_num, tol_den): (i128, i128)) -> bool {
        (i128::from(entero) * den - num).abs() * tol_den <= tol_num * den
    }

    /// `|αr entero − αr real| ≤ 0,5` con `αr real = (255·αs + 255·αb − αs·αb) / 255`.
    pub fn alfa_cumple(entero: u8, ab: i128, as_: i128) -> bool {
        let real_por_255 = 255 * as_ + 255 * ab - as_ * ab;
        2 * (i128::from(entero) * 255 - real_por_255).abs() <= 255
    }

    /// Dominio del paso 4b (con `ab > 0` y `as_ > 0`).
    pub fn en_dominio(modo: BlendMode, ab: i128) -> bool {
        ab > 0 && ab < 255 && modo != BlendMode::Normal
    }

    /// Modos cuya función de mezcla redondea (`div255`): la mezcla ya está a ≤ 0,5 del real.
    fn mezcla_redondeada(modo: BlendMode) -> bool {
        matches!(
            modo,
            BlendMode::Multiply | BlendMode::Screen | BlendMode::Overlay
        )
    }

    /// Techo de error con el `αr` entero: la tolerancia empírica dentro del dominio del paso 4b
    /// y fuera, `1` (la interpolación trunca: < 1) más, en los tres modos que redondean la
    /// mezcla, el ≤ 0,5 de esa mezcla multiplicado por `αs / 255`: 3/2.
    // Cotas de la spec (propiedad 2): 3/2 en Multiplicar, Trama y Superponer (la mezcla redondea:
    // medido 1,44) y 1 en el resto fuera del dominio de 4b.
    pub fn tolerancia_entera(modo: BlendMode, ab: i128) -> (i128, i128) {
        if en_dominio(modo, ab) {
            (TOLERANCIA_EMPIRICA, 1)
        } else if mezcla_redondeada(modo) {
            (3, 2)
        } else {
            (1, 1)
        }
    }

    /// Techo de error con el `αr` real: con fondo opaco, el mismo que con el `αr` entero (son el
    /// mismo); con fondo translúcido, 5, también con Normal (el `αr` entero redondeado se
    /// amplifica con alfas pequeñas: máximo medido por la spec 4,57).
    // Cotas de la spec (propiedad 2): con fondo translúcido, 5 frente al `αr` real (el `αr`
    // entero se amplifica con alfas pequeñas); con `αb = 255` vale lo de `tolerancia_entera`.
    pub fn tolerancia_real(modo: BlendMode, ab: i128) -> (i128, i128) {
        if ab == 255 {
            tolerancia_entera(modo, ab)
        } else {
            (5, 1)
        }
    }
}

/// `x / 255` al entero más cercano (sin empates posibles), a partir de la definición y no del
/// truco de bits de la spec.
fn redondea_255(x: i128) -> i128 {
    (2 * x + 255).div_euclid(510)
}

// ---------------------------------------------------------------------------------------
// Colores, documentos y comprobaciones de apoyo.
// ---------------------------------------------------------------------------------------

const T: Rgba = Rgba::TRANSPARENT;

const fn c(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

/// Una capa de un documento de prueba. `celda: None` es una celda vacía (sin imagen); la capa 0
/// siempre tiene celda, así que en ella `None` deja la imagen inicial (toda a ceros).
#[derive(Clone, Debug)]
struct Capa {
    visible: bool,
    opacidad: u8,
    modo: BlendMode,
    celda: Option<Vec<Pixel>>,
}

impl Capa {
    fn nueva(celda: Vec<Pixel>) -> Capa {
        Capa {
            visible: true,
            opacidad: 255,
            modo: BlendMode::Normal,
            celda: Some(celda),
        }
    }

    fn rgba(celda: &[Rgba]) -> Capa {
        Capa::nueva(celda.iter().copied().map(Pixel::Rgba).collect())
    }

    fn vacia() -> Capa {
        Capa {
            visible: true,
            opacidad: 255,
            modo: BlendMode::Normal,
            celda: None,
        }
    }

    fn modo(mut self, modo: BlendMode) -> Capa {
        self.modo = modo;
        self
    }

    fn opacidad(mut self, opacidad: u8) -> Capa {
        self.opacidad = opacidad;
        self
    }

    fn oculta(mut self) -> Capa {
        self.visible = false;
        self
    }
}

/// Documento con `capas` de abajo arriba, a partir de la imagen `plantilla` (que es la celda de
/// la capa 0 y fija el tamaño y el modo). Solo usa los comandos del núcleo.
fn construye(plantilla: Image, capas: &[Capa]) -> Document {
    let mut doc = Document::new(plantilla);
    let (w, h) = (doc.width(), doc.height());
    for i in 1..capas.len() {
        let cmd = Command::AddLayer {
            name: format!("C{i}"),
            at: i,
        };
        assert_eq!(doc.apply(cmd), Ok(true), "añadir la capa {i}");
    }
    for (i, capa) in capas.iter().enumerate() {
        let (id, cel) = (doc.layers()[i].id(), doc.layers()[i].cel());
        if let Some(pixeles) = &capa.celda {
            assert_eq!(pixeles.len(), (w * h) as usize, "píxeles de la capa {i}");
            let pixels = pixeles
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
        if capa.opacidad != 255 {
            let cmd = Command::SetLayerOpacity {
                layer: id,
                opacity: capa.opacidad,
            };
            assert!(doc.apply(cmd).is_ok(), "opacidad de la capa {i}");
        }
        if capa.modo != BlendMode::Normal {
            let cmd = Command::SetLayerBlend {
                layer: id,
                blend: capa.modo,
            };
            assert!(doc.apply(cmd).is_ok(), "modo de la capa {i}");
        }
        if !capa.visible {
            let cmd = Command::SetLayerVisible {
                layer: id,
                visible: false,
            };
            assert!(doc.apply(cmd).is_ok(), "ocultar la capa {i}");
        }
    }
    doc
}

fn imagen(w: u32, h: u32, modo: Mode) -> Image {
    Image::new(w, h, modo).expect("imagen")
}

fn doc_rgba(w: u32, h: u32, capas: &[Capa]) -> Document {
    construye(imagen(w, h, Mode::Rgba), capas)
}

/// Paleta de `n` colores opacos distintos con el índice transparente `t`.
fn paleta(n: u8, t: Option<u8>) -> Palette {
    let mut p = Palette::from_colors(
        (0..n).map(|i| Rgba::new(i.wrapping_mul(30), 255 - i.wrapping_mul(30), 7, 255)),
    )
    .expect("paleta");
    p.set_transparent_index(t).expect("índice transparente");
    p
}

fn plantilla_indexada(w: u32, h: u32, n: u8, t: Option<u8>) -> Image {
    Image::new_indexed(w, h, paleta(n, t)).expect("imagen indexada")
}

fn idx(valores: &[u8]) -> Vec<Pixel> {
    valores.iter().copied().map(Pixel::Indexed).collect()
}

/// Los píxeles de una imagen RGBA, fila a fila.
fn rgba_de(img: &Image) -> Vec<Rgba> {
    assert_eq!(img.mode(), Mode::Rgba, "se esperaba una imagen RGBA");
    (0..img.height())
        .flat_map(|y| (0..img.width()).map(move |x| (x, y)))
        .map(|(x, y)| match img.get_pixel(x, y).expect("píxel dentro") {
            Pixel::Rgba(color) => color,
            otro => panic!("se esperaba RGBA, hay {otro:?}"),
        })
        .collect()
}

/// Los píxeles de una imagen en cualquier modo, fila a fila.
fn pixeles_de(img: &Image) -> Vec<Pixel> {
    (0..img.height())
        .flat_map(|y| (0..img.width()).map(move |x| (x, y)))
        .map(|(x, y)| img.get_pixel(x, y).expect("píxel dentro"))
        .collect()
}

/// Los índices de una imagen indexada, fila a fila.
fn indices_de(img: &Image) -> Vec<u8> {
    pixeles_de(img)
        .into_iter()
        .map(|p| match p {
            Pixel::Indexed(i) => i,
            otro => panic!("se esperaba indexado, hay {otro:?}"),
        })
        .collect()
}

/// `compose` que no debe fallar.
fn componer(doc: &Document) -> Image {
    compose(doc).expect("la composición no debe fallar")
}

/// El recorte de `img` en `r` (mismo modo y misma paleta).
fn recorte(img: &Image, r: Rect) -> Image {
    let mut salida = match img.palette() {
        Some(paleta) => Image::new_indexed(r.width, r.height, paleta.clone()),
        None => Image::new(r.width, r.height, img.mode()),
    }
    .expect("imagen del recorte");
    for y in 0..r.height {
        for x in 0..r.width {
            let pixel = img.get_pixel(r.x + x, r.y + y).expect("el recorte cabe");
            salida.set_pixel(x, y, pixel).expect("escribir el recorte");
        }
    }
    salida
}

// ---------------------------------------------------------------------------------------
// Fila 1: composite_pixel, identidades y bordes.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_01_identidades_y_bordes() {
    let fondo = c(196, 46, 52, 255);
    let origen = c(90, 60, 220, 255);

    for modo in BlendMode::ALL {
        // Origen con alfa 0 sobre fondo opaco: el fondo, sin cambios.
        assert_eq!(
            composite_pixel(fondo, c(10, 20, 30, 0), 255, modo),
            fondo,
            "αs = 0, {modo:?}"
        );
        // Opacidad 0: lo mismo.
        assert_eq!(
            composite_pixel(fondo, origen, 0, modo),
            fondo,
            "opacidad 0, {modo:?}"
        );
        // Contra el fondo vacío no hay con qué mezclar: el RGB del origen se conserva, también
        // con alfa 0.
        assert_eq!(
            composite_pixel(T, c(232, 198, 52, 0), 255, modo),
            c(232, 198, 52, 0),
            "fondo vacío, αs = 0, {modo:?}"
        );
        // Un origen exactamente `(0, 0, 0, 0)` es «nada» (como una celda vacía): no pisa el RGB
        // de un fondo con alfa 0.
        assert_eq!(
            composite_pixel(c(255, 255, 255, 0), T, 255, modo),
            c(255, 255, 255, 0),
            "origen (0, 0, 0, 0) sobre fondo con alfa 0 y RGB, {modo:?}"
        );
    }

    // Origen opaco con Normal: el origen exacto.
    assert_eq!(
        composite_pixel(fondo, origen, 255, BlendMode::Normal),
        origen
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
fn fila_02_los_ocho_modos_sobre_fondo_opaco() {
    let fondo = c(196, 46, 52, 255);
    let origen = c(90, 60, 220, 255);
    // Valores de la sección 5 de specs/corpus-hito-3.md, hechos a mano.
    let esperados = [
        (BlendMode::Normal, (90, 60, 220)),
        (BlendMode::Multiply, (69, 11, 45)),
        (BlendMode::Screen, (217, 95, 227)),
        (BlendMode::Overlay, (179, 22, 90)),
        (BlendMode::Darken, (90, 46, 52)),
        (BlendMode::Lighten, (196, 60, 220)),
        (BlendMode::Add, (255, 106, 255)),
        (BlendMode::Difference, (106, 14, 168)),
    ];
    for (modo, (r, g, b)) in esperados {
        assert_eq!(
            composite_pixel(fondo, origen, 255, modo),
            c(r, g, b, 255),
            "{modo:?}"
        );
    }

    // Superponer en el umbral: con el fondo en 127 se usa la rama de multiplicar y con 128, la de
    // trama (lo fijó el corpus). Origen 100.
    let gris = |v: u8| c(v, v, v, 255);
    assert_eq!(
        composite_pixel(gris(127), gris(100), 255, BlendMode::Overlay),
        gris(100)
    );
    assert_eq!(
        composite_pixel(gris(128), gris(100), 255, BlendMode::Overlay),
        gris(101)
    );
}

// ---------------------------------------------------------------------------------------
// Fila 3: opacidad sobre fondo opaco.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_03_opacidad_sobre_fondo_opaco() {
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
fn fila_04_a_fondo_translucido_valores() {
    // Normal sobre un fondo con alfa 150 (dato del corpus).
    let fondo = c(58, 168, 76, 150);
    assert_eq!(
        composite_pixel(fondo, c(211, 17, 99, 100), 255, BlendMode::Normal),
        c(138, 89, 88, 191)
    );
    assert_eq!(
        composite_pixel(fondo, c(211, 17, 99, 255), 255, BlendMode::Normal),
        c(211, 17, 99, 255)
    );

    // Empírica (4b). Los dos primeros son iguales al PNG de `mezcla-ocho-fondo-translucido`.
    let negro_150 = c(0, 0, 0, 150);
    assert_eq!(
        composite_pixel(negro_150, c(255, 255, 255, 255), 255, BlendMode::Multiply),
        c(43, 43, 43, 255)
    );
    assert_eq!(
        composite_pixel(negro_150, c(255, 255, 255, 100), 255, BlendMode::Multiply),
        c(42, 42, 42, 191)
    );
    assert_eq!(
        composite_pixel(negro_150, c(255, 255, 255, 255), 255, BlendMode::Difference),
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
/// son separables) a ≤ `TOLERANCIA_EMPIRICA` de la forma real observada con el `αr` entero.
fn barrido(alfas_fondo: &[u8], alfas_origen: &[u8], colores: &[u8]) {
    for modo in BlendMode::ALL {
        if modo == BlendMode::Normal {
            continue;
        }
        for &ab in alfas_fondo {
            for &a_s in alfas_origen {
                let (abi, asi) = (i128::from(ab), i128::from(a_s));
                for &cb in colores {
                    for &cs in colores {
                        let salida =
                            composite_pixel(c(cb, cb, cb, ab), c(cs, cs, cs, a_s), 255, modo);
                        assert!(
                            modelo::alfa_cumple(salida.a, abi, asi),
                            "{modo:?}: αb {ab}, αs {a_s}: el alfa de salida {} está a más de 0,5 del real",
                            salida.a
                        );
                        let esperado =
                            modelo::resultado(modo, cb, cs, abi, asi, (i128::from(salida.a), 1));
                        // Grises: los tres canales iguales (los modos son separables).
                        assert!(
                            salida.r == salida.g && salida.g == salida.b,
                            "{modo:?}: αb {ab}, αs {a_s}, Cb {cb}, Cs {cs}: los canales de un gris \
                             difieren: {salida:?}"
                        );
                        assert!(
                            modelo::cumple(salida.r, esperado, (TOLERANCIA_EMPIRICA, 1)),
                            "{modo:?}: αb {ab}, αs {a_s}, Cb {cb}, Cs {cs}: el canal da {} y el \
                             modelo {}/{} (tolerancia {TOLERANCIA_EMPIRICA})",
                            salida.r,
                            esperado.0,
                            esperado.1
                        );
                    }
                }
            }
        }
    }
}

/// Rejilla de colores por lado: 0, 17, …, 255 (16 valores) y 1, 2, 127, 128, 129, 254.
// La lista de colores de la spec (0, 17, …, 255 más 1, 2, 127, 128, 129, 254) tiene 22 valores: 484 parejas.
fn rejilla_de_colores() -> Vec<u8> {
    let mut colores: Vec<u8> = (0..=255u8).step_by(17).collect();
    colores.extend([1, 2, 127, 128, 129, 254]);
    colores
}

#[test]
fn fila_04_b_barrido_rapido_de_alfas() {
    // αb en 1..=254 con paso 5 más 127; αs en 1..=255 con paso 5 más 127 y 255.
    let mut alfas_fondo: Vec<u8> = (1..=254u8).step_by(5).collect();
    alfas_fondo.push(127);
    let mut alfas_origen: Vec<u8> = (1..=255u8).step_by(5).collect();
    alfas_origen.extend([127, 255]);
    barrido(&alfas_fondo, &alfas_origen, &rejilla_de_colores());
}

#[test]
#[ignore = "barrido completo: cargo test --release -p px-raster --test compose -- --ignored"]
fn fila_04_c_barrido_completo_de_alfas() {
    // Todas las combinaciones: 254 alfas de fondo por 255 de origen.
    let alfas_fondo: Vec<u8> = (1..=254).collect();
    let alfas_origen: Vec<u8> = (1..=255).collect();
    barrido(&alfas_fondo, &alfas_origen, &rejilla_de_colores());
}

// ---------------------------------------------------------------------------------------
// Fila 5: pila de capas (RGBA).
// ---------------------------------------------------------------------------------------

const BLANCO: Rgba = c(255, 255, 255, 255);
const GRIS_128: Rgba = c(128, 128, 128, 255);

/// `D(3)` de la fila 5, de 2 × 1: el fondo opaco `A = (200, 100, 50)`; encima una capa Normal
/// con `(10, 20, 30)` solo en el primer píxel; y encima una capa Multiplicar con blanco en el
/// primero y gris 128 en el segundo.
fn pila_a_mano() -> Vec<Capa> {
    let a = c(200, 100, 50, 255);
    vec![
        Capa::rgba(&[a, a]),
        Capa::rgba(&[c(10, 20, 30, 255), T]),
        Capa::rgba(&[BLANCO, GRIS_128]).modo(BlendMode::Multiply),
    ]
}

/// El compuesto a mano de `pila_a_mano`: el primer píxel es `(10, 20, 30)` (multiplicar por
/// blanco no cambia nada) y el segundo, `A` multiplicado por 128: div255(200·128) = 100,
/// div255(100·128) = 50 y div255(50·128) = 25.
const COMPUESTO_A_MANO: [Rgba; 2] = [c(10, 20, 30, 255), c(100, 50, 25, 255)];

#[test]
fn fila_05_pila_de_capas() {
    // El compuesto a mano.
    assert_eq!(
        rgba_de(&componer(&doc_rgba(2, 1, &pila_a_mano()))),
        COMPUESTO_A_MANO
    );

    // Orden de abajo arriba: el mismo par de capas opacas en un orden y en el otro.
    let (a, b) = (c(200, 100, 50, 255), c(10, 20, 30, 255));
    let abajo_a = doc_rgba(2, 1, &[Capa::rgba(&[a, a]), Capa::rgba(&[b, b])]);
    let abajo_b = doc_rgba(2, 1, &[Capa::rgba(&[b, b]), Capa::rgba(&[a, a])]);
    assert_eq!(rgba_de(&componer(&abajo_a)), [b, b]);
    assert_eq!(rgba_de(&componer(&abajo_b)), [a, a]);

    // Una capa oculta no cuenta, aunque cubra todo y sea opaca.
    let mut con_oculta = pila_a_mano();
    con_oculta.push(Capa::rgba(&[c(1, 2, 3, 255), c(1, 2, 3, 255)]).oculta());
    assert_eq!(
        rgba_de(&componer(&doc_rgba(2, 1, &con_oculta))),
        COMPUESTO_A_MANO
    );

    // Una capa con la celda vacía no aporta nada (ni siquiera con modo y opacidad raros).
    let mut con_vacia = pila_a_mano();
    con_vacia.push(Capa::vacia().modo(BlendMode::Multiply).opacidad(100));
    assert_eq!(
        rgba_de(&componer(&doc_rgba(2, 1, &con_vacia))),
        COMPUESTO_A_MANO
    );

    // Ni siquiera el RGB: contra el fondo vacío, una celda con RGB bajo alfa 0 lo conserva y una
    // celda vacía, no.
    let vacio = [T, T];
    let con_rgb = doc_rgba(
        2,
        1,
        &[Capa::rgba(&vacio), Capa::rgba(&[c(232, 198, 52, 0), T])],
    );
    assert_eq!(rgba_de(&componer(&con_rgb)), [c(232, 198, 52, 0), T]);
    let sin_celda = doc_rgba(2, 1, &[Capa::rgba(&vacio), Capa::vacia()]);
    assert_eq!(rgba_de(&componer(&sin_celda)), [T, T]);

    // Opacidad 0: igual que oculta sobre un fondo opaco...
    let base = Capa::rgba(&[a, a]);
    let tapa = [b, b];
    let op0 = doc_rgba(2, 1, &[base.clone(), Capa::rgba(&tapa).opacidad(0)]);
    let oculta = doc_rgba(2, 1, &[base, Capa::rgba(&tapa).oculta()]);
    assert_eq!(rgba_de(&componer(&op0)), [a, a]);
    assert_eq!(componer(&op0), componer(&oculta));
    // ...y salvo el RGB si el fondo es vacío (αs = 0 contra el vacío conserva el origen).
    let origen = [c(232, 198, 52, 200), T];
    let op0_vacio = doc_rgba(2, 1, &[Capa::rgba(&vacio), Capa::rgba(&origen).opacidad(0)]);
    let oculta_vacio = doc_rgba(2, 1, &[Capa::rgba(&vacio), Capa::rgba(&origen).oculta()]);
    assert_eq!(rgba_de(&componer(&op0_vacio)), [c(232, 198, 52, 0), T]);
    assert_eq!(rgba_de(&componer(&oculta_vacio)), [T, T]);

    // Capa de fondo con opacidad 128 y modo Multiplicar: el modo no tiene efecto y el alfa se
    // escala (αs = 128 y div255(100 · 128) = 50).
    let fondo = doc_rgba(
        2,
        1,
        &[Capa::rgba(&[a, c(10, 20, 30, 100)])
            .modo(BlendMode::Multiply)
            .opacidad(128)],
    );
    assert_eq!(
        rgba_de(&componer(&fondo)),
        [c(200, 100, 50, 128), c(10, 20, 30, 50)]
    );

    // Todas ocultas: transparente en todo el lienzo, y del tamaño del documento.
    let todas: Vec<Capa> = pila_a_mano().into_iter().map(Capa::oculta).collect();
    let salida = componer(&doc_rgba(2, 1, &todas));
    assert_eq!(
        (salida.width(), salida.height(), salida.mode()),
        (2, 1, Mode::Rgba)
    );
    assert_eq!(rgba_de(&salida), [T, T]);
}

// ---------------------------------------------------------------------------------------
// Fila 6: grises con alfa y grises sin alfa.
// ---------------------------------------------------------------------------------------

/// Una pila de 3 × 2 en grises con alfa `(gris, alfa)`, con capa oculta, vacía, opacidad,
/// modos, alfas intermedios y RGB bajo alfa 0. `pixel` da el píxel del modo que se prueba.
fn pila_de_grises(pixel: impl Fn(u8, u8) -> Pixel) -> Vec<Capa> {
    let celda = |valores: [(u8, u8); 6]| -> Vec<Pixel> {
        valores.iter().map(|&(g, a)| pixel(g, a)).collect()
    };
    vec![
        Capa::nueva(celda([
            (200, 255),
            (100, 255),
            (50, 128),
            (0, 0),
            (255, 60),
            (30, 0),
        ])),
        Capa::nueva(celda([
            (128, 255),
            (255, 100),
            (10, 255),
            (90, 200),
            (0, 0),
            (77, 0),
        ]))
        .modo(BlendMode::Multiply)
        .opacidad(200),
        Capa::nueva(celda([(1, 255); 6])).oculta(),
        Capa::nueva(celda([
            (100, 150),
            (0, 0),
            (255, 255),
            (40, 40),
            (200, 200),
            (60, 60),
        ]))
        .modo(BlendMode::Screen)
        .opacidad(128),
        Capa::vacia().modo(BlendMode::Overlay),
        Capa::nueva(celda([(9, 99); 6]))
            .modo(BlendMode::Difference)
            .opacidad(0),
    ]
}

#[test]
fn fila_06_grises_con_y_sin_alfa() {
    // GrayAlpha: el canal rojo del RGBA equivalente (R = G = B) y su alfa.
    let rgba = doc_rgba_de_grises();
    let grises = construye(
        imagen(3, 2, Mode::GrayAlpha),
        &pila_de_grises(|gray, alpha| Pixel::GrayAlpha { gray, alpha }),
    );
    let esperado = rgba_de(&componer(&rgba));
    let salida = componer(&grises);
    assert_eq!(salida.mode(), Mode::GrayAlpha);
    assert_eq!((salida.width(), salida.height()), (3, 2));
    let obtenido = pixeles_de(&salida);
    assert_eq!(obtenido.len(), esperado.len());
    for (k, (pixel, rgb)) in obtenido.iter().zip(&esperado).enumerate() {
        assert_eq!(
            *pixel,
            Pixel::GrayAlpha {
                gray: rgb.r,
                alpha: rgb.a
            },
            "píxel {k}: el RGBA equivalente es {rgb:?}"
        );
        assert!(
            rgb.r == rgb.g && rgb.g == rgb.b,
            "el RGBA equivalente es gris"
        );
    }

    // GrayAlpha con todas las capas ocultas: transparente (gris 0, alfa 0) en todo el lienzo.
    let todas: Vec<Capa> = pila_de_grises(|gray, alpha| Pixel::GrayAlpha { gray, alpha })
        .into_iter()
        .map(Capa::oculta)
        .collect();
    let ocultas = componer(&construye(imagen(3, 2, Mode::GrayAlpha), &todas));
    assert!(
        pixeles_de(&ocultas)
            .iter()
            .all(|p| *p == Pixel::GrayAlpha { gray: 0, alpha: 0 })
    );

    // Gray (sin alfa) con una capa: la celda, tal cual.
    let mut celda = imagen(3, 2, Mode::Gray);
    for (k, valor) in [0u8, 10, 20, 30, 250, 255].into_iter().enumerate() {
        celda
            .set_pixel(k as u32 % 3, k as u32 / 3, Pixel::Gray(valor))
            .expect("píxel");
    }
    let mut gris = Document::new(celda.clone());
    assert_eq!(compose(&gris), Ok(celda));

    // Gray con la única capa oculta: no queda nada que dar y no hay valor transparente.
    let capa = gris.layers()[0].id();
    let cmd = Command::SetLayerVisible {
        layer: capa,
        visible: false,
    };
    assert_eq!(gris.apply(cmd), Ok(true));
    assert_eq!(
        compose(&gris),
        Err(ComposeError::NoTransparency { mode: Mode::Gray })
    );
}

/// La pila de `pila_de_grises` en RGBA con `R = G = B`.
fn doc_rgba_de_grises() -> Document {
    doc_rgba(
        3,
        2,
        &pila_de_grises(|g, a| Pixel::Rgba(Rgba::new(g, g, g, a))),
    )
}

// ---------------------------------------------------------------------------------------
// Fila 7: indexado.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_07_indexado_gana_la_capa_visible_mas_alta_no_transparente() {
    // Tres capas, transparente 0, lienzo de 4 × 1.
    let base = || plantilla_indexada(4, 1, 8, Some(0));
    let l0 = || Capa::nueva(idx(&[1, 2, 3, 0]));
    let l1 = || Capa::nueva(idx(&[0, 4, 0, 0]));
    let l2 = || Capa::nueva(idx(&[5, 0, 0, 0]));

    let tres = construye(base(), &[l0(), l1(), l2()]);
    let salida = componer(&tres);
    assert_eq!(indices_de(&salida), [5, 4, 3, 0]);
    assert_eq!(salida.mode(), Mode::Indexed);
    assert_eq!(
        salida.palette(),
        tres.palette(),
        "la paleta es la del lienzo"
    );
    assert_eq!((salida.width(), salida.height()), (4, 1));

    // Capa oculta: no cuenta.
    let oculta = construye(base(), &[l0(), l1(), l2().oculta()]);
    assert_eq!(indices_de(&componer(&oculta)), [1, 4, 3, 0]);

    // Celda vacía: no aporta nada (encima y en medio).
    let vacia_arriba = construye(base(), &[l0(), l1(), l2(), Capa::vacia()]);
    assert_eq!(indices_de(&componer(&vacia_arriba)), [5, 4, 3, 0]);
    let vacia_en_medio = construye(base(), &[l0(), Capa::vacia(), l2().oculta()]);
    assert_eq!(indices_de(&componer(&vacia_en_medio)), [1, 2, 3, 0]);

    // Todo transparente: el índice transparente.
    let nada = construye(base(), &[Capa::nueva(idx(&[0, 0, 0, 0])), Capa::vacia()]);
    assert_eq!(indices_de(&componer(&nada)), [0, 0, 0, 0]);

    // Con el transparente 7, el índice 0 sí es opaco y tapa a lo de debajo.
    let base7 = || plantilla_indexada(4, 1, 8, Some(7));
    let con7 = construye(
        base7(),
        &[
            Capa::nueva(idx(&[3, 3, 3, 3])),
            Capa::nueva(idx(&[7, 2, 0, 7])),
        ],
    );
    let salida7 = componer(&con7);
    assert_eq!(indices_de(&salida7), [3, 2, 0, 3]);
    assert_eq!(salida7.palette(), con7.palette());

    // Y donde no hay nada, el 7.
    let nada7 = construye(base7(), &[Capa::nueva(idx(&[7, 7, 7, 7]))]);
    assert_eq!(indices_de(&componer(&nada7)), [7, 7, 7, 7]);
}

#[test]
fn fila_07_indexado_sin_indice_transparente() {
    // Una sola capa llena: la celda tal cual.
    let llena = Document::new({
        let mut img = plantilla_indexada(4, 1, 8, None);
        for (x, i) in [1u8, 2, 3, 0].into_iter().enumerate() {
            img.set_pixel(x as u32, 0, Pixel::Indexed(i))
                .expect("píxel");
        }
        img
    });
    let celda = llena.image(llena.layers()[0].cel()).expect("celda").clone();
    assert_eq!(compose(&llena), Ok(celda));

    // Con una celda vacía hace falta una segunda capa: se añade con un transparente y luego se
    // quita (`SetTransparentIndex`).
    let mut doc = construye(
        plantilla_indexada(4, 1, 8, Some(0)),
        &[Capa::nueva(idx(&[1, 2, 3, 0])), Capa::vacia()],
    );
    let cel = doc.layers()[0].cel();
    let quita = Command::SetTransparentIndex {
        image: cel,
        index: None,
    };
    assert_eq!(doc.apply(quita), Ok(true));
    assert_eq!(doc.palette().and_then(Palette::transparent_index), None);

    // La capa de abajo llena y visible: gana, también donde vale 0 (sin transparente, el 0 es
    // un color como los demás).
    assert_eq!(indices_de(&componer(&doc)), [1, 2, 3, 0]);

    // La de abajo oculta y la de arriba vacía: no queda ningún píxel que dar.
    let oculta = Command::SetLayerVisible {
        layer: doc.layers()[0].id(),
        visible: false,
    };
    assert_eq!(doc.apply(oculta), Ok(true));
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
    let fondo: Vec<Rgba> = (0..30u32)
        .map(|k| {
            let (x, y) = ((k % 6) as u8, (k / 6) as u8);
            c(10 + x * 40, 5 + y * 50, 20 * (x + y), 255)
        })
        .collect();
    let arriba: Vec<Rgba> = (0..30u32)
        .map(|k| {
            let (x, y) = ((k % 6) as u8, (k / 6) as u8);
            c(200 - 20 * x, 100 + 10 * y, 30 + 15 * x, 128 + 20 * y)
        })
        .collect();
    doc_rgba(
        6,
        5,
        &[
            Capa::rgba(&fondo),
            Capa::rgba(&arriba).modo(BlendMode::Screen).opacidad(200),
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
fn fila_08_compose_rect() {
    let doc = doc_6x5();
    let entero = componer(&doc);
    assert_eq!((entero.width(), entero.height()), (6, 5));

    // Interior, borde, una fila y todo el lienzo: el tamaño del rectángulo y el recorte exacto.
    for r in [
        rect(1, 1, 3, 2),
        rect(3, 0, 3, 5),
        rect(0, 4, 6, 1),
        rect(2, 0, 1, 1),
        rect(0, 0, 6, 5),
    ] {
        let parte = compose_rect(&doc, r).unwrap_or_else(|e| panic!("{r:?}: {e}"));
        assert_eq!(
            (parte.width(), parte.height()),
            (r.width, r.height),
            "{r:?}"
        );
        assert_eq!(parte, recorte(&entero, r), "{r:?}");
        // La memoria reservada es la del rectángulo.
        assert_eq!(
            Some(parte.bytes().len()),
            Image::byte_len(r.width, r.height, Mode::Rgba).ok(),
            "{r:?}"
        );
    }
    assert_eq!(compose_rect(&doc, rect(0, 0, 6, 5)), Ok(entero));

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
    let copia = r;
    assert_eq!(r, copia);
    assert!(!format!("{r:?}").is_empty());
    assert!(!ComposeError::EmptyRect.to_string().is_empty());
}

// ---------------------------------------------------------------------------------------
// Generadores de documentos para las propiedades.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Tipo {
    Rgba,
    GrisAlfa,
    /// Indexado con ese índice transparente (paleta de 4 colores).
    Indexado(u8),
    /// Indexado sin índice transparente (se quita después de añadir las capas).
    IndexadoSinT,
    Gris,
}

#[derive(Clone, Debug)]
struct DocGen {
    tipo: Tipo,
    w: u32,
    h: u32,
    capas: Vec<Capa>,
}

/// Alfa o opacidad con los extremos más frecuentes que en el uniforme.
fn alfa() -> impl Strategy<Value = u8> {
    prop_oneof![1 => Just(0u8), 1 => Just(255u8), 2 => any::<u8>()]
}

fn modo_cualquiera() -> impl Strategy<Value = BlendMode> {
    proptest::sample::select(BlendMode::ALL.to_vec())
}

fn tipo_strategy(completo: bool) -> BoxedStrategy<Tipo> {
    let basico = prop_oneof![
        Just(Tipo::Rgba),
        Just(Tipo::GrisAlfa),
        (0u8..4).prop_map(Tipo::Indexado),
    ];
    if completo {
        prop_oneof![
            3 => basico,
            1 => Just(Tipo::IndexadoSinT),
            1 => Just(Tipo::Gris),
        ]
        .boxed()
    } else {
        basico.boxed()
    }
}

fn pixel_strategy(tipo: Tipo) -> BoxedStrategy<Pixel> {
    match tipo {
        Tipo::Rgba => (any::<u8>(), any::<u8>(), any::<u8>(), alfa())
            .prop_map(|(r, g, b, a)| Pixel::Rgba(Rgba::new(r, g, b, a)))
            .boxed(),
        Tipo::GrisAlfa => (any::<u8>(), alfa())
            .prop_map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
            .boxed(),
        Tipo::Indexado(_) | Tipo::IndexadoSinT => (0u8..4).prop_map(Pixel::Indexed).boxed(),
        Tipo::Gris => any::<u8>().prop_map(Pixel::Gray).boxed(),
    }
}

fn capa_strategy(tipo: Tipo, pixeles: usize) -> impl Strategy<Value = Capa> {
    let indexado = matches!(tipo, Tipo::Indexado(_) | Tipo::IndexadoSinT);
    (
        proptest::bool::weighted(0.85),
        alfa(),
        modo_cualquiera(),
        proptest::option::weighted(
            0.8,
            proptest::collection::vec(pixel_strategy(tipo), pixeles),
        ),
    )
        .prop_map(move |(visible, opacidad, modo, celda)| {
            // En indexado el núcleo solo admite opacidad 255 y modo Normal.
            let (opacidad, modo) = if indexado {
                (255, BlendMode::Normal)
            } else {
                (opacidad, modo)
            };
            Capa {
                visible,
                opacidad,
                modo,
                celda,
            }
        })
}

/// Documentos de hasta 6 × 5 con 1 a 4 capas (una sola en grises sin alfa). Con `completo`
/// entran también los grises sin alfa y el indexado sin transparente.
fn doc_strategy(completo: bool) -> impl Strategy<Value = DocGen> {
    (tipo_strategy(completo), 1u32..=6, 1u32..=5).prop_flat_map(|(tipo, w, h)| {
        let capas = if matches!(tipo, Tipo::Gris) {
            1..=1
        } else {
            1..=4
        };
        proptest::collection::vec(capa_strategy(tipo, (w * h) as usize), capas)
            .prop_map(move |capas| DocGen { tipo, w, h, capas })
    })
}

fn documento(g: &DocGen) -> Document {
    let plantilla = match g.tipo {
        Tipo::Rgba => imagen(g.w, g.h, Mode::Rgba),
        Tipo::GrisAlfa => imagen(g.w, g.h, Mode::GrayAlpha),
        Tipo::Gris => imagen(g.w, g.h, Mode::Gray),
        Tipo::Indexado(t) => plantilla_indexada(g.w, g.h, 4, Some(t)),
        Tipo::IndexadoSinT => plantilla_indexada(g.w, g.h, 4, Some(0)),
    };
    let mut doc = construye(plantilla, &g.capas);
    if matches!(g.tipo, Tipo::IndexadoSinT) {
        let cmd = Command::SetTransparentIndex {
            image: doc.layers()[0].cel(),
            index: None,
        };
        assert!(doc.apply(cmd).is_ok(), "quitar el índice transparente");
    }
    doc
}

/// Plegado sencillo de `composite_pixel` sobre las capas visibles con celda (solo RGBA).
fn pliegue(doc: &Document, x: u32, y: u32) -> Rgba {
    let mut acumulado = T;
    for capa in doc.layers() {
        if !capa.visible() {
            continue;
        }
        let Some(imagen) = doc.image(capa.cel()) else {
            continue;
        };
        let Pixel::Rgba(origen) = imagen.get_pixel(x, y).expect("píxel dentro") else {
            panic!("se esperaba RGBA");
        };
        acumulado = composite_pixel(acumulado, origen, capa.opacity(), capa.blend());
    }
    acumulado
}

// ---------------------------------------------------------------------------------------
// Propiedades.
// ---------------------------------------------------------------------------------------

proptest! {
    // 1. Recortar es componer, y la versión rápida coincide con el plegado sencillo.
    #[test]
    fn propiedad_1_recortar_es_componer(
        g in doc_strategy(true),
        (fx, fy, fw, fh) in (0u32..64, 0u32..64, 0u32..64, 0u32..64),
    ) {
        let doc = documento(&g);
        // Rectángulo válido cualquiera dentro del lienzo.
        let x = fx % g.w;
        let y = fy % g.h;
        let r = Rect { x, y, width: 1 + fw % (g.w - x), height: 1 + fh % (g.h - y) };

        let entero = compose(&doc);
        let parte = compose_rect(&doc, r);
        // Si `compose` da `NoTransparency`, `compose_rect` de un rectángulo donde sí hay algo que
        // dar puede dar `Ok` (la spec decide `NoTransparency` por los píxeles del rectángulo):
        // solo se compara cuando el lienzo entero se compone.
        if let Ok(img) = &entero {
            prop_assert_eq!(parte, Ok(recorte(img, r)), "{:?}", r);
        }

        if let (Tipo::Rgba, Ok(img)) = (g.tipo, &entero) {
            for yy in 0..g.h {
                for xx in 0..g.w {
                    let esperado = Pixel::Rgba(pliegue(&doc, xx, yy));
                    prop_assert_eq!(
                        img.get_pixel(xx, yy).expect("píxel dentro"),
                        esperado,
                        "píxel ({}, {}): `compose` no es el plegado de `composite_pixel`", xx, yy
                    );
                }
            }
        }
    }

    // 2. Contra el modelo real independiente (RGBA, con racionales enteros).
    #[test]
    fn propiedad_2_contra_el_modelo_real(
        fondo in (any::<u8>(), any::<u8>(), any::<u8>(),
                  prop_oneof![1 => Just(0u8), 1 => Just(255u8), 3 => any::<u8>()]),
        origen in (any::<u8>(), any::<u8>(), any::<u8>(), alfa()),
        opacidad in alfa(),
        modo in modo_cualquiera(),
    ) {
        let back = Rgba::new(fondo.0, fondo.1, fondo.2, fondo.3);
        let src = Rgba::new(origen.0, origen.1, origen.2, origen.3);
        let salida = composite_pixel(back, src, opacidad, modo);

        // Paso 1 (convención de redondeo): el modelo parte del αs entero.
        let a_s = redondea_255(i128::from(src.a) * i128::from(opacidad));
        let ab = i128::from(back.a);
        let a_s_u8 = u8::try_from(a_s).expect("αs cabe en u8");

        // Paso 0: un origen exactamente `(0, 0, 0, 0)` es «nada» y deja el fondo.
        if src == Rgba::TRANSPARENT {
            prop_assert_eq!(salida, back);
            return Ok(());
        }
        // Sin fondo: el origen y su αs; sin origen: el fondo.
        if back.a == 0 {
            prop_assert_eq!(salida, Rgba::new(src.r, src.g, src.b, a_s_u8));
            return Ok(());
        }
        if a_s == 0 {
            prop_assert_eq!(salida, back);
            return Ok(());
        }

        // Invariantes exactas.
        prop_assert!(i128::from(salida.a) >= a_s && salida.a >= back.a, "αr ≥ αs y αr ≥ αb");
        if back.a == 255 {
            prop_assert_eq!(salida.a, 255, "con fondo opaco, αr = 255");
        }
        if modo == BlendMode::Normal && a_s == 255 {
            prop_assert_eq!(salida, Rgba::new(src.r, src.g, src.b, 255), "Normal opaco da el origen");
        }

        // αr: a ≤ 0,5 del real.
        prop_assert!(modelo::alfa_cumple(salida.a, ab, a_s), "αr = {}", salida.a);

        let tol_real = modelo::tolerancia_real(modo, ab);
        let tol_entera = modelo::tolerancia_entera(modo, ab);
        let alfa_real = (255 * a_s + 255 * ab - a_s * ab, 255);
        for (canal, cb, cs, cr) in [
            ("R", back.r, src.r, salida.r),
            ("G", back.g, src.g, salida.g),
            ("B", back.b, src.b, salida.b),
        ] {
            // Con el αr real: ver `tolerancia_real`.
            let real = modelo::resultado(modo, cb, cs, ab, a_s, alfa_real);
            prop_assert!(
                modelo::cumple(cr, real, tol_real),
                "{:?} {}: Cr = {} y el modelo (αr real) {}/{} (tolerancia {:?})",
                modo, canal, cr, real.0, real.1, tol_real
            );
            // Con el αr entero: ≤ TOLERANCIA_EMPIRICA dentro del dominio de 4b (ver
            // `tolerancia_entera` fuera).
            let entero = modelo::resultado(modo, cb, cs, ab, a_s, (i128::from(salida.a), 1));
            prop_assert!(
                modelo::cumple(cr, entero, tol_entera),
                "{:?} {}: Cr = {} y el modelo (αr entero) {}/{} (tolerancia {:?})",
                modo, canal, cr, entero.0, entero.1, tol_entera
            );
        }
    }

    // 3. Monotonía y simetrías exactas.
    #[test]
    fn propiedad_3_monotonia_y_simetrias(
        cb in (any::<u8>(), any::<u8>(), any::<u8>()),
        cs in (any::<u8>(), any::<u8>(), any::<u8>()),
        alfa_fondo in 1u8..=255,
        alfa_origen in any::<u8>(),
        modo in modo_cualquiera(),
    ) {
        let back = Rgba::new(cb.0, cb.1, cb.2, 255);
        let src = Rgba::new(cs.0, cs.1, cs.2, 255);
        let en = |m: BlendMode| composite_pixel(back, src, 255, m);
        let (oscuro, claro) = (en(BlendMode::Darken), en(BlendMode::Lighten));
        let (mult, trama, suma) = (en(BlendMode::Multiply), en(BlendMode::Screen), en(BlendMode::Add));

        for (k, (b, s)) in [(cb.0, cs.0), (cb.1, cs.1), (cb.2, cs.2)].into_iter().enumerate() {
            let canal = |p: Rgba| [p.r, p.g, p.b][k];
            prop_assert!(canal(oscuro) <= b && b <= canal(claro), "Oscurecer ≤ Cb ≤ Aclarar");
            prop_assert!(canal(mult) <= b.min(s), "Multiplicar ≤ min(Cb, Cs)");
            prop_assert!(canal(trama) >= b.max(s), "Trama ≥ max(Cb, Cs)");
            prop_assert!(canal(suma) >= b.max(s), "Sumar ≥ max(Cb, Cs)");
        }

        // Diferencia es simétrica en Cb y Cs.
        prop_assert_eq!(
            en(BlendMode::Difference),
            composite_pixel(src, back, 255, BlendMode::Difference)
        );

        // Multiplicar por 255 (blanco) y trama con 0 (negro) son identidades.
        prop_assert_eq!(composite_pixel(back, BLANCO, 255, BlendMode::Multiply), back);
        prop_assert_eq!(composite_pixel(back, c(0, 0, 0, 255), 255, BlendMode::Screen), back);

        // Opacidad 0 igual a capa oculta cuando hay fondo: el fondo, sin cambios.
        let con_fondo = Rgba::new(cb.0, cb.1, cb.2, alfa_fondo);
        let origen = Rgba::new(cs.0, cs.1, cs.2, alfa_origen);
        prop_assert_eq!(composite_pixel(con_fondo, origen, 0, modo), con_fondo);
    }

    // 4. Totalidad y determinismo.
    #[test]
    fn propiedad_4_totalidad_y_determinismo(
        g in doc_strategy(true),
        (rx, ry, rw, rh) in (0u32..9, 0u32..9, 0u32..9, 0u32..9),
    ) {
        let doc = documento(&g);
        let copia = doc.clone();
        let estado = doc.state_id();
        let r = Rect { x: rx, y: ry, width: rw, height: rh };

        let entero = compose(&doc);
        let parte = compose_rect(&doc, r);

        // Dos llamadas seguidas dan el mismo resultado byte a byte.
        prop_assert_eq!(&entero, &compose(&doc));
        prop_assert_eq!(&parte, &compose_rect(&doc, r));

        // Ni `compose` ni `compose_rect` modifican el documento.
        prop_assert_eq!(&doc, &copia);
        prop_assert_eq!(doc.state_id(), estado);

        // `compose`: `Ok` del tamaño y el modo del documento, o `NoTransparency` de su modo.
        match &entero {
            Ok(img) => {
                prop_assert_eq!((img.width(), img.height(), img.mode()), (g.w, g.h, doc.mode()));
                prop_assert_eq!(img.palette(), doc.palette());
            }
            Err(ComposeError::NoTransparency { mode }) => prop_assert_eq!(*mode, doc.mode()),
            Err(otro) => prop_assert!(false, "`compose` dio {:?}", otro),
        }

        // `compose_rect`: `Ok` del tamaño del rectángulo o un error de la lista, y cada error
        // solo cuando corresponde.
        let vacio = rw == 0 || rh == 0;
        let cabe = u64::from(rx) + u64::from(rw) <= u64::from(g.w)
            && u64::from(ry) + u64::from(rh) <= u64::from(g.h);
        match &parte {
            Ok(img) => {
                prop_assert!(!vacio && cabe, "`compose_rect` dio Ok con {:?}", r);
                prop_assert_eq!((img.width(), img.height(), img.mode()), (rw, rh, doc.mode()));
            }
            Err(ComposeError::EmptyRect) => prop_assert!(vacio),
            Err(ComposeError::RectOutside { rect, width, height }) => {
                prop_assert!(!cabe);
                prop_assert_eq!((*rect, *width, *height), (r, g.w, g.h));
            }
            Err(ComposeError::NoTransparency { mode }) => {
                prop_assert!(!vacio && cabe);
                prop_assert_eq!(*mode, doc.mode());
            }
            Err(otro) => prop_assert!(false, "`compose_rect` dio {:?}", otro),
        }
    }
}
