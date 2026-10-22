//! Tests de la pila de la ventana con sus piezas reales: menú, columna, panel de paletas, barra de
//! estado y lienzo (specs/hito-2-menu-herramientas.md, «Casos» filas 7, 9 y 10 en su parte de
//! banco de egui, y propiedad 4).
//!
//! El banco es el de `tests/panel_baja.rs` con las dos piezas nuevas: un `egui::Context` ejecuta
//! fotogramas con `RawInput` sintético, sin GPU ni pantalla. En cada fotograma, como hace la app:
//! `show_menu` (arriba), `status::show` (abajo), `show_toolbar`, `palette_panel::show_editing_view`
//! y el lienzo (`interact_ordered` y `paint`). Los rectángulos de cada pieza salen de la diferencia
//! de `ui.available_rect_before_wrap()` antes y después de pintarla (la caja y el panel están a la
//! derecha: se llama primero la caja y lo que queda disponible se encoge por su borde derecho).
//!
//! - Fila 7: los rectángulos del banco son los de `chrome::window_layout`, enteros en píxeles
//!   físicos (error de `f32` ≤ 0,001), a las cinco escalas y en la ventana por defecto (960 × 720 pt)
//!   y en la mínima; los botones golpean en su sitio dentro de la pila; el título más ancho y el
//!   desplegable de `Ver` (208 u) caben.
//! - Fila 9: el origen del lienzo está en `(0, 16·n)` (I.3): `hover` y `StrokeBegin` valen lo mismo que
//!   en un lienzo con origen (0,0) y el puntero desplazado, en la primera y la última coordenada
//!   física de cada píxel de la imagen, en las cuatro esquinas del lienzo y un píxel fuera; y
//!   `Ajustar` centra la imagen en **ese** rectángulo (no en la ventana).
//! - Fila 10 (respaldo B con la columna): con el panel de `200·n`, `100·n` y `305·n` de alto, la
//!   rueda solo mueve el panel; la columna y el menú no se desplazan y se pintan recortados a su
//!   rectángulo; los botones de la columna y los deslizadores golpean en su sitio; sin avisos en la
//!   barra; con alto ≥ bloque el desplazamiento vuelve a 0 en el mismo fotograma.
//! - Propiedad 4: conversión pantalla-imagen con el origen del lienzo desplazado (pura).
//!
//! Supuestos (la spec no los fija): `show_menu` crea su propio panel superior y `show_toolbar` el
//! suyo a la derecha, como `status::show` y `palette_panel::show` crean los suyos; los títulos del
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
use px_ui::toolbar::{TOOL_BUTTONS, ZOOM_BUTTONS};
use px_ui::view::{MIN_VISIBLE, Size, View, ViewOp, Zoom};

mod common;
use common::toolbox::{center_u, toolbox_2};
use common::{
    PPPS, Painted, button_2, context, frame, in_points, is_physical_integer_in_egui, mover, n_de,
    painted, tolerance_of_egui, wheel_points,
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

fn rgba(width_2: u32, height_2: u32) -> Image {
    Image::new(width_2, height_2, Mode::Rgba).expect("imagen válida")
}

/// Indexada de 64×48 con la paleta `[R, G, B, K]`.
fn indexed() -> Image {
    let palette = Palette::from_colors([
        Rgba::opaque(255, 0, 0),
        Rgba::opaque(0, 255, 0),
        Rgba::opaque(0, 0, 255),
        Rgba::opaque(0, 0, 0),
    ])
    .expect("paleta");
    Image::new_indexed(64, 48, palette).expect("imagen indexada")
}

/// Ventana por defecto (960 × 720 pt) y mínima, en píxeles físicos, a `ppp`.
fn windows(ppp: f32) -> [(i64, i64); 2] {
    [
        ((960.0 * ppp).round() as i64, (720.0 * ppp).round() as i64),
        (
            (min_window_width_pt(ppp) * ppp).round() as i64,
            (min_window_height_pt(ppp) * ppp).round() as i64,
        ),
    ]
}

/// Ventana cuyo panel mide `panel_h_px` de alto (menú y barra de estado aparte).
fn window_with_panel(ppp: f32, panel_h_px: i64) -> (i64, i64) {
    let n = n_de(ppp);
    (
        (960.0 * ppp).round() as i64,
        panel_h_px + (MENU_H_U + 20) * n,
    )
}

fn ui_state() -> UiState {
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
    state_2: Rect,
    column: Rect,
    panel: Rect,
    canvas_2: Rect,
}

struct TestFrame {
    rects: Rects,
    entries: Vec<CanvasInput>,
    info: CanvasInfo,
    panel: PanelOutput,
    menu: Option<AppAction>,
    column: Option<AppAction>,
    painted: Painted,
}

struct Bench {
    ctx: Context,
    ppp: f32,
    window_px: (i64, i64),
    canvas: Canvas,
    scroll: Scroll,
    view: PanelView,
    image: Image,
    choice: ColorChoice,
    slots: SlotColors,
    editor: ColorEditor,
    state_2: UiState,
    /// Lo que devolvió el panel en cada fotograma.
    history: Vec<PanelOutput>,
}

impl Bench {
    fn new_2(ppp: f32, window_px: (i64, i64), image: Image) -> Bench {
        let mut canvas = Canvas::new();
        canvas.set_image(&image);
        let mut bench = Bench {
            ctx: context(ppp),
            ppp,
            window_px,
            canvas,
            scroll: Scroll::default(),
            view: PanelView::default(),
            choice: ColorChoice::Entry(1),
            slots: SlotColors::defaults(image.mode()),
            image,
            editor: ColorEditor::default(),
            state_2: ui_state(),
            history: Vec::new(),
        };
        // Reparte el espacio, sube la textura y asienta los paneles.
        for _ in 0..3 {
            bench.rest_2_2();
        }
        bench.history.clear();
        bench
    }

    fn n(&self) -> i64 {
        n_de(self.ppp)
    }

    fn screen(&self) -> egui::Vec2 {
        vec2(
            self.window_px.0 as f32 / self.ppp,
            self.window_px.1 as f32 / self.ppp,
        )
    }

    /// Alto del panel en píxeles físicos con la ventana actual.
    fn panel_h_px(&self) -> i64 {
        self.window_px.1 - (MENU_H_U + 20) * self.n()
    }

    fn frame(&mut self, events: Vec<Event>) -> TestFrame {
        let actual = EditTarget::of(&self.image, self.choice).and_then(|target_2| {
            let color = target_2.stored(&self.image, &self.slots)?;
            Some((target_2, color))
        });
        self.editor.sync(actual);
        let enabled = !self.editor.dragging();
        let screen = self.screen();
        let Bench {
            ctx,
            canvas,
            scroll,
            view,
            image,
            choice,
            slots,
            editor,
            state_2,
            ..
        } = self;
        let mut result = None;
        let shapes_2 = frame(ctx, screen, events, |ui| {
            apply_scale(ui.ctx());
            let r0 = ui.available_rect_before_wrap();
            let menu = show_menu(ui, state_2, Platform::Linux);
            let r1 = ui.available_rect_before_wrap();
            status::show(ui, "lápiz · color 1 #000000", false);
            let r2 = ui.available_rect_before_wrap();
            let column = toolbox_2(ui, state_2, "×5");
            let r3 = ui.available_rect_before_wrap();
            let panel = palette_panel::show_editing_view(
                ui, image, *choice, scroll, view, enabled, slots, editor,
            );
            let r4 = ui.available_rect_before_wrap();
            let entries = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            let info = canvas.paint(ui);
            let rects = Rects {
                menu: Rect::from_min_max(r0.min, Pos2::new(r0.max.x, r1.min.y)),
                state_2: Rect::from_min_max(Pos2::new(r1.min.x, r2.max.y), r1.max),
                column: Rect::from_min_max(Pos2::new(r3.max.x, r2.min.y), r2.max),
                panel: Rect::from_min_max(Pos2::new(r4.max.x, r3.min.y), r3.max),
                canvas_2: r4,
            };
            result = Some((rects, entries, info, panel, menu, column));
        });
        let (rects, entries, info, panel, menu, column) = result.expect("se dibuja");
        self.history.push(panel.clone());
        TestFrame {
            rects,
            entries,
            info,
            panel,
            menu,
            column,
            painted: painted(&shapes_2),
        }
    }

    fn rest_2_2(&mut self) -> TestFrame {
        self.frame(vec![])
    }

    fn moves(&mut self, x_px: i64, y_px: i64) -> TestFrame {
        let pos = in_points(self.ppp, x_px, y_px);
        self.frame(vec![mover(pos)])
    }

    /// Mueve, pulsa y suelta; devuelve los tres fotogramas.
    fn click(&mut self, x_px: i64, y_px: i64) -> [TestFrame; 3] {
        let pos = in_points(self.ppp, x_px, y_px);
        let a = self.frame(vec![mover(pos)]);
        let b = self.frame(vec![button_2(pos, true)]);
        let c = self.frame(vec![button_2(pos, false)]);
        [a, b, c]
    }

    /// La rueda, `px_below` píxeles físicos hacia abajo, con el puntero en `(x_px, y_px)`.
    fn wheel_in(&mut self, x_px: i64, y_px: i64, px_below: i64) -> TestFrame {
        self.moves(x_px, y_px);
        let events_2 = vec![wheel_points(px_below, self.ppp)];
        self.frame(events_2)
    }

    /// Cambia el alto del panel (la ventana) para el siguiente fotograma.
    fn height_of_panel(&mut self, panel_h_px: i64) {
        self.window_px.1 = panel_h_px + (MENU_H_U + 20) * self.n();
    }
}

/// Los bordes de `rect` en píxeles físicos, o `None` si alguno no es entero con la tolerancia del
/// redondeo de egui (`common::tolerance_of_egui`, 1/32 pt: `Panel` y `allocate_rect` redondean a
/// múltiplos pares de `GUI_ROUNDING`, así que la spec «error de `f32` ≤ 0,001» no es alcanzable a
/// `ppp` 1,25 y 1,5).
fn borders_physical(rect: Rect, ppp: f32) -> Option<[i64; 4]> {
    let borders = [rect.min.x, rect.min.y, rect.max.x, rect.max.y];
    borders
        .iter()
        .all(|b| is_physical_integer_in_egui(*b, ppp))
        .then(|| borders.map(|b| (b * ppp).round() as i64))
}

fn expected_2(rect: px_ui::view::ScreenRect) -> [i64; 4] {
    [rect.x, rect.y, rect.x + rect.width, rect.y + rect.height]
}

// ---------------------------------------------------------------------------------------------
// Fila 7: la geometría del banco
// ---------------------------------------------------------------------------------------------

#[test]
fn row_7_the_bench_rectangles_are_those_of_window_layout_and_integers() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        for window in windows(ppp) {
            let mut bench = Bench::new_2(ppp, window, indexed());
            let f = bench.rest_2_2();
            let layout = window_layout(window, n);
            let pieces = [
                ("menu", f.rects.menu, layout.menu),
                ("estado", f.rects.state_2, layout.status),
                ("columna", f.rects.column, layout.toolbar),
                ("panel", f.rects.panel, layout.panel),
                ("lienzo", f.rects.canvas_2, layout.canvas),
            ];
            for (name_2, measured, expected_2_2) in pieces {
                let physical = borders_physical(measured, ppp).unwrap_or_else(|| {
                    panic!("ppp {ppp} ventana {window:?}: {name_2} no cae en píxeles enteros: {measured:?}")
                });
                assert_eq!(
                    physical,
                    expected_2(expected_2_2),
                    "ppp {ppp} ventana {window:?}: {name_2}"
                );
            }
        }
    }
}

