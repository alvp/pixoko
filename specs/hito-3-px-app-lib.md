# Hito 3 · Unidad 3.0: `px-app` en biblioteca y binario

Estado: **aprobada** (2026-10-21; decisiones 1 a 6 como están, con las condiciones de «Visto bueno» al final, que **prevalecen sobre cualquier otra frase de la página**).

Referencias: fila 3.0 y sección 3 de [`mapa-funciones.md`](mapa-funciones.md); origen en `hito-2-puerta.md`, D1 (la puerta se quedó como módulo `#[cfg(test)]` y se anotó este candidato). Clase: deseable.

Cambios: solo `px-app` (`Cargo.toml`, `main.rs` → `lib.rs`, un `main.rs` nuevo y una prueba nueva). **No se toca `px-core`, `px-raster`, `px-golden`, `ci.rs`, `ci.yml`, `deny.toml` ni `fuzz/`.** Sin dependencias nuevas ni `unsafe`; `Cargo.lock` y `docs/DEPENDENCIES.md` no cambian. **Ningún comportamiento visible cambia.**

## Objetivo

Hoy `px-app` es un binario sin `lib.rs`: sus módulos, `Session` y `App` solo se ven desde dentro, y la puerta del hito 2 (`puerta`, 3881 líneas) y `damero_guardado` cuelgan de `main.rs` como `#[cfg(test)]`. Se parte en una biblioteca (`px_app`) con toda la lógica y un `main.rs` mínimo. Es la base de 3.3 (la sesión con capas) y de 3.9 (puerta del hito 3); no añade funciones.

## Lo comprobado

- `main.rs` (1296 líneas) tiene: 24 `mod`, `PendingConversion`, `App` con dos bloques `impl` y `impl eframe::App`, y `fn main` (líneas 1247 a 1296: argumentos, `--licencias`, viewport y mínimo de ventana, fuentes, tema, atajos del usuario, iconos y `run_native`).
- Las variables de entorno se leen en `main` (`ENV_NO_MIN`, la carpeta de configuración) **y dentro de `App::new`** (`ENV_MESH`, `ENV_NO_MIN`, `ENV_DEMO_DIALOG`). `App::new(document, cli_path)` lo llaman dos pruebas (`session/tiles_tests.rs`, `session/transparent_tests.rs`).
- Los módulos de pruebas están **dentro de cada módulo** (`session/*_tests.rs`, `confirm/*_tests.rs`, `puerta/…`), no en `main.rs`; en `main.rs` solo cuelgan `puerta` y `damero_guardado`. Casi todo es `pub` dentro de módulos privados (solo hay un `pub(crate)`); `pub` y `pub(crate)` valen lo mismo mientras el módulo sea privado.
- Dos pruebas usan `crate::App` y sus campos y métodos privados (`canvas`, `session`, `notice`, `tile_alloc`, `apply_palette_op`, `finish_new_image`, `open_file`, `run_convert`): ven esos privados porque `App` está en la raíz del crate.
- La puerta usa de `px-app`: `Session`, `Saved`, `FileError`, `Confirm`, `DialogKind`, `DialogRequest`, `Dialogs`, `FilePicker`, `Finished`, `Outcome` y `open_gate` (11 nombres).
- `ci.yml` ejecuta `cargo build -p px-app --locked`; `ci.rs` fija ese comando en `EXPECTED_COMMANDS` y no menciona ficheros de `src/`. `structure.rs` solo exige que existan los 8 directorios de `crates/`. `fuzz/` depende solo de `px-core` y `px-formats`.
- Línea base: `cargo test -p px-app -- --list` da **1063 pruebas** (lista guardada fuera del repositorio para compararla al cerrar).

## Decisiones

**1. Qué va a la lib y qué al binario.** Todo a la lib, salvo un `main.rs` de unas diez líneas:

```rust
fn main() -> eframe::Result {
    px_app::run(std::env::args_os().skip(1).collect())
}
```

