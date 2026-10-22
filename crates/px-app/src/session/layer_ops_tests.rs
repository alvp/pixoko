//! Las operaciones de capa de la sesión (specs/hito-3-capas-interfaz.md, unidad 3.4: filas 8 a
//! 12 y propiedad 3, y condición 1 del visto bueno). En ROJO hasta la lógica: `LayerReport`,
//! `new_layer`, `duplicate_layer`, `remove_layer`, `move_layer`, `set_layer_*`, `rename_layer`,
//! `layer_nav`, `memory_crossing_notice_with`, `create_budget_notice`, `dropper_source_for`,
//! `confirm::Request::LayersToGrayAlpha` y `convert_mode::open_convert_mode_to` no existen todavía.
//!
//! Contrato de API que estas pruebas fijan (la lógica debe ofrecerlo):
//! - `Session::{new_layer, duplicate_layer, remove_layer}() -> LayerReport` y
//!   `move_layer(delta: i32)` (`+1` sube), `set_layer_visible/locked(id, bool)`,
//!   `rename_layer(id, String)`, `set_layer_opacity(id, u8)`, `set_layer_blend(id, BlendMode)`,
//!   todos `-> LayerReport { changed, notice: Option<String>, offer_convert }`.
//! - `Session::layer_nav() -> LayerNav`; `paint_refusal` con el bloqueo; `clear_page` con
//!   `ClearPageError::{LockedLayer, HiddenAndLockedLayer}`.
//! - `Session::memory_crossing_notice_with(warn: usize) -> Option<String>` (la versión sin
//!   argumento usa `Document::MEMORY_WARN_BYTES`) y `create_budget_notice(worst, limit, warn)`.
//! - `dropper_source_for(shift: bool) -> DropperSource` (`Mayús` = la capa activa).
//! - `undo` y `redo` de la sesión llevan la activa a la capa que entra.

use super::*;

use std::collections::BTreeSet;

use proptest::prelude::*;
use px_core::{BlendMode, ImageId, LayerId, Palette, PixelEdit, Rgba};
use px_ui::actions::{LayerNav, Locks};
use px_ui::canvas::CanvasEvent;
use px_ui::dialog::Modal;
use px_ui::pick::Picked;
use px_ui::tool::{DropperSource, Tool, ToolAction, ToolEvent, ToolState};

use crate::confirm::{Confirm, Confirmation, Request};
use crate::convert_mode::{FIELD_TARGET, open_convert_mode_to, targets};

const TEST_RED: Rgba = Rgba::opaque(255, 0, 0);
const TEST_BLUE: Rgba = Rgba::opaque(0, 0, 255);
const TEST_BACKGROUND: Rgba = Rgba::opaque(10, 20, 30);
const HIDDEN: &str = "la capa activa está oculta";
const LOCKED: &str = "la capa activa está bloqueada";
const HIDDEN_AND_LOCKED: &str = "la capa activa está oculta y bloqueada";

fn full(width: u32, height: u32, mode_2: Mode, pixel: Pixel) -> Image {
    let mut image_2 = Image::new(width, height, mode_2).expect("imagen");
    for y in 0..height {
        for x in 0..width {
            image_2.set_pixel(x, y, pixel).expect("dentro");
        }
    }
    image_2
}

fn session_rgba(width: u32, height: u32) -> Session {
    Session::sample(Document::new(full(
        width,
        height,
        Mode::Rgba,
        Pixel::Rgba(TEST_BACKGROUND),
    )))
}

fn ids(session_2: &Session) -> Vec<LayerId> {
    session_2.document.layers().iter().map(|c| c.id()).collect()
}

fn position(session_2: &Session, id: LayerId) -> usize {
    ids(session_2)
        .iter()
        .position(|c| *c == id)
        .expect("la capa existe")
}

