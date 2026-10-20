//! Tests de la pila de la ventana con sus piezas reales: menú, columna, panel de paletas, barra de
//! estado y lienzo (specs/hito-2-menu-herramientas.md, «Casos» filas 7, 9 y 10 en su parte de
//! banco de egui, y propiedad 4).
//!
//! El banco es el de `tests/panel_baja.rs` con las dos piezas nuevas: un `egui::Context` ejecuta
//! fotogramas con `RawInput` sintético, sin GPU ni pantalla. En cada fotograma, como hace la app:
//! `show_menu` (arriba), `status::show` (abajo), `show_toolbar`, `palette_panel::show_editing_view`
//! y el lienzo (`interact_ordered` y `paint`). Los rectángulos de cada pieza salen de la diferencia
//! de `ui.available_rect_before_wrap()` antes y después de pintarla.
//!
//! - Fila 7: los rectángulos del banco son los de `chrome::window_layout`, enteros en píxeles
//!   físicos (error de `f32` ≤ 0,001), a las cinco escalas y en la ventana por defecto (960 × 720 pt)
//!   y en la mínima; los botones golpean en su sitio dentro de la pila; el título más ancho y el
//!   desplegable de `Ver` (208 u) caben.
//! - Fila 9: el origen del lienzo está en `(340·n, 16·n)`: `hover` y `StrokeBegin` valen lo mismo que
//!   en un lienzo con origen (0,0) y el puntero desplazado, en la primera y la última coordenada
//!   física de cada píxel de la imagen, en las cuatro esquinas del lienzo y un píxel fuera; y
//!   `Ajustar` centra la imagen en **ese** rectángulo (no en la ventana).
//! - Fila 10 (respaldo B con la columna): con el panel de `200·n`, `100·n` y `297·n` de alto, la
//!   rueda solo mueve el panel; la columna y el menú no se desplazan y se pintan recortados a su
//!   rectángulo; los botones de la columna y los deslizadores golpean en su sitio; sin avisos en la
//!   barra; con alto ≥ bloque el desplazamiento vuelve a 0 en el mismo fotograma.
//! - Propiedad 4: conversión pantalla-imagen con el origen del lienzo desplazado (pura).
//!
//! Supuestos (la spec no los fija): `show_menu` crea su propio panel superior y `show_toolbar` el
//! suyo a la izquierda, como `status::show` y `palette_panel::show` crean los suyos; los títulos del
//! menú se pintan con el texto del título a `x = (4 + 6)·u` y `y = 1·u` del borde del menú.

use egui::{Context, Event, Pos2, Rect, vec2};
use proptest::prelude::*;
use px_core::{Image, Mode, Palette, Rgba};
use px_ui::actions::{AppAction, Locks, Platform, UiState};
use px_ui::canvas::{Canvas, CanvasEvent, CanvasInfo, CanvasInput};
use px_ui::chrome::{MENU_H_U, canvas_origin_px, window_layout};
use px_ui::color::{ColorChoice, SlotColors};
use px_ui::color_edit::{ColorEditor, Control, EditTarget, slider_x};
use px_ui::edit_layout::{
    ROW_H_U, TRACK_W_U, TRACK_X_U, block_height_px, control_row_u, min_window_height_pt,
    min_window_width_pt, min_window_width_u, panel_layout,
};
use px_ui::keymap::default_keymap;
use px_ui::menu::show_menu;
use px_ui::palette_grid::{PanelView, Scroll};
use px_ui::palette_panel::{self, PanelOutput};
use px_ui::status;
use px_ui::text::{apply_scale, text_scale};
use px_ui::tool::Tool;
use px_ui::toolbar::{TOOL_BUTTONS, ZOOM_BUTTONS, show_toolbar};
use px_ui::view::{MIN_VISIBLE, Size, View, ViewOp, Zoom};

mod common;
use common::{
    PPPS, Pintado, boton, contexto, en_puntos, es_entero_fisico_en_egui, fotograma, mover, n_de,
    pintado, rueda_puntos, tolerancia_de_egui,
};

// ---------------------------------------------------------------------------------------------
// Imágenes y medidas
// ---------------------------------------------------------------------------------------------

/// Tolerancia, en puntos, para comparar recortes y bordes de lo pintado.
const TOL_PT: f32 = 0.05;

/// Tolerancia, en píxeles físicos, para las posiciones del texto del menú y de los desplegables:
/// egui los coloca con sus márgenes (`add_space`, `button_padding`) redondeados a 1/16 pt, así que
/// a `ppp` 1,25 y 1,5 se desvían unas décimas de píxel (hasta medio píxel en el paso entre filas);
/// un margen o un alto de fila equivocado (1 u = 1 a 3 px, y el alto por defecto de egui, 18 pt) sí
/// se nota.
const TOL_MENU_PX: f32 = 0.75;

fn rgba(ancho: u32, alto: u32) -> Image {
    Image::new(ancho, alto, Mode::Rgba).expect("imagen válida")
}

/// Indexada de 64×48 con la paleta `[R, G, B, K]`.
fn indexada() -> Image {
    let paleta = Palette::from_colors([
        Rgba::opaque(255, 0, 0),
        Rgba::opaque(0, 255, 0),
        Rgba::opaque(0, 0, 255),
        Rgba::opaque(0, 0, 0),
    ])
    .expect("paleta");
    Image::new_indexed(64, 48, paleta).expect("imagen indexada")
}

/// Ventana por defecto (960 × 720 pt) y mínima, en píxeles físicos, a `ppp`.
fn ventanas(ppp: f32) -> [(i64, i64); 2] {
    [
        ((960.0 * ppp).round() as i64, (720.0 * ppp).round() as i64),
        (
            (min_window_width_pt(ppp) * ppp).round() as i64,
            (min_window_height_pt(ppp) * ppp).round() as i64,
        ),
    ]
}

/// Ventana cuyo panel mide `panel_h_px` de alto (menú y barra de estado aparte).
fn ventana_con_panel(ppp: f32, panel_h_px: i64) -> (i64, i64) {
    let n = n_de(ppp);
    (
        (960.0 * ppp).round() as i64,
        panel_h_px + (MENU_H_U + 20) * n,
    )
}

fn estado_de_la_ui() -> UiState {
    UiState {
        can_undo: true,
        can_redo: true,
        zoom: Zoom::from_int(5),
        tool: Tool::Pencil,
        locks: Locks::default(),
    }
}

