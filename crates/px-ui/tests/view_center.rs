//! Tests de «Zoom y centrar» (specs/interfaz-teclado.md, unidad I.1, fila 6): el zoom por las teclas
//! nuevas frente al de antes, `View::center_on` (parte pura) y `Canvas::request_view(
//! ViewOp::CenterOnCursor)` (banco de egui sin ventana, con y sin puntero sobre el lienzo, con otra
//! capa encima y con un trazo a medias). En ROJO hasta la lógica: `ViewOp::CenterOnCursor`,
//! `View::center_on` y la tabla de teclas no existen todavía.
//!
//! - Parte pura. Imagen de 64×48 en un lienzo de 700×500, zooms ×1, ×5 y ×64, con la vista
//!   desplazada o no, y el punto en el centro, sobre una esquina de la imagen y fuera de ella:
//!   el zoom no cambia, la vista queda con `clamp` y el píxel que había bajo el punto pasa a estar
//!   bajo el centro del lienzo. Fuera de la imagen no rompe nada y manda `clamp`.
//!   `View::apply_op(ViewOp::CenterOnCursor)` no la conoce y deja la vista como está.
//! - Zoom por teclas: `>`, `+` y `=` dan `ViewOp::ZoomIn`, `<` y `-` dan `ViewOp::ZoomOut`, `Inicio`
//!   da `Fit` y `Ctrl+1` da `Actual`, y aplicadas a las vistas dan lo mismo que el modelo de
//!   `view_ops.rs` (`set_zoom` en el centro y `clamp`).
//! - Banco (copiando el estilo de `view_ops.rs` y `canvas_layers.rs`): con el puntero sobre el
//!   lienzo, el píxel bajo el cursor pasa al centro y el zoom no cambia; sin puntero, con el
//!   puntero fuera del lienzo o con otra capa encima, la vista no cambia; con un trazo a medias
//!   mueve la vista y el trazo sigue.
//!
//! Suposiciones sobre lo que la spec no fija (anotadas en el informe):
//! - `View::center_on(point, viewport, image)` recibe `point` en **píxeles de pantalla relativos
//!   al lienzo** (como `screen_to_image` y `set_zoom(anchor)`), no en coordenadas de imagen.
//! - «El píxel bajo el punto queda en el centro» se comprueba por igualdad del índice de píxel
//!   bajo el centro del lienzo (antes: bajo el punto) cuando el punto cae sobre la imagen; la
//!   spec admite ±1 px y la igualdad lo cumple.
//! - `request_view(CenterOnCursor)` se resuelve en el siguiente `interact` con la última posición
//!   del puntero (`latest_pos`); sin ninguna posición no hace nada.

use egui::{Area, Context, Event, Id, Order, Pos2, Rect, Vec2, pos2, vec2};
use px_core::{Image, Mode};
use px_ui::actions::{AppAction, Platform};
use px_ui::canvas::{Canvas, CanvasEvent, CanvasInfo, CanvasInput};
use px_ui::keymap::{KeyId, Keymap, Press};
use px_ui::view::{Size, View, ViewOp, Zoom};

mod common;
use common::{PPPS, boton, contexto, fotograma, mover};

const LIENZO: Size = Size {
    width: 700,
    height: 500,
};
const IMAGEN: Size = Size {
    width: 64,
    height: 48,
};
const CENTRO: (i64, i64) = (350, 250);
const ZOOMS: [u32; 3] = [1, 5, 64];
const DESPLAZAMIENTOS: [(i64, i64); 5] = [(0, 0), (7, -5), (-20, 15), (40, 30), (-45, -40)];

fn zoom(n: u32) -> Zoom {
    Zoom::from_int(n).unwrap_or_else(|| panic!("el zoom {n} debería existir"))
}

