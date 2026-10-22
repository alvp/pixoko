//! Composición contra el corpus `golden/aseprite/` (specs/hito-3-mezcla.md, unidad 3.2, filas 9,
//! 10 y 11 de «Casos»; specs/corpus-hito-3.md para los casos).
//!
//! Cada caso se reconstruye aquí como un `Document` de `px-core` a partir de las tablas `A` y
//! `B` y de las reglas de `tools/corpus-h3/generar.lua` (rectángulos, rejillas, tiras y el disco
//! de `fondo-con-opacidad`, por desigualdad entera), y `px_raster::compose` se compara con el PNG
//! aplanado `golden/aseprite/<caso>.png` leído con `px_formats::read_png`. El formato
//! `.aseprite` no se lee (eso es la 3.8): por eso solo entran los casos que se reconstruyen sin
//! formas de celda; los demás esperan a la 3.9.
//!
//! - Fila 9: las ocho matrices `mezcla-*` (2 capas, 32 × 32), **tolerancia 0**.
//! - Fila 10: `mezcla-ocho-opacidad`, `mezcla-ocho-alfa`, `capas-opacidad`, `capas-alfa-origen` y
//!   `fondo-con-opacidad`, **tolerancia 0**.
//! - Fila 11: `capas-alfa-fondo`, `mezcla-ocho-fondo-translucido` y las dos rejillas de alfas.
//!   Fuera del dominio de la fórmula empírica del paso 4b (`0 < αb < 255` del fondo ya compuesto,
//!   `αs > 0` y modo distinto de Normal), tolerancia 0; dentro, techo `TOLERANCIA_EMPIRICA` y
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
const TOLERANCIA_EMPIRICA: i32 = 2;

// ---------------------------------------------------------------------------------------
// Los colores y las tablas de `generar.lua`.
// ---------------------------------------------------------------------------------------

const ROJO: [u8; 3] = [196, 46, 52];
const VERDE: [u8; 3] = [58, 168, 76];
const CIAN: [u8; 3] = [66, 196, 206];
const MAGENTA: [u8; 3] = [186, 64, 170];
const NARANJA: [u8; 3] = [236, 132, 44];
const BLANCO: [u8; 3] = [255, 255, 255];
const NEGRO: [u8; 3] = [0, 0, 0];
const GRIS: [u8; 3] = [128, 128, 128];
const AZUL: [u8; 3] = [44, 92, 200];

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

/// Los ocho modos, en el orden de `generar.lua` (el de las tiras).
const MODOS: [(&str, BlendMode); 8] = [
    ("normal", BlendMode::Normal),
    ("multiplicar", BlendMode::Multiply),
    ("trama", BlendMode::Screen),
    ("superponer", BlendMode::Overlay),
    ("oscurecer", BlendMode::Darken),
    ("aclarar", BlendMode::Lighten),
    ("sumar", BlendMode::Add),
    ("diferencia", BlendMode::Difference),
];

fn color(rgb: [u8; 3], alfa: u8) -> Rgba {
    Rgba::new(rgb[0], rgb[1], rgb[2], alfa)
}

// ---------------------------------------------------------------------------------------
// Lienzos de píxeles (las formas de `generar.lua`) y casos.
// ---------------------------------------------------------------------------------------

/// La imagen de una celda: `w × h` píxeles RGBA, fila a fila, transparentes al principio.
struct Lienzo {
    w: i32,
    h: i32,
    pixeles: Vec<Rgba>,
}

impl Lienzo {
    fn vacio(w: i32, h: i32) -> Lienzo {
        Lienzo::relleno(w, h, Rgba::TRANSPARENT)
    }

    fn relleno(w: i32, h: i32, c: Rgba) -> Lienzo {
        Lienzo {
            w,
            h,
            pixeles: vec![c; (w * h) as usize],
        }
    }

    /// Único punto donde se escribe un píxel; lo de fuera se ignora.
    fn pon(&mut self, x: i32, y: i32, c: Rgba) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.pixeles[(y * self.w + x) as usize] = c;
        }
    }

    /// Rectángulo con los dos extremos incluidos.
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgba) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.pon(x, y, c);
            }
        }
    }

    /// Disco por desigualdad entera: `(x − cx)² + (y − cy)² ≤ r²`.
    fn disco(&mut self, cx: i32, cy: i32, r: i32, c: Rgba) {
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                if (x - cx) * (x - cx) + (y - cy) * (y - cy) <= r * r {
                    self.pon(x, y, c);
                }
            }
        }
    }
}

