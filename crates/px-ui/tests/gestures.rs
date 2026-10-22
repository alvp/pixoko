//! Tests de la tabla de gestos del ratón y del trackpad (specs/interfaz-raton.md, casos 1 a 4 y 9
//! y propiedades 1 y 2). Sin ventana: `gestures` no usa egui y `pointer_events` solo usa
//! `egui::Event` como dato.

use egui::{Event, Modifiers, MouseWheelUnit, PointerButton, Pos2, Vec2};
use proptest::prelude::*;
use px_ui::actions::{AppAction, Platform};
use px_ui::color::Role;
use px_ui::gestures::{
    Button, Conflict, GestureAction, GestureMap, GestureRow, Input, Mods, PINCH_STEP_LN, Pointing,
    Scope, StepAccumulator, WHEEL_STEP,
};
use px_ui::keymap::Platforms;
use px_ui::pointer_events::{button_of, pointer_presses, wheel_of};

const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];
const BUTTONS: [Button; 5] = [
    Button::Left,
    Button::Middle,
    Button::Right,
    Button::Back,
    Button::Forward,
];

fn mods(command: Option<bool>, shift: Option<bool>, space: Option<bool>) -> Mods {
    Mods {
        command,
        shift,
        space,
    }
}

fn row_2(input: Input, mods: Mods, action: GestureAction, scope: Scope) -> GestureRow {
    GestureRow {
        input,
        mods,
        action,
        scope,
        active: Platforms::All,
    }
}

fn p(input: Input, command: bool, shift: bool, space: bool) -> Pointing {
    Pointing {
        input,
        command,
        shift,
        space,
    }
}

/// Las diez filas de la spec, escritas a mano.
fn scheme() -> Vec<GestureRow> {
    use GestureAction::*;
    let n = mods(None, None, None);
    vec![
        row_2(
            Input::Press(Button::Left),
            mods(None, None, Some(false)),
            Draw(Role::Foreground),
            Scope::Canvas,
        ),
        row_2(
            Input::Press(Button::Left),
            mods(None, None, Some(true)),
            Pan,
            Scope::Canvas,
        ),
        row_2(Input::Press(Button::Middle), n, Pan, Scope::Canvas),
        row_2(
            Input::Press(Button::Right),
            n,
            Draw(Role::Background),
            Scope::Canvas,
        ),
        row_2(
            Input::Wheel,
            mods(Some(false), Some(false), None),
            ZoomStep,
            Scope::Canvas,
        ),
        row_2(
            Input::Wheel,
            mods(Some(false), Some(true), None),
            PanX,
            Scope::Canvas,
        ),
        row_2(
            Input::Scroll,
            mods(Some(false), None, None),
            Pan,
            Scope::Canvas,
        ),
        row_2(Input::Pinch, n, ZoomStep, Scope::Canvas),
        row_2(
            Input::Press(Button::Back),
            n,
            App(AppAction::Undo),
            Scope::Window,
        ),
        row_2(
            Input::Press(Button::Forward),
            n,
            App(AppAction::Redo),
            Scope::Window,
        ),
    ]
}

// --- Fila 1 ---

#[test]
fn row_1_the_default_table_is_the_ten_rows_without_clashes() {
    let tabla = GestureMap::default_scheme();
    assert_eq!(tabla.rows(), scheme().as_slice());
    assert!(tabla.conflicts().is_empty());
    for platform_2 in PLATFORMS {
        let active = tabla
            .rows()
            .iter()
            .filter(|f| f.active.includes(platform_2))
            .count();
        assert_eq!(active, 10, "{platform_2:?}");
    }
}

// --- Fila 2 ---

#[test]
fn row_2_resolving_the_five_buttons_with_all_the_modifiers() {
    use GestureAction::*;
    let tabla = GestureMap::default_scheme();
    for platform_2 in PLATFORMS {
        for command in [false, true] {
            for shift in [false, true] {
                for space in [false, true] {
                    for over in [false, true] {
                        let r = |b: Button| {
                            tabla.resolve(
                                &p(Input::Press(b), command, shift, space),
                                over,
                                platform_2,
                            )
                        };
                        let expected_2 = |a: GestureAction| over.then_some(a);
                        let left_2 = if space { Pan } else { Draw(Role::Foreground) };
                        assert_eq!(r(Button::Left), expected_2(left_2));
                        assert_eq!(r(Button::Middle), expected_2(Pan));
                        assert_eq!(r(Button::Right), expected_2(Draw(Role::Background)));
                        // Los laterales valen en toda la ventana.
                        assert_eq!(r(Button::Back), Some(App(AppAction::Undo)));
                        assert_eq!(r(Button::Forward), Some(App(AppAction::Redo)));
                    }
                }
            }
        }
    }
}

