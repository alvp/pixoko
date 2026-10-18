//! Tests de la guarda de capa de `Canvas` y del zoom a mitad de trazo (specs/hito-2-menu-herramientas.md,
//! «Casos» fila 9b y la parte de banco de la 5b, y decisión 6).
//!
//! Banco de egui sin ventana (`Context::run_ui` con `RawInput` sintético), a las cinco escalas.
//!
//! - **Fila 9b.** El lienzo con, encima, un `egui::Area` en `Order::Foreground` (y otra prueba con
//!   `Order::Middle`) con un botón que cae sobre él, como un desplegable del menú. Con el área
//!   abierta: el clic **no** produce `StrokeBegin` ni `StrokeMove` (y llega al botón), la rueda y
//!   el pellizco **no cambian el zoom** y `hover` es `None` sobre el área; un trazo empezado
//!   **antes** de abrir el área sigue y termina con `StrokeEnd` aunque el puntero pase por el área.
//!   Con el área cerrada todo funciona como antes (clic dibuja, rueda hace zoom, `hover` válido).
//!   Además, la misma prueba con el `menu_button` real de `show_menu` abierto sobre el lienzo.
//! - **Fila 5b (banco).** Un trazo a medias con `Canvas::request_view(ZoomIn)` y `ToolState`: el
//!   zoom cambia, **el trazo sigue** (el siguiente `StrokeMove` usa la vista nueva), el documento
//!   no cambia hasta soltar y el `StrokeEnd` crea **un solo paso**. La rueda no cambia.
//!
//! El área se muestra **antes** del lienzo en el fotograma (como el menú en la app) y, como egui
//! decide qué capa está bajo el puntero con las áreas del fotograma anterior, el banco deja pasar
//! dos fotogramas con el área abierta antes de pulsar.

use egui::{Area, Context, Event, Id, Order, Pos2, Rect, Vec2, pos2, vec2};
use px_core::{Document, Image, Mode, Pixel, Rgba};
use px_ui::actions::{AppAction, Locks, Platform, UiState};
use px_ui::canvas::{Canvas, CanvasEvent, CanvasInfo, CanvasInput};
use px_ui::menu::show_menu;
use px_ui::tool::{Tool, ToolState};
use px_ui::view::{ViewOp, Zoom};

mod common;
use common::{PPPS, boton, contexto, en_puntos, fotograma, mover, n_de, rueda_linea};

const PANTALLA: Vec2 = vec2(800.0, 600.0);
const POS_DEL_AREA: Pos2 = pos2(300.0, 200.0);
const ORDENES: [Order; 2] = [Order::Foreground, Order::Middle];

fn imagen() -> Image {
    Image::new(64, 48, Mode::Rgba).expect("imagen válida")
}

struct Paso {
    entradas: Vec<CanvasInput>,
    info: CanvasInfo,
    /// El botón del área recibió un clic en este fotograma.
    clic_en_el_boton: bool,
}

impl Paso {
    fn hay_trazo(&self) -> bool {
        self.entradas
            .iter()
            .any(|e| matches!(e, CanvasInput::Stroke(_)))
    }

    fn empieza(&self) -> bool {
        self.entradas
            .iter()
            .any(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeBegin(_))))
    }

    fn termina(&self) -> bool {
        self.entradas
            .iter()
            .any(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeEnd)))
    }

    fn movimientos(&self) -> Vec<(i32, i32)> {
        self.entradas
            .iter()
            .filter_map(|e| match e {
                CanvasInput::Stroke(CanvasEvent::StrokeMove(p)) => Some(*p),
                _ => None,
            })
            .collect()
    }
}

struct Banco {
    ctx: Context,
    canvas: Canvas,
    id_del_area: Id,
    orden: Order,
    abierta: bool,
    /// Rectángulo del botón del área en el último fotograma (puntos).
    boton: Rect,
}

impl Banco {
    fn nuevo(ppp: f32, orden: Order, abierta: bool) -> Banco {
        let mut canvas = Canvas::new();
        canvas.set_image(&imagen());
        let mut banco = Banco {
            ctx: contexto(ppp),
            canvas,
            id_del_area: Id::unique("desplegable"),
            orden,
            abierta,
            boton: Rect::NOTHING,
        };
        // Reparte el espacio, sube la textura y deja el área en su sitio.
        for _ in 0..3 {
            banco.paso(vec![]);
        }
        banco
    }