/// Una capa del caso con su celda del tamaño del lienzo.
struct Capa {
    visible: bool,
    opacidad: u8,
    modo: BlendMode,
    celda: Lienzo,
}

impl Capa {
    fn normal(celda: Lienzo) -> Capa {
        Capa {
            visible: true,
            opacidad: 255,
            modo: BlendMode::Normal,
            celda,
        }
    }

    fn con(modo: BlendMode, opacidad: u8, celda: Lienzo) -> Capa {
        Capa {
            visible: true,
            opacidad,
            modo,
            celda,
        }
    }
}

/// Un caso del corpus: su nombre (el del PNG), el tamaño y las capas de abajo arriba.
struct Caso {
    nombre: String,
    w: i32,
    h: i32,
    capas: Vec<Capa>,
}

/// Matriz 8 × 8: columnas de 4 px con los colores `A` (fondo), filas de 4 px con los `B` (origen).
fn matriz(nombre_modo: &str, modo: BlendMode) -> Caso {
    let mut a = Lienzo::vacio(32, 32);
    let mut b = Lienzo::vacio(32, 32);
    for y in 0..32 {
        for x in 0..32 {
            a.pon(x, y, color(A[(x / 4) as usize], 255));
            b.pon(x, y, color(B[(y / 4) as usize], 255));
        }
    }
    Caso {
        nombre: format!("mezcla-{nombre_modo}"),
        w: 32,
        h: 32,
        capas: vec![Capa::normal(a), Capa::con(modo, 255, b)],
    }
}

/// Un sprite de 64 × 16: base con los colores `A` por columna (`x mod 8`) y ocho capas, una por
/// modo, cada una en su tira de 8 columnas; el origen cambia de color por fila.
fn ocho_modos(nombre: &str, opacidad: u8, alfa_alto: u8, alfa_bajo: u8, alfa_base: u8) -> Caso {
    let mut base = Lienzo::vacio(64, 16);
    for y in 0..16 {
        for x in 0..64 {
            base.pon(x, y, color(A[(x % 8) as usize], alfa_base));
        }
    }
    let mut capas = vec![Capa::normal(base)];
    for (k, (_, modo)) in MODOS.iter().enumerate() {
        let mut celda = Lienzo::vacio(64, 16);
        for y in 0..16 {
            let rgb = B[((y + if y >= 8 { 3 } else { 0 }) % 8) as usize];
            let alfa = if y < 8 { alfa_alto } else { alfa_bajo };
            for x in 8 * k as i32..8 * (k as i32 + 1) {
                celda.pon(x, y, color(rgb, alfa));
            }
        }
        capas.push(Capa::con(*modo, opacidad, celda));
    }
    Caso {
        nombre: nombre.to_string(),
        w: 64,
        h: 16,
        capas,
    }
}

fn capas_opacidad() -> Caso {
    let mut base = Lienzo::vacio(32, 16);
    for y in 0..16 {
        for x in 0..32 {
            let rgb = if (x / 4 + y / 4) % 2 == 0 {
                BLANCO
            } else {
                NEGRO
            };
            base.pon(x, y, color(rgb, 255));
        }
    }
    let mut capas = vec![Capa::normal(base)];
    for (k, op) in [0u8, 64, 128, 200, 255].into_iter().enumerate() {
        let mut t = Lienzo::vacio(32, 16);
        let x0 = 1 + 6 * k as i32;
        t.rect(x0, 2, x0 + 4, 13, color(ROJO, 255));
        capas.push(Capa::con(BlendMode::Normal, op, t));
    }
    Caso {
        nombre: "capas-opacidad".to_string(),
        w: 32,
        h: 16,
        capas,
    }
}

fn capas_alfa_origen() -> Caso {
    let mut fondo = Lienzo::vacio(32, 16);
    for (k, rgb) in [BLANCO, GRIS, AZUL, NEGRO].into_iter().enumerate() {
        fondo.rect(8 * k as i32, 0, 8 * (k as i32 + 1) - 1, 15, color(rgb, 255));
    }
    let mut origen = Lienzo::vacio(32, 16);
    for (k, a) in [0u8, 36, 73, 109, 146, 182, 219, 255]
        .into_iter()
        .enumerate()
    {
        let x0 = 4 * k as i32;
        origen.rect(x0, 0, x0 + 3, 7, color(NARANJA, a));
        origen.rect(x0, 8, x0 + 3, 15, color(CIAN, a));
    }
    Caso {
        nombre: "capas-alfa-origen".to_string(),
        w: 32,
        h: 16,
        capas: vec![Capa::normal(fondo), Capa::normal(origen)],
    }
}

