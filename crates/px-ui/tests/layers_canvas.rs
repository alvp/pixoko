//! Las baldosas con capas (specs/hito-3-capas-vista.md, unidad 3.3): filas 7, 8 y 9 en el lienzo
//! de egui sin ventana y propiedad 1 (la vista incremental es la composición).
//!
//! Mismo banco que `tiles_canvas.rs`, reducido: `Context::run_ui` con `RawInput` sintético, el
//! `TexturesDelta` de cada fotograma aplicado **en orden** con `common::texturas::Texturas` y la
//! textura resultante comparada con el oráculo: `esperado(compose(doc))`, esto es,
//! `Color32::from_rgba_unmultiplied` de cada píxel de la composición completa. Orden de un
//! fotograma, como en la app: `interact` → (cambios del documento con `invalidate`) →
//! `set_overlay` → `sync` con `DocSource` → `paint`.
//!
//! Geometría: imagen de 130 × 70 (bloques de 64: 3 × 2), baldosas de 64 (cada una es un bloque) y
//! una pantalla de 200 × 150 a ppp 1, donde la imagen cabe a ×1 y se ven todas las baldosas.

use std::collections::BTreeSet;

use egui::{Color32, Context, Pos2, RawInput, Rect, TextureId, vec2};
use proptest::prelude::*;
use px_core::{
    BlendMode, Command, Document, DocumentError, Image, ImageId, Mode, Pixel, PixelEdit, Rgba, Span,
};
use px_raster::compose;
use px_ui::canvas::Canvas;
use px_ui::dirty::{BlockSet, ChangeLog, dirty_of};
use px_ui::source::{DocSource, OverlayCache, overlay_pixels};
use px_ui::view::{Size, ViewOp};

mod common;
use common::contexto;
use common::texturas::Texturas;

mod tiles_support;
use tiles_support::{B, esperado, imagen_de, modelo_de_pixeles, modelo_lleno, pixel_de};

const ANCHO: u32 = 130;
const ALTO: u32 = 70;
const ROJO: Rgba = Rgba::opaque(255, 0, 0);
const AZUL: Rgba = Rgba::opaque(0, 0, 255);

fn rgba(color: Rgba) -> Pixel {
    Pixel::Rgba(color)
}

struct Banco {
    ctx: Context,
    canvas: Canvas,
    doc: Document,
    log: ChangeLog,
    texturas: Texturas,
}

impl Banco {
    fn nuevo(doc: Document) -> Banco {
        let ctx = contexto(1.0);
        let mut canvas = Canvas::with_tile_side(64);
        canvas.load(&ctx, &DocSource(&doc)).expect("load");
        let mut banco = Banco {
            ctx,
            canvas,
            doc,
            log: ChangeLog::new(),
            texturas: Texturas::new(),
        };
        for _ in 0..3 {
            banco.corre(|_, _, _, _| {});
        }
        banco
    }