    fn paso(&mut self, events: Vec<Event>) -> Paso {
        let mut resultado = None;
        let mut boton_rect = self.boton;
        let (orden, abierta, id) = (self.orden, self.abierta, self.id_del_area);
        let canvas = &mut self.canvas;
        fotograma(&self.ctx, PANTALLA, events, |ui| {
            let mut clic = false;
            if abierta {
                let respuesta = Area::new(id)
                    .order(orden)
                    .fixed_pos(POS_DEL_AREA)
                    .show(ui.ctx(), |ui| ui.button("Guardar"));
                clic = respuesta.inner.clicked();
                boton_rect = respuesta.inner.rect;
            }
            let entradas = canvas.interact_ordered(ui);
            let info = canvas.paint(ui, std::iter::empty());
            resultado = Some(Paso {
                entradas,
                info,
                clic_en_el_boton: clic,
            });
        });
        self.boton = boton_rect;
        resultado.expect("el lienzo se dibuja")
    }

    fn zoom(&mut self) -> Zoom {
        self.paso(vec![]).info.zoom
    }

    /// Centro del botón del área (un punto sobre el área).
    fn sobre_el_area(&self) -> Pos2 {
        assert!(self.boton.is_positive(), "el área no se ha pintado");
        self.boton.center()
    }

    /// Un punto del lienzo (sobre la imagen) que no toca el área.
    fn fuera_del_area(&self) -> Pos2 {
        pos2(100.0, 100.0)
    }

