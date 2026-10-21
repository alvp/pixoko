//! La sesión con capas (specs/hito-3-capas-vista.md, unidad 3.3): filas 1, 2, 6, 10, 11 y 12 y
//! propiedades 3, 4 y 5. Sin ventana: `Session`, `ToolState` con eventos a mano y ficheros en un
//! directorio temporal propio.
//!
//! Contrato de API que estas pruebas fijan (la lógica debe ofrecerlo):
//! - `Session`: `active_layer`, `active_cel`, `set_active_layer`, `apply_layer`, `paint_refusal`,
//!   `convert_refusal`, `layers_info`, `removal_needs_destination`, `begin_removal_for`,
//!   `mark_unmodified`; `ClearPageError::HiddenLayer`; `with_flatten_notice`.
//! - `crate::save_wait` (`SAVING_NOTICE`, `SAVE_SYNC_PIXELS`, `compose_pixels`, `start_save`,
//!   `StartSave`, `PendingSave::tick`) y `crate::debug_layers` (`ENV_LAYERS`, `LayersRequest`,
//!   `parse_layers`, `apply_debug_layers`).
//! - `TitleInfo.layers` pinta ` · capa {posición}/{total}` tras las coordenadas.

use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;
use px_core::{BlendMode, ImageId, LayerId, Palette, PixelEdit, Rgba};
use px_formats::read_png;
use px_ui::canvas::CanvasEvent;
use px_ui::dirty::BlockSet;
use px_ui::tool::{Tool, ToolAction, ToolState};
use px_ui::view::Zoom;

use crate::debug_layers::{LayersRequest, apply_debug_layers, parse_layers};
use crate::save_wait::{
    PendingSave, SAVE_SYNC_PIXELS, SAVING_NOTICE, StartSave, compose_pixels, start_save,
};
use crate::title::{TitleInfo, title_text};

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

static CONTADOR: AtomicU64 = AtomicU64::new(0);

struct Tmp {
    ruta: PathBuf,
}

impl Tmp {
    fn nuevo() -> Tmp {
        let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
        let ruta =
            std::env::temp_dir().join(format!("pixoko-px-app-capas-{}-{n}", std::process::id()));
        fs::create_dir_all(&ruta).expect("directorio temporal");
        Tmp { ruta }
    }

    fn ruta(&self) -> &Path {
        &self.ruta
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.ruta);
    }
}

const ROJO: Rgba = Rgba::opaque(255, 0, 0);
const VERDE: Rgba = Rgba::opaque(0, 255, 0);
const AZUL: Rgba = Rgba::opaque(0, 0, 255);
const FONDO: Rgba = Rgba::opaque(10, 20, 30);
const OCULTA: &str = "la capa activa está oculta";

fn rgba_lleno(ancho: u32, alto: u32, color: Rgba) -> Image {
    let mut imagen = Image::new(ancho, alto, Mode::Rgba).expect("imagen");
    for y in 0..alto {
        for x in 0..ancho {
            imagen.set_pixel(x, y, Pixel::Rgba(color)).expect("dentro");
        }
    }
    imagen
}

fn sesion_rgba(ancho: u32, alto: u32) -> Session {
    Session::sample(Document::new(rgba_lleno(ancho, alto, FONDO)))
}

/// Añade una capa encima con `apply_layer` y devuelve su `LayerId` y su celda.
fn anade(sesion: &mut Session, nombre: &str) -> (LayerId, ImageId) {
    let at = sesion.document.layers().len();
    sesion
        .apply_layer(Command::AddLayer {
            name: nombre.to_string(),
            at,
        })
        .expect("añadir capa");
    let capa = &sesion.document.layers()[at];
    (capa.id(), capa.cel())
}

fn pinta(documento: &mut Document, celda: ImageId, puntos: &[(u32, u32)], pixel: Pixel) {
    documento
        .apply(Command::SetPixels {
            image: celda,
            pixels: puntos
                .iter()
                .map(|&(x, y)| PixelEdit { x, y, pixel })
                .collect(),
        })
        .expect("pintar");
}

/// Un gesto de `herramienta` con el color de primer plano sobre la capa activa, como la app:
/// primero la guardia (`paint_refusal`), después los eventos y el registro de lo cambiado.
fn gesto(
    sesion: &mut Session,
    tools: &mut ToolState,
    herramienta: Tool,
    puntos: &[(i32, i32)],
) -> Result<bool, DocumentError> {
    if sesion.paint_refusal(herramienta).is_some() {
        return Ok(false);
    }
    let celda = sesion.active_cel();
    let pixel = sesion.pixel_of(Role::Foreground);
    tools.apply_action(ToolAction::Select(herramienta));
    let mut cambio = false;
    let mut eventos = vec![CanvasEvent::StrokeBegin(puntos[0])];
    eventos.extend(puntos[1..].iter().map(|&p| CanvasEvent::StrokeMove(p)));
    eventos.push(CanvasEvent::StrokeEnd);
    let mut resultado = Ok(());
    for evento in eventos {
        match tools.canvas_event_with(&mut sesion.document, celda, pixel, evento) {
            Ok(c) => cambio |= c,
            Err(error) => {
                resultado = Err(error);
                break;
            }
        }
    }
    sesion.absorb(tools.take_changes());
    resultado.map(|()| cambio)
}

