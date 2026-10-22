//! Los gestos del ratón y del trackpad como tabla de datos (specs/interfaz-raton.md), sin egui.
//!
//! Como `Keymap`: la tabla es la única fuente de qué hace cada botón, la rueda, los dos dedos y el
//! pellizco; el lienzo y la ventana la leen, y el hito 8 la sustituirá entera con
//! [`GestureMap::new`]. Hoy no hay carga ni guardado.
//!
//! Los acumuladores de pasos y el arrastre de restos de píxel (abajo) son lógica pura y también se
//! prueban sin ventana.

use crate::actions::{AppAction, Platform};
use crate::color::Role;
use crate::keymap::Platforms;

// ---------------------------------------------------------------------------------------------
// Constantes de sensibilidad. Viven TODAS AQUÍ, juntas.
//
// AJUSTAR TRAS LA PRUEBA MANUAL (specs/interfaz-raton.md): ninguna de las tres se ha podido medir
// con hardware real (ni pellizco, ni trackpad, ni rueda libre); son cifras de partida.
// ---------------------------------------------------------------------------------------------

/// Muescas de rueda por paso de zoom (una muesca = un paso).
pub const WHEEL_STEP: f32 = 1.0;

/// Logaritmo natural del cambio de escala que cuenta como un paso con el pellizco o con Ctrl +
/// rueda (0,15 ≈ ×1,16). Una muesca con Ctrl vale `ln` 0,2 (un paso); un pellizco lento de medio
/// segundo (`Zoom(1.02)` × 30), tres. Subirlo hace el pellizco más duro; bajarlo, más sensible.
pub const PINCH_STEP_LN: f32 = 0.15;

/// Puntos de egui que desplaza una muesca de rueda (y que equivalen a una muesca de zoom con los
/// dedos): el `line_scroll_speed` de egui nativo.
pub const PAN_LINE_POINTS: f32 = 40.0;

/// Los cinco botones que llegan a la app: `egui-winit` descarta del sexto en adelante.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
    /// El botón lateral «atrás» (el 4).
    Back,
    /// El botón lateral «adelante» (el 5).
    Forward,
}

/// Qué hace el usuario.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// Pulsar un botón (el arrastre dura hasta soltar).
    Press(Button),
    /// Muescas de rueda (`Line` y `Page`).
    Wheel,
    /// Desplazamiento en puntos (dos dedos: `Point`).
    Scroll,
    /// Pellizco, o Ctrl/Cmd + rueda (`zoom_delta`).
    Pinch,
}

/// Modificadores de una fila: `None` = no cuenta, `Some(v)` = debe ser exactamente `v`. `Alt` no
/// existe a propósito (nunca cuenta: KWin se queda `Alt` + arrastrar).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mods {
    pub command: Option<bool>,
    pub shift: Option<bool>,
    /// La tecla `Espacio` (la mano) está abajo.
    pub space: Option<bool>,
}

fn compatible(a: Option<bool>, b: Option<bool>) -> bool {
    a.is_none() || b.is_none() || a == b
}

impl Mods {
    fn matches(&self, p: &Pointing) -> bool {
        self.command.is_none_or(|v| v == p.command)
            && self.shift.is_none_or(|v| v == p.shift)
            && self.space.is_none_or(|v| v == p.space)
    }

    fn compatible(&self, other: &Mods) -> bool {
        compatible(self.command, other.command)
            && compatible(self.shift, other.shift)
            && compatible(self.space, other.space)
    }
}

/// Dónde vale la fila.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Con el cursor sobre el lienzo y ninguna otra capa encima.
    Canvas,
    /// En cualquier sitio de la ventana.
    Window,
}

/// Qué pasa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureAction {
    /// Dibujar con el color del rol, mientras dure el arrastre.
    Draw(Role),
    /// Desplazar la vista: arrastrando (`Press`) o con el delta (`Wheel`, `Scroll`).
    Pan,
    /// Desplazar en horizontal con la suma de los dos ejes del delta.
    PanX,
    /// Zoom por pasos anclado al cursor, con un acumulador.
    ZoomStep,
    /// Cualquier acción de la tabla de teclado (solo con `Press`).
    App(AppAction),
}

