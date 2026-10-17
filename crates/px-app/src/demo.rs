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

#[cfg(test)]
mod fill_tests {
    //! Filas 13 y 14 de specs/hito-2-linea-relleno-app.md: la herramienta de relleno sobre la imagen
    //! de muestra (`demo_image`), que vive en el binario.
    //!
    //! Las cifras están calculadas a mano (marco: 4 lados de un píxel, 220 píxeles en 94 tramos;
    //! agujero: 8×8 en 8 tramos; un píxel de la diagonal o del degradado: 1) y, además, se
    //! calcula el resultado con un oráculo ingenuo: recorrido pixel a pixel con cola (`VecDeque`,
    //! 4 vecinos, `Image::get_pixel`) y tramos por filas escritos aquí, sin `px-raster`. Con esos
    //! tramos se arma un `FillSpans` en un documento de referencia, y su historia ha de pesar lo
    //! mismo que la de la herramienta: un solo paso de relleno, sin trozos de más.

    use std::collections::{BTreeSet, VecDeque};

    use px_core::{Command, Document, Span};
    use px_ui::canvas::CanvasEvent;
    use px_ui::color::pixel_for_slot;
    use px_ui::tool::{Tool, ToolAction, ToolState};

    use super::*;

    /// Recorrido ingenuo pixel a pixel con cola: 4 vecinos y valor exacto (`get_pixel`).
    fn region_ingenua(img: &Image, sx: u32, sy: u32) -> BTreeSet<(u32, u32)> {
        let valor = img.get_pixel(sx, sy).expect("dentro");
        let mut region = BTreeSet::new();
        let mut cola = VecDeque::new();
        region.insert((sx, sy));
        cola.push_back((sx, sy));
        while let Some((x, y)) = cola.pop_front() {
            let (xi, yi) = (i64::from(x), i64::from(y));
            for (nx, ny) in [(xi - 1, yi), (xi + 1, yi), (xi, yi - 1), (xi, yi + 1)] {
                let (Ok(nx), Ok(ny)) = (u32::try_from(nx), u32::try_from(ny)) else {
                    continue;
                };
                if !region.contains(&(nx, ny)) && img.get_pixel(nx, ny) == Ok(valor) {
                    region.insert((nx, ny));
                    cola.push_back((nx, ny));
                }
            }
        }
        region
    }

    /// Rachas máximas de la región en cada fila, ordenadas por `(y, x_start)`.
    fn tramos_de(region: &BTreeSet<(u32, u32)>) -> Vec<Span> {
        let mut tramos: Vec<Span> = Vec::new();
        // `BTreeSet` de `(x, y)` ordena por `x` primero: se pasa a `(y, x)` para recorrer por filas.
        let por_filas: BTreeSet<(u32, u32)> = region.iter().map(|&(x, y)| (y, x)).collect();
        for (y, x) in por_filas {
            match tramos.last_mut() {
                Some(t) if t.y == y && t.x_end == x => t.x_end = x + 1,
                _ => tramos.push(Span {
                    y,
                    x_start: x,
                    x_end: x + 1,
                }),
            }
        }
        tramos
    }

    /// Cuántas veces ha dado `Ok(true)` un clic de relleno y con qué documento queda.
    struct Clic {
        doc: Document,
        id: px_core::ImageId,
        cambio: bool,
    }

