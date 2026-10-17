# Hito 0 · Unidad 2: CI en Linux, Windows y macOS

Estado: **aprobada** (2026-10-17; decisiones 1, 2 y 3 aceptadas como se recomendaban)

## Objetivo

Un workflow de GitHub Actions que, en cada push a `main` y en cada pull request, ejecuta en Linux, Windows y macOS las mismas comprobaciones que `CLAUDE.md` exige antes de cada commit: `cargo fmt`, `cargo clippy`, `cargo test` y `cargo deny`. Así los problemas propios de cada sistema aparecen en el hito 0 y no más tarde (decisión "Plataformas" de `docs/PLAN.md`).

## Entradas y salidas

- Entrada: eventos `push` (rama `main`) y `pull_request` (cualquier rama destino) del repositorio `alvp/pixoko`.
- Salida: un único archivo, `.github/workflows/ci.yml`, y un test de estructura que lo vigila. Ningún cambio en el código de los crates ni en `deny.toml`.

## Comportamiento

1. **Disparadores.** `push` a `main`, `pull_request` y `workflow_dispatch` (para relanzarlo a mano).
2. **Permisos.** `permissions: contents: read` a nivel de workflow; ninguna unidad de CI necesita escribir en el repositorio.
3. **Concurrencia.** Un grupo por workflow y referencia con `cancel-in-progress: true`, para no gastar minutos en commits ya superados de una misma rama de PR.
4. **Matriz.** Un job `ci` con `os: [ubuntu-latest, windows-latest, macos-latest]` y `fail-fast: false`, de modo que un fallo en un sistema no oculte el resultado de los otros dos. `timeout-minutes: 20`.
5. **Toolchain.** Se instala con `rustup` (ya presente en los runners), sin acciones de terceros: `rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy` y `rustup default 1.99.1`. La versión es la misma que `rust-version` del workspace (ver Decisión 1).
6. **Pasos, en este orden** (el más barato primero, para fallar pronto):
   1. `actions/checkout` (con `persist-credentials: false`).
   2. Instalar toolchain (punto 5) y `cargo-deny` con `taiki-e/install-action` (`tool: cargo-deny@0.20.2`, ver Decisión 2).
   3. `cargo fmt --all --check`
   4. `cargo clippy --workspace --all-targets --locked -- -D warnings`
   5. `cargo test --workspace --locked`
   6. `cargo build -p px-app --locked` (añadido en `specs/hito-2-ventana.md`: comprueba que la app compila y enlaza en los tres sistemas)
   7. `cargo deny --locked check`
7. **`--locked`.** Clippy, test y deny fallan si `Cargo.lock` no está al día, de modo que un lock desactualizado no pasa desapercibido.
8. **Entorno.** `CARGO_TERM_COLOR=always` y `RUST_BACKTRACE=1` a nivel de workflow. No se define `RUSTFLAGS`: los avisos ya se convierten en error con `-D warnings` en clippy.
9. **Comandos idénticos a los locales.** Los comandos de los pasos 3 a 7 son los mismos que pide `CLAUDE.md` y los criterios de aceptación de la unidad 1, salvo `--all`, `--locked` y `--all-targets`, que solo hacen la comprobación más estricta. Lo que pasa en local debe pasar en CI y al revés.
10. **Un solo shell.** Todos los `run` usan `shell: bash` para que la sintaxis sea la misma en Windows (Git Bash, preinstalado en el runner).

## Casos límite