fn paints(session_2: &mut Session, points: &[(u32, u32)], color: Rgba) {
    let cel = session_2.active_cel();
    session_2
        .apply_layer(Command::SetPixels {
            image: cel,
            pixels: points
                .iter()
                .map(|&(x, y)| PixelEdit {
                    x,
                    y,
                    pixel: Pixel::Rgba(color),
                })
                .collect(),
        })
        .expect("pintar");
}

/// Un gesto de `tool_2` sobre la capa activa, como la app: primero la guardia.
fn gesture(
    session_2: &mut Session,
    tools: &mut ToolState,
    tool_2: Tool,
    points: &[(i32, i32)],
) -> bool {
    if session_2.paint_refusal(tool_2).is_some() {
        return false;
    }
    let cel = session_2.active_cel();
    let pixel = session_2.pixel_of(Role::Foreground);
    tools.apply_action(ToolAction::Select(tool_2));
    let mut change = false;
    let mut events = vec![CanvasEvent::StrokeBegin(points[0])];
    events.extend(points[1..].iter().map(|&p| CanvasEvent::StrokeMove(p)));
    events.push(CanvasEvent::StrokeEnd);
    for event_2 in events {
        change |= tools
            .canvas_event_with(&mut session_2.document, cel, pixel, event_2)
            .unwrap_or(false);
    }
    session_2.absorb(tools.take_changes());
    change
}

/// Tres capas `Capa 1` a `Capa 3` con la del medio activa.
fn with_three_layers() -> (Session, [LayerId; 3]) {
    let mut session_2 = session_rgba(16, 12);
    for _ in 0..2 {
        assert!(session_2.new_layer().changed);
    }
    let layers_2 = [ids(&session_2)[0], ids(&session_2)[1], ids(&session_2)[2]];
    session_2.set_active_layer(layers_2[1]).expect("existe");
    (session_2, layers_2)
}

// ---------------------------------------------------------------------------------------------
// Fila 8: la sesión y la capa activa
// ---------------------------------------------------------------------------------------------

#[test]
fn row_8_new_layer_goes_above_the_active_one_and_becomes_the_active_one() {
    let (mut session_2, layers_2) = with_three_layers();
    let antes = session_2.document.state_id();
    let report = session_2.new_layer();
    assert!(report.changed && report.notice.is_none() && !report.offer_convert);
    assert_ne!(session_2.document.state_id(), antes);
    let now = ids(&session_2);
    assert_eq!(now.len(), 4);
    assert_eq!(&now[..2], &layers_2[..2]);
    assert_eq!(now[3], layers_2[2], "la que estaba encima sigue encima");
    assert_eq!(session_2.active_layer(), now[2]);
    assert_eq!(session_2.document.layers()[2].name(), "Capa 4");
    assert!(
        session_2.document.image(session_2.active_cel()).is_none(),
        "vacía: no reserva"
    );
    assert!(session_2.modified());
    // Un solo paso de historia.
    session_2.undo().expect("deshacer");
    assert_eq!(ids(&session_2), layers_2);
}

#[test]
fn row_8_duplicate_copies_on_top_keeps_the_lock_and_the_copy_is_the_active_one() {
    let (mut session_2, layers_2) = with_three_layers();
    paints(&mut session_2, &[(1, 1), (2, 2)], TEST_RED);
    session_2.set_layer_locked(layers_2[1], true);
    session_2
        .set_layer_opacity(layers_2[1], 128)
        .changed
        .then_some(())
        .expect("opacidad");
    let report = session_2.duplicate_layer();
    assert!(report.changed && report.notice.is_none(), "{report:?}");
    let now = ids(&session_2);
    assert_eq!(now.len(), 4);
    let copy = session_2.document.layers()[2].clone();
    assert_eq!(copy.id(), now[2]);
    assert_ne!(copy.id(), layers_2[1]);
    assert_eq!(
        session_2.active_layer(),
        copy.id(),
        "la copia pasa a ser la activa"
    );
    assert!(copy.locked(), "conserva el bloqueo");
    assert_eq!(copy.opacity(), 128);
    assert_eq!(copy.name(), "Capa 2 copia");
    let original = session_2
        .document
        .image(session_2.document.layers()[1].cel())
        .expect("píxeles");
    let new = session_2
        .document
        .image(copy.cel())
        .expect("copia con píxeles");
    assert_eq!(original, new);
    // Un solo paso: deshacer quita la copia y la activa pasa a la que ocupa su puesto (regla de la
    // vecina de 3.3: la que estaba encima).
    session_2.undo().expect("deshacer");
    assert_eq!(ids(&session_2).len(), 3);
    assert_eq!(session_2.active_layer(), layers_2[2]);
    session_2.redo().expect("rehacer");
    assert_eq!(
        session_2.active_layer(),
        now[2],
        "rehacer la copia la activa"
    );
}

