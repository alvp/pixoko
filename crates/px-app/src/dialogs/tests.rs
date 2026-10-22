//! Tests de `dialogs` (specs/hito-2-dialogos-archivo.md): filas 1, 2, 3, 4, 7, 11 y 12 (textos de
//! `dialogs.rs` y `open_dialog_warning`) y la parte de `dialogs.rs` de la fila 9
//! (`fallback_request`, `unavailable_notice` y `Cancelled` rápido).
//!
//! Los diálogos se prueban con un `FakePicker` que resuelve cuando el test lo dice y cuenta las
//! llamadas a `start` y `abandon`. El instante `now` lo da el test; nunca hay ventana, y el
//! padre es siempre `None`.

use super::*;

use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;
use px_ui::text::INTERFACE_CHARSET;

use crate::confirm::{Confirmation, open_dialog_warning};

// ---------------------------------------------------------------------------------------
// Ayudantes: directorio temporal, selector falso y atajos.
// ---------------------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Directorio temporal propio del test; el guardián lo borra al soltarse.
struct TmpDir {
    path: PathBuf,
}

impl TmpDir {
    fn new() -> TmpDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("pixoko-px-app-dialogs-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).expect("crear el directorio temporal");
        TmpDir { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Default)]
struct FakeState {
    starts: usize,
    abandons: usize,
    last: Option<DialogRequest>,
    /// `None`: sigue abierto. `Some(r)`: lo que devolverá el próximo `poll`.
    result: Option<Option<PathBuf>>,
}

/// Mando del test sobre el `FakePicker` que se entregó a `Dialogs`.
#[derive(Clone, Default)]
struct Handle(Rc<RefCell<FakeState>>);

impl Handle {
    /// El diálogo se cierra sin ruta.
    fn close(&self) {
        self.0.borrow_mut().result = Some(None);
    }

    /// El usuario elige `path`.
    fn choose(&self, path: &str) {
        self.0.borrow_mut().result = Some(Some(PathBuf::from(path)));
    }

    fn starts(&self) -> usize {
        self.0.borrow().starts
    }

    fn abandons(&self) -> usize {
        self.0.borrow().abandons
    }

    fn last(&self) -> Option<DialogRequest> {
        self.0.borrow().last.clone()
    }
}

struct FakePicker(Handle);

impl FilePicker for FakePicker {
    fn start(&mut self, request: &DialogRequest, parent: Option<&eframe::Frame>) {
        assert!(parent.is_none(), "los tests no tienen ventana");
        let mut state = self.0.0.borrow_mut();
        state.starts += 1;
        state.last = Some(request.clone());
        state.result = None;
    }

    fn poll(&mut self) -> Option<Option<PathBuf>> {
        self.0.0.borrow_mut().result.take()
    }

    fn abandon(&mut self) {
        self.0.0.borrow_mut().abandons += 1;
    }
}

/// `Dialogs` sobre un `FakePicker`, con su mando.
fn dialogs(fast_failure: bool, cwd: Option<&str>) -> (Dialogs, Handle) {
    let handle = Handle::default();
    let dialogs = Dialogs::new(
        Box::new(FakePicker(handle.clone())),
        fast_failure,
        cwd.map(PathBuf::from),
    );
    (dialogs, handle)
}

/// Abre un diálogo de abrir sin ruta ni descarte en el instante `now`.
fn begin_open(dialogs: &mut Dialogs, now: f64) -> bool {
    dialogs.begin(DialogKind::Open, false, None, false, now, None)
}

fn request(kind: DialogKind, dir: Option<&str>, file_name: Option<&str>) -> DialogRequest {
    DialogRequest {
        kind,
        dir: dir.map(PathBuf::from),
        file_name: file_name.map(str::to_string),
    }
}

fn assert_in_charset(text: &str) {
    for c in text.chars() {
        assert!(
            INTERFACE_CHARSET.contains(c),
            "{text:?} lleva {c:?}, que no está en INTERFACE_CHARSET"
        );
    }
}

const KINDS: [DialogKind; 3] = [DialogKind::Open, DialogKind::Save, DialogKind::SaveAs];

// ---------------------------------------------------------------------------------------
// Constantes.
// ---------------------------------------------------------------------------------------

#[test]
fn las_constantes_son_las_de_la_spec() {
    assert_eq!(UNAVAILABLE_SECS, 0.5);
    assert_eq!(REPAINT_EVERY, Duration::from_millis(100));
    assert_eq!(
        IGNORED_FILE_WHILE_DIALOG,
        "se ignora el fichero: hay un diálogo abierto"
    );
}

// ---------------------------------------------------------------------------------------
// Fila 1: directorio y nombre propuestos.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_1_abrir_con_ruta_propone_el_directorio_del_fichero() {
    let got = request_for(
        DialogKind::Open,
        Some(Path::new("/a/b/casa.png")),
        false,
        Some(Path::new("/x")),
        Some(Path::new("/c")),
    );
    assert_eq!(got, request(DialogKind::Open, Some("/a/b"), None));
}

