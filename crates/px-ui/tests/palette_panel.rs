//! Tests del panel de paletas dentro de la ventana (specs/hito-2-panel-paletas.md: filas 5 y 12 de
//! «Casos límite», propiedad 3, y el comportamiento de `palette_panel::show`: elegir al pulsar,
//! clics que no eligen y rueda).
//!
//! El banco es el de `tests/status.rs`, copiado y ampliado (ese fichero no se toca): un
//! `egui::Context` ejecuta fotogramas con `RawInput` sintético, sin GPU ni pantalla. En cada
//! fotograma, como hará la app, se monta `status::show`, luego `palette_panel::show` y por fin el
//! lienzo (`Canvas::interact_ordered` y `Canvas::paint`) en el mismo `Ui` raíz. El rectángulo del
//! lienzo es lo que queda libre justo antes de `interact_ordered`
//! (`Ui::available_rect_before_wrap`).
//!
//! Los cambios de fuentes, de `ppp` y de la política de glifo ausente se aplican al empezar el
//! pase siguiente: el banco hace dos pases vacíos de calentamiento antes del primer fotograma.
//!
//! Las posiciones de las muestras salen de las constantes de `palette_grid` (origen de la
//! rejilla `(GRID_X_U, GRID_Y_U)·u`, paso `PITCH_U·u`, con `u = text_scale(ppp)` píxeles
//! físicos); ningún número de la maqueta se repite aquí.

use egui::epaint::Shape;
use egui::{
    Context, Event, FontId, Key, MissingGlyphPolicy, Modifiers, MouseWheelUnit, PointerButton,
    Pos2, RawInput, Rect, TouchPhase, Vec2, pos2, vec2,
};
use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_ui::canvas::{Canvas, CanvasEvent, CanvasInfo, CanvasInput};
use px_ui::color::{
    ColorChoice, cell_count, cell_label, choice_for_cell, info_flags_alpha0, initial_choice,
};
use px_ui::palette_grid::{
    self, CELL_U, COLUMNS, GAP_U, GRID_X_U, GRID_Y_U, PANEL_WIDTH_U, PITCH_U, Scroll,
};
use px_ui::palette_panel;
use px_ui::status::{bar_height_px, columns};
use px_ui::text::{
    INTERFACE_CHARSET, apply_scale, fit_text, font_size_points, install_fonts, text_scale, ui_unit,
    with_glyph_check,
};
use px_ui::theme::SURFACE_1;
use px_ui::view::Zoom;

#[path = "common/zones.rs"]
mod zones;
use zones::panel_on_the_left;

// ---------------------------------------------------------------------------------------------
// Imágenes de prueba
// ---------------------------------------------------------------------------------------------

/// Imagen RGBA de 64×48 con un píxel de cada color en las esquinas.
fn image_rgba() -> Image {
    let mut image = Image::new(64, 48, Mode::Rgba).expect("imagen válida");
    image
        .set_pixel(0, 0, Pixel::Rgba(Rgba::opaque(255, 0, 0)))
        .expect("dentro");
    image
        .set_pixel(63, 47, Pixel::Rgba(Rgba::opaque(0, 0, 255)))
        .expect("dentro");
    image
}

/// Indexada de 64×48 con `count` colores distintos (1 a 256), el último `#ffffff80` si
/// `count == 256` y el índice transparente en el 17 si existe.
fn image_indexed(count: usize) -> Image {
    let colors_2: Vec<Rgba> = (0..count)
        .map(|i| {
            if i == 255 {
                Rgba::new(255, 255, 255, 128)
            } else {
                Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8)
            }
        })
        .collect();
    let mut palette = Palette::from_colors(colors_2).expect("paleta de 1 a 256 colores");
    if count > 17 {
        palette
            .set_transparent_index(Some(17))
            .expect("el índice existe");
    }
    Image::new_indexed(64, 48, palette).expect("imagen indexada")
}

fn image_of(mode: Mode) -> Image {
    match mode {
        Mode::Indexed => image_indexed(256),
        other => Image::new(64, 48, other).expect("imagen válida"),
    }
}

const TEST_MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

// ---------------------------------------------------------------------------------------------
// Banco
// ---------------------------------------------------------------------------------------------

struct Bench {
    ctx: Context,
    canvas: Canvas,
    scroll: Scroll,
    screen: Vec2,
}

struct TestFrame {
    /// Lo que quedaba libre justo antes del lienzo: pantalla menos la barra y el panel.
    rect_canvas: Rect,
    entries: Vec<CanvasInput>,
    info: CanvasInfo,
    /// Lo que devolvió `palette_panel::show`.
    choice_2: Option<ColorChoice>,
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
            scroll: Scroll::default(),
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

    /// Banco con `image` ya puesta en el lienzo.
    fn with_image(ppp: f32, screen: Vec2, image: &Image) -> Bench {
        let mut bench = Bench::new_2(ppp, screen);
        bench.canvas.set_image(image);
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

    fn n(&self) -> u32 {
        text_scale(self.ppp())
    }

    /// Ejecuta un fotograma con los eventos dados: aplica la escala, monta la barra, el panel
    /// (con `choice`, el `scroll` del banco y `enabled`) y después el lienzo, como la app.
    /// Limpia los cambios de textura (egui entra en pánico si se sueltan sin aplicar).
    fn frame(
        &mut self,
        image: &Image,
        choice: ColorChoice,
        enabled: bool,
        events: Vec<Event>,
    ) -> TestFrame {
        let input = self.input_2(events);
        let mut result = None;
        let canvas = &mut self.canvas;
        let scroll = &mut self.scroll;
        let mut output_2 = self.ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            px_ui::status::show(ui, "lápiz · color 1 #000000", false);
            let (choice_2, (rect_canvas, entries, info)) = panel_on_the_left(
                ui,
                |ui| palette_panel::show(ui, image, choice, scroll, enabled),
                |ui| {
                    let rect_canvas = ui.available_rect_before_wrap();
                    let entries = canvas
                        .interact_roles(ui)
                        .into_iter()
                        .map(|(input, _)| input)
                        .collect::<Vec<_>>();
                    let info = canvas.paint(ui);
                    (rect_canvas, entries, info)
                },
            );
            result = Some((rect_canvas, entries, info, choice_2));
        });
        output_2.textures_delta.clear();
        let (rect_canvas, entries, info, choice_2) = result.expect("el lienzo se dibuja");
        TestFrame {
            rect_canvas,
            entries,
            info,
            choice_2,
        }
    }

    /// Fotograma sin eventos, con la selección inicial de la imagen y el panel activo.
    fn rest_2_2(&mut self, image: &Image) -> TestFrame {
        self.frame(image, initial_choice(image), true, vec![])
    }

    /// Mueve el puntero a `pos` (un fotograma) y luego ejecuta `events` en otro.
    fn with_pointer(
        &mut self,
        image: &Image,
        choice: ColorChoice,
        enabled: bool,
        pos: Pos2,
        events: Vec<Event>,
    ) -> TestFrame {
        self.frame(image, choice, enabled, vec![Event::PointerMoved(pos)]);
        self.frame(image, choice, enabled, events)
    }
}

