//! Documento de muestra que la ventana muestra mientras no se puedan abrir ficheros
//! (abrir PNG es la unidad 15 del hito 2).

use px_core::{Image, Mode, Pixel, Rgba};
use px_ui::color_edit::hex_text;
use px_ui::dialog::{FieldKind, FieldSpec, FormSpec, Reaction, Rows, ShowWhen, Value};

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

/// Variable de entorno que abre el diálogo de prueba al arrancar (`PIXOKO_TEST_DIALOG=1`).
pub const ENV_DEMO_DIALOG: &str = "PIXOKO_TEST_DIALOG";

/// Si la variable pide el diálogo de prueba: solo el valor `1`.
pub fn demo_dialog_enabled(value: Option<&str>) -> bool {
    value == Some("1")
}

/// El formulario del diálogo de prueba (specs/hito-2-dialogos-modales.md, decisión 10): dos
/// numéricos, un grupo de cuatro opciones, una casilla, un hexadecimal que solo se muestra con la
/// última opción y un informe. No toca el documento: sirve para probar las piezas a mano.
pub fn demo_form() -> FormSpec {
    FormSpec {
        title: "Diálogo de prueba",
        accept: "Aceptar",
        cancel: Some("Cancelar"),
        fields: vec![
            FieldSpec {
                id: "ancho",
                label: "Ancho",
                kind: FieldKind::Number {
                    min: 1,
                    max: 8192,
                    default: 64,
                },
                shown_when: None,
            },
            FieldSpec {
                id: "limite",
                label: "Límite",
                kind: FieldKind::Number {
                    min: 0,
                    max: u32::MAX,
                    default: 1000,
                },
                shown_when: None,
            },
            FieldSpec {
                id: "modo",
                label: "Modo",
                kind: FieldKind::Choice {
                    options: &["Uno", "Dos", "Tres", "Con color"],
                    default: 0,
                },
                shown_when: None,
            },
            FieldSpec {
                id: "color",
                label: "Color",
                kind: FieldKind::Hex {
                    mode: Mode::Rgba,
                    default: Rgba::opaque(0x33, 0x66, 0x99),
                },
                shown_when: Some(ShowWhen {
                    field: "modo",
                    choice: 3,
                }),
            },
            FieldSpec {
                id: "opaco",
                label: "Opaco",
                kind: FieldKind::Check { default: false },
                shown_when: None,
            },
            FieldSpec {
                id: "informe",
                label: "Informe",
                kind: FieldKind::ReadOnly {
                    text: "Diálogo de prueba de las piezas de la unidad 18. No modifica el \
                           documento ni escribe ningún fichero: al cerrarlo, la barra de \
                           estado dice lo que se aceptó o que se canceló."
                        .to_string(),
                    rows: Rows::Fixed(4),
                },
                shown_when: None,
            },
        ],
        check: None,
    }
}

