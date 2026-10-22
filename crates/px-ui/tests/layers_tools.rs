//! Las herramientas y el origen compuesto con capas (specs/hito-3-capas-vista.md, unidad 3.3):
//! filas 3, 4 y 5 (línea, relleno, borrador y cuentagotas sobre la celda activa), la parte lógica
//! de las filas 7, 8 y 9 (`DocSource`, `dirty_of` de los comandos de capa y los colores de la
//! superposición) y la propiedad 2 (la superposición es la composición con el trazo aplicado).
//!
//! Sin ventana ni egui: `ToolState` con eventos a mano y el oráculo de la composición
//! (`px_raster::compose` y `compose_rect`). Los oráculos no llaman a `visible_pixel`: la
//! implementación se compara con `compose` sobre una copia del documento a la que se aplica de
//! verdad el comando.
//!
//! Contrato de API que estas pruebas fijan (la lógica debe ofrecerlo):
//! - `px_ui::source::{PixelSource, DocSource, identity_cell, flatten, visible_picked,
//!   overlay_pixels}`; `overlay_pixels(doc, celda_activa, edits) -> Vec<(u32, u32, Rgba)>` gives, per
//!   cada edición, el color que se ve con el valor de la celda activa sustituido por el del trazo.
//! - `px_ui::tool::DropperSource { Visible, Layer }` y `ToolState::set_dropper_source`.
//! - `Document::canvas_image()` (px-core, C-01).

use std::borrow::Cow;

use proptest::prelude::*;
use px_core::{
    BlendMode, Command, Document, DocumentError, Image, ImageId, LayerId, Mode, Pixel, PixelEdit,
    Rgba,
};
use px_raster::{Rect, compose, compose_rect, composite_pixel, fill_command, flood_fill};
use px_ui::canvas::CanvasEvent;
use px_ui::dirty::{BlockSet, dirty_of};
use px_ui::pick::{EraseError, Picked, picked_at};
use px_ui::source::{
    DocSource, OverlayCache, PixelSource, flatten, identity_cell, overlay_pixels, visible_picked,
};
use px_ui::texture::pixel_rgba;
use px_ui::tiles::{SystemTileAlloc, TileRect};
use px_ui::tool::{DropperSource, Tool, ToolAction, ToolEvent, ToolState};
use px_ui::view::Size;

mod tiles_support;
use tiles_support::{expected_2, palette};

const RED: Rgba = Rgba::opaque(255, 0, 0);
const GREEN: Rgba = Rgba::opaque(0, 255, 0);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);
const TEST_BACKGROUND: Rgba = Rgba::opaque(10, 20, 30);

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

fn rgba_full(width_2: u32, height_2: u32, color: Rgba) -> Image {
    let mut image_2 = Image::new(width_2, height_2, Mode::Rgba).expect("imagen");
    for y in 0..height_2 {
        for x in 0..width_2 {
            image_2.set_pixel(x, y, Pixel::Rgba(color)).expect("dentro");
        }
    }
    image_2
}

/// Un documento RGBA de `width_2 × height_2` con la capa A opaca de color `TEST_BACKGROUND`.
fn doc_rgba(width_2: u32, height_2: u32) -> Document {
    Document::new(rgba_full(width_2, height_2, TEST_BACKGROUND))
}

/// Un documento indexado (paleta de `tiles_support`, transparente en la 7) con la capa A en el
/// índice 1.
fn doc_indexed(width_2: u32, height_2: u32) -> Document {
    let mut image_2 = Image::new_indexed(width_2, height_2, palette()).expect("imagen");
    for y in 0..height_2 {
        for x in 0..width_2 {
            image_2.set_pixel(x, y, Pixel::Indexed(1)).expect("dentro");
        }
    }
    Document::new(image_2)
}

/// Añade una capa encima (vacía) y devuelve su `LayerId` y la celda.
fn layer_new(doc: &mut Document, name_2: &str) -> (LayerId, ImageId) {
    let at = doc.layers().len();
    doc.apply(Command::AddLayer {
        name: name_2.to_string(),
        at,
    })
    .expect("añadir capa");
    let layer_2 = &doc.layers()[at];
    (layer_2.id(), layer_2.cel())
}

fn paints(doc: &mut Document, cel: ImageId, points: &[(u32, u32)], pixel: Pixel) {
    let pixels = points
        .iter()
        .map(|&(x, y)| PixelEdit { x, y, pixel })
        .collect();
    doc.apply(Command::SetPixels { image: cel, pixels })
        .expect("pintar");
}

fn rgba(color: Rgba) -> Pixel {
    Pixel::Rgba(color)
}