#[test]
fn fila_1_abrir_sin_ruta_propone_el_ultimo_directorio() {
    let got = request_for(
        DialogKind::Open,
        None,
        false,
        Some(Path::new("/x")),
        Some(Path::new("/c")),
    );
    assert_eq!(got, request(DialogKind::Open, Some("/x"), None));
}

#[test]
fn fila_1_abrir_sin_ruta_ni_ultimo_propone_el_directorio_actual() {
    let got = request_for(DialogKind::Open, None, false, None, Some(Path::new("/c")));
    assert_eq!(got, request(DialogKind::Open, Some("/c"), None));
}

#[test]
fn fila_1_abrir_sin_nada_no_propone_directorio() {
    let got = request_for(DialogKind::Open, None, false, None, None);
    assert_eq!(got, request(DialogKind::Open, None, None));
}

#[test]
fn fila_1_abrir_nunca_propone_nombre_ni_aunque_se_convierta() {
    let got = request_for(
        DialogKind::Open,
        Some(Path::new("/a/b/casa.png")),
        true,
        None,
        None,
    );
    assert_eq!(got.file_name, None);
}

#[test]
fn fila_1_guardar_como_con_ruta_propone_el_nombre_del_fichero() {
    let got = request_for(
        DialogKind::SaveAs,
        Some(Path::new("/a/b/casa.png")),
        false,
        Some(Path::new("/x")),
        Some(Path::new("/c")),
    );
    assert_eq!(
        got,
        request(DialogKind::SaveAs, Some("/a/b"), Some("casa.png"))
    );
}

#[test]
fn fila_1_guardar_como_convertido_propone_el_nombre_de_la_copia() {
    let got = request_for(
        DialogKind::SaveAs,
        Some(Path::new("/a/b/casa.png")),
        true,
        Some(Path::new("/x")),
        Some(Path::new("/c")),
    );
    assert_eq!(
        got,
        request(DialogKind::SaveAs, Some("/a/b"), Some("casa-copia.png"))
    );
}

#[test]
fn fila_1_guardar_con_ruta_propone_lo_mismo_que_guardar_como() {
    let normal = request_for(
        DialogKind::Save,
        Some(Path::new("/a/b/casa.png")),
        false,
        None,
        None,
    );
    assert_eq!(
        normal,
        request(DialogKind::Save, Some("/a/b"), Some("casa.png"))
    );
    let converted = request_for(
        DialogKind::Save,
        Some(Path::new("/a/b/casa.png")),
        true,
        None,
        None,
    );
    assert_eq!(
        converted,
        request(DialogKind::Save, Some("/a/b"), Some("casa-copia.png"))
    );
}

#[test]
fn fila_1_guardar_y_guardar_como_sin_ruta_proponen_sin_titulo() {
    for kind in [DialogKind::Save, DialogKind::SaveAs] {
        let with_last = request_for(
            kind,
            None,
            false,
            Some(Path::new("/x")),
            Some(Path::new("/c")),
        );
        assert_eq!(with_last, request(kind, Some("/x"), Some("sin-titulo.png")));

        let with_cwd = request_for(kind, None, false, None, Some(Path::new("/c")));
        assert_eq!(with_cwd, request(kind, Some("/c"), Some("sin-titulo.png")));

        let nothing = request_for(kind, None, false, None, None);
        assert_eq!(nothing, request(kind, None, Some("sin-titulo.png")));
    }
}

#[test]
fn fila_1_la_copia_propuesta_es_la_de_copy_path() {
    // `casa-copia.png` ya existe: `copy_path` da el siguiente nombre libre.
    let tmp = TmpDir::new();
    fs::write(tmp.path().join("casa-copia.png"), b"x").unwrap();
    let original = tmp.path().join("casa.png");

    let got = request_for(DialogKind::SaveAs, Some(&original), true, None, None);

    assert_eq!(got.file_name.as_deref(), Some("casa-copia-2.png"));
    assert_eq!(got.dir.as_deref(), Some(tmp.path()));
}

