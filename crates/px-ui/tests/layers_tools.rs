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
//!   overlay_pixels}`; `overlay_pixels(doc, celda_activa, edits) -> Vec<(u32, u32, Rgba)>` da, por
//!   cada edición, el color que se ve con el valor de la celda activa sustituido por el del trazo.
//! - `px_ui::tool::DropperSource { Visible, Layer }` y `ToolState::set_dropper_source`.
//! - `Document::canvas_image()` (px-core, C-01).

use std::borrow::Cow;

use proptest::prelude::*;
use px_core::{
    BlendMode, Command, Document, DocumentError, Image, ImageId, LayerId, Mode, Palette, Pixel,
    PixelEdit, Rgba, Span,
};
use px_raster::{Rect, compose, compose_rect, composite_pixel, fill_command, flood_fill};
use px_ui::canvas::CanvasEvent;
use px_ui::dirty::{BlockSet, dirty_of};
use px_ui::pick::{EraseError, Picked, picked_at};
use px_ui::source::{
    DocSource, PixelSource, flatten, identity_cell, overlay_pixels, visible_picked,
};
use px_ui::texture::pixel_rgba;
use px_ui::tiles::{SystemTileAlloc, TileRect};
use px_ui::tool::{DropperSource, Tool, ToolAction, ToolEvent, ToolState};
use px_ui::view::Size;

mod tiles_support;
use tiles_support::{esperado, paleta};

const ROJO: Rgba = Rgba::opaque(255, 0, 0);
const VERDE: Rgba = Rgba::opaque(0, 255, 0);
const AZUL: Rgba = Rgba::opaque(0, 0, 255);
const FONDO: Rgba = Rgba::opaque(10, 20, 30);

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

fn rgba_lleno(ancho: u32, alto: u32, color: Rgba) -> Image {
    let mut imagen = Image::new(ancho, alto, Mode::Rgba).expect("imagen");
    for y in 0..alto {
        for x in 0..ancho {
            imagen.set_pixel(x, y, Pixel::Rgba(color)).expect("dentro");
        }
    }
    imagen
}

/// Un documento RGBA de `ancho × alto` con la capa A opaca de color `FONDO`.
fn doc_rgba(ancho: u32, alto: u32) -> Document {
    Document::new(rgba_lleno(ancho, alto, FONDO))
}

/// Un documento indexado (paleta de `tiles_support`, transparente en la 7) con la capa A en el
/// índice 1.
fn doc_indexado(ancho: u32, alto: u32) -> Document {
    let mut imagen = Image::new_indexed(ancho, alto, paleta()).expect("imagen");
    for y in 0..alto {
        for x in 0..ancho {
            imagen.set_pixel(x, y, Pixel::Indexed(1)).expect("dentro");
        }
    }
    Document::new(imagen)
}

/// Añade una capa encima (vacía) y devuelve su `LayerId` y la celda.
fn capa_nueva(doc: &mut Document, nombre: &str) -> (LayerId, ImageId) {
    let at = doc.layers().len();
    doc.apply(Command::AddLayer {
        name: nombre.to_string(),
        at,
    })
    .expect("añadir capa");
    let capa = &doc.layers()[at];
    (capa.id(), capa.cel())
}

fn pinta(doc: &mut Document, celda: ImageId, puntos: &[(u32, u32)], pixel: Pixel) {
    let pixels = puntos
        .iter()
        .map(|&(x, y)| PixelEdit { x, y, pixel })
        .collect();
    doc.apply(Command::SetPixels {
        image: celda,
        pixels,
    })
    .expect("pintar");
}

fn rgba(color: Rgba) -> Pixel {
    Pixel::Rgba(color)
}

/// Un gesto completo (pulsar, mover y soltar) con `herramienta`: devuelve el resultado de soltar.
fn gesto(
    tools: &mut ToolState,
    doc: &mut Document,
    celda: ImageId,
    pixel: Option<Pixel>,
    herramienta: Tool,
    puntos: &[(i32, i32)],
) -> Result<bool, DocumentError> {
    tools.apply_action(ToolAction::Select(herramienta));
    let mut resultado =
        tools.canvas_event_with(doc, celda, pixel, CanvasEvent::StrokeBegin(puntos[0]))?;
    for &punto in &puntos[1..] {
        resultado |= tools.canvas_event_with(doc, celda, pixel, CanvasEvent::StrokeMove(punto))?;
    }
    Ok(resultado | tools.canvas_event_with(doc, celda, pixel, CanvasEvent::StrokeEnd)?)
}

