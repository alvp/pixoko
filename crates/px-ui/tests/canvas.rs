//! Tests del lienzo de egui sin ventana (specs/hito-2-ventana.md, «Lienzo» y «Cómo se prueba»,
//! punto 3): un `egui::Context` ejecuta fotogramas con `RawInput` sintético, sin GPU ni pantalla.

use egui::epaint::Primitive;
use egui::{
    Color32, Context, Event, FullOutput, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2,
    RawInput, Rect, TextureFilter, TextureId, TexturesDelta, TouchPhase, Vec2, pos2, vec2,
};
use px_core::{Image, Mode, Pixel, Rgba};
use px_ui::canvas::{Canvas, CanvasError, CanvasInfo, CanvasInput};
use px_ui::view::Zoom;

mod common;
use common::textures::{Textures, texel_de};

const SCREEN: Vec2 = vec2(800.0, 600.0);

/// Imagen RGBA de 64×48 con un píxel de cada color en las esquinas.
fn image_of_test() -> Image {
    let mut image = Image::new(64, 48, Mode::Rgba).expect("imagen válida");
    image
        .set_pixel(0, 0, Pixel::Rgba(Rgba::opaque(255, 0, 0)))
        .expect("dentro");
    image
        .set_pixel(63, 47, Pixel::Rgba(Rgba::opaque(0, 0, 255)))
        .expect("dentro");
    image
}

struct Bench {
    ctx: Context,
    canvas: Canvas,
    /// El contenido de las texturas, para las pruebas que lo miran (`applies`).
    textures_2: Textures,
}

struct TestFrame {
    output: FullOutput,
    info: CanvasInfo,
}

impl Bench {
    fn new_2() -> Bench {
        Bench {
            ctx: Context::default(),
            canvas: Canvas::new(),
            textures_2: Textures::new(),
        }
    }

    /// Aplica a `textures` los cambios de textura de un fotograma (antes de `clean`), sin los de
    /// la fuente de egui. Las pruebas que miran el contenido de las baldosas lo llaman con
    /// **cada** fotograma, en orden.
    fn applies(&mut self, frame: &TestFrame) {
        let mut delta: TexturesDelta = frame.output.textures_delta.clone();
        delta.set.remove(&TextureId::Managed(0));
        delta.free.remove(&TextureId::Managed(0));
        self.textures_2.applies(&delta);
        // La copia no se entrega a la GPU: egui exige vaciarla antes de soltarla.
        delta.clear();
    }

    /// Ejecuta un fotograma con los eventos dados y devuelve su salida y la información del
    /// lienzo. Limpia los cambios de textura (egui entra en pánico si se sueltan sin aplicar).
    fn frame(&mut self, events: Vec<Event>, max_texture_side: Option<usize>) -> TestFrame {
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
        TestFrame {
            output,
            info: info.expect("el lienzo se dibuja"),
        }
    }

    fn clean(frame: &mut TestFrame) {
        frame.output.textures_delta.clear();
    }
}

fn wheel(top: bool) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, if top { 1.0 } else { -1.0 }),
        phase: TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    }
}