- **Cambio de línea en Windows.** `actions/checkout` puede convertir LF en CRLF según la configuración de git del runner. `rustfmt` detecta el estilo existente y el test de estructura de la unidad 1 lee con `str::lines`, que admite ambos. Si apareciera un fallo real de `fmt --check` solo en Windows, se añade `.gitattributes` con `* text=auto eol=lf` como unidad aparte; no se añade por adelantado.
- **Rutas.** `px-golden` localiza la raíz con `CARGO_MANIFEST_DIR/../..`; es válido con `\` en Windows.
- **`cargo deny` y la red.** `cargo deny check` incluye `advisories`, que descarga la base RustSec. Una caída de la red o de GitHub haría fallar el paso aunque el código esté bien; se relanza el job. No se desactiva la comprobación.
- **Versiones flotantes.** `*-latest` cambia de imagen sin aviso. Se acepta en el hito 0; si una actualización de imagen rompe el build se fija la versión concreta del runner.
- **Dependencias del sistema.** Hoy el workspace no tiene dependencias, así que no se instala ningún paquete del sistema. Cuando `px-ui` o `px-render` necesiten bibliotecas de Linux (X11, Wayland, etc.), esa unidad añade el paso de instalación.
- **Caché.** No se añade: sin dependencias externas no hay nada que cachear y evita una acción de terceros más. Se revisó al entrar la primera dependencia grande (`eframe`, `specs/hito-2-ventana.md`, §6) y se decidió no añadirla todavía.
- **PR desde un fork.** Con `permissions: contents: read` y sin secretos, el workflow es seguro para forks.

## Criterios de aceptación

Verificables sin GitHub:

- `cargo test --workspace` pasa, incluido el test de estructura nuevo (sección Tests).
- Los comandos del punto 6 pasan en local en Linux.
- `.github/workflows/ci.yml` es YAML válido (`python3 -c 'import yaml…'` o `actionlint` si está disponible; no hay ninguno de los dos instalado ahora, así que se comprobará lo que haya).

Verificable solo con un push (lo haces tú, o me indicas que lo haga):

- El workflow se ejecuta en `alvp/pixoko` y los tres jobs terminan en verde.
- Se rompe a propósito un archivo en una rama de prueba (por ejemplo, un `fmt` mal) y se comprueba que el job falla en los tres sistemas. Es la única forma de saber que el CI protege de verdad y no solo "pasa".

## Tests

Test de estructura en `px-golden` (`crates/px-golden/tests/ci.rs`), solo con `std`, en la línea del de la unidad 1: lee `.github/workflows/ci.yml` como texto, sin parser de YAML, y comprueba:

- aparecen los tres sistemas `ubuntu-latest`, `windows-latest` y `macos-latest`;
- aparece `fail-fast: false`;
- aparecen, como líneas `run`, los cinco comandos del punto 6 (comparados tras recortar espacios);
- aparece `-D warnings` en la línea de clippy;
- los cinco comandos aparecen en el orden fmt, clippy, test, build, deny;
- `permissions:` está declarado y no hay `write` en el archivo;
- la versión de `rustup toolchain install` coincide con el prefijo de `rust-version` del `Cargo.toml` raíz (`1.99.x` frente a `1.99`);
- toda acción `uses:` está en la lista permitida (`actions/checkout` y `taiki-e/install-action`) y va fijada por SHA de commit de 40 caracteres hexadecimales, con la etiqueta de versión en un comentario;
- aparece `tool: cargo-deny@0.20.2` (versión exacta, no flotante).

Es una red de seguridad contra ediciones accidentales (alguien quita un sistema o un paso "para que pase"), no una validación de la semántica de GitHub Actions: esa solo la da una ejecución real.

## Decisiones

1. **Toolchain fijado a `1.99.1`.** Evita que un clippy nuevo rompa el CI con `-D warnings` sin que cambie nuestro código. Se sube a mano al cerrar cada hito.
2. **`cargo-deny` con `taiki-e/install-action`** (licencia Apache-2.0, compatible con MIT y no entra en el binario: solo corre en el CI), fijada por SHA y con versión exacta `cargo-deny@0.20.2`. Comprobado que esa versión tiene binario para los tres sistemas. Las acciones usadas, ambas fijadas por SHA de commit completo, son:
   - `actions/checkout` v7.0.1: `3d3c42e5aac5ba805825da76410c181273ba90b1` (MIT).
   - `taiki-e/install-action` v2.87.29: `8cca0918f8471c33515202b73f56ab27f2bed283` (Apache-2.0 OR MIT).

   Se anotan en `docs/DEPENDENCIES.md` (sección "Acciones de GitHub"), aunque no son dependencias de Cargo, porque `CLAUDE.md` pide registrar lo nuevo que se incorpora. Si prefieres no registrarlas ahí, se quita esa sección.
3. **`cargo deny` en los tres sistemas**, dentro de la misma matriz.

## Fuera de alcance

- Compilaciones de publicación, binarios, instaladores y versiones (hito 8).
- Benchmarks en CI (se añadirán con el hito del compositor).
- Fuzzing en CI (hito 1 en adelante, con los lectores de formatos).
- `.gitattributes`, caché de compilación y paquetes del sistema: se añaden solo cuando hagan falta (ver Casos límite).
- Protección de la rama `main` y comprobaciones obligatorias en GitHub: se configuran en el repositorio, no en el código.
- Unidades 3 y 4 del hito 0 (programas de referencia y prueba del hook).