    /// Un fotograma. Devuelve los píxeles subidos a las texturas del lienzo (suma de las áreas de
    /// los deltas, completos y parciales).
    fn corre<F>(&mut self, fase: F) -> usize
    where
        F: FnOnce(&Context, &mut Canvas, &mut Document, &mut ChangeLog),
    {
        let mut fase = Some(fase);
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(200.0, 150.0))),
            ..RawInput::default()
        };
        let canvas = &mut self.canvas;
        let doc = &mut self.doc;
        let log = &mut self.log;
        let ids: BTreeSet<TextureId> = canvas.texture_ids().into_iter().collect();
        let mut salida = self.ctx.run_ui(entrada, |ui| {
            canvas.interact_ordered(ui);
            if let Some(fase) = fase.take() {
                fase(ui.ctx(), canvas, doc, log);
            }
            canvas.sync(ui.ctx(), &DocSource(doc)).expect("sync");
            canvas.paint(ui);
        });
        let mut subidos = 0;
        for (id, deltas) in &salida.textures_delta.set {
            if !ids.contains(id) {
                continue;
            }
            for delta in deltas {
                let egui::ImageData::Color(imagen) = &delta.image;
                subidos += imagen.size[0] * imagen.size[1];
            }
        }
        self.texturas.aplica(&salida.textures_delta);
        salida.textures_delta.clear();
        subidos
    }

    /// `dirty_of`, `apply`, registro y aviso al lienzo: lo que hace `Session`.
    fn aplica(&mut self, comando: Command) -> (Result<bool, DocumentError>, usize) {
        let mut resultado = Ok(false);
        let subidos = self.corre(|_, canvas, doc, log| {
            let region = dirty_of(&comando, Size::new(doc.width(), doc.height()));
            resultado = doc.apply(comando);
            if let Ok(true) = resultado {
                log.record(doc.state_id(), region.clone());
                canvas.invalidate(&region);
            }
        });
        (resultado, subidos)
    }

    fn deshace(&mut self) -> usize {
        self.corre(|_, canvas, doc, log| {
            let desde = doc.state_id();
            if doc.undo().is_ok() {
                let region = log
                    .get(desde)
                    .cloned()
                    .unwrap_or_else(|| BlockSet::full(doc.width(), doc.height()));
                canvas.invalidate(&region);
            }
        })
    }

    fn rehace(&mut self) -> usize {
        self.corre(|_, canvas, doc, log| {
            if doc.redo().is_ok() {
                let region = log
                    .get(doc.state_id())
                    .cloned()
                    .unwrap_or_else(|| BlockSet::full(doc.width(), doc.height()));
                canvas.invalidate(&region);
            }
        })
    }

    /// Las diferencias entre la textura de todas las baldosas y la composición del documento.
    fn discrepancias(&self) -> Vec<(u32, u32)> {
        let Ok(compuesta) = compose(&self.doc) else {
            return Vec::new();
        };
        let oraculo = esperado(&compuesta);
        let ancho = self.doc.width() as usize;
        let ids = self.canvas.texture_ids();
        let mut fallos = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            let r = self.canvas.tile_rect(i);
            for y in r.y..r.y + r.height {
                for x in r.x..r.x + r.width {
                    let visto = self
                        .texturas
                        .texel(*id, (x - r.x) as usize, (y - r.y) as usize);
                    if visto != Some(oraculo[y as usize * ancho + x as usize]) {
                        fallos.push((x, y));
                    }
                }
            }
        }
        fallos
    }

    fn contenidos(&self) -> Vec<Vec<Color32>> {
        self.canvas
            .texture_ids()
            .iter()
            .map(|id| {
                let textura = self.texturas.get(*id).expect("textura");
                textura.texels.clone()
            })
            .collect()
    }
}

/// Píxeles de las baldosas de 64 que cubren los bloques de `modelo` (recortados a la imagen).
fn area_de(modelo: &BTreeSet<(u32, u32)>) -> usize {
    modelo
        .iter()
        .map(|&(bx, by)| {
            let ancho = (ANCHO - bx * B).min(B) as usize;
            let alto = (ALTO - by * B).min(B) as usize;
            ancho * alto
        })
        .sum()
}

