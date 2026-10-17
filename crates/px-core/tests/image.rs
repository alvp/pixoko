//! Tests de `Image`, imagen en modos RGBA, indexado y grises (specs/hito-1-image.md).
//!
//! Ningún test reserva más de unos pocos KiB: el límite exacto de `Image::MAX_BYTES`
//! se prueba con `Image::byte_len`, que no reserva; `new` solo se prueba con tamaños
//! pequeños y por el lado del rechazo.

use std::fmt::{Debug, Display};
use std::hash::Hash;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Image, ImageError, Mode, Palette, Pixel, Rgba};

// Comprobaciones en tiempo de compilación de los traits exigidos.
fn assert_value_traits<T: Clone + Copy + Debug + PartialEq + Eq + Hash>() {}
fn assert_image_traits<T: Clone + Debug + PartialEq + Eq>() {}
fn assert_error_traits<T: Debug + Clone + PartialEq + Eq + Display + std::error::Error>() {}

#[test]
fn traits_derivados() {
    assert_value_traits::<Mode>();
    assert_value_traits::<Pixel>();
    assert_image_traits::<Image>();
    assert_error_traits::<ImageError>();
}

#[test]
fn max_bytes_vale_1_gib() {
    assert_eq!(Image::MAX_BYTES, 1usize << 30);
}

// Colores, paletas e imágenes de apoyo.

const NO_INDEXADOS: [Mode; 3] = [Mode::Rgba, Mode::Gray, Mode::GrayAlpha];
const TODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

fn c0() -> Rgba {
    Rgba::new(10, 20, 30, 255)
}

fn c1() -> Rgba {
    Rgba::new(40, 50, 60, 255)
}

fn paleta(colores: &[Rgba]) -> Palette {
    Palette::from_colors(colores.iter().copied()).unwrap()
}

fn paleta_de(n: usize) -> Palette {
    Palette::from_colors((0..n).map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))).unwrap()
}

/// Crea una imagen de cualquier modo; el indexado usa una paleta de `n` entradas.
fn crea(mode: Mode, width: u32, height: u32, n: usize) -> Image {
    match mode {
        Mode::Indexed => Image::new_indexed(width, height, paleta_de(n)).unwrap(),
        _ => Image::new(width, height, mode).unwrap(),
    }
}

/// Imagen 3 x 2 de cualquier modo (el indexado con paleta de 2).
fn imagen_3x2(mode: Mode) -> Image {
    crea(mode, 3, 2, 2)
}

fn inicial(mode: Mode) -> Pixel {
    match mode {
        Mode::Rgba => Pixel::Rgba(Rgba::TRANSPARENT),
        Mode::Indexed => Pixel::Indexed(0),
        Mode::Gray => Pixel::Gray(0),
        Mode::GrayAlpha => Pixel::GrayAlpha { gray: 0, alpha: 0 },
    }
}

fn modo_de(pixel: Pixel) -> Mode {
    match pixel {
        Pixel::Rgba(_) => Mode::Rgba,
        Pixel::Indexed(_) => Mode::Indexed,
        Pixel::Gray(_) => Mode::Gray,
        Pixel::GrayAlpha { .. } => Mode::GrayAlpha,
    }
}

fn bytes_por_pixel(mode: Mode) -> u128 {
    match mode {
        Mode::Rgba => 4,
        Mode::GrayAlpha => 2,
        Mode::Indexed | Mode::Gray => 1,
    }
}

/// Todos los píxeles de la imagen, fila a fila (todas las coordenadas son válidas).
fn todos_los_pixeles(img: &Image) -> Vec<Pixel> {
    let mut v = Vec::new();
    for y in 0..img.height() {
        for x in 0..img.width() {
            v.push(img.get_pixel(x, y).unwrap());
        }
    }
    v
}

fn fuera(x: u32, y: u32, width: u32, height: u32) -> ImageError {
    ImageError::OutOfBounds {
        x,
        y,
        width,
        height,
    }
}

fn desajuste(expected: Mode, found: Mode) -> ImageError {
    ImageError::ModeMismatch { expected, found }
}

fn fuera_de_paleta(index: u8, len: usize) -> ImageError {
    ImageError::IndexNotInPalette { index, len }
}

fn es_too_large<T: Debug>(r: Result<T, ImageError>) -> bool {
    matches!(r, Err(ImageError::TooLarge { .. }))
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn new_4x3_rgba() {
    let img = Image::new(4, 3, Mode::Rgba).unwrap();
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 3);
    assert_eq!(img.mode(), Mode::Rgba);
    assert_eq!(img.palette(), None);
    let pixeles = todos_los_pixeles(&img);
    assert_eq!(pixeles.len(), 12);
    assert!(pixeles.iter().all(|&p| p == Pixel::Rgba(Rgba::TRANSPARENT)));
}