fn pixel_de(doc: &Document, celda: ImageId, x: u32, y: u32) -> Pixel {
    doc.image(celda)
        .expect("la celda tiene imagen")
        .get_pixel(x, y)
        .expect("dentro")
}

/// El color que se ve en `(x, y)`: el píxel de `compose` pasado por `pixel_rgba`.
fn visto(doc: &Document, x: u32, y: u32) -> Rgba {
    let compuesta = compose(doc).expect("compose");
    let pixel = compuesta.get_pixel(x, y).expect("dentro");
    pixel_rgba(&compuesta, pixel).expect("color")
}

// ---------------------------------------------------------------------------------------------
// Fila 3: línea y relleno sobre una celda vacía
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_3_la_linea_sobre_celda_vacia_es_un_setpixels_recortado_sin_materializar_antes() {
    let mut doc = doc_rgba(8, 6);
    let (_, celda) = capa_nueva(&mut doc, "B");
    let antes = doc.clone();
    let bytes = doc.materialized_bytes();
    let mut tools = ToolState::new();
    tools.apply_action(ToolAction::Select(Tool::Line));
    let pixel = Some(rgba(ROJO));
    tools
        .canvas_event_with(&mut doc, celda, pixel, CanvasEvent::StrokeBegin((1, 1)))
        .expect("pulsar");
    tools
        .canvas_event_with(&mut doc, celda, pixel, CanvasEvent::StrokeMove((20, 1)))
        .expect("mover");
    assert!(
        doc.image(celda).is_none(),
        "no se materializa antes de soltar"
    );
    assert_eq!(doc.materialized_bytes(), bytes);
    let cambio = tools
        .canvas_event_with(&mut doc, celda, pixel, CanvasEvent::StrokeEnd)
        .expect("soltar");
    assert!(cambio);
    for x in 0..8 {
        let esperado = if x >= 1 {
            rgba(ROJO)
        } else {
            rgba(Rgba::TRANSPARENT)
        };
        assert_eq!(pixel_de(&doc, celda, x, 1), esperado, "x = {x}");
    }
    assert_eq!(pixel_de(&doc, celda, 0, 0), rgba(Rgba::TRANSPARENT));
    let cambios = tools.take_changes();
    assert_eq!(cambios.len(), 1, "un solo paso anotado");
    assert!(
        !cambios[0].blocks.is_empty(),
        "la región sucia no está vacía"
    );
    doc.undo().expect("deshacer");
    assert_eq!(doc, antes, "un paso: deshacer deja la celda vacía");
}

#[test]
fn fila_3_el_relleno_sobre_celda_vacia_cubre_el_lienzo_con_un_tramo_por_fila() {
    let mut doc = doc_rgba(9, 5);
    let (_, celda) = capa_nueva(&mut doc, "B");
    let antes = doc.clone();
    let mut tools = ToolState::new();
    let cambio = gesto(
        &mut tools,
        &mut doc,
        celda,
        Some(rgba(ROJO)),
        Tool::Fill,
        &[(3, 3)],
    )
    .expect("relleno");
    assert!(cambio);
    for y in 0..5 {
        for x in 0..9 {
            assert_eq!(pixel_de(&doc, celda, x, y), rgba(ROJO), "({x}, {y})");
        }
    }
    let cambios = tools.take_changes();
    assert_eq!(cambios.len(), 1);
    assert_eq!(
        cambios[0].blocks,
        BlockSet::full(9, 5),
        "todo el lienzo queda sucio"
    );
    doc.undo().expect("deshacer");
    assert_eq!(doc, antes);
    assert!(
        doc.image(celda).is_none(),
        "deshacer la deja vacía otra vez"
    );
}