// ---------------------------------------------------------------------------------------
// Fila 2: with_png_extension.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_2_with_png_extension() {
    let cases = [
        ("casa", "casa.png"),
        ("casa.png", "casa.png"),
        ("casa.PNG", "casa.PNG"),
        ("casa.Png", "casa.Png"),
        ("casa.bmp", "casa.bmp.png"),
        ("casa.tar.gz", "casa.tar.gz.png"),
        (".png", ".png.png"),
        ("dir/casa", "dir/casa.png"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            with_png_extension(PathBuf::from(input)),
            PathBuf::from(expected),
            "entrada {input:?}"
        );
    }
}

proptest! {
    /// Un nombre sin extensión `png` recibe `.png` al final y nada más; con ella, queda igual.
    #[test]
    fn fila_2_propiedad_with_png_extension(
        stem in "[a-zA-Z0-9]{1,8}",
        ext in prop::option::of("[a-zA-Z0-9]{1,4}"),
    ) {
        let name = match &ext {
            Some(ext) => format!("{stem}.{ext}"),
            None => stem.clone(),
        };
        let input = PathBuf::from("dir").join(&name);
        let out = with_png_extension(input.clone());
        let is_png = ext.as_deref().is_some_and(|e| e.eq_ignore_ascii_case("png"));
        if is_png {
            prop_assert_eq!(&out, &input);
        } else {
            prop_assert_eq!(&out, &PathBuf::from("dir").join(format!("{name}.png")));
        }
        // Idempotente.
        prop_assert_eq!(with_png_extension(out.clone()), out);
    }
}

// ---------------------------------------------------------------------------------------
// Fila 3: begin, poll y repaint_after.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_3_sin_dialogo_no_hay_pendiente_ni_repintado_ni_resultado() {
    let (mut dialogs, handle) = dialogs(true, None);
    assert!(!dialogs.pending());
    assert_eq!(dialogs.repaint_after(), None);
    assert_eq!(dialogs.poll(1.0), None);
    assert_eq!(handle.starts(), 0);
}

#[test]
fn fila_3_un_dialogo_abierto_impide_otro_y_se_consulta_hasta_resolver() {
    let (mut dialogs, handle) = dialogs(true, None);

    assert!(begin_open(&mut dialogs, 0.0));
    assert!(dialogs.pending());
    assert!(!begin_open(&mut dialogs, 0.1), "el segundo begin es falso");
    assert_eq!(handle.starts(), 1, "start se llamó una sola vez");

    assert_eq!(dialogs.poll(0.2), None);
    assert!(dialogs.pending());
    assert_eq!(dialogs.repaint_after(), Some(Duration::from_millis(100)));
    assert_eq!(dialogs.repaint_after(), Some(REPAINT_EVERY));

    handle.choose("/a/b/casa.png");
    assert_eq!(
        dialogs.poll(0.3),
        Some(Finished {
            kind: DialogKind::Open,
            discard: false,
            outcome: Outcome::Chosen(PathBuf::from("/a/b/casa.png")),
        })
    );
    assert!(!dialogs.pending());
    assert_eq!(dialogs.repaint_after(), None);
    assert_eq!(dialogs.poll(0.4), None, "el resultado se da una sola vez");
}

#[test]
fn fila_3_un_dialogo_largo_sigue_pendiente() {
    let (mut dialogs, _handle) = dialogs(true, None);
    assert!(begin_open(&mut dialogs, 0.0));
    for now in [0.1, 1.0, 60.0, 3600.0] {
        assert_eq!(dialogs.poll(now), None, "now {now}");
        assert!(dialogs.pending());
    }
}

#[test]
fn fila_3_begin_pasa_al_selector_lo_que_da_request_for() {
    let (mut dialogs, handle) = dialogs(true, Some("/c"));
    let path = Path::new("/a/b/casa.png");
    assert!(dialogs.begin(DialogKind::SaveAs, false, Some(path), true, 0.0, None));
    assert_eq!(
        handle.last(),
        Some(request(
            DialogKind::SaveAs,
            Some("/a/b"),
            Some("casa-copia.png")
        ))
    );
}

