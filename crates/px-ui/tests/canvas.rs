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

// ---------------------------------------------------------------------------------------------
// Unidad 19: `Canvas::set_owned_image` (specs/hito-2-nueva-imagen.md, D4 y fila 12)
// ---------------------------------------------------------------------------------------------
//
// `set_owned_image(Image)` es `set_image` sin clonar: deja el mismo estado (vista al ajuste, textura
// pendiente de subir, gesto en nada, zoom pedido y vista previa descartados) y `set_image` pasa a
// clonar y delegar. Las pruebas de arriba (la 18 y el lienzo) no cambian.
//
// Que `set_owned_image` no clona no se puede observar desde fuera (no hay un contador de copias):
// se prueba que da exactamente lo mismo que `set_image` en todo lo observable (la información del
// fotograma, la textura subida byte a byte, la vista, el gesto y el error de tamaño).

use px_ui::canvas::CanvasEvent;
use px_ui::view::ViewOp;

fn imagen_distinta() -> Image {
    let mut image = Image::new(200, 100, Mode::Rgba).expect("imagen válida");
    image
        .set_pixel(199, 99, Pixel::Rgba(Rgba::opaque(0, 255, 0)))
        .expect("dentro");
    image
}

/// Un fotograma que solo llama a `interact` y devuelve sus eventos de trazo.
fn interactua(banco: &mut Banco, events: Vec<Event>) -> Vec<CanvasEvent> {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
        events,
        ..RawInput::default()
    };
    let mut eventos = Vec::new();
    let mut salida = banco.ctx.run_ui(input, |ui| {
        eventos = banco.canvas.interact(ui);
    });
    salida.textures_delta.clear();
    eventos
}