#[test]
fn fila_3_rellenar_una_celda_vacia_con_el_transparente_no_cambia_nada() {
    let mut doc = doc_rgba(6, 4);
    let (_, celda) = capa_nueva(&mut doc, "B");
    let antes = doc.clone();
    let mut tools = ToolState::new();
    let cambio = gesto(
        &mut tools,
        &mut doc,
        celda,
        Some(rgba(Rgba::TRANSPARENT)),
        Tool::Fill,
        &[(2, 2)],
    )
    .expect("relleno");
    assert!(!cambio);
    assert_eq!(doc, antes);
    assert!(doc.image(celda).is_none());
    assert_eq!(doc.state_id(), antes.state_id());
    assert!(tools.take_changes().is_empty());
}

#[test]
fn fila_3_en_indexado_el_relleno_de_una_celda_vacia_usa_el_indice_transparente_como_semilla() {
    let mut doc = doc_indexado(6, 4);
    let (_, celda) = capa_nueva(&mut doc, "B");
    let mut tools = ToolState::new();
    // El índice transparente de la paleta de prueba es el 7: rellenar con él no cambia nada.
    let nada = gesto(
        &mut tools,
        &mut doc,
        celda,
        Some(Pixel::Indexed(7)),
        Tool::Fill,
        &[(1, 1)],
    )
    .expect("relleno");
    assert!(!nada);
    assert!(doc.image(celda).is_none());
    let cambio = gesto(
        &mut tools,
        &mut doc,
        celda,
        Some(Pixel::Indexed(2)),
        Tool::Fill,
        &[(1, 1)],
    )
    .expect("relleno");
    assert!(cambio);
    assert_eq!(pixel_de(&doc, celda, 5, 3), Pixel::Indexed(2));
}

#[test]
fn fila_3_el_relleno_sobre_una_celda_con_pixeles_es_el_de_siempre() {
    let mut doc = doc_rgba(8, 6);
    let celda_a = doc.layers()[0].cel();
    pinta(&mut doc, celda_a, &[(2, 2), (3, 2)], rgba(ROJO));
    let mut esperado = doc.clone();
    flood_fill(&mut esperado, celda_a, 5, 5, rgba(VERDE)).expect("oráculo");
    let mut tools = ToolState::new();
    gesto(
        &mut tools,
        &mut doc,
        celda_a,
        Some(rgba(VERDE)),
        Tool::Fill,
        &[(5, 5)],
    )
    .expect("relleno");
    assert_eq!(
        pixel_de(&doc, celda_a, 5, 5),
        pixel_de(&esperado, celda_a, 5, 5)
    );
    assert_eq!(doc.image(celda_a), esperado.image(celda_a));
}

#[test]
fn fila_3_un_id_que_no_es_celda_sigue_siendo_unknown_image_en_raster_y_no_hace_nada_en_la_herramienta()
 {
    let mut doc = doc_rgba(4, 4);
    let ninguna = ImageId::from_raw(999);
    assert!(matches!(
        fill_command(&doc, ninguna, 1, 1, rgba(ROJO)),
        Err(DocumentError::UnknownImage(_))
    ));
    let antes = doc.clone();
    let mut tools = ToolState::new();
    let cambio = gesto(
        &mut tools,
        &mut doc,
        ninguna,
        Some(rgba(ROJO)),
        Tool::Fill,
        &[(1, 1)],
    )
    .expect("sin error");
    assert!(!cambio);
    assert_eq!(doc, antes);
}

// ---------------------------------------------------------------------------------------------
// Fila 4: el borrador
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_4_el_borrador_sobre_una_celda_vacia_no_hace_nada_ni_la_materializa() {
    let mut doc = doc_rgba(6, 4);
    let (_, celda) = capa_nueva(&mut doc, "B");
    let antes = doc.clone();
    let mut tools = ToolState::new();
    let cambio = gesto(
        &mut tools,
        &mut doc,
        celda,
        None,
        Tool::Eraser,
        &[(1, 1), (4, 1)],
    )
    .expect("borrador");
    assert!(!cambio);
    assert_eq!(doc, antes);
    assert!(doc.image(celda).is_none());
    assert!(tools.take_changes().is_empty());
    assert!(
        tools.take_events().is_empty(),
        "sin aviso: el RGBA admite borrar"
    );
}