/// Un gesto completo (pulsar, mover y soltar) con `tool_2`: devuelve el resultado de soltar.
fn gesture_2(
    tools: &mut ToolState,
    doc: &mut Document,
    cel: ImageId,
    pixel: Option<Pixel>,
    tool_2: Tool,
    points: &[(i32, i32)],
) -> Result<bool, DocumentError> {
    tools.apply_action(ToolAction::Select(tool_2));
    let mut result =
        tools.canvas_event_with(doc, cel, pixel, CanvasEvent::StrokeBegin(points[0]))?;
    for &point in &points[1..] {
        result |= tools.canvas_event_with(doc, cel, pixel, CanvasEvent::StrokeMove(point))?;
    }
    Ok(result | tools.canvas_event_with(doc, cel, pixel, CanvasEvent::StrokeEnd)?)
}

fn pixel_de(doc: &Document, cel: ImageId, x: u32, y: u32) -> Pixel {
    doc.image(cel)
        .expect("la celda tiene imagen")
        .get_pixel(x, y)
        .expect("dentro")
}

/// El color que se ve en `(x, y)`: el píxel de `compose` pasado por `pixel_rgba`.
fn seen(doc: &Document, x: u32, y: u32) -> Rgba {
    let composite = compose(doc).expect("compose");
    let pixel = composite.get_pixel(x, y).expect("dentro");
    pixel_rgba(&composite, pixel).expect("color")
}

// ---------------------------------------------------------------------------------------------
// Fila 3: línea y relleno sobre una celda vacía
// ---------------------------------------------------------------------------------------------

#[test]
fn row_3_the_line_over_an_empty_cel_is_a_clipped_setpixels_without_materializing_first() {
    let mut doc = doc_rgba(8, 6);
    let (_, cel) = layer_new(&mut doc, "B");
    let before = doc.clone();
    let bytes = doc.materialized_bytes();
    let mut tools = ToolState::new();
    tools.apply_action(ToolAction::Select(Tool::Line));
    let pixel = Some(rgba(RED));
    tools
        .canvas_event_with(&mut doc, cel, pixel, CanvasEvent::StrokeBegin((1, 1)))
        .expect("pulsar");
    tools
        .canvas_event_with(&mut doc, cel, pixel, CanvasEvent::StrokeMove((20, 1)))
        .expect("mover");
    assert!(
        doc.image(cel).is_none(),
        "no se materializa antes de soltar"
    );
    assert_eq!(doc.materialized_bytes(), bytes);
    let change = tools
        .canvas_event_with(&mut doc, cel, pixel, CanvasEvent::StrokeEnd)
        .expect("soltar");
    assert!(change);
    for x in 0..8 {
        let expected_2 = if x >= 1 {
            rgba(RED)
        } else {
            rgba(Rgba::TRANSPARENT)
        };
        assert_eq!(pixel_de(&doc, cel, x, 1), expected_2, "x = {x}");
    }
    assert_eq!(pixel_de(&doc, cel, 0, 0), rgba(Rgba::TRANSPARENT));
    let changes_2 = tools.take_changes();
    assert_eq!(changes_2.len(), 1, "un solo paso anotado");
    assert!(
        !changes_2[0].blocks.is_empty(),
        "la región sucia no está vacía"
    );
    doc.undo().expect("deshacer");
    assert_eq!(doc, before, "un paso: deshacer deja la celda vacía");
}

#[test]
fn row_3_the_fill_over_an_empty_cel_covers_the_canvas_with_one_span_per_row() {
    let mut doc = doc_rgba(9, 5);
    let (_, cel) = layer_new(&mut doc, "B");
    let before = doc.clone();
    let mut tools = ToolState::new();
    let change = gesture_2(
        &mut tools,
        &mut doc,
        cel,
        Some(rgba(RED)),
        Tool::Fill,
        &[(3, 3)],
    )
    .expect("relleno");
    assert!(change);
    for y in 0..5 {
        for x in 0..9 {
            assert_eq!(pixel_de(&doc, cel, x, y), rgba(RED), "({x}, {y})");
        }
    }
    let changes_2 = tools.take_changes();
    assert_eq!(changes_2.len(), 1);
    assert_eq!(
        changes_2[0].blocks,
        BlockSet::full(9, 5),
        "todo el lienzo queda sucio"
    );
    doc.undo().expect("deshacer");
    assert_eq!(doc, before);
    assert!(doc.image(cel).is_none(), "deshacer la deja vacía otra vez");
}

#[test]
fn row_3_filling_an_empty_cel_with_the_transparent_changes_nothing() {
    let mut doc = doc_rgba(6, 4);
    let (_, cel) = layer_new(&mut doc, "B");
    let before = doc.clone();
    let mut tools = ToolState::new();
    let change = gesture_2(
        &mut tools,
        &mut doc,
        cel,
        Some(rgba(Rgba::TRANSPARENT)),
        Tool::Fill,
        &[(2, 2)],
    )
    .expect("relleno");
    assert!(!change);
    assert_eq!(doc, before);
    assert!(doc.image(cel).is_none());
    assert_eq!(doc.state_id(), before.state_id());
    assert!(tools.take_changes().is_empty());
}