#[test]
fn new_2x2_gray_y_gray_alpha_inicializan_a_cero() {
    let g = Image::new(2, 2, Mode::Gray).unwrap();
    assert_eq!(g.mode(), Mode::Gray);
    assert_eq!(g.palette(), None);
    assert_eq!(todos_los_pixeles(&g), vec![Pixel::Gray(0); 4]);

    let ga = Image::new(2, 2, Mode::GrayAlpha).unwrap();
    assert_eq!(ga.mode(), Mode::GrayAlpha);
    assert_eq!(ga.palette(), None);
    assert_eq!(
        todos_los_pixeles(&ga),
        vec![Pixel::GrayAlpha { gray: 0, alpha: 0 }; 4]
    );
}

#[test]
fn new_1x1_en_cada_modo_no_indexado() {
    for mode in NO_INDEXADOS {
        let img = Image::new(1, 1, mode).unwrap();
        assert_eq!((img.width(), img.height()), (1, 1));
        assert_eq!(img.mode(), mode);
        assert_eq!(img.get_pixel(0, 0), Ok(inicial(mode)));
    }
}

#[test]
fn new_con_ancho_o_alto_cero_es_zero_size() {
    for mode in NO_INDEXADOS {
        for (w, h) in [(0u32, 5u32), (5, 0), (0, 0)] {
            assert_eq!(
                Image::new(w, h, mode),
                Err(ImageError::ZeroSize {
                    width: w,
                    height: h
                }),
                "new({w}, {h}, {mode:?})"
            );
        }
    }
}

#[test]
fn new_con_producto_desbordado_es_too_large() {
    assert!(es_too_large(Image::new(u32::MAX, u32::MAX, Mode::Rgba)));
    assert!(es_too_large(Image::new(u32::MAX, u32::MAX, Mode::Gray)));
    assert!(es_too_large(Image::new(
        u32::MAX,
        u32::MAX,
        Mode::GrayAlpha
    )));
}

#[test]
fn new_por_encima_de_max_bytes_es_too_large() {
    assert!(es_too_large(Image::new(u32::MAX, 1, Mode::Gray)));
    assert!(es_too_large(Image::new(16385, 16384, Mode::Rgba)));
}

#[test]
fn byte_len_en_el_limite_exacto_cabe() {
    let tope = Image::MAX_BYTES;
    assert_eq!(Image::byte_len(16384, 16384, Mode::Rgba), Ok(tope));
    assert_eq!(Image::byte_len(tope as u32, 1, Mode::Gray), Ok(tope));
    assert_eq!(Image::byte_len(1, tope as u32, Mode::Gray), Ok(tope));
    assert_eq!(
        Image::byte_len(tope as u32 / 2, 1, Mode::GrayAlpha),
        Ok(tope)
    );
    assert_eq!(
        Image::byte_len(16384, 16384, Mode::Indexed),
        Ok(16384 * 16384)
    );
}

#[test]
fn byte_len_un_byte_por_encima_es_too_large() {
    let uno_mas = Image::MAX_BYTES as u32 + 1;
    for (width, height, mode) in [
        (uno_mas, 1, Mode::Gray),
        (1, uno_mas, Mode::Gray),
        (uno_mas, 1, Mode::Indexed),
        (16385, 16384, Mode::Rgba),
        (16384, 16385, Mode::Rgba),
        (uno_mas / 2 + 1, 1, Mode::GrayAlpha),
        (u32::MAX, u32::MAX, Mode::Rgba),
    ] {
        assert_eq!(
            Image::byte_len(width, height, mode),
            Err(ImageError::TooLarge { width, height }),
            "{width} x {height} en {mode:?}"
        );
    }
}

#[test]
fn byte_len_con_cero_es_zero_size() {
    for mode in TODOS {
        assert_eq!(
            Image::byte_len(0, 5, mode),
            Err(ImageError::ZeroSize {
                width: 0,
                height: 5
            })
        );
        assert_eq!(
            Image::byte_len(5, 0, mode),
            Err(ImageError::ZeroSize {
                width: 5,
                height: 0
            })
        );
        assert_eq!(
            Image::byte_len(0, 0, mode),
            Err(ImageError::ZeroSize {
                width: 0,
                height: 0
            })
        );
    }
}

#[test]
fn byte_len_pequenos_por_modo() {
    for mode in TODOS {
        assert_eq!(
            Image::byte_len(1, 1, mode),
            Ok(bytes_por_pixel(mode) as usize)
        );
        assert_eq!(
            Image::byte_len(4, 3, mode),
            Ok(12 * bytes_por_pixel(mode) as usize)
        );
    }
}

#[test]
fn new_indexed_sin_paleta_y_orden_de_comprobacion() {
    assert_eq!(
        Image::new(2, 2, Mode::Indexed),
        Err(ImageError::PaletteRequired)
    );
    // Las dimensiones se comprueban antes que la paleta.
    assert_eq!(
        Image::new(0, 2, Mode::Indexed),
        Err(ImageError::ZeroSize {
            width: 0,
            height: 2
        })
    );
}

#[test]
fn new_indexed_demasiado_grande_gana_a_paleta_requerida() {
    // Comportamiento 3: ZeroSize, luego TooLarge, luego PaletteRequired.
    assert!(es_too_large(Image::new(u32::MAX, u32::MAX, Mode::Indexed)));
    assert!(es_too_large(Image::new(u32::MAX, 1, Mode::Indexed)));
}