#[test]
fn fila_4_el_borrador_solo_toca_la_capa_activa_y_deja_ver_lo_de_debajo() {
    let mut doc = doc_rgba(6, 4);
    let celda_a = doc.layers()[0].cel();
    let (_, celda_b) = capa_nueva(&mut doc, "B");
    pinta(&mut doc, celda_b, &[(1, 1), (2, 1), (3, 1)], rgba(ROJO));
    let a_antes = doc.image(celda_a).cloned();
    assert_eq!(visto(&doc, 2, 1), ROJO);
    let mut tools = ToolState::new();
    let cambio = gesto(
        &mut tools,
        &mut doc,
        celda_b,
        None,
        Tool::Eraser,
        &[(1, 1), (2, 1)],
    )
    .expect("borrador");
    assert!(cambio);
    assert_eq!(pixel_de(&doc, celda_b, 1, 1), rgba(Rgba::TRANSPARENT));
    assert_eq!(pixel_de(&doc, celda_b, 2, 1), rgba(Rgba::TRANSPARENT));
    assert_eq!(
        pixel_de(&doc, celda_b, 3, 1),
        rgba(ROJO),
        "fuera del trazo no se toca"
    );
    assert_eq!(
        doc.image(celda_a).cloned(),
        a_antes,
        "la capa de debajo no cambia"
    );
    assert_eq!(visto(&doc, 2, 1), FONDO, "se ve lo de debajo");
}

#[test]
fn fila_4_en_indexado_el_borrador_escribe_el_indice_transparente_del_lienzo() {
    let mut doc = doc_indexado(6, 4);
    let (_, celda_b) = capa_nueva(&mut doc, "B");
    pinta(&mut doc, celda_b, &[(1, 1)], Pixel::Indexed(3));
    let mut tools = ToolState::new();
    gesto(&mut tools, &mut doc, celda_b, None, Tool::Eraser, &[(1, 1)]).expect("borrador");
    assert_eq!(pixel_de(&doc, celda_b, 1, 1), Pixel::Indexed(7));
}

