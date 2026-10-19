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
//! `msg.starts_with(MENSAJE_COTA)`.

use super::*;

/// Comienzo del mensaje del error de la cota (spec, «Error»).
pub(super) const MENSAJE_COTA: &str = "tamaño declarado inconsistente con el fichero";

// ---------------------------------------------------------------------------------------
// `Z(n)`: flujo zlib de razón máxima.
// ---------------------------------------------------------------------------------------

/// Escritor de bits de DEFLATE: los campos numéricos van con el bit menos significativo primero y
/// los códigos de Huffman con el más significativo primero.
struct Bits {
    salida: Vec<u8>,
    acumulador: u32,
    n: u32,
}

impl Bits {
    fn new() -> Bits {
        Bits {
            salida: Vec::new(),
            acumulador: 0,
            n: 0,
        }
    }

    fn bit(&mut self, b: u32) {
        self.acumulador |= (b & 1) << self.n;
        self.n += 1;
        if self.n == 8 {
            self.salida.push(self.acumulador as u8);
            self.acumulador = 0;
            self.n = 0;
        }
    }

    /// Campo numérico de `n` bits, LSB primero.
    fn campo(&mut self, valor: u32, n: u32) {
        for i in 0..n {
            self.bit(valor >> i);
        }
    }

    /// Código de Huffman de `n` bits, MSB primero.
    fn codigo(&mut self, valor: u32, n: u32) {
        for i in (0..n).rev() {
            self.bit(valor >> i);
        }
    }

    fn termina(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.salida.push(self.acumulador as u8);
        }
        self.salida
    }
}

/// Cuántas coincidencias de longitud 258 hacen falta para llegar a `n` bytes: el flujo produce
/// `1 + 258 · k` bytes cero (un literal 0 y `k` coincidencias a distancia 1).
fn coincidencias(n: u64) -> u64 {
    if n <= 1 { 0 } else { (n - 1).div_ceil(258) }
}

/// Bytes que produce `z(n)`: `1 + 258 · k >= n`.
pub(super) fn bytes_de_z(n: u64) -> u64 {
    1 + 258 * coincidencias(n)
}

/// `Z(n)`: flujo zlib de razón máxima que produce `1 + 258 · k >= n` bytes cero.
///
/// Un único bloque dinámico: literal 0 con código de 2 bits, símbolo 285 (longitud 258) con 1 bit,
/// un único código de distancia de 1 bit y fin de bloque (256) con 2 bits; cada coincidencia
/// cuesta 2 bits. El Adler-32 es el de los bytes que de verdad produce el flujo
/// (`((L mod 65521) << 16) | 1`, con `L = 1 + 258 · k`).
pub(super) fn z(n: u64) -> Vec<u8> {
    let k = coincidencias(n);
    let producidos = bytes_de_z(n);

    let mut b = Bits::new();
    b.campo(1, 1); // BFINAL
    b.campo(2, 2); // BTYPE = 10 (códigos dinámicos)
    b.campo(286 - 257, 5); // HLIT
    b.campo(0, 5); // HDIST: un código de distancia
    // Alfabeto de longitudes: 0 con 1 bit, 1 y 2 con 2 bits.
    const ORDEN: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let mut largo_de = [0u32; 19];
    largo_de[0] = 1;
    largo_de[1] = 2;
    largo_de[2] = 2;
    let ultimo = ORDEN.iter().rposition(|&s| largo_de[s] != 0).unwrap();
    b.campo((ultimo + 1 - 4) as u32, 4); // HCLEN
    for &s in &ORDEN[..=ultimo] {
        b.campo(largo_de[s], 3);
    }
    // Códigos canónicos del alfabeto de longitudes: 0 -> '0', 1 -> '10', 2 -> '11'.
    let mut longitudes = [0u32; 286 + 1];
    longitudes[0] = 2;
    longitudes[256] = 2;
    longitudes[285] = 1;
    longitudes[286] = 1; // el único código de distancia
    for &l in &longitudes {
        match l {
            0 => b.codigo(0b0, 1),
            1 => b.codigo(0b10, 2),
            2 => b.codigo(0b11, 2),
            _ => unreachable!(),
        }
    }
    // Datos. Código de literal/longitud: 285 -> '0', 0 -> '10', 256 -> '11'; distancia 1 -> '0'.
    b.codigo(0b10, 2); // literal 0
    for _ in 0..k {
        b.codigo(0b0, 1); // símbolo 285 (longitud 258, sin bits extra)
        b.codigo(0b0, 1); // distancia 1
    }
    b.codigo(0b11, 2); // fin de bloque

    let mut v = vec![0x78, 0x01];
    v.extend(b.termina());
    v.extend(((((producidos % 65521) as u32) << 16) | 1).to_be_bytes());
    v
}

