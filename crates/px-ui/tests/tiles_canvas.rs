//! El lienzo con texturas por baldosas, subidas parciales y superposición como textura
//! (specs/hito-2-zoom-baldosas.md, unidad 14a-2: filas B5 a B10 y propiedades Q2 y Q4; contrato de
//! API en «Contrato de API de la 14a-2»).
//!
//! Banco de egui sin ventana (`Context::run_ui` con `RawInput` sintético, sin GPU ni capturas). Lo
//! que llega a la GPU es el `TexturesDelta` de cada fotograma: el banco lo cuenta (qué baldosa, si
//! es completo o parcial y de qué tamaño) y lo aplica **en orden** con `common::textures::Textures`,
//! que da la textura compuesta; esa se compara con el oráculo de la spec: `from_rgba_unmultiplied`
//! de cada píxel de `to_rgba8(document)` (con los píxeles pendientes escritos durante un gesto y
//! con la entrada cambiada durante una vista previa). Orden de un fotograma, como en la app:
//! `interact` → (eventos y `apply` sobre el documento; `invalidate`) → `set_overlay` →
//! `preview_entry` / `end_preview` → `sync` → `paint`.
//!
//! - B5: `load` atómico (una textura `NEAREST` por baldosa, del tamaño recortado, subida en el primer
//!   fotograma; con un asignador falso que falla en la baldosa 4 el lienzo anterior queda intacto;
//!   lado 16385 → `TooLarge`; `set_image` y `set_owned_image` como antes).
//! - B6: la textura compuesta es el oráculo en todas las baldosas visibles tras cada operación de
//!   documento real, a zoom 1/64, 1/4, ×1 y ×5 con la vista desplazada; las no visibles conservan su
//!   contenido viejo y, al verse, quedan idénticas.
//! - B7: qué se sube: solo las series (`runs`) de los bloques sucios de las baldosas visibles; cero
//!   en reposo; un delta completo por baldosa visible con la imagen entera sucia; las no visibles
//!   conservan sus bloques (`pending_blocks`).
//! - B8: la superposición a las cinco escalas y zoom 1/2, 1/4, ×1, ×2 y ×3 con los cuatro fondos y el
//!   respaldo por malla: cada píxel físico es el oráculo (rasterizador de prueba, suyo, que muestrea
//!   las mallas de las baldosas con `Textures::samples`); ninguna forma de superposición ni de área
//!   cero; antes y después de soltar, igual.
//! - B9: vida de la superposición (solo los bloques con píxeles nuevos; la línea que cambia de
//!   extremo; soltar con `Ok(true)`, con `Ok(false)`, `Esc` y un error; la diagonal de 8192 px).
//! - B10: vista previa del editor por baldosas visibles.
//! - Q2 (oráculo con secuencias aleatorias) y Q4 (superposición con gestos aleatorios).
//!
//! Supuestos que la spec no fija del todo:
//! - Los contadores y los índices de baldosa de `Canvas` (`tile_count`, `pending_blocks`,
//!   `overlay_blocks`, `tile_rect(i)`) son `usize`, como `tile_side` y `Tiling::new(.., side: usize)`;
//!   `tile_rect(i)` devuelve `TileRect`; `texture_ids()` devuelve `Vec<TextureId>`.
//! - Geometría de los bancos: pantalla de 160×120 a ppp 1 y baldosas de 128 (con bloques de 64 dentro
//!   de cada una, para que haya series parciales); imagen de 300×200 (3×2 baldosas: 128, 128 y 44 de
//!   ancho; 128 y 72 de alto) y, a ×1 tras `ActualSize`, se ven las baldosas 0, 1, 3 y 4. Cada prueba lo
//!   comprueba con un oráculo propio (`visible_2`) y falla con un mensaje claro si la preparación no
//!   es la esperada.
//! - Las baldosas pequeñas (`with_tile_side(16)`) de B6, B8 y Q2 no son múltiplo de 64: a 1/64 casi
//!   ninguna tiene área (la columna 1 mide `⌈32/64⌉ − ⌈16/64⌉ = 0`), y el oráculo de lo visible es el
//!   de la spec (cuadrilátero con área positiva).
//! - La subida de la superposición se compara por **bloques cubiertos** (conjunto de bloques de 64×64
//!   que tocan los deltas), no por rectángulos exactos: la spec dice «los bloques cambiados» en la
//!   decisión 7 y «series» solo para las invalidaciones de B7.
//! - `overlay_blocks()` cuenta los bloques de trabajo vivos: 1, 2 y 3 con el lápiz de B9, y en la
//!   línea que cambia de extremo solo los de la línea actual (los anteriores «vuelven al documento»).

use px_core::ImageId;
use std::collections::{HashMap, HashSet};

use egui::epaint::{ClippedPrimitive, ClippedShape, Primitive, Shape, Vertex};
use egui::{
    Color32, Context, Event, ImageData, Key, Modifiers, PointerButton, Pos2, RawInput, Rect,
    TextureId, TextureOptions, Vec2, pos2, vec2,
};
use proptest::prelude::*;
use px_core::{Allowed, Command, Document, DocumentError, Image, Mode, Pixel, PixelEdit, Rgba};
use px_raster::{Line, fill_command, line_command};
use px_ui::background::{CanvasBackground, Fill, background_fill};
use px_ui::canvas::{BACKGROUND, Canvas, CanvasError, CanvasInfo, CanvasInput};
use px_ui::checker::shade_at;
use px_ui::dirty::{BlockSet, ChangeLog, dirty_of};
use px_ui::text::text_scale;
use px_ui::texture::{pixel_rgba, to_rgba8, to_rgba8_with_entry};
use px_ui::tiles::{TileAlloc, TileRect};
use px_ui::view::{Size, ViewOp, Zoom};

mod common;
use common::textures::{Texture, Textures};
use common::{PPPS, button_2, button_of_2, context, key_2, move_pointer};

mod tiles_support;
use tiles_support::{
    B, Model, Seed, Step, TEST_MODES, clip_2, deleted, expected_2, image_of, model_full,
    model_of_pixels, pixel_de, runs_expected, seed, step_of, texel,
};

/// Pantalla de los bancos, en puntos (a ppp 1, en píxeles físicos).
const TEST_SCREEN: Vec2 = vec2(160.0, 120.0);

const RED: Rgba = Rgba::opaque(255, 0, 0);
const GREEN: Rgba = Rgba::opaque(0, 255, 0);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);

/// Niveles de la escalera (índices en `Zoom::levels()`).
const N_1_64: usize = 0;
const N_1_4: usize = 4;
const N_1_2: usize = 5;
const N_X1: usize = 6;
const N_X2: usize = 7;
const N_X3: usize = 8;
const N_X5: usize = 10;

// ---------------------------------------------------------------------------------------------
// Lo que llega a la GPU
// ---------------------------------------------------------------------------------------------

/// Un delta de textura de un fotograma.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Delta {
    pos: Option<[usize; 2]>,
    size_2: [usize; 2],
    nearest: bool,
}

#[derive(Clone, Debug, Default)]
struct Uploads {
    deltas: HashMap<TextureId, Vec<Delta>>,
}

impl Uploads {
    fn de(delta: &egui::TexturesDelta) -> Uploads {
        let mut deltas: HashMap<TextureId, Vec<Delta>> = HashMap::new();
        for (id, list) in &delta.set {
            for d in list {
                let ImageData::Color(image_2) = &d.image;
                deltas.entry(*id).or_default().push(Delta {
                    pos: d.pos,
                    size_2: image_2.size,
                    nearest: d.options == TextureOptions::NEAREST,
                });
            }
        }
        Uploads { deltas }
    }

    fn of_texture(&self, id: TextureId) -> Vec<Delta> {
        self.deltas.get(&id).cloned().unwrap_or_default()
    }

    /// Los deltas de cada baldosa, en el orden de `ids`.
    fn per_tile(&self, ids: &[TextureId]) -> Vec<Vec<Delta>> {
        ids.iter().map(|&id| self.of_texture(id)).collect()
    }

    fn sum(&mut self, other: Uploads) {
        for (id, list) in other.deltas {
            self.deltas.entry(id).or_default().extend(list);
        }
    }

    /// Cuántos deltas llegaron a las baldosas `ids`.
    fn total(&self, ids: &[TextureId]) -> usize {
        ids.iter().map(|&id| self.of_texture(id).len()).sum()
    }
}

/// Lo que da un fotograma del banco.
struct TestOutput {
    shapes_2: Vec<ClippedShape>,
    info: CanvasInfo,
    entries: Vec<CanvasInput>,
    uploads: Uploads,
    sync: Result<(), CanvasError>,
}

// ---------------------------------------------------------------------------------------------
// Operaciones de documento con su región sucia (lo que hace `Session`)
// ---------------------------------------------------------------------------------------------

fn size_of(doc: &Document) -> Size {
    let image_2 = doc.image(ImageId::from_raw(0)).expect("imagen");
    Size::new(image_2.width(), image_2.height())
}

/// `dirty_of` antes de aplicar, `apply`, anotar en el registro y avisar al lienzo.
fn applies_and_records(
    doc: &mut Document,
    log: &mut ChangeLog,
    canvas: &mut Canvas,
    command: Command,
) -> Result<bool, DocumentError> {
    let region = dirty_of(&command, size_of(doc));
    let result = doc.apply(command);
    if let Ok(true) = result {
        log.record(doc.state_id(), region.clone());
        canvas.invalidate(&region);
    }
    result
}

/// `undo` e invalidación de `log[from_2]` (toda la imagen si falta la clave).
fn undo(doc: &mut Document, log: &ChangeLog, canvas: &mut Canvas) -> bool {
    let size_2 = size_of(doc);
    let from_2 = doc.state_id();
    if doc.undo().is_err() {
        return false;
    }
    let region = log
        .get(from_2)
        .cloned()
        .unwrap_or_else(|| BlockSet::full(size_2.width, size_2.height));
    canvas.invalidate(&region);
    true
}

/// `redo` e invalidación de `log[towards]`.
fn redo(doc: &mut Document, log: &ChangeLog, canvas: &mut Canvas) -> bool {
    let size_2 = size_of(doc);
    if doc.redo().is_err() {
        return false;
    }
    let towards = doc.state_id();
    let region = log
        .get(towards)
        .cloned()
        .unwrap_or_else(|| BlockSet::full(size_2.width, size_2.height));
    canvas.invalidate(&region);
    true
}

fn index_of(zoom: Zoom) -> usize {
    Zoom::levels().position(|z| z == zoom).expect("un nivel")
}

// ---------------------------------------------------------------------------------------------
// El banco
// ---------------------------------------------------------------------------------------------

struct Bench {
    ctx: Context,
    ppp: f32,
    screen: Vec2,
    canvas: Canvas,
    doc: Document,
    log: ChangeLog,
    textures_2: Textures,
    max_texture: Option<usize>,
    /// Aplicar los deltas a `textures` (las pruebas de memoria grande lo desactivan).
    saves: bool,
    info: Option<CanvasInfo>,
}

impl Bench {
    /// Sin cargar nada en el lienzo.
    fn empty(ctx: Context, ppp: f32, screen: Vec2, canvas: Canvas, image: Image) -> Bench {
        Bench {
            ctx,
            ppp,
            screen,
            canvas,
            doc: Document::new(image),
            log: ChangeLog::new(),
            textures_2: Textures::new(),
            max_texture: None,
            saves: true,
            info: None,
        }
    }

    /// Con la imagen cargada (`load`) y tres fotogramas de reposo, ya aplicados los deltas.
    fn with(ctx: Context, ppp: f32, screen: Vec2, canvas: Canvas, image: Image) -> Bench {
        let mut bench = Bench::empty(ctx, ppp, screen, canvas, image);
        let image_2 = bench.doc.image(ImageId::from_raw(0)).expect("imagen");
        bench.canvas.load(&bench.ctx, image_2).expect("load");
        for _ in 0..3 {
            bench.rest_2_2();
        }
        bench
    }

    /// ppp 1, la pantalla de siempre y baldosas del lado dado.
    fn simple(side: usize, image: Image) -> Bench {
        Bench::with(
            context(1.0),
            1.0,
            TEST_SCREEN,
            Canvas::with_tile_side(side),
            image,
        )
    }

    fn image_2(&self) -> &Image {
        self.doc.image(ImageId::from_raw(0)).expect("imagen")
    }

