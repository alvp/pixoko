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
//! `common::texturas::Texturas` y guarda en cada `Fotograma` la textura de la baldosa 0 tal como
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
use common::texturas::{Textura, Texturas};
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
    /// La textura de la baldosa 0 compuesta con los `textures_delta` hasta este fotograma.
    textura: Textura,
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
    /// Contenido de cada textura, compuesto en orden con los `textures_delta` de cada fotograma.
    texturas: Texturas,
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
            texturas: Texturas::new(),
        }
    }

    fn imagen(&self) -> &Image {
        self.doc.image(self.id).expect("hay imagen")
    }

    /// Un fotograma como el de la app (`interact`, eventos y `apply`, `set_overlay`, `refresh` o
    /// `sync`, `paint`). Los cambios de textura se aplican a `texturas` y se devuelven también en
    /// `output` (quien llama decide si los mira y los limpia: egui entra en pánico si se sueltan
    /// sin aplicar).
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
            texturas,
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
            let pendientes: Vec<PixelEdit> = tool.preview().collect();
            let overlay: Vec<(u32, u32, Rgba)> = pendientes
                .iter()
                .filter_map(|e| Some((e.x, e.y, pixel_rgba(image, e.pixel)?)))
                .collect();
            // El trazo pendiente entero, después de `interact` y antes de `paint`; al soltar queda
            // vacío y los bloques que tocó se regeneran desde el documento en `sync` o `refresh`.
            canvas.set_overlay(ui.ctx(), image, &overlay);
            if cambio {
                canvas.refresh(ui.ctx(), image);
            } else {
                canvas.sync(ui.ctx(), image).expect("sync");
            }
            let info = canvas.paint(ui);
            resultado = Some((info, cambio, pendientes));
        });
        texturas.aplica(&output.textures_delta);
        let textura = self
            .canvas
            .texture_id()
            .and_then(|id| self.texturas.get(id))
            .cloned()
            .unwrap_or_default();
        let (info, cambio, pendientes) = resultado.expect("el lienzo se dibuja");
        Fotograma {
            output,
            textura,
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

/// Lo que debe haber en la textura de la baldosa: `to_rgba8` de la imagen original (el oráculo de
/// la unidad 14a, premultiplicado) con los `pixeles` borrados (alfa 0: `Color32::TRANSPARENT`).
fn textura_esperada(bytes: &[u8], pixeles: impl IntoIterator<Item = (u32, u32)>) -> Vec<Color32> {
    let mut texels: Vec<Color32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    for (x, y) in pixeles {
        texels[(y * ANCHO + x) as usize] = Color32::TRANSPARENT;
    }
    texels
}

/// Lo que se subió en el fotograma: solo la textura del lienzo (la baldosa), con deltas parciales
/// (bloques) que caben en ella, y sin liberar ninguna textura. Con `hay_cambio` falso, nada.
fn comprueba_subidas(output: &FullOutput, textura: TextureId, hay_cambio: bool, etiqueta: &str) {
    assert!(
        output.textures_delta.free.is_empty(),
        "{etiqueta}: sin textures_delta.free"
    );
    if !hay_cambio {
        assert!(
            output.textures_delta.set.is_empty(),
            "{etiqueta}: sin cambios no se sube nada"
        );
        return;
    }
    for (id, deltas) in &output.textures_delta.set {
        assert_eq!(
            *id, textura,
            "{etiqueta}: solo se sube la textura del lienzo"
        );
        for delta in deltas {
            let ImageData::Color(color) = &delta.image;
            let [x0, y0] = delta.pos.unwrap_or_else(|| {
                panic!("{etiqueta}: la superposición sube bloques, no la entera")
            });
            assert!(
                x0 + color.size[0] <= ANCHO as usize && y0 + color.size[1] <= ALTO as usize,
                "{etiqueta}: el delta {:?} en {:?} cabe en la baldosa",
                color.size,
                (x0, y0)
            );
        }
    }
}

/// Lo que debe cumplir un fotograma con `n` píxeles pendientes y el botón aún abajo. `nuevo` es
/// que este fotograma añadió píxeles (y por tanto sube bloques).
fn comprueba_fotograma_con_trazo(s: &Sesion, f: &Fotograma, n: usize, nuevo: bool, etiqueta: &str) {
    let rects = rectangulos(&f.output);
    let mallas = mallas_del_damero(&f.output, s.damero);
    assert_eq!(f.pendientes.len(), n, "{etiqueta}: píxeles pendientes");
    let esperados = &los_veinte()[..n];
    let pendientes: Vec<(u32, u32)> = f.pendientes.iter().map(|e| (e.x, e.y)).collect();
    assert_eq!(
        pendientes, esperados,
        "{etiqueta}: el trazo entero, en el orden del lápiz"
    );
    // Antes: un cuadrilátero del damero por píxel pendiente (y antes un rectángulo de `BACKGROUND`)
    // sobre la base. Ahora la superposición no es una forma: el fotograma con trazo da las mismas
    // formas que el de reposo, y el borrado se ve en la textura.
    assert_eq!(
        rects, s.base,
        "{etiqueta}: la superposición no añade rectángulos (la base no cambia)"
    );
    assert_eq!(
        mallas, s.base_damero,
        "{etiqueta}: ni cuadriláteros del damero por píxel pendiente: solo el damero de fondo"
    );
    // La vista previa del borrador muestra los píxeles borrados: la textura compuesta lleva cada
    // píxel pendiente transparente (alfa 0) y el resto de la imagen como estaba.
    assert_eq!(
        f.textura.tamano,
        [ANCHO as usize, ALTO as usize],
        "{etiqueta}: tamaño de la textura"
    );
    assert_eq!(
        f.textura.texels,
        textura_esperada(&s.bytes, esperados.iter().copied()),
        "{etiqueta}: la textura es la imagen con los píxeles pendientes transparentes"
    );
    for &(x, y) in esperados {
        let texel = f.textura.texel(x as usize, y as usize).expect("dentro");
        assert_eq!(
            texel,
            Color32::TRANSPARENT,
            "{etiqueta}: ({x}, {y}) borrado"
        );
        assert_eq!(texel.a(), 0, "{etiqueta}: ({x}, {y}) con alfa 0");
    }
    // Solo se suben bloques de la textura, y solo en el fotograma en que cambia.
    let textura = s.banco.canvas.texture_id().expect("la textura está subida");
    comprueba_subidas(&f.output, textura, nuevo, etiqueta);
}

// Fila 10.

#[test]
fn el_fondo_del_lienzo_es_el_gris_40() {
    assert_eq!(BACKGROUND, Color32::from_gray(40));
}

#[test]
fn fila_10_cada_fotograma_muestra_en_la_textura_los_pixeles_pendientes_borrados_sin_formas_extra() {
    let mut s = Sesion::nueva();
    let texture = s.banco.canvas.texture_id().expect("la textura está subida");
    let subidas = s.banco.canvas.preview_uploads();
    let state_id = s.banco.doc.state_id();

    let mut fotogramas = s.arrastra_sin_soltar();
    // El primero (mover sin pulsar) no tiene nada pendiente; el segundo es la pulsación.
    let mut sin_pulsar = fotogramas.remove(0);
    assert!(sin_pulsar.pendientes.is_empty());
    assert_eq!(rectangulos(&sin_pulsar.output), s.base);
    assert_eq!(
        sin_pulsar.textura.texels,
        textura_esperada(&s.bytes, []),
        "sin trazo, la textura es el documento"
    );
    comprueba_subidas(&sin_pulsar.output, texture, false, "sin pulsar");
    sin_pulsar.output.textures_delta.clear();

    for (i, f) in fotogramas.iter_mut().enumerate() {
        let pendientes = i + 1;
        comprueba_fotograma_con_trazo(&s, f, pendientes, true, &format!("fotograma {pendientes}"));
        f.output.textures_delta.clear();
    }
    assert_eq!(fotogramas.len(), 20, "veinte píxeles visibles");

    // Varios fotogramas más sin soltar ni mover: el trazo sigue y la superposición, igual (la
    // textura lleva los veinte y ya no sube nada).
    for k in 0..4 {
        let mut f = s.banco.fotograma(vec![]);
        comprueba_fotograma_con_trazo(&s, &f, 20, false, &format!("reposo {k}"));
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
    // Cada fotograma lleva el trazo entero hasta ese momento: un prefijo del orden del lápiz.
    for (i, f) in fotogramas.iter().enumerate().skip(1) {
        let hasta: Vec<(u32, u32)> = f.pendientes.iter().map(|e| (e.x, e.y)).collect();
        assert_eq!(hasta, los_veinte()[..i], "fotograma {i}: prefijo del trazo");
    }
    let ultimo = fotogramas.last_mut().expect("hay fotogramas");
    let pendientes: Vec<(u32, u32)> = ultimo.pendientes.iter().map(|e| (e.x, e.y)).collect();
    assert_eq!(pendientes, los_veinte());
    for edicion in &ultimo.pendientes {
        assert_eq!(edicion.pixel, Pixel::Rgba(Rgba::TRANSPARENT));
    }
    // Y la textura compuesta del último fotograma lleva todos los píxeles pendientes (los veinte,
    // transparentes) y nada más.
    assert_eq!(
        ultimo.textura.texels,
        textura_esperada(&s.bytes, los_veinte()),
        "la textura lleva el trazo entero"
    );
    for f in &mut fotogramas {
        f.output.textures_delta.clear();
    }
}

#[test]
fn fila_10_tras_el_end_y_el_refresh_la_textura_es_el_documento_con_esos_pixeles_transparentes() {
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
    assert_eq!(f.textura.tamano, [ANCHO as usize, ALTO as usize]);
    let veinte: BTreeSet<(u32, u32)> = los_veinte().into_iter().collect();
    for y in 0..ALTO {
        for x in 0..ANCHO {
            let pintado = f.textura.texel(x as usize, y as usize).expect("dentro");
            if veinte.contains(&(x, y)) {
                assert_eq!(pintado.a(), 0, "({x}, {y}) transparente en la textura");
            } else {
                assert_eq!(pintado, Color32::from_rgb(0, 0, 255), "({x}, {y}) intacto");
            }
        }
    }
    // La textura compuesta es `from_rgba_unmultiplied(to_rgba8(documento))`, píxel a píxel.
    let documento: Vec<Color32> = to_rgba8(s.banco.imagen())
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    assert_eq!(f.textura.texels, documento, "la textura es el documento");

    // Sin superposición: solo la base. El fondo que se ve bajo esos píxeles es el damero de fondo
    // de la imagen, el mismo que se ve a través de los texels de alfa 0 (y fuera de la imagen,
    // `BACKGROUND`).
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

    // Y los fotogramas siguientes ya no suben nada y la textura no cambia.
    let mut siguiente = s.banco.fotograma(vec![]);
    assert!(siguiente.output.textures_delta.set.is_empty());
    assert_eq!(rectangulos(&siguiente.output), s.base);
    assert_eq!(siguiente.textura.texels, documento);
    siguiente.output.textures_delta.clear();
}

#[test]
fn fila_10_el_trazo_de_un_borrador_que_no_cambia_nada_deja_documento_y_textura_como_estaban() {
    // Sobre píxeles ya transparentes, la superposición se ve igual (borrar lo ya transparente no
    // cambia lo visible) y al soltar no hay `Ok(true)` ni `refresh`: el documento no cambia y la
    // textura vuelve a ser el documento (los bloques de la superposición se regeneran).
    let mut s = Sesion::nueva();
    // Primero se borra la fila 5 (de `PRIMERO` a `ULTIMO`) y se suelta.
    for mut f in s.arrastra_sin_soltar() {
        f.output.textures_delta.clear();
    }
    let ultimo = s.pos(ULTIMO, FILA);
    let mut f = s.banco.fotograma(vec![izquierdo(ultimo, false)]);
    f.output.textures_delta.clear();
    assert!(f.cambio);
    let borrada = textura_esperada(&s.bytes, los_veinte());
    assert_eq!(f.textura.texels, borrada, "la fila 5 borrada");
    let bytes_borrada = to_rgba8(s.banco.imagen());
    let state_id = s.banco.doc.state_id();

    // Segundo trazo por los mismos píxeles: durante el gesto la textura se ve igual.
    let inicio = s.pos(PRIMERO, FILA);
    let f = s.banco.paso(vec![mover(inicio)]);
    assert_eq!(f.textura.texels, borrada);
    let f = s.banco.paso(vec![izquierdo(inicio, true)]);
    assert_eq!(f.pendientes.len(), 1, "el borrador pinta la superposición");
    assert_eq!(
        f.textura.texels, borrada,
        "borrar lo transparente no cambia lo visible"
    );
    let medio = s.pos(PRIMERO + 5, FILA);
    let f = s.banco.paso(vec![mover(medio)]);
    assert!(!f.pendientes.is_empty());
    assert_eq!(f.textura.texels, borrada, "ni durante el trazo");
    assert_eq!(
        to_rgba8(s.banco.imagen()),
        bytes_borrada,
        "el documento no cambia"
    );

    let mut f = s.banco.fotograma(vec![izquierdo(medio, false)]);
    assert!(!f.cambio, "ya estaban transparentes");
    assert!(f.pendientes.is_empty(), "nada pendiente tras el End");
    // Al terminar, la textura vuelve a ser el documento (que no cambió).
    assert_eq!(
        to_rgba8(s.banco.imagen()),
        bytes_borrada,
        "documento como estaba"
    );
    assert_eq!(s.banco.doc.state_id(), state_id, "ni un paso nuevo");
    assert_eq!(f.textura.texels, borrada, "la textura es el documento");
    assert_eq!(
        f.textura.texels,
        textura_esperada(&bytes_borrada, []),
        "y es from_rgba_unmultiplied(to_rgba8(documento))"
    );
    f.output.textures_delta.clear();

    // En reposo, nada más se sube.
    let mut reposo = s.banco.fotograma(vec![]);
    assert!(
        reposo.output.textures_delta.set.is_empty(),
        "reposo: sin subidas"
    );
    assert_eq!(reposo.textura.texels, borrada);
    reposo.output.textures_delta.clear();
}