// ---------------------------------------------------------------------------------------------
// Banco de la pila completa
// ---------------------------------------------------------------------------------------------

/// Rectángulos de las cinco piezas en un fotograma, en puntos.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rects {
    menu: Rect,
    estado: Rect,
    columna: Rect,
    panel: Rect,
    lienzo: Rect,
}

struct Fot {
    rects: Rects,
    entradas: Vec<CanvasInput>,
    info: CanvasInfo,
    panel: PanelOutput,
    menu: Option<AppAction>,
    columna: Option<AppAction>,
    pintado: Pintado,
}

struct Banco {
    ctx: Context,
    ppp: f32,
    ventana_px: (i64, i64),
    canvas: Canvas,
    scroll: Scroll,
    view: PanelView,
    image: Image,
    choice: ColorChoice,
    slots: SlotColors,
    editor: ColorEditor,
    estado: UiState,
    /// Lo que devolvió el panel en cada fotograma.
    historial: Vec<PanelOutput>,
}

impl Banco {
    fn nuevo(ppp: f32, ventana_px: (i64, i64), image: Image) -> Banco {
        let mut canvas = Canvas::new();
        canvas.set_image(&image);
        let mut banco = Banco {
            ctx: contexto(ppp),
            ppp,
            ventana_px,
            canvas,
            scroll: Scroll::default(),
            view: PanelView::default(),
            choice: ColorChoice::Entry(1),
            slots: SlotColors::defaults(image.mode()),
            image,
            editor: ColorEditor::default(),
            estado: estado_de_la_ui(),
            historial: Vec::new(),
        };
        // Reparte el espacio, sube la textura y asienta los paneles.
        for _ in 0..3 {
            banco.reposo();
        }
        banco.historial.clear();
        banco
    }

    fn n(&self) -> i64 {
        n_de(self.ppp)
    }

    fn pantalla(&self) -> egui::Vec2 {
        vec2(
            self.ventana_px.0 as f32 / self.ppp,
            self.ventana_px.1 as f32 / self.ppp,
        )
    }

    /// Alto del panel en píxeles físicos con la ventana actual.
    fn panel_h_px(&self) -> i64 {
        self.ventana_px.1 - (MENU_H_U + 20) * self.n()
    }

    fn fotograma(&mut self, events: Vec<Event>) -> Fot {
        let actual = EditTarget::of(&self.image, self.choice).and_then(|destino| {
            let color = destino.stored(&self.image, &self.slots)?;
            Some((destino, color))
        });
        self.editor.sync(actual);
        let enabled = !self.editor.dragging();
        let pantalla = self.pantalla();
        let Banco {
            ctx,
            canvas,
            scroll,
            view,
            image,
            choice,
            slots,
            editor,
            estado,
            ..
        } = self;
        let mut resultado = None;
        let formas = fotograma(ctx, pantalla, events, |ui| {
            apply_scale(ui.ctx());
            let r0 = ui.available_rect_before_wrap();
            let menu = show_menu(ui, estado, Platform::Linux);
            let r1 = ui.available_rect_before_wrap();
            status::show(ui, "lápiz · color 1 #000000", false);
            let r2 = ui.available_rect_before_wrap();
            let columna = show_toolbar(ui, estado, "×5");
            let r3 = ui.available_rect_before_wrap();
            let panel = palette_panel::show_editing_view(
                ui, image, *choice, scroll, view, enabled, slots, editor,
            );
            let r4 = ui.available_rect_before_wrap();
            let entradas = canvas.interact_ordered(ui);
            let info = canvas.paint(ui);
            let rects = Rects {
                menu: Rect::from_min_max(r0.min, Pos2::new(r0.max.x, r1.min.y)),
                estado: Rect::from_min_max(Pos2::new(r1.min.x, r2.max.y), r1.max),
                columna: Rect::from_min_max(r2.min, Pos2::new(r3.min.x, r2.max.y)),
                panel: Rect::from_min_max(r3.min, Pos2::new(r4.min.x, r3.max.y)),
                lienzo: r4,
            };
            resultado = Some((rects, entradas, info, panel, menu, columna));
        });
        let (rects, entradas, info, panel, menu, columna) = resultado.expect("se dibuja");
        self.historial.push(panel.clone());
        Fot {
            rects,
            entradas,
            info,
            panel,
            menu,
            columna,
            pintado: pintado(&formas),
        }
    }

    fn reposo(&mut self) -> Fot {
        self.fotograma(vec![])
    }

    fn mueve(&mut self, x_px: i64, y_px: i64) -> Fot {
        let pos = en_puntos(self.ppp, x_px, y_px);
        self.fotograma(vec![mover(pos)])
    }

    /// Mueve, pulsa y suelta; devuelve los tres fotogramas.
    fn clic(&mut self, x_px: i64, y_px: i64) -> [Fot; 3] {
        let pos = en_puntos(self.ppp, x_px, y_px);
        let a = self.fotograma(vec![mover(pos)]);
        let b = self.fotograma(vec![boton(pos, true)]);
        let c = self.fotograma(vec![boton(pos, false)]);
        [a, b, c]
    }

    /// La rueda, `px_abajo` píxeles físicos hacia abajo, con el puntero en `(x_px, y_px)`.
    fn rueda_en(&mut self, x_px: i64, y_px: i64, px_abajo: i64) -> Fot {
        self.mueve(x_px, y_px);
        let eventos = vec![rueda_puntos(px_abajo, self.ppp)];
        self.fotograma(eventos)
    }

    /// Cambia el alto del panel (la ventana) para el siguiente fotograma.
    fn alto_de_panel(&mut self, panel_h_px: i64) {
        self.ventana_px.1 = panel_h_px + (MENU_H_U + 20) * self.n();
    }
}

/// Los bordes de `rect` en píxeles físicos, o `None` si alguno no es entero con la tolerancia del
/// redondeo de egui (`common::tolerancia_de_egui`, 1/32 pt: `Panel` y `allocate_rect` redondean a
/// múltiplos pares de `GUI_ROUNDING`, así que la spec «error de `f32` ≤ 0,001» no es alcanzable a
/// `ppp` 1,25 y 1,5).
fn bordes_fisicos(rect: Rect, ppp: f32) -> Option<[i64; 4]> {
    let bordes = [rect.min.x, rect.min.y, rect.max.x, rect.max.y];
    bordes
        .iter()
        .all(|b| es_entero_fisico_en_egui(*b, ppp))
        .then(|| bordes.map(|b| (b * ppp).round() as i64))
}