#[test]
fn row_8_duplicating_an_empty_cel_leaves_the_copy_empty() {
    let (mut session_2, _) = with_three_layers();
    let antes = session_2.document.materialized_bytes();
    assert!(session_2.duplicate_layer().changed);
    assert_eq!(session_2.document.materialized_bytes(), antes);
    assert!(session_2.document.image(session_2.active_cel()).is_none());
}

#[test]
fn row_8_raise_and_lower_move_one_place_and_the_active_one_is_the_same() {
    let (mut session_2, layers_2) = with_three_layers();
    assert!(session_2.move_layer(1).changed);
    assert_eq!(position(&session_2, layers_2[1]), 2);
    assert_eq!(session_2.active_layer(), layers_2[1]);
    // Ya arriba: no cambia nada ni la historia.
    let id = session_2.document.state_id();
    assert!(!session_2.move_layer(1).changed);
    assert_eq!(session_2.document.state_id(), id);
    assert!(session_2.move_layer(-1).changed && session_2.move_layer(-1).changed);
    assert_eq!(position(&session_2, layers_2[1]), 0);
    assert!(!session_2.move_layer(-1).changed);
    assert_eq!(session_2.active_layer(), layers_2[1]);
    // Deshacer un movimiento no cambia cuál es la activa.
    session_2.undo().expect("deshacer");
    assert_eq!(session_2.active_layer(), layers_2[1]);
}

#[test]
fn row_8_removing_moves_to_the_neighbor_and_undo_and_redo_follow_the_entry() {
    let (mut session_2, layers_2) = with_three_layers();
    let report = session_2.remove_layer();
    assert!(report.changed);
    assert_eq!(ids(&session_2), [layers_2[0], layers_2[2]]);
    assert!(
        layers_2[..1]
            .iter()
            .chain(&layers_2[2..])
            .any(|c| *c == session_2.active_layer())
    );
    // Deshacer `Remove`: la capa que entra pasa a ser la activa.
    session_2.undo().expect("deshacer");
    assert_eq!(ids(&session_2), layers_2);
    assert_eq!(session_2.active_layer(), layers_2[1]);
    // Rehacer `New layer_2`: la nueva es la activa; deshacerla pasa a la que ocupa su puesto.
    session_2.set_active_layer(layers_2[0]).expect("existe");
    assert!(session_2.new_layer().changed);
    let new = session_2.active_layer();
    session_2.undo().expect("deshacer");
    assert_eq!(
        session_2.active_layer(),
        layers_2[1],
        "la que ocupa el puesto de la nueva"
    );
    session_2.redo().expect("rehacer");
    assert_eq!(session_2.active_layer(), new);
}

#[test]
fn row_8_with_a_single_layer_remove_changes_nothing_and_warns() {
    let mut session_2 = session_rgba(8, 8);
    let copy = session_2.document.clone();
    let report = session_2.remove_layer();
    assert!(!report.changed);
    assert!(report.notice.expect("aviso").contains("última"));
    assert_eq!(session_2.document, copy);
    assert!(!session_2.modified());
    let nav = session_2.layer_nav();
    assert_eq!((nav.count, nav.active_pos, nav.can_add), (1, 0, true));
}