#[test]
fn row_7_the_canvas_origin_of_the_bench_is_0_16n() {
    for ppp in PPPS {
        let n = text_scale(ppp);
        for window in windows(ppp) {
            let mut bench = Bench::new_2(ppp, window, indexed());
            let f = bench.rest_2_2();
            let [x, y, ..] = borders_physical(f.rects.canvas_2, ppp).expect("entero");
            assert_eq!((x, y), canvas_origin_px(n), "ppp {ppp}");
            assert_eq!((x, y), (0, 16 * i64::from(n)));
        }
    }
}

#[test]
fn row_7_the_column_buttons_hit_in_their_place_inside_the_stack() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for window in windows(ppp) {
            let mut bench = Bench::new_2(ppp, window, indexed());
            let toolbox_2 = window_layout(window, n as u32).toolbar;
            for (i, button) in TOOL_BUTTONS.iter().chain(ZOOM_BUTTONS.iter()).enumerate() {
                let (cx, cy) = center_u(i);
                let (x, y) = (toolbox_2.x + cx * n, (16 + cy) * n);
                let actions: Vec<_> = bench.click(x, y).iter().filter_map(|f| f.column).collect();
                assert_eq!(actions, vec![button.action], "ppp {ppp}: {}", button.label);
            }
        }
    }
}

/// Texto del menú pintado en la barra superior (los títulos).
fn title<'a>(f: &'a TestFrame, ppp: f32, n: i64, name_2: &str) -> &'a common::Text {
    f.painted
        .texts
        .iter()
        .find(|t| {
            t.text_2 == name_2 && t.rect().max.y * ppp <= (16 * n) as f32 + tolerance_of_egui(ppp)
        })
        .unwrap_or_else(|| panic!("el título {name_2} no se pinta en el menú"))
}

#[test]
fn row_7_the_menu_paints_its_five_titles_inside_its_bar_and_they_fit_in_the_minimum_width() {
    for ppp in PPPS {
        let n = n_de(ppp);
        let mut bench = Bench::new_2(ppp, windows(ppp)[1], indexed());
        let f = bench.rest_2_2();
        for name_2 in ["Archivo", "Editar", "Imagen", "Ver", "Ayuda"] {
            let t = title(&f, ppp, n, name_2);
            let r = t.rect();
            assert!(
                r.max.x * ppp <= (min_window_width_u() * n) as f32,
                "ppp {ppp}: {name_2} se sale del ancho mínimo"
            );
            assert!(r.min.y >= f.rects.menu.min.y - TOL_PT);
            assert!(r.max.y <= f.rects.menu.max.y + TOL_PT);
        }
        // El primero: 4 u de margen y 6 u de margen del texto, y 1 u arriba.
        let file = title(&f, ppp, n, "Archivo");
        assert!(
            (file.pos.x * ppp - (10 * n) as f32).abs() <= TOL_MENU_PX,
            "ppp {ppp}: x = {}",
            file.pos.x * ppp
        );
        assert!(
            (file.pos.y * ppp - n as f32).abs() <= TOL_MENU_PX,
            "ppp {ppp}: y = {}",
            file.pos.y * ppp
        );
        // El último título termina antes de 4 + 249 u más la separación entre botones.
        let help = title(&f, ppp, n, "Ayuda");
        assert!(help.rect().max.x * ppp <= ((4 + 289) * n) as f32 + 4.0 * n as f32);
    }
}

