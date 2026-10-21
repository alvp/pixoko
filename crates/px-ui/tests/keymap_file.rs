//! Tests del fichero de atajos (specs/interfaz-ayuda.md, unidad I.5, decisiones 9 y 10 y las
//! condiciones 1 y 4 del visto bueno). En ROJO hasta la lógica: `keymap_file` no existe todavía.
//!
//! - Fila 9: formato y lectura (`to_text`, `parse`, gramática de los atajos).
//! - Fila 10: tolerancia (bytes malos, ficheros ilegibles, choques a mano, aviso).
//! - Condición 1: ante un choque con un atajo por defecto nuevo, gana el del usuario y nunca se le
//!   quita nada.
//! - Propiedad 1a: bytes aleatorios (500 casos), sin pánico y con lo construido acotado.
//! - Propiedad 1b: ida y vuelta con todas las combinaciones asignables y con secuencias del editor.

use proptest::prelude::*;
use px_ui::actions::{AppAction, Platform};
use px_ui::help_texts::{action_of_key, help_table, is_editable};
use px_ui::key_events::Capture;
use px_ui::keymap::{Binding, Chord, KeyId, Keymap, Platforms, Press};
use px_ui::keymap_file::{Fatal, MAX_BYTES, MAX_LINES, Loaded, load_notice, parse, to_text};
use px_ui::shortcut_editor::{CaptureResult, ShortcutEditor, Slot, slots_of};
use px_ui::tool::Tool;

fn defecto() -> Keymap {
    Keymap::default_scheme()
}

fn lee(texto: &str) -> Loaded {
    parse(texto.as_bytes(), &defecto())
}