#[test]
fn fila_3_sin_ruta_usa_el_directorio_actual_y_luego_el_ultimo_directorio() {
    let (mut dialogs, handle) = dialogs(true, Some("/c"));
    assert!(begin_open(&mut dialogs, 0.0));
    assert_eq!(
        handle.last(),
        Some(request(DialogKind::Open, Some("/c"), None))
    );
    handle.close();
    assert!(dialogs.poll(5.0).is_some());

    dialogs.remember_dir(Path::new("/x/foto.png"));

    assert!(dialogs.begin(DialogKind::Save, false, None, false, 6.0, None));
    assert_eq!(
        handle.last(),
        Some(request(
            DialogKind::Save,
            Some("/x"),
            Some("sin-titulo.png")
        ))
    );
}

#[test]
fn fila_3_sin_cwd_ni_ultimo_directorio_no_se_propone_directorio() {
    let (mut dialogs, handle) = dialogs(true, None);
    assert!(begin_open(&mut dialogs, 0.0));
    assert_eq!(handle.last(), Some(request(DialogKind::Open, None, None)));
}

#[test]
fn fila_3_el_descarte_y_el_tipo_se_conservan_hasta_el_resultado() {
    for kind in KINDS {
        for discard in [false, true] {
            let (mut dialogs, handle) = dialogs(false, None);
            assert!(dialogs.begin(kind, discard, None, false, 0.0, None));
            handle.choose("/a/casa.png");
            assert_eq!(
                dialogs.poll(1.0),
                Some(Finished {
                    kind,
                    discard,
                    outcome: Outcome::Chosen(PathBuf::from("/a/casa.png")),
                }),
                "{kind:?} discard {discard}"
            );
        }
    }
}

#[test]
fn fila_3_tras_resolver_se_puede_abrir_otro() {
    let (mut dialogs, handle) = dialogs(true, None);
    assert!(begin_open(&mut dialogs, 0.0));
    handle.choose("/a/casa.png");
    assert!(dialogs.poll(1.0).is_some());

    assert!(begin_open(&mut dialogs, 2.0));
    assert_eq!(handle.starts(), 2);
    assert!(dialogs.pending());
}

// ---------------------------------------------------------------------------------------
// Fila 4: qué es `Unavailable`.
// ---------------------------------------------------------------------------------------

/// `begin` en `begin_at`, el diálogo se cierra sin ruta y se consulta en `poll_at`.
fn closed_without_path(
    fast_failure: bool,
    kind: DialogKind,
    begin_at: f64,
    poll_at: f64,
) -> Outcome {
    let (mut dialogs, handle) = dialogs(fast_failure, None);
    assert!(dialogs.begin(kind, false, None, false, begin_at, None));
    handle.close();
    let finished = dialogs.poll(poll_at).expect("el diálogo se cerró");
    assert_eq!(finished.kind, kind);
    finished.outcome
}

#[test]
fn fila_4_cierre_rapido_con_fast_failure_es_unavailable() {
    assert_eq!(
        closed_without_path(true, DialogKind::Open, 0.0, 0.2),
        Outcome::Unavailable
    );
}

#[test]
fn fila_4_cierre_lento_con_fast_failure_es_cancelled() {
    assert_eq!(
        closed_without_path(true, DialogKind::Open, 0.0, 3.0),
        Outcome::Cancelled
    );
}

#[test]
fn fila_4_cierre_rapido_sin_fast_failure_es_cancelled() {
    assert_eq!(
        closed_without_path(false, DialogKind::Open, 0.0, 0.2),
        Outcome::Cancelled
    );
}

#[test]
fn fila_4_ruta_elegida_al_instante_es_siempre_chosen() {
    for fast_failure in [true, false] {
        let (mut dialogs, handle) = dialogs(fast_failure, None);
        assert!(begin_open(&mut dialogs, 0.0));
        handle.choose("/a/casa.png");
        let finished = dialogs.poll(0.0).expect("resuelto");
        assert_eq!(
            finished.outcome,
            Outcome::Chosen(PathBuf::from("/a/casa.png")),
            "fast_failure {fast_failure}"
        );
    }
}

#[test]
fn fila_4_el_plazo_cuenta_desde_begin_y_el_limite_es_cancelled() {
    // «Antes de UNAVAILABLE_SECS desde begin»: 0,5 s justos ya no es un fallo.
    assert_eq!(
        closed_without_path(true, DialogKind::Open, 10.0, 10.0 + UNAVAILABLE_SECS - 0.01),
        Outcome::Unavailable
    );
    assert_eq!(
        closed_without_path(true, DialogKind::Open, 10.0, 10.0 + UNAVAILABLE_SECS),
        Outcome::Cancelled
    );
    assert_eq!(
        closed_without_path(true, DialogKind::Open, 10.0, 12.0),
        Outcome::Cancelled
    );
}

