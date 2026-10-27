//! Row 9 of specs/hito-4-imagen.md (unit 4.1): the six image operations in the menu, without a
//! default key, editable, with help, and blocked like any change of the document.

use px_ui::actions::{AppAction, Locks, Origin, Platform, is_blocked};
use px_ui::help_texts::{action_of_key, help_key, help_table, is_editable};
use px_ui::image_ops_es::{CANVAS_ANCHORS, IMAGE_OP_NOTHING};
use px_ui::menu::{MENUS, shortcut_for};
use px_ui::text::INTERFACE_CHARSET;

const SIX: [(&str, AppAction, &str); 6] = [
    (
        "Voltear horizontal",
        AppAction::FlipHorizontal,
        "image.flip-h",
    ),
    ("Voltear vertical", AppAction::FlipVertical, "image.flip-v"),
    (
        "Girar a la derecha",
        AppAction::TurnRight,
        "image.turn-right",
    ),
    (
        "Girar a la izquierda",
        AppAction::TurnLeft,
        "image.turn-left",
    ),
    ("Girar media vuelta", AppAction::TurnHalf, "image.turn-half"),
    (
        "Tamaño del lienzo…",
        AppAction::CanvasSize,
        "image.canvas-size",
    ),
];

#[test]
fn row_9_image_menu_has_the_six_after_convert_and_quantize_in_order() {
    let image = MENUS.iter().find(|m| m.title == "Imagen").expect("menu");
    let items: Vec<_> = image.items.iter().map(|i| (i.label, i.action)).collect();
    assert_eq!(items.len(), 11);
    assert_eq!(items[0].1, AppAction::ConvertMode);
    // `Reducir colores…` (4.2) sits between `Convertir modo…` and the six.
    assert_eq!(items[1].1, AppAction::Quantize);
    for (at, (label, action, _)) in SIX.iter().enumerate() {
        assert_eq!(items[2 + at], (*label, *action));
    }
    assert_eq!(
        items[8..].iter().map(|i| i.1).collect::<Vec<_>>(),
        [
            AppAction::AddPaletteEntry,
            AppAction::RemovePaletteEntry,
            AppAction::PaletteSize
        ]
    );
}

#[test]
fn row_9_the_six_have_no_default_key_are_editable_and_have_help() {
    for (label, action, key) in SIX {
        for platform in [Platform::Linux, Platform::Windows, Platform::Mac] {
            assert_eq!(shortcut_for(action, platform), None, "{action:?}");
        }
        assert_eq!(help_key(action), Some(key));
        assert_eq!(action_of_key(key), Some(action));
        assert!(is_editable(action), "{action:?}");
        let entry = help_table().iter().find(|e| e.key == key).expect("entry");
        assert!(entry.name.chars().count() <= 20, "{key}");
        assert!(entry.description.chars().count() <= 8 * 42, "{key}");
        for text in [label, entry.name, entry.description] {
            assert!(
                text.chars().all(|c| INTERFACE_CHARSET.contains(c)),
                "{text}"
            );
        }
    }
    assert_eq!(help_table().len(), 47);
    for text in CANVAS_ANCHORS.iter().chain([&IMAGE_OP_NOTHING]) {
        assert!(
            text.chars().all(|c| INTERFACE_CHARSET.contains(c)),
            "{text}"
        );
    }
}

#[test]
fn row_9_they_are_blocked_like_a_change_of_the_document() {
    let free = Locks::default();
    for (_, action, _) in SIX {
        assert!(action.changes_image());
        assert!(!is_blocked(action, free, Origin::Menu), "{action:?} free");
        for locks in [
            Locks {
                gesture: true,
                ..free
            },
            Locks {
                text_focus: true,
                ..free
            },
            Locks {
                removal: true,
                ..free
            },
            Locks {
                modal: true,
                ..free
            },
            Locks {
                dialog: true,
                ..free
            },
        ] {
            assert!(
                is_blocked(action, locks, Origin::Menu),
                "{action:?} {locks:?}"
            );
        }
    }
    assert!(AppAction::CanvasSize.opens_dialog());
    assert!(!AppAction::FlipHorizontal.opens_dialog());
    assert!(!AppAction::ConvertMode.changes_image());
}