#[test]
fn new_indexed_valido() {
    let p = paleta(&[c0(), c1()]);
    let img = Image::new_indexed(2, 2, p.clone()).unwrap();
    assert_eq!(img.mode(), Mode::Indexed);
    assert_eq!((img.width(), img.height()), (2, 2));
    assert_eq!(img.palette(), Some(&p));
    assert_eq!(todos_los_pixeles(&img), vec![Pixel::Indexed(0); 4]);
}

#[test]
fn new_indexed_errores() {
    assert_eq!(
        Image::new_indexed(2, 2, Palette::new()),
        Err(ImageError::EmptyPalette)
    );
    assert_eq!(
        Image::new_indexed(0, 2, Palette::new()),
        Err(ImageError::ZeroSize {
            width: 0,
            height: 2
        })
    );
    assert!(es_too_large(Image::new_indexed(
        u32::MAX,
        u32::MAX,
        paleta(&[c0()])
    )));
}

#[test]
fn new_indexed_dimensiones_se_comprueban_antes_que_la_paleta_vacia() {
    // Comportamiento 3: TooLarge antes que EmptyPalette.
    assert!(es_too_large(Image::new_indexed(
        u32::MAX,
        u32::MAX,
        Palette::new()
    )));
    assert_eq!(
        Image::new_indexed(5, 0, Palette::new()),
        Err(ImageError::ZeroSize {
            width: 5,
            height: 0
        })
    );
}

#[test]
fn get_pixel_en_esquinas_y_fuera() {
    for mode in TODOS {
        let img = imagen_3x2(mode);
        assert_eq!(img.get_pixel(0, 0), Ok(inicial(mode)), "{mode:?}");
        assert_eq!(img.get_pixel(2, 1), Ok(inicial(mode)), "{mode:?}");
        for (x, y) in [(3u32, 0u32), (0, 2), (3, 2), (u32::MAX, u32::MAX)] {
            assert_eq!(
                img.get_pixel(x, y),
                Err(fuera(x, y, 3, 2)),
                "{mode:?} en ({x}, {y})"
            );
        }
    }
}

/// Escribe `pixel` en `(x, y)` y comprueba la ida y vuelta y que nada más cambia.
fn escribe_y_comprueba(img: &mut Image, x: u32, y: u32, pixel: Pixel) {
    let antes = todos_los_pixeles(img);
    assert_eq!(img.set_pixel(x, y, pixel), Ok(()), "set_pixel({x}, {y})");
    assert_eq!(img.get_pixel(x, y), Ok(pixel), "get_pixel({x}, {y})");
    let despues = todos_los_pixeles(img);
    let ancho = img.width();
    for (k, (a, d)) in antes.iter().zip(&despues).enumerate() {
        let (kx, ky) = (k as u32 % ancho, k as u32 / ancho);
        if (kx, ky) != (x, y) {
            assert_eq!(a, d, "cambió el píxel ({kx}, {ky})");
        }
    }
}

#[test]
fn set_y_get_en_cada_modo() {
    let casos: [(Mode, Vec<Pixel>); 4] = [
        (
            Mode::Rgba,
            vec![
                Pixel::Rgba(c0()),
                Pixel::Rgba(Rgba::new(1, 2, 3, 0)),
                Pixel::Rgba(Rgba::new(255, 255, 255, 255)),
            ],
        ),
        (Mode::Indexed, vec![Pixel::Indexed(1), Pixel::Indexed(0)]),
        (Mode::Gray, vec![Pixel::Gray(7), Pixel::Gray(255)]),
        (
            Mode::GrayAlpha,
            vec![
                Pixel::GrayAlpha { gray: 7, alpha: 9 },
                Pixel::GrayAlpha { gray: 7, alpha: 0 },
            ],
        ),
    ];
    for (mode, pixeles) in casos {
        for pixel in pixeles {
            // Esquinas y un punto intermedio.
            for (x, y) in [(0u32, 0u32), (2, 1), (1, 0), (1, 1)] {
                let mut img = imagen_3x2(mode);
                escribe_y_comprueba(&mut img, x, y, pixel);
            }
        }
    }
}

#[test]
fn set_conserva_el_rgb_con_alfa_cero() {
    let mut img = Image::new(3, 2, Mode::Rgba).unwrap();
    let p = Pixel::Rgba(Rgba::new(1, 2, 3, 0));
    img.set_pixel(1, 1, p).unwrap();
    assert_eq!(img.get_pixel(1, 1), Ok(p));
    assert_ne!(p, Pixel::Rgba(Rgba::TRANSPARENT));

    let mut ga = Image::new(3, 2, Mode::GrayAlpha).unwrap();
    let q = Pixel::GrayAlpha {
        gray: 200,
        alpha: 0,
    };
    ga.set_pixel(0, 1, q).unwrap();
    assert_eq!(ga.get_pixel(0, 1), Ok(q));
}

