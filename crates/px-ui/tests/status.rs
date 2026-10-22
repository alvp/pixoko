//! Tests de la barra de estado (specs/hito-2-texto.md, «Barra de estado»; filas 11, 13 y 16 de
//! «Casos límite»): su altura exacta en píxeles físicos, el lienzo que se queda con el resto de
//! la ventana, y el texto de la barra (`bar_text` y `columns`).
//!
//! El banco es el de `tests/canvas.rs`, copiado y ampliado (ese fichero no se toca): un
//! `egui::Context` ejecuta fotogramas con `RawInput` sintético, sin GPU ni pantalla. En cada
//! fotograma, como hará la app, la barra se monta **antes** del lienzo en el mismo `Ui` raíz:
//! `status::show`, luego `Canvas::interact_ordered` y `Canvas::paint`. El rectángulo del lienzo
//! es lo que queda libre justo antes de `interact_ordered`
//! (`Ui::available_rect_before_wrap`), que es el que el lienzo usa para el ajuste de vista y
//! para convertir pantalla en imagen.
//!
//! Los cambios de fuentes, de `ppp` y de la política de glifo ausente se aplican al empezar el
//! pase siguiente: el banco hace dos pases vacíos de calentamiento antes del primer fotograma.

use egui::{
    Context, Event, Key, MissingGlyphPolicy, Modifiers, MouseWheelUnit, PointerButton, Pos2,
    RawInput, Rect, TouchPhase, Vec2, pos2, vec2,
};
use px_core::{DocumentError, Image, Mode, Pixel, Rgba};
use px_ui::canvas::{Canvas, CanvasInfo, CanvasInput};
use px_ui::edit_layout::min_window_width_pt;
use px_ui::status::{bar_height_px, bar_text, columns, show};
use px_ui::text::{apply_scale, install_fonts, ui_unit};
use px_ui::view::Zoom;

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
    screen: Vec2,
}

struct TestFrame {
    /// Lo que quedaba libre justo antes del lienzo: pantalla menos la barra.
    rect_canvas: Rect,
    entries: Vec<CanvasInput>,
    info: CanvasInfo,
}

impl Bench {
    /// Contexto de `screen` puntos a `ppp`, con la fuente instalada y calentado.
    fn new_2(ppp: f32, screen: Vec2) -> Bench {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
        install_fonts(&ctx);
        let bench = Bench {
            ctx,
            canvas: Canvas::new(),
            screen,
        };
        for _ in 0..2 {
            let output_2 = bench.ctx.run_ui(bench.input_2(vec![]), |ui| {
                apply_scale(ui.ctx());
            });
            output_2.drop_without_applying_deltas();
        }
        bench
    }

