//! Cota de lectura de `read_png`, parte de aceptación y orden de errores (specs/hito-2-png-flujo.md,
//! unidad 13): constructor de `Z(n)`, filas 5, 6 y 8 y propiedad 2.
//!
//! Sin `png` ni `flate2`: el flujo de razón máxima se construye bit a bit (ver `z`). Las pruebas de
//! rechazo con medida de memoria (fila 4) están en `tests/png_reserve.rs`; la fila 7 y la
//! propiedad 3, que sí usan el crate `png` como codificador, en `compressors.rs`.
//!
//! Estado hoy (antes de la cota): las pruebas de **aceptación** (filas 5 y 6, propiedad 2) ya
//! pasan, porque el decodificador actual acepta esos ficheros; guardan que la cota, cuando llegue,
//! no los rechace. Lo que está en rojo es la parte de «mensaje» de la fila 8.
//!
//! Regla de las pruebas de rechazo: el error es `PngError::Decode(msg)` con
//! `msg.starts_with(MESSAGE_BOUND)`.

use super::*;

/// Comienzo del mensaje del error de la cota (spec, «Error»).
pub(super) const MESSAGE_BOUND: &str = "tamaño declarado inconsistente con el fichero";

// ---------------------------------------------------------------------------------------
// `Z(n)`: flujo zlib de razón máxima.
// ---------------------------------------------------------------------------------------

/// Escritor de bits de DEFLATE: los campos numéricos van con el bit menos significativo primero y
/// los códigos de Huffman con el más significativo primero.
struct Bits {
    output: Vec<u8>,
    accumulator: u32,
    n: u32,
}

impl Bits {
    fn new() -> Bits {
        Bits {
            output: Vec::new(),
            accumulator: 0,
            n: 0,
        }
    }

    fn bit(&mut self, b: u32) {
        self.accumulator |= (b & 1) << self.n;
        self.n += 1;
        if self.n == 8 {
            self.output.push(self.accumulator as u8);
            self.accumulator = 0;
            self.n = 0;
        }
    }

    /// Campo numérico de `n` bits, LSB primero.
    fn field(&mut self, valor: u32, n: u32) {
        for i in 0..n {
            self.bit(valor >> i);
        }
    }

    /// Código de Huffman de `n` bits, MSB primero.
    fn code_2(&mut self, valor: u32, n: u32) {
        for i in (0..n).rev() {
            self.bit(valor >> i);
        }
    }

    fn ends(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.output.push(self.accumulator as u8);
        }
        self.output
    }
}

/// Cuántas coincidencias de longitud 258 hacen falta para llegar a `n` bytes: el flujo produce
/// `1 + 258 · k` bytes cero (un literal 0 y `k` coincidencias a distancia 1).
fn matches(n: u64) -> u64 {
    if n <= 1 { 0 } else { (n - 1).div_ceil(258) }
}

/// Bytes que produce `z(n)`: `1 + 258 · k >= n`.
pub(super) fn bytes_de_z(n: u64) -> u64 {
    1 + 258 * matches(n)
}

/// `Z(n)`: flujo zlib de razón máxima que produce `1 + 258 · k >= n` bytes cero.
///
/// Un único bloque dinámico: literal 0 con código de 2 bits, símbolo 285 (longitud 258) con 1 bit,
/// un único código de distancia de 1 bit y fin de bloque (256) con 2 bits; cada coincidencia
/// cuesta 2 bits. El Adler-32 es el de los bytes que de verdad produce el flujo
/// (`((L mod 65521) << 16) | 1`, con `L = 1 + 258 · k`).
pub(super) fn z(n: u64) -> Vec<u8> {
    let k = matches(n);
    let produced = bytes_de_z(n);

    let mut b = Bits::new();
    b.field(1, 1); // BFINAL
    b.field(2, 2); // BTYPE = 10 (códigos dinámicos)
    b.field(286 - 257, 5); // HLIT
    b.field(0, 5); // HDIST: un código de distancia
    // Alfabeto de longitudes: 0 con 1 bit, 1 y 2 con 2 bits.
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let mut largo_de = [0u32; 19];
    largo_de[0] = 1;
    largo_de[1] = 2;
    largo_de[2] = 2;
    let ultimo = ORDER.iter().rposition(|&s| largo_de[s] != 0).unwrap();
    b.field((ultimo + 1 - 4) as u32, 4); // HCLEN
    for &s in &ORDER[..=ultimo] {
        b.field(largo_de[s], 3);
    }
    // Códigos canónicos del alfabeto de longitudes: 0 -> '0', 1 -> '10', 2 -> '11'.
    let mut longitudes = [0u32; 286 + 1];
    longitudes[0] = 2;
    longitudes[256] = 2;
    longitudes[285] = 1;
    longitudes[286] = 1; // el único código de distancia
    for &l in &longitudes {
        match l {
            0 => b.code_2(0b0, 1),
            1 => b.code_2(0b10, 2),
            2 => b.code_2(0b11, 2),
            _ => unreachable!(),
        }
    }
    // Datos. Código de literal/longitud: 285 -> '0', 0 -> '10', 256 -> '11'; distancia 1 -> '0'.
    b.code_2(0b10, 2); // literal 0
    for _ in 0..k {
        b.code_2(0b0, 1); // símbolo 285 (longitud 258, sin bits extra)
        b.code_2(0b0, 1); // distancia 1
    }
    b.code_2(0b11, 2); // fin de bloque

    let mut v = vec![0x78, 0x01];
    v.extend(b.ends());
    v.extend(((((produced % 65521) as u32) << 16) | 1).to_be_bytes());
    v
}