La lib trae `pub fn run(args: Vec<OsString>) -> eframe::Result` con el cuerpo actual de `main` **sin tocar una línea** (`--licencias`, `parse_args` con `exit(2)`, viewport, mínimo, fuentes, tema, atajos, iconos, `run_native`). Razones, y en qué difiere de dejar argumentos y variables en el binario:
- Tres de las cuatro variables de depuración (`PIXOKO_DAMERO_MALLA`, `PIXOKO_SIN_MINIMO`, `PIXOKO_DIALOGO_PRUEBA`) se leen **dentro de `App::new`**: dejarlas en el binario obliga a pasarlas por argumento y cambiar la firma que llaman dos pruebas, y la unidad es «solo estructura».
- Dejar `parse_args`, `wants_licenses`, `min_inner_size`, `ShortcutStore` e `icons::load` en el binario obliga a hacerlos públicos en la lib (hasta ocho nombres más) solo para que el binario los llame.
- El arranque no se puede probar sin ventana de todos modos; lo que sí (`--licencias`, argumento inválido) se prueba ejecutando el binario (filas 3 y 4).

`std::process::exit(2)` por argumento inválido se queda dentro de `run` para no cambiar nada; limpiarlo es del hito 8 (CLI).

**2. API pública mínima.** `lib.rs` declara **todos** los `mod` privados y exporta una sola cosa: `pub fn run`. No se reescriben los `pub` actuales a `pub(crate)`: dentro de un módulo privado son equivalentes, el diff sería de cientos de líneas sin efecto, y el `dead_code` actúa igual que hoy (un `pub` sin uso en un módulo privado de una lib sigue avisando). `App`, `PendingConversion` y sus `impl` se quedan en la raíz de la lib: **así las dos pruebas que usan `crate::App` no cambian ni un carácter**. Partir `App` en `app.rs` obligaría a `pub(crate)` en esos campos y métodos (ocho nombres) y a editar esas dos pruebas: no es de esta unidad; si 3.3 lo necesita, lo decide allí.

**3. Dónde viven las pruebas de la puerta.** `#[cfg(test)] mod puerta` y `mod damero_guardado` pasan **tal cual** a `lib.rs`: mismos ficheros, mismas rutas `crate::…`, mismos nombres de prueba (`puerta::filas::…`), un solo ejecutable de pruebas. **`px-golden` no puede usar `Session` tras la 3.0**, porque la lib solo exporta `run`. Mover la puerta a `px-golden/tests/` exigiría exportar los 11 nombres de arriba (sin consumidor real hasta 3.9, y `Session` cambia en 3.3), añadir `px-app`, `px-ui` y `proptest` como dev-dependencias de `px-golden` (cambia `Cargo.lock`, `cargo test -p px-golden` pasa a compilar `eframe`) y reescribir 6 líneas `use`; y `px-golden/png_interop/gate_oraculo.rs`, que existe por esa limitación, ya es el oráculo independiente. **Se decide en 3.9**, cuando haya un caso (sesión con capas más dorados) que lo pida.

**4. CI, `cargo deny` y fuzzer.**
- Nombre del binario: el del paquete, `px-app` (`px-app.exe` en Windows), igual que hoy; la lib es `libpx_app`. `cargo build -p px-app --locked` construye ambos. **`ci.rs`, sus `ALLOWED_ACTIONS` y `EXPECTED_COMMANDS` y `ci.yml` no cambian y no hace falta tocarlos.** Los tres sistemas ejecutan los mismos cinco comandos.
- `cargo deny`: el grafo de paquetes y `Cargo.lock` son los mismos (un paquete con dos objetivos). Sin cambios en `deny.toml` ni en `docs/DEPENDENCIES.md`.
- Fuzzer: no depende de `px-app`; sin efecto.
- En `px-app/Cargo.toml`: `[lib] doctest = false` (no hay doctests; evita un `rustdoc` por crate) y `[[bin]] name = "px-app", path = "src/main.rs", test = false` (el binario no tiene pruebas; un ejecutable de pruebas vacío menos que enlazar). Las pruebas de la lib siguen en un único ejecutable. Las `#[ignore]` de 8192² siguen con `cargo test -p px-app --release -- --ignored`.

**5. Pruebas anteriores que cambian.** **Ninguna.** Ficheros, rutas de módulo, visibilidad y firmas quedan como están; `git mv main.rs lib.rs` conserva el historial. Lo único que se escribe nuevo es `main.rs`, el `fn run` (el antiguo `main` con otro nombre y `pub`) y `tests/arranque.rs`. Si al compilar apareciera algo que obligara a tocar una prueba anterior, me paro y aviso, sea cual sea el cambio.