#[test]
fn row_8_the_properties_are_one_step_each_and_mark_modified() {
    let (mut session_2, layers_2) = with_three_layers();
    session_2.mark_unmodified();
    // Pasos de historia: cuántas veces se puede deshacer en una copia.
    let steps_2 = |s: &Session| {
        let mut copy = s.document.clone();
        let mut n = 0;
        while copy.undo().is_ok() {
            n += 1;
        }
        n
    };
    let base = steps_2(&session_2);
    assert!(session_2.set_layer_visible(layers_2[2], false).changed);
    assert!(session_2.set_layer_locked(layers_2[2], true).changed);
    assert!(
        session_2
            .rename_layer(layers_2[2], "Cielo".to_string())
            .changed
    );
    assert!(session_2.set_layer_opacity(layers_2[2], 100).changed);
    assert!(
        session_2
            .set_layer_blend(layers_2[2], BlendMode::Multiply)
            .changed
    );
    assert_eq!(steps_2(&session_2), base + 5);
    assert!(session_2.modified());
    // Repetir un valor no hace nada.
    assert!(!session_2.set_layer_opacity(layers_2[2], 100).changed);
    assert_eq!(steps_2(&session_2), base + 5);
    for _ in 0..5 {
        session_2.undo().expect("deshacer");
    }
    assert!(!session_2.modified());
}

// ---------------------------------------------------------------------------------------------
// Fila 9: capa bloqueada u oculta
// ---------------------------------------------------------------------------------------------

#[test]
fn row_9_locked_hidden_or_both_no_tool_draws_and_the_dropper_does() {
    let (mut session_2, layers_2) = with_three_layers();
    let mut tools = ToolState::new();
    let tools_2 = [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Eraser];
    for (hidden, locked_2, notice) in [
        (false, true, LOCKED),
        (true, false, HIDDEN),
        (true, true, HIDDEN_AND_LOCKED),
    ] {
        session_2.set_layer_visible(layers_2[1], !hidden);
        session_2.set_layer_locked(layers_2[1], locked_2);
        let copy = session_2.document.clone();
        let id = session_2.document.state_id();
        for tool_2 in tools_2 {
            assert_eq!(session_2.paint_refusal(tool_2), Some(notice), "{tool_2:?}");
            assert!(!gesture(
                &mut session_2,
                &mut tools,
                tool_2,
                &[(1, 1), (5, 5)]
            ));
        }
        assert_eq!(session_2.paint_refusal(Tool::Dropper), None);
        assert_eq!(session_2.document, copy);
        assert_eq!(session_2.document.state_id(), id);
        // `Delete página` también se niega, con el mismo motivo.
        let error = session_2.clear_page().expect_err("negada");
        assert_eq!(error.to_string(), notice);
        assert_eq!(session_2.document, copy);
    }
    // Al desbloquear y mostrar, se pinta.
    session_2.set_layer_visible(layers_2[1], true);
    session_2.set_layer_locked(layers_2[1], false);
    assert_eq!(session_2.paint_refusal(Tool::Pencil), None);
    assert!(gesture(
        &mut session_2,
        &mut tools,
        Tool::Pencil,
        &[(1, 1), (5, 5)]
    ));
}

#[test]
fn row_9_with_the_layer_locked_it_can_be_renamed_changed_moved_duplicated_and_removed() {
    let (mut session_2, layers_2) = with_three_layers();
    session_2.set_layer_locked(layers_2[1], true);
    assert!(
        session_2
            .rename_layer(layers_2[1], "Fondo".to_string())
            .changed
    );
    assert!(session_2.set_layer_opacity(layers_2[1], 90).changed);
    assert!(
        session_2
            .set_layer_blend(layers_2[1], BlendMode::Screen)
            .changed
    );
    assert!(session_2.set_layer_visible(layers_2[1], false).changed);
    assert!(session_2.move_layer(1).changed);
    assert!(session_2.duplicate_layer().changed);
    session_2.set_active_layer(layers_2[1]).expect("existe");
    assert!(session_2.remove_layer().changed);
}

