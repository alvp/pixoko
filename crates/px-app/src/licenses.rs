//! `px-app --licencias`: imprime las licencias de terceros y sale, sin abrir ventana
//! (specs/hito-2-texto.md).

use std::ffi::OsString;

/// Licencias de terceros que se distribuyen con la app (`docs/THIRD-PARTY.md`).
pub const THIRD_PARTY: &str = include_str!("../../../docs/THIRD-PARTY.md");

/// `true` si algún argumento es exactamente `--licencias`.
pub fn wants_licenses(args: &[OsString]) -> bool {
    args.iter().any(|arg| arg == "--licencias")
}

#[cfg(test)]
mod tests {
    //! Fila 17 de specs/hito-2-texto.md: `wants_licenses(args)` es verdadero solo si algún
    //! argumento es exactamente `--licencias` (en cualquier posición), y no entra en pánico con
    //! argumentos que no son UTF-8. `parse_args` no cambia: `--licencias` solo se atiende antes de
    //! `parse_args`, en `main`.

    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::wants_licenses;
    use crate::files::parse_args;

    fn args(valores: &[&str]) -> Vec<OsString> {
        valores.iter().map(OsString::from).collect()
    }

    /// Un `OsString` que no es UTF-8 válido: `fo` seguido de un byte suelto (Unix) o de un
    /// sustituto suelto (Windows).
    #[cfg(unix)]
    fn no_utf8() -> OsString {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(vec![b'f', b'o', 0x80, 0xff])
    }

    #[cfg(windows)]
    fn no_utf8() -> OsString {
        use std::os::windows::ffi::OsStringExt;
        OsString::from_wide(&[0x66, 0x6f, 0xD800])
    }

    #[test]
    fn fila_17_sin_argumentos_no_se_piden_las_licencias() {
        assert!(!wants_licenses(&[]));
    }

    #[test]
    fn fila_17_licencias_solo_o_con_un_fichero_si_se_piden() {
        assert!(wants_licenses(&args(&["--licencias"])));
        assert!(wants_licenses(&args(&["a.png", "--licencias"])));
        assert!(wants_licenses(&args(&["--licencias", "a.png"])));
    }

    #[test]
    fn fila_17_un_fichero_o_una_variante_del_nombre_no_las_piden() {
        assert!(!wants_licenses(&args(&["a.png"])));
        assert!(!wants_licenses(&args(&["--licencia"])));
        assert!(!wants_licenses(&args(&["--LICENCIAS"])));
        assert!(!wants_licenses(&args(&["--licencias="])));
        assert!(!wants_licenses(&args(&["-licencias"])));
        assert!(!wants_licenses(&args(&[" --licencias"])));
        assert!(!wants_licenses(&args(&["--licencias "])));
        assert!(!wants_licenses(&args(&[""])));
    }

    #[test]
    fn fila_17_un_argumento_que_no_es_utf8_no_entra_en_panico_y_no_las_pide() {
        let raro = no_utf8();
        assert!(
            raro.to_str().is_none(),
            "el argumento de prueba no es UTF-8"
        );
        assert!(!wants_licenses(std::slice::from_ref(&raro)));
        // Junto a un argumento válido, `--licencias` se sigue reconociendo.
        assert!(wants_licenses(&[
            raro.clone(),
            OsString::from("--licencias")
        ]));
        assert!(wants_licenses(&[OsString::from("--licencias"), raro]));
    }

    #[test]
    fn fila_17_parse_args_no_cambia() {
        assert_eq!(parse_args(&[]), Ok(None));
        assert_eq!(
            parse_args(&args(&["a.png"])),
            Ok(Some(PathBuf::from("a.png")))
        );
        // Un solo argumento sigue siendo una ruta; con dos, el uso de siempre.
        assert_eq!(
            parse_args(&args(&["--licencias"])),
            Ok(Some(PathBuf::from("--licencias")))
        );
        assert_eq!(
            parse_args(&args(&["a.png", "--licencias"])),
            Err("uso: px-app [archivo]".to_string())
        );
    }
}
