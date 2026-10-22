//! Tests de la superposición del borrador en el lienzo de egui sin ventana (specs/hito-2-
//! cuentagotas-borrador.md, decisión 3 y fila 10 de «Casos límite»): el mismo banco que
//! `tests/pencil.rs` (un `egui::Context` con `RawInput` sintético), copiado y no modificado, pero
//! con una `ToolState` con el borrador, como hace la app: `Canvas::interact_ordered`, los eventos
//! a la herramienta, `Canvas::set_overlay` con la superposición de `ToolState::preview`
//! convertida con `pixel_rgba` (alfa 0 en el borrador), `refresh` si el documento cambió (o `sync`
//! si no) y `Canvas::paint`.
//!
//! Qué se ve se lee de la **textura compuesta** (unidad 14a-2, specs/hito-2-zoom-baldosas.md,
//! decisión 7): el banco aplica en orden los `textures_delta` de cada fotograma con
//! `common::textures::Textures` y guarda en cada `TestFrame` la textura de la baldosa 0 tal como
//! quedó en ese fotograma. Un píxel borrado vale `Color32::TRANSPARENT` (alfa 0) en esa textura y
//! la GPU lo compone sobre lo que hay debajo (el damero de transparencia o el fondo elegido, que
//! se dibuja **bajo** las baldosas); por eso no hay ninguna forma de superposición: el fotograma
//! con trazo debe dar exactamente las mismas formas que el de reposo (specs/hito-2-damero-
//! lienzo.md, decisión 8 y fila 7, afirmaban antes un cuadrilátero del damero por píxel
//! pendiente; la unidad 14a-2 lo sustituye por la textura).
//!
//! Cada cambio de estado del botón va en su propio fotograma (egui necesita uno por pulsación o
//! soltado). 800×600 con 64×48 da zoom ×12 y las posiciones son enteras.

use std::collections::BTreeSet;

use egui::{
    Color32, Context, Event, FullOutput, ImageData, Modifiers, PointerButton, Pos2, RawInput, Rect,
    Shape, TextureId, pos2, vec2,
};
mod common;
use common::textures::{Texture, Textures};
use px_core::{Document, Image, ImageId, Mode, Pixel, PixelEdit, Rgba};
use px_ui::canvas::{BACKGROUND, Canvas, CanvasInfo};
use px_ui::texture::{pixel_rgba, to_rgba8};
use px_ui::tool::{Tool, ToolAction, ToolState};

const SCREEN: egui::Vec2 = vec2(800.0, 600.0);
const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;
/// Píxeles del trazo: la fila 5, de `x = 10` a `x = 29`.
const FIRST: i32 = 10;
const ULTIMO: i32 = 29;
const ROW: i32 = 5;

fn blue() -> Pixel {
    Pixel::Rgba(Rgba::opaque(0, 0, 255))
}

/// Imagen RGBA de 64×48 azul opaca.
fn image_blue() -> Image {
    let mut image = Image::new(WIDTH, HEIGHT, Mode::Rgba).expect("imagen válida");
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            image.set_pixel(x, y, blue()).expect("dentro");
        }
    }
    image
}

// Entradas sintéticas.

fn mover(pos: Pos2) -> Event {
    Event::PointerMoved(pos)
}

