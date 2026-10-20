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

use egui::epaint::{ClippedShape, Shape};
use egui::{
    Context, Event, FontId, Key, MissingGlyphPolicy, Modifiers, MouseWheelUnit, PointerButton,
    Pos2, RawInput, Rect, TouchPhase, Vec2, pos2, vec2,
};
use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_ui::canvas::{Canvas, CanvasEvent, CanvasInfo, CanvasInput};
use px_ui::color::{
    ColorChoice, cell_count, cell_label, choice_for_cell, info_flags, initial_choice,
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

// ---------------------------------------------------------------------------------------------
// Imágenes de prueba
// ---------------------------------------------------------------------------------------------

/// Imagen RGBA de 64×48 con un píxel de cada color en las esquinas.
fn imagen_rgba() -> Image {
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
fn imagen_indexada(count: usize) -> Image {
    let colores: Vec<Rgba> = (0..count)
        .map(|i| {
            if i == 255 {
                Rgba::new(255, 255, 255, 128)
            } else {
                Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8)
            }
        })
        .collect();
    let mut palette = Palette::from_colors(colores).expect("paleta de 1 a 256 colores");
    if count > 17 {
        palette
            .set_transparent_index(Some(17))
            .expect("el índice existe");
    }
    Image::new_indexed(64, 48, palette).expect("imagen indexada")
}

fn imagen_de(mode: Mode) -> Image {
    match mode {
        Mode::Indexed => imagen_indexada(256),
        other => Image::new(64, 48, other).expect("imagen válida"),
    }
}

const MODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

// ---------------------------------------------------------------------------------------------
// Banco
// ---------------------------------------------------------------------------------------------

struct Banco {
    ctx: Context,
    canvas: Canvas,
    scroll: Scroll,
    pantalla: Vec2,
}

struct Fotograma {
    /// Lo que quedaba libre justo antes del lienzo: pantalla menos la barra y el panel.
    rect_lienzo: Rect,
    entradas: Vec<CanvasInput>,
    info: CanvasInfo,
    /// Lo que devolvió `palette_panel::show`.
    eleccion: Option<ColorChoice>,
    /// Lo pintado en el fotograma.
    formas: Vec<ClippedShape>,
}

impl Banco {
    /// Contexto de `pantalla` puntos a `ppp`, con la fuente instalada y calentado.
    fn nuevo(ppp: f32, pantalla: Vec2) -> Banco {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
        install_fonts(&ctx);
        let banco = Banco {
            ctx,
            canvas: Canvas::new(),
            scroll: Scroll::default(),
            pantalla,
        };
        for _ in 0..2 {
            let salida = banco.ctx.run_ui(banco.entrada(vec![]), |ui| {
                apply_scale(ui.ctx());
            });
            salida.drop_without_applying_deltas();
        }
        banco
    }

    /// Banco con `image` ya puesta en el lienzo.
    fn con_imagen(ppp: f32, pantalla: Vec2, image: &Image) -> Banco {
        let mut banco = Banco::nuevo(ppp, pantalla);
        banco.canvas.set_image(image);
        banco
    }

    fn entrada(&self, events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.pantalla)),
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
    fn fotograma(
        &mut self,
        image: &Image,
        choice: ColorChoice,
        enabled: bool,
        events: Vec<Event>,
    ) -> Fotograma {
        let input = self.entrada(events);
        let mut resultado = None;
        let canvas = &mut self.canvas;
        let scroll = &mut self.scroll;
        let mut salida = self.ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            px_ui::status::show(ui, "lápiz · color 1 #000000", false);
            let eleccion = palette_panel::show(ui, image, choice, scroll, enabled);
            let rect_lienzo = ui.available_rect_before_wrap();
            let entradas = canvas.interact_ordered(ui);
            let info = canvas.paint(ui);
            resultado = Some((rect_lienzo, entradas, info, eleccion));
        });
        salida.textures_delta.clear();
        let formas = std::mem::take(&mut salida.shapes);
        let (rect_lienzo, entradas, info, eleccion) = resultado.expect("el lienzo se dibuja");
        Fotograma {
            rect_lienzo,
            entradas,
            info,
            eleccion,
            formas,
        }
    }

    /// Fotograma sin eventos, con la selección inicial de la imagen y el panel activo.
    fn reposo(&mut self, image: &Image) -> Fotograma {
        self.fotograma(image, initial_choice(image), true, vec![])
    }

    /// Mueve el puntero a `pos` (un fotograma) y luego ejecuta `events` en otro.
    fn con_puntero(
        &mut self,
        image: &Image,
        choice: ColorChoice,
        enabled: bool,
        pos: Pos2,
        events: Vec<Event>,
    ) -> Fotograma {
        self.fotograma(image, choice, enabled, vec![Event::PointerMoved(pos)]);
        self.fotograma(image, choice, enabled, events)
    }
}

fn rueda_lineas(delta_y: f32) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, delta_y),
        phase: TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    }
}

fn rueda_puntos(delta_y: f32) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: vec2(0.0, delta_y),
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

