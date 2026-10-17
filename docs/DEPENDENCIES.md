# Dependencias

Registro de lo que se incorpora al proyecto. Las dependencias de Cargo deben pasar `cargo deny check` (política en `deny.toml`); cada excepción se anota aquí con su motivo.

## Crates de Cargo

| Crate | Versión | Licencia | Tipo | Uso |
| --- | --- | --- | --- | --- |
| [`proptest`](https://crates.io/crates/proptest) | 1.12.0 | MIT OR Apache-2.0 | dev-dependency de `px-core`, `px-formats` y `px-golden` | Pruebas de propiedades (ida y vuelta, ausencia de pánicos) |
| [`png`](https://crates.io/crates/png) | 0.18.1 | MIT OR Apache-2.0 | dependencia de `px-formats` | Leer y escribir PNG (`specs/hito-1-png.md`). Sin features: no tiene features por defecto y no se activan `zlib-rs`, `unstable` ni `benchmarks` |

`proptest` deja 49 crates transitivos en `Cargo.lock`, de los cuales 32 se compilan con las features por defecto (`rand`, `tempfile`, `rustix`, `windows-sys`…); `serde` y `syn`, por ejemplo, solo figuran en el lockfile. Todos son solo para tests y no entran en los binarios (`cargo tree -p px-core -e normal` está vacío). `cargo deny check` los acepta.

Nota sobre `r-efi 6.0.0` (transitivo, solo UEFI; no se compila en Linux, Windows ni macOS): su licencia es `MIT OR Apache-2.0 OR LGPL-2.1-or-later`. Es una disyunción: se usa bajo MIT o Apache-2.0, nunca bajo LGPL, así que no afecta a la política MIT del proyecto.

### Transitivas de `png`

`cargo tree -p px-formats -e normal` muestra estas crates, todas con una alternativa MIT o Apache-2.0; `cargo deny check` las acepta (`advisories ok, bans ok, licenses ok, sources ok`).

| Crate | Versión | Licencia |
| --- | --- | --- |
| `bitflags` | 2.13.2 | MIT OR Apache-2.0 |
| `crc32fast` | 1.5.2 | MIT OR Apache-2.0 |
| `cfg-if` | 1.0.5 | MIT OR Apache-2.0 |
| `fdeflate` | 0.3.7 | MIT OR Apache-2.0 |
| `flate2` | 1.1.10 | MIT OR Apache-2.0 |
| `miniz_oxide` | 0.8.9 y 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| `adler2` | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| `simd-adler32` | 0.3.10 | MIT |

Notas:

- `adler2` se ofrece como `0BSD OR MIT OR Apache-2.0`. `0BSD` no está en la lista de `deny.toml`, pero es una disyunción: `cargo deny` la acepta por la rama MIT, y es bajo MIT (o Apache-2.0) como se usa.
- `miniz_oxide` aparece **duplicada** (0.8.9 la pide `png` y 0.9.1 `flate2`). `cargo deny` da solo un aviso (`duplicate`), que no hace fallar el check; no hay nada que hacer salvo volver a mirarlo al actualizar `png`.
- `zlib-rs` 0.6.8 (Zlib, que está en la lista) figura solo en `Cargo.lock`, por la feature opcional `zlib-rs` de `flate2`; no se compila.

### Fuzzing (`fuzz/`, fuera del workspace)

`fuzz/Cargo.toml` (crate `pixoko-fuzz`, con su propio `Cargo.lock`) depende de `libfuzzer-sys`, de `px-core` y `px-formats` (rutas) y, por tanto, de `png` y sus transitivas (arriba). Solo se compila en el fuzzer, que no se distribuye: nada de esto entra en los binarios de Pixoko. El `cargo deny` del CI no lo ve; se comprueba en local con `cargo deny --manifest-path fuzz/Cargo.toml --locked check` (`docs/FUZZING.md`), que da `advisories ok, bans ok, licenses ok, sources ok`.

| Crate | Versión | Licencia | Uso |
| --- | --- | --- | --- |
| [`libfuzzer-sys`](https://crates.io/crates/libfuzzer-sys) | 0.4.13 | (MIT OR Apache-2.0) AND NCSA | Enlaza libFuzzer con el objetivo `read_png` (`specs/hito-1-fuzz.md`) |
| `arbitrary` | 1.5.0 | MIT OR Apache-2.0 | Dependencia de `libfuzzer-sys` |
| `cc` | 1.8.0 | MIT OR Apache-2.0 | Solo build (de `libfuzzer-sys`) |
| `find-msvc-tools` | 0.1.14 | MIT OR Apache-2.0 | Solo build (de `cc`) |
| `jobserver` | 0.1.35 | MIT OR Apache-2.0 | Solo build (de `cc`) |
| `libc` | 0.2.191 | MIT OR Apache-2.0 | Solo build (de `jobserver`) |
| `getrandom` | 0.4.3 | MIT OR Apache-2.0 | Solo build (de `jobserver`); su dependencia `r-efi` es solo UEFI y ya está explicada arriba |
| `shlex` | 2.0.1 | MIT OR Apache-2.0 | Solo build (de `cc`) |

**Excepción de licencia: `NCSA` para `libfuzzer-sys` (2026-10-17).** La licencia de la crate es `(MIT OR Apache-2.0) AND NCSA`; con `AND`, NCSA es obligatoria (empaqueta libFuzzer, de LLVM) y no hay rama alternativa. NCSA (University of Illinois/NCSA Open Source License) es permisiva, del estilo de MIT y BSD-3 (conservar el aviso de copyright y el descargo), está aprobada por la OSI y no es GPL, LGPL ni AGPL. Por eso `deny.toml` no la añade a `allow`: declara una excepción `[[licenses.exceptions]]` solo para `libfuzzer-sys`. El `cargo deny check` del workspace no usa esa crate y da un aviso inofensivo (`license-exception-not-encountered`). Si otra crate pidiera NCSA, habría que decidirlo de nuevo.

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
| [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) | 0.13.2 | MIT OR Apache-2.0 | Lanzar el fuzzer (`docs/FUZZING.md`); solo en local, no en el CI |
| Rust nightly (con `rust-src`) | 1.101.0 (2026-10-16) | MIT OR Apache-2.0 | `cargo-fuzz` compila con `-Zbuild-std` y AddressSanitizer; solo en local |