#[test]
fn set_fuera_de_la_imagen_es_out_of_bounds_sin_cambios() {
    let validos = [
        (Mode::Rgba, Pixel::Rgba(c0())),
        (Mode::Indexed, Pixel::Indexed(1)),
        (Mode::Gray, Pixel::Gray(7)),
        (Mode::GrayAlpha, Pixel::GrayAlpha { gray: 7, alpha: 9 }),
    ];
    for (mode, pixel) in validos {
        for (x, y) in [(3u32, 0u32), (u32::MAX, 0), (0, 2), (0, u32::MAX)] {
            let mut img = imagen_3x2(mode);
            let copia = img.clone();
            assert_eq!(
                img.set_pixel(x, y, pixel),
                Err(fuera(x, y, 3, 2)),
                "{mode:?} en ({x}, {y})"
            );
            assert_eq!(img, copia);
        }
    }
}

#[test]
fn set_de_otro_modo_es_mode_mismatch_sin_cambios() {
    let casos = [
        (Mode::Rgba, Pixel::Gray(1)),
        (Mode::Gray, Pixel::GrayAlpha { gray: 1, alpha: 2 }),
        (Mode::Rgba, Pixel::Indexed(0)),
    ];
    for (mode, pixel) in casos {
        let mut img = imagen_3x2(mode);
        let copia = img.clone();
        assert_eq!(
            img.set_pixel(0, 0, pixel),
            Err(desajuste(mode, modo_de(pixel))),
            "{pixel:?} en una imagen {mode:?}"
        );
        assert_eq!(img, copia);
    }
}

#[test]
fn set_gray_y_gray_alpha_son_modos_distintos() {
    let mut ga = imagen_3x2(Mode::GrayAlpha);
    let copia = ga.clone();
    assert_eq!(
        ga.set_pixel(0, 0, Pixel::Gray(1)),
        Err(desajuste(Mode::GrayAlpha, Mode::Gray))
    );
    assert_eq!(ga, copia);
}

#[test]
fn set_indexado_dentro_y_fuera_de_la_paleta() {
    // Paleta de 2.
    let mut img = Image::new_indexed(3, 2, paleta_de(2)).unwrap();
    assert_eq!(img.set_pixel(0, 0, Pixel::Indexed(1)), Ok(()));
    assert_eq!(img.get_pixel(0, 0), Ok(Pixel::Indexed(1)));

    for index in [2u8, 255] {
        let copia = img.clone();
        assert_eq!(
            img.set_pixel(1, 1, Pixel::Indexed(index)),
            Err(fuera_de_paleta(index, 2))
        );
        assert_eq!(img, copia);
    }

    // Paleta de 1.
    let mut una = Image::new_indexed(3, 2, paleta_de(1)).unwrap();
    let copia = una.clone();
    assert_eq!(
        una.set_pixel(0, 0, Pixel::Indexed(1)),
        Err(fuera_de_paleta(1, 1))
    );
    assert_eq!(una, copia);
    assert_eq!(una.set_pixel(0, 0, Pixel::Indexed(0)), Ok(()));

    // Paleta de 256.
    let mut llena = Image::new_indexed(3, 2, paleta_de(256)).unwrap();
    assert_eq!(llena.set_pixel(2, 1, Pixel::Indexed(255)), Ok(()));
    assert_eq!(llena.get_pixel(2, 1), Ok(Pixel::Indexed(255)));
}

#[test]
fn indexado_con_indice_transparente_no_se_resuelve() {
    let mut p = paleta(&[c0(), c1()]);
    p.set_transparent_index(Some(1)).unwrap();
    let mut img = Image::new_indexed(2, 2, p).unwrap();

    assert_eq!(img.set_pixel(0, 0, Pixel::Indexed(1)), Ok(()));
    assert_eq!(img.get_pixel(0, 0), Ok(Pixel::Indexed(1)));
    assert_eq!(
        img.palette().unwrap().resolved(1),
        Ok(Rgba::new(c1().r, c1().g, c1().b, 0))
    );
}

#[test]
fn orden_de_comprobacion_de_set_pixel() {
    // OutOfBounds gana a ModeMismatch.
    let mut img = imagen_3x2(Mode::Rgba);
    let copia = img.clone();
    assert_eq!(img.set_pixel(3, 0, Pixel::Gray(1)), Err(fuera(3, 0, 3, 2)));
    assert_eq!(img, copia);

    // ModeMismatch gana a IndexNotInPalette: Indexed(9) en una imagen Rgba.
    assert_eq!(
        img.set_pixel(0, 0, Pixel::Indexed(9)),
        Err(desajuste(Mode::Rgba, Mode::Indexed))
    );
    assert_eq!(img, copia);

    // OutOfBounds gana a IndexNotInPalette en una imagen indexada.
    let mut idx = imagen_3x2(Mode::Indexed);
    let copia = idx.clone();
    assert_eq!(
        idx.set_pixel(0, 2, Pixel::Indexed(9)),
        Err(fuera(0, 2, 3, 2))
    );
    assert_eq!(idx, copia);

    // Un píxel de otro modo en una imagen indexada es ModeMismatch.
    assert_eq!(
        idx.set_pixel(0, 0, Pixel::Gray(9)),
        Err(desajuste(Mode::Indexed, Mode::Gray))
    );
    assert_eq!(idx, copia);
}