    /// Un fotograma: `interact`, la fase del documento (que puede llamar a `invalidate`,
    /// `set_overlay`, `preview_entry`…), `sync` y `paint`.
    fn runs<F>(&mut self, events: Vec<Event>, phase: F) -> TestOutput
    where
        F: FnOnce(&Context, &mut Canvas, &mut Document, &mut ChangeLog),
    {
        let mut phase = Some(phase);
        let mut captured: Option<(CanvasInfo, Vec<CanvasInput>, Result<(), CanvasError>)> = None;
        let input_2 = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.screen)),
            events,
            max_texture_side: self.max_texture,
            ..RawInput::default()
        };
        let canvas = &mut self.canvas;
        let doc = &mut self.doc;
        let log = &mut self.log;
        let mut output_2 = self.ctx.run_ui(input_2, |ui| {
            let entries = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            if let Some(phase) = phase.take() {
                phase(ui.ctx(), canvas, doc, log);
            }
            let id = ImageId::from_raw(0);
            let image_2 = doc.image(id).expect("imagen");
            let sync = canvas.sync(ui.ctx(), image_2);
            let info = canvas.paint(ui);
            captured = Some((info, entries, sync));
        });
        let uploads = Uploads::de(&output_2.textures_delta);
        if self.saves {
            self.textures_2.applies(&output_2.textures_delta);
        }
        output_2.textures_delta.clear();
        let shapes_2 = std::mem::take(&mut output_2.shapes);
        let (info, entries, sync) = captured.expect("el lienzo se dibuja");
        self.info = Some(info.clone());
        TestOutput {
            shapes_2,
            info,
            entries,
            uploads,
            sync,
        }
    }

    fn rest_2_2(&mut self) -> TestOutput {
        self.runs(vec![], |_, _, _, _| {})
    }

    fn applies(&mut self, command: Command) -> TestOutput {
        self.runs(vec![], move |_, canvas, doc, log| {
            let _ = applies_and_records(doc, log, canvas, command);
        })
    }

    fn undoes(&mut self) -> TestOutput {
        self.runs(vec![], |_, canvas, doc, log| {
            undo(doc, log, canvas);
        })
    }

    fn redoes(&mut self) -> TestOutput {
        self.runs(vec![], |_, canvas, doc, log| {
            redo(doc, log, canvas);
        })
    }

    fn asks(&mut self, op: ViewOp) -> TestOutput {
        self.canvas.request_view(op);
        self.rest_2_2()
    }

    /// Lleva el zoom al nivel `goal` de la escalera con peticiones de vista (ancladas en el
    /// centro) y un solo fotograma.
    fn go_to_level(&mut self, goal: usize) -> TestOutput {
        let actual = index_of(self.info.as_ref().expect("hubo un fotograma").zoom);
        for _ in actual..goal {
            self.canvas.request_view(ViewOp::ZoomIn);
        }
        for _ in goal..actual {
            self.canvas.request_view(ViewOp::ZoomOut);
        }
        let output_2 = self.rest_2_2();
        assert_eq!(
            index_of(output_2.info.zoom),
            goal,
            "el banco no llegó al nivel {goal}"
        );
        output_2
    }

    /// Arrastra la vista con el botón central `dx` y `dy` píxeles físicos (si el límite de la vista
    /// no lo impide), un fotograma por paso, llamando a `phase` en cada uno. egui no decide que es
    /// un arrastre hasta que el puntero se aleja de donde se pulsó: el primer movimiento va lejos
    /// (40 puntos) y el segundo vuelve al destino, así el desplazamiento neto es `(dx, dy)`.
    fn drags_with<F>(&mut self, dx: i64, dy: i64, mut phase: F) -> Vec<TestOutput>
    where
        F: FnMut(&Context, &mut Canvas, &mut Document, &mut ChangeLog),
    {
        let start_2 = pos2(self.screen.x / 2.0, self.screen.y / 2.0);
        let far = start_2 + vec2(40.0, 40.0);
        let target_2 = start_2 + vec2(dx as f32 / self.ppp, dy as f32 / self.ppp);
        let steps_2 = [
            vec![move_pointer(start_2)],
            vec![button_of_2(start_2, PointerButton::Middle, true)],
            vec![move_pointer(far)],
            vec![move_pointer(target_2)],
            vec![button_of_2(target_2, PointerButton::Middle, false)],
            vec![],
        ];
        steps_2
            .into_iter()
            .map(|events| self.runs(events, |a, b, c, d| phase(a, b, c, d)))
            .collect()
    }

    fn drags(&mut self, dx: i64, dy: i64) -> Vec<TestOutput> {
        self.drags_with(dx, dy, |_, _, _, _| {})
    }

    fn viewport(&self) -> Size {
        Size::new(
            (self.screen.x * self.ppp).round() as u32,
            (self.screen.y * self.ppp).round() as u32,
        )
    }

    fn tiles_2(&self) -> usize {
        self.canvas.tile_count()
    }

    fn rect_de(&self, i: usize) -> TileRect {
        self.canvas.tile_rect(i)
    }

    fn ids(&self) -> Vec<TextureId> {
        self.canvas.texture_ids()
    }

    /// El origen de la imagen en píxeles físicos, del último fotograma.
    fn source_2(&self) -> (i64, i64) {
        let rect = self
            .info
            .as_ref()
            .expect("hubo un fotograma")
            .image_rect
            .expect("hay imagen");
        (
            (rect.min.x * self.ppp).round() as i64,
            (rect.min.y * self.ppp).round() as i64,
        )
    }

    /// Qué baldosas se ven en el último fotograma: el cuadrilátero `[o + extent(a), o + extent(b))`
    /// corta el lienzo con área positiva (oráculo propio, con `Zoom::extent` de la escalera).
    fn visible_2(&self) -> Vec<bool> {
        let zoom = self.info.as_ref().expect("hubo un fotograma").zoom;
        let (ox, oy) = self.source_2();
        let vp = self.viewport();
        (0..self.tiles_2())
            .map(|i| {
                let r = self.rect_de(i);
                let x0 = (ox + zoom.extent(i64::from(r.x))).max(0);
                let x1 = (ox + zoom.extent(i64::from(r.x + r.width))).min(i64::from(vp.width));
                let y0 = (oy + zoom.extent(i64::from(r.y))).max(0);
                let y1 = (oy + zoom.extent(i64::from(r.y + r.height))).min(i64::from(vp.height));
                x1 > x0 && y1 > y0
            })
            .collect()
    }

    fn ensures_visible(&self, expected_2: &[bool], label_2: &str) {
        assert_eq!(
            self.visible_2(),
            expected_2,
            "{label_2}: la preparación del banco no es la esperada"
        );
    }

    /// El contenido compuesto de la baldosa `i`.
    fn content(&self, i: usize) -> Texture {
        self.textures_2
            .get(self.ids()[i])
            .unwrap_or_else(|| panic!("la baldosa {i} no tiene textura"))
            .clone()
    }

    fn contents(&self) -> Vec<Texture> {
        (0..self.tiles_2()).map(|i| self.content(i)).collect()
    }

    /// Diferencias con el oráculo: las baldosas de `must` (visibles) tienen que ser idénticas a la
    /// imagen esperada; las demás, idénticas a `before` (si se da).
    fn discrepancies(
        &self,
        expected: &[Color32],
        must: &[bool],
        before: Option<&[Texture]>,
    ) -> Vec<String> {
        let width_2 = self.image_2().width() as usize;
        let mut failures = Vec::new();
        for i in 0..self.tiles_2() {
            let r = self.rect_de(i);
            let actual = self.content(i);
            if must[i] {
                let size_2 = [r.width as usize, r.height as usize];
                if actual.size_2 != size_2 {
                    failures.push(format!(
                        "baldosa {i}: tamaño {:?}, esperado {size_2:?}",
                        actual.size_2
                    ));
                } else if actual.texels != clip_2(expected, width_2, r) {
                    let n = actual
                        .texels
                        .iter()
                        .zip(clip_2(expected, width_2, r))
                        .filter(|(a, b)| **a != *b)
                        .count();
                    failures.push(format!(
                        "baldosa {i} ({r:?}) visible: {n} texels distintos del oráculo"
                    ));
                }
            } else if let Some(before) = before
                && actual != before[i]
            {
                failures.push(format!("baldosa {i} no visible: cambió su contenido"));
            }
        }
        failures
    }

    /// Todas las baldosas visibles son el oráculo del documento.
    fn checks_visible(&self, label_2: &str) {
        let failures = self.discrepancies(&expected_2(self.image_2()), &self.visible_2(), None);
        assert!(failures.is_empty(), "{label_2}: {failures:?}");
    }

    /// Lleva todas las baldosas a la vista (Ajustar) y comprueba **todas**.
    fn checks_all(&mut self, label_2: &str) {
        self.asks(ViewOp::Fit);
        self.rest_2_2();
        let visible_2 = self.visible_2();
        assert!(
            visible_2.iter().all(|&v| v),
            "{label_2}: Ajustar debía dejar todas a la vista: {visible_2:?}"
        );
        let failures = self.discrepancies(&expected_2(self.image_2()), &visible_2, None);
        assert!(failures.is_empty(), "{label_2} (todas): {failures:?}");
    }

    fn primitives(&self, output_2: &TestOutput) -> Vec<ClippedPrimitive> {
        self.ctx.tessellate(output_2.shapes_2.clone(), self.ppp)
    }
}

/// Asignador falso: da memoria las primeras `good` veces y después falla.
struct Fail {
    good: usize,
    calls: usize,
}

impl Fail {
    fn after(good: usize) -> Fail {
        Fail { good, calls: 0 }
    }
}

impl TileAlloc for Fail {
    fn pixels(&mut self, len: usize) -> Option<Vec<Color32>> {
        self.calls += 1;
        (self.calls <= self.good).then(|| Vec::with_capacity(len))
    }
}

/// Las series de los bloques sucios de una baldosa como deltas (completo si están todos).
fn deltas_expected(model: &Model, tile: TileRect) -> Vec<Delta> {
    let cut = |bx: u32, by: u32| tiles_support::short(bx, by, tile);
    let dirty_2 = model.iter().filter(|&&(bx, by)| cut(bx, by)).count();
    let of_the_tile = ((tile.x + tile.width - 1) / B - tile.x / B + 1) as usize
        * ((tile.y + tile.height - 1) / B - tile.y / B + 1) as usize;
    let size_2 = [tile.width as usize, tile.height as usize];
    if dirty_2 == 0 {
        return Vec::new();
    }
    if dirty_2 == of_the_tile {
        return vec![Delta {
            pos: None,
            size_2,
            nearest: true,
        }];
    }
    let mut v: Vec<Delta> = runs_expected(model, tile)
        .into_iter()
        .map(|r| Delta {
            pos: Some([(r.x - tile.x) as usize, (r.y - tile.y) as usize]),
            size_2: [r.width as usize, r.height as usize],
            nearest: true,
        })
        .collect();
    v.sort();
    v
}

/// Los bloques (en la imagen) que cubren los deltas de una baldosa.
fn blocks_covered(deltas: &[Delta], tile: TileRect) -> Model {
    let mut m = Model::new();
    for d in deltas {
        let [px, py] = d.pos.unwrap_or([0, 0]);
        let (x0, y0) = (tile.x + px as u32, tile.y + py as u32);
        let (x1, y1) = (x0 + d.size_2[0] as u32, y0 + d.size_2[1] as u32);
        for by in y0 / B..=(y1 - 1) / B {
            for bx in x0 / B..=(x1 - 1) / B {
                m.insert((bx, by));
            }
        }
    }
    m
}

/// Banco de las filas B7 y B9: 300×200 RGBA en baldosas de 128 y vista a ×1.
fn bench_300x200() -> Bench {
    let mut bench = Bench::simple(128, image_of(Mode::Rgba, 300, 200, 7));
    bench.asks(ViewOp::ActualSize);
    bench.ensures_visible(&[true, true, false, true, true, false], "300×200 a ×1");
    bench
}

fn strip(x0: u32, y: u32, n: u32, color: Rgba) -> Vec<(u32, u32, Rgba)> {
    (0..n).map(|i| (x0 + i, y, color)).collect()
}

// ---------------------------------------------------------------------------------------------
// B5 · Carga atómica
// ---------------------------------------------------------------------------------------------

fn sizes_of_tiles(bench: &Bench) -> Vec<[usize; 2]> {
    (0..bench.tiles_2())
        .map(|i| {
            let r = bench.rect_de(i);
            [r.width as usize, r.height as usize]
        })
        .collect()
}

#[test]
fn row_b5_load_creates_a_nearest_texture_per_tile_of_the_clipped_size_uploaded_in_the_first_frame()
{
    // Una baldosa y 3×2 baldosas (con recorte en el borde derecho y en el inferior).
    type Case = ((u32, u32), usize, Vec<[usize; 2]>);
    let cases: [Case; 2] = [
        ((10, 8), 16, vec![[10, 8]]),
        (
            (40, 24),
            16,
            vec![[16, 16], [16, 16], [8, 16], [16, 8], [16, 8], [8, 8]],
        ),
    ];
    for ((w, h), side, sizes) in cases {
        let image_2 = image_of(Mode::Rgba, w, h, 3);
        let mut bench = Bench::empty(
            context(1.0),
            1.0,
            TEST_SCREEN,
            Canvas::with_tile_side(side),
            image_2.clone(),
        );
        assert_eq!(bench.canvas.load(&bench.ctx, &image_2), Ok(()));
        let first = bench.rest_2_2();
        let ids = bench.ids();
        let label_2 = format!("{w}×{h} con baldosas de {side}");
        assert_eq!(ids.len(), sizes.len(), "{label_2}: una textura por baldosa");
        assert_eq!(bench.tiles_2(), sizes.len(), "{label_2}");
        assert_eq!(sizes_of_tiles(&bench), sizes, "{label_2}");
        assert_eq!(
            ids.iter().collect::<HashSet<_>>().len(),
            ids.len(),
            "{label_2}: texturas distintas"
        );
        assert_eq!(bench.canvas.texture_id(), Some(ids[0]), "{label_2}");
        // Subidas: un delta completo `NEAREST` del tamaño recortado por baldosa, en el primer
        // fotograma, y ninguno después.
        for (i, deltas) in first.uploads.per_tile(&ids).iter().enumerate() {
            assert_eq!(
                deltas,
                &vec![Delta {
                    pos: None,
                    size_2: sizes[i],
                    nearest: true
                }],
                "{label_2}: baldosa {i} en el primer fotograma"
            );
        }
        for n in 0..3 {
            let after_2 = bench.rest_2_2();
            assert_eq!(
                after_2.uploads.total(&ids),
                0,
                "{label_2}: fotograma {n} después"
            );
        }
        // Todas las baldosas se construyeron al cargar: el contenido es el oráculo.
        let failures = bench.discrepancies(&expected_2(&image_2), &vec![true; ids.len()], None);
        assert!(failures.is_empty(), "{label_2}: {failures:?}");
    }
}