// ---------------------------------------------------------------------------------------------
// Fila 1: la capa activa
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_1_por_defecto_la_activa_es_la_capa_0_y_su_celda() {
    let mut sesion = sesion_rgba(8, 6);
    assert_eq!(sesion.active_layer(), sesion.document.layers()[0].id());
    assert_eq!(sesion.active_cel(), sesion.document.layers()[0].cel());
    let (capa_c, celda_c) = {
        anade(&mut sesion, "B");
        anade(&mut sesion, "C")
    };
    sesion.set_active_layer(capa_c).expect("existe");
    assert_eq!(sesion.active_cel(), celda_c);
    assert_ne!(sesion.active_cel(), ImageId::from_raw(0));
}

#[test]
fn fila_1_cambiar_de_capa_no_toca_el_documento_ni_la_historia_ni_lo_modificado() {
    let mut sesion = sesion_rgba(8, 6);
    let (capa_b, _) = anade(&mut sesion, "B");
    sesion.mark_unmodified();
    let (estado, copia) = (sesion.document.state_id(), sesion.document.clone());
    let (deshacer, rehacer) = (sesion.document.can_undo(), sesion.document.can_redo());
    sesion.set_active_layer(capa_b).expect("existe");
    let desconocida = LayerId::from_raw(4242);
    assert_eq!(
        sesion.set_active_layer(desconocida),
        Err(DocumentError::UnknownLayer(desconocida))
    );
    assert_eq!(
        sesion.active_layer(),
        capa_b,
        "un error no cambia la activa"
    );
    assert_eq!(sesion.document.state_id(), estado);
    assert_eq!(sesion.document, copia);
    assert_eq!(
        (sesion.document.can_undo(), sesion.document.can_redo()),
        (deshacer, rehacer)
    );
    assert!(!sesion.modified());
}

#[test]
fn fila_1_abrir_o_crear_un_documento_reinicia_la_activa() {
    let mut sesion = sesion_rgba(8, 6);
    let (capa_b, _) = anade(&mut sesion, "B");
    sesion.set_active_layer(capa_b).expect("existe");
    sesion
        .new_image(rgba_lleno(4, 4, FONDO), true)
        .expect("nueva imagen");
    assert_eq!(sesion.active_layer(), sesion.document.layers()[0].id());
    assert_eq!(sesion.document.layers().len(), 1);
}

#[test]
fn fila_1_si_la_activa_desaparece_pasa_a_la_vecina() {
    // [A, B, C] con B activa: al quitarla pasa a la que ocupa su posición (C).
    let mut sesion = sesion_rgba(8, 6);
    let (capa_b, _) = anade(&mut sesion, "B");
    let (capa_c, _) = anade(&mut sesion, "C");
    sesion.set_active_layer(capa_b).expect("existe");
    sesion
        .apply_layer(Command::RemoveLayer { layer: capa_b })
        .expect("quitar");
    assert_eq!(sesion.active_layer(), capa_c);
    // La última activa: pasa a la nueva última.
    sesion.set_active_layer(capa_c).expect("existe");
    sesion
        .apply_layer(Command::RemoveLayer { layer: capa_c })
        .expect("quitar");
    assert_eq!(sesion.active_layer(), sesion.document.layers()[0].id());
    // La de abajo con otra encima: la que queda en su posición.
    let (capa_d, _) = anade(&mut sesion, "D");
    let abajo = sesion.document.layers()[0].id();
    sesion.set_active_layer(abajo).expect("existe");
    sesion
        .apply_layer(Command::RemoveLayer { layer: abajo })
        .expect("quitar");
    assert_eq!(sesion.active_layer(), capa_d);
}

#[test]
fn fila_1_deshacer_el_añadir_de_la_capa_activa_pasa_a_la_ultima() {
    let mut sesion = sesion_rgba(8, 6);
    let (capa_b, _) = anade(&mut sesion, "B");
    sesion.set_active_layer(capa_b).expect("existe");
    sesion.undo().expect("deshacer");
    assert_eq!(sesion.document.layers().len(), 1);
    assert_eq!(sesion.active_layer(), sesion.document.layers()[0].id());
}

// ---------------------------------------------------------------------------------------------
// Fila 2: pintar sobre la celda activa
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_2_el_lapiz_sobre_una_celda_vacia_la_materializa_en_un_paso_y_deshacer_la_vacia() {
    let mut sesion = sesion_rgba(8, 6);
    let (_, celda_b) = anade(&mut sesion, "B");
    let (capa_c, _) = anade(&mut sesion, "C");
    let capa_b = sesion.document.layers()[1].id();
    let _ = capa_c;
    sesion.set_active_layer(capa_b).expect("existe");
    sesion.slots.get_mut(Mode::Rgba).set(1, ROJO);
    sesion.choice = ColorChoice::Slot(1);
    let antes = sesion.document.clone();
    let celda_a = sesion.document.layers()[0].cel();
    let mut tools = ToolState::new();
    assert_eq!(
        gesto(&mut sesion, &mut tools, Tool::Pencil, &[(1, 1), (4, 1)]),
        Ok(true)
    );
    let imagen = sesion.document.image(celda_b).expect("materializada");
    assert_eq!(imagen.get_pixel(3, 1), Ok(Pixel::Rgba(ROJO)));
    assert_eq!(sesion.document.image(celda_a), antes.image(celda_a));
    sesion.undo().expect("un solo paso");
    assert_eq!(sesion.document, antes);
    assert!(sesion.document.image(celda_b).is_none());
    sesion.redo().expect("rehacer");
    assert!(sesion.document.image(celda_b).is_some());
}