/// Vista a ×`n` anclada en el centro, con `clamp`, y desplazada `(dx, dy)` (con `pan`).
fn vista(n: u32, desplazamiento: (i64, i64)) -> View {
    let mut v = View::fit(LIENZO, IMAGEN);
    v.set_zoom(zoom(n), CENTRO);
    v.clamp(LIENZO, IMAGEN);
    v.pan(desplazamiento.0, desplazamiento.1, LIENZO, IMAGEN);
    v
}

fn con_clamp(mut v: View) -> View {
    v.clamp(LIENZO, IMAGEN);
    v
}

fn centrada(mut v: View, punto: (i64, i64)) -> View {
    v.center_on(punto, LIENZO, IMAGEN);
    v
}

/// Todas las vistas de prueba: ×1, ×5 y ×64, sin desplazar y desplazadas.
fn vistas() -> Vec<(String, View)> {
    let mut v = Vec::new();
    for n in ZOOMS {
        for d in DESPLAZAMIENTOS {
            v.push((format!("×{n} desplazada {d:?}"), vista(n, d)));
        }
    }
    v
}

/// El centro del bloque del píxel `(x, y)` en pantalla.
fn centro_del_pixel(v: &View, x: u32, y: u32) -> (i64, i64) {
    let (min, max) = v.pixel_block(x, y).expect("zoom entero: hay bloque");
    ((min.0 + max.0) / 2, (min.1 + max.1) / 2)
}

// ---------------------------------------------------------------------------------------------
// Zoom por las teclas nuevas frente al de antes
// ---------------------------------------------------------------------------------------------

fn press_de(key: KeyId, command: bool) -> Press {
    Press {
        key: Some(key),
        command,
        shift: false,
        alt: false,
        other_ctrl: false,
        repeat: false,
    }
}

fn op_de(action: AppAction) -> ViewOp {
    match action {
        AppAction::ZoomIn => ViewOp::ZoomIn,
        AppAction::ZoomOut => ViewOp::ZoomOut,
        AppAction::ZoomFit => ViewOp::Fit,
        AppAction::ZoomActual => ViewOp::Actual,
        otra => panic!("{otra:?} no es una acción de zoom"),
    }
}

/// Modelo de `apply_op` de antes de la unidad, escrito con `set_zoom` y `clamp`.
fn modelo(v: View, op: ViewOp) -> View {
    let mut m = v;
    match op {
        ViewOp::ZoomIn => m.set_zoom(v.zoom().zoom_in(), CENTRO),
        ViewOp::ZoomOut => m.set_zoom(v.zoom().zoom_out(), CENTRO),
        ViewOp::Actual => m.set_zoom(Zoom::ONE, CENTRO),
        ViewOp::Fit => return View::fit(LIENZO, IMAGEN),
        otra => panic!("{otra:?} no es una operación de zoom"),
    }
    con_clamp(m)
}

#[test]
fn fila_6_mayor_menos_y_mas_igual_menos_dan_zoom_in_y_zoom_out_como_antes() {
    let km = Keymap::default_scheme();
    for platform in [Platform::Linux, Platform::Windows, Platform::Mac] {
        for (tecla, esperada) in [
            ('>', ViewOp::ZoomIn),
            ('+', ViewOp::ZoomIn),
            ('=', ViewOp::ZoomIn),
            ('<', ViewOp::ZoomOut),
            ('-', ViewOp::ZoomOut),
        ] {
            let action = km
                .action_for(&press_de(KeyId::Char(tecla), false), platform)
                .unwrap_or_else(|| panic!("`{tecla}` no da acción en {platform:?}"));
            assert_eq!(op_de(action), esperada, "`{tecla}` en {platform:?}");
        }
        let fit = km.action_for(&press_de(KeyId::Home, false), platform);
        assert_eq!(fit.map(op_de), Some(ViewOp::Fit), "{platform:?}");
        let actual = km.action_for(&press_de(KeyId::Char('1'), true), platform);
        assert_eq!(actual.map(op_de), Some(ViewOp::Actual), "{platform:?}");
    }
}