// ---------------------------------------------------------------------------------------
// Ficheros con un IDAT dado y el modelo de `needed` de las pruebas.
// ---------------------------------------------------------------------------------------

/// Fichero con `antes` entre IHDR e IDAT y, si `idat` es `Some`, ese IDAT (ya comprimido).
pub(super) fn file(
    width: u32,
    height: u32,
    prof: u8,
    color: u8,
    interlaced_2: bool,
    antes: &[Vec<u8>],
    idat: Option<&[u8]>,
) -> Vec<u8> {
    let mut ihdr = Vec::new();
    ihdr.extend(width.to_be_bytes());
    ihdr.extend(height.to_be_bytes());
    ihdr.extend([prof, color, 0, 0, u8::from(interlaced_2)]);
    let mut v = TEST_SIGNATURE.to_vec();
    v.extend(piece(b"IHDR", &ihdr));
    for c in antes {
        v.extend(c);
    }
    if let Some(idat) = idat {
        v.extend(piece(b"IDAT", idat));
    }
    v.extend(piece(b"IEND", &[]));
    v
}

/// PLTE de `n` entradas negras.
pub(super) fn plte_black(n: usize) -> Vec<u8> {
    plte(&vec![(0, 0, 0); n])
}

/// Bytes que el flujo zlib debe producir para la imagen por defecto, según la spec PNG:
/// `rows × (1 + ceil(columns × bpp / 8))` por pasada, con las siete pasadas de Adam7 si
/// `interlaced_2` (las vacías no cuentan). Modelo propio de la prueba, independiente del código.
pub(super) fn necessary(width: u32, height: u32, bpp: u64, interlaced_2: bool) -> u64 {
    let pass = |columns: u64, rows: u64| {
        if columns == 0 || rows == 0 {
            0
        } else {
            rows * (1 + (columns * bpp).div_ceil(8))
        }
    };
    let (width, height) = (u64::from(width), u64::from(height));
    if !interlaced_2 {
        return pass(width, height);
    }
    ADAM7
        .iter()
        .map(|&(x0, y0, dx, dy)| {
            let count = |total: u64, ini: usize, jump: usize| {
                if total <= ini as u64 {
                    0
                } else {
                    (total - ini as u64).div_ceil(jump as u64)
                }
            };
            pass(count(width, x0, dx), count(height, y0, dy))
        })
        .sum()
}

/// Las cuatro formas de la fila 5 en 8 bits: (nombre, tipo de color, bits por píxel, modo).
const SHAPES_8_BITS: [(&str, u8, u64, Mode); 4] = [
    ("gris 8", 0, 8, Mode::Gray),
    ("gris con alfa 8", 4, 16, Mode::GrayAlpha),
    ("RGBA 8", 6, 32, Mode::Rgba),
    ("indexado 8", 3, 8, Mode::Indexed),
];

/// Chunks previos al IDAT que necesita ese tipo de color (PLTE de 256 entradas en indexado).
fn previous(color: u8, prof: u8) -> Vec<Vec<u8>> {
    if color == 3 {
        vec![plte_black(1 << prof)]
    } else {
        Vec::new()
    }
}

/// Los bytes de la imagen son los de «todo a cero»: con RGB (tipo 2) el alfa sale a 255.
fn is_all_zero(img: &Image, color: u8) -> bool {
    if color == 2 {
        img.bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 0, 0, 255])
    } else {
        img.bytes().iter().all(|&b| b == 0)
    }
}