// ---------------------------------------------------------------------------------------
// Ficheros con un IDAT dado y el modelo de `needed` de las pruebas.
// ---------------------------------------------------------------------------------------

/// Fichero con `antes` entre IHDR e IDAT y, si `idat` es `Some`, ese IDAT (ya comprimido).
pub(super) fn fichero(
    ancho: u32,
    alto: u32,
    prof: u8,
    color: u8,
    entrelazado: bool,
    antes: &[Vec<u8>],
    idat: Option<&[u8]>,
) -> Vec<u8> {
    let mut ihdr = Vec::new();
    ihdr.extend(ancho.to_be_bytes());
    ihdr.extend(alto.to_be_bytes());
    ihdr.extend([prof, color, 0, 0, u8::from(entrelazado)]);
    let mut v = FIRMA.to_vec();
    v.extend(trozo(b"IHDR", &ihdr));
    for c in antes {
        v.extend(c);
    }
    if let Some(idat) = idat {
        v.extend(trozo(b"IDAT", idat));
    }
    v.extend(trozo(b"IEND", &[]));
    v
}

/// PLTE de `n` entradas negras.
pub(super) fn plte_negra(n: usize) -> Vec<u8> {
    plte(&vec![(0, 0, 0); n])
}

/// Bytes que el flujo zlib debe producir para la imagen por defecto, según la spec PNG:
/// `filas × (1 + ceil(columnas × bpp / 8))` por pasada, con las siete pasadas de Adam7 si
/// `entrelazado` (las vacías no cuentan). Modelo propio de la prueba, independiente del código.
pub(super) fn necesarios(ancho: u32, alto: u32, bpp: u64, entrelazado: bool) -> u64 {
    let pasada = |columnas: u64, filas: u64| {
        if columnas == 0 || filas == 0 {
            0
        } else {
            filas * (1 + (columnas * bpp).div_ceil(8))
        }
    };
    let (ancho, alto) = (u64::from(ancho), u64::from(alto));
    if !entrelazado {
        return pasada(ancho, alto);
    }
    ADAM7
        .iter()
        .map(|&(x0, y0, dx, dy)| {
            let cuenta = |total: u64, ini: usize, salto: usize| {
                if total <= ini as u64 {
                    0
                } else {
                    (total - ini as u64).div_ceil(salto as u64)
                }
            };
            pasada(cuenta(ancho, x0, dx), cuenta(alto, y0, dy))
        })
        .sum()
}

/// Las cuatro formas de la fila 5 en 8 bits: (nombre, tipo de color, bits por píxel, modo).
const FORMAS_8_BITS: [(&str, u8, u64, Mode); 4] = [
    ("gris 8", 0, 8, Mode::Gray),
    ("gris con alfa 8", 4, 16, Mode::GrayAlpha),
    ("RGBA 8", 6, 32, Mode::Rgba),
    ("indexado 8", 3, 8, Mode::Indexed),
];

/// Chunks previos al IDAT que necesita ese tipo de color (PLTE de 256 entradas en indexado).
fn previos(color: u8, prof: u8) -> Vec<Vec<u8>> {
    if color == 3 {
        vec![plte_negra(1 << prof)]
    } else {
        Vec::new()
    }
}

/// Los bytes de la imagen son los de «todo a cero»: con RGB (tipo 2) el alfa sale a 255.
fn es_todo_cero(img: &Image, color: u8) -> bool {
    if color == 2 {
        img.bytes().chunks_exact(4).all(|p| p == [0, 0, 0, 255])
    } else {
        img.bytes().iter().all(|&b| b == 0)
    }
}

