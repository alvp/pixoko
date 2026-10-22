//! Banco de egui de los campos `Button` y de las ventanas de ayuda y del editor de atajo
//! (specs/interfaz-ayuda.md, unidad I.5, filas 2, 8 y 12 en lo que toca al pintado), sin ventana ni
//! capturas: `Context::run_ui` con `RawInput` sintético. En ROJO hasta la lógica: `FieldKind::Button`,
//! `Reaction::Pressed` y `help` no existen todavía.

use egui::{Context, Event, Key, Modifiers, Pos2, Vec2, vec2};
use px_ui::actions::{AppAction, Platform};
use px_ui::dialog::{Dialog, DialogKind, Reaction};
use px_ui::dialog_keys::FocusTarget;
use px_ui::dialog_view::show_dialog;
use px_ui::help::{HELP_PRIMARY, HELP_SECONDARY, HELP_ZONE, HelpTarget, help_form, shortcuts_form};
use px_ui::key_events::Capture;
use px_ui::keymap::{KeyId, Keymap, Press};
use px_ui::shortcut_editor::{
    EDITOR_CONFLICT, EDITOR_PRIMARY, ShortcutEditor, Slot, editor_form, sync_editor_dialog,
};
use px_ui::text::apply_scale;
use px_ui::theme::{TEXT, visuals};
use px_ui::tool::Tool;

mod common;
use common::{PPPS, button_2, context, frame, key_2, move_pointer, n_de, painted};

fn screen(ppp: f32) -> Vec2 {
    let n = n_de(ppp);
    vec2(468.0 * n as f32 / ppp, 341.0 * n as f32 / ppp)
}

struct Bench {
    ctx: Context,
    ppp: f32,
    dialog: Dialog,
    focus: Option<FocusTarget>,
    reactions: Vec<Reaction>,
}

impl Bench {
    fn new_2(ppp: f32, dialog: Dialog) -> Bench {
        let ctx = context(ppp);
        ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
        let mut b = Bench {
            ctx,
            ppp,
            dialog,
            focus: None,
            reactions: Vec::new(),
        };
        b.rest_2_2();
        b
    }

    fn frame(&mut self, events: Vec<Event>) -> common::Painted {
        let screen = screen(self.ppp);
        let mut reaction = Reaction::None;
        let shapes_2 = {
            let Bench {
                ctx, dialog, focus, ..
            } = &mut *self;
            frame(ctx, screen, events, |ui| {
                let c = ui.ctx().clone();
                apply_scale(&c);
                reaction = show_dialog(&c, dialog, focus);
            })
        };
        if !matches!(reaction, Reaction::None) {
            self.reactions.push(reaction);
        }
        painted(&shapes_2)
    }

    fn rest_2_2(&mut self) -> common::Painted {
        self.frame(vec![]);
        self.frame(vec![]);
        self.frame(vec![])
    }

    fn click(&mut self, pos: Pos2) -> common::Painted {
        self.frame(vec![move_pointer(pos)]);
        self.frame(vec![button_2(pos, true)]);
        self.frame(vec![button_2(pos, false)]);
        self.rest_2_2()
    }

    fn presses_2(&mut self, key: Key) -> common::Painted {
        self.frame(vec![key_2(key, Modifiers::NONE)]);
        self.rest_2_2()
    }
}

/// El centro del texto pintado exactamente igual a `text_2`.
fn center(p: &common::Painted, text_2: &str) -> Pos2 {
    p.texts
        .iter()
        .find(|t| t.text_2 == text_2)
        .unwrap_or_else(|| panic!("no se pinta {text_2:?}; hay {:?}", p.all_the_texts()))
        .rect()
        .center()
}

/// El centro del primer texto pintado que empieza por `prefix`.
fn center_of_line(p: &common::Painted, prefix: &str) -> Pos2 {
    p.texts
        .iter()
        .find(|t| t.text_2.starts_with(prefix))
        .unwrap_or_else(|| panic!("no hay línea {prefix:?}; hay {:?}", p.all_the_texts()))
        .rect()
        .center()
}

