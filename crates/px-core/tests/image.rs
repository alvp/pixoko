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
fn traits_derived() {
    assert_value_traits::<Mode>();
    assert_value_traits::<Pixel>();
    assert_image_traits::<Image>();
    assert_error_traits::<ImageError>();
}

#[test]
fn max_bytes_is_1_gib() {
    assert_eq!(Image::MAX_BYTES, 1usize << 30);
}

// Colores, paletas e imágenes de apoyo.

const NOT_INDEXED: [Mode; 3] = [Mode::Rgba, Mode::Gray, Mode::GrayAlpha];
const ALL_2: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

fn c0() -> Rgba {
    Rgba::new(10, 20, 30, 255)
}

fn c1() -> Rgba {
    Rgba::new(40, 50, 60, 255)
}

fn palette_2(colors_2: &[Rgba]) -> Palette {
    Palette::from_colors(colors_2.iter().copied()).unwrap()
}

fn palette_of(n: usize) -> Palette {
    Palette::from_colors((0..n).map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))).unwrap()
}

/// Crea una imagen de cualquier modo; el indexado usa una paleta de `n` entradas.
fn creates(mode: Mode, width: u32, height: u32, n: usize) -> Image {
    match mode {
        Mode::Indexed => Image::new_indexed(width, height, palette_of(n)).unwrap(),
        _ => Image::new(width, height, mode).unwrap(),
    }
}

/// Imagen 3 x 2 de cualquier modo (el indexado con paleta de 2).
fn image_3_x_2(mode: Mode) -> Image {
    creates(mode, 3, 2, 2)
}

fn initial(mode: Mode) -> Pixel {
    match mode {
        Mode::Rgba => Pixel::Rgba(Rgba::TRANSPARENT),
        Mode::Indexed => Pixel::Indexed(0),
        Mode::Gray => Pixel::Gray(0),
        Mode::GrayAlpha => Pixel::GrayAlpha { gray: 0, alpha: 0 },
    }
}

fn mode_of(pixel: Pixel) -> Mode {
    match pixel {
        Pixel::Rgba(_) => Mode::Rgba,
        Pixel::Indexed(_) => Mode::Indexed,
        Pixel::Gray(_) => Mode::Gray,
        Pixel::GrayAlpha { .. } => Mode::GrayAlpha,
    }
}

fn bytes_per_pixel(mode: Mode) -> u128 {
    match mode {
        Mode::Rgba => 4,
        Mode::GrayAlpha => 2,
        Mode::Indexed | Mode::Gray => 1,
    }
}

/// Todos los píxeles de la imagen, fila a fila (todas las coordenadas son válidas).
fn all_the_pixels(img: &Image) -> Vec<Pixel> {
    let mut v = Vec::new();
    for y in 0..img.height() {
        for x in 0..img.width() {
            v.push(img.get_pixel(x, y).unwrap());
        }
    }
    v
}

fn outside(x: u32, y: u32, width: u32, height: u32) -> ImageError {
    ImageError::OutOfBounds {
        x,
        y,
        width,
        height,
    }
}

fn mismatch(expected: Mode, found: Mode) -> ImageError {
    ImageError::ModeMismatch { expected, found }
}

fn out_of_palette(index: u8, len: usize) -> ImageError {
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
    let pixels = all_the_pixels(&img);
    assert_eq!(pixels.len(), 12);
    assert!(pixels.iter().all(|&p| p == Pixel::Rgba(Rgba::TRANSPARENT)));
}

#[test]
fn new_2x2_gray_and_gray_alpha_initialize_to_zero() {
    let g = Image::new(2, 2, Mode::Gray).unwrap();
    assert_eq!(g.mode(), Mode::Gray);
    assert_eq!(g.palette(), None);
    assert_eq!(all_the_pixels(&g), vec![Pixel::Gray(0); 4]);

    let ga = Image::new(2, 2, Mode::GrayAlpha).unwrap();
    assert_eq!(ga.mode(), Mode::GrayAlpha);
    assert_eq!(ga.palette(), None);
    assert_eq!(
        all_the_pixels(&ga),
        vec![Pixel::GrayAlpha { gray: 0, alpha: 0 }; 4]
    );
}