#[test]
fn row_7_the_view_dropdown_measures_208_u_with_rows_of_16_u() {
    for ppp in PPPS {
        let n = n_de(ppp);
        let mut bench = Bench::new_2(ppp, windows(ppp)[0], indexed());
        let f = bench.rest_2_2();
        let center = title(&f, ppp, n, "Ver").rect().center();
        let (x, y) = (
            (center.x * ppp).round() as i64,
            (center.y * ppp).round() as i64,
        );
        bench.click(x, y);
        bench.rest_2_2();
        let f = bench.rest_2_2();
        let searches = |text_2: &str| -> common::Text {
            f.painted
                .texts
                .iter()
                .find(|t| {
                    // El desplegable cae sobre el lienzo, no sobre la columna (que tiene sus
                    // propios `Acercar`, `Alejar` y `1:1`).
                    t.text_2.trim() == text_2
                        && f.rects.menu.max.y <= t.pos.y
                        && t.pos.x < f.rects.column.min.x
                })
                .unwrap_or_else(|| {
                    panic!(
                        "ppp {ppp}: el desplegable de Ver no pinta {text_2:?}; hay {:?}",
                        f.painted.all_the_texts()
                    )
                })
                .clone()
        };
        // Filas de 16 u: el paso entre rótulos consecutivos es de 16 u (sin separación).
        let rows_2 = ["Acercar", "Alejar", "Ajustar a la ventana", "1:1"].map(searches);
        for pair in rows_2.windows(2) {
            let step = (pair[1].pos.y - pair[0].pos.y) * ppp;
            assert!(
                (step - (16 * n) as f32).abs() <= TOL_MENU_PX,
                "ppp {ppp}: {:?} y {:?} distan {step} px y no {}",
                pair[0].text_2,
                pair[1].text_2,
                16 * n
            );
        }
        // Ancho 208 u = 6 u de margen + (rótulo + 2 + atajo)·7 u + 6 u de margen: del comienzo del
        // rótulo más largo al final de su atajo hay (20 + 2 + 6)·7 = 196 u.
        let fit_2 = searches("Ajustar a la ventana");
        let start_2 = searches("Inicio");
        let width_2 = (start_2.rect().max.x - fit_2.rect().min.x) * ppp;
        assert!(
            (width_2 - (196 * n) as f32).abs() <= TOL_MENU_PX,
            "ppp {ppp}: del rótulo al final del atajo hay {width_2} px y no {}",
            196 * n
        );
        // Las demás filas llevan su atajo alineado a la derecha, en el mismo borde.
        let border = start_2.rect().max.x * ppp;
        for shortcut_2 in [">", "<", "Ctrl+1"] {
            let t = searches(shortcut_2);
            assert!(
                (t.rect().max.x * ppp - border).abs() <= TOL_MENU_PX,
                "ppp {ppp}: el atajo {shortcut_2:?} no está alineado a la derecha"
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
fn inside_of_the_pixel(ppp: f32, x_px: i64, y_px: i64) -> Pos2 {
    Pos2::new((x_px as f32 + 0.25) / ppp, (y_px as f32 + 0.25) / ppp)
}

/// Lienzo suelto, con el origen en (0,0) y el tamaño del lienzo de la pila: la referencia.
struct Reference {
    ctx: Context,
    canvas: Canvas,
    ppp: f32,
    screen: egui::Vec2,
}

impl Reference {
    fn new_2(ppp: f32, canvas_px: (i64, i64), image: &Image) -> Reference {
        let mut canvas = Canvas::new();
        canvas.set_image(image);
        let mut reference = Reference {
            ctx: context(ppp),
            canvas,
            ppp,
            screen: vec2(canvas_px.0 as f32 / ppp, canvas_px.1 as f32 / ppp),
        };
        for _ in 0..3 {
            reference.frame(vec![]);
        }
        reference
    }

    fn frame(&mut self, events: Vec<Event>) -> (CanvasInfo, Vec<CanvasInput>) {
        let mut result = None;
        let canvas = &mut self.canvas;
        frame(&self.ctx, self.screen, events, |ui| {
            let entries = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            let info = canvas.paint(ui);
            result = Some((info, entries));
        });
        result.expect("se dibuja")
    }

    fn hover(&mut self, x_px: i64, y_px: i64) -> Option<(u32, u32)> {
        let pos = inside_of_the_pixel(self.ppp, x_px, y_px);
        self.frame(vec![mover(pos)]).0.hover
    }

    /// El `StrokeBegin` de un clic en `(x_px, y_px)`, si lo hay.
    fn starts(&mut self, x_px: i64, y_px: i64) -> Option<(i32, i32)> {
        let pos = inside_of_the_pixel(self.ppp, x_px, y_px);
        self.frame(vec![mover(pos)]);
        let b = self.frame(vec![button_2(pos, true)]).1;
        self.frame(vec![button_2(pos, false)]);
        start_of(&b)
    }
}

impl Bench {
    fn hover(&mut self, x_px: i64, y_px: i64) -> Option<(u32, u32)> {
        let pos = inside_of_the_pixel(self.ppp, x_px, y_px);
        self.frame(vec![mover(pos)]).info.hover
    }

    fn starts(&mut self, x_px: i64, y_px: i64) -> Option<(i32, i32)> {
        let pos = inside_of_the_pixel(self.ppp, x_px, y_px);
        self.frame(vec![mover(pos)]);
        let b = self.frame(vec![button_2(pos, true)]).entries;
        self.frame(vec![button_2(pos, false)]);
        start_of(&b)
    }
}

fn start_of(entries: &[CanvasInput]) -> Option<(i32, i32)> {
    entries.iter().find_map(|e| match e {
        CanvasInput::Stroke(CanvasEvent::StrokeBegin(p)) => Some(*p),
        _ => None,
    })
}

/// Origen de la imagen respecto al lienzo y píxeles físicos por píxel de imagen.
fn image_on_the_canvas(f: &TestFrame, ppp: f32) -> ((i64, i64), i64) {
    let rect = f.info.image_rect.expect("hay imagen");
    let physical = |v: f32| (v * ppp).round() as i64;
    (
        (
            physical(rect.min.x - f.rects.canvas_2.min.x),
            physical(rect.min.y - f.rects.canvas_2.min.y),
        ),
        f.info.zoom.scale(1),
    )
}

#[test]
fn row_9_hover_at_the_first_and_the_last_coordinate_of_each_pixel_is_worth_what_it_is_with_origin_zero()
 {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let window = windows(ppp)[0];
        let mut bench = Bench::new_2(ppp, window, rgba(6, 4));
        let f0 = bench.rest_2_2();
        let (source_canvas_x, source_canvas_y) = canvas_origin_px(n as u32);
        let canvas_2 = window_layout(window, n as u32).canvas;
        let mut reference = Reference::new_2(ppp, (canvas_2.width, canvas_2.height), &rgba(6, 4));
        let (source_2, z) = image_on_the_canvas(&f0, ppp);
        assert!(z > 1, "ppp {ppp}: la imagen de 6×4 se amplía (×{z})");
        for iy in 0..4i64 {
            for ix in 0..6i64 {
                for (dx, dy) in [(0, 0), (z - 1, z - 1), (z - 1, 0), (0, z - 1)] {
                    let (x, y) = (source_2.0 + ix * z + dx, source_2.1 + iy * z + dy);
                    let expected_2 = Some((ix as u32, iy as u32));
                    assert_eq!(
                        bench.hover(source_canvas_x + x, source_canvas_y + y),
                        expected_2,
                        "ppp {ppp}: pila, píxel ({ix}, {iy}) en ({dx}, {dy})"
                    );
                    assert_eq!(
                        reference.hover(x, y),
                        expected_2,
                        "ppp {ppp}: referencia, píxel ({ix}, {iy}) en ({dx}, {dy})"
                    );
                }
            }
        }
    }
}

#[test]
fn row_9_stroke_begin_at_each_pixel_is_worth_what_it_is_with_origin_zero() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let window = windows(ppp)[0];
        let mut bench = Bench::new_2(ppp, window, rgba(6, 4));
        let f0 = bench.rest_2_2();
        let canvas_2 = window_layout(window, n as u32).canvas;
        let mut reference = Reference::new_2(ppp, (canvas_2.width, canvas_2.height), &rgba(6, 4));
        let (source_2, z) = image_on_the_canvas(&f0, ppp);
        for iy in 0..4i64 {
            for ix in 0..6i64 {
                for (dx, dy) in [(0, 0), (z - 1, z - 1)] {
                    let (x, y) = (source_2.0 + ix * z + dx, source_2.1 + iy * z + dy);
                    let expected_2 = Some((ix as i32, iy as i32));
                    assert_eq!(
                        bench.starts(canvas_origin_px(n as u32).0 + x, 16 * n + y),
                        expected_2,
                        "ppp {ppp}: pila, píxel ({ix}, {iy}) en ({dx}, {dy})"
                    );
                    assert_eq!(
                        reference.starts(x, y),
                        expected_2,
                        "ppp {ppp}: referencia, píxel ({ix}, {iy}) en ({dx}, {dy})"
                    );
                }
            }
        }
    }
}

#[test]
fn row_9_at_the_four_corners_of_the_canvas_and_one_pixel_outside() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        // Ventana mínima y una imagen de 1200 × 1200: a ×1 cubre el lienzo entero (el mayor mide
        // 384 × 1053 a ppp 3 desde I.2a), así que las esquinas del lienzo caen sobre la imagen.
        let window = windows(ppp)[1];
        let canvas_2 = window_layout(window, n as u32).canvas;
        let (w, h) = (canvas_2.width, canvas_2.height);
        let image_2 = rgba(1200, 1200);
        let mut bench = Bench::new_2(ppp, window, image_2.clone());
        // Ajustar da ahora un nivel 1/K para esta imagen: se pasa a ×1, que es el zoom en el que la
        // imagen cubre el lienzo entero.
        bench.canvas.request_view(ViewOp::Actual);
        let f0 = bench.rest_2_2();
        let (source_2, z) = image_on_the_canvas(&f0, ppp);
        assert_eq!(z, 1, "ppp {ppp}");
        let mut reference = Reference::new_2(ppp, (w, h), &image_2);
        reference.canvas.request_view(ViewOp::Actual);
        reference.frame(vec![]);
        let (ox, oy) = canvas_origin_px(n as u32);

        for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
            let expected_2 = Some(((x - source_2.0) as u32, (y - source_2.1) as u32));
            assert_eq!(
                bench.hover(ox + x, oy + y),
                expected_2,
                "ppp {ppp}: esquina ({x}, {y})"
            );
            assert_eq!(
                reference.hover(x, y),
                expected_2,
                "ppp {ppp}: esquina ({x}, {y}) (referencia)"
            );
            let begin = Some(((x - source_2.0) as i32, (y - source_2.1) as i32));
            assert_eq!(
                bench.starts(ox + x, oy + y),
                begin,
                "ppp {ppp}: StrokeBegin en ({x}, {y})"
            );
            assert_eq!(
                reference.starts(x, y),
                begin,
                "ppp {ppp}: StrokeBegin en ({x}, {y}) (referencia)"
            );
        }
        // Un píxel fuera del rectángulo del lienzo: `None` y sin trazo.
        for (x, y) in [(-1, 5), (w, 5), (5, -1), (5, h), (-1, -1), (w, h)] {
            assert_eq!(
                bench.hover(ox + x, oy + y),
                None,
                "ppp {ppp}: ({x}, {y}) fuera"
            );
            assert_eq!(
                bench.starts(ox + x, oy + y),
                None,
                "ppp {ppp}: StrokeBegin fuera en ({x}, {y})"
            );
            assert_eq!(
                reference.hover(x, y),
                None,
                "ppp {ppp}: ({x}, {y}) fuera (referencia)"
            );
            assert_eq!(
                reference.starts(x, y),
                None,
                "ppp {ppp}: StrokeBegin fuera en ({x}, {y}) (referencia)"
            );
        }
    }
}