#[allow(clippy::too_many_arguments)]
/// Lee `Z(needed)` para esa cabecera y comprueba que es `Ok`, todo cero y, en 8 bits, sin informe.
fn reads_with_z(
    name_2: &str,
    width: u32,
    height: u32,
    prof: u8,
    color: u8,
    bpp: u64,
    interlaced_2: bool,
    mode: Mode,
    reason_minimum: u64,
) {
    let needed = necessary(width, height, bpp, interlaced_2);
    let flow = z(needed);
    // El flujo es de razón ajustada: `needed / idat` entre `reason_minimum` y 1032. La cabecera
    // dinámica (~55 bytes) pesa más a 4096² (razón ≈ 1028,5) que a 8192² (≈ 1031,2, margen de
    // 0,08 %): `needed` no puede sobrestimarse más que ese margen sin que la cota rechace el
    // fichero, así que esta prueba también acota por arriba el error de `needed`.
    let idat = flow.len() as u64;
    assert!(needed <= 1032 * idat, "{name_2}: razón por encima de 1032");
    assert!(
        needed >= reason_minimum * idat,
        "{name_2}: razón por debajo de {reason_minimum} ({needed} / {idat})"
    );
    let bytes = file(
        width,
        height,
        prof,
        color,
        interlaced_2,
        &previous(color, prof),
        Some(&flow),
    );
    drop(flow);
    let r = read_png(&bytes);
    let r = match r {
        Ok(r) => r,
        Err(e) => panic!("{name_2} {width}x{height} (Adam7: {interlaced_2}): {e:?}"),
    };
    assert_eq!(r.image.mode(), mode, "{name_2}");
    assert_eq!(
        (r.image.width(), r.image.height()),
        (width, height),
        "{name_2}"
    );
    assert!(
        is_all_zero(&r.image, color),
        "{name_2}: hay bytes distintos de 0"
    );
    assert!(r.report.is_empty(), "{name_2}: {:?}", r.report);
}

// ---------------------------------------------------------------------------------------
// Comprobación del constructor.
// ---------------------------------------------------------------------------------------

#[test]
fn z_prototype_anchors() {
    // k = 260144 da 67 117 153 bytes en 65 088 bytes de zlib (razón 1031,18).
    let n = 1 + 258 * 260_144;
    assert_eq!(bytes_de_z(n), 67_117_153);
    assert_eq!(z(n).len(), 65_088);
    // Cabecera zlib y Adler-32 de `produced` ceros.
    let flow = z(n);
    assert_eq!(&flow[..2], [0x78, 0x01]);
    let adler = (((67_117_153u64 % 65521) as u32) << 16) | 1;
    assert_eq!(&flow[flow.len() - 4..], adler.to_be_bytes());
    // `bytes_de_z` redondea hacia arriba y nunca baja de `n`.
    assert_eq!(bytes_de_z(0), 1);
    assert_eq!(bytes_de_z(1), 1);
    assert_eq!(bytes_de_z(2), 259);
    assert_eq!(bytes_de_z(259), 259);
    assert_eq!(bytes_de_z(260), 517);
}

#[test]
fn small_z_decodes_to_gray_1x1_and_gray_3x2() {
    // 1 x 1 gris 8: needed = 2 (filtro + muestra); Z(2) produce 259 bytes, de sobra.
    let one = file(1, 1, 8, 0, false, &[], Some(&z(2)));
    let r = read_png(&one).unwrap();
    assert_eq!(pixels(&r.image), grays(&[0]));
    let three_per_two = file(3, 2, 8, 0, false, &[], Some(&z(8)));
    let r = read_png(&three_per_two).unwrap();
    assert_eq!(pixels(&r.image), grays(&[0; 6]));
}

// ---------------------------------------------------------------------------------------
// Fila 5: cota ajustada con Z.
// ---------------------------------------------------------------------------------------

/// Gris 8, gris con alfa, RGBA e indexado de `side`² con `Z(needed)`, y gris 8 entrelazado.
fn row_5(side: u32) {
    let minimum = if side >= 8192 { 1030 } else { 1025 };
    for (name_2, color, bpp, mode) in SHAPES_8_BITS {
        reads_with_z(name_2, side, side, 8, color, bpp, false, mode, minimum);
    }
    reads_with_z(
        "gris 8 Adam7",
        side,
        side,
        8,
        0,
        8,
        true,
        Mode::Gray,
        minimum,
    );
}