#[test]
fn row_2_wheel_scroll_and_pinch() {
    use GestureAction::*;
    let tabla = GestureMap::default_scheme();
    for shift in [false, true] {
        for space in [false, true] {
            let wheel = |command| {
                tabla.resolve(
                    &p(Input::Wheel, command, shift, space),
                    true,
                    Platform::Linux,
                )
            };
            assert_eq!(wheel(false), Some(if shift { PanX } else { ZoomStep }));
            assert_eq!(wheel(true), None, "con command es del pellizco");
            let fingers = |command| {
                tabla.resolve(
                    &p(Input::Scroll, command, shift, space),
                    true,
                    Platform::Mac,
                )
            };
            assert_eq!(fingers(false), Some(Pan));
            assert_eq!(fingers(true), None);
            for command in [false, true] {
                let pinch = tabla.resolve(
                    &p(Input::Pinch, command, shift, space),
                    true,
                    Platform::Windows,
                );
                assert_eq!(pinch, Some(ZoomStep));
            }
        }
    }
    // Fuera del lienzo no valen las filas del lienzo.
    assert_eq!(
        tabla.resolve(
            &p(Input::Wheel, false, false, false),
            false,
            Platform::Linux
        ),
        None
    );
    assert_eq!(
        tabla.resolve(
            &p(Input::Pinch, false, false, false),
            false,
            Platform::Linux
        ),
        None
    );
}

// --- Fila 3 ---

fn wheel(unit: MouseWheelUnit, delta: Vec2, modifiers: Modifiers) -> Event {
    Event::MouseWheel {
        unit,
        delta,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers,
    }
}

fn button_2(button: PointerButton, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton {
        pos: Pos2::ZERO,
        button,
        pressed,
        modifiers,
    }
}

#[test]
fn row_3_the_five_egui_buttons_are_the_five_of_the_table() {
    assert_eq!(button_of(PointerButton::Primary), Button::Left);
    assert_eq!(button_of(PointerButton::Secondary), Button::Right);
    assert_eq!(button_of(PointerButton::Middle), Button::Middle);
    assert_eq!(button_of(PointerButton::Extra1), Button::Back);
    assert_eq!(button_of(PointerButton::Extra2), Button::Forward);
}

#[test]
fn row_3_the_unit_separates_wheel_and_fingers_and_command_belongs_to_the_pinch() {
    let d = Vec2::new(0.25, -1.5);
    let without = Modifiers::NONE;
    assert_eq!(
        wheel_of(MouseWheelUnit::Line, d, without),
        Some((Input::Wheel, 0.25, -1.5))
    );
    assert_eq!(
        wheel_of(MouseWheelUnit::Page, d, without),
        Some((Input::Wheel, 0.25, -1.5))
    );
    assert_eq!(
        wheel_of(MouseWheelUnit::Point, d, without),
        Some((Input::Scroll, 0.25, -1.5))
    );
    for unit in [
        MouseWheelUnit::Line,
        MouseWheelUnit::Page,
        MouseWheelUnit::Point,
    ] {
        assert_eq!(
            wheel_of(unit, d, Modifiers::COMMAND),
            None,
            "{unit:?} con command"
        );
        assert!(
            wheel_of(unit, d, Modifiers::SHIFT).is_some(),
            "{unit:?} con shift"
        );
    }
}

#[test]
fn row_3_pointer_presses_gives_one_per_press_and_never_per_release() {
    let events_2 = vec![
        button_2(PointerButton::Extra1, true, Modifiers::NONE),
        button_2(PointerButton::Extra1, false, Modifiers::NONE),
        wheel(MouseWheelUnit::Line, Vec2::new(0.0, 1.0), Modifiers::NONE),
        button_2(PointerButton::Extra2, true, Modifiers::SHIFT),
        button_2(PointerButton::Primary, true, Modifiers::COMMAND),
        Event::Zoom(1.1),
    ];
    assert_eq!(
        pointer_presses(&events_2),
        vec![
            (Button::Back, false, false),
            (Button::Forward, false, true),
            (Button::Left, true, false),
        ]
    );
}

// --- Fila 4 ---

fn sum(accumulator: &mut StepAccumulator, amounts: &[f32]) -> Vec<i32> {
    amounts.iter().map(|&c| accumulator.push(c)).collect()
}

