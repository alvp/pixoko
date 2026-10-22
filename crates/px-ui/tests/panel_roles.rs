//! Unidad I.2a, filas 5 y 6 en el panel (specs/interfaz-dos-colores.md): `palette_panel::show_roles`
//! con el banco de egui sin ventana (el de `tests/palette_panel.rs`, reducido). Clic y clic
//! derecho en las muestras, anillos de los dos colores, texto de la muestra, indicador de color
//! (clic izquierdo: cuentagotas; clic derecho: foco al editor y rol) y los dos selectores.
//!
//! Las posiciones salen de las constantes y funciones de `palette_grid`; el panel empieza en
//! `(0, 0)`. Las pruebas de anillos usan `ppp` 1,0 (un punto es un píxel físico).

use egui::epaint::{ClippedShape, Shape};
use egui::{
    Color32, Context, Event, MissingGlyphPolicy, Modifiers, PointerButton, Pos2, RawInput, Rect,
    Vec2, pos2, vec2,
};
use px_core::{Image, Palette, Rgba};
use px_ui::color::{ColorChoice, Role, SlotColors};
use px_ui::color_edit::{ColorEditor, EditTarget};
use px_ui::color_editor::hex_field_id;
use px_ui::palette_grid::{
    CELL_U, COLUMNS, GRID_X_U, GRID_Y_U, PITCH_U, PanelView, Scroll, corner_rects, indicator_rects,
    ring_rects, selector_rect,
};
use px_ui::palette_ops::PaletteOps;
use px_ui::palette_panel::{Indicator, PanelOutput, Roles, show_roles};
use px_ui::text::{apply_scale, install_fonts, text_scale};
use px_ui::theme::{RING_ACTIVE, RING_BACKGROUND};
use px_ui::view::ScreenRect;

#[path = "common/zones.rs"]
mod zones;
use zones::panel_on_the_left;

const TEST_SCREEN: Vec2 = vec2(1200.0, 1000.0);

fn indexed(count: usize) -> Image {
    let colors_2: Vec<Rgba> = (0..count)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 5) as u8))
        .collect();
    let palette = Palette::from_colors(colors_2).expect("paleta");
    Image::new_indexed(8, 8, palette).expect("imagen")
}

struct Bench {
    ctx: Context,
    scroll: Scroll,
    view: PanelView,
    editor: ColorEditor,
    ops: PaletteOps,
}

struct TestFrame {
    output_2: PanelOutput,
    shapes_2: Vec<ClippedShape>,
}

impl Bench {
    fn new_2(ppp: f32) -> Bench {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
        install_fonts(&ctx);
        let bench = Bench {
            ctx,
            scroll: Scroll::default(),
            view: PanelView::default(),
            editor: ColorEditor::default(),
            ops: PaletteOps::default(),
        };
        for _ in 0..2 {
            let output_2 = bench
                .ctx
                .run_ui(Bench::input_2(vec![]), |ui| apply_scale(ui.ctx()));
            output_2.drop_without_applying_deltas();
        }
        bench
    }

