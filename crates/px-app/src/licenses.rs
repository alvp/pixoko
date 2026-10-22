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

    fn args(values_2: &[&str]) -> Vec<OsString> {
        values_2.iter().map(OsString::from).collect()
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
    fn row_17_without_arguments_the_licenses_are_not_requested() {
        assert!(!wants_licenses(&[]));
    }

    #[test]
    fn row_17_licenses_alone_or_with_a_file_if_requested() {
        assert!(wants_licenses(&args(&["--licencias"])));
        assert!(wants_licenses(&args(&["a.png", "--licencias"])));
        assert!(wants_licenses(&args(&["--licencias", "a.png"])));
    }

    #[test]
    fn row_17_a_file_or_a_variant_of_the_name_does_not_request_them() {
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
    fn row_17_a_non_utf8_argument_does_not_panic_and_does_not_request_them() {
        let odd = no_utf8();
        assert!(odd.to_str().is_none(), "el argumento de prueba no es UTF-8");
        assert!(!wants_licenses(std::slice::from_ref(&odd)));
        // Junto a un argumento válido, `--licencias` se sigue reconociendo.
        assert!(wants_licenses(&[
            odd.clone(),
            OsString::from("--licencias")
        ]));
        assert!(wants_licenses(&[OsString::from("--licencias"), odd]));
    }

    #[test]
    fn row_17_parse_args_does_not_change() {
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