#[test]
fn fila_4_el_borrador_de_un_gris_avisa_y_no_cambia_nada() {
    let mut imagen = Image::new(4, 4, Mode::Gray).expect("imagen");
    imagen.set_pixel(1, 1, Pixel::Gray(200)).expect("dentro");
    let mut doc = Document::new(imagen);
    let celda = doc.layers()[0].cel();
    let antes = doc.clone();
    let mut tools = ToolState::new();
    gesto(&mut tools, &mut doc, celda, None, Tool::Eraser, &[(1, 1)]).expect("borrador");
    assert_eq!(doc, antes);
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
fn doc_de_cuentagotas() -> (Document, ImageId, ImageId) {
    let mut doc = doc_rgba(10, 10);
    let (capa_b, celda_b) = capa_nueva(&mut doc, "B");
    let (capa_c, celda_c) = capa_nueva(&mut doc, "C");
    pinta(&mut doc, celda_b, &[(5, 5)], rgba(ROJO));
    pinta(&mut doc, celda_c, &[(6, 6)], rgba(AZUL));
    doc.apply(Command::SetLayerOpacity {
        layer: capa_b,
        opacity: 128,
    })
    .expect("opacidad");
    doc.apply(Command::SetLayerVisible {
        layer: capa_c,
        visible: false,
    })
    .expect("ocultar");
    (doc, celda_b, celda_c)
}

fn elige(
    doc: &mut Document,
    celda: ImageId,
    fuente: Option<DropperSource>,
    punto: (i32, i32),
) -> Vec<Picked> {
    let mut tools = ToolState::new();
    if let Some(fuente) = fuente {
        tools.set_dropper_source(fuente);
    }
    gesto(&mut tools, doc, celda, None, Tool::Dropper, &[punto]).expect("cuentagotas");
    tools
        .take_events()
        .into_iter()
        .filter_map(|evento| match evento {
            ToolEvent::Picked(picked) => Some(picked),
            ToolEvent::Refused(_) => None,
        })
        .collect()
}

#[test]
fn fila_5_el_cuentagotas_elige_lo_visible_por_defecto() {
    let (mut doc, celda_b, _) = doc_de_cuentagotas();
    let esperado = composite_pixel(FONDO, ROJO, 128, BlendMode::Normal);
    assert_ne!(esperado, ROJO, "la mezcla cambia el color");
    assert_eq!(
        elige(&mut doc, celda_b, None, (5, 5)),
        vec![Picked::Color(esperado)]
    );
    // La capa oculta no se ve.
    assert_eq!(
        elige(&mut doc, celda_b, None, (6, 6)),
        vec![Picked::Color(FONDO)]
    );
    // Sobre la celda vacía de la activa se ve lo de debajo, no «transparente».
    assert_eq!(
        elige(&mut doc, celda_b, None, (0, 0)),
        vec![Picked::Color(FONDO)]
    );
}

#[test]
fn fila_5_la_fuente_capa_lee_la_celda_activa_y_la_vacia_es_transparente() {
    let (mut doc, celda_b, _) = doc_de_cuentagotas();
    let capa = Some(DropperSource::Layer);
    assert_eq!(
        elige(&mut doc, celda_b, capa, (5, 5)),
        vec![Picked::Color(ROJO)]
    );
    assert_eq!(
        elige(&mut doc, celda_b, capa, (0, 0)),
        vec![Picked::Color(Rgba::TRANSPARENT)]
    );
    let (mut vacio, _, _) = doc_de_cuentagotas();
    let (_, celda_d) = capa_nueva(&mut vacio, "D");
    assert!(vacio.image(celda_d).is_none());
    assert_eq!(
        elige(&mut vacio, celda_d, capa, (3, 3)),
        vec![Picked::Color(Rgba::TRANSPARENT)]
    );
    let antes = vacio.clone();
    assert_eq!(vacio, antes, "el cuentagotas no modifica el documento");
}

#[test]
fn fila_5_con_una_capa_visible_las_dos_fuentes_dan_lo_de_hoy_incluido_el_alfa_cero() {
    let mut imagen = rgba_lleno(4, 4, FONDO);
    let oculto = Rgba::new(12, 34, 56, 0);
    imagen.set_pixel(2, 2, Pixel::Rgba(oculto)).expect("dentro");
    let mut doc = Document::new(imagen.clone());
    let celda = doc.layers()[0].cel();
    let de_hoy = picked_at(&imagen, 2, 2).expect("dentro");
    assert_eq!(de_hoy, Picked::Color(oculto));
    for fuente in [None, Some(DropperSource::Layer)] {
        assert_eq!(elige(&mut doc, celda, fuente, (2, 2)), vec![de_hoy]);
    }
    assert!(
        px_ui::pick::pick_notice(&de_hoy).is_some(),
        "el aviso de «copiado tal cual»"
    );
}

#[test]
fn fila_5_en_indexado_lo_visible_es_la_entrada_de_la_capa_visible_mas_alta_no_transparente() {
    let mut doc = doc_indexado(6, 4);
    let (_, celda_b) = capa_nueva(&mut doc, "B");
    let (_, celda_c) = capa_nueva(&mut doc, "C");
    pinta(&mut doc, celda_b, &[(1, 1)], Pixel::Indexed(3));
    pinta(&mut doc, celda_c, &[(1, 1)], Pixel::Indexed(7));
    pinta(&mut doc, celda_c, &[(2, 2)], Pixel::Indexed(5));
    assert_eq!(
        elige(&mut doc, celda_c, None, (1, 1)),
        vec![Picked::Entry(3)]
    );
    assert_eq!(
        elige(&mut doc, celda_c, None, (2, 2)),
        vec![Picked::Entry(5)]
    );
    assert_eq!(
        elige(&mut doc, celda_c, None, (0, 0)),
        vec![Picked::Entry(1)]
    );
    let oraculo = compose_rect(
        &doc,
        Rect {
            x: 2,
            y: 2,
            width: 1,
            height: 1,
        },
    )
    .expect("compose");
    assert_eq!(visible_picked(&doc, 2, 2), picked_at(&oraculo, 0, 0));
}

// ---------------------------------------------------------------------------------------------
// Filas 7, 8 y 9 (parte lógica): el origen compuesto, la región sucia y la superposición
// ---------------------------------------------------------------------------------------------

/// Un documento de 3 capas con mezcla, opacidad y una oculta, en RGBA.
fn doc_de_tres_capas() -> Document {
    let mut doc = doc_rgba(130, 70);
    let (capa_b, celda_b) = capa_nueva(&mut doc, "B");
    let (capa_c, celda_c) = capa_nueva(&mut doc, "C");
    let puntos: Vec<(u32, u32)> = (0..130).step_by(3).map(|x| (x, (x * 7) % 70)).collect();
    pinta(
        &mut doc,
        celda_b,
        &puntos,
        rgba(Rgba::new(200, 100, 50, 200)),
    );
    pinta(&mut doc, celda_c, &puntos, rgba(AZUL));
    doc.apply(Command::SetLayerOpacity {
        layer: capa_b,
        opacity: 128,
    })
    .expect("opacidad");
    doc.apply(Command::SetLayerBlend {
        layer: capa_b,
        blend: BlendMode::Multiply,
    })
    .expect("mezcla");
    doc.apply(Command::SetLayerVisible {
        layer: capa_c,
        visible: false,
    })
    .expect("ocultar");
    doc
}

fn region_completa(fuente: &dyn PixelSource, ancho: u32, alto: u32) -> Vec<egui::Color32> {
    fuente
        .region(
            TileRect {
                x: 0,
                y: 0,
                width: ancho,
                height: alto,
            },
            None,
            &mut SystemTileAlloc,
        )
        .expect("región")
}

#[test]
fn fila_7_el_origen_compuesto_de_varias_capas_es_compose() {
    let doc = doc_de_tres_capas();
    assert!(identity_cell(&doc).is_none());
    let oraculo = esperado(&compose(&doc).expect("compose"));
    assert_eq!(region_completa(&DocSource(&doc), 130, 70), oraculo);
    // Un recorte también es el recorte de la composición.
    let parte = DocSource(&doc)
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
    let mut esperada = Vec::new();
    for y in 10..40usize {
        esperada.extend_from_slice(&oraculo[y * 130 + 60..y * 130 + 100]);
    }
    assert_eq!(parte, esperada);
}

#[test]
fn fila_7_el_origen_compuesto_de_un_indexado_es_compose_y_sustituye_una_entrada() {
    let mut doc = doc_indexado(20, 10);
    let (_, celda_b) = capa_nueva(&mut doc, "B");
    pinta(&mut doc, celda_b, &[(3, 3), (4, 3)], Pixel::Indexed(5));
    let compuesta = compose(&doc).expect("compose");
    assert_eq!(
        region_completa(&DocSource(&doc), 20, 10),
        esperado(&compuesta)
    );
    let color = Rgba::opaque(1, 2, 3);
    let con_entrada = DocSource(&doc)
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
        con_entrada[3 * 20 + 3],
        egui::Color32::from_rgba_unmultiplied(1, 2, 3, 255)
    );
    assert_eq!(DocSource(&doc).transparent_index(), Some(7));
}