/// El aviso de la barra al cerrar el diálogo de prueba: lo aceptado (rótulo y valor de cada campo
/// visible), «cancelado» o el motivo de un rechazo. Pura: no recibe la sesión, así que no puede
/// tocar el documento.
pub fn demo_notice(reaction: &Reaction) -> String {
    match reaction {
        Reaction::Cancelled => "diálogo de prueba cancelado".to_string(),
        Reaction::Rejected(reason) => format!("diálogo de prueba sin aceptar: {reason}"),
        Reaction::None | Reaction::Pressed { .. } => "diálogo de prueba abierto".to_string(),
        Reaction::Accepted(values) => {
            let spec = demo_form();
            let parts: Vec<String> = spec
                .fields
                .iter()
                .filter_map(|field| {
                    let value = values.get(field.id)?;
                    let text = match (value, &field.kind) {
                        (Value::Number(n), _) => n.to_string(),
                        (Value::Choice(i), FieldKind::Choice { options, .. }) => {
                            options.get(*i).copied().unwrap_or("?").to_string()
                        }
                        (Value::Check(on), _) => if *on { "sí" } else { "no" }.to_string(),
                        (Value::Color(color), _) => hex_text(*color),
                        _ => return None,
                    };
                    Some(format!("{} {text}", field.label))
                })
                .collect();
            format!("diálogo de prueba aceptado: {}", parts.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_image_has_the_size_and_the_expected_cases() {
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

    use px_core::ImageId;
    use std::collections::{BTreeSet, VecDeque};

    use px_core::{Command, Document, Span};
    use px_ui::canvas::CanvasEvent;
    use px_ui::color::pixel_for_slot;
    use px_ui::tool::{Tool, ToolAction, ToolState};

    use super::*;

    /// Recorrido ingenuo pixel a pixel con cola: 4 vecinos y valor exacto (`get_pixel`).
    fn region_naive(img: &Image, sx: u32, sy: u32) -> BTreeSet<(u32, u32)> {
        let valor = img.get_pixel(sx, sy).expect("dentro");
        let mut region = BTreeSet::new();
        let mut queue = VecDeque::new();
        region.insert((sx, sy));
        queue.push_back((sx, sy));
        while let Some((x, y)) = queue.pop_front() {
            let (xi, yi) = (i64::from(x), i64::from(y));
            for (nx, ny) in [(xi - 1, yi), (xi + 1, yi), (xi, yi - 1), (xi, yi + 1)] {
                let (Ok(nx), Ok(ny)) = (u32::try_from(nx), u32::try_from(ny)) else {
                    continue;
                };
                if !region.contains(&(nx, ny)) && img.get_pixel(nx, ny) == Ok(valor) {
                    region.insert((nx, ny));
                    queue.push_back((nx, ny));
                }
            }
        }
        region
    }

    /// Rachas máximas de la región en cada fila, ordenadas por `(y, x_start)`.
    fn spans_of(region: &BTreeSet<(u32, u32)>) -> Vec<Span> {
        let mut spans_2: Vec<Span> = Vec::new();
        // `BTreeSet` de `(x, y)` ordena por `x` primero: se pasa a `(y, x)` para recorrer por filas.
        let per_rows: BTreeSet<(u32, u32)> = region.iter().map(|&(x, y)| (y, x)).collect();
        for (y, x) in per_rows {
            match spans_2.last_mut() {
                Some(t) if t.y == y && t.x_end == x => t.x_end = x + 1,
                _ => spans_2.push(Span {
                    y,
                    x_start: x,
                    x_end: x + 1,
                }),
            }
        }
        spans_2
    }

    /// Cuántas veces ha dado `Ok(true)` un clic de relleno y con qué documento queda.
    struct Click {
        doc: Document,
        id: px_core::ImageId,
        change: bool,
    }

    /// Un clic de relleno con la ranura `slot` en `(x, y)` sobre una imagen de muestra nueva:
    /// `Begin`, un `Move` y `End`, con la herramienta elegida con `Select(Fill)`.
    fn click(slot: u8, x: i32, y: i32) -> Click {
        let mut doc = Document::new(demo_image());
        let id = ImageId::from_raw(0);
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(Tool::Fill));
        assert_eq!(tool.tool(), Tool::Fill);
        let change = tool
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
        Click { doc, id, change }
    }

    /// Fila 13: el resultado de la herramienta es el del oráculo ingenuo, con las cifras a mano.
    fn checks_fill(slot: u8, (x, y): (u32, u32), pixels_2: usize, spans_2: usize) {
        let original = demo_image();
        let pixel = pixel_for_slot(&original, slot).expect("la ranura tiene color");
        let region = region_naive(&original, x, y);
        let of_the_oracle = spans_of(&region);
        assert_eq!(region.len(), pixels_2, "píxeles de la región (a mano)");
        assert_eq!(of_the_oracle.len(), spans_2, "tramos de la región (a mano)");

        let done = click(slot, x as i32, y as i32);
        assert!(done.change, "Ok(true) en el Begin");

        // Igual al oráculo: la original con la región en `pixel` y nada más.
        let mut expected = original.clone();
        for &(rx, ry) in &region {
            expected.set_pixel(rx, ry, pixel).expect("dentro");
        }
        let result = done.doc.image(done.id).expect("hay imagen");
        assert_eq!(result, &expected);
        let changed = (0..HEIGHT)
            .flat_map(|py| (0..WIDTH).map(move |px| (px, py)))
            .filter(|&(px, py)| result.get_pixel(px, py) != original.get_pixel(px, py))
            .count();
        assert_eq!(changed, pixels_2, "solo cambia la región");

        // Un solo `FillSpans` con los tramos del oráculo: mismo peso de historia.
        let mut reference = Document::new(original.clone());
        let rid = ImageId::from_raw(0);
        let step = Command::FillSpans {
            image: rid,
            spans: of_the_oracle,
            pixel,
        };
        assert_eq!(reference.apply(step), Ok(true));
        assert_eq!(done.doc.history_bytes(), reference.history_bytes());
        assert_eq!(result, reference.image(rid).expect("hay imagen"));

        // Un paso y `undo` exacto.
        let mut doc = done.doc;
        assert!(doc.can_undo());
        assert_eq!(doc.undo(), Ok(()));
        assert_eq!(doc.image(done.id), Some(&original), "undo exacto");
        assert!(!doc.can_undo(), "un solo paso");
        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(doc.image(done.id), Some(&expected), "redo exacto");
    }

    #[test]
    fn row_13_the_white_frame_is_220_pixels_in_94_spans() {
        // Ranura 1: negro.
        checks_fill(1, (0, 0), 220, 94);
    }

    #[test]
    fn row_13_the_transparent_hole_is_64_pixels_in_8_spans() {
        // Ranura 3: rojo.
        checks_fill(3, (30, 22), 64, 8);
    }

    #[test]
    fn row_13_an_interior_pixel_is_its_own_region() {
        // (10, 10) es de la diagonal semitransparente; ningún vecino de 4 vale lo mismo.
        checks_fill(3, (10, 10), 1, 1);
    }

    #[test]
    fn row_13_a_pixel_outside_the_hole_and_the_frame_is_also_its_region() {
        // Fuera de la fila 13 de la tabla: otro píxel interior, sin diagonal, también vale distinto
        // que sus vecinos (`(10, 11)` del degradado).
        checks_fill(3, (10, 11), 1, 1);
    }

    #[test]
    fn row_14_filling_with_the_value_already_there_gives_no_step() {
        let original = demo_image();
        // (30, 22) ya es transparente y la ranura 9 también; (0, 0) ya es blanco y la ranura 2
        // también.
        for (slot, x, y) in [(9, 30, 22), (2, 0, 0)] {
            assert_eq!(
                pixel_for_slot(&original, slot),
                original.get_pixel(x as u32, y as u32).ok(),
                "ranura {slot}: el valor ya es ese"
            );
            let done = click(slot, x, y);
            assert!(!done.change, "ranura {slot} en ({x}, {y}): Ok(false)");
            assert!(!done.doc.can_undo(), "sin paso");
            assert!(!done.doc.can_redo());
            assert_eq!(done.doc.history_bytes(), 0);
            assert_eq!(done.doc.image(done.id), Some(&original));
        }
    }
}

/// Pruebas del diálogo de prueba (`demo_form` y `demo_notice`, fila 12 de
/// specs/hito-2-dialogos-modales.md). Con `#[path]` porque `mod tests` ya es el módulo en línea de
/// arriba.
#[cfg(test)]
#[path = "demo/tests.rs"]
mod form_tests;