#[allow(clippy::too_many_arguments)]
/// Lee `Z(needed)` para esa cabecera y comprueba que es `Ok`, todo cero y, en 8 bits, sin informe.
fn lee_con_z(
    nombre: &str,
    ancho: u32,
    alto: u32,
    prof: u8,
    color: u8,
    bpp: u64,
    entrelazado: bool,
    modo: Mode,
    razon_minima: u64,
) {
    let needed = necesarios(ancho, alto, bpp, entrelazado);
    let flujo = z(needed);
    // El flujo es de razón ajustada: `needed / idat` entre `razon_minima` y 1032. La cabecera
    // dinámica (~55 bytes) pesa más a 4096² (razón ≈ 1028,5) que a 8192² (≈ 1031,2, margen de
    // 0,08 %): `needed` no puede sobrestimarse más que ese margen sin que la cota rechace el
    // fichero, así que esta prueba también acota por arriba el error de `needed`.
    let idat = flujo.len() as u64;
    assert!(needed <= 1032 * idat, "{nombre}: razón por encima de 1032");
    assert!(
        needed >= razon_minima * idat,
        "{nombre}: razón por debajo de {razon_minima} ({needed} / {idat})"
    );
    let bytes = fichero(
        ancho,
        alto,
        prof,
        color,
        entrelazado,
        &previos(color, prof),
        Some(&flujo),
    );
    drop(flujo);
    let r = read_png(&bytes);
    let r = match r {
        Ok(r) => r,
        Err(e) => panic!("{nombre} {ancho}x{alto} (Adam7: {entrelazado}): {e:?}"),
    };
    assert_eq!(r.image.mode(), modo, "{nombre}");
    assert_eq!(
        (r.image.width(), r.image.height()),
        (ancho, alto),
        "{nombre}"
    );
    assert!(
        es_todo_cero(&r.image, color),
        "{nombre}: hay bytes distintos de 0"
    );
    assert!(r.report.is_empty(), "{nombre}: {:?}", r.report);
}

// ---------------------------------------------------------------------------------------
// Comprobación del constructor.
// ---------------------------------------------------------------------------------------

#[test]
fn z_anclas_del_prototipo() {
    // k = 260144 da 67 117 153 bytes en 65 088 bytes de zlib (razón 1031,18).
    let n = 1 + 258 * 260_144;
    assert_eq!(bytes_de_z(n), 67_117_153);
    assert_eq!(z(n).len(), 65_088);
    // Cabecera zlib y Adler-32 de `producidos` ceros.
    let flujo = z(n);
    assert_eq!(&flujo[..2], [0x78, 0x01]);
    let adler = (((67_117_153u64 % 65521) as u32) << 16) | 1;
    assert_eq!(&flujo[flujo.len() - 4..], adler.to_be_bytes());
    // `bytes_de_z` redondea hacia arriba y nunca baja de `n`.
    assert_eq!(bytes_de_z(0), 1);
    assert_eq!(bytes_de_z(1), 1);
    assert_eq!(bytes_de_z(2), 259);
    assert_eq!(bytes_de_z(259), 259);
    assert_eq!(bytes_de_z(260), 517);
}

#[test]
fn z_pequeno_se_decodifica_en_gris_1x1_y_en_gris_3x2() {
    // 1 x 1 gris 8: needed = 2 (filtro + muestra); Z(2) produce 259 bytes, de sobra.
    let uno = fichero(1, 1, 8, 0, false, &[], Some(&z(2)));
    let r = read_png(&uno).unwrap();
    assert_eq!(pixeles(&r.image), grises(&[0]));
    let tres_por_dos = fichero(3, 2, 8, 0, false, &[], Some(&z(8)));
    let r = read_png(&tres_por_dos).unwrap();
    assert_eq!(pixeles(&r.image), grises(&[0; 6]));
}

// ---------------------------------------------------------------------------------------
// Fila 5: cota ajustada con Z.
// ---------------------------------------------------------------------------------------

/// Gris 8, gris con alfa, RGBA e indexado de `lado`² con `Z(needed)`, y gris 8 entrelazado.
fn fila5(lado: u32) {
    let minima = if lado >= 8192 { 1030 } else { 1025 };
    for (nombre, color, bpp, modo) in FORMAS_8_BITS {
        lee_con_z(nombre, lado, lado, 8, color, bpp, false, modo, minima);
    }
    lee_con_z(
        "gris 8 Adam7",
        lado,
        lado,
        8,
        0,
        8,
        true,
        Mode::Gray,
        minima,
    );
}