// Estrategias.

fn color() -> impl Strategy<Value = Rgba> {
    (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
        .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
}

fn modo() -> impl Strategy<Value = Mode> {
    prop::sample::select(TODOS.to_vec())
}

fn modo_no_indexado() -> impl Strategy<Value = Mode> {
    prop::sample::select(NO_INDEXADOS.to_vec())
}

/// Imagen pequeña (1 a 16 de lado) de cualquier modo; el indexado lleva una
/// paleta de 1 a 256 entradas.
fn imagen_pequena() -> impl Strategy<Value = Image> {
    (modo(), 1u32..=16, 1u32..=16, 1usize..=256).prop_map(|(m, w, h, n)| crea(m, w, h, n))
}

/// Imagen pequeña con paleta de 1 a 256 entradas, siempre indexada.
fn imagen_indexada() -> impl Strategy<Value = Image> {
    (1u32..=16, 1u32..=16, 1usize..=256).prop_map(|(w, h, n)| crea(Mode::Indexed, w, h, n))
}

/// Píxel de cualquier variante, con índices que a menudo caen fuera de la paleta.
fn pixel_cualquiera() -> impl Strategy<Value = Pixel> {
    prop_oneof![
        color().prop_map(Pixel::Rgba),
        prop_oneof![3 => 0u8..4, 1 => any::<u8>()].prop_map(Pixel::Indexed),
        any::<u8>().prop_map(Pixel::Gray),
        (any::<u8>(), any::<u8>()).prop_map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha }),
    ]
}

/// Píxel de cualquier variante con índice `u8` cualquiera (para el invariante).
fn pixel_con_indice_libre() -> impl Strategy<Value = Pixel> {
    prop_oneof![
        4 => any::<u8>().prop_map(Pixel::Indexed),
        1 => pixel_cualquiera(),
    ]
}

/// Construye un píxel válido para `img` a partir de bytes arbitrarios.
fn pixel_valido(img: &Image, (a, b, c, d): (u8, u8, u8, u8)) -> Pixel {
    match img.mode() {
        Mode::Rgba => Pixel::Rgba(Rgba::new(a, b, c, d)),
        Mode::Indexed => {
            let len = img.palette().unwrap().len();
            Pixel::Indexed((a as usize % len) as u8)
        }
        Mode::Gray => Pixel::Gray(a),
        Mode::GrayAlpha => Pixel::GrayAlpha { gray: a, alpha: b },
    }
}

/// Casos de la propiedad 1: `(width, height, modo)`. La última rama cae justo
/// por encima de `MAX_BYTES`, para probar el límite sin reservar memoria.
fn caso_de_tamano() -> impl Strategy<Value = (u32, u32, Mode)> {
    prop_oneof![
        (0u32..=32, 0u32..=32, modo_no_indexado()),
        (any::<u32>(), any::<u32>(), modo_no_indexado()),
        (Just(0u32), any::<u32>(), modo_no_indexado()),
        (any::<u32>(), Just(0u32), modo_no_indexado()),
        (modo_no_indexado(), 1u32..=70_000, 1u64..=3).prop_map(|(m, h, d)| {
            let por_fila = bytes_por_pixel(m) as u64 * u64::from(h);
            let w = Image::MAX_BYTES as u64 / por_fila + d;
            (w as u32, h, m)
        }),
    ]
}

/// Casilla (x, y) de `u32` completo, con sesgo hacia valores cercanos a la imagen.
fn coordenada() -> impl Strategy<Value = u32> {
    prop_oneof![
        3 => 0u32..20,
        2 => any::<u32>(),
        1 => Just(u32::MAX),
    ]
}

// Modelo de referencia para las secuencias de `set_pixel`.

/// Resultado esperado de `set_pixel` según el orden del comportamiento 6.
fn esperado(img: &Image, x: u32, y: u32, pixel: Pixel) -> Result<(), ImageError> {
    let (width, height) = (img.width(), img.height());
    if x >= width || y >= height {
        return Err(fuera(x, y, width, height));
    }
    if modo_de(pixel) != img.mode() {
        return Err(desajuste(img.mode(), modo_de(pixel)));
    }
    if let Pixel::Indexed(index) = pixel {
        let len = img.palette().unwrap().len();
        if index as usize >= len {
            return Err(fuera_de_paleta(index, len));
        }
    }
    Ok(())
}

