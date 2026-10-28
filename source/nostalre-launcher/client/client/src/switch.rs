//! Native Horizon entry point and embedded offline-server lifecycle.
use std::ffi::{CString, c_char, c_int};
use std::path::Path;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[repr(C)]
#[derive(Clone, Copy)]
struct OfflineProfile {
    account_id: u32, session_id1: u32, session_id2: u32, group_id: u32,
    sex: u8, character_slots: u8, reserved: [u8; 2],
    email: [c_char; 40], birthdate: [c_char; 11], name: [c_char; 24],
}
static OFFLINE_PROFILE: OnceLock<OfflineProfile> = OnceLock::new();
pub(super) fn local_session(packetver: u32) -> nostalre_network::session::Session {
    let profile = OFFLINE_PROFILE.get().expect("Offline profile must be ready before the client starts");
    let mut session = nostalre_network::session::Session::new(packetver);
    session.store_login(profile.account_id, profile.session_id1 as i32, profile.session_id2, profile.sex);
    session.char_server_addr = Some("127.0.0.1:6121".into());
    session.world_name = "Ragnarok offline".into();
    session
}
unsafe extern "C" {
    fn nostalre_offline_initialize(path: *const c_char, profile: *mut OfflineProfile, error: *mut c_char, capacity: usize) -> c_int;
    fn nostalre_sqlite_init() -> c_int;
    fn nostalre_server_runtime_init() -> c_int;
    fn nostalre_sqlite_check_database(path: *const c_char, error: *mut c_char, capacity: usize) -> c_int;
    fn nostalre_sqlite_check_schema(path: *const c_char, error: *mut c_char, capacity: usize) -> c_int;
    fn nostalre_server_call(entry: unsafe extern "C" fn(c_int, *mut *mut c_char) -> c_int,
        argc: c_int, argv: *mut *mut c_char) -> c_int;
    fn nostalre_char_main(argc: c_int, argv: *mut *mut c_char) -> c_int;
    fn nostalre_map_main(argc: c_int, argv: *mut *mut c_char) -> c_int;
    fn nostalre_char_stop();
    fn nostalre_map_stop();
    fn nostalre_char_status() -> c_int;
    fn nostalre_map_status() -> c_int;
    fn nostalre_show_message(message: *const c_char);
    fn nostalre_loading_begin();
    fn nostalre_loading_poll() -> bool;
    fn nostalre_loading_end();
    fn nostalre_full_memory() -> bool;
    fn nostalre_loading_status(status: *const c_char);
}