#[test]
fn fila_7_con_una_capa_visible_opaca_y_normal_se_lee_la_celda_tal_cual() {
    let mut imagen = rgba_lleno(8, 8, FONDO);
    imagen
        .set_pixel(3, 3, Pixel::Rgba(Rgba::new(9, 8, 7, 0)))
        .expect("dentro");
    let doc = Document::new(imagen.clone());
    assert_eq!(identity_cell(&doc), Some(&imagen));
    assert_eq!(region_completa(&DocSource(&doc), 8, 8), esperado(&imagen));
    assert!(matches!(flatten(&doc).expect("aplanar"), Cow::Borrowed(_)));
    // Con la opacidad 254 ya no es la identidad.
    let mut doc = doc;
    let capa = doc.layers()[0].id();
    doc.apply(Command::SetLayerOpacity {
        layer: capa,
        opacity: 254,
    })
    .expect("opacidad");
    assert!(identity_cell(&doc).is_none());
    assert!(matches!(flatten(&doc).expect("aplanar"), Cow::Owned(_)));
}

#[test]
fn fila_11_aplanar_con_varias_capas_es_compose_y_un_gris_sin_nada_visible_da_error() {
    let doc = doc_de_tres_capas();
    assert_eq!(
        *flatten(&doc).expect("aplanar"),
        compose(&doc).expect("compose")
    );
    let mut gris = Document::new(Image::new(3, 3, Mode::Gray).expect("imagen"));
    let capa = gris.layers()[0].id();
    gris.apply(Command::SetLayerVisible {
        layer: capa,
        visible: false,
    })
    .expect("ocultar");
    assert!(flatten(&gris).is_err());
}

