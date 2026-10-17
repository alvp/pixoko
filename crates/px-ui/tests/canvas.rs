//! Tests del lienzo de egui sin ventana (specs/hito-2-ventana.md, «Lienzo» y «Cómo se prueba»,
//! punto 3): un `egui::Context` ejecuta fotogramas con `RawInput` sintético, sin GPU ni pantalla.

use egui::epaint::Primitive;
use egui::{
    Context, Event, FullOutput, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput,
    Rect, TextureFilter, TouchPhase, Vec2, pos2, vec2,
};
use px_core::{Image, Mode, Pixel, Rgba};
use px_ui::canvas::{Canvas, CanvasError, CanvasInfo};
use px_ui::view::Zoom;

const SCREEN: Vec2 = vec2(800.0, 600.0);

/// Imagen RGBA de 64×48 con un píxel de cada color en las esquinas.
fn imagen_de_prueba() -> Image {
    let mut image = Image::new(64, 48, Mode::Rgba).expect("imagen válida");
    image
        .set_pixel(0, 0, Pixel::Rgba(Rgba::opaque(255, 0, 0)))
        .expect("dentro");
    image
        .set_pixel(63, 47, Pixel::Rgba(Rgba::opaque(0, 0, 255)))
        .expect("dentro");
    image
}

struct Banco {
    ctx: Context,
    canvas: Canvas,
}

struct Fotograma {
    output: FullOutput,
    info: CanvasInfo,
}

impl Banco {
    fn nuevo() -> Banco {
        Banco {
            ctx: Context::default(),
            canvas: Canvas::new(),
        }
    }

    /// Ejecuta un fotograma con los eventos dados y devuelve su salida y la información del
    /// lienzo. Limpia los cambios de textura (egui entra en pánico si se sueltan sin aplicar).
    fn fotograma(&mut self, events: Vec<Event>, max_texture_side: Option<usize>) -> Fotograma {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events,
            max_texture_side,
            ..RawInput::default()
        };
        let mut info = None;
        let output = self.ctx.run_ui(input, |ui| {
            info = Some(self.canvas.show(ui));
        });
        Fotograma {
            output,
            info: info.expect("el lienzo se dibuja"),
        }
    }

    fn limpiar(fotograma: &mut Fotograma) {
        fotograma.output.textures_delta.clear();
    }
}

fn rueda(arriba: bool) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, if arriba { 1.0 } else { -1.0 }),
        phase: TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    }
}

