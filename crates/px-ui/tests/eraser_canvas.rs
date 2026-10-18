//! Tests de la superposición del borrador en el lienzo de egui sin ventana (specs/hito-2-
//! cuentagotas-borrador.md, decisión 3 y fila 10 de «Casos límite»): el mismo banco que
//! `tests/pencil.rs` (un `egui::Context` con `RawInput` sintético), copiado y no modificado, pero
//! con una `ToolState` con el borrador, como hace la app: `Canvas::interact_ordered`, los eventos
//! a la herramienta, `refresh` si el documento cambió y `Canvas::paint` con la superposición de
//! `ToolState::preview` convertida con `pixel_rgba`.
//!
//! Lo pintado se lee de las formas del fotograma: los rectángulos rellenos (`Shape::Rect`) y las
//! mallas con la textura del damero (`Canvas::checker_texture_id`). Desde la unidad 17b un píxel
//! borrado se ve como el damero de transparencia y no como el fondo liso: el fotograma sin trazo
//! da la base (el fondo liso del lienzo y el cuadrilátero del damero de la imagen) y cada
//! fotograma con trazo debe añadir **un cuadrilátero del damero** por píxel pendiente, en el sitio
//! de ese píxel y sin tinte, y ningún rectángulo ni otra forma más (specs/hito-2-damero-lienzo.md,
//! decisión 8 y fila 7; antes era un rectángulo de `BACKGROUND` por píxel).
//!
//! Cada cambio de estado del botón va en su propio fotograma (egui necesita uno por pulsación o
//! soltado). 800×600 con 64×48 da zoom ×12 y las posiciones son enteras.

use std::collections::BTreeSet;

use egui::{
    Color32, Context, Event, FullOutput, ImageData, Modifiers, PointerButton, Pos2, RawInput, Rect,
    Shape, TextureId, pos2, vec2,
};
use px_core::{Document, Image, ImageId, Mode, Pixel, PixelEdit, Rgba};
use px_ui::canvas::{BACKGROUND, Canvas, CanvasInfo};
use px_ui::texture::{pixel_rgba, to_rgba8};
use px_ui::tool::{Tool, ToolAction, ToolState};

const SCREEN: egui::Vec2 = vec2(800.0, 600.0);
const ANCHO: u32 = 64;
const ALTO: u32 = 48;
/// Píxeles del trazo: la fila 5, de `x = 10` a `x = 29`.
const PRIMERO: i32 = 10;
const ULTIMO: i32 = 29;
const FILA: i32 = 5;

fn azul() -> Pixel {
    Pixel::Rgba(Rgba::opaque(0, 0, 255))
}

/// Imagen RGBA de 64×48 azul opaca.
fn imagen_azul() -> Image {
    let mut image = Image::new(ANCHO, ALTO, Mode::Rgba).expect("imagen válida");
    for y in 0..ALTO {
        for x in 0..ANCHO {
            image.set_pixel(x, y, azul()).expect("dentro");
        }
    }
    image
}

// Entradas sintéticas.

fn mover(pos: Pos2) -> Event {
    Event::PointerMoved(pos)
}