#[test]
fn row_4_wheel_accumulator() {
    let mut a = StepAccumulator::new(WHEEL_STEP);
    assert_eq!(a.push(1.0), 1);
    assert_eq!(a.push(3.0), 3, "tres muescas de golpe, tres pasos");
    assert_eq!(a.push(-1.0), -1);
    a.reset();
    assert_eq!(
        sum(&mut a, &[0.4, 0.4, 0.4]),
        [0, 0, 1],
        "el resto se arrastra"
    );
    a.reset();
    assert_eq!(
        sum(&mut a, &[0.6, -0.6]),
        [0, 0],
        "el cambio de signo descarta el resto"
    );
    a.reset();
    assert_eq!(a.push(0.6), 0);
    a.reset();
    assert_eq!(a.push(0.6), 0, "tras reset no queda resto");
    assert_eq!(a.remainder(), 0.6);
}

#[test]
fn row_4_pinch_accumulator() {
    let total = |amounts: &[f32]| {
        let mut a = StepAccumulator::new(PINCH_STEP_LN);
        sum(&mut a, amounts).iter().sum::<i32>()
    };
    assert_eq!(
        total(&[1.02f32.ln(); 30]),
        3,
        "un pellizco lento de 0,5 s: 3 pasos (antes 30)"
    );
    assert_eq!(total(&[1.3f32.ln()]), 1);
    assert_eq!(total(&[1.2f32.ln()]), 1);
    assert_eq!(total(&[0.7f32.ln()]), -2);
    assert_eq!(
        total(&[0.025; 8]),
        1,
        "una muesca con Ctrl (ln total 0,2): un paso (antes 11)"
    );
}

#[test]
fn row_4_the_non_finite_ones_are_ignored() {
    let mut a = StepAccumulator::new(WHEEL_STEP);
    assert_eq!(a.push(0.5), 0);
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(a.push(bad), 0);
    }
    assert_eq!(a.remainder(), 0.5);
    assert_eq!(a.push(0.5), 1);
}

// --- Fila 9 ---

#[test]
fn row_9_reassigning_to_any_action_of_the_keyboard_table() {
    let tabla = GestureMap::new(vec![row_2(
        Input::Press(Button::Back),
        mods(None, None, None),
        GestureAction::App(AppAction::ColorNext),
        Scope::Window,
    )]);
    assert_eq!(
        tabla.resolve(
            &p(Input::Press(Button::Back), false, false, false),
            false,
            Platform::Linux
        ),
        Some(GestureAction::App(AppAction::ColorNext))
    );
    assert!(tabla.conflicts().is_empty());
    assert_eq!(tabla.rows().len(), 1);
}

#[test]
fn row_9_duplicates_only_if_the_modifiers_are_compatible() {
    let left = |space| {
        row_2(
            Input::Press(Button::Left),
            mods(None, None, space),
            GestureAction::Pan,
            Scope::Canvas,
        )
    };
    let clashes = |a, b| GestureMap::new(vec![left(a), left(b)]).conflicts();
    assert_eq!(clashes(None, Some(false)), vec![Conflict::Duplicate(0, 1)]);
    assert_eq!(clashes(Some(true), None), vec![Conflict::Duplicate(0, 1)]);
    assert_eq!(
        clashes(Some(true), Some(true)),
        vec![Conflict::Duplicate(0, 1)]
    );
    assert!(clashes(Some(true), Some(false)).is_empty(), "se excluyen");
    // Plataformas que no se solapan tampoco chocan.
    let mut linux = left(None);
    linux.active = Platforms::Only(Platform::Linux);
    let mut mac = left(None);
    mac.active = Platforms::Only(Platform::Mac);
    assert!(GestureMap::new(vec![linux, mac]).conflicts().is_empty());
}

#[test]
fn row_9_invalid_combinations() {
    let n = mods(None, None, None);
    let invalid = [
        row_2(
            Input::Wheel,
            n,
            GestureAction::Draw(Role::Foreground),
            Scope::Canvas,
        ),
        row_2(
            Input::Press(Button::Left),
            n,
            GestureAction::ZoomStep,
            Scope::Canvas,
        ),
        row_2(
            Input::Press(Button::Left),
            n,
            GestureAction::PanX,
            Scope::Canvas,
        ),
        row_2(Input::Pinch, n, GestureAction::PanX, Scope::Canvas),
        row_2(Input::Pinch, n, GestureAction::Pan, Scope::Canvas),
        row_2(
            Input::Scroll,
            n,
            GestureAction::App(AppAction::Undo),
            Scope::Window,
        ),
        row_2(
            Input::Wheel,
            n,
            GestureAction::App(AppAction::Undo),
            Scope::Window,
        ),
    ];
    for f in invalid {
        assert_eq!(
            GestureMap::new(vec![f.clone()]).conflicts(),
            vec![Conflict::Invalid(0)],
            "{f:?}"
        );
    }
    let valid = [
        row_2(Input::Wheel, n, GestureAction::Pan, Scope::Canvas),
        row_2(Input::Scroll, n, GestureAction::ZoomStep, Scope::Canvas),
        row_2(
            Input::Press(Button::Back),
            n,
            GestureAction::App(AppAction::ZoomIn),
            Scope::Window,
        ),
        row_2(
            Input::Press(Button::Right),
            n,
            GestureAction::Pan,
            Scope::Canvas,
        ),
    ];
    for f in valid {
        assert!(
            GestureMap::new(vec![f.clone()]).conflicts().is_empty(),
            "{f:?}"
        );
    }
}