#[test]
fn fila_6_las_teclas_de_zoom_aplicadas_a_la_vista_dan_lo_mismo_que_el_modelo_de_antes() {
    let km = Keymap::default_scheme();
    let teclas = [
        (KeyId::Char('>'), false),
        (KeyId::Char('+'), false),
        (KeyId::Char('='), false),
        (KeyId::Char('<'), false),
        (KeyId::Char('-'), false),
        (KeyId::Home, false),
        (KeyId::Char('1'), true),
    ];
    for (nombre, antes) in vistas() {
        for (key, command) in teclas {
            let action = km
                .action_for(&press_de(key, command), Platform::Linux)
                .expect("la tecla da una acción");
            let op = op_de(action);
            let mut despues = antes;
            despues.apply_op(op, LIENZO, IMAGEN);
            assert_eq!(despues, modelo(antes, op), "{nombre}: {key:?}");
        }
        // `>`, `+` y `=` son la misma operación: el mismo resultado.
        let por = |tecla: char| {
            let action = km
                .action_for(&press_de(KeyId::Char(tecla), false), Platform::Linux)
                .expect("acción");
            let mut v = antes;
            v.apply_op(op_de(action), LIENZO, IMAGEN);
            v
        };
        assert_eq!(por('>'), por('+'), "{nombre}");
        assert_eq!(por('>'), por('='), "{nombre}");
        assert_eq!(por('<'), por('-'), "{nombre}");
    }
}

