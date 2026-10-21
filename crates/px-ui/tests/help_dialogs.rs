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
use common::{PPPS, boton, contexto, fotograma, mover, n_de, pintado, tecla};

fn pantalla(ppp: f32) -> Vec2 {
    let n = n_de(ppp);
    vec2(468.0 * n as f32 / ppp, 341.0 * n as f32 / ppp)
}

struct Banco {
    ctx: Context,
    ppp: f32,
    dialog: Dialog,
    focus: Option<FocusTarget>,
    reacciones: Vec<Reaction>,
}

impl Banco {
    fn nuevo(ppp: f32, dialog: Dialog) -> Banco {
        let ctx = contexto(ppp);
        ctx.set_visuals_of(egui::Theme::Dark, visuals(ppp));
        let mut b = Banco {
            ctx,
            ppp,
            dialog,
            focus: None,
            reacciones: Vec::new(),
        };
        b.reposo();
        b
    }

    fn fotograma(&mut self, events: Vec<Event>) -> common::Pintado {
        let pantalla = pantalla(self.ppp);
        let mut reaccion = Reaction::None;
        let formas = {
            let Banco {
                ctx, dialog, focus, ..
            } = &mut *self;
            fotograma(ctx, pantalla, events, |ui| {
                let c = ui.ctx().clone();
                apply_scale(&c);
                reaccion = show_dialog(&c, dialog, focus);
            })
        };
        if !matches!(reaccion, Reaction::None) {
            self.reacciones.push(reaccion);
        }
        pintado(&formas)
    }

    fn reposo(&mut self) -> common::Pintado {
        self.fotograma(vec![]);
        self.fotograma(vec![]);
        self.fotograma(vec![])
    }

    fn clic(&mut self, pos: Pos2) -> common::Pintado {
        self.fotograma(vec![mover(pos)]);
        self.fotograma(vec![boton(pos, true)]);
        self.fotograma(vec![boton(pos, false)]);
        self.reposo()
    }

    fn pulsa(&mut self, key: Key) -> common::Pintado {
        self.fotograma(vec![tecla(key, Modifiers::NONE)]);
        self.reposo()
    }
}

/// El centro del texto pintado exactamente igual a `texto`.
fn centro(p: &common::Pintado, texto: &str) -> Pos2 {
    p.textos
        .iter()
        .find(|t| t.texto == texto)
        .unwrap_or_else(|| panic!("no se pinta {texto:?}; hay {:?}", p.todos_los_textos()))
        .rect()
        .center()
}

/// El centro del primer texto pintado que empieza por `prefijo`.
fn centro_de_linea(p: &common::Pintado, prefijo: &str) -> Pos2 {
    p.textos
        .iter()
        .find(|t| t.texto.starts_with(prefijo))
        .unwrap_or_else(|| panic!("no hay línea {prefijo:?}; hay {:?}", p.todos_los_textos()))
        .rect()
        .center()
}