#[test]
fn row5_tight_bound_with_z_at_4096() {
    row_5(4096);
}

#[test]
#[ignore = "8192²: lento sin optimizar; se ejecuta al cerrar con --release -- --ignored"]
fn row5_tight_bound_with_z_at_8192() {
    row_5(8192);
}

// ---------------------------------------------------------------------------------------
// Fila 6: un solo color con el codificador por defecto.
// ---------------------------------------------------------------------------------------

/// Un constructor de imagen con su nombre, para los casos de `a_only_color`.
type Constructor = (&'static str, Box<dyn Fn() -> Image>);

fn a_only_color(side: u32) {
    let palette = Palette::from_colors([
        Rgba::opaque(1, 2, 3),
        Rgba::opaque(40, 50, 60),
        Rgba::opaque(7, 8, 9),
    ])
    .unwrap();
    let constructors: [Constructor; 4] = [
        (
            "gris",
            Box::new(move || Image::filled(side, side, Pixel::Gray(77)).unwrap()),
        ),
        (
            "gris con alfa",
            Box::new(move || {
                Image::filled(
                    side,
                    side,
                    Pixel::GrayAlpha {
                        gray: 77,
                        alpha: 200,
                    },
                )
                .unwrap()
            }),
        ),
        (
            "indexado",
            Box::new(move || Image::filled_indexed(side, side, palette.clone(), 1).unwrap()),
        ),
        (
            "RGBA",
            Box::new(move || {
                Image::filled(side, side, Pixel::Rgba(Rgba::new(10, 20, 30, 40))).unwrap()
            }),
        ),
    ];
    for (name_2, builds) in constructors {
        let img = builds();
        let bytes = write_png(&img).unwrap();
        let expected = img;
        let r = match read_png(&bytes) {
            Ok(r) => r,
            Err(e) => panic!("{name_2} {side}²: {e:?}"),
        };
        drop(bytes);
        assert!(r.report.is_empty(), "{name_2}: {:?}", r.report);
        assert!(r.image == expected, "{name_2}: la imagen leída es distinta");
    }
}

#[test]
fn row6_a_single_color_with_the_default_encoder_at_4096() {
    a_only_color(4096);
}

#[test]
#[ignore = "8192²: lento sin optimizar; se ejecuta al cerrar con --release -- --ignored"]
fn row6_a_single_color_with_the_default_encoder_at_8192() {
    a_only_color(8192);
}

// ---------------------------------------------------------------------------------------
// Fila 8: orden de errores, truncados e IDAT declarado enorme.
// ---------------------------------------------------------------------------------------

fn is_decode_with_bound_message(r: &Result<PngRead, PngError>) -> bool {
    matches!(r, Err(PngError::Decode(m)) if m.starts_with(MESSAGE_BOUND))
}

#[test]
fn row8_too_large_beats_the_bound() {
    // Gris 8 de 100000 × 100000 con IDAT mínimo: `TooLarge`, la cota no se mira antes.
    let r = read_png(&Png::new(100_000, 100_000, 8, 0).bytes());
    assert_eq!(
        r,
        Err(PngError::TooLarge {
            width: 100_000,
            height: 100_000
        })
    );
}

#[test]
fn row8_invalid_palette_and_trns_beat_the_bound() {
    // Indexado de 8192 × 8192 sin PLTE (IDAT mínimo): `InvalidPalette`.
    let without_plte = Png::new(8192, 8192, 8, 3).bytes();
    assert_eq!(error_de(&without_plte), PngError::InvalidPalette);

    // Indexado con tRNS de 3 entradas y PLTE de 2 (8192 × 8192, IDAT mínimo): `InvalidTrns`.
    let trns_largo = Png::new(8192, 8192, 8, 3)
        .antes(plte(&[(1, 1, 1), (2, 2, 2)]))
        .antes(trns(&[0, 1, 2]))
        .bytes();
    assert_eq!(error_de(&trns_largo), PngError::InvalidTrns);
}

#[test]
fn row8_truncated_in_the_middle_of_idat_is_decode() {
    // Ruido de 64 × 64 (xorshift fijo) cortado a media IDAT: lo rechaza `scan_chunks`.
    let mut s = 0x9E37_79B9_7F4A_7C15u64;
    let swatches: Vec<u16> = (0..64 * 64)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 24) as u8 as u16
        })
        .collect();
    let valid = Png::new(64, 64, 8, 0).swatches(&swatches).bytes();
    assert!(read_png(&valid).is_ok());
    let (start, largo) = position_idat(&valid);
    let cut = &valid[..start + 8 + largo / 2];
    let r = read_png(cut);
    assert!(es_decode(&r), "{r:?}");
}