fn tecla(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// Mallas del fotograma con la textura dada, con sus vértices.
fn mallas_con_textura(
    ctx: &Context,
    output: &mut FullOutput,
    texture: egui::TextureId,
) -> Vec<egui::epaint::Mesh> {
    let shapes = std::mem::take(&mut output.shapes);
    ctx.tessellate(shapes, output.pixels_per_point)
        .into_iter()
        .filter_map(|clipped| match clipped.primitive {
            Primitive::Mesh(mesh) if mesh.texture_id == texture => Some(mesh),
            _ => None,
        })
        .collect()
}

fn limites(mesh: &egui::epaint::Mesh) -> (Rect, Rect) {
    let mut pos = Rect::NOTHING;
    let mut uv = Rect::NOTHING;
    for v in &mesh.vertices {
        pos.extend_with(v.pos);
        uv.extend_with(v.uv);
    }
    (pos, uv)
}

fn cerca(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn la_imagen_se_pinta_con_una_textura_sin_suavizado_y_una_sola_malla() {
    let mut banco = Banco::nuevo();
    banco.canvas.set_image(&imagen_de_prueba());
    let mut f = banco.fotograma(vec![], None);
    let texture = banco.canvas.texture_id().expect("la textura está subida");
    let deltas = f
        .output
        .textures_delta
        .set
        .get(&texture)
        .expect("se sube la textura de la imagen");
    let options = deltas.iter().next().expect("un delta").options;
    assert_eq!(options.magnification, TextureFilter::Nearest);
    assert_eq!(options.minification, TextureFilter::Nearest);

    let mallas = mallas_con_textura(&banco.ctx, &mut f.output, texture);
    Banco::limpiar(&mut f);
    assert_eq!(
        mallas.len(),
        1,
        "una sola malla con la textura de la imagen"
    );
    let (pos, uv) = limites(&mallas[0]);
    assert!(cerca(uv.min.x, 0.0) && cerca(uv.min.y, 0.0));
    assert!(cerca(uv.max.x, 1.0) && cerca(uv.max.y, 1.0));
    let rect = f.info.image_rect.expect("hay imagen");
    assert!(cerca(pos.min.x, rect.min.x) && cerca(pos.min.y, rect.min.y));
    assert!(cerca(pos.max.x, rect.max.x) && cerca(pos.max.y, rect.max.y));
}

#[test]
fn la_textura_se_sube_una_sola_vez() {
    let mut banco = Banco::nuevo();
    banco.canvas.set_image(&imagen_de_prueba());
    let mut primero = banco.fotograma(vec![], None);
    let texture = banco.canvas.texture_id().expect("la textura está subida");
    assert!(primero.output.textures_delta.set.contains_key(&texture));
    Banco::limpiar(&mut primero);
    let mut segundo = banco.fotograma(vec![], None);
    assert!(
        !segundo.output.textures_delta.set.contains_key(&texture),
        "el segundo fotograma no vuelve a subir la imagen"
    );
    Banco::limpiar(&mut segundo);
}

#[test]
fn el_primer_fotograma_ajusta_la_imagen_al_lienzo() {
    let mut banco = Banco::nuevo();
    banco.canvas.set_image(&imagen_de_prueba());
    let mut f = banco.fotograma(vec![], None);
    Banco::limpiar(&mut f);
    // 800×600 con 64×48: el mayor zoom entero que cabe es 12 (768×576).
    assert_eq!(Some(f.info.zoom), Zoom::from_int(12));
    let rect = f.info.image_rect.expect("hay imagen");
    assert!(cerca(rect.width(), 768.0) && cerca(rect.height(), 576.0));
    assert!(cerca(rect.center().x, 400.0) && cerca(rect.center().y, 300.0));
}

#[test]
fn la_rueda_cambia_el_zoom_en_uno_y_no_mueve_el_pixel_bajo_el_cursor() {
    let mut banco = Banco::nuevo();
    banco.canvas.set_image(&imagen_de_prueba());
    banco.fotograma(vec![], None).output.textures_delta.clear();

    let cursor = pos2(300.0, 200.0);
    let mut quieto = banco.fotograma(vec![Event::PointerMoved(cursor)], None);
    Banco::limpiar(&mut quieto);
    let pixel = quieto.info.hover.expect("el cursor está sobre la imagen");

    let mut f = banco.fotograma(vec![rueda(true)], None);
    Banco::limpiar(&mut f);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(13));
    assert_eq!(
        f.info.hover,
        Some(pixel),
        "el píxel bajo el cursor se conserva"
    );

    let mut f = banco.fotograma(vec![rueda(false), rueda(false)], None);
    Banco::limpiar(&mut f);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(11), "un paso por evento");
    assert_eq!(f.info.hover, Some(pixel));
}

#[test]
fn el_zoom_se_detiene_en_1_y_en_64() {
    let mut banco = Banco::nuevo();
    banco.canvas.set_image(&imagen_de_prueba());
    banco.fotograma(vec![], None).output.textures_delta.clear();
    let mut f = banco.fotograma(vec![Event::PointerMoved(pos2(400.0, 300.0))], None);
    Banco::limpiar(&mut f);

    let mut f = banco.fotograma((0..200).map(|_| rueda(true)).collect(), None);
    Banco::limpiar(&mut f);
    assert_eq!(f.info.zoom, Zoom::MAX);
    let mut f = banco.fotograma((0..200).map(|_| rueda(false)).collect(), None);
    Banco::limpiar(&mut f);
    assert_eq!(f.info.zoom, Zoom::MIN);
}

#[test]
fn home_vuelve_al_ajuste() {
    let mut banco = Banco::nuevo();
    banco.canvas.set_image(&imagen_de_prueba());
    banco.fotograma(vec![], None).output.textures_delta.clear();
    let mut f = banco.fotograma(
        vec![
            Event::PointerMoved(pos2(100.0, 100.0)),
            rueda(true),
            rueda(true),
        ],
        None,
    );
    Banco::limpiar(&mut f);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(14));
    let mut f = banco.fotograma(vec![tecla(Key::Home)], None);
    Banco::limpiar(&mut f);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(12));
    let rect = f.info.image_rect.expect("hay imagen");
    assert!(cerca(rect.center().x, 400.0) && cerca(rect.center().y, 300.0));
}

#[test]
fn mas_y_menos_cambian_el_zoom() {
    let mut banco = Banco::nuevo();
    banco.canvas.set_image(&imagen_de_prueba());
    banco.fotograma(vec![], None).output.textures_delta.clear();
    let mut f = banco.fotograma(vec![tecla(Key::Minus)], None);
    Banco::limpiar(&mut f);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(11));
    let mut f = banco.fotograma(vec![tecla(Key::Plus), tecla(Key::Plus)], None);
    Banco::limpiar(&mut f);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(13));
}