// --- Propiedades ---

fn any_input() -> impl Strategy<Value = Input> {
    prop_oneof![
        prop::sample::select(BUTTONS.to_vec()).prop_map(Input::Press),
        Just(Input::Wheel),
        Just(Input::Scroll),
        Just(Input::Pinch),
    ]
}

/// Lo que dice la spec, escrito a mano sin mirar las filas.
fn oracle(p: &Pointing, over: bool) -> Option<GestureAction> {
    use GestureAction::*;
    match p.input {
        Input::Press(Button::Left) if over => {
            Some(if p.space { Pan } else { Draw(Role::Foreground) })
        }
        Input::Press(Button::Middle) if over => Some(Pan),
        Input::Press(Button::Right) if over => Some(Draw(Role::Background)),
        Input::Press(Button::Back) => Some(App(AppAction::Undo)),
        Input::Press(Button::Forward) => Some(App(AppAction::Redo)),
        Input::Wheel if over && !p.command => Some(if p.shift { PanX } else { ZoomStep }),
        Input::Scroll if over && !p.command => Some(Pan),
        Input::Pinch if over => Some(ZoomStep),
        _ => None,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 1: sin choques, independiente del orden, igual al oráculo y sin filas inalcanzables.
    #[test]
    fn property_1_without_clashes_deterministic_and_reachable(
        input in any_input(),
        command: bool, shift: bool, space: bool, over: bool,
        platform_2 in prop::sample::select(PLATFORMS.to_vec()),
    ) {
        let tabla = GestureMap::default_scheme();
        let inverse = GestureMap::new(tabla.rows().iter().rev().cloned().collect());
        prop_assert!(tabla.conflicts().is_empty());
        let point = p(input, command, shift, space);
        let result = tabla.resolve(&point, over, platform_2);
        prop_assert_eq!(result, inverse.resolve(&point, over, platform_2));
        prop_assert_eq!(result, oracle(&point, over));
    }

    #[test]
    fn property_1_each_row_is_reachable(index_2 in 0usize..10) {
        let tabla = GestureMap::default_scheme();
        let f = &tabla.rows()[index_2];
        let point = Pointing {
            input: f.input,
            command: f.mods.command.unwrap_or(false),
            shift: f.mods.shift.unwrap_or(false),
            space: f.mods.space.unwrap_or(false),
        };
        for platform_2 in PLATFORMS {
            prop_assert_eq!(tabla.resolve(&point, true, platform_2), Some(f.action));
        }
    }

    /// Propiedad 2: el acumulador conserva lo que no ha dado en pasos y nunca da un paso sin cruzar
    /// el umbral.
    #[test]
    fn property_2_accumulator(
        threshold in prop::sample::select(vec![WHEEL_STEP, PINCH_STEP_LN]),
        amounts in prop::collection::vec(-3.0f32..3.0, 0..40),
        reset_en in prop::option::of(0usize..40),
    ) {
        let mut a = StepAccumulator::new(threshold);
        let mut rest_2 = 0.0f32;
        for (i, &c) in amounts.iter().enumerate() {
            if reset_en == Some(i) {
                a.reset();
                rest_2 = 0.0;
                prop_assert_eq!(a.remainder(), 0.0);
            }
            let previous = if rest_2 != 0.0 && c != 0.0 && (rest_2 > 0.0) != (c > 0.0) { 0.0 } else { rest_2 };
            let steps_2 = a.push(c);
            let total = previous + c;
            prop_assert!(a.remainder().abs() < threshold + 1e-4, "resto {}", a.remainder());
            prop_assert!(steps_2 == 0 || (steps_2 > 0) == (total > 0.0));
            prop_assert!((total - steps_2 as f32 * threshold - a.remainder()).abs() < 1e-3);
            if steps_2 == 0 {
                prop_assert!(total.abs() < threshold + 1e-4, "sin paso con {total} y umbral {threshold}");
            }
            rest_2 = a.remainder();
        }
    }
}