fn key_2(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// Mallas del fotograma con la textura dada, con sus vértices.
fn meshes_with_texture(
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

fn limits(mesh: &egui::epaint::Mesh) -> (Rect, Rect) {
    let mut pos = Rect::NOTHING;
    let mut uv = Rect::NOTHING;
    for v in &mesh.vertices {
        pos.extend_with(v.pos);
        uv.extend_with(v.uv);
    }
    (pos, uv)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn the_image_is_painted_with_an_unsmoothed_texture_and_a_single_mesh() {
    let mut bench = Bench::new_2();
    bench.canvas.set_image(&image_of_test());
    let mut f = bench.frame(vec![], None);
    let texture = bench.canvas.texture_id().expect("la textura está subida");
    let deltas = f
        .output
        .textures_delta
        .set
        .get(&texture)
        .expect("se sube la textura de la imagen");
    let options = deltas.iter().next().expect("un delta").options;
    assert_eq!(options.magnification, TextureFilter::Nearest);
    assert_eq!(options.minification, TextureFilter::Nearest);

    let meshes = meshes_with_texture(&bench.ctx, &mut f.output, texture);
    Bench::clean(&mut f);
    assert_eq!(
        meshes.len(),
        1,
        "una sola malla con la textura de la imagen"
    );
    let (pos, uv) = limits(&meshes[0]);
    assert!(near(uv.min.x, 0.0) && near(uv.min.y, 0.0));
    assert!(near(uv.max.x, 1.0) && near(uv.max.y, 1.0));
    let rect = f.info.image_rect.expect("hay imagen");
    assert!(near(pos.min.x, rect.min.x) && near(pos.min.y, rect.min.y));
    assert!(near(pos.max.x, rect.max.x) && near(pos.max.y, rect.max.y));
}

#[test]
fn the_texture_is_uploaded_only_once() {
    let mut bench = Bench::new_2();
    bench.canvas.set_image(&image_of_test());
    let mut first = bench.frame(vec![], None);
    let texture = bench.canvas.texture_id().expect("la textura está subida");
    assert!(first.output.textures_delta.set.contains_key(&texture));
    Bench::clean(&mut first);
    let mut second = bench.frame(vec![], None);
    assert!(
        !second.output.textures_delta.set.contains_key(&texture),
        "el segundo fotograma no vuelve a subir la imagen"
    );
    Bench::clean(&mut second);
}

#[test]
fn the_first_frame_fits_the_image_to_the_canvas() {
    let mut bench = Bench::new_2();
    bench.canvas.set_image(&image_of_test());
    let mut f = bench.frame(vec![], None);
    Bench::clean(&mut f);
    // 800×600 con 64×48: el mayor zoom entero que cabe es 12 (768×576).
    assert_eq!(Some(f.info.zoom), Zoom::from_int(12));
    let rect = f.info.image_rect.expect("hay imagen");
    assert!(near(rect.width(), 768.0) && near(rect.height(), 576.0));
    assert!(near(rect.center().x, 400.0) && near(rect.center().y, 300.0));
}

#[test]
fn the_wheel_changes_the_zoom_by_one_and_does_not_move_the_pixel_under_the_cursor() {
    let mut bench = Bench::new_2();
    bench.canvas.set_image(&image_of_test());
    bench.frame(vec![], None).output.textures_delta.clear();

    let cursor = pos2(300.0, 200.0);
    let mut still = bench.frame(vec![Event::PointerMoved(cursor)], None);
    Bench::clean(&mut still);
    let pixel = still.info.hover.expect("el cursor está sobre la imagen");

    let mut f = bench.frame(vec![wheel(true)], None);
    Bench::clean(&mut f);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(13));
    assert_eq!(
        f.info.hover,
        Some(pixel),
        "el píxel bajo el cursor se conserva"
    );

    let mut f = bench.frame(vec![wheel(false), wheel(false)], None);
    Bench::clean(&mut f);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(11), "un paso por evento");
    assert_eq!(f.info.hover, Some(pixel));
}

#[test]
fn the_zoom_stops_at_the_minimum_and_at_64() {
    let mut bench = Bench::new_2();
    bench.canvas.set_image(&image_of_test());
    bench.frame(vec![], None).output.textures_delta.clear();
    let mut f = bench.frame(vec![Event::PointerMoved(pos2(400.0, 300.0))], None);
    Bench::clean(&mut f);

    let mut f = bench.frame((0..200).map(|_| wheel(true)).collect(), None);
    Bench::clean(&mut f);
    assert_eq!(f.info.zoom, Zoom::MAX);
    let mut f = bench.frame((0..200).map(|_| wheel(false)).collect(), None);
    Bench::clean(&mut f);
    assert_eq!(f.info.zoom, Zoom::MIN);
}

#[test]
fn home_returns_to_fit() {
    let mut bench = Bench::new_2();
    bench.canvas.set_image(&image_of_test());
    bench.frame(vec![], None).output.textures_delta.clear();
    let mut f = bench.frame(
        vec![
            Event::PointerMoved(pos2(100.0, 100.0)),
            wheel(true),
            wheel(true),
        ],
        None,
    );
    Bench::clean(&mut f);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(14));
    let mut f = bench.frame(vec![key_2(Key::Home)], None);
    Bench::clean(&mut f);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(12));
    let rect = f.info.image_rect.expect("hay imagen");
    assert!(near(rect.center().x, 400.0) && near(rect.center().y, 300.0));
}