fn left_2(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// Centro del píxel `(x, y)` en una vista con el rectángulo y el zoom dados (puntos de egui).
fn center(rect: Rect, zoom: i64, x: i32, y: i32) -> Pos2 {
    let z = zoom as f32;
    pos2(
        rect.min.x + (x as f32 + 0.5) * z,
        rect.min.y + (y as f32 + 0.5) * z,
    )
}

/// Rectángulos rellenos de una forma, con su color de relleno (aplana `Shape::Vec`).
fn collects_rectangles(shape: &Shape, output_2: &mut Vec<(Rect, Color32)>) {
    match shape {
        Shape::Rect(rect) => output_2.push((rect.rect, rect.fill)),
        Shape::Vec(shapes_2) => {
            for shape_2 in shapes_2 {
                collects_rectangles(shape_2, output_2);
            }
        }
        _ => {}
    }
}

fn rectangles(output: &FullOutput) -> Vec<(Rect, Color32)> {
    let mut output_2 = Vec::new();
    for clipped in &output.shapes {
        collects_rectangles(&clipped.shape, &mut output_2);
    }
    output_2
}

/// Mallas con la textura `texture_2` (la del damero), con su caja y su tinte (el color de sus
/// vértices, que debe ser el mismo en todos). Aplana `Shape::Vec`.
fn collects_meshes(shape: &Shape, texture_2: TextureId, output_2: &mut Vec<(Rect, Color32)>) {
    match shape {
        Shape::Mesh(mesh) if mesh.texture_id == texture_2 => {
            let mut toolbox_2 = Rect::NOTHING;
            for v in &mesh.vertices {
                toolbox_2.extend_with(v.pos);
            }
            let tint = mesh.vertices.first().expect("malla con vértices").color;
            assert!(
                mesh.vertices.iter().all(|v| v.color == tint),
                "todos los vértices de una malla del damero llevan el mismo tinte"
            );
            output_2.push((toolbox_2, tint));
        }
        Shape::Vec(shapes_2) => {
            for shape_2 in shapes_2 {
                collects_meshes(shape_2, texture_2, output_2);
            }
        }
        _ => {}
    }
}

fn meshes_of_the_checker(output: &FullOutput, texture_2: TextureId) -> Vec<(Rect, Color32)> {
    let mut output_2 = Vec::new();
    for clipped in &output.shapes {
        collects_meshes(&clipped.shape, texture_2, &mut output_2);
    }
    output_2
}

/// Lo que el fotograma dejó: formas, cambios de textura y lo que hizo la herramienta.
struct TestFrame {
    output: FullOutput,
    /// La textura de la baldosa 0 compuesta con los `textures_delta` hasta este fotograma.
    texture_2: Texture,
    info: CanvasInfo,
    /// `Ok(true)` de algún evento del fotograma: el documento cambió.
    change: bool,
    /// La superposición pendiente que se pintó, en orden.
    pending_2: Vec<PixelEdit>,
}

/// La app mínima: lienzo, documento y herramienta, con el borrador elegido.
struct Bench {
    ctx: Context,
    canvas: Canvas,
    doc: Document,
    id: ImageId,
    tool: ToolState,
    /// Contenido de cada textura, compuesto en orden con los `textures_delta` de cada fotograma.
    textures_2: Textures,
}

impl Bench {
    fn new_2() -> Bench {
        let image = image_blue();
        let doc = Document::new(image.clone());
        let id = ImageId::from_raw(0);
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(Tool::Eraser));
        assert_eq!(tool.tool(), Tool::Eraser);
        let mut canvas = Canvas::new();
        canvas.set_image(&image);
        Bench {
            ctx: Context::default(),
            canvas,
            doc,
            id,
            tool,
            textures_2: Textures::new(),
        }
    }

    fn image_2(&self) -> &Image {
        self.doc.image(self.id).expect("hay imagen")
    }

    /// Un fotograma como el de la app (`interact`, eventos y `apply`, `set_overlay`, `refresh` o
    /// `sync`, `paint`). Los cambios de textura se aplican a `textures` y se devuelven también en
    /// `output` (quien llama decide si los mira y los limpia: egui entra en pánico si se sueltan
    /// sin aplicar).
    fn frame(&mut self, events: Vec<Event>) -> TestFrame {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events,
            ..RawInput::default()
        };
        let Bench {
            ctx,
            canvas,
            doc,
            id,
            tool,
            textures_2,
        } = self;
        let mut result = None;
        let output = ctx.run_ui(input, |ui| {
            let mut change = false;
            for input_2 in canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>()
            {
                change |= tool
                    .canvas_input_with(doc, *id, None, input_2)
                    .expect("el borrador no falla");
            }
            let image = doc.image(*id).expect("hay imagen");
            let pending_2: Vec<PixelEdit> = tool.preview().collect();
            let overlay: Vec<(u32, u32, Rgba)> = pending_2
                .iter()
                .filter_map(|e| Some((e.x, e.y, pixel_rgba(image, e.pixel)?)))
                .collect();
            // El trazo pendiente entero, después de `interact` y antes de `paint`; al soltar queda
            // vacío y los bloques que tocó se regeneran desde el documento en `sync` o `refresh`.
            canvas.set_overlay(ui.ctx(), image, &overlay);
            if change {
                canvas.refresh(ui.ctx(), image);
            } else {
                canvas.sync(ui.ctx(), image).expect("sync");
            }
            let info = canvas.paint(ui);
            result = Some((info, change, pending_2));
        });
        textures_2.applies(&output.textures_delta);
        let texture_2 = self
            .canvas
            .texture_id()
            .and_then(|id| self.textures_2.get(id))
            .cloned()
            .unwrap_or_default();
        let (info, change, pending_2) = result.expect("el lienzo se dibuja");
        TestFrame {
            output,
            texture_2,
            info,
            change,
            pending_2,
        }
    }

    /// Un fotograma con los cambios de textura ya limpiados.
    fn step(&mut self, events: Vec<Event>) -> TestFrame {
        let mut f = self.frame(events);
        f.output.textures_delta.clear();
        f
    }
}