fn esperado(rect: px_ui::view::ScreenRect) -> [i64; 4] {
    [rect.x, rect.y, rect.x + rect.width, rect.y + rect.height]
}

// ---------------------------------------------------------------------------------------------
// Fila 7: la geometría del banco
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_7_los_rectangulos_del_banco_son_los_de_window_layout_y_enteros() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        for ventana in ventanas(ppp) {
            let mut banco = Banco::nuevo(ppp, ventana, indexada());
            let f = banco.reposo();
            let layout = window_layout(ventana, n);
            let piezas = [
                ("menu", f.rects.menu, layout.menu),
                ("estado", f.rects.estado, layout.status),
                ("columna", f.rects.columna, layout.toolbar),
                ("panel", f.rects.panel, layout.panel),
                ("lienzo", f.rects.lienzo, layout.canvas),
            ];
            for (nombre, medido, previsto) in piezas {
                let fisico = bordes_fisicos(medido, ppp).unwrap_or_else(|| {
                    panic!("ppp {ppp} ventana {ventana:?}: {nombre} no cae en píxeles enteros: {medido:?}")
                });
                assert_eq!(
                    fisico,
                    esperado(previsto),
                    "ppp {ppp} ventana {ventana:?}: {nombre}"
                );
            }
        }
    }
}

#[test]
fn fila_7_el_origen_del_lienzo_del_banco_es_340n_16n() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        for ventana in ventanas(ppp) {
            let mut banco = Banco::nuevo(ppp, ventana, indexada());
            let f = banco.reposo();
            let [x, y, ..] = bordes_fisicos(f.rects.lienzo, ppp).expect("entero");
            assert_eq!((x, y), canvas_origin_px(n), "ppp {ppp}");
            assert_eq!((x, y), (340 * i64::from(n), 16 * i64::from(n)));
        }
    }
}

#[test]
fn fila_7_los_botones_de_la_columna_golpean_en_su_sitio_dentro_de_la_pila() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for ventana in ventanas(ppp) {
            let mut banco = Banco::nuevo(ppp, ventana, indexada());
            let x = (4 + 99 / 2) * n;
            for (i, button) in TOOL_BUTTONS.iter().enumerate() {
                let y = (16 + 4 + 18 * i as i64) * n + 8 * n;
                let acciones: Vec<_> = banco.clic(x, y).iter().filter_map(|f| f.columna).collect();
                assert_eq!(acciones, vec![button.action], "ppp {ppp}: {}", button.label);
            }
            for (i, button) in ZOOM_BUTTONS.iter().enumerate() {
                let y = (16 + 119 + 18 * i as i64) * n + 8 * n;
                let acciones: Vec<_> = banco.clic(x, y).iter().filter_map(|f| f.columna).collect();
                assert_eq!(acciones, vec![button.action], "ppp {ppp}: {}", button.label);
            }
        }
    }
}

/// Texto del menú pintado en la barra superior (los títulos).
fn titulo<'a>(f: &'a Fot, ppp: f32, n: i64, nombre: &str) -> &'a common::Texto {
    f.pintado
        .textos
        .iter()
        .find(|t| {
            t.texto == nombre && t.rect().max.y * ppp <= (16 * n) as f32 + tolerancia_de_egui(ppp)
        })
        .unwrap_or_else(|| panic!("el título {nombre} no se pinta en el menú"))
}

#[test]
fn fila_7_el_menu_pinta_sus_cinco_titulos_dentro_de_su_barra_y_caben_en_el_ancho_minimo() {
    for ppp in PPPS {
        let n = n_de(ppp);
        let mut banco = Banco::nuevo(ppp, ventanas(ppp)[1], indexada());
        let f = banco.reposo();
        for nombre in ["Archivo", "Editar", "Imagen", "Ver", "Ayuda"] {
            let t = titulo(&f, ppp, n, nombre);
            let r = t.rect();
            assert!(
                r.max.x * ppp <= (min_window_width_u() * n) as f32,
                "ppp {ppp}: {nombre} se sale del ancho mínimo"
            );
            assert!(r.min.y >= f.rects.menu.min.y - TOL_PT);
            assert!(r.max.y <= f.rects.menu.max.y + TOL_PT);
        }
        // El primero: 4 u de margen y 6 u de margen del texto, y 1 u arriba.
        let archivo = titulo(&f, ppp, n, "Archivo");
        assert!(
            (archivo.pos.x * ppp - (10 * n) as f32).abs() <= TOL_MENU_PX,
            "ppp {ppp}: x = {}",
            archivo.pos.x * ppp
        );
        assert!(
            (archivo.pos.y * ppp - n as f32).abs() <= TOL_MENU_PX,
            "ppp {ppp}: y = {}",
            archivo.pos.y * ppp
        );
        // El último título termina antes de 4 + 249 u más la separación entre botones.
        let ayuda = titulo(&f, ppp, n, "Ayuda");
        assert!(ayuda.rect().max.x * ppp <= ((4 + 249) * n) as f32 + 4.0 * n as f32);
    }
}