fn wheel_lines(delta_y: f32) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, delta_y),
        phase: TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    }
}

fn wheel_points(delta_y: f32) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: vec2(0.0, delta_y),
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

fn button_2(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn primary(pos: Pos2, pressed: bool) -> Event {
    button_2(pos, PointerButton::Primary, pressed)
}

/// Tolerancia en píxeles físicos: `egui` redondea los rectángulos de los paneles a 1/32 de punto
/// (error máximo de 1/64 pt, o sea 0,023 px a 1,5× y 0,047 px a 3×), como en `status.rs`.
const TOLERANCE_PX: f32 = 0.05;

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// Punto en píxeles físicos de la ventana a puntos.
fn in_points(ppp: f32, x_px: f32, y_px: f32) -> Pos2 {
    pos2(x_px / ppp, y_px / ppp)
}

fn zoom(n: u32) -> Zoom {
    Zoom::from_int(n).expect("el zoom existe")
}

// ---------------------------------------------------------------------------------------------
// Geometría de las muestras, a partir de las constantes de `palette_grid`
// ---------------------------------------------------------------------------------------------

/// Centro de la muestra `index_2` en píxeles físicos de la ventana, con la primera fila visible
/// `first_row`. El panel empieza en (0, 0) de la ventana.
fn swatch_center_px(n: u32, index_2: usize, first_row: usize) -> (f32, f32) {
    let n = i64::from(n);
    let column = (index_2 % COLUMNS) as i64;
    let row_2 = (index_2 / COLUMNS) as i64 - first_row as i64;
    let x = (GRID_X_U + column * PITCH_U) * n + CELL_U * n / 2;
    let y = (GRID_Y_U + row_2 * PITCH_U) * n + CELL_U * n / 2;
    (x as f32, y as f32)
}

/// Una elección distinta de la de la muestra `index_2`, para que devolverla sea un cambio.
fn other_choice(image: &Image, index_2: usize) -> ColorChoice {
    let total = cell_count(image);
    choice_for_cell(image, (index_2 + 1) % total).expect("la muestra existe")
}

/// Pulsa (y suelta) el botón `button` en `(x_px, y_px)` y devuelve lo que `show` devolvió al
/// pulsar y al soltar.
fn click(
    bench: &mut Bench,
    image: &Image,
    choice: ColorChoice,
    enabled: bool,
    button: PointerButton,
    (x_px, y_px): (f32, f32),
) -> (Option<ColorChoice>, Option<ColorChoice>) {
    let pos = in_points(bench.ppp(), x_px, y_px);
    let pressed_2 = bench
        .with_pointer(
            image,
            choice,
            enabled,
            pos,
            vec![button_2(pos, button, true)],
        )
        .choice_2;
    let released = bench
        .frame(image, choice, enabled, vec![button_2(pos, button, false)])
        .choice_2;
    (pressed_2, released)
}

// ---------------------------------------------------------------------------------------------
// Fila 5: el lienzo desplazado por el panel
// ---------------------------------------------------------------------------------------------

/// Un caso de la fila 5, en píxeles físicos (salvo la pantalla, en puntos).
struct CasePanel {
    ppp: f32,
    /// Ancho del panel.
    panel: f32,
    /// Lienzo: ancho y alto.
    canvas_2: (f32, f32),
    zoom: u32,
    /// Esquina superior izquierda de la imagen y su tamaño, desde la esquina de la ventana.
    image_min: (f32, f32),
    image_size: (f32, f32),
    /// Centro del píxel (2, 3).
    pointer: (f32, f32),
}

const SCREEN_LARGE: Vec2 = vec2(800.0, 600.0);

fn cases_row_5() -> [CasePanel; 3] {
    [
        CasePanel {
            ppp: 1.0,
            panel: 233.0,
            canvas_2: (567.0, 580.0),
            zoom: 8,
            image_min: (260.0, 98.0),
            image_size: (512.0, 384.0),
            pointer: (280.0, 126.0),
        },
        CasePanel {
            ppp: 2.0,
            panel: 466.0,
            canvas_2: (1134.0, 1160.0),
            zoom: 17,
            image_min: (489.0, 172.0),
            image_size: (1088.0, 816.0),
            pointer: (531.5, 231.5),
        },
        CasePanel {
            ppp: 1.5,
            panel: 466.0,
            canvas_2: (734.0, 860.0),
            zoom: 11,
            image_min: (481.0, 166.0),
            image_size: (704.0, 528.0),
            pointer: (508.5, 204.5),
        },
    ]
}

/// El zoom de ajuste es el del lienzo recortado por el panel y la barra, y la imagen cae donde
/// dice la spec.
fn checks_fit(f: &TestFrame, case: &CasePanel) {
    let ppp = case.ppp;
    assert_eq!(f.info.zoom, zoom(case.zoom), "ppp {ppp}: zoom de ajuste");
    let image_2 = f.info.image_rect.expect("hay imagen");
    let min = (image_2.min.x * ppp, image_2.min.y * ppp);
    let size_2 = (image_2.width() * ppp, image_2.height() * ppp);
    assert!(
        near(min.0, case.image_min.0, TOLERANCE_PX) && near(min.1, case.image_min.1, TOLERANCE_PX),
        "ppp {ppp}: la imagen empieza en {min:?} px, se esperaba {:?}",
        case.image_min
    );
    assert!(
        near(size_2.0, case.image_size.0, TOLERANCE_PX)
            && near(size_2.1, case.image_size.1, TOLERANCE_PX),
        "ppp {ppp}: la imagen mide {size_2:?} px, se esperaba {:?}",
        case.image_size
    );
}

#[test]
fn row_05_the_panel_takes_its_width_from_the_canvas_and_home_fits_to_the_rest() {
    let image = image_rgba();
    for case in cases_row_5() {
        let ppp = case.ppp;
        let mut bench = Bench::with_image(ppp, SCREEN_LARGE, &image);
        let f = bench.rest_2_2(&image);

        // El rectángulo del lienzo: pantalla menos el panel (a la izquierda) y la barra (abajo).
        let rect = f.rect_canvas;
        assert!(
            near(rect.min.x * ppp, case.panel, TOLERANCE_PX),
            "ppp {ppp}: el lienzo empieza en {} px, el panel mide {} px",
            rect.min.x * ppp,
            case.panel
        );
        assert!(near(rect.min.y, 0.0, 1e-3), "ppp {ppp}: {rect:?}");
        assert!(
            near(rect.max.x, SCREEN_LARGE.x, 1e-3),
            "ppp {ppp}: {rect:?}"
        );
        assert!(
            near(rect.width() * ppp, case.canvas_2.0, TOLERANCE_PX)
                && near(rect.height() * ppp, case.canvas_2.1, TOLERANCE_PX),
            "ppp {ppp}: lienzo de {}×{} px, se esperaba {:?}",
            rect.width() * ppp,
            rect.height() * ppp,
            case.canvas_2
        );
        checks_fit(&f, &case);

        // Se cambia el zoom con la rueda (con el puntero sobre el lienzo) y `Home` lo devuelve.
        let center = rect.center();
        let f = bench.with_pointer(
            &image,
            initial_choice(&image),
            true,
            center,
            vec![wheel_lines(1.0)],
        );
        assert_eq!(
            Some(f.info.zoom),
            Zoom::from_int(case.zoom + 1),
            "ppp {ppp}: la rueda sobre el lienzo sube un paso"
        );
        let f = bench.frame(&image, initial_choice(&image), true, vec![key_2(Key::Home)]);
        checks_fit(&f, &case);
    }
}

#[test]
fn row_05_the_pointer_over_pixel_2_3_gives_that_pixel() {
    let image = image_rgba();
    for case in cases_row_5() {
        let ppp = case.ppp;
        let mut bench = Bench::with_image(ppp, SCREEN_LARGE, &image);
        let f = bench.rest_2_2(&image);
        assert_eq!(f.info.zoom, zoom(case.zoom), "ppp {ppp}");

        let over_the_pixel = in_points(ppp, case.pointer.0, case.pointer.1);
        let f = bench.frame(
            &image,
            initial_choice(&image),
            true,
            vec![Event::PointerMoved(over_the_pixel)],
        );
        assert_eq!(
            f.info.hover,
            Some((2, 3)),
            "ppp {ppp}: el puntero en {over_the_pixel:?} pt"
        );
        assert!(f.entries.is_empty());

        // Contraste: pulsar ahí sí empieza un trazo en ese píxel, así que la ausencia de
        // eventos sobre el panel y la barra no es un banco que no recoge nada.
        let f = bench.frame(
            &image,
            initial_choice(&image),
            true,
            vec![primary(over_the_pixel, true)],
        );
        assert_eq!(
            f.entries,
            vec![CanvasInput::Stroke(CanvasEvent::StrokeBegin((2, 3)))],
            "ppp {ppp}"
        );
        let f = bench.frame(
            &image,
            initial_choice(&image),
            true,
            vec![primary(over_the_pixel, false)],
        );
        assert!(
            f.entries
                .contains(&CanvasInput::Stroke(CanvasEvent::StrokeEnd)),
            "ppp {ppp}"
        );
    }
}

#[test]
fn row_05_over_the_panel_or_the_bar_neither_hover_nor_stroke_nor_zoom() {
    let image = image_rgba();
    let choice = initial_choice(&image);
    for case in cases_row_5() {
        let ppp = case.ppp;
        let mut bench = Bench::with_image(ppp, SCREEN_LARGE, &image);
        let base = bench.rest_2_2(&image);
        let over_the_pixel = in_points(ppp, case.pointer.0, case.pointer.1);

        // Sobre el panel (100, 100) pt y sobre la barra: la franja de abajo.
        for (name_2, place) in [
            ("el panel", pos2(100.0, 100.0)),
            ("la barra", pos2(500.0, 595.0)),
        ] {
            assert!(
                !base.rect_canvas.contains(place),
                "ppp {ppp}: {name_2} cae fuera del lienzo"
            );
            let f = bench.frame(&image, choice, true, vec![Event::PointerMoved(place)]);
            assert_eq!(f.info.hover, None, "ppp {ppp}: hover sobre {name_2}");
            assert!(f.entries.is_empty(), "ppp {ppp}: {:?}", f.entries);

            // Pulsar, arrastrar por encima del lienzo y soltar: ningún trazo.
            let f = bench.frame(&image, choice, true, vec![primary(place, true)]);
            assert!(
                f.entries.is_empty(),
                "ppp {ppp}: pulsar en {name_2}: {:?}",
                f.entries
            );
            let f = bench.frame(
                &image,
                choice,
                true,
                vec![Event::PointerMoved(over_the_pixel)],
            );
            assert!(
                f.entries.is_empty(),
                "ppp {ppp}: arrastre desde {name_2}: {:?}",
                f.entries
            );
            let f = bench.frame(&image, choice, true, vec![primary(over_the_pixel, false)]);
            assert!(
                f.entries.is_empty(),
                "ppp {ppp}: soltar tras {name_2}: {:?}",
                f.entries
            );

            // La rueda sobre el panel o la barra no cambia el zoom del lienzo.
            let antes = bench.rest_2_2(&image).info.zoom;
            let f = bench.with_pointer(&image, choice, true, place, vec![wheel_lines(1.0)]);
            assert_eq!(
                f.info.zoom, antes,
                "ppp {ppp}: la rueda sobre {name_2} cambia el zoom"
            );
            let f = bench.frame(&image, choice, true, vec![wheel_lines(-1.0)]);
            assert_eq!(f.info.zoom, antes, "ppp {ppp}: rueda hacia abajo");
        }
    }
}

#[test]
fn row_05_with_200_pt_of_width_the_canvas_has_width_0_and_there_is_no_panic() {
    let image = image_rgba();
    let choice = initial_choice(&image);
    for ppp in [1.0_f32, 1.5, 2.0] {
        let mut bench = Bench::with_image(ppp, vec2(200.0, 600.0), &image);
        let f = bench.rest_2_2(&image);
        assert!(
            (f.rect_canvas.width() * ppp).round() <= 0.0,
            "ppp {ppp}: el lienzo mide {} px de ancho",
            f.rect_canvas.width() * ppp
        );
        let zoom_antes = f.info.zoom;
        assert_eq!(f.info.hover, None, "ppp {ppp}");

        // `Home` sin efecto, y el puntero en varios sitios (sobre el panel recortado y sobre
        // la barra) no da `hover` ni trazos.
        let f = bench.frame(&image, choice, true, vec![key_2(Key::Home)]);
        assert_eq!(f.info.zoom, zoom_antes, "ppp {ppp}: Home sin efecto");
        for place in [pos2(150.0, 300.0), pos2(20.0, 10.0), pos2(100.0, 595.0)] {
            let f = bench.frame(&image, choice, true, vec![Event::PointerMoved(place)]);
            assert_eq!(f.info.hover, None, "ppp {ppp}: hover en {place:?}");
            let f = bench.frame(&image, choice, true, vec![primary(place, true)]);
            assert!(f.entries.is_empty(), "ppp {ppp}: {:?}", f.entries);
            let f = bench.frame(&image, choice, true, vec![primary(place, false)]);
            assert!(f.entries.is_empty(), "ppp {ppp}: {:?}", f.entries);
        }
        // Y con una imagen indexada de 256 colores (rejilla grande) y la rueda sobre el panel.
        let large = image_indexed(256);
        bench.canvas.set_image(&large);
        let choice = initial_choice(&large);
        bench.frame(
            &large,
            choice,
            true,
            vec![Event::PointerMoved(pos2(100.0, 100.0))],
        );
        bench.frame(&large, choice, true, vec![wheel_lines(-1.0)]);
        bench.frame(&large, choice, true, vec![wheel_lines(1.0)]);
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12: los textos caben y se maquetan con `MissingGlyphPolicy::Panic`
// ---------------------------------------------------------------------------------------------

const PPP_ROW_12: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

/// Todos los textos del panel: `cell_label` de todas las muestras de cada modo e `info_flags`
/// en las ocho combinaciones.
fn texts_of_the_panel() -> Vec<String> {
    let mut texts = Vec::new();
    for mode in TEST_MODES {
        let image = image_of(mode);
        for cell in 0..cell_count(&image) {
            texts.push(cell_label(&image, cell).expect("la muestra existe"));
        }
    }
    // Paletas con alfa y sin él, de varios tamaños.
    for count in [1, 3, 17, 18, 255] {
        let image = image_indexed(count);
        for cell in 0..cell_count(&image) {
            texts.push(cell_label(&image, cell).expect("la muestra existe"));
        }
    }
    for bits in 0..8 {
        texts.push(info_flags_alpha0(
            bits & 1 != 0,
            bits & 2 != 0,
            false,
            bits & 4 != 0,
            false,
        ));
    }
    texts
}

#[test]
fn row_12_the_longest_texts_measure_20_and_30_characters() {
    let image = image_indexed(256);
    assert_eq!(
        cell_label(&image, 255).as_deref(),
        Some("índice 255 #ffffff80")
    );
    assert_eq!(
        cell_label(&image, 255).unwrap().chars().count(),
        20,
        "el más largo de cell_label"
    );
    assert_eq!(
        info_flags_alpha0(true, true, false, true, false),
        "cursor · activa · transparente"
    );
    assert_eq!(
        info_flags_alpha0(true, true, false, true, false)
            .chars()
            .count(),
        30
    );
    let more_long_2 = texts_of_the_panel()
        .iter()
        .map(|t| t.chars().count())
        .max()
        .unwrap();
    assert_eq!(more_long_2, 30, "ningún texto del panel supera los 30");
}

#[test]
fn row_12_columns_of_the_225_useful_u_is_32_with_any_n() {
    for ppp in PPP_ROW_12 {
        let n = text_scale(ppp);
        assert_eq!(
            columns(palette_grid::TEXT_WIDTH_U as f32 * n as f32, ppp),
            32,
            "ppp {ppp} (n = {n})"
        );
        assert_eq!(columns(225.0 * n as f32, ppp), 32, "ppp {ppp}");
    }
}

#[test]
fn row_12_each_text_is_laid_out_without_panic_and_fits_without_clipping() {
    let texts = texts_of_the_panel();
    for ppp in PPP_ROW_12 {
        let bench = Bench::new_2(ppp, SCREEN_LARGE);
        let n = text_scale(ppp);
        let maximum_2 = columns(palette_grid::TEXT_WIDTH_U as f32 * n as f32, ppp);
        assert_eq!(maximum_2, 32);
        let width_useful_px = (palette_grid::TEXT_WIDTH_U * i64::from(n)) as f32;

        // Todo sale de `INTERFACE_CHARSET`.
        for text_2 in &texts {
            for c in text_2.chars() {
                assert!(
                    INTERFACE_CHARSET.contains(c),
                    "ppp {ppp}: {text_2:?} lleva {c:?}, que no está en INTERFACE_CHARSET"
                );
            }
            assert!(
                text_2.chars().count() <= maximum_2,
                "ppp {ppp}: {text_2:?} no cabe en {maximum_2} columnas"
            );
            // `fit_text` no recorta nada.
            let clipped = with_glyph_check(&bench.ctx, |has| fit_text(text_2, maximum_2, has));
            assert_eq!(&clipped, text_2, "ppp {ppp}");
        }

        // Maquetado con la política `Panic`, y el ancho cabe en los 225 u útiles.
        let mut widths = Vec::new();
        let output_2 = bench.ctx.run_ui(bench.input_2(vec![]), |ui| {
            apply_scale(ui.ctx());
            let source = FontId::proportional(font_size_points(ui.ctx().pixels_per_point()));
            ui.ctx().fonts_mut(|f| {
                for text_2 in &texts {
                    let gallery =
                        f.layout_no_wrap(text_2.clone(), source.clone(), egui::Color32::WHITE);
                    widths.push((text_2.clone(), gallery.size().x * ppp));
                }
            });
        });
        output_2.drop_without_applying_deltas();
        assert_eq!(widths.len(), texts.len());
        for (text_2, width_2) in widths {
            assert!(
                width_2 <= width_useful_px + 0.5,
                "ppp {ppp}: {text_2:?} mide {width_2} px y caben {width_useful_px}"
            );
        }
    }
}

#[test]
fn row_12_the_panel_paints_each_mode_with_the_cursor_on_the_last_swatch_without_panic() {
    for ppp in PPP_ROW_12 {
        for mode in TEST_MODES {
            let image = image_of(mode);
            let mut bench = Bench::with_image(ppp, SCREEN_LARGE, &image);
            let n = bench.n();
            let last = cell_count(&image) - 1;
            let (x, y) = swatch_center_px(n, last, 0);
            let pos = in_points(ppp, x, y);
            let choice = initial_choice(&image);
            bench.with_pointer(&image, choice, true, pos, vec![]);
            bench.frame(&image, choice, true, vec![Event::PointerMoved(pos)]);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3: nitidez
// ---------------------------------------------------------------------------------------------

/// Los `ppp` que se prueban siempre.
const PPP_ALWAYS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

/// Todos los múltiplos de 0,05 entre 1 y 4.
fn ppp_multiples_of_0_05() -> impl Iterator<Item = f32> {
    (20..=80).map(|i| i as f32 / 20.0)
}

/// Cada borde de la maqueta, en puntos por `ppp`, es un entero de píxeles físicos (±0,001) y
/// múltiplo de `n`.
fn checks_sharpness(ppp: f32) {
    let n = text_scale(ppp);
    let u = ui_unit(ppp);
    let edges = palette_grid::edges_u();
    assert!(!edges.is_empty(), "edges_u no devuelve ningún borde");
    for k in edges {
        let physical = k as f64 * f64::from(u) * f64::from(ppp);
        assert!(
            (physical - physical.round()).abs() <= 0.001,
            "ppp {ppp}: el borde {k} u cae en {physical} px físicos"
        );
        assert_eq!(
            (physical.round() as i64) % i64::from(n),
            0,
            "ppp {ppp}: el borde {k} u ({physical} px) no es múltiplo de n = {n}"
        );
    }
}

#[test]
fn property_3_the_borders_fall_on_whole_physical_pixels_at_the_ppp_of_the_spec() {
    for ppp in PPP_ALWAYS {
        checks_sharpness(ppp);
    }
}

#[test]
fn property_3_the_borders_fall_on_whole_physical_pixels_at_all_the_multiples_of_0_05() {
    for ppp in ppp_multiples_of_0_05() {
        checks_sharpness(ppp);
    }
}

#[test]
fn property_3_the_edges_include_the_borders_of_the_panel_and_the_grid() {
    let edges = palette_grid::edges_u();
    for k in [
        0,
        PANEL_WIDTH_U,
        GRID_X_U,
        GRID_Y_U,
        palette_grid::SEPARATOR_Y_U,
        palette_grid::TRACK_X_U,
        palette_grid::RIGHT_LINE_X_U,
        palette_grid::LINE1_Y_U,
        palette_grid::LINE2_Y_U,
        GRID_X_U + CELL_U,
        GRID_Y_U + CELL_U,
        GRID_X_U + PITCH_U,
    ] {
        assert!(edges.contains(&k), "falta el borde {k} u en edges_u()");
    }
}

#[test]
fn property_3_the_real_panel_measures_233_n_physical_pixels_and_ends_at_the_right_edge() {
    let image = image_rgba();
    let width_u = PANEL_WIDTH_U as f32;
    for ppp in PPP_ALWAYS {
        let mut bench = Bench::with_image(ppp, SCREEN_LARGE, &image);
        let n = bench.n() as f32;
        // El panel real, sin ranura: directamente en el `Ui` raíz, tras la barra de estado.
        let mut rect_canvas = None;
        let mut shapes_2 = Vec::new();
        for _ in 0..2 {
            let mut output_2 = bench.ctx.run_ui(bench.input_2(vec![]), |ui| {
                apply_scale(ui.ctx());
                px_ui::status::show(ui, "lápiz · color 1 #000000", false);
                palette_panel::show(ui, &image, initial_choice(&image), &mut bench.scroll, true);
                rect_canvas = Some(ui.available_rect_before_wrap());
            });
            output_2.textures_delta.clear();
            shapes_2 = std::mem::take(&mut output_2.shapes);
        }
        let rect_canvas = rect_canvas.expect("el lienzo se reparte");

        // Lo que el panel quita al lienzo: mide `233·n` px y termina en el borde derecho.
        assert!(
            near(
                (SCREEN_LARGE.x - rect_canvas.max.x) * ppp,
                width_u * n,
                TOLERANCE_PX
            ),
            "ppp {ppp}: el panel mide {} px, se esperaba {}",
            (SCREEN_LARGE.x - rect_canvas.max.x) * ppp,
            width_u * n
        );
        assert!(near(rect_canvas.min.x, 0.0, 1e-3), "ppp {ppp}");
        assert!(near(rect_canvas.min.y, 0.0, 1e-3), "ppp {ppp}");

        // Y el fondo pintado (`SURFACE_1`) del panel termina en el borde derecho con ese ancho.
        let mut rects = Vec::new();
        for clipped in &shapes_2 {
            collects_rectangles(&clipped.shape, &mut rects);
        }
        let of_the_panel: Vec<Rect> = rects
            .iter()
            .filter(|(_, fill)| *fill == SURFACE_1)
            .map(|(rect, _)| *rect)
            .filter(|rect| {
                near(rect.max.x, SCREEN_LARGE.x, TOLERANCE_PX / ppp)
                    && near(rect.min.y * ppp, 0.0, TOLERANCE_PX)
            })
            .collect();
        assert!(
            !of_the_panel.is_empty(),
            "ppp {ppp}: ningún rectángulo SURFACE_1 termina en el borde derecho"
        );
        assert!(
            of_the_panel
                .iter()
                .any(|rect| near(rect.width() * ppp, width_u * n, TOLERANCE_PX)),
            "ppp {ppp}: ningún fondo del panel mide {} px: {of_the_panel:?}",
            width_u * n
        );
    }
}

/// Rectángulos rellenos de una forma, con su color de relleno (aplana `Shape::Vec`).
fn collects_rectangles(shape: &Shape, output_2: &mut Vec<(Rect, egui::Color32)>) {
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

// ---------------------------------------------------------------------------------------------
// `show`: elegir al pulsar
// ---------------------------------------------------------------------------------------------

#[test]
fn show_pressing_the_center_of_a_swatch_chooses_that_swatch_in_an_indexed_of_256() {
    let image = image_indexed(256);
    for ppp in PPP_ALWAYS {
        let mut bench = Bench::with_image(ppp, SCREEN_LARGE, &image);
        let n = bench.n();
        bench.rest_2_2(&image);
        for cell in [0usize, 1, 8, 15, 16, 17, 200, 254, 255] {
            let expected_2 = choice_for_cell(&image, cell).expect("la muestra existe");
            let actual = other_choice(&image, cell);
            let (pressed_2, released) = click(
                &mut bench,
                &image,
                actual,
                true,
                PointerButton::Primary,
                swatch_center_px(n, cell, 0),
            );
            assert_eq!(
                pressed_2,
                Some(expected_2),
                "ppp {ppp}: pulsar la muestra {cell}"
            );
            assert_eq!(expected_2, ColorChoice::Entry(cell as u8));
            assert_eq!(
                released, None,
                "ppp {ppp}: soltar no elige (muestra {cell})"
            );
        }
    }
}

#[test]
fn show_pressing_a_swatch_chooses_slot_in_rgba_gray_and_gray_alpha() {
    for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        let image = image_of(mode);
        let mut bench = Bench::with_image(1.0, SCREEN_LARGE, &image);
        bench.rest_2_2(&image);
        for cell in 0..cell_count(&image) {
            let (pressed_2, _) = click(
                &mut bench,
                &image,
                other_choice(&image, cell),
                true,
                PointerButton::Primary,
                swatch_center_px(1, cell, 0),
            );
            assert_eq!(
                pressed_2,
                Some(ColorChoice::Slot(cell as u8 + 1)),
                "{mode:?}: muestra {cell}"
            );
            assert_eq!(pressed_2, choice_for_cell(&image, cell));
        }
    }
}

#[test]
fn show_with_scrolled_rows_the_pressed_swatch_is_the_one_that_is_seen() {
    // Pantalla baja: pocas filas visibles de las 16.
    let image = image_indexed(256);
    for (ppp, height_2) in [(1.0_f32, 200.0_f32), (2.0, 150.0)] {
        let mut bench = Bench::with_image(ppp, vec2(800.0, height_2), &image);
        let n = bench.n();
        let panel_h_px = (height_2 * ppp) as i64 - i64::from(bar_height_px(n));
        let visible_2 = palette_grid::visible_rows(panel_h_px, n);
        let total = palette_grid::total_rows(256);
        assert!(
            visible_2 >= 3 && visible_2 < total,
            "ppp {ppp}: {visible_2} filas"
        );

        let first = 2usize;
        bench.scroll = Scroll {
            first_row: first,
            rest: 0,
        };
        bench.rest_2_2(&image);
        assert_eq!(bench.scroll.first_row, first, "ppp {ppp}: reposo no mueve");
        for (row_visible, column) in [(0usize, 0usize), (1, 5), (visible_2 - 1, 15)] {
            let cell = (first + row_visible) * COLUMNS + column;
            let (pressed_2, _) = click(
                &mut bench,
                &image,
                other_choice(&image, cell),
                true,
                PointerButton::Primary,
                swatch_center_px(n, cell, first),
            );
            assert_eq!(
                pressed_2,
                Some(ColorChoice::Entry(cell as u8)),
                "ppp {ppp}: muestra {cell} con la fila {first} arriba"
            );
        }
        assert_eq!(bench.scroll.first_row, first, "elegir no desplaza");
    }
}

// ---------------------------------------------------------------------------------------------
// `show`: clics que no eligen (fila 7)
// ---------------------------------------------------------------------------------------------

#[test]
fn show_clicks_in_gaps_and_outside_the_grid_do_not_choose() {
    let image = image_indexed(256);
    for ppp in PPP_ALWAYS {
        let mut bench = Bench::with_image(ppp, SCREEN_LARGE, &image);
        let n = i64::from(bench.n());
        let choice = ColorChoice::Entry(5);
        bench.rest_2_2(&image);

        let (cx, cy) = swatch_center_px(n as u32, 0, 0);
        let half_gap = GAP_U * n / 2;
        let positions = [
            // Hueco entre la columna 0 y la 1, a la altura de la fila 0.
            (
                "hueco horizontal",
                ((GRID_X_U + CELL_U) * n + half_gap) as f32,
                cy,
            ),
            // Hueco entre la fila 0 y la 1, a la altura de la columna 0.
            (
                "hueco vertical",
                cx,
                ((GRID_Y_U + CELL_U) * n + half_gap) as f32,
            ),
            // Esquina de huecos.
            (
                "esquina de huecos",
                ((GRID_X_U + CELL_U) * n + half_gap) as f32,
                ((GRID_Y_U + CELL_U) * n + half_gap) as f32,
            ),
            // Margen izquierdo, antes de la rejilla.
            ("margen izquierdo", (GRID_X_U * n / 2) as f32, cy),
            // Sobre la rejilla, en el separador y en las líneas de texto.
            (
                "separador",
                cx,
                (palette_grid::SEPARATOR_Y_U * n) as f32 + 0.5,
            ),
            ("texto", cx, (palette_grid::LINE1_Y_U * n + n) as f32),
            // A la derecha de la última columna, dentro del panel.
            (
                "derecha de la rejilla",
                ((GRID_X_U + COLUMNS as i64 * PITCH_U + 1) * n) as f32,
                cy,
            ),
            // Debajo de la última fila (16 filas ocupan todo el alto que sigue).
            (
                "debajo de la última fila",
                cx,
                ((GRID_Y_U + 16 * PITCH_U + 2) * n) as f32,
            ),
            // En el hueco tras la última fila y la última columna.
            (
                "hueco de la última muestra",
                ((GRID_X_U + 15 * PITCH_U + CELL_U) * n + half_gap) as f32,
                ((GRID_Y_U + 15 * PITCH_U + CELL_U) * n + half_gap) as f32,
            ),
        ];
        for (name_2, x, y) in positions {
            let (pressed_2, released) = click(
                &mut bench,
                &image,
                choice,
                true,
                PointerButton::Primary,
                (x, y),
            );
            assert_eq!(
                pressed_2, None,
                "ppp {ppp}: pulsar en {name_2} ({x}, {y}) px"
            );
            assert_eq!(released, None, "ppp {ppp}: soltar en {name_2}");
        }
    }
}

#[test]
fn show_a_click_on_a_swatch_that_does_not_exist_does_not_choose() {
    // RGBA: solo las 9 primeras; la columna 9 de la fila 0 está vacía. Y una indexada de 3.
    for image in [image_rgba(), image_indexed(3)] {
        let count = cell_count(&image);
        let mut bench = Bench::with_image(1.0, SCREEN_LARGE, &image);
        bench.rest_2_2(&image);
        let choice = initial_choice(&image);
        for cell in [count, count + 1, 15, 16, 255] {
            let (pressed_2, _) = click(
                &mut bench,
                &image,
                choice,
                true,
                PointerButton::Primary,
                swatch_center_px(1, cell, 0),
            );
            assert_eq!(pressed_2, None, "{count} muestras: la {cell} no existe");
        }
    }
}

#[test]
fn show_a_row_that_is_not_seen_is_not_chosen() {
    // Pantalla de 146 pt a ppp 1: el panel mide 126 px y solo se ven 3 filas (la banda del
    // indicador, de I.2a, ocupa 36 u más que antes).
    let image = image_indexed(256);
    let mut bench = Bench::with_image(1.0, vec2(800.0, 146.0), &image);
    let n = bench.n();
    let panel_h_px = 146 - i64::from(bar_height_px(n));
    assert_eq!(palette_grid::visible_rows(panel_h_px, n), 3);
    let choice = ColorChoice::Entry(5);
    bench.rest_2_2(&image);

    // La fila 3 (la cuarta) empieza dentro del panel (a 119 px) pero no se ve.
    let (x, y) = swatch_center_px(n, 3 * COLUMNS, 0);
    assert!(y < panel_h_px as f32, "la fila oculta cae dentro del panel");
    let (pressed_2, _) = click(
        &mut bench,
        &image,
        choice,
        true,
        PointerButton::Primary,
        (x, y),
    );
    assert_eq!(pressed_2, None, "la fila 3 no se ve");
    // La fila 2 sí.
    let (pressed_2, _) = click(
        &mut bench,
        &image,
        choice,
        true,
        PointerButton::Primary,
        swatch_center_px(n, 2 * COLUMNS + 4, 0),
    );
    assert_eq!(pressed_2, Some(ColorChoice::Entry(36)));
}

#[test]
fn show_the_right_and_middle_buttons_do_not_choose() {
    let image = image_indexed(256);
    let mut bench = Bench::with_image(1.0, SCREEN_LARGE, &image);
    bench.rest_2_2(&image);
    for button in [PointerButton::Secondary, PointerButton::Middle] {
        let (pressed_2, released) = click(
            &mut bench,
            &image,
            ColorChoice::Entry(5),
            true,
            button,
            swatch_center_px(1, 200, 0),
        );
        assert_eq!(pressed_2, None, "{button:?}");
        assert_eq!(released, None, "{button:?}");
    }
    // Contraste: el botón izquierdo en el mismo sitio sí elige.
    let (pressed_2, _) = click(
        &mut bench,
        &image,
        ColorChoice::Entry(5),
        true,
        PointerButton::Primary,
        swatch_center_px(1, 200, 0),
    );
    assert_eq!(pressed_2, Some(ColorChoice::Entry(200)));
}

#[test]
fn show_with_a_gesture_in_progress_enabled_false_does_not_choose() {
    for mode in TEST_MODES {
        let image = image_of(mode);
        let mut bench = Bench::with_image(1.0, SCREEN_LARGE, &image);
        bench.rest_2_2(&image);
        let cell = cell_count(&image) - 1;
        let choice = other_choice(&image, cell);
        let (pressed_2, released) = click(
            &mut bench,
            &image,
            choice,
            false,
            PointerButton::Primary,
            swatch_center_px(1, cell, 0),
        );
        assert_eq!(pressed_2, None, "{mode:?}: enabled falso");
        assert_eq!(released, None, "{mode:?}: enabled falso");
        // Contraste con `enabled` verdadero.
        let (pressed_2, _) = click(
            &mut bench,
            &image,
            choice,
            true,
            PointerButton::Primary,
            swatch_center_px(1, cell, 0),
        );
        assert_eq!(pressed_2, choice_for_cell(&image, cell), "{mode:?}");
    }
}

#[test]
fn show_without_events_chooses_nothing() {
    let image = image_indexed(256);
    let mut bench = Bench::with_image(1.0, SCREEN_LARGE, &image);
    for _ in 0..3 {
        assert_eq!(bench.rest_2_2(&image).choice_2, None);
    }
    // Un puntero que solo pasa por encima de una muestra tampoco elige.
    let (x, y) = swatch_center_px(1, 200, 0);
    let f = bench.frame(
        &image,
        ColorChoice::Entry(0),
        true,
        vec![Event::PointerMoved(in_points(1.0, x, y))],
    );
    assert_eq!(f.choice_2, None);
}

// ---------------------------------------------------------------------------------------------
// `show`: la rueda
// ---------------------------------------------------------------------------------------------

/// Casos `(ppp, height_2 de screen en pt)` con muchas filas y poca altura.
const SCREENS_LOW: [(f32, f32); 2] = [(1.0, 200.0), (2.0, 150.0)];

fn rows_visible(bench: &Bench) -> (usize, usize) {
    let n = bench.n();
    let panel_h_px = (bench.screen.y * bench.ppp()) as i64 - i64::from(bar_height_px(n));
    (
        palette_grid::visible_rows(panel_h_px, n),
        palette_grid::total_rows(256),
    )
}

#[test]
fn show_the_wheel_over_the_panel_scrolls_the_rows_and_does_not_zoom() {
    let image = indexed_with_canvas();
    let choice = initial_choice(&image);
    for (ppp, height_2) in SCREENS_LOW {
        let mut bench = Bench::with_image(ppp, vec2(800.0, height_2), &image);
        let (visible_2, total) = rows_visible(&bench);
        assert!(
            visible_2 >= 1 && visible_2 < total,
            "ppp {ppp}: {visible_2}"
        );
        let maximum = palette_grid::max_first_row(total, visible_2);
        let zoom_antes = bench.rest_2_2(&image).info.zoom;
        assert_eq!(bench.scroll, Scroll::default());

        let over_the_panel = pos2(100.0, 60.0);
        // Una línea de rueda hacia abajo son 3 filas.
        let f = bench.with_pointer(
            &image,
            choice,
            true,
            over_the_panel,
            vec![wheel_lines(-1.0)],
        );
        assert_eq!(
            bench.scroll.first_row,
            3.min(maximum),
            "ppp {ppp}: una línea de rueda hacia abajo"
        );
        assert_eq!(f.info.zoom, zoom_antes, "ppp {ppp}: la rueda no hace zoom");
        assert_eq!(f.choice_2, None, "ppp {ppp}: la rueda no elige");

        // Hacia arriba vuelve a la fila 0 (y no pasa de ahí).
        bench.frame(&image, choice, true, vec![wheel_lines(1.0)]);
        assert_eq!(bench.scroll.first_row, 0, "ppp {ppp}: hacia arriba");
        let f = bench.frame(&image, choice, true, vec![wheel_lines(1.0)]);
        assert_eq!(bench.scroll.first_row, 0, "ppp {ppp}: no baja de 0");
        assert_eq!(f.info.zoom, zoom_antes);

        // Muchas líneas hacia abajo: se queda en la última fila posible.
        for _ in 0..10 {
            bench.frame(&image, choice, true, vec![wheel_lines(-1.0)]);
        }
        assert_eq!(bench.scroll.first_row, maximum, "ppp {ppp}: tope");
        let f = bench.rest_2_2(&image);
        assert_eq!(f.info.zoom, zoom_antes);
    }
}

#[test]
fn show_the_wheel_in_points_is_physical_pixels() {
    // Con n = 2 (ppp 2), una fila son 28 px físicos = 14 pt: `delta · ppp`.
    let image = indexed_with_canvas();
    let choice = initial_choice(&image);
    let mut bench = Bench::with_image(2.0, vec2(800.0, 150.0), &image);
    let n = bench.n();
    let row_pt = (palette_grid::PITCH_U * i64::from(n)) as f32 / bench.ppp();
    bench.rest_2_2(&image);
    let over_the_panel = pos2(100.0, 60.0);
    bench.with_pointer(
        &image,
        choice,
        true,
        over_the_panel,
        vec![wheel_points(-row_pt)],
    );
    assert_eq!(bench.scroll.first_row, 1, "una fila exacta");
    // Media fila no mueve las filas: se acumula (la fila no se corta).
    bench.frame(&image, choice, true, vec![wheel_points(-row_pt / 2.0)]);
    assert_eq!(bench.scroll.first_row, 1, "media fila se acumula");
    bench.frame(&image, choice, true, vec![wheel_points(-row_pt / 2.0)]);
    assert_eq!(bench.scroll.first_row, 2, "las dos mitades suman una fila");
    // Hacia arriba, de vuelta.
    bench.frame(&image, choice, true, vec![wheel_points(row_pt * 2.0)]);
    assert_eq!(bench.scroll.first_row, 0);
}

#[test]
fn show_the_wheel_over_the_canvas_zooms_and_does_not_scroll_the_panel() {
    let image = indexed_with_canvas();
    let choice = initial_choice(&image);
    for (ppp, height_2) in SCREENS_LOW {
        let mut bench = Bench::with_image(ppp, vec2(800.0, height_2), &image);
        let base = bench.rest_2_2(&image);
        let zoom_antes = base.info.zoom;
        let over_the_canvas = pos2((base.rect_canvas.min.x + 800.0) / 2.0, 50.0);
        assert!(base.rect_canvas.contains(over_the_canvas));

        let f = bench.with_pointer(
            &image,
            choice,
            true,
            over_the_canvas,
            vec![wheel_lines(1.0)],
        );
        assert_eq!(
            f.info.zoom,
            zoom_antes.zoom_in(),
            "ppp {ppp}: la rueda sobre el lienzo hace zoom"
        );
        assert_eq!(
            bench.scroll,
            Scroll::default(),
            "ppp {ppp}: la rueda sobre el lienzo no desplaza el panel"
        );
    }
}

#[test]
fn show_with_enabled_false_the_panel_keeps_scrolling() {
    let image = indexed_with_canvas();
    let choice = initial_choice(&image);
    let mut bench = Bench::with_image(1.0, vec2(800.0, 200.0), &image);
    bench.rest_2_2(&image);
    bench.with_pointer(
        &image,
        choice,
        false,
        pos2(100.0, 60.0),
        vec![wheel_lines(-1.0)],
    );
    assert!(bench.scroll.first_row > 0, "desplaza aunque no elija");
}

/// Indexada de 256 con la imagen ya lista para el lienzo.
fn indexed_with_canvas() -> Image {
    image_indexed(256)
}

#[test]
fn show_the_bench_uses_the_same_panel_width_as_the_constants() {
    // Sanidad del banco: a ppp 1 el panel son PANEL_WIDTH_U puntos y la muestra 0 queda donde
    // dicen las constantes.
    let image = image_rgba();
    let mut bench = Bench::with_image(1.0, SCREEN_LARGE, &image);
    let f = bench.rest_2_2(&image);
    assert!(near(f.rect_canvas.min.x, PANEL_WIDTH_U as f32, 0.05));
    assert_eq!(
        swatch_center_px(1, 0, 0),
        (
            (GRID_X_U + CELL_U / 2) as f32,
            (GRID_Y_U + CELL_U / 2) as f32
        )
    );
}