fn izquierdo(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// Centro del píxel `(x, y)` en una vista con el rectángulo y el zoom dados (puntos de egui).
fn centro(rect: Rect, zoom: i64, x: i32, y: i32) -> Pos2 {
    let z = zoom as f32;
    pos2(
        rect.min.x + (x as f32 + 0.5) * z,
        rect.min.y + (y as f32 + 0.5) * z,
    )
}

/// Rectángulos rellenos de una forma, con su color de relleno (aplana `Shape::Vec`).
fn recoge_rectangulos(shape: &Shape, salida: &mut Vec<(Rect, Color32)>) {
    match shape {
        Shape::Rect(rect) => salida.push((rect.rect, rect.fill)),
        Shape::Vec(formas) => {
            for forma in formas {
                recoge_rectangulos(forma, salida);
            }
        }
        _ => {}
    }
}

fn rectangulos(output: &FullOutput) -> Vec<(Rect, Color32)> {
    let mut salida = Vec::new();
    for clipped in &output.shapes {
        recoge_rectangulos(&clipped.shape, &mut salida);
    }
    salida
}

/// Mallas con la textura `textura` (la del damero), con su caja y su tinte (el color de sus
/// vértices, que debe ser el mismo en todos). Aplana `Shape::Vec`.
fn recoge_mallas(shape: &Shape, textura: TextureId, salida: &mut Vec<(Rect, Color32)>) {
    match shape {
        Shape::Mesh(malla) if malla.texture_id == textura => {
            let mut caja = Rect::NOTHING;
            for v in &malla.vertices {
                caja.extend_with(v.pos);
            }
            let tinte = malla.vertices.first().expect("malla con vértices").color;
            assert!(
                malla.vertices.iter().all(|v| v.color == tinte),
                "todos los vértices de una malla del damero llevan el mismo tinte"
            );
            salida.push((caja, tinte));
        }
        Shape::Vec(formas) => {
            for forma in formas {
                recoge_mallas(forma, textura, salida);
            }
        }
        _ => {}
    }
}

fn mallas_del_damero(output: &FullOutput, textura: TextureId) -> Vec<(Rect, Color32)> {
    let mut salida = Vec::new();
    for clipped in &output.shapes {
        recoge_mallas(&clipped.shape, textura, &mut salida);
    }
    salida
}

/// Lo que el fotograma dejó: formas, cambios de textura y lo que hizo la herramienta.
struct Fotograma {
    output: FullOutput,
    info: CanvasInfo,
    /// `Ok(true)` de algún evento del fotograma: el documento cambió.
    cambio: bool,
    /// La superposición pendiente que se pintó, en orden.
    pendientes: Vec<PixelEdit>,
}

/// La app mínima: lienzo, documento y herramienta, con el borrador elegido.
struct Banco {
    ctx: Context,
    canvas: Canvas,
    doc: Document,
    id: ImageId,
    tool: ToolState,
}

impl Banco {
    fn nuevo() -> Banco {
        let image = imagen_azul();
        let doc = Document::new(image.clone());
        let id = doc.main_image();
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(Tool::Eraser));
        assert_eq!(tool.tool(), Tool::Eraser);
        let mut canvas = Canvas::new();
        canvas.set_image(&image);
        Banco {
            ctx: Context::default(),
            canvas,
            doc,
            id,
            tool,
        }
    }

    fn imagen(&self) -> &Image {
        self.doc.image(self.id).expect("hay imagen")
    }

    /// Un fotograma como el de la app. Los cambios de textura se devuelven en `output` (quien
    /// llama decide si los mira y los limpia: egui entra en pánico si se sueltan sin aplicar).
    fn fotograma(&mut self, events: Vec<Event>) -> Fotograma {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events,
            ..RawInput::default()
        };
        let Banco {
            ctx,
            canvas,
            doc,
            id,
            tool,
        } = self;
        let mut resultado = None;
        let output = ctx.run_ui(input, |ui| {
            let mut cambio = false;
            for entrada in canvas.interact_ordered(ui) {
                cambio |= tool
                    .canvas_input_with(doc, *id, None, entrada)
                    .expect("el borrador no falla");
            }
            let image = doc.image(*id).expect("hay imagen");
            if cambio {
                canvas.refresh(ui.ctx(), image);
            }
            let pendientes: Vec<PixelEdit> = tool.preview().collect();
            let overlay: Vec<(u32, u32, Rgba)> = pendientes
                .iter()
                .filter_map(|e| Some((e.x, e.y, pixel_rgba(image, e.pixel)?)))
                .collect();
            let info = canvas.paint(ui, overlay);
            resultado = Some((info, cambio, pendientes));
        });
        let (info, cambio, pendientes) = resultado.expect("el lienzo se dibuja");
        Fotograma {
            output,
            info,
            cambio,
            pendientes,
        }
    }

    /// Un fotograma con los cambios de textura ya limpiados.
    fn paso(&mut self, events: Vec<Event>) -> Fotograma {
        let mut f = self.fotograma(events);
        f.output.textures_delta.clear();
        f
    }
}

/// Un banco con la imagen subida y el primer fotograma ya ejecutado.
struct Sesion {
    banco: Banco,
    /// Rectángulos del fotograma en reposo: el fondo del lienzo, sin superposición.
    base: Vec<(Rect, Color32)>,
    /// Textura del damero y sus mallas del fotograma en reposo: el cuadrilátero del damero de
    /// fondo de la imagen, sin superposición.
    damero: TextureId,
    base_damero: Vec<(Rect, Color32)>,
    /// `to_rgba8` de la imagen antes de empezar.
    bytes: Vec<u8>,
    rect: Rect,
    zoom: i64,
}

