//! Row 10 of specs/hito-4-cuantizador.md (unit 4.2), the `px-ui` part: `Reducir colores…` in the menu,
//! without a default key, editable, with help, blocked like any change of the document, and its
//! texts inside the character set of the interface.
//!
//! Red phase: `AppAction::Quantize`, `DialogKind::Quantize` and `quantize_es` do not exist yet.

use px_ui::actions::{AppAction, Locks, Origin, Platform, is_blocked};
use px_ui::dialog::DialogKind;
use px_ui::help_texts::{action_of_key, help_key, help_table, is_editable};
use px_ui::menu::{MENUS, shortcut_for};
use px_ui::quantize_es::{
    QUANTIZE_ACCEPT, QUANTIZE_CANCEL, QUANTIZE_COLORS, QUANTIZE_DITHER, QUANTIZE_DITHERS,
    QUANTIZE_ONLY_COLOR, QUANTIZE_PALETTE, QUANTIZE_PALETTES_ONE, QUANTIZE_PALETTES_TWO,
    QUANTIZE_THRESHOLD, QUANTIZE_TITLE, QUANTIZE_WORKING,
};
use px_ui::text::INTERFACE_CHARSET;

#[test]
fn row_10_the_menu_entry_follows_convert_and_has_no_key_but_help_and_editing() {
    let image = MENUS.iter().find(|m| m.title == "Imagen").expect("menu");
    let items: Vec<_> = image.items.iter().map(|i| (i.label, i.action)).collect();
    assert_eq!(items[0].1, AppAction::ConvertMode);
    assert_eq!(items[1], ("Reducir colores…", AppAction::Quantize));
    for platform in [Platform::Linux, Platform::Windows, Platform::Mac] {
        assert_eq!(shortcut_for(AppAction::Quantize, platform), None);
    }
    assert_eq!(help_key(AppAction::Quantize), Some("image.quantize"));
    assert_eq!(action_of_key("image.quantize"), Some(AppAction::Quantize));
    assert!(is_editable(AppAction::Quantize));
    let entry = help_table()
        .iter()
        .find(|e| e.key == "image.quantize")
        .expect("entry");
    assert!(entry.name.chars().count() <= 20);
    assert!(entry.description.chars().count() <= 8 * 42);
    for text in [entry.name, entry.description] {
        assert!(
            text.chars().all(|c| INTERFACE_CHARSET.contains(c)),
            "{text}"
        );
    }
}

#[test]
fn row_10_quantize_opens_a_dialog_and_is_blocked_like_a_change_of_the_document() {
    let free = Locks::default();
    assert!(AppAction::Quantize.changes_image());
    assert!(AppAction::Quantize.opens_dialog());
    assert!(!is_blocked(AppAction::Quantize, free, Origin::Menu));
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
            is_blocked(AppAction::Quantize, locks, Origin::Menu),
            "{locks:?}"
        );
    }
    assert_ne!(DialogKind::Quantize, DialogKind::ConvertMode);
}

#[test]
fn row_10_the_texts_are_spanish_short_and_inside_the_charset() {
    let singles = [
        QUANTIZE_TITLE,
        QUANTIZE_ACCEPT,
        QUANTIZE_CANCEL,
        QUANTIZE_COLORS,
        QUANTIZE_PALETTE,
        QUANTIZE_DITHER,
        QUANTIZE_THRESHOLD,
        QUANTIZE_ONLY_COLOR,
        QUANTIZE_WORKING,
    ];
    for text in singles
        .iter()
        .chain(QUANTIZE_DITHERS.iter())
        .chain(QUANTIZE_PALETTES_ONE.iter())
        .chain(QUANTIZE_PALETTES_TWO.iter())
    {
        assert!(
            text.chars().all(|c| INTERFACE_CHARSET.contains(c)),
            "{text}"
        );
        assert!(text.chars().count() <= 42, "{text}");
    }
    assert_eq!(QUANTIZE_DITHERS.len(), 3);
    assert_eq!(QUANTIZE_PALETTES_ONE.len(), 1);
    assert_eq!(QUANTIZE_PALETTES_TWO.len(), 2);
    assert_eq!(QUANTIZE_TITLE, "Reducir colores");
    assert_eq!(QUANTIZE_WORKING, "Reduciendo colores…");
}
