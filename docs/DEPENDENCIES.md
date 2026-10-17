# Dependencias

Registro de lo que se incorpora al proyecto. Las dependencias de Cargo deben pasar `cargo deny check` (política en `deny.toml`); cada excepción se anota aquí con su motivo.

## Crates de Cargo

| Crate | Versión | Licencia | Tipo | Uso |
| --- | --- | --- | --- | --- |
| [`proptest`](https://crates.io/crates/proptest) | 1.12.0 | MIT OR Apache-2.0 | dev-dependency de `px-core` | Pruebas de propiedades (ida y vuelta, ausencia de pánicos) |

`proptest` deja 49 crates transitivos en `Cargo.lock`, de los cuales 32 se compilan con las features por defecto (`rand`, `tempfile`, `rustix`, `windows-sys`…); `serde` y `syn`, por ejemplo, solo figuran en el lockfile. Todos son solo para tests y no entran en los binarios (`cargo tree -p px-core -e normal` está vacío). `cargo deny check` los acepta.

Nota sobre `r-efi 6.0.0` (transitivo, solo UEFI; no se compila en Linux, Windows ni macOS): su licencia es `MIT OR Apache-2.0 OR LGPL-2.1-or-later`. Es una disyunción: se usa bajo MIT o Apache-2.0, nunca bajo LGPL, así que no afecta a la política MIT del proyecto.

## Acciones de GitHub

No son dependencias de Cargo ni entran en los binarios: solo se ejecutan en el CI (`.github/workflows/ci.yml`). Van fijadas por SHA de commit completo; para actualizarlas se resuelve el SHA de la nueva versión y se cambia aquí y en el workflow.

| Acción | Versión | SHA | Licencia | Uso |
| --- | --- | --- | --- | --- |
| [`actions/checkout`](https://github.com/actions/checkout) | v7.0.1 | `3d3c42e5aac5ba805825da76410c181273ba90b1` | MIT | Descargar el repositorio |
| [`taiki-e/install-action`](https://github.com/taiki-e/install-action) | v2.87.29 | `8cca0918f8471c33515202b73f56ab27f2bed283` | Apache-2.0 OR MIT | Instalar el binario `cargo-deny@0.20.2` |

## Herramientas del CI

| Herramienta | Versión | Licencia | Uso |
| --- | --- | --- | --- |
| Rust (toolchain) | 1.99.1 | MIT OR Apache-2.0 | Compilar, `rustfmt`, `clippy` |
| [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) | 0.20.2 | MIT OR Apache-2.0 | Licencias, avisos de seguridad y orígenes |
