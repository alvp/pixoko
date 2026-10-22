//! La caja de herramientas en los bancos que solo necesitan pulsar sus botones (specs/interfaz-iconos.md,
//! decisión 13): sustituye a los envoltorios `show_toolbar` y `show_toolbar_with`, ya retirados. La
//! caja se pinta sin iconos (un `IconSet` vacío): lo que se prueba aquí es dónde golpea cada botón.
//!
//! También da el centro de cada uno de los nueve botones en `u` desde la esquina de la caja, escrito
//! con literales de la spec (casilla `i` en `(4 + 25·(i mod 4), 4 + 25·(i div 4))` de 24 × 24 u; el
//! zoom, una fila en y = 80), independiente de `chrome`.

#![allow(dead_code)]

use egui::Ui;
use px_ui::actions::{AppAction, Platform, UiState};
use px_ui::icons::IconSet;
use px_ui::keymap::{Keymap, default_keymap};
use px_ui::options::OptionsState;
use px_ui::toolbar::{ToolboxView, show_toolbox};

/// Pinta la caja con la tabla de teclas dada y devuelve la acción del botón pulsado.
pub fn caja_con(
    ui: &mut Ui,
    state: &UiState,
    zoom_label: &str,
    keymap: &Keymap,
    platform: Platform,
) -> Option<AppAction> {
    let view = ToolboxView {
        state,
        zoom_label,
        options: &OptionsState::default(),
        icons: &IconSet::default(),
    };
    show_toolbox(ui, &view, keymap, platform).action
}

/// Como [`caja_con`] con la tabla por defecto y la plataforma actual.
pub fn caja(ui: &mut Ui, state: &UiState, zoom_label: &str) -> Option<AppAction> {
    caja_con(ui, state, zoom_label, default_keymap(), Platform::current())
}

/// Centro `(x_u, y_u)` del botón `i` de la caja: 0 a 4 las herramientas, 5 a 8 el zoom.
pub fn centro_u(i: usize) -> (i64, i64) {
    let (columna, y) = if i < 5 {
        ((i % 4) as i64, 4 + 25 * (i / 4) as i64)
    } else {
        ((i - 5) as i64, 80)
    };
    (4 + 25 * columna + 12, y + 12)
}