fn capas_alfa_fondo() -> Caso {
    let alfas = [255u8, 200, 128, 64];
    let mut base = Lienzo::vacio(32, 16);
    let mut arriba = Lienzo::vacio(32, 16);
    for (k, &a) in alfas.iter().enumerate() {
        let k = k as i32;
        base.rect(8 * k, 0, 8 * (k + 1) - 1, 15, color(VERDE, a));
        arriba.rect(0, 4 * k, 31, 4 * (k + 1) - 1, color(MAGENTA, a));
    }
    Caso {
        nombre: "capas-alfa-fondo".to_string(),
        w: 32,
        h: 16,
        capas: vec![Capa::normal(base), Capa::normal(arriba)],
    }
}

/// La capa de abajo con opacidad 128 y modo Multiplicar sobre el lienzo transparente y una capa
/// Normal encima (con un disco y un cuadrado con alfa 128). La fila de alfa 0 lleva RGB.
fn fondo_con_opacidad() -> Caso {
    let alfas = [255u8, 223, 191, 159, 127, 95, 63, 0];
    let mut base = Lienzo::vacio(32, 32);
    for y in 0..32 {
        for x in 0..32 {
            base.pon(x, y, color(A[(x / 4) as usize], alfas[(y / 4) as usize]));
        }
    }
    let mut arriba = Lienzo::vacio(32, 32);
    arriba.disco(16, 16, 9, color(NARANJA, 255));
    arriba.rect(2, 2, 13, 13, color(CIAN, 128));
    Caso {
        nombre: "fondo-con-opacidad".to_string(),
        w: 32,
        h: 32,
        capas: vec![
            Capa::con(BlendMode::Multiply, 128, base),
            Capa::normal(arriba),
        ],
    }
}

/// Alfas de las rejillas (el 127 al final, fuera de orden, a propósito).
const ALFAS_REJILLA: [u8; 8] = [0, 40, 80, 120, 160, 200, 255, 127];
/// Los dos pares de colores de las rejillas: `(x0, fuente, fondo)`; el segundo cambia fuente y
/// fondo de sitio.
const PARES_REJILLA: [(i32, [u8; 3], [u8; 3]); 2] = [
    (4, [200, 60, 20], [30, 120, 220]),
    (36, [30, 120, 220], [200, 60, 20]),
];

/// Dos rejillas de 8 × 8 celdas de 3 × 3 px: columnas = alfa del fondo, filas = alfa de la
/// fuente. La base es gris 90 opaco fuera de las rejillas y translúcida dentro; la capa de
/// fuente solo existe dentro.
fn rejilla(nombre: &str, modo: BlendMode, opacidad: u8) -> Caso {
    let mut base = Lienzo::relleno(64, 32, color([90, 90, 90], 255));
    let mut fuente = Lienzo::vacio(64, 32);
    for (x_inicial, c_fuente, c_fondo) in PARES_REJILLA {
        for (i, &alfa_fondo) in ALFAS_REJILLA.iter().enumerate() {
            for (j, &alfa_fuente) in ALFAS_REJILLA.iter().enumerate() {
                let x0 = x_inicial + 3 * i as i32;
                let y0 = 4 + 3 * j as i32;
                base.rect(x0, y0, x0 + 2, y0 + 2, color(c_fondo, alfa_fondo));
                fuente.rect(x0, y0, x0 + 2, y0 + 2, color(c_fuente, alfa_fuente));
            }
        }
    }
    Caso {
        nombre: nombre.to_string(),
        w: 64,
        h: 32,
        capas: vec![Capa::normal(base), Capa::con(modo, opacidad, fuente)],
    }
}

// ---------------------------------------------------------------------------------------
// Del caso al documento, el PNG y la comparación.
// ---------------------------------------------------------------------------------------