#[test]
fn fila_2_sin_memoria_para_la_celda_el_documento_queda_intacto() {
    let mut sesion = sesion_rgba(8, 6);
    anade(&mut sesion, "B");
    let capa_b = sesion.document.layers()[1].id();
    sesion.set_active_layer(capa_b).expect("existe");
    sesion.slots.get_mut(Mode::Rgba).set(1, ROJO);
    sesion.choice = ColorChoice::Slot(1);
    let limite = sesion.document.cel_bytes();
    sesion.document.set_cells_limit(limite);
    let antes = sesion.document.clone();
    let mut tools = ToolState::new();
    let resultado = gesto(&mut sesion, &mut tools, Tool::Pencil, &[(1, 1), (4, 1)]);
    assert!(
        matches!(resultado, Err(DocumentError::CelsTooLarge { .. })),
        "{resultado:?}"
    );
    assert_eq!(sesion.document, antes);
    assert_eq!(sesion.document.state_id(), antes.state_id());
    assert!(!sesion.document.can_undo() || antes.can_undo());
}

#[test]
fn fila_2_borrar_la_pagina_actua_sobre_la_activa() {
    let mut sesion = sesion_rgba(6, 4);
    let (_, celda_b) = anade(&mut sesion, "B");
    let capa_b = sesion.document.layers()[1].id();
    sesion.set_active_layer(capa_b).expect("existe");
    // Con el fondo transparente por defecto sobre una celda vacía no hay nada que cambiar.
    let antes = sesion.document.clone();
    assert_eq!(sesion.clear_page(), Ok(false));
    assert_eq!(sesion.document, antes);
    assert!(sesion.document.image(celda_b).is_none());
    // Con fondo rojo la rellena (se materializa) en un paso.
    sesion.slots.get_mut(Mode::Rgba).set(3, ROJO);
    sesion.background = ColorChoice::Slot(3);
    assert_eq!(sesion.clear_page(), Ok(true));
    assert_eq!(
        sesion
            .document
            .image(celda_b)
            .expect("materializada")
            .get_pixel(5, 3),
        Ok(Pixel::Rgba(ROJO))
    );
    sesion.undo().expect("un paso");
    assert_eq!(sesion.document, antes);
}

// ---------------------------------------------------------------------------------------------
// Fila 6: la capa activa oculta no se pinta
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_6_con_la_activa_oculta_nada_pinta_y_el_cuentagotas_si_funciona() {
    let mut sesion = sesion_rgba(8, 6);
    let (capa_b, _) = anade(&mut sesion, "B");
    sesion.set_active_layer(capa_b).expect("existe");
    sesion
        .apply_layer(Command::SetLayerVisible {
            layer: capa_b,
            visible: false,
        })
        .expect("ocultar");
    for herramienta in [Tool::Pencil, Tool::Line, Tool::Fill, Tool::Eraser] {
        assert_eq!(
            sesion.paint_refusal(herramienta),
            Some(OCULTA),
            "{herramienta:?}"
        );
    }
    assert_eq!(sesion.paint_refusal(Tool::Dropper), None);
    let antes = sesion.document.clone();
    let mut tools = ToolState::new();
    assert_eq!(
        gesto(&mut sesion, &mut tools, Tool::Pencil, &[(1, 1), (3, 1)]),
        Ok(false)
    );
    sesion.slots.get_mut(Mode::Rgba).set(3, ROJO);
    sesion.background = ColorChoice::Slot(3);
    assert_eq!(sesion.clear_page(), Err(ClearPageError::HiddenLayer));
    assert_eq!(ClearPageError::HiddenLayer.to_string(), OCULTA);
    assert_eq!(sesion.document, antes);
    assert_eq!(sesion.document.state_id(), antes.state_id());
    // Al mostrarla se vuelve a pintar.
    sesion
        .apply_layer(Command::SetLayerVisible {
            layer: capa_b,
            visible: true,
        })
        .expect("mostrar");
    assert_eq!(sesion.paint_refusal(Tool::Pencil), None);
    assert_eq!(sesion.clear_page(), Ok(true));
}

// ---------------------------------------------------------------------------------------------
// Fila 8 (sesión): `apply_layer` anota la región sucia
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_8_apply_layer_anota_la_region_sucia_y_deshacer_la_invalida() {
    let mut sesion = sesion_rgba(70, 70);
    let _ = sesion.take_dirty();
    let (capa_b, _) = anade(&mut sesion, "B");
    assert!(
        sesion.take_dirty().is_empty(),
        "añadir una capa vacía no cambia lo que se ve"
    );
    sesion
        .apply_layer(Command::RenameLayer {
            layer: capa_b,
            name: "Z".to_string(),
        })
        .expect("renombrar");
    assert!(sesion.take_dirty().is_empty());
    sesion
        .apply_layer(Command::SetLayerVisible {
            layer: capa_b,
            visible: false,
        })
        .expect("ocultar");
    assert_eq!(sesion.take_dirty(), BlockSet::full(70, 70));
    // Un comando sin cambios no ensucia ni deja paso.
    let estado = sesion.document.state_id();
    assert_eq!(
        sesion.apply_layer(Command::SetLayerVisible {
            layer: capa_b,
            visible: false
        }),
        Ok(false)
    );
    assert_eq!(sesion.document.state_id(), estado);
    // Deshacer lo que ensució todo lo vuelve a ensuciar todo.
    sesion.undo().expect("deshacer");
    assert_eq!(sesion.take_dirty(), BlockSet::full(70, 70));
    sesion.redo().expect("rehacer");
    assert_eq!(sesion.take_dirty(), BlockSet::full(70, 70));
}