## Casos (7 filas; la 1 a la 4 son pruebas nuevas en `crates/px-app/tests/arranque.rs`, un solo ejecutable; la 5 a la 7, órdenes del cierre)

| # | Caso | Se comprueba con |
| --- | --- | --- |
| 1 | `src/lib.rs` y `src/main.rs` existen; `main.rs` no declara `mod`, solo llama a `px_app::run` y tiene ≤ 12 líneas de código (sin comentarios ni vacías) | prueba de texto |
| 2 | API mínima: en `lib.rs` ninguna línea empieza por `pub mod`, `pub use`, `pub struct`, `pub enum`, `pub trait` o `pub const`, y la única `pub fn` de nivel 0 es `run` | prueba de texto |
| 3 | `px-app --licencias` escribe en la salida estándar exactamente `docs/THIRD-PARTY.md` y sale con 0, sin ventana | prueba con `CARGO_BIN_EXE_px-app` |
| 4 | `px-app a b` (dos argumentos) escribe `uso: px-app [archivo.png]` en la salida de errores, nada en la estándar, y sale con 2 | ídem |
| 5 | `cargo test -p px-app -- --list` da los 1063 nombres de antes (ni uno menos ni renombrado) más las 4 pruebas nuevas, todas en verde | `diff` de listas ordenadas |
| 6 | CI: `cargo build -p px-app --locked` deja `target/debug/px-app`; las pruebas de `px-golden` (`ci`, `structure`) pasan sin tocarlas; `git diff` no toca `Cargo.lock`, `deny.toml`, `ci.yml` ni `ci.rs`; `cargo deny --locked check` igual que antes | órdenes del cierre |
| 7 | Sin avisos: `cargo clippy --workspace --all-targets --locked -- -D warnings` y `cargo build -p px-app` limpios (ningún ítem queda sin usar al pasar `main` a la lib) | órdenes del cierre |

## Propiedades (2; no son de `proptest`: hay que comparar dos estados del repositorio)

- **P1 · Misma batería.** Para cada prueba anterior, su nombre completo existe en la lib y su resultado no cambia (en verde antes, en verde después); lo mismo con las `#[ignore]` (mismos nombres, siguen ignoradas). Es la fila 5 como invariante.
- **P2 · Mismo comportamiento fuera de la ventana.** Para `--licencias` y para uno, dos y tres argumentos inválidos, el binario nuevo y el del commit anterior (compilado en `CARGO_TARGET_DIR=~/.cache/pixoko-scratch`, que se borra al terminar) dan los mismos bytes en la salida estándar y en la de errores y el mismo código de salida. El caso con ventana (sin argumentos o con un PNG) es de la prueba manual.

## Orden de commits (modo ahorro: sin `test-writer`, sin `license-auditor`: no hay dependencias ni código de producción nuevos)

1. Spec (este fichero y su fila del índice), con una sola corrección posterior al final.
2. `git mv main.rs lib.rs` **solo**, sin ningún cambio de contenido (el commit no compila como binario: así el historial sigue el fichero).
3. `fn run` en `lib.rs`, `main.rs` nuevo y `Cargo.toml`. **Sin pruebas anteriores que adaptar**, así que no hay commit de adaptación.
4. Pruebas: `tests/arranque.rs` (las filas 1 a 4), tras la estructura (orden indicado en el visto bueno; las cuatro nacen en verde).
4. Documentación: `PLAN-RESUMEN.md` (fila de `px-app` de la tabla de crates y la 3.0), `mapa-funciones.md` (la fila 3.0 y la sección 9) y `hito-2-puerta.md` (una línea: «hecho en la 3.0»).

Cierre: `cargo fmt`, clippy `-D warnings`, `cargo test --workspace --locked`, `cargo deny` y P1 y P2, todo una sola vez.

## Prueba manual (la haces tú, sin capturas)

1. `cargo run -p px-app` abre la ventana de siempre (imagen de demostración, título, menús).
2. `cargo run -p px-app -- <un PNG>` abre ese PNG; `--licencias` imprime las licencias y vuelve a la terminal.
3. Dibujar con el lápiz, deshacer y rehacer, y `Ctrl+S` sobre una copia.
4. `F1` abre la ayuda; `Ayuda → Atajos de teclado…` abre la lista.
5. `PIXOKO_SIN_MINIMO=1` deja encoger la ventana por debajo del mínimo (la variable la lee ahora la lib).