/// Una fila de la tabla.
#[derive(Clone, Debug, PartialEq)]
pub struct GestureRow {
    pub input: Input,
    pub mods: Mods,
    pub action: GestureAction,
    pub scope: Scope,
    pub active: Platforms,
}

impl GestureRow {
    /// Si la acción tiene sentido con esa entrada.
    fn valid(&self) -> bool {
        use GestureAction::*;
        matches!(
            (self.input, self.action),
            (Input::Press(_), Draw(_) | Pan | App(_))
                | (Input::Wheel | Input::Scroll, ZoomStep | Pan | PanX)
                | (Input::Pinch, ZoomStep)
        )
    }
}

/// Lo que ocurre, para buscarlo en la tabla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pointing {
    pub input: Input,
    pub command: bool,
    pub shift: bool,
    pub space: bool,
}

/// Un problema de la tabla (el validador del hito 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conflict {
    /// Dos filas que casan con lo mismo en la misma plataforma.
    Duplicate(usize, usize),
    /// Una fila con una acción que no vale para su entrada.
    Invalid(usize),
}

/// La tabla de gestos.
#[derive(Clone, Debug, PartialEq)]
pub struct GestureMap {
    rows: Vec<GestureRow>,
}

impl Default for GestureMap {
    fn default() -> GestureMap {
        GestureMap::default_scheme()
    }
}

impl GestureMap {
    /// Las diez filas del esquema por defecto.
    pub fn default_scheme() -> GestureMap {
        use GestureAction::*;
        let row = |input, command, shift, space, action, scope| GestureRow {
            input,
            mods: Mods {
                command,
                shift,
                space,
            },
            action,
            scope,
            active: Platforms::All,
        };
        let (n, no, yes) = (None, Some(false), Some(true));
        GestureMap {
            rows: vec![
                row(
                    Input::Press(Button::Left),
                    n,
                    n,
                    no,
                    Draw(Role::Foreground),
                    Scope::Canvas,
                ),
                row(Input::Press(Button::Left), n, n, yes, Pan, Scope::Canvas),
                row(Input::Press(Button::Middle), n, n, n, Pan, Scope::Canvas),
                row(
                    Input::Press(Button::Right),
                    n,
                    n,
                    n,
                    Draw(Role::Background),
                    Scope::Canvas,
                ),
                row(Input::Wheel, no, no, n, ZoomStep, Scope::Canvas),
                row(Input::Wheel, no, yes, n, PanX, Scope::Canvas),
                row(Input::Scroll, no, n, n, Pan, Scope::Canvas),
                row(Input::Pinch, n, n, n, ZoomStep, Scope::Canvas),
                row(
                    Input::Press(Button::Back),
                    n,
                    n,
                    n,
                    App(AppAction::Undo),
                    Scope::Window,
                ),
                row(
                    Input::Press(Button::Forward),
                    n,
                    n,
                    n,
                    App(AppAction::Redo),
                    Scope::Window,
                ),
            ],
        }
    }

    /// Sustitución entera de la tabla (hito 8).
    pub fn new(rows: Vec<GestureRow>) -> GestureMap {
        GestureMap { rows }
    }

    pub fn rows(&self) -> &[GestureRow] {
        &self.rows
    }

    /// La acción de la primera fila activa en `platform` cuya entrada, modificadores y ámbito
    /// casan. `over_canvas`: el cursor está sobre el lienzo y ninguna otra capa encima.
    pub fn resolve(
        &self,
        p: &Pointing,
        over_canvas: bool,
        platform: Platform,
    ) -> Option<GestureAction> {
        self.rows
            .iter()
            .find(|row| {
                row.active.includes(platform)
                    && row.input == p.input
                    && row.mods.matches(p)
                    && (row.scope == Scope::Window || over_canvas)
            })
            .map(|row| row.action)
    }