#[test]
fn fila_7_el_desplegable_de_ver_mide_208_u_con_filas_de_16_u() {
    for ppp in PPPS {
        let n = n_de(ppp);
        let mut banco = Banco::nuevo(ppp, ventanas(ppp)[0], indexada());
        let f = banco.reposo();
        let centro = titulo(&f, ppp, n, "Ver").rect().center();
        let (x, y) = (
            (centro.x * ppp).round() as i64,
            (centro.y * ppp).round() as i64,
        );
        banco.clic(x, y);
        banco.reposo();
        let f = banco.reposo();
        let busca = |texto: &str| -> common::Texto {
            f.pintado
                .textos
                .iter()
                .find(|t| {
                    // El desplegable cae sobre el panel y el lienzo, no sobre la columna (que
                    // tiene sus propios `Acercar`, `Alejar` y `1:1`).
                    t.texto.trim() == texto
                        && f.rects.menu.max.y <= t.pos.y
                        && t.pos.x >= f.rects.columna.max.x
                })
                .unwrap_or_else(|| {
                    panic!(
                        "ppp {ppp}: el desplegable de Ver no pinta {texto:?}; hay {:?}",
                        f.pintado.todos_los_textos()
                    )
                })
                .clone()
        };
        // Filas de 16 u: el paso entre rótulos consecutivos es de 16 u (sin separación).
        let filas = ["Acercar", "Alejar", "Ajustar a la ventana", "1:1"].map(busca);
        for pareja in filas.windows(2) {
            let paso = (pareja[1].pos.y - pareja[0].pos.y) * ppp;
            assert!(
                (paso - (16 * n) as f32).abs() <= TOL_MENU_PX,
                "ppp {ppp}: {:?} y {:?} distan {paso} px y no {}",
                pareja[0].texto,
                pareja[1].texto,
                16 * n
            );
        }
        // Ancho 208 u = 6 u de margen + (rótulo + 2 + atajo)·7 u + 6 u de margen: del comienzo del
        // rótulo más largo al final de su atajo hay (20 + 2 + 6)·7 = 196 u.
        let ajustar = busca("Ajustar a la ventana");
        let inicio = busca("Inicio");
        let ancho = (inicio.rect().max.x - ajustar.rect().min.x) * ppp;
        assert!(
            (ancho - (196 * n) as f32).abs() <= TOL_MENU_PX,
            "ppp {ppp}: del rótulo al final del atajo hay {ancho} px y no {}",
            196 * n
        );
        // Las demás filas llevan su atajo alineado a la derecha, en el mismo borde.
        let borde = inicio.rect().max.x * ppp;
        for atajo in [">", "<", "Ctrl+1"] {
            let t = busca(atajo);
            assert!(
                (t.rect().max.x * ppp - borde).abs() <= TOL_MENU_PX,
                "ppp {ppp}: el atajo {atajo:?} no está alineado a la derecha"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9: origen del lienzo desplazado
// ---------------------------------------------------------------------------------------------

/// Posición del puntero a un cuarto de píxel físico **dentro** del píxel `(x_px, y_px)`: así una
/// coordenada de borde no depende de que egui haya redondeado el rectángulo del lienzo hasta
/// 1/32 pt (a `ppp` 1,25 y 1,5 el origen del lienzo cae a unas centésimas de píxel del entero) y
/// el redondeo a píxel físico del lienzo da siempre la coordenada pedida.
fn dentro_del_pixel(ppp: f32, x_px: i64, y_px: i64) -> Pos2 {
    Pos2::new((x_px as f32 + 0.25) / ppp, (y_px as f32 + 0.25) / ppp)
}

/// Lienzo suelto, con el origen en (0,0) y el tamaño del lienzo de la pila: la referencia.
struct Referencia {
    ctx: Context,
    canvas: Canvas,
    ppp: f32,
    pantalla: egui::Vec2,
}

impl Referencia {
    fn nuevo(ppp: f32, lienzo_px: (i64, i64), image: &Image) -> Referencia {
        let mut canvas = Canvas::new();
        canvas.set_image(image);
        let mut referencia = Referencia {
            ctx: contexto(ppp),
            canvas,
            ppp,
            pantalla: vec2(lienzo_px.0 as f32 / ppp, lienzo_px.1 as f32 / ppp),
        };
        for _ in 0..3 {
            referencia.fotograma(vec![]);
        }
        referencia
    }

    fn fotograma(&mut self, events: Vec<Event>) -> (CanvasInfo, Vec<CanvasInput>) {
        let mut resultado = None;
        let canvas = &mut self.canvas;
        fotograma(&self.ctx, self.pantalla, events, |ui| {
            let entradas = canvas.interact_ordered(ui);
            let info = canvas.paint(ui);
            resultado = Some((info, entradas));
        });
        resultado.expect("se dibuja")
    }

    fn hover(&mut self, x_px: i64, y_px: i64) -> Option<(u32, u32)> {
        let pos = dentro_del_pixel(self.ppp, x_px, y_px);
        self.fotograma(vec![mover(pos)]).0.hover
    }

    /// El `StrokeBegin` de un clic en `(x_px, y_px)`, si lo hay.
    fn empieza(&mut self, x_px: i64, y_px: i64) -> Option<(i32, i32)> {
        let pos = dentro_del_pixel(self.ppp, x_px, y_px);
        self.fotograma(vec![mover(pos)]);
        let b = self.fotograma(vec![boton(pos, true)]).1;
        self.fotograma(vec![boton(pos, false)]);
        inicio_de(&b)
    }
}

impl Banco {
    fn hover(&mut self, x_px: i64, y_px: i64) -> Option<(u32, u32)> {
        let pos = dentro_del_pixel(self.ppp, x_px, y_px);
        self.fotograma(vec![mover(pos)]).info.hover
    }

    fn empieza(&mut self, x_px: i64, y_px: i64) -> Option<(i32, i32)> {
        let pos = dentro_del_pixel(self.ppp, x_px, y_px);
        self.fotograma(vec![mover(pos)]);
        let b = self.fotograma(vec![boton(pos, true)]).entradas;
        self.fotograma(vec![boton(pos, false)]);
        inicio_de(&b)
    }
}

fn inicio_de(entradas: &[CanvasInput]) -> Option<(i32, i32)> {
    entradas.iter().find_map(|e| match e {
        CanvasInput::Stroke(CanvasEvent::StrokeBegin(p)) => Some(*p),
        _ => None,
    })
}

/// Origen de la imagen respecto al lienzo y píxeles físicos por píxel de imagen.
fn imagen_en_el_lienzo(f: &Fot, ppp: f32) -> ((i64, i64), i64) {
    let rect = f.info.image_rect.expect("hay imagen");
    let fisico = |v: f32| (v * ppp).round() as i64;
    (
        (
            fisico(rect.min.x - f.rects.lienzo.min.x),
            fisico(rect.min.y - f.rects.lienzo.min.y),
        ),
        f.info.zoom.scale(1),
    )
}

#[test]
fn fila_9_hover_en_la_primera_y_la_ultima_coordenada_de_cada_pixel_vale_lo_que_con_origen_cero() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let ventana = ventanas(ppp)[0];
        let mut banco = Banco::nuevo(ppp, ventana, rgba(6, 4));
        let f0 = banco.reposo();
        let (origen_lienzo_x, origen_lienzo_y) = (340 * n, 16 * n);
        let lienzo = window_layout(ventana, n as u32).canvas;
        let mut referencia = Referencia::nuevo(ppp, (lienzo.width, lienzo.height), &rgba(6, 4));
        let (origen, z) = imagen_en_el_lienzo(&f0, ppp);
        assert!(z > 1, "ppp {ppp}: la imagen de 6×4 se amplía (×{z})");
        for iy in 0..4i64 {
            for ix in 0..6i64 {
                for (dx, dy) in [(0, 0), (z - 1, z - 1), (z - 1, 0), (0, z - 1)] {
                    let (x, y) = (origen.0 + ix * z + dx, origen.1 + iy * z + dy);
                    let esperado = Some((ix as u32, iy as u32));
                    assert_eq!(
                        banco.hover(origen_lienzo_x + x, origen_lienzo_y + y),
                        esperado,
                        "ppp {ppp}: pila, píxel ({ix}, {iy}) en ({dx}, {dy})"
                    );
                    assert_eq!(
                        referencia.hover(x, y),
                        esperado,
                        "ppp {ppp}: referencia, píxel ({ix}, {iy}) en ({dx}, {dy})"
                    );
                }
            }
        }
    }
}

#[test]
fn fila_9_stroke_begin_en_cada_pixel_vale_lo_que_con_origen_cero() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let ventana = ventanas(ppp)[0];
        let mut banco = Banco::nuevo(ppp, ventana, rgba(6, 4));
        let f0 = banco.reposo();
        let lienzo = window_layout(ventana, n as u32).canvas;
        let mut referencia = Referencia::nuevo(ppp, (lienzo.width, lienzo.height), &rgba(6, 4));
        let (origen, z) = imagen_en_el_lienzo(&f0, ppp);
        for iy in 0..4i64 {
            for ix in 0..6i64 {
                for (dx, dy) in [(0, 0), (z - 1, z - 1)] {
                    let (x, y) = (origen.0 + ix * z + dx, origen.1 + iy * z + dy);
                    let esperado = Some((ix as i32, iy as i32));
                    assert_eq!(
                        banco.empieza(340 * n + x, 16 * n + y),
                        esperado,
                        "ppp {ppp}: pila, píxel ({ix}, {iy}) en ({dx}, {dy})"
                    );
                    assert_eq!(
                        referencia.empieza(x, y),
                        esperado,
                        "ppp {ppp}: referencia, píxel ({ix}, {iy}) en ({dx}, {dy})"
                    );
                }
            }
        }
    }
}

