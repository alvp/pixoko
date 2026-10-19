//! Tests del acceso por filas y bytes a `Image` (specs/hito-2-image-filas-bytes.md).
//!
//! Ningún test reserva más de unos pocos KiB: el límite exacto de `Image::MAX_BYTES` se prueba
//! por el lado de `byte_len` y de `WrongLength` / `TooLarge`, que no reservan memoria.

use std::fmt::Debug;

use proptest::prelude::*;
use px_core::{Image, ImageError, Mode, Palette, Pixel, Rgba};

// Colores, paletas e imágenes de apoyo.

const NO_INDEXADOS: [Mode; 3] = [Mode::Rgba, Mode::Gray, Mode::GrayAlpha];
const TODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

fn paleta_de(n: usize) -> Palette {
    Palette::from_colors((0..n).map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))).unwrap()
}

/// Crea una imagen a ceros de cualquier modo; el indexado usa una paleta de `n` entradas.
fn crea(mode: Mode, width: u32, height: u32, n: usize) -> Image {
    match mode {
        Mode::Indexed => Image::new_indexed(width, height, paleta_de(n)).unwrap(),
        _ => Image::new(width, height, mode).unwrap(),
    }
}

/// Bytes por píxel de la tabla del formato, escritos aquí a mano (no con la API probada).
fn bpp(mode: Mode) -> usize {
    match mode {
        Mode::Rgba => 4,
        Mode::GrayAlpha => 2,
        Mode::Indexed | Mode::Gray => 1,
    }
}