fn boton(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn primario(pos: Pos2, pressed: bool) -> Event {
    boton(pos, PointerButton::Primary, pressed)
}

/// Tolerancia en píxeles físicos: `egui` redondea los rectángulos de los paneles a 1/32 de punto
/// (error máximo de 1/64 pt, o sea 0,023 px a 1,5× y 0,047 px a 3×), como en `status.rs`.
const TOLERANCIA_PX: f32 = 0.05;

fn cerca(a: f32, b: f32, tolerancia: f32) -> bool {
    (a - b).abs() <= tolerancia
}

/// Punto en píxeles físicos de la ventana a puntos.
fn en_puntos(ppp: f32, x_px: f32, y_px: f32) -> Pos2 {
    pos2(x_px / ppp, y_px / ppp)
}

fn zoom(n: u32) -> Zoom {
    Zoom::from_int(n).expect("el zoom existe")
}

// ---------------------------------------------------------------------------------------------
// Geometría de las muestras, a partir de las constantes de `palette_grid`
// ---------------------------------------------------------------------------------------------

/// Centro de la muestra `indice` en píxeles físicos de la ventana, con la primera fila visible
/// `primera_fila`. El panel empieza en (0, 0) de la ventana.
fn centro_de_muestra_px(n: u32, indice: usize, primera_fila: usize) -> (f32, f32) {
    let n = i64::from(n);
    let columna = (indice % COLUMNS) as i64;
    let fila = (indice / COLUMNS) as i64 - primera_fila as i64;
    let x = (GRID_X_U + columna * PITCH_U) * n + CELL_U * n / 2;
    let y = (GRID_Y_U + fila * PITCH_U) * n + CELL_U * n / 2;
    (x as f32, y as f32)
}

/// Una elección distinta de la de la muestra `indice`, para que devolverla sea un cambio.
fn otra_eleccion(image: &Image, indice: usize) -> ColorChoice {
    let total = cell_count(image);
    choice_for_cell(image, (indice + 1) % total).expect("la muestra existe")
}

/// Pulsa (y suelta) el botón `button` en `(x_px, y_px)` y devuelve lo que `show` devolvió al
/// pulsar y al soltar.
fn clic(
    banco: &mut Banco,
    image: &Image,
    choice: ColorChoice,
    enabled: bool,
    button: PointerButton,
    (x_px, y_px): (f32, f32),
) -> (Option<ColorChoice>, Option<ColorChoice>) {
    let pos = en_puntos(banco.ppp(), x_px, y_px);
    let pulsado = banco
        .con_puntero(image, choice, enabled, pos, vec![boton(pos, button, true)])
        .eleccion;
    let soltado = banco
        .fotograma(image, choice, enabled, vec![boton(pos, button, false)])
        .eleccion;
    (pulsado, soltado)
}

// ---------------------------------------------------------------------------------------------
// Fila 5: el lienzo desplazado por el panel
// ---------------------------------------------------------------------------------------------

/// Un caso de la fila 5, en píxeles físicos (salvo la pantalla, en puntos).
struct CasoPanel {
    ppp: f32,
    /// Ancho del panel.
    panel: f32,
    /// Lienzo: ancho y alto.
    lienzo: (f32, f32),
    zoom: u32,
    /// Esquina superior izquierda de la imagen y su tamaño, desde la esquina de la ventana.
    imagen_min: (f32, f32),
    imagen_tamano: (f32, f32),
    /// Centro del píxel (2, 3).
    puntero: (f32, f32),
}

const PANTALLA_GRANDE: Vec2 = vec2(800.0, 600.0);

fn casos_fila_5() -> [CasoPanel; 3] {
    [
        CasoPanel {
            ppp: 1.0,
            panel: 233.0,
            lienzo: (567.0, 580.0),
            zoom: 8,
            imagen_min: (260.0, 98.0),
            imagen_tamano: (512.0, 384.0),
            puntero: (280.0, 126.0),
        },
        CasoPanel {
            ppp: 2.0,
            panel: 466.0,
            lienzo: (1134.0, 1160.0),
            zoom: 17,
            imagen_min: (489.0, 172.0),
            imagen_tamano: (1088.0, 816.0),
            puntero: (531.5, 231.5),
        },
        CasoPanel {
            ppp: 1.5,
            panel: 466.0,
            lienzo: (734.0, 860.0),
            zoom: 11,
            imagen_min: (481.0, 166.0),
            imagen_tamano: (704.0, 528.0),
            puntero: (508.5, 204.5),
        },
    ]
}

/// El zoom de ajuste es el del lienzo recortado por el panel y la barra, y la imagen cae donde
/// dice la spec.
fn comprueba_ajuste(f: &Fotograma, caso: &CasoPanel) {
    let ppp = caso.ppp;
    assert_eq!(f.info.zoom, zoom(caso.zoom), "ppp {ppp}: zoom de ajuste");
    let imagen = f.info.image_rect.expect("hay imagen");
    let min = (imagen.min.x * ppp, imagen.min.y * ppp);
    let tamano = (imagen.width() * ppp, imagen.height() * ppp);
    assert!(
        cerca(min.0, caso.imagen_min.0, TOLERANCIA_PX)
            && cerca(min.1, caso.imagen_min.1, TOLERANCIA_PX),
        "ppp {ppp}: la imagen empieza en {min:?} px, se esperaba {:?}",
        caso.imagen_min
    );
    assert!(
        cerca(tamano.0, caso.imagen_tamano.0, TOLERANCIA_PX)
            && cerca(tamano.1, caso.imagen_tamano.1, TOLERANCIA_PX),
        "ppp {ppp}: la imagen mide {tamano:?} px, se esperaba {:?}",
        caso.imagen_tamano
    );
}

#[test]
fn fila_05_el_panel_quita_su_ancho_al_lienzo_y_home_ajusta_al_resto() {
    let image = imagen_rgba();
    for caso in casos_fila_5() {
        let ppp = caso.ppp;
        let mut banco = Banco::con_imagen(ppp, PANTALLA_GRANDE, &image);
        let f = banco.reposo(&image);

        // El rectángulo del lienzo: pantalla menos el panel (a la izquierda) y la barra (abajo).
        let rect = f.rect_lienzo;
        assert!(
            cerca(rect.min.x * ppp, caso.panel, TOLERANCIA_PX),
            "ppp {ppp}: el lienzo empieza en {} px, el panel mide {} px",
            rect.min.x * ppp,
            caso.panel
        );
        assert!(cerca(rect.min.y, 0.0, 1e-3), "ppp {ppp}: {rect:?}");
        assert!(
            cerca(rect.max.x, PANTALLA_GRANDE.x, 1e-3),
            "ppp {ppp}: {rect:?}"
        );
        assert!(
            cerca(rect.width() * ppp, caso.lienzo.0, TOLERANCIA_PX)
                && cerca(rect.height() * ppp, caso.lienzo.1, TOLERANCIA_PX),
            "ppp {ppp}: lienzo de {}×{} px, se esperaba {:?}",
            rect.width() * ppp,
            rect.height() * ppp,
            caso.lienzo
        );
        comprueba_ajuste(&f, &caso);

        // Se cambia el zoom con la rueda (con el puntero sobre el lienzo) y `Home` lo devuelve.
        let centro = rect.center();
        let f = banco.con_puntero(
            &image,
            initial_choice(&image),
            true,
            centro,
            vec![rueda_lineas(1.0)],
        );
        assert_eq!(
            Some(f.info.zoom),
            Zoom::from_int(caso.zoom + 1),
            "ppp {ppp}: la rueda sobre el lienzo sube un paso"
        );
        let f = banco.fotograma(&image, initial_choice(&image), true, vec![tecla(Key::Home)]);
        comprueba_ajuste(&f, &caso);
    }
}

#[test]
fn fila_05_el_puntero_sobre_el_pixel_2_3_da_ese_pixel() {
    let image = imagen_rgba();
    for caso in casos_fila_5() {
        let ppp = caso.ppp;
        let mut banco = Banco::con_imagen(ppp, PANTALLA_GRANDE, &image);
        let f = banco.reposo(&image);
        assert_eq!(f.info.zoom, zoom(caso.zoom), "ppp {ppp}");

        let sobre_el_pixel = en_puntos(ppp, caso.puntero.0, caso.puntero.1);
        let f = banco.fotograma(
            &image,
            initial_choice(&image),
            true,
            vec![Event::PointerMoved(sobre_el_pixel)],
        );
        assert_eq!(
            f.info.hover,
            Some((2, 3)),
            "ppp {ppp}: el puntero en {sobre_el_pixel:?} pt"
        );
        assert!(f.entradas.is_empty());

        // Contraste: pulsar ahí sí empieza un trazo en ese píxel, así que la ausencia de
        // eventos sobre el panel y la barra no es un banco que no recoge nada.
        let f = banco.fotograma(
            &image,
            initial_choice(&image),
            true,
            vec![primario(sobre_el_pixel, true)],
        );
        assert_eq!(
            f.entradas,
            vec![CanvasInput::Stroke(CanvasEvent::StrokeBegin((2, 3)))],
            "ppp {ppp}"
        );
        let f = banco.fotograma(
            &image,
            initial_choice(&image),
            true,
            vec![primario(sobre_el_pixel, false)],
        );
        assert!(
            f.entradas
                .contains(&CanvasInput::Stroke(CanvasEvent::StrokeEnd)),
            "ppp {ppp}"
        );
    }
}

#[test]
fn fila_05_sobre_el_panel_o_la_barra_ni_hover_ni_trazo_ni_zoom() {
    let image = imagen_rgba();
    let choice = initial_choice(&image);
    for caso in casos_fila_5() {
        let ppp = caso.ppp;
        let mut banco = Banco::con_imagen(ppp, PANTALLA_GRANDE, &image);
        let base = banco.reposo(&image);
        let sobre_el_pixel = en_puntos(ppp, caso.puntero.0, caso.puntero.1);

        // Sobre el panel (100, 100) pt y sobre la barra: la franja de abajo.
        for (nombre, sitio) in [
            ("el panel", pos2(100.0, 100.0)),
            ("la barra", pos2(500.0, 595.0)),
        ] {
            assert!(
                !base.rect_lienzo.contains(sitio),
                "ppp {ppp}: {nombre} cae fuera del lienzo"
            );
            let f = banco.fotograma(&image, choice, true, vec![Event::PointerMoved(sitio)]);
            assert_eq!(f.info.hover, None, "ppp {ppp}: hover sobre {nombre}");
            assert!(f.entradas.is_empty(), "ppp {ppp}: {:?}", f.entradas);

            // Pulsar, arrastrar por encima del lienzo y soltar: ningún trazo.
            let f = banco.fotograma(&image, choice, true, vec![primario(sitio, true)]);
            assert!(
                f.entradas.is_empty(),
                "ppp {ppp}: pulsar en {nombre}: {:?}",
                f.entradas
            );
            let f = banco.fotograma(
                &image,
                choice,
                true,
                vec![Event::PointerMoved(sobre_el_pixel)],
            );
            assert!(
                f.entradas.is_empty(),
                "ppp {ppp}: arrastre desde {nombre}: {:?}",
                f.entradas
            );
            let f = banco.fotograma(&image, choice, true, vec![primario(sobre_el_pixel, false)]);
            assert!(
                f.entradas.is_empty(),
                "ppp {ppp}: soltar tras {nombre}: {:?}",
                f.entradas
            );

            // La rueda sobre el panel o la barra no cambia el zoom del lienzo.
            let antes = banco.reposo(&image).info.zoom;
            let f = banco.con_puntero(&image, choice, true, sitio, vec![rueda_lineas(1.0)]);
            assert_eq!(
                f.info.zoom, antes,
                "ppp {ppp}: la rueda sobre {nombre} cambia el zoom"
            );
            let f = banco.fotograma(&image, choice, true, vec![rueda_lineas(-1.0)]);
            assert_eq!(f.info.zoom, antes, "ppp {ppp}: rueda hacia abajo");
        }
    }
}

#[test]
fn fila_05_con_200_pt_de_ancho_el_lienzo_tiene_ancho_0_y_no_hay_panico() {
    let image = imagen_rgba();
    let choice = initial_choice(&image);
    for ppp in [1.0_f32, 1.5, 2.0] {
        let mut banco = Banco::con_imagen(ppp, vec2(200.0, 600.0), &image);
        let f = banco.reposo(&image);
        assert!(
            (f.rect_lienzo.width() * ppp).round() <= 0.0,
            "ppp {ppp}: el lienzo mide {} px de ancho",
            f.rect_lienzo.width() * ppp
        );
        let zoom_antes = f.info.zoom;
        assert_eq!(f.info.hover, None, "ppp {ppp}");

        // `Home` sin efecto, y el puntero en varios sitios (sobre el panel recortado y sobre
        // la barra) no da `hover` ni trazos.
        let f = banco.fotograma(&image, choice, true, vec![tecla(Key::Home)]);
        assert_eq!(f.info.zoom, zoom_antes, "ppp {ppp}: Home sin efecto");
        for sitio in [pos2(150.0, 300.0), pos2(20.0, 10.0), pos2(100.0, 595.0)] {
            let f = banco.fotograma(&image, choice, true, vec![Event::PointerMoved(sitio)]);
            assert_eq!(f.info.hover, None, "ppp {ppp}: hover en {sitio:?}");
            let f = banco.fotograma(&image, choice, true, vec![primario(sitio, true)]);
            assert!(f.entradas.is_empty(), "ppp {ppp}: {:?}", f.entradas);
            let f = banco.fotograma(&image, choice, true, vec![primario(sitio, false)]);
            assert!(f.entradas.is_empty(), "ppp {ppp}: {:?}", f.entradas);
        }
        // Y con una imagen indexada de 256 colores (rejilla grande) y la rueda sobre el panel.
        let grande = imagen_indexada(256);
        banco.canvas.set_image(&grande);
        let choice = initial_choice(&grande);
        banco.fotograma(
            &grande,
            choice,
            true,
            vec![Event::PointerMoved(pos2(100.0, 100.0))],
        );
        banco.fotograma(&grande, choice, true, vec![rueda_lineas(-1.0)]);
        banco.fotograma(&grande, choice, true, vec![rueda_lineas(1.0)]);
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12: los textos caben y se maquetan con `MissingGlyphPolicy::Panic`
// ---------------------------------------------------------------------------------------------

const PPP_FILA_12: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

/// Todos los textos del panel: `cell_label` de todas las muestras de cada modo e `info_flags`
/// en las ocho combinaciones.
fn textos_del_panel() -> Vec<String> {
    let mut textos = Vec::new();
    for mode in MODOS {
        let image = imagen_de(mode);
        for cell in 0..cell_count(&image) {
            textos.push(cell_label(&image, cell).expect("la muestra existe"));
        }
    }
    // Paletas con alfa y sin él, de varios tamaños.
    for count in [1, 3, 17, 18, 255] {
        let image = imagen_indexada(count);
        for cell in 0..cell_count(&image) {
            textos.push(cell_label(&image, cell).expect("la muestra existe"));
        }
    }
    for bits in 0..8 {
        textos.push(info_flags(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0));
    }
    textos
}

#[test]
fn fila_12_los_textos_mas_largos_miden_20_y_30_caracteres() {
    let image = imagen_indexada(256);
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
        info_flags(true, true, true),
        "cursor · activa · transparente"
    );
    assert_eq!(info_flags(true, true, true).chars().count(), 30);
    let mas_largo = textos_del_panel()
        .iter()
        .map(|t| t.chars().count())
        .max()
        .unwrap();
    assert_eq!(mas_largo, 30, "ningún texto del panel supera los 30");
}

#[test]
fn fila_12_columns_de_los_225_u_utiles_es_32_con_cualquier_n() {
    for ppp in PPP_FILA_12 {
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
fn fila_12_cada_texto_se_maqueta_sin_panico_y_cabe_sin_recorte() {
    let textos = textos_del_panel();
    for ppp in PPP_FILA_12 {
        let banco = Banco::nuevo(ppp, PANTALLA_GRANDE);
        let n = text_scale(ppp);
        let maximo = columns(palette_grid::TEXT_WIDTH_U as f32 * n as f32, ppp);
        assert_eq!(maximo, 32);
        let ancho_util_px = (palette_grid::TEXT_WIDTH_U * i64::from(n)) as f32;

        // Todo sale de `INTERFACE_CHARSET`.
        for texto in &textos {
            for c in texto.chars() {
                assert!(
                    INTERFACE_CHARSET.contains(c),
                    "ppp {ppp}: {texto:?} lleva {c:?}, que no está en INTERFACE_CHARSET"
                );
            }
            assert!(
                texto.chars().count() <= maximo,
                "ppp {ppp}: {texto:?} no cabe en {maximo} columnas"
            );
            // `fit_text` no recorta nada.
            let recortado = with_glyph_check(&banco.ctx, |has| fit_text(texto, maximo, has));
            assert_eq!(&recortado, texto, "ppp {ppp}");
        }

        // Maquetado con la política `Panic`, y el ancho cabe en los 225 u útiles.
        let mut anchos = Vec::new();
        let salida = banco.ctx.run_ui(banco.entrada(vec![]), |ui| {
            apply_scale(ui.ctx());
            let fuente = FontId::proportional(font_size_points(ui.ctx().pixels_per_point()));
            ui.ctx().fonts_mut(|f| {
                for texto in &textos {
                    let galeria =
                        f.layout_no_wrap(texto.clone(), fuente.clone(), egui::Color32::WHITE);
                    anchos.push((texto.clone(), galeria.size().x * ppp));
                }
            });
        });
        salida.drop_without_applying_deltas();
        assert_eq!(anchos.len(), textos.len());
        for (texto, ancho) in anchos {
            assert!(
                ancho <= ancho_util_px + 0.5,
                "ppp {ppp}: {texto:?} mide {ancho} px y caben {ancho_util_px}"
            );
        }
    }
}

#[test]
fn fila_12_el_panel_pinta_cada_modo_con_el_cursor_en_la_ultima_muestra_sin_panico() {
    for ppp in PPP_FILA_12 {
        for mode in MODOS {
            let image = imagen_de(mode);
            let mut banco = Banco::con_imagen(ppp, PANTALLA_GRANDE, &image);
            let n = banco.n();
            let ultima = cell_count(&image) - 1;
            let (x, y) = centro_de_muestra_px(n, ultima, 0);
            let pos = en_puntos(ppp, x, y);
            let choice = initial_choice(&image);
            banco.con_puntero(&image, choice, true, pos, vec![]);
            banco.fotograma(&image, choice, true, vec![Event::PointerMoved(pos)]);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3: nitidez
// ---------------------------------------------------------------------------------------------

/// Los `ppp` que se prueban siempre.
const PPP_SIEMPRE: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

/// Todos los múltiplos de 0,05 entre 1 y 4.
fn ppp_multiplos_de_0_05() -> impl Iterator<Item = f32> {
    (20..=80).map(|i| i as f32 / 20.0)
}

/// Cada borde de la maqueta, en puntos por `ppp`, es un entero de píxeles físicos (±0,001) y
/// múltiplo de `n`.
fn comprueba_nitidez(ppp: f32) {
    let n = text_scale(ppp);
    let u = ui_unit(ppp);
    let aristas = palette_grid::edges_u();
    assert!(!aristas.is_empty(), "edges_u no devuelve ningún borde");
    for k in aristas {
        let fisico = k as f64 * f64::from(u) * f64::from(ppp);
        assert!(
            (fisico - fisico.round()).abs() <= 0.001,
            "ppp {ppp}: el borde {k} u cae en {fisico} px físicos"
        );
        assert_eq!(
            (fisico.round() as i64) % i64::from(n),
            0,
            "ppp {ppp}: el borde {k} u ({fisico} px) no es múltiplo de n = {n}"
        );
    }
}

#[test]
fn propiedad_3_los_bordes_caen_en_pixeles_fisicos_enteros_a_los_ppp_de_la_spec() {
    for ppp in PPP_SIEMPRE {
        comprueba_nitidez(ppp);
    }
}

#[test]
fn propiedad_3_los_bordes_caen_en_pixeles_fisicos_enteros_en_todos_los_multiplos_de_0_05() {
    for ppp in ppp_multiplos_de_0_05() {
        comprueba_nitidez(ppp);
    }
}

#[test]
fn propiedad_3_las_aristas_incluyen_los_bordes_del_panel_y_de_la_rejilla() {
    let aristas = palette_grid::edges_u();
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
        assert!(aristas.contains(&k), "falta el borde {k} u en edges_u()");
    }
}

#[test]
fn propiedad_3_el_panel_real_mide_233_n_pixeles_fisicos_y_empieza_en_cero() {
    let image = imagen_rgba();
    let ancho_u = PANEL_WIDTH_U as f32;
    for ppp in PPP_SIEMPRE {
        let mut banco = Banco::con_imagen(ppp, PANTALLA_GRANDE, &image);
        let n = banco.n() as f32;
        let f = banco.reposo(&image);

        // Lo que el panel quita al lienzo: empieza en x = 0 y mide `233·n` px.
        assert!(
            cerca(f.rect_lienzo.min.x * ppp, ancho_u * n, TOLERANCIA_PX),
            "ppp {ppp}: el panel mide {} px, se esperaba {}",
            f.rect_lienzo.min.x * ppp,
            ancho_u * n
        );
        assert!(cerca(f.rect_lienzo.min.y, 0.0, 1e-3), "ppp {ppp}");

        // Y el fondo pintado (`SURFACE_1`) del panel arranca en (0, 0) con ese ancho.
        let mut rects = Vec::new();
        for clipped in &f.formas {
            recoge_rectangulos(&clipped.shape, &mut rects);
        }
        let del_panel: Vec<Rect> = rects
            .iter()
            .filter(|(_, relleno)| *relleno == SURFACE_1)
            .map(|(rect, _)| *rect)
            .filter(|rect| {
                cerca(rect.min.x * ppp, 0.0, TOLERANCIA_PX)
                    && cerca(rect.min.y * ppp, 0.0, TOLERANCIA_PX)
            })
            .collect();
        assert!(
            !del_panel.is_empty(),
            "ppp {ppp}: ningún rectángulo SURFACE_1 empieza en (0, 0)"
        );
        assert!(
            del_panel
                .iter()
                .any(|rect| cerca(rect.width() * ppp, ancho_u * n, TOLERANCIA_PX)),
            "ppp {ppp}: ningún fondo del panel mide {} px: {del_panel:?}",
            ancho_u * n
        );
    }
}

/// Rectángulos rellenos de una forma, con su color de relleno (aplana `Shape::Vec`).
fn recoge_rectangulos(shape: &Shape, salida: &mut Vec<(Rect, egui::Color32)>) {
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

// ---------------------------------------------------------------------------------------------
// `show`: elegir al pulsar
// ---------------------------------------------------------------------------------------------

#[test]
fn show_pulsar_el_centro_de_una_muestra_elige_esa_muestra_en_una_indexada_de_256() {
    let image = imagen_indexada(256);
    for ppp in PPP_SIEMPRE {
        let mut banco = Banco::con_imagen(ppp, PANTALLA_GRANDE, &image);
        let n = banco.n();
        banco.reposo(&image);
        for cell in [0usize, 1, 8, 15, 16, 17, 200, 254, 255] {
            let esperado = choice_for_cell(&image, cell).expect("la muestra existe");
            let actual = otra_eleccion(&image, cell);
            let (pulsado, soltado) = clic(
                &mut banco,
                &image,
                actual,
                true,
                PointerButton::Primary,
                centro_de_muestra_px(n, cell, 0),
            );
            assert_eq!(
                pulsado,
                Some(esperado),
                "ppp {ppp}: pulsar la muestra {cell}"
            );
            assert_eq!(esperado, ColorChoice::Entry(cell as u8));
            assert_eq!(soltado, None, "ppp {ppp}: soltar no elige (muestra {cell})");
        }
    }
}

#[test]
fn show_pulsar_una_muestra_elige_slot_en_rgba_gray_y_gray_alpha() {
    for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        let image = imagen_de(mode);
        let mut banco = Banco::con_imagen(1.0, PANTALLA_GRANDE, &image);
        banco.reposo(&image);
        for cell in 0..cell_count(&image) {
            let (pulsado, _) = clic(
                &mut banco,
                &image,
                otra_eleccion(&image, cell),
                true,
                PointerButton::Primary,
                centro_de_muestra_px(1, cell, 0),
            );
            assert_eq!(
                pulsado,
                Some(ColorChoice::Slot(cell as u8 + 1)),
                "{mode:?}: muestra {cell}"
            );
            assert_eq!(pulsado, choice_for_cell(&image, cell));
        }
    }
}

#[test]
fn show_con_filas_desplazadas_la_muestra_pulsada_es_la_que_se_ve() {
    // Pantalla baja: pocas filas visibles de las 16.
    let image = imagen_indexada(256);
    for (ppp, alto) in [(1.0_f32, 200.0_f32), (2.0, 150.0)] {
        let mut banco = Banco::con_imagen(ppp, vec2(800.0, alto), &image);
        let n = banco.n();
        let panel_h_px = (alto * ppp) as i64 - i64::from(bar_height_px(n));
        let visibles = palette_grid::visible_rows(panel_h_px, n);
        let total = palette_grid::total_rows(256);
        assert!(
            visibles >= 3 && visibles < total,
            "ppp {ppp}: {visibles} filas"
        );

        let primera = 2usize;
        banco.scroll = Scroll {
            first_row: primera,
            rest: 0,
        };
        banco.reposo(&image);
        assert_eq!(
            banco.scroll.first_row, primera,
            "ppp {ppp}: reposo no mueve"
        );
        for (fila_visible, columna) in [(0usize, 0usize), (1, 5), (visibles - 1, 15)] {
            let cell = (primera + fila_visible) * COLUMNS + columna;
            let (pulsado, _) = clic(
                &mut banco,
                &image,
                otra_eleccion(&image, cell),
                true,
                PointerButton::Primary,
                centro_de_muestra_px(n, cell, primera),
            );
            assert_eq!(
                pulsado,
                Some(ColorChoice::Entry(cell as u8)),
                "ppp {ppp}: muestra {cell} con la fila {primera} arriba"
            );
        }
        assert_eq!(banco.scroll.first_row, primera, "elegir no desplaza");
    }
}

// ---------------------------------------------------------------------------------------------
// `show`: clics que no eligen (fila 7)
// ---------------------------------------------------------------------------------------------

#[test]
fn show_los_clics_en_huecos_y_fuera_de_la_rejilla_no_eligen() {
    let image = imagen_indexada(256);
    for ppp in PPP_SIEMPRE {
        let mut banco = Banco::con_imagen(ppp, PANTALLA_GRANDE, &image);
        let n = i64::from(banco.n());
        let choice = ColorChoice::Entry(5);
        banco.reposo(&image);

        let (cx, cy) = centro_de_muestra_px(n as u32, 0, 0);
        let mitad_hueco = GAP_U * n / 2;
        let posiciones = [
            // Hueco entre la columna 0 y la 1, a la altura de la fila 0.
            (
                "hueco horizontal",
                ((GRID_X_U + CELL_U) * n + mitad_hueco) as f32,
                cy,
            ),
            // Hueco entre la fila 0 y la 1, a la altura de la columna 0.
            (
                "hueco vertical",
                cx,
                ((GRID_Y_U + CELL_U) * n + mitad_hueco) as f32,
            ),
            // Esquina de huecos.
            (
                "esquina de huecos",
                ((GRID_X_U + CELL_U) * n + mitad_hueco) as f32,
                ((GRID_Y_U + CELL_U) * n + mitad_hueco) as f32,
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
                ((GRID_X_U + 15 * PITCH_U + CELL_U) * n + mitad_hueco) as f32,
                ((GRID_Y_U + 15 * PITCH_U + CELL_U) * n + mitad_hueco) as f32,
            ),
        ];
        for (nombre, x, y) in posiciones {
            let (pulsado, soltado) = clic(
                &mut banco,
                &image,
                choice,
                true,
                PointerButton::Primary,
                (x, y),
            );
            assert_eq!(pulsado, None, "ppp {ppp}: pulsar en {nombre} ({x}, {y}) px");
            assert_eq!(soltado, None, "ppp {ppp}: soltar en {nombre}");
        }
    }
}

#[test]
fn show_un_clic_en_una_muestra_que_no_existe_no_elige() {
    // RGBA: solo las 9 primeras; la columna 9 de la fila 0 está vacía. Y una indexada de 3.
    for image in [imagen_rgba(), imagen_indexada(3)] {
        let count = cell_count(&image);
        let mut banco = Banco::con_imagen(1.0, PANTALLA_GRANDE, &image);
        banco.reposo(&image);
        let choice = initial_choice(&image);
        for cell in [count, count + 1, 15, 16, 255] {
            let (pulsado, _) = clic(
                &mut banco,
                &image,
                choice,
                true,
                PointerButton::Primary,
                centro_de_muestra_px(1, cell, 0),
            );
            assert_eq!(pulsado, None, "{count} muestras: la {cell} no existe");
        }
    }
}

#[test]
fn show_una_fila_que_no_se_ve_no_se_elige() {
    // Pantalla de 146 pt a ppp 1: el panel mide 126 px y solo se ven 3 filas (la banda del
    // indicador, de I.2a, ocupa 36 u más que antes).
    let image = imagen_indexada(256);
    let mut banco = Banco::con_imagen(1.0, vec2(800.0, 146.0), &image);
    let n = banco.n();
    let panel_h_px = 146 - i64::from(bar_height_px(n));
    assert_eq!(palette_grid::visible_rows(panel_h_px, n), 3);
    let choice = ColorChoice::Entry(5);
    banco.reposo(&image);

    // La fila 3 (la cuarta) empieza dentro del panel (a 119 px) pero no se ve.
    let (x, y) = centro_de_muestra_px(n, 3 * COLUMNS, 0);
    assert!(y < panel_h_px as f32, "la fila oculta cae dentro del panel");
    let (pulsado, _) = clic(
        &mut banco,
        &image,
        choice,
        true,
        PointerButton::Primary,
        (x, y),
    );
    assert_eq!(pulsado, None, "la fila 3 no se ve");
    // La fila 2 sí.
    let (pulsado, _) = clic(
        &mut banco,
        &image,
        choice,
        true,
        PointerButton::Primary,
        centro_de_muestra_px(n, 2 * COLUMNS + 4, 0),
    );
    assert_eq!(pulsado, Some(ColorChoice::Entry(36)));
}

#[test]
fn show_los_botones_derecho_y_central_no_eligen() {
    let image = imagen_indexada(256);
    let mut banco = Banco::con_imagen(1.0, PANTALLA_GRANDE, &image);
    banco.reposo(&image);
    for button in [PointerButton::Secondary, PointerButton::Middle] {
        let (pulsado, soltado) = clic(
            &mut banco,
            &image,
            ColorChoice::Entry(5),
            true,
            button,
            centro_de_muestra_px(1, 200, 0),
        );
        assert_eq!(pulsado, None, "{button:?}");
        assert_eq!(soltado, None, "{button:?}");
    }
    // Contraste: el botón izquierdo en el mismo sitio sí elige.
    let (pulsado, _) = clic(
        &mut banco,
        &image,
        ColorChoice::Entry(5),
        true,
        PointerButton::Primary,
        centro_de_muestra_px(1, 200, 0),
    );
    assert_eq!(pulsado, Some(ColorChoice::Entry(200)));
}

#[test]
fn show_con_un_gesto_en_curso_enabled_falso_no_elige() {
    for mode in MODOS {
        let image = imagen_de(mode);
        let mut banco = Banco::con_imagen(1.0, PANTALLA_GRANDE, &image);
        banco.reposo(&image);
        let cell = cell_count(&image) - 1;
        let choice = otra_eleccion(&image, cell);
        let (pulsado, soltado) = clic(
            &mut banco,
            &image,
            choice,
            false,
            PointerButton::Primary,
            centro_de_muestra_px(1, cell, 0),
        );
        assert_eq!(pulsado, None, "{mode:?}: enabled falso");
        assert_eq!(soltado, None, "{mode:?}: enabled falso");
        // Contraste con `enabled` verdadero.
        let (pulsado, _) = clic(
            &mut banco,
            &image,
            choice,
            true,
            PointerButton::Primary,
            centro_de_muestra_px(1, cell, 0),
        );
        assert_eq!(pulsado, choice_for_cell(&image, cell), "{mode:?}");
    }
}

#[test]
fn show_sin_eventos_no_elige_nada() {
    let image = imagen_indexada(256);
    let mut banco = Banco::con_imagen(1.0, PANTALLA_GRANDE, &image);
    for _ in 0..3 {
        assert_eq!(banco.reposo(&image).eleccion, None);
    }
    // Un puntero que solo pasa por encima de una muestra tampoco elige.
    let (x, y) = centro_de_muestra_px(1, 200, 0);
    let f = banco.fotograma(
        &image,
        ColorChoice::Entry(0),
        true,
        vec![Event::PointerMoved(en_puntos(1.0, x, y))],
    );
    assert_eq!(f.eleccion, None);
}

// ---------------------------------------------------------------------------------------------
// `show`: la rueda
// ---------------------------------------------------------------------------------------------

/// Casos `(ppp, alto de pantalla en pt)` con muchas filas y poca altura.
const PANTALLAS_BAJAS: [(f32, f32); 2] = [(1.0, 200.0), (2.0, 150.0)];

fn filas_visibles(banco: &Banco) -> (usize, usize) {
    let n = banco.n();
    let panel_h_px = (banco.pantalla.y * banco.ppp()) as i64 - i64::from(bar_height_px(n));
    (
        palette_grid::visible_rows(panel_h_px, n),
        palette_grid::total_rows(256),
    )
}

#[test]
fn show_la_rueda_sobre_el_panel_desplaza_las_filas_y_no_hace_zoom() {
    let image = indexada_con_lienzo();
    let choice = initial_choice(&image);
    for (ppp, alto) in PANTALLAS_BAJAS {
        let mut banco = Banco::con_imagen(ppp, vec2(800.0, alto), &image);
        let (visibles, total) = filas_visibles(&banco);
        assert!(visibles >= 1 && visibles < total, "ppp {ppp}: {visibles}");
        let maxima = palette_grid::max_first_row(total, visibles);
        let zoom_antes = banco.reposo(&image).info.zoom;
        assert_eq!(banco.scroll, Scroll::default());

        let sobre_el_panel = pos2(100.0, 60.0);
        // Una línea de rueda hacia abajo son 3 filas.
        let f = banco.con_puntero(
            &image,
            choice,
            true,
            sobre_el_panel,
            vec![rueda_lineas(-1.0)],
        );
        assert_eq!(
            banco.scroll.first_row,
            3.min(maxima),
            "ppp {ppp}: una línea de rueda hacia abajo"
        );
        assert_eq!(f.info.zoom, zoom_antes, "ppp {ppp}: la rueda no hace zoom");
        assert_eq!(f.eleccion, None, "ppp {ppp}: la rueda no elige");

        // Hacia arriba vuelve a la fila 0 (y no pasa de ahí).
        banco.fotograma(&image, choice, true, vec![rueda_lineas(1.0)]);
        assert_eq!(banco.scroll.first_row, 0, "ppp {ppp}: hacia arriba");
        let f = banco.fotograma(&image, choice, true, vec![rueda_lineas(1.0)]);
        assert_eq!(banco.scroll.first_row, 0, "ppp {ppp}: no baja de 0");
        assert_eq!(f.info.zoom, zoom_antes);

        // Muchas líneas hacia abajo: se queda en la última fila posible.
        for _ in 0..10 {
            banco.fotograma(&image, choice, true, vec![rueda_lineas(-1.0)]);
        }
        assert_eq!(banco.scroll.first_row, maxima, "ppp {ppp}: tope");
        let f = banco.reposo(&image);
        assert_eq!(f.info.zoom, zoom_antes);
    }
}

#[test]
fn show_la_rueda_en_puntos_son_pixeles_fisicos() {
    // Con n = 2 (ppp 2), una fila son 28 px físicos = 14 pt: `delta · ppp`.
    let image = indexada_con_lienzo();
    let choice = initial_choice(&image);
    let mut banco = Banco::con_imagen(2.0, vec2(800.0, 150.0), &image);
    let n = banco.n();
    let fila_pt = (palette_grid::PITCH_U * i64::from(n)) as f32 / banco.ppp();
    banco.reposo(&image);
    let sobre_el_panel = pos2(100.0, 60.0);
    banco.con_puntero(
        &image,
        choice,
        true,
        sobre_el_panel,
        vec![rueda_puntos(-fila_pt)],
    );
    assert_eq!(banco.scroll.first_row, 1, "una fila exacta");
    // Media fila no mueve las filas: se acumula (la fila no se corta).
    banco.fotograma(&image, choice, true, vec![rueda_puntos(-fila_pt / 2.0)]);
    assert_eq!(banco.scroll.first_row, 1, "media fila se acumula");
    banco.fotograma(&image, choice, true, vec![rueda_puntos(-fila_pt / 2.0)]);
    assert_eq!(banco.scroll.first_row, 2, "las dos mitades suman una fila");
    // Hacia arriba, de vuelta.
    banco.fotograma(&image, choice, true, vec![rueda_puntos(fila_pt * 2.0)]);
    assert_eq!(banco.scroll.first_row, 0);
}

#[test]
fn show_la_rueda_sobre_el_lienzo_hace_zoom_y_no_desplaza_el_panel() {
    let image = indexada_con_lienzo();
    let choice = initial_choice(&image);
    for (ppp, alto) in PANTALLAS_BAJAS {
        let mut banco = Banco::con_imagen(ppp, vec2(800.0, alto), &image);
        let base = banco.reposo(&image);
        let zoom_antes = base.info.zoom;
        let sobre_el_lienzo = pos2((base.rect_lienzo.min.x + 800.0) / 2.0, 50.0);
        assert!(base.rect_lienzo.contains(sobre_el_lienzo));

        let f = banco.con_puntero(
            &image,
            choice,
            true,
            sobre_el_lienzo,
            vec![rueda_lineas(1.0)],
        );
        assert_eq!(
            f.info.zoom,
            zoom_antes.zoom_in(),
            "ppp {ppp}: la rueda sobre el lienzo hace zoom"
        );
        assert_eq!(
            banco.scroll,
            Scroll::default(),
            "ppp {ppp}: la rueda sobre el lienzo no desplaza el panel"
        );
    }
}

#[test]
fn show_con_enabled_falso_el_panel_sigue_desplazando() {
    let image = indexada_con_lienzo();
    let choice = initial_choice(&image);
    let mut banco = Banco::con_imagen(1.0, vec2(800.0, 200.0), &image);
    banco.reposo(&image);
    banco.con_puntero(
        &image,
        choice,
        false,
        pos2(100.0, 60.0),
        vec![rueda_lineas(-1.0)],
    );
    assert!(banco.scroll.first_row > 0, "desplaza aunque no elija");
}

/// Indexada de 256 con la imagen ya lista para el lienzo.
fn indexada_con_lienzo() -> Image {
    imagen_indexada(256)
}

#[test]
fn show_el_banco_usa_el_mismo_ancho_de_panel_que_las_constantes() {
    // Sanidad del banco: a ppp 1 el panel son PANEL_WIDTH_U puntos y la muestra 0 queda donde
    // dicen las constantes.
    let image = imagen_rgba();
    let mut banco = Banco::con_imagen(1.0, PANTALLA_GRANDE, &image);
    let f = banco.reposo(&image);
    assert!(cerca(f.rect_lienzo.min.x, PANEL_WIDTH_U as f32, 0.05));
    assert_eq!(
        centro_de_muestra_px(1, 0, 0),
        (
            (GRID_X_U + CELL_U / 2) as f32,
            (GRID_Y_U + CELL_U / 2) as f32
        )
    );
}