#[test]
fn fila_9_en_las_cuatro_esquinas_del_lienzo_y_un_pixel_fuera() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        // Ventana mínima y una imagen de 1200 × 1200: a ×1 cubre el lienzo entero (el mayor mide
        // 384 × 1053 a ppp 3 desde I.2a), así que las esquinas del lienzo caen sobre la imagen.
        let ventana = ventanas(ppp)[1];
        let lienzo = window_layout(ventana, n as u32).canvas;
        let (w, h) = (lienzo.width, lienzo.height);
        let imagen = rgba(1200, 1200);
        let mut banco = Banco::nuevo(ppp, ventana, imagen.clone());
        // Ajustar da ahora un nivel 1/K para esta imagen: se pasa a ×1, que es el zoom en el que la
        // imagen cubre el lienzo entero.
        banco.canvas.request_view(ViewOp::Actual);
        let f0 = banco.reposo();
        let (origen, z) = imagen_en_el_lienzo(&f0, ppp);
        assert_eq!(z, 1, "ppp {ppp}");
        let mut referencia = Referencia::nuevo(ppp, (w, h), &imagen);
        referencia.canvas.request_view(ViewOp::Actual);
        referencia.fotograma(vec![]);
        let (ox, oy) = (340 * n, 16 * n);

        for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
            let esperado = Some(((x - origen.0) as u32, (y - origen.1) as u32));
            assert_eq!(
                banco.hover(ox + x, oy + y),
                esperado,
                "ppp {ppp}: esquina ({x}, {y})"
            );
            assert_eq!(
                referencia.hover(x, y),
                esperado,
                "ppp {ppp}: esquina ({x}, {y}) (referencia)"
            );
            let begin = Some(((x - origen.0) as i32, (y - origen.1) as i32));
            assert_eq!(
                banco.empieza(ox + x, oy + y),
                begin,
                "ppp {ppp}: StrokeBegin en ({x}, {y})"
            );
            assert_eq!(
                referencia.empieza(x, y),
                begin,
                "ppp {ppp}: StrokeBegin en ({x}, {y}) (referencia)"
            );
        }
        // Un píxel fuera del rectángulo del lienzo: `None` y sin trazo.
        for (x, y) in [(-1, 5), (w, 5), (5, -1), (5, h), (-1, -1), (w, h)] {
            assert_eq!(
                banco.hover(ox + x, oy + y),
                None,
                "ppp {ppp}: ({x}, {y}) fuera"
            );
            assert_eq!(
                banco.empieza(ox + x, oy + y),
                None,
                "ppp {ppp}: StrokeBegin fuera en ({x}, {y})"
            );
            assert_eq!(
                referencia.hover(x, y),
                None,
                "ppp {ppp}: ({x}, {y}) fuera (referencia)"
            );
            assert_eq!(
                referencia.empieza(x, y),
                None,
                "ppp {ppp}: StrokeBegin fuera en ({x}, {y}) (referencia)"
            );
        }
    }
}