#[test]
fn row_b5_with_an_allocator_that_fails_at_tile_4_the_previous_canvas_stays_intact() {
    let previous = image_of(Mode::Rgba, 40, 24, 3);
    let mut bench = Bench::simple(16, previous.clone());
    // Un lienzo con vista propia y un gesto de dibujo en curso.
    bench.go_to_level(N_X3);
    let center = pos2(80.0, 60.0);
    bench.runs(
        vec![move_pointer(center), button_2(center, true)],
        |_, _, _, _| {},
    );
    let before = bench.rest_2_2();
    let (zoom, rect) = (before.info.zoom, before.info.image_rect);
    let ids = bench.ids();
    let contents = bench.contents();
    let live = bench.ctx.tex_manager().read().num_allocated();

    // 80×48 con baldosas de 16 son 15 baldosas: la 4.ª reserva falla.
    let new = image_of(Mode::Gray, 80, 48, 5);
    let mut fail = Fail::after(3);
    assert_eq!(
        bench.canvas.load_with(&bench.ctx, &new, &mut fail),
        Err(CanvasError::OutOfMemory)
    );
    assert_eq!(fail.calls, 4, "la carga para en la baldosa que falla");

    // Sin texturas nuevas vivas, ahora ni tras el fotograma que libera lo que hubiera.
    assert_eq!(bench.ctx.tex_manager().read().num_allocated(), live);
    let after = bench.rest_2_2();
    assert_eq!(bench.ctx.tex_manager().read().num_allocated(), live);
    assert_eq!(bench.ids(), ids, "las texturas de antes");
    assert_eq!(
        after.uploads.total(&ids),
        0,
        "ni una subida de lo que ya estaba"
    );
    assert_eq!(bench.contents(), contents);
    let failures = bench.discrepancies(&expected_2(&previous), &vec![true; ids.len()], None);
    assert!(failures.is_empty(), "{failures:?}");
    let en_gpu: HashSet<TextureId> = bench.textures_2.ids().collect();
    for id in &en_gpu {
        assert!(
            ids.contains(id)
                || Some(*id) == bench.canvas.checker_texture_id()
                || *id == TextureId::default(),
            "una textura que sobra en la GPU: {id:?}"
        );
    }
    // La vista y el gesto: el mismo nivel y el mismo rectángulo, y el trazo sigue y termina.
    assert_eq!(after.info.zoom, zoom);
    assert_eq!(after.info.image_rect, rect);
    assert_eq!(bench.tiles_2(), 6);
    let followed = bench.runs(vec![move_pointer(center + vec2(6.0, 0.0))], |_, _, _, _| {});
    assert!(
        followed.entries.iter().any(|e| matches!(
            e,
            CanvasInput::Stroke(px_ui::canvas::CanvasEvent::StrokeMove(_))
        )),
        "el gesto sigue: {:?}",
        followed.entries
    );
    let fin = bench.runs(
        vec![button_2(center + vec2(6.0, 0.0), false)],
        |_, _, _, _| {},
    );
    assert!(
        fin.entries.iter().any(|e| matches!(
            e,
            CanvasInput::Stroke(px_ui::canvas::CanvasEvent::StrokeEnd)
        )),
        "y termina: {:?}",
        fin.entries
    );

    // Con memoria otra vez, la carga sí cambia la imagen y la vista vuelve al ajuste.
    assert_eq!(bench.canvas.load(&bench.ctx, &new), Ok(()));
    bench.doc = Document::new(new.clone());
    let after_2 = bench.rest_2_2();
    assert_eq!(bench.tiles_2(), 15);
    assert_ne!(bench.ids(), ids);
    let fit = px_ui::view::View::fit(bench.viewport(), Size::new(80, 48)).zoom();
    assert_eq!(after_2.info.zoom, fit, "la vista vuelve al ajuste");
}

#[test]
fn row_b5_if_the_first_load_fails_nothing_is_left() {
    let image_2 = image_of(Mode::Rgba, 40, 24, 3);
    let ctx = context(1.0);
    let live = ctx.tex_manager().read().num_allocated();
    let mut canvas = Canvas::with_tile_side(16);
    let mut fail = Fail::after(3);
    assert_eq!(
        canvas.load_with(&ctx, &image_2, &mut fail),
        Err(CanvasError::OutOfMemory)
    );
    assert_eq!(canvas.tile_count(), 0);
    assert_eq!(canvas.texture_id(), None);
    assert!(canvas.texture_ids().is_empty());
    assert_eq!(ctx.tex_manager().read().num_allocated(), live);
}

#[test]
fn row_b5_an_image_of_side_16385_gives_too_large_with_max_16384_and_leaves_the_canvas_as_it_was() {
    assert_eq!(Canvas::MAX_SIDE, 16384);
    for (w, h) in [(16385u32, 1u32), (1, 16385), (20000, 3)] {
        let large = Image::new(w, h, Mode::Gray).expect("imagen de un píxel de alto");
        let mut canvas = Canvas::new();
        assert_eq!(
            canvas.load(&context(1.0), &large),
            Err(CanvasError::TooLarge {
                width: w,
                height: h,
                max: 16384
            }),
            "{w}×{h}"
        );
        assert_eq!(canvas.tile_count(), 0, "{w}×{h}");
    }
    // El lado exacto vale: 16384×1 son 8 baldosas de 2048.
    let just = Image::new(16384, 1, Mode::Gray).expect("imagen");
    let mut bench = Bench::empty(context(1.0), 1.0, TEST_SCREEN, Canvas::new(), just.clone());
    assert_eq!(bench.canvas.load(&bench.ctx, &just), Ok(()));
    bench.rest_2_2();
    assert_eq!(bench.tiles_2(), 8);
    // Con un lienzo anterior, el rechazo no lo toca.
    let previous = image_of(Mode::Rgba, 40, 24, 3);
    let mut bench = Bench::simple(16, previous);
    let ids = bench.ids();
    let large = Image::new(16385, 1, Mode::Gray).expect("imagen");
    assert!(matches!(
        bench.canvas.load(&bench.ctx, &large),
        Err(CanvasError::TooLarge { max: 16384, .. })
    ));
    let after = bench.rest_2_2();
    assert_eq!(bench.ids(), ids);
    assert_eq!(after.uploads.total(&ids), 0);
    bench.checks_visible("tras el rechazo");
}

#[test]
fn row_b5_set_image_and_set_owned_image_build_in_the_first_frame_as_before() {
    for own in [false, true] {
        let image_2 = image_of(Mode::Rgba, 40, 24, 3);
        let mut bench = Bench::empty(
            context(1.0),
            1.0,
            TEST_SCREEN,
            Canvas::with_tile_side(16),
            image_2.clone(),
        );
        if own {
            bench.canvas.set_owned_image(image_2.clone());
        } else {
            bench.canvas.set_image(&image_2);
        }
        assert_eq!(
            bench.canvas.texture_id(),
            None,
            "propia {own}: aún sin textura"
        );
        assert_eq!(bench.canvas.zoom(), None, "propia {own}: aún sin vista");
        let first = bench.rest_2_2();
        let ids = bench.ids();
        assert_eq!(ids.len(), 6, "propia {own}");
        assert_eq!(bench.canvas.texture_id(), Some(ids[0]));
        for (i, deltas) in first.uploads.per_tile(&ids).iter().enumerate() {
            assert_eq!(deltas.len(), 1, "propia {own}: baldosa {i}");
            assert_eq!(deltas[0].pos, None);
            assert!(deltas[0].nearest);
        }
        assert_eq!(
            first.info.zoom,
            px_ui::view::View::fit(bench.viewport(), Size::new(40, 24)).zoom()
        );
        let failures = bench.discrepancies(&expected_2(&image_2), &[true; 6], None);
        assert!(failures.is_empty(), "propia {own}: {failures:?}");
        // Otra imagen: la vista vuelve al ajuste y el gesto se descarta.
        bench.go_to_level(N_X5);
        let other = image_of(Mode::Gray, 20, 12, 3);
        if own {
            bench.canvas.set_owned_image(other.clone());
        } else {
            bench.canvas.set_image(&other);
        }
        bench.doc = Document::new(other.clone());
        let after = bench.rest_2_2();
        assert_eq!(
            after.info.zoom,
            px_ui::view::View::fit(bench.viewport(), Size::new(20, 12)).zoom(),
            "propia {own}"
        );
        assert_eq!(bench.tiles_2(), 2, "20×12 con baldosas de 16");
    }
}

#[test]
fn row_b5_set_image_with_side_16385_gives_the_error_and_does_not_paint() {
    let large = Image::new(16385, 1, Mode::Gray).expect("imagen");
    let mut bench = Bench::empty(context(1.0), 1.0, TEST_SCREEN, Canvas::new(), large.clone());
    bench.canvas.set_image(&large);
    let output_2 = bench.rest_2_2();
    assert_eq!(
        output_2.info.error,
        Some(CanvasError::TooLarge {
            width: 16385,
            height: 1,
            max: 16384
        })
    );
    assert_eq!(output_2.info.image_rect, None);
    assert_eq!(bench.canvas.texture_id(), None);
}

#[test]
fn row_b5_sync_with_returns_out_of_memory_if_the_allocator_fails() {
    let mut bench = Bench::simple(16, image_of(Mode::Rgba, 40, 24, 3));
    let mut result = None;
    bench.runs(vec![], |ctx, canvas, doc, _| {
        canvas.invalidate_all();
        let image_2 = doc.image(ImageId::from_raw(0)).expect("imagen");
        result = Some(canvas.sync_with(ctx, image_2, &mut Fail::after(0)));
    });
    assert_eq!(result, Some(Err(CanvasError::OutOfMemory)));
}

// ---------------------------------------------------------------------------------------------
// B6 · Oráculo de la textura compuesta
// ---------------------------------------------------------------------------------------------

/// La operación `label_2` sobre el documento y su fotograma: las visibles son el oráculo y las no
/// visibles siguen con su contenido de antes; el documento no lo toca el lienzo.
fn step_b_6(bench: &mut Bench, label_2: &str, operation: impl FnOnce(&mut Bench) -> TestOutput) {
    let before = bench.contents();
    let output_2 = operation(bench);
    assert_eq!(output_2.sync, Ok(()), "{label_2}");
    let visible_2 = bench.visible_2();
    let failures = bench.discrepancies(&expected_2(bench.image_2()), &visible_2, Some(&before));
    assert!(failures.is_empty(), "{label_2}: {failures:?}");
    // Ni el estado ni la imagen del documento cambian por pintar, y en reposo no pasa nada.
    let (state_2, image_2) = (bench.doc.state_id(), bench.image_2().clone());
    let contents = bench.contents();
    for _ in 0..2 {
        let rest_2_2 = bench.rest_2_2();
        assert_eq!(rest_2_2.uploads.total(&bench.ids()), 0, "{label_2}: reposo");
    }
    assert_eq!(bench.doc.state_id(), state_2, "{label_2}: state_id");
    assert_eq!(bench.image_2(), &image_2, "{label_2}: documento");
    assert_eq!(bench.contents(), contents, "{label_2}: reposo");
}

fn other_mode(mode_2: Mode) -> Mode {
    match mode_2 {
        Mode::Rgba => Mode::GrayAlpha,
        _ => Mode::Rgba,
    }
}

/// Todas las operaciones de la fila B6 sobre un banco, en el modo de su imagen.
fn operations_b_6(bench: &mut Bench, label_2: &str) {
    let mode_2 = bench.image_2().mode();
    let (w, h) = (bench.image_2().width(), bench.image_2().height());
    let id = ImageId::from_raw(0);
    let entries = bench.image_2().palette().map_or(1, |p| p.len());
    let pixel = |k: u32| pixel_de(mode_2, k, entries);

    // Línea.
    let line = line_command(
        &bench.doc,
        id,
        (2, 3),
        (w as i32 - 3, h as i32 - 4),
        pixel(5),
    )
    .expect("línea")
    .expect("cae dentro");
    step_b_6(bench, &format!("{label_2} · línea"), |b| b.applies(line));

    // Trazo: píxeles sueltos en las esquinas y en el centro.
    let stroke_2 = Command::SetPixels {
        image: id,
        pixels: [
            (0, 0),
            (w - 1, h - 1),
            (w - 1, 0),
            (0, h - 1),
            (w / 2, h / 2),
        ]
        .into_iter()
        .enumerate()
        .map(|(k, (x, y))| PixelEdit {
            x,
            y,
            pixel: pixel(30 + k as u32),
        })
        .collect(),
    };
    step_b_6(bench, &format!("{label_2} · trazo"), |b| {
        b.applies(stroke_2)
    });

    // Relleno.
    if let Ok(Some(fill)) = fill_command(&bench.doc, id, 1, 1, pixel(23)) {
        step_b_6(bench, &format!("{label_2} · relleno"), |b| b.applies(fill));
    }

    // Borrador.
    if let Some(delete) = deleted(bench.image_2()) {
        let eraser = Command::SetPixels {
            image: id,
            pixels: (0..w)
                .map(|x| PixelEdit {
                    x,
                    y: h / 2,
                    pixel: delete,
                })
                .collect(),
        };
        step_b_6(bench, &format!("{label_2} · borrador"), |b| {
            b.applies(eraser)
        });
    }

    // Deshacer dos veces y rehacer una.
    step_b_6(bench, &format!("{label_2} · undo"), |b| b.undoes());
    step_b_6(bench, &format!("{label_2} · undo"), |b| b.undoes());
    step_b_6(bench, &format!("{label_2} · redo"), |b| b.redoes());

    // Paleta (solo indexado): color, añadir y mover (no cambian lo que se ve), quitar.
    if mode_2 == Mode::Indexed {
        let color = Command::SetPaletteColor {
            image: id,
            index: 2,
            color: Rgba::opaque(9, 99, 199),
        };
        step_b_6(bench, &format!("{label_2} · color"), |b| b.applies(color));
        let move_input = Command::MovePaletteEntry {
            image: id,
            from: 1,
            to: 4,
        };
        step_b_6(bench, &format!("{label_2} · mover"), |b| {
            b.applies(move_input)
        });
        let add = Command::AddPaletteEntry {
            image: id,
            color: Rgba::opaque(1, 2, 3),
        };
        step_b_6(bench, &format!("{label_2} · añadir"), |b| b.applies(add));
        let remove = Command::RemovePaletteEntry {
            image: id,
            index: 3,
            into: 2,
        };
        step_b_6(bench, &format!("{label_2} · quitar"), |b| {
            b.applies(remove)
        });
    }

    // Conversión a otro modo y su deshacer.
    let convert = Command::ConvertMode {
        image: id,
        target: other_mode(mode_2),
        allowed: Allowed {
            luma: true,
            drop_alpha: Some([255, 255, 255]),
        },
    };
    step_b_6(bench, &format!("{label_2} · conversión"), |b| {
        b.applies(convert)
    });
    step_b_6(
        bench,
        &format!("{label_2} · undo de la conversión"),
        |b| b.undoes(),
    );
}