    /// Filas inválidas y pares que chocan: dos filas con la misma entrada, plataformas que se
    /// solapan y modificadores compatibles (un `Canvas` y un `Window` siempre se solapan: sobre el
    /// lienzo valen los dos). Sin choques, el resultado no depende del orden.
    pub fn conflicts(&self) -> Vec<Conflict> {
        let mut found = Vec::new();
        for (i, row) in self.rows.iter().enumerate() {
            if !row.valid() {
                found.push(Conflict::Invalid(i));
            }
            for (j, other) in self.rows.iter().enumerate().skip(i + 1) {
                if row.input == other.input
                    && row.active.overlaps(other.active)
                    && row.mods.compatible(&other.mods)
                {
                    found.push(Conflict::Duplicate(i, j));
                }
            }
        }
        found
    }
}

/// Convierte cantidades continuas (muescas, logaritmo de una escala) en pasos enteros: acumula,
/// da un paso cada vez que el acumulado cruza el umbral y arrastra el resto. Un cambio de signo
/// descarta lo acumulado, y [`StepAccumulator::reset`] (un fotograma sin eventos) también.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepAccumulator {
    acc: f32,
    threshold: f32,
}

impl StepAccumulator {
    pub fn new(threshold: f32) -> StepAccumulator {
        StepAccumulator {
            acc: 0.0,
            threshold,
        }
    }

    /// Suma `amount` y devuelve los pasos (con signo) que ya se han cruzado. Los no finitos se
    /// ignoran.
    pub fn push(&mut self, amount: f32) -> i32 {
        if !amount.is_finite() {
            return 0;
        }
        if amount != 0.0 && self.acc != 0.0 && (amount > 0.0) != (self.acc > 0.0) {
            self.acc = 0.0;
        }
        self.acc += amount;
        let steps = (self.acc / self.threshold).trunc();
        self.acc -= steps * self.threshold;
        steps as i32
    }

    /// Descarta el resto.
    pub fn reset(&mut self) {
        self.acc = 0.0;
    }

    /// Lo acumulado que aún no ha dado un paso.
    pub fn remainder(&self) -> f32 {
        self.acc
    }
}

/// Convierte puntos de egui en píxeles físicos enteros arrastrando el resto de cada eje: con
/// `Point(0, 0,4)` a `ppp` 1 el redondeo a ojo se comería el gesto entero.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PanCarry {
    x: f64,
    y: f64,
}

impl PanCarry {
    pub fn new() -> PanCarry {
        PanCarry::default()
    }

    /// Píxeles físicos enteros a mover por `dx` y `dy` puntos (el resto, con su signo, se guarda).
    pub fn add(&mut self, dx_points: f32, dy_points: f32, ppp: f32) -> (i64, i64) {
        let take = |rest: &mut f64, points: f32| {
            if points.is_finite() {
                *rest += f64::from(points) * f64::from(ppp);
            }
            let whole = rest.trunc();
            *rest -= whole;
            whole as i64
        };
        (take(&mut self.x, dx_points), take(&mut self.y, dy_points))
    }

    pub fn reset(&mut self) {
        *self = PanCarry::default();
    }
}

/// Los acumuladores del lienzo: uno por cada entrada que da pasos y el resto de píxel.
#[derive(Clone, Copy, Debug)]
pub struct Navigation {
    /// Rueda: muescas.
    pub wheel: StepAccumulator,
    /// Dos dedos asignados a zoom: puntos entre [`PAN_LINE_POINTS`].
    pub scroll_zoom: StepAccumulator,
    /// Pellizco y Ctrl + rueda: logaritmo de `zoom_delta`.
    pub pinch: StepAccumulator,
    pub carry: PanCarry,
}

impl Default for Navigation {
    fn default() -> Navigation {
        Navigation {
            wheel: StepAccumulator::new(WHEEL_STEP),
            scroll_zoom: StepAccumulator::new(WHEEL_STEP),
            pinch: StepAccumulator::new(PINCH_STEP_LN),
            carry: PanCarry::new(),
        }
    }
}