## Fuera de alcance

Partir `App` en `app.rs`; exportar `Session` o cualquier otro tipo; mover la puerta a `px-golden`; quitar el `exit(2)` de `run`; cambiar nombres de módulos o de ficheros; tocar `ci.rs`.

## Decisiones que necesito que confirmes

1. **`run(args)` en la lib y `main` de una línea** (en vez de dejar argumentos y variables de entorno en el binario). Razón: tres variables se leen dentro de `App::new`; lo otro ensancha la API y cambia una firma que usan dos pruebas.
2. **`App` y `PendingConversion` se quedan en la raíz de `lib.rs`**, y los `pub` actuales no se reescriben a `pub(crate)`; la lib exporta solo `run`. Con ello, cero pruebas anteriores cambian.
3. **La puerta se queda en `px-app` como `#[cfg(test)]`; `px-golden` no usa `Session` hasta 3.9.** Alternativa: exportar los 11 nombres y moverla ahora (coste en la decisión 3 de arriba).
4. **`[lib] doctest = false` y `[[bin]] test = false`** en `px-app/Cargo.toml`.
5. **Las pruebas nuevas van en `px-app/tests/arranque.rs`** (la lectura de `lib.rs` y `main.rs` y la ejecución del binario), no en `px-golden/tests/structure.rs`; así no se toca nada de `px-golden`.
6. **P1 y P2 son comprobaciones del cierre** (no ficheros de prueba), porque comparan dos commits.

## Visto bueno (2026-10-21)

Decisiones 1 a 6 aprobadas con estas condiciones:

1. `run` conserva exactamente la semántica de salida de hoy: `--licencias` imprime y sale con 0; dos argumentos dan el error de uso con código 2. `arranque.rs` lo fija lanzando el binario con `env!("CARGO_BIN_EXE_px-app")`.
2. **La unidad 3.9 deberá exportar los 11 nombres de la puerta con la misma regla (lo mínimo) y mover entonces la puerta a `px-golden` si procede.**
3. `git mv main.rs lib.rs` va en un commit **aparte**, sin ningún cambio de contenido; el `main.rs` mínimo y el resto, en otro.
4. Se mide el binario de `release` antes y después (tamaño y configuración de optimización: no hay `[profile]` en el `Cargo.toml` raíz ni en el del paquete, así que rige el perfil por defecto de Cargo: `opt-level` 3, sin LTO) y se informan las dos cifras.
5. P1: la lista de nombres de prueba de antes (`px-app-antes.txt`) debe ser idéntica a la de después salvo las 4 pruebas nuevas de `arranque.rs`.
6. Orden de trabajo: spec; `git mv`; `main` mínimo y lib; pruebas de arranque; cierre completo una sola vez. Sin auditor (sin dependencias ni cambios en `px-core` o `px-raster`). No se tocan `ci.rs`, `ci.yml`, `deny.toml`, `Cargo.lock` ni `fuzz/`; si algo lo exigiera, parar.

## Corrección posterior (al cerrar la unidad; la única)

- P2 decía «uno, dos y tres argumentos inválidos», pero **un argumento es un fichero válido** y abre ventana. Se comparó el binario anterior y el nuevo (release) con `--licencias`, `a b`, `a b c`, `a b c d`, `x --licencias` y `--licencias a b`: mismos bytes en las dos salidas y mismo código (0 o 2) en los seis.
- P1: la lista de nombres solo gana las 4 pruebas de `arranque.rs` (1063 → 1067); `cargo test --workspace` sin fallos; las 5 `#[ignore]` de `px-app` siguen ignoradas.
- Binario de `release`: 12 677 568 bytes antes y 12 676 896 después (−672). No hay `[profile]` en ningún `Cargo.toml` (ni raíz ni paquete; Cargo solo admite perfiles en la raíz), así que antes y después rige el perfil por defecto (`opt-level` 3, sin LTO).
- `Cargo.lock`, `deny.toml`, `ci.yml`, `ci.rs`, `fuzz/`, `px-core`, `px-raster` y `px-golden`: sin cambios. `cargo deny`, clippy `-D warnings` y fmt, limpios.
- **3.9** deberá exportar los 11 nombres de la puerta con la misma regla (lo mínimo) y mover entonces la puerta a `px-golden` si procede.
