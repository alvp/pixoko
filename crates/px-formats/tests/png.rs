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

// ---------------------------------------------------------------------------------------
// Ayudante de corpus: CRC-32, Adler-32, deflate «stored», chunks y Adam7.
// ---------------------------------------------------------------------------------------

const FIRMA: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// CRC-32 de la especificación PNG (polinomio 0xEDB88320, bit a bit).
fn crc32(datos: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in datos {
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
fn adler32(datos: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in datos {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Flujo zlib con bloques deflate «stored» (sin comprimir). `adler_xor` estropea el Adler-32.
fn zlib_stored(datos: &[u8], adler_xor: u32) -> Vec<u8> {
    let mut v = vec![0x78, 0x01];
    if datos.is_empty() {
        v.extend([1, 0, 0, 0xFF, 0xFF]);
    } else {
        let bloques = datos.chunks(65535).count();
        for (i, bloque) in datos.chunks(65535).enumerate() {
            v.push(u8::from(i + 1 == bloques));
            let n = bloque.len() as u16;
            v.extend(n.to_le_bytes());
            v.extend((!n).to_le_bytes());
            v.extend(bloque);
        }
    }
    v.extend((adler32(datos) ^ adler_xor).to_be_bytes());
    v
}

/// Chunk completo: longitud, tipo, datos y CRC.
fn trozo(tipo: &[u8; 4], datos: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + datos.len());
    v.extend((datos.len() as u32).to_be_bytes());
    v.extend(tipo);
    v.extend(datos);
    let mut cuerpo = tipo.to_vec();
    cuerpo.extend(datos);
    v.extend(crc32(&cuerpo).to_be_bytes());
    v
}

fn plte(colores: &[(u8, u8, u8)]) -> Vec<u8> {
    let datos: Vec<u8> = colores.iter().flat_map(|&(r, g, b)| [r, g, b]).collect();
    trozo(b"PLTE", &datos)
}

fn trns(datos: &[u8]) -> Vec<u8> {
    trozo(b"tRNS", datos)
}

/// Canales por píxel de cada tipo de color PNG.
fn canales(color: u8) -> usize {
    match color {
        2 => 3,
        4 => 2,
        6 => 4,
        _ => 1,
    }
}

/// Empaqueta las muestras de una fila: sub-8 bits de más a menos significativo con relleno
/// a byte, 8 bits tal cual y 16 bits en big-endian.
fn empaqueta(fila: &[u16], prof: u8) -> Vec<u8> {
    match prof {
        16 => fila.iter().flat_map(|v| v.to_be_bytes()).collect(),
        8 => fila.iter().map(|&v| v as u8).collect(),
        _ => {
            let p = usize::from(prof);
            fila.chunks(8 / p)
                .map(|grupo| {
                    let mut byte = 0u8;
                    for (i, &v) in grupo.iter().enumerate() {
                        byte |= (v as u8) << (8 - p * (i + 1));
                    }
                    byte
                })
                .collect()
        }
    }
}

/// Filas (con su byte de filtro 0) de los píxeles en las columnas `xs` y filas `ys`.
fn pasada(
    m: &[u16],
    ancho: usize,
    canales: usize,
    prof: u8,
    xs: &[usize],
    ys: &[usize],
) -> Vec<u8> {
    let mut salida = Vec::new();
    for &y in ys {
        let fila: Vec<u16> = xs
            .iter()
            .flat_map(|&x| (0..canales).map(move |c| m[(y * ancho + x) * canales + c]))
            .collect();
        salida.push(0);
        salida.extend(empaqueta(&fila, prof));
    }
    salida
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

/// Datos sin comprimir del IDAT: las filas en orden o, con `entrelazado`, las pasadas Adam7
/// (las pasadas vacías no aportan ni un byte).
fn escaneos(
    ancho: usize,
    alto: usize,
    canales: usize,
    prof: u8,
    m: &[u16],
    entrelazado: bool,
) -> Vec<u8> {
    if !entrelazado {
        let xs: Vec<usize> = (0..ancho).collect();
        let ys: Vec<usize> = (0..alto).collect();
        return pasada(m, ancho, canales, prof, &xs, &ys);
    }
    let mut salida = Vec::new();
    for (x0, y0, dx, dy) in ADAM7 {
        let xs: Vec<usize> = (x0..ancho).step_by(dx).collect();
        let ys: Vec<usize> = (y0..alto).step_by(dy).collect();
        if !xs.is_empty() && !ys.is_empty() {
            salida.extend(pasada(m, ancho, canales, prof, &xs, &ys));
        }
    }
    salida
}

/// Fichero PNG en construcción. Con `muestras` vacío no se genera ningún dato de imagen
/// (así se hacen ficheros de cabecera enorme y datos mínimos).
#[derive(Clone)]
struct Png {
    ancho: u32,
    alto: u32,
    prof: u8,
    color: u8,
    entrelazado: bool,
    /// Muestras por píxel y canal, fila a fila, en el orden de la imagen final.
    muestras: Vec<u16>,
    /// Si es `Some`, datos sin comprimir del IDAT tal cual (anula `muestras`).
    crudo_fijo: Option<Vec<u8>>,
    /// Chunks ya serializados entre IHDR e IDAT.
    antes: Vec<Vec<u8>>,
    /// Chunks ya serializados entre IDAT e IEND.
    despues: Vec<Vec<u8>>,
    adler_xor: u32,
}

impl Png {
    fn new(ancho: u32, alto: u32, prof: u8, color: u8) -> Png {
        Png {
            ancho,
            alto,
            prof,
            color,
            entrelazado: false,
            muestras: Vec::new(),
            crudo_fijo: None,
            antes: Vec::new(),
            despues: Vec::new(),
            adler_xor: 0,
        }
    }

    fn muestras(mut self, m: &[u16]) -> Png {
        self.muestras = m.to_vec();
        self
    }

    fn entrelazado(mut self, si: bool) -> Png {
        self.entrelazado = si;
        self
    }

    fn antes(mut self, chunk: Vec<u8>) -> Png {
        self.antes.push(chunk);
        self
    }

    fn despues(mut self, chunk: Vec<u8>) -> Png {
        self.despues.push(chunk);
        self
    }

    fn crudo(&self) -> Vec<u8> {
        if let Some(c) = &self.crudo_fijo {
            return c.clone();
        }
        if self.muestras.is_empty() {
            return Vec::new();
        }
        escaneos(
            self.ancho as usize,
            self.alto as usize,
            canales(self.color),
            self.prof,
            &self.muestras,
            self.entrelazado,
        )
    }

    fn bytes(&self) -> Vec<u8> {
        let mut ihdr = Vec::new();
        ihdr.extend(self.ancho.to_be_bytes());
        ihdr.extend(self.alto.to_be_bytes());
        ihdr.extend([self.prof, self.color, 0, 0, u8::from(self.entrelazado)]);
        let mut v = FIRMA.to_vec();
        v.extend(trozo(b"IHDR", &ihdr));
        for c in &self.antes {
            v.extend(c);
        }
        v.extend(trozo(b"IDAT", &zlib_stored(&self.crudo(), self.adler_xor)));
        for c in &self.despues {
            v.extend(c);
        }
        v.extend(trozo(b"IEND", &[]));
        v
    }
}

/// Un chunk encontrado por el recorredor: tipo, posición del campo de longitud y datos.
struct Trozo {
    tipo: [u8; 4],
    inicio: usize,
    datos: Vec<u8>,
}

/// Recorre la estructura de chunks de un fichero bien formado: firma, longitud, tipo, datos
/// y CRC de cada chunk, hasta un IEND que termina exactamente en el último byte.
fn recorre(bytes: &[u8]) -> Result<Vec<Trozo>, String> {
    if bytes.len() < 8 || bytes[..8] != FIRMA {
        return Err("firma incorrecta".into());
    }
    let mut pos = 8usize;
    let mut trozos = Vec::new();
    loop {
        let cabecera = bytes
            .get(pos..pos + 8)
            .ok_or_else(|| format!("cabecera truncada en {pos}"))?;
        let largo = u32::from_be_bytes(cabecera[..4].try_into().unwrap()) as usize;
        let tipo: [u8; 4] = cabecera[4..8].try_into().unwrap();
        let datos = bytes
            .get(pos + 8..pos + 8 + largo)
            .ok_or_else(|| format!("datos truncados en {pos}"))?;
        let crc = bytes
            .get(pos + 8 + largo..pos + 12 + largo)
            .ok_or_else(|| format!("CRC truncado en {pos}"))?;
        let mut cuerpo = tipo.to_vec();
        cuerpo.extend(datos);
        if u32::from_be_bytes(crc.try_into().unwrap()) != crc32(&cuerpo) {
            return Err(format!("CRC incorrecto en {pos}"));
        }
        trozos.push(Trozo {
            tipo,
            inicio: pos,
            datos: datos.to_vec(),
        });
        pos += 12 + largo;
        if tipo == *b"IEND" {
            if pos != bytes.len() {
                return Err(format!("sobran {} bytes tras IEND", bytes.len() - pos));
            }
            return Ok(trozos);
        }
    }
}

/// Posición del chunk IDAT y longitud de sus datos.
fn posicion_idat(bytes: &[u8]) -> (usize, usize) {
    let t = recorre(bytes)
        .unwrap()
        .into_iter()
        .find(|t| t.tipo == *b"IDAT")
        .unwrap();
    (t.inicio, t.datos.len())
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
fn pixeles(img: &Image) -> Vec<Pixel> {
    let mut v = Vec::new();
    for y in 0..img.height() {
        for x in 0..img.width() {
            v.push(img.get_pixel(x, y).unwrap());
        }
    }
    v
}

fn grises(v: &[u8]) -> Vec<Pixel> {
    v.iter().map(|&g| Pixel::Gray(g)).collect()
}

fn grises_alfa(v: &[(u8, u8)]) -> Vec<Pixel> {
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

fn paleta_opaca(colores: &[(u8, u8, u8)]) -> Palette {
    Palette::from_colors(colores.iter().map(|&(r, g, b)| Rgba::opaque(r, g, b))).unwrap()
}

fn paleta_con_alfas(colores: &[(u8, u8, u8)], alfas: &[u8]) -> Palette {
    Palette::from_colors(
        colores
            .iter()
            .zip(alfas)
            .map(|(&(r, g, b), &a)| Rgba::new(r, g, b, a)),
    )
    .unwrap()
}

/// Imagen indexada de una fila con la paleta dada.
fn indexada(colores: &[Rgba], transparente: Option<u8>, px: &[u8]) -> Image {
    let mut p = Palette::from_colors(colores.iter().copied()).unwrap();
    p.set_transparent_index(transparente).unwrap();
    let mut img = Image::new_indexed(px.len() as u32, 1, p).unwrap();
    for (x, &i) in px.iter().enumerate() {
        img.set_pixel(x as u32, 0, Pixel::Indexed(i)).unwrap();
    }
    img
}

/// Imagen 1 x 1 de un modo no indexado con ese píxel.
fn imagen_1x1(mode: Mode, pixel: Pixel) -> Image {
    let mut img = Image::new(1, 1, mode).unwrap();
    img.set_pixel(0, 0, pixel).unwrap();
    img
}

// ---------------------------------------------------------------------------------------
// Comprobación del propio ayudante (el corpus depende de él).
// ---------------------------------------------------------------------------------------

#[test]
fn ayudante_crc32_y_adler32_con_valores_conocidos() {
    // CRC del chunk IEND vacío, conocido: 0xAE426082.
    assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    assert_eq!(adler32(b""), 1);
    assert_eq!(
        trozo(b"IEND", &[]),
        [0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]
    );
}

#[test]
fn ayudante_recorre_el_corpus_y_rechaza_lo_mal_formado() {
    let bueno = Png::new(2, 1, 8, 0).muestras(&[1, 2]).bytes();
    let tipos: Vec<[u8; 4]> = recorre(&bueno).unwrap().iter().map(|t| t.tipo).collect();
    assert_eq!(tipos, [*b"IHDR", *b"IDAT", *b"IEND"]);

    let mut con_basura = bueno.clone();
    con_basura.push(0);
    assert!(recorre(&con_basura).is_err());
    assert!(recorre(&bueno[..bueno.len() - 1]).is_err());
    let mut crc_roto = bueno.clone();
    crc_roto[32] ^= 0xFF;
    assert!(recorre(&crc_roto).is_err());
}

#[test]
fn ayudante_adam7_reparte_todos_los_pixeles_de_8x8() {
    // 8 x 8 en gris 8: 1 + 1 + 2 + 4 + 8 + 16 + 32 = 64 píxeles en siete pasadas.
    let m: Vec<u16> = (0..64).collect();
    let crudo = escaneos(8, 8, 1, 8, &m, true);
    // Filas por pasada: 1, 1, 1, 2, 2, 4, 4 (15 en total); cada una lleva su byte de filtro.
    assert_eq!(crudo.len(), 64 + 15);
    // Primera pasada: el píxel (0, 0); segunda: el (4, 0).
    assert_eq!(&crudo[..4], &[0, 0, 0, 4]);
}

// ---------------------------------------------------------------------------------------
// Traits e informe.
// ---------------------------------------------------------------------------------------

#[test]
fn traits_derivados() {
    assert_read_traits::<PngRead>();
    assert_conversion_traits::<Conversion>();
    assert_error_traits::<PngError>();
}

#[test]
fn is_exact_es_informe_vacio() {
    let exacta = lee(&Png::new(1, 1, 8, 0).muestras(&[9]).bytes());
    assert!(exacta.report.is_empty());
    assert!(exacta.is_exact());
    let convertida = lee(&Png::new(1, 1, 8, 2).muestras(&[1, 2, 3]).bytes());
    assert!(!convertida.report.is_empty());
    assert!(!convertida.is_exact());
}

// ---------------------------------------------------------------------------------------
// Una prueba por fila de la tabla de casos límite.
// ---------------------------------------------------------------------------------------

#[test]
fn indexado_intacto_a_1_2_4_y_8_bits() {
    let colores = [(1, 2, 3), (4, 5, 6), (7, 8, 9)];
    // A 1 bit solo caben 2 entradas (con 3 sería `InvalidPalette`), así que ahí la paleta
    // es de 2 y los índices son 0 y 1.
    let casos: [(u8, usize, [u16; 3]); 4] = [
        (1, 2, [0, 1, 1]),
        (2, 3, [0, 1, 2]),
        (4, 3, [0, 1, 2]),
        (8, 3, [0, 1, 2]),
    ];
    for (prof, n, px) in casos {
        let png = Png::new(3, 1, prof, 3)
            .muestras(&px)
            .antes(plte(&colores[..n]))
            .bytes();
        let r = lee(&png);
        assert_eq!(r.image.mode(), Mode::Indexed, "{prof} bits");
        assert_eq!((r.image.width(), r.image.height()), (3, 1));
        let esperados: Vec<u8> = px.iter().map(|&i| i as u8).collect();
        assert_eq!(pixeles(&r.image), indices(&esperados), "{prof} bits");
        let paleta = r.image.palette().unwrap();
        assert_eq!(paleta, &paleta_opaca(&colores[..n]), "{prof} bits");
        assert_eq!(paleta.len(), n);
        assert_eq!(paleta.transparent_index(), None);
        assert!(r.report.is_empty(), "{prof} bits");
    }
}

#[test]
fn indexado_con_trns_corto_completa_con_255() {
    let colores = [(1, 2, 3), (4, 5, 6), (7, 8, 9)];
    let png = Png::new(3, 1, 8, 3)
        .muestras(&[0, 1, 2])
        .antes(plte(&colores))
        .antes(trns(&[0, 128]))
        .bytes();
    let r = lee(&png);
    assert_eq!(r.image.mode(), Mode::Indexed);
    let paleta = r.image.palette().unwrap();
    let alfas: Vec<u8> = paleta.colors().iter().map(|c| c.a).collect();
    assert_eq!(alfas, [0, 128, 255]);
    assert_eq!(paleta, &paleta_con_alfas(&colores, &[0, 128, 255]));
    assert_eq!(paleta.transparent_index(), None);
    assert_eq!(pixeles(&r.image), indices(&[0, 1, 2]));
    assert!(r.report.is_empty());
}

#[test]
fn indexado_invalido_paleta_e_indices() {
    // Píxel 5 con PLTE de 2.
    let fuera = Png::new(2, 1, 8, 3)
        .muestras(&[0, 5])
        .antes(plte(&[(1, 1, 1), (2, 2, 2)]))
        .bytes();
    assert_eq!(
        error_de(&fuera),
        PngError::PaletteIndexOutOfRange { index: 5, len: 2 }
    );

    // Sin PLTE.
    let sin_plte = Png::new(2, 1, 8, 3).muestras(&[0, 1]).bytes();
    assert_eq!(error_de(&sin_plte), PngError::InvalidPalette);

    // PLTE de 3 entradas a 1 bit (el máximo es 2).
    let demasiadas = Png::new(2, 1, 1, 3)
        .muestras(&[0, 1])
        .antes(plte(&[(1, 1, 1), (2, 2, 2), (3, 3, 3)]))
        .bytes();
    assert_eq!(error_de(&demasiadas), PngError::InvalidPalette);

    // PLTE vacío: lo rechaza el crate.
    let vacia = Png::new(2, 1, 8, 3)
        .muestras(&[0, 0])
        .antes(trozo(b"PLTE", &[]))
        .bytes();
    assert!(es_decode(&read_png(&vacia)), "{:?}", read_png(&vacia));

    // tRNS de 3 entradas con PLTE de 2.
    let trns_largo = Png::new(2, 1, 8, 3)
        .muestras(&[0, 1])
        .antes(plte(&[(1, 1, 1), (2, 2, 2)]))
        .antes(trns(&[0, 1, 2]))
        .bytes();
    assert_eq!(error_de(&trns_largo), PngError::InvalidTrns);
}

#[test]
fn tipos_8_bits_con_alfa_cero_y_rgb_distinto_de_cero_no_se_tocan() {
    // Gris 8: sin alfa en el fichero.
    let g = lee(&Png::new(2, 1, 8, 0).muestras(&[77, 200]).bytes());
    assert_eq!(g.image.mode(), Mode::Gray);
    assert_eq!(pixeles(&g.image), grises(&[77, 200]));
    assert!(g.report.is_empty());

    // Gris con alfa 8: alfa 0 con gris 77.
    let ga = lee(&Png::new(2, 1, 8, 4).muestras(&[77, 0, 200, 255]).bytes());
    assert_eq!(ga.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixeles(&ga.image), grises_alfa(&[(77, 0), (200, 255)]));
    assert!(ga.report.is_empty());

    // RGBA 8: alfa 0 con RGB (10, 20, 30) intacto.
    let c = lee(&Png::new(2, 1, 8, 6)
        .muestras(&[10, 20, 30, 0, 40, 50, 60, 255])
        .bytes());
    assert_eq!(c.image.mode(), Mode::Rgba);
    assert_eq!(
        pixeles(&c.image),
        [rgba(10, 20, 30, 0), rgba(40, 50, 60, 255)]
    );
    assert!(c.report.is_empty());
}

#[test]
fn gris_de_1_2_y_4_bits_se_escala_a_0_255() {
    // 1 bit: 0 y 255.
    let r1 = lee(&Png::new(2, 1, 1, 0).muestras(&[0, 1]).bytes());
    assert_eq!(r1.image.mode(), Mode::Gray);
    assert_eq!(pixeles(&r1.image), grises(&[0, 255]));
    assert_eq!(r1.report, [Conversion::DepthExpanded { from_bits: 1 }]);

    // 2 bits: 0, 85, 170, 255.
    let r2 = lee(&Png::new(4, 1, 2, 0).muestras(&[0, 1, 2, 3]).bytes());
    assert_eq!(pixeles(&r2.image), grises(&[0, 85, 170, 255]));
    assert_eq!(r2.report, [Conversion::DepthExpanded { from_bits: 2 }]);

    // 4 bits: v * 17 (0, 17, 34, ..., 255).
    let todos: Vec<u16> = (0..16).collect();
    let r4 = lee(&Png::new(16, 1, 4, 0).muestras(&todos).bytes());
    let esperados: Vec<u8> = (0..16u8).map(|v| v * 17).collect();
    assert_eq!(esperados[15], 255);
    assert_eq!(pixeles(&r4.image), grises(&esperados));
    assert_eq!(r4.report, [Conversion::DepthExpanded { from_bits: 4 }]);
}

#[test]
fn dieciseis_bits_se_reduce_con_redondeo() {
    let muestras: [u16; 5] = [0x0000, 0x0080, 0x0081, 0x01FF, 0xFFFF];

    // Gris 16: 0x0080 -> 0, 0x0081 -> 1, 0x01FF -> 2 (el byte alto daría 1), 0xFFFF -> 255.
    let g = lee(&Png::new(5, 1, 16, 0).muestras(&muestras).bytes());
    assert_eq!(g.image.mode(), Mode::Gray);
    assert_eq!(pixeles(&g.image), grises(&[0, 0, 1, 2, 255]));
    assert_eq!(g.report, [Conversion::Depth16Reduced]);

    // Gris con alfa 16: cada muestra, gris y alfa, con la misma regla.
    let ga = lee(&Png::new(3, 1, 16, 4)
        .muestras(&[0x0000, 0x0080, 0x0081, 0x01FF, 0xFFFF, 0x0000])
        .bytes());
    assert_eq!(ga.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixeles(&ga.image), grises_alfa(&[(0, 0), (1, 2), (255, 0)]));
    assert_eq!(ga.report, [Conversion::Depth16Reduced]);

    // RGBA 16: 0x0100 y 0x0101 y 0x00FF dan 1.
    let c = lee(&Png::new(2, 1, 16, 6)
        .muestras(&[
            0x0000, 0x0080, 0x0081, 0x01FF, 0xFFFF, 0x0100, 0x0101, 0x00FF,
        ])
        .bytes());
    assert_eq!(c.image.mode(), Mode::Rgba);
    assert_eq!(pixeles(&c.image), [rgba(0, 0, 1, 2), rgba(255, 1, 1, 1)]);
    assert_eq!(c.report, [Conversion::Depth16Reduced]);
}

#[test]
fn rgb_pasa_a_rgba_opaco() {
    let r8 = lee(&Png::new(1, 1, 8, 2).muestras(&[1, 2, 3]).bytes());
    assert_eq!(r8.image.mode(), Mode::Rgba);
    assert_eq!(pixeles(&r8.image), [rgba(1, 2, 3, 255)]);
    assert_eq!(r8.report, [Conversion::RgbToRgba]);

    let r16 = lee(&Png::new(1, 1, 16, 2)
        .muestras(&[0x0102, 0x0304, 0xFFFF])
        .bytes());
    assert_eq!(r16.image.mode(), Mode::Rgba);
    assert_eq!(pixeles(&r16.image), [rgba(1, 3, 255, 255)]);
    assert_eq!(
        r16.report,
        [Conversion::Depth16Reduced, Conversion::RgbToRgba]
    );
}

#[test]
fn gris_con_color_clave_da_gris_alfa_comparando_la_muestra_original() {
    // Gris 8, clave 7.
    let r8 = lee(&Png::new(2, 1, 8, 0)
        .muestras(&[7, 8])
        .antes(trns(&[0, 7]))
        .bytes());
    assert_eq!(r8.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixeles(&r8.image), grises_alfa(&[(7, 0), (8, 255)]));
    assert_eq!(r8.report, [Conversion::ColorKeyToAlpha]);

    // Gris 2 bits, clave 1: 1 -> 85 transparente, 2 -> 170 opaco.
    let r2 = lee(&Png::new(2, 1, 2, 0)
        .muestras(&[1, 2])
        .antes(trns(&[0, 1]))
        .bytes());
    assert_eq!(r2.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixeles(&r2.image), grises_alfa(&[(85, 0), (170, 255)]));
    assert_eq!(
        r2.report,
        [
            Conversion::DepthExpanded { from_bits: 2 },
            Conversion::ColorKeyToAlpha
        ]
    );

    // Gris 16, clave 0x0100: 0x0100 y 0x0101 se reducen a 1, pero solo la primera es la clave.
    let r16 = lee(&Png::new(2, 1, 16, 0)
        .muestras(&[0x0100, 0x0101])
        .antes(trns(&[0x01, 0x00]))
        .bytes());
    assert_eq!(r16.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixeles(&r16.image), grises_alfa(&[(1, 0), (1, 255)]));
    assert_eq!(
        r16.report,
        [Conversion::Depth16Reduced, Conversion::ColorKeyToAlpha]
    );
}

#[test]
fn rgb_con_color_clave_conserva_el_rgb_y_gris_sin_coincidencias_es_opaco() {
    // tRNS de RGB: tres muestras de 2 bytes (1, 2, 3).
    let r = lee(&Png::new(2, 1, 8, 2)
        .muestras(&[1, 2, 3, 1, 2, 4])
        .antes(trns(&[0, 1, 0, 2, 0, 3]))
        .bytes());
    assert_eq!(r.image.mode(), Mode::Rgba);
    assert_eq!(pixeles(&r.image), [rgba(1, 2, 3, 0), rgba(1, 2, 4, 255)]);
    assert_eq!(
        r.report,
        [Conversion::RgbToRgba, Conversion::ColorKeyToAlpha]
    );

    // Gris con tRNS y ningún píxel igual a la clave: el modo lo decide el fichero.
    let g = lee(&Png::new(2, 1, 8, 0)
        .muestras(&[1, 2])
        .antes(trns(&[0, 200]))
        .bytes());
    assert_eq!(g.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixeles(&g.image), grises_alfa(&[(1, 255), (2, 255)]));
    assert_eq!(g.report, [Conversion::ColorKeyToAlpha]);
}

#[test]
fn trns_mal_formado_en_gris_y_rgb_y_ignorado_en_tipos_con_alfa() {
    // Gris con tRNS de 1 byte.
    let corto = Png::new(1, 1, 8, 0)
        .muestras(&[1])
        .antes(trns(&[7]))
        .bytes();
    assert_eq!(error_de(&corto), PngError::InvalidTrns);

    // RGB con tRNS de 4 bytes.
    let rgb4 = Png::new(1, 1, 8, 2)
        .muestras(&[1, 2, 3])
        .antes(trns(&[0, 1, 0, 2]))
        .bytes();
    assert_eq!(error_de(&rgb4), PngError::InvalidTrns);

    // Gris de 2 bits con clave 9 (el máximo es 3).
    let fuera = Png::new(1, 1, 2, 0)
        .muestras(&[1])
        .antes(trns(&[0, 9]))
        .bytes();
    assert_eq!(error_de(&fuera), PngError::InvalidTrns);

    // GrayAlpha y RGBA con tRNS: PNG lo prohíbe, se ignora.
    let ga = lee(&Png::new(1, 1, 8, 4)
        .muestras(&[5, 6])
        .antes(trns(&[0, 5]))
        .bytes());
    assert_eq!(pixeles(&ga.image), grises_alfa(&[(5, 6)]));
    assert!(ga.report.is_empty());
    let c = lee(&Png::new(1, 1, 8, 6)
        .muestras(&[1, 2, 3, 4])
        .antes(trns(&[0, 1, 0, 2, 0, 3]))
        .bytes());
    assert_eq!(pixeles(&c.image), [rgba(1, 2, 3, 4)]);
    assert!(c.report.is_empty());
}

#[test]
fn adam7_da_la_misma_imagen_que_sin_entrelazar() {
    let paleta16: Vec<(u8, u8, u8)> = (0..16u8).map(|i| (i, 255 - i, i * 3)).collect();
    // (tipo, profundidad, muestras por píxel, chunks previos al IDAT, modo esperado)
    let casos = [
        (0, 8, 1, vec![], Mode::Gray),
        (3, 4, 1, vec![plte(&paleta16)], Mode::Indexed),
        (6, 8, 4, vec![], Mode::Rgba),
    ];
    for (color, prof, n, previos, modo) in casos {
        let muestras: Vec<u16> = (0..64 * n as u16)
            .map(|i| match color {
                3 => (i * 7 + i / 3) % 16,
                _ => (i * 37 + 5) % 256,
            })
            .collect();
        let construye = |entrelazado: bool| {
            let mut p = Png::new(8, 8, prof, color)
                .muestras(&muestras)
                .entrelazado(entrelazado);
            for c in &previos {
                p = p.antes(c.clone());
            }
            lee(&p.bytes())
        };
        let plano = construye(false);
        let entrelazada = construye(true);
        assert_eq!(plano.image.mode(), modo, "tipo {color}");
        assert_eq!(entrelazada.image, plano.image, "tipo {color}");
        assert!(plano.report.is_empty(), "tipo {color}");
        assert!(entrelazada.report.is_empty(), "tipo {color}");
        assert_eq!(pixeles(&plano.image).len(), 64);
    }
}

/// Chunks auxiliares válidos que la lectura debe ignorar.
fn auxiliares() -> Vec<Vec<u8>> {
    let mut iccp = b"perfil\0".to_vec();
    iccp.push(0);
    iccp.extend(zlib_stored(b"datos del perfil", 0));
    let mut phys = Vec::new();
    phys.extend(2835u32.to_be_bytes());
    phys.extend(2835u32.to_be_bytes());
    phys.push(1);
    vec![
        trozo(b"gAMA", &45455u32.to_be_bytes()),
        trozo(b"sRGB", &[0]),
        trozo(b"iCCP", &iccp),
        trozo(b"pHYs", &phys),
        trozo(b"tEXt", b"Title\0pixoko"),
        trozo(b"tIME", &[0x07, 0xEA, 10, 17, 12, 0, 0]),
        trozo(b"teSt", &[1, 2, 3]),
    ]
}

#[test]
fn chunks_auxiliares_no_cambian_imagen_ni_informe() {
    let base = Png::new(2, 1, 8, 2).muestras(&[1, 2, 3, 4, 5, 6]);
    let sin = lee(&base.bytes());
    assert_eq!(sin.report, [Conversion::RgbToRgba]);
    let mut con = base.clone();
    for c in auxiliares() {
        con = con.antes(c);
    }
    let r = lee(&con.bytes());
    // Los auxiliares no cambian ni la imagen ni el informe de conversiones; solo `aux` los refleja.
    assert_eq!(r.image, sin.image);
    assert_eq!(r.report, sin.report);
    assert!(sin.aux.found.is_empty());
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
    assert_eq!(pixeles(&r.image), [rgba(1, 2, 3, 255), rgba(4, 5, 6, 255)]);
}

#[test]
fn apng_se_lee_como_su_imagen_por_defecto() {
    // fcTL: secuencia, ancho, alto, x, y, retardo (num y den), dispose y blend.
    let fctl = |secuencia: u32| {
        let mut d = Vec::new();
        d.extend(secuencia.to_be_bytes());
        d.extend(2u32.to_be_bytes());
        d.extend(1u32.to_be_bytes());
        d.extend([0u8; 8]);
        d.extend([0, 1, 0, 10, 0, 0]);
        d
    };
    let mut fdat = 2u32.to_be_bytes().to_vec();
    fdat.extend(zlib_stored(&[0, 30, 40], 0));
    let png = Png::new(2, 1, 8, 0)
        .muestras(&[10, 20])
        .antes(trozo(b"acTL", &[0, 0, 0, 2, 0, 0, 0, 0]))
        .antes(trozo(b"fcTL", &fctl(0)))
        .despues(trozo(b"fcTL", &fctl(1)))
        .despues(trozo(b"fdAT", &fdat))
        .bytes();
    let r = lee(&png);
    assert_eq!(r.image.mode(), Mode::Gray);
    assert_eq!(pixeles(&r.image), grises(&[10, 20]));
    assert!(r.report.is_empty());
}

#[test]
fn chunk_critico_desconocido_es_decode() {
    let png = Png::new(1, 1, 8, 0)
        .muestras(&[5])
        .antes(trozo(b"TeSt", &[1]))
        .bytes();
    let r = read_png(&png);
    assert!(es_decode(&r), "{r:?}");
}

#[test]
fn ficheros_corruptos_son_decode_sin_panico() {
    let gris16x16: Vec<u16> = (0..256).collect();
    let base = Png::new(16, 16, 8, 0).muestras(&gris16x16);
    let valido = base.bytes();
    let (idat, largo_idat) = posicion_idat(&valido);

    let mut casos: Vec<(&str, Vec<u8>)> = vec![("vacío", Vec::new())];

    let mut firma = valido.clone();
    firma[0] = 0x88;
    casos.push(("firma incorrecta", firma));

    casos.push((
        "truncado a mitad de IDAT",
        valido[..idat + 8 + largo_idat / 2].to_vec(),
    ));
    casos.push(("sin IEND", valido[..valido.len() - 12].to_vec()));

    // CRC de IHDR: bytes 29..33 (8 de firma + 4 de longitud + 4 de tipo + 13 de datos).
    let mut crc_ihdr = valido.clone();
    crc_ihdr[32] ^= 0xFF;
    casos.push(("CRC erróneo en IHDR", crc_ihdr));

    let mut crc_idat = valido.clone();
    crc_idat[idat + 8 + largo_idat + 3] ^= 0xFF;
    casos.push(("CRC erróneo en IDAT", crc_idat));

    // Adler-32 erróneo, pero con el CRC del chunk recalculado (lo hace `trozo`).
    let mut adler = base.clone();
    adler.adler_xor = 0x0001_0000;
    casos.push(("Adler-32 erróneo", adler.bytes()));

    let mut filtro = base.clone();
    let mut crudo = filtro.crudo();
    crudo[0] = 9;
    filtro.crudo_fijo = Some(crudo);
    casos.push(("filtro de fila 9", filtro.bytes()));

    let mut rgb4 = Png::new(1, 1, 4, 2);
    rgb4.crudo_fijo = Some(vec![0, 0, 0]);
    casos.push(("RGB a 4 bits", rgb4.bytes()));

    let mut ancho0 = Png::new(0, 1, 8, 0);
    ancho0.crudo_fijo = Some(vec![0]);
    casos.push(("ancho 0", ancho0.bytes()));

    for (nombre, bytes) in casos {
        let r = read_png(&bytes);
        assert!(es_decode(&r), "{nombre}: {r:?}");
    }
}

#[test]
fn basura_tras_iend_es_trailing_data_y_iend_incompleto_es_decode() {
    let valido = Png::new(2, 1, 8, 0).muestras(&[1, 2]).bytes();
    assert!(read_png(&valido).is_ok());

    // Un byte de más.
    let mut con_x = valido.clone();
    con_x.extend(b"x");
    assert_eq!(error_de(&con_x), PngError::TrailingData);

    // Un chunk tEXt completo después de IEND.
    let mut con_texto = valido.clone();
    con_texto.extend(trozo(b"tEXt", b"k\0v"));
    assert_eq!(error_de(&con_texto), PngError::TrailingData);

    // Sin los últimos 1 a 11 bytes: IEND incompleto.
    for quitar in 1..=11 {
        let corto = &valido[..valido.len() - quitar];
        let r = read_png(corto);
        assert!(es_decode(&r), "sin los últimos {quitar} bytes: {r:?}");
    }
}

#[test]
fn cabecera_enorme_con_datos_minimos_es_too_large_o_decode() {
    // Gris 8 de 32769 x 32768: 1 byte por encima de `Image::MAX_BYTES`.
    let casi = Png::new(32769, 32768, 8, 0).bytes();
    assert_eq!(
        error_de(&casi),
        PngError::TooLarge {
            width: 32769,
            height: 32768
        }
    );

    // Gris 8 de 100000 x 100000.
    let enorme = Png::new(100_000, 100_000, 8, 0).bytes();
    assert_eq!(
        error_de(&enorme),
        PngError::TooLarge {
            width: 100_000,
            height: 100_000
        }
    );

    // RGBA 16 de 2 000 000 000 x 1: lo rechazan los límites del crate (Decode).
    let fila_gigante = Png::new(2_000_000_000, 1, 16, 6).bytes();
    let r = read_png(&fila_gigante);
    assert!(es_decode(&r), "{r:?}");
}

#[test]
fn rgb16_cuyo_bufer_del_fichero_no_cabe_aunque_la_imagen_si() {
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
fn escribe_indexado_con_plte_y_trns_segun_el_comportamiento_11() {
    let c = |r, g, b, a| Rgba::new(r, g, b, a);

    // Todo opaco: PLTE de 6 bytes y ninguna tRNS.
    let opaco = indexada(&[c(1, 2, 3, 255), c(4, 5, 6, 255)], None, &[0, 1]);
    let bytes = write_png(&opaco).unwrap();
    let trozos = recorre(&bytes).unwrap();
    let ihdr = &trozos[0];
    assert_eq!(ihdr.tipo, *b"IHDR");
    assert_eq!(&ihdr.datos[..4], &2u32.to_be_bytes());
    assert_eq!(&ihdr.datos[4..8], &1u32.to_be_bytes());
    // Profundidad 8, tipo 3, compresión 0, filtro 0, sin entrelazado.
    assert_eq!(&ihdr.datos[8..], &[8, 3, 0, 0, 0]);
    let tipos: Vec<[u8; 4]> = trozos.iter().map(|t| t.tipo).collect();
    assert_eq!(tipos, [*b"IHDR", *b"PLTE", *b"IDAT", *b"IEND"]);
    assert_eq!(trozos[1].datos, [1, 2, 3, 4, 5, 6]);
    assert_eq!(lee(&bytes).image, opaco);

    // Alfas [0, 255]: tRNS [0] (se omiten los 255 finales).
    let a = indexada(&[c(1, 2, 3, 0), c(4, 5, 6, 255)], None, &[0, 1]);
    let bytes = write_png(&a).unwrap();
    let trozos = recorre(&bytes).unwrap();
    let tipos: Vec<[u8; 4]> = trozos.iter().map(|t| t.tipo).collect();
    assert_eq!(tipos, [*b"IHDR", *b"PLTE", *b"tRNS", *b"IDAT", *b"IEND"]);
    assert_eq!(trozos[1].datos.len(), 6);
    assert_eq!(trozos[2].datos, [0]);
    assert_eq!(lee(&bytes).image, a);

    // Alfas [255, 128]: tRNS [255, 128].
    let b = indexada(&[c(1, 2, 3, 255), c(4, 5, 6, 128)], None, &[0, 1]);
    let bytes = write_png(&b).unwrap();
    let trozos = recorre(&bytes).unwrap();
    assert_eq!(trozos[2].tipo, *b"tRNS");
    assert_eq!(trozos[2].datos, [255, 128]);
    assert_eq!(lee(&bytes).image, b);

    // transparent_index = Some(1) con alfas guardados 255: tRNS [255, 0]; al leerlo, la
    // entrada 1 sale con a = 0 y sin índice transparente.
    let t = indexada(&[c(1, 2, 3, 255), c(4, 5, 6, 255)], Some(1), &[0, 1]);
    let bytes = write_png(&t).unwrap();
    let trozos = recorre(&bytes).unwrap();
    assert_eq!(trozos[2].tipo, *b"tRNS");
    assert_eq!(trozos[2].datos, [255, 0]);
    let r = lee(&bytes);
    let paleta = r.image.palette().unwrap();
    assert_eq!(paleta.colors(), [c(1, 2, 3, 255), c(4, 5, 6, 0)]);
    assert_eq!(paleta.transparent_index(), None);
    assert_eq!(pixeles(&r.image), indices(&[0, 1]));
    assert!(r.report.is_empty());
}

#[test]
fn escribe_gris_gris_alfa_y_rgba_tal_cual() {
    let casos = [
        (Mode::Gray, Pixel::Gray(77), 0u8),
        (Mode::GrayAlpha, Pixel::GrayAlpha { gray: 77, alpha: 0 }, 4),
        (Mode::Rgba, rgba(10, 20, 30, 0), 6),
    ];
    for (modo, pixel, tipo) in casos {
        let img = imagen_1x1(modo, pixel);
        let bytes = write_png(&img).unwrap();
        let trozos = recorre(&bytes).unwrap();
        let tipos: Vec<[u8; 4]> = trozos.iter().map(|t| t.tipo).collect();
        assert_eq!(tipos, [*b"IHDR", *b"IDAT", *b"IEND"], "{modo:?}");
        assert_eq!(&trozos[0].datos[8..], &[8, tipo, 0, 0, 0], "{modo:?}");
        let r = lee(&bytes);
        assert_eq!(r.image, img, "{modo:?}");
        assert!(r.report.is_empty(), "{modo:?}");
    }
}

// ---------------------------------------------------------------------------------------
// Propiedades.
// ---------------------------------------------------------------------------------------

fn rgba_arbitrario() -> impl Strategy<Value = Rgba> {
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
fn rellena(mut img: Image, px: impl IntoIterator<Item = Pixel>) -> Image {
    let w = img.width();
    for (i, p) in px.into_iter().enumerate() {
        img.set_pixel(i as u32 % w, i as u32 / w, p).unwrap();
    }
    img
}

/// Imagen de 1 a 8 de lado en cualquiera de los cuatro modos.
fn imagen_arbitraria() -> BoxedStrategy<Image> {
    (1u32..=8, 1u32..=8)
        .prop_flat_map(|(w, h)| {
            let n = (w * h) as usize;
            prop_oneof![
                vec(rgba_arbitrario(), n).prop_map(move |px| rellena(
                    Image::new(w, h, Mode::Rgba).unwrap(),
                    px.into_iter().map(Pixel::Rgba)
                )),
                vec(any::<u8>(), n).prop_map(move |px| rellena(
                    Image::new(w, h, Mode::Gray).unwrap(),
                    px.into_iter().map(Pixel::Gray)
                )),
                vec(any::<(u8, u8)>(), n).prop_map(move |px| rellena(
                    Image::new(w, h, Mode::GrayAlpha).unwrap(),
                    px.into_iter()
                        .map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
                )),
                (1usize..=256)
                    .prop_flat_map(move |len| {
                        (
                            vec(rgba_arbitrario(), len),
                            proptest::option::of(0..len),
                            vec(0..len, n),
                        )
                    })
                    .prop_map(move |(colores, transparente, px)| {
                        let mut p = Palette::from_colors(colores).unwrap();
                        p.set_transparent_index(transparente.map(|t| t as u8))
                            .unwrap();
                        rellena(
                            Image::new_indexed(w, h, p).unwrap(),
                            px.into_iter().map(|i| Pixel::Indexed(i as u8)),
                        )
                    }),
            ]
        })
        .boxed()
}

/// `img`, salvo en un indexado con índice transparente, cuya paleta sale ya resuelta.
fn normalizada(img: &Image) -> Image {
    let Some(p) = img.palette() else {
        return img.clone();
    };
    if p.transparent_index().is_none() {
        return img.clone();
    }
    let colores: Vec<Rgba> = (0..p.len()).map(|i| p.resolved(i as u8).unwrap()).collect();
    let copia = Image::new_indexed(
        img.width(),
        img.height(),
        Palette::from_colors(colores).unwrap(),
    )
    .unwrap();
    rellena(copia, pixeles(img))
}

/// Invariantes de `Image` observables desde la API pública.
fn comprueba_invariantes(img: &Image) -> Result<(), TestCaseError> {
    prop_assert!(img.width() >= 1 && img.height() >= 1);
    let todos = pixeles(img);
    prop_assert_eq!(
        todos.len() as u64,
        u64::from(img.width()) * u64::from(img.height())
    );
    match img.mode() {
        Mode::Indexed => {
            let paleta = img.palette();
            prop_assert!(paleta.is_some());
            let paleta = paleta.unwrap();
            prop_assert!(!paleta.is_empty());
            if let Some(t) = paleta.transparent_index() {
                prop_assert!(usize::from(t) < paleta.len());
            }
            for p in todos {
                match p {
                    Pixel::Indexed(i) => prop_assert!(usize::from(i) < paleta.len()),
                    otro => {
                        return Err(TestCaseError::fail(format!(
                            "píxel {otro:?} en una imagen indexada"
                        )));
                    }
                }
            }
        }
        modo => {
            prop_assert!(img.palette().is_none());
            for p in todos {
                let coincide = matches!(
                    (modo, p),
                    (Mode::Rgba, Pixel::Rgba(_))
                        | (Mode::Gray, Pixel::Gray(_))
                        | (Mode::GrayAlpha, Pixel::GrayAlpha { .. })
                );
                prop_assert!(coincide, "píxel {:?} en modo {:?}", p, modo);
            }
        }
    }
    Ok(())
}

fn comprueba_resultado(r: Result<PngRead, PngError>) -> Result<(), TestCaseError> {
    match r {
        Ok(leido) => comprueba_invariantes(&leido.image),
        Err(_) => Ok(()),
    }
}

proptest! {
    // Propiedad 1: ida y vuelta en los cuatro modos.
    #[test]
    fn p1_ida_y_vuelta_en_los_cuatro_modos(img in imagen_arbitraria()) {
        let bytes = write_png(&img).unwrap();
        let r = read_png(&bytes).unwrap();
        prop_assert!(r.report.is_empty());
        prop_assert!(r.is_exact());
        prop_assert_eq!(r.image, normalizada(&img));
    }
}

/// «Casi PNG»: firma e IHDR con CRC válido y valores cualesquiera, IDAT mínimo e IEND.
///
/// Para que ningún caso reserve más de unos pocos KiB, cada dimensión es pequeña (0 a 16) o
/// al menos 2^31 (con la otra distinta de cero, el tamaño supera `Image::MAX_BYTES` en
/// cualquier modo; con cero, es dimensión cero). Es la misma idea que los casos límite de
/// tamaño: rechazo por cabecera enorme con datos mínimos.
fn casi_png() -> impl Strategy<Value = Vec<u8>> {
    let dimension = prop_oneof![3 => 0u32..=16, 1 => (1u32 << 31)..=u32::MAX];
    let prof = prop_oneof![select(vec![1u8, 2, 4, 8, 16]), any::<u8>()];
    let color = prop_oneof![select(vec![0u8, 2, 3, 4, 6]), any::<u8>()];
    (dimension.clone(), dimension, color, prof).prop_map(|(ancho, alto, color, prof)| {
        let mut ihdr = Vec::new();
        ihdr.extend(ancho.to_be_bytes());
        ihdr.extend(alto.to_be_bytes());
        ihdr.extend([prof, color, 0, 0, 0]);
        let mut v = FIRMA.to_vec();
        v.extend(trozo(b"IHDR", &ihdr));
        v.extend(trozo(b"IDAT", &zlib_stored(&[], 0)));
        v.extend(trozo(b"IEND", &[]));
        v
    })
}

proptest! {
    // Propiedad 2a: bytes arbitrarios (la mitad, con la firma delante).
    #[test]
    fn p2_bytes_arbitrarios_nunca_hacen_panico(
        bytes in prop_oneof![
            vec(any::<u8>(), 0..=512),
            vec(any::<u8>(), 0..=504).prop_map(|b| [FIRMA.to_vec(), b].concat()),
        ]
    ) {
        comprueba_resultado(read_png(&bytes))?;
    }

    // Propiedad 2b: «casi PNG».
    #[test]
    fn p2_casi_png_nunca_hace_panico(bytes in casi_png()) {
        comprueba_resultado(read_png(&bytes))?;
    }

    // Propiedad 2c: un PNG válido al que se cambian 1 a 4 bytes.
    #[test]
    fn p2_png_valido_con_bytes_cambiados_nunca_hace_panico(
        img in imagen_arbitraria(),
        cambios in vec((any::<usize>(), any::<u8>()), 1..=4),
    ) {
        let mut bytes = write_png(&img).unwrap();
        for (donde, nuevo) in cambios {
            let i = donde % bytes.len();
            bytes[i] = nuevo;
        }
        comprueba_resultado(read_png(&bytes))?;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    // Propiedad 3: truncado y basura final.
    #[test]
    fn p3_truncado_y_basura_final(
        img in imagen_arbitraria(),
        extra in vec(any::<u8>(), 1..=16),
    ) {
        let bytes = write_png(&img).unwrap();
        for largo in 0..bytes.len() {
            prop_assert!(
                read_png(&bytes[..largo]).is_err(),
                "el prefijo de {} bytes (de {}) dio Ok", largo, bytes.len()
            );
        }
        let mut con_basura = bytes.clone();
        con_basura.extend(&extra);
        prop_assert_eq!(read_png(&con_basura).unwrap_err(), PngError::TrailingData);
    }
}

#[derive(Clone, Debug)]
struct CasoIndexado {
    prof: u8,
    ancho: usize,
    alto: usize,
    entrelazado: bool,
    colores: Vec<(u8, u8, u8)>,
    alfas: Vec<u8>,
    px: Vec<u16>,
}

fn caso_indexado() -> impl Strategy<Value = CasoIndexado> {
    (
        select(vec![1u8, 2, 4, 8]),
        1usize..=12,
        1usize..=12,
        any::<bool>(),
    )
        .prop_flat_map(|(prof, ancho, alto, entrelazado)| {
            (1usize..=(1usize << prof)).prop_flat_map(move |n| {
                (
                    vec(any::<(u8, u8, u8)>(), n),
                    vec(any::<u8>(), 0..=n),
                    vec(0..n as u16, ancho * alto),
                )
                    .prop_map(move |(colores, alfas, px)| CasoIndexado {
                        prof,
                        ancho,
                        alto,
                        entrelazado,
                        colores,
                        alfas,
                        px,
                    })
            })
        })
}

proptest! {
    // Propiedad 4: indexado intacto, con y sin Adam7.
    #[test]
    fn p4_indexado_intacto(caso in caso_indexado()) {
        let mut png = Png::new(caso.ancho as u32, caso.alto as u32, caso.prof, 3)
            .muestras(&caso.px)
            .entrelazado(caso.entrelazado)
            .antes(plte(&caso.colores));
        if !caso.alfas.is_empty() {
            png = png.antes(trns(&caso.alfas));
        }
        let r = read_png(&png.bytes()).unwrap();
        prop_assert!(r.report.is_empty());
        prop_assert_eq!(r.image.mode(), Mode::Indexed);
        prop_assert_eq!(
            (r.image.width() as usize, r.image.height() as usize),
            (caso.ancho, caso.alto)
        );
        let esperados: Vec<Pixel> = caso.px.iter().map(|&i| Pixel::Indexed(i as u8)).collect();
        prop_assert_eq!(pixeles(&r.image), esperados);
        let paleta = r.image.palette().unwrap();
        prop_assert_eq!(paleta.transparent_index(), None);
        prop_assert_eq!(paleta.len(), caso.colores.len());
        for (i, &(rr, gg, bb)) in caso.colores.iter().enumerate() {
            let a = caso.alfas.get(i).copied().unwrap_or(255);
            prop_assert_eq!(paleta.colors()[i], Rgba::new(rr, gg, bb, a), "entrada {}", i);
        }
    }
}

/// Combinaciones (tipo de color, profundidad) de la propiedad 5.
const COMBINACIONES: [(u8, u8); 11] = [
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
struct CasoConversion {
    color: u8,
    prof: u8,
    ancho: usize,
    alto: usize,
    entrelazado: bool,
    /// Muestras originales, píxel a píxel y canal a canal.
    muestras: Vec<u16>,
    /// Color clave (1 muestra en gris, 3 en RGB), solo con tRNS de gris o RGB.
    clave: Option<Vec<u16>>,
}

/// Muestra válida para la profundidad; a 16 bits, con valores cercanos entre sí para que
/// haya diferencias en el byte bajo y reducciones que coincidan.
fn muestra(prof: u8) -> BoxedStrategy<u16> {
    if prof == 16 {
        prop_oneof![
            any::<u16>(),
            any::<u8>().prop_map(|b| u16::from(b) * 257),
            (0u16..=255, 0u16..=3).prop_map(|(alto, bajo)| (alto << 8) | bajo),
        ]
        .boxed()
    } else {
        (0u16..(1u16 << prof)).boxed()
    }
}

fn caso_conversion() -> impl Strategy<Value = CasoConversion> {
    (
        select(COMBINACIONES.to_vec()),
        1usize..=8,
        1usize..=8,
        any::<bool>(),
    )
        .prop_flat_map(|((color, prof), ancho, alto, entrelazado)| {
            let por_pixel = canales(color);
            let n_clave = if color == 0 { 1 } else { 3 };
            let clave = if color == 0 || color == 2 {
                proptest::option::of((any::<bool>(), 0..ancho * alto, vec(muestra(prof), 3)))
                    .boxed()
            } else {
                Just(None).boxed()
            };
            (vec(muestra(prof), ancho * alto * por_pixel), clave).prop_map(
                move |(muestras, clave)| {
                    // La clave, a veces, es la de un píxel que existe.
                    let clave = clave.map(|(existente, p, otras)| {
                        if existente {
                            muestras[p * por_pixel..p * por_pixel + n_clave].to_vec()
                        } else {
                            otras[..n_clave].to_vec()
                        }
                    });
                    CasoConversion {
                        color,
                        prof,
                        ancho,
                        alto,
                        entrelazado,
                        muestras,
                        clave,
                    }
                },
            )
        })
}

/// De 16 u 8 bits, o de `n < 8` bits de gris, a 8 bits: el redondeo al más cercano de
/// `v * 255 / (2^n - 1)`, calculado con coma flotante (independiente de la fórmula entera
/// `(v + 128) / 257` de la spec).
fn a_8_bits(v: u16, prof: u8) -> u8 {
    let maximo = f64::from((1u32 << prof) - 1);
    (f64::from(v) * 255.0 / maximo).round() as u8
}

/// Resultado esperado según la tabla del comportamiento 2.
fn modelo(c: &CasoConversion) -> (Mode, Vec<Pixel>, Vec<Conversion>) {
    let por_pixel = canales(c.color);
    let mut informe = Vec::new();
    if c.color == 0 && c.prof < 8 {
        informe.push(Conversion::DepthExpanded { from_bits: c.prof });
    }
    if c.prof == 16 {
        informe.push(Conversion::Depth16Reduced);
    }
    if c.color == 2 {
        informe.push(Conversion::RgbToRgba);
    }
    if c.clave.is_some() {
        informe.push(Conversion::ColorKeyToAlpha);
    }

    let pixeles: Vec<Pixel> = c
        .muestras
        .chunks(por_pixel)
        .map(|m| {
            // La clave se compara con las muestras originales, antes de escalar.
            let es_clave = c.clave.as_deref() == Some(&m[..m.len().min(3)]);
            let a = |i: usize| a_8_bits(m[i], c.prof);
            let alfa_clave = if es_clave { 0 } else { 255 };
            match (c.color, c.clave.is_some()) {
                (0, false) => Pixel::Gray(a(0)),
                (0, true) => Pixel::GrayAlpha {
                    gray: a(0),
                    alpha: alfa_clave,
                },
                (2, _) => rgba(
                    a(0),
                    a(1),
                    a(2),
                    if c.clave.is_some() { alfa_clave } else { 255 },
                ),
                (4, _) => Pixel::GrayAlpha {
                    gray: a(0),
                    alpha: a(1),
                },
                (6, _) => rgba(a(0), a(1), a(2), a(3)),
                otro => unreachable!("{otro:?}"),
            }
        })
        .collect();

    let modo = match (c.color, c.clave.is_some()) {
        (0, false) => Mode::Gray,
        (0, true) | (4, _) => Mode::GrayAlpha,
        _ => Mode::Rgba,
    };
    (modo, pixeles, informe)
}

proptest! {
    // Propiedad 5: conversiones contra un modelo independiente.
    #[test]
    fn p5_conversiones_contra_un_modelo(caso in caso_conversion()) {
        let mut png = Png::new(caso.ancho as u32, caso.alto as u32, caso.prof, caso.color)
            .muestras(&caso.muestras)
            .entrelazado(caso.entrelazado);
        if let Some(clave) = &caso.clave {
            let datos: Vec<u8> = clave.iter().flat_map(|v| v.to_be_bytes()).collect();
            png = png.antes(trns(&datos));
        }
        let r = read_png(&png.bytes()).unwrap();
        let (modo, esperados, informe) = modelo(&caso);
        prop_assert_eq!(r.image.mode(), modo);
        prop_assert_eq!(
            (r.image.width() as usize, r.image.height() as usize),
            (caso.ancho, caso.alto)
        );
        prop_assert_eq!(pixeles(&r.image), esperados);
        prop_assert_eq!(r.report, informe);
    }
}

proptest! {
    // Propiedad 6: forma del fichero escrito.
    #[test]
    fn p6_forma_del_fichero_escrito(img in imagen_arbitraria()) {
        let bytes = write_png(&img).unwrap();
        let trozos = recorre(&bytes).map_err(TestCaseError::fail)?;
        let tipos: Vec<[u8; 4]> = trozos.iter().map(|t| t.tipo).collect();

        // Secuencia: IHDR, [PLTE, [tRNS]], IDAT+, IEND.
        let alfas: Vec<u8> = img
            .palette()
            .map(|p| (0..p.len()).map(|i| p.resolved(i as u8).unwrap().a).collect())
            .unwrap_or_default();
        let largo_trns = alfas.iter().rposition(|&a| a != 255).map_or(0, |i| i + 1);
        let mut esperados: Vec<[u8; 4]> = vec![*b"IHDR"];
        if img.mode() == Mode::Indexed {
            esperados.push(*b"PLTE");
            if largo_trns > 0 {
                esperados.push(*b"tRNS");
            }
        }
        esperados.push(*b"IDAT");
        esperados.push(*b"IEND");

        let n_idat = tipos.iter().filter(|t| **t == *b"IDAT").count();
        prop_assert!(n_idat >= 1);
        let primero = tipos.iter().position(|t| *t == *b"IDAT").unwrap();
        prop_assert!(tipos[primero..primero + n_idat].iter().all(|t| *t == *b"IDAT"));
        let colapsados: Vec<[u8; 4]> = tipos[..primero]
            .iter()
            .chain(std::iter::once(&tipos[primero]))
            .chain(&tipos[primero + n_idat..])
            .copied()
            .collect();
        prop_assert_eq!(colapsados, esperados);

        // IHDR: tamaño, 8 bits, tipo de color del modo y sin entrelazado.
        let tipo_color = match img.mode() {
            Mode::Indexed => 3u8,
            Mode::Gray => 0,
            Mode::GrayAlpha => 4,
            Mode::Rgba => 6,
        };
        let ihdr = &trozos[0].datos;
        prop_assert_eq!(ihdr.len(), 13);
        prop_assert_eq!(&ihdr[..4], &img.width().to_be_bytes());
        prop_assert_eq!(&ihdr[4..8], &img.height().to_be_bytes());
        prop_assert_eq!(&ihdr[8..], &[8, tipo_color, 0, 0, 0]);

        // PLTE con 3 * len bytes y tRNS con la longitud del comportamiento 11.
        if let Some(paleta) = img.palette() {
            let plte = trozos.iter().find(|t| t.tipo == *b"PLTE").unwrap();
            prop_assert_eq!(plte.datos.len(), 3 * paleta.len());
            let rgb: Vec<u8> = paleta.colors().iter().flat_map(|c| [c.r, c.g, c.b]).collect();
            prop_assert_eq!(&plte.datos, &rgb);
            let trns = trozos.iter().find(|t| t.tipo == *b"tRNS");
            match trns {
                Some(t) => prop_assert_eq!(&t.datos[..], &alfas[..largo_trns]),
                None => prop_assert_eq!(largo_trns, 0),
            }
        }

        // Determinista.
        prop_assert_eq!(write_png(&img).unwrap(), bytes);
    }
}