#[test]
fn new_1x1_in_each_non_indexed_mode() {
    for mode in NOT_INDEXED {
        let img = Image::new(1, 1, mode).unwrap();
        assert_eq!((img.width(), img.height()), (1, 1));
        assert_eq!(img.mode(), mode);
        assert_eq!(img.get_pixel(0, 0), Ok(initial(mode)));
    }
}

#[test]
fn new_with_zero_width_or_height_is_zero_size() {
    for mode in NOT_INDEXED {
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
fn new_with_overflowing_product_is_too_large() {
    assert!(es_too_large(Image::new(u32::MAX, u32::MAX, Mode::Rgba)));
    assert!(es_too_large(Image::new(u32::MAX, u32::MAX, Mode::Gray)));
    assert!(es_too_large(Image::new(
        u32::MAX,
        u32::MAX,
        Mode::GrayAlpha
    )));
}

#[test]
fn new_above_max_bytes_is_too_large() {
    assert!(es_too_large(Image::new(u32::MAX, 1, Mode::Gray)));
    assert!(es_too_large(Image::new(16385, 16384, Mode::Rgba)));
}

#[test]
fn byte_len_at_the_exact_limit_fits() {
    let cap = Image::MAX_BYTES;
    assert_eq!(Image::byte_len(16384, 16384, Mode::Rgba), Ok(cap));
    assert_eq!(Image::byte_len(cap as u32, 1, Mode::Gray), Ok(cap));
    assert_eq!(Image::byte_len(1, cap as u32, Mode::Gray), Ok(cap));
    assert_eq!(Image::byte_len(cap as u32 / 2, 1, Mode::GrayAlpha), Ok(cap));
    assert_eq!(
        Image::byte_len(16384, 16384, Mode::Indexed),
        Ok(16384 * 16384)
    );
}

#[test]
fn byte_len_one_byte_over_is_too_large() {
    let one_more = Image::MAX_BYTES as u32 + 1;
    for (width, height, mode) in [
        (one_more, 1, Mode::Gray),
        (1, one_more, Mode::Gray),
        (one_more, 1, Mode::Indexed),
        (16385, 16384, Mode::Rgba),
        (16384, 16385, Mode::Rgba),
        (one_more / 2 + 1, 1, Mode::GrayAlpha),
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
fn byte_len_with_zero_is_zero_size() {
    for mode in ALL_2 {
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
fn byte_len_small_per_mode() {
    for mode in ALL_2 {
        assert_eq!(
            Image::byte_len(1, 1, mode),
            Ok(bytes_per_pixel(mode) as usize)
        );
        assert_eq!(
            Image::byte_len(4, 3, mode),
            Ok(12 * bytes_per_pixel(mode) as usize)
        );
    }
}

#[test]
fn new_indexed_without_palette_and_check_order() {
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
fn new_indexed_too_large_beats_required_palette() {
    // Comportamiento 3: ZeroSize, luego TooLarge, luego PaletteRequired.
    assert!(es_too_large(Image::new(u32::MAX, u32::MAX, Mode::Indexed)));
    assert!(es_too_large(Image::new(u32::MAX, 1, Mode::Indexed)));
}

#[test]
fn new_indexed_valid() {
    let p = palette_2(&[c0(), c1()]);
    let img = Image::new_indexed(2, 2, p.clone()).unwrap();
    assert_eq!(img.mode(), Mode::Indexed);
    assert_eq!((img.width(), img.height()), (2, 2));
    assert_eq!(img.palette(), Some(&p));
    assert_eq!(all_the_pixels(&img), vec![Pixel::Indexed(0); 4]);
}

#[test]
fn new_indexed_errors() {
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
        palette_2(&[c0()])
    )));
}

#[test]
fn new_indexed_dimensions_are_checked_before_the_empty_palette() {
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
fn get_pixel_at_corners_and_outside() {
    for mode in ALL_2 {
        let img = image_3_x_2(mode);
        assert_eq!(img.get_pixel(0, 0), Ok(initial(mode)), "{mode:?}");
        assert_eq!(img.get_pixel(2, 1), Ok(initial(mode)), "{mode:?}");
        for (x, y) in [(3u32, 0u32), (0, 2), (3, 2), (u32::MAX, u32::MAX)] {
            assert_eq!(
                img.get_pixel(x, y),
                Err(outside(x, y, 3, 2)),
                "{mode:?} en ({x}, {y})"
            );
        }
    }
}

/// Escribe `pixel` en `(x, y)` y comprueba la ida y vuelta y que nada más cambia.
fn writes_and_checks(img: &mut Image, x: u32, y: u32, pixel: Pixel) {
    let before = all_the_pixels(img);
    assert_eq!(img.set_pixel(x, y, pixel), Ok(()), "set_pixel({x}, {y})");
    assert_eq!(img.get_pixel(x, y), Ok(pixel), "get_pixel({x}, {y})");
    let after = all_the_pixels(img);
    let width = img.width();
    for (k, (a, d)) in before.iter().zip(&after).enumerate() {
        let (kx, ky) = (k as u32 % width, k as u32 / width);
        if (kx, ky) != (x, y) {
            assert_eq!(a, d, "cambió el píxel ({kx}, {ky})");
        }
    }
}

#[test]
fn set_and_get_in_each_mode() {
    let cases: [(Mode, Vec<Pixel>); 4] = [
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
    for (mode, pixels) in cases {
        for pixel in pixels {
            // Esquinas y un punto intermedio.
            for (x, y) in [(0u32, 0u32), (2, 1), (1, 0), (1, 1)] {
                let mut img = image_3_x_2(mode);
                writes_and_checks(&mut img, x, y, pixel);
            }
        }
    }
}

#[test]
fn set_keeps_the_rgb_with_alpha_zero() {
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
fn set_outside_the_image_is_out_of_bounds_without_changes() {
    let valid = [
        (Mode::Rgba, Pixel::Rgba(c0())),
        (Mode::Indexed, Pixel::Indexed(1)),
        (Mode::Gray, Pixel::Gray(7)),
        (Mode::GrayAlpha, Pixel::GrayAlpha { gray: 7, alpha: 9 }),
    ];
    for (mode, pixel) in valid {
        for (x, y) in [(3u32, 0u32), (u32::MAX, 0), (0, 2), (0, u32::MAX)] {
            let mut img = image_3_x_2(mode);
            let copy = img.clone();
            assert_eq!(
                img.set_pixel(x, y, pixel),
                Err(outside(x, y, 3, 2)),
                "{mode:?} en ({x}, {y})"
            );
            assert_eq!(img, copy);
        }
    }
}

#[test]
fn set_of_another_mode_is_mode_mismatch_without_changes() {
    let cases = [
        (Mode::Rgba, Pixel::Gray(1)),
        (Mode::Gray, Pixel::GrayAlpha { gray: 1, alpha: 2 }),
        (Mode::Rgba, Pixel::Indexed(0)),
    ];
    for (mode, pixel) in cases {
        let mut img = image_3_x_2(mode);
        let copy = img.clone();
        assert_eq!(
            img.set_pixel(0, 0, pixel),
            Err(mismatch(mode, mode_of(pixel))),
            "{pixel:?} en una imagen {mode:?}"
        );
        assert_eq!(img, copy);
    }
}

#[test]
fn set_gray_and_gray_alpha_are_different_modes() {
    let mut ga = image_3_x_2(Mode::GrayAlpha);
    let copy = ga.clone();
    assert_eq!(
        ga.set_pixel(0, 0, Pixel::Gray(1)),
        Err(mismatch(Mode::GrayAlpha, Mode::Gray))
    );
    assert_eq!(ga, copy);
}

#[test]
fn set_indexed_inside_and_outside_the_palette() {
    // Paleta de 2.
    let mut img = Image::new_indexed(3, 2, palette_of(2)).unwrap();
    assert_eq!(img.set_pixel(0, 0, Pixel::Indexed(1)), Ok(()));
    assert_eq!(img.get_pixel(0, 0), Ok(Pixel::Indexed(1)));

    for index in [2u8, 255] {
        let copy = img.clone();
        assert_eq!(
            img.set_pixel(1, 1, Pixel::Indexed(index)),
            Err(out_of_palette(index, 2))
        );
        assert_eq!(img, copy);
    }

    // Paleta de 1.
    let mut a = Image::new_indexed(3, 2, palette_of(1)).unwrap();
    let copy = a.clone();
    assert_eq!(
        a.set_pixel(0, 0, Pixel::Indexed(1)),
        Err(out_of_palette(1, 1))
    );
    assert_eq!(a, copy);
    assert_eq!(a.set_pixel(0, 0, Pixel::Indexed(0)), Ok(()));

    // Paleta de 256.
    let mut full = Image::new_indexed(3, 2, palette_of(256)).unwrap();
    assert_eq!(full.set_pixel(2, 1, Pixel::Indexed(255)), Ok(()));
    assert_eq!(full.get_pixel(2, 1), Ok(Pixel::Indexed(255)));
}

#[test]
fn indexed_with_transparent_index_is_not_resolved() {
    let mut p = palette_2(&[c0(), c1()]);
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
fn check_order_of_set_pixel() {
    // OutOfBounds gana a ModeMismatch.
    let mut img = image_3_x_2(Mode::Rgba);
    let copy = img.clone();
    assert_eq!(
        img.set_pixel(3, 0, Pixel::Gray(1)),
        Err(outside(3, 0, 3, 2))
    );
    assert_eq!(img, copy);

    // ModeMismatch gana a IndexNotInPalette: Indexed(9) en una imagen Rgba.
    assert_eq!(
        img.set_pixel(0, 0, Pixel::Indexed(9)),
        Err(mismatch(Mode::Rgba, Mode::Indexed))
    );
    assert_eq!(img, copy);

    // OutOfBounds gana a IndexNotInPalette en una imagen indexada.
    let mut idx = image_3_x_2(Mode::Indexed);
    let copy = idx.clone();
    assert_eq!(
        idx.set_pixel(0, 2, Pixel::Indexed(9)),
        Err(outside(0, 2, 3, 2))
    );
    assert_eq!(idx, copy);

    // Un píxel de otro modo en una imagen indexada es ModeMismatch.
    assert_eq!(
        idx.set_pixel(0, 0, Pixel::Gray(9)),
        Err(mismatch(Mode::Indexed, Mode::Gray))
    );
    assert_eq!(idx, copy);
}

// Estrategias.

fn color() -> impl Strategy<Value = Rgba> {
    (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
        .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
}

fn mode_2() -> impl Strategy<Value = Mode> {
    prop::sample::select(ALL_2.to_vec())
}

fn mode_not_indexed() -> impl Strategy<Value = Mode> {
    prop::sample::select(NOT_INDEXED.to_vec())
}

/// Imagen pequeña (1 a 16 de lado) de cualquier modo; el indexado lleva una
/// paleta de 1 a 256 entradas.
fn image_small() -> impl Strategy<Value = Image> {
    (mode_2(), 1u32..=16, 1u32..=16, 1usize..=256).prop_map(|(m, w, h, n)| creates(m, w, h, n))
}

/// Imagen pequeña con paleta de 1 a 256 entradas, siempre indexada.
fn image_indexed() -> impl Strategy<Value = Image> {
    (1u32..=16, 1u32..=16, 1usize..=256).prop_map(|(w, h, n)| creates(Mode::Indexed, w, h, n))
}

/// Píxel de cualquier variante, con índices que a menudo caen fuera de la paleta.
fn pixel_any() -> impl Strategy<Value = Pixel> {
    prop_oneof![
        color().prop_map(Pixel::Rgba),
        prop_oneof![3 => 0u8..4, 1 => any::<u8>()].prop_map(Pixel::Indexed),
        any::<u8>().prop_map(Pixel::Gray),
        (any::<u8>(), any::<u8>()).prop_map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha }),
    ]
}

/// Píxel de cualquier variante con índice `u8` cualquiera (para el invariante).
fn pixel_with_index_free() -> impl Strategy<Value = Pixel> {
    prop_oneof![
        4 => any::<u8>().prop_map(Pixel::Indexed),
        1 => pixel_any(),
    ]
}

/// Construye un píxel válido para `img` a partir de bytes arbitrarios.
fn pixel_valid(img: &Image, (a, b, c, d): (u8, u8, u8, u8)) -> Pixel {
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

/// Casos de la propiedad 1: `(width, height, mode_2)`. La última rama cae justo
/// por encima de `MAX_BYTES`, para probar el límite sin reservar memoria.
fn case_of_size() -> impl Strategy<Value = (u32, u32, Mode)> {
    prop_oneof![
        (0u32..=32, 0u32..=32, mode_not_indexed()),
        (any::<u32>(), any::<u32>(), mode_not_indexed()),
        (Just(0u32), any::<u32>(), mode_not_indexed()),
        (any::<u32>(), Just(0u32), mode_not_indexed()),
        (mode_not_indexed(), 1u32..=70_000, 1u64..=3).prop_map(|(m, h, d)| {
            let per_row = bytes_per_pixel(m) as u64 * u64::from(h);
            let w = Image::MAX_BYTES as u64 / per_row + d;
            (w as u32, h, m)
        }),
    ]
}

/// Casilla (x, y) de `u32` completo, con sesgo hacia valores cercanos a la imagen.
fn coordinate() -> impl Strategy<Value = u32> {
    prop_oneof![
        3 => 0u32..20,
        2 => any::<u32>(),
        1 => Just(u32::MAX),
    ]
}

// Modelo de referencia para las secuencias de `set_pixel`.

/// Resultado esperado de `set_pixel` según el orden del comportamiento 6.
fn expected(img: &Image, x: u32, y: u32, pixel: Pixel) -> Result<(), ImageError> {
    let (width, height) = (img.width(), img.height());
    if x >= width || y >= height {
        return Err(outside(x, y, width, height));
    }
    if mode_of(pixel) != img.mode() {
        return Err(mismatch(img.mode(), mode_of(pixel)));
    }
    if let Pixel::Indexed(index) = pixel {
        let len = img.palette().unwrap().len();
        if index as usize >= len {
            return Err(out_of_palette(index, len));
        }
    }
    Ok(())
}

/// Aplica una secuencia de `(x_bruto, y_bruto, pixel)` y la compara con un
/// `Vec<Pixel>` como modelo. Las coordenadas caen en un rango algo mayor que la imagen.
fn runs_sequence(img: &mut Image, ops: &[(u32, u32, Pixel)]) -> Result<(), TestCaseError> {
    let (width, height) = (img.width(), img.height());
    let mut model = vec![initial(img.mode()); (width * height) as usize];
    let palette_initial = img.palette().cloned();

    for &(xb, yb, pixel) in ops {
        let (x, y) = (xb % (width + 4), yb % (height + 4));
        let copy = img.clone();
        let expected_2 = expected(img, x, y, pixel);

        let real = img.set_pixel(x, y, pixel);
        prop_assert_eq!(&real, &expected_2, "set_pixel({}, {}, {:?})", x, y, pixel);

        if real.is_ok() {
            model[(y * width + x) as usize] = pixel;
        } else {
            prop_assert_eq!(&*img, &copy);
        }
        prop_assert_eq!(all_the_pixels(img), model.clone());
        prop_assert_eq!((img.width(), img.height()), (width, height));
        prop_assert_eq!(img.palette().cloned(), palette_initial.clone());
    }
    Ok(())
}

// Propiedades.

proptest! {
    #[test]
    fn dimensions_and_size((width, height, mode) in case_of_size()) {
        let bytes = u128::from(width) * u128::from(height) * bytes_per_pixel(mode);
        let cap = Image::MAX_BYTES as u128;
        // Las imágenes que serían Ok se limitan a un tamaño pequeño (4 KiB).
        prop_assume!(width == 0 || height == 0 || bytes > cap || bytes <= 4096);

        let r = Image::new(width, height, mode);
        if width == 0 || height == 0 {
            prop_assert_eq!(r, Err(ImageError::ZeroSize { width, height }));
        } else if bytes > cap {
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
            let pixels = all_the_pixels(&img);
            prop_assert_eq!(pixels.len() as u128, u128::from(width) * u128::from(height));
            prop_assert!(pixels.iter().all(|&p| p == initial(mode)));
        }
    }

    #[test]
    fn byte_len_is_the_product_or_the_error(
        (width, height, mode) in case_of_size(),
        indexed_2 in any::<bool>(),
    ) {
        let mode = if indexed_2 { Mode::Indexed } else { mode };
        let bytes = u128::from(width) * u128::from(height) * bytes_per_pixel(mode);
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
    fn coordinates_inside_and_outside(img in image_small(), x in coordinate(), y in coordinate()) {
        let (width, height) = (img.width(), img.height());
        let r = img.get_pixel(x, y);
        if x < width && y < height {
            prop_assert!(r.is_ok());
        } else {
            prop_assert_eq!(r, Err(outside(x, y, width, height)));
        }
    }

    #[test]
    fn round_trip_and_locality(
        mut img in image_small(),
        xb in any::<u32>(),
        yb in any::<u32>(),
        bytes in (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()),
    ) {
        let (x, y) = (xb % img.width(), yb % img.height());
        let pixel = pixel_valid(&img, bytes);
        let before = all_the_pixels(&img);

        prop_assert_eq!(img.set_pixel(x, y, pixel), Ok(()));
        prop_assert_eq!(img.get_pixel(x, y), Ok(pixel));

        let after = all_the_pixels(&img);
        let width = img.width();
        for (k, (a, d)) in before.iter().zip(&after).enumerate() {
            let (kx, ky) = (k as u32 % width, k as u32 / width);
            if (kx, ky) != (x, y) {
                prop_assert_eq!(a, d, "cambió el píxel ({}, {})", kx, ky);
            }
        }
    }

    #[test]
    fn sequences_against_a_model(
        mut img in image_small(),
        ops in prop::collection::vec((any::<u32>(), any::<u32>(), pixel_any()), 0..=80),
    ) {
        runs_sequence(&mut img, &ops)?;
    }

    #[test]
    fn set_pixel_with_index_outside_the_palette(
        n in 1usize..=256,
        index in any::<u8>(),
        w in 1u32..=8,
        h in 1u32..=8,
        xb in any::<u32>(),
        yb in any::<u32>(),
    ) {
        let mut img = creates(Mode::Indexed, w, h, n);
        let (x, y) = (xb % w, yb % h);
        let copy = img.clone();

        let r = img.set_pixel(x, y, Pixel::Indexed(index));
        if (index as usize) < n {
            prop_assert_eq!(r, Ok(()));
            prop_assert_eq!(img.get_pixel(x, y), Ok(Pixel::Indexed(index)));
        } else {
            prop_assert_eq!(r, Err(out_of_palette(index, n)));
            prop_assert_eq!(&img, &copy);
        }
    }

    #[test]
    fn invariant_of_the_indexed(
        mut img in image_indexed(),
        ops in prop::collection::vec((any::<u32>(), any::<u32>(), pixel_with_index_free()), 0..=80),
    ) {
        let palette_initial = img.palette().cloned();
        prop_assert!(palette_initial.is_some());
        let (width, height) = (img.width(), img.height());

        for &(xb, yb, pixel) in &ops {
            let _ = img.set_pixel(xb % (width + 4), yb % (height + 4), pixel);

            prop_assert_eq!(img.mode(), Mode::Indexed);
            prop_assert_eq!(img.palette().cloned(), palette_initial.clone());
            let len = img.palette().unwrap().len();
            prop_assert!(len >= 1);
            for p in all_the_pixels(&img) {
                match p {
                    Pixel::Indexed(i) => prop_assert!((i as usize) < len, "índice {} con paleta de {}", i, len),
                    other => prop_assert!(false, "píxel de otro modo en una imagen indexada: {:?}", other),
                }
            }
        }
    }
}

// `Image::check_pixel` (pública desde el hito 2, unidad 5; specs/hito-2-relleno.md).

/// Un píxel de cada modo, con valores que valen en cualquier imagen del modo.
fn swatch(mode: Mode) -> Pixel {
    match mode {
        Mode::Rgba => Pixel::Rgba(c0()),
        Mode::Indexed => Pixel::Indexed(0),
        Mode::Gray => Pixel::Gray(7),
        Mode::GrayAlpha => Pixel::GrayAlpha { gray: 7, alpha: 9 },
    }
}

#[test]
fn check_pixel_accepts_a_pixel_of_the_mode() {
    let cases = [
        (image_3_x_2(Mode::Rgba), Pixel::Rgba(c0())),
        (image_3_x_2(Mode::Rgba), Pixel::Rgba(Rgba::TRANSPARENT)),
        (image_3_x_2(Mode::Rgba), Pixel::Rgba(Rgba::new(1, 2, 3, 0))),
        (image_3_x_2(Mode::Gray), Pixel::Gray(0)),
        (image_3_x_2(Mode::Gray), Pixel::Gray(255)),
        (
            image_3_x_2(Mode::GrayAlpha),
            Pixel::GrayAlpha { gray: 0, alpha: 0 },
        ),
        (
            image_3_x_2(Mode::GrayAlpha),
            Pixel::GrayAlpha {
                gray: 255,
                alpha: 255,
            },
        ),
    ];
    for (img, pixel) in cases {
        assert_eq!(
            img.check_pixel(pixel),
            Ok(()),
            "{pixel:?} en {:?}",
            img.mode()
        );
    }
}

#[test]
fn check_pixel_indexed_inside_the_palette_including_the_last() {
    // Paleta de 2: los índices 0 y 1 (el último).
    let img = image_3_x_2(Mode::Indexed);
    assert_eq!(img.check_pixel(Pixel::Indexed(0)), Ok(()));
    assert_eq!(img.check_pixel(Pixel::Indexed(1)), Ok(()));
    // Paleta de 1, de 3 y de 256 (último índice, 255).
    for (n, last) in [(1usize, 0u8), (3, 2), (255, 254), (256, 255)] {
        let img = creates(Mode::Indexed, 3, 2, n);
        assert_eq!(img.check_pixel(Pixel::Indexed(0)), Ok(()), "paleta de {n}");
        assert_eq!(
            img.check_pixel(Pixel::Indexed(last)),
            Ok(()),
            "último índice de una paleta de {n}"
        );
    }
}

#[test]
fn check_pixel_of_another_mode_is_mode_mismatch() {
    for expected in ALL_2 {
        let img = image_3_x_2(expected);
        let copy = img.clone();
        for found in ALL_2 {
            if found == expected {
                continue;
            }
            let pixel = swatch(found);
            assert_eq!(
                img.check_pixel(pixel),
                Err(mismatch(expected, found)),
                "{pixel:?} en una imagen {expected:?}"
            );
        }
        assert_eq!(img, copy);
    }
}

#[test]
fn check_pixel_mode_mismatch_with_exact_values() {
    // Las combinaciones que la spec nombra o que más importan, con los valores literales.
    let rgba = image_3_x_2(Mode::Rgba);
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
    let gray = image_3_x_2(Mode::Gray);
    assert_eq!(
        gray.check_pixel(Pixel::GrayAlpha { gray: 1, alpha: 2 }),
        Err(ImageError::ModeMismatch {
            expected: Mode::Gray,
            found: Mode::GrayAlpha
        })
    );
    let gray_alpha = image_3_x_2(Mode::GrayAlpha);
    assert_eq!(
        gray_alpha.check_pixel(Pixel::Gray(1)),
        Err(ImageError::ModeMismatch {
            expected: Mode::GrayAlpha,
            found: Mode::Gray
        })
    );
    let indexed = image_3_x_2(Mode::Indexed);
    assert_eq!(
        indexed.check_pixel(Pixel::Rgba(c0())),
        Err(ImageError::ModeMismatch {
            expected: Mode::Indexed,
            found: Mode::Rgba
        })
    );
    assert_eq!(
        indexed.check_pixel(Pixel::Gray(9)),
        Err(ImageError::ModeMismatch {
            expected: Mode::Indexed,
            found: Mode::Gray
        })
    );
}

#[test]
fn check_pixel_mode_mismatch_beats_index_outside_the_palette() {
    // Indexed(9) en una imagen RGBA: el modo va primero, no hay paleta con la que comparar.
    for mode in NOT_INDEXED {
        let img = image_3_x_2(mode);
        assert_eq!(
            img.check_pixel(Pixel::Indexed(9)),
            Err(mismatch(mode, Mode::Indexed))
        );
        assert_eq!(
            img.check_pixel(Pixel::Indexed(255)),
            Err(mismatch(mode, Mode::Indexed))
        );
    }
}

#[test]
fn check_pixel_index_outside_the_palette_does_not_change_the_image() {
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
        let img = creates(Mode::Indexed, 3, 2, n);
        let copy = img.clone();
        assert_eq!(
            img.check_pixel(Pixel::Indexed(index)),
            Err(ImageError::IndexNotInPalette { index, len: n }),
            "índice {index} con paleta de {n}"
        );
        assert_eq!(img, copy);
        assert_eq!(all_the_pixels(&img), all_the_pixels(&copy));
    }
}

#[test]
fn check_pixel_agrees_with_set_pixel_in_fixed_cases() {
    let pixels = [
        Pixel::Rgba(c0()),
        Pixel::Indexed(0),
        Pixel::Indexed(1),
        Pixel::Indexed(2),
        Pixel::Indexed(255),
        Pixel::Gray(7),
        Pixel::GrayAlpha { gray: 7, alpha: 9 },
    ];
    for mode in ALL_2 {
        for pixel in pixels {
            let img = image_3_x_2(mode);
            let mut copy = img.clone();
            assert_eq!(
                img.check_pixel(pixel),
                copy.set_pixel(0, 0, pixel),
                "{pixel:?} en una imagen {mode:?}"
            );
        }
    }
}

proptest! {
    /// Para cualquier píxel y modo, `check_pixel(p)` es `Err(e)` si y solo si
    /// `set_pixel(0, 0, p)` da `Err(e)` sobre una copia; y no cambia la imagen.
    #[test]
    fn check_pixel_agrees_with_set_pixel(img in image_small(), pixel in pixel_any()) {
        let before = img.clone();
        let checked = img.check_pixel(pixel);
        let mut copy = img.clone();
        let written = copy.set_pixel(0, 0, pixel);
        prop_assert_eq!(&checked, &written, "{:?} en una imagen {:?}", pixel, img.mode());
        prop_assert_eq!(checked.is_ok(), written.is_ok());
        prop_assert_eq!(&img, &before);
        // Si falla, `set_pixel` tampoco cambió su copia.
        if written.is_err() {
            prop_assert_eq!(&copy, &before);
        }
    }

    /// El resultado es el del modelo: modo distinto es `ModeMismatch`, índice `>= len` es
    /// `IndexNotInPalette` y en cualquier otro caso `Ok`.
    #[test]
    fn check_pixel_is_the_model(img in image_small(), pixel in pixel_any()) {
        let expected_2 = if mode_of(pixel) != img.mode() {
            Err(mismatch(img.mode(), mode_of(pixel)))
        } else {
            match (pixel, img.palette()) {
                (Pixel::Indexed(index), Some(p)) if usize::from(index) >= p.len() => {
                    Err(out_of_palette(index, p.len()))
                }
                _ => Ok(()),
            }
        };
        prop_assert_eq!(img.check_pixel(pixel), expected_2);
    }
}
