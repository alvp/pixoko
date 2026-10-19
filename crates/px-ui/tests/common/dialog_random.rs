//! Generadores de proptest de formularios y eventos al azar (propiedades 2 y 4 de la unidad 18,
//! specs/hito-2-dialogos-modales.md). Los comparten `dialog_state.rs` y `dialog_keys.rs`, que lo
//! incluyen con `#[path = "common/dialog_random.rs"] mod dialog_random;` (no está en
//! `common/mod.rs`).
//!
//! Un formulario al azar tiene de 0 a 8 campos de los cinco tipos, con ids de la tabla fija `IDS`
//! (`f0` a `f7`; `f8`, `f9` y `zz` no existen nunca: son los «campos desconocidos» de los
//! eventos). Un campo solo lleva `shown_when` si hay un `Choice` anterior que no depende de otro
//! (nada de cadenas de campos ocultos: la spec no dice qué significa «visible» entonces) y la
//! opción de la condición existe.
#![allow(dead_code)]

use proptest::prelude::*;
use px_core::{Mode, Rgba};
use px_ui::dialog::{Event, FieldKind, FieldSpec, FormSpec, Rows, ShowWhen, Values};

pub const IDS: [&str; 10] = ["f0", "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9"];
pub const UNKNOWN_ID: &str = "zz";
pub const LABELS: [&str; 8] = [
    "Campo 0", "Campo 1", "Campo 2", "Campo 3", "Campo 4", "Campo 5", "Campo 6", "Campo 7",
];
pub const OPTION_SETS: [&[&str]; 4] = [
    &["Uno"],
    &["A", "B"],
    &["A", "B", "C"],
    &["P", "Q", "R", "S"],
];

/// Textos que se escriben en los eventos: válidos, inválidos, largos y de otros alfabetos.
pub const TEXTS: [&str; 24] = [
    "",
    "0",
    "1",
    "5",
    "7",
    "64",
    "299",
    "300",
    "007",
    "12345678901",
    "123456789012",
    "1234567890123456789",
    "abc",
    "-5",
    " 5",
    "5 ",
    "#abc",
    "#f008",
    "#ff0000",
    "#12",
    "zz",
    "#808080",
    "#80808080",
    "日✓",
];

/// Cómo es un campo, en números que `build` interpreta.
#[derive(Clone, Debug)]
pub struct FieldRecipe {
    pub kind: u8,
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub flag: bool,
    pub lines: usize,
    pub cond: Option<(usize, usize)>,
}

#[derive(Clone, Debug)]
pub struct FormRecipe {
    pub fields: Vec<FieldRecipe>,
    pub cancel: bool,
    pub check: bool,
}

pub fn field_recipe() -> impl Strategy<Value = FieldRecipe> {
    (
        0u8..5,
        0u32..300,
        0u32..300,
        0u32..1000,
        any::<bool>(),
        0usize..40,
        proptest::option::of((any::<usize>(), any::<usize>())),
    )
        .prop_map(|(kind, a, b, c, flag, lines, cond)| FieldRecipe {
            kind,
            a,
            b,
            c,
            flag,
            lines,
            cond,
        })
}

pub fn form_recipe() -> impl Strategy<Value = FormRecipe> {
    (
        prop::collection::vec(field_recipe(), 0..=8),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(fields, cancel, check)| FormRecipe {
            fields,
            cancel,
            check,
        })
}

/// El `check` de los formularios al azar: falla si la suma de los numéricos visibles es impar.
pub fn check_suma(values: &Values) -> Option<String> {
    let suma: u64 = IDS
        .iter()
        .filter_map(|id| values.number(id))
        .map(u64::from)
        .sum();
    (suma % 2 == 1).then(|| "suma impar".to_string())
}

/// El mensaje de `check_suma`.
pub const SUMA_IMPAR: &str = "suma impar";

