//! Documento de muestra que la ventana muestra mientras no se puedan abrir ficheros
//! (abrir PNG es la unidad 15 del hito 2).

use px_core::{Image, Mode, Pixel, Rgba};

pub const WIDTH: u32 = 64;
pub const HEIGHT: u32 = 48;

/// Imagen RGBA de 64×48: degradado con tablero, marco blanco de un píxel, un agujero
/// transparente de 8×8 en el centro y una diagonal semitransparente.
pub fn demo_image() -> Image {
    let mut image = Image::new(WIDTH, HEIGHT, Mode::Rgba).expect("64×48 RGBA es un tamaño válido");
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            image
                .set_pixel(x, y, Pixel::Rgba(demo_color(x, y)))
                .expect("las coordenadas están dentro de la imagen");
        }
    }
    image
}

fn demo_color(x: u32, y: u32) -> Rgba {
    if x == 0 || y == 0 || x == WIDTH - 1 || y == HEIGHT - 1 {
        return Rgba::opaque(255, 255, 255);
    }
    if (28..36).contains(&x) && (20..28).contains(&y) {
        return Rgba::TRANSPARENT;
    }
    let checker = (x / 8 + y / 8).is_multiple_of(2);
    let (r, g, b) = (x * 4, y * 5, if checker { 200 } else { 60 });
    let alpha = if x == y { 128 } else { 255 };
    Rgba::new(r as u8, g as u8, b, alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_imagen_de_muestra_tiene_el_tamano_y_los_casos_esperados() {
        let image = demo_image();
        assert_eq!((image.width(), image.height()), (WIDTH, HEIGHT));
        let at = |x, y| image.get_pixel(x, y).expect("dentro");
        assert_eq!(at(0, 0), Pixel::Rgba(Rgba::opaque(255, 255, 255)));
        assert_eq!(at(63, 47), Pixel::Rgba(Rgba::opaque(255, 255, 255)));
        assert_eq!(at(30, 22), Pixel::Rgba(Rgba::TRANSPARENT), "agujero");
        assert_eq!(
            at(10, 10),
            Pixel::Rgba(Rgba::new(40, 50, 200, 128)),
            "diagonal"
        );
        assert_eq!(at(10, 11), Pixel::Rgba(Rgba::new(40, 55, 200, 255)));
    }
}