#[test]
fn row_9_fit_centers_the_image_in_the_canvas_rectangle_and_not_in_the_window() {
    for ppp in PPPS {
        let n = i64::from(text_scale(ppp));
        let window = windows(ppp)[0];
        let mut bench = Bench::new_2(ppp, window, rgba(64, 48));
        let canvas_2 = window_layout(window, n as u32).canvas;
        // Aleja la imagen de su sitio: acerca con la rueda sobre una esquina del lienzo.
        let corner_x = canvas_2.x + 20;
        let corner_y = canvas_2.y + 20;
        bench.moves(corner_x, corner_y);
        for _ in 0..3 {
            bench.frame(vec![common::wheel_line(true)]);
        }
        bench.canvas.request_view(ViewOp::Fit);
        let f = bench.rest_2_2();
        let fit = View::fit(
            Size::new(canvas_2.width as u32, canvas_2.height as u32),
            Size::new(64, 48),
        );
        assert_eq!(f.info.zoom, fit.zoom(), "ppp {ppp}");
        let rect = f.info.image_rect.expect("hay imagen");
        let center_canvas = f.rects.canvas_2.center();
        assert!(
            (rect.center().x - center_canvas.x).abs() * ppp <= 1.0 + 0.01
                && (rect.center().y - center_canvas.y).abs() * ppp <= 1.0 + 0.01,
            "ppp {ppp}: la imagen {:?} no está centrada en el lienzo {:?}",
            rect.center(),
            center_canvas
        );
        // Y el centro del lienzo no es el de la ventana: el panel y el menú lo desplazan.
        let center_window = Rect::from_min_size(Pos2::ZERO, bench.screen()).center();
        assert!(
            (rect.center().x - center_window.x).abs() >= 1.0,
            "ppp {ppp}: centrada en la ventana en vez de en el lienzo"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 10: respaldo B con la columna
// ---------------------------------------------------------------------------------------------

/// Los tres altos de la fila 10, en `u`.
const PANEL_HEIGHTS_U: [i64; 3] = [200, 100, 305];

fn bench_with_panel_of(ppp: f32, panel_u: i64) -> Bench {
    let n = n_de(ppp);
    Bench::new_2(ppp, window_with_panel(ppp, panel_u * n), indexed())
}

#[test]
fn row_10_only_the_wheel_over_the_panel_moves_offset_px() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for panel_u in [200, 100] {
            let mut bench = bench_with_panel_of(ppp, panel_u);
            let layout = window_layout(bench.window_px, n as u32);
            assert!(
                panel_layout(Mode::Indexed, bench.panel_h_px(), n as u32, 0).scrolling,
                "ppp {ppp} panel {panel_u} u: el panel debería desplazarse"
            );
            assert_eq!(bench.view.offset_px, 0);

            // Sobre la columna: nada.
            let (cx, cy) = (layout.toolbar.x + 2 * n, layout.toolbar.y + 10 * n);
            bench.wheel_in(cx, cy, 10 * n);
            assert_eq!(
                bench.view.offset_px, 0,
                "ppp {ppp} panel {panel_u} u: la columna no desplaza"
            );
            // Sobre el menú: nada.
            bench.wheel_in(100 * n, 4 * n, 10 * n);
            assert_eq!(
                bench.view.offset_px, 0,
                "ppp {ppp} panel {panel_u} u: el menú no desplaza"
            );
            // Sobre la barra de estado: nada.
            bench.wheel_in(100 * n, layout.status.y + 4 * n, 10 * n);
            assert_eq!(
                bench.view.offset_px, 0,
                "ppp {ppp} panel {panel_u} u: la barra no desplaza"
            );
            // Sobre el lienzo: nada.
            bench.wheel_in(layout.canvas.x + 20 * n, layout.canvas.y + 20 * n, 10 * n);
            assert_eq!(
                bench.view.offset_px, 0,
                "ppp {ppp} panel {panel_u} u: el lienzo no desplaza el panel"
            );
            // Sobre el panel: sí.
            bench.wheel_in(layout.panel.x + 10 * n, layout.panel.y + 10 * n, 10 * n);
            assert_eq!(
                bench.view.offset_px,
                10 * n,
                "ppp {ppp} panel {panel_u} u: la rueda sobre el panel desplaza"
            );
        }
    }
}

#[test]
fn row_10_the_column_and_the_menu_do_not_scroll_with_any_offset_px() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for panel_u in PANEL_HEIGHTS_U {
            let mut bench = bench_with_panel_of(ppp, panel_u);
            let layout = window_layout(bench.window_px, n as u32);
            let antes = bench.rest_2_2();
            let texts_before: Vec<(String, [i64; 2])> = column_and_menu_texts(&antes, ppp);
            bench.wheel_in(layout.panel.x + 10 * n, layout.panel.y + 10 * n, 15 * n);
            let after_2 = bench.rest_2_2();
            if panel_u < 305 {
                assert_eq!(bench.view.offset_px, 15 * n, "ppp {ppp} panel {panel_u} u");
            }
            // Mismos rectángulos con cualquier offset.
            assert_eq!(antes.rects, after_2.rects, "ppp {ppp} panel {panel_u} u");
            // Mismos textos en los mismos sitios.
            assert_eq!(
                texts_before,
                column_and_menu_texts(&after_2, ppp),
                "ppp {ppp} panel {panel_u} u: la columna o el menú se movieron"
            );
        }
    }
}