#[test]
fn row8_without_idat_is_decode() {
    // Solo IHDR e IEND, 8 × 8 gris 8.
    let bytes = file(8, 8, 8, 0, false, &[], None);
    let r = read_png(&bytes);
    assert!(es_decode(&r), "{r:?}");
}

#[test]
fn row8_declared_huge_idat_in_a_short_file_is_decode() {
    // Gris de 8192 × 8192 con un IDAT que declara 2 GiB y un fichero de ~100 bytes: el chunk
    // truncado ya lo rechaza `scan_chunks` y no aporta bytes a la cota.
    let mut v = file(8192, 8192, 8, 0, false, &[], None);
    v.truncate(v.len() - 12); // quita IEND
    v.extend(0x8000_0000u32.to_be_bytes());
    v.extend(b"IDAT");
    v.extend([0x78, 0x9c, 0x63, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01]);
    v.extend(piece(b"IEND", &[]));
    assert!(v.len() < 128, "{} bytes", v.len());
    let r = read_png(&v);
    assert!(es_decode(&r), "{r:?}");
}

#[test]
fn row8_the_bound_rejects_with_its_message_a_file_without_enough_data() {
    // Parte de «mensaje» de la fila 8, en ROJO hoy (hoy es otro `Decode`). El IDAT mínimo de
    // `Png` mide 11 bytes (zlib con un bloque «stored» vacío): 1032 × 11 = 11 352. Un gris 8 de
    // 200 × 200 necesita 200 × 201 = 40 200 bytes, así que la cota lo rechaza.
    let png = Png::new(200, 200, 8, 0).bytes();
    let r = read_png(&png);
    assert!(is_decode_with_bound_message(&r), "{r:?}");

    // La misma cabecera con `Z(needed)` es válida: la cota no la toca.
    let valid = file(200, 200, 8, 0, false, &[], Some(&z(40_200)));
    assert!(read_png(&valid).is_ok());
}

// ---------------------------------------------------------------------------------------
// Propiedad 2: razón máxima con dimensiones al azar.
// ---------------------------------------------------------------------------------------

/// Tipos de color y profundidades válidos: (color, profundidad, bits por píxel).
fn types_valid() -> Vec<(u8, u8, u64)> {
    let mut v = Vec::new();
    for (color, channels, depths) in [
        (0u8, 1u64, &[1u8, 2, 4, 8, 16][..]),
        (2, 3, &[8, 16][..]),
        (3, 1, &[1, 2, 4, 8][..]),
        (4, 2, &[8, 16][..]),
        (6, 4, &[8, 16][..]),
    ] {
        for &p in depths {
            v.push((color, p, channels * u64::from(p)));
        }
    }
    v
}

/// Ancho y alto de 1 a 3000 con `needed <= 2^23`: el alto se reduce hasta que cabe.
fn dimensions_bounded() -> impl Strategy<Value = (u32, u32, (u8, u8, u64), bool)> {
    (
        1u32..=3000,
        1u32..=3000,
        select(types_valid()),
        any::<bool>(),
    )
        .prop_map(|(width, mut height, kind, interlaced_2)| {
            while height > 1 && necessary(width, height, kind.2, interlaced_2) > 1 << 23 {
                height = (height * 3 / 4).max(1);
            }
            (width, height, kind, interlaced_2)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    // Propiedad 2: la cota nunca rechaza un fichero que se decodifica.
    #[test]
    fn p2_max_ratio_with_random_dimensions(
        (width, height, (color, prof, bpp), interlaced_2) in dimensions_bounded()
    ) {
        let needed = necessary(width, height, bpp, interlaced_2);
        prop_assume!(needed <= 1 << 23);
        let flow = z(needed);
        prop_assert!(needed <= 1032 * flow.len() as u64);
        let bytes = file(
            width, height, prof, color, interlaced_2, &previous(color, prof), Some(&flow),
        );
        let r = read_png(&bytes);
        prop_assert!(
            r.is_ok(),
            "{}x{} tipo {} prof {} Adam7 {}: {:?}",
            width, height, color, prof, interlaced_2, r.as_ref().err()
        );
        let r = r.unwrap();
        prop_assert_eq!((r.image.width(), r.image.height()), (width, height));
        prop_assert!(is_all_zero(&r.image, color));
    }
}
