# Dependencias

Registro de lo que se incorpora al proyecto. Las dependencias de Cargo deben pasar `cargo deny check` (política en `deny.toml`); cada excepción se anota aquí con su motivo.

## Crates de Cargo

| Crate | Versión | Licencia | Tipo | Uso |
| --- | --- | --- | --- | --- |
| [`proptest`](https://crates.io/crates/proptest) | 1.12.0 | MIT OR Apache-2.0 | dev-dependency de `px-app`, `px-core`, `px-formats`, `px-golden` y `px-raster` | Pruebas de propiedades (ida y vuelta, ausencia de pánicos) |
| [`png`](https://crates.io/crates/png) | 0.18.1 | MIT OR Apache-2.0 | dependencia de `px-formats` (y, a través de ella, de `px-app` desde la unidad 15) | Leer y escribir PNG (`specs/hito-1-png.md`; abrir y guardar en la app, `specs/hito-2-abrir-guardar.md`). Sin features: no tiene features por defecto y no se activan `zlib-rs`, `unstable` ni `benchmarks` |
| [`eframe`](https://crates.io/crates/eframe) | 0.37.0 | MIT OR Apache-2.0 | dependencia de `px-app` | Ventana, bucle de eventos y renderizador OpenGL (`specs/hito-2-ventana.md`). `default-features = false`; features `glow` y, solo en Linux, `x11` y `wayland`. No se activan `wgpu`, `accesskit`, `persistence`, `clipboard`, `links`, `system_fonts` ni `default_fonts` |
| [`egui`](https://crates.io/crates/egui) | 0.37.0 | MIT OR Apache-2.0 | dependencia de `px-ui` | Lienzo de `px-ui` y pruebas del lienzo sin ventana. `default-features = false`; la misma versión que usa `eframe`, fijada con `=` en las dos |
| [`rfd`](https://crates.io/crates/rfd) | 0.17.2 | MIT | dependencia de `px-app` | Diálogos nativos de abrir y guardar, no bloqueantes (`specs/hito-2-dialogos-archivo.md`). `default-features = false`; en Linux, features `xdg-portal` y `wayland`; en Windows y macOS, ninguna. **Usa `unsafe`** (ver «Diálogos de archivo: `rfd`») |

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

### Ventana: `eframe` y `egui` (`specs/hito-2-ventana.md`)

Árbol de `px-app` con la configuración de arriba: 125 crates distintos en Linux, 92 en Windows y 108 en macOS (`cargo tree -p px-app -e normal`); 236 con todos los destinos y las dependencias de build; `Cargo.lock` pasa de 67 a 317 entradas. `cargo deny --locked check` da `advisories ok, bans ok, licenses ok, sources ok` sin tocar `deny.toml`; solo avisa de crates duplicadas (`windows-sys`, `objc2*`, `rustix`, `syn`, `thiserror`…), que no hacen fallar el check. `png` 0.18.1 es la misma crate que usa `px-formats` (la trae `image`, que usa `eframe`): no se duplica.

Todos los crates del árbol tienen una licencia de `allow` o una disyunción con una rama de `allow`. Las que mencionan identificadores que no están en `allow`, siempre como alternativa de un `OR` (se usa la rama MIT, Apache-2.0 o similar; los textos están en `specs/hito-2-ventana-licencias.md`):

| Crate | Licencia declarada | Nota |
| --- | --- | --- |
| `self_cell` 1.3.0 (vía `epaint`) | `Apache-2.0 OR GPL-2.0-only` | **`GPL-2.0-only` es copyleft y no se usa**: es una disyunción y se usa bajo Apache-2.0, nunca bajo la GPL, así que no afecta a la política MIT del proyecto (igual que `r-efi`, arriba) |
| `r-efi` 5.3.0 (vía `getrandom`) y 6.0.0 | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | Solo UEFI, no se compila en Linux, Windows ni macOS. Misma nota que arriba: no se usa la rama LGPL |
| `byteorder-lite` 0.1.0 (vía `image`), `memchr` 2.8.3 | `Unlicense OR MIT` | Se usa MIT |
| `same-file` 1.0.6, `walkdir` 2.5.0, `winapi-util` 0.1.11 | `Unlicense OR MIT` (o `Unlicense/MIT`) | Se usa MIT. Solo dependencias de build de Android: no se compilan en los tres sistemas |
| `adler2` 2.0.1 | `0BSD OR MIT OR Apache-2.0` | Ya anotada arriba |
| `rustix` 0.38.44 y 1.1.5, `linux-raw-sys` 0.4.15 y 0.12.1, `wasip2` 1.0.4, `wit-bindgen` 0.57.1 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | Se usa Apache-2.0 o MIT |

Sin `default_fonts`, la crate `epaint_default_fonts` (`(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`) **no entra** en el árbol, y por eso `deny.toml` no cambia. La fuente de la interfaz se decidió en la unidad 11a (`specs/hito-2-texto.md`): Departure Mono 1.500 (SIL OFL 1.1) cargada con `include_bytes!` desde `assets/fonts/`. No es una dependencia de Cargo: su origen, copyright, SHA-256 y licencia están en `docs/THIRD-PARTY.md`. Activar `default_fonts` exigiría una excepción de licencia solo para `epaint_default_fonts`; no se activa.

`libfuzzer-sys` y su excepción NCSA siguen como antes; el aviso `license-exception-not-encountered` de `cargo deny` es esa excepción, que solo aplica al crate `fuzz/`, fuera del workspace.

### Diálogos de archivo: `rfd` (`specs/hito-2-dialogos-archivo.md`)

`rfd` 0.17.2 (2026-01-12), licencia **MIT**, fijada con `=` en `crates/px-app/Cargo.toml`. `Cargo.lock` gana **2 entradas** (`rfd` y `pollster`; de 317 a 319); el resto de su árbol ya estaba por `eframe`. La entrada ya existente de `dispatch2` 0.3.1 gana dos dependencias (`block2` y `libc`), porque `rfd` la usa con sus features por defecto; ninguna es una crate nueva. `cargo deny --locked check` da `advisories ok, bans ok, licenses ok, sources ok` sin tocar `deny.toml`. Árbol de `rfd` por sistema (`cargo tree -p rfd -e normal --target …`):

| Sistema | Dependencias de `rfd` con la configuración de arriba |
| --- | --- |
| Linux | `libc`, `log`, `percent-encoding`, `pollster`, `raw-window-handle`; con la feature `wayland`, `wayland-client`, `wayland-protocols` y `wayland-backend` (con sus transitivas, que ya traía `eframe`) |
| Windows | `log`, `raw-window-handle`, `windows-sys` |
| macOS | `log`, `raw-window-handle`, `block2`, `dispatch2`, `objc2`, `objc2-app-kit`, `objc2-core-foundation`, `objc2-foundation` (ya traídas por `eframe`) |

| Crate | Versión | Licencia |
| --- | --- | --- |
| `rfd` | 0.17.2 | MIT |
| `pollster` | 0.4.0 | Apache-2.0 / MIT (disyunción, se usa MIT) |
| `raw-window-handle` | 0.6.2 | MIT OR Apache-2.0 OR Zlib |
| `percent-encoding`, `log`, `libc` | 2.3.2, 0.4.34, 0.2.191 | MIT OR Apache-2.0 |

`Cargo.lock` también lista `js-sys`, `wasm-bindgen`, `wasm-bindgen-futures` y `web-sys` como dependencias de `rfd` en WebAssembly: no se compilan en Linux, Windows ni macOS.

**`unsafe` en `rfd`.** El código de `rfd` usa `unsafe`: en Linux carga `libdbus-1.so.3` con `dlopen` en ejecución (`src/backend/xdg_desktop_portal/portal/ffi.rs` y `libdbus.rs`) y llama a la API de D-Bus por punteros; con la feature `wayland` obtiene el identificador de la ventana padre con `unsafe` (`window_identifier/mod.rs` y `wayland.rs`); en Windows y macOS llama a la API nativa (`windows-sys`, `objc2`). La regla de `CLAUDE.md` (`unsafe` prohibido salvo en `px-render`) es sobre **nuestro** código: `px-app` sigue con `unsafe_code = "forbid"` y no escribe ningún bloque `unsafe`. El `unsafe` de `rfd` es el de una dependencia aceptada por `cargo deny` y queda anotado aquí.

**Requisitos en Linux (en ejecución, no en compilación).** `xdg-desktop-portal` con un backend de GTK, GNOME o KDE (el de wlroots solo no implementa la API que usa `rfd`) y `libdbus-1.so.3`. Si el portal falla, `rfd` cae a `zenity`; sin ninguno de los dos, Pixoko avisa en la barra de estado y arrastrar y soltar y `Ctrl`+`S` siguen funcionando. El CI no necesita paquetes nuevos (la biblioteca se carga en ejecución).

**Sin comprobar:** la compilación en Windows y macOS la hace el CI en el primer *push*; en local solo se ha compilado en Linux.