struct Server { name: &'static str, worker: JoinHandle<c_int>, stop: unsafe extern "C" fn(), status: unsafe extern "C" fn() -> c_int }
static SERVERS: Mutex<Vec<Server>> = Mutex::new(Vec::new());

fn spawn_server(name: &'static str, entry: unsafe extern "C" fn(c_int, *mut *mut c_char) -> c_int,
    stop: unsafe extern "C" fn(), status: unsafe extern "C" fn() -> c_int) -> std::io::Result<Server> {
    let worker = std::thread::Builder::new().name(name.into()).stack_size(4 * 1024 * 1024).spawn(move || {
        let program = CString::new(name).unwrap();
        let mut argv = [program.as_ptr().cast_mut(), std::ptr::null_mut()];
        // Each archive has its own localized globals. argv lives until its main returns.
        let result = unsafe { nostalre_server_call(entry, 1, argv.as_mut_ptr()) };
        tracing::info!("{name} exited with {result}");
        result
    })?;
    Ok(Server { name, worker, stop, status })
}

fn stop_servers() {
    let Ok(mut servers) = SERVERS.lock() else { return; };
    // World must finish saving while the character store is still available.
    while let Some(server) = servers.pop() {
        tracing::info!("Stopping {}", server.name);
        record_stage(&format!("[Shutdown] Stopping {}", server.name));
        unsafe { (server.stop)(); }
        if server.worker.thread().id() == std::thread::current().id() { continue; }
        let deadline = Instant::now() + Duration::from_secs(30);
        while !server.worker.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        if server.worker.is_finished() {
            let name = server.name;
            let result = server.worker.join().map_err(|_| "server worker panicked");
            record_stage(&format!("[Shutdown] {name} joined: {result:?}"));
        }
        else {
            tracing::error!("{} did not stop within 30 seconds", server.name);
            record_stage(&format!("[Shutdown] {} exceeded 30-second stop timeout", server.name));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

pub(super) fn record_stage(stage: &str) {
    let root = std::env::var("BEVY_ASSET_ROOT").unwrap_or_else(|_| "sdmc:/switch/ragswitch-pre".into());
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(Path::new(&root).join("startup.txt")) {
        let _ = writeln!(file, "{stage}");
        let _ = file.sync_all();
    }
    tracing::info!("[Startup] {stage}");
}

fn loading_status(text: &str) {
    if let Ok(text) = CString::new(text) { unsafe { nostalre_loading_status(text.as_ptr()); } }
}

#[path = "switch_install.rs"]
mod install;
use install::{InstallProgress, install_tree};

#[path = "switch_population.rs"]
mod population_install;

#[path = "switch_offline_profile.rs"]
mod offline_profile;

fn start_servers() -> Result<(), String> {
    if unsafe { nostalre_server_runtime_init() } != 0 {
        return Err("Falha ao registrar a protecao de erros dos servidores. Consulte log.txt.".into());
    }
    record_stage("Protecao de erros C++ verificada");
    let sqlite = unsafe { nostalre_sqlite_init() };
    if sqlite != 0 { return Err(format!("Falha ao abrir o banco de personagens (SQLite {sqlite}).")); }
    record_stage("Verificando banco de personagens");
    loading_status("Verificando banco de personagens...");
    let root = std::env::var("BEVY_ASSET_ROOT").unwrap_or_else(|_| "sdmc:/switch/ragswitch-pre".into());
    let database = CString::new(format!("{root}/ragnarok.db")).map_err(|_| "Caminho do banco invalido")?;
    let mut error = [0 as c_char; 256];
    if unsafe { nostalre_sqlite_check_database(database.as_ptr(), error.as_mut_ptr(), error.len()) } != 0 {
        let detail = unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) }.to_string_lossy();
        return Err(format!("Falha ao abrir ragnarok.db:\n{detail}\nConsulte log.txt. Seu banco foi preservado."));
    }
    record_stage("Banco verificado: gravacao e multiplas conexoes");
    if unsafe { nostalre_sqlite_check_schema(database.as_ptr(), error.as_mut_ptr(), error.len()) } != 0 {
        let detail = unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) }.to_string_lossy();
        return Err(format!("Falha ao verificar estrutura de ragnarok.db:\n{detail}\nConsulte log.txt."));
    }
    record_stage("Estrutura de contas e personagens verificada");
    let mut profile = std::mem::MaybeUninit::<OfflineProfile>::uninit();
    if unsafe { nostalre_offline_initialize(database.as_ptr(), profile.as_mut_ptr(), error.as_mut_ptr(), error.len()) } != 0 {
        let detail = unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) }.to_string_lossy();
        return Err(format!("Falha ao abrir perfil local:\n{detail}"));
    }
    OFFLINE_PROFILE.set(unsafe { profile.assume_init() }).map_err(|_| "Perfil local ja inicializado")?;
    record_stage("Perfil local restaurado; login-server removido");
    let mut servers = SERVERS.lock().map_err(|_| "Estado dos servidores indisponivel")?;
    for (name, entry, stop, status) in [
        ("char-server", nostalre_char_main as unsafe extern "C" fn(_, _) -> _, nostalre_char_stop as unsafe extern "C" fn(), nostalre_char_status as unsafe extern "C" fn() -> _),
        ("map-server", nostalre_map_main as unsafe extern "C" fn(_, _) -> _, nostalre_map_stop as unsafe extern "C" fn(), nostalre_map_status as unsafe extern "C" fn() -> _),
    ] {
        record_stage(&format!("Iniciando {name}"));
        loading_status(&format!("Iniciando {name}..."));
        servers.push(spawn_server(name, entry, stop, status).map_err(|e| format!("Falha ao iniciar {name}: {e}"))?);
    }
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        if !unsafe { nostalre_loading_poll() } { return Err("Inicializacao cancelada.".into()); }
        if let Some(server) = servers.iter().find(|s| s.worker.is_finished()) {
            return Err(format!("{} encerrou durante a inicializacao. Consulte log.txt.", server.name));
        }
        // rAthena's Core::RUNNING is 5. All startup data has been loaded by then.
        if servers.iter().all(|s| unsafe { (s.status)() } == 5) { return Ok(()); }
        if Instant::now() >= deadline { return Err("Os servidores nao iniciaram em cinco minutos. Consulte log.txt.".into()); }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn message(text: &str) {
    tracing::error!("{text}");
    let text = CString::new(text.replace('\0', " ")).unwrap();
    unsafe { nostalre_show_message(text.as_ptr()); }
}