// ---------------------------------------------------------------------------------------------
// Fila 10: indexado, paleta y conversión con varias capas
// ---------------------------------------------------------------------------------------------

fn paleta_de_seis() -> Palette {
    let mut paleta = Palette::from_colors([
        Rgba::opaque(0, 0, 0),
        Rgba::opaque(200, 20, 20),
        Rgba::opaque(20, 200, 20),
        Rgba::opaque(20, 20, 200),
        Rgba::opaque(200, 200, 20),
        Rgba::opaque(255, 255, 255),
    ])
    .expect("paleta");
    paleta.set_transparent_index(Some(5)).expect("existe");
    paleta
}

fn indexada_de(ancho: u32, alto: u32, indice: u8) -> Image {
    let mut imagen = Image::new_indexed(ancho, alto, paleta_de_seis()).expect("imagen");
    for y in 0..alto {
        for x in 0..ancho {
            imagen
                .set_pixel(x, y, Pixel::Indexed(indice))
                .expect("dentro");
        }
    }
    imagen
}

#[test]
fn fila_10_con_la_celda_activa_vacia_el_color_sale_del_lienzo() {
    let imagen = indexada_de(6, 4, 1);
    let uno = Session::sample(Document::new(imagen.clone()));
    let mut dos = Session::sample(Document::new(imagen));
    anade(&mut dos, "B");
    let capa_b = dos.document.layers()[1].id();
    dos.set_active_layer(capa_b).expect("existe");
    assert!(dos.document.image(dos.active_cel()).is_none());
    assert_eq!(dos.choice, uno.choice);
    assert_eq!(dos.background, uno.background);
    assert_eq!(
        dos.pixel_of(Role::Foreground),
        uno.pixel_of(Role::Foreground)
    );
    assert_eq!(
        dos.pixel_of(Role::Background),
        uno.pixel_of(Role::Background)
    );
    let mut uno = uno;
    uno.cycle_choice(1);
    dos.cycle_choice(1);
    assert_eq!(dos.choice, uno.choice);
    dos.sync_editor();
    uno.sync_editor();
    assert_eq!(dos.editor.pending_target(), uno.editor.pending_target());
}

#[test]
fn fila_10_quitar_una_entrada_que_usa_otra_celda_pide_destino() {
    let mut base = indexada_de(6, 4, 1);
    base.set_pixel(2, 2, Pixel::Indexed(3)).expect("dentro");
    let mut sesion = Session::sample(Document::new(base));
    anade(&mut sesion, "B");
    let capa_b = sesion.document.layers()[1].id();
    sesion.set_active_layer(capa_b).expect("existe");
    assert!(sesion.document.image(sesion.active_cel()).is_none());
    // La 3 solo la usa la celda de abajo, no la activa.
    assert!(sesion.removal_needs_destination(3));
    assert!(!sesion.removal_needs_destination(4));
    sesion.choice = ColorChoice::Entry(3);
    let antes = sesion.document.clone();
    assert!(sesion.begin_removal_for(3));
    assert_eq!(sesion.ops.removal().map(|r| r.index), Some(3));
    assert_eq!(
        sesion.document, antes,
        "pedir el destino no toca el documento"
    );
    // Confirmar con destino la quita de todas las celdas en un paso.
    let salida = sesion
        .apply_palette_op(PaletteOp::Remove { index: 3, into: 1 }, false)
        .expect("quitar");
    assert!(salida.changed);
    let celda_a = sesion.document.layers()[0].cel();
    let imagen = sesion.document.image(celda_a).expect("imagen");
    assert_eq!(imagen.get_pixel(2, 2), Ok(Pixel::Indexed(1)));
    assert_eq!(sesion.document.palette().expect("paleta").len(), 5);
    sesion.undo().expect("un paso");
    assert_eq!(sesion.document, antes);
}

#[test]
fn fila_10_quitar_una_entrada_que_nadie_usa_la_quita_en_seco() {
    let mut sesion = Session::sample(Document::new(indexada_de(6, 4, 1)));
    anade(&mut sesion, "B");
    sesion.choice = ColorChoice::Entry(4);
    assert!(!sesion.removal_needs_destination(4));
    let salida = sesion
        .apply_palette_op(PaletteOp::Remove { index: 4, into: 0 }, true)
        .expect("quitar");
    assert!(salida.changed);
    assert!(sesion.ops.removal().is_none());
}