fn help(target: HelpTarget, table: &Keymap) -> Dialog {
    Dialog::new(
        DialogKind::Help,
        help_form(target, Tool::Pencil, table, Platform::Linux),
    )
    .unwrap()
}

fn press(c: char) -> Press {
    Press {
        key: Some(KeyId::Char(c)),
        command: false,
        shift: false,
        alt: false,
        other_ctrl: false,
        repeat: false,
    }
}

#[test]
fn row_2_the_shortcut_buttons_are_painted_and_a_click_gives_pressed_without_closing() {
    let table = Keymap::default_scheme();
    for ppp in PPPS {
        let mut b = Bench::new_2(ppp, help(HelpTarget::Tool(Tool::Pencil), &table));
        let p = b.rest_2_2();
        // El rótulo de la fila, el atajo y `without shortcut_2` se pintan; la ayuda tiene el nombre.
        for text_2 in [
            "Lápiz",
            "Principal",
            "Secundario",
            "d",
            "sin atajo",
            "Cerrar",
        ] {
            assert!(
                p.paints(text_2),
                "{text_2:?} a ppp {ppp}: {:?}",
                p.all_the_texts()
            );
        }
        let p = b.click(center(&p, "d"));
        assert_eq!(
            b.reactions,
            vec![Reaction::Pressed {
                field: HELP_PRIMARY,
                aside: false
            }],
            "a ppp {ppp}"
        );
        assert!(
            b.reactions
                .iter()
                .all(|r| !matches!(r, Reaction::Accepted(_) | Reaction::Cancelled))
        );
        let _ = p;
        b.reactions.clear();
        let p = b.rest_2_2();
        b.click(center(&p, "sin atajo"));
        assert_eq!(
            b.reactions,
            vec![Reaction::Pressed {
                field: HELP_SECONDARY,
                aside: false
            }]
        );
    }
}

#[test]
fn row_2_the_initial_focus_is_the_area_and_tab_walks_the_area_the_buttons_and_close() {
    let table = Keymap::default_scheme();
    let mut b = Bench::new_2(1.0, help(HelpTarget::Tool(Tool::Pencil), &table));
    assert_eq!(b.focus, Some(FocusTarget::Zone(HELP_ZONE)));
    b.presses_2(Key::Tab);
    assert_eq!(b.focus, Some(FocusTarget::Button(HELP_PRIMARY)));
    b.presses_2(Key::Tab);
    assert_eq!(b.focus, Some(FocusTarget::Button(HELP_SECONDARY)));
    b.presses_2(Key::Tab);
    assert_eq!(b.focus, Some(FocusTarget::Accept));
    b.presses_2(Key::Tab);
    assert_eq!(b.focus, Some(FocusTarget::Zone(HELP_ZONE)), "da la vuelta");
    // `Intro` en la zona no cierra; en un botón lo pulsa (y no cierra).
    b.presses_2(Key::Enter);
    assert!(b.reactions.is_empty());
    b.presses_2(Key::Tab);
    b.presses_2(Key::Enter);
    assert!(matches!(
        b.reactions.as_slice(),
        [Reaction::Pressed {
            field: HELP_PRIMARY,
            aside: false
        }]
    ));
    // `Escape` cierra.
    b.reactions.clear();
    b.presses_2(Key::Escape);
    assert!(matches!(b.reactions.as_slice(), [Reaction::Cancelled]));
}