const TITLES: [&str; 4] = ["Archivo", "Editar", "Ver", "Ayuda"];

/// Textos de la columna: rótulos, nivel de zoom y letras de atajo (estas por contenido y por estar
/// dentro de la columna: la `L` del deslizador de luminosidad es del panel).
fn is_column_text(text_2: &common::Text, column: Rect) -> bool {
    let t = text_2.text_2.trim();
    TOOL_BUTTONS.iter().any(|b| t == b.label)
        || ZOOM_BUTTONS.iter().any(|b| t == b.label)
        || t.starts_with("Zoom ")
        || (TOOL_BUTTONS.iter().any(|b| {
            default_keymap()
                .shown_text(b.action, px_ui::actions::Platform::Linux)
                .is_some_and(|k| t == k)
        }) && text_2.pos.x >= column.min.x - TOL_PT)
}

fn is_menu_title(text_2: &str) -> bool {
    TITLES.contains(&text_2.trim())
}

/// Textos pintados de la columna o el menú (por su contenido: lo que el panel desplaza hacia arriba
/// por debajo del menú tiene la misma posición pero otro recorte), con su posición en píxeles
/// físicos, en orden.
fn column_and_menu_texts(f: &TestFrame, ppp: f32) -> Vec<(String, [i64; 2])> {
    let mut v: Vec<(String, [i64; 2])> = f
        .painted
        .texts
        .iter()
        .filter(|t| is_menu_title(&t.text_2) || is_column_text(t, f.rects.column))
        .map(|t| {
            (
                t.text_2.clone(),
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
fn row_10_the_column_and_the_menu_are_painted_clipped_to_their_rectangle() {
    for ppp in PPPS {
        for panel_u in PANEL_HEIGHTS_U {
            let mut bench = bench_with_panel_of(ppp, panel_u);
            let f = bench.rest_2_2();
            let column = f.rects.column.expand(TOL_PT);
            let menu = f.rects.menu.expand(TOL_PT);
            let (mut seen_column, mut seen_menu) = (0, 0);
            for t in &f.painted.texts {
                if is_menu_title(&t.text_2) {
                    seen_menu += 1;
                    assert!(
                        menu.contains_rect(t.clip),
                        "ppp {ppp} panel {panel_u} u: {:?} del menú se pinta fuera de su rectángulo: recorte {:?}",
                        t.text_2,
                        t.clip
                    );
                } else if is_column_text(t, f.rects.column) {
                    seen_column += 1;
                    assert!(
                        column.contains_rect(t.clip),
                        "ppp {ppp} panel {panel_u} u: {:?} de la columna se pinta fuera de su rectángulo: recorte {:?}",
                        t.text_2,
                        t.clip
                    );
                }
            }
            assert!(
                seen_menu >= 4,
                "ppp {ppp} panel {panel_u} u: títulos pintados {seen_menu}"
            );
            assert!(
                seen_column >= 1,
                "ppp {ppp} panel {panel_u} u: textos de la columna pintados {seen_column}"
            );
        }
    }
}

#[test]
fn row_10_the_column_buttons_hit_in_their_place_with_the_low_panel() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for panel_u in PANEL_HEIGHTS_U {
            let mut bench = bench_with_panel_of(ppp, panel_u);
            // Con el panel desplazado a un extremo la columna sigue en su sitio.
            let layout = window_layout(bench.window_px, n as u32);
            bench.wheel_in(layout.panel.x + 10 * n, layout.panel.y + 10 * n, 20 * n);
            for (i, button) in TOOL_BUTTONS.iter().enumerate() {
                let (cx, cy) = center_u(i);
                let (x, y) = (layout.toolbar.x + cx * n, (16 + cy) * n);
                let actions: Vec<_> = bench.click(x, y).iter().filter_map(|f| f.column).collect();
                assert_eq!(
                    actions,
                    vec![button.action],
                    "ppp {ppp} panel {panel_u} u: {}",
                    button.label
                );
            }
            for (i, button) in ZOOM_BUTTONS.iter().enumerate() {
                let (cx, cy) = center_u(TOOL_BUTTONS.len() + i);
                let (x, y) = (layout.toolbar.x + cx * n, (16 + cy) * n);
                let actions: Vec<_> = bench.click(x, y).iter().filter_map(|f| f.column).collect();
                // La fila de zoom (y = 80 a 104 u) cabe en 200 u y en 305 u y, con 100 u, su centro
                // (y = 92 u) sigue dentro de la columna recortada.
                assert_eq!(
                    actions,
                    vec![button.action],
                    "ppp {ppp} panel {panel_u} u: {}",
                    button.label
                );
            }
            // Con un panel de 60 u la fila de zoom queda fuera de la columna recortada y no golpea
            // nada.
            let mut under = bench_with_panel_of(ppp, 60);
            let layout = window_layout(under.window_px, n as u32);
            for (i, button) in ZOOM_BUTTONS.iter().enumerate() {
                let (cx, cy) = center_u(TOOL_BUTTONS.len() + i);
                let (x, y) = (layout.toolbar.x + cx * n, (16 + cy) * n);
                let actions: Vec<_> = under.click(x, y).iter().filter_map(|f| f.column).collect();
                assert!(actions.is_empty(), "ppp {ppp} panel 60 u: {}", button.label);
            }
        }
    }
}

#[test]
fn row_10_a_click_on_an_editor_slider_hits_it_with_the_panel_in_the_stack() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for panel_u in [200, 305] {
            let mut bench = bench_with_panel_of(ppp, panel_u);
            let layout = window_layout(bench.window_px, n as u32);
            if panel_u == 200 {
                bench.wheel_in(layout.panel.x + 10 * n, layout.panel.y + 10 * n, 10 * n);
                assert_eq!(bench.view.offset_px, 10 * n);
            }
            let placed = panel_layout(
                Mode::Indexed,
                bench.panel_h_px(),
                n as u32,
                bench.view.offset_px,
            );
            let row_2 = control_row_u(Mode::Indexed, Control::Red).expect("hay deslizador rojo");
            let y = layout.panel.y + placed.top_px + row_2 * n + ROW_H_U * n / 2;
            let x_hint = layout.panel.x + TRACK_X_U * n;
            let x = x_hint + slider_x(128, TRACK_W_U * n, Control::Red.max());
            bench.moves(x, y);
            let pulsar = bench.frame(vec![button_2(in_points(ppp, x, y), true)]);
            assert_eq!(
                bench.editor.dragging_control(),
                Some(Control::Red),
                "ppp {ppp} panel {panel_u} u: el clic debe coger el deslizador rojo"
            );
            assert!(pulsar.column.is_none() && pulsar.menu.is_none());
            let release_2 = bench.frame(vec![button_2(in_points(ppp, x, y), false)]);
            assert!(
                release_2.panel.commit.is_some(),
                "ppp {ppp} panel {panel_u} u: soltar aplica el color"
            );
        }
    }
}

