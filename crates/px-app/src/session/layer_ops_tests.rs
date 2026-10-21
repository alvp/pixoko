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
use px_ui::dialog::Modal;
use px_ui::canvas::CanvasEvent;
use px_ui::pick::Picked;
use px_ui::tool::{DropperSource, Tool, ToolAction, ToolEvent, ToolState};

use crate::confirm::{Confirm, Confirmation, Request};
use crate::convert_mode::{FIELD_TARGET, open_convert_mode_to, targets};

const ROJO: Rgba = Rgba::opaque(255, 0, 0);
const AZUL: Rgba = Rgba::opaque(0, 0, 255);
const FONDO: Rgba = Rgba::opaque(10, 20, 30);
const OCULTA: &str = "la capa activa está oculta";
const BLOQUEADA: &str = "la capa activa está bloqueada";
const OCULTA_Y_BLOQUEADA: &str = "la capa activa está oculta y bloqueada";

fn lleno(ancho: u32, alto: u32, modo: Mode, pixel: Pixel) -> Image {
    let mut imagen = Image::new(ancho, alto, modo).expect("imagen");
    for y in 0..alto {
        for x in 0..ancho {
            imagen.set_pixel(x, y, pixel).expect("dentro");
        }
    }
    imagen
}

fn sesion_rgba(ancho: u32, alto: u32) -> Session {
    Session::sample(Document::new(lleno(ancho, alto, Mode::Rgba, Pixel::Rgba(FONDO))))
}

fn ids(sesion: &Session) -> Vec<LayerId> {
    sesion.document.layers().iter().map(|c| c.id()).collect()
}

fn posicion(sesion: &Session, id: LayerId) -> usize {
    ids(sesion).iter().position(|c| *c == id).expect("la capa existe")
}