    fn input_2(events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, TEST_SCREEN)),
            events,
            ..RawInput::default()
        }
    }

    fn n(&self) -> i64 {
        i64::from(text_scale(self.ctx.pixels_per_point()))
    }

    fn frame(
        &mut self,
        image: &Image,
        roles: Roles,
        enabled: bool,
        events: Vec<Event>,
    ) -> TestFrame {
        let slots = SlotColors::defaults(image.mode());
        let editing = match roles.editing {
            Role::Foreground => roles.foreground,
            Role::Background => roles.background,
        };
        if let Some(target) = EditTarget::of(image, editing) {
            let color = target.stored(image, &slots).expect("color");
            self.editor.sync(Some((target, color)));
        }
        let scroll = &mut self.scroll;
        let view = &mut self.view;
        let editor = &mut self.editor;
        let ops = &mut self.ops;
        let mut result = None;
        let mut output_2 = self.ctx.run_ui(Bench::input_2(events), |ui| {
            apply_scale(ui.ctx());
            let (panel, ()) = panel_on_the_left(
                ui,
                |ui| show_roles(ui, image, roles, scroll, view, enabled, &slots, editor, ops),
                |_| (),
            );
            result = Some(panel);
        });
        output_2.textures_delta.clear();
        let shapes_2 = std::mem::take(&mut output_2.shapes);
        TestFrame {
            output_2: result.expect("el panel se pinta"),
            shapes_2,
        }
    }

    /// Mueve el puntero a `pos` y luego ejecuta `events` en otro fotograma.
    fn with_pointer(
        &mut self,
        image: &Image,
        roles: Roles,
        enabled: bool,
        pos: Pos2,
        events: Vec<Event>,
    ) -> TestFrame {
        self.frame(image, roles, enabled, vec![Event::PointerMoved(pos)]);
        self.frame(image, roles, enabled, events)
    }

    /// Pulsa `button` en `(x, y)` píxeles físicos y devuelve lo que el panel dijo al pulsar.
    fn click(
        &mut self,
        image: &Image,
        roles: Roles,
        enabled: bool,
        button: PointerButton,
        (x, y): (i64, i64),
    ) -> PanelOutput {
        let ppp = self.ctx.pixels_per_point();
        let pos = pos2(x as f32 / ppp, y as f32 / ppp);
        let pressed_2 = self
            .with_pointer(
                image,
                roles,
                enabled,
                pos,
                vec![button_2(pos, button, true)],
            )
            .output_2;
        self.frame(image, roles, enabled, vec![button_2(pos, button, false)]);
        pressed_2
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

fn roles(foreground: ColorChoice, background: ColorChoice) -> Roles {
    Roles {
        foreground,
        background,
        editing: Role::Foreground,
    }
}

fn center_of_swatch(n: i64, index_2: usize) -> (i64, i64) {
    let column = (index_2 % COLUMNS) as i64;
    let row_2 = (index_2 / COLUMNS) as i64;
    (
        (GRID_X_U + column * PITCH_U) * n + CELL_U * n / 2,
        (GRID_Y_U + row_2 * PITCH_U) * n + CELL_U * n / 2,
    )
}

fn collects(shape: &Shape, output_2: &mut Vec<(Rect, Color32)>) {
    match shape {
        Shape::Rect(rect) => output_2.push((rect.rect, rect.fill)),
        Shape::Vec(shapes_2) => shapes_2.iter().for_each(|f| collects(f, output_2)),
        _ => {}
    }
}

fn rectangles(shapes_2: &[ClippedShape]) -> Vec<(Rect, Color32)> {
    let mut output_2 = Vec::new();
    for shape_2 in shapes_2 {
        collects(&shape_2.shape, &mut output_2);
    }
    output_2
}

fn texts(shapes_2: &[ClippedShape]) -> Vec<String> {
    fn rec(shape: &Shape, output_2: &mut Vec<String>) {
        match shape {
            Shape::Text(text_2) => output_2.push(text_2.galley.text().to_string()),
            Shape::Vec(shapes_2) => shapes_2.iter().for_each(|f| rec(f, output_2)),
            _ => {}
        }
    }
    let mut output_2 = Vec::new();
    for shape_2 in shapes_2 {
        rec(&shape_2.shape, &mut output_2);
    }
    output_2
}

/// Rectángulos pintados con `color` (a `ppp` 1,0, en píxeles físicos de la ventana).
fn with_color(shapes_2: &[ClippedShape], color: Color32) -> Vec<Rect> {
    rectangles(shapes_2)
        .into_iter()
        .filter(|(_, fill)| *fill == color)
        .map(|(rect, _)| rect)
        .collect()
}

/// La esquina superior izquierda de la muestra `index_2` a `ppp` 1,0.
fn cel(index_2: usize) -> ScreenRect {
    let column = (index_2 % COLUMNS) as i64;
    let row_2 = (index_2 / COLUMNS) as i64;
    ScreenRect {
        x: GRID_X_U + column * PITCH_U,
        y: GRID_Y_U + row_2 * PITCH_U,
        width: CELL_U,
        height: CELL_U,
    }
}

fn a_rect(r: ScreenRect) -> Rect {
    Rect::from_min_size(
        pos2(r.x as f32, r.y as f32),
        vec2(r.width as f32, r.height as f32),
    )
}

// ---------------------------------------------------------------------------------------------
// Fila 5: clic, clic derecho y anillos
// ---------------------------------------------------------------------------------------------

#[test]
fn row_5_click_chooses_the_foreground_and_right_click_the_background() {
    let image = indexed(256);
    let mut bench = Bench::new_2(1.0);
    let r = roles(ColorChoice::Entry(5), ColorChoice::Entry(9));
    let left_2 = bench.click(
        &image,
        r,
        true,
        PointerButton::Primary,
        center_of_swatch(1, 200),
    );
    assert_eq!(left_2.choice, Some(ColorChoice::Entry(200)));
    assert_eq!(left_2.background, None);
    let right = bench.click(
        &image,
        r,
        true,
        PointerButton::Secondary,
        center_of_swatch(1, 200),
    );
    assert_eq!(right.background, Some(ColorChoice::Entry(200)));
    assert_eq!(right.choice, None);
}

#[test]
fn row_5_in_rgba_the_right_click_chooses_the_slot_as_the_background() {
    let image = Image::new(64, 48, px_core::Mode::Rgba).expect("imagen");
    let mut bench = Bench::new_2(1.0);
    let r = roles(ColorChoice::Slot(1), ColorChoice::Slot(9));
    let right = bench.click(
        &image,
        r,
        true,
        PointerButton::Secondary,
        center_of_swatch(1, 2),
    );
    assert_eq!(right.background, Some(ColorChoice::Slot(3)));
}

#[test]
fn row_5_the_middle_the_gap_what_is_outside_and_enabled_false_choose_none() {
    let image = indexed(256);
    let mut bench = Bench::new_2(1.0);
    let r = roles(ColorChoice::Entry(5), ColorChoice::Entry(9));
    let gap = (GRID_X_U + CELL_U + 1, GRID_Y_U + 1);
    for (button, place, enabled) in [
        (PointerButton::Middle, center_of_swatch(1, 20), true),
        (PointerButton::Primary, gap, true),
        (PointerButton::Secondary, gap, true),
        (PointerButton::Secondary, (1100, 900), true),
        (PointerButton::Primary, center_of_swatch(1, 20), false),
        (PointerButton::Secondary, center_of_swatch(1, 20), false),
    ] {
        let output_2 = bench.click(&image, r, enabled, button, place);
        assert_eq!(output_2.choice, None, "{button:?} {place:?} {enabled}");
        assert_eq!(output_2.background, None, "{button:?} {place:?} {enabled}");
    }
}

#[test]
fn row_5_the_two_rings_each_surround_their_swatch() {
    let image = indexed(256);
    let mut bench = Bench::new_2(1.0);
    let r = roles(ColorChoice::Entry(3), ColorChoice::Entry(40));
    let f = bench.frame(&image, r, true, vec![]);
    let greens = with_color(&f.shapes_2, RING_ACTIVE);
    for expected_2 in ring_rects(cel(3), 2) {
        assert!(greens.contains(&a_rect(expected_2)), "falta {expected_2:?}");
    }
    let yellows = with_color(&f.shapes_2, RING_BACKGROUND);
    let expected_2_2 = corner_rects(cel(40), 0, 2, 5);
    assert_eq!(yellows.len(), 8, "{yellows:?}");
    for expected_2 in expected_2_2 {
        assert!(
            yellows.contains(&a_rect(expected_2)),
            "falta {expected_2:?}"
        );
    }
}

#[test]
fn row_5_with_the_same_swatch_the_foreground_goes_inside_and_the_background_outside() {
    let image = indexed(256);
    let mut bench = Bench::new_2(1.0);
    let r = roles(ColorChoice::Entry(7), ColorChoice::Entry(7));
    let f = bench.frame(&image, r, true, vec![]);
    let greens = with_color(&f.shapes_2, RING_ACTIVE);
    let yellows = with_color(&f.shapes_2, RING_BACKGROUND);
    for expected_2 in ring_rects(cel(7), 2) {
        assert!(greens.contains(&a_rect(expected_2)));
    }
    for expected_2 in corner_rects(cel(7), 2, 2, 5) {
        assert!(yellows.contains(&a_rect(expected_2)));
    }
    for v in &greens {
        for a in &yellows {
            assert!(!v.intersect(*a).is_positive(), "{v:?} y {a:?}");
        }
    }
}

#[test]
fn row_5_line_2_says_foreground_and_background_for_the_swatch_of_the_two() {
    let image = indexed(256);
    let mut bench = Bench::new_2(1.0);
    let r = roles(ColorChoice::Entry(7), ColorChoice::Entry(7));
    let f = bench.frame(&image, r, true, vec![]);
    let texts = texts(&f.shapes_2);
    assert!(
        texts.iter().any(|t| t == "frente · fondo"),
        "textos: {texts:?}"
    );
    // Con el cursor en otra muestra, la línea 2 habla de esa muestra y no del fondo.
    let pos = pos2(
        center_of_swatch(1, 20).0 as f32,
        center_of_swatch(1, 20).1 as f32,
    );
    let f = bench.with_pointer(&image, r, true, pos, vec![]);
    let texts = texts_of(&f);
    assert!(texts.iter().any(|t| t == "cursor"), "textos: {texts:?}");
}

fn texts_of(f: &TestFrame) -> Vec<String> {
    texts(&f.shapes_2)
}

// ---------------------------------------------------------------------------------------------
// Fila 6: indicador y selector
// ---------------------------------------------------------------------------------------------

fn center(r: &ScreenRect) -> (i64, i64) {
    (r.x + r.width / 2, r.y + r.height / 2)
}

#[test]
fn row_6_left_click_on_the_indicator_asks_for_the_dropper_in_the_two_rectangles() {
    let image = indexed(16);
    for ppp in [1.0, 2.0, 3.0] {
        let mut bench = Bench::new_2(ppp);
        let n = bench.n();
        let (fg, bg) = indicator_rects(n as u32);
        let r = roles(ColorChoice::Entry(2), ColorChoice::Entry(3));
        for point in [center(&fg), (bg.x + bg.width - 1, bg.y + bg.height - 1)] {
            let output_2 = bench.click(&image, r, true, PointerButton::Primary, point);
            assert_eq!(output_2.indicator, Some(Indicator::Dropper), "ppp {ppp}");
            assert_eq!(output_2.choice, None);
        }
    }
}

#[test]
fn row_6_right_click_takes_the_focus_to_the_editor_with_the_role_of_the_pressed_rectangle() {
    let image = indexed(16);
    let mut bench = Bench::new_2(1.0);
    let (fg, bg) = indicator_rects(1);
    let r = roles(ColorChoice::Entry(2), ColorChoice::Entry(3));
    let output_2 = bench.click(&image, r, true, PointerButton::Secondary, center(&fg));
    assert_eq!(
        output_2.indicator,
        Some(Indicator::FocusEditor(Role::Foreground))
    );
    let output_2 = bench.click(
        &image,
        r,
        true,
        PointerButton::Secondary,
        (bg.x + bg.width - 1, bg.y + bg.height - 1),
    );
    assert_eq!(
        output_2.indicator,
        Some(Indicator::FocusEditor(Role::Background))
    );
    // El foco está en el campo hexadecimal del editor.
    bench.frame(&image, r, true, vec![]);
    assert_eq!(bench.ctx.memory(|m| m.focused()), Some(hex_field_id()));
}

#[test]
fn row_6_a_click_in_the_gap_of_the_band_or_outside_does_nothing_and_enabled_false_neither() {
    let image = indexed(16);
    let mut bench = Bench::new_2(1.0);
    let (fg, bg) = indicator_rects(1);
    let r = roles(ColorChoice::Entry(2), ColorChoice::Entry(3));
    let corner_empty = (bg.x + bg.width - 1, fg.y);
    for button in [PointerButton::Primary, PointerButton::Secondary] {
        let output_2 = bench.click(&image, r, true, button, corner_empty);
        assert_eq!(output_2.indicator, None, "{button:?}");
        assert_eq!(output_2.editing, None);
        let output_2 = bench.click(&image, r, false, button, center(&fg));
        assert_eq!(output_2.indicator, None, "{button:?} deshabilitado");
    }
}

#[test]
fn row_6_the_selectors_change_the_editor_target_and_exclude_each_other() {
    let image = indexed(16);
    let mut bench = Bench::new_2(1.0);
    let r = roles(ColorChoice::Entry(2), ColorChoice::Entry(3));
    for role in [Role::Background, Role::Foreground] {
        let row_2 = selector_rect(role, 1);
        let output_2 = bench.click(&image, r, true, PointerButton::Primary, center(&row_2));
        assert_eq!(output_2.editing, Some(role));
        assert_eq!(output_2.indicator, None);
    }
}

#[test]
fn row_6_the_small_one_is_painted_after_the_large_one_and_with_its_colors() {
    let image = indexed(16);
    let mut bench = Bench::new_2(1.0);
    let r = roles(ColorChoice::Entry(2), ColorChoice::Entry(3));
    let f = bench.frame(&image, r, true, vec![]);
    let (fg, bg) = indicator_rects(1);
    let palette = image.palette().expect("paleta");
    let color = |i: u8| {
        let c = palette.resolved(i).expect("entrada");
        Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
    };
    let all_2 = rectangles(&f.shapes_2);
    let pos_de = |rect: Rect, fill: Color32| {
        all_2
            .iter()
            .position(|(r, c)| *r == rect && *c == fill)
            .unwrap_or_else(|| panic!("no se pinta {rect:?} {fill:?}"))
    };
    let p_bg = pos_de(a_rect(bg), color(3));
    let p_fg = pos_de(a_rect(fg), color(2));
    assert!(p_fg > p_bg, "el pequeño va encima");
}