/// Un banco de B6 en un modo, un tamaño y un nivel de zoom con la vista desplazada.
fn scenario_b_6(mode_2: Mode, (w, h): (u32, u32), level: usize, drag: Option<(i64, i64)>) {
    let label_2 = format!("{mode_2:?} {w}×{h} nivel {level} arrastre {drag:?}");
    let mut bench = Bench::simple(128, image_of(mode_2, w, h, 7));
    bench.go_to_level(level);
    if let Some((dx, dy)) = drag {
        bench.drags(dx, dy);
    }
    operations_b_6(&mut bench, &label_2);
    // Al hacerse visibles las que esperaban, quedan idénticas al oráculo.
    bench.checks_all(&label_2);
}

#[test]
fn row_b6_one_tile_in_the_four_modes_at_four_zooms() {
    for mode_2 in TEST_MODES {
        for (level, drag) in [
            (N_1_64, None),
            (N_1_4, Some((-20, 10))),
            (N_X1, Some((-30, -20))),
            (N_X5, Some((15, -25))),
        ] {
            scenario_b_6(mode_2, (100, 60), level, drag);
        }
    }
}

#[test]
fn row_b6_three_by_two_tiles_in_the_four_modes_at_four_zooms() {
    for mode_2 in TEST_MODES {
        for (level, drag) in [
            (N_1_64, None),
            (N_1_4, Some((-20, 10))),
            (N_X1, Some((-90, -50))),
            (N_X5, Some((100, 0))),
        ] {
            scenario_b_6(mode_2, (300, 140), level, drag);
        }
    }
}

#[test]
fn row_b6_the_non_visible_ones_keep_their_content_and_when_made_visible_they_are_identical() {
    // A ×5 solo se ve una baldosa de 300×200: el trazo en las demás no cambia sus texturas.
    let mut bench = Bench::simple(128, image_of(Mode::Rgba, 300, 200, 7));
    bench.go_to_level(N_X5);
    let visible_2 = bench.visible_2();
    assert_eq!(visible_2.iter().filter(|&&v| v).count(), 1, "{visible_2:?}");
    let id = ImageId::from_raw(0);
    let before = bench.contents();
    let stroke_2 = Command::SetPixels {
        image: id,
        pixels: (0..300)
            .step_by(7)
            .map(|x| PixelEdit {
                x,
                y: 10 + x % 150,
                pixel: Pixel::Rgba(RED),
            })
            .collect(),
    };
    bench.applies(stroke_2);
    let after = bench.contents();
    for (i, (a, t)) in before.iter().zip(&after).enumerate() {
        if !visible_2[i] {
            assert_eq!(a, t, "baldosa {i}: no se ve y no debe cambiar");
        }
    }
    // Un desplazamiento que muestra otra baldosa: se construye con lo que tiene el documento.
    for dx in [-120, 240, -40, 0] {
        bench.drags(dx, 0);
        bench.checks_visible(&format!("tras desplazar {dx}"));
    }
    bench.checks_all("al final");
}

// ---------------------------------------------------------------------------------------------
// B7 · Qué se sube
// ---------------------------------------------------------------------------------------------

/// Comprueba, baldosa a baldosa, lo que llegó a la GPU: las visibles reciben las series de los
/// bloques sucios de `dirty` (o un delta completo si lo están todos) y las demás, nada.
fn checks_uploads(bench: &Bench, output_2: &TestOutput, dirty: &Model, label_2: &str) {
    let ids = bench.ids();
    let visible_2 = bench.visible_2();
    for (i, id) in ids.iter().enumerate() {
        let mut real = output_2.uploads.of_texture(*id);
        real.sort();
        let expected_2 = if visible_2[i] {
            deltas_expected(dirty, bench.rect_de(i))
        } else {
            Vec::new()
        };
        assert_eq!(
            real, expected_2,
            "{label_2}: baldosa {i} (visible: {})",
            visible_2[i]
        );
        // Ningún delta fuera de su textura.
        let r = bench.rect_de(i);
        for d in &real {
            let [px, py] = d.pos.unwrap_or([0, 0]);
            assert!(
                px + d.size_2[0] <= r.width as usize && py + d.size_2[1] <= r.height as usize,
                "{label_2}: delta {d:?} fuera de la textura de la baldosa {i}"
            );
        }
    }
}

fn pixels_of(command: &Command) -> Vec<(u32, u32)> {
    match command {
        Command::SetPixels { pixels, .. } => pixels.iter().map(|p| (p.x, p.y)).collect(),
        other_2 => panic!("solo SetPixels: {other_2:?}"),
    }
}

fn stroke_2(id: px_core::ImageId, points: &[(u32, u32)]) -> Command {
    Command::SetPixels {
        image: id,
        pixels: points
            .iter()
            .map(|&(x, y)| PixelEdit {
                x,
                y,
                pixel: Pixel::Rgba(RED),
            })
            .collect(),
    }
}

#[test]
fn row_b7_a_stroke_on_a_visible_tile_uploads_only_its_blocks_and_only_on_that_tile() {
    let mut bench = bench_300x200();
    let id = ImageId::from_raw(0);
    // Un bloque en la baldosa 4 (columna 1, fila 1): el bloque (2, 2) → serie (128,128,64,64).
    let command = stroke_2(id, &[(130, 130), (131, 131)]);
    let dirty = model_of_pixels(&pixels_of(&command), 300, 200);
    let output_2 = bench.applies(command);
    checks_uploads(&bench, &output_2, &dirty, "un bloque en la baldosa 4");
    let ids = bench.ids();
    assert_eq!(
        output_2.uploads.of_texture(ids[4]),
        vec![Delta {
            pos: Some([0, 0]),
            size_2: [64, 64],
            nearest: true
        }]
    );
    assert_eq!(output_2.uploads.total(&ids), 1);
    assert_eq!(bench.canvas.pending_blocks(), 0);
    bench.checks_visible("tras el trazo");

    // Un trazo con bloques contiguos en una fila (se fusionan) y otro en la fila siguiente.
    let command = stroke_2(id, &[(60, 10), (70, 10), (10, 100)]);
    let dirty = model_of_pixels(&pixels_of(&command), 300, 200);
    let output_2 = bench.applies(command);
    checks_uploads(&bench, &output_2, &dirty, "series de la baldosa 0");
    assert_eq!(
        {
            let mut v = output_2.uploads.of_texture(ids[0]);
            v.sort();
            v
        },
        vec![
            Delta {
                pos: Some([0, 0]),
                size_2: [128, 64],
                nearest: true
            },
            Delta {
                pos: Some([0, 64]),
                size_2: [64, 64],
                nearest: true
            }
        ]
    );
    bench.checks_visible("tras el segundo trazo");
}

#[test]
fn row_b7_a_stroke_that_crosses_the_edge_of_two_tiles_uploads_one_block_in_each() {
    let mut bench = bench_300x200();
    let id = ImageId::from_raw(0);
    let command = stroke_2(id, &[(126, 10), (127, 10), (128, 10), (129, 10)]);
    let dirty = model_of_pixels(&pixels_of(&command), 300, 200);
    let output_2 = bench.applies(command);
    checks_uploads(&bench, &output_2, &dirty, "cruza de la baldosa 0 a la 1");
    let ids = bench.ids();
    assert_eq!(output_2.uploads.total(&ids), 2);
    bench.checks_visible("tras cruzar");
}

#[test]
fn row_b7_if_all_the_blocks_of_the_tile_are_dirty_it_uploads_a_full_delta() {
    let mut bench = bench_300x200();
    let id = ImageId::from_raw(0);
    // La baldosa 0 (128×128) tiene 2×2 bloques: tres sucios dan series; cuatro, un delta completo.
    let three = stroke_2(id, &[(1, 1), (70, 1), (1, 70)]);
    let dirty = model_of_pixels(&pixels_of(&three), 300, 200);
    let output_2 = bench.applies(three);
    checks_uploads(&bench, &output_2, &dirty, "tres de cuatro bloques");
    let ids = bench.ids();
    assert_eq!(output_2.uploads.of_texture(ids[0]).len(), 2, "dos series");
    assert!(
        output_2
            .uploads
            .of_texture(ids[0])
            .iter()
            .all(|d| d.pos.is_some())
    );
    let four = stroke_2(id, &[(2, 2), (71, 2), (2, 71), (71, 71)]);
    let dirty = model_of_pixels(&pixels_of(&four), 300, 200);
    let output_2 = bench.applies(four);
    checks_uploads(&bench, &output_2, &dirty, "los cuatro bloques");
    assert_eq!(
        output_2.uploads.of_texture(ids[0]),
        vec![Delta {
            pos: None,
            size_2: [128, 128],
            nearest: true
        }]
    );
    bench.checks_visible("tras los cuatro bloques");
}

#[test]
fn row_b7_a_stroke_on_a_tile_outside_the_canvas_uploads_nothing_and_keeps_its_blocks() {
    let mut bench = bench_300x200();
    let id = ImageId::from_raw(0);
    let ids = bench.ids();
    let before = bench.contents();
    // La baldosa 2 (x de 256 a 300, y de 0 a 128) no se ve: el bloque (4, 0) queda pendiente.
    let command = stroke_2(id, &[(270, 10), (271, 10)]);
    let dirty = model_of_pixels(&pixels_of(&command), 300, 200);
    let output_2 = bench.applies(command);
    checks_uploads(&bench, &output_2, &dirty, "baldosa no visible");
    assert_eq!(output_2.uploads.total(&ids), 0, "no sube nada");
    assert_eq!(bench.canvas.pending_blocks(), 1, "conserva el bloque");
    assert_eq!(bench.contents(), before, "la textura sigue como estaba");
    // Sigue pendiente aunque pasen fotogramas.
    for _ in 0..3 {
        let rest_2_2 = bench.rest_2_2();
        assert_eq!(rest_2_2.uploads.total(&ids), 0);
        assert_eq!(bench.canvas.pending_blocks(), 1);
    }
    // Al verse (un desplazamiento), sube su bloque, recortado a la baldosa de 44 de ancho.
    let mut sum = Uploads::default();
    for s in bench.drags(-120, 0) {
        sum.sum(s.uploads);
    }
    assert_eq!(
        sum.of_texture(ids[2]),
        vec![Delta {
            pos: Some([0, 0]),
            size_2: [44, 64],
            nearest: true
        }]
    );
    assert_eq!(
        sum.total(&[ids[0], ids[1], ids[3], ids[4], ids[5]]),
        0,
        "solo esa baldosa"
    );
    assert_eq!(bench.canvas.pending_blocks(), 0);
    bench.checks_visible("tras verse");
}

#[test]
fn row_b7_at_rest_nothing_is_uploaded() {
    let mut bench = bench_300x200();
    let ids = bench.ids();
    for n in 0..4 {
        let output_2 = bench.rest_2_2();
        assert_eq!(output_2.uploads.total(&ids), 0, "fotograma {n}");
    }
    // Ni moviendo el puntero sobre el lienzo ni con el ratón quieto.
    for pos in [pos2(10.0, 10.0), pos2(80.0, 60.0), pos2(150.0, 110.0)] {
        let output_2 = bench.runs(vec![move_pointer(pos)], |_, _, _, _| {});
        assert_eq!(output_2.uploads.total(&ids), 0, "puntero en {pos:?}");
    }
    // Ni una invalidación vacía.
    let output_2 = bench.runs(vec![], |_, canvas, doc, _| {
        let t = size_of(doc);
        canvas.invalidate(&BlockSet::new(t.width, t.height));
    });
    assert_eq!(output_2.uploads.total(&ids), 0, "invalidación vacía");
    assert_eq!(bench.canvas.pending_blocks(), 0);
}

#[test]
fn row_b7_the_palette_commands_that_do_not_change_the_image_upload_nothing() {
    let mut bench = Bench::simple(128, image_of(Mode::Indexed, 300, 200, 7));
    bench.asks(ViewOp::ActualSize);
    let id = ImageId::from_raw(0);
    let ids = bench.ids();
    for (label_2, command) in [
        (
            "mover",
            Command::MovePaletteEntry {
                image: id,
                from: 1,
                to: 5,
            },
        ),
        (
            "permutar",
            Command::PermutePalette {
                image: id,
                order: vec![1, 2, 3, 4, 5, 6, 7, 0],
            },
        ),
        (
            "añadir",
            Command::AddPaletteEntry {
                image: id,
                color: Rgba::opaque(3, 4, 5),
            },
        ),
    ] {
        let output_2 = bench.applies(command);
        assert_eq!(output_2.uploads.total(&ids), 0, "{label_2}");
        assert_eq!(bench.canvas.pending_blocks(), 0, "{label_2}");
        bench.checks_visible(label_2);
    }
}

#[test]
fn row_b7_a_whole_image_operation_with_everything_visible_uploads_each_tile_once() {
    let mut bench = bench_300x200();
    bench.asks(ViewOp::Fit);
    bench.ensures_visible(&[true; 6], "Ajustar");
    let ids = bench.ids();
    let full = model_full(300, 200);
    // `invalidate_all`: un delta completo por baldosa.
    let output_2 = bench.runs(vec![], |_, canvas, _, _| canvas.invalidate_all());
    checks_uploads(&bench, &output_2, &full, "invalidate_all");
    for (i, id) in ids.iter().enumerate() {
        let r = bench.rect_de(i);
        assert_eq!(
            output_2.uploads.of_texture(*id),
            vec![Delta {
                pos: None,
                size_2: [r.width as usize, r.height as usize],
                nearest: true
            }],
            "baldosa {i}"
        );
    }
    // Una conversión de modo del documento: igual.
    let id = ImageId::from_raw(0);
    let output_2 = bench.applies(Command::ConvertMode {
        image: id,
        target: Mode::GrayAlpha,
        allowed: Allowed {
            luma: true,
            drop_alpha: None,
        },
    });
    checks_uploads(&bench, &output_2, &full, "conversión");
    assert_eq!(output_2.uploads.total(&ids), 6, "una por baldosa");
    bench.checks_visible("tras convertir");
    // Y un cambio de color de paleta de una imagen indexada.
    let mut indexed = Bench::simple(128, image_of(Mode::Indexed, 300, 200, 7));
    indexed.asks(ViewOp::Fit);
    let id = ImageId::from_raw(0);
    let output_2 = indexed.applies(Command::SetPaletteColor {
        image: id,
        index: 2,
        color: Rgba::opaque(5, 6, 7),
    });
    assert_eq!(output_2.uploads.total(&indexed.ids()), 6);
    indexed.checks_visible("tras cambiar el color");
}