impl Sesion {
    fn nueva() -> Sesion {
        let mut banco = Banco::nuevo();
        let primero = banco.paso(vec![]);
        let rect = primero.info.image_rect.expect("hay imagen");
        let zoom = primero.info.zoom.scale(1);
        assert_eq!(zoom, 12, "800×600 con 64×48 da zoom ×12");
        let reposo = banco.paso(vec![]);
        assert!(reposo.pendientes.is_empty());
        let base = rectangulos(&reposo.output);
        let damero = banco
            .canvas
            .checker_texture_id()
            .expect("el damero tiene su textura");
        let base_damero = mallas_del_damero(&reposo.output, damero);
        assert_eq!(base_damero.len(), 1, "un cuadrilátero de damero de fondo");
        let bytes = to_rgba8(banco.imagen());
        Sesion {
            banco,
            base,
            damero,
            base_damero,
            bytes,
            rect,
            zoom,
        }
    }

    fn pos(&self, x: i32, y: i32) -> Pos2 {
        centro(self.rect, self.zoom, x, y)
    }

    /// Pulsa en el primer píxel y arrastra hasta el último, un fotograma por píxel, y deja el
    /// botón abajo. Devuelve los fotogramas, el primero el de la pulsación.
    fn arrastra_sin_soltar(&mut self) -> Vec<Fotograma> {
        let inicio = self.pos(PRIMERO, FILA);
        let mut fotogramas = vec![self.banco.fotograma(vec![mover(inicio)])];
        fotogramas.push(self.banco.fotograma(vec![izquierdo(inicio, true)]));
        for x in PRIMERO + 1..=ULTIMO {
            let p = self.pos(x, FILA);
            fotogramas.push(self.banco.fotograma(vec![mover(p)]));
        }
        fotogramas
    }
}

/// Posiciones `(x, y)` de los píxeles de la fila `FILA` de `PRIMERO` a `ULTIMO`.
fn los_veinte() -> Vec<(u32, u32)> {
    (PRIMERO..=ULTIMO)
        .map(|x| (u32::try_from(x).unwrap(), u32::try_from(FILA).unwrap()))
        .collect()
}

/// Esquinas (redondeadas) de los rectángulos de `extras`, para comparar sin importar el orden.
fn esquinas(extras: &[(Rect, Color32)]) -> BTreeSet<(i32, i32)> {
    extras
        .iter()
        .map(|(rect, _)| (rect.min.x.round() as i32, rect.min.y.round() as i32))
        .collect()
}

/// Esquinas que debe tener la superposición de los píxeles dados con la vista de `rect` y `zoom`.
fn esquinas_esperadas(
    rect: Rect,
    zoom: i64,
    pixeles: impl IntoIterator<Item = (u32, u32)>,
) -> BTreeSet<(i32, i32)> {
    pixeles
        .into_iter()
        .map(|(x, y)| {
            (
                (rect.min.x + x as f32 * zoom as f32).round() as i32,
                (rect.min.y + y as f32 * zoom as f32).round() as i32,
            )
        })
        .collect()
}

/// Lo que debe cumplir un fotograma con `n` píxeles pendientes y el botón aún abajo.
fn comprueba_fotograma_con_trazo(s: &Sesion, f: &Fotograma, n: usize, etiqueta: &str) {
    let rects = rectangulos(&f.output);
    let mallas = mallas_del_damero(&f.output, s.damero);
    assert_eq!(f.pendientes.len(), n, "{etiqueta}: píxeles pendientes");
    // Antes: un rectángulo de `BACKGROUND` por píxel pendiente y la base intacta. Ahora la
    // superposición del borrador (alfa 0) no añade ningún rectángulo: son cuadriláteros del damero.
    assert_eq!(
        rects, s.base,
        "{etiqueta}: la superposición no añade rectángulos (la base no cambia)"
    );
    assert_eq!(
        mallas.len(),
        s.base_damero.len() + n,
        "{etiqueta}: un cuadrilátero del damero por píxel pendiente y nada más"
    );
    assert_eq!(
        mallas[..s.base_damero.len()],
        s.base_damero[..],
        "{etiqueta}: la base no cambia"
    );
    let extras = &mallas[s.base_damero.len()..];
    for (rect, fill) in extras {
        assert_eq!(
            *fill,
            Color32::WHITE,
            "{etiqueta}: ningún otro color (sin tinte): {rect:?}"
        );
        assert!(
            (rect.width() - s.zoom as f32).abs() < 1e-3
                && (rect.height() - s.zoom as f32).abs() < 1e-3,
            "{etiqueta}: un rectángulo de zoom × zoom, no {rect:?}"
        );
    }
    let rect_imagen = f.info.image_rect.expect("hay imagen");
    assert_eq!(
        esquinas(extras),
        esquinas_esperadas(rect_imagen, s.zoom, f.pendientes.iter().map(|e| (e.x, e.y))),
        "{etiqueta}: cada rectángulo está sobre su píxel"
    );
    // Sin subir nada: ni la textura ni la vista previa de la paleta.
    assert!(
        f.output.textures_delta.set.is_empty(),
        "{etiqueta}: sin textures_delta.set"
    );
    assert!(
        f.output.textures_delta.free.is_empty(),
        "{etiqueta}: sin textures_delta.free"
    );
}

