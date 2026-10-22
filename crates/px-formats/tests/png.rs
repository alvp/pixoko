//! Tests de PNG en `px-formats` (specs/hito-1-png.md, unidad 5).
//!
//! El corpus se construye aquí, byte a byte: chunks, CRC-32, Adler-32, deflate «stored» y
//! extracción de pasadas Adam7, todo escrito a mano (sin `flate2`, `crc32fast` ni `png`).
//! Los valores esperados salen de la tabla del comportamiento 2 y de las fórmulas del
//! comportamiento 3 de la spec, no de ningún programa de referencia.
//!
//! Ningún test reserva más de unos pocos KiB: los casos de tamaño se prueban por el lado del
//! rechazo, con cabecera enorme y datos mínimos.

use std::fmt::{Debug, Display};

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::select;
use proptest::test_runner::TestCaseError;
use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_formats::{AuxChunk, Conversion, PngError, PngRead, read_png, write_png};

#[path = "png/aux_chunks.rs"]
mod aux_chunks;
// Unidad 13 (cota de lectura). Estos dos módulos SÍ usan el crate `png` (compressors.rs como
// codificador); la cabecera de este fichero dice «sin `png`» solo del corpus de la unidad 5.
#[path = "png/compressors.rs"]
mod compressors;
#[path = "png/max_ratio.rs"]
mod max_ratio;

// ---------------------------------------------------------------------------------------
// Ayudante de corpus: CRC-32, Adler-32, deflate «stored», chunks y Adam7.
// ---------------------------------------------------------------------------------------

const TEST_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// CRC-32 de la especificación PNG (polinomio 0xEDB88320, bit a bit).
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Adler-32 de la especificación zlib (RFC 1950).
fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Flujo zlib con bloques deflate «stored» (sin comprimir). `adler_xor` estropea el Adler-32.
fn zlib_stored(data: &[u8], adler_xor: u32) -> Vec<u8> {
    let mut v = vec![0x78, 0x01];
    if data.is_empty() {
        v.extend([1, 0, 0, 0xFF, 0xFF]);
    } else {
        let blocks = data.chunks(65535).count();
        for (i, block) in data.chunks(65535).enumerate() {
            v.push(u8::from(i + 1 == blocks));
            let n = block.len() as u16;
            v.extend(n.to_le_bytes());
            v.extend((!n).to_le_bytes());
            v.extend(block);
        }
    }
    v.extend((adler32(data) ^ adler_xor).to_be_bytes());
    v
}

/// Chunk completo: longitud, tipo, datos y CRC.
fn piece(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + data.len());
    v.extend((data.len() as u32).to_be_bytes());
    v.extend(kind);
    v.extend(data);
    let mut body = kind.to_vec();
    body.extend(data);
    v.extend(crc32(&body).to_be_bytes());
    v
}

fn plte(colors: &[(u8, u8, u8)]) -> Vec<u8> {
    let data: Vec<u8> = colors.iter().flat_map(|&(r, g, b)| [r, g, b]).collect();
    piece(b"PLTE", &data)
}

fn trns(data: &[u8]) -> Vec<u8> {
    piece(b"tRNS", data)
}

/// Canales por píxel de cada tipo de color PNG.
fn channels(color: u8) -> usize {
    match color {
        2 => 3,
        4 => 2,
        6 => 4,
        _ => 1,
    }
}

/// Empaqueta las muestras de una fila: sub-8 bits de más a menos significativo con relleno
/// a byte, 8 bits tal cual y 16 bits en big-endian.
fn packs(row: &[u16], prof: u8) -> Vec<u8> {
    match prof {
        16 => row.iter().flat_map(|v| v.to_be_bytes()).collect(),
        8 => row.iter().map(|&v| v as u8).collect(),
        _ => {
            let p = usize::from(prof);
            row.chunks(8 / p)
                .map(|group| {
                    let mut byte = 0u8;
                    for (i, &v) in group.iter().enumerate() {
                        byte |= (v as u8) << (8 - p * (i + 1));
                    }
                    byte
                })
                .collect()
        }
    }
}

/// Filas (con su byte de filtro 0) de los píxeles en las columnas `xs` y filas `ys`.
fn pass(m: &[u16], width: usize, channels: usize, prof: u8, xs: &[usize], ys: &[usize]) -> Vec<u8> {
    let mut output = Vec::new();
    for &y in ys {
        let row: Vec<u16> = xs
            .iter()
            .flat_map(|&x| (0..channels).map(move |c| m[(y * width + x) * channels + c]))
            .collect();
        output.push(0);
        output.extend(packs(&row, prof));
    }
    output
}

/// Las siete pasadas de Adam7: (x inicial, y inicial, salto en x, salto en y).
const ADAM7: [(usize, usize, usize, usize); 7] = [
    (0, 0, 8, 8),
    (4, 0, 8, 8),
    (0, 4, 4, 8),
    (2, 0, 4, 4),
    (0, 2, 2, 4),
    (1, 0, 2, 2),
    (0, 1, 1, 2),
];

/// Datos sin comprimir del IDAT: las filas en orden o, con `interlaced_2`, las pasadas Adam7
/// (las pasadas vacías no aportan ni un byte).
fn scans(
    width: usize,
    height: usize,
    channels: usize,
    prof: u8,
    m: &[u16],
    interlaced_2: bool,
) -> Vec<u8> {
    if !interlaced_2 {
        let xs: Vec<usize> = (0..width).collect();
        let ys: Vec<usize> = (0..height).collect();
        return pass(m, width, channels, prof, &xs, &ys);
    }
    let mut output = Vec::new();
    for (x0, y0, dx, dy) in ADAM7 {
        let xs: Vec<usize> = (x0..width).step_by(dx).collect();
        let ys: Vec<usize> = (y0..height).step_by(dy).collect();
        if !xs.is_empty() && !ys.is_empty() {
            output.extend(pass(m, width, channels, prof, &xs, &ys));
        }
    }
    output
}

/// Fichero PNG en construcción. Con `swatches` vacío no se genera ningún dato de imagen
/// (así se hacen ficheros de cabecera enorme y datos mínimos).
#[derive(Clone)]
struct Png {
    width: u32,
    height: u32,
    prof: u8,
    color: u8,
    interlaced_2: bool,
    /// Muestras por píxel y canal, fila a fila, en el orden de la imagen final.
    swatches: Vec<u16>,
    /// Si es `Some`, datos sin comprimir del IDAT tal cual (anula `swatches`).
    raw_fixed: Option<Vec<u8>>,
    /// Chunks ya serializados entre IHDR e IDAT.
    before: Vec<Vec<u8>>,
    /// Chunks ya serializados entre IDAT e IEND.
    after: Vec<Vec<u8>>,
    adler_xor: u32,
}

impl Png {
    fn new(width: u32, height: u32, prof: u8, color: u8) -> Png {
        Png {
            width,
            height,
            prof,
            color,
            interlaced_2: false,
            swatches: Vec::new(),
            raw_fixed: None,
            before: Vec::new(),
            after: Vec::new(),
            adler_xor: 0,
        }
    }

    fn swatches(mut self, m: &[u16]) -> Png {
        self.swatches = m.to_vec();
        self
    }

    fn interlaced_2(mut self, si: bool) -> Png {
        self.interlaced_2 = si;
        self
    }

    fn before(mut self, chunk: Vec<u8>) -> Png {
        self.before.push(chunk);
        self
    }

    fn after(mut self, chunk: Vec<u8>) -> Png {
        self.after.push(chunk);
        self
    }

    fn raw(&self) -> Vec<u8> {
        if let Some(c) = &self.raw_fixed {
            return c.clone();
        }
        if self.swatches.is_empty() {
            return Vec::new();
        }
        scans(
            self.width as usize,
            self.height as usize,
            channels(self.color),
            self.prof,
            &self.swatches,
            self.interlaced_2,
        )
    }