#[test]
fn fila_4_vale_para_los_tres_tipos() {
    for kind in KINDS {
        assert_eq!(
            closed_without_path(true, kind, 0.0, 0.2),
            Outcome::Unavailable,
            "{kind:?}"
        );
        assert_eq!(
            closed_without_path(true, kind, 0.0, 3.0),
            Outcome::Cancelled,
            "{kind:?}"
        );
    }
}

#[test]
fn fila_4_no_se_recuerda_el_fallo() {
    let (mut dialogs, handle) = dialogs(true, None);
    assert!(begin_open(&mut dialogs, 0.0));
    handle.close();
    assert_eq!(dialogs.poll(0.1).unwrap().outcome, Outcome::Unavailable);

    // Un diálogo posterior que el usuario cierra despacio es una cancelación.
    assert!(begin_open(&mut dialogs, 5.0));
    handle.close();
    assert_eq!(dialogs.poll(8.0).unwrap().outcome, Outcome::Cancelled);
}

// ---------------------------------------------------------------------------------------
// Fila 7: open_gate.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_7_sin_cambios_abre_sin_descartar_ni_armar() {
    let mut confirm = Confirm::new();
    assert_eq!(open_gate(false, &mut confirm, 0.0), Some(false));
    assert_eq!(
        confirm.request(Request::OpenDialog, 1.0),
        Confirmation::Armed,
        "sin cambios no se armó nada"
    );
}

#[test]
fn fila_7_con_cambios_la_segunda_pulsacion_dentro_de_5_s_abre_con_descarte() {
    let mut confirm = Confirm::new();
    assert_eq!(open_gate(true, &mut confirm, 0.0), None);
    assert_eq!(open_gate(true, &mut confirm, 4.9), Some(true));
    // Al confirmar se vació: la siguiente vuelve a armar.
    assert_eq!(open_gate(true, &mut confirm, 5.0), None);
}

#[test]
fn fila_7_con_cambios_separadas_5_s_se_vuelve_a_armar() {
    let mut confirm = Confirm::new();
    assert_eq!(open_gate(true, &mut confirm, 0.0), None);
    assert_eq!(open_gate(true, &mut confirm, 5.0), None);
}

#[test]
fn fila_7_otra_peticion_entre_medias_rearma() {
    let mut confirm = Confirm::new();
    assert_eq!(open_gate(true, &mut confirm, 0.0), None);
    assert_eq!(confirm.request(Request::Close, 1.0), Confirmation::Armed);
    assert_eq!(open_gate(true, &mut confirm, 2.0), None);
}

#[test]
fn fila_7_un_fichero_soltado_entre_medias_rearma() {
    let mut confirm = Confirm::new();
    assert_eq!(open_gate(true, &mut confirm, 0.0), None);
    assert_eq!(
        confirm.request(Request::Open(PathBuf::from("a.png")), 1.0),
        Confirmation::Armed
    );
    assert_eq!(open_gate(true, &mut confirm, 2.0), None);
}

#[test]
fn fila_7_la_peticion_que_arma_es_open_dialog() {
    let mut confirm = Confirm::new();
    assert_eq!(open_gate(true, &mut confirm, 0.0), None);
    assert_eq!(
        confirm.request(Request::OpenDialog, 1.0),
        Confirmation::Confirmed,
        "open_gate armó Request::OpenDialog"
    );
}

// ---------------------------------------------------------------------------------------
// Fila 9 (dialogs.rs): fallback_request, unavailable_notice y Cancelled rápido.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_9_fallback_request_de_los_tres_tipos() {
    assert_eq!(fallback_request(DialogKind::Open), None);
    assert_eq!(
        fallback_request(DialogKind::Save),
        Some(Request::SaveFallback { as_copy: false })
    );
    assert_eq!(
        fallback_request(DialogKind::SaveAs),
        Some(Request::SaveFallback { as_copy: true })
    );
}

const UNAVAILABLE_PREFIX: &str = "no hay diálogo de archivos: falta xdg-desktop-portal o zenity; ";

