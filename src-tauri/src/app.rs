use crate::{
    fixtures,
    library::Library,
    midi,
    model::{ImportResult, LibraryEntry, Song, SongSettings},
    runtime::{AppStatus, Command, Preferences, Runtime},
};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};

pub struct AppState {
    pub library: Arc<Mutex<Library>>,
    pub runtime: Runtime,
    pub preferences: Arc<Mutex<Preferences>>,
    pub root: PathBuf,
    pub pending: Mutex<Vec<String>>,
    pub importing: Arc<AtomicBool>,
    pub operations: Arc<Mutex<()>>,
    smoke: Option<SmokeConfig>,
    started: Instant,
    launch_to_ui_us: AtomicU64,
}
#[derive(Clone, Serialize)]
struct SmokeConfig {
    phase: String,
    paths: Vec<String>,
    expected_count: usize,
    device_id: Option<String>,
}
#[derive(Serialize)]
pub struct ImportOutcome {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<ImportResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}
#[derive(Serialize)]
pub struct OpenSong {
    entry: LibraryEntry,
    song: Song,
    settings: SongSettings,
}
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| format!("Background operation failed: {e}"))?
}
fn lock_error<T>(e: std::sync::PoisonError<T>) -> String {
    format!("Keyloom state unavailable: {e}")
}
pub fn persist(state: &AppState) -> Result<(), String> {
    let _guard = state.operations.lock().map_err(lock_error)?;
    persist_session(&state.library, &state.runtime)
}
fn persist_session(library: &Arc<Mutex<Library>>, runtime: &Runtime) -> Result<(), String> {
    if let Some((id, settings)) = runtime.checkpoint()? {
        library
            .lock()
            .map_err(lock_error)?
            .save_settings(&id, &settings)?;
    }
    Ok(())
}
fn write_preferences(root: &Path, value: &Preferences) -> Result<(), String> {
    use std::io::Write;
    let temporary = root.join("preferences.tmp");
    let destination = root.join("preferences.json");
    let mut file = std::fs::File::create(&temporary).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    // Windows rename does not replace: keep one backup for interrupted replacement recovery.
    let backup = root.join("preferences.previous.json");
    if backup.exists() {
        std::fs::remove_file(&backup).map_err(|e| e.to_string())?;
    }
    if destination.exists() {
        std::fs::rename(&destination, &backup).map_err(|e| e.to_string())?;
    }
    std::fs::rename(temporary, destination).map_err(|e| e.to_string())
}
pub fn read_preferences(root: &Path) -> Preferences {
    [
        root.join("preferences.json"),
        root.join("preferences.previous.json"),
    ]
    .iter()
    .find_map(|p| {
        std::fs::read(p)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
    })
    .unwrap_or_default()
}
#[tauri::command(rename_all = "snake_case")]
async fn library_list(
    query: String,
    favorites: bool,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<LibraryEntry>, String> {
    let lib = state.library.clone();
    blocking(move || lib.lock().map_err(lock_error)?.list(&query, favorites)).await
}
struct ImportGuard(Arc<AtomicBool>);
impl Drop for ImportGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
fn import_batch(
    paths: Vec<String>,
    lib: Arc<Mutex<Library>>,
    busy: Arc<AtomicBool>,
) -> Result<Vec<ImportOutcome>, String> {
    if paths.len() > 64 {
        return Err("Import up to 64 files per batch".into());
    }
    busy.compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .map_err(|_| "An import batch is already running; wait for it to finish")?;
    let _guard = ImportGuard(busy);
    let mut output = Vec::new();
    for path in paths {
        let result = lib
            .lock()
            .map_err(lock_error)?
            .import_path(Path::new(&path));
        output.push(match result {
            Ok(result) => ImportOutcome {
                path,
                result: Some(result),
                error: None,
            },
            Err(error) => ImportOutcome {
                path,
                result: None,
                error: Some(error),
            },
        });
    }
    Ok(output)
}
#[tauri::command(rename_all = "snake_case")]
async fn import_files(
    paths: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ImportOutcome>, String> {
    let lib = state.library.clone();
    let busy = state.importing.clone();
    blocking(move || import_batch(paths, lib, busy)).await
}
#[tauri::command]
async fn import_dialog(state: tauri::State<'_, AppState>) -> Result<Vec<ImportOutcome>, String> {
    let lib = state.library.clone();
    let busy = state.importing.clone();
    blocking(move || {
        let paths = rfd::FileDialog::new()
            .set_title("Import MIDI into Keyloom")
            .add_filter("Standard MIDI Files", &["mid", "midi"])
            .pick_files()
            .unwrap_or_default()
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        import_batch(paths, lib, busy)
    })
    .await
}
#[tauri::command(rename_all = "snake_case")]
async fn open_song(id: String, state: tauri::State<'_, AppState>) -> Result<OpenSong, String> {
    let lib = state.library.clone();
    let runtime = state.runtime.clone();
    let operations = state.operations.clone();
    blocking(move || {
        let _guard = operations.lock().map_err(lock_error)?;
        runtime.send(Command::Pause)?;
        let old = runtime.checkpoint()?;
        let mut library = lib.lock().map_err(lock_error)?;
        if let Some((old_id, settings)) = old {
            library.save_settings(&old_id, &settings)?;
        }
        let song = library.load(&id)?;
        let settings = library.settings(&id)?;
        library.mark_played(&id)?;
        let entry = library.entry(&id)?;
        runtime.send(Command::Load {
            id,
            song: song.clone(),
            settings: settings.clone(),
        })?;
        Ok(OpenSong {
            entry,
            song,
            settings,
        })
    })
    .await
}
#[tauri::command(rename_all = "snake_case")]
async fn save_metadata(
    entry: LibraryEntry,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let lib = state.library.clone();
    blocking(move || lib.lock().map_err(lock_error)?.update_metadata(&entry)).await
}
#[tauri::command(rename_all = "snake_case")]
async fn save_settings(
    id: String,
    mut settings: SongSettings,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let lib = state.library.clone();
    let runtime = state.runtime.clone();
    let operations = state.operations.clone();
    blocking(move || {
        let _guard = operations.lock().map_err(lock_error)?;
        if let Some((current_id, current)) = runtime.checkpoint()? {
            if current_id == id {
                settings.position_ms = current.position_ms;
            }
        }
        lib.lock()
            .map_err(lock_error)?
            .save_settings(&id, &settings)
    })
    .await
}
#[tauri::command(rename_all = "snake_case")]
async fn remove_song(
    id: String,
    confirmed: bool,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if !confirmed {
        return Err(
            "Confirm removal of this managed copy; the external original is never deleted".into(),
        );
    }
    let lib = state.library.clone();
    let runtime = state.runtime.clone();
    let operations = state.operations.clone();
    blocking(move || {
        let _guard = operations.lock().map_err(lock_error)?;
        if runtime
            .checkpoint()?
            .as_ref()
            .is_some_and(|(key, _)| *key == id)
        {
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            runtime.send(Command::Unload(tx))?;
            rx.recv_timeout(Duration::from_secs(5))
                .map_err(|_| "Timed out unloading the current song")?;
        }
        lib.lock().map_err(lock_error)?.remove(&id)
    })
    .await
}
#[tauri::command(rename_all = "snake_case")]
async fn export_song(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let lib = state.library.clone();
    blocking(move || {
        let title = lib.lock().map_err(lock_error)?.entry(&id)?.title;
        let filename: String = title
            .chars()
            .filter(|c| !"<>:\"/\\|?*".contains(*c) && !c.is_control())
            .collect();
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Export original MIDI")
            .add_filter("MIDI", &["mid"])
            .set_file_name(format!("{filename}.mid"))
            .save_file()
        {
            lib.lock().map_err(lock_error)?.export(&id, &path)?;
        }
        Ok(())
    })
    .await
}
#[tauri::command]
async fn devices() -> Result<Vec<midi::Device>, String> {
    blocking(midi::devices).await
}
#[tauri::command(rename_all = "snake_case")]
async fn connect_device(
    id: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let runtime = state.runtime.clone();
    let prefs = state.preferences.clone();
    let root = state.root.clone();
    blocking(move || {
        // Preferences uses this same mutex while enqueueing its update. Keep
        // connection selection and its persisted ID in that ordering domain;
        // otherwise an old-ID Preferences command can follow a new Connect.
        // The transport worker never acquires this mutex, so waiting for its
        // reply here cannot create a lock cycle.
        let mut p = prefs.lock().map_err(lock_error)?;
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        runtime.send(Command::Connect(id.clone(), tx))?;
        let result = rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "MIDI connection timed out")?;
        p.device_id = id;
        write_preferences(&root, &p)?;
        result
    })
    .await
}
#[tauri::command(rename_all = "snake_case")]
async fn control(
    action: String,
    position_ms: Option<f64>,
    settings: Option<SongSettings>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let command = match action.as_str() {
        "play" => Command::Play,
        "pause" => Command::Pause,
        "stop" => Command::Stop,
        "panic" => Command::Panic,
        "seek" => Command::Seek(
            position_ms
                .filter(|n| n.is_finite() && *n >= 0.0)
                .ok_or("Seek needs a valid position")?,
        ),
        "configure" => Command::Configure(settings.ok_or("Configure needs song settings")?),
        _ => return Err("Unknown transport action".into()),
    };
    state.runtime.send(command)?;
    if ["pause", "stop", "seek", "panic"].contains(&action.as_str()) {
        let lib = state.library.clone();
        let runtime = state.runtime.clone();
        let operations = state.operations.clone();
        blocking(move || {
            let _guard = operations.lock().map_err(lock_error)?;
            persist_session(&lib, &runtime)
        })
        .await?;
    }
    Ok(())
}
#[tauri::command(rename_all = "snake_case")]
fn input_note(
    pitch: u8,
    on: bool,
    velocity: u8,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    if pitch > 127 {
        return Err("MIDI note must be 0–127".into());
    }
    if !state.preferences.lock().map_err(lock_error)?.simulated {
        return Err("Enable explicitly simulated input first".into());
    }
    state.runtime.send(Command::Input {
        pitch,
        on,
        velocity,
    })
}
#[tauri::command]
async fn preferences(
    value: Option<Preferences>,
    state: tauri::State<'_, AppState>,
) -> Result<Preferences, String> {
    let prefs = state.preferences.clone();
    let runtime = state.runtime.clone();
    let root = state.root.clone();
    blocking(move || {
        let mut current = prefs.lock().map_err(lock_error)?;
        if let Some(mut value) = value {
            if value.low >= value.high || value.high > 127 {
                return Err("Keyboard range needs low < high within 0–127".into());
            }
            if !["yamaha", "software"].contains(&value.monitor.as_str()) {
                return Err("Choose Yamaha or software monitoring".into());
            }
            value.device_id = current.device_id.clone();
            write_preferences(&root, &value)?;
            runtime.send(Command::Preferences(value.clone()))?;
            *current = value;
        }
        Ok(current.clone())
    })
    .await
}
#[tauri::command]
fn status(state: tauri::State<AppState>) -> Result<AppStatus, String> {
    state
        .runtime
        .status
        .lock()
        .map(|s| s.clone())
        .map_err(lock_error)
}
#[tauri::command]
async fn demo_import(state: tauri::State<'_, AppState>) -> Result<ImportResult, String> {
    let root = state.root.clone();
    let lib = state.library.clone();
    blocking(move || {
        let path = root.join("Keyloom - First threads.mid");
        std::fs::write(&path, fixtures::demo_midi()).map_err(|e| e.to_string())?;
        let result = lib.lock().map_err(lock_error)?.import_path(&path);
        std::fs::remove_file(path).ok();
        result
    })
    .await
}
#[tauri::command]
fn take_pending_files(state: tauri::State<AppState>) -> Vec<String> {
    state
        .pending
        .lock()
        .map(|mut p| std::mem::take(&mut *p))
        .unwrap_or_default()
}
#[tauri::command]
fn smoke_config(state: tauri::State<AppState>) -> Option<SmokeConfig> {
    state
        .launch_to_ui_us
        .compare_exchange(
            0,
            state.started.elapsed().as_micros() as u64,
            Ordering::Relaxed,
            Ordering::Relaxed,
        )
        .ok();
    state.smoke.clone()
}
#[tauri::command(rename_all = "snake_case")]
fn smoke_open_files(
    paths: Vec<String>,
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    if state.smoke.is_none() {
        return Err("Native smoke tests were not enabled on the command line".into());
    }
    if paths.len() > 64 {
        return Err("Test batch exceeds 64 files".into());
    }
    state.pending.lock().map_err(lock_error)?.extend(paths);
    app.emit("files-opened", ()).map_err(|e| e.to_string())
}
#[tauri::command]
async fn smoke_report(
    mut report: serde_json::Value,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let smoke = state
        .smoke
        .clone()
        .ok_or("Native smoke tests are not enabled")?;
    let root = state.root.clone();
    let lib = state.library.clone();
    let runtime = state.runtime.clone();
    let operations = state.operations.clone();
    report["launch_to_first_ui_ipc_ms"] =
        serde_json::json!(state.launch_to_ui_us.load(Ordering::Relaxed) as f64 / 1000.0);
    let passed = report["passed"].as_bool().unwrap_or(false);
    blocking(move || {
        let _guard = operations.lock().map_err(lock_error)?;
        persist_session(&lib, &runtime)?;
        report["native_status"] =
            serde_json::to_value(runtime.status.lock().map_err(lock_error)?.clone())
                .map_err(|e| e.to_string())?;
        report["devices"] =
            serde_json::to_value(midi::devices().unwrap_or_default()).map_err(|e| e.to_string())?;
        let path = root.join(format!("smoke-{}.json", smoke.phase));
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    })
    .await?;
    app.exit(if passed { 0 } else { 1 });
    Ok(())
}
pub fn file_args(args: impl Iterator<Item = String>, cwd: &Path) -> Vec<String> {
    args.filter_map(|a| {
        let path = Path::new(&a);
        let ext = path.extension()?.to_str()?;
        if ext.eq_ignore_ascii_case("mid") || ext.eq_ignore_ascii_case("midi") {
            Some(
                if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    cwd.join(path)
                }
                .to_string_lossy()
                .into_owned(),
            )
        } else {
            None
        }
    })
    .take(64)
    .collect()
}
pub fn run() {
    let started = Instant::now();
    let builder=tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app,args,cwd|{
        let paths=file_args(args.into_iter().skip(1),Path::new(&cwd));if let Some(state)=app.try_state::<AppState>(){if let Ok(mut p)=state.pending.lock(){let capacity=64_usize.saturating_sub(p.len());p.extend(paths.into_iter().take(capacity));}app.emit("files-opened",()).ok();}
        if let Some(window)=app.get_webview_window("main"){window.unminimize().ok();window.set_focus().ok();}
    })).setup(move |app|{
        let args:Vec<String>=std::env::args().collect();let smoke_root=args.iter().position(|a|a=="--smoke-test").and_then(|i|args.get(i+1)).map(PathBuf::from);
        let root=smoke_root.clone().unwrap_or(app.path().app_local_data_dir()?);std::fs::create_dir_all(&root)?;let library=Library::open(root.clone()).map_err(std::io::Error::other)?;
        let smoke=if smoke_root.is_some(){let directory=root.join("inputs with spaces 日本語");std::fs::create_dir_all(&directory)?;let first=directory.join("First threads.mid");let second=directory.join("Tempo and pedal.midi");let corrupt=directory.join("Broken.mid");if !args.iter().any(|a|a=="--smoke-restart"){std::fs::write(&first,fixtures::demo_midi())?;std::fs::write(&second,fixtures::tempo_sustain())?;std::fs::write(&corrupt,b"invalid MIDI")?;}let first=args.iter().position(|a|a=="--smoke-midi").and_then(|i|args.get(i+1)).map(PathBuf::from).unwrap_or(first);Some(SmokeConfig{phase:if args.iter().any(|a|a=="--smoke-restart"){"restart"}else{"exercise"}.into(),paths:[first,second,corrupt].iter().map(|p|p.to_string_lossy().into_owned()).collect(),expected_count:2,device_id:args.iter().position(|a|a=="--smoke-device").and_then(|i|args.get(i+1)).cloned()})}else{None};
        let preferences=read_preferences(&root);let runtime=Runtime::start(preferences.clone());let pending=if smoke.is_some(){Vec::new()}else{file_args(std::env::args().skip(1),&std::env::current_dir()?)};
        app.manage(AppState{library:Arc::new(Mutex::new(library)),runtime,preferences:Arc::new(Mutex::new(preferences)),root,pending:Mutex::new(pending),importing:Arc::new(AtomicBool::new(false)),operations:Arc::new(Mutex::new(())),smoke,started,launch_to_ui_us:AtomicU64::new(0)});
        let handle=app.handle().clone();std::thread::Builder::new().name("keyloom-save".into()).spawn(move||loop{std::thread::sleep(Duration::from_secs(5));if let Some(state)=handle.try_state::<AppState>(){if persist(&state).is_err(){/* Kept out of timing path; exit retries persistence. */}}else{break;}})?;
        Ok(())
    }).invoke_handler(tauri::generate_handler![library_list,import_files,import_dialog,open_song,save_metadata,save_settings,remove_song,export_song,devices,connect_device,control,input_note,preferences,status,demo_import,take_pending_files,smoke_config,smoke_open_files,smoke_report]);
    let application = builder
        .build(tauri::generate_context!())
        .expect("Keyloom could not initialize its local application data or WebView2 window");
    application.run(|app, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            if let Some(state) = app.try_state::<AppState>() {
                persist(&state).ok();
                state.runtime.send(Command::Shutdown).ok();
            }
        }
    });
}