#[test]
fn row_9_a_palette_change_also_affects_the_locked_layer() {
    let mut palette = Palette::from_colors([
        Rgba::opaque(0, 0, 0),
        Rgba::opaque(200, 20, 20),
        Rgba::opaque(255, 255, 255),
    ])
    .expect("paleta");
    palette.set_transparent_index(Some(2)).expect("existe");
    let mut image_2 = Image::new_indexed(4, 4, palette).expect("imagen");
    image_2.set_pixel(1, 1, Pixel::Indexed(1)).expect("dentro");
    let mut session_2 = Session::sample(Document::new(image_2));
    assert!(session_2.new_layer().changed);
    let layer_2 = ids(&session_2)[0];
    session_2.set_layer_locked(layer_2, true);
    let cel = session_2.document.layers()[0].cel();
    let new_2 = Rgba::opaque(1, 2, 3);
    session_2
        .apply_layer(Command::SetPaletteColor {
            image: cel,
            index: 1,
            color: new_2,
        })
        .expect("la paleta es del documento");
    for layer_2 in session_2.document.layers() {
        if let Some(image_2) = session_2.document.image(layer_2.cel()) {
            assert_eq!(image_2.palette().expect("indexada").get(1), Ok(new_2));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 10: presupuesto
// ---------------------------------------------------------------------------------------------

const GIB: usize = 1 << 30;
const MIB: usize = 1 << 20;

#[test]
fn row_10_the_notice_on_creating_comes_out_with_the_worst_case_above_1_gib() {
    // 8192² RGBA: celda de 256 MiB; avisa al llegar a la quinta (peor caso 1,25 GiB).
    let cel = 256 * MIB;
    for layers_2 in 1..=8usize {
        let notice = create_budget_notice(layers_2 * cel, 2 * GIB, GIB);
        assert_eq!(notice.is_some(), layers_2 >= 5, "{layers_2} capas");
    }
    let text_2 = create_budget_notice(5 * cel, 2 * GIB, GIB).expect("aviso");
    assert!(
        text_2.contains("1,2 GiB") && text_2.contains("2 GiB"),
        "{text_2}"
    );
    // 4096² (64 MiB): a la decimoséptima; 1024² (4 MiB): nunca (256 × 4 MiB = 1 GiB).
    assert!(create_budget_notice(16 * 64 * MIB, 2 * GIB, GIB).is_none());
    assert!(create_budget_notice(17 * 64 * MIB, 2 * GIB, GIB).is_some());
    assert!(create_budget_notice(256 * 4 * MIB, 2 * GIB, GIB).is_none());
}

#[test]
fn row_10_new_layer_and_duplicate_warn_when_crossing_the_real_worst_case() {
    // 2048² RGBA = 16 MiB: la 65.ª capa pasa de 1 GiB de peor caso.
    let mut session_2 = Session::sample(Document::new(full(
        2048,
        2048,
        Mode::Rgba,
        Pixel::Rgba(TEST_BACKGROUND),
    )));
    for n in 2..=64usize {
        let report = session_2.new_layer();
        assert!(
            report.changed && report.notice.is_none(),
            "capa {n}: {report:?}"
        );
    }
    let report = session_2.new_layer();
    assert!(report.changed, "la capa se crea igualmente");
    assert!(report.notice.expect("aviso de presupuesto").contains("GiB"));
}

#[test]
fn row_10_the_notice_on_painting_comes_out_once_per_crossing_and_not_on_undoing_below() {
    let mut session_2 = session_rgba(64, 64);
    let cel_bytes = session_2.document.cel_bytes();
    let threshold = 3 * cel_bytes; // «1 GiB» de la prueba: tres celdas
    assert!(session_2.memory_crossing_notice_with(threshold).is_none());
    for _ in 0..3 {
        assert!(session_2.new_layer().changed);
    }
    assert!(session_2.memory_crossing_notice_with(threshold).is_none());
    // Materializar las celdas 2 y 3 (la 1 ya lo está): al llegar a 3 celdas se cruza.
    paints(&mut session_2, &[(1, 1)], TEST_RED);
    assert!(
        session_2.memory_crossing_notice_with(threshold).is_none(),
        "dos celdas"
    );
    let layer_2 = ids(&session_2)[2];
    session_2.set_active_layer(layer_2).expect("existe");
    paints(&mut session_2, &[(2, 2)], TEST_RED);
    let notice = session_2
        .memory_crossing_notice_with(threshold)
        .expect("cruza");
    assert!(notice.contains("ya ocupan"), "{notice}");
    // No se repite sin volver a cruzar.
    assert!(session_2.memory_crossing_notice_with(threshold).is_none());
    paints(&mut session_2, &[(3, 3)], TEST_BLUE);
    assert!(session_2.memory_crossing_notice_with(threshold).is_none());
    // Deshacer por debajo no avisa; volver a cruzar (rehacer) sí.
    session_2.undo().expect("deshacer");
    session_2.undo().expect("deshacer");
    assert!(session_2.memory_crossing_notice_with(threshold).is_none());
    session_2.redo().expect("rehacer");
    session_2.redo().expect("rehacer");
    assert!(session_2.memory_crossing_notice_with(threshold).is_some());
}

#[test]
fn row_10_without_room_and_with_256_layers_are_notices_with_the_document_intact() {
    let mut session_2 = session_rgba(64, 64);
    // `CelsTooLarge`: el tope solo deja la celda que ya existe.
    let limit_2 = session_2.document.cel_bytes();
    session_2.document.set_cells_limit(limit_2);
    let copy = session_2.document.clone();
    let id = session_2.document.state_id();
    let report = session_2.duplicate_layer();
    assert!(!report.changed);
    assert!(report.notice.is_some());
    assert_eq!(session_2.document, copy);
    assert_eq!(session_2.document.state_id(), id);
    session_2
        .document
        .set_cells_limit(Document::DEFAULT_CELLS_LIMIT);
    // 256 capas: `TooManyLayers` y `LayerNav::can_add` en falso.
    while session_2.document.layers().len() < Document::MAX_LAYERS {
        assert!(session_2.new_layer().changed);
    }
    let nav: LayerNav = session_2.layer_nav();
    assert!(!nav.can_add);
    let copy = session_2.document.clone();
    for report in [session_2.new_layer(), session_2.duplicate_layer()] {
        assert!(!report.changed);
        assert!(report.notice.is_some());
    }
    assert_eq!(session_2.document, copy);
}

// ---------------------------------------------------------------------------------------------
// Fila 11: modos sin transparencia
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_in_gray_it_is_not_created_or_duplicated_and_going_to_gray_alpha_is_offered() {
    let mut session_2 =
        Session::sample(Document::new(Image::new(4, 4, Mode::Gray).expect("imagen")));
    let copy = session_2.document.clone();
    for report in [session_2.new_layer(), session_2.duplicate_layer()] {
        assert!(!report.changed);
        assert!(report.offer_convert);
        let notice = report.notice.expect("aviso");
        assert!(
            notice.contains("gris con alfa") && notice.contains("repite"),
            "{notice}"
        );
    }
    assert_eq!(session_2.document, copy);
    assert!(!session_2.document.can_undo());
    assert!(!session_2.modified());
}

#[test]
fn row_11_repeating_the_command_in_5_s_opens_convert_mode_with_gray_alpha_chosen() {
    let mut confirmation = Confirm::new();
    assert_eq!(
        confirmation.request(Request::LayersToGrayAlpha, 10.0),
        Confirmation::Armed
    );
    assert_eq!(
        confirmation.request(Request::LayersToGrayAlpha, 14.0),
        Confirmation::Confirmed
    );
    assert_eq!(
        confirmation.request(Request::LayersToGrayAlpha, 20.0),
        Confirmation::Armed
    );
    // A los 6 s no vale: vuelve a avisar.
    assert_eq!(
        confirmation.request(Request::LayersToGrayAlpha, 26.0),
        Confirmation::Armed
    );
    // El diálogo se abre con el destino ya elegido.
    let mut modal = Modal::default();
    assert!(open_convert_mode_to(
        &mut modal,
        Mode::Gray,
        Mode::GrayAlpha,
        Locks::default()
    ));
    let expected_2 = targets(Mode::Gray)
        .iter()
        .position(|mode_2| *mode_2 == Mode::GrayAlpha)
        .expect("gris con alfa es un destino");
    assert_eq!(
        modal.dialog().expect("abierto").chosen(FIELD_TARGET),
        Some(expected_2)
    );
}

#[test]
fn row_11_indexed_without_transparent_warns_without_offering_and_with_transparent_creates() {
    let palette =
        Palette::from_colors([Rgba::opaque(0, 0, 0), Rgba::opaque(9, 9, 9)]).expect("paleta");
    let without = Image::new_indexed(4, 4, palette.clone()).expect("imagen");
    let mut session_2 = Session::sample(Document::new(without));
    let copy = session_2.document.clone();
    let report = session_2.new_layer();
    assert!(!report.changed && !report.offer_convert);
    assert!(report.notice.expect("aviso").contains("transparente"));
    assert_eq!(session_2.document, copy);
    let mut with = palette;
    with.set_transparent_index(Some(1)).expect("existe");
    let mut session_2 = Session::sample(Document::new(
        Image::new_indexed(4, 4, with).expect("imagen"),
    ));
    assert!(session_2.new_layer().changed);
}

// ---------------------------------------------------------------------------------------------
// Fila 12 (parte de `px-app`): el cuentagotas con Mayús
// ---------------------------------------------------------------------------------------------

fn chooses(session_2: &mut Session, shift: bool, point: (i32, i32)) -> Vec<Picked> {
    let mut tools = ToolState::new();
    tools.set_dropper_source(dropper_source_for(shift));
    let cel = session_2.active_cel();
    tools.apply_action(ToolAction::Select(Tool::Dropper));
    for event_2 in [CanvasEvent::StrokeBegin(point), CanvasEvent::StrokeEnd] {
        tools
            .canvas_event_with(&mut session_2.document, cel, None, event_2)
            .expect("cuentagotas");
    }
    tools
        .take_events()
        .into_iter()
        .filter_map(|event_2| match event_2 {
            ToolEvent::Picked(picked) => Some(picked),
            ToolEvent::Refused(_) => None,
        })
        .collect()
}

#[test]
fn row_12_with_shift_the_dropper_reads_the_active_layer_and_without_it_what_is_visible() {
    assert_eq!(dropper_source_for(true), DropperSource::Layer);
    assert_eq!(dropper_source_for(false), DropperSource::Visible);
    // Fondo (capa 1), un píxel rojo en la capa 2 activa y un azul opaco encima en la 3.
    let (mut session_2, layers_2) = with_three_layers();
    paints(&mut session_2, &[(2, 2)], TEST_RED);
    session_2.set_active_layer(layers_2[2]).expect("existe");
    paints(&mut session_2, &[(2, 2)], TEST_BLUE);
    session_2.set_active_layer(layers_2[1]).expect("existe");
    assert_eq!(
        chooses(&mut session_2, false, (2, 2)),
        vec![Picked::Color(TEST_BLUE)],
        "lo que se ve"
    );
    assert_eq!(
        chooses(&mut session_2, true, (2, 2)),
        vec![Picked::Color(TEST_RED)],
        "la capa activa"
    );
    // Sobre un píxel sin nada en la activa: transparente con Mayús, el fondo sin ella.
    assert_eq!(
        chooses(&mut session_2, false, (5, 5)),
        vec![Picked::Color(TEST_BACKGROUND)]
    );
    assert_eq!(
        chooses(&mut session_2, true, (5, 5)),
        vec![Picked::Color(Rgba::TRANSPARENT)]
    );
}

#[test]
fn row_12_the_dropper_is_not_denied_by_a_locked_or_hidden_layer() {
    let (mut session_2, layers_2) = with_three_layers();
    session_2.set_layer_locked(layers_2[1], true);
    session_2.set_layer_visible(layers_2[1], false);
    assert!(session_2.paint_refusal(Tool::Dropper).is_none());
    assert_eq!(chooses(&mut session_2, true, (0, 0)).len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3: la activa es siempre válida y solo ella se pinta
// ---------------------------------------------------------------------------------------------

fn cels(session_2: &Session) -> Vec<(ImageId, Option<Image>)> {
    session_2
        .document
        .layers()
        .iter()
        .map(|c| (c.cel(), session_2.document.image(c.cel()).cloned()))
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_3_the_active_one_is_valid_follows_the_entry_and_only_it_is_painted(
        steps_2 in prop::collection::vec((0u8..12, any::<u32>(), 0i32..16, 0i32..12), 1..24),
    ) {
        let mut session_2 = session_rgba(16, 12);
        let mut tools = ToolState::new();
        for (action, chance, x, y) in steps_2 {
            let antes_ids: BTreeSet<LayerId> = ids(&session_2).into_iter().collect();
            let before_cels = cels(&session_2);
            let layer_2 = {
                let v = ids(&session_2);
                v[chance as usize % v.len()]
            };
            let (undoes_or_redoes, pinto) = match action {
                0 | 1 => { session_2.new_layer(); (false, false) }
                2 => { session_2.duplicate_layer(); (false, false) }
                3 => { session_2.remove_layer(); (false, false) }
                4 => { session_2.move_layer(1); (false, false) }
                5 => { session_2.move_layer(-1); (false, false) }
                6 => { let v = session_2.document.layer(layer_2).is_some_and(|c| c.locked()); session_2.set_layer_locked(layer_2, !v); (false, false) }
                7 => { let v = session_2.document.layer(layer_2).is_some_and(|c| c.visible()); session_2.set_layer_visible(layer_2, !v); (false, false) }
                8 => { let _ = session_2.set_active_layer(layer_2); (false, false) }
                9 => { let _ = session_2.undo(); (true, false) }
                10 => { let _ = session_2.redo(); (true, false) }
                _ => {
                    let active_2 = session_2.active_layer();
                    let layer_2 = session_2.document.layer(active_2).expect("activa");
                    let can = layer_2.visible() && !layer_2.locked();
                    let change = gesture(&mut session_2, &mut tools, Tool::Pencil, &[(x, y)]);
                    prop_assert!(!change || can, "pintó una capa oculta o bloqueada");
                    (false, true)
                }
            };
            let now: BTreeSet<LayerId> = ids(&session_2).into_iter().collect();
            // La activa es siempre una capa del documento.
            prop_assert!(now.contains(&session_2.active_layer()));
            // Sigue a la entrada: si deshacer o rehacer añade exactamente una capa, es la activa.
            if undoes_or_redoes {
                let new_2_2: Vec<_> = now.difference(&antes_ids).collect();
                if new_2_2.len() == 1 {
                    prop_assert_eq!(*new_2_2[0], session_2.active_layer());
                }
            }
            // Un gesto solo cambia la celda activa: las demás son iguales a antes.
            if pinto {
                let active_cel = session_2.active_cel();
                for ((cel, antes), (cel2, now)) in before_cels.iter().zip(cels(&session_2)) {
                    prop_assert_eq!(*cel, cel2);
                    if *cel != active_cel {
                        prop_assert_eq!(antes, &now);
                    }
                }
            }
        }
    }
}