    fn input_2(&self, events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.screen)),
            events,
            ..RawInput::default()
        }
    }

    fn ppp(&self) -> f32 {
        self.ctx.pixels_per_point()
    }

    /// Ejecuta un fotograma con los eventos dados: aplica la escala, monta la barra con `text_2`
    /// y después el lienzo, como la app. Limpia los cambios de textura (egui entra en pánico si
    /// se sueltan sin aplicar).
    fn frame(&mut self, text_2: &str, notice: bool, events: Vec<Event>) -> TestFrame {
        let input = self.input_2(events);
        let mut result = None;
        let canvas = &mut self.canvas;
        let mut output_2 = self.ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            show(ui, text_2, notice);
            let rect_canvas = ui.available_rect_before_wrap();
            let entries = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            let info = canvas.paint(ui);
            result = Some(TestFrame {
                rect_canvas,
                entries,
                info,
            });
        });
        output_2.textures_delta.clear();
        result.expect("el lienzo se dibuja")
    }

    /// Alto de la barra en píxeles físicos: lo que la pantalla pierde en el lienzo.
    fn bar_height_px(&self, f: &TestFrame) -> f32 {
        (self.screen.y - f.rect_canvas.height()) * self.ppp()
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

fn button_2(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// Tolerancia en píxeles físicos para el alto de la barra y su esquina. La spec dice ±0,01, pero
/// egui redondea los rectángulos de los paneles a 1/32 de punto (a 1,5× la barra de 40 px, que son
/// 26,667 pt, queda en 26,656 pt, es decir 39,984 px): el error máximo por esa cuantización es de
/// 1/64 pt, o sea 0,0156 px a 1,5× y 0,031 px a 3×. Se pide menos de 0,05 px, que es lo que no
/// se ve, y además se comprueba el alto exacto del lienzo en píxeles enteros.
const TOLERANCE_PX: f32 = 0.05;

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn zoom(n: u32) -> Zoom {
    Zoom::from_int(n).expect("el zoom existe")
}

// ---------------------------------------------------------------------------------------------
// Fila 11: altura de la barra
// ---------------------------------------------------------------------------------------------

#[test]
fn row_11_bar_height_px_is_20_times_n() {
    assert_eq!(bar_height_px(1), 20);
    assert_eq!(bar_height_px(2), 40);
    assert_eq!(bar_height_px(3), 60);
    assert_eq!(bar_height_px(8), 160);
}

#[test]
fn row_11_the_bar_measures_20_n_physical_pixels_and_the_canvas_is_the_rest() {
    // (ppp, alto de la barra en píxeles físicos): n = 1, 2, 2.
    for (ppp, bar) in [(1.0_f32, 20.0_f32), (1.5, 40.0), (2.0, 40.0)] {
        let screen = vec2(400.0, 300.0);
        let mut bench = Bench::new_2(ppp, screen);
        bench.canvas.set_image(&image_of_test());
        let short = bench.frame("lápiz · color 1 #ff0000", false, vec![]);
        let height_2 = bench.bar_height_px(&short);
        assert!(
            near(height_2, bar, TOLERANCE_PX),
            "ppp {ppp}: barra de {height_2} px"
        );

        // El lienzo es la pantalla menos la barra, pegada abajo.
        let rect = short.rect_canvas;
        assert!(near(rect.min.x, 0.0, 1e-3) && near(rect.min.y, 0.0, 1e-3));
        assert!(near(rect.max.x, screen.x, 1e-3), "ppp {ppp}: {rect:?}");
        // La esquina superior de la barra (la inferior del lienzo) cae en píxel físico entero.
        let corner = rect.max.y * ppp;
        assert!(
            near(corner, corner.round(), TOLERANCE_PX),
            "ppp {ppp}: la esquina de la barra cae en el píxel {corner}"
        );
        assert!(near(corner, screen.y * ppp - bar, TOLERANCE_PX));
        // Y el lienzo, en píxeles físicos enteros (así lo lee `Canvas`), es la pantalla menos la
        // barra, exactamente.
        assert_eq!(
            (rect.height() * ppp).round(),
            screen.y * ppp - bar,
            "ppp {ppp}: alto del lienzo en píxeles físicos"
        );

        // Con textos larguísimos (avisos de 400 caracteres, y con saltos de línea) el alto no
        // cambia.
        let length_2 = "abcdefghij".repeat(40);
        for (text_2, notice) in [
            (length_2.as_str(), true),
            (length_2.as_str(), false),
            ("primera línea\nsegunda línea\r\ntercera", true),
            ("日本語 😀 \u{202E}", true),
            ("", false),
        ] {
            let f = bench.frame(text_2, notice, vec![]);
            assert_eq!(
                f.rect_canvas, rect,
                "ppp {ppp}: el texto {text_2:?} cambia el lienzo"
            );
            assert!(near(bench.bar_height_px(&f), bar, TOLERANCE_PX));
        }
    }
}

#[test]
fn row_11_long_text_gives_40_characters_and_ends_in_an_ellipsis() {
    let bench = Bench::new_2(1.0, vec2(400.0, 300.0));
    let length_2 = "abcdefghij".repeat(40);
    assert_eq!(length_2.chars().count(), 400);
    let text_2 = bar_text(&bench.ctx, &length_2, false, 40);
    assert_eq!(text_2.chars().count(), 40);
    assert!(text_2.ends_with('…'), "{text_2:?}");
    assert!(text_2.starts_with("abcdefghij"));
    // También con aviso: el prefijo cuenta dentro de las 40 columnas.
    let notice = bar_text(&bench.ctx, &length_2, true, 40);
    assert_eq!(notice.chars().count(), 40);
    assert!(notice.starts_with("aviso: abc"), "{notice:?}");
    assert!(notice.ends_with('…'));
}

// ---------------------------------------------------------------------------------------------
// Fila 13: el lienzo desplazado por la barra
// ---------------------------------------------------------------------------------------------

/// Un caso de la fila 13, en píxeles físicos.
struct CaseBar {
    ppp: f32,
    bar: f32,
    /// Lienzo: ancho y alto.
    canvas_2: (f32, f32),
    zoom: u32,
    /// Esquina superior izquierda de la imagen y su tamaño, relativas al lienzo.
    image_min: (f32, f32),
    image_size: (f32, f32),
}

#[test]
fn row_13_home_fits_the_image_to_the_canvas_and_not_to_the_window() {
    let cases = [
        CaseBar {
            ppp: 1.0,
            bar: 20.0,
            canvas_2: (400.0, 280.0),
            zoom: 5,
            image_min: (40.0, 20.0),
            image_size: (320.0, 240.0),
        },
        CaseBar {
            ppp: 2.0,
            bar: 40.0,
            canvas_2: (800.0, 560.0),
            zoom: 11,
            image_min: (48.0, 16.0),
            image_size: (704.0, 528.0),
        },
    ];
    for case in cases {
        let ppp = case.ppp;
        let mut bench = Bench::new_2(ppp, vec2(400.0, 300.0));
        bench.canvas.set_image(&image_of_test());
        let text_2 = "64×48 · lápiz";

        // Primer fotograma: ajuste inicial.
        let f = bench.frame(text_2, false, vec![]);
        let rect = f.rect_canvas;
        assert!(near(bench.bar_height_px(&f), case.bar, 0.01), "ppp {ppp}");
        assert!(
            near(rect.width() * ppp, case.canvas_2.0, 0.01)
                && near(rect.height() * ppp, case.canvas_2.1, 0.01),
            "ppp {ppp}: lienzo {rect:?}"
        );
        checks_fit(&f, &case);

        // Se cambia el zoom con la rueda (con el puntero sobre el lienzo) y `Home` lo devuelve.
        let center = rect.center();
        bench.frame(text_2, false, vec![Event::PointerMoved(center)]);
        let f = bench.frame(text_2, false, vec![wheel(true)]);
        assert_eq!(
            Some(f.info.zoom),
            Zoom::from_int(case.zoom + 1),
            "ppp {ppp}: la rueda sube un paso"
        );
        let f = bench.frame(text_2, false, vec![key_2(Key::Home)]);
        checks_fit(&f, &case);
    }
}

/// El zoom de ajuste es el del lienzo (sin la barra saldría uno más) y la imagen cae donde dice
/// la spec, en píxeles físicos desde la esquina del lienzo.
fn checks_fit(f: &TestFrame, case: &CaseBar) {
    let ppp = case.ppp;
    assert_eq!(f.info.zoom, zoom(case.zoom), "ppp {ppp}: zoom de ajuste");
    let image_2 = f.info.image_rect.expect("hay imagen");
    let min = (
        (image_2.min.x - f.rect_canvas.min.x) * ppp,
        (image_2.min.y - f.rect_canvas.min.y) * ppp,
    );
    let size_2 = (image_2.width() * ppp, image_2.height() * ppp);
    assert!(
        near(min.0, case.image_min.0, 0.01) && near(min.1, case.image_min.1, 0.01),
        "ppp {ppp}: la imagen empieza en {min:?} px, se esperaba {:?}",
        case.image_min
    );
    assert!(
        near(size_2.0, case.image_size.0, 0.01) && near(size_2.1, case.image_size.1, 0.01),
        "ppp {ppp}: la imagen mide {size_2:?} px, se esperaba {:?}",
        case.image_size
    );
    // La esquina de la imagen cae en un píxel físico entero.
    let corner = (image_2.min.x * ppp, image_2.min.y * ppp);
    assert!(near(corner.0, corner.0.round(), 0.01) && near(corner.1, corner.1.round(), 0.01));
}

#[test]
fn row_13_the_pointer_over_pixel_2_3_gives_that_pixel_and_over_the_bar_none() {
    // Centro del píxel (2, 3) de la imagen, en píxeles físicos desde la esquina de la ventana.
    let cases = [
        (1.0_f32, 5.0_f32, (40.0_f32, 20.0_f32)),
        (2.0, 11.0, (48.0, 16.0)),
    ];
    for (ppp, z, (img_x, img_y)) in cases {
        let mut bench = Bench::new_2(ppp, vec2(400.0, 300.0));
        bench.canvas.set_image(&image_of_test());
        let text_2 = "64×48 · lápiz";
        let f = bench.frame(text_2, false, vec![]);
        assert_eq!(Some(f.info.zoom), Zoom::from_int(z as u32), "ppp {ppp}");

        let center_px = (img_x + 2.0 * z + z / 2.0, img_y + 3.0 * z + z / 2.0);
        let over_the_pixel = pos2(center_px.0 / ppp, center_px.1 / ppp);
        let f = bench.frame(text_2, false, vec![Event::PointerMoved(over_the_pixel)]);
        assert_eq!(
            f.info.hover,
            Some((2, 3)),
            "ppp {ppp}: el puntero en {over_the_pixel:?} pt"
        );
        assert!(f.entries.is_empty());

        // Sobre la barra (la franja de abajo, de 20 pt a ppp 1 y a ppp 2): ningún píxel.
        let over_the_bar = pos2(200.0, 290.0);
        let f = bench.frame(text_2, false, vec![Event::PointerMoved(over_the_bar)]);
        assert!(
            !f.rect_canvas.contains(over_the_bar),
            "ppp {ppp}: la barra está abajo"
        );
        assert_eq!(f.info.hover, None, "ppp {ppp}: hover sobre la barra");
        assert!(f.entries.is_empty(), "ppp {ppp}: {:?}", f.entries);

        // Pulsar sobre la barra, arrastrar por encima del lienzo y soltar: ningún trazo.
        let f = bench.frame(text_2, false, vec![button_2(over_the_bar, true)]);
        assert!(
            f.entries.is_empty(),
            "ppp {ppp}: pulsar en la barra: {:?}",
            f.entries
        );
        let f = bench.frame(text_2, false, vec![Event::PointerMoved(over_the_pixel)]);
        assert!(
            f.entries.is_empty(),
            "ppp {ppp}: arrastre desde la barra: {:?}",
            f.entries
        );
        let f = bench.frame(text_2, false, vec![button_2(over_the_pixel, false)]);
        assert!(f.entries.is_empty(), "ppp {ppp}: soltar: {:?}", f.entries);
    }
}

#[test]
fn row_13_pressing_over_pixel_2_3_does_start_a_stroke_on_that_pixel() {
    // Contraste con lo anterior: el mismo gesto sobre el lienzo sí da evento de trazo, así que
    // la ausencia de eventos sobre la barra no es un banco que no recoge nada.
    let mut bench = Bench::new_2(1.0, vec2(400.0, 300.0));
    bench.canvas.set_image(&image_of_test());
    let text_2 = "64×48 · lápiz";
    bench.frame(text_2, false, vec![]);
    let over_the_pixel = pos2(52.5, 37.5);
    bench.frame(text_2, false, vec![Event::PointerMoved(over_the_pixel)]);
    let f = bench.frame(text_2, false, vec![button_2(over_the_pixel, true)]);
    assert_eq!(
        f.entries,
        vec![CanvasInput::Stroke(
            px_ui::canvas::CanvasEvent::StrokeBegin((2, 3))
        )]
    );
    let f = bench.frame(text_2, false, vec![button_2(over_the_pixel, false)]);
    assert!(
        f.entries
            .contains(&CanvasInput::Stroke(px_ui::canvas::CanvasEvent::StrokeEnd))
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 16: bar_text y columns
// ---------------------------------------------------------------------------------------------

#[test]
fn row_16_bar_text_prepends_the_prefix_only_to_the_notices() {
    let bench = Bench::new_2(1.0, vec2(400.0, 300.0));
    let ctx = &bench.ctx;
    assert_eq!(
        bar_text(ctx, "se ignora el fichero", true, 100),
        "aviso: se ignora el fichero"
    );
    assert_eq!(
        bar_text(ctx, "se ignora el fichero", false, 100),
        "se ignora el fichero"
    );
}

#[test]
fn row_16_bar_text_substitutes_the_characters_without_a_glyph() {
    let bench = Bench::new_2(1.0, vec2(400.0, 300.0));
    assert_eq!(
        bar_text(&bench.ctx, "no se pudo leer 日本.png", true, 100),
        "aviso: no se pudo leer ??.png"
    );
}

#[test]
fn row_16_bar_text_truncates_to_the_columns_with_the_prefix_inside() {
    let bench = Bench::new_2(1.0, vec2(400.0, 300.0));
    assert_eq!(
        bar_text(&bench.ctx, "se ignora el fichero", true, 10),
        "aviso: se…"
    );
    assert_eq!(bar_text(&bench.ctx, "se ignora el fichero", true, 0), "");
}

#[test]
fn row_16_columns_is_the_width_divided_by_7_n() {
    // 400 / 7 = 57,14; 800 / 14 = 57,14 (n = 2 con 1,5 y con 2,0).
    assert_eq!(columns(400.0, 1.0), 57);
    assert_eq!(columns(800.0, 1.5), 57);
    assert_eq!(columns(800.0, 2.0), 57);
}

#[test]
fn row_16_columns_of_an_invalid_width_is_0() {
    assert_eq!(columns(-1.0, 1.0), 0);
    assert_eq!(columns(f32::NAN, 1.0), 0);
    assert_eq!(columns(f32::INFINITY, 1.0), 0);
}

// ---------------------------------------------------------------------------------------------
// Fila 13 de specs/hito-2-limites-relleno.md: el aviso del relleno demasiado fragmentado cabe
// ---------------------------------------------------------------------------------------------

#[test]
fn row_13_the_fragmented_fill_notice_fits_in_the_bar_at_the_minimum_window() {
    let text_2 = DocumentError::FillTooFragmented { limit: 3_000_000 }.to_string();
    let expected_2 = "aviso: relleno cancelado: región demasiado fragmentada";
    assert_eq!(expected_2.chars().count(), 54);
    for ppp in [1.0f32, 1.25, 1.5, 2.0, 3.0] {
        let width_2 = min_window_width_pt(ppp);
        let bench = Bench::new_2(ppp, vec2(width_2, 300.0));
        // Como `show`: ancho útil = (ancho - 2 * margen) píxeles de interfaz, en físicos.
        let margin = 3.0 * ui_unit(ppp);
        let cols = columns((width_2 - 2.0 * margin) * ppp, ppp);
        assert!(
            cols >= 54,
            "ppp {ppp}: solo caben {cols} columnas (ancho {width_2} pt)"
        );
        assert_eq!(
            bar_text(&bench.ctx, &text_2, true, cols),
            expected_2,
            "ppp {ppp}: el texto se recorta o cambia"
        );
    }
}