#[test]
fn fila_9_unavailable_notice_de_los_tres_tipos() {
    assert_eq!(
        unavailable_notice(DialogKind::Save),
        format!("{UNAVAILABLE_PREFIX}pulsa Ctrl+S otra vez en 5 s para guardar en sin-titulo.png")
    );
    assert_eq!(
        unavailable_notice(DialogKind::SaveAs),
        format!("{UNAVAILABLE_PREFIX}pulsa Ctrl+Mayús+S otra vez en 5 s para guardar una copia")
    );
    assert_eq!(
        unavailable_notice(DialogKind::Open),
        format!("{UNAVAILABLE_PREFIX}arrastra un PNG sobre la ventana")
    );
}

#[test]
fn fila_9_un_fallo_rapido_de_guardar_arma_y_la_segunda_pulsacion_confirma() {
    let (mut dialogs, handle) = dialogs(true, None);
    let mut confirm = Confirm::new();
    assert!(dialogs.begin(DialogKind::Save, false, None, false, 10.0, None));
    handle.close();
    let finished = dialogs.poll(10.1).expect("resuelto");
    assert_eq!(finished.outcome, Outcome::Unavailable);

    // Lo que hace la app con `Unavailable`: armar la petición de respaldo, ya con el instante.
    let fallback = fallback_request(finished.kind).expect("Save tiene respaldo");
    assert_eq!(confirm.request(fallback.clone(), 10.1), Confirmation::Armed);

    assert!(confirm.confirm_armed(&fallback, 12.0));
    assert!(
        !confirm.confirm_armed(&fallback, 12.5),
        "la confirmación vació la pendiente"
    );
}

#[test]
fn fila_9_cancelled_no_arma_nada() {
    // Cancelación lenta del usuario: sin respaldo armado, la «segunda pulsación» no existe.
    let (mut dialogs, handle) = dialogs(true, None);
    let mut confirm = Confirm::new();
    assert!(dialogs.begin(DialogKind::Save, false, None, false, 0.0, None));
    handle.close();
    let finished = dialogs.poll(4.0).expect("resuelto");
    assert_eq!(finished.outcome, Outcome::Cancelled);

    for as_copy in [false, true] {
        assert!(!confirm.confirm_armed(&Request::SaveFallback { as_copy }, 4.1));
    }
}

#[test]
fn fila_9_abrir_no_tiene_segunda_pulsacion() {
    let mut confirm = Confirm::new();
    assert_eq!(fallback_request(DialogKind::Open), None);
    for as_copy in [false, true] {
        assert!(!confirm.confirm_armed(&Request::SaveFallback { as_copy }, 1.0));
    }
}

// ---------------------------------------------------------------------------------------
// Fila 11: diálogo pendiente, abandono y resolución.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_11_otro_begin_con_un_dialogo_pendiente_es_falso_sin_start() {
    let (mut dialogs, handle) = dialogs(true, None);
    assert!(begin_open(&mut dialogs, 0.0));
    for kind in [DialogKind::Save, DialogKind::SaveAs, DialogKind::Open] {
        assert!(
            !dialogs.begin(kind, true, Some(Path::new("/a/b.png")), true, 0.5, None),
            "{kind:?}"
        );
    }
    assert_eq!(handle.starts(), 1);
    assert_eq!(
        handle.last().map(|request| request.kind),
        Some(DialogKind::Open),
        "el selector sigue con el primer diálogo"
    );
    assert!(dialogs.pending());
}

#[test]
fn fila_11_abandon_cierra_el_estado_y_ignora_el_resultado_tardio() {
    let (mut dialogs, handle) = dialogs(true, None);
    assert!(begin_open(&mut dialogs, 0.0));

    dialogs.abandon();

    assert!(!dialogs.pending());
    assert_eq!(handle.abandons(), 1, "abandon se llamó una vez");
    assert_eq!(dialogs.repaint_after(), None);

    handle.choose("/a/casa.png");
    assert_eq!(dialogs.poll(1.0), None, "el resultado se ignora");
    assert_eq!(dialogs.poll(2.0), None);
    assert_eq!(handle.abandons(), 1);
}

#[test]
fn fila_11_abandon_tambien_ignora_un_cierre_tardio_sin_ruta() {
    let (mut dialogs, handle) = dialogs(true, None);
    assert!(begin_open(&mut dialogs, 0.0));
    dialogs.abandon();
    handle.close();
    assert_eq!(dialogs.poll(0.1), None);
}