fn ayuda(target: HelpTarget, tabla: &Keymap) -> Dialog {
    Dialog::new(
        DialogKind::Help,
        help_form(target, Tool::Pencil, tabla, Platform::Linux),
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
fn fila_2_los_botones_de_atajo_se_pintan_y_un_clic_da_pressed_sin_cerrar() {
    let tabla = Keymap::default_scheme();
    for ppp in PPPS {
        let mut b = Banco::nuevo(ppp, ayuda(HelpTarget::Tool(Tool::Pencil), &tabla));
        let p = b.reposo();
        // El rótulo de la fila, el atajo y `sin atajo` se pintan; la ayuda tiene el nombre.
        for texto in [
            "Lápiz",
            "Principal",
            "Secundario",
            "d",
            "sin atajo",
            "Cerrar",
        ] {
            assert!(
                p.pinta(texto),
                "{texto:?} a ppp {ppp}: {:?}",
                p.todos_los_textos()
            );
        }
        let p = b.clic(centro(&p, "d"));
        assert_eq!(
            b.reacciones,
            vec![Reaction::Pressed {
                field: HELP_PRIMARY,
                aside: false
            }],
            "a ppp {ppp}"
        );
        assert!(
            b.reacciones
                .iter()
                .all(|r| !matches!(r, Reaction::Accepted(_) | Reaction::Cancelled))
        );
        let _ = p;
        b.reacciones.clear();
        let p = b.reposo();
        b.clic(centro(&p, "sin atajo"));
        assert_eq!(
            b.reacciones,
            vec![Reaction::Pressed {
                field: HELP_SECONDARY,
                aside: false
            }]
        );
    }
}

#[test]
fn fila_2_el_foco_inicial_es_la_zona_y_tab_recorre_la_zona_los_botones_y_cerrar() {
    let tabla = Keymap::default_scheme();
    let mut b = Banco::nuevo(1.0, ayuda(HelpTarget::Tool(Tool::Pencil), &tabla));
    assert_eq!(b.focus, Some(FocusTarget::Zone(HELP_ZONE)));
    b.pulsa(Key::Tab);
    assert_eq!(b.focus, Some(FocusTarget::Button(HELP_PRIMARY)));
    b.pulsa(Key::Tab);
    assert_eq!(b.focus, Some(FocusTarget::Button(HELP_SECONDARY)));
    b.pulsa(Key::Tab);
    assert_eq!(b.focus, Some(FocusTarget::Accept));
    b.pulsa(Key::Tab);
    assert_eq!(b.focus, Some(FocusTarget::Zone(HELP_ZONE)), "da la vuelta");
    // `Intro` en la zona no cierra; en un botón lo pulsa (y no cierra).
    b.pulsa(Key::Enter);
    assert!(b.reacciones.is_empty());
    b.pulsa(Key::Tab);
    b.pulsa(Key::Enter);
    assert!(matches!(
        b.reacciones.as_slice(),
        [Reaction::Pressed {
            field: HELP_PRIMARY,
            aside: false
        }]
    ));
    // `Escape` cierra.
    b.reacciones.clear();
    b.pulsa(Key::Escape);
    assert!(matches!(b.reacciones.as_slice(), [Reaction::Cancelled]));
}

#[test]
fn fila_8_el_editor_pinta_sus_botones_y_quitar_y_los_atenuados_no_responden() {
    let tabla = Keymap::default_scheme();
    let mut dialog = Dialog::new(DialogKind::ShortcutEditor, editor_form()).unwrap();
    let editor =
        ShortcutEditor::new(AppAction::SelectTool(Tool::Pencil), &tabla, Platform::Linux).unwrap();
    sync_editor_dialog(&mut dialog, &editor, Platform::Linux);
    let mut b = Banco::nuevo(1.0, dialog);
    let p = b.reposo();
    for texto in [
        "Principal",
        "Secundario",
        "Quitar",
        "Restablecer",
        "Reasignar",
        "Dejarlo",
        "Aceptar",
        "Cancelar",
    ] {
        assert!(p.pinta(texto), "{texto:?}: {:?}", p.todos_los_textos());
    }
    // `Reasignar` y `Dejarlo` están atenuados sin conflicto: un clic no hace nada.
    b.clic(centro(&p, "Reasignar"));
    b.clic(centro(&p, "Dejarlo"));
    assert!(b.reacciones.is_empty(), "{:?}", b.reacciones);
    // El texto atenuado es el único que puede bajar de 4,5:1.
    let texto = p.texto_con("Reasignar").unwrap();
    assert_ne!(texto.color(), TEXT);
    // `Quitar` de la principal da `Pressed` con `aside`.
    let primeros: Vec<_> = p.textos.iter().filter(|t| t.texto == "Quitar").collect();
    assert_eq!(primeros.len(), 2, "un Quitar por casilla");
    b.clic(primeros[0].rect().center());
    assert_eq!(
        b.reacciones,
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
    b.reacciones.clear();
    let p = b.reposo();
    b.clic(centro(&p, "Reasignar"));
    assert!(matches!(
        b.reacciones.as_slice(),
        [Reaction::Pressed {
            field: EDITOR_CONFLICT,
            aside: false
        }]
    ));
}

#[test]
fn fila_12_la_lista_de_atajos_se_pinta_y_sus_botones_responden() {
    let tabla = Keymap::default_scheme();
    let spec = shortcuts_form(
        &tabla,
        Platform::Linux,
        &[AppAction::SelectTool(Tool::Fill)],
    );
    assert!(spec.validate().is_ok());
    let dialog = Dialog::new(DialogKind::ShortcutList, spec).unwrap();
    let mut b = Banco::nuevo(1.0, dialog);
    let p = b.reposo();
    // Hay más botones de los que caben: se desplaza el cuerpo.
    assert!(p.pinta("Cerrar"));
    let linea = p
        .textos
        .iter()
        .find(|t| t.texto.starts_with("Lápiz"))
        .unwrap_or_else(|| panic!("{:?}", p.todos_los_textos()))
        .texto
        .clone();
    assert!(linea.chars().count() <= 44, "{linea:?}");
    b.clic(centro_de_linea(&p, "Lápiz"));
    assert!(matches!(
        b.reacciones.as_slice(),
        [Reaction::Pressed {
            field: "tool.pencil",
            aside: false
        }]
    ));
}
