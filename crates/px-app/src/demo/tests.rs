//! Pruebas de la fila 12 de specs/hito-2-dialogos-modales.md que tocan a `px-app`: el diálogo de
//! prueba (`PIXOKO_TEST_DIALOG=1`), es decir `demo_form() -> FormSpec` y
//! `demo_notice(&Reaction) -> String`. En ROJO hasta la implementación: ninguna de las dos existe.
//!
//! - `demo_form`: `validate()` correcto y los cinco tipos de campo (dos numéricos, opciones —cuatro
//!   en total—, una casilla, un hexadecimal con `shown_when` sobre un grupo de opciones y un informe
//!   con `Rows::Fixed`), con `Cancelar` (el aviso dice «cancelado», así que tiene que poder
//!   cancelarse); abre con `Dialog::new`, es válido por defecto y el hexadecimal solo se muestra con
//!   su opción. Los rangos de la prueba manual (`1-8192` y hasta `4294967295`) van en su propia
//!   prueba, derivada del texto de «Prueba manual», por si la implementación elige otros.
//! - `demo_notice`: función pura, **sin sesión** (la firma lo impide), con textos en
//!   `INTERFACE_CHARSET`, de una sola línea y no vacíos.
//!
//! Lo que la spec no fija y aquí se espera de `demo_notice` (anotado para quien lo implemente):
//! - `Cancelled`: contiene «cancelado» (minúsculas) y no «aceptado».
//! - `Accepted(values)`: contiene «aceptado» y, de cada campo **visible** con valor, su rótulo y su
//!   valor: el número en decimal, el texto de la opción elegida, `sí` o `no` para la casilla y el
//!   color con `hex_text`. Un formato natural es `aceptado: Ancho 64, Alto 64, …`. Los campos ocultos
//!   (`Values` no los trae) no salen: con el hexadecimal oculto no sale el texto de su valor por
//!   defecto.
//! - `Rejected(motivo)`: contiene el motivo.
//! - `None`: un texto no vacío cualquiera (la barra no debe quedar con un texto engañoso: no dice
//!   «aceptado» ni «cancelado»).
//! - Todo texto: sin saltos de línea, sin espacios al principio ni al final y en
//!   `INTERFACE_CHARSET`.
//!
//! Suposiciones sobre las firmas de «Piezas»: `Reaction` y `Values`: `Clone + Debug + PartialEq`.

use super::*;

use px_ui::color_edit::hex_text;
use px_ui::dialog::{
    Dialog, DialogKind, Event, FieldKind, FieldSpec, FormSpec, Reaction, Rows, Values,
};
use px_ui::text::INTERFACE_CHARSET;

/// El `FormSpec` por su firma exacta (que `demo_notice` no recibe sesión lo fija la firma).
const _FORM: fn() -> FormSpec = demo_form;
const _NOTICE: fn(&Reaction) -> String = demo_notice;

fn abre() -> Dialog {
    Dialog::new(DialogKind::Licenses, demo_form()).expect("la spec de prueba es válida")
}

fn campos_de<'a>(
    spec: &'a FormSpec,
    quiere: impl Fn(&FieldKind) -> bool + 'a,
) -> Vec<&'a FieldSpec> {
    spec.fields.iter().filter(|f| quiere(&f.kind)).collect()
}

fn numericos(spec: &FormSpec) -> Vec<&FieldSpec> {
    campos_de(spec, |k| matches!(k, FieldKind::Number { .. }))
}

fn grupos(spec: &FormSpec) -> Vec<&FieldSpec> {
    campos_de(spec, |k| matches!(k, FieldKind::Choice { .. }))
}

fn casillas(spec: &FormSpec) -> Vec<&FieldSpec> {
    campos_de(spec, |k| matches!(k, FieldKind::Check { .. }))
}

fn hexadecimales(spec: &FormSpec) -> Vec<&FieldSpec> {
    campos_de(spec, |k| matches!(k, FieldKind::Hex { .. }))
}