#[test]
fn row_3_in_indexed_the_fill_of_an_empty_cel_uses_the_transparent_index_as_the_seed() {
    let mut doc = doc_indexed(6, 4);
    let (_, cel) = layer_new(&mut doc, "B");
    let mut tools = ToolState::new();
    // El índice transparente de la paleta de prueba es el 7: rellenar con él no cambia nada.
    let nothing = gesture_2(
        &mut tools,
        &mut doc,
        cel,
        Some(Pixel::Indexed(7)),
        Tool::Fill,
        &[(1, 1)],
    )
    .expect("relleno");
    assert!(!nothing);
    assert!(doc.image(cel).is_none());
    let change = gesture_2(
        &mut tools,
        &mut doc,
        cel,
        Some(Pixel::Indexed(2)),
        Tool::Fill,
        &[(1, 1)],
    )
    .expect("relleno");
    assert!(change);
    assert_eq!(pixel_de(&doc, cel, 5, 3), Pixel::Indexed(2));
}

#[test]
fn row_3_the_fill_over_a_cel_with_pixels_is_the_usual_one() {
    let mut doc = doc_rgba(8, 6);
    let cel_a = doc.layers()[0].cel();
    paints(&mut doc, cel_a, &[(2, 2), (3, 2)], rgba(RED));
    let mut expected_2 = doc.clone();
    flood_fill(&mut expected_2, cel_a, 5, 5, rgba(GREEN)).expect("oráculo");
    let mut tools = ToolState::new();
    gesture_2(
        &mut tools,
        &mut doc,
        cel_a,
        Some(rgba(GREEN)),
        Tool::Fill,
        &[(5, 5)],
    )
    .expect("relleno");
    assert_eq!(
        pixel_de(&doc, cel_a, 5, 5),
        pixel_de(&expected_2, cel_a, 5, 5)
    );
    assert_eq!(doc.image(cel_a), expected_2.image(cel_a));
}

#[test]
fn row_3_an_id_that_is_not_a_cel_is_still_unknown_image_in_raster_and_does_nothing_in_the_tool() {
    let mut doc = doc_rgba(4, 4);
    let no = ImageId::from_raw(999);
    assert!(matches!(
        fill_command(&doc, no, 1, 1, rgba(RED)),
        Err(DocumentError::UnknownImage(_))
    ));
    let before = doc.clone();
    let mut tools = ToolState::new();
    let change = gesture_2(
        &mut tools,
        &mut doc,
        no,
        Some(rgba(RED)),
        Tool::Fill,
        &[(1, 1)],
    )
    .expect("sin error");
    assert!(!change);
    assert_eq!(doc, before);
}

// ---------------------------------------------------------------------------------------------
// Fila 4: el borrador
// ---------------------------------------------------------------------------------------------

#[test]
fn row_4_the_eraser_on_an_empty_cel_does_nothing_or_materializes_it() {
    let mut doc = doc_rgba(6, 4);
    let (_, cel) = layer_new(&mut doc, "B");
    let before = doc.clone();
    let mut tools = ToolState::new();
    let change = gesture_2(
        &mut tools,
        &mut doc,
        cel,
        None,
        Tool::Eraser,
        &[(1, 1), (4, 1)],
    )
    .expect("borrador");
    assert!(!change);
    assert_eq!(doc, before);
    assert!(doc.image(cel).is_none());
    assert!(tools.take_changes().is_empty());
    assert!(
        tools.take_events().is_empty(),
        "sin aviso: el RGBA admite borrar"
    );
}

#[test]
fn row_4_the_eraser_only_touches_the_active_layer_and_lets_what_is_below_show() {
    let mut doc = doc_rgba(6, 4);
    let cel_a = doc.layers()[0].cel();
    let (_, cel_b) = layer_new(&mut doc, "B");
    paints(&mut doc, cel_b, &[(1, 1), (2, 1), (3, 1)], rgba(RED));
    let to_before = doc.image(cel_a).cloned();
    assert_eq!(seen(&doc, 2, 1), RED);
    let mut tools = ToolState::new();
    let change = gesture_2(
        &mut tools,
        &mut doc,
        cel_b,
        None,
        Tool::Eraser,
        &[(1, 1), (2, 1)],
    )
    .expect("borrador");
    assert!(change);
    assert_eq!(pixel_de(&doc, cel_b, 1, 1), rgba(Rgba::TRANSPARENT));
    assert_eq!(pixel_de(&doc, cel_b, 2, 1), rgba(Rgba::TRANSPARENT));
    assert_eq!(
        pixel_de(&doc, cel_b, 3, 1),
        rgba(RED),
        "fuera del trazo no se toca"
    );
    assert_eq!(
        doc.image(cel_a).cloned(),
        to_before,
        "la capa de debajo no cambia"
    );
    assert_eq!(seen(&doc, 2, 1), TEST_BACKGROUND, "se ve lo de debajo");
}