#[test]
fn plus_and_minus_change_the_zoom() {
    let mut bench = Bench::new_2();
    bench.canvas.set_image(&image_of_test());
    bench.frame(vec![], None).output.textures_delta.clear();
    let mut f = bench.frame(vec![key_2(Key::Minus)], None);
    Bench::clean(&mut f);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(11));
    let mut f = bench.frame(vec![key_2(Key::Plus), key_2(Key::Plus)], None);
    Bench::clean(&mut f);
    assert_eq!(Some(f.info.zoom), Zoom::from_int(13));
}

#[test]
fn dragging_with_the_middle_button_pans_the_image() {
    let mut bench = Bench::new_2();
    bench.canvas.set_image(&image_of_test());
    let mut f = bench.frame(vec![], None);
    Bench::clean(&mut f);
    let antes = f.info.image_rect.expect("hay imagen");

    let start_2 = pos2(400.0, 300.0);
    let pulsar = |pressed| Event::PointerButton {
        pos: start_2,
        button: PointerButton::Middle,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let mut f = bench.frame(vec![Event::PointerMoved(start_2)], None);
    Bench::clean(&mut f);
    let mut f = bench.frame(vec![pulsar(true)], None);
    Bench::clean(&mut f);
    for step in 1..=4 {
        let pos = start_2 + vec2(10.0, 5.0) * step as f32;
        let mut f = bench.frame(vec![Event::PointerMoved(pos)], None);
        Bench::clean(&mut f);
    }
    let mut f = bench.frame(vec![pulsar(false)], None);
    Bench::clean(&mut f);
    let after_2 = f.info.image_rect.expect("hay imagen");
    let (dx, dy) = (after_2.min.x - antes.min.x, after_2.min.y - antes.min.y);
    assert!(dx > 0.0 && dx <= 40.0 + 1e-3, "dx = {dx}");
    assert!(dy > 0.0 && dy <= 20.0 + 1e-3, "dy = {dy}");
    assert!(near(after_2.width(), antes.width()), "el zoom no cambia");
}

// ---------------------------------------------------------------------------------------------
// Unidad 14a-2: la imagen ya no depende del lado máximo de textura (specs/hito-2-zoom-baldosas.md,
// decisiones 5 y 9). Con una textura máxima de 1024, la imagen de 1025×2 se pinta en 2 baldosas
// (antes: `CanvasError::TooLarge { max: 1024 }`); `TooLarge` solo queda para un lado mayor de
// `Canvas::MAX_SIDE` (16384, `max = 16384`).
// ---------------------------------------------------------------------------------------------

/// Píxeles con color de la imagen ancha, que distinguen las dos baldosas (de 1024 y de 1 columna).
const PIXELS_OF_THE_WIDE_ONE: [((u32, u32), Rgba); 5] = [
    ((0, 0), Rgba::opaque(255, 0, 0)),
    ((1023, 0), Rgba::opaque(200, 100, 50)),
    ((1023, 1), Rgba::opaque(0, 0, 255)),
    ((1024, 0), Rgba::opaque(0, 255, 0)),
    ((1024, 1), Rgba::new(10, 20, 30, 128)),
];

/// Imagen RGBA de 1025×2: un píxel más ancha que una textura de 1024.
fn image_wide() -> Image {
    let mut image = Image::new(1025, 2, Mode::Rgba).expect("imagen válida");
    for ((x, y), color) in PIXELS_OF_THE_WIDE_ONE {
        image.set_pixel(x, y, Pixel::Rgba(color)).expect("dentro");
    }
    image
}

/// El texel que debe tener el píxel `(x, y)` de la imagen ancha en su baldosa.
fn texel_of_the_wide_one(x: u32, y: u32) -> Color32 {
    let color = PIXELS_OF_THE_WIDE_ONE
        .iter()
        .find(|(p, _)| *p == (x, y))
        .map_or(Rgba::new(0, 0, 0, 0), |(_, c)| *c);
    texel_de(color.r, color.g, color.b, color.a)
}

/// Las dos baldosas de la imagen ancha: la primera es `texture_id()`, y la textura compuesta de
/// cada una es la imagen recortada (1024×2 y 1×2), idéntica a la que daría el documento.
fn checks_the_two_tiles(bench: &Bench, label_2: &str) {
    let ids = bench.canvas.texture_ids();
    assert_eq!(ids.len(), 2, "{label_2}: dos baldosas");
    assert_eq!(
        bench.canvas.texture_id(),
        Some(ids[0]),
        "{label_2}: `texture_id()` es la primera baldosa"
    );
    assert_ne!(ids[0], ids[1], "{label_2}");
    for (tile, (size_2, x0)) in [([1024, 2], 0u32), ([1, 2], 1024)].into_iter().enumerate() {
        let texture_2 = bench
            .textures_2
            .get(ids[tile])
            .unwrap_or_else(|| panic!("{label_2}: la baldosa {tile} se subió"));
        assert_eq!(
            texture_2.size_2, size_2,
            "{label_2}: la baldosa {tile} es la imagen recortada"
        );
        for y in 0..2usize {
            for x in 0..size_2[0] {
                assert_eq!(
                    texture_2.texel(x, y),
                    Some(texel_of_the_wide_one(x0 + x as u32, y as u32)),
                    "{label_2}: texel ({x}, {y}) de la baldosa {tile}"
                );
            }
        }
    }
}

/// El píxel de imagen bajo el píxel físico `dx` (desde el borde izquierdo de la imagen) y la
/// primera fila de pantalla de la imagen, con el puntero ahí (a `ppp` 1: un punto = un píxel).
fn hover_en(bench: &mut Bench, rect: Rect, dx: f32) -> Option<(u32, u32)> {
    // +0,25 y no +0,5: `interact` redondea a píxeles físicos y 0,5 sube al siguiente.
    let pos = pos2(rect.min.x + dx + 0.25, rect.min.y + 0.25);
    let mut f = bench.frame(vec![Event::PointerMoved(pos)], Some(1024));
    Bench::clean(&mut f);
    f.info.hover
}

#[test]
fn an_image_wider_than_the_maximum_texture_opens_and_is_painted_in_two_tiles() {
    let mut bench = Bench::new_2();
    // 1024 es el mínimo que admite el atlas de fuentes de egui; la imagen lo supera en ancho.
    bench.canvas.set_image(&image_wide());
    let mut f = bench.frame(vec![], Some(1024));
    bench.applies(&f);
    Bench::clean(&mut f);
    assert_eq!(f.info.error, None, "ya no hay error de textura máxima");
    let rect = f.info.image_rect.expect("la imagen se pinta");
    checks_the_two_tiles(&bench, "tras el primer fotograma");

    // `hover` funciona en las dos mitades. Con la imagen ajustada a 1/2 (800×600 no admite
    // 1025 píxeles a ×1), el píxel de pantalla `s` es el píxel de imagen `2·s`: el 511 está en la
    // primera baldosa (x = 1022) y el 512 en la segunda (x = 1024).
    assert_eq!(f.info.zoom.to_string(), "×1/2");
    assert_eq!(hover_en(&mut bench, rect, 10.0), Some((20, 0)));
    assert_eq!(hover_en(&mut bench, rect, 511.0), Some((1022, 0)));
    assert_eq!(hover_en(&mut bench, rect, 512.0), Some((1024, 0)));

    // Un fotograma posterior sigue pintándola y sin fallar: con rueda y `Home` no hay error, la
    // imagen y las baldosas son las mismas y no se vuelve a subir nada.
    let ids = bench.canvas.texture_ids();
    let mut f = bench.frame(vec![wheel(true), key_2(Key::Home)], Some(1024));
    bench.applies(&f);
    assert!(
        ids.iter()
            .all(|id| !f.output.textures_delta.set.contains_key(id)),
        "las baldosas no se vuelven a subir"
    );
    Bench::clean(&mut f);
    assert_eq!(f.info.error, None);
    assert!(f.info.image_rect.is_some());
    assert_eq!(bench.canvas.texture_ids(), ids);
    // `Home` solo: de vuelta al ajuste y a la misma imagen centrada.
    let mut f = bench.frame(vec![key_2(Key::Home)], Some(1024));
    bench.applies(&f);
    Bench::clean(&mut f);
    assert_eq!(f.info.error, None);
    assert_eq!(f.info.zoom.to_string(), "×1/2");
    assert_eq!(f.info.image_rect, Some(rect));
    checks_the_two_tiles(&bench, "tras la rueda y Home");
}

#[test]
fn an_image_with_a_side_larger_than_16384_gives_an_error_and_is_not_painted() {
    // El error por tamaño sigue existiendo, ahora por `Canvas::MAX_SIDE` (16384) y no por la
    // textura máxima de la GPU: 16385×1 RGBA son ~64 KiB, y da lo mismo con 1024 que con 2048.
    let expected_2 = CanvasError::TooLarge {
        width: 16385,
        height: 1,
        max: 16384,
    };
    let long = || Image::new(16385, 1, Mode::Rgba).expect("imagen válida");
    for max_texture_side in [None, Some(1024)] {
        let mut with_set = Bench::new_2();
        with_set.canvas.set_image(&long());
        let mut with_owned = Bench::new_2();
        with_owned.canvas.set_owned_image(long());
        for bench in [&mut with_set, &mut with_owned] {
            let mut f = bench.frame(vec![], max_texture_side);
            bench.applies(&f);
            Bench::clean(&mut f);
            assert_eq!(
                bench.canvas.texture_id(),
                None,
                "{max_texture_side:?}: no se sube ninguna textura"
            );
            assert!(bench.canvas.texture_ids().is_empty());
            assert_eq!(
                f.info.error,
                Some(expected_2.clone()),
                "{max_texture_side:?}"
            );
            assert!(f.info.image_rect.is_none());
            assert!(f.info.hover.is_none());
            let _ = f.info.error.as_ref().map(ToString::to_string);

            // Un fotograma posterior sigue sin pintarla ni fallar.
            let mut f = bench.frame(vec![wheel(true), key_2(Key::Home)], max_texture_side);
            bench.applies(&f);
            Bench::clean(&mut f);
            assert_eq!(
                f.info.error,
                Some(expected_2.clone()),
                "{max_texture_side:?}"
            );
            assert!(f.info.image_rect.is_none());
            assert_eq!(bench.canvas.texture_id(), None);
        }
        // `set_image` y `set_owned_image` dan lo mismo.
        let mut a = with_set.frame(vec![], max_texture_side);
        let mut b = with_owned.frame(vec![], max_texture_side);
        Bench::clean(&mut a);
        Bench::clean(&mut b);
        assert_eq!(a.info, b.info);
    }
}

#[test]
fn with_screen_scale_the_blocks_measure_zoom_physical_pixels() {
    for ppp in [1.0_f32, 1.5, 2.0] {
        let mut bench = Bench::new_2();
        bench.ctx.set_pixels_per_point(ppp);
        bench.canvas.set_image(&image_of_test());
        let mut f = bench.frame(vec![], None);
        Bench::clean(&mut f);
        let zoom = f.info.zoom;
        let rect = f.info.image_rect.expect("hay imagen");
        let physical = (rect.width() * ppp, rect.height() * ppp);
        let factor = zoom.scale(1) as f32;
        assert!(
            near(physical.0, 64.0 * factor) && near(physical.1, 48.0 * factor),
            "ppp {ppp}: {physical:?} con zoom {zoom}"
        );
        // La esquina de la imagen cae en un píxel físico entero.
        let corner = (rect.min.x * ppp, rect.min.y * ppp);
        assert!(
            near(corner.0, corner.0.round()) && near(corner.1, corner.1.round()),
            "ppp {ppp}: esquina {corner:?}"
        );
    }
}

#[test]
fn without_an_image_the_background_is_painted_without_failing() {
    let mut bench = Bench::new_2();
    let mut f = bench.frame(vec![wheel(true), key_2(Key::Home)], None);
    Bench::clean(&mut f);
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

fn image_different() -> Image {
    let mut image = Image::new(200, 100, Mode::Rgba).expect("imagen válida");
    image
        .set_pixel(199, 99, Pixel::Rgba(Rgba::opaque(0, 255, 0)))
        .expect("dentro");
    image
}

/// Un fotograma que solo llama a `interact` y devuelve sus eventos de trazo.
fn interacts(bench: &mut Bench, events: Vec<Event>) -> Vec<CanvasEvent> {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
        events,
        ..RawInput::default()
    };
    let mut events_2 = Vec::new();
    let mut output_2 = bench.ctx.run_ui(input, |ui| {
        events_2 = bench
            .canvas
            .interact_roles(ui)
            .into_iter()
            .filter_map(|(input, _)| match input {
                CanvasInput::Stroke(event) => Some(event),
                CanvasInput::Escape => None,
            })
            .collect();
    });
    output_2.textures_delta.clear();
    events_2
}

fn presses_the_primary_button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn row_12_set_owned_image_uploads_the_same_texture_and_gives_the_same_information_as_set_image() {
    let mut with_set = Bench::new_2();
    with_set.canvas.set_image(&image_of_test());
    let mut with_owned = Bench::new_2();
    with_owned.canvas.set_owned_image(image_of_test());

    let mut a = with_set.frame(vec![], None);
    let mut b = with_owned.frame(vec![], None);
    assert_eq!(a.info, b.info);

    let ta = with_set.canvas.texture_id().expect("textura de set_image");
    let tb = with_owned
        .canvas
        .texture_id()
        .expect("textura de set_owned_image");
    let gives = a
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
    assert!(gives == db, "la textura subida es byte a byte la misma");
    Bench::clean(&mut a);
    Bench::clean(&mut b);
}

#[test]
fn row_12_set_owned_image_leaves_the_texture_pending_until_the_first_frame() {
    for owned in [false, true] {
        let mut bench = Bench::new_2();
        // Con una imagen ya subida delante: el reemplazo la deja pendiente de nuevo.
        bench.canvas.set_image(&image_of_test());
        let mut f = bench.frame(vec![], None);
        Bench::clean(&mut f);
        let old = bench.canvas.texture_id().expect("subida");

        if owned {
            bench.canvas.set_owned_image(image_different());
        } else {
            bench.canvas.set_image(&image_different());
        }
        assert!(
            bench.canvas.texture_id().is_none(),
            "owned {owned}: la textura vieja ya no está y la nueva espera al fotograma"
        );

        let mut f = bench.frame(vec![], None);
        let new = bench.canvas.texture_id().expect("subida en este fotograma");
        assert_ne!(new, old, "owned {owned}");
        assert!(
            f.output.textures_delta.set.contains_key(&new),
            "owned {owned}: se sube en el primer fotograma"
        );
        Bench::clean(&mut f);
        let mut f = bench.frame(vec![], None);
        assert!(
            !f.output.textures_delta.set.contains_key(&new),
            "owned {owned}: una sola subida"
        );
        Bench::clean(&mut f);
    }
}

#[test]
fn row_12_set_owned_image_returns_the_view_to_fit_and_discards_the_requested_zoom() {
    // La referencia: un lienzo nuevo con la imagen B (vista al ajuste).
    let mut reference = Bench::new_2();
    reference.canvas.set_image(&image_different());
    let mut r = reference.frame(vec![], None);
    Bench::clean(&mut r);

    for owned in [false, true] {
        let mut bench = Bench::new_2();
        bench.canvas.set_image(&image_of_test());
        let mut f = bench.frame(vec![], None);
        Bench::clean(&mut f);
        // Aleja con la rueda: la vista deja de ser la de ajuste.
        let center = pos2(400.0, 300.0);
        let mut f = bench.frame(vec![Event::PointerMoved(center), wheel(false)], None);
        Bench::clean(&mut f);
        let fit_a = {
            let mut fresco = Bench::new_2();
            fresco.canvas.set_image(&image_of_test());
            let mut f = fresco.frame(vec![], None);
            Bench::clean(&mut f);
            f.info.zoom
        };
        assert_ne!(f.info.zoom, fit_a, "owned {owned}: la rueda cambió el zoom");

        // Y un zoom pedido que aún no se aplicó.
        bench.canvas.request_view(ViewOp::ZoomIn);
        if owned {
            bench.canvas.set_owned_image(image_different());
        } else {
            bench.canvas.set_image(&image_different());
        }
        let mut f = bench.frame(vec![], None);
        Bench::clean(&mut f);
        // El puntero sigue sobre el lienzo (hay `hover`): se compara lo que fija la vista.
        assert_eq!(f.info.zoom, r.info.zoom, "owned {owned}: al ajuste");
        assert_eq!(
            f.info.image_rect, r.info.image_rect,
            "owned {owned}: centrada como en un lienzo nuevo"
        );
        assert_eq!(f.info.error, r.info.error, "owned {owned}");
        assert_eq!(
            bench.canvas.zoom(),
            reference.canvas.zoom(),
            "owned {owned}"
        );
    }
}

#[test]
fn row_12_set_owned_image_leaves_the_gesture_at_none() {
    for owned in [false, true] {
        let mut bench = Bench::new_2();
        bench.canvas.set_image(&image_of_test());
        interacts(&mut bench, vec![]);
        // Un trazo en curso: pulsado dentro del lienzo y sin soltar.
        let p = pos2(400.0, 300.0);
        let events_2 = interacts(
            &mut bench,
            vec![Event::PointerMoved(p), presses_the_primary_button(p, true)],
        );
        assert!(
            events_2
                .iter()
                .any(|e| matches!(e, CanvasEvent::StrokeBegin(_))),
            "owned {owned}: el trazo empezó: {events_2:?}"
        );

        if owned {
            bench.canvas.set_owned_image(image_different());
        } else {
            bench.canvas.set_image(&image_different());
        }
        // Con el gesto en nada, mover el puntero (el botón sigue pulsado) no continúa el trazo.
        let events_2 = interacts(&mut bench, vec![Event::PointerMoved(pos2(410.0, 310.0))]);
        assert!(
            events_2.is_empty(),
            "owned {owned}: el gesto debía perderse y salió {events_2:?}"
        );
    }
}

#[test]
fn row_12_set_owned_image_also_paints_the_1025x2_image_in_two_tiles_like_set_image() {
    // Antes: los dos daban `TooLarge { max: 1024 }` con una textura máxima de 1024. Ahora la
    // imagen se abre y se pinta en 2 baldosas, y `set_image` y `set_owned_image` dan lo mismo.
    let mut with_set = Bench::new_2();
    with_set.canvas.set_image(&image_wide());
    let mut with_owned = Bench::new_2();
    with_owned.canvas.set_owned_image(image_wide());
    let mut a = with_set.frame(vec![], Some(1024));
    let mut b = with_owned.frame(vec![], Some(1024));
    with_set.applies(&a);
    with_owned.applies(&b);
    Bench::clean(&mut a);
    Bench::clean(&mut b);
    assert_eq!(a.info, b.info);
    assert_eq!(b.info.error, None);
    assert!(b.info.image_rect.is_some());
    checks_the_two_tiles(&with_set, "set_image");
    checks_the_two_tiles(&with_owned, "set_owned_image");
}

#[test]
fn row_12_set_owned_image_on_a_new_canvas_paints_the_image_and_set_image_does_not_consume_its_own()
{
    let mut bench = Bench::new_2();
    bench.canvas.set_owned_image(image_of_test());
    let mut f = bench.frame(vec![], None);
    Bench::clean(&mut f);
    assert!(f.info.image_rect.is_some());
    assert!(f.info.error.is_none());

    // `set_image` sigue tomando una referencia: la imagen del que llama queda intacta y usable.
    let image_2 = image_of_test();
    let mut other_2 = Bench::new_2();
    other_2.canvas.set_image(&image_2);
    other_2.canvas.set_image(&image_2);
    assert_eq!(image_2, image_of_test());
}