fn informes(spec: &FormSpec) -> Vec<&FieldSpec> {
    campos_de(spec, |k| matches!(k, FieldKind::ReadOnly { .. }))
}

fn opciones(campo: &FieldSpec) -> &'static [&'static str] {
    match &campo.kind {
        FieldKind::Choice { options, .. } => options,
        otro => panic!("{:?} no es un grupo de opciones: {otro:?}", campo.id),
    }
}

/// Lo aceptado con el diálogo en el estado actual: `Accepted(values)` de verdad.
fn aceptado(dialog: &mut Dialog) -> Reaction {
    let reaccion = dialog.event(Event::Accept);
    assert!(
        matches!(reaccion, Reaction::Accepted(_)),
        "el diálogo de prueba es aceptable: {reaccion:?}"
    );
    reaccion
}

fn comprueba_texto_valido(texto: &str, contexto: &str) {
    assert!(!texto.is_empty(), "{contexto}: texto vacío");
    assert_eq!(
        texto.trim(),
        texto,
        "{contexto}: espacios sobrantes: {texto:?}"
    );
    assert!(
        !texto.contains('\n') && !texto.contains('\r'),
        "{contexto}: {texto:?}"
    );
    for c in texto.chars() {
        assert!(
            INTERFACE_CHARSET.contains(c),
            "{contexto}: {c:?} no está en INTERFACE_CHARSET: {texto:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// demo_form
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_demo_form_es_valida() {
    let spec = demo_form();
    assert!(spec.validate().is_ok());
    // Y la acepta un `Dialog` (la única `DialogKind` hoy es `Licenses`).
    assert!(Dialog::new(DialogKind::Licenses, demo_form()).is_ok());
    // Siempre da la misma spec (función pura de nada).
    let otra = demo_form();
    assert_eq!(spec.title, otra.title);
    assert_eq!(spec.accept, otra.accept);
    assert_eq!(spec.cancel, otra.cancel);
    assert_eq!(spec.fields.len(), otra.fields.len());
}

#[test]
fn fila_12_demo_form_contiene_los_cinco_tipos_de_campo() {
    let spec = demo_form();
    assert_eq!(numericos(&spec).len(), 2, "dos numéricos");
    let grupos = grupos(&spec);
    assert!(!grupos.is_empty(), "al menos un grupo de opciones");
    let total_opciones: usize = grupos.iter().map(|g| opciones(g).len()).sum();
    assert_eq!(total_opciones, 4, "cuatro opciones en total");
    assert_eq!(casillas(&spec).len(), 1, "una casilla");
    assert_eq!(hexadecimales(&spec).len(), 1, "un hexadecimal");
    let informes = informes(&spec);
    assert_eq!(informes.len(), 1, "un informe");
    assert!(
        matches!(
            informes[0].kind,
            FieldKind::ReadOnly {
                rows: Rows::Fixed(_),
                ..
            }
        ),
        "el informe es de filas fijas (con `Fill` no puede haber otros campos visibles)"
    );
}

#[test]
fn fila_12_demo_form_tiene_cancelar_y_rotulos_en_el_juego_de_caracteres() {
    let spec = demo_form();
    assert!(spec.cancel.is_some(), "tiene que poder cancelarse");
    comprueba_texto_valido(spec.title, "título");
    comprueba_texto_valido(spec.accept, "aceptar");
    comprueba_texto_valido(spec.cancel.expect("hay Cancelar"), "cancelar");
    for campo in &spec.fields {
        comprueba_texto_valido(campo.label, campo.id);
        if let FieldKind::Choice { options, .. } = &campo.kind {
            for opcion in *options {
                comprueba_texto_valido(opcion, campo.id);
            }
        }
    }
}

#[test]
fn fila_12_demo_form_es_valida_por_defecto_y_el_hexadecimal_solo_se_muestra_con_su_opcion() {
    let spec = demo_form();
    let hex = hexadecimales(&spec)[0];
    let condicion = hex
        .shown_when
        .as_ref()
        .expect("el hexadecimal está condicionado a una opción");
    let grupo = *grupos(&spec)
        .iter()
        .find(|g| g.id == condicion.field)
        .expect("la condición apunta a un grupo de opciones");
    assert!(condicion.choice < opciones(grupo).len());

    let mut dialog = abre();
    assert!(
        dialog.is_valid(),
        "válido por defecto: {:?}",
        dialog.reason()
    );
    assert_eq!(dialog.reason(), None);
    assert!(dialog.values().is_some());

    for k in 0..opciones(grupo).len() {
        dialog.event(Event::Choice {
            field: grupo.id,
            index: k,
        });
        assert_eq!(
            dialog.is_shown(hex.id),
            k == condicion.choice,
            "opción {k}: el hexadecimal solo se muestra con la opción {}",
            condicion.choice
        );
        assert!(dialog.is_valid(), "opción {k}");
        let values = dialog.values().expect("válido");
        assert_eq!(values.choice(grupo.id), Some(k));
        assert_eq!(values.color(hex.id).is_some(), k == condicion.choice);
    }
}

#[test]
fn fila_12_demo_form_ofrece_los_rangos_del_ensayo_manual() {
    // De «Prueba manual», punto 5: un numérico de 1 a 8192 (`0` da «mínimo 1» y `8193`
    // «máximo 8192») y otro que llega a `4294967295`.
    let spec = demo_form();
    let rangos: Vec<(u32, u32)> = numericos(&spec)
        .iter()
        .map(|f| match f.kind {
            FieldKind::Number { min, max, .. } => (min, max),
            _ => unreachable!(),
        })
        .collect();
    assert!(
        rangos.contains(&(1, 8192)),
        "un numérico de 1 a 8192: {rangos:?}"
    );
    assert!(
        rangos.iter().any(|&(_, max)| max == u32::MAX),
        "un numérico que llega a u32::MAX: {rangos:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// demo_notice
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_demo_notice_con_cancelled_dice_cancelado() {
    let texto = demo_notice(&Reaction::Cancelled);
    comprueba_texto_valido(&texto, "Cancelled");
    assert!(texto.contains("cancelado"), "{texto:?}");
    assert!(!texto.contains("aceptado"), "{texto:?}");
}

#[test]
fn fila_12_demo_notice_con_accepted_lista_lo_aceptado() {
    let spec = demo_form();
    let grupo = grupos(&spec)[0];
    let hex = hexadecimales(&spec)[0];
    let casilla = casillas(&spec)[0];

    // Todos los valores por defecto y, a continuación, cada opción con la casilla en los dos
    // estados; los numéricos con valores distintos de los de por defecto.
    for k in 0..opciones(grupo).len() {
        for marcada in [false, true] {
            let mut dialog = abre();
            dialog.event(Event::Choice {
                field: grupo.id,
                index: k,
            });
            dialog.event(Event::Check {
                field: casilla.id,
                on: marcada,
            });
            for (i, n) in numericos(&spec).iter().enumerate() {
                dialog.event(Event::Number {
                    field: n.id,
                    text: format!("{}", 300 + 111 * i),
                });
            }
            let reaccion = aceptado(&mut dialog);
            let Reaction::Accepted(values) = &reaccion else {
                unreachable!()
            };
            let texto = demo_notice(&reaccion);
            comprueba_texto_valido(&texto, "Accepted");
            assert!(texto.contains("aceptado"), "{texto:?}");
            assert!(!texto.contains("cancelado"), "{texto:?}");

            // Cada campo visible con valor: su rótulo y su valor.
            for n in numericos(&spec) {
                assert!(
                    texto.contains(n.label),
                    "falta el rótulo {:?}: {texto:?}",
                    n.label
                );
                let valor = values.number(n.id).expect("el numérico se entrega");
                assert!(
                    texto.contains(&valor.to_string()),
                    "falta el valor {valor} de {:?}: {texto:?}",
                    n.label
                );
            }
            assert!(texto.contains(grupo.label), "{texto:?}");
            assert!(
                texto.contains(opciones(grupo)[k]),
                "falta la opción elegida {:?}: {texto:?}",
                opciones(grupo)[k]
            );
            assert!(texto.contains(casilla.label), "{texto:?}");
            assert!(
                texto.contains(if marcada { "sí" } else { "no" }),
                "la casilla ({marcada}) se dice con sí o no: {texto:?}"
            );
            // El hexadecimal solo sale si se muestra.
            let FieldKind::Hex { default, .. } = &hex.kind else {
                unreachable!()
            };
            match values.color(hex.id) {
                Some(color) => {
                    assert!(texto.contains(hex.label), "{texto:?}");
                    assert!(
                        texto.contains(&hex_text(color)),
                        "falta el color: {texto:?}"
                    );
                }
                None => assert!(
                    !texto.contains(&hex_text(*default)),
                    "un campo oculto no sale: {texto:?}"
                ),
            }
        }
    }
}

#[test]
fn fila_12_demo_notice_con_accepted_y_un_color_distinto_lo_lista() {
    let spec = demo_form();
    let grupo = grupos(&spec)[0];
    let hex = hexadecimales(&spec)[0];
    let condicion = hex.shown_when.as_ref().expect("condicionado");
    let mut dialog = abre();
    dialog.event(Event::Choice {
        field: grupo.id,
        index: condicion.choice,
    });
    // Un color que vale en todos los modos (gris opaco): se escribe y se confirma.
    dialog.event(Event::Hex {
        field: hex.id,
        text: "#777777".into(),
    });
    dialog.event(Event::HexConfirm {
        field: hex.id,
        blur: false,
    });
    let reaccion = aceptado(&mut dialog);
    let Reaction::Accepted(values) = &reaccion else {
        unreachable!()
    };
    let color = values.color(hex.id).expect("el hexadecimal se muestra");
    assert_eq!((color.r, color.g, color.b), (0x77, 0x77, 0x77));
    let texto = demo_notice(&reaccion);
    comprueba_texto_valido(&texto, "Accepted con color");
    assert!(texto.contains(&hex_text(color)), "{texto:?}");
}

#[test]
fn fila_12_demo_notice_con_rejected_dice_el_motivo() {
    for motivo in [
        "Ancho: escribe un número",
        "demasiado grande",
        "a",
        "Color: usa 3, 4, 6 u 8 dígitos",
    ] {
        let texto = demo_notice(&Reaction::Rejected(motivo.to_string()));
        comprueba_texto_valido(&texto, "Rejected");
        assert!(
            texto.contains(motivo),
            "falta el motivo {motivo:?}: {texto:?}"
        );
        assert!(!texto.contains("aceptado"), "{texto:?}");
    }
}

#[test]
fn fila_12_demo_notice_con_none_da_un_texto_razonable() {
    let texto = demo_notice(&Reaction::None);
    comprueba_texto_valido(&texto, "None");
    assert!(!texto.contains("aceptado"), "{texto:?}");
    assert!(!texto.contains("cancelado"), "{texto:?}");
}

#[test]
fn fila_12_demo_notice_es_pura_y_no_consume_la_reaccion() {
    let mut dialog = abre();
    let reaccion = aceptado(&mut dialog);
    let reacciones = [
        Reaction::None,
        Reaction::Cancelled,
        Reaction::Rejected("Ancho: máximo 8192".into()),
        reaccion.clone(),
    ];
    for r in &reacciones {
        let copia = r.clone();
        let a = demo_notice(r);
        let b = demo_notice(r);
        assert_eq!(a, b, "misma entrada, misma salida");
        assert_eq!(r, &copia, "la reacción no cambia");
    }
    // Tampoco toca el diálogo del que salió.
    let valores: Option<Values> = dialog.values();
    demo_notice(&reaccion);
    assert_eq!(dialog.values(), valores);
}