/// Aplica una secuencia de `(x_bruto, y_bruto, pixel)` y la compara con un
/// `Vec<Pixel>` como modelo. Las coordenadas caen en un rango algo mayor que la imagen.
fn ejecuta_secuencia(img: &mut Image, ops: &[(u32, u32, Pixel)]) -> Result<(), TestCaseError> {
    let (width, height) = (img.width(), img.height());
    let mut modelo = vec![inicial(img.mode()); (width * height) as usize];
    let palette_inicial = img.palette().cloned();

    for &(xb, yb, pixel) in ops {
        let (x, y) = (xb % (width + 4), yb % (height + 4));
        let copia = img.clone();
        let previsto = esperado(img, x, y, pixel);

        let real = img.set_pixel(x, y, pixel);
        prop_assert_eq!(&real, &previsto, "set_pixel({}, {}, {:?})", x, y, pixel);

        if real.is_ok() {
            modelo[(y * width + x) as usize] = pixel;
        } else {
            prop_assert_eq!(&*img, &copia);
        }
        prop_assert_eq!(todos_los_pixeles(img), modelo.clone());
        prop_assert_eq!((img.width(), img.height()), (width, height));
        prop_assert_eq!(img.palette().cloned(), palette_inicial.clone());
    }
    Ok(())
}

// Propiedades.

proptest! {
    #[test]
    fn dimensiones_y_tamano((width, height, mode) in caso_de_tamano()) {
        let bytes = u128::from(width) * u128::from(height) * bytes_por_pixel(mode);
        let tope = Image::MAX_BYTES as u128;
        // Las imágenes que serían Ok se limitan a un tamaño pequeño (4 KiB).
        prop_assume!(width == 0 || height == 0 || bytes > tope || bytes <= 4096);

        let r = Image::new(width, height, mode);
        if width == 0 || height == 0 {
            prop_assert_eq!(r, Err(ImageError::ZeroSize { width, height }));
        } else if bytes > tope {
            prop_assert!(
                matches!(r, Err(ImageError::TooLarge { .. })),
                "esperaba TooLarge, salió {:?}", r
            );
        } else {
            let img = r.unwrap();
            prop_assert_eq!(img.width(), width);
            prop_assert_eq!(img.height(), height);
            prop_assert_eq!(img.mode(), mode);
            prop_assert_eq!(img.palette(), None);
            let pixeles = todos_los_pixeles(&img);
            prop_assert_eq!(pixeles.len() as u128, u128::from(width) * u128::from(height));
            prop_assert!(pixeles.iter().all(|&p| p == inicial(mode)));
        }
    }

    #[test]
    fn byte_len_es_el_producto_o_el_error(
        (width, height, mode) in caso_de_tamano(),
        indexado in any::<bool>(),
    ) {
        let mode = if indexado { Mode::Indexed } else { mode };
        let bytes = u128::from(width) * u128::from(height) * bytes_por_pixel(mode);
        let r = Image::byte_len(width, height, mode);
        if width == 0 || height == 0 {
            prop_assert_eq!(r, Err(ImageError::ZeroSize { width, height }));
        } else if bytes > Image::MAX_BYTES as u128 {
            prop_assert_eq!(r, Err(ImageError::TooLarge { width, height }));
        } else {
            prop_assert_eq!(r, Ok(bytes as usize));
        }
    }

    #[test]
    fn coordenadas_dentro_y_fuera(img in imagen_pequena(), x in coordenada(), y in coordenada()) {
        let (width, height) = (img.width(), img.height());
        let r = img.get_pixel(x, y);
        if x < width && y < height {
            prop_assert!(r.is_ok());
        } else {
            prop_assert_eq!(r, Err(fuera(x, y, width, height)));
        }
    }

    #[test]
    fn ida_y_vuelta_y_localidad(
        mut img in imagen_pequena(),
        xb in any::<u32>(),
        yb in any::<u32>(),
        bytes in (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()),
    ) {
        let (x, y) = (xb % img.width(), yb % img.height());
        let pixel = pixel_valido(&img, bytes);
        let antes = todos_los_pixeles(&img);

        prop_assert_eq!(img.set_pixel(x, y, pixel), Ok(()));
        prop_assert_eq!(img.get_pixel(x, y), Ok(pixel));

        let despues = todos_los_pixeles(&img);
        let ancho = img.width();
        for (k, (a, d)) in antes.iter().zip(&despues).enumerate() {
            let (kx, ky) = (k as u32 % ancho, k as u32 / ancho);
            if (kx, ky) != (x, y) {
                prop_assert_eq!(a, d, "cambió el píxel ({}, {})", kx, ky);
            }
        }
    }

    #[test]
    fn secuencias_contra_un_modelo(
        mut img in imagen_pequena(),
        ops in prop::collection::vec((any::<u32>(), any::<u32>(), pixel_cualquiera()), 0..=80),
    ) {
        ejecuta_secuencia(&mut img, &ops)?;
    }

    #[test]
    fn set_pixel_con_indice_fuera_de_la_paleta(
        n in 1usize..=256,
        index in any::<u8>(),
        w in 1u32..=8,
        h in 1u32..=8,
        xb in any::<u32>(),
        yb in any::<u32>(),
    ) {
        let mut img = crea(Mode::Indexed, w, h, n);
        let (x, y) = (xb % w, yb % h);
        let copia = img.clone();

        let r = img.set_pixel(x, y, Pixel::Indexed(index));
        if (index as usize) < n {
            prop_assert_eq!(r, Ok(()));
            prop_assert_eq!(img.get_pixel(x, y), Ok(Pixel::Indexed(index)));
        } else {
            prop_assert_eq!(r, Err(fuera_de_paleta(index, n)));
            prop_assert_eq!(&img, &copia);
        }
    }

    #[test]
    fn invariante_del_indexado(
        mut img in imagen_indexada(),
        ops in prop::collection::vec((any::<u32>(), any::<u32>(), pixel_con_indice_libre()), 0..=80),
    ) {
        let palette_inicial = img.palette().cloned();
        prop_assert!(palette_inicial.is_some());
        let (width, height) = (img.width(), img.height());

        for &(xb, yb, pixel) in &ops {
            let _ = img.set_pixel(xb % (width + 4), yb % (height + 4), pixel);

            prop_assert_eq!(img.mode(), Mode::Indexed);
            prop_assert_eq!(img.palette().cloned(), palette_inicial.clone());
            let len = img.palette().unwrap().len();
            prop_assert!(len >= 1);
            for p in todos_los_pixeles(&img) {
                match p {
                    Pixel::Indexed(i) => prop_assert!((i as usize) < len, "índice {} con paleta de {}", i, len),
                    otro => prop_assert!(false, "píxel de otro modo en una imagen indexada: {:?}", otro),
                }
            }
        }
    }
}