/// La spec de una receta. Siempre es válida.
pub fn build(recipe: &FormRecipe) -> FormSpec {
    let mut fields: Vec<FieldSpec> = Vec::new();
    for (i, f) in recipe.fields.iter().enumerate() {
        let kind = match f.kind {
            0 => {
                let (min, max) = (f.a.min(f.b), f.a.max(f.b));
                FieldKind::Number {
                    min,
                    max,
                    default: min + f.c % (max - min + 1),
                }
            }
            1 => {
                let options = OPTION_SETS[f.a as usize % OPTION_SETS.len()];
                FieldKind::Choice {
                    options,
                    default: f.b as usize % options.len(),
                }
            }
            2 => FieldKind::Check { default: f.flag },
            3 => {
                let v = f.c as u8;
                match f.a % 3 {
                    0 => FieldKind::Hex {
                        mode: Mode::Rgba,
                        default: Rgba::new(f.a as u8, f.b as u8, v, if f.flag { 255 } else { 128 }),
                    },
                    1 => FieldKind::Hex {
                        mode: Mode::Gray,
                        default: Rgba::opaque(v, v, v),
                    },
                    _ => FieldKind::Hex {
                        mode: Mode::GrayAlpha,
                        default: Rgba::new(v, v, v, f.b as u8),
                    },
                }
            }
            _ => FieldKind::ReadOnly {
                text: (0..f.lines)
                    .map(|n| format!("linea {n}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                rows: Rows::Fixed(1 + f.a as usize % 5),
            },
        };
        let candidates: Vec<usize> = fields
            .iter()
            .enumerate()
            .filter(|(_, g)| matches!(g.kind, FieldKind::Choice { .. }) && g.shown_when.is_none())
            .map(|(j, _)| j)
            .collect();
        let shown_when = match f.cond {
            Some((x, y)) if !candidates.is_empty() => {
                let j = candidates[x % candidates.len()];
                let FieldKind::Choice { options, .. } = &fields[j].kind else {
                    unreachable!()
                };
                Some(ShowWhen {
                    field: IDS[j],
                    choice: y % options.len(),
                })
            }
            _ => None,
        };
        fields.push(FieldSpec {
            id: IDS[i],
            label: LABELS[i],
            kind,
            shown_when,
        });
    }
    FormSpec {
        title: "Azar",
        accept: "Aceptar",
        cancel: recipe.cancel.then_some("Cancelar"),
        fields,
        check: recipe
            .check
            .then_some(check_suma as fn(&Values) -> Option<String>),
    }
}

/// Un evento en números: `kind` 0 a 8 son los de `Event` en el orden de la spec (`Number`,
/// `Step`, `Choice`, `Check`, `Hex`, `HexConfirm`, `HexRevert`, `Accept`, `Cancel`) y 9 es
/// «abandonar el diálogo» (no es un `Event`).
#[derive(Clone, Debug)]
pub struct EventRecipe {
    pub kind: u8,
    pub id: usize,
    pub text: usize,
    pub index: usize,
    pub flag: bool,
}

pub const KIND_ABANDON: u8 = 9;

pub fn event_recipe() -> impl Strategy<Value = EventRecipe> {
    (
        prop_oneof![21 => 0u8..7, 2 => Just(7u8), 1 => Just(8u8), 1 => Just(KIND_ABANDON)],
        0usize..IDS.len() + 1,
        0usize..TEXTS.len(),
        prop_oneof![4 => 0usize..5, 1 => Just(usize::MAX)],
        any::<bool>(),
    )
        .prop_map(|(kind, id, text, index, flag)| EventRecipe {
            kind,
            id,
            text,
            index,
            flag,
        })
}

/// El id al que apunta una receta de evento (el último índice es el desconocido).
pub fn target_id(r: &EventRecipe) -> &'static str {
    IDS.get(r.id).copied().unwrap_or(UNKNOWN_ID)
}

/// El evento de una receta; `None` para «abandonar».
pub fn event_of(r: &EventRecipe) -> Option<Event> {
    let field = target_id(r);
    let text = TEXTS[r.text].to_string();
    Some(match r.kind {
        0 => Event::Number { field, text },
        1 => Event::Step {
            field,
            delta: if r.flag { 1 } else { -1 },
        },
        2 => Event::Choice {
            field,
            index: r.index,
        },
        3 => Event::Check { field, on: r.flag },
        4 => Event::Hex { field, text },
        5 => Event::HexConfirm {
            field,
            blur: r.flag,
        },
        6 => Event::HexRevert { field },
        7 => Event::Accept,
        8 => Event::Cancel,
        _ => return None,
    })
}