    /// Mueve, pulsa y suelta en `pos`; devuelve los tres pasos.
    fn clic(&mut self, pos: Pos2) -> [Paso; 3] {
        let a = self.paso(vec![mover(pos)]);
        let b = self.paso(vec![boton(pos, true)]);
        let c = self.paso(vec![boton(pos, false)]);
        [a, b, c]
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9b: con el área abierta
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_9b_el_area_se_pinta_encima_del_lienzo_y_cubre_parte_de_la_imagen() {
    for ppp in PPPS {
        for orden in ORDENES {
            let banco = Banco::nuevo(ppp, orden, true);
            let rect = banco.boton;
            assert!(rect.is_positive(), "ppp {ppp} {orden:?}");
            let pantalla = Rect::from_min_size(Pos2::ZERO, PANTALLA);
            assert!(
                pantalla.contains_rect(rect),
                "ppp {ppp} {orden:?}: {rect:?}"
            );
            // Cae sobre la imagen: el lienzo ocupa toda la pantalla.
            let imagen = banco.canvas.zoom().is_some();
            assert!(imagen);
        }
    }
}

#[test]
fn fila_9b_con_el_area_abierta_el_clic_no_empieza_trazo_y_llega_al_boton() {
    for ppp in PPPS {
        for orden in ORDENES {
            let mut banco = Banco::nuevo(ppp, orden, true);
            let pos = banco.sobre_el_area();
            let pasos = banco.clic(pos);
            for (i, paso) in pasos.iter().enumerate() {
                assert!(
                    !paso.hay_trazo(),
                    "ppp {ppp} {orden:?}: el paso {i} produjo {:?}",
                    paso.entradas
                );
                assert_eq!(paso.info.hover, None, "ppp {ppp} {orden:?}: paso {i}");
            }
            assert!(
                pasos.iter().any(|p| p.clic_en_el_boton),
                "ppp {ppp} {orden:?}: el clic no llegó al botón"
            );
        }
    }
}

#[test]
fn fila_9b_con_el_area_abierta_hover_es_none_sobre_ella_y_valido_fuera() {
    for ppp in PPPS {
        for orden in ORDENES {
            let mut banco = Banco::nuevo(ppp, orden, true);
            let sobre = banco.sobre_el_area();
            let fuera = banco.fuera_del_area();
            assert_eq!(banco.paso(vec![mover(sobre)]).info.hover, None);
            assert_eq!(banco.paso(vec![]).info.hover, None);
            let paso = banco.paso(vec![mover(fuera)]);
            assert!(
                paso.info.hover.is_some(),
                "ppp {ppp} {orden:?}: fuera del área hay hover"
            );
            // Y vuelve a ser `None` al regresar al área.
            assert_eq!(banco.paso(vec![mover(sobre)]).info.hover, None);
        }
    }
}

#[test]
fn fila_9b_con_el_area_abierta_la_rueda_no_cambia_el_zoom() {
    for ppp in PPPS {
        for orden in ORDENES {
            let mut banco = Banco::nuevo(ppp, orden, true);
            let antes = banco.zoom();
            let sobre = banco.sobre_el_area();
            banco.paso(vec![mover(sobre)]);
            banco.paso(vec![]);
            for arriba in [true, false] {
                let paso = banco.paso(vec![rueda_linea(arriba), rueda_linea(arriba)]);
                assert_eq!(
                    paso.info.zoom, antes,
                    "ppp {ppp} {orden:?}: rueda arriba={arriba}"
                );
            }
            assert_eq!(banco.zoom(), antes);
            // Fuera del área la misma rueda sí hace zoom.
            let fuera = banco.fuera_del_area();
            banco.paso(vec![mover(fuera)]);
            let paso = banco.paso(vec![rueda_linea(true)]);
            assert_eq!(
                paso.info.zoom,
                antes.zoom_in(),
                "ppp {ppp} {orden:?}: fuera del área"
            );
        }
    }
}

#[test]
fn fila_9b_con_el_area_abierta_el_pellizco_no_cambia_el_zoom() {
    for ppp in PPPS {
        for orden in ORDENES {
            let mut banco = Banco::nuevo(ppp, orden, true);
            let antes = banco.zoom();
            let sobre = banco.sobre_el_area();
            banco.paso(vec![mover(sobre)]);
            banco.paso(vec![]);
            for factor in [1.3, 0.7] {
                let paso = banco.paso(vec![Event::Zoom(factor)]);
                assert_eq!(
                    paso.info.zoom, antes,
                    "ppp {ppp} {orden:?}: pellizco {factor}"
                );
            }
            let fuera = banco.fuera_del_area();
            banco.paso(vec![mover(fuera)]);
            let paso = banco.paso(vec![Event::Zoom(1.3)]);
            assert_eq!(
                paso.info.zoom,
                antes.zoom_in(),
                "ppp {ppp} {orden:?}: fuera del área"
            );
        }
    }
}

#[test]
fn fila_9b_un_trazo_empezado_antes_de_abrir_el_area_sigue_y_termina_con_stroke_end() {
    for ppp in PPPS {
        for orden in ORDENES {
            let mut banco = Banco::nuevo(ppp, orden, false);
            let inicio = banco.fuera_del_area();
            banco.paso(vec![mover(inicio)]);
            let empezar = banco.paso(vec![boton(inicio, true)]);
            assert!(
                empezar.empieza(),
                "ppp {ppp} {orden:?}: con el área cerrada el clic dibuja"
            );

            // Se abre el área a mitad del gesto y se deja asentar (el puntero no se mueve).
            banco.abierta = true;
            banco.paso(vec![]);
            banco.paso(vec![]);
            let sobre = banco.sobre_el_area();

            // El puntero pasa por encima del área: el trazo sigue.
            let paso = banco.paso(vec![mover(sobre)]);
            assert!(
                !paso.movimientos().is_empty(),
                "ppp {ppp} {orden:?}: el trazo debe seguir sobre el área: {:?}",
                paso.entradas
            );
            assert!(!paso.empieza() && !paso.termina());
            // Soltar sobre el área termina el trazo (y no es un clic del botón).
            let soltar = banco.paso(vec![boton(sobre, false)]);
            assert!(
                soltar.termina(),
                "ppp {ppp} {orden:?}: {:?}",
                soltar.entradas
            );
            assert_eq!(
                soltar
                    .entradas
                    .iter()
                    .filter(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeEnd)))
                    .count(),
                1
            );
            assert!(
                !soltar.clic_en_el_boton,
                "ppp {ppp} {orden:?}: no es un clic del botón"
            );
            // Y el gesto terminó: un movimiento más no dibuja.
            let despues = banco.paso(vec![mover(inicio)]);
            assert!(
                !despues.hay_trazo(),
                "ppp {ppp} {orden:?}: {:?}",
                despues.entradas
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9b: con el área cerrada todo funciona como antes
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_9b_con_el_area_cerrada_el_clic_dibuja_la_rueda_hace_zoom_y_hover_es_valido() {
    for ppp in PPPS {
        // El área está cerrada: el mismo recorrido que en las pruebas de arriba.
        let mut banco = Banco::nuevo(ppp, Order::Foreground, false);
        // El punto donde estaría el botón.
        let pos = pos2(POS_DEL_AREA.x + 10.0, POS_DEL_AREA.y + 8.0);

        let a = banco.paso(vec![mover(pos)]);
        assert!(a.info.hover.is_some(), "ppp {ppp}: hover válido");
        let b = banco.paso(vec![boton(pos, true)]);
        assert!(b.empieza(), "ppp {ppp}: el clic dibuja: {:?}", b.entradas);
        let c = banco.paso(vec![boton(pos, false)]);
        assert!(c.termina(), "ppp {ppp}");

        let antes = banco.zoom();
        let paso = banco.paso(vec![rueda_linea(true)]);
        assert_eq!(
            paso.info.zoom,
            antes.zoom_in(),
            "ppp {ppp}: la rueda hace zoom"
        );
        let antes = banco.zoom();
        let paso = banco.paso(vec![Event::Zoom(1.3)]);
        assert_eq!(
            paso.info.zoom,
            antes.zoom_in(),
            "ppp {ppp}: el pellizco hace zoom"
        );
    }
}

#[test]
fn fila_9b_al_cerrar_el_area_el_lienzo_vuelve_a_responder() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, Order::Foreground, true);
        let sobre = banco.sobre_el_area();
        let [_, b, _] = banco.clic(sobre);
        assert!(!b.empieza());
        banco.abierta = false;
        banco.paso(vec![]);
        banco.paso(vec![]);
        let [a, b, c] = banco.clic(sobre);
        assert!(a.info.hover.is_some(), "ppp {ppp}");
        assert!(b.empieza(), "ppp {ppp}: {:?}", b.entradas);
        assert!(c.termina(), "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9b: con el menu_button real de show_menu
// ---------------------------------------------------------------------------------------------

/// Banco con `show_menu` arriba y el lienzo debajo: el desplegable de `Archivo` cae sobre el lienzo.
struct BancoConMenu {
    ctx: Context,
    canvas: Canvas,
    ppp: f32,
    estado: UiState,
}

struct PasoConMenu {
    entradas: Vec<CanvasInput>,
    info: CanvasInfo,
    accion: Option<AppAction>,
    textos: common::Pintado,
}

impl BancoConMenu {
    fn nuevo(ppp: f32) -> BancoConMenu {
        let mut canvas = Canvas::new();
        canvas.set_image(&imagen());
        let mut banco = BancoConMenu {
            ctx: contexto(ppp),
            canvas,
            ppp,
            estado: UiState {
                can_undo: true,
                can_redo: true,
                zoom: Zoom::from_int(12),
                tool: Tool::Pencil,
                locks: Locks::default(),
            },
        };
        for _ in 0..3 {
            banco.paso(vec![]);
        }
        banco
    }

    fn paso(&mut self, events: Vec<Event>) -> PasoConMenu {
        let mut resultado = None;
        let canvas = &mut self.canvas;
        let estado = &self.estado;
        let formas = fotograma(&self.ctx, PANTALLA, events, |ui| {
            let accion = show_menu(ui, estado, Platform::Linux);
            let entradas = canvas.interact_ordered(ui);
            let info = canvas.paint(ui, std::iter::empty());
            resultado = Some((accion, entradas, info));
        });
        let (accion, entradas, info) = resultado.expect("se dibuja");
        PasoConMenu {
            entradas,
            info,
            accion,
            textos: common::pintado(&formas),
        }
    }

    /// Mueve, pulsa y suelta en `pos` y devuelve los tres pasos.
    fn clic(&mut self, pos: Pos2) -> [PasoConMenu; 3] {
        let a = self.paso(vec![mover(pos)]);
        let b = self.paso(vec![boton(pos, true)]);
        let c = self.paso(vec![boton(pos, false)]);
        [a, b, c]
    }

    /// Abre `Archivo` con un clic en su título (a 4 u del borde, con 61 u de ancho).
    fn abre_archivo(&mut self) {
        let n = n_de(self.ppp);
        let titulo = en_puntos(self.ppp, (4 + 61 / 2) * n, 8 * n);
        self.clic(titulo);
        self.paso(vec![]);
        self.paso(vec![]);
    }

    /// Centro de la entrada `Guardar` del desplegable abierto (no `Guardar como…`).
    fn centro_de_guardar(&mut self) -> Pos2 {
        let paso = self.paso(vec![]);
        let texto = paso
            .textos
            .textos
            .iter()
            .find(|t| t.texto.starts_with("Guardar") && !t.texto.starts_with("Guardar como"))
            .unwrap_or_else(|| {
                panic!(
                    "el desplegable de Archivo no se ve; hay {:?}",
                    paso.textos.todos_los_textos()
                )
            });
        texto.rect().center()
    }
}

#[test]
fn fila_9b_con_el_menu_real_abierto_un_clic_en_guardar_no_dibuja_y_da_save() {
    for ppp in PPPS {
        let mut banco = BancoConMenu::nuevo(ppp);
        banco.abre_archivo();
        let guardar = banco.centro_de_guardar();
        // La entrada cae sobre el lienzo (el menú no ocupa más que su barra).
        let n = n_de(ppp);
        assert!(guardar.y * ppp > (16 * n) as f32, "ppp {ppp}: {guardar:?}");

        let pasos = banco.clic(guardar);
        for (i, paso) in pasos.iter().enumerate() {
            assert!(
                !paso
                    .entradas
                    .iter()
                    .any(|e| matches!(e, CanvasInput::Stroke(_))),
                "ppp {ppp}: el paso {i} dibujó: {:?}",
                paso.entradas
            );
            assert_eq!(paso.info.hover, None, "ppp {ppp}: paso {i}");
        }
        assert!(
            pasos.iter().any(|p| p.accion == Some(AppAction::Save)),
            "ppp {ppp}: el clic debe dar Save; dio {:?}",
            pasos.iter().map(|p| p.accion).collect::<Vec<_>>()
        );
    }
}

#[test]
fn fila_9b_con_el_menu_real_abierto_la_rueda_y_el_pellizco_no_cambian_el_zoom() {
    for ppp in PPPS {
        let mut banco = BancoConMenu::nuevo(ppp);
        let antes = banco.paso(vec![]).info.zoom;
        banco.abre_archivo();
        let guardar = banco.centro_de_guardar();
        banco.paso(vec![mover(guardar)]);
        banco.paso(vec![]);
        assert_eq!(banco.paso(vec![]).info.hover, None, "ppp {ppp}");
        let paso = banco.paso(vec![rueda_linea(true)]);
        assert_eq!(
            paso.info.zoom, antes,
            "ppp {ppp}: rueda sobre el desplegable"
        );
        let paso = banco.paso(vec![Event::Zoom(1.3)]);
        assert_eq!(
            paso.info.zoom, antes,
            "ppp {ppp}: pellizco sobre el desplegable"
        );
    }
}

#[test]
fn fila_9b_con_el_menu_cerrado_el_mismo_punto_dibuja() {
    for ppp in PPPS {
        let mut banco = BancoConMenu::nuevo(ppp);
        banco.abre_archivo();
        let guardar = banco.centro_de_guardar();
        // Elegir `Guardar` cierra el desplegable.
        banco.clic(guardar);
        banco.paso(vec![]);
        banco.paso(vec![]);
        let [a, b, c] = banco.clic(guardar);
        assert!(
            a.info.hover.is_some(),
            "ppp {ppp}: hover válido con el menú cerrado"
        );
        assert!(
            b.entradas
                .iter()
                .any(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeBegin(_)))),
            "ppp {ppp}: con el menú cerrado el clic dibuja: {:?}",
            b.entradas
        );
        assert!(
            c.entradas
                .iter()
                .any(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeEnd))),
            "ppp {ppp}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5b (banco): el zoom a mitad de trazo
// ---------------------------------------------------------------------------------------------

fn rojo() -> Option<Pixel> {
    Some(Pixel::Rgba(Rgba::opaque(255, 0, 0)))
}

/// El píxel de imagen (con signo) bajo `pos` según el rectángulo de la imagen del fotograma.
fn pixel_bajo(info: &CanvasInfo, ppp: f32, pos: Pos2) -> (i32, i32) {
    let rect = info.image_rect.expect("hay imagen");
    let fisico = |v: f32| (v * ppp).round() as i64;
    let x = info.zoom.unscale(fisico(pos.x - rect.min.x));
    let y = info.zoom.unscale(fisico(pos.y - rect.min.y));
    (x as i32, y as i32)
}

struct Sesion {
    banco: Banco,
    doc: Document,
    tools: ToolState,
}

impl Sesion {
    fn nueva(ppp: f32) -> Sesion {
        let banco = Banco::nuevo(ppp, Order::Foreground, false);
        Sesion {
            banco,
            doc: Document::new(imagen()),
            tools: ToolState::new(),
        }
    }

    /// Un fotograma cuyos eventos de trazo pasan por `ToolState` (como `main`).
    fn paso(&mut self, events: Vec<Event>) -> Paso {
        let paso = self.banco.paso(events);
        let id = self.doc.main_image();
        for input in &paso.entradas {
            self.tools
                .canvas_input_with(&mut self.doc, id, rojo(), *input)
                .expect("el evento es válido");
        }
        paso
    }
}

#[test]
fn fila_5b_un_trazo_a_medias_con_request_view_zoom_in_cambia_el_zoom_y_el_trazo_sigue() {
    for ppp in PPPS {
        let mut s = Sesion::nueva(ppp);
        let estado_inicial = s.doc.state_id();
        let p0 = pos2(400.0, 300.0);
        let p1 = pos2(430.0, 300.0);
        let p2 = pos2(460.0, 320.0);

        s.paso(vec![mover(p0)]);
        let empezar = s.paso(vec![boton(p0, true)]);
        assert!(empezar.empieza(), "ppp {ppp}");
        assert!(s.tools.busy());
        s.paso(vec![mover(p1)]);
        assert!(s.tools.busy());

        let zoom_antes = s.banco.zoom();
        s.banco.canvas.request_view(ViewOp::ZoomIn);
        let tras = s.paso(vec![]);
        assert_eq!(
            tras.info.zoom,
            zoom_antes.zoom_in(),
            "ppp {ppp}: el zoom cambia a mitad de trazo"
        );
        assert!(s.tools.busy(), "ppp {ppp}: el trazo sigue");
        assert!(
            !tras.termina(),
            "ppp {ppp}: pedir la vista no termina el trazo"
        );
        assert_eq!(
            s.doc.state_id(),
            estado_inicial,
            "ppp {ppp}: el documento no cambia hasta soltar"
        );
        assert!(!s.doc.can_undo());

        // El siguiente StrokeMove usa la vista nueva.
        let paso = s.paso(vec![mover(p2)]);
        let esperado = pixel_bajo(&paso.info, ppp, p2);
        assert_eq!(paso.movimientos(), vec![esperado], "ppp {ppp}: vista nueva");
        assert!(s.tools.busy());
        assert_eq!(s.doc.state_id(), estado_inicial);

        // Soltar crea un solo paso.
        let soltar = s.paso(vec![boton(p2, false)]);
        assert!(soltar.termina(), "ppp {ppp}");
        assert!(!s.tools.busy());
        assert_ne!(
            s.doc.state_id(),
            estado_inicial,
            "ppp {ppp}: el trazo se aplicó"
        );
        assert!(s.doc.can_undo());
        s.doc.undo().expect("un paso que deshacer");
        assert!(
            !s.doc.can_undo(),
            "ppp {ppp}: el trazo entero fue un solo paso"
        );
        assert_eq!(s.doc.state_id(), estado_inicial);
    }
}

#[test]
fn fila_5b_varias_peticiones_de_vista_a_mitad_de_trazo_siguen_siendo_un_solo_paso() {
    let mut s = Sesion::nueva(1.0);
    let p0 = pos2(400.0, 300.0);
    s.paso(vec![mover(p0)]);
    s.paso(vec![boton(p0, true)]);
    for (i, op) in [ViewOp::ZoomIn, ViewOp::ZoomOut, ViewOp::Actual, ViewOp::Fit]
        .into_iter()
        .enumerate()
    {
        s.banco.canvas.request_view(op);
        s.paso(vec![]);
        assert!(s.tools.busy(), "tras {op:?}");
        s.paso(vec![mover(pos2(400.0 + 10.0 * i as f32, 310.0))]);
    }
    s.paso(vec![boton(p0, false)]);
    assert!(!s.tools.busy());
    assert!(s.doc.can_undo());
    s.doc.undo().expect("un paso");
    assert!(
        !s.doc.can_undo(),
        "un solo paso aunque el zoom cambió cuatro veces"
    );
}

#[test]
fn fila_5b_la_rueda_a_mitad_de_trazo_sigue_cambiando_el_zoom_como_antes() {
    for ppp in PPPS {
        let mut s = Sesion::nueva(ppp);
        let p0 = pos2(400.0, 300.0);
        s.paso(vec![mover(p0)]);
        s.paso(vec![boton(p0, true)]);
        let antes = s.banco.zoom();
        let paso = s.paso(vec![rueda_linea(true)]);
        assert_eq!(
            paso.info.zoom,
            antes.zoom_in(),
            "ppp {ppp}: la rueda no cambia"
        );
        assert!(s.tools.busy(), "ppp {ppp}");
        s.paso(vec![boton(p0, false)]);
        assert!(!s.tools.busy());
        assert!(s.doc.can_undo());
    }
}
