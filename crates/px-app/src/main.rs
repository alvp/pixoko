//! Pixoko: el binario solo arranca la biblioteca (specs/hito-3-px-app-lib.md).

fn main() -> eframe::Result {
    px_app::run(std::env::args_os().skip(1).collect())
}