// `Image::check_pixel` (pública desde el hito 2, unidad 5; specs/hito-2-relleno.md).

/// Un píxel de cada modo, con valores que valen en cualquier imagen del modo.
fn muestra(mode: Mode) -> Pixel {
    match mode {
        Mode::Rgba => Pixel::Rgba(c0()),
        Mode::Indexed => Pixel::Indexed(0),
        Mode::Gray => Pixel::Gray(7),
        Mode::GrayAlpha => Pixel::GrayAlpha { gray: 7, alpha: 9 },
    }
}

#[test]
fn check_pixel_acepta_un_pixel_del_modo() {
    let casos = [
        (imagen_3x2(Mode::Rgba), Pixel::Rgba(c0())),
        (imagen_3x2(Mode::Rgba), Pixel::Rgba(Rgba::TRANSPARENT)),
        (imagen_3x2(Mode::Rgba), Pixel::Rgba(Rgba::new(1, 2, 3, 0))),
        (imagen_3x2(Mode::Gray), Pixel::Gray(0)),
        (imagen_3x2(Mode::Gray), Pixel::Gray(255)),
        (
            imagen_3x2(Mode::GrayAlpha),
            Pixel::GrayAlpha { gray: 0, alpha: 0 },
        ),
        (
            imagen_3x2(Mode::GrayAlpha),
            Pixel::GrayAlpha {
                gray: 255,
                alpha: 255,
            },
        ),
    ];
    for (img, pixel) in casos {
        assert_eq!(
            img.check_pixel(pixel),
            Ok(()),
            "{pixel:?} en {:?}",
            img.mode()
        );
    }
}

#[test]
fn check_pixel_indexado_dentro_de_la_paleta_incluido_el_ultimo() {
    // Paleta de 2: los índices 0 y 1 (el último).
    let img = imagen_3x2(Mode::Indexed);
    assert_eq!(img.check_pixel(Pixel::Indexed(0)), Ok(()));
    assert_eq!(img.check_pixel(Pixel::Indexed(1)), Ok(()));
    // Paleta de 1, de 3 y de 256 (último índice, 255).
    for (n, ultimo) in [(1usize, 0u8), (3, 2), (255, 254), (256, 255)] {
        let img = crea(Mode::Indexed, 3, 2, n);
        assert_eq!(img.check_pixel(Pixel::Indexed(0)), Ok(()), "paleta de {n}");
        assert_eq!(
            img.check_pixel(Pixel::Indexed(ultimo)),
            Ok(()),
            "último índice de una paleta de {n}"
        );
    }
}

#[test]
fn check_pixel_de_otro_modo_es_mode_mismatch() {
    for esperado in TODOS {
        let img = imagen_3x2(esperado);
        let copia = img.clone();
        for encontrado in TODOS {
            if encontrado == esperado {
                continue;
            }
            let pixel = muestra(encontrado);
            assert_eq!(
                img.check_pixel(pixel),
                Err(desajuste(esperado, encontrado)),
                "{pixel:?} en una imagen {esperado:?}"
            );
        }
        assert_eq!(img, copia);
    }
}