#[test]
fn fila_9_ajustar_centra_la_imagen_en_el_rectangulo_del_lienzo_y_no_en_la_ventana() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let ventana = ventanas(ppp)[0];
        let mut banco = Banco::nuevo(ppp, ventana, rgba(64, 48));
        let lienzo = window_layout(ventana, n as u32).canvas;
        // Aleja la imagen de su sitio: acerca con la rueda sobre una esquina del lienzo.
        let esquina_x = lienzo.x + 20;
        let esquina_y = lienzo.y + 20;
        banco.mueve(esquina_x, esquina_y);
        for _ in 0..3 {
            banco.fotograma(vec![common::rueda_linea(true)]);
        }
        banco.canvas.request_view(ViewOp::Fit);
        let f = banco.reposo();
        let ajuste = View::fit(
            Size::new(lienzo.width as u32, lienzo.height as u32),
            Size::new(64, 48),
        );
        assert_eq!(f.info.zoom, ajuste.zoom(), "ppp {ppp}");
        let rect = f.info.image_rect.expect("hay imagen");
        let centro_lienzo = f.rects.lienzo.center();
        assert!(
            (rect.center().x - centro_lienzo.x).abs() * ppp <= 1.0 + 0.01
                && (rect.center().y - centro_lienzo.y).abs() * ppp <= 1.0 + 0.01,
            "ppp {ppp}: la imagen {:?} no está centrada en el lienzo {:?}",
            rect.center(),
            centro_lienzo
        );
        // Y el centro del lienzo no es el de la ventana: el panel y el menú lo desplazan.
        let centro_ventana = Rect::from_min_size(Pos2::ZERO, banco.pantalla()).center();
        assert!(
            (rect.center().x - centro_ventana.x).abs() >= 1.0,
            "ppp {ppp}: centrada en la ventana en vez de en el lienzo"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 10: respaldo B con la columna
// ---------------------------------------------------------------------------------------------

/// Los tres altos de la fila 10, en `u`.
const ALTOS_DEL_PANEL_U: [i64; 3] = [200, 100, 297];

fn banco_con_panel_de(ppp: f32, panel_u: i64) -> Banco {
    let n = n_de(ppp);
    Banco::nuevo(ppp, ventana_con_panel(ppp, panel_u * n), indexada())
}

#[test]
fn fila_10_solo_la_rueda_sobre_el_panel_mueve_offset_px() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for panel_u in [200, 100] {
            let mut banco = banco_con_panel_de(ppp, panel_u);
            let layout = window_layout(banco.ventana_px, n as u32);
            assert!(
                panel_layout(Mode::Indexed, banco.panel_h_px(), n as u32, 0).scrolling,
                "ppp {ppp} panel {panel_u} u: el panel debería desplazarse"
            );
            assert_eq!(banco.view.offset_px, 0);

            // Sobre la columna: nada.
            let (cx, cy) = (layout.toolbar.x + 2 * n, layout.toolbar.y + 10 * n);
            banco.rueda_en(cx, cy, 10 * n);
            assert_eq!(
                banco.view.offset_px, 0,
                "ppp {ppp} panel {panel_u} u: la columna no desplaza"
            );
            // Sobre el menú: nada.
            banco.rueda_en(100 * n, 4 * n, 10 * n);
            assert_eq!(
                banco.view.offset_px, 0,
                "ppp {ppp} panel {panel_u} u: el menú no desplaza"
            );
            // Sobre la barra de estado: nada.
            banco.rueda_en(100 * n, layout.status.y + 4 * n, 10 * n);
            assert_eq!(
                banco.view.offset_px, 0,
                "ppp {ppp} panel {panel_u} u: la barra no desplaza"
            );
            // Sobre el lienzo: nada.
            banco.rueda_en(layout.canvas.x + 20 * n, layout.canvas.y + 20 * n, 10 * n);
            assert_eq!(
                banco.view.offset_px, 0,
                "ppp {ppp} panel {panel_u} u: el lienzo no desplaza el panel"
            );
            // Sobre el panel: sí.
            banco.rueda_en(layout.panel.x + 10 * n, layout.panel.y + 10 * n, 10 * n);
            assert_eq!(
                banco.view.offset_px,
                10 * n,
                "ppp {ppp} panel {panel_u} u: la rueda sobre el panel desplaza"
            );
        }
    }
}

#[test]
fn fila_10_la_columna_y_el_menu_no_se_desplazan_con_ningun_offset_px() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for panel_u in ALTOS_DEL_PANEL_U {
            let mut banco = banco_con_panel_de(ppp, panel_u);
            let layout = window_layout(banco.ventana_px, n as u32);
            let antes = banco.reposo();
            let textos_antes: Vec<(String, [i64; 2])> = textos_de_columna_y_menu(&antes, ppp);
            banco.rueda_en(layout.panel.x + 10 * n, layout.panel.y + 10 * n, 15 * n);
            let despues = banco.reposo();
            if panel_u < 297 {
                assert_eq!(banco.view.offset_px, 15 * n, "ppp {ppp} panel {panel_u} u");
            }
            // Mismos rectángulos con cualquier offset.
            assert_eq!(antes.rects, despues.rects, "ppp {ppp} panel {panel_u} u");
            // Mismos textos en los mismos sitios.
            assert_eq!(
                textos_antes,
                textos_de_columna_y_menu(&despues, ppp),
                "ppp {ppp} panel {panel_u} u: la columna o el menú se movieron"
            );
        }
    }
}

const TITULOS: [&str; 4] = ["Archivo", "Editar", "Ver", "Ayuda"];

/// Textos de la columna: rótulos, nivel de zoom y letras de atajo (estas por contenido y por estar
/// a la izquierda de la columna: la `L` del deslizador de luminosidad es del panel).
fn es_texto_de_la_columna(texto: &common::Texto, ppp: f32) -> bool {
    let t = texto.texto.trim();
    TOOL_BUTTONS.iter().any(|b| t == b.label)
        || ZOOM_BUTTONS.iter().any(|b| t == b.label)
        || t.starts_with("Zoom ")
        || (TOOL_BUTTONS.iter().any(|b| {
            default_keymap()
                .shown_text(b.action, px_ui::actions::Platform::Linux)
                .is_some_and(|k| t == k)
        }) && texto.pos.x * ppp < (107 * n_de(ppp)) as f32)
}

fn es_titulo_del_menu(texto: &str) -> bool {
    TITULOS.contains(&texto.trim())
}