fn pinta(sesion: &mut Session, puntos: &[(u32, u32)], color: Rgba) {
    let celda = sesion.active_cel();
    sesion
        .apply_layer(Command::SetPixels {
            image: celda,
            pixels: puntos
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

/// Un gesto de `herramienta` sobre la capa activa, como la app: primero la guardia.
fn gesto(sesion: &mut Session, tools: &mut ToolState, herramienta: Tool, puntos: &[(i32, i32)]) -> bool {
    if sesion.paint_refusal(herramienta).is_some() {
        return false;
    }
    let celda = sesion.active_cel();
    let pixel = sesion.pixel_of(Role::Foreground);
    tools.apply_action(ToolAction::Select(herramienta));
    let mut cambio = false;
    let mut eventos = vec![CanvasEvent::StrokeBegin(puntos[0])];
    eventos.extend(puntos[1..].iter().map(|&p| CanvasEvent::StrokeMove(p)));
    eventos.push(CanvasEvent::StrokeEnd);
    for evento in eventos {
        cambio |= tools
            .canvas_event_with(&mut sesion.document, celda, pixel, evento)
            .unwrap_or(false);
    }
    sesion.absorb(tools.take_changes());
    cambio
}

/// Tres capas `Capa 1` a `Capa 3` con la del medio activa.
fn con_tres_capas() -> (Session, [LayerId; 3]) {
    let mut sesion = sesion_rgba(16, 12);
    for _ in 0..2 {
        assert!(sesion.new_layer().changed);
    }
    let capas = [ids(&sesion)[0], ids(&sesion)[1], ids(&sesion)[2]];
    sesion.set_active_layer(capas[1]).expect("existe");
    (sesion, capas)
}

// ---------------------------------------------------------------------------------------------
// Fila 8: la sesión y la capa activa
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_8_nueva_capa_va_encima_de_la_activa_y_pasa_a_ser_la_activa() {
    let (mut sesion, capas) = con_tres_capas();
    let antes = sesion.document.state_id();
    let informe = sesion.new_layer();
    assert!(informe.changed && informe.notice.is_none() && !informe.offer_convert);
    assert_ne!(sesion.document.state_id(), antes);
    let ahora = ids(&sesion);
    assert_eq!(ahora.len(), 4);
    assert_eq!(&ahora[..2], &capas[..2]);
    assert_eq!(ahora[3], capas[2], "la que estaba encima sigue encima");
    assert_eq!(sesion.active_layer(), ahora[2]);
    assert_eq!(sesion.document.layers()[2].name(), "Capa 4");
    assert!(sesion.document.image(sesion.active_cel()).is_none(), "vacía: no reserva");
    assert!(sesion.modified());
    // Un solo paso de historia.
    sesion.undo().expect("deshacer");
    assert_eq!(ids(&sesion), capas);
}

#[test]
fn fila_8_duplicar_copia_encima_conserva_el_bloqueo_y_la_copia_es_la_activa() {
    let (mut sesion, capas) = con_tres_capas();
    pinta(&mut sesion, &[(1, 1), (2, 2)], ROJO);
    sesion.set_layer_locked(capas[1], true);
    sesion
        .set_layer_opacity(capas[1], 128)
        .changed
        .then_some(())
        .expect("opacidad");
    let informe = sesion.duplicate_layer();
    assert!(informe.changed && informe.notice.is_none(), "{informe:?}");
    let ahora = ids(&sesion);
    assert_eq!(ahora.len(), 4);
    let copia = sesion.document.layers()[2].clone();
    assert_eq!(copia.id(), ahora[2]);
    assert_ne!(copia.id(), capas[1]);
    assert_eq!(sesion.active_layer(), copia.id(), "la copia pasa a ser la activa");
    assert!(copia.locked(), "conserva el bloqueo");
    assert_eq!(copia.opacity(), 128);
    assert_eq!(copia.name(), "Capa 2 copia");
    let original = sesion.document.image(sesion.document.layers()[1].cel()).expect("píxeles");
    let nueva = sesion.document.image(copia.cel()).expect("copia con píxeles");
    assert_eq!(original, nueva);
    // Un solo paso: deshacer quita la copia y la activa vuelve a la vecina (la original).
    sesion.undo().expect("deshacer");
    assert_eq!(ids(&sesion).len(), 3);
    assert_eq!(sesion.active_layer(), capas[1]);
    sesion.redo().expect("rehacer");
    assert_eq!(sesion.active_layer(), ahora[2], "rehacer la copia la activa");
}

#[test]
fn fila_8_duplicar_una_celda_vacia_deja_la_copia_vacia() {
    let (mut sesion, _) = con_tres_capas();
    let antes = sesion.document.materialized_bytes();
    assert!(sesion.duplicate_layer().changed);
    assert_eq!(sesion.document.materialized_bytes(), antes);
    assert!(sesion.document.image(sesion.active_cel()).is_none());
}

#[test]
fn fila_8_subir_y_bajar_mueven_un_puesto_y_la_activa_es_la_misma() {
    let (mut sesion, capas) = con_tres_capas();
    assert!(sesion.move_layer(1).changed);
    assert_eq!(posicion(&sesion, capas[1]), 2);
    assert_eq!(sesion.active_layer(), capas[1]);
    // Ya arriba: no cambia nada ni la historia.
    let id = sesion.document.state_id();
    assert!(!sesion.move_layer(1).changed);
    assert_eq!(sesion.document.state_id(), id);
    assert!(sesion.move_layer(-1).changed && sesion.move_layer(-1).changed);
    assert_eq!(posicion(&sesion, capas[1]), 0);
    assert!(!sesion.move_layer(-1).changed);
    assert_eq!(sesion.active_layer(), capas[1]);
    // Deshacer un movimiento no cambia cuál es la activa.
    sesion.undo().expect("deshacer");
    assert_eq!(sesion.active_layer(), capas[1]);
}

#[test]
fn fila_8_quitar_pasa_a_la_vecina_y_deshacer_y_rehacer_siguen_a_la_entrada() {
    let (mut sesion, capas) = con_tres_capas();
    let informe = sesion.remove_layer();
    assert!(informe.changed);
    assert_eq!(ids(&sesion), [capas[0], capas[2]]);
    assert!(capas[..1].iter().chain(&capas[2..]).any(|c| *c == sesion.active_layer()));
    // Deshacer `Quitar`: la capa que entra pasa a ser la activa.
    sesion.undo().expect("deshacer");
    assert_eq!(ids(&sesion), capas);
    assert_eq!(sesion.active_layer(), capas[1]);
    // Rehacer `Nueva capa`: la nueva es la activa; deshacerla vuelve a la vecina.
    sesion.set_active_layer(capas[0]).expect("existe");
    assert!(sesion.new_layer().changed);
    let nueva = sesion.active_layer();
    sesion.undo().expect("deshacer");
    assert_eq!(sesion.active_layer(), capas[0]);
    sesion.redo().expect("rehacer");
    assert_eq!(sesion.active_layer(), nueva);
}

#[test]
fn fila_8_con_una_sola_capa_quitar_no_cambia_nada_y_avisa() {
    let mut sesion = sesion_rgba(8, 8);
    let copia = sesion.document.clone();
    let informe = sesion.remove_layer();
    assert!(!informe.changed);
    assert!(informe.notice.expect("aviso").contains("última"));
    assert_eq!(sesion.document, copia);
    assert!(!sesion.modified());
    let nav = sesion.layer_nav();
    assert_eq!((nav.count, nav.active_pos, nav.can_add), (1, 0, true));
}

#[test]
fn fila_8_las_propiedades_son_un_paso_cada_una_y_marcan_modificado() {
    let (mut sesion, capas) = con_tres_capas();
    sesion.mark_unmodified();
    // Pasos de historia: cuántas veces se puede deshacer en una copia.
    let pasos = |s: &Session| {
        let mut copia = s.document.clone();
        let mut n = 0;
        while copia.undo().is_ok() {
            n += 1;
        }
        n
    };
    let base = pasos(&sesion);
    assert!(sesion.set_layer_visible(capas[2], false).changed);
    assert!(sesion.set_layer_locked(capas[2], true).changed);
    assert!(sesion.rename_layer(capas[2], "Cielo".to_string()).changed);
    assert!(sesion.set_layer_opacity(capas[2], 100).changed);
    assert!(sesion.set_layer_blend(capas[2], BlendMode::Multiply).changed);
    assert_eq!(pasos(&sesion), base + 5);
    assert!(sesion.modified());
    // Repetir un valor no hace nada.
    assert!(!sesion.set_layer_opacity(capas[2], 100).changed);
    assert_eq!(pasos(&sesion), base + 5);
    for _ in 0..5 {
        sesion.undo().expect("deshacer");
    }
    assert!(!sesion.modified());
}

// ---------------------------------------------------------------------------------------------
// Fila 9: capa bloqueada u oculta
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_9_bloqueada_oculta_o_ambas_ninguna_herramienta_dibuja_y_el_cuentagotas_si() {
    let (mut sesion, capas) = con_tres_capas();
    let mut tools = ToolState::new();
    let herramientas = [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Eraser];
    for (oculta, bloqueada, aviso) in [
        (false, true, BLOQUEADA),
        (true, false, OCULTA),
        (true, true, OCULTA_Y_BLOQUEADA),
    ] {
        sesion.set_layer_visible(capas[1], !oculta);
        sesion.set_layer_locked(capas[1], bloqueada);
        let copia = sesion.document.clone();
        let id = sesion.document.state_id();
        for herramienta in herramientas {
            assert_eq!(sesion.paint_refusal(herramienta), Some(aviso), "{herramienta:?}");
            assert!(!gesto(&mut sesion, &mut tools, herramienta, &[(1, 1), (5, 5)]));
        }
        assert_eq!(sesion.paint_refusal(Tool::Dropper), None);
        assert_eq!(sesion.document, copia);
        assert_eq!(sesion.document.state_id(), id);
        // `Borrar página` también se niega, con el mismo motivo.
        let error = sesion.clear_page().expect_err("negada");
        assert_eq!(error.to_string(), aviso);
        assert_eq!(sesion.document, copia);
    }
    // Al desbloquear y mostrar, se pinta.
    sesion.set_layer_visible(capas[1], true);
    sesion.set_layer_locked(capas[1], false);
    assert_eq!(sesion.paint_refusal(Tool::Pencil), None);
    assert!(gesto(&mut sesion, &mut tools, Tool::Pencil, &[(1, 1), (5, 5)]));
}

#[test]
fn fila_9_con_la_capa_bloqueada_se_puede_renombrar_cambiar_mover_duplicar_y_quitar() {
    let (mut sesion, capas) = con_tres_capas();
    sesion.set_layer_locked(capas[1], true);
    assert!(sesion.rename_layer(capas[1], "Fondo".to_string()).changed);
    assert!(sesion.set_layer_opacity(capas[1], 90).changed);
    assert!(sesion.set_layer_blend(capas[1], BlendMode::Screen).changed);
    assert!(sesion.set_layer_visible(capas[1], false).changed);
    assert!(sesion.move_layer(1).changed);
    assert!(sesion.duplicate_layer().changed);
    sesion.set_active_layer(capas[1]).expect("existe");
    assert!(sesion.remove_layer().changed);
}

#[test]
fn fila_9_un_cambio_de_paleta_afecta_tambien_a_la_capa_bloqueada() {
    let mut paleta = Palette::from_colors([
        Rgba::opaque(0, 0, 0),
        Rgba::opaque(200, 20, 20),
        Rgba::opaque(255, 255, 255),
    ])
    .expect("paleta");
    paleta.set_transparent_index(Some(2)).expect("existe");
    let mut imagen = Image::new_indexed(4, 4, paleta).expect("imagen");
    imagen.set_pixel(1, 1, Pixel::Indexed(1)).expect("dentro");
    let mut sesion = Session::sample(Document::new(imagen));
    assert!(sesion.new_layer().changed);
    let capa = ids(&sesion)[0];
    sesion.set_layer_locked(capa, true);
    let celda = sesion.document.layers()[0].cel();
    let nuevo = Rgba::opaque(1, 2, 3);
    sesion
        .apply_layer(Command::SetPaletteColor {
            image: celda,
            index: 1,
            color: nuevo,
        })
        .expect("la paleta es del documento");
    for capa in sesion.document.layers() {
        if let Some(imagen) = sesion.document.image(capa.cel()) {
            assert_eq!(imagen.palette().expect("indexada").get(1), Ok(nuevo));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 10: presupuesto
// ---------------------------------------------------------------------------------------------

const GIB: usize = 1 << 30;
const MIB: usize = 1 << 20;

#[test]
fn fila_10_el_aviso_al_crear_sale_con_el_peor_caso_por_encima_de_1_gib() {
    // 8192² RGBA: celda de 256 MiB; avisa al llegar a la quinta (peor caso 1,25 GiB).
    let celda = 256 * MIB;
    for capas in 1..=8usize {
        let aviso = create_budget_notice(capas * celda, 2 * GIB, GIB);
        assert_eq!(aviso.is_some(), capas >= 5, "{capas} capas");
    }
    let texto = create_budget_notice(5 * celda, 2 * GIB, GIB).expect("aviso");
    assert!(texto.contains("1,2 GiB") && texto.contains("2 GiB"), "{texto}");
    // 4096² (64 MiB): a la decimoséptima; 1024² (4 MiB): nunca (256 × 4 MiB = 1 GiB).
    assert!(create_budget_notice(16 * 64 * MIB, 2 * GIB, GIB).is_none());
    assert!(create_budget_notice(17 * 64 * MIB, 2 * GIB, GIB).is_some());
    assert!(create_budget_notice(256 * 4 * MIB, 2 * GIB, GIB).is_none());
}

#[test]
fn fila_10_nueva_capa_y_duplicar_avisan_al_cruzar_el_peor_caso_de_verdad() {
    // 2048² RGBA = 16 MiB: la 65.ª capa pasa de 1 GiB de peor caso.
    let mut sesion = Session::sample(Document::new(lleno(2048, 2048, Mode::Rgba, Pixel::Rgba(FONDO))));
    for n in 2..=64usize {
        let informe = sesion.new_layer();
        assert!(informe.changed && informe.notice.is_none(), "capa {n}: {informe:?}");
    }
    let informe = sesion.new_layer();
    assert!(informe.changed, "la capa se crea igualmente");
    assert!(informe.notice.expect("aviso de presupuesto").contains("GiB"));
}

#[test]
fn fila_10_el_aviso_al_pintar_sale_una_vez_por_cruce_y_no_al_deshacer_por_debajo() {
    let mut sesion = sesion_rgba(64, 64);
    let celda_bytes = sesion.document.cel_bytes();
    let umbral = 3 * celda_bytes; // «1 GiB» de la prueba: tres celdas
    assert!(sesion.memory_crossing_notice_with(umbral).is_none());
    for _ in 0..3 {
        assert!(sesion.new_layer().changed);
    }
    assert!(sesion.memory_crossing_notice_with(umbral).is_none());
    // Materializar las celdas 2 y 3 (la 1 ya lo está): al llegar a 3 celdas se cruza.
    pinta(&mut sesion, &[(1, 1)], ROJO);
    assert!(sesion.memory_crossing_notice_with(umbral).is_none(), "dos celdas");
    let capa = ids(&sesion)[2];
    sesion.set_active_layer(capa).expect("existe");
    pinta(&mut sesion, &[(2, 2)], ROJO);
    let aviso = sesion.memory_crossing_notice_with(umbral).expect("cruza");
    assert!(aviso.contains("ya ocupan"), "{aviso}");
    // No se repite sin volver a cruzar.
    assert!(sesion.memory_crossing_notice_with(umbral).is_none());
    pinta(&mut sesion, &[(3, 3)], AZUL);
    assert!(sesion.memory_crossing_notice_with(umbral).is_none());
    // Deshacer por debajo no avisa; volver a cruzar (rehacer) sí.
    sesion.undo().expect("deshacer");
    sesion.undo().expect("deshacer");
    assert!(sesion.memory_crossing_notice_with(umbral).is_none());
    sesion.redo().expect("rehacer");
    sesion.redo().expect("rehacer");
    assert!(sesion.memory_crossing_notice_with(umbral).is_some());
}

#[test]
fn fila_10_sin_sitio_y_con_256_capas_son_avisos_con_el_documento_intacto() {
    let mut sesion = sesion_rgba(64, 64);
    // `CelsTooLarge`: el tope solo deja la celda que ya existe.
    let limite = sesion.document.cel_bytes();
    sesion.document.set_cells_limit(limite);
    let copia = sesion.document.clone();
    let id = sesion.document.state_id();
    let informe = sesion.duplicate_layer();
    assert!(!informe.changed);
    assert!(informe.notice.is_some());
    assert_eq!(sesion.document, copia);
    assert_eq!(sesion.document.state_id(), id);
    sesion.document.set_cells_limit(Document::DEFAULT_CELLS_LIMIT);
    // 256 capas: `TooManyLayers` y `LayerNav::can_add` en falso.
    while sesion.document.layers().len() < Document::MAX_LAYERS {
        assert!(sesion.new_layer().changed);
    }
    let nav: LayerNav = sesion.layer_nav();
    assert!(!nav.can_add);
    let copia = sesion.document.clone();
    for informe in [sesion.new_layer(), sesion.duplicate_layer()] {
        assert!(!informe.changed);
        assert!(informe.notice.is_some());
    }
    assert_eq!(sesion.document, copia);
}

// ---------------------------------------------------------------------------------------------
// Fila 11: modos sin transparencia
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_en_gris_no_se_crea_ni_se_duplica_y_se_ofrece_pasar_a_gris_con_alfa() {
    let mut sesion = Session::sample(Document::new(Image::new(4, 4, Mode::Gray).expect("imagen")));
    let copia = sesion.document.clone();
    for informe in [sesion.new_layer(), sesion.duplicate_layer()] {
        assert!(!informe.changed);
        assert!(informe.offer_convert);
        let aviso = informe.notice.expect("aviso");
        assert!(aviso.contains("gris con alfa") && aviso.contains("repite"), "{aviso}");
    }
    assert_eq!(sesion.document, copia);
    assert!(!sesion.document.can_undo());
    assert!(!sesion.modified());
}

#[test]
fn fila_11_repetir_la_orden_en_5_s_abre_convertir_modo_con_gris_con_alfa_elegido() {
    let mut confirmacion = Confirm::new();
    assert_eq!(confirmacion.request(Request::LayersToGrayAlpha, 10.0), Confirmation::Armed);
    assert_eq!(confirmacion.request(Request::LayersToGrayAlpha, 14.0), Confirmation::Confirmed);
    assert_eq!(confirmacion.request(Request::LayersToGrayAlpha, 20.0), Confirmation::Armed);
    // A los 6 s no vale: vuelve a avisar.
    assert_eq!(confirmacion.request(Request::LayersToGrayAlpha, 26.0), Confirmation::Armed);
    // El diálogo se abre con el destino ya elegido.
    let mut modal = Modal::default();
    assert!(open_convert_mode_to(&mut modal, Mode::Gray, Mode::GrayAlpha, Locks::default()));
    let esperado = targets(Mode::Gray)
        .iter()
        .position(|modo| *modo == Mode::GrayAlpha)
        .expect("gris con alfa es un destino");
    assert_eq!(modal.dialog().expect("abierto").chosen(FIELD_TARGET), Some(esperado));
}

#[test]
fn fila_11_indexado_sin_transparente_avisa_sin_ofrecer_y_con_transparente_crea() {
    let paleta = Palette::from_colors([Rgba::opaque(0, 0, 0), Rgba::opaque(9, 9, 9)]).expect("paleta");
    let sin = Image::new_indexed(4, 4, paleta.clone()).expect("imagen");
    let mut sesion = Session::sample(Document::new(sin));
    let copia = sesion.document.clone();
    let informe = sesion.new_layer();
    assert!(!informe.changed && !informe.offer_convert);
    assert!(informe.notice.expect("aviso").contains("transparente"));
    assert_eq!(sesion.document, copia);
    let mut con = paleta;
    con.set_transparent_index(Some(1)).expect("existe");
    let mut sesion = Session::sample(Document::new(Image::new_indexed(4, 4, con).expect("imagen")));
    assert!(sesion.new_layer().changed);
}

// ---------------------------------------------------------------------------------------------
// Fila 12 (parte de `px-app`): el cuentagotas con Mayús
// ---------------------------------------------------------------------------------------------

fn elige(sesion: &mut Session, shift: bool, punto: (i32, i32)) -> Vec<Picked> {
    let mut tools = ToolState::new();
    tools.set_dropper_source(dropper_source_for(shift));
    let celda = sesion.active_cel();
    tools.apply_action(ToolAction::Select(Tool::Dropper));
    for evento in [CanvasEvent::StrokeBegin(punto), CanvasEvent::StrokeEnd] {
        tools
            .canvas_event_with(&mut sesion.document, celda, None, evento)
            .expect("cuentagotas");
    }
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
fn fila_12_con_mayus_el_cuentagotas_lee_la_capa_activa_y_sin_ella_lo_visible() {
    assert_eq!(dropper_source_for(true), DropperSource::Layer);
    assert_eq!(dropper_source_for(false), DropperSource::Visible);
    // Fondo (capa 1), un píxel rojo en la capa 2 activa y un azul opaco encima en la 3.
    let (mut sesion, capas) = con_tres_capas();
    pinta(&mut sesion, &[(2, 2)], ROJO);
    sesion.set_active_layer(capas[2]).expect("existe");
    pinta(&mut sesion, &[(2, 2)], AZUL);
    sesion.set_active_layer(capas[1]).expect("existe");
    assert_eq!(elige(&mut sesion, false, (2, 2)), vec![Picked::Color(AZUL)], "lo que se ve");
    assert_eq!(elige(&mut sesion, true, (2, 2)), vec![Picked::Color(ROJO)], "la capa activa");
    // Sobre un píxel sin nada en la activa: transparente con Mayús, el fondo sin ella.
    assert_eq!(elige(&mut sesion, false, (5, 5)), vec![Picked::Color(FONDO)]);
    assert_eq!(elige(&mut sesion, true, (5, 5)), vec![Picked::Color(Rgba::TRANSPARENT)]);
}

#[test]
fn fila_12_el_cuentagotas_no_esta_negado_por_una_capa_bloqueada_u_oculta() {
    let (mut sesion, capas) = con_tres_capas();
    sesion.set_layer_locked(capas[1], true);
    sesion.set_layer_visible(capas[1], false);
    assert!(sesion.paint_refusal(Tool::Dropper).is_none());
    assert_eq!(elige(&mut sesion, true, (0, 0)).len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3: la activa es siempre válida y solo ella se pinta
// ---------------------------------------------------------------------------------------------

fn celdas(sesion: &Session) -> Vec<(ImageId, Option<Image>)> {
    sesion
        .document
        .layers()
        .iter()
        .map(|c| (c.cel(), sesion.document.image(c.cel()).cloned()))
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn propiedad_3_la_activa_es_valida_sigue_a_la_entrada_y_solo_ella_se_pinta(
        pasos in prop::collection::vec((0u8..12, any::<u32>(), 0i32..16, 0i32..12), 1..24),
    ) {
        let mut sesion = sesion_rgba(16, 12);
        let mut tools = ToolState::new();
        for (accion, azar, x, y) in pasos {
            let antes_ids: BTreeSet<LayerId> = ids(&sesion).into_iter().collect();
            let antes_celdas = celdas(&sesion);
            let capa = {
                let v = ids(&sesion);
                v[azar as usize % v.len()]
            };
            let (deshace_o_rehace, pinto) = match accion {
                0 | 1 => { sesion.new_layer(); (false, false) }
                2 => { sesion.duplicate_layer(); (false, false) }
                3 => { sesion.remove_layer(); (false, false) }
                4 => { sesion.move_layer(1); (false, false) }
                5 => { sesion.move_layer(-1); (false, false) }
                6 => { let v = sesion.document.layer(capa).is_some_and(|c| c.locked()); sesion.set_layer_locked(capa, !v); (false, false) }
                7 => { let v = sesion.document.layer(capa).is_some_and(|c| c.visible()); sesion.set_layer_visible(capa, !v); (false, false) }
                8 => { let _ = sesion.set_active_layer(capa); (false, false) }
                9 => { let _ = sesion.undo(); (true, false) }
                10 => { let _ = sesion.redo(); (true, false) }
                _ => {
                    let activa = sesion.active_layer();
                    let capa = sesion.document.layer(activa).expect("activa");
                    let puede = capa.visible() && !capa.locked();
                    let cambio = gesto(&mut sesion, &mut tools, Tool::Pencil, &[(x, y)]);
                    prop_assert!(!cambio || puede, "pintó una capa oculta o bloqueada");
                    (false, true)
                }
            };
            let ahora: BTreeSet<LayerId> = ids(&sesion).into_iter().collect();
            // La activa es siempre una capa del documento.
            prop_assert!(ahora.contains(&sesion.active_layer()));
            // Sigue a la entrada: si deshacer o rehacer añade exactamente una capa, es la activa.
            if deshace_o_rehace {
                let nuevas: Vec<_> = ahora.difference(&antes_ids).collect();
                if nuevas.len() == 1 {
                    prop_assert_eq!(*nuevas[0], sesion.active_layer());
                }
            }
            // Un gesto solo cambia la celda activa: las demás son iguales a antes.
            if pinto {
                let activa_cel = sesion.active_cel();
                for ((cel, antes), (cel2, ahora)) in antes_celdas.iter().zip(celdas(&sesion)) {
                    prop_assert_eq!(*cel, cel2);
                    if *cel != activa_cel {
                        prop_assert_eq!(antes, &ahora);
                    }
                }
            }
        }
    }
}