#[test]
fn fila_6_mayor_acerca_un_nivel_y_menor_aleja_un_nivel() {
    let km = Keymap::default_scheme();
    for (nombre, antes) in vistas() {
        let aplica = |tecla: char| {
            let action = km
                .action_for(&press_de(KeyId::Char(tecla), false), Platform::Linux)
                .expect("acción");
            let mut v = antes;
            v.apply_op(op_de(action), LIENZO, IMAGEN);
            v
        };
        assert_eq!(
            aplica('>').zoom(),
            antes.zoom().zoom_in(),
            "{nombre}: `>` acerca"
        );
        assert_eq!(
            aplica('<').zoom(),
            antes.zoom().zoom_out(),
            "{nombre}: `<` aleja"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// View::center_on (parte pura)
// ---------------------------------------------------------------------------------------------

#[test]
fn view_op_center_on_cursor_es_un_tipo_plano() {
    fn plano<T: Clone + Copy + std::fmt::Debug + PartialEq + Eq>() {}
    plano::<ViewOp>();
    assert_ne!(ViewOp::CenterOnCursor, ViewOp::Fit);
}

#[test]
fn fila_6_center_on_no_cambia_el_zoom() {
    let puntos = [CENTRO, (0, 0), (699, 499), (-500, -500), (5000, 5000)];
    for (nombre, antes) in vistas() {
        for punto in puntos {
            assert_eq!(
                centrada(antes, punto).zoom(),
                antes.zoom(),
                "{nombre}: punto {punto:?}"
            );
        }
    }
}

#[test]
fn fila_6_center_on_deja_la_vista_con_clamp_en_todos_los_puntos() {
    let puntos = [
        CENTRO,
        (0, 0),
        (699, 499),
        (0, 499),
        (699, 0),
        (-500, -500),
        (5000, 5000),
        (-100_000, 100_000),
        (1_000_000_000, -1_000_000_000),
    ];
    for (nombre, antes) in vistas() {
        for punto in puntos {
            let despues = centrada(antes, punto);
            assert_eq!(
                despues,
                con_clamp(despues),
                "{nombre}: punto {punto:?} no deja la vista con clamp"
            );
        }
    }
}

#[test]
fn fila_6_el_pixel_bajo_el_punto_pasa_a_estar_bajo_el_centro_del_lienzo() {
    for (nombre, antes) in vistas() {
        // El punto en el centro del lienzo, si cae sobre la imagen.
        let mut puntos: Vec<(String, (i64, i64))> = vec![("el centro".to_string(), CENTRO)];
        // El centro de cada una de las cuatro esquinas de la imagen y del píxel del medio.
        for (x, y) in [(0, 0), (63, 0), (0, 47), (63, 47), (32, 24)] {
            puntos.push((
                format!("el píxel ({x}, {y})"),
                centro_del_pixel(&antes, x, y),
            ));
        }
        for (descripcion, punto) in puntos {
            let Some(pixel) = antes.screen_to_image(punto, IMAGEN) else {
                // Sobre un punto fuera de la imagen no hay píxel que mantener (véase abajo).
                continue;
            };
            let despues = centrada(antes, punto);
            assert_eq!(
                despues.screen_to_image(CENTRO, IMAGEN),
                Some(pixel),
                "{nombre}: {descripcion} ({punto:?}) no quedó bajo el centro"
            );
        }
    }
}

#[test]
fn fila_6_con_el_punto_ya_en_el_centro_el_pixel_bajo_el_centro_no_cambia() {
    for (nombre, antes) in vistas() {
        if let Some(pixel) = antes.screen_to_image(CENTRO, IMAGEN) {
            assert_eq!(
                centrada(antes, CENTRO).screen_to_image(CENTRO, IMAGEN),
                Some(pixel),
                "{nombre}"
            );
        }
    }
}

#[test]
fn fila_6_con_un_punto_sobre_la_imagen_no_hace_falta_clamp_y_el_centrado_es_completo() {
    // Si el punto está sobre la imagen, centrarlo nunca saca la imagen del lienzo: el resultado
    // es el de desplazar la vista lo que falta para llevar el punto al centro, sin recorte
    // (±1 px de redondeo).
    for (nombre, antes) in vistas() {
        let punto = centro_del_pixel(&antes, 10, 10);
        if antes.screen_to_image(punto, IMAGEN).is_none() {
            continue;
        }
        let despues = centrada(antes, punto);
        let (bx, by) = centro_del_pixel(&despues, 10, 10);
        let z = antes.zoom().scale(1);
        assert!(
            (bx - CENTRO.0).abs() <= z.max(1) && (by - CENTRO.1).abs() <= z.max(1),
            "{nombre}: el píxel (10, 10) quedó en ({bx}, {by}) y el centro es {CENTRO:?}"
        );
    }
}

#[test]
fn fila_6_fuera_de_la_imagen_no_rompe_nada_y_el_zoom_no_cambia() {
    let fuera = [
        (-500, -500),
        (-1, -1),
        (699, 499),
        (5000, 5000),
        (-100_000, 7),
        (1_000_000_000, -1_000_000_000),
    ];
    for (nombre, antes) in vistas() {
        for punto in fuera {
            let despues = centrada(antes, punto);
            assert_eq!(despues.zoom(), antes.zoom(), "{nombre}: {punto:?}");
            // `clamp` manda: la imagen sigue visible en el lienzo.
            assert_eq!(despues, con_clamp(despues), "{nombre}: {punto:?}");
            assert!(
                despues.image_rect(IMAGEN).width > 0,
                "{nombre}: {punto:?}: la imagen existe"
            );
        }
    }
}

#[test]
fn fila_6_center_on_con_un_lienzo_o_una_imagen_vacios_no_entra_en_panico() {
    for (viewport, imagen) in [
        (Size::new(0, 0), IMAGEN),
        (LIENZO, Size::new(0, 0)),
        (Size::new(0, 0), Size::new(0, 0)),
        (Size::new(1, 1), Size::new(1, 1)),
    ] {
        let mut v = View::fit(viewport, imagen);
        v.center_on((10, 10), viewport, imagen);
        let mut clamped = v;
        clamped.clamp(viewport, imagen);
        assert_eq!(v, clamped, "{viewport:?} con {imagen:?}");
    }
}

#[test]
fn fila_6_apply_op_no_conoce_center_on_cursor_y_deja_la_vista_como_esta() {
    // La operación necesita el cursor: la resuelve el lienzo con `center_on`. `apply_op` la deja.
    for (nombre, antes) in vistas() {
        let mut despues = antes;
        despues.apply_op(ViewOp::CenterOnCursor, LIENZO, IMAGEN);
        assert_eq!(despues, con_clamp(antes), "{nombre}");
    }
}

// ---------------------------------------------------------------------------------------------
// Canvas::request_view(CenterOnCursor) en el banco de egui
// ---------------------------------------------------------------------------------------------

/// Pantalla de 800×600 puntos y imagen RGBA de 64×48: a `ppp` 1,0 el ajuste es ×12 y el lienzo es
/// la pantalla entera.
const PANTALLA: Vec2 = vec2(800.0, 600.0);
const CENTRO_DE_PANTALLA: Pos2 = pos2(400.0, 300.0);
const POS_DEL_AREA: Pos2 = pos2(300.0, 200.0);

fn imagen() -> Image {
    Image::new(64, 48, Mode::Rgba).expect("imagen válida")
}

struct Banco {
    ctx: Context,
    canvas: Canvas,
    /// Con un `Area` encima del lienzo (como un desplegable del menú).
    area: Option<Id>,
    boton: Rect,
}

impl Banco {
    fn nuevo(ppp: f32) -> Banco {
        let mut canvas = Canvas::new();
        canvas.set_image(&imagen());
        let mut banco = Banco {
            ctx: contexto(ppp),
            canvas,
            area: None,
            boton: Rect::NOTHING,
        };
        banco.reposo();
        banco
    }

    fn con_area(ppp: f32) -> Banco {
        let mut banco = Banco::nuevo(ppp);
        banco.area = Some(Id::unique("desplegable"));
        for _ in 0..3 {
            banco.fotograma(vec![]);
        }
        banco
    }

    fn fotograma(&mut self, events: Vec<Event>) -> (CanvasInfo, Vec<CanvasInput>) {
        let mut resultado = None;
        let canvas = &mut self.canvas;
        let area = self.area;
        let mut boton_rect = self.boton;
        fotograma(&self.ctx, PANTALLA, events, |ui| {
            if let Some(id) = area {
                let respuesta = Area::new(id)
                    .order(Order::Foreground)
                    .fixed_pos(POS_DEL_AREA)
                    .show(ui.ctx(), |ui| ui.button("Guardar"));
                boton_rect = respuesta.inner.rect;
            }
            let entradas = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            let info = canvas.paint(ui);
            resultado = Some((info, entradas));
        });
        self.boton = boton_rect;
        resultado.expect("el lienzo se dibuja")
    }

    fn info(&mut self, events: Vec<Event>) -> CanvasInfo {
        self.fotograma(events).0
    }

    fn reposo(&mut self) -> CanvasInfo {
        self.info(vec![]);
        self.info(vec![])
    }

    /// Mueve el puntero a `pos` y deja un fotograma de reposo.
    fn puntero_en(&mut self, pos: Pos2) -> CanvasInfo {
        self.info(vec![mover(pos)]);
        self.info(vec![])
    }

    /// Pide centrar y deja pasar un fotograma.
    fn centra(&mut self) -> CanvasInfo {
        self.canvas.request_view(ViewOp::CenterOnCursor);
        self.info(vec![])
    }
}

#[test]
fn fila_6_request_view_se_aplica_en_el_siguiente_interact_y_no_antes() {
    let mut banco = Banco::nuevo(1.0);
    banco.puntero_en(pos2(100.0, 100.0));
    let antes = banco.info(vec![]);
    banco.canvas.request_view(ViewOp::CenterOnCursor);
    assert_eq!(
        banco.canvas.zoom(),
        Some(antes.zoom),
        "pedirlo no cambia la vista: lo hace el siguiente interact"
    );
    let despues = banco.info(vec![]);
    assert_ne!(
        despues.image_rect, antes.image_rect,
        "se aplica en ese fotograma"
    );
    // Y una sola vez: otro fotograma no la mueve más.
    let otra = banco.info(vec![]);
    assert_eq!(otra.image_rect, despues.image_rect);
}

#[test]
fn fila_6_con_el_puntero_sobre_el_lienzo_el_pixel_bajo_el_cursor_pasa_al_centro_y_el_zoom_no_cambia()
 {
    for ppp in PPPS {
        for cursor in [
            pos2(100.0, 100.0),
            pos2(250.0, 450.0),
            pos2(600.0, 150.0),
            pos2(400.0, 300.0),
        ] {
            let mut banco = Banco::nuevo(ppp);
            let antes = banco.puntero_en(cursor);
            let Some(pixel) = antes.hover else {
                // El cursor cae fuera de la imagen a esta escala: nada que comprobar.
                continue;
            };
            let despues = banco.centra();
            assert_eq!(
                despues.zoom, antes.zoom,
                "ppp {ppp}, cursor {cursor:?}: mismo zoom"
            );
            // Lo que había bajo el cursor está ahora bajo el centro del lienzo: se comprueba
            // llevando el puntero allí.
            let en_el_centro = banco.puntero_en(CENTRO_DE_PANTALLA);
            assert_eq!(
                en_el_centro.hover,
                Some(pixel),
                "ppp {ppp}, cursor {cursor:?}: el píxel {pixel:?} no quedó en el centro"
            );
            assert_eq!(en_el_centro.zoom, antes.zoom, "ppp {ppp}");
        }
    }
}

#[test]
fn fila_6_con_el_puntero_ya_en_el_centro_el_pixel_bajo_el_centro_no_cambia() {
    let mut banco = Banco::nuevo(1.0);
    let antes = banco.puntero_en(CENTRO_DE_PANTALLA);
    let pixel = antes.hover.expect("el centro está sobre la imagen");
    let despues = banco.centra();
    assert_eq!(despues.zoom, antes.zoom);
    assert_eq!(despues.hover, Some(pixel));
}

#[test]
fn fila_6_sin_puntero_sobre_el_lienzo_la_vista_no_cambia() {
    for ppp in PPPS {
        // Nunca hubo puntero.
        let mut banco = Banco::nuevo(ppp);
        let antes = banco.reposo();
        let despues = banco.centra();
        assert_eq!(
            despues.image_rect, antes.image_rect,
            "ppp {ppp}: sin puntero"
        );
        assert_eq!(despues.zoom, antes.zoom, "ppp {ppp}");

        // El puntero fuera del lienzo (a la derecha de la pantalla).
        let mut banco = Banco::nuevo(ppp);
        banco.puntero_en(pos2(900.0, 300.0));
        let antes = banco.reposo();
        let despues = banco.centra();
        assert_eq!(
            despues.image_rect, antes.image_rect,
            "ppp {ppp}: puntero fuera del lienzo"
        );
        assert_eq!(despues.zoom, antes.zoom, "ppp {ppp}");
    }
}

#[test]
fn fila_6_sin_imagen_pedir_centrar_no_rompe_nada() {
    let mut banco = Banco {
        ctx: contexto(1.0),
        canvas: Canvas::new(),
        area: None,
        boton: Rect::NOTHING,
    };
    banco.puntero_en(pos2(100.0, 100.0));
    banco.canvas.request_view(ViewOp::CenterOnCursor);
    banco.info(vec![]);
    assert_eq!(banco.canvas.zoom(), None);
}

#[test]
fn fila_6_con_otra_capa_encima_del_cursor_la_vista_no_cambia() {
    let mut banco = Banco::con_area(1.0);
    assert!(banco.boton.is_positive(), "el área se pintó");
    let sobre_el_area = banco.boton.center();
    banco.puntero_en(sobre_el_area);
    let antes = banco.reposo();
    let despues = banco.centra();
    assert_eq!(
        despues.image_rect, antes.image_rect,
        "con el cursor sobre un desplegable no se centra"
    );
    // Con el cursor fuera del área sí se centra (el control de la prueba).
    banco.puntero_en(pos2(100.0, 100.0));
    let antes = banco.reposo();
    let despues = banco.centra();
    assert_ne!(despues.image_rect, antes.image_rect);
}

#[test]
fn fila_6_con_un_trazo_a_medias_n_mueve_la_vista_y_el_trazo_sigue() {
    let cursor = pos2(100.0, 100.0);
    let mut banco = Banco::nuevo(1.0);
    banco.puntero_en(cursor);
    let antes = banco.reposo();

    // Empieza el trazo: pulsa sin soltar.
    let (_, empezar) = banco.fotograma(vec![boton(cursor, true)]);
    let origen = match empezar.as_slice() {
        [CanvasInput::Stroke(CanvasEvent::StrokeBegin(p))] => *p,
        otra => panic!("se esperaba un StrokeBegin y salió {otra:?}"),
    };

    // Centra a mitad del trazo.
    banco.canvas.request_view(ViewOp::CenterOnCursor);
    let (info, entradas) = banco.fotograma(vec![]);
    assert!(
        !entradas
            .iter()
            .any(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeEnd))),
        "centrar no corta el trazo: {entradas:?}"
    );
    assert_ne!(
        info.image_rect, antes.image_rect,
        "la vista se movió con el trazo a medias"
    );
    assert_eq!(info.zoom, antes.zoom);

    // El trazo sigue: el siguiente movimiento usa la vista nueva (otro píxel bajo el mismo punto).
    let (_, movimiento) = banco.fotograma(vec![mover(pos2(101.0, 100.0))]);
    let destino = movimiento
        .iter()
        .find_map(|e| match e {
            CanvasInput::Stroke(CanvasEvent::StrokeMove(p)) => Some(*p),
            _ => None,
        })
        .expect("el trazo sigue y da un StrokeMove");
    assert_ne!(
        destino, origen,
        "el movimiento usa la vista nueva: {origen:?} → {destino:?}"
    );

    // Y termina como siempre, al soltar.
    let (_, soltar) = banco.fotograma(vec![boton(pos2(101.0, 100.0), false)]);
    assert_eq!(
        soltar.last(),
        Some(&CanvasInput::Stroke(CanvasEvent::StrokeEnd))
    );
}

#[test]
fn fila_6_centrar_con_un_trazo_a_medias_no_cambia_el_pixel_que_se_dibuja_bajo_el_cursor_del_centro()
{
    // Tras centrar, llevar el cursor al centro da el mismo píxel que había bajo el cursor antes.
    let cursor = pos2(100.0, 100.0);
    let mut banco = Banco::nuevo(1.0);
    let antes = banco.puntero_en(cursor);
    let pixel = antes.hover.expect("sobre la imagen");
    banco.fotograma(vec![boton(cursor, true)]);
    banco.canvas.request_view(ViewOp::CenterOnCursor);
    banco.fotograma(vec![]);
    let (info, _) = banco.fotograma(vec![mover(CENTRO_DE_PANTALLA)]);
    assert_eq!(info.hover, Some(pixel));
    banco.fotograma(vec![boton(CENTRO_DE_PANTALLA, false)]);
}

#[test]
fn fila_6_las_demas_operaciones_de_zoom_siguen_igual_con_la_nueva_en_la_cola() {
    // `CenterOnCursor` sin puntero no estropea las operaciones pedidas en el mismo fotograma.
    let mut banco = Banco::nuevo(1.0);
    let ajuste = banco.reposo().zoom;
    banco.canvas.request_view(ViewOp::CenterOnCursor);
    banco.canvas.request_view(ViewOp::ZoomIn);
    let despues = banco.info(vec![]);
    assert_eq!(despues.zoom, ajuste.zoom_in());
}