#[test]
fn row_b7_a_whole_image_operation_with_part_outside_the_canvas_leaves_the_outside_ones_pending() {
    let mut bench = bench_300x200();
    let ids = bench.ids();
    let full = model_full(300, 200);
    let output_2 = bench.runs(vec![], |_, canvas, _, _| canvas.invalidate_all());
    checks_uploads(&bench, &output_2, &full, "invalidate_all con parte fuera");
    assert_eq!(
        output_2.uploads.total(&ids),
        4,
        "las cuatro visibles, una vez cada una"
    );
    // Las baldosas 2 y 5 (x ≥ 256) tienen 2 bloques cada una.
    assert_eq!(bench.canvas.pending_blocks(), 4);
    bench.asks(ViewOp::Fit);
    assert_eq!(bench.canvas.pending_blocks(), 0);
    bench.checks_visible("tras ajustar");
}

// ---------------------------------------------------------------------------------------------
// B8 · Superposición como textura a las cinco escalas
// ---------------------------------------------------------------------------------------------

/// Pantalla de B8 (a ppp 3 son 288×216 píxeles físicos).
const SCREEN_B_8: Vec2 = vec2(96.0, 72.0);
const TOL: f32 = 0.02;
const DESPLAZO_X: f64 = 0.0013;
const DESPLAZO_Y: f64 = 0.0007;

const BACKGROUNDS_B_8: [(CanvasBackground, bool); 5] = [
    (CanvasBackground::Checker, false),
    (CanvasBackground::Black, false),
    (CanvasBackground::Gray, false),
    (CanvasBackground::White, false),
    (CanvasBackground::Checker, true),
];

/// Lo que se ve: color premultiplicado, como egui.
#[derive(Clone, Copy, Debug)]
struct Seen {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl Seen {
    const EMPTY: Seen = Seen {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    fn de_color(c: Color32) -> Seen {
        Seen {
            r: f32::from(c.r()) / 255.0,
            g: f32::from(c.g()) / 255.0,
            b: f32::from(c.b()) / 255.0,
            a: f32::from(c.a()) / 255.0,
        }
    }

    /// `src` por encima de `self` (alfa premultiplicado).
    fn below_of(self, src: Seen) -> Seen {
        let k = 1.0 - src.a;
        Seen {
            r: src.r + self.r * k,
            g: src.g + self.g * k,
            b: src.b + self.b * k,
            a: src.a + self.a * k,
        }
    }

    fn per(self, o: Seen) -> Seen {
        Seen {
            r: self.r * o.r,
            g: self.g * o.g,
            b: self.b * o.b,
            a: self.a * o.a,
        }
    }

    fn near(&self, o: Seen, tolerance: f32) -> bool {
        (self.r - o.r).abs() <= tolerance
            && (self.g - o.g).abs() <= tolerance
            && (self.b - o.b).abs() <= tolerance
            && (self.a - o.a).abs() <= tolerance
    }
}

struct Buffer {
    width_2: usize,
    height_2: usize,
    pixels_2: Vec<Seen>,
}

impl Buffer {
    fn en(&self, x: i64, y: i64) -> Seen {
        self.pixels_2[y as usize * self.width_2 + x as usize]
    }
}

fn barycentric(p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<[f64; 3]> {
    let area = (b.0 - a.0) * (c.1 - a.1) - (c.0 - a.0) * (b.1 - a.1);
    if area.abs() < 1e-12 {
        return None;
    }
    let w0 = ((b.0 - p.0) * (c.1 - p.1) - (c.0 - p.0) * (b.1 - p.1)) / area;
    let w1 = ((c.0 - p.0) * (a.1 - p.1) - (a.0 - p.0) * (c.1 - p.1)) / area;
    let w2 = 1.0 - w0 - w1;
    (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0).then_some([w0, w1, w2])
}

/// Texel de la textura de 2×2 del damero (`A, B, B, A`), `NEAREST` y envoltura `Repeat`.
fn texel_of_the_checker(texture_2: &Texture, u: f64, v: f64) -> Color32 {
    let tx = ((u * 2.0).floor() as i64).rem_euclid(2) as usize;
    let ty = ((v * 2.0).floor() as i64).rem_euclid(2) as usize;
    texture_2.texels[ty * texture_2.size_2[0] + tx]
}

/// Pinta las primitivas, en orden, en un búfer de `width_2 × height_2` píxeles físicos, muestreando cada
/// píxel en su centro (algo desplazado). Las mallas de las baldosas se muestrean con
/// `Textures::samples` en el uv interpolado (`NEAREST`, recortado al borde); la del damero, con
/// envoltura; los rellenos y las mallas de color (textura por defecto) usan el color de sus vértices.
fn rasterizes(
    primitives: &[ClippedPrimitive],
    ppp: f32,
    width_2: usize,
    height_2: usize,
    textures_2: &Textures,
    checker_2: Option<TextureId>,
) -> Buffer {
    let ppp = f64::from(ppp);
    let mut pixels_2 = vec![Seen::EMPTY; width_2 * height_2];
    for primitive in primitives {
        let Primitive::Mesh(mesh) = &primitive.primitive else {
            continue;
        };
        let clip = primitive.clip_rect;
        let (cx0, cy0) = (f64::from(clip.min.x) * ppp, f64::from(clip.min.y) * ppp);
        let (cx1, cy1) = (f64::from(clip.max.x) * ppp, f64::from(clip.max.y) * ppp);
        let id = mesh.texture_id;
        let flat = id == TextureId::default();
        let is_checker = checker_2 == Some(id);
        assert!(
            flat || is_checker || textures_2.get(id).is_some(),
            "una malla con una textura desconocida: {id:?}"
        );
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let v: [&Vertex; 3] = [
                &mesh.vertices[triangle[0] as usize],
                &mesh.vertices[triangle[1] as usize],
                &mesh.vertices[triangle[2] as usize],
            ];
            let p = v.map(|v| (f64::from(v.pos.x) * ppp, f64::from(v.pos.y) * ppp));
            let min_x = p.iter().map(|q| q.0).fold(f64::INFINITY, f64::min);
            let max_x = p.iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max);
            let min_y = p.iter().map(|q| q.1).fold(f64::INFINITY, f64::min);
            let max_y = p.iter().map(|q| q.1).fold(f64::NEG_INFINITY, f64::max);
            let x0 = (min_x.floor() - 1.0).max(cx0.floor()).max(0.0) as usize;
            let y0 = (min_y.floor() - 1.0).max(cy0.floor()).max(0.0) as usize;
            let x1 = (max_x.ceil() + 1.0)
                .min(cx1.ceil())
                .min(width_2 as f64)
                .max(0.0) as usize;
            let y1 = (max_y.ceil() + 1.0)
                .min(cy1.ceil())
                .min(height_2 as f64)
                .max(0.0) as usize;
            let tints = v.map(|v| Seen::de_color(v.color));
            for py in y0..y1 {
                for px in x0..x1 {
                    let s = (px as f64 + 0.5 + DESPLAZO_X, py as f64 + 0.5 + DESPLAZO_Y);
                    if s.0 < cx0 || s.0 > cx1 || s.1 < cy0 || s.1 > cy1 {
                        continue;
                    }
                    let Some(w) = barycentric(s, p[0], p[1], p[2]) else {
                        continue;
                    };
                    let wf = w.map(|w| w as f32);
                    let tint = Seen {
                        r: wf[0] * tints[0].r + wf[1] * tints[1].r + wf[2] * tints[2].r,
                        g: wf[0] * tints[0].g + wf[1] * tints[1].g + wf[2] * tints[2].g,
                        b: wf[0] * tints[0].b + wf[1] * tints[1].b + wf[2] * tints[2].b,
                        a: wf[0] * tints[0].a + wf[1] * tints[1].a + wf[2] * tints[2].a,
                    };
                    let color = if flat {
                        tint
                    } else {
                        let u = w[0] * f64::from(v[0].uv.x)
                            + w[1] * f64::from(v[1].uv.x)
                            + w[2] * f64::from(v[2].uv.x);
                        let t = w[0] * f64::from(v[0].uv.y)
                            + w[1] * f64::from(v[1].uv.y)
                            + w[2] * f64::from(v[2].uv.y);
                        let texel = if is_checker {
                            texel_of_the_checker(textures_2.get(id).expect("damero"), u, t)
                        } else {
                            textures_2.samples(id, u, t).expect("una baldosa")
                        };
                        Seen::de_color(texel).per(tint)
                    };
                    let target_2 = &mut pixels_2[py * width_2 + px];
                    *target_2 = target_2.below_of(color);
                }
            }
        }
    }
    Buffer {
        width_2,
        height_2,
        pixels_2,
    }
}

/// La firma de lo pintado: recorte, textura, vértices e índices de cada malla.
type Signature = Vec<(Rect, TextureId, Vec<Vertex>, Vec<u32>)>;

fn signature(primitives: &[ClippedPrimitive]) -> Signature {
    primitives
        .iter()
        .filter_map(|p| match &p.primitive {
            Primitive::Mesh(m) => Some((
                p.clip_rect,
                m.texture_id,
                m.vertices.clone(),
                m.indices.clone(),
            )),
            Primitive::Callback(_) => None,
        })
        .collect()
}

/// Área de los triángulos de una malla, en píxeles físicos cuadrados (el menor de todos).
fn smaller_area_physical(vertices: &[Vertex], indices: &[u32], ppp: f32) -> Option<f64> {
    let k = f64::from(ppp);
    indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| vertices[i as usize].pos);
            let cross = f64::from(b.x - a.x) * f64::from(c.y - a.y)
                - f64::from(c.x - a.x) * f64::from(b.y - a.y);
            cross.abs() / 2.0 * k * k
        })
        .reduce(f64::min)
}

/// Ninguna forma de área cero: ni rectángulos vacíos ni triángulos degenerados, antes y después de
/// teselar (la regresión `b702d8e` de zoom < 1).
fn without_zero_area_shapes(
    shapes_2: &[ClippedShape],
    primitives: &[ClippedPrimitive],
    ppp: f32,
    label_2: &str,
) {
    fn walks(shape_2: &Shape, ppp: f32, label_2: &str) {
        match shape_2 {
            Shape::Rect(r) => assert!(
                r.rect.width() > 0.0 && r.rect.height() > 0.0,
                "{label_2}: rectángulo de área cero {:?}",
                r.rect
            ),
            Shape::Mesh(m) => {
                if let Some(area) = smaller_area_physical(&m.vertices, &m.indices, ppp) {
                    assert!(
                        area > 1e-6,
                        "{label_2}: triángulo degenerado (área {area} px²)"
                    );
                }
            }
            Shape::Vec(v) => v.iter().for_each(|f| walks(f, ppp, label_2)),
            _ => {}
        }
    }
    for shape_2 in shapes_2 {
        walks(&shape_2.shape, ppp, label_2);
    }
    for primitive in primitives {
        if let Primitive::Mesh(m) = &primitive.primitive
            && let Some(area) = smaller_area_physical(&m.vertices, &m.indices, ppp)
        {
            assert!(
                area > 1e-6,
                "{label_2}: triángulo degenerado tras teselar (área {area} px²)"
            );
        }
    }
}

fn gray_2(c: Rgba) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// Color del fondo elegido bajo la transparencia en el píxel físico `(px, py)`: el oráculo de la
/// 17b (`background_fill` y `shade_at`).
fn background_in(
    background_2: CanvasBackground,
    px: i64,
    py: i64,
    source_2: (i64, i64),
    cell: i64,
) -> Color32 {
    match background_fill(background_2) {
        Fill::Checker => gray_2(shade_at(px, py, source_2, cell).color()),
        Fill::Solid(color) => gray_2(color),
    }
}

/// Un color con alfa directo por encima de un fondo opaco.
fn composes(background_2: Color32, color: Rgba) -> Seen {
    Seen::de_color(background_2).below_of(Seen::de_color(texel(color)))
}

/// Geometría de lo pintado, en píxeles físicos, y el zoom como fracción.
struct Geom {
    source_2: (i64, i64),
    viewport: Size,
    cell: i64,
    num: i128,
    den: i128,
}

impl Geom {
    fn de(bench: &Bench, level: usize) -> Geom {
        let (num, den) = if level < 6 {
            (1, 1i128 << (6 - level))
        } else {
            ((level - 5) as i128, 1)
        };
        Geom {
            source_2: bench.source_2(),
            viewport: bench.viewport(),
            cell: 8 * i64::from(text_scale(bench.ppp)),
            num,
            den,
        }
    }

    fn extent(&self, n: u32) -> i64 {
        let q = i128::from(n) * self.num;
        (-((-q).div_euclid(self.den))) as i64
    }
}

/// El color esperado de un píxel físico: fuera de la imagen, `BACKGROUND`; dentro, el píxel de
/// imagen `floor(s / zoom)` (el pendiente, si lo hay, lo sustituye) sobre el fondo elegido.
fn seen_expected(
    g: &Geom,
    image_2: &Image,
    bytes: &[u8],
    pending_2: &HashMap<(u32, u32), Rgba>,
    background_2: CanvasBackground,
    px: i64,
    py: i64,
) -> Seen {
    let (sx, sy) = (px - g.source_2.0, py - g.source_2.1);
    if sx < 0 || sy < 0 || sx >= g.extent(image_2.width()) || sy >= g.extent(image_2.height()) {
        return Seen::de_color(BACKGROUND);
    }
    let ix = (i128::from(sx) * g.den / g.num) as u32;
    let iy = (i128::from(sy) * g.den / g.num) as u32;
    let color = pending_2.get(&(ix, iy)).copied().unwrap_or_else(|| {
        let i = (iy as usize * image_2.width() as usize + ix as usize) * 4;
        Rgba::new(bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3])
    });
    composes(
        background_in(background_2, px, py, g.source_2, g.cell),
        color,
    )
}