fn pulsa_el_boton_primario(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn fila_12_set_owned_image_sube_la_misma_textura_y_da_la_misma_informacion_que_set_image() {
    let mut con_set = Banco::nuevo();
    con_set.canvas.set_image(&imagen_de_prueba());
    let mut con_owned = Banco::nuevo();
    con_owned.canvas.set_owned_image(imagen_de_prueba());

    let mut a = con_set.fotograma(vec![], None);
    let mut b = con_owned.fotograma(vec![], None);
    assert_eq!(a.info, b.info);

    let ta = con_set.canvas.texture_id().expect("textura de set_image");
    let tb = con_owned
        .canvas
        .texture_id()
        .expect("textura de set_owned_image");
    let da = a
        .output
        .textures_delta
        .set
        .get(&ta)
        .expect("sube set_image");
    let db = b
        .output
        .textures_delta
        .set
        .get(&tb)
        .expect("sube set_owned_image");
    assert!(da == db, "la textura subida es byte a byte la misma");
    Banco::limpiar(&mut a);
    Banco::limpiar(&mut b);
}

#[test]
fn fila_12_set_owned_image_deja_la_textura_pendiente_hasta_el_primer_fotograma() {
    for owned in [false, true] {
        let mut banco = Banco::nuevo();
        // Con una imagen ya subida delante: el reemplazo la deja pendiente de nuevo.
        banco.canvas.set_image(&imagen_de_prueba());
        let mut f = banco.fotograma(vec![], None);
        Banco::limpiar(&mut f);
        let antigua = banco.canvas.texture_id().expect("subida");

        if owned {
            banco.canvas.set_owned_image(imagen_distinta());
        } else {
            banco.canvas.set_image(&imagen_distinta());
        }
        assert!(
            banco.canvas.texture_id().is_none(),
            "owned {owned}: la textura vieja ya no está y la nueva espera al fotograma"
        );

        let mut f = banco.fotograma(vec![], None);
        let nueva = banco.canvas.texture_id().expect("subida en este fotograma");
        assert_ne!(nueva, antigua, "owned {owned}");
        assert!(
            f.output.textures_delta.set.contains_key(&nueva),
            "owned {owned}: se sube en el primer fotograma"
        );
        Banco::limpiar(&mut f);
        let mut f = banco.fotograma(vec![], None);
        assert!(
            !f.output.textures_delta.set.contains_key(&nueva),
            "owned {owned}: una sola subida"
        );
        Banco::limpiar(&mut f);
    }
}

#[test]
fn fila_12_set_owned_image_devuelve_la_vista_al_ajuste_y_descarta_el_zoom_pedido() {
    // La referencia: un lienzo nuevo con la imagen B (vista al ajuste).
    let mut referencia = Banco::nuevo();
    referencia.canvas.set_image(&imagen_distinta());
    let mut r = referencia.fotograma(vec![], None);
    Banco::limpiar(&mut r);

    for owned in [false, true] {
        let mut banco = Banco::nuevo();
        banco.canvas.set_image(&imagen_de_prueba());
        let mut f = banco.fotograma(vec![], None);
        Banco::limpiar(&mut f);
        // Aleja con la rueda: la vista deja de ser la de ajuste.
        let centro = pos2(400.0, 300.0);
        let mut f = banco.fotograma(vec![Event::PointerMoved(centro), rueda(false)], None);
        Banco::limpiar(&mut f);
        let ajuste_a = {
            let mut fresco = Banco::nuevo();
            fresco.canvas.set_image(&imagen_de_prueba());
            let mut f = fresco.fotograma(vec![], None);
            Banco::limpiar(&mut f);
            f.info.zoom
        };
        assert_ne!(
            f.info.zoom, ajuste_a,
            "owned {owned}: la rueda cambió el zoom"
        );

        // Y un zoom pedido que aún no se aplicó.
        banco.canvas.request_view(ViewOp::ZoomIn);
        if owned {
            banco.canvas.set_owned_image(imagen_distinta());
        } else {
            banco.canvas.set_image(&imagen_distinta());
        }
        let mut f = banco.fotograma(vec![], None);
        Banco::limpiar(&mut f);
        // El puntero sigue sobre el lienzo (hay `hover`): se compara lo que fija la vista.
        assert_eq!(f.info.zoom, r.info.zoom, "owned {owned}: al ajuste");
        assert_eq!(
            f.info.image_rect, r.info.image_rect,
            "owned {owned}: centrada como en un lienzo nuevo"
        );
        assert_eq!(f.info.error, r.info.error, "owned {owned}");
        assert_eq!(
            banco.canvas.zoom(),
            referencia.canvas.zoom(),
            "owned {owned}"
        );
    }
}

#[test]
fn fila_12_set_owned_image_deja_el_gesto_en_nada() {
    for owned in [false, true] {
        let mut banco = Banco::nuevo();
        banco.canvas.set_image(&imagen_de_prueba());
        interactua(&mut banco, vec![]);
        // Un trazo en curso: pulsado dentro del lienzo y sin soltar.
        let p = pos2(400.0, 300.0);
        let eventos = interactua(
            &mut banco,
            vec![Event::PointerMoved(p), pulsa_el_boton_primario(p, true)],
        );
        assert!(
            eventos
                .iter()
                .any(|e| matches!(e, CanvasEvent::StrokeBegin(_))),
            "owned {owned}: el trazo empezó: {eventos:?}"
        );

        if owned {
            banco.canvas.set_owned_image(imagen_distinta());
        } else {
            banco.canvas.set_image(&imagen_distinta());
        }
        // Con el gesto en nada, mover el puntero (el botón sigue pulsado) no continúa el trazo.
        let eventos = interactua(&mut banco, vec![Event::PointerMoved(pos2(410.0, 310.0))]);
        assert!(
            eventos.is_empty(),
            "owned {owned}: el gesto debía perderse y salió {eventos:?}"
        );
    }
}

#[test]
fn fila_12_set_owned_image_tambien_da_el_error_de_textura_maxima_como_set_image() {
    // 1024 es el mínimo que admite el atlas de fuentes de egui; la imagen lo supera en ancho.
    let ancha = || Image::new(1025, 2, Mode::Rgba).expect("imagen válida");
    let mut con_set = Banco::nuevo();
    con_set.canvas.set_image(&ancha());
    let mut con_owned = Banco::nuevo();
    con_owned.canvas.set_owned_image(ancha());
    let mut a = con_set.fotograma(vec![], Some(1024));
    let mut b = con_owned.fotograma(vec![], Some(1024));
    Banco::limpiar(&mut a);
    Banco::limpiar(&mut b);
    assert_eq!(a.info, b.info);
    assert_eq!(
        b.info.error,
        Some(CanvasError::TooLarge {
            width: 1025,
            height: 2,
            max: 1024
        })
    );
    assert!(b.info.image_rect.is_none());
    assert!(
        con_owned.canvas.texture_id().is_none(),
        "no se sube ninguna textura"
    );
}

#[test]
fn fila_12_set_owned_image_en_un_lienzo_nuevo_pinta_la_imagen_y_set_image_no_consume_la_suya() {
    let mut banco = Banco::nuevo();
    banco.canvas.set_owned_image(imagen_de_prueba());
    let mut f = banco.fotograma(vec![], None);
    Banco::limpiar(&mut f);
    assert!(f.info.image_rect.is_some());
    assert!(f.info.error.is_none());

    // `set_image` sigue tomando una referencia: la imagen del que llama queda intacta y usable.
    let imagen = imagen_de_prueba();
    let mut otro = Banco::nuevo();
    otro.canvas.set_image(&imagen);
    otro.canvas.set_image(&imagen);
    assert_eq!(imagen, imagen_de_prueba());
}
