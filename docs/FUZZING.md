# Fuzzing

El objetivo `read_png` (`fuzz/fuzz_targets/read_png.rs`, spec en `specs/hito-1-fuzz.md`) llama a `read_png` con bytes arbitrarios y busca pánicos, desbordamientos de enteros, agotamiento de memoria y de tiempo. Si `read_png` acepta la entrada, comprueba además que `write_png` y una nueva lectura devuelven la misma imagen. **El CI no ejecuta el fuzzer**: se lanza a mano.

## Requisitos

- Toolchain nightly con `rust-src` (`rustup toolchain install nightly --component rust-src`): `cargo-fuzz` compila con `-Zbuild-std` y AddressSanitizer.
- Un compilador de C++ (`clang` o `g++`) para `libfuzzer-sys`.
- `cargo install cargo-fuzz --version 0.13.2 --locked`.

## Lanzarlo

Desde la raíz del repo:

```
cargo +nightly fuzz run read_png fuzz/corpus/read_png golden/png -- \
  -rss_limit_mb=4096 -timeout=10 -create_missing_dirs=1
```

Sin `-max_total_time` corre hasta que lo pares (Ctrl-C) o encuentre un fallo; para una pasada corta, añade `-max_total_time=60`.

- **El corpus generado va primero** (`fuzz/corpus/read_png`) porque libFuzzer escribe las entradas nuevas en el primer directorio. `golden/png` va detrás, solo como semillas (también lee su `README.md` y los `.sha256`; es inofensivo). Si lo pusieras primero, el fuzzer llenaría `golden/png/` de ficheros y fallaría `png_interop`.
- `-create_missing_dirs=1` crea `fuzz/corpus/read_png` la primera vez.
- `-rss_limit_mb=4096`: la lectura puede llegar por diseño a 2 GiB (`specs/hito-1-png.md`, comportamiento 7), y una cabecera que declara 1 GiB hace reservar esa memoria al instante. El límite por defecto de libFuzzer (2048) quedaría al borde.
- `-timeout=10`: una entrada que tarda más de 10 s es un fallo.

`fuzz/corpus/`, `fuzz/artifacts/`, `fuzz/target/` y `fuzz/coverage/` están en `.gitignore`: no se commitea nada generado.

**Al terminar una sesión larga:** `git status golden/` debe quedar limpio (sin ficheros nuevos ni cambios). Si no lo está, el corpus dorado se ha ensuciado: borra lo que sobre con `git clean -n golden/` (revisa antes) y mira cómo se lanzó.

## Reproducir un fallo

Un fallo deja su entrada en `fuzz/artifacts/read_png/` (`crash-…` para un pánico o una aserción, `oom-…` para memoria, `timeout-…` para tiempo, `slow-unit-…` para entradas lentas que no llegan al límite).

```
cargo +nightly fuzz run read_png fuzz/artifacts/read_png/crash-<hash>     # lo repite
cargo +nightly fuzz tmin read_png fuzz/artifacts/read_png/crash-<hash>    # lo minimiza
xxd -i <entrada minimizada>                                               # los bytes para el test
```

Cada fallo se convierte en un test de regresión en `crates/px-formats/tests/png_fuzz.rs`, con los bytes mínimos como `const` en el código (sin ficheros `.png` nuevos), y se corrige en el lector; un fallo por commit. El test comprueba el resultado correcto según la spec (la variante exacta de `PngError`, o la imagen y el informe), no solo que no haya pánico, y debe fallar antes del arreglo.

## Comprobaciones locales de la crate del fuzzer

`fuzz/` está fuera del workspace, así que `cargo fmt --all`, `cargo clippy --workspace`, `cargo test --workspace` y el `cargo deny` del CI no la cubren. Antes de commitear un cambio en `fuzz/`:

```
cargo fmt --manifest-path fuzz/Cargo.toml --check
cargo clippy --manifest-path fuzz/Cargo.toml -- -D warnings
cargo deny --manifest-path fuzz/Cargo.toml --locked check
```

Las dos primeras compilan con la toolchain estable. Las dependencias del fuzzer y su excepción de licencia (NCSA, solo para `libfuzzer-sys`) están en `docs/DEPENDENCIES.md`.
