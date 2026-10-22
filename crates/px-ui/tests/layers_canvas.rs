//! Las baldosas con capas (specs/hito-3-capas-vista.md, unidad 3.3): filas 7, 8 y 9 en el lienzo
//! de egui sin ventana y propiedad 1 (la vista incremental es la composición).
//!
//! Mismo banco que `tiles_canvas.rs`, reducido: `Context::run_ui` con `RawInput` sintético, el
//! `TexturesDelta` de cada fotograma aplicado **en orden** con `common::textures::Textures` y la
//! textura resultante comparada con el oráculo: `expected_2(compose(doc))`, esto es,
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
use common::context;
use common::textures::Textures;

mod tiles_support;
use tiles_support::{B, expected_2, image_of, model_full, model_of_pixels, pixel_de};

const WIDTH: u32 = 130;
const HEIGHT: u32 = 70;
const RED: Rgba = Rgba::opaque(255, 0, 0);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);

fn rgba(color: Rgba) -> Pixel {
    Pixel::Rgba(color)
}

struct Bench {
    ctx: Context,
    canvas: Canvas,
    doc: Document,
    log: ChangeLog,
    textures_2: Textures,
}

impl Bench {
    fn new_2(doc: Document) -> Bench {
        let ctx = context(1.0);
        let mut canvas = Canvas::with_tile_side(64);
        canvas.load(&ctx, &DocSource(&doc)).expect("load");
        let mut bench = Bench {
            ctx,
            canvas,
            doc,
            log: ChangeLog::new(),
            textures_2: Textures::new(),
        };
        for _ in 0..3 {
            bench.runs(|_, _, _, _| {});
        }
        bench
    }