// Fila 10.

#[test]
fn el_fondo_del_lienzo_es_el_gris_40() {
    assert_eq!(BACKGROUND, Color32::from_gray(40));
}

#[test]
fn fila_10_cada_fotograma_pinta_un_cuadrilatero_del_damero_por_pixel_pendiente() {
    let mut s = Sesion::nueva();
    let texture = s.banco.canvas.texture_id().expect("la textura está subida");
    let subidas = s.banco.canvas.preview_uploads();
    let state_id = s.banco.doc.state_id();

    let mut fotogramas = s.arrastra_sin_soltar();
    // El primero (mover sin pulsar) no tiene nada pendiente; el segundo es la pulsación.
    let sin_pulsar = fotogramas.remove(0);
    assert!(sin_pulsar.pendientes.is_empty());
    assert_eq!(rectangulos(&sin_pulsar.output), s.base);

    for (i, f) in fotogramas.iter_mut().enumerate() {
        let pendientes = i + 1;
        comprueba_fotograma_con_trazo(&s, f, pendientes, &format!("fotograma {pendientes}"));
        f.output.textures_delta.clear();
    }
    assert_eq!(fotogramas.len(), 20, "veinte píxeles visibles");

    // Varios fotogramas más sin soltar ni mover: el trazo sigue y la superposición, igual.
    for k in 0..4 {
        let mut f = s.banco.fotograma(vec![]);
        comprueba_fotograma_con_trazo(&s, &f, 20, &format!("reposo {k}"));
        assert!(!f.cambio);
        f.output.textures_delta.clear();
    }

    // La textura, la vista previa y el documento, como estaban.
    assert_eq!(s.banco.canvas.texture_id(), Some(texture), "misma textura");
    assert_eq!(s.banco.canvas.preview_uploads(), subidas);
    assert_eq!(to_rgba8(s.banco.imagen()), s.bytes, "to_rgba8 intacta");
    assert!(!s.banco.doc.can_undo());
    assert_eq!(s.banco.doc.state_id(), state_id);
    assert!(s.banco.tool.busy());
}

#[test]
fn fila_10_la_superposicion_es_el_trazo_entero_en_el_orden_del_lapiz() {
    let mut s = Sesion::nueva();
    let mut fotogramas = s.arrastra_sin_soltar();
    let ultimo = fotogramas.last_mut().expect("hay fotogramas");
    ultimo.output.textures_delta.clear();
    let pendientes: Vec<(u32, u32)> = ultimo.pendientes.iter().map(|e| (e.x, e.y)).collect();
    assert_eq!(pendientes, los_veinte());
    for edicion in &ultimo.pendientes {
        assert_eq!(edicion.pixel, Pixel::Rgba(Rgba::TRANSPARENT));
    }
}