/// Un banco con la imagen subida y el primer fotograma ya ejecutado.
struct Session {
    bench: Bench,
    /// Rectángulos del fotograma en reposo: el fondo del lienzo, sin superposición.
    base: Vec<(Rect, Color32)>,
    /// Textura del damero y sus mallas del fotograma en reposo: el cuadrilátero del damero de
    /// fondo de la imagen, sin superposición.
    checker_2: TextureId,
    base_checker: Vec<(Rect, Color32)>,
    /// `to_rgba8` de la imagen antes de empezar.
    bytes: Vec<u8>,
    rect: Rect,
    zoom: i64,
}

impl Session {
    fn new() -> Session {
        let mut bench = Bench::new_2();
        let first = bench.step(vec![]);
        let rect = first.info.image_rect.expect("hay imagen");
        let zoom = first.info.zoom.scale(1);
        assert_eq!(zoom, 12, "800×600 con 64×48 da zoom ×12");
        let rest_2_2 = bench.step(vec![]);
        assert!(rest_2_2.pending_2.is_empty());
        let base = rectangles(&rest_2_2.output);
        let checker_2 = bench
            .canvas
            .checker_texture_id()
            .expect("el damero tiene su textura");
        let base_checker = meshes_of_the_checker(&rest_2_2.output, checker_2);
        assert_eq!(base_checker.len(), 1, "un cuadrilátero de damero de fondo");
        let bytes = to_rgba8(bench.image_2());
        Session {
            bench,
            base,
            checker_2,
            base_checker,
            bytes,
            rect,
            zoom,
        }
    }

    fn pos(&self, x: i32, y: i32) -> Pos2 {
        center(self.rect, self.zoom, x, y)
    }

    /// Pulsa en el primer píxel y arrastra hasta el último, un fotograma por píxel, y deja el
    /// botón abajo. Devuelve los fotogramas, el primero el de la pulsación.
    fn drags_without_release(&mut self) -> Vec<TestFrame> {
        let start_2 = self.pos(FIRST, ROW);
        let mut frames = vec![self.bench.frame(vec![mover(start_2)])];
        frames.push(self.bench.frame(vec![left_2(start_2, true)]));
        for x in FIRST + 1..=ULTIMO {
            let p = self.pos(x, ROW);
            frames.push(self.bench.frame(vec![mover(p)]));
        }
        frames
    }
}

/// Posiciones `(x, y)` de los píxeles de la fila `ROW` de `FIRST` a `ULTIMO`.
fn the_twenty() -> Vec<(u32, u32)> {
    (FIRST..=ULTIMO)
        .map(|x| (u32::try_from(x).unwrap(), u32::try_from(ROW).unwrap()))
        .collect()
}

/// Lo que debe haber en la textura de la baldosa: `to_rgba8` de la imagen original (el oráculo de
/// la unidad 14a, premultiplicado) con los `pixels_2` borrados (alfa 0: `Color32::TRANSPARENT`).
fn texture_expected(bytes: &[u8], pixels_2: impl IntoIterator<Item = (u32, u32)>) -> Vec<Color32> {
    let mut texels: Vec<Color32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    for (x, y) in pixels_2 {
        texels[(y * WIDTH + x) as usize] = Color32::TRANSPARENT;
    }
    texels
}

/// Lo que se subió en el fotograma: solo la textura del lienzo (la baldosa), con deltas parciales
/// (bloques) que caben en ella, y sin liberar ninguna textura. Con `there_is_change` falso, nada.
fn checks_uploads(output: &FullOutput, texture_2: TextureId, there_is_change: bool, label_2: &str) {
    assert!(
        output.textures_delta.free.is_empty(),
        "{label_2}: sin textures_delta.free"
    );
    if !there_is_change {
        assert!(
            output.textures_delta.set.is_empty(),
            "{label_2}: sin cambios no se sube nada"
        );
        return;
    }
    for (id, deltas) in &output.textures_delta.set {
        assert_eq!(
            *id, texture_2,
            "{label_2}: solo se sube la textura del lienzo"
        );
        for delta in deltas {
            let ImageData::Color(color) = &delta.image;
            let [x0, y0] = delta.pos.unwrap_or_else(|| {
                panic!("{label_2}: la superposición sube bloques, no la entera")
            });
            assert!(
                x0 + color.size[0] <= WIDTH as usize && y0 + color.size[1] <= HEIGHT as usize,
                "{label_2}: el delta {:?} en {:?} cabe en la baldosa",
                color.size,
                (x0, y0)
            );
        }
    }
}