/// Los bloques de la 17b: píxeles rojos opacos (lápiz), transparentes (borrador) y rojos con alfa
/// 128, en rectángulos que cruzan los bordes de las baldosas de 16 de una imagen de 40×24.
fn pending_b_8() -> Vec<(u32, u32, Rgba)> {
    let mut v = Vec::new();
    let mut fills = |x0: u32, x1: u32, y0: u32, y1: u32, color: Rgba| {
        for y in y0..y1 {
            for x in x0..x1 {
                v.push((x, y, color));
            }
        }
    };
    fills(8, 20, 4, 10, RED);
    fills(20, 30, 4, 10, Rgba::TRANSPARENT);
    fills(8, 30, 14, 20, Rgba::new(255, 0, 0, 128));
    v
}

/// RGBA de 40×24 con un color distinto por posición y alfas 0 (las cuatro primeras columnas), 128
/// (las cuatro siguientes) y 255 (el resto).
fn image_b_8() -> Image {
    let mut image_2 = Image::new(40, 24, Mode::Rgba).expect("imagen");
    for y in 0..24u32 {
        for x in 0..40u32 {
            let alpha = match x {
                0..=3 => 0,
                4..=7 => 128,
                _ => 255,
            };
            let color = Rgba::new((x * 6) as u8, (y * 10) as u8, ((x * y) % 251) as u8, alpha);
            image_2.set_pixel(x, y, Pixel::Rgba(color)).expect("dentro");
        }
    }
    image_2
}

/// Compara todos los píxeles físicos del lienzo con el oráculo y devuelve los fallos.
fn compares_b_8(
    buffer: &Buffer,
    g: &Geom,
    image_2: &Image,
    pending_2: &HashMap<(u32, u32), Rgba>,
    background_2: CanvasBackground,
    label_2: &str,
) -> Vec<String> {
    let bytes = to_rgba8(image_2);
    let mut failures = Vec::new();
    let mut total = 0usize;
    for py in 0..buffer.height_2 as i64 {
        for px in 0..buffer.width_2 as i64 {
            let expected_2 = seen_expected(g, image_2, &bytes, pending_2, background_2, px, py);
            let seen = buffer.en(px, py);
            if !seen.near(expected_2, TOL) {
                total += 1;
                if failures.len() < 4 {
                    failures.push(format!(
                        "{label_2}: el píxel físico ({px}, {py}) se ve {seen:?} y debía verse {expected_2:?}"
                    ));
                }
            }
        }
    }
    if total > 0 {
        failures.push(format!(
            "{label_2}: {total} píxeles físicos distintos del oráculo"
        ));
    }
    failures
}

fn equal(a: &Buffer, b: &Buffer, label_2: &str) {
    assert_eq!((a.width_2, a.height_2), (b.width_2, b.height_2));
    let different = a
        .pixels_2
        .iter()
        .zip(&b.pixels_2)
        .filter(|(p, q)| !p.near(**q, 0.01))
        .count();
    assert_eq!(
        different, 0,
        "{label_2}: antes y después de soltar difieren en {different} píxeles"
    );
}

fn raster(bench: &Bench, output_2: &TestOutput) -> Buffer {
    let vp = bench.viewport();
    rasterizes(
        &bench.primitives(output_2),
        bench.ppp,
        vp.width as usize,
        vp.height as usize,
        &bench.textures_2,
        bench.canvas.checker_texture_id(),
    )
}

/// Un caso de B8: escala, nivel de zoom, fondo y camino del damero.
fn checks_b_8(ctx: &Context, ppp: f32, level: usize, background_2: CanvasBackground, mesh: bool) {
    let label_2 = format!(
        "ppp {ppp} nivel {level} fondo {background_2:?}{}",
        if mesh { " (malla)" } else { "" }
    );
    let image_2 = image_b_8();
    let mut canvas = Canvas::with_tile_side(16);
    canvas.set_background(background_2);
    canvas.set_checker_mesh(mesh);
    let mut bench = Bench::with(ctx.clone(), ppp, SCREEN_B_8, canvas, image_2);
    bench.go_to_level(level);
    let pending_2 = pending_b_8();
    let map: HashMap<(u32, u32), Rgba> = pending_2.iter().map(|&(x, y, c)| ((x, y), c)).collect();

    // Con la superposición: el trazo entero cada fotograma, como en la app.
    let mut output_2 = bench.rest_2_2();
    for _ in 0..2 {
        let pend = pending_2.clone();
        output_2 = bench.runs(vec![], move |ctx, canvas, doc, _| {
            canvas.set_overlay(ctx, doc.image(ImageId::from_raw(0)).expect("imagen"), &pend);
        });
    }
    assert_eq!(output_2.sync, Ok(()), "{label_2}");
    let g = Geom::de(&bench, level);
    let primitives = bench.primitives(&output_2);
    without_zero_area_shapes(
        &output_2.shapes_2,
        &primitives,
        ppp,
        &format!("{label_2} (con superposición)"),
    );
    let before = raster(&bench, &output_2);
    let failures = compares_b_8(
        &before,
        &g,
        bench.image_2(),
        &map,
        background_2,
        &format!("{label_2} durante el gesto"),
    );
    assert!(failures.is_empty(), "{failures:#?}");
    let shape_with = signature(&primitives);

    // Soltar: el trazo se aplica al documento y la superposición se descarta.
    let apply = Command::SetPixels {
        image: ImageId::from_raw(0),
        pixels: pending_2
            .iter()
            .map(|&(x, y, c)| PixelEdit {
                x,
                y,
                pixel: Pixel::Rgba(c),
            })
            .collect(),
    };
    bench.runs(vec![], move |_, canvas, doc, log| {
        let _ = applies_and_records(doc, log, canvas, apply);
        canvas.clear_overlay();
    });
    let output_2 = bench.rest_2_2();
    let g = Geom::de(&bench, level);
    let primitives = bench.primitives(&output_2);
    without_zero_area_shapes(
        &output_2.shapes_2,
        &primitives,
        ppp,
        &format!("{label_2} (tras soltar)"),
    );
    let after_2 = raster(&bench, &output_2);
    let without_pending = HashMap::new();
    let failures = compares_b_8(
        &after_2,
        &g,
        bench.image_2(),
        &without_pending,
        background_2,
        &format!("{label_2} tras soltar"),
    );
    assert!(failures.is_empty(), "{failures:#?}");
    equal(&before, &after_2, &label_2);
    // Ninguna forma ni malla de superposición: lo pintado con y sin ella es lo mismo.
    assert!(
        shape_with == signature(&primitives),
        "{label_2}: el trazo cambia las formas pintadas (¿hay formas de superposición?)"
    );
}

fn b8_a(ppp: f32) {
    let ctx = context(ppp);
    for level in [N_1_2, N_1_4, N_X1, N_X2, N_X3] {
        for (background_2, mesh) in BACKGROUNDS_B_8 {
            checks_b_8(&ctx, ppp, level, background_2, mesh);
        }
    }
}

#[test]
fn row_b8_the_overlay_as_a_texture_at_ppp_1_0() {
    b8_a(PPPS[0]);
}

#[test]
fn row_b8_the_overlay_as_a_texture_at_ppp_1_25() {
    b8_a(PPPS[1]);
}

#[test]
fn row_b8_the_overlay_as_a_texture_at_ppp_1_5() {
    b8_a(PPPS[2]);
}

#[test]
fn row_b8_the_overlay_as_a_texture_at_ppp_2_0() {
    b8_a(PPPS[3]);
}

#[test]
fn row_b8_the_overlay_as_a_texture_at_ppp_3_0() {
    b8_a(PPPS[4]);
}

#[test]
fn row_b8_the_eraser_shows_the_chosen_background_and_the_erased_color_is_the_same_before_and_after()
{
    // Solo el borrador (alfa 0) sobre píxeles opacos, ×3 y ppp 1: cada píxel borrado se ve como el
    // fondo (el damero o el color liso), antes de soltar y después.
    let ctx = context(1.0);
    for (background_2, mesh) in BACKGROUNDS_B_8 {
        let image_2 = image_b_8();
        let mut canvas = Canvas::with_tile_side(16);
        canvas.set_background(background_2);
        canvas.set_checker_mesh(mesh);
        let mut bench = Bench::with(ctx.clone(), 1.0, SCREEN_B_8, canvas, image_2);
        bench.go_to_level(N_X3);
        let deleted_2: Vec<(u32, u32, Rgba)> = (8..30)
            .flat_map(|x| (4..10).map(move |y| (x, y, Rgba::TRANSPARENT)))
            .collect();
        let map: HashMap<(u32, u32), Rgba> =
            deleted_2.iter().map(|&(x, y, c)| ((x, y), c)).collect();
        let pend = deleted_2.clone();
        let output_2 = bench.runs(vec![], move |ctx, canvas, doc, _| {
            canvas.set_overlay(ctx, doc.image(ImageId::from_raw(0)).expect("imagen"), &pend);
        });
        let g = Geom::de(&bench, N_X3);
        let before = raster(&bench, &output_2);
        // Lo borrado se ve exactamente como el fondo.
        for &(x, y, _) in &deleted_2 {
            let (px, py) = (
                g.source_2.0 + i64::from(x) * 3 + 1,
                g.source_2.1 + i64::from(y) * 3 + 1,
            );
            if px < 0
                || py < 0
                || px >= i64::from(g.viewport.width)
                || py >= i64::from(g.viewport.height)
            {
                continue;
            }
            let expected_2 =
                Seen::de_color(background_in(background_2, px, py, g.source_2, g.cell));
            assert!(
                before.en(px, py).near(expected_2, TOL),
                "{background_2:?} malla {mesh}: ({x}, {y}) no deja ver el fondo"
            );
        }
        let failures = compares_b_8(&before, &g, bench.image_2(), &map, background_2, "borrador");
        assert!(failures.is_empty(), "{failures:#?}");
        // Soltar con el borrador aplicado.
        let id = ImageId::from_raw(0);
        let delete = Command::SetPixels {
            image: id,
            pixels: deleted_2
                .iter()
                .map(|&(x, y, _)| PixelEdit {
                    x,
                    y,
                    pixel: Pixel::Rgba(Rgba::TRANSPARENT),
                })
                .collect(),
        };
        bench.runs(vec![], move |_, canvas, doc, log| {
            let _ = applies_and_records(doc, log, canvas, delete);
            canvas.clear_overlay();
        });
        let output_2 = bench.rest_2_2();
        let after_2 = raster(&bench, &output_2);
        equal(&before, &after_2, &format!("{background_2:?} malla {mesh}"));
    }
}

// ---------------------------------------------------------------------------------------------
// B9 · Vida de la superposición
// ---------------------------------------------------------------------------------------------

/// La imagen del documento con los píxeles pendientes escritos, en orden (oráculo con `pixel_rgba`
/// para los de la prueba, que ya llevan su `Rgba`).
fn with_pending(image_2: &Image, pending_2: &[(u32, u32, Rgba)]) -> Vec<Color32> {
    let mut texels = expected_2(image_2);
    let width_2 = image_2.width() as usize;
    for &(x, y, color) in pending_2 {
        texels[y as usize * width_2 + x as usize] = texel(color);
    }
    texels
}

fn checks_with_pending(bench: &Bench, pending_2: &[(u32, u32, Rgba)], label_2: &str) {
    let failures = bench.discrepancies(
        &with_pending(bench.image_2(), pending_2),
        &bench.visible_2(),
        None,
    );
    assert!(failures.is_empty(), "{label_2}: {failures:?}");
}

fn frame_with_overlay(bench: &mut Bench, pending_2: Vec<(u32, u32, Rgba)>) -> TestOutput {
    bench.runs(vec![], move |ctx, canvas, doc, _| {
        canvas.set_overlay(
            ctx,
            doc.image(ImageId::from_raw(0)).expect("imagen"),
            &pending_2,
        );
    })
}

/// Los bloques que cubren los deltas de un fotograma, por baldosa, como conjunto de la imagen.
fn blocks_uploaded(bench: &Bench, output_2: &TestOutput) -> Model {
    let mut m = Model::new();
    for (i, id) in bench.ids().iter().enumerate() {
        m.extend(blocks_covered(
            &output_2.uploads.of_texture(*id),
            bench.rect_de(i),
        ));
    }
    m
}

fn model(blocks: &[(u32, u32)]) -> Model {
    blocks.iter().copied().collect()
}

#[test]
fn row_b9_a_20_pixel_pencil_in_3_frames_writes_and_uploads_only_the_new_ones() {
    let mut bench = bench_300x200();
    let state_2 = bench.doc.state_id();
    let initial = bench.image_2().clone();
    // Siete píxeles en el bloque (1, 0) de la baldosa 0; siete en el (2, 0) de la 1; seis en el
    // (1, 2) de la 3.
    let f1 = strip(70, 45, 7, RED);
    let mut f2 = f1.clone();
    f2.extend(strip(130, 45, 7, GREEN));
    let mut f3 = f2.clone();
    f3.extend(strip(70, 130, 6, BLUE));
    assert_eq!(f3.len(), 20);
    for (n, (pend, blocks, live)) in [
        (f1, vec![(1, 0)], 1usize),
        (f2, vec![(2, 0)], 2),
        (f3, vec![(1, 2)], 3),
    ]
    .into_iter()
    .enumerate()
    {
        let output_2 = frame_with_overlay(&mut bench, pend.clone());
        assert_eq!(output_2.sync, Ok(()));
        assert_eq!(
            blocks_uploaded(&bench, &output_2),
            model(&blocks),
            "fotograma {}: solo los bloques con píxeles nuevos",
            n + 1
        );
        assert_eq!(bench.canvas.overlay_blocks(), live, "fotograma {}", n + 1);
        checks_with_pending(&bench, &pend, &format!("fotograma {}", n + 1));
        assert_eq!(
            bench.doc.state_id(),
            state_2,
            "el documento no cambia hasta soltar"
        );
        assert_eq!(bench.image_2(), &initial);
    }
    // Repetir el mismo trazo entero no sube nada.
    let output_2 = frame_with_overlay(&mut bench, {
        let mut p = strip(70, 45, 7, RED);
        p.extend(strip(130, 45, 7, GREEN));
        p.extend(strip(70, 130, 6, BLUE));
        p
    });
    assert_eq!(
        output_2.uploads.total(&bench.ids()),
        0,
        "ningún píxel nuevo"
    );
}