#[test]
fn fila_11_tras_abandonar_se_puede_abrir_otro() {
    let (mut dialogs, handle) = dialogs(true, None);
    assert!(begin_open(&mut dialogs, 0.0));
    dialogs.abandon();
    assert!(begin_open(&mut dialogs, 1.0));
    assert_eq!(handle.starts(), 2);
    assert!(dialogs.pending());
}

#[test]
fn fila_11_pendiente_hasta_que_poll_da_finished() {
    let (mut dialogs, handle) = dialogs(true, None);
    assert!(begin_open(&mut dialogs, 0.0));
    handle.choose("/a/casa.png");

    // El selector ya resolvió, pero mientras `poll` no lo recoja el diálogo sigue pendiente
    // (la app sigue ocupada).
    assert!(dialogs.pending());

    assert!(dialogs.poll(0.5).is_some());
    assert!(!dialogs.pending());
}

// ---------------------------------------------------------------------------------------
// remember_dir (fila 1, 3 y 7 del comportamiento): el directorio del fichero.
// ---------------------------------------------------------------------------------------

#[test]
fn remember_dir_guarda_el_directorio_del_fichero_y_el_ultimo_gana() {
    let (mut dialogs, handle) = dialogs(true, Some("/c"));
    dialogs.remember_dir(Path::new("/x/y/uno.png"));
    dialogs.remember_dir(Path::new("/z/dos.png"));

    assert!(begin_open(&mut dialogs, 0.0));
    assert_eq!(
        handle.last(),
        Some(request(DialogKind::Open, Some("/z"), None))
    );
}

#[test]
fn remember_dir_no_pisa_el_directorio_del_fichero_actual() {
    let (mut dialogs, handle) = dialogs(true, Some("/c"));
    dialogs.remember_dir(Path::new("/x/uno.png"));

    assert!(dialogs.begin(
        DialogKind::Open,
        false,
        Some(Path::new("/a/b/casa.png")),
        false,
        0.0,
        None
    ));
    assert_eq!(
        handle.last(),
        Some(request(DialogKind::Open, Some("/a/b"), None))
    );
}

// ---------------------------------------------------------------------------------------
// Fila 12: textos nuevos de dialogs.rs y de confirm.rs.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_12_textos_solo_con_caracteres_de_la_interfaz() {
    for kind in KINDS {
        assert_in_charset(&unavailable_notice(kind));
    }
    assert_in_charset(&open_dialog_warning());
    assert_in_charset(IGNORED_FILE_WHILE_DIALOG);
}

#[test]
fn fila_12_los_avisos_de_guardar_dicen_portal_zenity_y_el_atajo() {
    let save = unavailable_notice(DialogKind::Save);
    let save_as = unavailable_notice(DialogKind::SaveAs);
    for text in [&save, &save_as] {
        assert!(text.contains("xdg-desktop-portal"), "{text}");
        assert!(text.contains("zenity"), "{text}");
        assert!(text.contains("otra vez en 5 s"), "{text}");
    }
    assert!(save.contains("Ctrl+S"), "{save}");
    assert!(!save.contains("Mayús"), "{save}");
    assert!(save.contains("sin-titulo.png"), "{save}");
    assert!(save_as.contains("Ctrl+Mayús+S"), "{save_as}");
}

#[test]
fn fila_12_el_aviso_de_abrir_dice_portal_zenity_y_arrastrar() {
    let open = unavailable_notice(DialogKind::Open);
    assert!(open.contains("xdg-desktop-portal"), "{open}");
    assert!(open.contains("zenity"), "{open}");
    assert!(open.contains("arrastra"), "{open}");
}

#[test]
fn fila_12_el_aviso_de_abrir_con_cambios_dice_ctrl_o_y_ctrl_s() {
    let text = open_dialog_warning();
    assert!(text.contains("Ctrl+O"), "{text}");
    assert!(text.contains("Ctrl+S"), "{text}");
    assert!(text.contains("otra vez en 5 s"), "{text}");
    assert_eq!(
        text,
        "hay cambios sin guardar: pulsa Ctrl+O otra vez en 5 s para descartarlos, o guarda con Ctrl+S"
    );
}

// ---------------------------------------------------------------------------------------
// Unidad 18, fila 6 (e): `ignored_file_notice(file_dialog, modal)`
// (specs/hito-2-dialogos-modales.md, decisión 2). Función pura: con un diálogo de archivo o un
// diálogo modal abierto (o los dos) avisa `IGNORED_FILE_WHILE_DIALOG`; con ninguno de los dos
// (queda un trazo en curso) avisa «se ignora el fichero: hay un trazo en curso». Todo en
// `INTERFACE_CHARSET`.
//
// Decisión donde la spec no fija un detalle: el texto del aviso del trazo es el que tiene hoy
// `main.rs` al soltar un fichero con un gesto en curso, sin cambios.
// ---------------------------------------------------------------------------------------