/// Lo que debe cumplir un fotograma con `n` píxeles pendientes y el botón aún abajo. `new_2` es
/// que este fotograma añadió píxeles (y por tanto sube bloques).
fn checks_frame_with_stroke(s: &Session, f: &TestFrame, n: usize, new_2: bool, label_2: &str) {
    let rects = rectangles(&f.output);
    let meshes = meshes_of_the_checker(&f.output, s.checker_2);
    assert_eq!(f.pending_2.len(), n, "{label_2}: píxeles pendientes");
    let expected_2_2 = &the_twenty()[..n];
    let pending_2: Vec<(u32, u32)> = f.pending_2.iter().map(|e| (e.x, e.y)).collect();
    assert_eq!(
        pending_2, expected_2_2,
        "{label_2}: el trazo entero, en el orden del lápiz"
    );
    // Antes: un cuadrilátero del damero por píxel pendiente (y antes un rectángulo de `BACKGROUND`)
    // sobre la base. Ahora la superposición no es una forma: el fotograma con trazo da las mismas
    // formas que el de reposo, y el borrado se ve en la textura.
    assert_eq!(
        rects, s.base,
        "{label_2}: la superposición no añade rectángulos (la base no cambia)"
    );
    assert_eq!(
        meshes, s.base_checker,
        "{label_2}: ni cuadriláteros del damero por píxel pendiente: solo el damero de fondo"
    );
    // La vista previa del borrador muestra los píxeles borrados: la textura compuesta lleva cada
    // píxel pendiente transparente (alfa 0) y el resto de la imagen como estaba.
    assert_eq!(
        f.texture_2.size_2,
        [WIDTH as usize, HEIGHT as usize],
        "{label_2}: tamaño de la textura"
    );
    assert_eq!(
        f.texture_2.texels,
        texture_expected(&s.bytes, expected_2_2.iter().copied()),
        "{label_2}: la textura es la imagen con los píxeles pendientes transparentes"
    );
    for &(x, y) in expected_2_2 {
        let texel = f.texture_2.texel(x as usize, y as usize).expect("dentro");
        assert_eq!(texel, Color32::TRANSPARENT, "{label_2}: ({x}, {y}) borrado");
        assert_eq!(texel.a(), 0, "{label_2}: ({x}, {y}) con alfa 0");
    }
    // Solo se suben bloques de la textura, y solo en el fotograma en que cambia.
    let texture_2 = s.bench.canvas.texture_id().expect("la textura está subida");
    checks_uploads(&f.output, texture_2, new_2, label_2);
}

// Fila 10.

#[test]
fn the_canvas_background_is_gray_40() {
    assert_eq!(BACKGROUND, Color32::from_gray(40));
}

#[test]
fn row_10_each_frame_shows_in_the_texture_the_pending_erased_pixels_without_extra_shapes() {
    let mut s = Session::new();
    let texture = s.bench.canvas.texture_id().expect("la textura está subida");
    let uploads = s.bench.canvas.preview_uploads();
    let state_id = s.bench.doc.state_id();

    let mut frames = s.drags_without_release();
    // El primero (mover sin pulsar) no tiene nada pendiente; el segundo es la pulsación.
    let mut without_press = frames.remove(0);
    assert!(without_press.pending_2.is_empty());
    assert_eq!(rectangles(&without_press.output), s.base);
    assert_eq!(
        without_press.texture_2.texels,
        texture_expected(&s.bytes, []),
        "sin trazo, la textura es el documento"
    );
    checks_uploads(&without_press.output, texture, false, "sin pulsar");
    without_press.output.textures_delta.clear();

    for (i, f) in frames.iter_mut().enumerate() {
        let pending_2 = i + 1;
        checks_frame_with_stroke(&s, f, pending_2, true, &format!("fotograma {pending_2}"));
        f.output.textures_delta.clear();
    }
    assert_eq!(frames.len(), 20, "veinte píxeles visibles");

    // Varios fotogramas más sin soltar ni mover: el trazo sigue y la superposición, igual (la
    // textura lleva los veinte y ya no sube nada).
    for k in 0..4 {
        let mut f = s.bench.frame(vec![]);
        checks_frame_with_stroke(&s, &f, 20, false, &format!("reposo {k}"));
        assert!(!f.change);
        f.output.textures_delta.clear();
    }

    // La textura, la vista previa y el documento, como estaban.
    assert_eq!(s.bench.canvas.texture_id(), Some(texture), "misma textura");
    assert_eq!(s.bench.canvas.preview_uploads(), uploads);
    assert_eq!(to_rgba8(s.bench.image_2()), s.bytes, "to_rgba8 intacta");
    assert!(!s.bench.doc.can_undo());
    assert_eq!(s.bench.doc.state_id(), state_id);
    assert!(s.bench.tool.busy());
}