#[test]
fn row_8_the_editor_paints_its_buttons_and_remove_and_the_dimmed_ones_do_not_respond() {
    let table = Keymap::default_scheme();
    let mut dialog = Dialog::new(DialogKind::ShortcutEditor, editor_form()).unwrap();
    let editor =
        ShortcutEditor::new(AppAction::SelectTool(Tool::Pencil), &table, Platform::Linux).unwrap();
    sync_editor_dialog(&mut dialog, &editor, Platform::Linux);
    let mut b = Bench::new_2(1.0, dialog);
    let p = b.rest_2_2();
    for text_2 in [
        "Principal",
        "Secundario",
        "Quitar",
        "Restablecer",
        "Reasignar",
        "Dejarlo",
        "Aceptar",
        "Cancelar",
    ] {
        assert!(p.paints(text_2), "{text_2:?}: {:?}", p.all_the_texts());
    }
    // `Reasignar` y `Dejarlo` están atenuados sin conflicto: un clic no hace nada.
    b.click(center(&p, "Reasignar"));
    b.click(center(&p, "Dejarlo"));
    assert!(b.reactions.is_empty(), "{:?}", b.reactions);
    // El texto atenuado es el único que puede bajar de 4,5:1.
    let text_2 = p.text_with("Reasignar").unwrap();
    assert_ne!(text_2.color(), TEXT);
    // `Remove` de la principal da `Pressed` con `aside`.
    let first_2: Vec<_> = p.texts.iter().filter(|t| t.text_2 == "Quitar").collect();
    assert_eq!(first_2.len(), 2, "un Quitar por casilla");
    b.click(first_2[0].rect().center());
    assert_eq!(
        b.reactions,
        vec![Reaction::Pressed {
            field: EDITOR_PRIMARY,
            aside: true
        }]
    );
    assert_eq!(EDITOR_CONFLICT, "conflicto");
    // Con un conflicto, `Reasignar` responde.
    let mut editor = editor;
    editor.begin_capture(Slot::Primary);
    editor.capture(Capture::Press(press('v')));
    sync_editor_dialog(&mut b.dialog, &editor, Platform::Linux);
    b.reactions.clear();
    let p = b.rest_2_2();
    b.click(center(&p, "Reasignar"));
    assert!(matches!(
        b.reactions.as_slice(),
        [Reaction::Pressed {
            field: EDITOR_CONFLICT,
            aside: false
        }]
    ));
}

#[test]
fn row_12_the_shortcut_list_is_painted_and_its_buttons_respond() {
    let table = Keymap::default_scheme();
    let spec = shortcuts_form(
        &table,
        Platform::Linux,
        &[AppAction::SelectTool(Tool::Fill)],
    );
    assert!(spec.validate().is_ok());
    let dialog = Dialog::new(DialogKind::ShortcutList, spec).unwrap();
    let mut b = Bench::new_2(1.0, dialog);
    let p = b.rest_2_2();
    // Hay más botones de los que caben: se desplaza el cuerpo.
    assert!(p.paints("Cerrar"));
    let line = p
        .texts
        .iter()
        .find(|t| t.text_2.starts_with("Lápiz"))
        .unwrap_or_else(|| panic!("{:?}", p.all_the_texts()))
        .text_2
        .clone();
    assert!(line.chars().count() <= 44, "{line:?}");
    b.click(center_of_line(&p, "Lápiz"));
    assert!(matches!(
        b.reactions.as_slice(),
        [Reaction::Pressed {
            field: "tool.pencil",
            aside: false
        }]
    ));
}

#[test]
fn row_2_the_22_help_texts_and_the_confirmation_are_painted_without_missing_glyphs() {
    // La política del banco es el pánico ante un glifo ausente: pintar cada ayuda lo comprueba.
    let table = Keymap::default_scheme();
    for input_2 in px_ui::help_texts::help_table() {
        let spec = px_ui::help::help_form_for(input_2.key, &table, Platform::Linux).unwrap();
        let mut b = Bench::new_2(1.0, Dialog::new(DialogKind::Help, spec).unwrap());
        let p = b.rest_2_2();
        assert!(p.paints(input_2.name), "{}", input_2.key);
    }
    let confirmation = Dialog::new(
        DialogKind::ResetShortcuts,
        px_ui::help::confirm_reset_form(),
    )
    .unwrap();
    let mut b = Bench::new_2(1.0, confirmation);
    let p = b.rest_2_2();
    assert!(p.paints("Aceptar") && p.paints("Cancelar"));
}
