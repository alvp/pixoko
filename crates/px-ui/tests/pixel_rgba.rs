//! Tests de `texture::pixel_rgba`, la conversión de un `Pixel` al color con que se ve
//! (specs/hito-2-lapiz-deshacer.md, sección 2: superposición del trazo). Debe coincidir con lo
//! que `to_rgba8` pone en la textura para el mismo píxel.

use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_ui::texture::{pixel_rgba, to_rgba8};

#[test]
fn each_mode_gives_the_color_that_is_seen() {
    let rgba = Image::new(1, 1, Mode::Rgba).expect("imagen válida");
    let color = Rgba::new(1, 2, 3, 4);
    assert_eq!(pixel_rgba(&rgba, Pixel::Rgba(color)), Some(color));

    let gray = Image::new(1, 1, Mode::Gray).expect("imagen válida");
    assert_eq!(
        pixel_rgba(&gray, Pixel::Gray(96)),
        Some(Rgba::opaque(96, 96, 96))
    );

    let gray_alpha = Image::new(1, 1, Mode::GrayAlpha).expect("imagen válida");
    assert_eq!(
        pixel_rgba(&gray_alpha, Pixel::GrayAlpha { gray: 7, alpha: 0 }),
        Some(Rgba::new(7, 7, 7, 0))
    );
}

#[test]
fn indexed_uses_the_palette_and_none_if_the_index_is_not_there() {
    let mut palette = Palette::from_colors([Rgba::opaque(9, 8, 7), Rgba::new(1, 2, 3, 77)])
        .expect("paleta válida");
    palette
        .set_transparent_index(Some(1))
        .expect("el índice existe");
    let image = Image::new_indexed(1, 1, palette).expect("imagen válida");
    assert_eq!(
        pixel_rgba(&image, Pixel::Indexed(0)),
        Some(Rgba::opaque(9, 8, 7))
    );
    assert_eq!(
        pixel_rgba(&image, Pixel::Indexed(1)),
        Some(Rgba::new(1, 2, 3, 0)),
        "el índice transparente da alfa 0 y conserva su RGB"
    );
    assert_eq!(pixel_rgba(&image, Pixel::Indexed(2)), None);
    // En una imagen que no es indexada no hay paleta.
    let rgba = Image::new(1, 1, Mode::Rgba).expect("imagen válida");
    assert_eq!(pixel_rgba(&rgba, Pixel::Indexed(0)), None);
}

#[test]
fn matches_to_rgba8() {
    let mut image = Image::new(2, 1, Mode::GrayAlpha).expect("imagen válida");
    let pixels = [
        Pixel::GrayAlpha { gray: 5, alpha: 6 },
        Pixel::GrayAlpha {
            gray: 200,
            alpha: 255,
        },
    ];
    for (x, pixel) in pixels.into_iter().enumerate() {
        image.set_pixel(x as u32, 0, pixel).expect("dentro");
    }
    let bytes = to_rgba8(&image);
    for (x, pixel) in pixels.into_iter().enumerate() {
        let c = pixel_rgba(&image, pixel).expect("gris con alfa");
        assert_eq!(&bytes[x * 4..x * 4 + 4], &[c.r, c.g, c.b, c.a]);
    }
}
