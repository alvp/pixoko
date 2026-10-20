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
    pantalla: Vec2,
}

struct Fotograma {
    /// Lo que quedaba libre justo antes del lienzo: pantalla menos la barra.
    rect_lienzo: Rect,
    entradas: Vec<CanvasInput>,
    info: CanvasInfo,
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

    /// Ejecuta un fotograma con los eventos dados: aplica la escala, monta la barra con `texto`
    /// y después el lienzo, como la app. Limpia los cambios de textura (egui entra en pánico si
    /// se sueltan sin aplicar).
    fn fotograma(&mut self, texto: &str, aviso: bool, events: Vec<Event>) -> Fotograma {
        let input = self.entrada(events);
        let mut resultado = None;
        let canvas = &mut self.canvas;
        let mut salida = self.ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            show(ui, texto, aviso);
            let rect_lienzo = ui.available_rect_before_wrap();
            let entradas = canvas.interact_ordered(ui);
            let info = canvas.paint(ui);
            resultado = Some(Fotograma {
                rect_lienzo,
                entradas,
                info,
            });
        });
        salida.textures_delta.clear();
        resultado.expect("el lienzo se dibuja")
    }

    /// Alto de la barra en píxeles físicos: lo que la pantalla pierde en el lienzo.
    fn alto_de_barra_px(&self, f: &Fotograma) -> f32 {
        (self.pantalla.y - f.rect_lienzo.height()) * self.ppp()
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

fn boton(pos: Pos2, pressed: bool) -> Event {
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
const TOLERANCIA_PX: f32 = 0.05;

fn cerca(a: f32, b: f32, tolerancia: f32) -> bool {
    (a - b).abs() <= tolerancia
}

fn zoom(n: u32) -> Zoom {
    Zoom::from_int(n).expect("el zoom existe")
}

// ---------------------------------------------------------------------------------------------
// Fila 11: altura de la barra
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_11_bar_height_px_son_20_por_n() {
    assert_eq!(bar_height_px(1), 20);
    assert_eq!(bar_height_px(2), 40);
    assert_eq!(bar_height_px(3), 60);
    assert_eq!(bar_height_px(8), 160);
}

#[test]
fn fila_11_la_barra_mide_20_n_pixeles_fisicos_y_el_lienzo_es_el_resto() {
    // (ppp, alto de la barra en píxeles físicos): n = 1, 2, 2.
    for (ppp, barra) in [(1.0_f32, 20.0_f32), (1.5, 40.0), (2.0, 40.0)] {
        let pantalla = vec2(400.0, 300.0);
        let mut banco = Banco::nuevo(ppp, pantalla);
        banco.canvas.set_image(&imagen_de_prueba());
        let corto = banco.fotograma("lápiz · color 1 #ff0000", false, vec![]);
        let alto = banco.alto_de_barra_px(&corto);
        assert!(
            cerca(alto, barra, TOLERANCIA_PX),
            "ppp {ppp}: barra de {alto} px"
        );

        // El lienzo es la pantalla menos la barra, pegada abajo.
        let rect = corto.rect_lienzo;
        assert!(cerca(rect.min.x, 0.0, 1e-3) && cerca(rect.min.y, 0.0, 1e-3));
        assert!(cerca(rect.max.x, pantalla.x, 1e-3), "ppp {ppp}: {rect:?}");
        // La esquina superior de la barra (la inferior del lienzo) cae en píxel físico entero.
        let esquina = rect.max.y * ppp;
        assert!(
            cerca(esquina, esquina.round(), TOLERANCIA_PX),
            "ppp {ppp}: la esquina de la barra cae en el píxel {esquina}"
        );
        assert!(cerca(esquina, pantalla.y * ppp - barra, TOLERANCIA_PX));
        // Y el lienzo, en píxeles físicos enteros (así lo lee `Canvas`), es la pantalla menos la
        // barra, exactamente.
        assert_eq!(
            (rect.height() * ppp).round(),
            pantalla.y * ppp - barra,
            "ppp {ppp}: alto del lienzo en píxeles físicos"
        );

        // Con textos larguísimos (avisos de 400 caracteres, y con saltos de línea) el alto no
        // cambia.
        let largo = "abcdefghij".repeat(40);
        for (texto, aviso) in [
            (largo.as_str(), true),
            (largo.as_str(), false),
            ("primera línea\nsegunda línea\r\ntercera", true),
            ("日本語 😀 \u{202E}", true),
            ("", false),
        ] {
            let f = banco.fotograma(texto, aviso, vec![]);
            assert_eq!(
                f.rect_lienzo, rect,
                "ppp {ppp}: el texto {texto:?} cambia el lienzo"
            );
            assert!(cerca(banco.alto_de_barra_px(&f), barra, TOLERANCIA_PX));
        }
    }
}

#[test]
fn fila_11_el_texto_largo_da_40_caracteres_y_termina_en_puntos_suspensivos() {
    let banco = Banco::nuevo(1.0, vec2(400.0, 300.0));
    let largo = "abcdefghij".repeat(40);
    assert_eq!(largo.chars().count(), 400);
    let texto = bar_text(&banco.ctx, &largo, false, 40);
    assert_eq!(texto.chars().count(), 40);
    assert!(texto.ends_with('…'), "{texto:?}");
    assert!(texto.starts_with("abcdefghij"));
    // También con aviso: el prefijo cuenta dentro de las 40 columnas.
    let aviso = bar_text(&banco.ctx, &largo, true, 40);
    assert_eq!(aviso.chars().count(), 40);
    assert!(aviso.starts_with("aviso: abc"), "{aviso:?}");
    assert!(aviso.ends_with('…'));
}

// ---------------------------------------------------------------------------------------------
// Fila 13: el lienzo desplazado por la barra
// ---------------------------------------------------------------------------------------------

/// Un caso de la fila 13, en píxeles físicos.
struct CasoBarra {
    ppp: f32,
    barra: f32,
    /// Lienzo: ancho y alto.
    lienzo: (f32, f32),
    zoom: u32,
    /// Esquina superior izquierda de la imagen y su tamaño, relativas al lienzo.
    imagen_min: (f32, f32),
    imagen_tamano: (f32, f32),
}

#[test]
fn fila_13_home_ajusta_la_imagen_al_lienzo_y_no_a_la_ventana() {
    let casos = [
        CasoBarra {
            ppp: 1.0,
            barra: 20.0,
            lienzo: (400.0, 280.0),
            zoom: 5,
            imagen_min: (40.0, 20.0),
            imagen_tamano: (320.0, 240.0),
        },
        CasoBarra {
            ppp: 2.0,
            barra: 40.0,
            lienzo: (800.0, 560.0),
            zoom: 11,
            imagen_min: (48.0, 16.0),
            imagen_tamano: (704.0, 528.0),
        },
    ];
    for caso in casos {
        let ppp = caso.ppp;
        let mut banco = Banco::nuevo(ppp, vec2(400.0, 300.0));
        banco.canvas.set_image(&imagen_de_prueba());
        let texto = "64×48 · lápiz";

        // Primer fotograma: ajuste inicial.
        let f = banco.fotograma(texto, false, vec![]);
        let rect = f.rect_lienzo;
        assert!(
            cerca(banco.alto_de_barra_px(&f), caso.barra, 0.01),
            "ppp {ppp}"
        );
        assert!(
            cerca(rect.width() * ppp, caso.lienzo.0, 0.01)
                && cerca(rect.height() * ppp, caso.lienzo.1, 0.01),
            "ppp {ppp}: lienzo {rect:?}"
        );
        comprueba_ajuste(&f, &caso);

        // Se cambia el zoom con la rueda (con el puntero sobre el lienzo) y `Home` lo devuelve.
        let centro = rect.center();
        banco.fotograma(texto, false, vec![Event::PointerMoved(centro)]);
        let f = banco.fotograma(texto, false, vec![rueda(true)]);
        assert_eq!(
            Some(f.info.zoom),
            Zoom::from_int(caso.zoom + 1),
            "ppp {ppp}: la rueda sube un paso"
        );
        let f = banco.fotograma(texto, false, vec![tecla(Key::Home)]);
        comprueba_ajuste(&f, &caso);
    }
}

/// El zoom de ajuste es el del lienzo (sin la barra saldría uno más) y la imagen cae donde dice
/// la spec, en píxeles físicos desde la esquina del lienzo.
fn comprueba_ajuste(f: &Fotograma, caso: &CasoBarra) {
    let ppp = caso.ppp;
    assert_eq!(f.info.zoom, zoom(caso.zoom), "ppp {ppp}: zoom de ajuste");
    let imagen = f.info.image_rect.expect("hay imagen");
    let min = (
        (imagen.min.x - f.rect_lienzo.min.x) * ppp,
        (imagen.min.y - f.rect_lienzo.min.y) * ppp,
    );
    let tamano = (imagen.width() * ppp, imagen.height() * ppp);
    assert!(
        cerca(min.0, caso.imagen_min.0, 0.01) && cerca(min.1, caso.imagen_min.1, 0.01),
        "ppp {ppp}: la imagen empieza en {min:?} px, se esperaba {:?}",
        caso.imagen_min
    );
    assert!(
        cerca(tamano.0, caso.imagen_tamano.0, 0.01) && cerca(tamano.1, caso.imagen_tamano.1, 0.01),
        "ppp {ppp}: la imagen mide {tamano:?} px, se esperaba {:?}",
        caso.imagen_tamano
    );
    // La esquina de la imagen cae en un píxel físico entero.
    let esquina = (imagen.min.x * ppp, imagen.min.y * ppp);
    assert!(cerca(esquina.0, esquina.0.round(), 0.01) && cerca(esquina.1, esquina.1.round(), 0.01));
}

#[test]
fn fila_13_el_puntero_sobre_el_pixel_2_3_da_ese_pixel_y_sobre_la_barra_ninguno() {
    // Centro del píxel (2, 3) de la imagen, en píxeles físicos desde la esquina de la ventana.
    let casos = [
        (1.0_f32, 5.0_f32, (40.0_f32, 20.0_f32)),
        (2.0, 11.0, (48.0, 16.0)),
    ];
    for (ppp, z, (img_x, img_y)) in casos {
        let mut banco = Banco::nuevo(ppp, vec2(400.0, 300.0));
        banco.canvas.set_image(&imagen_de_prueba());
        let texto = "64×48 · lápiz";
        let f = banco.fotograma(texto, false, vec![]);
        assert_eq!(Some(f.info.zoom), Zoom::from_int(z as u32), "ppp {ppp}");

        let centro_px = (img_x + 2.0 * z + z / 2.0, img_y + 3.0 * z + z / 2.0);
        let sobre_el_pixel = pos2(centro_px.0 / ppp, centro_px.1 / ppp);
        let f = banco.fotograma(texto, false, vec![Event::PointerMoved(sobre_el_pixel)]);
        assert_eq!(
            f.info.hover,
            Some((2, 3)),
            "ppp {ppp}: el puntero en {sobre_el_pixel:?} pt"
        );
        assert!(f.entradas.is_empty());

        // Sobre la barra (la franja de abajo, de 20 pt a ppp 1 y a ppp 2): ningún píxel.
        let sobre_la_barra = pos2(200.0, 290.0);
        let f = banco.fotograma(texto, false, vec![Event::PointerMoved(sobre_la_barra)]);
        assert!(
            !f.rect_lienzo.contains(sobre_la_barra),
            "ppp {ppp}: la barra está abajo"
        );
        assert_eq!(f.info.hover, None, "ppp {ppp}: hover sobre la barra");
        assert!(f.entradas.is_empty(), "ppp {ppp}: {:?}", f.entradas);

        // Pulsar sobre la barra, arrastrar por encima del lienzo y soltar: ningún trazo.
        let f = banco.fotograma(texto, false, vec![boton(sobre_la_barra, true)]);
        assert!(
            f.entradas.is_empty(),
            "ppp {ppp}: pulsar en la barra: {:?}",
            f.entradas
        );
        let f = banco.fotograma(texto, false, vec![Event::PointerMoved(sobre_el_pixel)]);
        assert!(
            f.entradas.is_empty(),
            "ppp {ppp}: arrastre desde la barra: {:?}",
            f.entradas
        );
        let f = banco.fotograma(texto, false, vec![boton(sobre_el_pixel, false)]);
        assert!(f.entradas.is_empty(), "ppp {ppp}: soltar: {:?}", f.entradas);
    }
}

#[test]
fn fila_13_pulsar_sobre_el_pixel_2_3_si_empieza_un_trazo_en_ese_pixel() {
    // Contraste con lo anterior: el mismo gesto sobre el lienzo sí da evento de trazo, así que
    // la ausencia de eventos sobre la barra no es un banco que no recoge nada.
    let mut banco = Banco::nuevo(1.0, vec2(400.0, 300.0));
    banco.canvas.set_image(&imagen_de_prueba());
    let texto = "64×48 · lápiz";
    banco.fotograma(texto, false, vec![]);
    let sobre_el_pixel = pos2(52.5, 37.5);
    banco.fotograma(texto, false, vec![Event::PointerMoved(sobre_el_pixel)]);
    let f = banco.fotograma(texto, false, vec![boton(sobre_el_pixel, true)]);
    assert_eq!(
        f.entradas,
        vec![CanvasInput::Stroke(
            px_ui::canvas::CanvasEvent::StrokeBegin((2, 3))
        )]
    );
    let f = banco.fotograma(texto, false, vec![boton(sobre_el_pixel, false)]);
    assert!(
        f.entradas
            .contains(&CanvasInput::Stroke(px_ui::canvas::CanvasEvent::StrokeEnd))
    );
}

// ---------------------------------------------------------------------------------------------
// Fila 16: bar_text y columns
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_16_bar_text_antepone_el_prefijo_solo_a_los_avisos() {
    let banco = Banco::nuevo(1.0, vec2(400.0, 300.0));
    let ctx = &banco.ctx;
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
fn fila_16_bar_text_sustituye_los_caracteres_sin_glifo() {
    let banco = Banco::nuevo(1.0, vec2(400.0, 300.0));
    assert_eq!(
        bar_text(&banco.ctx, "no se pudo leer 日本.png", true, 100),
        "aviso: no se pudo leer ??.png"
    );
}

#[test]
fn fila_16_bar_text_trunca_a_las_columnas_con_el_prefijo_dentro() {
    let banco = Banco::nuevo(1.0, vec2(400.0, 300.0));
    assert_eq!(
        bar_text(&banco.ctx, "se ignora el fichero", true, 10),
        "aviso: se…"
    );
    assert_eq!(bar_text(&banco.ctx, "se ignora el fichero", true, 0), "");
}

#[test]
fn fila_16_columns_es_el_ancho_entre_7_n() {
    // 400 / 7 = 57,14; 800 / 14 = 57,14 (n = 2 con 1,5 y con 2,0).
    assert_eq!(columns(400.0, 1.0), 57);
    assert_eq!(columns(800.0, 1.5), 57);
    assert_eq!(columns(800.0, 2.0), 57);
}

#[test]
fn fila_16_columns_de_un_ancho_no_valido_es_0() {
    assert_eq!(columns(-1.0, 1.0), 0);
    assert_eq!(columns(f32::NAN, 1.0), 0);
    assert_eq!(columns(f32::INFINITY, 1.0), 0);
}

// ---------------------------------------------------------------------------------------------
// Fila 13 de specs/hito-2-limites-relleno.md: el aviso del relleno demasiado fragmentado cabe
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_13_el_aviso_de_relleno_fragmentado_cabe_en_la_barra_a_la_ventana_minima() {
    let texto = DocumentError::FillTooFragmented { limit: 3_000_000 }.to_string();
    let esperado = "aviso: relleno cancelado: región demasiado fragmentada";
    assert_eq!(esperado.chars().count(), 54);
    for ppp in [1.0f32, 1.25, 1.5, 2.0, 3.0] {
        let ancho = min_window_width_pt(ppp);
        let banco = Banco::nuevo(ppp, vec2(ancho, 300.0));
        // Como `show`: ancho útil = (ancho - 2 * margen) píxeles de interfaz, en físicos.
        let margen = 3.0 * ui_unit(ppp);
        let cols = columns((ancho - 2.0 * margen) * ppp, ppp);
        assert!(
            cols >= 54,
            "ppp {ppp}: solo caben {cols} columnas (ancho {ancho} pt)"
        );
        assert_eq!(
            bar_text(&banco.ctx, &texto, true, cols),
            esperado,
            "ppp {ppp}: el texto se recorta o cambia"
        );
    }
}