pub fn main() {
    record_stage(&format!("--- teste {}: entrada no main ---", env!("NOSTALRE_SWITCH_VERSION")));
    let event_loop = match winit::event_loop::EventLoop::new() {
        Ok(value) => value,
        Err(error) => { message(&format!("Falha ao inicializar o Switch: {error}")); return; }
    };
    record_stage("Servicos Horizon inicializados");
    crate::init_logging(std::io::stdout);
    if !unsafe { nostalre_full_memory() } {
        message("O jogo precisa da memoria do modo aplicativo.\nAbra o hbmenu segurando R ao iniciar um jogo instalado\ne execute o nostalRE por esse menu.");
        return;
    }
    let root = std::env::var("BEVY_ASSET_ROOT").unwrap_or_else(|_| "sdmc:/switch/ragswitch-pre".into());
    if let Err(error) = std::env::set_current_dir(&root) {
        message(&format!("Falha ao abrir {root}: {error}")); return;
    }
    // Set before starting any server/client worker. The runtime's logging worker
    // has already read its configuration and does not access these variables.
    unsafe { std::env::set_var("RA_KEEP_CWD", "1"); std::env::set_var("RA_SQLITE_DIR", &root); }
    unsafe { nostalre_loading_begin(); }
    record_stage("Instalando dados locais do RomFS");
    if Path::new("romfs:/server").is_dir() {
        let mut progress = InstallProgress::new(!Path::new(".server-data-ready").is_file());
        let started = Instant::now();
        let mut last_display = Instant::now() - Duration::from_millis(150);
        let mut on_file = |files: usize, destination: &Path| -> std::io::Result<()> {
            if last_display.elapsed() >= Duration::from_millis(150) {
                if !unsafe { nostalre_loading_poll() } { return Err(std::io::ErrorKind::Interrupted.into()); }
                loading_status(&format!("Arquivos locais verificados: {files}\n{}", destination.display()));
                last_display = Instant::now();
            }
            if files % 100 == 0 { tracing::info!("[Startup] Arquivos locais: {files}"); }
            Ok(())
        };
        if let Err(error) = install_tree(Path::new("romfs:/server"), Path::new("."), &mut progress, &mut on_file) {
            record_stage(&format!("Falha na instalacao: {error}"));
            message(&format!("Falha ao instalar dados do servidor: {error}")); return;
        }
        let _ = std::fs::write(".server-data-ready", "0.1.1\n");
        record_stage(&format!("Instalacao concluida: {} arquivos", progress.files));
        tracing::info!("[Perf] install_ms={} files={} copied={} target_directory_lists={} target_metadata_queries={}",
            started.elapsed().as_millis(), progress.files, progress.copied, progress.directory_lists, progress.target_metadata_queries);
    }
    match population_install::ensure_population_import(Path::new(".")) {
        Ok(true) => record_stage("Perfil de populacao 50 habilitado na atualizacao"),
        Ok(false) => {},
        Err(error) => {
            message(&format!("Falha ao preparar populacao: {error}")); return;
        }
    }
    match offline_profile::sync_offline_profile(Path::new("romfs:/server"), Path::new(".")) {
        Ok(changed) => tracing::info!("[Startup] Perfil offline atualizado: {changed} arquivos"),
        Err(error) => {
            message(&format!("Falha ao preparar classes e taxas locais: {error}")); return;
        }
    }
    if !Path::new("config.json").exists() {
        if let Err(error) = std::fs::write("config.json", include_str!("../../switch/config.json")) {
            message(&format!("Falha ao gravar config.json: {error}")); return;
        }
    }
    let config = crate::Config::load_or_default("config.json");
    if let Some(missing) = config.grf_paths.iter().find(|path| !Path::new(path).is_file()) {
        message(&format!("Arquivo de jogo ausente:\n{missing}\n\nCopie os arquivos do kRO 2025-07-16 e official_data.grf\npara sdmc:/switch/ragswitch-pre/game-data/.")); return;
    }
    if !winit::platform::horizon::register_release_hook(stop_servers) {
        message("Falha ao registrar encerramento dos servidores."); return;
    }
    if let Err(error) = start_servers() {
        record_stage(&format!("Falha nos servidores: {error}"));
        stop_servers(); message(&error); return;
    }
    record_stage("Servidores prontos; iniciando video e interface");
    loading_status("Servidores prontos. Preparando video e interface...");
    unsafe { nostalre_loading_end(); }
    crate::run(event_loop);
    stop_servers();
}