#[test]
fn row_10_the_overlay_is_the_whole_stroke_in_the_order_of_the_pencil() {
    let mut s = Session::new();
    let mut frames = s.drags_without_release();
    // Cada fotograma lleva el trazo entero hasta ese momento: un prefijo del orden del lápiz.
    for (i, f) in frames.iter().enumerate().skip(1) {
        let until: Vec<(u32, u32)> = f.pending_2.iter().map(|e| (e.x, e.y)).collect();
        assert_eq!(until, the_twenty()[..i], "fotograma {i}: prefijo del trazo");
    }
    let ultimo = frames.last_mut().expect("hay fotogramas");
    let pending_2: Vec<(u32, u32)> = ultimo.pending_2.iter().map(|e| (e.x, e.y)).collect();
    assert_eq!(pending_2, the_twenty());
    for editing in &ultimo.pending_2 {
        assert_eq!(editing.pixel, Pixel::Rgba(Rgba::TRANSPARENT));
    }
    // Y la textura compuesta del último fotograma lleva todos los píxeles pendientes (los veinte,
    // transparentes) y nada más.
    assert_eq!(
        ultimo.texture_2.texels,
        texture_expected(&s.bytes, the_twenty()),
        "la textura lleva el trazo entero"
    );
    for f in &mut frames {
        f.output.textures_delta.clear();
    }
}

#[test]
fn row_10_after_the_end_and_the_refresh_the_texture_is_the_document_with_those_transparent_pixels()
{
    let mut s = Session::new();
    let texture = s.bench.canvas.texture_id().expect("la textura está subida");
    for mut f in s.drags_without_release() {
        f.output.textures_delta.clear();
    }
    let ultimo = s.pos(ULTIMO, ROW);

    // Soltar: `Move` y `End` en el mismo fotograma; el documento cambia y la app hace `refresh`.
    let mut f = s.bench.frame(vec![left_2(ultimo, false)]);
    assert!(f.change, "End da Ok(true)");
    assert!(f.pending_2.is_empty(), "nada pendiente tras el End");
    assert!(!s.bench.tool.busy());
    // Antes: «una sola subida» entera (`delta.is_whole()`). Ahora se afirma el resultado: lo único
    // que se sube es la textura del lienzo y, compuesta, es el documento ya con el borrado.
    assert!(
        f.output.textures_delta.set.contains_key(&texture),
        "la textura se actualiza"
    );
    assert!(
        f.output.textures_delta.set.keys().all(|id| *id == texture),
        "y de nada más"
    );
    assert_eq!(f.texture_2.size_2, [WIDTH as usize, HEIGHT as usize]);
    let twenty: BTreeSet<(u32, u32)> = the_twenty().into_iter().collect();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let painted = f.texture_2.texel(x as usize, y as usize).expect("dentro");
            if twenty.contains(&(x, y)) {
                assert_eq!(painted.a(), 0, "({x}, {y}) transparente en la textura");
            } else {
                assert_eq!(painted, Color32::from_rgb(0, 0, 255), "({x}, {y}) intacto");
            }
        }
    }
    // La textura compuesta es `from_rgba_unmultiplied(to_rgba8(document))`, píxel a píxel.
    let document: Vec<Color32> = to_rgba8(s.bench.image_2())
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    assert_eq!(f.texture_2.texels, document, "la textura es el documento");

    // Sin superposición: solo la base. El fondo que se ve bajo esos píxeles es el damero de fondo
    // de la imagen, el mismo que se ve a través de los texels de alfa 0 (y fuera de la imagen,
    // `BACKGROUND`).
    let rects = rectangles(&f.output);
    assert_eq!(rects, s.base, "sin superposición tras el End");
    assert_eq!(rects.first().map(|(_, fill)| *fill), Some(BACKGROUND));
    assert_eq!(
        meshes_of_the_checker(&f.output, s.checker_2),
        s.base_checker,
        "sin superposición tras el End: solo el damero de fondo"
    );
    f.output.textures_delta.clear();

    // `to_rgba8` de la imagen: esos píxeles con alfa 0 y el resto como estaba.
    let now = to_rgba8(s.bench.image_2());
    assert_eq!(now.len(), s.bytes.len());
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let i = ((y * WIDTH + x) * 4) as usize;
            if twenty.contains(&(x, y)) {
                assert_eq!(now[i + 3], 0, "({x}, {y}) con alfa 0");
            } else {
                assert_eq!(now[i..i + 4], s.bytes[i..i + 4], "({x}, {y}) intacto");
            }
        }
    }
    assert!(s.bench.doc.can_undo(), "un paso");

    // Y los fotogramas siguientes ya no suben nada y la textura no cambia.
    let mut next = s.bench.frame(vec![]);
    assert!(next.output.textures_delta.set.is_empty());
    assert_eq!(rectangles(&next.output), s.base);
    assert_eq!(next.texture_2.texels, document);
    next.output.textures_delta.clear();
}