const IGNORED_FILE_WHILE_STROKE: &str = "se ignora el fichero: hay un trazo en curso";

#[test]
fn fila_6e_con_un_dialogo_de_archivo_o_modal_avisa_del_dialogo() {
    for (file_dialog, modal) in [(true, false), (false, true), (true, true)] {
        assert_eq!(
            ignored_file_notice(file_dialog, modal),
            IGNORED_FILE_WHILE_DIALOG,
            "archivo {file_dialog}, modal {modal}"
        );
    }
}

#[test]
fn fila_6e_sin_dialogo_de_archivo_ni_modal_avisa_del_trazo() {
    assert_eq!(ignored_file_notice(false, false), IGNORED_FILE_WHILE_STROKE);
}

#[test]
fn fila_6e_los_textos_son_los_de_la_spec_distintos_entre_si_y_del_juego_de_caracteres() {
    assert_eq!(
        IGNORED_FILE_WHILE_DIALOG,
        "se ignora el fichero: hay un diálogo abierto"
    );
    assert_ne!(
        ignored_file_notice(true, false),
        ignored_file_notice(false, false)
    );
    for file_dialog in [false, true] {
        for modal in [false, true] {
            let aviso: &'static str = ignored_file_notice(file_dialog, modal);
            assert!(!aviso.is_empty());
            assert_in_charset(aviso);
        }
    }
    assert_in_charset(IGNORED_FILE_WHILE_STROKE);
}

#[test]
fn fila_6e_el_aviso_solo_depende_de_si_hay_algun_dialogo() {
    for file_dialog in [false, true] {
        for modal in [false, true] {
            let esperado = if file_dialog || modal {
                IGNORED_FILE_WHILE_DIALOG
            } else {
                IGNORED_FILE_WHILE_STROKE
            };
            assert_eq!(
                ignored_file_notice(file_dialog, modal),
                esperado,
                "archivo {file_dialog}, modal {modal}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Unidad L1, fila 5 (specs/limpieza-1.md): `DialogKind::Export`. En ROJO hasta la lógica.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_05_export_se_titula_exportar_png_aplanado_con_cualquier_capas_y_ruta() {
    for layers in [0, 1, 2, 5] {
        for path in [None, Some("dibujo.png"), Some("dibujo.aseprite")] {
            assert_eq!(
                dialog_title(DialogKind::Export, layers, path.map(Path::new)),
                "Exportar PNG aplanado",
                "{layers} capas, {path:?}"
            );
        }
    }
}

#[test]
fn fila_05_save_save_as_y_open_conservan_sus_titulos() {
    assert_eq!(
        dialog_title(DialogKind::Open, 1, None),
        "Abrir imagen (PNG o Aseprite)"
    );
    for kind in [DialogKind::Save, DialogKind::SaveAs] {
        assert_eq!(dialog_title(kind, 1, None), "Guardar PNG");
        assert_eq!(
            dialog_title(kind, 1, Some(Path::new("a.png"))),
            "Guardar PNG"
        );
        assert_eq!(
            dialog_title(kind, 1, Some(Path::new("a.aseprite"))),
            "Guardar .aseprite"
        );
        assert_eq!(
            dialog_title(kind, 2, None),
            "Guardar como .aseprite (con capas) o PNG (aplanado)"
        );
    }
}

#[test]
fn fila_05_el_nombre_propuesto_al_exportar_es_nombre_aplanado_png() {
    assert_eq!(
        export_name(Some(Path::new("/a/b/dibujo.aseprite"))),
        PathBuf::from("/a/b/dibujo-aplanado.png")
    );
    assert_eq!(
        export_name(Some(Path::new("dibujo.png"))),
        PathBuf::from("dibujo-aplanado.png")
    );
    assert_eq!(export_name(None), PathBuf::from("sin-titulo-aplanado.png"));
}

#[test]
fn fila_05_sin_dialogo_de_archivos_exportar_avisa_y_no_arma_segunda_pulsacion() {
    assert_eq!(
        unavailable_notice(DialogKind::Export),
        "no hay diálogo de archivos: no se puede exportar"
    );
    assert_eq!(fallback_request(DialogKind::Export), None);
}
