//! Tests de la navegación del lienzo con la tabla de gestos (specs/interfaz-raton.md, casos 5, 6,
//! 8 (en el lienzo), 10, 11 y 12 y propiedad 3): un `egui::Context` con `RawInput` sintético a 60
//! fotogramas por segundo, sin GPU ni pantalla. El lienzo es toda la pantalla de 800×600 con una
//! imagen de 64×48 (ajuste = ×12), `ppp` 1,0 salvo donde se dice.

use egui::{
    Context, Event, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, Pos2, RawInput,
    Rect, TouchPhase, Vec2, pos2, vec2,
};
use proptest::prelude::*;
use px_core::{Image, Mode};
use px_ui::canvas::{Canvas, CanvasInfo, CanvasInput};
use px_ui::color::Role;
use px_ui::gestures::{GestureAction, GestureMap, GestureRow, Input, Mods, Scope};
use px_ui::keymap::Platforms;
use px_ui::view::Zoom;

const SCREEN: Vec2 = vec2(800.0, 600.0);
const CURSOR: Pos2 = pos2(300.0, 200.0);

struct Bench {
    ctx: Context,
    canvas: Canvas,
    t: f64,
    info: CanvasInfo,
    entries: Vec<(CanvasInput, Role)>,
}

impl Bench {
    fn new_2() -> Bench {
        Bench::with_ppp(1.0)
    }

    fn with_ppp(ppp: f32) -> Bench {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        // `set_pixels_per_point` se activa al empezar el pase siguiente: un pase vacío antes de que
        // el lienzo calcule su ajuste.
        let mut empty = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
                ..RawInput::default()
            },
            |_| {},
        );
        empty.textures_delta.clear();
        let mut canvas = Canvas::new();
        canvas.set_image(&Image::new(64, 48, Mode::Rgba).expect("imagen"));
        let mut bench = Bench {
            ctx,
            canvas,
            t: 0.0,
            info: CanvasInfo {
                zoom: Zoom::default(),
                hover: None,
                image_rect: None,
                error: None,
            },
            entries: Vec::new(),
        };
        bench.step(vec![]);
        bench
    }

    /// Un banco con el cursor ya sobre el lienzo.
    fn with_cursor() -> Bench {
        let mut bench = Bench::new_2();
        bench.step(vec![Event::PointerMoved(CURSOR)]);
        bench
    }

    fn step(&mut self, events: Vec<Event>) -> Vec<(CanvasInput, Role)> {
        self.t += 1.0 / 60.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..RawInput::default()
        };
        let canvas = &mut self.canvas;
        let mut result = None;
        let mut output_2 = self.ctx.run_ui(input, |ui| {
            let entries = canvas.interact_roles(ui);
            result = Some((entries, canvas.paint(ui)));
        });
        output_2.textures_delta.clear();
        let (entries, info) = result.expect("el lienzo se dibuja");
        self.info = info;
        self.entries.extend(entries.iter().copied());
        entries
    }

    fn zoom(&self) -> Zoom {
        self.info.zoom
    }

    fn source_2(&self) -> Pos2 {
        self.info.image_rect.expect("hay imagen").min
    }
}

fn event_wheel(
    unit: MouseWheelUnit,
    delta: Vec2,
    modifiers: Modifiers,
    source: MouseWheelSource,
) -> Event {
    Event::MouseWheel {
        unit,
        delta,
        phase: TouchPhase::Move,
        source,
        modifiers,
    }
}

fn line(dy: f32) -> Event {
    event_wheel(
        MouseWheelUnit::Line,
        vec2(0.0, dy),
        Modifiers::NONE,
        MouseWheelSource::Unknown,
    )
}

fn fingers(dx: f32, dy: f32) -> Event {
    event_wheel(
        MouseWheelUnit::Point,
        vec2(dx, dy),
        Modifiers::NONE,
        MouseWheelSource::Trackpad,
    )
}

fn button_2(pos: Pos2, button: PointerButton, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers,
    }
}