    fn bytes(&self) -> Vec<u8> {
        let mut ihdr = Vec::new();
        ihdr.extend(self.width.to_be_bytes());
        ihdr.extend(self.height.to_be_bytes());
        ihdr.extend([self.prof, self.color, 0, 0, u8::from(self.interlaced_2)]);
        let mut v = TEST_SIGNATURE.to_vec();
        v.extend(piece(b"IHDR", &ihdr));
        for c in &self.before {
            v.extend(c);
        }
        v.extend(piece(b"IDAT", &zlib_stored(&self.raw(), self.adler_xor)));
        for c in &self.after {
            v.extend(c);
        }
        v.extend(piece(b"IEND", &[]));
        v
    }
}

/// Un chunk encontrado por el recorredor: tipo, posición del campo de longitud y datos.
struct Piece {
    kind: [u8; 4],
    start: usize,
    data: Vec<u8>,
}

/// Recorre la estructura de chunks de un fichero bien formado: firma, longitud, tipo, datos
/// y CRC de cada chunk, hasta un IEND que termina exactamente en el último byte.
fn walks(bytes: &[u8]) -> Result<Vec<Piece>, String> {
    if bytes.len() < 8 || bytes[..8] != TEST_SIGNATURE {
        return Err("firma incorrecta".into());
    }
    let mut pos = 8usize;
    let mut pieces = Vec::new();
    loop {
        let header = bytes
            .get(pos..pos + 8)
            .ok_or_else(|| format!("cabecera truncada en {pos}"))?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let kind: [u8; 4] = header[4..8].try_into().unwrap();
        let data = bytes
            .get(pos + 8..pos + 8 + length)
            .ok_or_else(|| format!("datos truncados en {pos}"))?;
        let crc = bytes
            .get(pos + 8 + length..pos + 12 + length)
            .ok_or_else(|| format!("CRC truncado en {pos}"))?;
        let mut body = kind.to_vec();
        body.extend(data);
        if u32::from_be_bytes(crc.try_into().unwrap()) != crc32(&body) {
            return Err(format!("CRC incorrecto en {pos}"));
        }
        pieces.push(Piece {
            kind,
            start: pos,
            data: data.to_vec(),
        });
        pos += 12 + length;
        if kind == *b"IEND" {
            if pos != bytes.len() {
                return Err(format!("sobran {} bytes tras IEND", bytes.len() - pos));
            }
            return Ok(pieces);
        }
    }
}

/// Posición del chunk IDAT y longitud de sus datos.
fn position_idat(bytes: &[u8]) -> (usize, usize) {
    let t = walks(bytes)
        .unwrap()
        .into_iter()
        .find(|t| t.kind == *b"IDAT")
        .unwrap();
    (t.start, t.data.len())
}

// ---------------------------------------------------------------------------------------
// Ayudantes de aserción.
// ---------------------------------------------------------------------------------------

fn assert_read_traits<T: Clone + Debug + PartialEq + Eq>() {}
fn assert_conversion_traits<T: Clone + Copy + Debug + PartialEq + Eq>() {}
fn assert_error_traits<T: Debug + Clone + PartialEq + Eq + Display + std::error::Error>() {}

fn lee(bytes: &[u8]) -> PngRead {
    read_png(bytes).unwrap()
}

fn error_de(bytes: &[u8]) -> PngError {
    read_png(bytes).unwrap_err()
}

fn es_decode(r: &Result<PngRead, PngError>) -> bool {
    matches!(r, Err(PngError::Decode(_)))
}

/// Todos los píxeles, fila a fila.
fn pixels(img: &Image) -> Vec<Pixel> {
    let mut v = Vec::new();
    for y in 0..img.height() {
        for x in 0..img.width() {
            v.push(img.get_pixel(x, y).unwrap());
        }
    }
    v
}

fn grays(v: &[u8]) -> Vec<Pixel> {
    v.iter().map(|&g| Pixel::Gray(g)).collect()
}