#[test]
fn fila_10_tras_el_end_y_el_refresh_hay_una_sola_subida_con_esos_pixeles_transparentes() {
    let mut s = Sesion::nueva();
    let texture = s.banco.canvas.texture_id().expect("la textura está subida");
    for mut f in s.arrastra_sin_soltar() {
        f.output.textures_delta.clear();
    }
    let ultimo = s.pos(ULTIMO, FILA);

    // Soltar: `Move` y `End` en el mismo fotograma; el documento cambia y la app hace `refresh`.
    let mut f = s.banco.fotograma(vec![izquierdo(ultimo, false)]);
    assert!(f.cambio, "End da Ok(true)");
    assert!(f.pendientes.is_empty(), "nada pendiente tras el End");
    assert!(!s.banco.tool.busy());
    let subidas = f
        .output
        .textures_delta
        .set
        .get(&texture)
        .expect("la textura se actualiza");
    assert_eq!(subidas.len(), 1, "una sola subida");
    assert_eq!(f.output.textures_delta.set.len(), 1, "y de nada más");
    let delta = &subidas[0];
    assert!(delta.is_whole(), "se regenera la textura completa");
    let ImageData::Color(color) = &delta.image;
    assert_eq!(color.size, [ANCHO as usize, ALTO as usize]);
    let veinte: BTreeSet<(u32, u32)> = los_veinte().into_iter().collect();
    for y in 0..ALTO {
        for x in 0..ANCHO {
            let pintado = color.pixels[(y * ANCHO + x) as usize];
            if veinte.contains(&(x, y)) {
                assert_eq!(pintado.a(), 0, "({x}, {y}) transparente en la textura");
            } else {
                assert_eq!(pintado, Color32::from_rgb(0, 0, 255), "({x}, {y}) intacto");
            }
        }
    }

    // Sin superposición: solo la base. El fondo que se ve bajo esos píxeles es el damero de fondo
    // de la imagen, el mismo que pintaba la superposición (y fuera de la imagen, `BACKGROUND`).
    let rects = rectangulos(&f.output);
    assert_eq!(rects, s.base, "sin superposición tras el End");
    assert_eq!(rects.first().map(|(_, fill)| *fill), Some(BACKGROUND));
    assert_eq!(
        mallas_del_damero(&f.output, s.damero),
        s.base_damero,
        "sin superposición tras el End: solo el damero de fondo"
    );
    f.output.textures_delta.clear();

    // `to_rgba8` de la imagen: esos píxeles con alfa 0 y el resto como estaba.
    let ahora = to_rgba8(s.banco.imagen());
    assert_eq!(ahora.len(), s.bytes.len());
    for y in 0..ALTO {
        for x in 0..ANCHO {
            let i = ((y * ANCHO + x) * 4) as usize;
            if veinte.contains(&(x, y)) {
                assert_eq!(ahora[i + 3], 0, "({x}, {y}) con alfa 0");
            } else {
                assert_eq!(ahora[i..i + 4], s.bytes[i..i + 4], "({x}, {y}) intacto");
            }
        }
    }
    assert!(s.banco.doc.can_undo(), "un paso");

    // Y los fotogramas siguientes ya no suben nada.
    let mut siguiente = s.banco.fotograma(vec![]);
    assert!(siguiente.output.textures_delta.set.is_empty());
    assert_eq!(rectangulos(&siguiente.output), s.base);
    siguiente.output.textures_delta.clear();
}

#[test]
fn fila_10_el_trazo_de_un_borrador_que_no_cambia_nada_no_sube_la_textura() {
    // Sobre píxeles ya transparentes, la superposición se pinta igual pero al soltar no hay
    // `Ok(true)`, no hay `refresh` y no hay subida.
    let mut s = Sesion::nueva();
    // Primero se borra la fila 5 (de `PRIMERO` a `ULTIMO`) y se suelta.
    for mut f in s.arrastra_sin_soltar() {
        f.output.textures_delta.clear();
    }
    let ultimo = s.pos(ULTIMO, FILA);
    let mut f = s.banco.fotograma(vec![izquierdo(ultimo, false)]);
    f.output.textures_delta.clear();
    assert!(f.cambio);

    // Segundo trazo por los mismos píxeles.
    let inicio = s.pos(PRIMERO, FILA);
    s.banco.paso(vec![mover(inicio)]);
    s.banco.paso(vec![izquierdo(inicio, true)]);
    let medio = s.pos(PRIMERO + 5, FILA);
    s.banco.paso(vec![mover(medio)]);
    let mut f = s.banco.fotograma(vec![izquierdo(medio, false)]);
    assert!(!f.cambio, "ya estaban transparentes");
    assert!(f.output.textures_delta.set.is_empty(), "sin subida");
    f.output.textures_delta.clear();
}