#[test]
fn fila_8_la_region_sucia_de_los_comandos_de_capa() {
    let size = Size::new(130, 70);
    let todo = BlockSet::full(130, 70);
    let doc = doc_de_tres_capas();
    let capa = doc.layers()[1].id();
    let nada = |orden: Command| assert!(dirty_of(&orden, size).is_empty(), "{orden:?}");
    let entera = |orden: Command| assert_eq!(dirty_of(&orden, size), todo, "{orden:?}");
    nada(Command::AddLayer {
        name: "X".to_string(),
        at: 1,
    });
    nada(Command::RenameLayer {
        layer: capa,
        name: "Y".to_string(),
    });
    entera(Command::SetLayerOpacity {
        layer: capa,
        opacity: 3,
    });
    entera(Command::SetLayerVisible {
        layer: capa,
        visible: false,
    });
    entera(Command::SetLayerBlend {
        layer: capa,
        blend: BlendMode::Screen,
    });
    entera(Command::MoveLayer { layer: capa, to: 0 });
    entera(Command::RemoveLayer { layer: capa });
}

#[test]
fn fila_9_la_superposicion_oculta_el_trazo_bajo_una_capa_opaca_y_lo_mezcla_con_128() {
    let mut doc = doc_rgba(8, 8);
    let (_, celda_b) = capa_nueva(&mut doc, "B");
    let (capa_c, celda_c) = capa_nueva(&mut doc, "C");
    pinta(&mut doc, celda_c, &[(2, 2)], rgba(ROJO));
    let edits = [
        PixelEdit {
            x: 2,
            y: 2,
            pixel: rgba(VERDE),
        },
        PixelEdit {
            x: 3,
            y: 3,
            pixel: rgba(VERDE),
        },
    ];
    // C opaca encima en (2, 2): el trazo no se ve ahí; en (3, 3) no hay nada encima.
    let colores = overlay_pixels(&doc, celda_b, edits);
    assert_eq!(colores, vec![(2, 2, ROJO), (3, 3, VERDE)]);
    // Con C a 128 se mezcla: es lo que dará compose tras aplicar el comando de verdad.
    doc.apply(Command::SetLayerOpacity {
        layer: capa_c,
        opacity: 128,
    })
    .expect("opacidad");
    let colores = overlay_pixels(&doc, celda_b, edits);
    let mut aplicado = doc.clone();
    aplicado
        .apply(Command::SetPixels {
            image: celda_b,
            pixels: edits.to_vec(),
        })
        .expect("aplicar");
    assert_eq!(colores[0].2, visto(&aplicado, 2, 2));
    assert_eq!(colores[1].2, visto(&aplicado, 3, 3));
    assert_ne!(colores[0].2, ROJO);
}

#[test]
fn fila_9_el_borrador_en_la_superposicion_muestra_lo_de_debajo() {
    let mut doc = doc_rgba(8, 8);
    let (_, celda_b) = capa_nueva(&mut doc, "B");
    pinta(&mut doc, celda_b, &[(1, 1)], rgba(ROJO));
    let borrar = [PixelEdit {
        x: 1,
        y: 1,
        pixel: rgba(Rgba::TRANSPARENT),
    }];
    assert_eq!(overlay_pixels(&doc, celda_b, borrar), vec![(1, 1, FONDO)]);
}

#[test]
fn fila_9_con_una_capa_la_superposicion_es_el_color_crudo_de_hoy() {
    let doc = doc_rgba(8, 8);
    let celda = doc.layers()[0].cel();
    let semitransparente = Rgba::new(1, 2, 3, 0);
    let edits = [
        PixelEdit {
            x: 1,
            y: 1,
            pixel: rgba(ROJO),
        },
        PixelEdit {
            x: 2,
            y: 2,
            pixel: rgba(semitransparente),
        },
    ];
    assert_eq!(
        overlay_pixels(&doc, celda, edits),
        vec![(1, 1, ROJO), (2, 2, semitransparente)]
    );
}