/// El trazo de tres tiras de `lapiz_de_20_pixeles`, para los finales de B9.
fn pending_of_20() -> Vec<(u32, u32, Rgba)> {
    let mut p = strip(70, 45, 7, RED);
    p.extend(strip(130, 45, 7, GREEN));
    p.extend(strip(70, 130, 6, BLUE));
    p
}

fn bench_b9_with_the_pencil_half_way() -> (Bench, Image, Vec<(u32, u32, Rgba)>) {
    let mut bench = bench_300x200();
    let initial = bench.image_2().clone();
    let pend = pending_of_20();
    frame_with_overlay(&mut bench, pend.clone());
    assert_eq!(bench.canvas.overlay_blocks(), 3);
    (bench, initial, pend)
}

fn after_the_gesture(bench: &mut Bench, expected_doc: &Image, label_2: &str) {
    assert_eq!(
        bench.canvas.overlay_blocks(),
        0,
        "{label_2}: la copia de trabajo se libera"
    );
    assert_eq!(bench.image_2(), expected_doc, "{label_2}: el documento");
    // La textura vuelve a ser el documento (con el trazo si se aplicó, sin él si no).
    let failures = bench.discrepancies(&expected_2(expected_doc), &bench.visible_2(), None);
    assert!(failures.is_empty(), "{label_2}: {failures:?}");
    bench.rest_2_2();
    bench.checks_all(label_2);
}

#[test]
fn row_b9_releasing_with_ok_true_leaves_the_texture_with_the_stroke_and_frees_the_copy() {
    let (mut bench, _, pend) = bench_b9_with_the_pencil_half_way();
    let id = ImageId::from_raw(0);
    let command = Command::SetPixels {
        image: id,
        pixels: pend
            .iter()
            .map(|&(x, y, c)| PixelEdit {
                x,
                y,
                pixel: Pixel::Rgba(c),
            })
            .collect(),
    };
    let output_2 = bench.runs(vec![], move |_, canvas, doc, log| {
        assert_eq!(applies_and_records(doc, log, canvas, command), Ok(true));
        canvas.clear_overlay();
    });
    assert_eq!(output_2.sync, Ok(()));
    let expected = bench.image_2().clone();
    for &(x, y, c) in &pend {
        assert_eq!(
            expected.get_pixel(x, y),
            Ok(Pixel::Rgba(c)),
            "el trazo está en el documento"
        );
    }
    after_the_gesture(&mut bench, &expected, "soltar con Ok(true)");
}

#[test]
fn row_b9_releasing_with_ok_false_returns_the_texture_to_the_document_without_the_stroke() {
    let (mut bench, initial, _) = bench_b9_with_the_pencil_half_way();
    // `Ok(false)`: el documento no cambió; el lienzo solo descarta la superposición.
    let state_2 = bench.doc.state_id();
    bench.runs(vec![], |_, canvas, _, _| canvas.clear_overlay());
    assert_eq!(bench.doc.state_id(), state_2);
    after_the_gesture(&mut bench, &initial, "soltar con Ok(false)");
}

#[test]
fn row_b9_escape_and_empty_set_overlay_return_the_texture_to_the_document() {
    let (mut bench, initial, _) = bench_b9_with_the_pencil_half_way();
    // Esc: la tecla llega al lienzo (que la emite en orden) y la app cancela el gesto; la
    // superposición vacía equivale a `clear_overlay`.
    let output_2 = bench.runs(
        vec![key_2(Key::Escape, Modifiers::NONE)],
        |ctx, canvas, doc, _| {
            canvas.set_overlay(ctx, doc.image(ImageId::from_raw(0)).expect("imagen"), &[]);
        },
    );
    assert!(output_2.entries.contains(&CanvasInput::Escape));
    after_the_gesture(&mut bench, &initial, "Esc");
}

#[test]
fn row_b9_an_apply_error_leaves_the_document_and_the_texture_as_before_the_gesture() {
    let (mut bench, initial, pend) = bench_b9_with_the_pencil_half_way();
    let id = ImageId::from_raw(0);
    // Un píxel de otro modo: `apply` da error y el documento queda intacto.
    let bad = Command::SetPixels {
        image: id,
        pixels: pend
            .iter()
            .map(|&(x, y, _)| PixelEdit {
                x,
                y,
                pixel: Pixel::Gray(7),
            })
            .collect(),
    };
    bench.runs(vec![], move |_, canvas, doc, log| {
        assert!(applies_and_records(doc, log, canvas, bad).is_err());
        canvas.clear_overlay();
    });
    after_the_gesture(&mut bench, &initial, "error de apply");
}

#[test]
fn row_b9_the_line_that_changes_end_uploads_the_blocks_of_the_previous_and_the_new_one() {
    let mut bench = bench_300x200();
    let initial = bench.image_2().clone();
    let state_2 = bench.doc.state_id();
    let line = |a: (i32, i32), b: (i32, i32)| -> Vec<(u32, u32, Rgba)> {
        Line::new(a, b)
            .clip_to(300, 200)
            .map(|(x, y)| (x as u32, y as u32, RED))
            .collect()
    };
    // Primera goma: (50,45)→(100,45), bloques (0,0) y (1,0) de la baldosa 0.
    let l1 = line((50, 45), (100, 45));
    let output_2 = frame_with_overlay(&mut bench, l1.clone());
    assert_eq!(blocks_uploaded(&bench, &output_2), model(&[(0, 0), (1, 0)]));
    assert_eq!(bench.canvas.overlay_blocks(), 2);
    checks_with_pending(&bench, &l1, "primera línea");
    // Cambia de extremo: (50,45)→(50,100), bloques (0,0) y (0,1). No es un prefijo: se sube lo de
    // la anterior (que vuelve al documento) y lo nuevo.
    let l2 = line((50, 45), (50, 100));
    let output_2 = frame_with_overlay(&mut bench, l2.clone());
    assert_eq!(
        blocks_uploaded(&bench, &output_2),
        model(&[(0, 0), (1, 0), (0, 1)]),
        "los bloques de la anterior y los de la nueva"
    );
    assert_eq!(
        bench.canvas.overlay_blocks(),
        2,
        "solo los de la línea actual"
    );
    checks_with_pending(
        &bench,
        &l2,
        "segunda línea (el bloque (1,0) volvió al documento)",
    );
    // Sin cambios: nada.
    let output_2 = frame_with_overlay(&mut bench, l2.clone());
    assert_eq!(output_2.uploads.total(&bench.ids()), 0);
    // De vuelta a la primera: otra vez la unión.
    let output_2 = frame_with_overlay(&mut bench, l1.clone());
    assert_eq!(
        blocks_uploaded(&bench, &output_2),
        model(&[(0, 0), (1, 0), (0, 1)])
    );
    checks_with_pending(&bench, &l1, "de vuelta a la primera");
    assert_eq!(bench.doc.state_id(), state_2);
    assert_eq!(bench.image_2(), &initial);
    // Soltar con la línea real del documento.
    let id = ImageId::from_raw(0);
    let command = line_command(&bench.doc, id, (50, 45), (100, 45), Pixel::Rgba(RED))
        .expect("línea")
        .expect("cae dentro");
    bench.runs(vec![], move |_, canvas, doc, log| {
        assert_eq!(applies_and_records(doc, log, canvas, command), Ok(true));
        canvas.clear_overlay();
    });
    let applied = bench.image_2().clone();
    after_the_gesture(&mut bench, &applied, "soltar la línea");
}

#[test]
fn row_b9_a_diagonal_line_of_8192_px_in_8192_touches_at_most_255_blocks() {
    // 8192×8192 en grises (64 MiB) con las baldosas por defecto de 2048. No se guardan las
    // texturas compuestas (serían otros 256 MiB): solo se cuentan los bloques de trabajo.
    let image_2 = Image::new(8192, 8192, Mode::Gray).expect("imagen de 64 MiB");
    let mut bench = Bench::empty(context(1.0), 1.0, TEST_SCREEN, Canvas::new(), image_2);
    bench.saves = false;
    let loaded = bench.doc.image(ImageId::from_raw(0)).expect("imagen");
    bench.canvas.load(&bench.ctx, loaded).expect("load");
    let _ = bench.rest_2_2();
    assert_eq!(bench.tiles_2(), 16, "8192² en baldosas de 2048");
    let line: Vec<(u32, u32, Rgba)> = Line::new((0, 0), (8191, 8190))
        .clip_to(8192, 8192)
        .map(|(x, y)| (x as u32, y as u32, Rgba::opaque(250, 250, 250)))
        .collect();
    assert_eq!(line.len(), 8192);
    let output_2 = frame_with_overlay(&mut bench, line);
    assert_eq!(output_2.sync, Ok(()));
    let blocks = bench.canvas.overlay_blocks();
    assert!(
        (128..=255).contains(&blocks),
        "la diagonal toca entre 128 y 255 bloques de 64×64 y toca {blocks}"
    );
    bench.runs(vec![], |_, canvas, _, _| canvas.clear_overlay());
    assert_eq!(bench.canvas.overlay_blocks(), 0);
}

// ---------------------------------------------------------------------------------------------
// B10 · Vista previa del editor
// ---------------------------------------------------------------------------------------------

const YELLOW: Rgba = Rgba::opaque(255, 255, 0);
const GREEN_YELLOWISH: Rgba = Rgba::opaque(128, 255, 0);

/// 300×100 indexada en 3 baldosas de 128; a ×1 se ven las dos primeras.
fn bench_b_10() -> Bench {
    let mut bench = Bench::simple(128, image_of(Mode::Indexed, 300, 100, 5));
    bench.asks(ViewOp::ActualSize);
    bench.ensures_visible(&[true, true, false], "300×100 a ×1");
    bench
}

fn expected_with_input(image_2: &Image, index: u8, color: Rgba) -> Vec<Color32> {
    to_rgba8_with_entry(image_2, index, color)
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]))
        .collect()
}

fn previews(bench: &mut Bench, index: u8, color: Rgba) -> TestOutput {
    bench.runs(vec![], move |ctx, canvas, doc, _| {
        canvas.preview_entry(
            ctx,
            doc.image(ImageId::from_raw(0)).expect("imagen"),
            index,
            color,
        );
    })
}

fn the_preview_ends(bench: &mut Bench) -> TestOutput {
    bench.runs(vec![], |ctx, canvas, doc, _| {
        canvas.end_preview(ctx, doc.image(ImageId::from_raw(0)).expect("imagen"));
    })
}

/// Solo las visibles reciben deltas.
fn checks_only_visible(bench: &Bench, output_2: &TestOutput, label_2: &str) {
    let ids = bench.ids();
    for (i, visible) in bench.visible_2().iter().enumerate() {
        let n = output_2.uploads.of_texture(ids[i]).len();
        assert_eq!(
            n > 0,
            *visible,
            "{label_2}: baldosa {i} (visible {visible}) recibió {n} deltas"
        );
    }
}

#[test]
fn row_b10_the_preview_only_rebuilds_and_uploads_the_visible_tiles_and_counts_as_today() {
    let mut bench = bench_b_10();
    let ids = bench.ids();
    let before = bench.contents();
    let doc = bench.image_2().clone();
    assert_eq!(bench.canvas.preview_uploads(), 0);

    // Tres valores iguales: una sola subida.
    let s1 = previews(&mut bench, 2, YELLOW);
    assert_eq!(bench.canvas.preview_uploads(), 1);
    checks_only_visible(&bench, &s1, "primer valor");
    let failures = bench.discrepancies(
        &expected_with_input(&doc, 2, YELLOW),
        &bench.visible_2(),
        Some(&before),
    );
    assert!(failures.is_empty(), "{failures:?}");
    for _ in 0..2 {
        let s = previews(&mut bench, 2, YELLOW);
        assert_eq!(bench.canvas.preview_uploads(), 1, "el valor se repite");
        assert_eq!(s.uploads.total(&ids), 0, "y no llega nada a la GPU");
    }
    // Un fotograma sin llamar a `preview_entry`: nada.
    assert_eq!(bench.rest_2_2().uploads.total(&ids), 0);
    assert_eq!(bench.canvas.preview_uploads(), 1);

    // Un valor nuevo: +1.
    let s2 = previews(&mut bench, 2, GREEN_YELLOWISH);
    assert_eq!(bench.canvas.preview_uploads(), 2);
    checks_only_visible(&bench, &s2, "valor nuevo");
    let failures = bench.discrepancies(
        &expected_with_input(&doc, 2, GREEN_YELLOWISH),
        &bench.visible_2(),
        Some(&before),
    );
    assert!(failures.is_empty(), "{failures:?}");

    // De vuelta al primero: +1.
    let s3 = previews(&mut bench, 2, YELLOW);
    assert_eq!(bench.canvas.preview_uploads(), 3);
    checks_only_visible(&bench, &s3, "de vuelta");
    let failures = bench.discrepancies(
        &expected_with_input(&doc, 2, YELLOW),
        &bench.visible_2(),
        Some(&before),
    );
    assert!(failures.is_empty(), "{failures:?}");
    // El documento no cambia.
    assert_eq!(bench.image_2(), &doc);
    assert_eq!(bench.doc.state_id(), 0);
}

