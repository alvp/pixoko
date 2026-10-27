//! La caja de herramientas en los bancos que solo necesitan pulsar sus botones (specs/interfaz-iconos.md,
//! decisión 13): sustituye a los envoltorios `show_toolbar` y `show_toolbar_with`, ya retirados. La
//! caja se pinta sin iconos (un `IconSet` vacío): lo que se prueba aquí es dónde golpea cada botón.
//!
//! También da el centro de cada uno de los diez botones en `u` desde la esquina de la caja con las
//! 16 columnas de hoy, escrito con literales de la spec (specs/ui-single-column.md: las seis
//! herramientas en una fila en y = 4, casilla `i` en `x = 4 + 25·i` de 24 × 24 u; el zoom, una fila
//! en y = 55), independiente de `chrome`.

#![allow(dead_code)]

use egui::Ui;
use px_ui::actions::{AppAction, Platform, UiState};
use px_ui::icons::IconSet;
use px_ui::keymap::{Keymap, default_keymap};
use px_ui::toolbar::{ToolboxView, show_toolbox};

/// Pinta la caja con la tabla de teclas dada y devuelve la acción del botón pulsado.
pub fn toolbox_with(
    ui: &mut Ui,
    state: &UiState,
    zoom_label: &str,
    keymap: &Keymap,
    platform: Platform,
) -> Option<AppAction> {
    let view = ToolboxView {
        state,
        zoom_label,
        icons: &IconSet::default(),
    };
    show_toolbox(ui, &view, keymap, platform).action
}

/// Como [`toolbox_with`] con la tabla por defecto y la plataforma actual.
pub fn toolbox_2(ui: &mut Ui, state: &UiState, zoom_label: &str) -> Option<AppAction> {
    toolbox_with(ui, state, zoom_label, default_keymap(), Platform::current())
}

/// Centro `(x_u, y_u)` del botón `i` de la caja: 0 a 5 las herramientas, 6 a 9 el zoom.
pub fn center_u(i: usize) -> (i64, i64) {
    let (column, y) = if i < 6 {
        (i as i64, 4)
    } else {
        ((i - 6) as i64, 55)
    };
    (4 + 25 * column + 12, y + 12)
}