fn grays_alpha(v: &[(u8, u8)]) -> Vec<Pixel> {
    v.iter()
        .map(|&(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
        .collect()
}

fn indices(v: &[u8]) -> Vec<Pixel> {
    v.iter().map(|&i| Pixel::Indexed(i)).collect()
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Pixel {
    Pixel::Rgba(Rgba::new(r, g, b, a))
}

fn palette_opaque(colors: &[(u8, u8, u8)]) -> Palette {
    Palette::from_colors(colors.iter().map(|&(r, g, b)| Rgba::opaque(r, g, b))).unwrap()
}

fn palette_with_alphas(colors: &[(u8, u8, u8)], alphas: &[u8]) -> Palette {
    Palette::from_colors(
        colors
            .iter()
            .zip(alphas)
            .map(|(&(r, g, b), &a)| Rgba::new(r, g, b, a)),
    )
    .unwrap()
}

/// Imagen indexada de una fila con la paleta dada.
fn indexed(colors: &[Rgba], transparent: Option<u8>, px: &[u8]) -> Image {
    let mut p = Palette::from_colors(colors.iter().copied()).unwrap();
    p.set_transparent_index(transparent).unwrap();
    let mut img = Image::new_indexed(px.len() as u32, 1, p).unwrap();
    for (x, &i) in px.iter().enumerate() {
        img.set_pixel(x as u32, 0, Pixel::Indexed(i)).unwrap();
    }
    img
}

/// Imagen 1 x 1 de un modo no indexado con ese píxel.
fn image_1x1(mode: Mode, pixel: Pixel) -> Image {
    let mut img = Image::new(1, 1, mode).unwrap();
    img.set_pixel(0, 0, pixel).unwrap();
    img
}

// ---------------------------------------------------------------------------------------
// Comprobación del propio ayudante (el corpus depende de él).
// ---------------------------------------------------------------------------------------

#[test]
fn crc32_and_adler32_helper_with_known_values() {
    // CRC del chunk IEND vacío, conocido: 0xAE426082.
    assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    assert_eq!(adler32(b""), 1);
    assert_eq!(
        piece(b"IEND", &[]),
        [0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]
    );
}

#[test]
fn helper_walks_the_corpus_and_rejects_the_malformed() {
    let good = Png::new(2, 1, 8, 0).swatches(&[1, 2]).bytes();
    let types: Vec<[u8; 4]> = walks(&good).unwrap().iter().map(|t| t.kind).collect();
    assert_eq!(types, [*b"IHDR", *b"IDAT", *b"IEND"]);

    let mut with_garbage = good.clone();
    with_garbage.push(0);
    assert!(walks(&with_garbage).is_err());
    assert!(walks(&good[..good.len() - 1]).is_err());
    let mut crc_broken = good.clone();
    crc_broken[32] ^= 0xFF;
    assert!(walks(&crc_broken).is_err());
}

#[test]
fn adam7_helper_distributes_all_pixels_of_8x8() {
    // 8 x 8 en gris 8: 1 + 1 + 2 + 4 + 8 + 16 + 32 = 64 píxeles en siete pasadas.
    let m: Vec<u16> = (0..64).collect();
    let raw = scans(8, 8, 1, 8, &m, true);
    // Filas por pasada: 1, 1, 1, 2, 2, 4, 4 (15 en total); cada una lleva su byte de filtro.
    assert_eq!(raw.len(), 64 + 15);
    // Primera pasada: el píxel (0, 0); segunda: el (4, 0).
    assert_eq!(&raw[..4], &[0, 0, 0, 4]);
}

// ---------------------------------------------------------------------------------------
// Traits e informe.
// ---------------------------------------------------------------------------------------

#[test]
fn traits_derived() {
    assert_read_traits::<PngRead>();
    assert_conversion_traits::<Conversion>();
    assert_error_traits::<PngError>();
}

#[test]
fn is_exact_is_empty_report() {
    let exact = lee(&Png::new(1, 1, 8, 0).swatches(&[9]).bytes());
    assert!(exact.report.is_empty());
    assert!(exact.is_exact());
    let converted = lee(&Png::new(1, 1, 8, 2).swatches(&[1, 2, 3]).bytes());
    assert!(!converted.report.is_empty());
    assert!(!converted.is_exact());
}

// ---------------------------------------------------------------------------------------
// Una prueba por fila de la tabla de casos límite.
// ---------------------------------------------------------------------------------------

#[test]
fn indexed_intact_at_1_2_4_and_8_bits() {
    let colors = [(1, 2, 3), (4, 5, 6), (7, 8, 9)];
    // A 1 bit solo caben 2 entradas (con 3 sería `InvalidPalette`), así que ahí la paleta
    // es de 2 y los índices son 0 y 1.
    let cases: [(u8, usize, [u16; 3]); 4] = [
        (1, 2, [0, 1, 1]),
        (2, 3, [0, 1, 2]),
        (4, 3, [0, 1, 2]),
        (8, 3, [0, 1, 2]),
    ];
    for (prof, n, px) in cases {
        let png = Png::new(3, 1, prof, 3)
            .swatches(&px)
            .before(plte(&colors[..n]))
            .bytes();
        let r = lee(&png);
        assert_eq!(r.image.mode(), Mode::Indexed, "{prof} bits");
        assert_eq!((r.image.width(), r.image.height()), (3, 1));
        let expected_2_2: Vec<u8> = px.iter().map(|&i| i as u8).collect();
        assert_eq!(pixels(&r.image), indices(&expected_2_2), "{prof} bits");
        let palette = r.image.palette().unwrap();
        assert_eq!(palette, &palette_opaque(&colors[..n]), "{prof} bits");
        assert_eq!(palette.len(), n);
        assert_eq!(palette.transparent_index(), None);
        assert!(r.report.is_empty(), "{prof} bits");
    }
}

#[test]
fn indexed_with_short_trns_completes_with_255() {
    let colors = [(1, 2, 3), (4, 5, 6), (7, 8, 9)];
    let png = Png::new(3, 1, 8, 3)
        .swatches(&[0, 1, 2])
        .before(plte(&colors))
        .before(trns(&[0, 128]))
        .bytes();
    let r = lee(&png);
    assert_eq!(r.image.mode(), Mode::Indexed);
    let palette = r.image.palette().unwrap();
    let alphas: Vec<u8> = palette.colors().iter().map(|c| c.a).collect();
    assert_eq!(alphas, [0, 128, 255]);
    assert_eq!(palette, &palette_with_alphas(&colors, &[0, 128, 255]));
    assert_eq!(palette.transparent_index(), None);
    assert_eq!(pixels(&r.image), indices(&[0, 1, 2]));
    assert!(r.report.is_empty());
}

#[test]
fn invalid_indexed_palette_and_indices() {
    // Píxel 5 con PLTE de 2.
    let outside = Png::new(2, 1, 8, 3)
        .swatches(&[0, 5])
        .before(plte(&[(1, 1, 1), (2, 2, 2)]))
        .bytes();
    assert_eq!(
        error_de(&outside),
        PngError::PaletteIndexOutOfRange { index: 5, len: 2 }
    );

    // Sin PLTE.
    let without_plte = Png::new(2, 1, 8, 3).swatches(&[0, 1]).bytes();
    assert_eq!(error_de(&without_plte), PngError::InvalidPalette);

    // PLTE de 3 entradas a 1 bit (el máximo es 2).
    let too_many = Png::new(2, 1, 1, 3)
        .swatches(&[0, 1])
        .before(plte(&[(1, 1, 1), (2, 2, 2), (3, 3, 3)]))
        .bytes();
    assert_eq!(error_de(&too_many), PngError::InvalidPalette);

    // PLTE vacío: lo rechaza el crate.
    let empty = Png::new(2, 1, 8, 3)
        .swatches(&[0, 0])
        .before(piece(b"PLTE", &[]))
        .bytes();
    assert!(es_decode(&read_png(&empty)), "{:?}", read_png(&empty));

    // tRNS de 3 entradas con PLTE de 2.
    let trns_length = Png::new(2, 1, 8, 3)
        .swatches(&[0, 1])
        .before(plte(&[(1, 1, 1), (2, 2, 2)]))
        .before(trns(&[0, 1, 2]))
        .bytes();
    assert_eq!(error_de(&trns_length), PngError::InvalidTrns);
}

#[test]
fn eight_bit_types_with_alpha_zero_and_nonzero_rgb_are_not_touched() {
    // Gris 8: sin alfa en el fichero.
    let g = lee(&Png::new(2, 1, 8, 0).swatches(&[77, 200]).bytes());
    assert_eq!(g.image.mode(), Mode::Gray);
    assert_eq!(pixels(&g.image), grays(&[77, 200]));
    assert!(g.report.is_empty());

    // Gris con alfa 8: alfa 0 con gris 77.
    let ga = lee(&Png::new(2, 1, 8, 4).swatches(&[77, 0, 200, 255]).bytes());
    assert_eq!(ga.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixels(&ga.image), grays_alpha(&[(77, 0), (200, 255)]));
    assert!(ga.report.is_empty());

    // RGBA 8: alfa 0 con RGB (10, 20, 30) intacto.
    let c = lee(&Png::new(2, 1, 8, 6)
        .swatches(&[10, 20, 30, 0, 40, 50, 60, 255])
        .bytes());
    assert_eq!(c.image.mode(), Mode::Rgba);
    assert_eq!(
        pixels(&c.image),
        [rgba(10, 20, 30, 0), rgba(40, 50, 60, 255)]
    );
    assert!(c.report.is_empty());
}

#[test]
fn gray_of_1_2_and_4_bits_scales_to_0_255() {
    // 1 bit: 0 y 255.
    let r1 = lee(&Png::new(2, 1, 1, 0).swatches(&[0, 1]).bytes());
    assert_eq!(r1.image.mode(), Mode::Gray);
    assert_eq!(pixels(&r1.image), grays(&[0, 255]));
    assert_eq!(r1.report, [Conversion::DepthExpanded { from_bits: 1 }]);

    // 2 bits: 0, 85, 170, 255.
    let r2 = lee(&Png::new(4, 1, 2, 0).swatches(&[0, 1, 2, 3]).bytes());
    assert_eq!(pixels(&r2.image), grays(&[0, 85, 170, 255]));
    assert_eq!(r2.report, [Conversion::DepthExpanded { from_bits: 2 }]);

    // 4 bits: v * 17 (0, 17, 34, ..., 255).
    let all: Vec<u16> = (0..16).collect();
    let r4 = lee(&Png::new(16, 1, 4, 0).swatches(&all).bytes());
    let expected_2_2: Vec<u8> = (0..16u8).map(|v| v * 17).collect();
    assert_eq!(expected_2_2[15], 255);
    assert_eq!(pixels(&r4.image), grays(&expected_2_2));
    assert_eq!(r4.report, [Conversion::DepthExpanded { from_bits: 4 }]);
}

#[test]
fn sixteen_bits_reduce_with_rounding() {
    let swatches: [u16; 5] = [0x0000, 0x0080, 0x0081, 0x01FF, 0xFFFF];

    // Gris 16: 0x0080 -> 0, 0x0081 -> 1, 0x01FF -> 2 (el byte alto daría 1), 0xFFFF -> 255.
    let g = lee(&Png::new(5, 1, 16, 0).swatches(&swatches).bytes());
    assert_eq!(g.image.mode(), Mode::Gray);
    assert_eq!(pixels(&g.image), grays(&[0, 0, 1, 2, 255]));
    assert_eq!(g.report, [Conversion::Depth16Reduced]);

    // Gris con alfa 16: cada muestra, gris y alfa, con la misma regla.
    let ga = lee(&Png::new(3, 1, 16, 4)
        .swatches(&[0x0000, 0x0080, 0x0081, 0x01FF, 0xFFFF, 0x0000])
        .bytes());
    assert_eq!(ga.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixels(&ga.image), grays_alpha(&[(0, 0), (1, 2), (255, 0)]));
    assert_eq!(ga.report, [Conversion::Depth16Reduced]);

    // RGBA 16: 0x0100 y 0x0101 y 0x00FF dan 1.
    let c = lee(&Png::new(2, 1, 16, 6)
        .swatches(&[
            0x0000, 0x0080, 0x0081, 0x01FF, 0xFFFF, 0x0100, 0x0101, 0x00FF,
        ])
        .bytes());
    assert_eq!(c.image.mode(), Mode::Rgba);
    assert_eq!(pixels(&c.image), [rgba(0, 0, 1, 2), rgba(255, 1, 1, 1)]);
    assert_eq!(c.report, [Conversion::Depth16Reduced]);
}

#[test]
fn rgb_becomes_opaque_rgba() {
    let r8 = lee(&Png::new(1, 1, 8, 2).swatches(&[1, 2, 3]).bytes());
    assert_eq!(r8.image.mode(), Mode::Rgba);
    assert_eq!(pixels(&r8.image), [rgba(1, 2, 3, 255)]);
    assert_eq!(r8.report, [Conversion::RgbToRgba]);

    let r16 = lee(&Png::new(1, 1, 16, 2)
        .swatches(&[0x0102, 0x0304, 0xFFFF])
        .bytes());
    assert_eq!(r16.image.mode(), Mode::Rgba);
    assert_eq!(pixels(&r16.image), [rgba(1, 3, 255, 255)]);
    assert_eq!(
        r16.report,
        [Conversion::Depth16Reduced, Conversion::RgbToRgba]
    );
}

#[test]
fn gray_with_color_key_gives_gray_alpha_comparing_the_original_sample() {
    // Gris 8, clave 7.
    let r8 = lee(&Png::new(2, 1, 8, 0)
        .swatches(&[7, 8])
        .before(trns(&[0, 7]))
        .bytes());
    assert_eq!(r8.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixels(&r8.image), grays_alpha(&[(7, 0), (8, 255)]));
    assert_eq!(r8.report, [Conversion::ColorKeyToAlpha]);

    // Gris 2 bits, clave 1: 1 -> 85 transparente, 2 -> 170 opaco.
    let r2 = lee(&Png::new(2, 1, 2, 0)
        .swatches(&[1, 2])
        .before(trns(&[0, 1]))
        .bytes());
    assert_eq!(r2.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixels(&r2.image), grays_alpha(&[(85, 0), (170, 255)]));
    assert_eq!(
        r2.report,
        [
            Conversion::DepthExpanded { from_bits: 2 },
            Conversion::ColorKeyToAlpha
        ]
    );

    // Gris 16, clave 0x0100: 0x0100 y 0x0101 se reducen a 1, pero solo la primera es la clave.
    let r16 = lee(&Png::new(2, 1, 16, 0)
        .swatches(&[0x0100, 0x0101])
        .before(trns(&[0x01, 0x00]))
        .bytes());
    assert_eq!(r16.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixels(&r16.image), grays_alpha(&[(1, 0), (1, 255)]));
    assert_eq!(
        r16.report,
        [Conversion::Depth16Reduced, Conversion::ColorKeyToAlpha]
    );
}

#[test]
fn rgb_with_color_key_keeps_the_rgb_and_gray_without_matches_is_opaque() {
    // tRNS de RGB: tres muestras de 2 bytes (1, 2, 3).
    let r = lee(&Png::new(2, 1, 8, 2)
        .swatches(&[1, 2, 3, 1, 2, 4])
        .before(trns(&[0, 1, 0, 2, 0, 3]))
        .bytes());
    assert_eq!(r.image.mode(), Mode::Rgba);
    assert_eq!(pixels(&r.image), [rgba(1, 2, 3, 0), rgba(1, 2, 4, 255)]);
    assert_eq!(
        r.report,
        [Conversion::RgbToRgba, Conversion::ColorKeyToAlpha]
    );

    // Gris con tRNS y ningún píxel igual a la clave: el modo lo decide el fichero.
    let g = lee(&Png::new(2, 1, 8, 0)
        .swatches(&[1, 2])
        .before(trns(&[0, 200]))
        .bytes());
    assert_eq!(g.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixels(&g.image), grays_alpha(&[(1, 255), (2, 255)]));
    assert_eq!(g.report, [Conversion::ColorKeyToAlpha]);
}

#[test]
fn malformed_trns_in_gray_and_rgb_and_ignored_in_types_with_alpha() {
    // Gris con tRNS de 1 byte.
    let short = Png::new(1, 1, 8, 0)
        .swatches(&[1])
        .before(trns(&[7]))
        .bytes();
    assert_eq!(error_de(&short), PngError::InvalidTrns);

    // RGB con tRNS de 4 bytes.
    let rgb4 = Png::new(1, 1, 8, 2)
        .swatches(&[1, 2, 3])
        .before(trns(&[0, 1, 0, 2]))
        .bytes();
    assert_eq!(error_de(&rgb4), PngError::InvalidTrns);

    // Gris de 2 bits con clave 9 (el máximo es 3).
    let outside = Png::new(1, 1, 2, 0)
        .swatches(&[1])
        .before(trns(&[0, 9]))
        .bytes();
    assert_eq!(error_de(&outside), PngError::InvalidTrns);

    // GrayAlpha y RGBA con tRNS: PNG lo prohíbe, se ignora.
    let ga = lee(&Png::new(1, 1, 8, 4)
        .swatches(&[5, 6])
        .before(trns(&[0, 5]))
        .bytes());
    assert_eq!(pixels(&ga.image), grays_alpha(&[(5, 6)]));
    assert!(ga.report.is_empty());
    let c = lee(&Png::new(1, 1, 8, 6)
        .swatches(&[1, 2, 3, 4])
        .before(trns(&[0, 1, 0, 2, 0, 3]))
        .bytes());
    assert_eq!(pixels(&c.image), [rgba(1, 2, 3, 4)]);
    assert!(c.report.is_empty());
}

#[test]
fn adam7_gives_the_same_image_as_non_interlaced() {
    let palette_16: Vec<(u8, u8, u8)> = (0..16u8).map(|i| (i, 255 - i, i * 3)).collect();
    // (tipo, profundidad, muestras por píxel, chunks previos al IDAT, modo esperado)
    let cases = [
        (0, 8, 1, vec![], Mode::Gray),
        (3, 4, 1, vec![plte(&palette_16)], Mode::Indexed),
        (6, 8, 4, vec![], Mode::Rgba),
    ];
    for (color, prof, n, previous, mode) in cases {
        let swatches: Vec<u16> = (0..64 * n as u16)
            .map(|i| match color {
                3 => (i * 7 + i / 3) % 16,
                _ => (i * 37 + 5) % 256,
            })
            .collect();
        let builds = |interlaced_2: bool| {
            let mut p = Png::new(8, 8, prof, color)
                .swatches(&swatches)
                .interlaced_2(interlaced_2);
            for c in &previous {
                p = p.before(c.clone());
            }
            lee(&p.bytes())
        };
        let plane = builds(false);
        let interlaced = builds(true);
        assert_eq!(plane.image.mode(), mode, "tipo {color}");
        assert_eq!(interlaced.image, plane.image, "tipo {color}");
        assert!(plane.report.is_empty(), "tipo {color}");
        assert!(interlaced.report.is_empty(), "tipo {color}");
        assert_eq!(pixels(&plane.image).len(), 64);
    }
}

/// Chunks auxiliares válidos que la lectura debe ignorar.
fn auxiliary() -> Vec<Vec<u8>> {
    let mut iccp = b"perfil\0".to_vec();
    iccp.push(0);
    iccp.extend(zlib_stored(b"datos del perfil", 0));
    let mut phys = Vec::new();
    phys.extend(2835u32.to_be_bytes());
    phys.extend(2835u32.to_be_bytes());
    phys.push(1);
    vec![
        piece(b"gAMA", &45455u32.to_be_bytes()),
        piece(b"sRGB", &[0]),
        piece(b"iCCP", &iccp),
        piece(b"pHYs", &phys),
        piece(b"tEXt", b"Title\0pixoko"),
        piece(b"tIME", &[0x07, 0xEA, 10, 17, 12, 0, 0]),
        piece(b"teSt", &[1, 2, 3]),
    ]
}

#[test]
fn auxiliary_chunks_change_neither_image_nor_report() {
    let base = Png::new(2, 1, 8, 2).swatches(&[1, 2, 3, 4, 5, 6]);
    let without = lee(&base.bytes());
    assert_eq!(without.report, [Conversion::RgbToRgba]);
    let mut with = base.clone();
    for c in auxiliary() {
        with = with.before(c);
    }
    let r = lee(&with.bytes());
    // Los auxiliares no cambian ni la imagen ni el informe de conversiones; solo `aux` los refleja.
    assert_eq!(r.image, without.image);
    assert_eq!(r.report, without.report);
    assert!(without.aux.found.is_empty());
    assert_eq!(
        r.aux.found,
        [
            AuxChunk::Icc,
            AuxChunk::Srgb,
            AuxChunk::Gamma,
            AuxChunk::Physical,
            AuxChunk::Text,
            AuxChunk::Time,
            AuxChunk::Other,
        ]
    );
    assert_eq!(pixels(&r.image), [rgba(1, 2, 3, 255), rgba(4, 5, 6, 255)]);
}

#[test]
fn apng_reads_as_its_default_image() {
    // fcTL: secuencia, ancho, alto, x, y, retardo (num y den), dispose y blend.
    let fctl = |sequence: u32| {
        let mut d = Vec::new();
        d.extend(sequence.to_be_bytes());
        d.extend(2u32.to_be_bytes());
        d.extend(1u32.to_be_bytes());
        d.extend([0u8; 8]);
        d.extend([0, 1, 0, 10, 0, 0]);
        d
    };
    let mut fdat = 2u32.to_be_bytes().to_vec();
    fdat.extend(zlib_stored(&[0, 30, 40], 0));
    let png = Png::new(2, 1, 8, 0)
        .swatches(&[10, 20])
        .before(piece(b"acTL", &[0, 0, 0, 2, 0, 0, 0, 0]))
        .before(piece(b"fcTL", &fctl(0)))
        .after(piece(b"fcTL", &fctl(1)))
        .after(piece(b"fdAT", &fdat))
        .bytes();
    let r = lee(&png);
    assert_eq!(r.image.mode(), Mode::Gray);
    assert_eq!(pixels(&r.image), grays(&[10, 20]));
    assert!(r.report.is_empty());
}

#[test]
fn unknown_critical_chunk_is_decode() {
    let png = Png::new(1, 1, 8, 0)
        .swatches(&[5])
        .before(piece(b"TeSt", &[1]))
        .bytes();
    let r = read_png(&png);
    assert!(es_decode(&r), "{r:?}");
}

#[test]
fn corrupt_files_are_decode_without_panic() {
    let gray16x16: Vec<u16> = (0..256).collect();
    let base = Png::new(16, 16, 8, 0).swatches(&gray16x16);
    let valid = base.bytes();
    let (idat, length_idat) = position_idat(&valid);

    let mut cases: Vec<(&str, Vec<u8>)> = vec![("vacío", Vec::new())];

    let mut signature = valid.clone();
    signature[0] = 0x88;
    cases.push(("firma incorrecta", signature));

    cases.push((
        "truncado a mitad de IDAT",
        valid[..idat + 8 + length_idat / 2].to_vec(),
    ));
    cases.push(("sin IEND", valid[..valid.len() - 12].to_vec()));

    // CRC de IHDR: bytes 29..33 (8 de firma + 4 de longitud + 4 de tipo + 13 de datos).
    let mut crc_ihdr = valid.clone();
    crc_ihdr[32] ^= 0xFF;
    cases.push(("CRC erróneo en IHDR", crc_ihdr));

    let mut crc_idat = valid.clone();
    crc_idat[idat + 8 + length_idat + 3] ^= 0xFF;
    cases.push(("CRC erróneo en IDAT", crc_idat));

    // Adler-32 erróneo, pero con el CRC del chunk recalculado (lo hace `piece`).
    let mut adler = base.clone();
    adler.adler_xor = 0x0001_0000;
    cases.push(("Adler-32 erróneo", adler.bytes()));

    let mut filter = base.clone();
    let mut raw = filter.raw();
    raw[0] = 9;
    filter.raw_fixed = Some(raw);
    cases.push(("filtro de fila 9", filter.bytes()));

    let mut rgb4 = Png::new(1, 1, 4, 2);
    rgb4.raw_fixed = Some(vec![0, 0, 0]);
    cases.push(("RGB a 4 bits", rgb4.bytes()));

    let mut width_0 = Png::new(0, 1, 8, 0);
    width_0.raw_fixed = Some(vec![0]);
    cases.push(("ancho 0", width_0.bytes()));

    for (name_2, bytes) in cases {
        let r = read_png(&bytes);
        assert!(es_decode(&r), "{name_2}: {r:?}");
    }
}

#[test]
fn garbage_after_iend_is_trailing_data_and_incomplete_iend_is_decode() {
    let valid = Png::new(2, 1, 8, 0).swatches(&[1, 2]).bytes();
    assert!(read_png(&valid).is_ok());

    // Un byte de más.
    let mut with_x = valid.clone();
    with_x.extend(b"x");
    assert_eq!(error_de(&with_x), PngError::TrailingData);

    // Un chunk tEXt completo después de IEND.
    let mut with_text = valid.clone();
    with_text.extend(piece(b"tEXt", b"k\0v"));
    assert_eq!(error_de(&with_text), PngError::TrailingData);

    // Sin los últimos 1 a 11 bytes: IEND incompleto.
    for remove in 1..=11 {
        let short = &valid[..valid.len() - remove];
        let r = read_png(short);
        assert!(es_decode(&r), "sin los últimos {remove} bytes: {r:?}");
    }
}

#[test]
fn huge_header_with_minimal_data_is_too_large_or_decode() {
    // Gris 8 de 32769 x 32768: 1 byte por encima de `Image::MAX_BYTES`.
    let almost = Png::new(32769, 32768, 8, 0).bytes();
    assert_eq!(
        error_de(&almost),
        PngError::TooLarge {
            width: 32769,
            height: 32768
        }
    );

    // Gris 8 de 100000 x 100000.
    let huge = Png::new(100_000, 100_000, 8, 0).bytes();
    assert_eq!(
        error_de(&huge),
        PngError::TooLarge {
            width: 100_000,
            height: 100_000
        }
    );

    // RGBA 16 de 2 000 000 000 x 1: lo rechazan los límites del crate (Decode).
    let row_giant = Png::new(2_000_000_000, 1, 16, 6).bytes();
    let r = read_png(&row_giant);
    assert!(es_decode(&r), "{r:?}");
}

#[test]
fn rgb16_whose_file_buffer_does_not_fit_although_the_image_does() {
    // 14000 x 14000: la imagen Rgba son 784 000 000 bytes (cabe en 2^30) y el búfer de
    // muestras del fichero, con 6 bytes por píxel, 1 176 000 000 (no cabe). Cada fila mide
    // 84 000 bytes, muy por debajo del límite del crate.
    let png = Png::new(14_000, 14_000, 16, 2).bytes();
    assert_eq!(
        error_de(&png),
        PngError::TooLarge {
            width: 14_000,
            height: 14_000
        }
    );
}

#[test]
fn writes_indexed_with_plte_and_trns_per_behavior_11() {
    let c = |r, g, b, a| Rgba::new(r, g, b, a);

    // Todo opaco: PLTE de 6 bytes y ninguna tRNS.
    let opaque_2 = indexed(&[c(1, 2, 3, 255), c(4, 5, 6, 255)], None, &[0, 1]);
    let bytes = write_png(&opaque_2).unwrap();
    let pieces = walks(&bytes).unwrap();
    let ihdr = &pieces[0];
    assert_eq!(ihdr.kind, *b"IHDR");
    assert_eq!(&ihdr.data[..4], &2u32.to_be_bytes());
    assert_eq!(&ihdr.data[4..8], &1u32.to_be_bytes());
    // Profundidad 8, tipo 3, compresión 0, filtro 0, sin entrelazado.
    assert_eq!(&ihdr.data[8..], &[8, 3, 0, 0, 0]);
    let types: Vec<[u8; 4]> = pieces.iter().map(|t| t.kind).collect();
    assert_eq!(types, [*b"IHDR", *b"PLTE", *b"IDAT", *b"IEND"]);
    assert_eq!(pieces[1].data, [1, 2, 3, 4, 5, 6]);
    assert_eq!(lee(&bytes).image, opaque_2);

    // Alfas [0, 255]: tRNS [0] (se omiten los 255 finales).
    let a = indexed(&[c(1, 2, 3, 0), c(4, 5, 6, 255)], None, &[0, 1]);
    let bytes = write_png(&a).unwrap();
    let pieces = walks(&bytes).unwrap();
    let types: Vec<[u8; 4]> = pieces.iter().map(|t| t.kind).collect();
    assert_eq!(types, [*b"IHDR", *b"PLTE", *b"tRNS", *b"IDAT", *b"IEND"]);
    assert_eq!(pieces[1].data.len(), 6);
    assert_eq!(pieces[2].data, [0]);
    assert_eq!(lee(&bytes).image, a);

    // Alfas [255, 128]: tRNS [255, 128].
    let b = indexed(&[c(1, 2, 3, 255), c(4, 5, 6, 128)], None, &[0, 1]);
    let bytes = write_png(&b).unwrap();
    let pieces = walks(&bytes).unwrap();
    assert_eq!(pieces[2].kind, *b"tRNS");
    assert_eq!(pieces[2].data, [255, 128]);
    assert_eq!(lee(&bytes).image, b);

    // transparent_index = Some(1) con alfas guardados 255: tRNS [255, 0]; al leerlo, la
    // entrada 1 sale con a = 0 y sin índice transparente.
    let t = indexed(&[c(1, 2, 3, 255), c(4, 5, 6, 255)], Some(1), &[0, 1]);
    let bytes = write_png(&t).unwrap();
    let pieces = walks(&bytes).unwrap();
    assert_eq!(pieces[2].kind, *b"tRNS");
    assert_eq!(pieces[2].data, [255, 0]);
    let r = lee(&bytes);
    let palette = r.image.palette().unwrap();
    assert_eq!(palette.colors(), [c(1, 2, 3, 255), c(4, 5, 6, 0)]);
    assert_eq!(palette.transparent_index(), None);
    assert_eq!(pixels(&r.image), indices(&[0, 1]));
    assert!(r.report.is_empty());
}

#[test]
fn writes_gray_gray_alpha_and_rgba_as_is() {
    let cases = [
        (Mode::Gray, Pixel::Gray(77), 0u8),
        (Mode::GrayAlpha, Pixel::GrayAlpha { gray: 77, alpha: 0 }, 4),
        (Mode::Rgba, rgba(10, 20, 30, 0), 6),
    ];
    for (mode, pixel, kind) in cases {
        let img = image_1x1(mode, pixel);
        let bytes = write_png(&img).unwrap();
        let pieces = walks(&bytes).unwrap();
        let types: Vec<[u8; 4]> = pieces.iter().map(|t| t.kind).collect();
        assert_eq!(types, [*b"IHDR", *b"IDAT", *b"IEND"], "{mode:?}");
        assert_eq!(&pieces[0].data[8..], &[8, kind, 0, 0, 0], "{mode:?}");
        let r = lee(&bytes);
        assert_eq!(r.image, img, "{mode:?}");
        assert!(r.report.is_empty(), "{mode:?}");
    }
}

// ---------------------------------------------------------------------------------------
// Propiedades.
// ---------------------------------------------------------------------------------------

fn rgba_arbitrary() -> impl Strategy<Value = Rgba> {
    (
        any::<u8>(),
        any::<u8>(),
        any::<u8>(),
        // Con frecuencia a = 0, con RGB casi siempre distinto de 0.
        prop_oneof![Just(0u8), any::<u8>()],
    )
        .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
}

/// Rellena una imagen con los píxeles dados, fila a fila.
fn fills(mut img: Image, px: impl IntoIterator<Item = Pixel>) -> Image {
    let w = img.width();
    for (i, p) in px.into_iter().enumerate() {
        img.set_pixel(i as u32 % w, i as u32 / w, p).unwrap();
    }
    img
}

/// Imagen de 1 a 8 de lado en cualquiera de los cuatro modos.
fn image_arbitrary() -> BoxedStrategy<Image> {
    (1u32..=8, 1u32..=8)
        .prop_flat_map(|(w, h)| {
            let n = (w * h) as usize;
            prop_oneof![
                vec(rgba_arbitrary(), n).prop_map(move |px| fills(
                    Image::new(w, h, Mode::Rgba).unwrap(),
                    px.into_iter().map(Pixel::Rgba)
                )),
                vec(any::<u8>(), n).prop_map(move |px| fills(
                    Image::new(w, h, Mode::Gray).unwrap(),
                    px.into_iter().map(Pixel::Gray)
                )),
                vec(any::<(u8, u8)>(), n).prop_map(move |px| fills(
                    Image::new(w, h, Mode::GrayAlpha).unwrap(),
                    px.into_iter()
                        .map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
                )),
                (1usize..=256)
                    .prop_flat_map(move |len| {
                        (
                            vec(rgba_arbitrary(), len),
                            proptest::option::of(0..len),
                            vec(0..len, n),
                        )
                    })
                    .prop_map(move |(colors, transparent, px)| {
                        let mut p = Palette::from_colors(colors).unwrap();
                        p.set_transparent_index(transparent.map(|t| t as u8))
                            .unwrap();
                        fills(
                            Image::new_indexed(w, h, p).unwrap(),
                            px.into_iter().map(|i| Pixel::Indexed(i as u8)),
                        )
                    }),
            ]
        })
        .boxed()
}

/// `img`, salvo en un indexado con índice transparente, cuya paleta sale ya resuelta.
fn normalized(img: &Image) -> Image {
    let Some(p) = img.palette() else {
        return img.clone();
    };
    if p.transparent_index().is_none() {
        return img.clone();
    }
    let colors: Vec<Rgba> = (0..p.len()).map(|i| p.resolved(i as u8).unwrap()).collect();
    let copy = Image::new_indexed(
        img.width(),
        img.height(),
        Palette::from_colors(colors).unwrap(),
    )
    .unwrap();
    fills(copy, pixels(img))
}

/// Invariantes de `Image` observables desde la API pública.
fn checks_invariants(img: &Image) -> Result<(), TestCaseError> {
    prop_assert!(img.width() >= 1 && img.height() >= 1);
    let all = pixels(img);
    prop_assert_eq!(
        all.len() as u64,
        u64::from(img.width()) * u64::from(img.height())
    );
    match img.mode() {
        Mode::Indexed => {
            let palette = img.palette();
            prop_assert!(palette.is_some());
            let palette = palette.unwrap();
            prop_assert!(!palette.is_empty());
            if let Some(t) = palette.transparent_index() {
                prop_assert!(usize::from(t) < palette.len());
            }
            for p in all {
                match p {
                    Pixel::Indexed(i) => prop_assert!(usize::from(i) < palette.len()),
                    other_2 => {
                        return Err(TestCaseError::fail(format!(
                            "píxel {other_2:?} en una imagen indexada"
                        )));
                    }
                }
            }
        }
        mode => {
            prop_assert!(img.palette().is_none());
            for p in all {
                let matches_2 = matches!(
                    (mode, p),
                    (Mode::Rgba, Pixel::Rgba(_))
                        | (Mode::Gray, Pixel::Gray(_))
                        | (Mode::GrayAlpha, Pixel::GrayAlpha { .. })
                );
                prop_assert!(matches_2, "píxel {:?} en modo {:?}", p, mode);
            }
        }
    }
    Ok(())
}

fn checks_result(r: Result<PngRead, PngError>) -> Result<(), TestCaseError> {
    match r {
        Ok(read) => checks_invariants(&read.image),
        Err(_) => Ok(()),
    }
}

proptest! {
    // Propiedad 1: ida y vuelta en los cuatro modos.
    #[test]
    fn p1_round_trip_in_the_four_modes(img in image_arbitrary()) {
        let bytes = write_png(&img).unwrap();
        let r = read_png(&bytes).unwrap();
        prop_assert!(r.report.is_empty());
        prop_assert!(r.is_exact());
        prop_assert_eq!(r.image, normalized(&img));
    }
}

/// «Casi PNG»: firma e IHDR con CRC válido y valores cualesquiera, IDAT mínimo e IEND.
///
/// Para que ningún caso reserve más de unos pocos KiB, cada dimensión es pequeña (0 a 16) o
/// al menos 2^31 (con la otra distinta de cero, el tamaño supera `Image::MAX_BYTES` en
/// cualquier modo; con cero, es dimensión cero). Es la misma idea que los casos límite de
/// tamaño: rechazo por cabecera enorme con datos mínimos.
fn almost_png() -> impl Strategy<Value = Vec<u8>> {
    let dimension = prop_oneof![3 => 0u32..=16, 1 => (1u32 << 31)..=u32::MAX];
    let prof = prop_oneof![select(vec![1u8, 2, 4, 8, 16]), any::<u8>()];
    let color = prop_oneof![select(vec![0u8, 2, 3, 4, 6]), any::<u8>()];
    (dimension.clone(), dimension, color, prof).prop_map(|(width, height, color, prof)| {
        let mut ihdr = Vec::new();
        ihdr.extend(width.to_be_bytes());
        ihdr.extend(height.to_be_bytes());
        ihdr.extend([prof, color, 0, 0, 0]);
        let mut v = TEST_SIGNATURE.to_vec();
        v.extend(piece(b"IHDR", &ihdr));
        v.extend(piece(b"IDAT", &zlib_stored(&[], 0)));
        v.extend(piece(b"IEND", &[]));
        v
    })
}

proptest! {
    // Propiedad 2a: bytes arbitrarios (la mitad, con la firma delante).
    #[test]
    fn p2_arbitrary_bytes_never_panic(
        bytes in prop_oneof![
            vec(any::<u8>(), 0..=512),
            vec(any::<u8>(), 0..=504).prop_map(|b| [TEST_SIGNATURE.to_vec(), b].concat()),
        ]
    ) {
        checks_result(read_png(&bytes))?;
    }

    // Propiedad 2b: «casi PNG».
    #[test]
    fn p2_almost_png_never_panics(bytes in almost_png()) {
        checks_result(read_png(&bytes))?;
    }

    // Propiedad 2c: un PNG válido al que se cambian 1 a 4 bytes.
    #[test]
    fn p2_valid_png_with_changed_bytes_never_panics(
        img in image_arbitrary(),
        changes in vec((any::<usize>(), any::<u8>()), 1..=4),
    ) {
        let mut bytes = write_png(&img).unwrap();
        for (location, new_2) in changes {
            let i = location % bytes.len();
            bytes[i] = new_2;
        }
        checks_result(read_png(&bytes))?;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    // Propiedad 3: truncado y basura final.
    #[test]
    fn p3_truncated_and_trailing_garbage(
        img in image_arbitrary(),
        extra in vec(any::<u8>(), 1..=16),
    ) {
        let bytes = write_png(&img).unwrap();
        for length in 0..bytes.len() {
            prop_assert!(
                read_png(&bytes[..length]).is_err(),
                "el prefijo de {} bytes (de {}) dio Ok", length, bytes.len()
            );
        }
        let mut with_garbage = bytes.clone();
        with_garbage.extend(&extra);
        prop_assert_eq!(read_png(&with_garbage).unwrap_err(), PngError::TrailingData);
    }
}

#[derive(Clone, Debug)]
struct CaseIndexed {
    prof: u8,
    width: usize,
    height: usize,
    interlaced_2: bool,
    colors: Vec<(u8, u8, u8)>,
    alphas: Vec<u8>,
    px: Vec<u16>,
}

fn case_indexed() -> impl Strategy<Value = CaseIndexed> {
    (
        select(vec![1u8, 2, 4, 8]),
        1usize..=12,
        1usize..=12,
        any::<bool>(),
    )
        .prop_flat_map(|(prof, width, height, interlaced_2)| {
            (1usize..=(1usize << prof)).prop_flat_map(move |n| {
                (
                    vec(any::<(u8, u8, u8)>(), n),
                    vec(any::<u8>(), 0..=n),
                    vec(0..n as u16, width * height),
                )
                    .prop_map(move |(colors, alphas, px)| CaseIndexed {
                        prof,
                        width,
                        height,
                        interlaced_2,
                        colors,
                        alphas,
                        px,
                    })
            })
        })
}

proptest! {
    // Propiedad 4: indexado intacto, con y sin Adam7.
    #[test]
    fn p4_indexed_intact(case in case_indexed()) {
        let mut png = Png::new(case.width as u32, case.height as u32, case.prof, 3)
            .swatches(&case.px)
            .interlaced_2(case.interlaced_2)
            .before(plte(&case.colors));
        if !case.alphas.is_empty() {
            png = png.before(trns(&case.alphas));
        }
        let r = read_png(&png.bytes()).unwrap();
        prop_assert!(r.report.is_empty());
        prop_assert_eq!(r.image.mode(), Mode::Indexed);
        prop_assert_eq!(
            (r.image.width() as usize, r.image.height() as usize),
            (case.width, case.height)
        );
        let expected_2_2: Vec<Pixel> = case.px.iter().map(|&i| Pixel::Indexed(i as u8)).collect();
        prop_assert_eq!(pixels(&r.image), expected_2_2);
        let palette = r.image.palette().unwrap();
        prop_assert_eq!(palette.transparent_index(), None);
        prop_assert_eq!(palette.len(), case.colors.len());
        for (i, &(rr, gg, bb)) in case.colors.iter().enumerate() {
            let a = case.alphas.get(i).copied().unwrap_or(255);
            prop_assert_eq!(palette.colors()[i], Rgba::new(rr, gg, bb, a), "entrada {}", i);
        }
    }
}

/// Combinaciones (tipo de color, profundidad) de la propiedad 5.
const COMBINATIONS: [(u8, u8); 11] = [
    (0, 1),
    (0, 2),
    (0, 4),
    (0, 8),
    (0, 16),
    (4, 8),
    (4, 16),
    (2, 8),
    (2, 16),
    (6, 8),
    (6, 16),
];

#[derive(Clone, Debug)]
struct CaseConversion {
    color: u8,
    prof: u8,
    width: usize,
    height: usize,
    interlaced_2: bool,
    /// Muestras originales, píxel a píxel y canal a canal.
    swatches: Vec<u16>,
    /// Color clave (1 muestra en gris, 3 en RGB), solo con tRNS de gris o RGB.
    key: Option<Vec<u16>>,
}

/// Muestra válida para la profundidad; a 16 bits, con valores cercanos entre sí para que
/// haya diferencias en el byte bajo y reducciones que coincidan.
fn swatch(prof: u8) -> BoxedStrategy<u16> {
    if prof == 16 {
        prop_oneof![
            any::<u16>(),
            any::<u8>().prop_map(|b| u16::from(b) * 257),
            (0u16..=255, 0u16..=3).prop_map(|(height, under)| (height << 8) | under),
        ]
        .boxed()
    } else {
        (0u16..(1u16 << prof)).boxed()
    }
}

fn case_conversion() -> impl Strategy<Value = CaseConversion> {
    (
        select(COMBINATIONS.to_vec()),
        1usize..=8,
        1usize..=8,
        any::<bool>(),
    )
        .prop_flat_map(|((color, prof), width, height, interlaced_2)| {
            let per_pixel = channels(color);
            let n_key = if color == 0 { 1 } else { 3 };
            let key = if color == 0 || color == 2 {
                proptest::option::of((any::<bool>(), 0..width * height, vec(swatch(prof), 3)))
                    .boxed()
            } else {
                Just(None).boxed()
            };
            (vec(swatch(prof), width * height * per_pixel), key).prop_map(move |(swatches, key)| {
                // La clave, a veces, es la de un píxel que existe.
                let key = key.map(|(existing, p, other_2_2)| {
                    if existing {
                        swatches[p * per_pixel..p * per_pixel + n_key].to_vec()
                    } else {
                        other_2_2[..n_key].to_vec()
                    }
                });
                CaseConversion {
                    color,
                    prof,
                    width,
                    height,
                    interlaced_2,
                    swatches,
                    key,
                }
            })
        })
}

/// De 16 u 8 bits, o de `n < 8` bits de gris, a 8 bits: el redondeo al más cercano de
/// `v * 255 / (2^n - 1)`, calculado con coma flotante (independiente de la fórmula entera
/// `(v + 128) / 257` de la spec).
fn a_8_bits(v: u16, prof: u8) -> u8 {
    let maximum = f64::from((1u32 << prof) - 1);
    (f64::from(v) * 255.0 / maximum).round() as u8
}

/// Resultado esperado según la tabla del comportamiento 2.
fn model(c: &CaseConversion) -> (Mode, Vec<Pixel>, Vec<Conversion>) {
    let per_pixel = channels(c.color);
    let mut report = Vec::new();
    if c.color == 0 && c.prof < 8 {
        report.push(Conversion::DepthExpanded { from_bits: c.prof });
    }
    if c.prof == 16 {
        report.push(Conversion::Depth16Reduced);
    }
    if c.color == 2 {
        report.push(Conversion::RgbToRgba);
    }
    if c.key.is_some() {
        report.push(Conversion::ColorKeyToAlpha);
    }

    let pixels: Vec<Pixel> = c
        .swatches
        .chunks(per_pixel)
        .map(|m| {
            // La clave se compara con las muestras originales, antes de escalar.
            let is_key = c.key.as_deref() == Some(&m[..m.len().min(3)]);
            let a = |i: usize| a_8_bits(m[i], c.prof);
            let alpha_key = if is_key { 0 } else { 255 };
            match (c.color, c.key.is_some()) {
                (0, false) => Pixel::Gray(a(0)),
                (0, true) => Pixel::GrayAlpha {
                    gray: a(0),
                    alpha: alpha_key,
                },
                (2, _) => rgba(
                    a(0),
                    a(1),
                    a(2),
                    if c.key.is_some() { alpha_key } else { 255 },
                ),
                (4, _) => Pixel::GrayAlpha {
                    gray: a(0),
                    alpha: a(1),
                },
                (6, _) => rgba(a(0), a(1), a(2), a(3)),
                other_2 => unreachable!("{other_2:?}"),
            }
        })
        .collect();

    let mode = match (c.color, c.key.is_some()) {
        (0, false) => Mode::Gray,
        (0, true) | (4, _) => Mode::GrayAlpha,
        _ => Mode::Rgba,
    };
    (mode, pixels, report)
}

proptest! {
    // Propiedad 5: conversiones contra un modelo independiente.
    #[test]
    fn p5_conversions_against_a_model(case in case_conversion()) {
        let mut png = Png::new(case.width as u32, case.height as u32, case.prof, case.color)
            .swatches(&case.swatches)
            .interlaced_2(case.interlaced_2);
        if let Some(key) = &case.key {
            let data: Vec<u8> = key.iter().flat_map(|v| v.to_be_bytes()).collect();
            png = png.before(trns(&data));
        }
        let r = read_png(&png.bytes()).unwrap();
        let (mode, expected_2_2, report) = model(&case);
        prop_assert_eq!(r.image.mode(), mode);
        prop_assert_eq!(
            (r.image.width() as usize, r.image.height() as usize),
            (case.width, case.height)
        );
        prop_assert_eq!(pixels(&r.image), expected_2_2);
        prop_assert_eq!(r.report, report);
    }
}

proptest! {
    // Propiedad 6: forma del fichero escrito.
    #[test]
    fn p6_shape_of_the_written_file(img in image_arbitrary()) {
        let bytes = write_png(&img).unwrap();
        let pieces = walks(&bytes).map_err(TestCaseError::fail)?;
        let types: Vec<[u8; 4]> = pieces.iter().map(|t| t.kind).collect();

        // Secuencia: IHDR, [PLTE, [tRNS]], IDAT+, IEND.
        let alphas: Vec<u8> = img
            .palette()
            .map(|p| (0..p.len()).map(|i| p.resolved(i as u8).unwrap().a).collect())
            .unwrap_or_default();
        let length_trns = alphas.iter().rposition(|&a| a != 255).map_or(0, |i| i + 1);
        let mut expected_2_2: Vec<[u8; 4]> = vec![*b"IHDR"];
        if img.mode() == Mode::Indexed {
            expected_2_2.push(*b"PLTE");
            if length_trns > 0 {
                expected_2_2.push(*b"tRNS");
            }
        }
        expected_2_2.push(*b"IDAT");
        expected_2_2.push(*b"IEND");

        let n_idat = types.iter().filter(|t| **t == *b"IDAT").count();
        prop_assert!(n_idat >= 1);
        let first = types.iter().position(|t| *t == *b"IDAT").unwrap();
        prop_assert!(types[first..first + n_idat].iter().all(|t| *t == *b"IDAT"));
        let collapsed: Vec<[u8; 4]> = types[..first]
            .iter()
            .chain(std::iter::once(&types[first]))
            .chain(&types[first + n_idat..])
            .copied()
            .collect();
        prop_assert_eq!(collapsed, expected_2_2);

        // IHDR: tamaño, 8 bits, tipo de color del modo y sin entrelazado.
        let type_color = match img.mode() {
            Mode::Indexed => 3u8,
            Mode::Gray => 0,
            Mode::GrayAlpha => 4,
            Mode::Rgba => 6,
        };
        let ihdr = &pieces[0].data;
        prop_assert_eq!(ihdr.len(), 13);
        prop_assert_eq!(&ihdr[..4], &img.width().to_be_bytes());
        prop_assert_eq!(&ihdr[4..8], &img.height().to_be_bytes());
        prop_assert_eq!(&ihdr[8..], &[8, type_color, 0, 0, 0]);

        // PLTE con 3 * len bytes y tRNS con la longitud del comportamiento 11.
        if let Some(palette) = img.palette() {
            let plte = pieces.iter().find(|t| t.kind == *b"PLTE").unwrap();
            prop_assert_eq!(plte.data.len(), 3 * palette.len());
            let rgb: Vec<u8> = palette.colors().iter().flat_map(|c| [c.r, c.g, c.b]).collect();
            prop_assert_eq!(&plte.data, &rgb);
            let trns = pieces.iter().find(|t| t.kind == *b"tRNS");
            match trns {
                Some(t) => prop_assert_eq!(&t.data[..], &alphas[..length_trns]),
                None => prop_assert_eq!(length_trns, 0),
            }
        }

        // Determinista.
        prop_assert_eq!(write_png(&img).unwrap(), bytes);
    }
}