#[test]
fn row_b10_a_pan_half_way_builds_the_tile_that_appears_already_with_the_preview() {
    let mut bench = bench_b_10();
    let ids = bench.ids();
    let doc = bench.image_2().clone();
    previews(&mut bench, 2, YELLOW);
    assert_eq!(bench.canvas.preview_uploads(), 1);
    // La baldosa 2 sigue como estaba.
    let doc_tile2 = clip_2(&expected_2(&doc), 300, bench.rect_de(2));
    assert_eq!(bench.content(2).texels, doc_tile2);
    // Se desplaza con la vista previa activa (la app repite `preview_entry` en cada fotograma).
    let mut sum = Uploads::default();
    for s in bench.drags_with(-120, 0, |ctx, canvas, doc, _| {
        canvas.preview_entry(
            ctx,
            doc.image(ImageId::from_raw(0)).expect("imagen"),
            2,
            YELLOW,
        );
    }) {
        sum.sum(s.uploads);
    }
    assert_eq!(
        bench.canvas.preview_uploads(),
        1,
        "es el mismo valor: no cuenta"
    );
    assert!(bench.visible_2()[2], "la baldosa 2 ya se ve");
    assert!(
        !sum.of_texture(ids[2]).is_empty(),
        "se construyó y se subió"
    );
    // Todas las visibles llevan la vista previa, también la que apareció.
    let failures = bench.discrepancies(
        &expected_with_input(&doc, 2, YELLOW),
        &bench.visible_2(),
        None,
    );
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn row_b10_the_transparent_entry_keeps_alpha_0_in_the_preview() {
    let mut bench = bench_b_10();
    let doc = bench.image_2().clone();
    let transparent = doc
        .palette()
        .and_then(|p| p.transparent_index())
        .expect("índice transparente");
    // Un rojo opaco para la entrada transparente: sus píxeles siguen invisibles.
    previews(&mut bench, transparent, RED);
    let oracle = expected_with_input(&doc, transparent, RED);
    let failures = bench.discrepancies(&oracle, &bench.visible_2(), None);
    assert!(failures.is_empty(), "{failures:?}");
    let mut seen_2 = 0;
    for i in [0usize, 1] {
        let r = bench.rect_de(i);
        let texels = bench.content(i).texels;
        for y in 0..r.height {
            for x in 0..r.width {
                if doc.get_pixel(r.x + x, r.y + y) == Ok(Pixel::Indexed(transparent)) {
                    assert_eq!(
                        texels[(y * r.width + x) as usize].a(),
                        0,
                        "({}, {})",
                        r.x + x,
                        r.y + y
                    );
                    seen_2 += 1;
                }
            }
        }
    }
    assert!(seen_2 > 0, "la imagen usa el índice transparente");
}

#[test]
fn row_b10_cancel_returns_the_visible_ones_to_the_document_and_the_non_visible_ones_when_seen() {
    let mut bench = bench_b_10();
    let doc = bench.image_2().clone();
    previews(&mut bench, 2, YELLOW);
    // Desplazar para que la baldosa 0 salga del lienzo mientras se previsualiza.
    bench.drags_with(-200, 0, |ctx, canvas, doc, _| {
        canvas.preview_entry(
            ctx,
            doc.image(ImageId::from_raw(0)).expect("imagen"),
            2,
            YELLOW,
        );
    });
    let visible_2 = bench.visible_2();
    assert!(!visible_2[0] && visible_2[2], "preparación: {visible_2:?}");
    // Cancelar: `end_preview`. Las visibles vuelven al documento ya.
    let output_2 = the_preview_ends(&mut bench);
    assert_eq!(output_2.sync, Ok(()));
    let failures = bench.discrepancies(&expected_2(&doc), &bench.visible_2(), None);
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(bench.image_2(), &doc, "cancelar no cambia el documento");
    // Las que se tocaron y no se veían vuelven al documento al verse.
    bench.checks_all("tras cancelar y ajustar");
    // Sin vista previa activa, `end_preview` no sube nada.
    let ids = bench.ids();
    assert_eq!(the_preview_ends(&mut bench).uploads.total(&ids), 0);
}

#[test]
fn row_b10_releasing_with_a_step_leaves_the_textures_of_the_new_document() {
    let mut bench = bench_b_10();
    let id = ImageId::from_raw(0);
    previews(&mut bench, 2, YELLOW);
    previews(&mut bench, 2, GREEN_YELLOWISH);
    // Soltar: la edición se aplica (`SetPaletteColor`), la app invalida y termina la vista previa.
    let command = Command::SetPaletteColor {
        image: id,
        index: 2,
        color: GREEN_YELLOWISH,
    };
    let output_2 = bench.runs(vec![], move |ctx, canvas, doc, log| {
        assert_eq!(applies_and_records(doc, log, canvas, command), Ok(true));
        canvas.end_preview(ctx, doc.image(ImageId::from_raw(0)).expect("imagen"));
    });
    assert_eq!(output_2.sync, Ok(()));
    let new_2 = bench.image_2().clone();
    let failures = bench.discrepancies(&expected_2(&new_2), &bench.visible_2(), None);
    assert!(failures.is_empty(), "{failures:?}");
    bench.checks_all("tras soltar con paso");
    // Y `undo` devuelve el color de antes en todas.
    bench.undoes();
    bench.checks_all("tras deshacer");
}

// ---------------------------------------------------------------------------------------------
// Q2 · Oráculo con secuencias aleatorias
// ---------------------------------------------------------------------------------------------

/// Un paso de Q2: la semilla de la operación, el nivel de zoom y el desplazamiento.
type StepQ2 = (Seed, u8, i8, i8);

fn step_q_2() -> impl Strategy<Value = StepQ2> {
    (seed(), 0u8..70, -60i8..=60, -60i8..=60)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Q2: secuencias aleatorias (0 a 12) de trazo, línea, relleno, borrador, `undo`, `redo`,
    /// conversión, edición de color, mover y quitar entrada, con zoom y vista aleatorios (también
    /// menores que 1) sobre imágenes de 1×1 a 40×40 con baldosas de 16: tras cada paso y un
    /// fotograma, la textura compuesta de todas las baldosas visibles es el oráculo; y tras llevar
    /// todas a la vista, **todas**.
    #[test]
    fn property_q2_the_composite_texture_is_the_oracle_in_the_visible_ones_and_in_all_at_the_end(
        mode_2 in 0u8..4,
        width_2 in 1u32..=40,
        height_2 in 1u32..=40,
        cel in 1u32..=6,
        steps_2 in proptest::collection::vec(step_q_2(), 0..=12),
    ) {
        let image_2 = image_of(TEST_MODES[usize::from(mode_2)], width_2, height_2, cel);
        let mut bench = Bench::with(
            context(1.0),
            1.0,
            vec2(96.0, 72.0),
            Canvas::with_tile_side(16),
            image_2,
        );
        for (n, (seed, level, dx, dy)) in steps_2.into_iter().enumerate() {
            bench.go_to_level(usize::from(level));
            bench.drags(i64::from(dx), i64::from(dy));
            let before = bench.contents();
            let output_2 = match step_of(&bench.doc, seed) {
                Step::Applies(command) => bench.applies(command),
                Step::Undo => bench.undoes(),
                Step::Redo => bench.redoes(),
                Step::Nothing => bench.rest_2_2(),
            };
            prop_assert_eq!(output_2.sync, Ok(()), "paso {}", n);
            let visible_2 = bench.visible_2();
            let failures = bench.discrepancies(&expected_2(bench.image_2()), &visible_2, Some(&before));
            prop_assert!(failures.is_empty(), "paso {} {:?}: {:?}", n, seed, failures);
        }
        bench.asks(ViewOp::Fit);
        bench.rest_2_2();
        let visible_2 = bench.visible_2();
        prop_assert!(visible_2.iter().all(|&v| v), "Ajustar deja todas a la vista: {:?}", visible_2);
        let failures = bench.discrepancies(&expected_2(bench.image_2()), &visible_2, None);
        prop_assert!(failures.is_empty(), "al final, todas: {:?}", failures);
    }
}

// ---------------------------------------------------------------------------------------------
// Q4 · Superposición con gestos aleatorios
// ---------------------------------------------------------------------------------------------

/// Los pendientes de un gesto de lápiz tal como los guarda `Stroke`: sin coordenadas repetidas, en
/// el orden de la primera visita y con el último valor.
fn without_repeated(points: &[(u32, u32, Pixel)]) -> Vec<(u32, u32, Pixel)> {
    let mut position: HashMap<(u32, u32), usize> = HashMap::new();
    let mut v: Vec<(u32, u32, Pixel)> = Vec::new();
    for &(x, y, pixel) in points {
        match position.get(&(x, y)) {
            Some(&i) => v[i].2 = pixel,
            None => {
                position.insert((x, y), v.len());
                v.push((x, y, pixel));
            }
        }
    }
    v
}

fn en_rgba(image_2: &Image, pending_2: &[(u32, u32, Pixel)]) -> Vec<(u32, u32, Rgba)> {
    pending_2
        .iter()
        .map(|&(x, y, p)| (x, y, pixel_rgba(image_2, p).expect("píxel del modo")))
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Q4: para cualquier gesto (píxeles al azar, varios fotogramas, línea o lápiz, aplicado,
    /// cancelado o con error), durante el gesto la textura compuesta de las baldosas visibles es el
    /// documento con los píxeles pendientes escritos (oráculo con `pixel_rgba`), el documento no
    /// cambia, y tras terminarlo la textura es el documento (con el trazo si se aplicó); con todas a
    /// la vista, todas.
    #[test]
    fn property_q4_during_the_gesture_the_texture_is_the_document_with_the_pending_ones_and_afterwards_the_document(
        mode_2 in 0u8..4,
        width_2 in 1u32..=200,
        height_2 in 1u32..=150,
        cel in 1u32..=9,
        is_line in any::<bool>(),
        eraser in any::<bool>(),
        raw_2_2 in proptest::collection::vec((any::<u32>(), any::<u32>(), any::<u32>()), 2..=40),
        frames in 1usize..=4,
        fin in 0u8..4,
        level in 0u8..70,
    ) {
        let image_2 = image_of(TEST_MODES[usize::from(mode_2)], width_2, height_2, cel);
        let entries = image_2.palette().map_or(1, |p| p.len());
        let mut bench = Bench::with(
            context(1.0),
            1.0,
            TEST_SCREEN,
            Canvas::with_tile_side(128),
            image_2.clone(),
        );
        bench.go_to_level(usize::from(level));
        let (state_2, initial) = (bench.doc.state_id(), bench.image_2().clone());

        // Los puntos y su valor: el del borrador si lo hay y se pide, y si no uno por punto.
        let value_of_the_gesture = deleted(&image_2).filter(|_| eraser);
        let points: Vec<(u32, u32, Pixel)> = raw_2_2
            .iter()
            .map(|&(a, b, k)| {
                (
                    a % width_2,
                    b % height_2,
                    value_of_the_gesture.unwrap_or_else(|| pixel_de(image_2.mode(), k, entries)),
                )
            })
            .collect();
        let pixel_line =
            value_of_the_gesture.unwrap_or_else(|| pixel_de(image_2.mode(), raw_2_2[0].2 % 97, entries));
        let n = points.len();
        // Fotograma `f` (de 1 a `frames`): el lápiz tiene el prefijo de `until(f)` puntos y la
        // línea va del primero al punto `end(f)`.
        let until = |f: usize| (f * n).div_ceil(frames).max(1);
        let end = |f: usize| (f * (n - 1)).div_ceil(frames).clamp(1, n - 1);
        let line_until = |f: usize| -> Vec<(u32, u32, Pixel)> {
            let (a, b) = (points[0], points[end(f)]);
            Line::new((a.0 as i32, a.1 as i32), (b.0 as i32, b.1 as i32))
                .clip_to(width_2, height_2)
                .map(|(x, y)| (x as u32, y as u32, pixel_line))
                .collect()
        };

        let mut last_2: Vec<(u32, u32, Pixel)> = Vec::new();
        for f in 1..=frames {
            let pending_2 = if is_line {
                line_until(f)
            } else {
                without_repeated(&points[..until(f)])
            };
            let rgba = en_rgba(&image_2, &pending_2);
            let output_2 = frame_with_overlay(&mut bench, rgba.clone());
            prop_assert_eq!(output_2.sync, Ok(()));
            let visible_2 = bench.visible_2();
            let failures = bench.discrepancies(&with_pending(&image_2, &rgba), &visible_2, None);
            prop_assert!(failures.is_empty(), "fotograma {}: {:?}", f, failures);
            prop_assert_eq!(bench.doc.state_id(), state_2);
            prop_assert_eq!(bench.image_2(), &initial);
            last_2 = pending_2;
        }

        // El final del gesto: aplicado, cancelado, con error o con Esc.
        let id = ImageId::from_raw(0);
        match fin {
            0 => {
                let command = if is_line {
                    let (a, b) = (points[0], points[end(frames)]);
                    line_command(&bench.doc, id, (a.0 as i32, a.1 as i32), (b.0 as i32, b.1 as i32), pixel_line)
                        .expect("línea")
                } else {
                    Some(Command::SetPixels {
                        image: id,
                        pixels: last_2.iter().map(|&(x, y, pixel)| PixelEdit { x, y, pixel }).collect(),
                    })
                };
                bench.runs(vec![], move |_, canvas, doc, log| {
                    if let Some(command) = command {
                        let _ = applies_and_records(doc, log, canvas, command);
                    }
                    canvas.clear_overlay();
                });
            }
            1 => {
                bench.runs(vec![], |_, canvas, _, _| canvas.clear_overlay());
            }
            2 => {
                // Un píxel fuera de la imagen: `apply` da error y el documento queda intacto.
                let bad = Command::SetPixels {
                    image: id,
                    pixels: vec![PixelEdit {
                        x: width_2,
                        y: 0,
                        pixel: pixel_de(image_2.mode(), 1, entries),
                    }],
                };
                bench.runs(vec![], move |_, canvas, doc, log| {
                    assert!(applies_and_records(doc, log, canvas, bad).is_err());
                    canvas.clear_overlay();
                });
            }
            _ => {
                bench.runs(vec![key_2(Key::Escape, Modifiers::NONE)], |ctx, canvas, doc, _| {
                    canvas.set_overlay(ctx, doc.image(ImageId::from_raw(0)).expect("imagen"), &[]);
                });
            }
        }
        prop_assert_eq!(bench.canvas.overlay_blocks(), 0, "la copia de trabajo se libera");
        if fin != 0 {
            prop_assert_eq!(bench.doc.state_id(), state_2);
            prop_assert_eq!(bench.image_2(), &initial);
        }
        let visible_2 = bench.visible_2();
        let failures = bench.discrepancies(&expected_2(bench.image_2()), &visible_2, None);
        prop_assert!(failures.is_empty(), "tras el gesto: {:?}", failures);
        bench.asks(ViewOp::Fit);
        bench.rest_2_2();
        let visible_2 = bench.visible_2();
        let failures = bench.discrepancies(&expected_2(bench.image_2()), &visible_2, None);
        prop_assert!(failures.is_empty(), "tras el gesto, todas: {:?}", failures);
    }
}
