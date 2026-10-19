//! Formularios de ayuda de las pruebas de la unidad 18 (specs/hito-2-dialogos-modales.md).
//!
//! Lo comparten `dialog_state.rs`, `dialog_keys.rs` y los demás ficheros de la unidad. **No está
//! registrado en `common/mod.rs`**: cada fichero lo incluye con
//! `#[path = "common/dialog_forms.rs"] mod dialog_forms;`.
//!
//! `all_kinds` es el formulario de la fila 3 (dos numéricos, un grupo de 3 opciones, un
//! hexadecimal que solo se muestra con la opción 2, una casilla, un informe y un `check`);
//! `huge` sirve para contenido enorme; `open` abre un `Dialog` con una spec válida.
#![allow(dead_code)]

use std::cell::Cell;

use px_core::{Mode, Rgba};
use px_ui::dialog::{Dialog, DialogKind, FieldKind, FieldSpec, FormSpec, Rows, ShowWhen, Values};

thread_local! {
    static CHECK_CALLS: Cell<usize> = const { Cell::new(0) };
}

/// Veces que se ha llamado a `check_grande` en este hilo desde el último `reset_check_calls`.
pub fn check_calls() -> usize {
    CHECK_CALLS.with(Cell::get)
}

pub fn reset_check_calls() {
    CHECK_CALLS.with(|c| c.set(0));
}

/// La comprobación cruzada de `all_kinds`: `ancho × alto` no pasa de un millón de píxeles.
fn check_grande(values: &Values) -> Option<String> {
    CHECK_CALLS.with(|c| c.set(c.get() + 1));
    let ancho = u64::from(values.number("ancho")?);
    let alto = u64::from(values.number("alto")?);
    if ancho * alto > 1_000_000 {
        Some("demasiado grande".into())
    } else {
        None
    }
}

/// Dos numéricos (1 a 8192, por defecto 64), un grupo de tres opciones, un hexadecimal que solo se
/// muestra con la opción 2 (`#336699`), una casilla y un informe de 30 líneas con 4 filas.
pub fn all_kinds() -> FormSpec {
    let informe = (1..=30)
        .map(|n| format!("linea {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    FormSpec {
        title: "Prueba",
        accept: "Aceptar",
        cancel: Some("Cancelar"),
        fields: vec![
            FieldSpec {
                id: "ancho",
                label: "Ancho",
                kind: FieldKind::Number {
                    min: 1,
                    max: 8192,
                    default: 64,
                },
                shown_when: None,
            },
            FieldSpec {
                id: "alto",
                label: "Alto",
                kind: FieldKind::Number {
                    min: 1,
                    max: 8192,
                    default: 64,
                },
                shown_when: None,
            },
            FieldSpec {
                id: "modo",
                label: "Modo",
                kind: FieldKind::Choice {
                    options: &["Uno", "Dos", "Tres"],
                    default: 0,
                },
                shown_when: None,
            },
            FieldSpec {
                id: "color",
                label: "Color",
                kind: FieldKind::Hex {
                    mode: Mode::Rgba,
                    default: Rgba::opaque(0x33, 0x66, 0x99),
                },
                shown_when: Some(ShowWhen {
                    field: "modo",
                    choice: 2,
                }),
            },
            FieldSpec {
                id: "opaco",
                label: "Opaco",
                kind: FieldKind::Check { default: false },
                shown_when: None,
            },
            FieldSpec {
                id: "informe",
                label: "Informe",
                kind: FieldKind::ReadOnly {
                    text: informe,
                    rows: Rows::Fixed(4),
                },
                shown_when: None,
            },
        ],
        check: Some(check_grande),
    }
}

/// `fields` casillas de ids `c0`, `c1`… y rótulo «Casilla», sin `check`: contenido enorme.
pub fn huge(fields: usize) -> FormSpec {
    let fields = (0..fields)
        .map(|i| FieldSpec {
            // Los ids son `&'static str`: se filtran, que en una prueba no importa.
            id: Box::leak(format!("c{i}").into_boxed_str()),
            label: "Casilla",
            kind: FieldKind::Check { default: false },
            shown_when: None,
        })
        .collect();
    FormSpec {
        title: "Grande",
        accept: "Aceptar",
        cancel: Some("Cancelar"),
        fields,
        check: None,
    }
}

/// Abre un `Dialog` con una spec que tiene que ser válida.
pub fn open(spec: FormSpec) -> Dialog {
    Dialog::new(DialogKind::Licenses, spec).expect("spec válida")
}
