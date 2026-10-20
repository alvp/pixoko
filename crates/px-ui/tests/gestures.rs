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

const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];
const BOTONES: [Button; 5] = [
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

fn fila(input: Input, mods: Mods, action: GestureAction, scope: Scope) -> GestureRow {
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
fn esquema() -> Vec<GestureRow> {
    use GestureAction::*;
    let n = mods(None, None, None);
    vec![
        fila(
            Input::Press(Button::Left),
            mods(None, None, Some(false)),
            Draw(Role::Foreground),
            Scope::Canvas,
        ),
        fila(
            Input::Press(Button::Left),
            mods(None, None, Some(true)),
            Pan,
            Scope::Canvas,
        ),
        fila(Input::Press(Button::Middle), n, Pan, Scope::Canvas),
        fila(
            Input::Press(Button::Right),
            n,
            Draw(Role::Background),
            Scope::Canvas,
        ),
        fila(
            Input::Wheel,
            mods(Some(false), Some(false), None),
            ZoomStep,
            Scope::Canvas,
        ),
        fila(
            Input::Wheel,
            mods(Some(false), Some(true), None),
            PanX,
            Scope::Canvas,
        ),
        fila(
            Input::Scroll,
            mods(Some(false), None, None),
            Pan,
            Scope::Canvas,
        ),
        fila(Input::Pinch, n, ZoomStep, Scope::Canvas),
        fila(
            Input::Press(Button::Back),
            n,
            App(AppAction::Undo),
            Scope::Window,
        ),
        fila(
            Input::Press(Button::Forward),
            n,
            App(AppAction::Redo),
            Scope::Window,
        ),
    ]
}

// --- Fila 1 ---

#[test]
fn fila_1_la_tabla_por_defecto_son_las_diez_filas_sin_choques() {
    let tabla = GestureMap::default_scheme();
    assert_eq!(tabla.rows(), esquema().as_slice());
    assert!(tabla.conflicts().is_empty());
    for plataforma in PLATAFORMAS {
        let activas = tabla
            .rows()
            .iter()
            .filter(|f| f.active.includes(plataforma))
            .count();
        assert_eq!(activas, 10, "{plataforma:?}");
    }
}

// --- Fila 2 ---

#[test]
fn fila_2_resolver_los_cinco_botones_con_todos_los_modificadores() {
    use GestureAction::*;
    let tabla = GestureMap::default_scheme();
    for plataforma in PLATAFORMAS {
        for command in [false, true] {
            for shift in [false, true] {
                for space in [false, true] {
                    for over in [false, true] {
                        let r = |b: Button| {
                            tabla.resolve(
                                &p(Input::Press(b), command, shift, space),
                                over,
                                plataforma,
                            )
                        };
                        let esperado = |a: GestureAction| over.then_some(a);
                        let izquierdo = if space { Pan } else { Draw(Role::Foreground) };
                        assert_eq!(r(Button::Left), esperado(izquierdo));
                        assert_eq!(r(Button::Middle), esperado(Pan));
                        assert_eq!(r(Button::Right), esperado(Draw(Role::Background)));
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
fn fila_2_rueda_desplazamiento_y_pellizco() {
    use GestureAction::*;
    let tabla = GestureMap::default_scheme();
    for shift in [false, true] {
        for space in [false, true] {
            let rueda = |command| {
                tabla.resolve(
                    &p(Input::Wheel, command, shift, space),
                    true,
                    Platform::Linux,
                )
            };
            assert_eq!(rueda(false), Some(if shift { PanX } else { ZoomStep }));
            assert_eq!(rueda(true), None, "con command es del pellizco");
            let dedos = |command| {
                tabla.resolve(
                    &p(Input::Scroll, command, shift, space),
                    true,
                    Platform::Mac,
                )
            };
            assert_eq!(dedos(false), Some(Pan));
            assert_eq!(dedos(true), None);
            for command in [false, true] {
                let pellizco = tabla.resolve(
                    &p(Input::Pinch, command, shift, space),
                    true,
                    Platform::Windows,
                );
                assert_eq!(pellizco, Some(ZoomStep));
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

fn rueda(unit: MouseWheelUnit, delta: Vec2, modifiers: Modifiers) -> Event {
    Event::MouseWheel {
        unit,
        delta,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers,
    }
}

fn boton(button: PointerButton, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton {
        pos: Pos2::ZERO,
        button,
        pressed,
        modifiers,
    }
}

#[test]
fn fila_3_los_cinco_botones_de_egui_son_los_cinco_de_la_tabla() {
    assert_eq!(button_of(PointerButton::Primary), Button::Left);
    assert_eq!(button_of(PointerButton::Secondary), Button::Right);
    assert_eq!(button_of(PointerButton::Middle), Button::Middle);
    assert_eq!(button_of(PointerButton::Extra1), Button::Back);
    assert_eq!(button_of(PointerButton::Extra2), Button::Forward);
}

#[test]
fn fila_3_la_unidad_separa_rueda_y_dedos_y_command_es_del_pellizco() {
    let d = Vec2::new(0.25, -1.5);
    let sin = Modifiers::NONE;
    assert_eq!(
        wheel_of(MouseWheelUnit::Line, d, sin),
        Some((Input::Wheel, 0.25, -1.5))
    );
    assert_eq!(
        wheel_of(MouseWheelUnit::Page, d, sin),
        Some((Input::Wheel, 0.25, -1.5))
    );
    assert_eq!(
        wheel_of(MouseWheelUnit::Point, d, sin),
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
fn fila_3_pointer_presses_da_una_por_pulsado_y_nunca_por_soltado() {
    let eventos = vec![
        boton(PointerButton::Extra1, true, Modifiers::NONE),
        boton(PointerButton::Extra1, false, Modifiers::NONE),
        rueda(MouseWheelUnit::Line, Vec2::new(0.0, 1.0), Modifiers::NONE),
        boton(PointerButton::Extra2, true, Modifiers::SHIFT),
        boton(PointerButton::Primary, true, Modifiers::COMMAND),
        Event::Zoom(1.1),
    ];
    assert_eq!(
        pointer_presses(&eventos),
        vec![
            (Button::Back, false, false),
            (Button::Forward, false, true),
            (Button::Left, true, false),
        ]
    );
}

// --- Fila 4 ---

fn suma(acumulador: &mut StepAccumulator, cantidades: &[f32]) -> Vec<i32> {
    cantidades.iter().map(|&c| acumulador.push(c)).collect()
}

#[test]
fn fila_4_acumulador_de_rueda() {
    let mut a = StepAccumulator::new(WHEEL_STEP);
    assert_eq!(a.push(1.0), 1);
    assert_eq!(a.push(3.0), 3, "tres muescas de golpe, tres pasos");
    assert_eq!(a.push(-1.0), -1);
    a.reset();
    assert_eq!(
        suma(&mut a, &[0.4, 0.4, 0.4]),
        [0, 0, 1],
        "el resto se arrastra"
    );
    a.reset();
    assert_eq!(
        suma(&mut a, &[0.6, -0.6]),
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
fn fila_4_acumulador_de_pellizco() {
    let total = |cantidades: &[f32]| {
        let mut a = StepAccumulator::new(PINCH_STEP_LN);
        suma(&mut a, cantidades).iter().sum::<i32>()
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
fn fila_4_los_no_finitos_se_ignoran() {
    let mut a = StepAccumulator::new(WHEEL_STEP);
    assert_eq!(a.push(0.5), 0);
    for malo in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(a.push(malo), 0);
    }
    assert_eq!(a.remainder(), 0.5);
    assert_eq!(a.push(0.5), 1);
}

// --- Fila 9 ---

#[test]
fn fila_9_reasignar_a_cualquier_accion_de_la_tabla_de_teclado() {
    let tabla = GestureMap::new(vec![fila(
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
fn fila_9_duplicados_solo_si_los_modificadores_son_compatibles() {
    let izq = |space| {
        fila(
            Input::Press(Button::Left),
            mods(None, None, space),
            GestureAction::Pan,
            Scope::Canvas,
        )
    };
    let choca = |a, b| GestureMap::new(vec![izq(a), izq(b)]).conflicts();
    assert_eq!(choca(None, Some(false)), vec![Conflict::Duplicate(0, 1)]);
    assert_eq!(choca(Some(true), None), vec![Conflict::Duplicate(0, 1)]);
    assert_eq!(
        choca(Some(true), Some(true)),
        vec![Conflict::Duplicate(0, 1)]
    );
    assert!(choca(Some(true), Some(false)).is_empty(), "se excluyen");
    // Plataformas que no se solapan tampoco chocan.
    let mut linux = izq(None);
    linux.active = Platforms::Only(Platform::Linux);
    let mut mac = izq(None);
    mac.active = Platforms::Only(Platform::Mac);
    assert!(GestureMap::new(vec![linux, mac]).conflicts().is_empty());
}

#[test]
fn fila_9_combinaciones_invalidas() {
    let n = mods(None, None, None);
    let invalidas = [
        fila(
            Input::Wheel,
            n,
            GestureAction::Draw(Role::Foreground),
            Scope::Canvas,
        ),
        fila(
            Input::Press(Button::Left),
            n,
            GestureAction::ZoomStep,
            Scope::Canvas,
        ),
        fila(
            Input::Press(Button::Left),
            n,
            GestureAction::PanX,
            Scope::Canvas,
        ),
        fila(Input::Pinch, n, GestureAction::PanX, Scope::Canvas),
        fila(Input::Pinch, n, GestureAction::Pan, Scope::Canvas),
        fila(
            Input::Scroll,
            n,
            GestureAction::App(AppAction::Undo),
            Scope::Window,
        ),
        fila(
            Input::Wheel,
            n,
            GestureAction::App(AppAction::Undo),
            Scope::Window,
        ),
    ];
    for f in invalidas {
        assert_eq!(
            GestureMap::new(vec![f.clone()]).conflicts(),
            vec![Conflict::Invalid(0)],
            "{f:?}"
        );
    }
    let validas = [
        fila(Input::Wheel, n, GestureAction::Pan, Scope::Canvas),
        fila(Input::Scroll, n, GestureAction::ZoomStep, Scope::Canvas),
        fila(
            Input::Press(Button::Back),
            n,
            GestureAction::App(AppAction::ZoomIn),
            Scope::Window,
        ),
        fila(
            Input::Press(Button::Right),
            n,
            GestureAction::Pan,
            Scope::Canvas,
        ),
    ];
    for f in validas {
        assert!(
            GestureMap::new(vec![f.clone()]).conflicts().is_empty(),
            "{f:?}"
        );
    }
}

// --- Propiedades ---

fn cualquier_entrada() -> impl Strategy<Value = Input> {
    prop_oneof![
        prop::sample::select(BOTONES.to_vec()).prop_map(Input::Press),
        Just(Input::Wheel),
        Just(Input::Scroll),
        Just(Input::Pinch),
    ]
}

/// Lo que dice la spec, escrito a mano sin mirar las filas.
fn oraculo(p: &Pointing, over: bool) -> Option<GestureAction> {
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
    fn propiedad_1_sin_choques_determinista_y_alcanzable(
        input in cualquier_entrada(),
        command: bool, shift: bool, space: bool, over: bool,
        plataforma in prop::sample::select(PLATAFORMAS.to_vec()),
    ) {
        let tabla = GestureMap::default_scheme();
        let inversa = GestureMap::new(tabla.rows().iter().rev().cloned().collect());
        prop_assert!(tabla.conflicts().is_empty());
        let punto = p(input, command, shift, space);
        let resultado = tabla.resolve(&punto, over, plataforma);
        prop_assert_eq!(resultado, inversa.resolve(&punto, over, plataforma));
        prop_assert_eq!(resultado, oraculo(&punto, over));
    }

    #[test]
    fn propiedad_1_cada_fila_es_alcanzable(indice in 0usize..10) {
        let tabla = GestureMap::default_scheme();
        let f = &tabla.rows()[indice];
        let punto = Pointing {
            input: f.input,
            command: f.mods.command.unwrap_or(false),
            shift: f.mods.shift.unwrap_or(false),
            space: f.mods.space.unwrap_or(false),
        };
        for plataforma in PLATAFORMAS {
            prop_assert_eq!(tabla.resolve(&punto, true, plataforma), Some(f.action));
        }
    }

    /// Propiedad 2: el acumulador conserva lo que no ha dado en pasos y nunca da un paso sin cruzar
    /// el umbral.
    #[test]
    fn propiedad_2_acumulador(
        umbral in prop::sample::select(vec![WHEEL_STEP, PINCH_STEP_LN]),
        cantidades in prop::collection::vec(-3.0f32..3.0, 0..40),
        reset_en in prop::option::of(0usize..40),
    ) {
        let mut a = StepAccumulator::new(umbral);
        let mut resto = 0.0f32;
        for (i, &c) in cantidades.iter().enumerate() {
            if reset_en == Some(i) {
                a.reset();
                resto = 0.0;
                prop_assert_eq!(a.remainder(), 0.0);
            }
            let previo = if resto != 0.0 && c != 0.0 && (resto > 0.0) != (c > 0.0) { 0.0 } else { resto };
            let pasos = a.push(c);
            let total = previo + c;
            prop_assert!(a.remainder().abs() < umbral + 1e-4, "resto {}", a.remainder());
            prop_assert!(pasos == 0 || (pasos > 0) == (total > 0.0));
            prop_assert!((total - pasos as f32 * umbral - a.remainder()).abs() < 1e-3);
            if pasos == 0 {
                prop_assert!(total.abs() < umbral + 1e-4, "sin paso con {total} y umbral {umbral}");
            }
            resto = a.remainder();
        }
    }
}