/// El documento del caso, solo con los comandos del núcleo.
fn documento(caso: &Caso) -> Document {
    let nombre = &caso.nombre;
    let imagen = Image::new(caso.w as u32, caso.h as u32, Mode::Rgba).expect("imagen RGBA");
    let mut doc = Document::new(imagen);
    for i in 1..caso.capas.len() {
        let cmd = Command::AddLayer {
            name: format!("capa {i}"),
            at: i,
        };
        assert_eq!(doc.apply(cmd), Ok(true), "{nombre}: añadir la capa {i}");
    }
    for (i, capa) in caso.capas.iter().enumerate() {
        let (id, cel) = (doc.layers()[i].id(), doc.layers()[i].cel());
        // Solo los píxeles que no son `(0, 0, 0, 0)`: lo demás ya es así en una celda nueva.
        let pixels: Vec<PixelEdit> = capa
            .celda
            .pixeles
            .iter()
            .enumerate()
            .filter(|(_, c)| **c != Rgba::TRANSPARENT)
            .map(|(k, &c)| PixelEdit {
                x: k as u32 % caso.w as u32,
                y: k as u32 / caso.w as u32,
                pixel: Pixel::Rgba(c),
            })
            .collect();
        if !pixels.is_empty() {
            let cmd = Command::SetPixels { image: cel, pixels };
            assert!(doc.apply(cmd).is_ok(), "{nombre}: escribir la capa {i}");
        }
        if capa.opacidad != 255 {
            let cmd = Command::SetLayerOpacity {
                layer: id,
                opacity: capa.opacidad,
            };
            assert!(doc.apply(cmd).is_ok(), "{nombre}: opacidad de la capa {i}");
        }
        if capa.modo != BlendMode::Normal {
            let cmd = Command::SetLayerBlend {
                layer: id,
                blend: capa.modo,
            };
            assert!(doc.apply(cmd).is_ok(), "{nombre}: modo de la capa {i}");
        }
        if !capa.visible {
            let cmd = Command::SetLayerVisible {
                layer: id,
                visible: false,
            };
            assert!(doc.apply(cmd).is_ok(), "{nombre}: ocultar la capa {i}");
        }
    }
    doc
}

/// El PNG aplanado del caso (`golden/aseprite/<caso>.png`), RGBA de 8 bits.
fn lee_png(nombre: &str) -> Image {
    let ruta = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../golden/aseprite")
        .join(format!("{nombre}.png"));
    let bytes =
        fs::read(&ruta).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", ruta.display()));
    let leido =
        read_png(&bytes).unwrap_or_else(|e| panic!("{nombre}.png: read_png dio error: {e}"));
    assert_eq!(leido.image.mode(), Mode::Rgba, "{nombre}.png debe ser RGBA");
    leido.image
}

fn pixel_de(img: &Image, x: u32, y: u32) -> Rgba {
    match img.get_pixel(x, y).expect("píxel dentro") {
        Pixel::Rgba(c) => c,
        otro => panic!("se esperaba RGBA, hay {otro:?}"),
    }
}

/// `compose` del caso y el PNG, con el tamaño ya comprobado.
fn compuesto_y_png(caso: &Caso) -> (Image, Image) {
    let nombre = &caso.nombre;
    let compuesto = compose(&documento(caso)).unwrap_or_else(|e| panic!("{nombre}: compose: {e}"));
    let png = lee_png(nombre);
    assert_eq!(
        (compuesto.width(), compuesto.height(), compuesto.mode()),
        (png.width(), png.height(), Mode::Rgba),
        "{nombre}: tamaño o modo de la composición distinto del PNG"
    );
    (compuesto, png)
}

/// Tolerancia 0: todos los bytes de la composición (RGB bajo alfa 0 incluido) son los del PNG.
/// Falla con el nombre del caso y el primer píxel distinto.
fn exige_igual_al_png(caso: &Caso) {
    let (compuesto, png) = compuesto_y_png(caso);
    exige_igual_imagen(caso, &compuesto, &png);
}