#[test]
fn fila_10_convertir_el_modo_con_varias_capas_se_rechaza_con_un_aviso_claro() {
    let mut sesion = sesion_rgba(4, 4);
    assert_eq!(sesion.convert_refusal(), None);
    anade(&mut sesion, "B");
    assert_eq!(
        sesion.convert_refusal(),
        Some(
            "no se puede convertir el modo con varias capas: deja una sola o espera a la unidad 3.5"
        )
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 11: el guardado aplanado
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_con_una_capa_se_guarda_la_celda_tal_cual() {
    let mut imagen = rgba_lleno(5, 4, FONDO);
    imagen
        .set_pixel(2, 2, Pixel::Rgba(Rgba::new(9, 8, 7, 0)))
        .expect("dentro");
    let mut sesion = Session::sample(Document::new(imagen.clone()));
    let tmp = Tmp::nuevo();
    let guardado = sesion.save_as(tmp.ruta()).expect("guardar");
    assert_eq!(
        fs::read(&guardado.path).expect("leer"),
        px_formats::write_png(&imagen).expect("PNG")
    );
}

#[test]
fn fila_11_con_tres_capas_se_guarda_lo_visible_y_el_documento_no_cambia() {
    let mut sesion = sesion_rgba(6, 4);
    let (_, celda_b) = anade(&mut sesion, "B");
    let (capa_c, celda_c) = anade(&mut sesion, "C");
    pinta(&mut sesion.document, celda_b, &[(1, 1)], Pixel::Rgba(ROJO));
    pinta(&mut sesion.document, celda_c, &[(2, 2)], Pixel::Rgba(AZUL));
    sesion
        .apply_layer(Command::SetLayerVisible {
            layer: capa_c,
            visible: false,
        })
        .expect("ocultar");
    let copia = sesion.document.clone();
    let tmp = Tmp::nuevo();
    let guardado = sesion.save_as(tmp.ruta()).expect("guardar");
    let leida = read_png(&fs::read(&guardado.path).expect("leer"))
        .expect("PNG")
        .image;
    let mut esperada = rgba_lleno(6, 4, FONDO);
    esperada.set_pixel(1, 1, Pixel::Rgba(ROJO)).expect("dentro");
    assert_eq!(leida, esperada, "la capa oculta no entra");
    assert_eq!(sesion.document, copia);
    assert!(!sesion.modified());
}

#[test]
fn fila_11_un_gris_con_todo_oculto_no_se_guarda_y_sigue_modificado() {
    let mut sesion = Session::sample(Document::new(Image::new(3, 3, Mode::Gray).expect("imagen")));
    let capa = sesion.document.layers()[0].id();
    sesion
        .apply_layer(Command::SetLayerVisible {
            layer: capa,
            visible: false,
        })
        .expect("ocultar");
    assert!(sesion.modified());
    let tmp = Tmp::nuevo();
    let resultado = sesion.save_as(tmp.ruta());
    assert!(
        matches!(resultado, Err(FileError::Write(_))),
        "{resultado:?}"
    );
    assert!(sesion.modified());
    assert_eq!(fs::read_dir(tmp.ruta()).expect("directorio").count(), 0);
}

#[test]
fn fila_11_el_aviso_dice_aplanado_solo_con_mas_de_una_capa() {
    let mut sesion = sesion_rgba(4, 4);
    let texto = || "guardado a.png".to_string();
    assert_eq!(with_flatten_notice(texto(), &sesion.document), texto());
    let (_, _) = anade(&mut sesion, "B");
    let (capa_c, _) = anade(&mut sesion, "C");
    let plano = " · se guarda aplanado; el formato con capas llega con .aseprite";
    assert_eq!(
        with_flatten_notice(texto(), &sesion.document),
        format!("{}{plano}", texto())
    );
    sesion
        .apply_layer(Command::SetLayerVisible {
            layer: capa_c,
            visible: false,
        })
        .expect("ocultar");
    assert_eq!(
        with_flatten_notice(texto(), &sesion.document),
        format!("{}{plano} · 1 capa oculta no se guarda", texto())
    );
    let capa_b = sesion.document.layers()[1].id();
    sesion
        .apply_layer(Command::SetLayerVisible {
            layer: capa_b,
            visible: false,
        })
        .expect("ocultar");
    assert_eq!(
        with_flatten_notice(texto(), &sesion.document),
        format!("{}{plano} · 2 capas ocultas no se guardan", texto())
    );
}

#[test]
fn fila_11_guardando_se_pide_dos_fotogramas_antes_si_hay_mucho_que_componer() {
    assert_eq!(SAVING_NOTICE, "Guardando…");
    assert_eq!(SAVE_SYNC_PIXELS, 1 << 24);
    // Hasta 2^24 píxeles de capa visible se guarda en el acto.
    assert_eq!(start_save(0), StartSave::Now);
    assert_eq!(start_save(SAVE_SYNC_PIXELS), StartSave::Now);
    // Con más, el aviso se ve antes: la acción llega en el segundo fotograma, una sola vez.
    let StartSave::Later(mut espera) = start_save(SAVE_SYNC_PIXELS + 1) else {
        panic!("más de 2^24 píxeles esperan");
    };
    assert!(!espera.tick(), "fotograma 1: solo el aviso");
    assert!(espera.tick(), "fotograma 2: guarda");
    assert!(!espera.tick(), "nunca más");
    assert!(!espera.tick());
    let _: Option<PendingSave> = None;
}

#[test]
fn fila_11_los_pixeles_a_componer_son_ancho_por_alto_por_capas_visibles_salvo_la_identidad() {
    let mut sesion = sesion_rgba(4, 4);
    assert_eq!(
        compose_pixels(&sesion.document),
        0,
        "una capa visible y opaca: nada que componer"
    );
    let (_, celda_b) = anade(&mut sesion, "B");
    let (capa_c, celda_c) = anade(&mut sesion, "C");
    // B vacía: no cuenta; A y C sí.
    pinta(&mut sesion.document, celda_c, &[(0, 0)], Pixel::Rgba(ROJO));
    assert_eq!(compose_pixels(&sesion.document), 4 * 4 * 2);
    pinta(&mut sesion.document, celda_b, &[(0, 0)], Pixel::Rgba(ROJO));
    assert_eq!(compose_pixels(&sesion.document), 4 * 4 * 3);
    sesion
        .apply_layer(Command::SetLayerVisible {
            layer: capa_c,
            visible: false,
        })
        .expect("ocultar");
    assert_eq!(
        compose_pixels(&sesion.document),
        4 * 4 * 2,
        "la oculta no cuenta"
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 12: título y variable de depuración
// ---------------------------------------------------------------------------------------------

fn informacion(capas: Option<(usize, usize)>) -> TitleInfo {
    TitleInfo {
        size: Some((64, 48)),
        zoom: Zoom::from_int(8).expect("zoom"),
        hover: Some((3, 4)),
        tool: Some(Tool::Pencil),
        color: Some("color 1 #000000".to_string()),
        can_undo: false,
        can_redo: false,
        error: None,
        layers: capas,
    }
}

#[test]
fn fila_12_el_titulo_lleva_la_capa_solo_con_mas_de_una() {
    assert_eq!(
        title_text(&informacion(None)),
        "Pixoko: 64×48 · ×8 · (3, 4) · lápiz · color 1 #000000"
    );
    assert_eq!(
        title_text(&informacion(Some((2, 3)))),
        "Pixoko: 64×48 · ×8 · (3, 4) · capa 2/3 · lápiz · color 1 #000000"
    );
    let mut sesion = sesion_rgba(4, 4);
    assert_eq!(sesion.layers_info(), None);
    anade(&mut sesion, "B");
    anade(&mut sesion, "C");
    let capa_b = sesion.document.layers()[1].id();
    sesion.set_active_layer(capa_b).expect("existe");
    assert_eq!(sesion.layers_info(), Some((2, 3)));
}

#[test]
fn fila_12_la_variable_pide_de_2_a_256_capas_y_una_activa_valida() {
    assert_eq!(parse_layers(None), LayersRequest::None);
    assert_eq!(parse_layers(Some("")), LayersRequest::None);
    assert_eq!(
        parse_layers(Some("3")),
        LayersRequest::Create {
            count: 3,
            active: 1
        }
    );
    assert_eq!(
        parse_layers(Some("3,2")),
        LayersRequest::Create {
            count: 3,
            active: 2
        }
    );
    assert_eq!(
        parse_layers(Some("256,256")),
        LayersRequest::Create {
            count: 256,
            active: 256
        }
    );
    for malo in [
        "3,4", "1", "0", "257", "x", "3,0", "3,", ",2", "-3", "3,2,1",
    ] {
        assert!(
            matches!(parse_layers(Some(malo)), LayersRequest::Invalid(ref aviso) if aviso.contains("PIXOKO_CAPAS")),
            "{malo:?}"
        );
    }
}

#[test]
fn fila_12_las_capas_de_depuracion_llevan_bandas_y_el_documento_queda_sin_modificar() {
    let mut sesion = sesion_rgba(64, 48);
    apply_debug_layers(&mut sesion, 3, 2).expect("crear");
    let capas = sesion.document.layers();
    assert_eq!(capas.len(), 3);
    assert_eq!(
        capas
            .iter()
            .map(|c| c.name().to_string())
            .collect::<Vec<_>>(),
        ["Capa 1", "Capa 2", "Capa 3"]
    );
    assert_eq!(sesion.active_layer(), capas[1].id());
    assert_eq!(capas[1].opacity(), 128, "las pares a 128");
    assert_eq!(capas[2].opacity(), 255);
    let celda = |i: usize| sesion.document.image(capas[i].cel()).expect("con banda");
    assert_eq!(celda(1).get_pixel(5, 5), Ok(Pixel::Rgba(ROJO)));
    assert_eq!(
        celda(1).get_pixel(20, 5),
        Ok(Pixel::Rgba(Rgba::TRANSPARENT))
    );
    assert_eq!(celda(2).get_pixel(20, 5), Ok(Pixel::Rgba(VERDE)));
    assert_eq!(celda(2).get_pixel(5, 5), Ok(Pixel::Rgba(Rgba::TRANSPARENT)));
    assert!(!sesion.modified());
    // Deshacer las quita (es una ayuda de depuración).
    while sesion.undo().is_ok() {}
    assert_eq!(sesion.document.layers().len(), 1);
    // Por defecto, la capa de abajo.
    let mut otra = sesion_rgba(64, 48);
    apply_debug_layers(&mut otra, 4, 1).expect("crear");
    assert_eq!(otra.active_layer(), otra.document.layers()[0].id());
    // Abrir otro documento y volver a aplicarla.
    otra.new_image(rgba_lleno(32, 32, FONDO), true)
        .expect("nueva");
    apply_debug_layers(&mut otra, 3, 3).expect("crear");
    assert_eq!(otra.document.layers().len(), 3);
    assert_eq!(otra.active_layer(), otra.document.layers()[2].id());
}

#[test]
fn fila_12_en_un_modo_sin_transparencia_avisa_y_deja_el_documento_intacto() {
    let mut sesion = Session::sample(Document::new(Image::new(4, 4, Mode::Gray).expect("imagen")));
    let copia = sesion.document.clone();
    let aviso = apply_debug_layers(&mut sesion, 3, 1).expect_err("gris no admite capas");
    assert!(!aviso.is_empty());
    assert_eq!(sesion.document, copia);
    assert!(!sesion.document.can_undo());
    assert!(!sesion.modified());
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3: un gesto solo toca la capa activa y se deshace exactamente
// ---------------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn propiedad_3_un_gesto_solo_toca_la_capa_activa_y_se_deshace_exactamente(
        capas in prop::collection::vec((0u8..2, 0u8..3), 0..3),
        pasos in prop::collection::vec((0u8..7, 0u8..4, any::<u32>(), any::<u32>(), any::<u32>()), 1..8),
    ) {
        let mut sesion = sesion_rgba(12, 9);
        sesion.slots.get_mut(Mode::Rgba).set(1, ROJO);
        sesion.slots.get_mut(Mode::Rgba).set(3, VERDE);
        for (i, &(relleno, _)) in capas.iter().enumerate() {
            let (_, celda) = anade(&mut sesion, &format!("C{i}"));
            if relleno != 0 {
                pinta(&mut sesion.document, celda, &[(i as u32, 1), (2, i as u32)], Pixel::Rgba(AZUL));
            }
        }
        let mut tools = ToolState::new();
        for &(tipo, capa, a, b, c) in &pasos {
            let total = sesion.document.layers().len();
            let id = sesion.document.layers()[usize::from(capa) % total].id();
            sesion.set_active_layer(id).expect("existe");
            match tipo {
                5 => {
                    let visible = a % 2 == 0;
                    let _ = sesion.apply_layer(Command::SetLayerVisible { layer: id, visible });
                    continue;
                }
                6 => {
                    let limite = if a % 2 == 0 { sesion.document.cel_bytes() } else { usize::MAX };
                    sesion.document.set_cells_limit(limite);
                    continue;
                }
                _ => {}
            }
            let antes = sesion.document.clone();
            let celdas: Vec<ImageId> = antes.layers().iter().map(|l| l.cel()).collect();
            let activa = sesion.active_cel();
            let puntos = [
                ((a % 14) as i32 - 1, (b % 11) as i32 - 1),
                ((b % 14) as i32 - 1, (c % 11) as i32 - 1),
            ];
            let resultado = match tipo {
                0 => gesto(&mut sesion, &mut tools, Tool::Pencil, &puntos),
                1 => gesto(&mut sesion, &mut tools, Tool::Line, &puntos),
                2 => gesto(&mut sesion, &mut tools, Tool::Fill, &puntos[..1]),
                3 => gesto(&mut sesion, &mut tools, Tool::Eraser, &puntos),
                _ => sesion.clear_page().map_err(|e| match e {
                    ClearPageError::Document(error) => error,
                    ClearPageError::OutOfMemory => DocumentError::OutOfMemory,
                    ClearPageError::HiddenLayer => DocumentError::EmptyCommand,
                }),
            };
            match resultado {
                Ok(true) => {
                    for celda in celdas.iter().filter(|&&celda| celda != activa) {
                        prop_assert_eq!(sesion.document.image(*celda), antes.image(*celda));
                    }
                    let despues = sesion.document.clone();
                    sesion.undo().expect("deshacer");
                    prop_assert_eq!(&sesion.document, &antes);
                    prop_assert_eq!(sesion.document.state_id(), antes.state_id());
                    sesion.redo().expect("rehacer");
                    prop_assert_eq!(&sesion.document, &despues);
                }
                Ok(false) | Err(_) => {
                    prop_assert_eq!(&sesion.document, &antes);
                    prop_assert_eq!(sesion.document.state_id(), antes.state_id());
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 4: el PNG guardado es la composición
// ---------------------------------------------------------------------------------------------

/// El oráculo de la composición en el dominio de la prueba (capas a opacidad 255 y Normal, con
/// píxeles opacos o exactamente transparentes): gana la capa visible más alta con un píxel que
/// no es transparente. La mezcla con alfa y modos se verifica contra `compose` en `px-ui`.
fn oraculo(documento: &Document) -> Image {
    let (ancho, alto) = (documento.width(), documento.height());
    let transparente = documento.palette().and_then(|p| p.transparent_index());
    let mut salida = match documento.palette() {
        Some(paleta) => Image::new_indexed(ancho, alto, paleta.clone()).expect("imagen"),
        None => Image::new(ancho, alto, documento.mode()).expect("imagen"),
    };
    let vacio = match documento.mode() {
        Mode::Indexed => Pixel::Indexed(transparente.expect("transparente")),
        _ => Pixel::Rgba(Rgba::TRANSPARENT),
    };
    for y in 0..alto {
        for x in 0..ancho {
            let mut pixel = vacio;
            for capa in documento.layers() {
                if !capa.visible() {
                    continue;
                }
                let Some(celda) = documento.image(capa.cel()) else {
                    continue;
                };
                let valor = celda.get_pixel(x, y).expect("dentro");
                let cuenta = match valor {
                    Pixel::Rgba(color) => color.a != 0,
                    Pixel::Indexed(i) => Some(i) != transparente,
                    _ => true,
                };
                if cuenta {
                    pixel = valor;
                }
            }
            salida.set_pixel(x, y, pixel).expect("dentro");
        }
    }
    salida
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn propiedad_4_el_png_guardado_es_la_composicion(
        indexado in any::<bool>(),
        capas in prop::collection::vec((any::<bool>(), any::<u32>(), any::<u32>()), 0..4),
    ) {
        let base = if indexado { indexada_de(5, 4, 1) } else { rgba_lleno(5, 4, FONDO) };
        let mut sesion = Session::sample(Document::new(base));
        for (i, &(visible, a, b)) in capas.iter().enumerate() {
            let (id, celda) = anade(&mut sesion, &format!("C{i}"));
            if a % 3 != 0 {
                let pixel = if indexado {
                    Pixel::Indexed((a % 6) as u8)
                } else {
                    Pixel::Rgba([ROJO, VERDE, AZUL, Rgba::TRANSPARENT][(a % 4) as usize])
                };
                pinta(&mut sesion.document, celda, &[(a % 5, b % 4), ((a / 5) % 5, (b / 4) % 4)], pixel);
            }
            if !visible {
                sesion.apply_layer(Command::SetLayerVisible { layer: id, visible: false }).expect("ocultar");
            }
        }
        let copia = sesion.document.clone();
        let tmp = Tmp::nuevo();
        let guardado = sesion.save_as(tmp.ruta()).expect("guardar");
        let leida = read_png(&fs::read(&guardado.path).expect("leer")).expect("PNG").image;
        // El PNG guarda el índice transparente como alfa 0 de la entrada: se compara lo que se ve.
        let esperada = oraculo(&sesion.document);
        prop_assert_eq!((leida.width(), leida.height()), (esperada.width(), esperada.height()));
        prop_assert_eq!(px_ui::texture::to_rgba8(&leida), px_ui::texture::to_rgba8(&esperada));
        prop_assert_eq!(&sesion.document, &copia);
        prop_assert!(!sesion.modified());
        let aviso = with_flatten_notice("guardado".to_string(), &sesion.document);
        prop_assert_eq!(aviso.contains("aplanado"), sesion.document.layers().len() > 1);
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 5: ninguna celda referencia una entrada de paleta que no existe
// ---------------------------------------------------------------------------------------------

fn consistente(documento: &Document) -> Result<(), String> {
    let paleta = documento.palette().expect("indexado");
    for capa in documento.layers() {
        let Some(celda) = documento.image(capa.cel()) else {
            continue;
        };
        if celda.palette() != Some(paleta) {
            return Err("una celda tiene otra paleta".to_string());
        }
        for y in 0..celda.height() {
            for x in 0..celda.width() {
                match celda.get_pixel(x, y) {
                    Ok(Pixel::Indexed(i)) if usize::from(i) < paleta.len() => {}
                    otro => return Err(format!("({x}, {y}) de {:?}: {otro:?}", capa.id())),
                }
            }
        }
    }
    Ok(())
}

fn usa_alguna_celda(documento: &Document, indice: u8) -> bool {
    documento.layers().iter().any(|capa| {
        documento.image(capa.cel()).is_some_and(|celda| {
            (0..celda.height()).any(|y| {
                (0..celda.width()).any(|x| celda.get_pixel(x, y) == Ok(Pixel::Indexed(indice)))
            })
        })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn propiedad_5_ninguna_celda_referencia_una_entrada_que_no_existe(
        capas in prop::collection::vec((any::<bool>(), any::<u32>()), 0..4),
        pasos in prop::collection::vec((0u8..9, any::<u32>(), any::<u32>(), any::<u32>()), 0..20),
    ) {
        let mut sesion = Session::sample(Document::new(indexada_de(4, 4, 1)));
        for (i, &(contenido, a)) in capas.iter().enumerate() {
            let (_, celda) = anade(&mut sesion, &format!("C{i}"));
            if contenido {
                pinta(&mut sesion.document, celda, &[(a % 4, (a / 4) % 4)], Pixel::Indexed((a % 6) as u8));
            }
        }
        prop_assert_eq!(consistente(&sesion.document), Ok(()));
        for &(tipo, a, b, c) in &pasos {
            let largo = sesion.document.palette().expect("paleta").len();
            let total = sesion.document.layers().len();
            let id = sesion.document.layers()[(a as usize) % total].id();
            sesion.set_active_layer(id).expect("existe");
            match tipo {
                0 => { let _ = sesion.apply_palette_op(PaletteOp::Add, false); }
                1 | 2 if largo > 1 => {
                    let indice = (b as usize % largo) as u8;
                    let destino = ((usize::from(indice) + 1 + c as usize % (largo - 1)) % largo) as u8;
                    sesion.choice = ColorChoice::Entry(indice);
                    // Pedir destino ⇔ alguna celda (no solo la activa) usa la entrada.
                    prop_assert_eq!(
                        sesion.removal_needs_destination(indice),
                        usa_alguna_celda(&sesion.document, indice)
                    );
                    let _ = sesion.apply_palette_op(PaletteOp::Remove { index: indice, into: destino }, false);
                }
                3 if largo > 1 => {
                    let _ = sesion.apply_palette_op(
                        PaletteOp::Move { from: (b as usize % largo) as u8, to: (c as usize % largo) as u8 },
                        false,
                    );
                }
                4 => {
                    let inicio = (b as usize % largo) as u8;
                    let _ = sesion.apply_palette_op(
                        PaletteOp::Rotate { start: inicio, len: 1 + (c as usize % (largo - usize::from(inicio))) as u16, steps: (c % 5) as i32 - 2 },
                        false,
                    );
                }
                5 => {
                    let _ = sesion.apply_palette_op(
                        PaletteOp::SetTransparent { index: (b % 4 != 0).then(|| (c as usize % largo) as u8) },
                        false,
                    );
                }
                6 => {
                    let celda = sesion.active_cel();
                    let _ = sesion.document.apply(Command::SetPixels {
                        image: celda,
                        pixels: vec![PixelEdit { x: b % 4, y: c % 4, pixel: Pixel::Indexed((b as usize % largo) as u8) }],
                    });
                }
                7 => { let _ = sesion.undo(); }
                8 => { let _ = sesion.redo(); }
                _ => {}
            }
            sesion.normalize();
            prop_assert_eq!(consistente(&sesion.document), Ok(()), "tras el paso {}", tipo);
        }
    }
}

// Silencia el aviso de importaciones que no usa todo el módulo.
#[allow(dead_code)]
fn _usos(_: BlendMode, _: PendingSave) {}