// ---------------------------------------------------------------------------------------------
// Propiedad 2: la superposición no miente
// ---------------------------------------------------------------------------------------------

fn semilla_de_capas() -> impl Strategy<
    Value = (
        u8,
        Vec<(u8, u8, u8, u8, u32, u32)>,
        u8,
        Vec<(u32, u32, u32)>,
    ),
> {
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

/// Un píxel del modo `modo` según `k` (con alfas 0, 1, 128 y 255; índices dentro de la paleta).
fn pixel_para(modo: Mode, k: u32) -> Pixel {
    tiles_support::pixel_de(modo, k, paleta().len())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para cualquier documento (RGBA, gris con alfa o indexado; de 1 a 4 capas con opacidad,
    /// mezcla, visibilidad y celdas vacías aleatorias), capa activa y trazo (con el valor de
    /// borrado incluido): el color de cada píxel de la superposición es el píxel de `compose` del
    /// documento tras aplicar de verdad el comando del trazo.
    #[test]
    fn propiedad_2_la_superposicion_es_la_composicion_con_el_trazo_aplicado(
        (modo, capas, activa, trazo) in semilla_de_capas(),
    ) {
        let modo = [Mode::Rgba, Mode::GrayAlpha, Mode::Indexed][usize::from(modo)];
        let base = match modo {
            Mode::Indexed => tiles_support::imagen_de(modo, 12, 9, 3),
            _ => tiles_support::imagen_de(modo, 12, 9, 2),
        };
        let mut doc = Document::new(base);
        for (i, &(capa, opacidad, mezcla, visible, a, b)) in capas.iter().enumerate() {
            let at = doc.layers().len();
            if doc.apply(Command::AddLayer { name: format!("C{i}"), at }).is_err() {
                continue;
            }
            let id = doc.layers()[at].id();
            let celda = doc.layers()[at].cel();
            if capa != 0 {
                let pixels: Vec<PixelEdit> = (0..1 + b % 20)
                    .map(|j| PixelEdit {
                        x: a.wrapping_add(j.wrapping_mul(5)) % 12,
                        y: b.wrapping_add(j.wrapping_mul(3)) % 9,
                        pixel: pixel_para(modo, a.wrapping_add(j)),
                    })
                    .collect();
                let _ = doc.apply(Command::SetPixels { image: celda, pixels });
            }
            let _ = doc.apply(Command::SetLayerOpacity { layer: id, opacity: [255, 0, 1, 128, 200, 254, 255, 77][usize::from(opacidad)] });
            let _ = doc.apply(Command::SetLayerBlend { layer: id, blend: BlendMode::ALL[usize::from(mezcla) * 3 % 8] });
            let _ = doc.apply(Command::SetLayerVisible { layer: id, visible: visible != 0 });
        }
        let capa_activa = &doc.layers()[usize::from(activa) % doc.layers().len()];
        let celda = capa_activa.cel();
        let edits: Vec<PixelEdit> = trazo
            .iter()
            .map(|&(x, y, k)| PixelEdit { x, y, pixel: pixel_para(modo, k) })
            .collect();
        // Sin repetidos: el último valor de cada coordenada es el que se aplica.
        let mut unicos: Vec<PixelEdit> = Vec::new();
        for edit in edits {
            match unicos.iter_mut().find(|e| (e.x, e.y) == (edit.x, edit.y)) {
                Some(e) => e.pixel = edit.pixel,
                None => unicos.push(edit),
            }
        }
        let colores = overlay_pixels(&doc, celda, unicos.clone());
        prop_assert_eq!(colores.len(), unicos.len());
        let mut aplicado = doc.clone();
        if aplicado.apply(Command::SetPixels { image: celda, pixels: unicos.clone() }).is_ok() {
            if let Ok(compuesta) = compose(&aplicado) {
                for (x, y, color) in colores {
                    let pixel = compuesta.get_pixel(x, y).unwrap();
                    prop_assert_eq!(Some(color), pixel_rgba(&compuesta, pixel), "({}, {})", x, y);
                }
            }
        }
    }
}

// Lo que no se usa en todos los ejecutables.
#[allow(dead_code)]
fn _usos(_: Span, _: Palette) {}