/// Textos pintados de la columna o el menú (por su contenido: lo que el panel desplaza hacia arriba
/// por debajo del menú tiene la misma posición pero otro recorte), con su posición en píxeles
/// físicos, en orden.
fn textos_de_columna_y_menu(f: &Fot, ppp: f32) -> Vec<(String, [i64; 2])> {
    let mut v: Vec<(String, [i64; 2])> = f
        .pintado
        .textos
        .iter()
        .filter(|t| es_titulo_del_menu(&t.texto) || es_texto_de_la_columna(t, ppp))
        .map(|t| {
            (
                t.texto.clone(),
                [
                    (t.pos.x * ppp).round() as i64,
                    (t.pos.y * ppp).round() as i64,
                ],
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn fila_10_la_columna_y_el_menu_se_pintan_recortados_a_su_rectangulo() {
    for ppp in PPPS {
        for panel_u in ALTOS_DEL_PANEL_U {
            let mut banco = banco_con_panel_de(ppp, panel_u);
            let f = banco.reposo();
            let columna = f.rects.columna.expand(TOL_PT);
            let menu = f.rects.menu.expand(TOL_PT);
            let (mut vistos_columna, mut vistos_menu) = (0, 0);
            for t in &f.pintado.textos {
                if es_titulo_del_menu(&t.texto) {
                    vistos_menu += 1;
                    assert!(
                        menu.contains_rect(t.clip),
                        "ppp {ppp} panel {panel_u} u: {:?} del menú se pinta fuera de su rectángulo: recorte {:?}",
                        t.texto,
                        t.clip
                    );
                } else if es_texto_de_la_columna(t, ppp) {
                    vistos_columna += 1;
                    assert!(
                        columna.contains_rect(t.clip),
                        "ppp {ppp} panel {panel_u} u: {:?} de la columna se pinta fuera de su rectángulo: recorte {:?}",
                        t.texto,
                        t.clip
                    );
                }
            }
            assert!(
                vistos_menu >= 4,
                "ppp {ppp} panel {panel_u} u: títulos pintados {vistos_menu}"
            );
            assert!(
                vistos_columna >= 1,
                "ppp {ppp} panel {panel_u} u: textos de la columna pintados {vistos_columna}"
            );
        }
    }
}

#[test]
fn fila_10_los_botones_de_la_columna_golpean_en_su_sitio_con_el_panel_bajo() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for panel_u in ALTOS_DEL_PANEL_U {
            let mut banco = banco_con_panel_de(ppp, panel_u);
            let x = (4 + 99 / 2) * n;
            // Con el panel desplazado a un extremo la columna sigue en su sitio.
            let layout = window_layout(banco.ventana_px, n as u32);
            banco.rueda_en(layout.panel.x + 10 * n, layout.panel.y + 10 * n, 20 * n);
            for (i, button) in TOOL_BUTTONS.iter().enumerate() {
                let y = (16 + 4 + 18 * i as i64) * n + 8 * n;
                let acciones: Vec<_> = banco.clic(x, y).iter().filter_map(|f| f.columna).collect();
                assert_eq!(
                    acciones,
                    vec![button.action],
                    "ppp {ppp} panel {panel_u} u: {}",
                    button.label
                );
            }
            for (i, button) in ZOOM_BUTTONS.iter().enumerate() {
                let y = (16 + 119 + 18 * i as i64) * n + 8 * n;
                let acciones: Vec<_> = banco.clic(x, y).iter().filter_map(|f| f.columna).collect();
                // El contenido (193 u) cabe en 200 u y en 297 u; en 100 u los botones de zoom
                // quedan fuera de la columna recortada y no golpean nada.
                let esperado = if panel_u >= 193 {
                    vec![button.action]
                } else {
                    vec![]
                };
                assert_eq!(
                    acciones, esperado,
                    "ppp {ppp} panel {panel_u} u: {}",
                    button.label
                );
            }
        }
    }
}

#[test]
fn fila_10_un_clic_en_un_deslizador_del_editor_lo_golpea_con_el_panel_en_la_pila() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for panel_u in [200, 297] {
            let mut banco = banco_con_panel_de(ppp, panel_u);
            let layout = window_layout(banco.ventana_px, n as u32);
            if panel_u == 200 {
                banco.rueda_en(layout.panel.x + 10 * n, layout.panel.y + 10 * n, 10 * n);
                assert_eq!(banco.view.offset_px, 10 * n);
            }
            let colocado = panel_layout(
                Mode::Indexed,
                banco.panel_h_px(),
                n as u32,
                banco.view.offset_px,
            );
            let fila = control_row_u(Mode::Indexed, Control::Red).expect("hay deslizador rojo");
            let y = layout.panel.y + colocado.top_px + fila * n + ROW_H_U * n / 2;
            let x_pista = layout.panel.x + TRACK_X_U * n;
            let x = x_pista + slider_x(128, TRACK_W_U * n, Control::Red.max());
            banco.mueve(x, y);
            let pulsar = banco.fotograma(vec![boton(en_puntos(ppp, x, y), true)]);
            assert_eq!(
                banco.editor.dragging_control(),
                Some(Control::Red),
                "ppp {ppp} panel {panel_u} u: el clic debe coger el deslizador rojo"
            );
            assert!(pulsar.columna.is_none() && pulsar.menu.is_none());
            let soltar = banco.fotograma(vec![boton(en_puntos(ppp, x, y), false)]);
            assert!(
                soltar.panel.commit.is_some(),
                "ppp {ppp} panel {panel_u} u: soltar aplica el color"
            );
        }
    }
}