#[test]
fn el_arrastre_con_el_boton_central_desplaza_la_imagen() {
    let mut banco = Banco::nuevo();
    banco.canvas.set_image(&imagen_de_prueba());
    let mut f = banco.fotograma(vec![], None);
    Banco::limpiar(&mut f);
    let antes = f.info.image_rect.expect("hay imagen");

    let inicio = pos2(400.0, 300.0);
    let pulsar = |pressed| Event::PointerButton {
        pos: inicio,
        button: PointerButton::Middle,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let mut f = banco.fotograma(vec![Event::PointerMoved(inicio)], None);
    Banco::limpiar(&mut f);
    let mut f = banco.fotograma(vec![pulsar(true)], None);
    Banco::limpiar(&mut f);
    for paso in 1..=4 {
        let pos = inicio + vec2(10.0, 5.0) * paso as f32;
        let mut f = banco.fotograma(vec![Event::PointerMoved(pos)], None);
        Banco::limpiar(&mut f);
    }
    let mut f = banco.fotograma(vec![pulsar(false)], None);
    Banco::limpiar(&mut f);
    let despues = f.info.image_rect.expect("hay imagen");
    let (dx, dy) = (despues.min.x - antes.min.x, despues.min.y - antes.min.y);
    assert!(dx > 0.0 && dx <= 40.0 + 1e-3, "dx = {dx}");
    assert!(dy > 0.0 && dy <= 20.0 + 1e-3, "dy = {dy}");
    assert!(cerca(despues.width(), antes.width()), "el zoom no cambia");
}

#[test]
fn una_imagen_mayor_que_la_textura_maxima_da_error_y_no_se_pinta() {
    let mut banco = Banco::nuevo();
    // 1024 es el mínimo que admite el atlas de fuentes de egui; la imagen lo supera en ancho.
    let ancha = Image::new(1025, 2, Mode::Rgba).expect("imagen válida");
    banco.canvas.set_image(&ancha);
    let mut f = banco.fotograma(vec![], Some(1024));
    assert!(
        banco.canvas.texture_id().is_none(),
        "no se sube ninguna textura"
    );
    Banco::limpiar(&mut f);
    assert_eq!(
        f.info.error,
        Some(CanvasError::TooLarge {
            width: 1025,
            height: 2,
            max: 1024
        })
    );
    assert!(f.info.image_rect.is_none());
    assert!(f.info.hover.is_none());
    let _ = f.info.error.as_ref().map(ToString::to_string);

    // Un fotograma posterior sigue sin pintarla ni fallar.
    let mut f = banco.fotograma(vec![rueda(true), tecla(Key::Home)], Some(1024));
    Banco::limpiar(&mut f);
    assert!(f.info.error.is_some());
}

#[test]
fn con_escala_de_pantalla_los_bloques_miden_zoom_pixeles_fisicos() {
    for ppp in [1.0_f32, 1.5, 2.0] {
        let mut banco = Banco::nuevo();
        banco.ctx.set_pixels_per_point(ppp);
        banco.canvas.set_image(&imagen_de_prueba());
        let mut f = banco.fotograma(vec![], None);
        Banco::limpiar(&mut f);
        let zoom = f.info.zoom;
        let rect = f.info.image_rect.expect("hay imagen");
        let fisico = (rect.width() * ppp, rect.height() * ppp);
        let factor = zoom.scale(1) as f32;
        assert!(
            cerca(fisico.0, 64.0 * factor) && cerca(fisico.1, 48.0 * factor),
            "ppp {ppp}: {fisico:?} con zoom {zoom}"
        );
        // La esquina de la imagen cae en un píxel físico entero.
        let esquina = (rect.min.x * ppp, rect.min.y * ppp);
        assert!(
            cerca(esquina.0, esquina.0.round()) && cerca(esquina.1, esquina.1.round()),
            "ppp {ppp}: esquina {esquina:?}"
        );
    }
}

#[test]
fn sin_imagen_se_pinta_el_fondo_sin_fallar() {
    let mut banco = Banco::nuevo();
    let mut f = banco.fotograma(vec![rueda(true), tecla(Key::Home)], None);
    Banco::limpiar(&mut f);
    assert!(f.info.image_rect.is_none());
    assert!(f.info.error.is_none());
}