fn doc_de_tres_capas() -> Document {
    let mut doc = Document::new(imagen_de(Mode::Rgba, ANCHO, ALTO, 5));
    for nombre in ["B", "C"] {
        let at = doc.layers().len();
        doc.apply(Command::AddLayer {
            name: nombre.to_string(),
            at,
        })
        .expect("capa");
    }
    let (capa_b, celda_b) = (doc.layers()[1].id(), doc.layers()[1].cel());
    let (capa_c, celda_c) = (doc.layers()[2].id(), doc.layers()[2].cel());
    let puntos: Vec<PixelEdit> = (0..ANCHO)
        .step_by(3)
        .map(|x| PixelEdit {
            x,
            y: (x * 7) % ALTO,
            pixel: rgba(Rgba::new(200, 100, 50, 200)),
        })
        .collect();
    doc.apply(Command::SetPixels {
        image: celda_b,
        pixels: puntos.clone(),
    })
    .expect("pintar");
    doc.apply(Command::SetPixels {
        image: celda_c,
        pixels: puntos
            .iter()
            .map(|e| PixelEdit {
                pixel: rgba(AZUL),
                ..*e
            })
            .collect(),
    })
    .expect("pintar");
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

fn trazo(celda: ImageId, x0: u32, y: u32, n: u32, color: Rgba) -> Command {
    Command::SetPixels {
        image: celda,
        pixels: (0..n)
            .map(|i| PixelEdit {
                x: x0 + i,
                y,
                pixel: rgba(color),
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: las baldosas muestran la composición
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_7_las_baldosas_de_varias_capas_son_la_composicion() {
    let banco = Banco::nuevo(doc_de_tres_capas());
    assert_eq!(banco.canvas.tile_count(), 6);
    assert_eq!(banco.discrepancias(), vec![]);
}

#[test]
fn fila_7_las_baldosas_de_un_indexado_con_capas_son_la_composicion() {
    let mut doc = Document::new(imagen_de(Mode::Indexed, ANCHO, ALTO, 7));
    doc.apply(Command::AddLayer {
        name: "B".to_string(),
        at: 1,
    })
    .expect("capa");
    let celda_b = doc.layers()[1].cel();
    doc.apply(Command::SetPixels {
        image: celda_b,
        pixels: (0..40)
            .map(|x| PixelEdit {
                x,
                y: 3,
                pixel: Pixel::Indexed(5),
            })
            .collect(),
    })
    .expect("pintar");
    let banco = Banco::nuevo(doc);
    assert_eq!(banco.discrepancias(), vec![]);
}

#[test]
fn fila_7_con_una_capa_las_texturas_son_las_de_hoy_y_la_transparencia_deja_ver_el_fondo() {
    let imagen = imagen_de(Mode::Rgba, ANCHO, ALTO, 4);
    let banco = Banco::nuevo(Document::new(imagen.clone()));
    let oraculo = esperado(&imagen);
    let ancho = ANCHO as usize;
    for (i, id) in banco.canvas.texture_ids().iter().enumerate() {
        let r = banco.canvas.tile_rect(i);
        for y in r.y..r.y + r.height {
            for x in r.x..r.x + r.width {
                assert_eq!(
                    banco
                        .texturas
                        .texel(*id, (x - r.x) as usize, (y - r.y) as usize),
                    Some(oraculo[y as usize * ancho + x as usize])
                );
            }
        }
    }
    // Una composición toda transparente da texels de alfa 0: se ve el damero o el fondo elegido.
    let vacio = Document::new(Image::new(ANCHO, ALTO, Mode::Rgba).expect("imagen"));
    let banco = Banco::nuevo(vacio);
    assert!(banco.contenidos().iter().flatten().all(|c| c.a() == 0));
}

// ---------------------------------------------------------------------------------------------
// Fila 8: el refresco recompone solo lo sucio
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_8_un_trazo_deshacer_y_rehacer_suben_solo_los_bloques_del_trazo() {
    let mut banco = Banco::nuevo(doc_de_tres_capas());
    let celda_b = banco.doc.layers()[1].cel();
    // 40 píxeles en la fila 10 desde x = 50: cruzan los bloques (0, 0) y (1, 0).
    let comando = trazo(celda_b, 50, 10, 40, ROJO);
    let puntos: Vec<(u32, u32)> = (50..90).map(|x| (x, 10)).collect();
    let esperado_subido = area_de(&modelo_de_pixeles(&puntos, ANCHO, ALTO));
    assert!(
        esperado_subido < (ANCHO * ALTO) as usize,
        "la prueba necesita un trazo parcial"
    );
    let (resultado, subidos) = banco.aplica(comando);
    assert_eq!(resultado, Ok(true));
    assert_eq!(subidos, esperado_subido, "solo los bloques del trazo");
    assert_eq!(banco.discrepancias(), vec![]);
    assert_eq!(banco.deshace(), esperado_subido);
    assert_eq!(banco.discrepancias(), vec![]);
    assert_eq!(banco.rehace(), esperado_subido);
    assert_eq!(banco.discrepancias(), vec![]);
    assert_eq!(banco.corre(|_, _, _, _| {}), 0, "en reposo no se sube nada");
}

#[test]
fn fila_8_añadir_y_renombrar_no_ensucian_y_los_demas_comandos_de_capa_ensucian_todo() {
    let mut banco = Banco::nuevo(doc_de_tres_capas());
    let todo = area_de(&modelo_lleno(ANCHO, ALTO));
    assert_eq!(todo, (ANCHO * ALTO) as usize);
    let capa = banco.doc.layers()[1].id();
    let (r, subidos) = banco.aplica(Command::AddLayer {
        name: "D".to_string(),
        at: 3,
    });
    assert_eq!(r, Ok(true));
    assert_eq!(subidos, 0);
    let (r, subidos) = banco.aplica(Command::RenameLayer {
        layer: capa,
        name: "Z".to_string(),
    });
    assert_eq!(r, Ok(true));
    assert_eq!(subidos, 0);
    assert_eq!(banco.discrepancias(), vec![]);
    let ordenes = [
        Command::SetLayerOpacity {
            layer: capa,
            opacity: 90,
        },
        Command::SetLayerVisible {
            layer: capa,
            visible: false,
        },
        Command::SetLayerBlend {
            layer: capa,
            blend: BlendMode::Screen,
        },
        Command::MoveLayer { layer: capa, to: 0 },
        Command::RemoveLayer { layer: capa },
    ];
    for orden in ordenes {
        let nombre = format!("{orden:?}");
        let (r, subidos) = banco.aplica(orden);
        assert_eq!(r, Ok(true), "{nombre}");
        assert_eq!(subidos, todo, "{nombre}");
        assert_eq!(banco.discrepancias(), vec![], "{nombre}");
        assert_eq!(banco.deshace(), todo, "{nombre}: deshacer");
        assert_eq!(banco.discrepancias(), vec![], "{nombre}: deshacer");
        assert_eq!(banco.rehace(), todo, "{nombre}: rehacer");
        assert_eq!(banco.discrepancias(), vec![], "{nombre}: rehacer");
    }
}

#[test]
fn fila_8_con_zoom_una_baldosa_fuera_de_la_vista_se_pone_al_dia_al_verse() {
    let mut banco = Banco::nuevo(doc_de_tres_capas());
    for _ in 0..4 {
        banco.canvas.request_view(ViewOp::ZoomIn);
    }
    banco.corre(|_, _, _, _| {});
    let celda_b = banco.doc.layers()[1].cel();
    let (r, _) = banco.aplica(trazo(celda_b, 120, 65, 8, ROJO));
    assert_eq!(r, Ok(true));
    for _ in 0..4 {
        banco.canvas.request_view(ViewOp::ZoomOut);
    }
    for _ in 0..3 {
        banco.corre(|_, _, _, _| {});
    }
    assert_eq!(banco.canvas.pending_blocks(), 0);
    assert_eq!(banco.discrepancias(), vec![]);
}

// ---------------------------------------------------------------------------------------------
// Fila 9: la superposición con capas
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_9_la_superposicion_es_la_composicion_y_al_soltar_la_textura_no_cambia() {
    let mut doc = doc_de_tres_capas();
    // C visible y a opacidad 128 encima: el trazo en B se mezcla con ella.
    let capa_c = doc.layers()[2].id();
    doc.apply(Command::SetLayerVisible {
        layer: capa_c,
        visible: true,
    })
    .expect("mostrar");
    doc.apply(Command::SetLayerOpacity {
        layer: capa_c,
        opacity: 128,
    })
    .expect("opacidad");
    let mut banco = Banco::nuevo(doc);
    let celda_b = banco.doc.layers()[1].cel();
    let comando = trazo(celda_b, 20, 21, 30, ROJO);
    let Command::SetPixels { pixels, .. } = &comando else {
        unreachable!()
    };
    let pendiente = overlay_pixels(&banco.doc, celda_b, pixels.clone());
    // Fotograma del trazo en curso: el documento no cambia.
    let antes = banco.doc.clone();
    banco.corre(|ctx, canvas, doc, _| {
        canvas.set_overlay(ctx, &DocSource(doc), &pendiente);
    });
    assert_eq!(banco.doc, antes);
    let mut aplicado = banco.doc.clone();
    aplicado.apply(comando.clone()).expect("aplicar");
    let oraculo = esperado(&compose(&aplicado).expect("compose"));
    let durante = banco.contenidos();
    let ancho = ANCHO as usize;
    for (i, id) in banco.canvas.texture_ids().iter().enumerate() {
        let r = banco.canvas.tile_rect(i);
        for y in r.y..r.y + r.height {
            for x in r.x..r.x + r.width {
                assert_eq!(
                    banco
                        .texturas
                        .texel(*id, (x - r.x) as usize, (y - r.y) as usize),
                    Some(oraculo[y as usize * ancho + x as usize]),
                    "durante el trazo, ({x}, {y})"
                );
            }
        }
    }
    // Soltar: se aplica el comando, se descarta la superposición y se sube lo sucio.
    let (r, _) = banco.aplica(comando);
    assert_eq!(r, Ok(true));
    assert_eq!(banco.canvas.overlay_blocks(), 0);
    assert_eq!(
        banco.contenidos(),
        durante,
        "al soltar no cambia ningún píxel"
    );
    assert_eq!(banco.discrepancias(), vec![]);
}

#[test]
fn fila_9_el_borrador_en_curso_muestra_lo_de_debajo() {
    let mut doc = doc_de_tres_capas();
    let celda_b = doc.layers()[1].cel();
    doc.apply(trazo(celda_b, 10, 30, 20, ROJO)).expect("pintar");
    let mut banco = Banco::nuevo(doc);
    let borrar = trazo(celda_b, 12, 30, 10, Rgba::TRANSPARENT);
    let Command::SetPixels { pixels, .. } = &borrar else {
        unreachable!()
    };
    let pendiente = overlay_pixels(&banco.doc, celda_b, pixels.clone());
    banco.corre(|ctx, canvas, doc, _| {
        canvas.set_overlay(ctx, &DocSource(doc), &pendiente);
    });
    let mut aplicado = banco.doc.clone();
    aplicado.apply(borrar).expect("aplicar");
    let oraculo = esperado(&compose(&aplicado).expect("compose"));
    let (id, r) = (banco.canvas.texture_ids()[0], banco.canvas.tile_rect(0));
    for x in 12..22u32 {
        assert_eq!(
            banco.texturas.texel(id, x as usize, 30),
            Some(oraculo[30 * ANCHO as usize + x as usize]),
            "x = {x} (baldosa en {},{})",
            r.x,
            r.y
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 1: la vista incremental es la composición
// ---------------------------------------------------------------------------------------------

type Semilla = (u8, u32, u32, u32, u32);

type Capas = Vec<(u8, u8, u8, u8)>;

fn semillas() -> impl Strategy<Value = (u8, Capas, Vec<Semilla>)> {
    (
        0u8..3,
        prop::collection::vec((0u8..4, 0u8..8, 0u8..3, 0u8..4), 0..4),
        prop::collection::vec(
            (
                0u8..12,
                any::<u32>(),
                any::<u32>(),
                any::<u32>(),
                any::<u32>(),
            ),
            0..8,
        ),
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para secuencias de gestos de pintura sobre capas activas aleatorias, de comandos de capa,
    /// de `undo` y de `redo`, tras cada paso y su `sync` los píxeles de todas las baldosas son los
    /// de `compose(doc)`; con un `SetPixels` o `FillSpans` se subió exactamente el área de los
    /// bloques anotados (y con cualquier paso, nunca más que la imagen entera).
    #[test]
    fn propiedad_1_la_vista_incremental_es_la_composicion(
        (modo, capas, pasos) in semillas(),
    ) {
        let modo = [Mode::Rgba, Mode::GrayAlpha, Mode::Indexed][usize::from(modo)];
        let mut doc = Document::new(imagen_de(modo, ANCHO, ALTO, 6));
        for (i, &(contenido, opacidad, mezcla, visible)) in capas.iter().enumerate() {
            let at = doc.layers().len();
            if doc.apply(Command::AddLayer { name: format!("C{i}"), at }).is_err() {
                continue;
            }
            let id = doc.layers()[at].id();
            let celda = doc.layers()[at].cel();
            if contenido != 0 {
                let _ = doc.apply(Command::SetPixels {
                    image: celda,
                    pixels: (0..30)
                        .map(|j| PixelEdit {
                            x: (j * 13 + u32::from(contenido)) % ANCHO,
                            y: (j * 7) % ALTO,
                            pixel: pixel_de(modo, j + u32::from(opacidad), 8),
                        })
                        .collect(),
                });
            }
            let _ = doc.apply(Command::SetLayerOpacity { layer: id, opacity: [255, 0, 128, 200, 254, 1, 77, 255][usize::from(opacidad)] });
            let _ = doc.apply(Command::SetLayerBlend { layer: id, blend: BlendMode::ALL[usize::from(mezcla) * 3 % 8] });
            let _ = doc.apply(Command::SetLayerVisible { layer: id, visible: visible != 0 });
        }
        let mut banco = Banco::nuevo(doc);
        prop_assert_eq!(banco.discrepancias(), vec![]);
        let total = (ANCHO * ALTO) as usize;
        for (n, &(tipo, a, b, c, d)) in pasos.iter().enumerate() {
            let capas = banco.doc.layers().len();
            let capa = banco.doc.layers()[a as usize % capas].clone();
            let pixel = pixel_de(modo, d, 8);
            let (subidos, exacto) = match tipo {
                0..=3 => {
                    let pixels: Vec<PixelEdit> = (0..1 + b % 25)
                        .map(|j| PixelEdit {
                            x: c.wrapping_add(j * 11) % ANCHO,
                            y: d.wrapping_add(j * 5) % ALTO,
                            pixel,
                        })
                        .collect();
                    let puntos: Vec<(u32, u32)> = pixels.iter().map(|e| (e.x, e.y)).collect();
                    let esperado = area_de(&modelo_de_pixeles(&puntos, ANCHO, ALTO));
                    let (r, subidos) = banco.aplica(Command::SetPixels { image: capa.cel(), pixels });
                    (subidos, (r == Ok(true)).then_some(esperado).or(Some(0)))
                }
                4 | 5 => {
                    let y = b % ALTO;
                    let x0 = c % ANCHO;
                    let spans = vec![Span { y, x_start: x0, x_end: x0 + 1 + d % (ANCHO - x0) }];
                    let puntos: Vec<(u32, u32)> =
                        (spans[0].x_start..spans[0].x_end).map(|x| (x, y)).collect();
                    let esperado = area_de(&modelo_de_pixeles(&puntos, ANCHO, ALTO));
                    let (r, subidos) = banco.aplica(Command::FillSpans { image: capa.cel(), spans, pixel });
                    (subidos, (r == Ok(true)).then_some(esperado).or(Some(0)))
                }
                6 => (banco.aplica(Command::SetLayerOpacity { layer: capa.id(), opacity: (b % 256) as u8 }).1, None),
                7 => (banco.aplica(Command::SetLayerVisible { layer: capa.id(), visible: b % 2 == 0 }).1, None),
                8 => (banco.aplica(Command::SetLayerBlend { layer: capa.id(), blend: BlendMode::ALL[b as usize % 8] }).1, None),
                9 => (banco.aplica(Command::MoveLayer { layer: capa.id(), to: b as usize % capas }).1, None),
                10 => (banco.deshace(), None),
                _ => (banco.rehace(), None),
            };
            prop_assert!(subidos <= total, "paso {n}: subió más que la imagen entera");
            if let Some(esperado) = exacto {
                prop_assert_eq!(subidos, esperado, "paso {}: solo los bloques anotados", n);
            }
            prop_assert_eq!(banco.discrepancias(), vec![], "paso {}", n);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// El caché de la superposición no cambia lo que se ve
// ---------------------------------------------------------------------------------------------

/// Lo que ocurre en un fotograma además de pintar la superposición.
#[derive(Clone, Copy, Debug)]
enum Accion {
    Nada,
    Deshacer,
    Rehacer,
    /// Soltar el trazo: se aplica y la superposición se descarta.
    Soltar,
}

/// Un fotograma de la app: la acción sobre el documento, los colores del trazo pendiente (con o
/// sin caché) y `set_overlay`; vacío es descartar la superposición.
fn fotograma(
    banco: &mut Banco,
    cache: Option<&mut OverlayCache>,
    activa: ImageId,
    pendiente: &[PixelEdit],
    accion: Accion,
) {
    banco.corre(|ctx, canvas, doc, log| {
        match accion {
            Accion::Nada => {}
            Accion::Deshacer => {
                let desde = doc.state_id();
                if doc.undo().is_ok() {
                    let region = log
                        .get(desde)
                        .cloned()
                        .unwrap_or_else(|| BlockSet::full(doc.width(), doc.height()));
                    canvas.invalidate(&region);
                }
            }
            Accion::Rehacer => {
                if doc.redo().is_ok() {
                    let region = log
                        .get(doc.state_id())
                        .cloned()
                        .unwrap_or_else(|| BlockSet::full(doc.width(), doc.height()));
                    canvas.invalidate(&region);
                }
            }
            Accion::Soltar => {
                let comando = Command::SetPixels {
                    image: activa,
                    pixels: pendiente.to_vec(),
                };
                let region = dirty_of(&comando, Size::new(doc.width(), doc.height()));
                if let Ok(true) = doc.apply(comando) {
                    log.record(doc.state_id(), region.clone());
                    canvas.invalidate(&region);
                }
            }
        }
        let pendiente = if matches!(accion, Accion::Soltar) {
            &[][..]
        } else {
            pendiente
        };
        let colores = match cache {
            Some(cache) => cache
                .colors(doc, activa, pendiente.iter().copied())
                .to_vec(),
            None => overlay_pixels(doc, activa, pendiente.iter().copied()),
        };
        canvas.set_overlay(ctx, &DocSource(doc), &colores);
    });
}

type PasoDeTrazo = (u8, u32, u32, u32, u32);

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Con y sin `OverlayCache`, sobre trazos aleatorios (que crecen, que cambian de forma como la
    /// goma elástica de la línea, que se sueltan, que se cancelan con `Esc`), con deshacer y
    /// rehacer a mitad de trazo, cambio de zoom y cambio de capa activa, las texturas son
    /// idénticas píxel a píxel después de cada fotograma.
    #[test]
    fn el_cache_de_la_superposicion_no_cambia_lo_que_se_ve(
        (modo, capas, _) in semillas(),
        pasos in prop::collection::vec((0u8..9, any::<u32>(), any::<u32>(), any::<u32>(), any::<u32>()), 1..14),
    ) {
        let modo = [Mode::Rgba, Mode::GrayAlpha, Mode::Indexed][usize::from(modo)];
        let mut doc = Document::new(imagen_de(modo, ANCHO, ALTO, 6));
        for (i, &(contenido, opacidad, mezcla, visible)) in capas.iter().enumerate() {
            let at = doc.layers().len();
            if doc.apply(Command::AddLayer { name: format!("C{i}"), at }).is_err() {
                continue;
            }
            let id = doc.layers()[at].id();
            let celda = doc.layers()[at].cel();
            if contenido != 0 {
                let _ = doc.apply(Command::SetPixels {
                    image: celda,
                    pixels: (0..30)
                        .map(|j| PixelEdit {
                            x: (j * 13 + u32::from(contenido)) % ANCHO,
                            y: (j * 7) % ALTO,
                            pixel: pixel_de(modo, j + u32::from(opacidad), 8),
                        })
                        .collect(),
                });
            }
            let _ = doc.apply(Command::SetLayerOpacity { layer: id, opacity: [255, 0, 128, 200, 254, 1, 77, 255][usize::from(opacidad)] });
            let _ = doc.apply(Command::SetLayerBlend { layer: id, blend: BlendMode::ALL[usize::from(mezcla) * 3 % 8] });
            let _ = doc.apply(Command::SetLayerVisible { layer: id, visible: visible != 0 });
        }
        let mut con = Banco::nuevo(doc.clone());
        let mut sin = Banco::nuevo(doc);
        let mut cache = OverlayCache::default();
        let mut pendiente: Vec<PixelEdit> = Vec::new();
        let mut activa = con.doc.layers()[0].cel();
        let generar = |n: u32, a: u32, b: u32, c: u32| -> Vec<PixelEdit> {
            (0..n)
                .map(|j| PixelEdit {
                    x: a.wrapping_add(j.wrapping_mul(11)) % ANCHO,
                    y: b.wrapping_add(j.wrapping_mul(5)) % ALTO,
                    pixel: pixel_de(modo, c.wrapping_add(j), 8),
                })
                .collect()
        };
        for (n, &(tipo, a, b, c, d)) in pasos.iter().enumerate() {
            let paso: PasoDeTrazo = (tipo, a, b, c, d);
            let mut accion = Accion::Nada;
            match paso.0 {
                // El trazo crece (el lápiz y el borrador).
                0..=2 => pendiente.extend(generar(1 + a % 40, b, c, d)),
                // Cambia de forma (la goma elástica de la línea).
                3 => pendiente = generar(1 + a % 60, b, c, d),
                // Esc: se cancela.
                4 => pendiente.clear(),
                5 => accion = Accion::Deshacer,
                6 => accion = Accion::Rehacer,
                7 => {
                    let capas = con.doc.layers().len();
                    activa = con.doc.layers()[a as usize % capas].cel();
                }
                _ => {
                    if !pendiente.is_empty() {
                        accion = Accion::Soltar;
                    }
                }
            }
            // El zoom cambia entre fotogramas en los dos lienzos a la vez.
            if b % 5 == 0 {
                let operacion = if b % 2 == 0 { ViewOp::ZoomIn } else { ViewOp::ZoomOut };
                con.canvas.request_view(operacion);
                sin.canvas.request_view(operacion);
            }
            fotograma(&mut con, Some(&mut cache), activa, &pendiente, accion);
            fotograma(&mut sin, None, activa, &pendiente, accion);
            if matches!(accion, Accion::Soltar) {
                pendiente.clear();
            }
            prop_assert_eq!(con.doc.state_id(), sin.doc.state_id());
            prop_assert!(con.contenidos() == sin.contenidos(), "paso {} ({:?}): las texturas difieren", n, accion);
        }
    }
}