#[test]
fn row_10_the_stroke_of_an_eraser_that_changes_nothing_leaves_document_and_texture_as_they_were() {
    // Sobre píxeles ya transparentes, la superposición se ve igual (borrar lo ya transparente no
    // cambia lo visible) y al soltar no hay `Ok(true)` ni `refresh`: el documento no cambia y la
    // textura vuelve a ser el documento (los bloques de la superposición se regeneran).
    let mut s = Session::new();
    // Primero se borra la fila 5 (de `FIRST` a `ULTIMO`) y se suelta.
    for mut f in s.drags_without_release() {
        f.output.textures_delta.clear();
    }
    let ultimo = s.pos(ULTIMO, ROW);
    let mut f = s.bench.frame(vec![left_2(ultimo, false)]);
    f.output.textures_delta.clear();
    assert!(f.change);
    let deleted = texture_expected(&s.bytes, the_twenty());
    assert_eq!(f.texture_2.texels, deleted, "la fila 5 borrada");
    let bytes_deleted = to_rgba8(s.bench.image_2());
    let state_id = s.bench.doc.state_id();

    // Segundo trazo por los mismos píxeles: durante el gesto la textura se ve igual.
    let start_2 = s.pos(FIRST, ROW);
    let f = s.bench.step(vec![mover(start_2)]);
    assert_eq!(f.texture_2.texels, deleted);
    let f = s.bench.step(vec![left_2(start_2, true)]);
    assert_eq!(f.pending_2.len(), 1, "el borrador pinta la superposición");
    assert_eq!(
        f.texture_2.texels, deleted,
        "borrar lo transparente no cambia lo visible"
    );
    let middle = s.pos(FIRST + 5, ROW);
    let f = s.bench.step(vec![mover(middle)]);
    assert!(!f.pending_2.is_empty());
    assert_eq!(f.texture_2.texels, deleted, "ni durante el trazo");
    assert_eq!(
        to_rgba8(s.bench.image_2()),
        bytes_deleted,
        "el documento no cambia"
    );

    let mut f = s.bench.frame(vec![left_2(middle, false)]);
    assert!(!f.change, "ya estaban transparentes");
    assert!(f.pending_2.is_empty(), "nada pendiente tras el End");
    // Al terminar, la textura vuelve a ser el documento (que no cambió).
    assert_eq!(
        to_rgba8(s.bench.image_2()),
        bytes_deleted,
        "documento como estaba"
    );
    assert_eq!(s.bench.doc.state_id(), state_id, "ni un paso nuevo");
    assert_eq!(f.texture_2.texels, deleted, "la textura es el documento");
    assert_eq!(
        f.texture_2.texels,
        texture_expected(&bytes_deleted, []),
        "y es from_rgba_unmultiplied(to_rgba8(documento))"
    );
    f.output.textures_delta.clear();

    // En reposo, nada más se sube.
    let mut rest_2_2 = s.bench.frame(vec![]);
    assert!(
        rest_2_2.output.textures_delta.set.is_empty(),
        "reposo: sin subidas"
    );
    assert_eq!(rest_2_2.texture_2.texels, deleted);
    rest_2_2.output.textures_delta.clear();
}
