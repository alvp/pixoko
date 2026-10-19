//! El lienzo con texturas por baldosas, subidas parciales y superposición como textura
//! (specs/hito-2-zoom-baldosas.md, unidad 14a-2: filas B5 a B10 y propiedades Q2 y Q4; contrato de
//! API en «Contrato de API de la 14a-2»).
//!
//! Banco de egui sin ventana (`Context::run_ui` con `RawInput` sintético, sin GPU ni capturas). Lo
//! que llega a la GPU es el `TexturesDelta` de cada fotograma: el banco lo cuenta (qué baldosa, si
//! es completo o parcial y de qué tamaño) y lo aplica **en orden** con `common::texturas::Texturas`,
//! que da la textura compuesta; esa se compara con el oráculo de la spec: `from_rgba_unmultiplied`
//! de cada píxel de `to_rgba8(documento)` (con los píxeles pendientes escritos durante un gesto y
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
//!   las mallas de las baldosas con `Texturas::muestrea`); ninguna forma de superposición ni de área
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
//!   ancho; 128 y 72 de alto) y, a ×1 tras `Actual`, se ven las baldosas 0, 1, 3 y 4. Cada prueba lo
//!   comprueba con un oráculo propio (`visibles`) y falla con un mensaje claro si la preparación no
//!   es la esperada.
//! - Las baldosas pequeñas (`with_tile_side(16)`) de B6, B8 y Q2 no son múltiplo de 64: a 1/64 casi
//!   ninguna tiene área (la columna 1 mide `⌈32/64⌉ − ⌈16/64⌉ = 0`), y el oráculo de lo visible es el
//!   de la spec (cuadrilátero con área positiva).
//! - La subida de la superposición se compara por **bloques cubiertos** (conjunto de bloques de 64×64
//!   que tocan los deltas), no por rectángulos exactos: la spec dice «los bloques cambiados» en la
//!   decisión 7 y «series» solo para las invalidaciones de B7.
//! - `overlay_blocks()` cuenta los bloques de trabajo vivos: 1, 2 y 3 con el lápiz de B9, y en la
//!   línea que cambia de extremo solo los de la línea actual (los anteriores «vuelven al documento»).

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
use common::texturas::{Textura, Texturas};
use common::{PPPS, boton, boton_de, contexto, mover, tecla};

mod tiles_support;
use tiles_support::{
    B, MODOS, Modelo, Paso, Semilla, borrado, esperado, imagen_de, modelo_de_pixeles, modelo_lleno,
    paso_de, pixel_de, recorte, runs_esperados, semilla, texel,
};

/// Pantalla de los bancos, en puntos (a ppp 1, en píxeles físicos).
const PANTALLA: Vec2 = vec2(160.0, 120.0);

const ROJO: Rgba = Rgba::opaque(255, 0, 0);
const VERDE: Rgba = Rgba::opaque(0, 255, 0);
const AZUL: Rgba = Rgba::opaque(0, 0, 255);

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
    tamano: [usize; 2],
    nearest: bool,
}

#[derive(Clone, Debug, Default)]
struct Subidas {
    deltas: HashMap<TextureId, Vec<Delta>>,
}

impl Subidas {
    fn de(delta: &egui::TexturesDelta) -> Subidas {
        let mut deltas: HashMap<TextureId, Vec<Delta>> = HashMap::new();
        for (id, lista) in &delta.set {
            for d in lista {
                let ImageData::Color(imagen) = &d.image;
                deltas.entry(*id).or_default().push(Delta {
                    pos: d.pos,
                    tamano: imagen.size,
                    nearest: d.options == TextureOptions::NEAREST,
                });
            }
        }
        Subidas { deltas }
    }

    fn de_textura(&self, id: TextureId) -> Vec<Delta> {
        self.deltas.get(&id).cloned().unwrap_or_default()
    }

    /// Los deltas de cada baldosa, en el orden de `ids`.
    fn por_baldosa(&self, ids: &[TextureId]) -> Vec<Vec<Delta>> {
        ids.iter().map(|&id| self.de_textura(id)).collect()
    }

    fn suma(&mut self, otra: Subidas) {
        for (id, lista) in otra.deltas {
            self.deltas.entry(id).or_default().extend(lista);
        }
    }

    /// Cuántos deltas llegaron a las baldosas `ids`.
    fn total(&self, ids: &[TextureId]) -> usize {
        ids.iter().map(|&id| self.de_textura(id).len()).sum()
    }
}

/// Lo que da un fotograma del banco.
struct Salida {
    formas: Vec<ClippedShape>,
    info: CanvasInfo,
    entradas: Vec<CanvasInput>,
    subidas: Subidas,
    sync: Result<(), CanvasError>,
}

// ---------------------------------------------------------------------------------------------
// Operaciones de documento con su región sucia (lo que hace `Session`)
// ---------------------------------------------------------------------------------------------

fn tamano_de(doc: &Document) -> Size {
    let imagen = doc.image(doc.main_image()).expect("imagen");
    Size::new(imagen.width(), imagen.height())
}

/// `dirty_of` antes de aplicar, `apply`, anotar en el registro y avisar al lienzo.
fn aplica_y_registra(
    doc: &mut Document,
    log: &mut ChangeLog,
    canvas: &mut Canvas,
    comando: Command,
) -> Result<bool, DocumentError> {
    let region = dirty_of(&comando, tamano_de(doc));
    let resultado = doc.apply(comando);
    if let Ok(true) = resultado {
        log.record(doc.state_id(), region.clone());
        canvas.invalidate(&region);
    }
    resultado
}

/// `undo` e invalidación de `log[desde]` (toda la imagen si falta la clave).
fn deshaz(doc: &mut Document, log: &ChangeLog, canvas: &mut Canvas) -> bool {
    let tamano = tamano_de(doc);
    let desde = doc.state_id();
    if doc.undo().is_err() {
        return false;
    }
    let region = log
        .get(desde)
        .cloned()
        .unwrap_or_else(|| BlockSet::full(tamano.width, tamano.height));
    canvas.invalidate(&region);
    true
}

/// `redo` e invalidación de `log[hacia]`.
fn rehaz(doc: &mut Document, log: &ChangeLog, canvas: &mut Canvas) -> bool {
    let tamano = tamano_de(doc);
    if doc.redo().is_err() {
        return false;
    }
    let hacia = doc.state_id();
    let region = log
        .get(hacia)
        .cloned()
        .unwrap_or_else(|| BlockSet::full(tamano.width, tamano.height));
    canvas.invalidate(&region);
    true
}

fn indice_de(zoom: Zoom) -> usize {
    Zoom::levels().position(|z| z == zoom).expect("un nivel")
}

// ---------------------------------------------------------------------------------------------
// El banco
// ---------------------------------------------------------------------------------------------

struct Banco {
    ctx: Context,
    ppp: f32,
    pantalla: Vec2,
    canvas: Canvas,
    doc: Document,
    log: ChangeLog,
    texturas: Texturas,
    max_textura: Option<usize>,
    /// Aplicar los deltas a `texturas` (las pruebas de memoria grande lo desactivan).
    guarda: bool,
    info: Option<CanvasInfo>,
}

impl Banco {
    /// Sin cargar nada en el lienzo.
    fn vacio(ctx: Context, ppp: f32, pantalla: Vec2, canvas: Canvas, image: Image) -> Banco {
        Banco {
            ctx,
            ppp,
            pantalla,
            canvas,
            doc: Document::new(image),
            log: ChangeLog::new(),
            texturas: Texturas::new(),
            max_textura: None,
            guarda: true,
            info: None,
        }
    }

    /// Con la imagen cargada (`load`) y tres fotogramas de reposo, ya aplicados los deltas.
    fn con(ctx: Context, ppp: f32, pantalla: Vec2, canvas: Canvas, image: Image) -> Banco {
        let mut banco = Banco::vacio(ctx, ppp, pantalla, canvas, image);
        let imagen = banco.doc.image(banco.doc.main_image()).expect("imagen");
        banco.canvas.load(&banco.ctx, imagen).expect("load");
        for _ in 0..3 {
            banco.reposo();
        }
        banco
    }

    /// ppp 1, la pantalla de siempre y baldosas del lado dado.
    fn simple(lado: usize, image: Image) -> Banco {
        Banco::con(
            contexto(1.0),
            1.0,
            PANTALLA,
            Canvas::with_tile_side(lado),
            image,
        )
    }

    fn imagen(&self) -> &Image {
        self.doc.image(self.doc.main_image()).expect("imagen")
    }