/// Codificación de un píxel según la tabla del formato.
fn codifica(pixel: Pixel) -> Vec<u8> {
    match pixel {
        Pixel::Rgba(c) => vec![c.r, c.g, c.b, c.a],
        Pixel::Indexed(i) => vec![i],
        Pixel::Gray(g) => vec![g],
        Pixel::GrayAlpha { gray, alpha } => vec![gray, alpha],
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

/// Todos los píxeles de la imagen, fila a fila, por `get_pixel`.
fn todos_los_pixeles(img: &Image) -> Vec<Pixel> {
    let mut v = Vec::new();
    for y in 0..img.height() {
        for x in 0..img.width() {
            v.push(img.get_pixel(x, y).unwrap());
        }
    }
    v
}

/// Construcción lenta: `new` (o `new_indexed`) más `set_pixel` en cada píxel.
fn lenta(mode: Mode, width: u32, height: u32, n: usize, pixel: Pixel) -> Image {
    let mut img = crea(mode, width, height, n);
    for y in 0..height {
        for x in 0..width {
            img.set_pixel(x, y, pixel).unwrap();
        }
    }
    img
}

/// Píxel de la posición `k` de una imagen de ejemplo (con alfa 0 y RGB distinto de cero en
/// algunos píxeles; índices de 0 a 2).
fn pixel_de_ejemplo(mode: Mode, k: usize) -> Pixel {
    let k8 = k as u8;
    match mode {
        Mode::Rgba => Pixel::Rgba(Rgba::new(
            k8.wrapping_mul(7).wrapping_add(1),
            k8.wrapping_mul(3),
            255 - k8,
            if k.is_multiple_of(3) {
                0
            } else {
                k8.wrapping_mul(5)
            },
        )),
        Mode::Indexed => Pixel::Indexed((k % 3) as u8),
        Mode::Gray => Pixel::Gray(k8.wrapping_mul(11)),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: k8.wrapping_mul(11),
            alpha: if k.is_multiple_of(2) { 0 } else { k8 },
        },
    }
}

/// Imagen de ejemplo con píxeles variados; el indexado lleva una paleta de 3 colores.
fn imagen_de_ejemplo(mode: Mode, width: u32, height: u32) -> Image {
    let mut img = crea(mode, width, height, 3);
    for y in 0..height {
        for x in 0..width {
            let k = (y * width + x) as usize;
            img.set_pixel(x, y, pixel_de_ejemplo(mode, k)).unwrap();
        }
    }
    img
}

/// `from_bytes` o `from_bytes_indexed` según el modo, con la paleta de la imagen de origen.
fn reconstruye(original: &Image, data: Vec<u8>) -> Result<Image, ImageError> {
    let (w, h) = (original.width(), original.height());
    match original.palette() {
        Some(p) => Image::from_bytes_indexed(w, h, p.clone(), data),
        None => Image::from_bytes(w, h, original.mode(), data),
    }
}

fn byte_len(width: u32, height: u32, mode: Mode) -> usize {
    Image::byte_len(width, height, mode).unwrap()
}

fn fuera_de_paleta(index: u8, len: usize) -> ImageError {
    ImageError::IndexNotInPalette { index, len }
}

fn largo_erroneo(expected: usize, found: usize) -> ImageError {
    ImageError::WrongLength { expected, found }
}

fn es_too_large<T: Debug>(r: Result<T, ImageError>) -> bool {
    matches!(r, Err(ImageError::TooLarge { .. }))
}

// Una prueba por fila de la tabla de casos.

#[test]
fn fila_01_bytes_y_row_len_en_los_cuatro_modos() {
    // Modo, píxel escrito en (1, 0) y sus bytes.
    let casos: [(Mode, Pixel, &[u8]); 4] = [
        (
            Mode::Rgba,
            Pixel::Rgba(Rgba::new(1, 2, 3, 4)),
            &[1, 2, 3, 4],
        ),
        (Mode::Indexed, Pixel::Indexed(1), &[1]),
        (Mode::Gray, Pixel::Gray(7), &[7]),
        (
            Mode::GrayAlpha,
            Pixel::GrayAlpha { gray: 7, alpha: 9 },
            &[7, 9],
        ),
    ];
    for (mode, pixel, esperados) in casos {
        let mut img = crea(mode, 3, 2, 3);
        let por_pixel = mode.bytes_per_pixel();
        assert_eq!(por_pixel, bpp(mode), "bytes_per_pixel({mode:?})");
        assert_eq!(img.bytes().len(), byte_len(3, 2, mode), "{mode:?}");
        assert_eq!(img.row_len(), 3 * por_pixel, "{mode:?}");

        img.set_pixel(1, 0, pixel).unwrap();
        assert_eq!(img.bytes().len(), byte_len(3, 2, mode), "{mode:?}");
        let at = por_pixel; // (0 × 3 + 1) × bpp
        assert_eq!(&img.bytes()[at..at + por_pixel], esperados, "{mode:?}");
        // El resto sigue a ceros.
        for (k, &b) in img.bytes().iter().enumerate() {
            if k < at || k >= at + por_pixel {
                assert_eq!(b, 0, "{mode:?}: byte {k}");
            }
        }
    }
}

#[test]
fn fila_01_rgba_con_alfa_cero_conserva_el_rgb() {
    let mut img = Image::new(3, 2, Mode::Rgba).unwrap();
    img.set_pixel(1, 0, Pixel::Rgba(Rgba::new(1, 2, 3, 0)))
        .unwrap();
    assert_eq!(&img.bytes()[4..8], &[1, 2, 3, 0]);
}

#[test]
fn fila_01_bytes_per_pixel_vale_4_1_1_2() {
    assert_eq!(Mode::Rgba.bytes_per_pixel(), 4);
    assert_eq!(Mode::Indexed.bytes_per_pixel(), 1);
    assert_eq!(Mode::Gray.bytes_per_pixel(), 1);
    assert_eq!(Mode::GrayAlpha.bytes_per_pixel(), 2);
}

#[test]
fn fila_02_row_devuelve_la_fila_o_none() {
    for mode in TODOS {
        let img = imagen_de_ejemplo(mode, 3, 2);
        let largo = img.row_len();
        assert_eq!(largo, 3 * bpp(mode), "{mode:?}");
        for y in 0..2u32 {
            let fila = img
                .row(y)
                .unwrap_or_else(|| panic!("{mode:?}: row({y}) es None"));
            assert_eq!(fila.len(), largo, "{mode:?} y = {y}");
            let desde = y as usize * largo;
            assert_eq!(fila, &img.bytes()[desde..desde + largo], "{mode:?} y = {y}");
        }
        assert_eq!(img.row(2), None, "{mode:?}");
        assert_eq!(img.row(u32::MAX), None, "{mode:?}");
    }
}

/// Comprobación en tiempo de compilación de lo que promete `rows()`.
fn exige_iterador_de_filas<'a, I>(_: I)
where
    I: ExactSizeIterator<Item = &'a [u8]> + DoubleEndedIterator + Clone,
{
}

#[test]
fn fila_03_rows_es_la_secuencia_de_row() {
    for mode in TODOS {
        let img = imagen_de_ejemplo(mode, 3, 2);
        exige_iterador_de_filas(img.rows());

        let filas = img.rows();
        assert_eq!(filas.len(), 2, "{mode:?}");
        let por_filas: Vec<&[u8]> = filas.clone().collect();
        assert_eq!(por_filas.len(), 2, "{mode:?}");
        for (y, fila) in por_filas.iter().enumerate() {
            assert_eq!(Some(*fila), img.row(y as u32), "{mode:?} y = {y}");
        }

        // `.rev()` empieza por la última.
        assert_eq!(img.rows().next_back(), img.row(1), "{mode:?}");
        let al_reves: Vec<&[u8]> = img.rows().rev().collect();
        assert_eq!(al_reves, vec![img.row(1).unwrap(), img.row(0).unwrap()]);

        // La concatenación es `bytes()`.
        assert_eq!(por_filas.concat(), img.bytes(), "{mode:?}");
    }
}

#[test]
fn fila_04_formas_limite_en_los_cuatro_modos() {
    for mode in TODOS {
        for (w, h) in [(1u32, 1u32), (1, 5), (5, 1)] {
            let img = crea(mode, w, h, 3);
            assert_eq!(img.rows().len(), h as usize, "{mode:?} {w}x{h}");
            assert_eq!(img.row_len(), w as usize * bpp(mode), "{mode:?} {w}x{h}");
            assert_eq!(img.bytes().len(), byte_len(w, h, mode), "{mode:?} {w}x{h}");
            assert_eq!(img.rows().count(), h as usize, "{mode:?} {w}x{h}");
            assert!(img.row(h - 1).is_some(), "{mode:?} {w}x{h}");
            assert!(img.row(h).is_none(), "{mode:?} {w}x{h}");
        }
    }
}

#[test]
fn fila_05_filled_pone_el_mismo_pixel_en_todos_y_es_igual_a_la_lenta() {
    let pixeles = [
        (Mode::Rgba, Pixel::Rgba(Rgba::new(10, 20, 30, 255))),
        (Mode::Rgba, Pixel::Rgba(Rgba::new(1, 2, 3, 0))),
        (Mode::Gray, Pixel::Gray(255)),
        (
            Mode::GrayAlpha,
            Pixel::GrayAlpha {
                gray: 255,
                alpha: 0,
            },
        ),
    ];
    for (mode, p) in pixeles {
        let img = Image::filled(3, 2, p).unwrap_or_else(|e| panic!("filled({p:?}): {e:?}"));
        assert_eq!((img.width(), img.height()), (3, 2), "{p:?}");
        assert_eq!(img.mode(), mode, "{p:?}");
        assert_eq!(img.palette(), None, "{p:?}");
        assert_eq!(todos_los_pixeles(&img), vec![p; 6], "{p:?}");
        assert_eq!(img, lenta(mode, 3, 2, 3, p), "{p:?}");
    }
}

#[test]
fn fila_06_filled_errores_de_tamano_y_de_modo() {
    let pixeles = [
        Pixel::Rgba(Rgba::new(1, 2, 3, 4)),
        Pixel::Gray(5),
        Pixel::GrayAlpha { gray: 5, alpha: 6 },
    ];
    for p in pixeles {
        assert_eq!(
            Image::filled(0, 5, p),
            Err(ImageError::ZeroSize {
                width: 0,
                height: 5
            }),
            "{p:?}"
        );
        assert_eq!(
            Image::filled(u32::MAX, u32::MAX, p),
            Err(ImageError::TooLarge {
                width: u32::MAX,
                height: u32::MAX
            }),
            "{p:?}"
        );
    }
    // Un byte por encima de MAX_BYTES: se rechaza sin reservar.
    assert_eq!(
        Image::filled(16385, 16384, Pixel::Rgba(Rgba::new(1, 2, 3, 4))),
        Err(ImageError::TooLarge {
            width: 16385,
            height: 16384
        })
    );
    // Indexado: `PaletteRequired`, y las dimensiones ganan al modo.
    assert_eq!(
        Image::filled(2, 2, Pixel::Indexed(0)),
        Err(ImageError::PaletteRequired)
    );
    assert_eq!(
        Image::filled(0, 2, Pixel::Indexed(0)),
        Err(ImageError::ZeroSize {
            width: 0,
            height: 2
        })
    );
    assert_eq!(
        Image::filled(u32::MAX, u32::MAX, Pixel::Indexed(0)),
        Err(ImageError::TooLarge {
            width: u32::MAX,
            height: u32::MAX
        })
    );
}

#[test]
fn fila_07_filled_indexed_valido_y_errores_en_orden() {
    let p3 = paleta_de(3);
    let img = Image::filled_indexed(3, 2, p3.clone(), 2).unwrap();
    assert_eq!((img.width(), img.height()), (3, 2));
    assert_eq!(img.mode(), Mode::Indexed);
    assert_eq!(img.palette(), Some(&p3));
    assert_eq!(todos_los_pixeles(&img), vec![Pixel::Indexed(2); 6]);
    assert_eq!(img, lenta(Mode::Indexed, 3, 2, 3, Pixel::Indexed(2)));

    // Paleta de 256 e índice 255.
    let p256 = paleta_de(256);
    let img = Image::filled_indexed(3, 2, p256.clone(), 255).unwrap();
    assert_eq!(img.palette(), Some(&p256));
    assert_eq!(todos_los_pixeles(&img), vec![Pixel::Indexed(255); 6]);
    assert_eq!(img, lenta(Mode::Indexed, 3, 2, 256, Pixel::Indexed(255)));

    // Índice fuera de la paleta.
    assert_eq!(
        Image::filled_indexed(3, 2, p3, 3),
        Err(fuera_de_paleta(3, 3))
    );
    // Paleta vacía (gana al índice).
    assert_eq!(
        Image::filled_indexed(3, 2, Palette::new(), 0),
        Err(ImageError::EmptyPalette)
    );
    // Dimensiones, luego paleta, luego índice.
    assert_eq!(
        Image::filled_indexed(0, 2, paleta_de(1), 5),
        Err(ImageError::ZeroSize {
            width: 0,
            height: 2
        })
    );
    assert_eq!(
        Image::filled_indexed(0, 2, Palette::new(), 5),
        Err(ImageError::ZeroSize {
            width: 0,
            height: 2
        })
    );
    assert!(es_too_large(Image::filled_indexed(
        u32::MAX,
        u32::MAX,
        Palette::new(),
        5
    )));
}

#[test]
fn fila_08_try_clone_es_clone_sin_compartir_memoria() {
    for mode in TODOS {
        for (w, h) in [(3u32, 2u32), (1, 1)] {
            let original = imagen_de_ejemplo(mode, w, h);
            let mut copia = original
                .try_clone()
                .unwrap_or_else(|e| panic!("{mode:?} {w}x{h}: {e:?}"));
            assert_eq!(copia, original, "{mode:?} {w}x{h}");
            assert_eq!(copia, original.clone(), "{mode:?} {w}x{h}");
            assert_eq!(copia.palette(), original.palette(), "{mode:?} {w}x{h}");
            assert_ne!(
                copia.bytes().as_ptr(),
                original.bytes().as_ptr(),
                "{mode:?} {w}x{h}: comparten memoria"
            );

            // Escribir en la copia no cambia el original.
            let antes = original.clone();
            let nuevo = match mode {
                Mode::Rgba => Pixel::Rgba(Rgba::new(250, 251, 252, 253)),
                Mode::Indexed => Pixel::Indexed(2),
                Mode::Gray => Pixel::Gray(250),
                Mode::GrayAlpha => Pixel::GrayAlpha {
                    gray: 250,
                    alpha: 251,
                },
            };
            copia.set_pixel(0, 0, nuevo).unwrap();
            assert_eq!(original, antes, "{mode:?} {w}x{h}");
            assert_eq!(copia.get_pixel(0, 0), Ok(nuevo), "{mode:?} {w}x{h}");
        }
    }
}

#[test]
fn fila_09_from_bytes_de_los_bytes_de_una_imagen_es_igual_a_ella() {
    for mode in TODOS {
        let original = imagen_de_ejemplo(mode, 4, 3);
        let datos = original.bytes().to_vec();
        let reconstruida =
            reconstruye(&original, datos).unwrap_or_else(|e| panic!("{mode:?}: {e:?}"));
        assert_eq!(reconstruida, original, "{mode:?}");
        assert_eq!(reconstruida.bytes(), original.bytes(), "{mode:?}");
    }
    // El alfa 0 conserva su RGB.
    let original = imagen_de_ejemplo(Mode::Rgba, 4, 3);
    let reconstruida = reconstruye(&original, original.bytes().to_vec()).unwrap();
    assert_eq!(
        reconstruida.get_pixel(0, 0),
        Ok(Pixel::Rgba(Rgba::new(1, 0, 255, 0)))
    );
}

#[test]
fn fila_10_from_bytes_longitud_erronea_y_orden_de_errores() {
    for mode in NO_INDEXADOS {
        let largo = byte_len(3, 2, mode);
        for found in [largo - 1, largo + 1, 0] {
            assert_eq!(
                Image::from_bytes(3, 2, mode, vec![0; found]),
                Err(largo_erroneo(largo, found)),
                "{mode:?} con {found} bytes"
            );
        }
        // Dimensiones primero: `ZeroSize` aunque `data` no cuadre.
        assert_eq!(
            Image::from_bytes(0, 5, mode, vec![1, 2, 3]),
            Err(ImageError::ZeroSize {
                width: 0,
                height: 5
            }),
            "{mode:?}"
        );
        assert_eq!(
            Image::from_bytes(0, 5, mode, vec![]),
            Err(ImageError::ZeroSize {
                width: 0,
                height: 5
            }),
            "{mode:?}"
        );
    }
    // `TooLarge` antes que `WrongLength`, sin mirar `data`.
    assert_eq!(
        Image::from_bytes(16385, 16384, Mode::Rgba, vec![]),
        Err(ImageError::TooLarge {
            width: 16385,
            height: 16384
        })
    );
    // El límite exacto cabe en `byte_len` y se rechaza por longitud, sin reservar 1 GiB.
    assert_eq!(
        Image::from_bytes(16384, 16384, Mode::Rgba, vec![]),
        Err(largo_erroneo(Image::MAX_BYTES, 0))
    );
    // `PaletteRequired` por el modo, no por los bytes (y antes que la longitud).
    assert_eq!(
        Image::from_bytes(2, 2, Mode::Indexed, vec![0; 4]),
        Err(ImageError::PaletteRequired)
    );
    assert_eq!(
        Image::from_bytes(2, 2, Mode::Indexed, vec![]),
        Err(ImageError::PaletteRequired)
    );
    // Las dimensiones ganan a `PaletteRequired`.
    assert_eq!(
        Image::from_bytes(0, 2, Mode::Indexed, vec![]),
        Err(ImageError::ZeroSize {
            width: 0,
            height: 2
        })
    );
}

#[test]
fn fila_11_from_bytes_indexed_indices_paleta_y_longitud() {
    let p2 = paleta_de(2);

    let img = Image::from_bytes_indexed(2, 2, p2.clone(), vec![0, 1, 0, 1]).unwrap();
    assert_eq!(img.mode(), Mode::Indexed);
    assert_eq!(img.palette(), Some(&p2));
    assert_eq!(img.bytes(), &[0, 1, 0, 1]);
    assert_eq!(
        todos_los_pixeles(&img),
        vec![
            Pixel::Indexed(0),
            Pixel::Indexed(1),
            Pixel::Indexed(0),
            Pixel::Indexed(1)
        ]
    );

    assert_eq!(
        Image::from_bytes_indexed(2, 2, p2.clone(), vec![0, 1, 2, 1]),
        Err(fuera_de_paleta(2, 2))
    );
    // Con varios índices fuera, el mayor.
    assert_eq!(
        Image::from_bytes_indexed(2, 2, p2.clone(), vec![255, 3, 0, 1]),
        Err(fuera_de_paleta(255, 2))
    );
    assert_eq!(
        Image::from_bytes_indexed(2, 2, p2, vec![3, 255, 4, 0]),
        Err(fuera_de_paleta(255, 2))
    );

    // Paleta de 256 con los 256 valores (16 × 16).
    let p256 = paleta_de(256);
    let todos: Vec<u8> = (0..=255).collect();
    let img = Image::from_bytes_indexed(16, 16, p256.clone(), todos.clone()).unwrap();
    assert_eq!(img.bytes(), &todos[..]);
    assert_eq!(img.palette(), Some(&p256));

    // Paleta vacía con la longitud correcta; con la incorrecta gana la longitud.
    assert_eq!(
        Image::from_bytes_indexed(2, 2, Palette::new(), vec![0; 4]),
        Err(ImageError::EmptyPalette)
    );
    assert_eq!(
        Image::from_bytes_indexed(2, 2, Palette::new(), vec![0; 3]),
        Err(largo_erroneo(4, 3))
    );
    // Las dimensiones ganan a todo lo demás.
    assert_eq!(
        Image::from_bytes_indexed(0, 2, Palette::new(), vec![9; 3]),
        Err(ImageError::ZeroSize {
            width: 0,
            height: 2
        })
    );
    // Indexado es un byte por píxel: 16385 × 16384 cabe, hay que desbordar de verdad.
    assert!(es_too_large(Image::from_bytes_indexed(
        u32::MAX,
        u32::MAX,
        Palette::new(),
        vec![]
    )));
    // La longitud gana a los índices.
    assert_eq!(
        Image::from_bytes_indexed(2, 2, paleta_de(2), vec![200, 200, 200]),
        Err(largo_erroneo(4, 3))
    );
}

#[test]
fn fila_12_uses_index() {
    let mut img = Image::new_indexed(3, 2, paleta_de(3)).unwrap();
    for (k, i) in [0u8, 1, 0, 1, 1, 0].into_iter().enumerate() {
        img.set_pixel(k as u32 % 3, k as u32 / 3, Pixel::Indexed(i))
            .unwrap();
    }
    assert!(img.uses_index(0));
    assert!(img.uses_index(1));
    assert!(!img.uses_index(2));
    assert!(!img.uses_index(255));

    // Una imagen que no es indexada nunca usa un índice, aunque sus bytes valgan 0 o 2.
    for mode in NO_INDEXADOS {
        let a_ceros = crea(mode, 3, 2, 3);
        let variada = imagen_de_ejemplo(mode, 3, 2);
        for index in [0u8, 1, 2, 255] {
            assert!(!a_ceros.uses_index(index), "{mode:?} a ceros, {index}");
            assert!(!variada.uses_index(index), "{mode:?} variada, {index}");
        }
    }
}

// Propiedades.

/// Imagen de la propiedad 1 y 4: modo, tamaño, longitud de paleta y el modelo de píxeles.
#[derive(Clone, Debug)]
struct Caso {
    mode: Mode,
    width: u32,
    height: u32,
    n: usize,
    modelo: Vec<Pixel>,
}

impl Caso {
    /// La imagen, construida con `new` más `set_pixel` desde el modelo.
    fn imagen(&self) -> Image {
        let mut img = crea(self.mode, self.width, self.height, self.n);
        for y in 0..self.height {
            for x in 0..self.width {
                let p = self.modelo[(y * self.width + x) as usize];
                img.set_pixel(x, y, p).unwrap();
            }
        }
        img
    }
}

fn modo() -> impl Strategy<Value = Mode> {
    prop::sample::select(TODOS.to_vec())
}

/// Longitud de paleta de 1 a 256, con sesgo hacia las pequeñas (más repeticiones de índice).
fn longitud_de_paleta() -> impl Strategy<Value = usize> {
    prop_oneof![1usize..=8, 1usize..=256]
}

/// Píxel válido para un modo (y una paleta de `n` entradas) a partir de cuatro bytes.
fn pixel_valido(mode: Mode, n: usize, (a, b, c, d): (u8, u8, u8, u8)) -> Pixel {
    match mode {
        Mode::Rgba => Pixel::Rgba(Rgba::new(a, b, c, d)),
        Mode::Indexed => Pixel::Indexed((usize::from(a) % n) as u8),
        Mode::Gray => Pixel::Gray(a),
        Mode::GrayAlpha => Pixel::GrayAlpha { gray: a, alpha: b },
    }
}

/// Casos de 1 a 16 de lado con un modelo de píxeles con repeticiones (se eligen entre unos
/// pocos valores distintos, a veces con alfa 0).
fn caso() -> impl Strategy<Value = Caso> {
    (
        modo(),
        1u32..=16,
        1u32..=16,
        longitud_de_paleta(),
        prop::collection::vec(any::<(u8, u8, u8, u8)>(), 1..=6),
        prop::collection::vec(any::<usize>(), 256),
    )
        .prop_map(|(mode, width, height, n, valores, elecciones)| {
            let total = (width * height) as usize;
            let modelo = (0..total)
                .map(|k| {
                    let (a, b, c, d) = valores[elecciones[k] % valores.len()];
                    // Una parte de los píxeles con alfa 0 (y RGB distinto de cero).
                    let d = if elecciones[k] % 5 == 0 { 0 } else { d };
                    pixel_valido(mode, n, (a, b, c, d))
                })
                .collect();
            Caso {
                mode,
                width,
                height,
                n,
                modelo,
            }
        })
}

/// Bytes esperados de un modelo, según la tabla del formato (nunca con `get_pixel`).
fn bytes_del_modelo(modelo: &[Pixel]) -> Vec<u8> {
    modelo.iter().flat_map(|&p| codifica(p)).collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Bytes y filas frente a la construcción por `set_pixel`, en los cuatro modos.
    #[test]
    fn bytes_y_filas_son_los_del_modelo(caso in caso()) {
        let img = caso.imagen();
        let (w, h) = (caso.width, caso.height);
        let esperados = bytes_del_modelo(&caso.modelo);

        prop_assert_eq!(img.bytes().len(), byte_len(w, h, caso.mode));
        prop_assert_eq!(esperados.len(), img.bytes().len());
        prop_assert_eq!(img.bytes(), &esperados[..]);

        let largo = w as usize * bpp(caso.mode);
        prop_assert_eq!(img.row_len(), largo);
        for y in 0..h {
            let desde = y as usize * largo;
            prop_assert_eq!(img.row(y), Some(&esperados[desde..desde + largo]), "y = {}", y);
        }
        prop_assert_eq!(img.row(h), None);
        prop_assert_eq!(img.row(u32::MAX), None);

        prop_assert_eq!(img.rows().len(), h as usize);
        let filas: Vec<&[u8]> = img.rows().collect();
        prop_assert_eq!(filas.concat(), esperados);
        prop_assert_eq!(img.rows().next_back(), img.row(h - 1));

        for y in 0..h {
            for x in 0..w {
                let p = caso.modelo[(y * w + x) as usize];
                prop_assert_eq!(img.get_pixel(x, y), Ok(p), "({}, {})", x, y);
            }
        }
    }

    /// La construcción rápida es igual a la lenta; `try_clone` a `clone`; `from_bytes` de los
    /// bytes devuelve la imagen original, tras cualquier serie de `set_pixel`.
    #[test]
    fn construccion_rapida_igual_a_la_lenta(
        mode in modo(),
        width in 1u32..=16,
        height in 1u32..=16,
        n in longitud_de_paleta(),
        semilla in any::<(u8, u8, u8, u8)>(),
        serie in prop::collection::vec((any::<u32>(), any::<u32>(), any::<(u8, u8, u8, u8)>()), 0..=40),
    ) {
        let pixel = pixel_valido(mode, n, semilla);
        prop_assert_eq!(modo_de(pixel), mode);

        let rapida = match pixel {
            Pixel::Indexed(i) => Image::filled_indexed(width, height, paleta_de(n), i),
            _ => Image::filled(width, height, pixel),
        };
        prop_assert_eq!(rapida, Ok(lenta(mode, width, height, n, pixel)));

        let mut img = lenta(mode, width, height, n, pixel);
        for (xb, yb, bytes) in serie {
            img.set_pixel(xb % width, yb % height, pixel_valido(mode, n, bytes)).unwrap();
        }
        prop_assert_eq!(img.try_clone(), Ok(img.clone()));
        prop_assert_eq!(reconstruye(&img, img.bytes().to_vec()), Ok(img.clone()));
    }

    /// `from_bytes` con datos arbitrarios: `Ok` solo con la longitud exacta y, en indexado,
    /// con todos los índices dentro de la paleta; si no, el error del orden fijo.
    #[test]
    fn from_bytes_con_datos_arbitrarios(
        mode in modo(),
        width in 1u32..=8,
        height in 1u32..=8,
        n in longitud_de_paleta(),
        d in prop_oneof![3 => Just(0i32), 2 => -2i32..=2],
        crudos in prop::collection::vec(any::<u8>(), 258),
        dentro_de_la_paleta in any::<bool>(),
    ) {
        let esperado = byte_len(width, height, mode);
        let largo = (esperado as i64 + i64::from(d)).max(0) as usize;
        let mut data: Vec<u8> = crudos[..largo].to_vec();
        if mode == Mode::Indexed && dentro_de_la_paleta {
            for b in &mut data {
                *b = (usize::from(*b) % n) as u8;
            }
        }
        let copia = data.clone();

        let resultado = if mode == Mode::Indexed {
            // `from_bytes` con el modo indexado es `PaletteRequired`, sea cual sea `data`.
            prop_assert_eq!(
                Image::from_bytes(width, height, mode, data.clone()),
                Err(ImageError::PaletteRequired)
            );
            Image::from_bytes_indexed(width, height, paleta_de(n), data)
        } else {
            Image::from_bytes(width, height, mode, data)
        };

        let previsto = if copia.len() != esperado {
            Err(largo_erroneo(esperado, copia.len()))
        } else if mode == Mode::Indexed && usize::from(*copia.iter().max().unwrap()) >= n {
            Err(fuera_de_paleta(*copia.iter().max().unwrap(), n))
        } else {
            Ok(())
        };

        match (resultado, previsto) {
            (Ok(img), Ok(())) => {
                prop_assert_eq!(img.bytes(), &copia[..]);
                prop_assert_eq!((img.width(), img.height(), img.mode()), (width, height, mode));
                for y in 0..height {
                    for x in 0..width {
                        let p = img.get_pixel(x, y);
                        prop_assert!(p.is_ok(), "get_pixel({}, {}) = {:?}", x, y, p);
                        if let Ok(Pixel::Indexed(i)) = p {
                            prop_assert!(usize::from(i) < n, "índice {} con paleta de {}", i, n);
                        }
                    }
                }
                if mode == Mode::Indexed {
                    prop_assert_eq!(img.palette().map(Palette::len), Some(n));
                } else {
                    prop_assert_eq!(img.palette(), None);
                }
            }
            (Err(e), Err(esperado)) => prop_assert_eq!(e, esperado),
            (real, previsto) => {
                prop_assert!(false, "real {:?}, previsto {:?}", real, previsto);
            }
        }
    }

    /// `uses_index` es verdadero si y solo si algún píxel es `Indexed(index)`.
    #[test]
    fn uses_index_es_algun_pixel_igual(
        caso in caso(),
        index in prop_oneof![2 => any::<u8>(), 3 => 0u8..8],
    ) {
        let img = caso.imagen();
        let algun_pixel = todos_los_pixeles(&img).contains(&Pixel::Indexed(index));
        prop_assert_eq!(img.uses_index(index), algun_pixel);
        if caso.mode != Mode::Indexed {
            prop_assert!(!img.uses_index(index));
        }
        // Y con un índice que sí está: el de un píxel cualquiera del modelo.
        if let Some(Pixel::Indexed(i)) = caso.modelo.first().copied() {
            prop_assert!(img.uses_index(i));
        }
    }
}