/// Como [`exige_igual_al_png`] con la imagen ya calculada (la composición, o lo que dejan las
/// operaciones de la unidad 3.5).
fn exige_igual_imagen(caso: &Caso, compuesto: &Image, png: &Image) {
    let (mut distintos, mut primero) = (0usize, None);
    for y in 0..png.height() {
        for x in 0..png.width() {
            let (nuestro, esperado) = (pixel_de(compuesto, x, y), pixel_de(png, x, y));
            if nuestro != esperado {
                distintos += 1;
                primero.get_or_insert((x, y, nuestro, esperado));
            }
        }
    }
    if let Some((x, y, nuestro, esperado)) = primero {
        panic!(
            "{}: {distintos} píxeles distintos del PNG; el primero, ({x}, {y}): \
             compose da {nuestro:?} y el PNG {esperado:?}",
            caso.nombre
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 9: las ocho matrices de 64 combinaciones.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_09_ocho_matrices_de_mezcla_con_tolerancia_0() {
    for (nombre, modo) in MODOS {
        exige_igual_al_png(&matriz(nombre, modo));
    }
}

// ---------------------------------------------------------------------------------------
// Fila 10: opacidad, alfa de origen y capa de abajo con opacidad.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_10_opacidad_alfa_de_origen_y_fondo_con_opacidad_con_tolerancia_0() {
    let casos = [
        ocho_modos("mezcla-ocho-opacidad", 128, 255, 255, 255),
        ocho_modos("mezcla-ocho-alfa", 255, 200, 77, 255),
        capas_opacidad(),
        capas_alfa_origen(),
        // La capa de arriba lleva `(0, 0, 0, 0)` sobre la de abajo con alfa 0 y RGB ≠ 0: el paso 0
        // de la spec (un origen `(0, 0, 0, 0)` no aporta nada) conserva ese RGB, como el PNG.
        fondo_con_opacidad(),
    ];
    for caso in &casos {
        exige_igual_al_png(caso);
    }
}

// ---------------------------------------------------------------------------------------
// Fila 11: fondo translúcido (dominio de la fórmula empírica).
// ---------------------------------------------------------------------------------------

/// `x / 255` al entero más cercano (sin empates posibles), a partir de la definición.
fn redondea_255(x: u32) -> u32 {
    (2 * x + 255) / 510
}

/// `true` si el píxel `(x, y)` está en el dominio de la fórmula empírica: en alguna capa visible,
/// el fondo ya compuesto debajo tiene `0 < αb < 255`, el origen tiene `αs = div255(a ·
/// opacidad) > 0` y el modo no es Normal. El fondo se pliega con `composite_pixel` capa a capa.
fn en_dominio(caso: &Caso, x: u32, y: u32) -> bool {
    let k = (y as i32 * caso.w + x as i32) as usize;
    let mut fondo = Rgba::TRANSPARENT;
    let mut dentro = false;
    for capa in caso.capas.iter().filter(|c| c.visible) {
        let origen = capa.celda.pixeles[k];
        let alfa_origen = redondea_255(u32::from(origen.a) * u32::from(capa.opacidad));
        if fondo.a > 0 && fondo.a < 255 && alfa_origen > 0 && capa.modo != BlendMode::Normal {
            dentro = true;
        }
        fondo = composite_pixel(fondo, origen, capa.opacidad, capa.modo);
    }
    dentro
}

/// Lo que da evaluar un caso contra su PNG, agrupado (un grupo por tira, o uno solo).
struct Informe {
    /// Error máximo de canal en el dominio, por grupo.
    max_error: Vec<i32>,
    /// Píxeles del dominio, por grupo.
    en_dominio: Vec<usize>,
    /// El primer incumplimiento (nombre, píxel y valores), si lo hay.
    fallo: Option<String>,
}

/// Compara la imagen de un caso (`compose`, o lo que deja Aplanar) con su PNG: fuera del dominio, tolerancia 0 (el RGB bajo alfa 0 no
/// se exige cuando el alfa de la composición y el del PNG son 0); dentro, el alfa es exacto y el
/// error de cada canal RGB se acumula en el máximo de su grupo (`grupo(x, y)`) y debe ser como
/// mucho `TOLERANCIA_EMPIRICA`.
fn evalua_imagen(
    caso: &Caso,
    compuesto: &Image,
    png: &Image,
    grupo: impl Fn(u32, u32) -> usize,
    grupos: usize,
) -> Informe {
    let mut informe = Informe {
        max_error: vec![0; grupos],
        en_dominio: vec![0; grupos],
        fallo: None,
    };
    for y in 0..png.height() {
        for x in 0..png.width() {
            let (nuestro, esperado) = (pixel_de(compuesto, x, y), pixel_de(png, x, y));
            let dentro = en_dominio(caso, x, y);
            let mut error_pixel = 0;
            let mut motivo = None;
            if nuestro.a != esperado.a {
                motivo = Some("el alfa no es exacto");
            } else if dentro {
                let diferencias = [
                    i32::from(nuestro.r) - i32::from(esperado.r),
                    i32::from(nuestro.g) - i32::from(esperado.g),
                    i32::from(nuestro.b) - i32::from(esperado.b),
                ];
                error_pixel = diferencias.iter().map(|d| d.abs()).max().unwrap_or(0);
                if error_pixel > TOLERANCIA_EMPIRICA {
                    motivo = Some("el error pasa del techo de la fórmula empírica");
                }
            } else if !(nuestro.a == 0 && esperado.a == 0) && nuestro != esperado {
                motivo = Some("fuera del dominio la tolerancia es 0");
            }
            if dentro {
                let g = grupo(x, y);
                informe.en_dominio[g] += 1;
                informe.max_error[g] = informe.max_error[g].max(error_pixel);
            }
            if let Some(motivo) = motivo
                && informe.fallo.is_none()
            {
                informe.fallo = Some(format!(
                    "{}: píxel ({x}, {y}) {}: {motivo}; compose da {nuestro:?} y el PNG \
                     {esperado:?}",
                    caso.nombre,
                    if dentro {
                        "del dominio"
                    } else {
                        "fuera del dominio"
                    },
                ));
            }
        }
    }
    informe
}

/// Que no haya ningún incumplimiento.
fn exige_sin_fallo(informe: &Informe) {
    if let Some(fallo) = &informe.fallo {
        panic!("{fallo}");
    }
}

/// El error máximo del grupo `que` debe estar en `[1, TOLERANCIA_EMPIRICA]`: el techo vigila la
/// fórmula y el suelo, que la exención no sobre.
fn exige_techo_y_suelo(caso: &str, que: &str, maximo: i32) {
    assert!(
        maximo != 0,
        "{caso} ({que}): el error máximo dentro del dominio es 0: la exención es innecesaria: \
         usa tolerancia 0 en este caso"
    );
    assert!(
        maximo <= TOLERANCIA_EMPIRICA,
        "{caso} ({que}): el error máximo dentro del dominio es {maximo}, más que el techo \
         {TOLERANCIA_EMPIRICA}"
    );
}

#[test]
fn fila_11_fondo_translucido_dominio_de_la_formula_empirica() {
    fila_11_con(|caso| compuesto_y_png(caso).0);
}

/// El cuerpo de la fila 11 con la imagen que `produce` da para cada caso (la composición, en 3.2;
/// la celda que deja `Aplanar`, en 3.5).
fn fila_11_con(produce: impl Fn(&Caso) -> Image) {
    let evalua = |caso: &Caso, grupo: &dyn Fn(u32, u32) -> usize, grupos: usize| {
        let imagen = produce(caso);
        evalua_imagen(caso, &imagen, &lee_png(&caso.nombre), grupo, grupos)
    };
    // `capas-alfa-fondo`: dos translúcidos a la vez con Normal, que no es del dominio: todo con
    // tolerancia 0.
    let informe = evalua(&capas_alfa_fondo(), &|_, _| 0, 1);
    exige_sin_fallo(&informe);
    assert_eq!(
        informe.en_dominio,
        [0],
        "capas-alfa-fondo: Normal no es del dominio"
    );

    // `mezcla-ocho-fondo-translucido`: fondo con alfa 150 y origen con 255 (arriba) y 100
    // (abajo). La tira Normal (la primera) está fuera del dominio: tolerancia 0. Cada una de las
    // otras siete es del dominio entera (8 columnas × 16 filas) y su error máximo está en [1, 2].
    let tiras = ocho_modos("mezcla-ocho-fondo-translucido", 255, 255, 100, 150);
    let informe = evalua(&tiras, &|x, _| (x / 8) as usize, 8);
    exige_sin_fallo(&informe);
    assert_eq!(informe.en_dominio[0], 0, "la tira Normal no es del dominio");
    assert_eq!(informe.max_error[0], 0);
    for (k, (nombre_modo, _)) in MODOS.iter().enumerate().skip(1) {
        assert_eq!(
            informe.en_dominio[k], 128,
            "la tira {nombre_modo} es del dominio entera"
        );
        exige_techo_y_suelo(
            "mezcla-ocho-fondo-translucido",
            &format!("tira {nombre_modo}"),
            informe.max_error[k],
        );
    }

    // Las dos rejillas de alfas: de las 128 celdas de 3 × 3 de cada una, 84 son del dominio
    // (fondo translúcido, fuente visible, modo distinto de Normal) y el resto, no; el error
    // máximo de las 84 celdas está en [1, 2].
    for (nombre, modo, opacidad) in [
        ("mezcla-alfa-rejilla-multiplicar", BlendMode::Multiply, 255),
        ("mezcla-alfa-rejilla-superponer", BlendMode::Overlay, 128),
    ] {
        let informe = evalua(&rejilla(nombre, modo, opacidad), &|_, _| 0, 1);
        exige_sin_fallo(&informe);
        assert_eq!(informe.en_dominio, [84 * 9], "{nombre}: 84 celdas de 3 × 3");
        exige_techo_y_suelo(nombre, "rejilla", informe.max_error[0]);
    }
}

// ---------------------------------------------------------------------------------------
// Fila 8 de la unidad 3.5 (specs/hito-3-combinar.md): aplanar y combinar abajo contra los PNG.
// ---------------------------------------------------------------------------------------

/// Aplana el documento del caso con `flatten_command`: comprueba que queda una sola capa y que
/// su celda es la composición de antes, byte a byte, y devuelve esa celda.
fn aplana(caso: &Caso) -> Image {
    let nombre = &caso.nombre;
    let mut doc = documento(caso);
    let antes = compose(&doc).unwrap_or_else(|e| panic!("{nombre}: compose: {e:?}"));
    let orden = flatten_command(&doc).unwrap_or_else(|e| panic!("{nombre}: aplanar: {e:?}"));
    assert_eq!(doc.apply(orden), Ok(true), "{nombre}: aplicar el aplanado");
    assert_eq!(doc.layers().len(), 1, "{nombre}: queda una sola capa");
    let celda = doc
        .image(doc.layers()[0].cel())
        .unwrap_or_else(|| panic!("{nombre}: la celda aplanada tiene imagen"))
        .clone();
    assert_eq!(
        celda, antes,
        "{nombre}: la celda es la composición de antes"
    );
    assert_eq!(
        compose(&doc).expect("compose tras aplanar"),
        antes,
        "{nombre}: componer tras aplanar da lo mismo que antes"
    );
    celda
}

/// Los casos de las filas 9 y 10 de 3.2 (tolerancia 0).
fn casos_exactos() -> Vec<Caso> {
    let mut casos: Vec<Caso> = MODOS
        .iter()
        .map(|(nombre, modo)| matriz(nombre, *modo))
        .collect();
    casos.extend([
        ocho_modos("mezcla-ocho-opacidad", 128, 255, 255, 255),
        ocho_modos("mezcla-ocho-alfa", 255, 200, 77, 255),
        capas_opacidad(),
        capas_alfa_origen(),
        fondo_con_opacidad(),
    ]);
    casos
}

#[test]
fn fila_08_aplanar_los_17_casos_de_3_2_da_la_composicion_y_cumple_sus_tolerancias() {
    // Filas 9 y 10 de 3.2: tolerancia 0 contra el PNG.
    for caso in &casos_exactos() {
        let celda = aplana(caso);
        exige_igual_imagen(caso, &celda, &lee_png(&caso.nombre));
    }
    // Fila 11 de 3.2: tolerancia 0 fuera del dominio empírico y techo 2 dentro (suelo 1).
    fila_11_con(aplana);
}

#[test]
fn fila_08_combinar_abajo_en_las_ocho_matrices_a_tolerancia_0() {
    for (nombre, modo) in MODOS {
        let caso = matriz(nombre, modo);
        let mut doc = documento(&caso);
        let antes = compose(&doc).expect("compose");
        let arriba = doc.layers()[1].id();
        let orden = merge_down_command(&doc, arriba)
            .unwrap_or_else(|e| panic!("{nombre}: combinar abajo: {e:?}"));
        assert_eq!(doc.apply(orden), Ok(true), "{nombre}");
        assert_eq!(doc.layers().len(), 1, "{nombre}");
        let celda = doc
            .image(doc.layers()[0].cel())
            .expect("celda combinada")
            .clone();
        assert_eq!(
            celda, antes,
            "{nombre}: la matriz de dos capas se combina sin pérdida"
        );
        exige_igual_imagen(&caso, &celda, &lee_png(&caso.nombre));
    }
}