    /// Un fotograma: `interact`, la fase del documento (que puede llamar a `invalidate`,
    /// `set_overlay`, `preview_entry`…), `sync` y `paint`.
    fn corre<F>(&mut self, events: Vec<Event>, fase: F) -> Salida
    where
        F: FnOnce(&Context, &mut Canvas, &mut Document, &mut ChangeLog),
    {
        let mut fase = Some(fase);
        let mut capturado: Option<(CanvasInfo, Vec<CanvasInput>, Result<(), CanvasError>)> = None;
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.pantalla)),
            events,
            max_texture_side: self.max_textura,
            ..RawInput::default()
        };
        let canvas = &mut self.canvas;
        let doc = &mut self.doc;
        let log = &mut self.log;
        let mut salida = self.ctx.run_ui(entrada, |ui| {
            let entradas = canvas.interact_ordered(ui);
            if let Some(fase) = fase.take() {
                fase(ui.ctx(), canvas, doc, log);
            }
            let id = doc.main_image();
            let imagen = doc.image(id).expect("imagen");
            let sync = canvas.sync(ui.ctx(), imagen);
            let info = canvas.paint(ui);
            capturado = Some((info, entradas, sync));
        });
        let subidas = Subidas::de(&salida.textures_delta);
        if self.guarda {
            self.texturas.aplica(&salida.textures_delta);
        }
        salida.textures_delta.clear();
        let formas = std::mem::take(&mut salida.shapes);
        let (info, entradas, sync) = capturado.expect("el lienzo se dibuja");
        self.info = Some(info.clone());
        Salida {
            formas,
            info,
            entradas,
            subidas,
            sync,
        }
    }

    fn reposo(&mut self) -> Salida {
        self.corre(vec![], |_, _, _, _| {})
    }

    fn aplica(&mut self, comando: Command) -> Salida {
        self.corre(vec![], move |_, canvas, doc, log| {
            let _ = aplica_y_registra(doc, log, canvas, comando);
        })
    }

    fn deshace(&mut self) -> Salida {
        self.corre(vec![], |_, canvas, doc, log| {
            deshaz(doc, log, canvas);
        })
    }

    fn rehace(&mut self) -> Salida {
        self.corre(vec![], |_, canvas, doc, log| {
            rehaz(doc, log, canvas);
        })
    }

    fn pide(&mut self, op: ViewOp) -> Salida {
        self.canvas.request_view(op);
        self.reposo()
    }

    /// Lleva el zoom al nivel `objetivo` de la escalera con peticiones de vista (ancladas en el
    /// centro) y un solo fotograma.
    fn al_nivel(&mut self, objetivo: usize) -> Salida {
        let actual = indice_de(self.info.as_ref().expect("hubo un fotograma").zoom);
        for _ in actual..objetivo {
            self.canvas.request_view(ViewOp::ZoomIn);
        }
        for _ in objetivo..actual {
            self.canvas.request_view(ViewOp::ZoomOut);
        }
        let salida = self.reposo();
        assert_eq!(
            indice_de(salida.info.zoom),
            objetivo,
            "el banco no llegó al nivel {objetivo}"
        );
        salida
    }

    /// Arrastra la vista con el botón central `dx` y `dy` píxeles físicos (si el límite de la vista
    /// no lo impide), un fotograma por paso, llamando a `fase` en cada uno. egui no decide que es
    /// un arrastre hasta que el puntero se aleja de donde se pulsó: el primer movimiento va lejos
    /// (40 puntos) y el segundo vuelve al destino, así el desplazamiento neto es `(dx, dy)`.
    fn arrastra_con<F>(&mut self, dx: i64, dy: i64, mut fase: F) -> Vec<Salida>
    where
        F: FnMut(&Context, &mut Canvas, &mut Document, &mut ChangeLog),
    {
        let inicio = pos2(self.pantalla.x / 2.0, self.pantalla.y / 2.0);
        let lejos = inicio + vec2(40.0, 40.0);
        let destino = inicio + vec2(dx as f32 / self.ppp, dy as f32 / self.ppp);
        let pasos = [
            vec![mover(inicio)],
            vec![boton_de(inicio, PointerButton::Middle, true)],
            vec![mover(lejos)],
            vec![mover(destino)],
            vec![boton_de(destino, PointerButton::Middle, false)],
            vec![],
        ];
        pasos
            .into_iter()
            .map(|events| self.corre(events, |a, b, c, d| fase(a, b, c, d)))
            .collect()
    }

    fn arrastra(&mut self, dx: i64, dy: i64) -> Vec<Salida> {
        self.arrastra_con(dx, dy, |_, _, _, _| {})
    }

    fn viewport(&self) -> Size {
        Size::new(
            (self.pantalla.x * self.ppp).round() as u32,
            (self.pantalla.y * self.ppp).round() as u32,
        )
    }

    fn baldosas(&self) -> usize {
        self.canvas.tile_count()
    }

    fn rect_de(&self, i: usize) -> TileRect {
        self.canvas.tile_rect(i)
    }

    fn ids(&self) -> Vec<TextureId> {
        self.canvas.texture_ids()
    }

    /// El origen de la imagen en píxeles físicos, del último fotograma.
    fn origen(&self) -> (i64, i64) {
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
    fn visibles(&self) -> Vec<bool> {
        let zoom = self.info.as_ref().expect("hubo un fotograma").zoom;
        let (ox, oy) = self.origen();
        let vp = self.viewport();
        (0..self.baldosas())
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

    fn asegura_visibles(&self, esperado: &[bool], etiqueta: &str) {
        assert_eq!(
            self.visibles(),
            esperado,
            "{etiqueta}: la preparación del banco no es la esperada"
        );
    }

    /// El contenido compuesto de la baldosa `i`.
    fn contenido(&self, i: usize) -> Textura {
        self.texturas
            .get(self.ids()[i])
            .unwrap_or_else(|| panic!("la baldosa {i} no tiene textura"))
            .clone()
    }

    fn contenidos(&self) -> Vec<Textura> {
        (0..self.baldosas()).map(|i| self.contenido(i)).collect()
    }

    /// Diferencias con el oráculo: las baldosas de `deben` (visibles) tienen que ser idénticas a la
    /// imagen esperada; las demás, idénticas a `antes` (si se da).
    fn discrepancias(
        &self,
        esperada: &[Color32],
        deben: &[bool],
        antes: Option<&[Textura]>,
    ) -> Vec<String> {
        let ancho = self.imagen().width() as usize;
        let mut fallos = Vec::new();
        for i in 0..self.baldosas() {
            let r = self.rect_de(i);
            let actual = self.contenido(i);
            if deben[i] {
                let tamano = [r.width as usize, r.height as usize];
                if actual.tamano != tamano {
                    fallos.push(format!(
                        "baldosa {i}: tamaño {:?}, esperado {tamano:?}",
                        actual.tamano
                    ));
                } else if actual.texels != recorte(esperada, ancho, r) {
                    let n = actual
                        .texels
                        .iter()
                        .zip(recorte(esperada, ancho, r))
                        .filter(|(a, b)| **a != *b)
                        .count();
                    fallos.push(format!(
                        "baldosa {i} ({r:?}) visible: {n} texels distintos del oráculo"
                    ));
                }
            } else if let Some(antes) = antes
                && actual != antes[i]
            {
                fallos.push(format!("baldosa {i} no visible: cambió su contenido"));
            }
        }
        fallos
    }

    /// Todas las baldosas visibles son el oráculo del documento.
    fn comprueba_visibles(&self, etiqueta: &str) {
        let fallos = self.discrepancias(&esperado(self.imagen()), &self.visibles(), None);
        assert!(fallos.is_empty(), "{etiqueta}: {fallos:?}");
    }

    /// Lleva todas las baldosas a la vista (Ajustar) y comprueba **todas**.
    fn comprueba_todas(&mut self, etiqueta: &str) {
        self.pide(ViewOp::Fit);
        self.reposo();
        let visibles = self.visibles();
        assert!(
            visibles.iter().all(|&v| v),
            "{etiqueta}: Ajustar debía dejar todas a la vista: {visibles:?}"
        );
        let fallos = self.discrepancias(&esperado(self.imagen()), &visibles, None);
        assert!(fallos.is_empty(), "{etiqueta} (todas): {fallos:?}");
    }

    fn primitivas(&self, salida: &Salida) -> Vec<ClippedPrimitive> {
        self.ctx.tessellate(salida.formas.clone(), self.ppp)
    }
}

/// Asignador falso: da memoria las primeras `buenas` veces y después falla.
struct Falla {
    buenas: usize,
    llamadas: usize,
}

impl Falla {
    fn tras(buenas: usize) -> Falla {
        Falla {
            buenas,
            llamadas: 0,
        }
    }
}

impl TileAlloc for Falla {
    fn pixels(&mut self, len: usize) -> Option<Vec<Color32>> {
        self.llamadas += 1;
        (self.llamadas <= self.buenas).then(|| Vec::with_capacity(len))
    }
}

/// Las series de los bloques sucios de una baldosa como deltas (completo si están todos).
fn deltas_esperados(modelo: &Modelo, tile: TileRect) -> Vec<Delta> {
    let cortan = |bx: u32, by: u32| tiles_support::corta(bx, by, tile);
    let sucios = modelo.iter().filter(|&&(bx, by)| cortan(bx, by)).count();
    let de_la_baldosa = ((tile.x + tile.width - 1) / B - tile.x / B + 1) as usize
        * ((tile.y + tile.height - 1) / B - tile.y / B + 1) as usize;
    let tamano = [tile.width as usize, tile.height as usize];
    if sucios == 0 {
        return Vec::new();
    }
    if sucios == de_la_baldosa {
        return vec![Delta {
            pos: None,
            tamano,
            nearest: true,
        }];
    }
    let mut v: Vec<Delta> = runs_esperados(modelo, tile)
        .into_iter()
        .map(|r| Delta {
            pos: Some([(r.x - tile.x) as usize, (r.y - tile.y) as usize]),
            tamano: [r.width as usize, r.height as usize],
            nearest: true,
        })
        .collect();
    v.sort();
    v
}

/// Los bloques (en la imagen) que cubren los deltas de una baldosa.
fn bloques_cubiertos(deltas: &[Delta], tile: TileRect) -> Modelo {
    let mut m = Modelo::new();
    for d in deltas {
        let [px, py] = d.pos.unwrap_or([0, 0]);
        let (x0, y0) = (tile.x + px as u32, tile.y + py as u32);
        let (x1, y1) = (x0 + d.tamano[0] as u32, y0 + d.tamano[1] as u32);
        for by in y0 / B..=(y1 - 1) / B {
            for bx in x0 / B..=(x1 - 1) / B {
                m.insert((bx, by));
            }
        }
    }
    m
}

/// Banco de las filas B7 y B9: 300×200 RGBA en baldosas de 128 y vista a ×1.
fn banco_300x200() -> Banco {
    let mut banco = Banco::simple(128, imagen_de(Mode::Rgba, 300, 200, 7));
    banco.pide(ViewOp::Actual);
    banco.asegura_visibles(&[true, true, false, true, true, false], "300×200 a ×1");
    banco
}

fn tira(x0: u32, y: u32, n: u32, color: Rgba) -> Vec<(u32, u32, Rgba)> {
    (0..n).map(|i| (x0 + i, y, color)).collect()
}

// ---------------------------------------------------------------------------------------------
// B5 · Carga atómica
// ---------------------------------------------------------------------------------------------

fn tamanos_de_baldosas(banco: &Banco) -> Vec<[usize; 2]> {
    (0..banco.baldosas())
        .map(|i| {
            let r = banco.rect_de(i);
            [r.width as usize, r.height as usize]
        })
        .collect()
}

#[test]
fn fila_b5_load_crea_una_textura_nearest_por_baldosa_del_tamano_recortado_subida_en_el_primer_fotograma()
 {
    // Una baldosa y 3×2 baldosas (con recorte en el borde derecho y en el inferior).
    type Caso = ((u32, u32), usize, Vec<[usize; 2]>);
    let casos: [Caso; 2] = [
        ((10, 8), 16, vec![[10, 8]]),
        (
            (40, 24),
            16,
            vec![[16, 16], [16, 16], [8, 16], [16, 8], [16, 8], [8, 8]],
        ),
    ];
    for ((w, h), lado, tamanos) in casos {
        let imagen = imagen_de(Mode::Rgba, w, h, 3);
        let mut banco = Banco::vacio(
            contexto(1.0),
            1.0,
            PANTALLA,
            Canvas::with_tile_side(lado),
            imagen.clone(),
        );
        assert_eq!(banco.canvas.load(&banco.ctx, &imagen), Ok(()));
        let primero = banco.reposo();
        let ids = banco.ids();
        let etiqueta = format!("{w}×{h} con baldosas de {lado}");
        assert_eq!(
            ids.len(),
            tamanos.len(),
            "{etiqueta}: una textura por baldosa"
        );
        assert_eq!(banco.baldosas(), tamanos.len(), "{etiqueta}");
        assert_eq!(tamanos_de_baldosas(&banco), tamanos, "{etiqueta}");
        assert_eq!(
            ids.iter().collect::<HashSet<_>>().len(),
            ids.len(),
            "{etiqueta}: texturas distintas"
        );
        assert_eq!(banco.canvas.texture_id(), Some(ids[0]), "{etiqueta}");
        // Subidas: un delta completo `NEAREST` del tamaño recortado por baldosa, en el primer
        // fotograma, y ninguno después.
        for (i, deltas) in primero.subidas.por_baldosa(&ids).iter().enumerate() {
            assert_eq!(
                deltas,
                &vec![Delta {
                    pos: None,
                    tamano: tamanos[i],
                    nearest: true
                }],
                "{etiqueta}: baldosa {i} en el primer fotograma"
            );
        }
        for n in 0..3 {
            let despues = banco.reposo();
            assert_eq!(
                despues.subidas.total(&ids),
                0,
                "{etiqueta}: fotograma {n} después"
            );
        }
        // Todas las baldosas se construyeron al cargar: el contenido es el oráculo.
        let fallos = banco.discrepancias(&esperado(&imagen), &vec![true; ids.len()], None);
        assert!(fallos.is_empty(), "{etiqueta}: {fallos:?}");
    }
}

#[test]
fn fila_b5_con_un_asignador_que_falla_en_la_baldosa_4_el_lienzo_anterior_queda_intacto() {
    let anterior = imagen_de(Mode::Rgba, 40, 24, 3);
    let mut banco = Banco::simple(16, anterior.clone());
    // Un lienzo con vista propia y un gesto de dibujo en curso.
    banco.al_nivel(N_X3);
    let centro = pos2(80.0, 60.0);
    banco.corre(vec![mover(centro), boton(centro, true)], |_, _, _, _| {});
    let antes = banco.reposo();
    let (zoom, rect) = (antes.info.zoom, antes.info.image_rect);
    let ids = banco.ids();
    let contenidos = banco.contenidos();
    let vivas = banco.ctx.tex_manager().read().num_allocated();

    // 80×48 con baldosas de 16 son 15 baldosas: la 4.ª reserva falla.
    let nueva = imagen_de(Mode::Gray, 80, 48, 5);
    let mut falla = Falla::tras(3);
    assert_eq!(
        banco.canvas.load_with(&banco.ctx, &nueva, &mut falla),
        Err(CanvasError::OutOfMemory)
    );
    assert_eq!(falla.llamadas, 4, "la carga para en la baldosa que falla");

    // Sin texturas nuevas vivas, ahora ni tras el fotograma que libera lo que hubiera.
    assert_eq!(banco.ctx.tex_manager().read().num_allocated(), vivas);
    let tras = banco.reposo();
    assert_eq!(banco.ctx.tex_manager().read().num_allocated(), vivas);
    assert_eq!(banco.ids(), ids, "las texturas de antes");
    assert_eq!(
        tras.subidas.total(&ids),
        0,
        "ni una subida de lo que ya estaba"
    );
    assert_eq!(banco.contenidos(), contenidos);
    let fallos = banco.discrepancias(&esperado(&anterior), &vec![true; ids.len()], None);
    assert!(fallos.is_empty(), "{fallos:?}");
    let en_gpu: HashSet<TextureId> = banco.texturas.ids().collect();
    for id in &en_gpu {
        assert!(
            ids.contains(id)
                || Some(*id) == banco.canvas.checker_texture_id()
                || *id == TextureId::default(),
            "una textura que sobra en la GPU: {id:?}"
        );
    }
    // La vista y el gesto: el mismo nivel y el mismo rectángulo, y el trazo sigue y termina.
    assert_eq!(tras.info.zoom, zoom);
    assert_eq!(tras.info.image_rect, rect);
    assert_eq!(banco.baldosas(), 6);
    let seguido = banco.corre(vec![mover(centro + vec2(6.0, 0.0))], |_, _, _, _| {});
    assert!(
        seguido.entradas.iter().any(|e| matches!(
            e,
            CanvasInput::Stroke(px_ui::canvas::CanvasEvent::StrokeMove(_))
        )),
        "el gesto sigue: {:?}",
        seguido.entradas
    );
    let fin = banco.corre(vec![boton(centro + vec2(6.0, 0.0), false)], |_, _, _, _| {});
    assert!(
        fin.entradas.iter().any(|e| matches!(
            e,
            CanvasInput::Stroke(px_ui::canvas::CanvasEvent::StrokeEnd)
        )),
        "y termina: {:?}",
        fin.entradas
    );

    // Con memoria otra vez, la carga sí cambia la imagen y la vista vuelve al ajuste.
    assert_eq!(banco.canvas.load(&banco.ctx, &nueva), Ok(()));
    banco.doc = Document::new(nueva.clone());
    let despues = banco.reposo();
    assert_eq!(banco.baldosas(), 15);
    assert_ne!(banco.ids(), ids);
    let ajuste = px_ui::view::View::fit(banco.viewport(), Size::new(80, 48)).zoom();
    assert_eq!(despues.info.zoom, ajuste, "la vista vuelve al ajuste");
}

#[test]
fn fila_b5_si_la_primera_carga_falla_no_queda_nada() {
    let imagen = imagen_de(Mode::Rgba, 40, 24, 3);
    let ctx = contexto(1.0);
    let vivas = ctx.tex_manager().read().num_allocated();
    let mut canvas = Canvas::with_tile_side(16);
    let mut falla = Falla::tras(3);
    assert_eq!(
        canvas.load_with(&ctx, &imagen, &mut falla),
        Err(CanvasError::OutOfMemory)
    );
    assert_eq!(canvas.tile_count(), 0);
    assert_eq!(canvas.texture_id(), None);
    assert!(canvas.texture_ids().is_empty());
    assert_eq!(ctx.tex_manager().read().num_allocated(), vivas);
}

#[test]
fn fila_b5_una_imagen_de_lado_16385_da_too_large_con_max_16384_y_deja_el_lienzo_como_estaba() {
    assert_eq!(Canvas::MAX_SIDE, 16384);
    for (w, h) in [(16385u32, 1u32), (1, 16385), (20000, 3)] {
        let grande = Image::new(w, h, Mode::Gray).expect("imagen de un píxel de alto");
        let mut canvas = Canvas::new();
        assert_eq!(
            canvas.load(&contexto(1.0), &grande),
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
    let justo = Image::new(16384, 1, Mode::Gray).expect("imagen");
    let mut banco = Banco::vacio(contexto(1.0), 1.0, PANTALLA, Canvas::new(), justo.clone());
    assert_eq!(banco.canvas.load(&banco.ctx, &justo), Ok(()));
    banco.reposo();
    assert_eq!(banco.baldosas(), 8);
    // Con un lienzo anterior, el rechazo no lo toca.
    let anterior = imagen_de(Mode::Rgba, 40, 24, 3);
    let mut banco = Banco::simple(16, anterior);
    let ids = banco.ids();
    let grande = Image::new(16385, 1, Mode::Gray).expect("imagen");
    assert!(matches!(
        banco.canvas.load(&banco.ctx, &grande),
        Err(CanvasError::TooLarge { max: 16384, .. })
    ));
    let tras = banco.reposo();
    assert_eq!(banco.ids(), ids);
    assert_eq!(tras.subidas.total(&ids), 0);
    banco.comprueba_visibles("tras el rechazo");
}

#[test]
fn fila_b5_set_image_y_set_owned_image_construyen_en_el_primer_fotograma_como_antes() {
    for propia in [false, true] {
        let imagen = imagen_de(Mode::Rgba, 40, 24, 3);
        let mut banco = Banco::vacio(
            contexto(1.0),
            1.0,
            PANTALLA,
            Canvas::with_tile_side(16),
            imagen.clone(),
        );
        if propia {
            banco.canvas.set_owned_image(imagen.clone());
        } else {
            banco.canvas.set_image(&imagen);
        }
        assert_eq!(
            banco.canvas.texture_id(),
            None,
            "propia {propia}: aún sin textura"
        );
        assert_eq!(banco.canvas.zoom(), None, "propia {propia}: aún sin vista");
        let primero = banco.reposo();
        let ids = banco.ids();
        assert_eq!(ids.len(), 6, "propia {propia}");
        assert_eq!(banco.canvas.texture_id(), Some(ids[0]));
        for (i, deltas) in primero.subidas.por_baldosa(&ids).iter().enumerate() {
            assert_eq!(deltas.len(), 1, "propia {propia}: baldosa {i}");
            assert_eq!(deltas[0].pos, None);
            assert!(deltas[0].nearest);
        }
        assert_eq!(
            primero.info.zoom,
            px_ui::view::View::fit(banco.viewport(), Size::new(40, 24)).zoom()
        );
        let fallos = banco.discrepancias(&esperado(&imagen), &[true; 6], None);
        assert!(fallos.is_empty(), "propia {propia}: {fallos:?}");
        // Otra imagen: la vista vuelve al ajuste y el gesto se descarta.
        banco.al_nivel(N_X5);
        let otra = imagen_de(Mode::Gray, 20, 12, 3);
        if propia {
            banco.canvas.set_owned_image(otra.clone());
        } else {
            banco.canvas.set_image(&otra);
        }
        banco.doc = Document::new(otra.clone());
        let tras = banco.reposo();
        assert_eq!(
            tras.info.zoom,
            px_ui::view::View::fit(banco.viewport(), Size::new(20, 12)).zoom(),
            "propia {propia}"
        );
        assert_eq!(banco.baldosas(), 2, "20×12 con baldosas de 16");
    }
}

#[test]
fn fila_b5_set_image_con_lado_16385_da_el_error_y_no_pinta() {
    let grande = Image::new(16385, 1, Mode::Gray).expect("imagen");
    let mut banco = Banco::vacio(contexto(1.0), 1.0, PANTALLA, Canvas::new(), grande.clone());
    banco.canvas.set_image(&grande);
    let salida = banco.reposo();
    assert_eq!(
        salida.info.error,
        Some(CanvasError::TooLarge {
            width: 16385,
            height: 1,
            max: 16384
        })
    );
    assert_eq!(salida.info.image_rect, None);
    assert_eq!(banco.canvas.texture_id(), None);
}

#[test]
fn fila_b5_sync_with_devuelve_out_of_memory_si_el_asignador_falla() {
    let mut banco = Banco::simple(16, imagen_de(Mode::Rgba, 40, 24, 3));
    let mut resultado = None;
    banco.corre(vec![], |ctx, canvas, doc, _| {
        canvas.invalidate_all();
        let imagen = doc.image(doc.main_image()).expect("imagen");
        resultado = Some(canvas.sync_with(ctx, imagen, &mut Falla::tras(0)));
    });
    assert_eq!(resultado, Some(Err(CanvasError::OutOfMemory)));
}

// ---------------------------------------------------------------------------------------------
// B6 · Oráculo de la textura compuesta
// ---------------------------------------------------------------------------------------------

/// La operación `etiqueta` sobre el documento y su fotograma: las visibles son el oráculo y las no
/// visibles siguen con su contenido de antes; el documento no lo toca el lienzo.
fn paso_b6(banco: &mut Banco, etiqueta: &str, operacion: impl FnOnce(&mut Banco) -> Salida) {
    let antes = banco.contenidos();
    let salida = operacion(banco);
    assert_eq!(salida.sync, Ok(()), "{etiqueta}");
    let visibles = banco.visibles();
    let fallos = banco.discrepancias(&esperado(banco.imagen()), &visibles, Some(&antes));
    assert!(fallos.is_empty(), "{etiqueta}: {fallos:?}");
    // Ni el estado ni la imagen del documento cambian por pintar, y en reposo no pasa nada.
    let (estado, imagen) = (banco.doc.state_id(), banco.imagen().clone());
    let contenidos = banco.contenidos();
    for _ in 0..2 {
        let reposo = banco.reposo();
        assert_eq!(reposo.subidas.total(&banco.ids()), 0, "{etiqueta}: reposo");
    }
    assert_eq!(banco.doc.state_id(), estado, "{etiqueta}: state_id");
    assert_eq!(banco.imagen(), &imagen, "{etiqueta}: documento");
    assert_eq!(banco.contenidos(), contenidos, "{etiqueta}: reposo");
}

fn otro_modo(modo: Mode) -> Mode {
    match modo {
        Mode::Rgba => Mode::GrayAlpha,
        _ => Mode::Rgba,
    }
}

/// Todas las operaciones de la fila B6 sobre un banco, en el modo de su imagen.
fn operaciones_b6(banco: &mut Banco, etiqueta: &str) {
    let modo = banco.imagen().mode();
    let (w, h) = (banco.imagen().width(), banco.imagen().height());
    let id = banco.doc.main_image();
    let entradas = banco.imagen().palette().map_or(1, |p| p.len());
    let pixel = |k: u32| pixel_de(modo, k, entradas);

    // Línea.
    let linea = line_command(
        &banco.doc,
        id,
        (2, 3),
        (w as i32 - 3, h as i32 - 4),
        pixel(5),
    )
    .expect("línea")
    .expect("cae dentro");
    paso_b6(banco, &format!("{etiqueta} · línea"), |b| b.aplica(linea));

    // Trazo: píxeles sueltos en las esquinas y en el centro.
    let trazo = Command::SetPixels {
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
    paso_b6(banco, &format!("{etiqueta} · trazo"), |b| b.aplica(trazo));

    // Relleno.
    if let Ok(Some(relleno)) = fill_command(&banco.doc, id, 1, 1, pixel(23)) {
        paso_b6(banco, &format!("{etiqueta} · relleno"), |b| {
            b.aplica(relleno)
        });
    }

    // Borrador.
    if let Some(borrar) = borrado(banco.imagen()) {
        let borrador = Command::SetPixels {
            image: id,
            pixels: (0..w)
                .map(|x| PixelEdit {
                    x,
                    y: h / 2,
                    pixel: borrar,
                })
                .collect(),
        };
        paso_b6(banco, &format!("{etiqueta} · borrador"), |b| {
            b.aplica(borrador)
        });
    }

    // Deshacer dos veces y rehacer una.
    paso_b6(banco, &format!("{etiqueta} · undo"), |b| b.deshace());
    paso_b6(banco, &format!("{etiqueta} · undo"), |b| b.deshace());
    paso_b6(banco, &format!("{etiqueta} · redo"), |b| b.rehace());

    // Paleta (solo indexado): color, añadir y mover (no cambian lo que se ve), quitar.
    if modo == Mode::Indexed {
        let color = Command::SetPaletteColor {
            image: id,
            index: 2,
            color: Rgba::opaque(9, 99, 199),
        };
        paso_b6(banco, &format!("{etiqueta} · color"), |b| b.aplica(color));
        let mover_entrada = Command::MovePaletteEntry {
            image: id,
            from: 1,
            to: 4,
        };
        paso_b6(banco, &format!("{etiqueta} · mover"), |b| {
            b.aplica(mover_entrada)
        });
        let anadir = Command::AddPaletteEntry {
            image: id,
            color: Rgba::opaque(1, 2, 3),
        };
        paso_b6(banco, &format!("{etiqueta} · añadir"), |b| {
            b.aplica(anadir)
        });
        let quitar = Command::RemovePaletteEntry {
            image: id,
            index: 3,
            into: 2,
        };
        paso_b6(banco, &format!("{etiqueta} · quitar"), |b| {
            b.aplica(quitar)
        });
    }

    // Conversión a otro modo y su deshacer.
    let convertir = Command::ConvertMode {
        image: id,
        target: otro_modo(modo),
        allowed: Allowed {
            luma: true,
            drop_alpha: Some([255, 255, 255]),
        },
    };
    paso_b6(banco, &format!("{etiqueta} · conversión"), |b| {
        b.aplica(convertir)
    });
    paso_b6(
        banco,
        &format!("{etiqueta} · undo de la conversión"),
        |b| b.deshace(),
    );
}

/// Un banco de B6 en un modo, un tamaño y un nivel de zoom con la vista desplazada.
fn escenario_b6(modo: Mode, (w, h): (u32, u32), nivel: usize, arrastre: Option<(i64, i64)>) {
    let etiqueta = format!("{modo:?} {w}×{h} nivel {nivel} arrastre {arrastre:?}");
    let mut banco = Banco::simple(128, imagen_de(modo, w, h, 7));
    banco.al_nivel(nivel);
    if let Some((dx, dy)) = arrastre {
        banco.arrastra(dx, dy);
    }
    operaciones_b6(&mut banco, &etiqueta);
    // Al hacerse visibles las que esperaban, quedan idénticas al oráculo.
    banco.comprueba_todas(&etiqueta);
}

#[test]
fn fila_b6_una_baldosa_en_los_cuatro_modos_a_cuatro_zooms() {
    for modo in MODOS {
        for (nivel, arrastre) in [
            (N_1_64, None),
            (N_1_4, Some((-20, 10))),
            (N_X1, Some((-30, -20))),
            (N_X5, Some((15, -25))),
        ] {
            escenario_b6(modo, (100, 60), nivel, arrastre);
        }
    }
}

#[test]
fn fila_b6_tres_por_dos_baldosas_en_los_cuatro_modos_a_cuatro_zooms() {
    for modo in MODOS {
        for (nivel, arrastre) in [
            (N_1_64, None),
            (N_1_4, Some((-20, 10))),
            (N_X1, Some((-90, -50))),
            (N_X5, Some((100, 0))),
        ] {
            escenario_b6(modo, (300, 140), nivel, arrastre);
        }
    }
}

#[test]
fn fila_b6_las_no_visibles_conservan_su_contenido_y_al_hacerse_visibles_quedan_idénticas() {
    // A ×5 solo se ve una baldosa de 300×200: el trazo en las demás no cambia sus texturas.
    let mut banco = Banco::simple(128, imagen_de(Mode::Rgba, 300, 200, 7));
    banco.al_nivel(N_X5);
    let visibles = banco.visibles();
    assert_eq!(visibles.iter().filter(|&&v| v).count(), 1, "{visibles:?}");
    let id = banco.doc.main_image();
    let antes = banco.contenidos();
    let trazo = Command::SetPixels {
        image: id,
        pixels: (0..300)
            .step_by(7)
            .map(|x| PixelEdit {
                x,
                y: 10 + x % 150,
                pixel: Pixel::Rgba(ROJO),
            })
            .collect(),
    };
    banco.aplica(trazo);
    let tras = banco.contenidos();
    for (i, (a, t)) in antes.iter().zip(&tras).enumerate() {
        if !visibles[i] {
            assert_eq!(a, t, "baldosa {i}: no se ve y no debe cambiar");
        }
    }
    // Un desplazamiento que muestra otra baldosa: se construye con lo que tiene el documento.
    for dx in [-120, 240, -40, 0] {
        banco.arrastra(dx, 0);
        banco.comprueba_visibles(&format!("tras desplazar {dx}"));
    }
    banco.comprueba_todas("al final");
}

// ---------------------------------------------------------------------------------------------
// B7 · Qué se sube
// ---------------------------------------------------------------------------------------------

/// Comprueba, baldosa a baldosa, lo que llegó a la GPU: las visibles reciben las series de los
/// bloques sucios de `sucio` (o un delta completo si lo están todos) y las demás, nada.
fn comprueba_subidas(banco: &Banco, salida: &Salida, sucio: &Modelo, etiqueta: &str) {
    let ids = banco.ids();
    let visibles = banco.visibles();
    for (i, id) in ids.iter().enumerate() {
        let mut real = salida.subidas.de_textura(*id);
        real.sort();
        let esperado = if visibles[i] {
            deltas_esperados(sucio, banco.rect_de(i))
        } else {
            Vec::new()
        };
        assert_eq!(
            real, esperado,
            "{etiqueta}: baldosa {i} (visible: {})",
            visibles[i]
        );
        // Ningún delta fuera de su textura.
        let r = banco.rect_de(i);
        for d in &real {
            let [px, py] = d.pos.unwrap_or([0, 0]);
            assert!(
                px + d.tamano[0] <= r.width as usize && py + d.tamano[1] <= r.height as usize,
                "{etiqueta}: delta {d:?} fuera de la textura de la baldosa {i}"
            );
        }
    }
}

fn pixeles_de(comando: &Command) -> Vec<(u32, u32)> {
    match comando {
        Command::SetPixels { pixels, .. } => pixels.iter().map(|p| (p.x, p.y)).collect(),
        otro => panic!("solo SetPixels: {otro:?}"),
    }
}

fn trazo(id: px_core::ImageId, puntos: &[(u32, u32)]) -> Command {
    Command::SetPixels {
        image: id,
        pixels: puntos
            .iter()
            .map(|&(x, y)| PixelEdit {
                x,
                y,
                pixel: Pixel::Rgba(ROJO),
            })
            .collect(),
    }
}

#[test]
fn fila_b7_un_trazo_en_una_baldosa_visible_sube_solo_sus_bloques_y_solo_en_esa_baldosa() {
    let mut banco = banco_300x200();
    let id = banco.doc.main_image();
    // Un bloque en la baldosa 4 (columna 1, fila 1): el bloque (2, 2) → serie (128,128,64,64).
    let comando = trazo(id, &[(130, 130), (131, 131)]);
    let sucio = modelo_de_pixeles(&pixeles_de(&comando), 300, 200);
    let salida = banco.aplica(comando);
    comprueba_subidas(&banco, &salida, &sucio, "un bloque en la baldosa 4");
    let ids = banco.ids();
    assert_eq!(
        salida.subidas.de_textura(ids[4]),
        vec![Delta {
            pos: Some([0, 0]),
            tamano: [64, 64],
            nearest: true
        }]
    );
    assert_eq!(salida.subidas.total(&ids), 1);
    assert_eq!(banco.canvas.pending_blocks(), 0);
    banco.comprueba_visibles("tras el trazo");

    // Un trazo con bloques contiguos en una fila (se fusionan) y otro en la fila siguiente.
    let comando = trazo(id, &[(60, 10), (70, 10), (10, 100)]);
    let sucio = modelo_de_pixeles(&pixeles_de(&comando), 300, 200);
    let salida = banco.aplica(comando);
    comprueba_subidas(&banco, &salida, &sucio, "series de la baldosa 0");
    assert_eq!(
        {
            let mut v = salida.subidas.de_textura(ids[0]);
            v.sort();
            v
        },
        vec![
            Delta {
                pos: Some([0, 0]),
                tamano: [128, 64],
                nearest: true
            },
            Delta {
                pos: Some([0, 64]),
                tamano: [64, 64],
                nearest: true
            }
        ]
    );
    banco.comprueba_visibles("tras el segundo trazo");
}

#[test]
fn fila_b7_un_trazo_que_cruza_el_borde_de_dos_baldosas_sube_un_bloque_en_cada_una() {
    let mut banco = banco_300x200();
    let id = banco.doc.main_image();
    let comando = trazo(id, &[(126, 10), (127, 10), (128, 10), (129, 10)]);
    let sucio = modelo_de_pixeles(&pixeles_de(&comando), 300, 200);
    let salida = banco.aplica(comando);
    comprueba_subidas(&banco, &salida, &sucio, "cruza de la baldosa 0 a la 1");
    let ids = banco.ids();
    assert_eq!(salida.subidas.total(&ids), 2);
    banco.comprueba_visibles("tras cruzar");
}

#[test]
fn fila_b7_si_todos_los_bloques_de_la_baldosa_estan_sucios_sube_un_delta_completo() {
    let mut banco = banco_300x200();
    let id = banco.doc.main_image();
    // La baldosa 0 (128×128) tiene 2×2 bloques: tres sucios dan series; cuatro, un delta completo.
    let tres = trazo(id, &[(1, 1), (70, 1), (1, 70)]);
    let sucio = modelo_de_pixeles(&pixeles_de(&tres), 300, 200);
    let salida = banco.aplica(tres);
    comprueba_subidas(&banco, &salida, &sucio, "tres de cuatro bloques");
    let ids = banco.ids();
    assert_eq!(salida.subidas.de_textura(ids[0]).len(), 2, "dos series");
    assert!(
        salida
            .subidas
            .de_textura(ids[0])
            .iter()
            .all(|d| d.pos.is_some())
    );
    let cuatro = trazo(id, &[(2, 2), (71, 2), (2, 71), (71, 71)]);
    let sucio = modelo_de_pixeles(&pixeles_de(&cuatro), 300, 200);
    let salida = banco.aplica(cuatro);
    comprueba_subidas(&banco, &salida, &sucio, "los cuatro bloques");
    assert_eq!(
        salida.subidas.de_textura(ids[0]),
        vec![Delta {
            pos: None,
            tamano: [128, 128],
            nearest: true
        }]
    );
    banco.comprueba_visibles("tras los cuatro bloques");
}

#[test]
fn fila_b7_un_trazo_en_una_baldosa_fuera_del_lienzo_no_sube_nada_y_conserva_sus_bloques() {
    let mut banco = banco_300x200();
    let id = banco.doc.main_image();
    let ids = banco.ids();
    let antes = banco.contenidos();
    // La baldosa 2 (x de 256 a 300, y de 0 a 128) no se ve: el bloque (4, 0) queda pendiente.
    let comando = trazo(id, &[(270, 10), (271, 10)]);
    let sucio = modelo_de_pixeles(&pixeles_de(&comando), 300, 200);
    let salida = banco.aplica(comando);
    comprueba_subidas(&banco, &salida, &sucio, "baldosa no visible");
    assert_eq!(salida.subidas.total(&ids), 0, "no sube nada");
    assert_eq!(banco.canvas.pending_blocks(), 1, "conserva el bloque");
    assert_eq!(banco.contenidos(), antes, "la textura sigue como estaba");
    // Sigue pendiente aunque pasen fotogramas.
    for _ in 0..3 {
        let reposo = banco.reposo();
        assert_eq!(reposo.subidas.total(&ids), 0);
        assert_eq!(banco.canvas.pending_blocks(), 1);
    }
    // Al verse (un desplazamiento), sube su bloque, recortado a la baldosa de 44 de ancho.
    let mut suma = Subidas::default();
    for s in banco.arrastra(-120, 0) {
        suma.suma(s.subidas);
    }
    assert_eq!(
        suma.de_textura(ids[2]),
        vec![Delta {
            pos: Some([0, 0]),
            tamano: [44, 64],
            nearest: true
        }]
    );
    assert_eq!(
        suma.total(&[ids[0], ids[1], ids[3], ids[4], ids[5]]),
        0,
        "solo esa baldosa"
    );
    assert_eq!(banco.canvas.pending_blocks(), 0);
    banco.comprueba_visibles("tras verse");
}

#[test]
fn fila_b7_en_reposo_no_se_sube_nada() {
    let mut banco = banco_300x200();
    let ids = banco.ids();
    for n in 0..4 {
        let salida = banco.reposo();
        assert_eq!(salida.subidas.total(&ids), 0, "fotograma {n}");
    }
    // Ni moviendo el puntero sobre el lienzo ni con el ratón quieto.
    for pos in [pos2(10.0, 10.0), pos2(80.0, 60.0), pos2(150.0, 110.0)] {
        let salida = banco.corre(vec![mover(pos)], |_, _, _, _| {});
        assert_eq!(salida.subidas.total(&ids), 0, "puntero en {pos:?}");
    }
    // Ni una invalidación vacía.
    let salida = banco.corre(vec![], |_, canvas, doc, _| {
        let t = tamano_de(doc);
        canvas.invalidate(&BlockSet::new(t.width, t.height));
    });
    assert_eq!(salida.subidas.total(&ids), 0, "invalidación vacía");
    assert_eq!(banco.canvas.pending_blocks(), 0);
}

#[test]
fn fila_b7_los_comandos_de_paleta_que_no_cambian_la_imagen_no_suben_nada() {
    let mut banco = Banco::simple(128, imagen_de(Mode::Indexed, 300, 200, 7));
    banco.pide(ViewOp::Actual);
    let id = banco.doc.main_image();
    let ids = banco.ids();
    for (etiqueta, comando) in [
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
        let salida = banco.aplica(comando);
        assert_eq!(salida.subidas.total(&ids), 0, "{etiqueta}");
        assert_eq!(banco.canvas.pending_blocks(), 0, "{etiqueta}");
        banco.comprueba_visibles(etiqueta);
    }
}

#[test]
fn fila_b7_una_operacion_de_imagen_entera_con_todo_visible_sube_cada_baldosa_una_vez() {
    let mut banco = banco_300x200();
    banco.pide(ViewOp::Fit);
    banco.asegura_visibles(&[true; 6], "Ajustar");
    let ids = banco.ids();
    let lleno = modelo_lleno(300, 200);
    // `invalidate_all`: un delta completo por baldosa.
    let salida = banco.corre(vec![], |_, canvas, _, _| canvas.invalidate_all());
    comprueba_subidas(&banco, &salida, &lleno, "invalidate_all");
    for (i, id) in ids.iter().enumerate() {
        let r = banco.rect_de(i);
        assert_eq!(
            salida.subidas.de_textura(*id),
            vec![Delta {
                pos: None,
                tamano: [r.width as usize, r.height as usize],
                nearest: true
            }],
            "baldosa {i}"
        );
    }
    // Una conversión de modo del documento: igual.
    let id = banco.doc.main_image();
    let salida = banco.aplica(Command::ConvertMode {
        image: id,
        target: Mode::GrayAlpha,
        allowed: Allowed {
            luma: true,
            drop_alpha: None,
        },
    });
    comprueba_subidas(&banco, &salida, &lleno, "conversión");
    assert_eq!(salida.subidas.total(&ids), 6, "una por baldosa");
    banco.comprueba_visibles("tras convertir");
    // Y un cambio de color de paleta de una imagen indexada.
    let mut indexado = Banco::simple(128, imagen_de(Mode::Indexed, 300, 200, 7));
    indexado.pide(ViewOp::Fit);
    let id = indexado.doc.main_image();
    let salida = indexado.aplica(Command::SetPaletteColor {
        image: id,
        index: 2,
        color: Rgba::opaque(5, 6, 7),
    });
    assert_eq!(salida.subidas.total(&indexado.ids()), 6);
    indexado.comprueba_visibles("tras cambiar el color");
}

#[test]
fn fila_b7_una_operacion_de_imagen_entera_con_parte_fuera_del_lienzo_deja_pendientes_los_de_fuera()
{
    let mut banco = banco_300x200();
    let ids = banco.ids();
    let lleno = modelo_lleno(300, 200);
    let salida = banco.corre(vec![], |_, canvas, _, _| canvas.invalidate_all());
    comprueba_subidas(&banco, &salida, &lleno, "invalidate_all con parte fuera");
    assert_eq!(
        salida.subidas.total(&ids),
        4,
        "las cuatro visibles, una vez cada una"
    );
    // Las baldosas 2 y 5 (x ≥ 256) tienen 2 bloques cada una.
    assert_eq!(banco.canvas.pending_blocks(), 4);
    banco.pide(ViewOp::Fit);
    assert_eq!(banco.canvas.pending_blocks(), 0);
    banco.comprueba_visibles("tras ajustar");
}

// ---------------------------------------------------------------------------------------------
// B8 · Superposición como textura a las cinco escalas
// ---------------------------------------------------------------------------------------------

/// Pantalla de B8 (a ppp 3 son 288×216 píxeles físicos).
const PANTALLA_B8: Vec2 = vec2(96.0, 72.0);
const TOL: f32 = 0.02;
const DESPLAZO_X: f64 = 0.0013;
const DESPLAZO_Y: f64 = 0.0007;

const FONDOS_B8: [(CanvasBackground, bool); 5] = [
    (CanvasBackground::Checker, false),
    (CanvasBackground::Black, false),
    (CanvasBackground::Gray, false),
    (CanvasBackground::White, false),
    (CanvasBackground::Checker, true),
];

/// Lo que se ve: color premultiplicado, como egui.
#[derive(Clone, Copy, Debug)]
struct Visto {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl Visto {
    const VACIO: Visto = Visto {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    fn de_color(c: Color32) -> Visto {
        Visto {
            r: f32::from(c.r()) / 255.0,
            g: f32::from(c.g()) / 255.0,
            b: f32::from(c.b()) / 255.0,
            a: f32::from(c.a()) / 255.0,
        }
    }

    /// `src` por encima de `self` (alfa premultiplicado).
    fn debajo_de(self, src: Visto) -> Visto {
        let k = 1.0 - src.a;
        Visto {
            r: src.r + self.r * k,
            g: src.g + self.g * k,
            b: src.b + self.b * k,
            a: src.a + self.a * k,
        }
    }

    fn por(self, o: Visto) -> Visto {
        Visto {
            r: self.r * o.r,
            g: self.g * o.g,
            b: self.b * o.b,
            a: self.a * o.a,
        }
    }

    fn cerca(&self, o: Visto, tolerancia: f32) -> bool {
        (self.r - o.r).abs() <= tolerancia
            && (self.g - o.g).abs() <= tolerancia
            && (self.b - o.b).abs() <= tolerancia
            && (self.a - o.a).abs() <= tolerancia
    }
}

struct Buffer {
    ancho: usize,
    alto: usize,
    pixeles: Vec<Visto>,
}

impl Buffer {
    fn en(&self, x: i64, y: i64) -> Visto {
        self.pixeles[y as usize * self.ancho + x as usize]
    }
}

fn baricentricas(p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<[f64; 3]> {
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
fn texel_del_damero(textura: &Textura, u: f64, v: f64) -> Color32 {
    let tx = ((u * 2.0).floor() as i64).rem_euclid(2) as usize;
    let ty = ((v * 2.0).floor() as i64).rem_euclid(2) as usize;
    textura.texels[ty * textura.tamano[0] + tx]
}

/// Pinta las primitivas, en orden, en un búfer de `ancho × alto` píxeles físicos, muestreando cada
/// píxel en su centro (algo desplazado). Las mallas de las baldosas se muestrean con
/// `Texturas::muestrea` en el uv interpolado (`NEAREST`, recortado al borde); la del damero, con
/// envoltura; los rellenos y las mallas de color (textura por defecto) usan el color de sus vértices.
fn rasteriza(
    primitivas: &[ClippedPrimitive],
    ppp: f32,
    ancho: usize,
    alto: usize,
    texturas: &Texturas,
    damero: Option<TextureId>,
) -> Buffer {
    let ppp = f64::from(ppp);
    let mut pixeles = vec![Visto::VACIO; ancho * alto];
    for primitiva in primitivas {
        let Primitive::Mesh(malla) = &primitiva.primitive else {
            continue;
        };
        let clip = primitiva.clip_rect;
        let (cx0, cy0) = (f64::from(clip.min.x) * ppp, f64::from(clip.min.y) * ppp);
        let (cx1, cy1) = (f64::from(clip.max.x) * ppp, f64::from(clip.max.y) * ppp);
        let id = malla.texture_id;
        let plana = id == TextureId::default();
        let es_damero = damero == Some(id);
        assert!(
            plana || es_damero || texturas.get(id).is_some(),
            "una malla con una textura desconocida: {id:?}"
        );
        for triangulo in malla.indices.as_chunks::<3>().0 {
            let v: [&Vertex; 3] = [
                &malla.vertices[triangulo[0] as usize],
                &malla.vertices[triangulo[1] as usize],
                &malla.vertices[triangulo[2] as usize],
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
                .min(ancho as f64)
                .max(0.0) as usize;
            let y1 = (max_y.ceil() + 1.0)
                .min(cy1.ceil())
                .min(alto as f64)
                .max(0.0) as usize;
            let tintes = v.map(|v| Visto::de_color(v.color));
            for py in y0..y1 {
                for px in x0..x1 {
                    let s = (px as f64 + 0.5 + DESPLAZO_X, py as f64 + 0.5 + DESPLAZO_Y);
                    if s.0 < cx0 || s.0 > cx1 || s.1 < cy0 || s.1 > cy1 {
                        continue;
                    }
                    let Some(w) = baricentricas(s, p[0], p[1], p[2]) else {
                        continue;
                    };
                    let wf = w.map(|w| w as f32);
                    let tinte = Visto {
                        r: wf[0] * tintes[0].r + wf[1] * tintes[1].r + wf[2] * tintes[2].r,
                        g: wf[0] * tintes[0].g + wf[1] * tintes[1].g + wf[2] * tintes[2].g,
                        b: wf[0] * tintes[0].b + wf[1] * tintes[1].b + wf[2] * tintes[2].b,
                        a: wf[0] * tintes[0].a + wf[1] * tintes[1].a + wf[2] * tintes[2].a,
                    };
                    let color = if plana {
                        tinte
                    } else {
                        let u = w[0] * f64::from(v[0].uv.x)
                            + w[1] * f64::from(v[1].uv.x)
                            + w[2] * f64::from(v[2].uv.x);
                        let t = w[0] * f64::from(v[0].uv.y)
                            + w[1] * f64::from(v[1].uv.y)
                            + w[2] * f64::from(v[2].uv.y);
                        let texel = if es_damero {
                            texel_del_damero(texturas.get(id).expect("damero"), u, t)
                        } else {
                            texturas.muestrea(id, u, t).expect("una baldosa")
                        };
                        Visto::de_color(texel).por(tinte)
                    };
                    let destino = &mut pixeles[py * ancho + px];
                    *destino = destino.debajo_de(color);
                }
            }
        }
    }
    Buffer {
        ancho,
        alto,
        pixeles,
    }
}

/// La firma de lo pintado: recorte, textura, vértices e índices de cada malla.
type Firma = Vec<(Rect, TextureId, Vec<Vertex>, Vec<u32>)>;

fn firma(primitivas: &[ClippedPrimitive]) -> Firma {
    primitivas
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
fn menor_area_fisica(vertices: &[Vertex], indices: &[u32], ppp: f32) -> Option<f64> {
    let k = f64::from(ppp);
    indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| vertices[i as usize].pos);
            let cruz = f64::from(b.x - a.x) * f64::from(c.y - a.y)
                - f64::from(c.x - a.x) * f64::from(b.y - a.y);
            cruz.abs() / 2.0 * k * k
        })
        .reduce(f64::min)
}

/// Ninguna forma de área cero: ni rectángulos vacíos ni triángulos degenerados, antes y después de
/// teselar (la regresión `b702d8e` de zoom < 1).
fn sin_formas_de_area_cero(
    formas: &[ClippedShape],
    primitivas: &[ClippedPrimitive],
    ppp: f32,
    etiqueta: &str,
) {
    fn recorre(forma: &Shape, ppp: f32, etiqueta: &str) {
        match forma {
            Shape::Rect(r) => assert!(
                r.rect.width() > 0.0 && r.rect.height() > 0.0,
                "{etiqueta}: rectángulo de área cero {:?}",
                r.rect
            ),
            Shape::Mesh(m) => {
                if let Some(area) = menor_area_fisica(&m.vertices, &m.indices, ppp) {
                    assert!(
                        area > 1e-6,
                        "{etiqueta}: triángulo degenerado (área {area} px²)"
                    );
                }
            }
            Shape::Vec(v) => v.iter().for_each(|f| recorre(f, ppp, etiqueta)),
            _ => {}
        }
    }
    for forma in formas {
        recorre(&forma.shape, ppp, etiqueta);
    }
    for primitiva in primitivas {
        if let Primitive::Mesh(m) = &primitiva.primitive
            && let Some(area) = menor_area_fisica(&m.vertices, &m.indices, ppp)
        {
            assert!(
                area > 1e-6,
                "{etiqueta}: triángulo degenerado tras teselar (área {area} px²)"
            );
        }
    }
}

fn gris(c: Rgba) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// Color del fondo elegido bajo la transparencia en el píxel físico `(px, py)`: el oráculo de la
/// 17b (`background_fill` y `shade_at`).
fn fondo_en(fondo: CanvasBackground, px: i64, py: i64, origen: (i64, i64), cell: i64) -> Color32 {
    match background_fill(fondo) {
        Fill::Checker => gris(shade_at(px, py, origen, cell).color()),
        Fill::Solid(color) => gris(color),
    }
}

/// Un color con alfa directo por encima de un fondo opaco.
fn compone(fondo: Color32, color: Rgba) -> Visto {
    Visto::de_color(fondo).debajo_de(Visto::de_color(texel(color)))
}

/// Geometría de lo pintado, en píxeles físicos, y el zoom como fracción.
struct Geom {
    origen: (i64, i64),
    viewport: Size,
    cell: i64,
    num: i128,
    den: i128,
}

impl Geom {
    fn de(banco: &Banco, nivel: usize) -> Geom {
        let (num, den) = if nivel < 6 {
            (1, 1i128 << (6 - nivel))
        } else {
            ((nivel - 5) as i128, 1)
        };
        Geom {
            origen: banco.origen(),
            viewport: banco.viewport(),
            cell: 8 * i64::from(text_scale(banco.ppp)),
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
fn visto_esperado(
    g: &Geom,
    imagen: &Image,
    bytes: &[u8],
    pendientes: &HashMap<(u32, u32), Rgba>,
    fondo: CanvasBackground,
    px: i64,
    py: i64,
) -> Visto {
    let (sx, sy) = (px - g.origen.0, py - g.origen.1);
    if sx < 0 || sy < 0 || sx >= g.extent(imagen.width()) || sy >= g.extent(imagen.height()) {
        return Visto::de_color(BACKGROUND);
    }
    let ix = (i128::from(sx) * g.den / g.num) as u32;
    let iy = (i128::from(sy) * g.den / g.num) as u32;
    let color = pendientes.get(&(ix, iy)).copied().unwrap_or_else(|| {
        let i = (iy as usize * imagen.width() as usize + ix as usize) * 4;
        Rgba::new(bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3])
    });
    compone(fondo_en(fondo, px, py, g.origen, g.cell), color)
}

/// Los bloques de la 17b: píxeles rojos opacos (lápiz), transparentes (borrador) y rojos con alfa
/// 128, en rectángulos que cruzan los bordes de las baldosas de 16 de una imagen de 40×24.
fn pendientes_b8() -> Vec<(u32, u32, Rgba)> {
    let mut v = Vec::new();
    let mut rellena = |x0: u32, x1: u32, y0: u32, y1: u32, color: Rgba| {
        for y in y0..y1 {
            for x in x0..x1 {
                v.push((x, y, color));
            }
        }
    };
    rellena(8, 20, 4, 10, ROJO);
    rellena(20, 30, 4, 10, Rgba::TRANSPARENT);
    rellena(8, 30, 14, 20, Rgba::new(255, 0, 0, 128));
    v
}

/// RGBA de 40×24 con un color distinto por posición y alfas 0 (las cuatro primeras columnas), 128
/// (las cuatro siguientes) y 255 (el resto).
fn imagen_b8() -> Image {
    let mut imagen = Image::new(40, 24, Mode::Rgba).expect("imagen");
    for y in 0..24u32 {
        for x in 0..40u32 {
            let alfa = match x {
                0..=3 => 0,
                4..=7 => 128,
                _ => 255,
            };
            let color = Rgba::new((x * 6) as u8, (y * 10) as u8, ((x * y) % 251) as u8, alfa);
            imagen.set_pixel(x, y, Pixel::Rgba(color)).expect("dentro");
        }
    }
    imagen
}

/// Compara todos los píxeles físicos del lienzo con el oráculo y devuelve los fallos.
fn compara_b8(
    buffer: &Buffer,
    g: &Geom,
    imagen: &Image,
    pendientes: &HashMap<(u32, u32), Rgba>,
    fondo: CanvasBackground,
    etiqueta: &str,
) -> Vec<String> {
    let bytes = to_rgba8(imagen);
    let mut fallos = Vec::new();
    let mut total = 0usize;
    for py in 0..buffer.alto as i64 {
        for px in 0..buffer.ancho as i64 {
            let esperado = visto_esperado(g, imagen, &bytes, pendientes, fondo, px, py);
            let visto = buffer.en(px, py);
            if !visto.cerca(esperado, TOL) {
                total += 1;
                if fallos.len() < 4 {
                    fallos.push(format!(
                        "{etiqueta}: el píxel físico ({px}, {py}) se ve {visto:?} y debía verse {esperado:?}"
                    ));
                }
            }
        }
    }
    if total > 0 {
        fallos.push(format!(
            "{etiqueta}: {total} píxeles físicos distintos del oráculo"
        ));
    }
    fallos
}

fn iguales(a: &Buffer, b: &Buffer, etiqueta: &str) {
    assert_eq!((a.ancho, a.alto), (b.ancho, b.alto));
    let distintos = a
        .pixeles
        .iter()
        .zip(&b.pixeles)
        .filter(|(p, q)| !p.cerca(**q, 0.01))
        .count();
    assert_eq!(
        distintos, 0,
        "{etiqueta}: antes y después de soltar difieren en {distintos} píxeles"
    );
}

fn raster(banco: &Banco, salida: &Salida) -> Buffer {
    let vp = banco.viewport();
    rasteriza(
        &banco.primitivas(salida),
        banco.ppp,
        vp.width as usize,
        vp.height as usize,
        &banco.texturas,
        banco.canvas.checker_texture_id(),
    )
}

/// Un caso de B8: escala, nivel de zoom, fondo y camino del damero.
fn comprueba_b8(ctx: &Context, ppp: f32, nivel: usize, fondo: CanvasBackground, malla: bool) {
    let etiqueta = format!(
        "ppp {ppp} nivel {nivel} fondo {fondo:?}{}",
        if malla { " (malla)" } else { "" }
    );
    let imagen = imagen_b8();
    let mut canvas = Canvas::with_tile_side(16);
    canvas.set_background(fondo);
    canvas.set_checker_mesh(malla);
    let mut banco = Banco::con(ctx.clone(), ppp, PANTALLA_B8, canvas, imagen);
    banco.al_nivel(nivel);
    let pendientes = pendientes_b8();
    let mapa: HashMap<(u32, u32), Rgba> = pendientes.iter().map(|&(x, y, c)| ((x, y), c)).collect();

    // Con la superposición: el trazo entero cada fotograma, como en la app.
    let mut salida = banco.reposo();
    for _ in 0..2 {
        let pend = pendientes.clone();
        salida = banco.corre(vec![], move |ctx, canvas, doc, _| {
            canvas.set_overlay(ctx, doc.image(doc.main_image()).expect("imagen"), &pend);
        });
    }
    assert_eq!(salida.sync, Ok(()), "{etiqueta}");
    let g = Geom::de(&banco, nivel);
    let primitivas = banco.primitivas(&salida);
    sin_formas_de_area_cero(
        &salida.formas,
        &primitivas,
        ppp,
        &format!("{etiqueta} (con superposición)"),
    );
    let antes = raster(&banco, &salida);
    let fallos = compara_b8(
        &antes,
        &g,
        banco.imagen(),
        &mapa,
        fondo,
        &format!("{etiqueta} durante el gesto"),
    );
    assert!(fallos.is_empty(), "{fallos:#?}");
    let forma_con = firma(&primitivas);

    // Soltar: el trazo se aplica al documento y la superposición se descarta.
    let aplicar = Command::SetPixels {
        image: banco.doc.main_image(),
        pixels: pendientes
            .iter()
            .map(|&(x, y, c)| PixelEdit {
                x,
                y,
                pixel: Pixel::Rgba(c),
            })
            .collect(),
    };
    banco.corre(vec![], move |_, canvas, doc, log| {
        let _ = aplica_y_registra(doc, log, canvas, aplicar);
        canvas.clear_overlay();
    });
    let salida = banco.reposo();
    let g = Geom::de(&banco, nivel);
    let primitivas = banco.primitivas(&salida);
    sin_formas_de_area_cero(
        &salida.formas,
        &primitivas,
        ppp,
        &format!("{etiqueta} (tras soltar)"),
    );
    let despues = raster(&banco, &salida);
    let sin_pendientes = HashMap::new();
    let fallos = compara_b8(
        &despues,
        &g,
        banco.imagen(),
        &sin_pendientes,
        fondo,
        &format!("{etiqueta} tras soltar"),
    );
    assert!(fallos.is_empty(), "{fallos:#?}");
    iguales(&antes, &despues, &etiqueta);
    // Ninguna forma ni malla de superposición: lo pintado con y sin ella es lo mismo.
    assert!(
        forma_con == firma(&primitivas),
        "{etiqueta}: el trazo cambia las formas pintadas (¿hay formas de superposición?)"
    );
}

fn b8_a(ppp: f32) {
    let ctx = contexto(ppp);
    for nivel in [N_1_2, N_1_4, N_X1, N_X2, N_X3] {
        for (fondo, malla) in FONDOS_B8 {
            comprueba_b8(&ctx, ppp, nivel, fondo, malla);
        }
    }
}

#[test]
fn fila_b8_la_superposicion_como_textura_a_ppp_1_0() {
    b8_a(PPPS[0]);
}

#[test]
fn fila_b8_la_superposicion_como_textura_a_ppp_1_25() {
    b8_a(PPPS[1]);
}

#[test]
fn fila_b8_la_superposicion_como_textura_a_ppp_1_5() {
    b8_a(PPPS[2]);
}

#[test]
fn fila_b8_la_superposicion_como_textura_a_ppp_2_0() {
    b8_a(PPPS[3]);
}

#[test]
fn fila_b8_la_superposicion_como_textura_a_ppp_3_0() {
    b8_a(PPPS[4]);
}

#[test]
fn fila_b8_el_borrador_deja_ver_el_fondo_elegido_y_el_color_borrado_es_el_mismo_antes_y_despues() {
    // Solo el borrador (alfa 0) sobre píxeles opacos, ×3 y ppp 1: cada píxel borrado se ve como el
    // fondo (el damero o el color liso), antes de soltar y después.
    let ctx = contexto(1.0);
    for (fondo, malla) in FONDOS_B8 {
        let imagen = imagen_b8();
        let mut canvas = Canvas::with_tile_side(16);
        canvas.set_background(fondo);
        canvas.set_checker_mesh(malla);
        let mut banco = Banco::con(ctx.clone(), 1.0, PANTALLA_B8, canvas, imagen);
        banco.al_nivel(N_X3);
        let borrados: Vec<(u32, u32, Rgba)> = (8..30)
            .flat_map(|x| (4..10).map(move |y| (x, y, Rgba::TRANSPARENT)))
            .collect();
        let mapa: HashMap<(u32, u32), Rgba> =
            borrados.iter().map(|&(x, y, c)| ((x, y), c)).collect();
        let pend = borrados.clone();
        let salida = banco.corre(vec![], move |ctx, canvas, doc, _| {
            canvas.set_overlay(ctx, doc.image(doc.main_image()).expect("imagen"), &pend);
        });
        let g = Geom::de(&banco, N_X3);
        let antes = raster(&banco, &salida);
        // Lo borrado se ve exactamente como el fondo.
        for &(x, y, _) in &borrados {
            let (px, py) = (
                g.origen.0 + i64::from(x) * 3 + 1,
                g.origen.1 + i64::from(y) * 3 + 1,
            );
            if px < 0
                || py < 0
                || px >= i64::from(g.viewport.width)
                || py >= i64::from(g.viewport.height)
            {
                continue;
            }
            let esperado = Visto::de_color(fondo_en(fondo, px, py, g.origen, g.cell));
            assert!(
                antes.en(px, py).cerca(esperado, TOL),
                "{fondo:?} malla {malla}: ({x}, {y}) no deja ver el fondo"
            );
        }
        let fallos = compara_b8(&antes, &g, banco.imagen(), &mapa, fondo, "borrador");
        assert!(fallos.is_empty(), "{fallos:#?}");
        // Soltar con el borrador aplicado.
        let id = banco.doc.main_image();
        let borrar = Command::SetPixels {
            image: id,
            pixels: borrados
                .iter()
                .map(|&(x, y, _)| PixelEdit {
                    x,
                    y,
                    pixel: Pixel::Rgba(Rgba::TRANSPARENT),
                })
                .collect(),
        };
        banco.corre(vec![], move |_, canvas, doc, log| {
            let _ = aplica_y_registra(doc, log, canvas, borrar);
            canvas.clear_overlay();
        });
        let salida = banco.reposo();
        let despues = raster(&banco, &salida);
        iguales(&antes, &despues, &format!("{fondo:?} malla {malla}"));
    }
}

// ---------------------------------------------------------------------------------------------
// B9 · Vida de la superposición
// ---------------------------------------------------------------------------------------------

/// La imagen del documento con los píxeles pendientes escritos, en orden (oráculo con `pixel_rgba`
/// para los de la prueba, que ya llevan su `Rgba`).
fn con_pendientes(imagen: &Image, pendientes: &[(u32, u32, Rgba)]) -> Vec<Color32> {
    let mut texels = esperado(imagen);
    let ancho = imagen.width() as usize;
    for &(x, y, color) in pendientes {
        texels[y as usize * ancho + x as usize] = texel(color);
    }
    texels
}

fn comprueba_con_pendientes(banco: &Banco, pendientes: &[(u32, u32, Rgba)], etiqueta: &str) {
    let fallos = banco.discrepancias(
        &con_pendientes(banco.imagen(), pendientes),
        &banco.visibles(),
        None,
    );
    assert!(fallos.is_empty(), "{etiqueta}: {fallos:?}");
}

fn fotograma_con_overlay(banco: &mut Banco, pendientes: Vec<(u32, u32, Rgba)>) -> Salida {
    banco.corre(vec![], move |ctx, canvas, doc, _| {
        canvas.set_overlay(
            ctx,
            doc.image(doc.main_image()).expect("imagen"),
            &pendientes,
        );
    })
}

/// Los bloques que cubren los deltas de un fotograma, por baldosa, como conjunto de la imagen.
fn bloques_subidos(banco: &Banco, salida: &Salida) -> Modelo {
    let mut m = Modelo::new();
    for (i, id) in banco.ids().iter().enumerate() {
        m.extend(bloques_cubiertos(
            &salida.subidas.de_textura(*id),
            banco.rect_de(i),
        ));
    }
    m
}

fn modelo(bloques: &[(u32, u32)]) -> Modelo {
    bloques.iter().copied().collect()
}

#[test]
fn fila_b9_un_lapiz_de_20_pixeles_en_3_fotogramas_escribe_y_sube_solo_los_nuevos() {
    let mut banco = banco_300x200();
    let estado = banco.doc.state_id();
    let inicial = banco.imagen().clone();
    // Siete píxeles en el bloque (1, 0) de la baldosa 0; siete en el (2, 0) de la 1; seis en el
    // (1, 2) de la 3.
    let f1 = tira(70, 45, 7, ROJO);
    let mut f2 = f1.clone();
    f2.extend(tira(130, 45, 7, VERDE));
    let mut f3 = f2.clone();
    f3.extend(tira(70, 130, 6, AZUL));
    assert_eq!(f3.len(), 20);
    for (n, (pend, bloques, vivos)) in [
        (f1, vec![(1, 0)], 1usize),
        (f2, vec![(2, 0)], 2),
        (f3, vec![(1, 2)], 3),
    ]
    .into_iter()
    .enumerate()
    {
        let salida = fotograma_con_overlay(&mut banco, pend.clone());
        assert_eq!(salida.sync, Ok(()));
        assert_eq!(
            bloques_subidos(&banco, &salida),
            modelo(&bloques),
            "fotograma {}: solo los bloques con píxeles nuevos",
            n + 1
        );
        assert_eq!(banco.canvas.overlay_blocks(), vivos, "fotograma {}", n + 1);
        comprueba_con_pendientes(&banco, &pend, &format!("fotograma {}", n + 1));
        assert_eq!(
            banco.doc.state_id(),
            estado,
            "el documento no cambia hasta soltar"
        );
        assert_eq!(banco.imagen(), &inicial);
    }
    // Repetir el mismo trazo entero no sube nada.
    let salida = fotograma_con_overlay(&mut banco, {
        let mut p = tira(70, 45, 7, ROJO);
        p.extend(tira(130, 45, 7, VERDE));
        p.extend(tira(70, 130, 6, AZUL));
        p
    });
    assert_eq!(salida.subidas.total(&banco.ids()), 0, "ningún píxel nuevo");
}

/// El trazo de tres tiras de `lapiz_de_20_pixeles`, para los finales de B9.
fn pendientes_de_20() -> Vec<(u32, u32, Rgba)> {
    let mut p = tira(70, 45, 7, ROJO);
    p.extend(tira(130, 45, 7, VERDE));
    p.extend(tira(70, 130, 6, AZUL));
    p
}

fn banco_b9_con_el_lapiz_a_medias() -> (Banco, Image, Vec<(u32, u32, Rgba)>) {
    let mut banco = banco_300x200();
    let inicial = banco.imagen().clone();
    let pend = pendientes_de_20();
    fotograma_con_overlay(&mut banco, pend.clone());
    assert_eq!(banco.canvas.overlay_blocks(), 3);
    (banco, inicial, pend)
}

fn tras_el_gesto(banco: &mut Banco, esperado_doc: &Image, etiqueta: &str) {
    assert_eq!(
        banco.canvas.overlay_blocks(),
        0,
        "{etiqueta}: la copia de trabajo se libera"
    );
    assert_eq!(banco.imagen(), esperado_doc, "{etiqueta}: el documento");
    // La textura vuelve a ser el documento (con el trazo si se aplicó, sin él si no).
    let fallos = banco.discrepancias(&esperado(esperado_doc), &banco.visibles(), None);
    assert!(fallos.is_empty(), "{etiqueta}: {fallos:?}");
    banco.reposo();
    banco.comprueba_todas(etiqueta);
}

#[test]
fn fila_b9_soltar_con_ok_true_deja_la_textura_con_el_trazo_y_libera_la_copia() {
    let (mut banco, _, pend) = banco_b9_con_el_lapiz_a_medias();
    let id = banco.doc.main_image();
    let comando = Command::SetPixels {
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
    let salida = banco.corre(vec![], move |_, canvas, doc, log| {
        assert_eq!(aplica_y_registra(doc, log, canvas, comando), Ok(true));
        canvas.clear_overlay();
    });
    assert_eq!(salida.sync, Ok(()));
    let esperada = banco.imagen().clone();
    for &(x, y, c) in &pend {
        assert_eq!(
            esperada.get_pixel(x, y),
            Ok(Pixel::Rgba(c)),
            "el trazo está en el documento"
        );
    }
    tras_el_gesto(&mut banco, &esperada, "soltar con Ok(true)");
}

#[test]
fn fila_b9_soltar_con_ok_false_devuelve_la_textura_al_documento_sin_el_trazo() {
    let (mut banco, inicial, _) = banco_b9_con_el_lapiz_a_medias();
    // `Ok(false)`: el documento no cambió; el lienzo solo descarta la superposición.
    let estado = banco.doc.state_id();
    banco.corre(vec![], |_, canvas, _, _| canvas.clear_overlay());
    assert_eq!(banco.doc.state_id(), estado);
    tras_el_gesto(&mut banco, &inicial, "soltar con Ok(false)");
}

#[test]
fn fila_b9_escape_y_set_overlay_vacio_devuelven_la_textura_al_documento() {
    let (mut banco, inicial, _) = banco_b9_con_el_lapiz_a_medias();
    // Esc: la tecla llega al lienzo (que la emite en orden) y la app cancela el gesto; la
    // superposición vacía equivale a `clear_overlay`.
    let salida = banco.corre(
        vec![tecla(Key::Escape, Modifiers::NONE)],
        |ctx, canvas, doc, _| {
            canvas.set_overlay(ctx, doc.image(doc.main_image()).expect("imagen"), &[]);
        },
    );
    assert!(salida.entradas.contains(&CanvasInput::Escape));
    tras_el_gesto(&mut banco, &inicial, "Esc");
}

#[test]
fn fila_b9_un_error_de_apply_deja_el_documento_y_la_textura_como_antes_del_gesto() {
    let (mut banco, inicial, pend) = banco_b9_con_el_lapiz_a_medias();
    let id = banco.doc.main_image();
    // Un píxel de otro modo: `apply` da error y el documento queda intacto.
    let mala = Command::SetPixels {
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
    banco.corre(vec![], move |_, canvas, doc, log| {
        assert!(aplica_y_registra(doc, log, canvas, mala).is_err());
        canvas.clear_overlay();
    });
    tras_el_gesto(&mut banco, &inicial, "error de apply");
}

#[test]
fn fila_b9_la_linea_que_cambia_de_extremo_sube_los_bloques_de_la_anterior_y_la_nueva() {
    let mut banco = banco_300x200();
    let inicial = banco.imagen().clone();
    let estado = banco.doc.state_id();
    let linea = |a: (i32, i32), b: (i32, i32)| -> Vec<(u32, u32, Rgba)> {
        Line::new(a, b)
            .clip_to(300, 200)
            .map(|(x, y)| (x as u32, y as u32, ROJO))
            .collect()
    };
    // Primera goma: (50,45)→(100,45), bloques (0,0) y (1,0) de la baldosa 0.
    let l1 = linea((50, 45), (100, 45));
    let salida = fotograma_con_overlay(&mut banco, l1.clone());
    assert_eq!(bloques_subidos(&banco, &salida), modelo(&[(0, 0), (1, 0)]));
    assert_eq!(banco.canvas.overlay_blocks(), 2);
    comprueba_con_pendientes(&banco, &l1, "primera línea");
    // Cambia de extremo: (50,45)→(50,100), bloques (0,0) y (0,1). No es un prefijo: se sube lo de
    // la anterior (que vuelve al documento) y lo nuevo.
    let l2 = linea((50, 45), (50, 100));
    let salida = fotograma_con_overlay(&mut banco, l2.clone());
    assert_eq!(
        bloques_subidos(&banco, &salida),
        modelo(&[(0, 0), (1, 0), (0, 1)]),
        "los bloques de la anterior y los de la nueva"
    );
    assert_eq!(
        banco.canvas.overlay_blocks(),
        2,
        "solo los de la línea actual"
    );
    comprueba_con_pendientes(
        &banco,
        &l2,
        "segunda línea (el bloque (1,0) volvió al documento)",
    );
    // Sin cambios: nada.
    let salida = fotograma_con_overlay(&mut banco, l2.clone());
    assert_eq!(salida.subidas.total(&banco.ids()), 0);
    // De vuelta a la primera: otra vez la unión.
    let salida = fotograma_con_overlay(&mut banco, l1.clone());
    assert_eq!(
        bloques_subidos(&banco, &salida),
        modelo(&[(0, 0), (1, 0), (0, 1)])
    );
    comprueba_con_pendientes(&banco, &l1, "de vuelta a la primera");
    assert_eq!(banco.doc.state_id(), estado);
    assert_eq!(banco.imagen(), &inicial);
    // Soltar con la línea real del documento.
    let id = banco.doc.main_image();
    let comando = line_command(&banco.doc, id, (50, 45), (100, 45), Pixel::Rgba(ROJO))
        .expect("línea")
        .expect("cae dentro");
    banco.corre(vec![], move |_, canvas, doc, log| {
        assert_eq!(aplica_y_registra(doc, log, canvas, comando), Ok(true));
        canvas.clear_overlay();
    });
    let aplicada = banco.imagen().clone();
    tras_el_gesto(&mut banco, &aplicada, "soltar la línea");
}

#[test]
fn fila_b9_una_linea_diagonal_de_8192_px_en_8192_toca_como_mucho_255_bloques() {
    // 8192×8192 en grises (64 MiB) con las baldosas por defecto de 2048. No se guardan las
    // texturas compuestas (serían otros 256 MiB): solo se cuentan los bloques de trabajo.
    let imagen = Image::new(8192, 8192, Mode::Gray).expect("imagen de 64 MiB");
    let mut banco = Banco::vacio(contexto(1.0), 1.0, PANTALLA, Canvas::new(), imagen);
    banco.guarda = false;
    let cargada = banco.doc.image(banco.doc.main_image()).expect("imagen");
    banco.canvas.load(&banco.ctx, cargada).expect("load");
    let _ = banco.reposo();
    assert_eq!(banco.baldosas(), 16, "8192² en baldosas de 2048");
    let linea: Vec<(u32, u32, Rgba)> = Line::new((0, 0), (8191, 8190))
        .clip_to(8192, 8192)
        .map(|(x, y)| (x as u32, y as u32, Rgba::opaque(250, 250, 250)))
        .collect();
    assert_eq!(linea.len(), 8192);
    let salida = fotograma_con_overlay(&mut banco, linea);
    assert_eq!(salida.sync, Ok(()));
    let bloques = banco.canvas.overlay_blocks();
    assert!(
        (128..=255).contains(&bloques),
        "la diagonal toca entre 128 y 255 bloques de 64×64 y toca {bloques}"
    );
    banco.corre(vec![], |_, canvas, _, _| canvas.clear_overlay());
    assert_eq!(banco.canvas.overlay_blocks(), 0);
}

// ---------------------------------------------------------------------------------------------
// B10 · Vista previa del editor
// ---------------------------------------------------------------------------------------------

const AMARILLO: Rgba = Rgba::opaque(255, 255, 0);
const VERDE_AMARILLENTO: Rgba = Rgba::opaque(128, 255, 0);

/// 300×100 indexada en 3 baldosas de 128; a ×1 se ven las dos primeras.
fn banco_b10() -> Banco {
    let mut banco = Banco::simple(128, imagen_de(Mode::Indexed, 300, 100, 5));
    banco.pide(ViewOp::Actual);
    banco.asegura_visibles(&[true, true, false], "300×100 a ×1");
    banco
}

fn esperado_con_entrada(imagen: &Image, index: u8, color: Rgba) -> Vec<Color32> {
    to_rgba8_with_entry(imagen, index, color)
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]))
        .collect()
}

fn previsualiza(banco: &mut Banco, index: u8, color: Rgba) -> Salida {
    banco.corre(vec![], move |ctx, canvas, doc, _| {
        canvas.preview_entry(
            ctx,
            doc.image(doc.main_image()).expect("imagen"),
            index,
            color,
        );
    })
}

fn termina_la_vista_previa(banco: &mut Banco) -> Salida {
    banco.corre(vec![], |ctx, canvas, doc, _| {
        canvas.end_preview(ctx, doc.image(doc.main_image()).expect("imagen"));
    })
}

/// Solo las visibles reciben deltas.
fn comprueba_solo_visibles(banco: &Banco, salida: &Salida, etiqueta: &str) {
    let ids = banco.ids();
    for (i, visible) in banco.visibles().iter().enumerate() {
        let n = salida.subidas.de_textura(ids[i]).len();
        assert_eq!(
            n > 0,
            *visible,
            "{etiqueta}: baldosa {i} (visible {visible}) recibió {n} deltas"
        );
    }
}

#[test]
fn fila_b10_la_vista_previa_solo_reconstruye_y_sube_las_baldosas_visibles_y_cuenta_como_hoy() {
    let mut banco = banco_b10();
    let ids = banco.ids();
    let antes = banco.contenidos();
    let doc = banco.imagen().clone();
    assert_eq!(banco.canvas.preview_uploads(), 0);

    // Tres valores iguales: una sola subida.
    let s1 = previsualiza(&mut banco, 2, AMARILLO);
    assert_eq!(banco.canvas.preview_uploads(), 1);
    comprueba_solo_visibles(&banco, &s1, "primer valor");
    let fallos = banco.discrepancias(
        &esperado_con_entrada(&doc, 2, AMARILLO),
        &banco.visibles(),
        Some(&antes),
    );
    assert!(fallos.is_empty(), "{fallos:?}");
    for _ in 0..2 {
        let s = previsualiza(&mut banco, 2, AMARILLO);
        assert_eq!(banco.canvas.preview_uploads(), 1, "el valor se repite");
        assert_eq!(s.subidas.total(&ids), 0, "y no llega nada a la GPU");
    }
    // Un fotograma sin llamar a `preview_entry`: nada.
    assert_eq!(banco.reposo().subidas.total(&ids), 0);
    assert_eq!(banco.canvas.preview_uploads(), 1);

    // Un valor nuevo: +1.
    let s2 = previsualiza(&mut banco, 2, VERDE_AMARILLENTO);
    assert_eq!(banco.canvas.preview_uploads(), 2);
    comprueba_solo_visibles(&banco, &s2, "valor nuevo");
    let fallos = banco.discrepancias(
        &esperado_con_entrada(&doc, 2, VERDE_AMARILLENTO),
        &banco.visibles(),
        Some(&antes),
    );
    assert!(fallos.is_empty(), "{fallos:?}");

    // De vuelta al primero: +1.
    let s3 = previsualiza(&mut banco, 2, AMARILLO);
    assert_eq!(banco.canvas.preview_uploads(), 3);
    comprueba_solo_visibles(&banco, &s3, "de vuelta");
    let fallos = banco.discrepancias(
        &esperado_con_entrada(&doc, 2, AMARILLO),
        &banco.visibles(),
        Some(&antes),
    );
    assert!(fallos.is_empty(), "{fallos:?}");
    // El documento no cambia.
    assert_eq!(banco.imagen(), &doc);
    assert_eq!(banco.doc.state_id(), 0);
}

#[test]
fn fila_b10_un_pan_a_mitad_construye_la_baldosa_que_aparece_ya_con_la_vista_previa() {
    let mut banco = banco_b10();
    let ids = banco.ids();
    let doc = banco.imagen().clone();
    previsualiza(&mut banco, 2, AMARILLO);
    assert_eq!(banco.canvas.preview_uploads(), 1);
    // La baldosa 2 sigue como estaba.
    let doc_tile2 = recorte(&esperado(&doc), 300, banco.rect_de(2));
    assert_eq!(banco.contenido(2).texels, doc_tile2);
    // Se desplaza con la vista previa activa (la app repite `preview_entry` en cada fotograma).
    let mut suma = Subidas::default();
    for s in banco.arrastra_con(-120, 0, |ctx, canvas, doc, _| {
        canvas.preview_entry(
            ctx,
            doc.image(doc.main_image()).expect("imagen"),
            2,
            AMARILLO,
        );
    }) {
        suma.suma(s.subidas);
    }
    assert_eq!(
        banco.canvas.preview_uploads(),
        1,
        "es el mismo valor: no cuenta"
    );
    assert!(banco.visibles()[2], "la baldosa 2 ya se ve");
    assert!(
        !suma.de_textura(ids[2]).is_empty(),
        "se construyó y se subió"
    );
    // Todas las visibles llevan la vista previa, también la que apareció.
    let fallos = banco.discrepancias(
        &esperado_con_entrada(&doc, 2, AMARILLO),
        &banco.visibles(),
        None,
    );
    assert!(fallos.is_empty(), "{fallos:?}");
}

#[test]
fn fila_b10_la_entrada_transparente_conserva_el_alfa_0_en_la_vista_previa() {
    let mut banco = banco_b10();
    let doc = banco.imagen().clone();
    let transparente = doc
        .palette()
        .and_then(|p| p.transparent_index())
        .expect("índice transparente");
    // Un rojo opaco para la entrada transparente: sus píxeles siguen invisibles.
    previsualiza(&mut banco, transparente, ROJO);
    let oraculo = esperado_con_entrada(&doc, transparente, ROJO);
    let fallos = banco.discrepancias(&oraculo, &banco.visibles(), None);
    assert!(fallos.is_empty(), "{fallos:?}");
    let mut vistos = 0;
    for i in [0usize, 1] {
        let r = banco.rect_de(i);
        let texels = banco.contenido(i).texels;
        for y in 0..r.height {
            for x in 0..r.width {
                if doc.get_pixel(r.x + x, r.y + y) == Ok(Pixel::Indexed(transparente)) {
                    assert_eq!(
                        texels[(y * r.width + x) as usize].a(),
                        0,
                        "({}, {})",
                        r.x + x,
                        r.y + y
                    );
                    vistos += 1;
                }
            }
        }
    }
    assert!(vistos > 0, "la imagen usa el índice transparente");
}

#[test]
fn fila_b10_cancelar_devuelve_las_visibles_al_documento_y_las_no_visibles_al_verse() {
    let mut banco = banco_b10();
    let doc = banco.imagen().clone();
    previsualiza(&mut banco, 2, AMARILLO);
    // Desplazar para que la baldosa 0 salga del lienzo mientras se previsualiza.
    banco.arrastra_con(-200, 0, |ctx, canvas, doc, _| {
        canvas.preview_entry(
            ctx,
            doc.image(doc.main_image()).expect("imagen"),
            2,
            AMARILLO,
        );
    });
    let visibles = banco.visibles();
    assert!(!visibles[0] && visibles[2], "preparación: {visibles:?}");
    // Cancelar: `end_preview`. Las visibles vuelven al documento ya.
    let salida = termina_la_vista_previa(&mut banco);
    assert_eq!(salida.sync, Ok(()));
    let fallos = banco.discrepancias(&esperado(&doc), &banco.visibles(), None);
    assert!(fallos.is_empty(), "{fallos:?}");
    assert_eq!(banco.imagen(), &doc, "cancelar no cambia el documento");
    // Las que se tocaron y no se veían vuelven al documento al verse.
    banco.comprueba_todas("tras cancelar y ajustar");
    // Sin vista previa activa, `end_preview` no sube nada.
    let ids = banco.ids();
    assert_eq!(termina_la_vista_previa(&mut banco).subidas.total(&ids), 0);
}

#[test]
fn fila_b10_soltar_con_paso_deja_las_texturas_del_documento_nuevo() {
    let mut banco = banco_b10();
    let id = banco.doc.main_image();
    previsualiza(&mut banco, 2, AMARILLO);
    previsualiza(&mut banco, 2, VERDE_AMARILLENTO);
    // Soltar: la edición se aplica (`SetPaletteColor`), la app invalida y termina la vista previa.
    let comando = Command::SetPaletteColor {
        image: id,
        index: 2,
        color: VERDE_AMARILLENTO,
    };
    let salida = banco.corre(vec![], move |ctx, canvas, doc, log| {
        assert_eq!(aplica_y_registra(doc, log, canvas, comando), Ok(true));
        canvas.end_preview(ctx, doc.image(doc.main_image()).expect("imagen"));
    });
    assert_eq!(salida.sync, Ok(()));
    let nuevo = banco.imagen().clone();
    let fallos = banco.discrepancias(&esperado(&nuevo), &banco.visibles(), None);
    assert!(fallos.is_empty(), "{fallos:?}");
    banco.comprueba_todas("tras soltar con paso");
    // Y `undo` devuelve el color de antes en todas.
    banco.deshace();
    banco.comprueba_todas("tras deshacer");
}

// ---------------------------------------------------------------------------------------------
// Q2 · Oráculo con secuencias aleatorias
// ---------------------------------------------------------------------------------------------

/// Un paso de Q2: la semilla de la operación, el nivel de zoom y el desplazamiento.
type PasoQ2 = (Semilla, u8, i8, i8);

fn paso_q2() -> impl Strategy<Value = PasoQ2> {
    (semilla(), 0u8..70, -60i8..=60, -60i8..=60)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Q2: secuencias aleatorias (0 a 12) de trazo, línea, relleno, borrador, `undo`, `redo`,
    /// conversión, edición de color, mover y quitar entrada, con zoom y vista aleatorios (también
    /// menores que 1) sobre imágenes de 1×1 a 40×40 con baldosas de 16: tras cada paso y un
    /// fotograma, la textura compuesta de todas las baldosas visibles es el oráculo; y tras llevar
    /// todas a la vista, **todas**.
    #[test]
    fn propiedad_q2_la_textura_compuesta_es_el_oraculo_en_las_visibles_y_en_todas_al_final(
        modo in 0u8..4,
        ancho in 1u32..=40,
        alto in 1u32..=40,
        celda in 1u32..=6,
        pasos in proptest::collection::vec(paso_q2(), 0..=12),
    ) {
        let imagen = imagen_de(MODOS[usize::from(modo)], ancho, alto, celda);
        let mut banco = Banco::con(
            contexto(1.0),
            1.0,
            vec2(96.0, 72.0),
            Canvas::with_tile_side(16),
            imagen,
        );
        for (n, (semilla, nivel, dx, dy)) in pasos.into_iter().enumerate() {
            banco.al_nivel(usize::from(nivel));
            banco.arrastra(i64::from(dx), i64::from(dy));
            let antes = banco.contenidos();
            let salida = match paso_de(&banco.doc, semilla) {
                Paso::Aplica(comando) => banco.aplica(comando),
                Paso::Deshacer => banco.deshace(),
                Paso::Rehacer => banco.rehace(),
                Paso::Nada => banco.reposo(),
            };
            prop_assert_eq!(salida.sync, Ok(()), "paso {}", n);
            let visibles = banco.visibles();
            let fallos = banco.discrepancias(&esperado(banco.imagen()), &visibles, Some(&antes));
            prop_assert!(fallos.is_empty(), "paso {} {:?}: {:?}", n, semilla, fallos);
        }
        banco.pide(ViewOp::Fit);
        banco.reposo();
        let visibles = banco.visibles();
        prop_assert!(visibles.iter().all(|&v| v), "Ajustar deja todas a la vista: {:?}", visibles);
        let fallos = banco.discrepancias(&esperado(banco.imagen()), &visibles, None);
        prop_assert!(fallos.is_empty(), "al final, todas: {:?}", fallos);
    }
}

// ---------------------------------------------------------------------------------------------
// Q4 · Superposición con gestos aleatorios
// ---------------------------------------------------------------------------------------------

/// Los pendientes de un gesto de lápiz tal como los guarda `Stroke`: sin coordenadas repetidas, en
/// el orden de la primera visita y con el último valor.
fn sin_repetidos(puntos: &[(u32, u32, Pixel)]) -> Vec<(u32, u32, Pixel)> {
    let mut posicion: HashMap<(u32, u32), usize> = HashMap::new();
    let mut v: Vec<(u32, u32, Pixel)> = Vec::new();
    for &(x, y, pixel) in puntos {
        match posicion.get(&(x, y)) {
            Some(&i) => v[i].2 = pixel,
            None => {
                posicion.insert((x, y), v.len());
                v.push((x, y, pixel));
            }
        }
    }
    v
}

fn en_rgba(imagen: &Image, pendientes: &[(u32, u32, Pixel)]) -> Vec<(u32, u32, Rgba)> {
    pendientes
        .iter()
        .map(|&(x, y, p)| (x, y, pixel_rgba(imagen, p).expect("píxel del modo")))
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
    fn propiedad_q4_durante_el_gesto_la_textura_es_el_documento_con_los_pendientes_y_despues_el_documento(
        modo in 0u8..4,
        ancho in 1u32..=200,
        alto in 1u32..=150,
        celda in 1u32..=9,
        es_linea in any::<bool>(),
        borrador in any::<bool>(),
        crudos in proptest::collection::vec((any::<u32>(), any::<u32>(), any::<u32>()), 2..=40),
        fotogramas in 1usize..=4,
        fin in 0u8..4,
        nivel in 0u8..70,
    ) {
        let imagen = imagen_de(MODOS[usize::from(modo)], ancho, alto, celda);
        let entradas = imagen.palette().map_or(1, |p| p.len());
        let mut banco = Banco::con(
            contexto(1.0),
            1.0,
            PANTALLA,
            Canvas::with_tile_side(128),
            imagen.clone(),
        );
        banco.al_nivel(usize::from(nivel));
        let (estado, inicial) = (banco.doc.state_id(), banco.imagen().clone());

        // Los puntos y su valor: el del borrador si lo hay y se pide, y si no uno por punto.
        let valor_del_gesto = borrado(&imagen).filter(|_| borrador);
        let puntos: Vec<(u32, u32, Pixel)> = crudos
            .iter()
            .map(|&(a, b, k)| {
                (
                    a % ancho,
                    b % alto,
                    valor_del_gesto.unwrap_or_else(|| pixel_de(imagen.mode(), k, entradas)),
                )
            })
            .collect();
        let pixel_linea =
            valor_del_gesto.unwrap_or_else(|| pixel_de(imagen.mode(), crudos[0].2 % 97, entradas));
        let n = puntos.len();
        // Fotograma `f` (de 1 a `fotogramas`): el lápiz tiene el prefijo de `hasta(f)` puntos y la
        // línea va del primero al punto `extremo(f)`.
        let hasta = |f: usize| (f * n).div_ceil(fotogramas).max(1);
        let extremo = |f: usize| (f * (n - 1)).div_ceil(fotogramas).clamp(1, n - 1);
        let linea_hasta = |f: usize| -> Vec<(u32, u32, Pixel)> {
            let (a, b) = (puntos[0], puntos[extremo(f)]);
            Line::new((a.0 as i32, a.1 as i32), (b.0 as i32, b.1 as i32))
                .clip_to(ancho, alto)
                .map(|(x, y)| (x as u32, y as u32, pixel_linea))
                .collect()
        };

        let mut ultimo: Vec<(u32, u32, Pixel)> = Vec::new();
        for f in 1..=fotogramas {
            let pendientes = if es_linea {
                linea_hasta(f)
            } else {
                sin_repetidos(&puntos[..hasta(f)])
            };
            let rgba = en_rgba(&imagen, &pendientes);
            let salida = fotograma_con_overlay(&mut banco, rgba.clone());
            prop_assert_eq!(salida.sync, Ok(()));
            let visibles = banco.visibles();
            let fallos = banco.discrepancias(&con_pendientes(&imagen, &rgba), &visibles, None);
            prop_assert!(fallos.is_empty(), "fotograma {}: {:?}", f, fallos);
            prop_assert_eq!(banco.doc.state_id(), estado);
            prop_assert_eq!(banco.imagen(), &inicial);
            ultimo = pendientes;
        }

        // El final del gesto: aplicado, cancelado, con error o con Esc.
        let id = banco.doc.main_image();
        match fin {
            0 => {
                let comando = if es_linea {
                    let (a, b) = (puntos[0], puntos[extremo(fotogramas)]);
                    line_command(&banco.doc, id, (a.0 as i32, a.1 as i32), (b.0 as i32, b.1 as i32), pixel_linea)
                        .expect("línea")
                } else {
                    Some(Command::SetPixels {
                        image: id,
                        pixels: ultimo.iter().map(|&(x, y, pixel)| PixelEdit { x, y, pixel }).collect(),
                    })
                };
                banco.corre(vec![], move |_, canvas, doc, log| {
                    if let Some(comando) = comando {
                        let _ = aplica_y_registra(doc, log, canvas, comando);
                    }
                    canvas.clear_overlay();
                });
            }
            1 => {
                banco.corre(vec![], |_, canvas, _, _| canvas.clear_overlay());
            }
            2 => {
                // Un píxel fuera de la imagen: `apply` da error y el documento queda intacto.
                let mala = Command::SetPixels {
                    image: id,
                    pixels: vec![PixelEdit {
                        x: ancho,
                        y: 0,
                        pixel: pixel_de(imagen.mode(), 1, entradas),
                    }],
                };
                banco.corre(vec![], move |_, canvas, doc, log| {
                    assert!(aplica_y_registra(doc, log, canvas, mala).is_err());
                    canvas.clear_overlay();
                });
            }
            _ => {
                banco.corre(vec![tecla(Key::Escape, Modifiers::NONE)], |ctx, canvas, doc, _| {
                    canvas.set_overlay(ctx, doc.image(doc.main_image()).expect("imagen"), &[]);
                });
            }
        }
        prop_assert_eq!(banco.canvas.overlay_blocks(), 0, "la copia de trabajo se libera");
        if fin != 0 {
            prop_assert_eq!(banco.doc.state_id(), estado);
            prop_assert_eq!(banco.imagen(), &inicial);
        }
        let visibles = banco.visibles();
        let fallos = banco.discrepancias(&esperado(banco.imagen()), &visibles, None);
        prop_assert!(fallos.is_empty(), "tras el gesto: {:?}", fallos);
        banco.pide(ViewOp::Fit);
        banco.reposo();
        let visibles = banco.visibles();
        let fallos = banco.discrepancias(&esperado(banco.imagen()), &visibles, None);
        prop_assert!(fallos.is_empty(), "tras el gesto, todas: {:?}", fallos);
    }
}