fn space(pressed: bool) -> Event {
    Event::Key {
        key: egui::Key::Space,
        physical_key: Some(egui::Key::Space),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn steps_of_zoom(from_2: Zoom, until: Zoom) -> i32 {
    let (mut z, mut n) = (from_2, 0);
    while z != until && n < 200 {
        z = if until > z { z.zoom_in() } else { z.zoom_out() };
        n += if until > from_2 { 1 } else { -1 };
    }
    n
}

// --- Fila 5 ---

#[test]
fn row_5_a_notch_is_an_anchored_step_and_three_notches_at_once_are_three() {
    let mut b = Bench::with_cursor();
    let fit = b.zoom();
    let pixel = b.info.hover.expect("el cursor está sobre la imagen");
    b.step(vec![line(1.0)]);
    assert_eq!(b.zoom(), fit.zoom_in());
    assert_eq!(
        b.info.hover,
        Some(pixel),
        "el píxel bajo el cursor se conserva"
    );
    let antes = b.zoom();
    b.step(vec![line(3.0)]);
    assert_eq!(steps_of_zoom(antes, b.zoom()), 3, "antes de I.2b eran 1");
    assert_eq!(b.info.hover, Some(pixel));
    let antes = b.zoom();
    b.step(vec![line(-1.0), line(-1.0)]);
    assert_eq!(steps_of_zoom(antes, b.zoom()), -2, "un paso por muesca");
}

#[test]
fn row_5_fingers_in_points_scroll_and_do_not_zoom() {
    let mut b = Bench::with_cursor();
    let (zoom, source_2) = (b.zoom(), b.source_2());
    for _ in 0..30 {
        b.step(vec![fingers(0.0, 6.0)]);
    }
    assert_eq!(b.zoom(), zoom, "antes de I.2b eran 30 pasos de zoom");
    assert_ne!(b.source_2(), source_2, "la imagen se desplaza");
    assert!(b.entries.is_empty(), "sin eventos de trazo");
}

#[test]
fn row_5_shift_plus_wheel_scrolls_horizontally_without_zoom() {
    let mut b = Bench::with_cursor();
    let (zoom, source_2) = (b.zoom(), b.source_2());
    b.step(vec![event_wheel(
        MouseWheelUnit::Line,
        vec2(0.0, 1.0),
        Modifiers::SHIFT,
        MouseWheelSource::Unknown,
    )]);
    assert_eq!(b.zoom(), zoom);
    assert_eq!(
        b.source_2(),
        source_2 + vec2(40.0, 0.0),
        "40 puntos por muesca, a la derecha"
    );
}

#[test]
fn row_5_ctrl_plus_one_notch_gives_a_single_step() {
    let mut b = Bench::with_cursor();
    let fit = b.zoom();
    b.step(vec![event_wheel(
        MouseWheelUnit::Line,
        vec2(0.0, 1.0),
        Modifiers::COMMAND,
        MouseWheelSource::Unknown,
    )]);
    for _ in 0..40 {
        b.step(vec![]);
    }
    assert_eq!(steps_of_zoom(fit, b.zoom()), 1, "antes de I.2b eran 11");
}

#[test]
fn row_5_a_slow_pinch_gives_three_steps() {
    let mut b = Bench::with_cursor();
    let fit = b.zoom();
    for _ in 0..30 {
        b.step(vec![Event::Zoom(1.02)]);
    }
    for _ in 0..5 {
        b.step(vec![]);
    }
    assert_eq!(steps_of_zoom(fit, b.zoom()), 3, "antes de I.2b eran 30");
}

#[test]
fn row_5_without_a_cursor_over_the_canvas_the_wheel_does_nothing() {
    let mut b = Bench::new_2();
    let (zoom, source_2) = (b.zoom(), b.source_2());
    b.step(vec![line(1.0), fingers(0.0, 20.0), Event::Zoom(1.5)]);
    assert_eq!(b.zoom(), zoom);
    assert_eq!(b.source_2(), source_2);
}

// --- Fila 6 ---

#[test]
fn row_6_middle_and_space_plus_left_scroll_without_a_stroke() {
    for (button_pan, with_space) in [
        (PointerButton::Middle, false),
        (PointerButton::Primary, true),
    ] {
        let mut b = Bench::with_cursor();
        let source_2 = b.source_2();
        if with_space {
            b.step(vec![space(true)]);
        }
        b.step(vec![button_2(CURSOR, button_pan, true, Modifiers::NONE)]);
        b.step(vec![Event::PointerMoved(CURSOR + vec2(10.0, 5.0))]);
        b.step(vec![Event::PointerMoved(CURSOR + vec2(20.0, 10.0))]);
        b.step(vec![button_2(
            CURSOR + vec2(20.0, 10.0),
            button_pan,
            false,
            Modifiers::NONE,
        )]);
        assert!(b.entries.is_empty(), "{button_pan:?}: sin eventos de trazo");
        assert!(
            b.source_2().x > source_2.x && b.source_2().y > source_2.y,
            "{button_pan:?}"
        );
    }
}

#[test]
fn row_6_fingers_scroll_with_a_stroke_in_progress_and_the_stroke_continues() {
    let mut b = Bench::with_cursor();
    let source_2 = b.source_2();
    b.step(vec![button_2(
        CURSOR,
        PointerButton::Primary,
        true,
        Modifiers::NONE,
    )]);
    b.step(vec![fingers(0.0, 8.0)]);
    assert_eq!(b.source_2(), source_2 + vec2(0.0, 8.0), "la vista se mueve");
    b.step(vec![Event::PointerMoved(CURSOR + vec2(12.0, 0.0))]);
    b.step(vec![button_2(
        CURSOR + vec2(12.0, 0.0),
        PointerButton::Primary,
        false,
        Modifiers::NONE,
    )]);
    let has = |f: fn(&CanvasInput) -> bool| b.entries.iter().any(|(e, _)| f(e));
    assert!(has(|e| matches!(
        e,
        CanvasInput::Stroke(px_ui::canvas::CanvasEvent::StrokeBegin(_))
    )));
    assert!(has(|e| matches!(
        e,
        CanvasInput::Stroke(px_ui::canvas::CanvasEvent::StrokeEnd)
    )));
    assert!(!has(|e| matches!(e, CanvasInput::Escape)));
}

#[test]
fn row_6_the_scroll_is_an_integer_in_physical_pixels_and_clamp_rules() {
    for ppp in [1.0f32, 1.25, 1.5, 2.0] {
        let mut b = Bench::with_ppp(ppp);
        b.step(vec![Event::PointerMoved(CURSOR)]);
        let a = b.source_2();
        b.step(vec![fingers(0.0, 7.3)]);
        let d = (b.source_2().y - a.y) * ppp;
        assert!((d - d.round()).abs() < 1e-3, "ppp {ppp}: {d} px físicos");
        // Un desplazamiento enorme no saca la imagen del lienzo.
        b.step(vec![fingers(5000.0, 5000.0), fingers(-90000.0, 0.0)]);
        let rect = b.info.image_rect.expect("hay imagen");
        assert!(
            rect.intersects(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            "ppp {ppp}"
        );
    }
}

// --- Fila 8 (en el lienzo): tabla sustituida ---

fn row_of(input: Input, action: GestureAction) -> GestureRow {
    GestureRow {
        input,
        mods: Mods {
            command: None,
            shift: None,
            space: None,
        },
        action,
        scope: Scope::Canvas,
        active: Platforms::All,
    }
}

#[test]
fn row_8_with_an_empty_table_the_canvas_does_not_respond_to_anything() {
    let mut b = Bench::with_cursor();
    b.canvas.set_gestures(GestureMap::new(vec![]));
    let (zoom, source_2) = (b.zoom(), b.source_2());
    b.step(vec![line(1.0), fingers(0.0, 9.0), Event::Zoom(1.5)]);
    b.step(vec![button_2(
        CURSOR,
        PointerButton::Primary,
        true,
        Modifiers::NONE,
    )]);
    b.step(vec![button_2(
        CURSOR,
        PointerButton::Primary,
        false,
        Modifiers::NONE,
    )]);
    b.step(vec![button_2(
        CURSOR,
        PointerButton::Middle,
        true,
        Modifiers::NONE,
    )]);
    b.step(vec![Event::PointerMoved(CURSOR + vec2(30.0, 30.0))]);
    assert_eq!(b.zoom(), zoom);
    assert_eq!(b.source_2(), source_2);
    assert!(b.entries.is_empty());
}

#[test]
fn row_8_wheel_assigned_to_scroll_and_fingers_to_zoom() {
    let mut b = Bench::with_cursor();
    b.canvas.set_gestures(GestureMap::new(vec![
        row_of(Input::Wheel, GestureAction::Pan),
        row_of(Input::Scroll, GestureAction::ZoomStep),
    ]));
    let (zoom, source_2) = (b.zoom(), b.source_2());
    b.step(vec![line(1.0)]);
    assert_eq!(b.zoom(), zoom, "la rueda ya no hace zoom");
    assert_eq!(
        b.source_2(),
        source_2 + vec2(0.0, 40.0),
        "desplaza 40 puntos por muesca"
    );
    let antes = b.zoom();
    b.step(vec![fingers(0.0, 20.0)]);
    assert_eq!(
        b.zoom(),
        antes,
        "media muesca (20 de 40 puntos) no cruza el umbral"
    );
    b.step(vec![fingers(0.0, 20.0)]);
    assert_eq!(
        b.zoom(),
        antes.zoom_in(),
        "40 puntos de dedos = una muesca = un paso"
    );
}

// --- Fila 10 ---

#[test]
fn row_10_the_right_one_paints_with_the_background_with_and_without_space_and_alt_does_not_count() {
    for (space_below, alt) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut b = Bench::with_cursor();
        if space_below {
            b.step(vec![space(true)]);
        }
        let m = if alt { Modifiers::ALT } else { Modifiers::NONE };
        let e = b.step(vec![button_2(CURSOR, PointerButton::Secondary, true, m)]);
        assert!(
            matches!(
                e.as_slice(),
                [(
                    CanvasInput::Stroke(px_ui::canvas::CanvasEvent::StrokeBegin(_)),
                    Role::Background
                )]
            ),
            "espacio {space_below}, alt {alt}: {e:?}"
        );
    }
}

#[test]
fn row_10_alt_does_not_change_the_wheel_or_the_left_and_shift_left_draws() {
    let mut a = Bench::with_cursor();
    let mut b = Bench::with_cursor();
    a.step(vec![line(1.0)]);
    b.step(vec![event_wheel(
        MouseWheelUnit::Line,
        vec2(0.0, 1.0),
        Modifiers::ALT,
        MouseWheelSource::Unknown,
    )]);
    assert_eq!(a.zoom(), b.zoom(), "Alt no cuenta en la rueda");
    for m in [Modifiers::NONE, Modifiers::ALT, Modifiers::SHIFT] {
        let mut b = Bench::with_cursor();
        let e = b.step(vec![button_2(CURSOR, PointerButton::Primary, true, m)]);
        assert!(
            matches!(
                e.as_slice(),
                [(
                    CanvasInput::Stroke(px_ui::canvas::CanvasEvent::StrokeBegin(_)),
                    Role::Foreground
                )]
            ),
            "{m:?}: {e:?}"
        );
    }
}

// --- Fila 11 ---

#[test]
fn row_11_the_sign_is_given_by_the_system_and_the_app_does_not_invert_it() {
    // (dx, dy) del delta → cuánto se mueve la imagen: el mismo signo.
    for (dx, dy) in [(0.0, 6.0), (0.0, -6.0), (6.0, 0.0), (-6.0, 0.0)] {
        let mut b = Bench::with_cursor();
        let source_2 = b.source_2();
        b.step(vec![fingers(dx, dy)]);
        assert_eq!(b.source_2() - source_2, vec2(dx, dy), "delta ({dx}, {dy})");
    }
    // Con el desplazamiento natural de macOS el sistema ya entrega el signo contrario para el mismo
    // movimiento de dedos: movimientos opuestos, sin ramas por `source`.
    for source in [
        MouseWheelSource::Unknown,
        MouseWheelSource::Wheel,
        MouseWheelSource::Trackpad,
        MouseWheelSource::Momentum,
    ] {
        let mover = |dy: f32| {
            let mut b = Bench::with_cursor();
            let source_2 = b.source_2();
            b.step(vec![event_wheel(
                MouseWheelUnit::Point,
                vec2(0.0, dy),
                Modifiers::NONE,
                source,
            )]);
            b.source_2().y - source_2.y
        };
        assert_eq!(mover(9.0), 9.0, "{source:?}");
        assert_eq!(mover(-9.0), -9.0, "{source:?}");
    }
}

#[test]
fn row_11_shift_plus_line_adds_the_two_axes() {
    for (dx, dy, expected_2) in [
        (0.0, 1.0, 40.0),
        (1.0, 0.0, 40.0),
        (0.0, -1.0, -40.0),
        (-1.0, 0.0, -40.0),
    ] {
        let mut b = Bench::with_cursor();
        let source_2 = b.source_2();
        b.step(vec![event_wheel(
            MouseWheelUnit::Line,
            vec2(dx, dy),
            Modifiers::SHIFT,
            MouseWheelSource::Unknown,
        )]);
        assert_eq!(
            b.source_2() - source_2,
            vec2(expected_2, 0.0),
            "delta ({dx}, {dy})"
        );
    }
}

// --- Fila 12 ---

#[test]
fn row_12_the_half_pixel_remainders_are_carried() {
    // 7 eventos de 0,4 pt: a ppp 1,0 son 2,8 px (se mueven 2); a 1,5, 4,2 (4); a 2,0, 5,6 (5).
    for (ppp, physical) in [(1.0f32, 2.0f32), (1.5, 4.0), (2.0, 5.0)] {
        for sign in [1.0f32, -1.0] {
            let mut b = Bench::with_ppp(ppp);
            b.step(vec![Event::PointerMoved(CURSOR)]);
            let source_2 = b.source_2();
            for _ in 0..7 {
                b.step(vec![fingers(0.0, 0.4 * sign)]);
            }
            let moved = (b.source_2().y - source_2.y) * ppp;
            assert!(
                (moved - physical * sign).abs() < 1e-3,
                "ppp {ppp}, signo {sign}: {moved} px físicos"
            );
            assert_eq!(b.source_2().x, source_2.x);
        }
    }
}

#[test]
fn row_12_the_remainder_is_carried_per_axis() {
    let mut b = Bench::with_cursor();
    let source_2 = b.source_2();
    for _ in 0..3 {
        b.step(vec![fingers(0.4, 0.0)]);
    }
    for _ in 0..3 {
        b.step(vec![fingers(0.0, 0.4)]);
    }
    assert_eq!(
        b.source_2() - source_2,
        vec2(1.0, 1.0),
        "1,2 px en cada eje: uno entero"
    );
}

// --- Propiedad 3 ---

fn event_of_navigation() -> impl Strategy<Value = Event> {
    let modifiers_2 = (any::<bool>(), any::<bool>()).prop_map(|(c, s)| Modifiers {
        command: c,
        ctrl: c,
        shift: s,
        ..Modifiers::NONE
    });
    let unit = prop::sample::select(vec![
        MouseWheelUnit::Line,
        MouseWheelUnit::Point,
        MouseWheelUnit::Page,
    ]);
    prop_oneof![
        (unit, -6.0f32..6.0, -6.0f32..6.0, modifiers_2)
            .prop_map(|(u, x, y, m)| { event_wheel(u, vec2(x, y), m, MouseWheelSource::Unknown) }),
        (0.5f32..2.0).prop_map(Event::Zoom),
        any::<bool>().prop_map(space),
        any::<bool>().prop_map(|p| button_2(CURSOR, PointerButton::Middle, p, Modifiers::NONE)),
        (any::<bool>(), any::<bool>()).prop_map(|(a, p)| {
            let b = if a {
                PointerButton::Extra1
            } else {
                PointerButton::Extra2
            };
            button_2(CURSOR, b, p, Modifiers::NONE)
        }),
        (0.0f32..800.0, 0.0f32..600.0).prop_map(|(x, y)| Event::PointerMoved(pos2(x, y))),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 3: navegar no dibuja y la vista siempre vale.
    #[test]
    fn property_3_navigating_does_not_draw_and_the_view_is_always_valid(
        events_2 in prop::collection::vec(event_of_navigation(), 0..14),
    ) {
        let mut b = Bench::with_cursor();
        for e in events_2 {
            b.step(vec![e]);
            prop_assert!(b.zoom() >= Zoom::MIN && b.zoom() <= Zoom::MAX);
            let rect = b.info.image_rect.expect("hay imagen");
            prop_assert!(rect.intersects(Rect::from_min_size(Pos2::ZERO, SCREEN)), "{rect:?}");
        }
        b.step(vec![]);
        prop_assert!(b.entries.is_empty(), "eventos de trazo: {:?}", b.entries);
    }
}