fn chord(c: char) -> Chord {
    Chord {
        key: KeyId::Char(c),
        command: false,
        shift: false,
    }
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

fn lapiz() -> AppAction {
    AppAction::SelectTool(Tool::Pencil)
}

fn linea() -> AppAction {
    AppAction::SelectTool(Tool::Line)
}

fn relleno() -> AppAction {
    AppAction::SelectTool(Tool::Fill)
}

fn editables() -> Vec<AppAction> {
    help_table()
        .iter()
        .filter_map(|e| action_of_key(e.key))
        .filter(|a| is_editable(*a))
        .collect()
}

/// Asigna una `Press` a una casilla de una acción, resolviendo un conflicto.
fn asigna(tabla: &Keymap, action: AppAction, slot: Slot, p: Press, reasignar: bool) -> Keymap {
    let mut e = ShortcutEditor::new(action, tabla, Platform::Linux).unwrap();
    e.begin_capture(slot);
    if let CaptureResult::Pending(_) = e.capture(Capture::Press(p)) {
        e.resolve(reasignar);
    }
    e.accept()
}

/// Los atajos asignables: letras (con y sin `Mayús`), cifras, símbolos y las teclas con nombre.
fn atajos_asignables() -> Vec<Press> {
    let mut v = Vec::new();
    for c in b' '..=b'~' {
        let c = c as char;
        if !c.is_ascii_graphic() || c.is_ascii_uppercase() {
            continue;
        }
        v.push(press(c));
        if c.is_ascii_lowercase() {
            v.push(Press {
                shift: true,
                ..press(c)
            });
        }
    }
    for key in [KeyId::Home, KeyId::PageUp, KeyId::PageDown] {
        v.push(Press {
            key: Some(key),
            ..press('a')
        });
        if key != KeyId::Home {
            v.push(Press {
                key: Some(key),
                shift: true,
                ..press('a')
            });
        }
    }
    v
}

// ---------------------------------------------------------------------------------------------
// Fila 9: formato y lectura
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_9_la_tabla_por_defecto_da_solo_la_cabecera() {
    let texto = to_text(&defecto(), &defecto());
    let utiles: Vec<&str> = texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert_eq!(utiles, ["pixoko-atajos 1"], "{texto}");
    assert_eq!(texto.lines().next(), Some("pixoko-atajos 1"));
    let cargado = lee(&texto);
    assert_eq!(cargado.keymap, defecto());
    assert_eq!((cargado.ignored_lines, cargado.dropped_rows), (0, 0));
    assert!(cargado.fatal.is_none() && cargado.conflicted.is_empty());
    assert_eq!(load_notice(&cargado), None);
}

#[test]
fn fila_9_solo_se_escribe_lo_que_difiere_compactado_y_en_orden() {
    // Lápiz: `d` pasa a `x`; Siguiente color: se quita la principal; Borrar página: `Mayús+K`.
    let mut t = asigna(&defecto(), lapiz(), Slot::Primary, press('x'), false);
    let mut e = ShortcutEditor::new(AppAction::ColorNext, &t, Platform::Linux).unwrap();
    e.clear(Slot::Primary);
    t = e.accept();
    let texto = to_text(&t, &defecto());
    let lineas: Vec<&str> = texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert_eq!(
        lineas,
        [
            "pixoko-atajos 1",
            "tool.pencil 1 x",
            // Compactada: la secundaria (`Av Pág`) sube y la casilla 2 queda vacía.
            "color.next 1 pagedown",
            "color.next 2 none",
        ],
        "{texto}"
    );
    assert_eq!(lee(&texto).keymap, t, "ida y vuelta exacta");
}

#[test]
fn fila_9_la_gramatica_de_los_atajos() {
    let valido = |atajo: &str| {
        let l = lee(&format!("pixoko-atajos 1\ntool.pencil 1 {atajo}\n"));
        (l.ignored_lines, slots_of(&l.keymap, lapiz()).slots)
    };
    assert_eq!(valido("x"), (0, vec![chord('x')]));
    assert_eq!(valido(">"), (0, vec![chord('>')]));
    assert_eq!(valido("["), (0, vec![chord('[')]));
    assert_eq!(valido("+"), (0, vec![chord('+')]));
    assert_eq!(valido("3"), (0, vec![chord('3')]));
    assert_eq!(
        valido("shift+k"),
        (
            0,
            vec![Chord {
                key: KeyId::Char('k'),
                command: false,
                shift: true
            }]
        )
    );
    assert_eq!(
        valido("pagedown"),
        (
            0,
            vec![Chord {
                key: KeyId::PageDown,
                command: false,
                shift: false
            }]
        )
    );
    assert_eq!(
        valido("shift+pageup").1,
        vec![Chord {
            key: KeyId::PageUp,
            command: false,
            shift: true
        }]
    );
    assert_eq!(valido("home").1.len(), 1);
    assert_eq!(valido("none"), (0, vec![]), "none vacía la casilla");
    // Mal formados o no asignables: la línea se ignora y la tabla queda como estaba.
    for malo in [
        "X",
        "shift+",
        "shift+x ",
        "shift+>",
        "shift+home",
        "ctrl+x",
        "f1",
        "F1",
        "escape",
        "space",
        "xx",
        "é",
        "",
        "none none",
    ] {
        let l = lee(&format!("pixoko-atajos 1\ntool.pencil 1 {malo}\n"));
        if malo == "shift+x " {
            // El espacio final se recorta: es `shift+x`, válida.
            assert_eq!(l.ignored_lines, 0);
            continue;
        }
        assert_eq!(l.ignored_lines, 1, "{malo:?}");
        assert_eq!(l.keymap, defecto(), "{malo:?}");
    }
    // Tabuladores, CRLF, comentarios y líneas vacías valen.
    let l = lee("pixoko-atajos 1\r\n# nota\r\n\r\ntool.pencil\t1\tx\r\n");
    assert_eq!((l.ignored_lines, slots_of(&l.keymap, lapiz()).slots), (0, vec![chord('x')]));
}

// ---------------------------------------------------------------------------------------------
// Fila 10: tolerancia
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_10_los_casos_fatales_dan_la_tabla_por_defecto_y_su_aviso() {
    let ilegible = parse(&[0xff, 0xfe, 0x80], &defecto());
    assert_eq!(ilegible.fatal, Some(Fatal::Unreadable));
    assert_eq!(ilegible.keymap, defecto());
    assert_eq!(
        load_notice(&ilegible).as_deref(),
        Some("atajos: fichero ilegible, se usan los de fábrica")
    );
    let version = lee("pixoko-atajos 2\ntool.pencil 1 x\n");
    assert_eq!(version.fatal, Some(Fatal::Version));
    assert_eq!(version.keymap, defecto());
    assert_eq!(
        load_notice(&version).as_deref(),
        Some("atajos: versión desconocida, se usan los de fábrica")
    );
    // Sin cabecera: se ignora entero.
    assert!(lee("tool.pencil 1 x\n").fatal.is_some());
    assert!(lee("").fatal.is_some());
    // Demasiado grande y demasiadas líneas.
    let grande = vec![b'#'; MAX_BYTES + 1];
    assert_eq!(parse(&grande, &defecto()).fatal, Some(Fatal::TooBig));
    let mut muchas = String::from("pixoko-atajos 1\n");
    for _ in 0..=MAX_LINES {
        muchas.push_str("tool.pencil 1 x\n");
    }
    assert_eq!(lee(&muchas).fatal, Some(Fatal::TooBig));
    for l in [&ilegible, &version] {
        assert!(load_notice(l).unwrap().chars().count() <= 60);
    }
}

#[test]
fn fila_10_las_lineas_malas_se_ignoran_y_las_buenas_se_aplican() {
    let l = lee("pixoko-atajos 1\n\
        tool.pencil 1 x\n\
        file.save 1 g\n\
        no.existe 1 g\n\
        tool.line 3 none\n\
        tool.line 0 q\n\
        tool.line 1 ??\n\
        tool.fill 1 w\n\
        tool.fill 2 e\n\
        tool.fill 2 r\n");
    // Ignoradas: acción fija, clave desconocida, casilla 0, atajo mal formado y la segunda línea de
    // la casilla 2 de Relleno (gana la primera); la casilla 3 de Línea no existe pero es válida
    // (vacía un extra que no hay) y no cuenta como mala.
    assert_eq!(l.ignored_lines, 5, "{l:?}");
    assert_eq!(slots_of(&l.keymap, lapiz()).slots, vec![chord('x')]);
    assert_eq!(slots_of(&l.keymap, relleno()).slots, vec![chord('w'), chord('e')]);
    assert!(l.keymap.conflicts().is_empty());
    assert_eq!(
        load_notice(&l).as_deref(),
        Some("atajos: 5 líneas ignoradas")
    );
}

#[test]
fn fila_10_un_choque_a_mano_gana_la_primera_fila_y_se_cuenta() {
    // Lápiz con `f` (la de Relleno): en la tabla el Lápiz va antes, gana el Lápiz y a Relleno se le
    // quita; las dos son del usuario si Relleno también se escribe.
    let l = lee("pixoko-atajos 1\ntool.pencil 1 f\ntool.fill 1 f\n");
    assert!(l.keymap.conflicts().is_empty());
    assert_eq!(slots_of(&l.keymap, lapiz()).slots, vec![chord('f')]);
    assert_eq!(slots_of(&l.keymap, relleno()).slots, vec![], "pierde la segunda");
    assert_eq!(l.dropped_rows, 1);
    assert!(l.conflicted.contains(&relleno()));
    let aviso = load_notice(&l).unwrap();
    assert_eq!(aviso, "atajos: 1 repetidos");
    assert!(aviso.chars().count() <= 60);
    let los_dos = lee("pixoko-atajos 1\ntool.pencil 1 x\ntool.line 1 y\ntool.fill 1 q\ntool.fill 2 q\n");
    // La misma tecla dos veces en una acción: queda una.
    assert_eq!(slots_of(&los_dos.keymap, relleno()).slots, vec![chord('q')]);
    assert!(los_dos.keymap.conflicts().is_empty());
    assert_eq!(
        load_notice(&lee("pixoko-atajos 1\ntool.fill 9 x\n")).as_deref(),
        Some("atajos: 1 línea ignorada")
    );
}

// ---------------------------------------------------------------------------------------------
// Condición 1: siempre gana el atajo del usuario
// ---------------------------------------------------------------------------------------------

#[test]
fn condicion_1_un_atajo_por_defecto_nuevo_nunca_quita_el_del_usuario() {
    // Esquema «de una versión futura»: el Relleno estrena la `x`, que el usuario ya tiene en el Lápiz.
    let guardado = to_text(
        &asigna(&defecto(), lapiz(), Slot::Primary, press('x'), false),
        &defecto(),
    );
    let mut filas: Vec<Binding> = defecto().rows().to_vec();
    for fila in &mut filas {
        if fila.action == relleno() {
            fila.key = KeyId::Char('x');
        }
    }
    // El Relleno nuevo va ANTES que el Lápiz en la tabla: la «primera fila» sería la suya.
    let (relleno_filas, resto): (Vec<_>, Vec<_>) =
        filas.into_iter().partition(|f| f.action == relleno());
    let nuevo_defecto = Keymap::new(relleno_filas.into_iter().chain(resto).collect());
    let l = parse(guardado.as_bytes(), &nuevo_defecto);
    assert_eq!(
        slots_of(&l.keymap, lapiz()).slots,
        vec![chord('x')],
        "el atajo del usuario se conserva"
    );
    assert_eq!(slots_of(&l.keymap, relleno()).slots, vec![], "la acción nueva queda sin atajo");
    assert!(l.keymap.conflicts().is_empty());
    assert_eq!(l.conflicted, vec![relleno()], "marca `conflicto`");
    assert_eq!(l.dropped_rows, 1);
    assert_eq!(load_notice(&l).as_deref(), Some("atajos: 1 repetidos"));
    // Lo guardado después sigue diciendo lo del usuario.
    let relee = parse(to_text(&l.keymap, &nuevo_defecto).as_bytes(), &nuevo_defecto);
    assert_eq!(slots_of(&relee.keymap, lapiz()).slots, vec![chord('x')]);
    assert!(relee.keymap.conflicts().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Propiedades
// ---------------------------------------------------------------------------------------------

/// La cota de lo que `parse` construye: las filas por defecto más dos por acción editable (más un
/// margen de casillas por acción), nunca más.
fn cota_de_filas() -> usize {
    defecto().rows().len() + 8 * editables().len()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Propiedad 1a: bytes aleatorios (a veces con la cabecera y líneas casi válidas): `parse` no
    /// entra en pánico, lo construido está acotado, no hay conflictos y las nueve filas con
    /// `command` siguen intactas.
    #[test]
    fn propiedad_1a_bytes_aleatorios_no_hacen_panico_ni_reservan_de_mas(
        bytes in proptest::collection::vec(any::<u8>(), 0..4096),
        con_cabecera in any::<bool>(),
        relleno in 0usize..200_000,
    ) {
        let mut entrada = Vec::new();
        if con_cabecera {
            entrada.extend_from_slice(b"pixoko-atajos 1\n");
        }
        entrada.extend_from_slice(&bytes);
        // Y una entrada enorme de una sola línea: se rechaza o se ignora sin recorrerla entera.
        let mut enorme = entrada.clone();
        enorme.extend(std::iter::repeat_n(b'a', relleno));
        for e in [&entrada, &enorme] {
            let l = parse(e, &defecto());
            prop_assert!(l.keymap.rows().len() <= cota_de_filas());
            prop_assert!(l.keymap.conflicts().is_empty());
            let modernas: Vec<_> = l.keymap.rows().iter().filter(|r| r.command).collect();
            prop_assert_eq!(modernas.len(), 9);
            prop_assert!(l.ignored_lines <= MAX_LINES + 1);
            if e.len() > MAX_BYTES {
                prop_assert_eq!(l.fatal, Some(Fatal::TooBig));
                prop_assert_eq!(&l.keymap, &defecto());
            }
            if l.fatal.is_some() {
                prop_assert_eq!(&l.keymap, &defecto());
                prop_assert!(load_notice(&l).is_some());
            }
            if let Some(aviso) = load_notice(&l) {
                prop_assert!(aviso.chars().count() <= 60);
            }
        }
    }

    /// Propiedad 1b (secuencias): lo que se escribe se lee igual, para cualquier tabla alcanzable
    /// desde el editor.
    #[test]
    fn propiedad_1b_ida_y_vuelta_de_cualquier_tabla_alcanzable(
        pasos in proptest::collection::vec(
            (0usize..17, any::<bool>(), 0usize..120, any::<bool>(), any::<bool>()),
            0..16,
        ),
    ) {
        let atajos = atajos_asignables();
        let acciones = editables();
        let mut tabla = defecto();
        for (accion, secundaria, atajo, reasignar, quitar) in pasos {
            let action = acciones[accion % acciones.len()];
            let mut e = ShortcutEditor::new(action, &tabla, Platform::Linux).unwrap();
            let slot = if secundaria { Slot::Secondary } else { Slot::Primary };
            if quitar {
                e.clear(slot);
            } else {
                e.begin_capture(slot);
                if let CaptureResult::Pending(_) =
                    e.capture(Capture::Press(atajos[atajo % atajos.len()]))
                {
                    e.resolve(reasignar);
                }
            }
            tabla = e.accept();
        }
        let texto = to_text(&tabla, &defecto());
        let l = parse(texto.as_bytes(), &defecto());
        prop_assert_eq!((l.ignored_lines, l.dropped_rows), (0, 0), "{}", texto);
        prop_assert!(l.fatal.is_none());
        prop_assert_eq!(&l.keymap, &tabla, "{}", texto);
    }
}

/// Propiedad 1b (todas las combinaciones): cada acción editable con cada atajo asignable en cada
/// una de las dos casillas se escribe y se lee igual (con la reasignación si choca).
#[test]
fn propiedad_1b_ida_y_vuelta_con_todas_las_combinaciones_de_la_tabla() {
    let atajos = atajos_asignables();
    assert!(atajos.len() > 90);
    let mut comprobadas = 0usize;
    for action in editables() {
        for slot in [Slot::Primary, Slot::Secondary] {
            for p in &atajos {
                let tabla = asigna(&defecto(), action, slot, *p, true);
                let texto = to_text(&tabla, &defecto());
                let l = parse(texto.as_bytes(), &defecto());
                assert_eq!(
                    (l.ignored_lines, l.dropped_rows, l.fatal),
                    (0, 0, None),
                    "{action:?} {slot:?} {p:?}\n{texto}"
                );
                assert_eq!(l.keymap, tabla, "{action:?} {slot:?} {p:?}\n{texto}");
                comprobadas += 1;
            }
        }
    }
    assert_eq!(comprobadas, 17 * 2 * atajos.len());
}

/// Los atajos mostrados siguen funcionando tras leer un fichero cualquiera (la tabla leída es una
/// tabla válida de verdad).
#[test]
fn lo_leido_funciona_como_tabla() {
    let l = lee("pixoko-atajos 1\ntool.pencil 1 x\ntool.line 1 y\n");
    for (c, esperado) in [('x', Some(lapiz())), ('y', Some(linea())), ('d', None), ('v', None)] {
        assert_eq!(
            l.keymap.action_for(&press(c), Platform::Linux),
            esperado,
            "{c}"
        );
    }
    let _ = Platforms::All;
}