#[test]
fn row_4_in_indexed_the_eraser_writes_the_transparent_index_of_the_canvas() {
    let mut doc = doc_indexed(6, 4);
    let (_, cel_b) = layer_new(&mut doc, "B");
    paints(&mut doc, cel_b, &[(1, 1)], Pixel::Indexed(3));
    let mut tools = ToolState::new();
    gesture_2(&mut tools, &mut doc, cel_b, None, Tool::Eraser, &[(1, 1)]).expect("borrador");
    assert_eq!(pixel_de(&doc, cel_b, 1, 1), Pixel::Indexed(7));
}

#[test]
fn row_4_the_eraser_on_a_gray_warns_and_changes_nothing() {
    let mut image_2 = Image::new(4, 4, Mode::Gray).expect("imagen");
    image_2.set_pixel(1, 1, Pixel::Gray(200)).expect("dentro");
    let mut doc = Document::new(image_2);
    let cel = doc.layers()[0].cel();
    let before = doc.clone();
    let mut tools = ToolState::new();
    gesture_2(&mut tools, &mut doc, cel, None, Tool::Eraser, &[(1, 1)]).expect("borrador");
    assert_eq!(doc, before);
    assert_eq!(
        tools.take_events(),
        vec![ToolEvent::Refused(EraseError::NoTransparency)]
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 5: el cuentagotas
// ---------------------------------------------------------------------------------------------

/// Una A opaca, una B (activa) a opacidad 128 con un píxel rojo en (5, 5) y una C oculta con un
/// píxel azul en (6, 6). Devuelve el documento y las celdas de B y de C.
fn doc_of_dropper() -> (Document, ImageId, ImageId) {
    let mut doc = doc_rgba(10, 10);
    let (layer_b, cel_b) = layer_new(&mut doc, "B");
    let (layer_c, cel_c) = layer_new(&mut doc, "C");
    paints(&mut doc, cel_b, &[(5, 5)], rgba(RED));
    paints(&mut doc, cel_c, &[(6, 6)], rgba(BLUE));
    doc.apply(Command::SetLayerOpacity {
        layer: layer_b,
        opacity: 128,
    })
    .expect("opacidad");
    doc.apply(Command::SetLayerVisible {
        layer: layer_c,
        visible: false,
    })
    .expect("ocultar");
    (doc, cel_b, cel_c)
}

fn chooses(
    doc: &mut Document,
    cel: ImageId,
    source: Option<DropperSource>,
    point: (i32, i32),
) -> Vec<Picked> {
    let mut tools = ToolState::new();
    if let Some(source) = source {
        tools.set_dropper_source(source);
    }
    gesture_2(&mut tools, doc, cel, None, Tool::Dropper, &[point]).expect("cuentagotas");
    tools
        .take_events()
        .into_iter()
        .filter_map(|event| match event {
            ToolEvent::Picked(picked) => Some(picked),
            ToolEvent::Refused(_) => None,
        })
        .collect()
}

#[test]
fn row_5_the_dropper_chooses_what_is_visible_by_default() {
    let (mut doc, cel_b, _) = doc_of_dropper();
    let expected_2 = composite_pixel(TEST_BACKGROUND, RED, 128, BlendMode::Normal);
    assert_ne!(expected_2, RED, "la mezcla cambia el color");
    assert_eq!(
        chooses(&mut doc, cel_b, None, (5, 5)),
        vec![Picked::Color(expected_2)]
    );
    // La capa oculta no se ve.
    assert_eq!(
        chooses(&mut doc, cel_b, None, (6, 6)),
        vec![Picked::Color(TEST_BACKGROUND)]
    );
    // Sobre la celda vacía de la activa se ve lo de debajo, no «transparente».
    assert_eq!(
        chooses(&mut doc, cel_b, None, (0, 0)),
        vec![Picked::Color(TEST_BACKGROUND)]
    );
}

#[test]
fn row_5_the_layer_source_reads_the_active_cel_and_the_empty_one_is_transparent() {
    let (mut doc, cel_b, _) = doc_of_dropper();
    let layer_2 = Some(DropperSource::Layer);
    assert_eq!(
        chooses(&mut doc, cel_b, layer_2, (5, 5)),
        vec![Picked::Color(RED)]
    );
    assert_eq!(
        chooses(&mut doc, cel_b, layer_2, (0, 0)),
        vec![Picked::Color(Rgba::TRANSPARENT)]
    );
    let (mut empty, _, _) = doc_of_dropper();
    let (_, cel_d) = layer_new(&mut empty, "D");
    assert!(empty.image(cel_d).is_none());
    assert_eq!(
        chooses(&mut empty, cel_d, layer_2, (3, 3)),
        vec![Picked::Color(Rgba::TRANSPARENT)]
    );
    let before = empty.clone();
    assert_eq!(empty, before, "el cuentagotas no modifica el documento");
}

#[test]
fn row_5_with_one_visible_layer_the_two_sources_give_todays_including_alpha_zero() {
    let mut image_2 = rgba_full(4, 4, TEST_BACKGROUND);
    let hidden = Rgba::new(12, 34, 56, 0);
    image_2
        .set_pixel(2, 2, Pixel::Rgba(hidden))
        .expect("dentro");
    let mut doc = Document::new(image_2.clone());
    let cel = doc.layers()[0].cel();
    let of_today = picked_at(&image_2, 2, 2).expect("dentro");
    assert_eq!(of_today, Picked::Color(hidden));
    for source in [None, Some(DropperSource::Layer)] {
        assert_eq!(chooses(&mut doc, cel, source, (2, 2)), vec![of_today]);
    }
    assert!(
        px_ui::pick::pick_notice(&of_today).is_some(),
        "el aviso de «copiado tal cual»"
    );
}

#[test]
fn row_5_in_indexed_what_is_visible_is_the_entry_of_the_highest_non_transparent_visible_layer() {
    let mut doc = doc_indexed(6, 4);
    let (_, cel_b) = layer_new(&mut doc, "B");
    let (_, cel_c) = layer_new(&mut doc, "C");
    paints(&mut doc, cel_b, &[(1, 1)], Pixel::Indexed(3));
    paints(&mut doc, cel_c, &[(1, 1)], Pixel::Indexed(7));
    paints(&mut doc, cel_c, &[(2, 2)], Pixel::Indexed(5));
    assert_eq!(
        chooses(&mut doc, cel_c, None, (1, 1)),
        vec![Picked::Entry(3)]
    );
    assert_eq!(
        chooses(&mut doc, cel_c, None, (2, 2)),
        vec![Picked::Entry(5)]
    );
    assert_eq!(
        chooses(&mut doc, cel_c, None, (0, 0)),
        vec![Picked::Entry(1)]
    );
    let oracle = compose_rect(
        &doc,
        Rect {
            x: 2,
            y: 2,
            width: 1,
            height: 1,
        },
    )
    .expect("compose");
    assert_eq!(visible_picked(&doc, 2, 2), picked_at(&oracle, 0, 0));
}

// ---------------------------------------------------------------------------------------------
// Filas 7, 8 y 9 (parte lógica): el origen compuesto, la región sucia y la superposición
// ---------------------------------------------------------------------------------------------

/// Un documento de 3 capas con mezcla, opacidad y una oculta, en RGBA.
fn three_layer_doc() -> Document {
    let mut doc = doc_rgba(130, 70);
    let (layer_b, cel_b) = layer_new(&mut doc, "B");
    let (layer_c, cel_c) = layer_new(&mut doc, "C");
    let points: Vec<(u32, u32)> = (0..130).step_by(3).map(|x| (x, (x * 7) % 70)).collect();
    paints(&mut doc, cel_b, &points, rgba(Rgba::new(200, 100, 50, 200)));
    paints(&mut doc, cel_c, &points, rgba(BLUE));
    doc.apply(Command::SetLayerOpacity {
        layer: layer_b,
        opacity: 128,
    })
    .expect("opacidad");
    doc.apply(Command::SetLayerBlend {
        layer: layer_b,
        blend: BlendMode::Multiply,
    })
    .expect("mezcla");
    doc.apply(Command::SetLayerVisible {
        layer: layer_c,
        visible: false,
    })
    .expect("ocultar");
    doc
}

fn region_complete(source: &dyn PixelSource, width_2: u32, height_2: u32) -> Vec<egui::Color32> {
    source
        .region(
            TileRect {
                x: 0,
                y: 0,
                width: width_2,
                height: height_2,
            },
            None,
            &mut SystemTileAlloc,
        )
        .expect("región")
}

#[test]
fn row_7_the_composite_source_of_several_layers_is_compose() {
    let doc = three_layer_doc();
    assert!(identity_cell(&doc).is_none());
    let oracle = expected_2(&compose(&doc).expect("compose"));
    assert_eq!(region_complete(&DocSource(&doc), 130, 70), oracle);
    // Un recorte también es el recorte de la composición.
    let part = DocSource(&doc)
        .region(
            TileRect {
                x: 60,
                y: 10,
                width: 40,
                height: 30,
            },
            None,
            &mut SystemTileAlloc,
        )
        .expect("región");
    let mut expected = Vec::new();
    for y in 10..40usize {
        expected.extend_from_slice(&oracle[y * 130 + 60..y * 130 + 100]);
    }
    assert_eq!(part, expected);
}

#[test]
fn row_7_the_composite_source_of_an_indexed_one_is_compose_and_replaces_an_entry() {
    let mut doc = doc_indexed(20, 10);
    let (_, cel_b) = layer_new(&mut doc, "B");
    paints(&mut doc, cel_b, &[(3, 3), (4, 3)], Pixel::Indexed(5));
    let composite = compose(&doc).expect("compose");
    assert_eq!(
        region_complete(&DocSource(&doc), 20, 10),
        expected_2(&composite)
    );
    let color = Rgba::opaque(1, 2, 3);
    let with_input = DocSource(&doc)
        .region(
            TileRect {
                x: 0,
                y: 0,
                width: 20,
                height: 10,
            },
            Some((5, color)),
            &mut SystemTileAlloc,
        )
        .expect("región");
    assert_eq!(
        with_input[3 * 20 + 3],
        egui::Color32::from_rgba_unmultiplied(1, 2, 3, 255)
    );
    assert_eq!(DocSource(&doc).transparent_index(), Some(7));
}

#[test]
fn row_7_with_one_visible_opaque_normal_layer_the_cel_is_read_as_is() {
    let mut image_2 = rgba_full(8, 8, TEST_BACKGROUND);
    image_2
        .set_pixel(3, 3, Pixel::Rgba(Rgba::new(9, 8, 7, 0)))
        .expect("dentro");
    let doc = Document::new(image_2.clone());
    assert_eq!(identity_cell(&doc), Some(&image_2));
    assert_eq!(
        region_complete(&DocSource(&doc), 8, 8),
        expected_2(&image_2)
    );
    assert!(matches!(flatten(&doc).expect("aplanar"), Cow::Borrowed(_)));
    // Con la opacidad 254 ya no es la identidad.
    let mut doc = doc;
    let layer_2 = doc.layers()[0].id();
    doc.apply(Command::SetLayerOpacity {
        layer: layer_2,
        opacity: 254,
    })
    .expect("opacidad");
    assert!(identity_cell(&doc).is_none());
    assert!(matches!(flatten(&doc).expect("aplanar"), Cow::Owned(_)));
}

#[test]
fn row_11_flatten_with_several_layers_is_compose_and_a_gray_without_anything_visible_gives_an_error()
 {
    let doc = three_layer_doc();
    assert_eq!(
        *flatten(&doc).expect("aplanar"),
        compose(&doc).expect("compose")
    );
    let mut gray_2 = Document::new(Image::new(3, 3, Mode::Gray).expect("imagen"));
    let layer_2 = gray_2.layers()[0].id();
    gray_2
        .apply(Command::SetLayerVisible {
            layer: layer_2,
            visible: false,
        })
        .expect("ocultar");
    assert!(flatten(&gray_2).is_err());
}

#[test]
fn row_8_the_dirty_region_of_the_layer_commands() {
    let size = Size::new(130, 70);
    let all = BlockSet::full(130, 70);
    let doc = three_layer_doc();
    let layer_2 = doc.layers()[1].id();
    let nothing = |order: Command| assert!(dirty_of(&order, size).is_empty(), "{order:?}");
    let whole = |order: Command| assert_eq!(dirty_of(&order, size), all, "{order:?}");
    nothing(Command::AddLayer {
        name: "X".to_string(),
        at: 1,
    });
    nothing(Command::RenameLayer {
        layer: layer_2,
        name: "Y".to_string(),
    });
    whole(Command::SetLayerOpacity {
        layer: layer_2,
        opacity: 3,
    });
    whole(Command::SetLayerVisible {
        layer: layer_2,
        visible: false,
    });
    whole(Command::SetLayerBlend {
        layer: layer_2,
        blend: BlendMode::Screen,
    });
    whole(Command::MoveLayer {
        layer: layer_2,
        to: 0,
    });
    whole(Command::RemoveLayer { layer: layer_2 });
}

#[test]
fn row_9_the_overlay_hides_the_stroke_under_an_opaque_layer_and_mixes_it_at_128() {
    let mut doc = doc_rgba(8, 8);
    let (_, cel_b) = layer_new(&mut doc, "B");
    let (layer_c, cel_c) = layer_new(&mut doc, "C");
    paints(&mut doc, cel_c, &[(2, 2)], rgba(RED));
    let edits = [
        PixelEdit {
            x: 2,
            y: 2,
            pixel: rgba(GREEN),
        },
        PixelEdit {
            x: 3,
            y: 3,
            pixel: rgba(GREEN),
        },
    ];
    // C opaca encima en (2, 2): el trazo no se ve ahí; en (3, 3) no hay nada encima.
    let colors_2 = overlay_pixels(&doc, cel_b, edits);
    assert_eq!(colors_2, vec![(2, 2, RED), (3, 3, GREEN)]);
    // Con C a 128 se mezcla: es lo que dará compose tras aplicar el comando de verdad.
    doc.apply(Command::SetLayerOpacity {
        layer: layer_c,
        opacity: 128,
    })
    .expect("opacidad");
    let colors_2 = overlay_pixels(&doc, cel_b, edits);
    let mut applied = doc.clone();
    applied
        .apply(Command::SetPixels {
            image: cel_b,
            pixels: edits.to_vec(),
        })
        .expect("aplicar");
    assert_eq!(colors_2[0].2, seen(&applied, 2, 2));
    assert_eq!(colors_2[1].2, seen(&applied, 3, 3));
    assert_ne!(colors_2[0].2, RED);
}

#[test]
fn row_9_the_eraser_in_the_overlay_shows_what_is_below() {
    let mut doc = doc_rgba(8, 8);
    let (_, cel_b) = layer_new(&mut doc, "B");
    paints(&mut doc, cel_b, &[(1, 1)], rgba(RED));
    let delete = [PixelEdit {
        x: 1,
        y: 1,
        pixel: rgba(Rgba::TRANSPARENT),
    }];
    assert_eq!(
        overlay_pixels(&doc, cel_b, delete),
        vec![(1, 1, TEST_BACKGROUND)]
    );
}

#[test]
fn row_9_with_one_layer_the_overlay_is_todays_raw_color() {
    let doc = doc_rgba(8, 8);
    let cel = doc.layers()[0].cel();
    let semitransparent = Rgba::new(1, 2, 3, 0);
    let edits = [
        PixelEdit {
            x: 1,
            y: 1,
            pixel: rgba(RED),
        },
        PixelEdit {
            x: 2,
            y: 2,
            pixel: rgba(semitransparent),
        },
    ];
    assert_eq!(
        overlay_pixels(&doc, cel, edits),
        vec![(1, 1, RED), (2, 2, semitransparent)]
    );
}

#[test]
fn row_9_the_overlay_cache_only_computes_the_new_pixels() {
    let mut doc = doc_rgba(40, 10);
    let (_, cel_b) = layer_new(&mut doc, "B");
    let (_, cel_c) = layer_new(&mut doc, "C");
    paints(&mut doc, cel_c, &[(3, 1), (7, 1)], rgba(RED));
    let stroke_2: Vec<PixelEdit> = (0..30)
        .map(|x| PixelEdit {
            x,
            y: 1,
            pixel: rgba(GREEN),
        })
        .collect();
    let mut cache = OverlayCache::default();
    // El trazo crece: cada llamada calcula solo lo añadido y da lo que daría `overlay_pixels`.
    let mut seen_2 = 0;
    for until in [10, 20, 30] {
        let colors_2 = cache
            .colors(&doc, cel_b, stroke_2[..until].iter().copied())
            .to_vec();
        assert_eq!(
            colors_2,
            overlay_pixels(&doc, cel_b, stroke_2[..until].iter().copied())
        );
        assert_eq!(cache.last_computed(), until - seen_2);
        seen_2 = until;
    }
    // La misma lista otra vez: nada que calcular.
    cache.colors(&doc, cel_b, stroke_2.iter().copied());
    assert_eq!(cache.last_computed(), 0);
    // Una lista que no continúa la anterior (la goma elástica de la línea): se calcula entera.
    let other: Vec<PixelEdit> = stroke_2.iter().rev().take(12).copied().collect();
    let colors_2 = cache.colors(&doc, cel_b, other.iter().copied()).to_vec();
    assert_eq!(cache.last_computed(), 12);
    assert_eq!(colors_2, overlay_pixels(&doc, cel_b, other.iter().copied()));
    // Otro estado del documento o otra capa activa: todo de nuevo.
    cache.colors(&doc, cel_b, stroke_2.iter().copied());
    paints(&mut doc, cel_c, &[(9, 1)], rgba(BLUE));
    let colors_2 = cache.colors(&doc, cel_b, stroke_2.iter().copied()).to_vec();
    assert_eq!(cache.last_computed(), 30, "el documento cambió");
    assert_eq!(
        colors_2,
        overlay_pixels(&doc, cel_b, stroke_2.iter().copied())
    );
    cache.colors(&doc, cel_c, stroke_2.iter().copied());
    assert_eq!(cache.last_computed(), 30, "otra capa activa");
    // Vacío: nada, y el siguiente trazo empieza de cero.
    assert!(cache.colors(&doc, cel_c, std::iter::empty()).is_empty());
    cache.colors(&doc, cel_c, stroke_2[..5].iter().copied());
    assert_eq!(cache.last_computed(), 5);
}

// ---------------------------------------------------------------------------------------------
// Propiedad 2: la superposición no miente
// ---------------------------------------------------------------------------------------------

type Layers = Vec<(u8, u8, u8, u8, u32, u32)>;
type TestStroke = Vec<(u32, u32, u32)>;

fn seed_of_layers() -> impl Strategy<Value = (u8, Layers, u8, TestStroke)> {
    (
        0u8..3,
        prop::collection::vec(
            (0u8..4, 0u8..8, 0u8..3, 0u8..4, any::<u32>(), any::<u32>()),
            0..4,
        ),
        0u8..4,
        prop::collection::vec((0u32..12, 0u32..9, any::<u32>()), 1..12),
    )
}

/// Un píxel del modo `mode_2` según `k` (con alfas 0, 1, 128 y 255; índices dentro de la paleta).
fn pixel_for(mode_2: Mode, k: u32) -> Pixel {
    tiles_support::pixel_de(mode_2, k, palette().len())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para cualquier documento (RGBA, gris con alfa o indexado; de 1 a 4 capas con opacidad,
    /// mezcla, visibilidad y celdas vacías aleatorias), capa activa y trazo (con el valor de
    /// borrado incluido): el color de cada píxel de la superposición es el píxel de `compose` del
    /// documento tras aplicar de verdad el comando del trazo.
    #[test]
    fn property_2_the_overlay_is_the_composition_with_the_stroke_applied(
        (mode_2, layers, active_2, stroke_2) in seed_of_layers(),
    ) {
        let mode_2 = [Mode::Rgba, Mode::GrayAlpha, Mode::Indexed][usize::from(mode_2)];
        let base = match mode_2 {
            Mode::Indexed => tiles_support::image_of(mode_2, 12, 9, 3),
            _ => tiles_support::image_of(mode_2, 12, 9, 2),
        };
        let mut doc = Document::new(base);
        for (i, &(layer_2, opacity_2, blend_2, visible, a, b)) in layers.iter().enumerate() {
            let at = doc.layers().len();
            if doc.apply(Command::AddLayer { name: format!("C{i}"), at }).is_err() {
                continue;
            }
            let id = doc.layers()[at].id();
            let cel = doc.layers()[at].cel();
            if layer_2 != 0 {
                let pixels: Vec<PixelEdit> = (0..1 + b % 20)
                    .map(|j| PixelEdit {
                        x: a.wrapping_add(j.wrapping_mul(5)) % 12,
                        y: b.wrapping_add(j.wrapping_mul(3)) % 9,
                        pixel: pixel_for(mode_2, a.wrapping_add(j)),
                    })
                    .collect();
                let _ = doc.apply(Command::SetPixels { image: cel, pixels });
            }
            let _ = doc.apply(Command::SetLayerOpacity { layer: id, opacity: [255, 0, 1, 128, 200, 254, 255, 77][usize::from(opacity_2)] });
            let _ = doc.apply(Command::SetLayerBlend { layer: id, blend: BlendMode::ALL[usize::from(blend_2) * 3 % 8] });
            let _ = doc.apply(Command::SetLayerVisible { layer: id, visible: visible != 0 });
        }
        let layer_active = &doc.layers()[usize::from(active_2) % doc.layers().len()];
        let cel = layer_active.cel();
        let edits: Vec<PixelEdit> = stroke_2
            .iter()
            .map(|&(x, y, k)| PixelEdit { x, y, pixel: pixel_for(mode_2, k) })
            .collect();
        // Sin repetidos: el último valor de cada coordenada es el que se aplica.
        let mut unique: Vec<PixelEdit> = Vec::new();
        for edit in edits {
            match unique.iter_mut().find(|e| (e.x, e.y) == (edit.x, edit.y)) {
                Some(e) => e.pixel = edit.pixel,
                None => unique.push(edit),
            }
        }
        let colors_2 = overlay_pixels(&doc, cel, unique.clone());
        prop_assert_eq!(colors_2.len(), unique.len());
        let mut applied = doc.clone();
        let applicable = applied
            .apply(Command::SetPixels { image: cel, pixels: unique.clone() })
            .is_ok();
        if applicable && let Ok(composite) = compose(&applied) {
            for (x, y, color) in colors_2 {
                let pixel = composite.get_pixel(x, y).unwrap();
                prop_assert_eq!(Some(color), pixel_rgba(&composite, pixel), "({}, {})", x, y);
            }
        }
    }
}