#[test]
fn fila5_cota_ajustada_con_z_a_4096() {
    fila5(4096);
}

#[test]
#[ignore = "8192²: lento sin optimizar; se ejecuta al cerrar con --release -- --ignored"]
fn fila5_cota_ajustada_con_z_a_8192() {
    fila5(8192);
}

// ---------------------------------------------------------------------------------------
// Fila 6: un solo color con el codificador por defecto.
// ---------------------------------------------------------------------------------------

fn un_solo_color(lado: u32) {
    let paleta = Palette::from_colors([
        Rgba::opaque(1, 2, 3),
        Rgba::opaque(40, 50, 60),
        Rgba::opaque(7, 8, 9),
    ])
    .unwrap();
    let constructores: [(&str, Box<dyn Fn() -> Image>); 4] = [
        (
            "gris",
            Box::new(move || Image::filled(lado, lado, Pixel::Gray(77)).unwrap()),
        ),
        (
            "gris con alfa",
            Box::new(move || {
                Image::filled(
                    lado,
                    lado,
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
            Box::new(move || Image::filled_indexed(lado, lado, paleta.clone(), 1).unwrap()),
        ),
        (
            "RGBA",
            Box::new(move || {
                Image::filled(lado, lado, Pixel::Rgba(Rgba::new(10, 20, 30, 40))).unwrap()
            }),
        ),
    ];
    for (nombre, construye) in constructores {
        let img = construye();
        let bytes = write_png(&img).unwrap();
        let esperada = img;
        let r = match read_png(&bytes) {
            Ok(r) => r,
            Err(e) => panic!("{nombre} {lado}²: {e:?}"),
        };
        drop(bytes);
        assert!(r.report.is_empty(), "{nombre}: {:?}", r.report);
        assert!(r.image == esperada, "{nombre}: la imagen leída es distinta");
    }
}

#[test]
fn fila6_un_solo_color_con_el_codificador_por_defecto_a_4096() {
    un_solo_color(4096);
}

#[test]
#[ignore = "8192²: lento sin optimizar; se ejecuta al cerrar con --release -- --ignored"]
fn fila6_un_solo_color_con_el_codificador_por_defecto_a_8192() {
    un_solo_color(8192);
}

// ---------------------------------------------------------------------------------------
// Fila 8: orden de errores, truncados e IDAT declarado enorme.
// ---------------------------------------------------------------------------------------

fn es_decode_con_mensaje_de_cota(r: &Result<PngRead, PngError>) -> bool {
    matches!(r, Err(PngError::Decode(m)) if m.starts_with(MENSAJE_COTA))
}

#[test]
fn fila8_too_large_gana_a_la_cota() {
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
fn fila8_paleta_y_trns_invalidos_ganan_a_la_cota() {
    // Indexado de 8192 × 8192 sin PLTE (IDAT mínimo): `InvalidPalette`.
    let sin_plte = Png::new(8192, 8192, 8, 3).bytes();
    assert_eq!(error_de(&sin_plte), PngError::InvalidPalette);

    // Indexado con tRNS de 3 entradas y PLTE de 2 (8192 × 8192, IDAT mínimo): `InvalidTrns`.
    let trns_largo = Png::new(8192, 8192, 8, 3)
        .antes(plte(&[(1, 1, 1), (2, 2, 2)]))
        .antes(trns(&[0, 1, 2]))
        .bytes();
    assert_eq!(error_de(&trns_largo), PngError::InvalidTrns);
}

#[test]
fn fila8_truncado_a_media_idat_es_decode() {
    // Ruido de 64 × 64 (xorshift fijo) cortado a media IDAT: lo rechaza `scan_chunks`.
    let mut s = 0x9E37_79B9_7F4A_7C15u64;
    let muestras: Vec<u16> = (0..64 * 64)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 24) as u8 as u16
        })
        .collect();
    let valido = Png::new(64, 64, 8, 0).muestras(&muestras).bytes();
    assert!(read_png(&valido).is_ok());
    let (inicio, largo) = posicion_idat(&valido);
    let cortado = &valido[..inicio + 8 + largo / 2];
    let r = read_png(cortado);
    assert!(es_decode(&r), "{r:?}");
}

#[test]
fn fila8_sin_idat_es_decode() {
    // Solo IHDR e IEND, 8 × 8 gris 8.
    let bytes = fichero(8, 8, 8, 0, false, &[], None);
    let r = read_png(&bytes);
    assert!(es_decode(&r), "{r:?}");
}

#[test]
fn fila8_idat_declarado_enorme_en_fichero_corto_es_decode() {
    // Gris de 8192 × 8192 con un IDAT que declara 2 GiB y un fichero de ~100 bytes: el chunk
    // truncado ya lo rechaza `scan_chunks` y no aporta bytes a la cota.
    let mut v = fichero(8192, 8192, 8, 0, false, &[], None);
    v.truncate(v.len() - 12); // quita IEND
    v.extend(0x8000_0000u32.to_be_bytes());
    v.extend(b"IDAT");
    v.extend([0x78, 0x9c, 0x63, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01]);
    v.extend(trozo(b"IEND", &[]));
    assert!(v.len() < 128, "{} bytes", v.len());
    let r = read_png(&v);
    assert!(es_decode(&r), "{r:?}");
}

#[test]
fn fila8_la_cota_rechaza_con_su_mensaje_un_fichero_sin_datos_suficientes() {
    // Parte de «mensaje» de la fila 8, en ROJO hoy (hoy es otro `Decode`). El IDAT mínimo de
    // `Png` mide 11 bytes (zlib con un bloque «stored» vacío): 1032 × 11 = 11 352. Un gris 8 de
    // 200 × 200 necesita 200 × 201 = 40 200 bytes, así que la cota lo rechaza.
    let png = Png::new(200, 200, 8, 0).bytes();
    let r = read_png(&png);
    assert!(es_decode_con_mensaje_de_cota(&r), "{r:?}");

    // La misma cabecera con `Z(needed)` es válida: la cota no la toca.
    let valido = fichero(200, 200, 8, 0, false, &[], Some(&z(40_200)));
    assert!(read_png(&valido).is_ok());
}

// ---------------------------------------------------------------------------------------
// Propiedad 2: razón máxima con dimensiones al azar.
// ---------------------------------------------------------------------------------------

/// Tipos de color y profundidades válidos: (color, profundidad, bits por píxel).
fn tipos_validos() -> Vec<(u8, u8, u64)> {
    let mut v = Vec::new();
    for (color, canales, profundidades) in [
        (0u8, 1u64, &[1u8, 2, 4, 8, 16][..]),
        (2, 3, &[8, 16][..]),
        (3, 1, &[1, 2, 4, 8][..]),
        (4, 2, &[8, 16][..]),
        (6, 4, &[8, 16][..]),
    ] {
        for &p in profundidades {
            v.push((color, p, canales * u64::from(p)));
        }
    }
    v
}

/// Ancho y alto de 1 a 3000 con `needed <= 2^23`: el alto se reduce hasta que cabe.
fn dimensiones_acotadas() -> impl Strategy<Value = (u32, u32, (u8, u8, u64), bool)> {
    (
        1u32..=3000,
        1u32..=3000,
        select(tipos_validos()),
        any::<bool>(),
    )
        .prop_map(|(ancho, mut alto, tipo, entrelazado)| {
            while alto > 1 && necesarios(ancho, alto, tipo.2, entrelazado) > 1 << 23 {
                alto = (alto * 3 / 4).max(1);
            }
            (ancho, alto, tipo, entrelazado)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    // Propiedad 2: la cota nunca rechaza un fichero que se decodifica.
    #[test]
    fn p2_razon_maxima_con_dimensiones_al_azar(
        (ancho, alto, (color, prof, bpp), entrelazado) in dimensiones_acotadas()
    ) {
        let needed = necesarios(ancho, alto, bpp, entrelazado);
        prop_assume!(needed <= 1 << 23);
        let flujo = z(needed);
        prop_assert!(needed <= 1032 * flujo.len() as u64);
        let bytes = fichero(
            ancho, alto, prof, color, entrelazado, &previos(color, prof), Some(&flujo),
        );
        let r = read_png(&bytes);
        prop_assert!(
            r.is_ok(),
            "{}x{} tipo {} prof {} Adam7 {}: {:?}",
            ancho, alto, color, prof, entrelazado, r.as_ref().err()
        );
        let r = r.unwrap();
        prop_assert_eq!((r.image.width(), r.image.height()), (ancho, alto));
        prop_assert!(es_todo_cero(&r.image, color));
    }
}