#[test]
fn fila_10_el_editor_sigue_sin_aviso_en_la_barra_con_el_panel_bajo() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for panel_u in ALTOS_DEL_PANEL_U {
            let mut banco = banco_con_panel_de(ppp, panel_u);
            let layout = window_layout(banco.ventana_px, n as u32);
            banco.rueda_en(layout.panel.x + 10 * n, layout.panel.y + 10 * n, 12 * n);
            banco.rueda_en(layout.panel.x + 10 * n, layout.panel.y + 10 * n, -(6 * n));
            for _ in 0..3 {
                banco.reposo();
            }
            assert!(
                banco.historial.iter().all(|salida| salida.notice.is_none()),
                "ppp {ppp} panel {panel_u} u: avisos {:?}",
                banco
                    .historial
                    .iter()
                    .filter_map(|s| s.notice.clone())
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn fila_10_con_alto_igual_o_mayor_que_el_bloque_el_desplazamiento_vuelve_a_0_en_el_mismo_fotograma()
{
    for ppp in PPPS {
        let n = n_de(ppp);
        let mut banco = banco_con_panel_de(ppp, 200);
        let layout = window_layout(banco.ventana_px, n as u32);
        banco.rueda_en(layout.panel.x + 10 * n, layout.panel.y + 10 * n, 10 * n);
        assert_eq!(banco.view.offset_px, 10 * n, "ppp {ppp}");
        let bloque = block_height_px(Mode::Indexed, n as u32);
        banco.alto_de_panel(bloque);
        let f = banco.reposo();
        assert_eq!(
            banco.view.offset_px, 0,
            "ppp {ppp}: el desplazamiento vuelve a 0 en el mismo fotograma"
        );
        // Y la pila sigue siendo la de `window_layout` con la ventana nueva.
        let nuevo = window_layout(banco.ventana_px, n as u32);
        assert_eq!(
            bordes_fisicos(f.rects.panel, ppp),
            Some(esperado(nuevo.panel)),
            "ppp {ppp}"
        );
    }
}

#[test]
fn fila_10_bajar_y_subir_la_ventana_no_mueve_la_columna_ni_el_menu() {
    for ppp in PPPS {
        let n = n_de(ppp);
        let mut banco = banco_con_panel_de(ppp, 297);
        let alto = banco.reposo();
        let referencia = textos_de_columna_y_menu(&alto, ppp);
        for panel_u in [200, 100, 297, 150, 297] {
            banco.alto_de_panel(panel_u * n);
            banco.reposo();
            let f = banco.reposo();
            // La columna y el menú empiezan siempre arriba a la izquierda: sus textos no cambian
            // de sitio salvo los que se recortan por abajo; los que se ven quedan donde estaban.
            let ahora = textos_de_columna_y_menu(&f, ppp);
            for texto in ahora {
                assert!(
                    referencia.contains(&texto),
                    "ppp {ppp} panel {panel_u} u: {texto:?} cambió de sitio"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 4: conversión con el origen del lienzo desplazado
// ---------------------------------------------------------------------------------------------

fn visibles(origen: i64, extension: i64, lienzo: i64) -> i64 {
    (origen + extension).min(lienzo) - origen.max(0)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn propiedad_4_la_conversion_con_el_origen_desplazado_es_la_de_origen_cero(
        zoom_n in 1u32..=64,
        ancho in 1u32..=128,
        alto in 1u32..=128,
        lienzo_w in 1u32..=2000,
        lienzo_h in 1u32..=2000,
        n in 1u32..=3,
        celdas_x in 0i64..=400,
        celdas_y in 0i64..=400,
        puntero_x in -3000i64..=6000,
        puntero_y in -3000i64..=6000,
        desplazamiento in (-2000i64..=2000, -2000i64..=2000),
    ) {
        let k = i64::from(n);
        let lienzo = Size::new(lienzo_w, lienzo_h);
        let imagen = Size::new(ancho, alto);
        // Origen del lienzo: múltiplos de `n`, como el de la pila (340·n, 16·n).
        let (x0, y0) = (celdas_x * k, celdas_y * k);
        prop_assert_eq!(canvas_origin_px(n).0 % k, 0);
        prop_assert_eq!(canvas_origin_px(n).1 % k, 0);

        let zoom = Zoom::from_int(zoom_n).expect("zoom válido");
        let mut vista = View::fit(lienzo, imagen);
        vista.set_zoom(zoom, (i64::from(lienzo_w) / 2, i64::from(lienzo_h) / 2));
        vista.pan(desplazamiento.0, desplazamiento.1, lienzo, imagen);

        // El puntero está en coordenadas de ventana; el lienzo resta su origen.
        let en_lienzo = (puntero_x - x0, puntero_y - y0);
        let desplazado = vista.screen_to_image(en_lienzo, imagen);
        // Con origen (0,0) y el puntero ya restado, lo mismo.
        let en_cero = vista.screen_to_image((puntero_x - x0, puntero_y - y0), imagen);
        prop_assert_eq!(desplazado, en_cero);

        // Modelo: `floor((p − origen) / zoom)` si cae dentro de la imagen.
        let z = zoom.scale(1);
        let (ox, oy) = vista.origin();
        let (ix, iy) = ((en_lienzo.0 - ox).div_euclid(z), (en_lienzo.1 - oy).div_euclid(z));
        let modelo = (ix >= 0 && iy >= 0 && ix < i64::from(ancho) && iy < i64::from(alto))
            .then_some((ix as u32, iy as u32));
        prop_assert_eq!(desplazado, modelo);

        // Ida y vuelta: el píxel devuelto contiene al puntero en el rectángulo que se le asigna.
        if let Some((px, py)) = desplazado {
            let (sx, sy) = vista.image_to_screen(px, py);
            let (ex, ey) = vista.image_to_screen(px + 1, py + 1);
            prop_assert!(sx <= en_lienzo.0 && en_lienzo.0 < ex, "x: {} no está en [{}, {})", en_lienzo.0, sx, ex);
            prop_assert!(sy <= en_lienzo.1 && en_lienzo.1 < ey, "y: {} no está en [{}, {})", en_lienzo.1, sy, ey);
        }

        // `clamp` mantiene `MIN_VISIBLE` píxeles visibles (o toda la imagen / todo el lienzo).
        let extension = (zoom.scale(i64::from(ancho)), zoom.scale(i64::from(alto)));
        let minimo = |ext: i64, lienzo: i64| i64::from(MIN_VISIBLE).min(ext).min(lienzo);
        prop_assert!(
            visibles(ox, extension.0, i64::from(lienzo_w)) >= minimo(extension.0, i64::from(lienzo_w)),
            "visibles en x"
        );
        prop_assert!(
            visibles(oy, extension.1, i64::from(lienzo_h)) >= minimo(extension.1, i64::from(lienzo_h)),
            "visibles en y"
        );
    }

    #[test]
    fn propiedad_4_fit_deja_la_imagen_centrada_en_el_lienzo(
        ancho in 1u32..=128,
        alto in 1u32..=128,
        lienzo_w in 1u32..=2000,
        lienzo_h in 1u32..=2000,
    ) {
        let lienzo = Size::new(lienzo_w, lienzo_h);
        let imagen = Size::new(ancho, alto);
        let vista = View::fit(lienzo, imagen);
        let rect = vista.image_rect(imagen);
        let margen_izquierdo = rect.x;
        let margen_derecho = i64::from(lienzo_w) - rect.x - rect.width;
        let margen_superior = rect.y;
        let margen_inferior = i64::from(lienzo_h) - rect.y - rect.height;
        prop_assert!((margen_izquierdo - margen_derecho).abs() <= 1);
        prop_assert!((margen_superior - margen_inferior).abs() <= 1);
    }
}