#[test]
fn check_pixel_mode_mismatch_con_valores_exactos() {
    // Las combinaciones que la spec nombra o que más importan, con los valores literales.
    let rgba = imagen_3x2(Mode::Rgba);
    assert_eq!(
        rgba.check_pixel(Pixel::Indexed(0)),
        Err(ImageError::ModeMismatch {
            expected: Mode::Rgba,
            found: Mode::Indexed
        })
    );
    assert_eq!(
        rgba.check_pixel(Pixel::Gray(1)),
        Err(ImageError::ModeMismatch {
            expected: Mode::Rgba,
            found: Mode::Gray
        })
    );
    let gray = imagen_3x2(Mode::Gray);
    assert_eq!(
        gray.check_pixel(Pixel::GrayAlpha { gray: 1, alpha: 2 }),
        Err(ImageError::ModeMismatch {
            expected: Mode::Gray,
            found: Mode::GrayAlpha
        })
    );
    let gray_alpha = imagen_3x2(Mode::GrayAlpha);
    assert_eq!(
        gray_alpha.check_pixel(Pixel::Gray(1)),
        Err(ImageError::ModeMismatch {
            expected: Mode::GrayAlpha,
            found: Mode::Gray
        })
    );
    let indexada = imagen_3x2(Mode::Indexed);
    assert_eq!(
        indexada.check_pixel(Pixel::Rgba(c0())),
        Err(ImageError::ModeMismatch {
            expected: Mode::Indexed,
            found: Mode::Rgba
        })
    );
    assert_eq!(
        indexada.check_pixel(Pixel::Gray(9)),
        Err(ImageError::ModeMismatch {
            expected: Mode::Indexed,
            found: Mode::Gray
        })
    );
}

#[test]
fn check_pixel_mode_mismatch_gana_a_indice_fuera_de_la_paleta() {
    // Indexed(9) en una imagen RGBA: el modo va primero, no hay paleta con la que comparar.
    for mode in NO_INDEXADOS {
        let img = imagen_3x2(mode);
        assert_eq!(
            img.check_pixel(Pixel::Indexed(9)),
            Err(desajuste(mode, Mode::Indexed))
        );
        assert_eq!(
            img.check_pixel(Pixel::Indexed(255)),
            Err(desajuste(mode, Mode::Indexed))
        );
    }
}

#[test]
fn check_pixel_indice_fuera_de_la_paleta_no_cambia_la_imagen() {
    // `index == len` (el primero que no cabe) y 255.
    for (n, index) in [
        (1usize, 1u8),
        (1, 255),
        (2, 2),
        (2, 255),
        (3, 3),
        (3, 255),
        (255, 255),
    ] {
        let img = crea(Mode::Indexed, 3, 2, n);
        let copia = img.clone();
        assert_eq!(
            img.check_pixel(Pixel::Indexed(index)),
            Err(ImageError::IndexNotInPalette { index, len: n }),
            "índice {index} con paleta de {n}"
        );
        assert_eq!(img, copia);
        assert_eq!(todos_los_pixeles(&img), todos_los_pixeles(&copia));
    }
}

#[test]
fn check_pixel_concuerda_con_set_pixel_en_casos_fijos() {
    let pixeles = [
        Pixel::Rgba(c0()),
        Pixel::Indexed(0),
        Pixel::Indexed(1),
        Pixel::Indexed(2),
        Pixel::Indexed(255),
        Pixel::Gray(7),
        Pixel::GrayAlpha { gray: 7, alpha: 9 },
    ];
    for mode in TODOS {
        for pixel in pixeles {
            let img = imagen_3x2(mode);
            let mut copia = img.clone();
            assert_eq!(
                img.check_pixel(pixel),
                copia.set_pixel(0, 0, pixel),
                "{pixel:?} en una imagen {mode:?}"
            );
        }
    }
}

proptest! {
    /// Para cualquier píxel y modo, `check_pixel(p)` es `Err(e)` si y solo si
    /// `set_pixel(0, 0, p)` da `Err(e)` sobre una copia; y no cambia la imagen.
    #[test]
    fn check_pixel_concuerda_con_set_pixel(img in imagen_pequena(), pixel in pixel_cualquiera()) {
        let antes = img.clone();
        let comprobado = img.check_pixel(pixel);
        let mut copia = img.clone();
        let escrito = copia.set_pixel(0, 0, pixel);
        prop_assert_eq!(&comprobado, &escrito, "{:?} en una imagen {:?}", pixel, img.mode());
        prop_assert_eq!(comprobado.is_ok(), escrito.is_ok());
        prop_assert_eq!(&img, &antes);
        // Si falla, `set_pixel` tampoco cambió su copia.
        if escrito.is_err() {
            prop_assert_eq!(&copia, &antes);
        }
    }

    /// El resultado es el del modelo: modo distinto es `ModeMismatch`, índice `>= len` es
    /// `IndexNotInPalette` y en cualquier otro caso `Ok`.
    #[test]
    fn check_pixel_es_el_modelo(img in imagen_pequena(), pixel in pixel_cualquiera()) {
        let previsto = if modo_de(pixel) != img.mode() {
            Err(desajuste(img.mode(), modo_de(pixel)))
        } else {
            match (pixel, img.palette()) {
                (Pixel::Indexed(index), Some(p)) if usize::from(index) >= p.len() => {
                    Err(fuera_de_paleta(index, p.len()))
                }
                _ => Ok(()),
            }
        };
        prop_assert_eq!(img.check_pixel(pixel), previsto);
    }
}