    /// Un fotograma. Devuelve los píxeles subidos a las texturas del lienzo (suma de las áreas de
    /// los deltas, completos y parciales).
    fn runs<F>(&mut self, phase: F) -> usize
    where
        F: FnOnce(&Context, &mut Canvas, &mut Document, &mut ChangeLog),
    {
        let mut phase = Some(phase);
        let input_2 = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(200.0, 150.0))),
            ..RawInput::default()
        };
        let canvas = &mut self.canvas;
        let doc = &mut self.doc;
        let log = &mut self.log;
        let ids: BTreeSet<TextureId> = canvas.texture_ids().into_iter().collect();
        let mut output_2 = self.ctx.run_ui(input_2, |ui| {
            canvas.interact_roles(ui);
            if let Some(phase) = phase.take() {
                phase(ui.ctx(), canvas, doc, log);
            }
            canvas.sync(ui.ctx(), &DocSource(doc)).expect("sync");
            canvas.paint(ui);
        });
        let mut uploaded = 0;
        for (id, deltas) in &output_2.textures_delta.set {
            if !ids.contains(id) {
                continue;
            }
            for delta in deltas {
                let egui::ImageData::Color(image_2) = &delta.image;
                uploaded += image_2.size[0] * image_2.size[1];
            }
        }
        self.textures_2.applies(&output_2.textures_delta);
        output_2.textures_delta.clear();
        uploaded
    }

    /// `dirty_of`, `apply`, registro y aviso al lienzo: lo que hace `Session`.
    fn applies(&mut self, command: Command) -> (Result<bool, DocumentError>, usize) {
        let mut result = Ok(false);
        let uploaded = self.runs(|_, canvas, doc, log| {
            let region = dirty_of(&command, Size::new(doc.width(), doc.height()));
            result = doc.apply(command);
            if let Ok(true) = result {
                log.record(doc.state_id(), region.clone());
                canvas.invalidate(&region);
            }
        });
        (result, uploaded)
    }

    fn undoes(&mut self) -> usize {
        self.runs(|_, canvas, doc, log| {
            let from_2 = doc.state_id();
            if doc.undo().is_ok() {
                let region = log
                    .get(from_2)
                    .cloned()
                    .unwrap_or_else(|| BlockSet::full(doc.width(), doc.height()));
                canvas.invalidate(&region);
            }
        })
    }

    fn redoes(&mut self) -> usize {
        self.runs(|_, canvas, doc, log| {
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
    fn discrepancies(&self) -> Vec<(u32, u32)> {
        let Ok(composite) = compose(&self.doc) else {
            return Vec::new();
        };
        let oracle = expected_2(&composite);
        let width_2 = self.doc.width() as usize;
        let ids = self.canvas.texture_ids();
        let mut failures = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            let r = self.canvas.tile_rect(i);
            for y in r.y..r.y + r.height {
                for x in r.x..r.x + r.width {
                    let seen = self
                        .textures_2
                        .texel(*id, (x - r.x) as usize, (y - r.y) as usize);
                    if seen != Some(oracle[y as usize * width_2 + x as usize]) {
                        failures.push((x, y));
                    }
                }
            }
        }
        failures
    }

    fn contents(&self) -> Vec<Vec<Color32>> {
        self.canvas
            .texture_ids()
            .iter()
            .map(|id| {
                let texture_2 = self.textures_2.get(*id).expect("textura");
                texture_2.texels.clone()
            })
            .collect()
    }
}

/// Píxeles de las baldosas de 64 que cubren los bloques de `model` (recortados a la imagen).
fn area_de(model: &BTreeSet<(u32, u32)>) -> usize {
    model
        .iter()
        .map(|&(bx, by)| {
            let width_2 = (WIDTH - bx * B).min(B) as usize;
            let height_2 = (HEIGHT - by * B).min(B) as usize;
            width_2 * height_2
        })
        .sum()
}

fn three_layer_doc() -> Document {
    let mut doc = Document::new(image_of(Mode::Rgba, WIDTH, HEIGHT, 5));
    for name_2 in ["B", "C"] {
        let at = doc.layers().len();
        doc.apply(Command::AddLayer {
            name: name_2.to_string(),
            at,
        })
        .expect("capa");
    }
    let (layer_b, cel_b) = (doc.layers()[1].id(), doc.layers()[1].cel());
    let (layer_c, cel_c) = (doc.layers()[2].id(), doc.layers()[2].cel());
    let points: Vec<PixelEdit> = (0..WIDTH)
        .step_by(3)
        .map(|x| PixelEdit {
            x,
            y: (x * 7) % HEIGHT,
            pixel: rgba(Rgba::new(200, 100, 50, 200)),
        })
        .collect();
    doc.apply(Command::SetPixels {
        image: cel_b,
        pixels: points.clone(),
    })
    .expect("pintar");
    doc.apply(Command::SetPixels {
        image: cel_c,
        pixels: points
            .iter()
            .map(|e| PixelEdit {
                pixel: rgba(BLUE),
                ..*e
            })
            .collect(),
    })
    .expect("pintar");
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

fn stroke_2(cel: ImageId, x0: u32, y: u32, n: u32, color: Rgba) -> Command {
    Command::SetPixels {
        image: cel,
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
fn row_7_the_tiles_of_several_layers_are_the_composition() {
    let bench = Bench::new_2(three_layer_doc());
    assert_eq!(bench.canvas.tile_count(), 6);
    assert_eq!(bench.discrepancies(), vec![]);
}

#[test]
fn row_7_the_tiles_of_an_indexed_one_with_layers_are_the_composition() {
    let mut doc = Document::new(image_of(Mode::Indexed, WIDTH, HEIGHT, 7));
    doc.apply(Command::AddLayer {
        name: "B".to_string(),
        at: 1,
    })
    .expect("capa");
    let cel_b = doc.layers()[1].cel();
    doc.apply(Command::SetPixels {
        image: cel_b,
        pixels: (0..40)
            .map(|x| PixelEdit {
                x,
                y: 3,
                pixel: Pixel::Indexed(5),
            })
            .collect(),
    })
    .expect("pintar");
    let bench = Bench::new_2(doc);
    assert_eq!(bench.discrepancies(), vec![]);
}

#[test]
fn row_7_with_one_layer_the_textures_are_todays_and_the_transparency_shows_the_background() {
    let image_2 = image_of(Mode::Rgba, WIDTH, HEIGHT, 4);
    let bench = Bench::new_2(Document::new(image_2.clone()));
    let oracle = expected_2(&image_2);
    let width_2 = WIDTH as usize;
    for (i, id) in bench.canvas.texture_ids().iter().enumerate() {
        let r = bench.canvas.tile_rect(i);
        for y in r.y..r.y + r.height {
            for x in r.x..r.x + r.width {
                assert_eq!(
                    bench
                        .textures_2
                        .texel(*id, (x - r.x) as usize, (y - r.y) as usize),
                    Some(oracle[y as usize * width_2 + x as usize])
                );
            }
        }
    }
    // Una composición toda transparente da texels de alfa 0: se ve el damero o el fondo elegido.
    let empty = Document::new(Image::new(WIDTH, HEIGHT, Mode::Rgba).expect("imagen"));
    let bench = Bench::new_2(empty);
    assert!(bench.contents().iter().flatten().all(|c| c.a() == 0));
}

// ---------------------------------------------------------------------------------------------
// Defecto del hito 3: todas las capas ocultas
// ---------------------------------------------------------------------------------------------

/// Un documento de una sola capa, oculta, en cada modo; el indexado sin índice transparente.
fn doc_with_the_only_layer_hidden(mode_2: Mode) -> Document {
    let image_2 = match mode_2 {
        Mode::Indexed => Image::new_indexed(
            WIDTH,
            HEIGHT,
            px_core::Palette::from_colors([Rgba::opaque(1, 2, 3), Rgba::opaque(4, 5, 6)])
                .expect("paleta"),
        )
        .expect("imagen"),
        _ => Image::new(WIDTH, HEIGHT, mode_2).expect("imagen"),
    };
    let mut doc = Document::new(image_2);
    let layer_2 = doc.layers()[0].id();
    doc.apply(Command::SetLayerVisible {
        layer: layer_2,
        visible: false,
    })
    .expect("ocultar");
    doc
}

#[test]
fn with_all_layers_hidden_the_canvas_loads_and_looks_transparent_in_all_modes() {
    for mode_2 in [Mode::Rgba, Mode::GrayAlpha, Mode::Indexed, Mode::Gray] {
        let doc = doc_with_the_only_layer_hidden(mode_2);
        let ctx = context(1.0);
        let mut canvas = Canvas::with_tile_side(64);
        let loaded = canvas.load(&ctx, &DocSource(&doc));
        assert_eq!(loaded, Ok(()), "{mode_2:?}: cargar el lienzo");
        let mut bench = Bench::new_2(doc);
        for _ in 0..2 {
            bench.runs(|_, _, _, _| {});
        }
        let all_transparent = bench
            .contents()
            .iter()
            .all(|tile| tile.iter().all(|&c| c == Color32::TRANSPARENT));
        assert!(
            all_transparent,
            "{mode_2:?}: la vista es el fondo del lienzo"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8: el refresco recompone solo lo sucio
// ---------------------------------------------------------------------------------------------

#[test]
fn row_8_a_stroke_undo_and_redo_upload_only_the_blocks_of_the_stroke() {
    let mut bench = Bench::new_2(three_layer_doc());
    let cel_b = bench.doc.layers()[1].cel();
    // 40 píxeles en la fila 10 desde x = 50: cruzan los bloques (0, 0) y (1, 0).
    let command = stroke_2(cel_b, 50, 10, 40, RED);
    let points: Vec<(u32, u32)> = (50..90).map(|x| (x, 10)).collect();
    let expected_uploaded = area_de(&model_of_pixels(&points, WIDTH, HEIGHT));
    assert!(
        expected_uploaded < (WIDTH * HEIGHT) as usize,
        "la prueba necesita un trazo parcial"
    );
    let (result, uploaded) = bench.applies(command);
    assert_eq!(result, Ok(true));
    assert_eq!(uploaded, expected_uploaded, "solo los bloques del trazo");
    assert_eq!(bench.discrepancies(), vec![]);
    assert_eq!(bench.undoes(), expected_uploaded);
    assert_eq!(bench.discrepancies(), vec![]);
    assert_eq!(bench.redoes(), expected_uploaded);
    assert_eq!(bench.discrepancies(), vec![]);
    assert_eq!(bench.runs(|_, _, _, _| {}), 0, "en reposo no se sube nada");
}

#[test]
fn row_8_add_and_rename_do_not_dirty_and_the_other_layer_commands_dirty_everything() {
    let mut bench = Bench::new_2(three_layer_doc());
    let all = area_de(&model_full(WIDTH, HEIGHT));
    assert_eq!(all, (WIDTH * HEIGHT) as usize);
    let layer_2 = bench.doc.layers()[1].id();
    let (r, uploaded) = bench.applies(Command::AddLayer {
        name: "D".to_string(),
        at: 3,
    });
    assert_eq!(r, Ok(true));
    assert_eq!(uploaded, 0);
    let (r, uploaded) = bench.applies(Command::RenameLayer {
        layer: layer_2,
        name: "Z".to_string(),
    });
    assert_eq!(r, Ok(true));
    assert_eq!(uploaded, 0);
    assert_eq!(bench.discrepancies(), vec![]);
    let commands = [
        Command::SetLayerOpacity {
            layer: layer_2,
            opacity: 90,
        },
        Command::SetLayerVisible {
            layer: layer_2,
            visible: false,
        },
        Command::SetLayerBlend {
            layer: layer_2,
            blend: BlendMode::Screen,
        },
        Command::MoveLayer {
            layer: layer_2,
            to: 0,
        },
        Command::RemoveLayer { layer: layer_2 },
    ];
    for order in commands {
        let name_2 = format!("{order:?}");
        let (r, uploaded) = bench.applies(order);
        assert_eq!(r, Ok(true), "{name_2}");
        assert_eq!(uploaded, all, "{name_2}");
        assert_eq!(bench.discrepancies(), vec![], "{name_2}");
        assert_eq!(bench.undoes(), all, "{name_2}: deshacer");
        assert_eq!(bench.discrepancies(), vec![], "{name_2}: deshacer");
        assert_eq!(bench.redoes(), all, "{name_2}: rehacer");
        assert_eq!(bench.discrepancies(), vec![], "{name_2}: rehacer");
    }
}

#[test]
fn row_8_with_zoom_a_tile_outside_the_view_catches_up_when_seen() {
    let mut bench = Bench::new_2(three_layer_doc());
    for _ in 0..4 {
        bench.canvas.request_view(ViewOp::ZoomIn);
    }
    bench.runs(|_, _, _, _| {});
    let cel_b = bench.doc.layers()[1].cel();
    let (r, _) = bench.applies(stroke_2(cel_b, 120, 65, 8, RED));
    assert_eq!(r, Ok(true));
    for _ in 0..4 {
        bench.canvas.request_view(ViewOp::ZoomOut);
    }
    for _ in 0..3 {
        bench.runs(|_, _, _, _| {});
    }
    assert_eq!(bench.canvas.pending_blocks(), 0);
    assert_eq!(bench.discrepancies(), vec![]);
}

// ---------------------------------------------------------------------------------------------
// Fila 9: la superposición con capas
// ---------------------------------------------------------------------------------------------

#[test]
fn row_9_the_overlay_is_the_composition_and_on_release_the_texture_does_not_change() {
    let mut doc = three_layer_doc();
    // C visible y a opacidad 128 encima: el trazo en B se mezcla con ella.
    let layer_c = doc.layers()[2].id();
    doc.apply(Command::SetLayerVisible {
        layer: layer_c,
        visible: true,
    })
    .expect("mostrar");
    doc.apply(Command::SetLayerOpacity {
        layer: layer_c,
        opacity: 128,
    })
    .expect("opacidad");
    let mut bench = Bench::new_2(doc);
    let cel_b = bench.doc.layers()[1].cel();
    let command = stroke_2(cel_b, 20, 21, 30, RED);
    let Command::SetPixels { pixels, .. } = &command else {
        unreachable!()
    };
    let pending = overlay_pixels(&bench.doc, cel_b, pixels.clone());
    // Fotograma del trazo en curso: el documento no cambia.
    let antes = bench.doc.clone();
    bench.runs(|ctx, canvas, doc, _| {
        canvas.set_overlay(ctx, &DocSource(doc), &pending);
    });
    assert_eq!(bench.doc, antes);
    let mut applied = bench.doc.clone();
    applied.apply(command.clone()).expect("aplicar");
    let oracle = expected_2(&compose(&applied).expect("compose"));
    let during = bench.contents();
    let width_2 = WIDTH as usize;
    for (i, id) in bench.canvas.texture_ids().iter().enumerate() {
        let r = bench.canvas.tile_rect(i);
        for y in r.y..r.y + r.height {
            for x in r.x..r.x + r.width {
                assert_eq!(
                    bench
                        .textures_2
                        .texel(*id, (x - r.x) as usize, (y - r.y) as usize),
                    Some(oracle[y as usize * width_2 + x as usize]),
                    "durante el trazo, ({x}, {y})"
                );
            }
        }
    }
    // Soltar: se aplica el comando, se descarta la superposición y se sube lo sucio.
    let (r, _) = bench.applies(command);
    assert_eq!(r, Ok(true));
    assert_eq!(bench.canvas.overlay_blocks(), 0);
    assert_eq!(bench.contents(), during, "al soltar no cambia ningún píxel");
    assert_eq!(bench.discrepancies(), vec![]);
}

#[test]
fn row_9_the_eraser_in_progress_shows_what_is_below() {
    let mut doc = three_layer_doc();
    let cel_b = doc.layers()[1].cel();
    doc.apply(stroke_2(cel_b, 10, 30, 20, RED)).expect("pintar");
    let mut bench = Bench::new_2(doc);
    let delete = stroke_2(cel_b, 12, 30, 10, Rgba::TRANSPARENT);
    let Command::SetPixels { pixels, .. } = &delete else {
        unreachable!()
    };
    let pending = overlay_pixels(&bench.doc, cel_b, pixels.clone());
    bench.runs(|ctx, canvas, doc, _| {
        canvas.set_overlay(ctx, &DocSource(doc), &pending);
    });
    let mut applied = bench.doc.clone();
    applied.apply(delete).expect("aplicar");
    let oracle = expected_2(&compose(&applied).expect("compose"));
    let (id, r) = (bench.canvas.texture_ids()[0], bench.canvas.tile_rect(0));
    for x in 12..22u32 {
        assert_eq!(
            bench.textures_2.texel(id, x as usize, 30),
            Some(oracle[30 * WIDTH as usize + x as usize]),
            "x = {x} (baldosa en {},{})",
            r.x,
            r.y
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 1: la vista incremental es la composición
// ---------------------------------------------------------------------------------------------

type Seed = (u8, u32, u32, u32, u32);

type Layers = Vec<(u8, u8, u8, u8)>;

fn seeds() -> impl Strategy<Value = (u8, Layers, Vec<Seed>)> {
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
    fn property_1_the_incremental_view_is_the_composition(
        (mode_2, layers, steps_2) in seeds(),
    ) {
        let mode_2 = [Mode::Rgba, Mode::GrayAlpha, Mode::Indexed][usize::from(mode_2)];
        let mut doc = Document::new(image_of(mode_2, WIDTH, HEIGHT, 6));
        for (i, &(content, opacity_2, blend_2, visible)) in layers.iter().enumerate() {
            let at = doc.layers().len();
            if doc.apply(Command::AddLayer { name: format!("C{i}"), at }).is_err() {
                continue;
            }
            let id = doc.layers()[at].id();
            let cel = doc.layers()[at].cel();
            if content != 0 {
                let _ = doc.apply(Command::SetPixels {
                    image: cel,
                    pixels: (0..30)
                        .map(|j| PixelEdit {
                            x: (j * 13 + u32::from(content)) % WIDTH,
                            y: (j * 7) % HEIGHT,
                            pixel: pixel_de(mode_2, j + u32::from(opacity_2), 8),
                        })
                        .collect(),
                });
            }
            let _ = doc.apply(Command::SetLayerOpacity { layer: id, opacity: [255, 0, 128, 200, 254, 1, 77, 255][usize::from(opacity_2)] });
            let _ = doc.apply(Command::SetLayerBlend { layer: id, blend: BlendMode::ALL[usize::from(blend_2) * 3 % 8] });
            let _ = doc.apply(Command::SetLayerVisible { layer: id, visible: visible != 0 });
        }
        let mut bench = Bench::new_2(doc);
        prop_assert_eq!(bench.discrepancies(), vec![]);
        let total = (WIDTH * HEIGHT) as usize;
        for (n, &(kind, a, b, c, d)) in steps_2.iter().enumerate() {
            let layers = bench.doc.layers().len();
            let layer_2 = bench.doc.layers()[a as usize % layers].clone();
            let pixel = pixel_de(mode_2, d, 8);
            let (uploaded, exact) = match kind {
                0..=3 => {
                    let pixels: Vec<PixelEdit> = (0..1 + b % 25)
                        .map(|j| PixelEdit {
                            x: c.wrapping_add(j * 11) % WIDTH,
                            y: d.wrapping_add(j * 5) % HEIGHT,
                            pixel,
                        })
                        .collect();
                    let points: Vec<(u32, u32)> = pixels.iter().map(|e| (e.x, e.y)).collect();
                    let expected_2 = area_de(&model_of_pixels(&points, WIDTH, HEIGHT));
                    let (r, uploaded) = bench.applies(Command::SetPixels { image: layer_2.cel(), pixels });
                    (uploaded, (r == Ok(true)).then_some(expected_2).or(Some(0)))
                }
                4 | 5 => {
                    let y = b % HEIGHT;
                    let x0 = c % WIDTH;
                    let spans = vec![Span { y, x_start: x0, x_end: x0 + 1 + d % (WIDTH - x0) }];
                    let points: Vec<(u32, u32)> =
                        (spans[0].x_start..spans[0].x_end).map(|x| (x, y)).collect();
                    let expected_2 = area_de(&model_of_pixels(&points, WIDTH, HEIGHT));
                    let (r, uploaded) = bench.applies(Command::FillSpans { image: layer_2.cel(), spans, pixel });
                    (uploaded, (r == Ok(true)).then_some(expected_2).or(Some(0)))
                }
                6 => (bench.applies(Command::SetLayerOpacity { layer: layer_2.id(), opacity: (b % 256) as u8 }).1, None),
                7 => (bench.applies(Command::SetLayerVisible { layer: layer_2.id(), visible: b % 2 == 0 }).1, None),
                8 => (bench.applies(Command::SetLayerBlend { layer: layer_2.id(), blend: BlendMode::ALL[b as usize % 8] }).1, None),
                9 => (bench.applies(Command::MoveLayer { layer: layer_2.id(), to: b as usize % layers }).1, None),
                10 => (bench.undoes(), None),
                _ => (bench.redoes(), None),
            };
            prop_assert!(uploaded <= total, "paso {n}: subió más que la imagen entera");
            if let Some(expected_2) = exact {
                prop_assert_eq!(uploaded, expected_2, "paso {}: solo los bloques anotados", n);
            }
            prop_assert_eq!(bench.discrepancies(), vec![], "paso {}", n);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// El caché de la superposición no cambia lo que se ve
// ---------------------------------------------------------------------------------------------

/// Lo que ocurre en un fotograma además de pintar la superposición.
#[derive(Clone, Copy, Debug)]
enum TestAction {
    Nothing,
    Undo,
    Redo,
    /// Soltar el trazo: se aplica y la superposición se descarta.
    Release,
}

/// Un fotograma de la app: la acción sobre el documento, los colores del trazo pendiente (con o
/// sin caché) y `set_overlay`; vacío es descartar la superposición.
fn frame(
    bench: &mut Bench,
    cache: Option<&mut OverlayCache>,
    active_2: ImageId,
    pending: &[PixelEdit],
    action_2: TestAction,
) {
    bench.runs(|ctx, canvas, doc, log| {
        match action_2 {
            TestAction::Nothing => {}
            TestAction::Undo => {
                let from_2 = doc.state_id();
                if doc.undo().is_ok() {
                    let region = log
                        .get(from_2)
                        .cloned()
                        .unwrap_or_else(|| BlockSet::full(doc.width(), doc.height()));
                    canvas.invalidate(&region);
                }
            }
            TestAction::Redo => {
                if doc.redo().is_ok() {
                    let region = log
                        .get(doc.state_id())
                        .cloned()
                        .unwrap_or_else(|| BlockSet::full(doc.width(), doc.height()));
                    canvas.invalidate(&region);
                }
            }
            TestAction::Release => {
                let command = Command::SetPixels {
                    image: active_2,
                    pixels: pending.to_vec(),
                };
                let region = dirty_of(&command, Size::new(doc.width(), doc.height()));
                if let Ok(true) = doc.apply(command) {
                    log.record(doc.state_id(), region.clone());
                    canvas.invalidate(&region);
                }
            }
        }
        let pending = if matches!(action_2, TestAction::Release) {
            &[][..]
        } else {
            pending
        };
        let colors_2 = match cache {
            Some(cache) => cache
                .colors(doc, active_2, pending.iter().copied())
                .to_vec(),
            None => overlay_pixels(doc, active_2, pending.iter().copied()),
        };
        canvas.set_overlay(ctx, &DocSource(doc), &colors_2);
    });
}

type StepOfStroke = (u8, u32, u32, u32, u32);

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Con y sin `OverlayCache`, sobre trazos aleatorios (que crecen, que cambian de forma como la
    /// goma elástica de la línea, que se sueltan, que se cancelan con `Esc`), con deshacer y
    /// rehacer a mitad de trazo, cambio de zoom y cambio de capa activa, las texturas son
    /// idénticas píxel a píxel después de cada fotograma.
    #[test]
    fn the_overlay_cache_does_not_change_what_is_seen(
        (mode_2, layers, _) in seeds(),
        steps_2 in prop::collection::vec((0u8..9, any::<u32>(), any::<u32>(), any::<u32>(), any::<u32>()), 1..14),
    ) {
        let mode_2 = [Mode::Rgba, Mode::GrayAlpha, Mode::Indexed][usize::from(mode_2)];
        let mut doc = Document::new(image_of(mode_2, WIDTH, HEIGHT, 6));
        for (i, &(content, opacity_2, blend_2, visible)) in layers.iter().enumerate() {
            let at = doc.layers().len();
            if doc.apply(Command::AddLayer { name: format!("C{i}"), at }).is_err() {
                continue;
            }
            let id = doc.layers()[at].id();
            let cel = doc.layers()[at].cel();
            if content != 0 {
                let _ = doc.apply(Command::SetPixels {
                    image: cel,
                    pixels: (0..30)
                        .map(|j| PixelEdit {
                            x: (j * 13 + u32::from(content)) % WIDTH,
                            y: (j * 7) % HEIGHT,
                            pixel: pixel_de(mode_2, j + u32::from(opacity_2), 8),
                        })
                        .collect(),
                });
            }
            let _ = doc.apply(Command::SetLayerOpacity { layer: id, opacity: [255, 0, 128, 200, 254, 1, 77, 255][usize::from(opacity_2)] });
            let _ = doc.apply(Command::SetLayerBlend { layer: id, blend: BlendMode::ALL[usize::from(blend_2) * 3 % 8] });
            let _ = doc.apply(Command::SetLayerVisible { layer: id, visible: visible != 0 });
        }
        let mut with = Bench::new_2(doc.clone());
        let mut without = Bench::new_2(doc);
        let mut cache = OverlayCache::default();
        let mut pending: Vec<PixelEdit> = Vec::new();
        let mut active_2 = with.doc.layers()[0].cel();
        let generate = |n: u32, a: u32, b: u32, c: u32| -> Vec<PixelEdit> {
            (0..n)
                .map(|j| PixelEdit {
                    x: a.wrapping_add(j.wrapping_mul(11)) % WIDTH,
                    y: b.wrapping_add(j.wrapping_mul(5)) % HEIGHT,
                    pixel: pixel_de(mode_2, c.wrapping_add(j), 8),
                })
                .collect()
        };
        for (n, &(kind, a, b, c, d)) in steps_2.iter().enumerate() {
            let step: StepOfStroke = (kind, a, b, c, d);
            let mut action_2 = TestAction::Nothing;
            match step.0 {
                // El trazo crece (el lápiz y el borrador).
                0..=2 => pending.extend(generate(1 + a % 40, b, c, d)),
                // Cambia de forma (la goma elástica de la línea).
                3 => pending = generate(1 + a % 60, b, c, d),
                // Esc: se cancela.
                4 => pending.clear(),
                5 => action_2 = TestAction::Undo,
                6 => action_2 = TestAction::Redo,
                7 => {
                    let layers = with.doc.layers().len();
                    active_2 = with.doc.layers()[a as usize % layers].cel();
                }
                _ => {
                    if !pending.is_empty() {
                        action_2 = TestAction::Release;
                    }
                }
            }
            // El zoom cambia entre fotogramas en los dos lienzos a la vez.
            if b % 5 == 0 {
                let operation = if b % 2 == 0 { ViewOp::ZoomIn } else { ViewOp::ZoomOut };
                with.canvas.request_view(operation);
                without.canvas.request_view(operation);
            }
            frame(&mut with, Some(&mut cache), active_2, &pending, action_2);
            frame(&mut without, None, active_2, &pending, action_2);
            if matches!(action_2, TestAction::Release) {
                pending.clear();
            }
            prop_assert_eq!(with.doc.state_id(), without.doc.state_id());
            prop_assert!(with.contents() == without.contents(), "paso {} ({:?}): las texturas difieren", n, action_2);
        }
    }
}