#[test]
fn row_10_the_editor_continues_without_a_notice_in_the_bar_with_the_low_panel() {
    for ppp in PPPS {
        let n = n_de(ppp);
        for panel_u in PANEL_HEIGHTS_U {
            let mut bench = bench_with_panel_of(ppp, panel_u);
            let layout = window_layout(bench.window_px, n as u32);
            bench.wheel_in(layout.panel.x + 10 * n, layout.panel.y + 10 * n, 12 * n);
            bench.wheel_in(layout.panel.x + 10 * n, layout.panel.y + 10 * n, -(6 * n));
            for _ in 0..3 {
                bench.rest_2_2();
            }
            assert!(
                bench
                    .history
                    .iter()
                    .all(|output_2| output_2.notice.is_none()),
                "ppp {ppp} panel {panel_u} u: avisos {:?}",
                bench
                    .history
                    .iter()
                    .filter_map(|s| s.notice.clone())
                    .collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn row_10_with_height_equal_to_or_greater_than_the_block_the_scroll_goes_back_to_0_in_the_same_frame()
 {
    for ppp in PPPS {
        let n = n_de(ppp);
        let mut bench = bench_with_panel_of(ppp, 200);
        let layout = window_layout(bench.window_px, n as u32);
        bench.wheel_in(layout.panel.x + 10 * n, layout.panel.y + 10 * n, 10 * n);
        assert_eq!(bench.view.offset_px, 10 * n, "ppp {ppp}");
        let block = block_height_px(Mode::Indexed, n as u32);
        bench.height_of_panel(block);
        let f = bench.rest_2_2();
        assert_eq!(
            bench.view.offset_px, 0,
            "ppp {ppp}: el desplazamiento vuelve a 0 en el mismo fotograma"
        );
        // Y la pila sigue siendo la de `window_layout` con la ventana nueva.
        let new_2 = window_layout(bench.window_px, n as u32);
        assert_eq!(
            borders_physical(f.rects.panel, ppp),
            Some(expected_2(new_2.panel)),
            "ppp {ppp}"
        );
    }
}

#[test]
fn row_10_lowering_and_raising_the_window_does_not_move_the_column_or_the_menu() {
    for ppp in PPPS {
        let n = n_de(ppp);
        let mut bench = bench_with_panel_of(ppp, 305);
        let height_2 = bench.rest_2_2();
        let reference = column_and_menu_texts(&height_2, ppp);
        for panel_u in [200, 100, 305, 150, 305] {
            bench.height_of_panel(panel_u * n);
            bench.rest_2_2();
            let f = bench.rest_2_2();
            // La columna y el menú empiezan siempre arriba a la izquierda: sus textos no cambian
            // de sitio salvo los que se recortan por abajo; los que se ven quedan donde estaban.
            let now = column_and_menu_texts(&f, ppp);
            for text_2 in now {
                assert!(
                    reference.contains(&text_2),
                    "ppp {ppp} panel {panel_u} u: {text_2:?} cambió de sitio"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 4: conversión con el origen del lienzo desplazado
// ---------------------------------------------------------------------------------------------

fn visible_2(source_2: i64, extension: i64, canvas_2: i64) -> i64 {
    (source_2 + extension).min(canvas_2) - source_2.max(0)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_4_the_conversion_with_the_shifted_origin_is_the_one_with_origin_zero(
        zoom_n in 1u32..=64,
        width_2 in 1u32..=128,
        height_2 in 1u32..=128,
        canvas_w in 1u32..=2000,
        canvas_h in 1u32..=2000,
        n in 1u32..=3,
        cels_x in 0i64..=400,
        cels_y in 0i64..=400,
        pointer_x in -3000i64..=6000,
        pointer_y in -3000i64..=6000,
        pan in (-2000i64..=2000, -2000i64..=2000),
    ) {
        let k = i64::from(n);
        let canvas_2 = Size::new(canvas_w, canvas_h);
        let image_2 = Size::new(width_2, height_2);
        // Origen del lienzo: múltiplos de `n`, como el de la pila (0, 16·n).
        let (x0, y0) = (cels_x * k, cels_y * k);
        prop_assert_eq!(canvas_origin_px(n).0 % k, 0);
        prop_assert_eq!(canvas_origin_px(n).1 % k, 0);

        let zoom = Zoom::from_int(zoom_n).expect("zoom válido");
        let mut vista = View::fit(canvas_2, image_2);
        vista.set_zoom(zoom, (i64::from(canvas_w) / 2, i64::from(canvas_h) / 2));
        vista.pan(pan.0, pan.1, canvas_2, image_2);

        // El puntero está en coordenadas de ventana; el lienzo resta su origen.
        let in_canvas = (pointer_x - x0, pointer_y - y0);
        let shifted = vista.screen_to_image(in_canvas, image_2);
        // Con origen (0,0) y el puntero ya restado, lo mismo.
        let in_zero = vista.screen_to_image((pointer_x - x0, pointer_y - y0), image_2);
        prop_assert_eq!(shifted, in_zero);

        // Modelo: `floor((p − source_2) / zoom)` si cae dentro de la imagen.
        let z = zoom.scale(1);
        let (ox, oy) = vista.origin();
        let (ix, iy) = ((in_canvas.0 - ox).div_euclid(z), (in_canvas.1 - oy).div_euclid(z));
        let model = (ix >= 0 && iy >= 0 && ix < i64::from(width_2) && iy < i64::from(height_2))
            .then_some((ix as u32, iy as u32));
        prop_assert_eq!(shifted, model);

        // Ida y vuelta: el píxel devuelto contiene al puntero en el rectángulo que se le asigna.
        if let Some((px, py)) = shifted {
            let (sx, sy) = vista.image_to_screen(px, py);
            let (ex, ey) = vista.image_to_screen(px + 1, py + 1);
            prop_assert!(sx <= in_canvas.0 && in_canvas.0 < ex, "x: {} no está en [{}, {})", in_canvas.0, sx, ex);
            prop_assert!(sy <= in_canvas.1 && in_canvas.1 < ey, "y: {} no está en [{}, {})", in_canvas.1, sy, ey);
        }

        // `clamp` mantiene `MIN_VISIBLE` píxeles visibles (o toda la imagen / todo el lienzo).
        let extension = (zoom.scale(i64::from(width_2)), zoom.scale(i64::from(height_2)));
        let minimum_2 = |ext: i64, canvas_2: i64| i64::from(MIN_VISIBLE).min(ext).min(canvas_2);
        prop_assert!(
            visible_2(ox, extension.0, i64::from(canvas_w)) >= minimum_2(extension.0, i64::from(canvas_w)),
            "visibles en x"
        );
        prop_assert!(
            visible_2(oy, extension.1, i64::from(canvas_h)) >= minimum_2(extension.1, i64::from(canvas_h)),
            "visibles en y"
        );
    }

    #[test]
    fn property_4_fit_leaves_the_image_centered_in_the_canvas(
        width_2 in 1u32..=128,
        height_2 in 1u32..=128,
        canvas_w in 1u32..=2000,
        canvas_h in 1u32..=2000,
    ) {
        let canvas_2 = Size::new(canvas_w, canvas_h);
        let image_2 = Size::new(width_2, height_2);
        let vista = View::fit(canvas_2, image_2);
        let rect = vista.image_rect(image_2);
        let margin_left = rect.x;
        let margin_right = i64::from(canvas_w) - rect.x - rect.width;
        let margin_upper = rect.y;
        let margin_lower = i64::from(canvas_h) - rect.y - rect.height;
        prop_assert!((margin_left - margin_right).abs() <= 1);
        prop_assert!((margin_upper - margin_lower).abs() <= 1);
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad I.3 (specs/interfaz-caja-derecha.md): caja y panel a la derecha
// ---------------------------------------------------------------------------------------------

mod toolbox_right {
    use super::*;
    use egui::PointerButton;
    use px_ui::chrome::{OPTIONS_BODY_X_U, OPTIONS_BODY_Y_U};
    use px_ui::palette_grid::RIGHT_LINE_X_U;
    use px_ui::theme::SURFACE_3;

    use super::common::{button_of_2, wheel_line};

    fn rect_in_points(r: px_ui::view::ScreenRect, ppp: f32) -> Rect {
        Rect::from_min_size(
            Pos2::new(r.x as f32 / ppp, r.y as f32 / ppp),
            vec2(r.width as f32 / ppp, r.height as f32 / ppp),
        )
    }

    /// Tolerancia, en píxeles físicos, de un texto respecto de su posición ideal: egui coloca algunos
    /// (el campo hexadecimal) con márgenes redondeados a 1/16 pt, a unas centésimas de píxel, y
    /// pinta el texto en el píxel entero más cercano; borroso sería medio píxel.
    const TOL_BLURRY_PX: f32 = 0.1;

    /// Distancia de `valor` (px físicos) al múltiplo de `n` más cercano.
    fn rest_of_n(valor: f32, n: i64) -> f32 {
        let rest_2 = valor.rem_euclid(n as f32);
        rest_2.min(n as f32 - rest_2)
    }

    /// Fila 1 (condición 5 del visto bueno): con el ancho de la ventana cambiando de píxel en
    /// píxel (también impar) y las cinco escalas, ninguna zona cae en medio píxel y el texto de la
    /// caja y del panel queda a un múltiplo de `n` desde la esquina de su zona (nada borroso).
    #[test]
    fn row_1_with_odd_window_widths_no_area_falls_in_the_middle_of_a_pixel_or_leaves_the_text_blurry()
     {
        for ppp in PPPS {
            let n = n_de(ppp);
            let minimum = windows(ppp)[1];
            for extra in 0..=17 {
                let window = (minimum.0 + extra, minimum.1 + 2 * extra);
                let mut bench = Bench::new_2(ppp, window, indexed());
                let f = bench.rest_2_2();
                let layout = window_layout(window, n as u32);
                let pieces = [
                    ("menu", f.rects.menu, layout.menu),
                    ("estado", f.rects.state_2, layout.status),
                    ("columna", f.rects.column, layout.toolbar),
                    ("panel", f.rects.panel, layout.panel),
                    ("lienzo", f.rects.canvas_2, layout.canvas),
                ];
                for (name_2, measured, expected_2_2) in pieces {
                    let physical = borders_physical(measured, ppp).unwrap_or_else(|| {
                        panic!(
                            "ppp {ppp} ventana {window:?}: {name_2} en medio píxel: {measured:?}"
                        )
                    });
                    assert_eq!(
                        physical,
                        expected_2(expected_2_2),
                        "ppp {ppp} ventana {window:?}: {name_2}"
                    );
                }
                for (name_2, zone) in [("caja", layout.toolbar), ("panel", layout.panel)] {
                    let in_points = rect_in_points(zone, ppp).expand(TOL_PT);
                    let texts: Vec<_> = f
                        .painted
                        .texts
                        .iter()
                        .filter(|t| in_points.contains_rect(t.rect()))
                        .collect();
                    assert!(!texts.is_empty(), "ppp {ppp}: {name_2} sin texto");
                    for t in texts {
                        for (axis, pos, source_2) in
                            [("x", t.pos.x, zone.x), ("y", t.pos.y, zone.y)]
                        {
                            let rel = pos * ppp - source_2 as f32;
                            assert!(
                                rest_of_n(rel, n) <= TOL_BLURRY_PX,
                                "ppp {ppp} ventana {window:?}: {name_2}: {:?} en {axis} = {rel} px \
                                 desde su esquina (n = {n}): borroso",
                                t.text_2
                            );
                        }
                    }
                }
            }
        }
    }

    /// Fila 8: sobre el panel y sobre la caja no hay `hover` ni trazo, aunque la imagen cubra todo.
    #[test]
    fn row_8_over_the_panel_and_the_toolbox_there_is_no_hover_or_stroke() {
        for ppp in PPPS {
            let n = i64::from(text_scale(ppp));
            // Ventana mínima y una imagen de 1200 × 1200 a ×1: cubre el lienzo entero.
            let window = windows(ppp)[1];
            let layout = window_layout(window, n as u32);
            let mut bench = Bench::new_2(ppp, window, rgba(1200, 1200));
            bench.canvas.request_view(ViewOp::Actual);
            bench.rest_2_2();
            let y = layout.canvas.y + 100 * n;
            // Último píxel del lienzo: sí; primero del panel y de la caja y sus centros: no.
            let ultimo = layout.canvas.x + layout.canvas.width - 1;
            assert!(
                bench.hover(ultimo, y).is_some(),
                "ppp {ppp}: el borde del lienzo"
            );
            for (name_2, x) in [
                ("primer píxel del panel", layout.panel.x),
                ("centro del panel", layout.panel.x + layout.panel.width / 2),
                ("primer píxel de la caja", layout.toolbar.x),
                (
                    "centro de la caja",
                    layout.toolbar.x + layout.toolbar.width / 2,
                ),
            ] {
                assert_eq!(bench.hover(x, y), None, "ppp {ppp}: {name_2}");
                assert_eq!(bench.starts(x, y), None, "ppp {ppp}: trazo en {name_2}");
            }
        }
    }

    /// Fila 8: la imagen ajustada queda centrada en el lienzo, a la izquierda del panel.
    #[test]
    fn row_8_fit_leaves_the_image_to_the_left_of_the_panel_and_centered_in_the_canvas() {
        for ppp in PPPS {
            let n = i64::from(text_scale(ppp));
            let window = windows(ppp)[0];
            let layout = window_layout(window, n as u32);
            let mut bench = Bench::new_2(ppp, window, rgba(64, 48));
            bench.canvas.request_view(ViewOp::Fit);
            let f = bench.rest_2_2();
            let image_2 = f.info.image_rect.expect("hay imagen");
            assert!(
                image_2.max.x * ppp <= layout.panel.x as f32 + 0.01,
                "ppp {ppp}: la imagen invade el panel"
            );
            assert!(image_2.min.x >= -0.01, "ppp {ppp}");
            let center = f.rects.canvas_2.center();
            assert!(
                (image_2.center().x - center.x).abs() * ppp <= 1.01
                    && (image_2.center().y - center.y).abs() * ppp <= 1.01,
                "ppp {ppp}"
            );
        }
    }

    /// Fila 9: la rueda sobre la caja (botones y cuerpo del recuadro) no hace zoom ni desplaza el
    /// panel, con el respaldo B y con el alto del bloque (305 u).
    #[test]
    fn row_9_the_wheel_over_the_toolbox_does_neither_zoom_nor_scroll_the_panel() {
        for ppp in PPPS {
            let n = n_de(ppp);
            for panel_u in [200, 100, 305] {
                let mut bench = bench_with_panel_of(ppp, panel_u);
                let layout = window_layout(bench.window_px, n as u32);
                let zoom_antes = bench.rest_2_2().info.zoom;
                for (name_2, x, y) in [
                    (
                        "un botón",
                        layout.toolbar.x + 20 * n,
                        layout.toolbar.y + 10 * n,
                    ),
                    (
                        "el cuerpo de opciones",
                        layout.toolbar.x + (OPTIONS_BODY_X_U + 10) * n,
                        layout.toolbar.y + (OPTIONS_BODY_Y_U + 10) * n,
                    ),
                ] {
                    bench.moves(x, y);
                    bench.frame(vec![wheel_line(false)]);
                    bench.frame(vec![wheel_line(true)]);
                    bench.wheel_in(x, y, 10 * n);
                    assert_eq!(
                        bench.view.offset_px, 0,
                        "ppp {ppp} panel {panel_u} u: {name_2}"
                    );
                    assert_eq!(
                        bench.rest_2_2().info.zoom,
                        zoom_antes,
                        "ppp {ppp} panel {panel_u} u: {name_2} no hace zoom"
                    );
                }
            }
        }
    }

    /// Fila 9: con el alto del bloque (305 u) el panel no se desplaza ni con la rueda encima.
    #[test]
    fn row_9_with_305_u_the_panel_does_not_scroll() {
        for ppp in PPPS {
            let n = n_de(ppp);
            let mut bench = bench_with_panel_of(ppp, 305);
            let layout = window_layout(bench.window_px, n as u32);
            assert!(!panel_layout(Mode::Indexed, bench.panel_h_px(), n as u32, 0).scrolling);
            bench.wheel_in(layout.panel.x + 10 * n, layout.panel.y + 10 * n, 10 * n);
            assert_eq!(bench.view.offset_px, 0, "ppp {ppp}");
        }
    }

    /// Fila 9: todo lo pintado en la caja (también el recuadro de opciones) se recorta a la caja
    /// aunque la ventana sea más baja que su contenido, y no se mueve con el panel.
    #[test]
    fn row_9_the_options_box_is_clipped_to_the_toolbox_with_the_low_panel() {
        for ppp in PPPS {
            let n = n_de(ppp);
            for panel_u in [100, 200, 305] {
                let mut bench = bench_with_panel_of(ppp, panel_u);
                let antes = bench.rest_2_2();
                let layout = window_layout(bench.window_px, n as u32);
                bench.wheel_in(layout.panel.x + 10 * n, layout.panel.y + 10 * n, 20 * n);
                let f = bench.rest_2_2();
                let column = f.rects.column.expand(TOL_PT);
                let in_the_toolbox = |p: &Painted| {
                    p.texts
                        .iter()
                        .filter(|t| t.pos.x >= f.rects.column.min.x - TOL_PT)
                        .map(|t| {
                            (
                                t.text_2.clone(),
                                (t.pos.x * ppp).round() as i64,
                                (t.pos.y * ppp).round() as i64,
                            )
                        })
                        .collect::<Vec<_>>()
                };
                let mut now = in_the_toolbox(&f.painted);
                let mut antes_t = in_the_toolbox(&antes.painted);
                now.sort();
                antes_t.sort();
                assert_eq!(
                    now, antes_t,
                    "ppp {ppp} panel {panel_u} u: la caja se movió"
                );
                assert!(!now.is_empty(), "ppp {ppp} panel {panel_u} u");
                for t in f
                    .painted
                    .texts
                    .iter()
                    .filter(|t| t.pos.x >= f.rects.column.min.x - TOL_PT)
                {
                    assert!(
                        column.contains_rect(t.clip),
                        "ppp {ppp} panel {panel_u} u: {:?} se pinta fuera de la caja: {:?}",
                        t.text_2,
                        t.clip
                    );
                }
            }
        }
    }

    /// Fila 9: el clic derecho golpea en su sitio dentro de la pila (los nueve botones).
    #[test]
    fn row_9_the_right_click_hits_in_its_place_inside_the_stack() {
        for ppp in PPPS {
            let n = n_de(ppp);
            let window = windows(ppp)[0];
            let layout = window_layout(window, n as u32);
            let mut bench = Bench::new_2(ppp, window, indexed());
            let buttons = TOOL_BUTTONS.iter().chain(ZOOM_BUTTONS.iter()).enumerate();
            for (i, button) in buttons {
                let (cx, cy) = center_u(i);
                let pos = in_points(ppp, layout.toolbar.x + cx * n, (16 + cy) * n);
                let mut actions = Vec::new();
                for events in [
                    vec![mover(pos)],
                    vec![button_of_2(pos, PointerButton::Secondary, true)],
                    vec![button_of_2(pos, PointerButton::Secondary, false)],
                ] {
                    actions.extend(bench.frame(events).column);
                }
                let expected_2 = match button.label {
                    "Acercar" => vec![AppAction::ZoomOut],
                    "Alejar" => vec![AppAction::ZoomIn],
                    "Ajustar" | "1:1" => vec![],
                    _ => vec![button.action],
                };
                assert_eq!(actions, expected_2, "ppp {ppp}: {}", button.label);
            }
        }
    }

    /// Fila 10: línea de 1 u nueva en el borde izquierdo del panel, la de la derecha como hoy, y
    /// todo lo que pintan el panel y la caja cae dentro de sus rectángulos.
    #[test]
    fn row_10_the_panel_paints_its_left_line_the_right_one_and_everything_inside_its_rectangle() {
        for ppp in PPPS {
            let n = n_de(ppp);
            for window in windows(ppp) {
                let mut bench = Bench::new_2(ppp, window, indexed());
                let f = bench.rest_2_2();
                let layout = window_layout(window, n as u32);
                let panel = layout.panel;
                let line = |x: i64| {
                    f.painted.fills.iter().any(|r| {
                        r.fill == SURFACE_3
                            && (r.rect.min.x * ppp - x as f32).abs() <= tolerance_of_egui(ppp)
                            && ((r.rect.max.x - r.rect.min.x) * ppp - n as f32).abs()
                                <= tolerance_of_egui(ppp)
                            && (r.rect.min.y * ppp - panel.y as f32).abs() <= tolerance_of_egui(ppp)
                            && ((r.rect.max.y - r.rect.min.y) * ppp - panel.height as f32).abs()
                                <= tolerance_of_egui(ppp)
                    })
                };
                assert!(line(panel.x), "ppp {ppp} {window:?}: línea izquierda");
                assert!(
                    line(panel.x + RIGHT_LINE_X_U * n),
                    "ppp {ppp} {window:?}: línea derecha"
                );
                // Todo lo pintado a la derecha del lienzo cae dentro del panel y la caja.
                let union = rect_in_points(panel, ppp)
                    .union(rect_in_points(layout.toolbar, ppp))
                    .expand(TOL_PT);
                // Solo la franja central: el menú y la barra de estado ocupan todo el ancho.
                let band = |min_y: f32, max_y: f32| {
                    min_y * ppp >= panel.y as f32 - 0.5
                        && max_y * ppp <= (panel.y + panel.height) as f32 + 0.5
                };
                for r in f.painted.fills.iter().filter(|r| {
                    r.rect.center().x * ppp > panel.x as f32 && band(r.rect.min.y, r.rect.max.y)
                }) {
                    assert!(union.contains_rect(r.rect), "ppp {ppp}: {:?} fuera", r.rect);
                }
                for t in f.painted.texts.iter().filter(|t| {
                    t.rect().center().x * ppp > panel.x as f32 && band(t.pos.y, t.rect().max.y)
                }) {
                    assert!(union.contains_rect(t.clip), "ppp {ppp}: {:?}", t.text_2);
                }
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Propiedad 2: con el lienzo desde el origen `(0, Y0)` y un ancho cualquiera, la
        /// conversión es la de origen (0, 0) con el puntero restado, de ida y vuelta, y `fit`
        /// centra.
        #[test]
        fn property_2_the_conversion_with_the_canvas_from_x_0_and_any_width_is_exact(
            zoom_n in 1u32..=64,
            width_2 in 1u32..=128,
            height_2 in 1u32..=128,
            canvas_w in 1u32..=2001,
            canvas_h in 1u32..=2000,
            n in 1u32..=3,
            cels_y in 0i64..=40,
            pointer_x in -100i64..=2200,
            pointer_y in -100i64..=2200,
            pan in (-2000i64..=2000, -2000i64..=2000),
        ) {
            let k = i64::from(n);
            let canvas_2 = Size::new(canvas_w, canvas_h);
            let image_2 = Size::new(width_2, height_2);
            let (x0, y0) = (canvas_origin_px(n).0, cels_y * k);
            prop_assert_eq!(x0, 0);
            prop_assert_eq!(y0 % k, 0);
            let zoom = Zoom::from_int(zoom_n).expect("zoom válido");
            let mut vista = View::fit(canvas_2, image_2);
            vista.set_zoom(zoom, (i64::from(canvas_w) / 2, i64::from(canvas_h) / 2));
            vista.pan(pan.0, pan.1, canvas_2, image_2);
            let in_canvas = (pointer_x - x0, pointer_y - y0);
            prop_assert_eq!(
                vista.screen_to_image(in_canvas, image_2),
                vista.screen_to_image((pointer_x, pointer_y - y0), image_2)
            );
            if let Some((px, py)) = vista.screen_to_image(in_canvas, image_2) {
                let (sx, sy) = vista.image_to_screen(px, py);
                let (ex, ey) = vista.image_to_screen(px + 1, py + 1);
                prop_assert!(sx <= in_canvas.0 && in_canvas.0 < ex);
                prop_assert!(sy <= in_canvas.1 && in_canvas.1 < ey);
            }
            let fitted = View::fit(canvas_2, image_2).image_rect(image_2);
            let (left, right) = (fitted.x, i64::from(canvas_w) - fitted.x - fitted.width);
            prop_assert!((left - right).abs() <= 1);
        }
    }
}