    /// Un clic de relleno con la ranura `slot` en `(x, y)` sobre una imagen de muestra nueva:
    /// `Begin`, un `Move` y `End`, con la herramienta elegida con `Select(Fill)`.
    fn clic(slot: u8, x: i32, y: i32) -> Clic {
        let mut doc = Document::new(demo_image());
        let id = doc.main_image();
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(Tool::Fill));
        assert_eq!(tool.tool(), Tool::Fill);
        let cambio = tool
            .canvas_event(&mut doc, id, slot, CanvasEvent::StrokeBegin((x, y)))
            .expect("el relleno no falla");
        assert_eq!(
            tool.canvas_event(&mut doc, id, slot, CanvasEvent::StrokeMove((x, y))),
            Ok(false)
        );
        assert_eq!(
            tool.canvas_event(&mut doc, id, slot, CanvasEvent::StrokeEnd),
            Ok(false)
        );
        assert!(!tool.busy());
        Clic { doc, id, cambio }
    }

    /// Fila 13: el resultado de la herramienta es el del oráculo ingenuo, con las cifras a mano.
    fn comprueba_relleno(slot: u8, (x, y): (u32, u32), pixeles: usize, tramos: usize) {
        let original = demo_image();
        let pixel = pixel_for_slot(&original, slot).expect("la ranura tiene color");
        let region = region_ingenua(&original, x, y);
        let del_oraculo = tramos_de(&region);
        assert_eq!(region.len(), pixeles, "píxeles de la región (a mano)");
        assert_eq!(del_oraculo.len(), tramos, "tramos de la región (a mano)");

        let hecho = clic(slot, x as i32, y as i32);
        assert!(hecho.cambio, "Ok(true) en el Begin");

        // Igual al oráculo: la original con la región en `pixel` y nada más.
        let mut esperada = original.clone();
        for &(rx, ry) in &region {
            esperada.set_pixel(rx, ry, pixel).expect("dentro");
        }
        let resultado = hecho.doc.image(hecho.id).expect("hay imagen");
        assert_eq!(resultado, &esperada);
        let cambiados = (0..HEIGHT)
            .flat_map(|py| (0..WIDTH).map(move |px| (px, py)))
            .filter(|&(px, py)| resultado.get_pixel(px, py) != original.get_pixel(px, py))
            .count();
        assert_eq!(cambiados, pixeles, "solo cambia la región");

        // Un solo `FillSpans` con los tramos del oráculo: mismo peso de historia.
        let mut referencia = Document::new(original.clone());
        let rid = referencia.main_image();
        let paso = Command::FillSpans {
            image: rid,
            spans: del_oraculo,
            pixel,
        };
        assert_eq!(referencia.apply(paso), Ok(true));
        assert_eq!(hecho.doc.history_bytes(), referencia.history_bytes());
        assert_eq!(resultado, referencia.image(rid).expect("hay imagen"));

        // Un paso y `undo` exacto.
        let mut doc = hecho.doc;
        assert!(doc.can_undo());
        assert_eq!(doc.undo(), Ok(()));
        assert_eq!(doc.image(hecho.id), Some(&original), "undo exacto");
        assert!(!doc.can_undo(), "un solo paso");
        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(doc.image(hecho.id), Some(&esperada), "redo exacto");
    }

    #[test]
    fn fila_13_el_marco_blanco_son_220_pixeles_en_94_tramos() {
        // Ranura 1: negro.
        comprueba_relleno(1, (0, 0), 220, 94);
    }

    #[test]
    fn fila_13_el_agujero_transparente_son_64_pixeles_en_8_tramos() {
        // Ranura 3: rojo.
        comprueba_relleno(3, (30, 22), 64, 8);
    }

    #[test]
    fn fila_13_un_pixel_interior_es_su_propia_region() {
        // (10, 10) es de la diagonal semitransparente; ningún vecino de 4 vale lo mismo.
        comprueba_relleno(3, (10, 10), 1, 1);
    }

    #[test]
    fn fila_13_un_pixel_de_fuera_del_agujero_ni_del_marco_tambien_es_su_region() {
        // Fuera de la fila 13 de la tabla: otro píxel interior, sin diagonal, también vale distinto
        // que sus vecinos (`(10, 11)` del degradado).
        comprueba_relleno(3, (10, 11), 1, 1);
    }

    #[test]
    fn fila_14_rellenar_con_el_valor_que_ya_hay_no_da_paso() {
        let original = demo_image();
        // (30, 22) ya es transparente y la ranura 9 también; (0, 0) ya es blanco y la ranura 2
        // también.
        for (slot, x, y) in [(9, 30, 22), (2, 0, 0)] {
            assert_eq!(
                pixel_for_slot(&original, slot),
                original.get_pixel(x as u32, y as u32).ok(),
                "ranura {slot}: el valor ya es ese"
            );
            let hecho = clic(slot, x, y);
            assert!(!hecho.cambio, "ranura {slot} en ({x}, {y}): Ok(false)");
            assert!(!hecho.doc.can_undo(), "sin paso");
            assert!(!hecho.doc.can_redo());
            assert_eq!(hecho.doc.history_bytes(), 0);
            assert_eq!(hecho.doc.image(hecho.id), Some(&original));
        }
    }
}
