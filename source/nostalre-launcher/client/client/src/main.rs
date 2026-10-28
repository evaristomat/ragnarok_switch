#[cfg(target_os = "horizon")]
mod switch;

#[cfg(all(test, not(target_os = "horizon")))]
#[path = "switch_offline_profile.rs"]
mod switch_offline_profile_tests;

mod body_effect;
mod command;
mod config;
mod effect_loader;
mod emblem_web;
mod events;
mod fps_log;
mod game_state;
mod game_updates;
#[cfg(all(test, target_os = "linux"))]
mod gameplay_validation;
mod hover;
mod input;
mod input_action;
mod overlay;
mod scene;
mod script;
mod sound;
mod sprite;
mod ui;

use config::Config;
use game_state::{CursorInput, CursorPending, GameState, cursor_type_from_hover};
use input::InputState;
use models::enums::skill_enums::SkillEnum;
use nostalre_audio::SoundManager;
use nostalre_formats::act::SpriteAnimationState;
use nostalre_formats::grf::GrfArchive;
use nostalre_game::app_state::AppState;
use nostalre_game::cursor::RenderEntry;
use nostalre_game::data_table::accessory_table::AccessoryTable;
use nostalre_game::data_table::card_illustration_table::CardIllustrationTable;
use nostalre_game::data_table::card_name_table::CardNameTable;
use nostalre_game::data_table::item_description_table::ItemDescriptionTable;
use nostalre_game::data_table::item_name_table::ItemNameTable;
use nostalre_game::data_table::item_resource_table::ItemResourceTable;
use nostalre_game::data_table::item_slot_count_table::ItemSlotCountTable;
use nostalre_game::data_table::name_table::NameTable;
use nostalre_game::data_table::skill_description_table::SkillDescriptionTable;
use nostalre_game::data_table::skill_name_table::SkillNameTable;
use nostalre_game::data_table::skill_use_level_table::SkillUseLevelTable;
use nostalre_game::effect::EffectQueue;
use nostalre_game::event::GameEvent;
use nostalre_game::map_loader;
use nostalre_game::sound::SoundQueue;
use nostalre_game::sprite_path::{HiddenRender, hidden_render};
use nostalre_network::{NetworkCommand, build_reqname_packet, network_loop};
use nostalre_renderer::effect::EffectHolder;
use nostalre_renderer::{
    EffectSpriteCache, GridSelectorRenderer, Renderer, StrEffectCache, block_on,
};
use nostalre_ui::context::UiContext;
use nostalre_ui::frame::{UiFrame, UiOutput};
use nostalre_ui::state::StateCache;
use nostalre_ui_component::account::char_create_window::CharCreateWindow;
use nostalre_ui_component::account::char_select_window::CharSelectWindow;
use nostalre_ui_component::account::login_server_list_window::{
    LoginServerEntry, LoginServerListWindow,
};
use nostalre_ui_component::account::login_window::{
    LoginFocus, LoginWindow, PASSWORD_ID, USERNAME_ID,
};
use nostalre_ui_component::account::server_list_window::ServerListWindow;
use nostalre_ui_component::game::confirm_dialog::ConfirmDialog;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

struct GameChannel {
    cmd_tx: Option<mpsc::UnboundedSender<NetworkCommand>>,
    event_rx: Option<mpsc::UnboundedReceiver<GameEvent>>,
    /// The emblem worker (`emblem_web`); its results come back among the network's events.
    emblem_jobs: Option<std::sync::mpsc::Sender<emblem_web::EmblemJob>>,
}

impl GameChannel {
    fn new() -> Self {
        Self {
            cmd_tx: None,
            event_rx: None,
            emblem_jobs: None,
        }
    }

    /// Whether the worker took `job` (false without one: use the map server).
    fn queue_emblem_job(&self, job: emblem_web::EmblemJob) -> bool {
        self.emblem_jobs
            .as_ref()
            .is_some_and(|jobs| jobs.send(job).is_ok())
    }

    fn send_packet(&self, packet: Vec<u8>) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(NetworkCommand::SendPacket(packet));
        }
    }

    fn send_cmd(&self, cmd: NetworkCommand) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(cmd);
        }
    }

    fn drain_events(&mut self) -> Vec<GameEvent> {
        self.event_rx
            .as_mut()
            .map(|rx| std::iter::from_fn(|| rx.try_recv().ok()).collect())
            .unwrap_or_default()
    }
}

struct App {
    config: Config,
    touch_mouse: input::touch::TouchMouse,
    gamepad: input::gamepad::Gamepad,
    /// Between suspended and resumed: no surface to draw on.
    suspended: bool,
    saved_window_positions: HashMap<u32, [f32; 2]>,
    window_state_restored: bool,
    renderer: Option<Renderer>,
    effect_sprites: EffectSpriteCache,
    str_effects: StrEffectCache,
    effect_loader: effect_loader::EffectLoader,
    effect_holder: EffectHolder,
    effect_queue: EffectQueue,
    map_fog: Option<nostalre_formats::fog_table::FogEntry>,
    grf: Option<std::sync::Arc<GrfArchive>>,
    /// The install's Equipment Attributes scripts in their Lua state, loaded on the window's first opening; none
    /// without them
    equipment_properties: Option<nostalre_game::equipment_properties::EquipmentProperties>,
    equipment_properties_tried: bool,
    gr2_loader: Option<sprite::gr2_loader::Gr2Loader>,
    input: InputState,
    ui_context: Option<UiContext>,
    ui_state_cache: StateCache,
    login_window: LoginWindow,
    account_dialog: ConfirmDialog,
    login_server_list_window: Option<LoginServerListWindow>,
    selected_login_server: usize,
    /// `rathena_table::PACKETVER`, handed to the packet crate's builders
    active_packetver: u32,
    server_list_window: Option<ServerListWindow>,
    char_select_window: Option<CharSelectWindow>,
    char_create_window: Option<CharCreateWindow>,
    /// Ragnarok Solo's hub chose the character: login, server and character select run by themselves behind
    /// `account::draw_entering` until the map loads, or until one of them fails and its window is needed after all
    solo_entering: bool,
    /// A character select the char server answered with no map server up (just started with the app): asked again
    /// when due, `(due, slot, tries left)`; no due time while the asked-again one waits for its answer
    select_retry: Option<(Option<Instant>, u8, u8)>,
    account_background: Option<String>,
    account_anims: HashMap<u32, SpriteAnimationState>,
    char_create_built_appearance: Option<(u16, u16)>,
    /// The look the stylist's preview sprite was last built with
    stylist_built_look: Option<nostalre_game::stylist::StyleLook>,
    roulette_act: Option<nostalre_formats::act::ActFile>,
    roulette_textures: Option<nostalre_renderer::SpriteTextures>,
    channel: GameChannel,
    game: GameState,
    windows: ui::Windows,
    sound: SoundManager,
    sound_queue: SoundQueue,
    bgm_table: HashMap<String, String>,
    sfx_rng: u32,
    start_time: Instant,
    last_frame_instant: Instant,
    fps_smoothed: f32,
    fps_log: fps_log::FpsLog,
    /// NOSTALRE_SCRIPT: steps run on their own once in game (desktop test runs).
    script: Option<script::Script>,
    next_frame: Instant,
    /// GameEvents produced by raw keyboard handling (skill-bar / emotion hotkeys),
    /// drained into `handle_ui_events` on the next redraw.
    pending_events: Vec<GameEvent>,
    /// When `config.json` is to be written (`save_config_soon`)
    config_save_due: Option<Instant>,
    window_focused: bool,
    profiler: nostalre_profiling::Profiler,
    /// Window must be dropped last
    window: Option<Arc<Window>>,
}

impl App {
    fn new(config: Config) -> Self {
        let saved_window_positions = config
            .window_state
            .iter()
            .map(|(&id, entry)| (id, entry.position))
            .collect();
        let mut game = GameState::new();
        let mut windows = ui::Windows::new();
        windows.sound_options.set_values(config.sound());
        windows
            .escape
            .set_excluded(&config.custom.window.exclude_close_via_esc);
        windows.item_info_window.wrap_title = config.custom.window.wrap_item_info_title;
        windows.hud.layout = config.hud_layout;
        // Switch always uses its physical controls, including after a screen tap.
        windows.hud.touch_controls = !cfg!(target_os = "horizon")
            && (cfg!(target_os = "android")
                || std::env::var("NOSTALRE_TOUCH_CONTROLS").is_ok_and(|v| v == "1"));
        windows.menu.set_key_labels(&config.keybindings);
        game.prefs.self_config.refuse_party_invite = config.refuse_party_invite;
        let mut effect_queue = EffectQueue::new();
        effect_queue.set_effects_enabled(config.show_skill_effects);
        let mut sound =
            SoundManager::new(config.effective_bgm_volume(), config.effective_sfx_volume());
        sound.set_stereo(config.custom.sound.stereo);
        Self {
            touch_mouse: Default::default(),
            gamepad: Default::default(),
            suspended: false,
            config,
            saved_window_positions,
            window_state_restored: false,
            window: None,
            renderer: None,
            effect_sprites: EffectSpriteCache::new(),
            str_effects: StrEffectCache::new(),
            effect_loader: effect_loader::EffectLoader::new(),
            effect_holder: EffectHolder::new(),
            effect_queue,
            map_fog: None,
            grf: None,
            equipment_properties: None,
            equipment_properties_tried: false,
            gr2_loader: None,
            input: InputState::new(),
            ui_context: None,
            ui_state_cache: StateCache::new(),
            login_window: LoginWindow::new(),
            account_dialog: ConfirmDialog::new(),
            login_server_list_window: None,
            selected_login_server: 0,
            active_packetver: nostalre_network::rathena_table::PACKETVER,
            server_list_window: None,
            char_select_window: None,
            char_create_window: None,
            solo_entering: false,
            select_retry: None,
            account_background: None,
            account_anims: HashMap::new(),
            char_create_built_appearance: None,
            stylist_built_look: None,
            roulette_act: None,
            roulette_textures: None,
            channel: GameChannel::new(),
            game,
            windows,
            sound,
            sound_queue: SoundQueue::new(),
            bgm_table: HashMap::new(),
            sfx_rng: 0x1234_5678,
            start_time: Instant::now(),
            last_frame_instant: Instant::now(),
            fps_smoothed: 0.0,
            fps_log: fps_log::FpsLog::from_env(),
            script: script::Script::from_env(),
            next_frame: Instant::now(),
            pending_events: Vec::new(),
            config_save_due: None,
            window_focused: true,
            profiler: nostalre_profiling::Profiler::default(),
        }
    }

    /// Show the connection-server selection screen when more than one server is
    /// configured; otherwise go straight to the login screen with the sole server.
    #[cfg(target_os = "horizon")]
    fn setup_initial_screen(&mut self) {
        self.solo_entering = self
            .config
            .solo
            .as_ref()
            .is_some_and(|solo| solo.char_id.is_some());
        self.open_local_profile();
    }

    #[cfg(target_os = "horizon")]
    fn open_local_profile(&mut self) {
        self.connect_local_characters(switch::local_session(self.active_packetver));
    }

    #[cfg(any(target_os = "horizon", test))]
    fn connect_local_characters(&mut self, mut session: nostalre_network::session::Session) {
        session.state = nostalre_network::session::SessionState::ConnectingToChar;
        self.channel.send_cmd(NetworkCommand::Disconnect);
        self.channel.send_cmd(NetworkCommand::Connect {
            addr: session.char_server_addr.clone().unwrap(),
            expect_aid: true,
        });
        self.channel
            .send_packet(nostalre_network::build_char_enter_packet(&session));
        self.channel.send_cmd(NetworkCommand::SetKeepalive(
            nostalre_network::KeepaliveMode::CharServer {
                account_id: session.account_id,
            },
        ));
        self.game.session.login_session = Some(session);
        self.game.session.app_state = AppState::Login;
        self.account_dialog
            .show_message("Abrindo personagens locais...");
    }

    #[cfg(not(target_os = "horizon"))]
    fn setup_initial_screen(&mut self) {
        self.select_login_server(0);
        if self.config.login_servers.len() <= 1 {
            self.game.session.app_state = AppState::Login;
            if let Some(solo) = &self.config.solo {
                self.solo_entering = solo.char_id.is_some();
                self.pending_events.push(GameEvent::RequestLogin {
                    username: solo.username.clone(),
                    password: solo.password.clone(),
                });
            }
            return;
        }
        let entries = self
            .config
            .login_servers
            .iter()
            .map(|s| LoginServerEntry {
                name: s.name.clone(),
                detail: format!("{}:{}", s.host, s.port),
            })
            .collect();
        let mut win = LoginServerListWindow::new(entries);
        if let (Some(grf), Some(renderer)) = (&self.grf, &mut self.renderer) {
            events::preload_window(&mut win, renderer, grf);
        }
        self.login_server_list_window = Some(win);
        self.game.session.app_state = AppState::LoginServerSelect;
    }

    /// Adopt the connection server at `index`
    fn select_login_server(&mut self, index: usize) {
        if index < self.config.login_servers.len() {
            self.selected_login_server = index;
        }
    }

    fn load_map(&mut self, map_name: &str) {
        let grf = match &self.grf {
            Some(g) => g,
            None => return,
        };

        let map_data = match map_loader::load_map_data(grf, map_name) {
            Some(d) => d,
            None => {
                self.windows.map_missing_window.show(map_name.to_string());
                self.game.session.map_signs.clear();
                return;
            }
        };

        self.windows.map_missing_window.hide();
        self.game.session.map_coords = map_data.coordinates;
        let ground_sampler = map_data
            .gat
            .as_ref()
            .zip(map_data.coordinates.as_ref())
            .map(|(gat, coords)| nostalre_game::map_cloud::ground_sampler(gat, coords));
        self.game.session.gat = map_data.gat;
        self.game.session.actor_lightmap = map_data.actor_lightmap;
        let was_indoor = self.game.session.camera_locked;
        self.game.session.camera_locked = map_data.indoor;
        if let Some(renderer) = &mut self.renderer {
            let [r, g, b] =
                nostalre_game::data_table::map_cloud_table::map_background_color(map_name)
                    .unwrap_or([0.0, 0.0, 0.0]);
            renderer.clear_color = nostalre_renderer::wgpu::Color {
                r: r as f64,
                g: g as f64,
                b: b as f64,
                a: 1.0,
            };
            let leaving = renderer.camera.saved_view();
            if was_indoor {
                self.game.session.saved_camera_indoor = leaving;
            } else {
                self.game.session.saved_camera_outdoor = leaving;
            }
            let restore = if map_data.indoor {
                self.game.session.saved_camera_indoor
            } else {
                self.game.session.saved_camera_outdoor
            };
            renderer.camera.on_map_enter(map_data.indoor, restore);
        }
        self.game
            .schedulers
            .ambient_effects
            .clear(&mut self.effect_queue);
        self.game.schedulers.ambient_effects =
            nostalre_game::effects::AmbientEffectScheduler::from_rsw(&map_data.rsw, &map_data.gnd);
        self.game.schedulers.map_cloud.clear(&mut self.effect_queue);
        self.game.schedulers.map_cloud.set_map(map_name);
        self.effect_holder.set_ground_sampler(ground_sampler);
        self.game.schedulers.ambient_sounds =
            nostalre_game::sound::ambient::AmbientSoundScheduler::from_rsw(
                &map_data.rsw,
                &map_data.gnd,
            );
        self.game.schedulers.repeat_sounds.clear();

        self.game.schedulers.day_night.on_map_loaded(
            map_data.rsw.light.diffuse.unwrap_or([1.0, 1.0, 1.0]),
            map_data.rsw.light.ambient.unwrap_or([0.3, 0.3, 0.3]),
        );

        let rsw_key = map_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(map_name)
            .rsplit_once('.')
            .map(|(stem, _)| stem)
            .unwrap_or(map_name)
            .to_ascii_lowercase();
        let bgm_track = self.bgm_table.get(&format!("{rsw_key}.rsw")).cloned();

        let (mut spr_paths, mut str_names) =
            nostalre_game::effects::ambient_effect_assets(&map_data.rsw);

        self.map_fog = map_data.fog;

        if let Some(renderer) = &mut self.renderer {
            let fog = if self.config.fog { map_data.fog } else { None };
            renderer.load_map(&map_data.gnd, &map_data.rsw, grf, fog);

            // the map's own effects now; every other effect loads a little each frame (`effect_loader`)
            spr_paths.sort();
            spr_paths.dedup();
            for path in spr_paths {
                self.effect_sprites.load(
                    path,
                    grf,
                    &renderer.device.device,
                    &renderer.device.queue,
                    &renderer.texture_cache.bind_group_layout,
                );
            }

            str_names.sort();
            str_names.dedup();
            for name in &str_names {
                self.str_effects.load(
                    name,
                    self.effect_loader.other_names(name),
                    grf,
                    &mut renderer.texture_cache,
                    &renderer.device.device,
                    &renderer.device.queue,
                );
            }
        }

        if let Some(renderer) = &mut self.renderer
            && let Some(gat) = &self.game.session.gat
        {
            let mut grid = GridSelectorRenderer::new(
                &renderer.device.device,
                &renderer.device.queue,
                renderer.device.scene_format,
                &renderer.global_uniforms,
                &mut renderer.texture_cache,
                grf,
            );
            grid.build_grid_mesh(
                &renderer.device.device,
                gat,
                map_data.gnd.width,
                map_data.gnd.height,
                map_data.gnd.zoom,
            );
            renderer.grid_selector = Some(grid);
        }

        if let Some(track) = bgm_track {
            self.play_bgm_track(&track);
        }
        self.place_map_signs(map_name);
        self.show_map_title(map_name);
    }

    fn position_camera_at(&mut self, cell_x: f32, cell_y: f32) {
        self.aim_camera_at(cell_x, cell_y, false);
    }

    /// Jump the camera onto a cell without the usual glide: on map entry and
    /// warps the old target is meaningless.
    fn warp_camera_to(&mut self, cell_x: f32, cell_y: f32) {
        self.aim_camera_at(cell_x, cell_y, true);
    }

    fn aim_camera_at(&mut self, cell_x: f32, cell_y: f32, snap: bool) {
        if let (Some(coords), Some(renderer)) = (&self.game.session.map_coords, &mut self.renderer)
        {
            input::position_camera_at(
                &mut renderer.camera,
                self.game.session.gat.as_ref(),
                coords,
                cell_x,
                cell_y,
            );
            if snap {
                renderer.camera.snap_target();
            }
        }
    }

    fn hovered_cell(&self) -> Option<(i32, i32)> {
        let (renderer, coords, screen_w, screen_h) = self.screen_dims()?;
        input::hovered_cell(
            self.input.mouse_position,
            &renderer.camera,
            screen_w,
            screen_h,
            coords,
            self.game.session.gat.as_ref(),
        )
    }

    pub(crate) fn is_gm_account(&self) -> bool {
        let account_id = self
            .game
            .session
            .login_session
            .as_ref()
            .map_or(0, |s| s.account_id);
        self.config.is_gm_account(account_id)
    }

    fn spawn_network(&mut self) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        self.channel.cmd_tx = Some(cmd_tx);
        self.channel.event_rx = Some(event_rx);
        self.channel.emblem_jobs = emblem_web::spawn(event_tx.clone());

        let packetver = self.active_packetver;
        let debug_delay_ms = self.config.debug_network_delay_ms;
        let start_time = self.start_time;
        // Spawn on dedicated thread with single-threaded runtime
        // because network_loop uses non-Send packet types
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create network runtime");
            rt.block_on(network_loop(
                cmd_rx,
                event_tx,
                packetver,
                debug_delay_ms,
                start_time,
            ));
        });
    }

    fn start_account_interface(&mut self, setup: impl FnOnce(&mut Self)) {
        // Account setup may send immediately; the receiver must already exist.
        self.spawn_network();
        setup(self);
    }

    fn build_ui(&mut self, elapsed: f32) -> (UiOutput, Vec<GameEvent>) {
        nostalre_profiling::profile_function!();
        let now_ms = self.start_time.elapsed().as_millis() as u64;
        if let Some(ui_ctx) = &mut self.ui_context {
            ui_ctx.now_ms = now_ms;
        }
        let account_bg = self.account_background.clone();
        if self.game.session.app_state == AppState::InGame {
            self.refresh_equipment_attributes();
            self.fill_menu_settings();
        }
        if self.solo_entering && self.game.session.app_state != AppState::InGame {
            return match (&mut self.ui_context, &self.renderer) {
                (Some(ui_ctx), Some(renderer)) => {
                    let mut ui = UiFrame::new(
                        ui_ctx,
                        &renderer.font_atlas,
                        &mut self.ui_state_cache,
                        elapsed,
                        false,
                        None,
                        &self.saved_window_positions,
                    );
                    let line = match &self.game.session.selected_character {
                        _ if self.select_retry.is_some() => {
                            "Waiting for the map server…".to_string()
                        }
                        Some(character) => format!("Entering as {}…", character.name),
                        None => "Entering…".to_string(),
                    };
                    nostalre_ui_component::account::draw_entering(
                        &mut ui,
                        account_bg.clone().map(Into::into),
                        &line,
                    );
                    (ui.finish(), Vec::new())
                }
                _ => (UiOutput::default(), Vec::new()),
            };
        }
        match self.game.session.app_state {
            AppState::LoginServerSelect => {
                if let (Some(ui_ctx), Some(renderer), Some(server_win)) = (
                    &mut self.ui_context,
                    &self.renderer,
                    &mut self.login_server_list_window,
                ) {
                    let mut ui = UiFrame::new(
                        ui_ctx,
                        &renderer.font_atlas,
                        &mut self.ui_state_cache,
                        elapsed,
                        server_win.has_grf_textures,
                        None,
                        &self.saved_window_positions,
                    );
                    nostalre_ui_component::account::draw_background(
                        &mut ui,
                        account_bg.clone().map(Into::into),
                    );
                    if self.account_dialog.state.is_some() {
                        ui.block_keyboard();
                    }
                    let events = server_win.build(&mut ui);
                    self.account_dialog.build(&mut ui);
                    (ui.finish(), events)
                } else {
                    (UiOutput::default(), Vec::new())
                }
            }
            AppState::Login => {
                if let (Some(ui_ctx), Some(renderer)) = (&mut self.ui_context, &self.renderer) {
                    let initial_focus = match self.login_window.focus {
                        LoginFocus::Username => Some(USERNAME_ID),
                        LoginFocus::Password => Some(PASSWORD_ID),
                    };
                    let mut ui = UiFrame::new(
                        ui_ctx,
                        &renderer.font_atlas,
                        &mut self.ui_state_cache,
                        elapsed,
                        self.login_window.has_grf_textures,
                        initial_focus,
                        &self.saved_window_positions,
                    );
                    nostalre_ui_component::account::draw_background(
                        &mut ui,
                        account_bg.clone().map(Into::into),
                    );
                    if self.account_dialog.state.is_some() {
                        ui.block_keyboard();
                    }
                    let events = self.login_window.build(&mut ui);
                    self.account_dialog.build(&mut ui);
                    (ui.finish(), events)
                } else {
                    (UiOutput::default(), Vec::new())
                }
            }
            AppState::ServerSelect => {
                if let (Some(ui_ctx), Some(renderer), Some(server_win)) = (
                    &mut self.ui_context,
                    &self.renderer,
                    &mut self.server_list_window,
                ) {
                    let mut ui = UiFrame::new(
                        ui_ctx,
                        &renderer.font_atlas,
                        &mut self.ui_state_cache,
                        elapsed,
                        server_win.has_grf_textures,
                        None,
                        &self.saved_window_positions,
                    );
                    nostalre_ui_component::account::draw_background(
                        &mut ui,
                        account_bg.clone().map(Into::into),
                    );
                    if self.account_dialog.state.is_some() {
                        ui.block_keyboard();
                    }
                    let events = server_win.build(&mut ui);
                    self.account_dialog.build(&mut ui);
                    (ui.finish(), events)
                } else {
                    (UiOutput::default(), Vec::new())
                }
            }
            AppState::CharacterSelect => {
                if let (Some(ui_ctx), Some(renderer), Some(char_win)) = (
                    &mut self.ui_context,
                    &self.renderer,
                    &mut self.char_select_window,
                ) {
                    let mut ui = UiFrame::new(
                        ui_ctx,
                        &renderer.font_atlas,
                        &mut self.ui_state_cache,
                        elapsed,
                        char_win.has_grf_textures,
                        None,
                        &self.saved_window_positions,
                    );
                    nostalre_ui_component::account::draw_background(
                        &mut ui,
                        account_bg.clone().map(Into::into),
                    );
                    if self.account_dialog.state.is_some() {
                        ui.block_keyboard();
                    }
                    let events = char_win.build(&mut ui);
                    self.account_dialog.build(&mut ui);
                    (ui.finish(), events)
                } else {
                    (UiOutput::default(), Vec::new())
                }
            }
            AppState::CharacterCreate => {
                if let (Some(ui_ctx), Some(renderer), Some(create_win)) = (
                    &mut self.ui_context,
                    &self.renderer,
                    &mut self.char_create_window,
                ) {
                    let mut ui = UiFrame::new(
                        ui_ctx,
                        &renderer.font_atlas,
                        &mut self.ui_state_cache,
                        elapsed,
                        create_win.has_grf_textures,
                        None,
                        &self.saved_window_positions,
                    );
                    nostalre_ui_component::account::draw_background(
                        &mut ui,
                        account_bg.clone().map(Into::into),
                    );
                    if self.account_dialog.state.is_some() {
                        ui.block_keyboard();
                    }
                    let events = create_win.build(&mut ui);
                    self.account_dialog.build(&mut ui);
                    (ui.finish(), events)
                } else {
                    (UiOutput::default(), Vec::new())
                }
            }
            AppState::InGame => {
                let render_list = self.compute_render_list();
                if let (Some(ui_ctx), Some(renderer)) = (&mut self.ui_context, &self.renderer) {
                    let mut ui = UiFrame::new(
                        ui_ctx,
                        &renderer.font_atlas,
                        &mut self.ui_state_cache,
                        elapsed,
                        self.windows.system_menu.has_grf_textures,
                        None,
                        &self.saved_window_positions,
                    );
                    let events = crate::ui::build_in_game_ui(
                        &mut self.game,
                        &mut self.windows,
                        &mut ui,
                        &|name| renderer.texture_cache.texture_size(name),
                        &render_list,
                    );

                    let mut overlay_lines: Vec<String> = Vec::new();
                    if self.game.show_fps {
                        overlay_lines.push(format!("fps: {:.0}", self.fps_smoothed));
                    }
                    if self.game.show_ping {
                        let local_ms = self.start_time.elapsed().as_millis() as u32;
                        let st = &self.game.session.server_time;
                        let est = st.estimated_server_tick(local_ms);
                        let offset = est as i64 - local_ms as i64;
                        overlay_lines.push(format!(
                            "net sync: {}",
                            if st.is_synced() { "yes" } else { "no" }
                        ));
                        overlay_lines.push(format!(
                            "rtt: {} ms (avg {:.0})",
                            st.rtt(),
                            st.rtt_avg()
                        ));
                        overlay_lines.push(format!("server tick est: {est}"));
                        overlay_lines.push(format!("offset: {offset} ms"));
                    }
                    if !overlay_lines.is_empty() {
                        const MINIMAP_LEFT_MARGIN: f32 = 130.0;
                        const PADDING: f32 = 10.0;
                        let color = [1.0, 0.9, 0.25, 1.0];
                        let shadow = [0.0, 0.0, 0.0, 0.9];
                        let right_x =
                            (ui.ctx.screen_width - MINIMAP_LEFT_MARGIN - PADDING).max(0.0);
                        for (i, line) in overlay_lines.iter().enumerate() {
                            let y = 10.0 + i as f32 * 16.0;
                            let tw = ui.atlas.measure_text(line);
                            let x = right_x - tw;
                            ui.text(x + 1.0, y + 1.0, line, shadow);
                            ui.text(x, y, line, color);
                        }
                    }

                    (ui.finish(), events)
                } else {
                    (UiOutput::default(), Vec::new())
                }
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            // back on screen (Android): the native window is new, draw on a surface of it
            if self.suspended {
                self.suspended = false;
                if let Some(renderer) = &mut self.renderer {
                    renderer.recreate_surface(window.clone());
                }
                window.request_redraw();
            }
            return;
        }

        #[cfg(target_os = "horizon")]
        switch::record_stage("Criando janela do cliente");
        let attrs = WindowAttributes::default()
            .with_title("RagSWITCH Pre-Renewal")
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.screen_width,
                self.config.screen_height,
            ));

        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        if self.config.fullscreen {
            window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        }
        let os_scale = window.scale_factor() as f32;
        let dpi_scale = if self.config.dpi_scale > 0.0 {
            self.config.dpi_scale / 100.0
        } else {
            os_scale
        };
        #[cfg(target_os = "horizon")]
        switch::record_stage("Inicializando renderizador Vulkan e shaders");
        let mut renderer = block_on(Renderer::new(
            window.clone(),
            self.config.font_px_height(),
            dpi_scale,
        ));
        renderer.set_fog_scale(self.config.custom.fog_scale);
        #[cfg(target_os = "horizon")]
        switch::record_stage("Renderizador pronto; carregando recursos dos GRFs");
        renderer.set_world_filtering(self.config.custom.filtering.world, None);
        renderer.set_effect_filtering(self.config.custom.filtering.effects, None);
        self.str_effects
            .set_filtering(self.config.custom.filtering.effects);
        self.effect_sprites
            .set_filtering(self.config.custom.filtering.effects);
        nostalre_renderer::sprite::set_filtering(self.config.custom.filtering.sprites);
        renderer.set_sprite_upscale(
            self.config.custom.filtering.sprite_upscale && self.config.custom.filtering.sprites,
        );

        let physical_size = window.inner_size();
        self.window = Some(window);
        self.renderer = Some(renderer);
        let mut ui_ctx = UiContext::new(
            physical_size.width as f32 / dpi_scale,
            physical_size.height as f32 / dpi_scale,
        );
        ui_ctx.dpi_scale = dpi_scale;
        ui_ctx.lock_account_windows = true;
        self.ui_context = Some(ui_ctx);

        if !self.config.grf_paths.is_empty() {
            let data_dir = self.config.data_dir.as_deref().map(Path::new);
            match GrfArchive::open_layered(&self.config.grf_paths, data_dir) {
                Ok(mut grf) => {
                    #[cfg(any(target_os = "horizon", test))]
                    {
                        let skipped = grf.use_archived_map_geometry();
                        if skipped > 0 {
                            tracing::info!("Offline maps: {skipped} loose geometry overrides excluded; archive maps match the bundled server");
                        }
                    }
                    for (path, bytes) in nostalre_game::effect::own_textures::files() {
                        grf.set_override(&path, bytes.to_vec());
                    }
                    // The install's English Equipment Attributes tabs, which kRO's client reads before its archive
                    for (path, bytes) in nostalre_game::data_table::install_ui_textures(
                        self.config.install_dir.as_deref().map(Path::new),
                        "EquipmentPropertiesWnd",
                    ) {
                        grf.set_override(&path, bytes);
                    }
                    println!(
                        "GRF loaded: {} ({} files, {} overlay archive(s), data_dir: {})",
                        self.config.grf_paths[0],
                        grf.file_count(),
                        self.config.grf_paths.len() - 1,
                        self.config.data_dir.as_deref().unwrap_or("none"),
                    );

                    if let Some(renderer) = &mut self.renderer {
                        renderer.try_load_grf_font(&grf);
                        events::preload_window(&mut self.login_window, renderer, &grf);
                        events::preload_window(&mut self.account_dialog, renderer, &grf);
                        self.account_background = pick_account_background(
                            &self.config.account_backgrounds,
                        )
                        .and_then(|path| {
                            renderer
                                .preload_textures(&[path.as_str()], &grf)
                                .then_some(path)
                        });
                    }
                    self.login_window.keep_id = self.config.keep_login_id;
                    if self.config.keep_login_id {
                        self.login_window.username.text = self.config.saved_username.clone();
                        self.login_window.focus = LoginFocus::Password;
                    }

                    self.load_cursor_sprite(&grf);
                    self.load_emotion_sprite(&grf);
                    self.load_status_overlay_sprites(&grf);
                    self.game.data_table.damage_skins =
                        nostalre_game::data_table::load_damage_skins(
                            &grf,
                            self.config.install_dir.as_deref().map(Path::new),
                        );
                    if self.config.damage_skin as usize >= self.game.data_table.damage_skins.len() {
                        // kRO's client takes skin 0 for one the list no longer has (0xd620d0)
                        self.config.damage_skin = 0;
                    }
                    let direction = nostalre_game::damage_number::DamageDirection::from_saved(
                        self.config.damage_dir,
                    );
                    self.game.combat.damage_numbers.direction = direction;
                    self.windows
                        .equipment_window
                        .set_damage_indicator(self.config.damage_skin, direction);
                    if let Some(renderer) = &mut self.renderer {
                        let skin_icons: Vec<&str> = self
                            .game
                            .data_table
                            .damage_skins
                            .iter()
                            .map(|skin| skin.icon.as_str())
                            .collect();
                        renderer.preload_textures(&skin_icons, &grf);
                    }
                    self.load_damage_sprites(&grf);
                    self.game.data_table.accessory = Some(AccessoryTable::load_from_grf(&grf));
                    let item_info = nostalre_game::data_table::load_item_info(&grf);
                    self.game.data_table.styling_shop =
                        nostalre_game::data_table::load_styling_shop(&grf);
                    self.game.data_table.sign_boards =
                        nostalre_game::data_table::load_sign_boards(&grf);
                    nostalre_game::data_table::load_install_tables(
                        &mut self.game.data_table,
                        self.config.install_dir.as_deref().map(Path::new),
                    );
                    nostalre_game::sprite_path::load_hand_looks(&grf, &item_info);
                    self.game.data_table.name = Some(NameTable::load_from_grf(&grf));
                    let mut item_name = ItemNameTable::load(&grf);
                    item_name.fill_from_item_info(&item_info);
                    self.game.data_table.item_name = Some(item_name);
                    let mut item_resource = ItemResourceTable::load(&grf);
                    item_resource.fill_from_item_info(&item_info);
                    let accessory = self.game.data_table.accessory.as_ref();
                    let headgear_view = nostalre_game::data_table::read_view_table(
                        &grf,
                        nostalre_resources::table::HEADGEAR_VIEW,
                    );
                    item_resource.fill_headgear_icons(
                        &headgear_view,
                        |view| accessory?.get_suffix(view).map(str::to_string),
                        |resource| grf.file_exists(&nostalre_resources::ui::item::icon(resource)),
                    );
                    item_resource.fill_card_icons(
                        &grf.read_file(nostalre_resources::table::RATHENA_CARDS)
                            .map(|data| nostalre_formats::lua_table::parse_id_set_table(&data))
                            .unwrap_or_default(),
                    );
                    self.game.data_table.item_resource = Some(item_resource);
                    self.game.data_table.headgear_view = headgear_view;
                    self.game.data_table.item_slot_count = Some(ItemSlotCountTable::load(&grf));
                    self.game.data_table.card_name = Some(CardNameTable::load(&grf));
                    self.game.data_table.card_illustration =
                        Some(CardIllustrationTable::load(&grf));
                    self.game.data_table.item_description = Some(ItemDescriptionTable::load(&grf));
                    self.game.data_table.skill_name = Some(SkillNameTable::load(&grf));
                    self.game.data_table.skill_description =
                        Some(SkillDescriptionTable::load(&grf));
                    let mut skill_use_level = SkillUseLevelTable::load(&grf);
                    if self.config.custom.skill.al_teleport.separate_lvl {
                        skill_use_level.force_level_select(SkillEnum::AlTeleport);
                    }
                    self.game.data_table.skill_use_level = Some(skill_use_level);
                    self.game.data_table.quest_display = Some(
                        nostalre_game::data_table::quest_display_table::QuestDisplayTable::load(
                            &grf,
                        ),
                    );
                    self.game.data_table.msg_string = Some(
                        nostalre_game::data_table::msg_string_table::MsgStringTable::load(&grf),
                    );
                    self.game.data_table.map_position = Some(
                        nostalre_game::data_table::map_position_table::MapPositionTable::load(&grf),
                    );
                    self.game.data_table.map_name =
                        Some(nostalre_game::data_table::map_name_table::MapNameTable::load(&grf));
                    if let Ok(bytes) = grf.read_file(nostalre_resources::table::PET_TALK) {
                        self.game.data_table.pet_talk =
                            Some(nostalre_formats::pettalk::PetTalkTable::parse(&bytes));
                    }
                    if let Ok(bytes) = grf.read_file(nostalre_resources::table::MP3_NAME) {
                        let text = String::from_utf8_lossy(&bytes);
                        self.bgm_table =
                            nostalre_game::sound::bgm_table::parse_mp3_name_table(&text);
                    }
                    let grf = std::sync::Arc::new(grf);
                    self.gr2_loader = Some(sprite::gr2_loader::Gr2Loader::spawn(grf.clone()));
                    self.grf = Some(grf);
                }
                Err(e) => {
                    tracing::error!("Failed to open GRF {}: {e}", self.config.grf_paths[0]);
                }
            }
        }

        #[cfg(target_os = "horizon")]
        switch::record_stage("Recursos carregados; preparando login local");
        self.start_account_interface(Self::setup_initial_screen);
        #[cfg(target_os = "horizon")]
        switch::record_stage("Interface de conta pronta e rede iniciada");
        self.play_bgm_track("01.mp3");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let event = input::touch::back_as_escape(event);
        if let WindowEvent::KeyboardInput { event: key, .. } = &event
            && self.gamepad.key(key)
        {
            return;
        }
        if let WindowEvent::Touch(touch) = event {
            let mut on_control = false;
            if let Some(ctx) = &mut self.ui_context {
                // a finger after a controller: the on-screen controls and the hints come back
                ctx.pad_labels = false;
                let (x, y) = ctx.handle_touch(&touch);
                // a window drawn over a control takes the finger
                on_control = self.game.session.app_state == AppState::InGame
                    && self
                        .windows
                        .hud
                        .touch_zones()
                        .iter()
                        .any(|r| r.contains(x, y))
                    && !self.ui_state_cache.window_drawn_at(x, y);
            }
            for mouse in self.touch_mouse.translate(touch, on_control) {
                self.window_event(event_loop, id, mouse);
            }
            return;
        }
        if let Some(ui_ctx) = &mut self.ui_context {
            ui_ctx.handle_event(&event);
        }

        match event {
            WindowEvent::CloseRequested => self.handle_close_requested(event_loop),
            WindowEvent::Resized(size) => self.handle_resize(size),
            WindowEvent::Focused(focused) => self.handle_focus_changed(focused),
            WindowEvent::MouseInput { state, button, .. } => self.handle_mouse_input(state, button),
            WindowEvent::CursorMoved { position, .. } => self.handle_cursor_moved(position),
            WindowEvent::KeyboardInput { event, .. } => self.handle_keyboard_input(event),
            WindowEvent::ModifiersChanged(modifiers) => self.handle_modifiers_changed(modifiers),
            WindowEvent::RedrawRequested if self.suspended => {}
            WindowEvent::RedrawRequested => {
                #[cfg(target_os = "horizon")]
                let mut perf_stage = Instant::now();
                for mouse in self.touch_mouse.frame() {
                    self.window_event(event_loop, id, mouse);
                }
                self.gamepad_frame(event_loop, id);
                #[cfg(target_os = "horizon")]
                self.fps_log.stage(0, &mut perf_stage);
                self.profiler.new_frame();
                nostalre_profiling::profile_scope!("frame");
                let elapsed = self.start_time.elapsed().as_secs_f32();

                self.handle_game_events(event_loop);
                self.apply_script_input();

                #[cfg(target_os = "horizon")]
                if let Some(ctx) = &mut self.ui_context {
                    ctx.pad_labels = true;
                }

                #[cfg(target_os = "horizon")]
                self.fps_log.stage(1, &mut perf_stage);
                let (
                    UiOutput {
                        draw_calls: ui_draw_calls,
                        tooltip_draw_calls,
                        any_hovered: ui_any_hovered,
                        any_interactive_hovered: ui_any_interactive,
                    },
                    ui_events,
                ) = self.build_ui(elapsed);
                self.input.ui_hovered = ui_any_hovered;
                let menu_art = self.windows.menu.take_wanted_textures();
                if !menu_art.is_empty() {
                    self.preload_item_icons(menu_art);
                }
                self.apply_leftover_wheel();
                if let Some(dirty) = self.windows.hotkey_config_window.take_dirty_bindings() {
                    self.config.emotion_keys = dirty.emotion;
                    self.apply_key_bindings(dirty.interface);
                }
                // the screen opened this frame gets its values before the next
                self.fill_menu_settings();
                self.flush_config(Some(Instant::now()));
                self.retry_select_when_due(Instant::now());
                let mut queued = std::mem::take(&mut self.pending_events);
                queued.extend(ui_events);
                self.handle_ui_events(queued, event_loop);

                #[cfg(target_os = "horizon")]
                self.fps_log.stage(2, &mut perf_stage);
                if self.game.session.pending_disconnect_exit {
                    event_loop.exit();
                }
                let now = Instant::now();
                #[cfg(not(target_os = "horizon"))]
                self.fps_log.frame(now);
                self.run_script(now, event_loop);
                let raw_delta = now.duration_since(self.last_frame_instant).as_secs_f32();
                self.last_frame_instant = now;
                if raw_delta > 0.0 {
                    let instant_fps = 1.0 / raw_delta;
                    self.fps_smoothed = if self.fps_smoothed == 0.0 {
                        instant_fps
                    } else {
                        self.fps_smoothed * 0.9 + instant_fps * 0.1
                    };
                }
                let delta = raw_delta.min(0.1);
                self.run_game_updates(delta, elapsed);
                self.load_effects_in_background();
                self.drain_sound_queue(delta);

                #[cfg(target_os = "horizon")]
                self.fps_log.stage(3, &mut perf_stage);
                let hovered = self.update_grid_hover();
                let render_list = self.compute_render_list();
                let floor_item_render_list = self.compute_floor_item_render_list(elapsed);
                let mut cart_render_list = self.compute_cart_render_list();
                cart_render_list.extend(self.compute_falcon_render_list());
                // A stealthed actor the local player can't see is not hoverable or
                // attackable: drop it before hit-testing (self stays out of picking
                // regardless, so its shadow-only self view never enters here).
                let pick_render_list: Vec<RenderEntry> = render_list
                    .iter()
                    .filter(|entry| {
                        self.game.world.entities.get(entry.id).is_none_or(|e| {
                            hidden_render(
                                e.effect_state,
                                self.hidden_viewer_for(entry.id),
                                self.player_clairvoyant(),
                            ) != HiddenRender::Skip
                        })
                    })
                    .copied()
                    .collect();
                self.prune_companion_targets();
                let hover = self.resolve_hover(
                    hovered,
                    &pick_render_list,
                    &floor_item_render_list,
                    ui_any_hovered,
                    ui_any_interactive,
                );
                let cursor = cursor_type_from_hover(
                    &hover,
                    CursorInput {
                        in_game: self.game.session.app_state == AppState::InGame,
                        right_mouse_down: self.input.right_mouse_down,
                        ui_any_hovered,
                        ui_any_interactive_hovered: ui_any_interactive,
                        item_drag_active: nostalre_ui::frame::drag_active(&self.ui_state_cache),
                    },
                    CursorPending {
                        companion_target_armed: self
                            .game
                            .companions
                            .companion_attack_target
                            .iter()
                            .any(Option::is_some),
                        pending_companion_skill: self
                            .game
                            .pending_casts
                            .pending_companion_skill
                            .is_some(),
                        pending_companion_patrol: self
                            .game
                            .pending_casts
                            .pending_companion_patrol
                            .is_some(),
                        capture_targeting: self.game.companions.capture_targeting,
                        pending_skill: self.game.pending_casts.pending_skill_target.is_some(),
                        marriage_targeting: self.game.pending_casts.marriage_targeting,
                    },
                );
                self.game.assets.cursor_animation.set_cursor_type(cursor);
                self.game.hover = hover;

                let hovered_named_id = self
                    .game
                    .hover
                    .target_id()
                    .or(self.game.hover.hovered_player_id)
                    .or(self.game.hover.hovered_self_id);
                let hovered_floor_item_id = self.game.hover.hovered_floor_item_id;
                self.request_visible_npc_names(&render_list);
                if let Some(entity_id) = hovered_named_id
                    && let Some(entity) = self.game.world.entities.get_mut(entity_id)
                    && !entity.name_requested
                {
                    entity.name_requested = true;
                    self.channel
                        .send_packet(build_reqname_packet(entity_id, self.active_packetver));
                }

                #[cfg(target_os = "horizon")]
                self.fps_log.stage(4, &mut perf_stage);
                let cursor_clips =
                    self.build_cursor_sprite_clips(delta, &render_list, &floor_item_render_list);
                let lock_cursor_clips = self.build_lock_cursor_clips(delta, &render_list);

                let world_overlay_calls = self.build_world_overlays(
                    &render_list,
                    &floor_item_render_list,
                    hovered_named_id,
                    hovered_floor_item_id,
                );
                let skill_level_calls = self.build_skill_overlay();

                #[cfg(target_os = "horizon")]
                self.fps_log.stage(5, &mut perf_stage);
                self.compose_and_render(
                    &render_list,
                    &floor_item_render_list,
                    &cart_render_list,
                    elapsed,
                    delta,
                    cursor_clips,
                    lock_cursor_clips,
                    world_overlay_calls,
                    skill_level_calls,
                    ui_draw_calls,
                    tooltip_draw_calls,
                );

                #[cfg(target_os = "horizon")]
                self.fps_log.stage(6, &mut perf_stage);
                if let Some(ui_ctx) = &mut self.ui_context {
                    ui_ctx.begin_frame();
                }
                #[cfg(target_os = "horizon")]
                {
                    self.fps_log.stage(7, &mut perf_stage);
                    self.fps_log.render_parts(
                        self.renderer
                            .as_ref()
                            .map_or([0; 3], |renderer| renderer.frame_timing_us),
                    );
                    self.fps_log.finish_stages(
                        self.game
                            .world
                            .entities
                            .player()
                            .is_some_and(|player| player.movement.is_moving()),
                        self.game
                            .world
                            .entities
                            .player()
                            .map(|player| player.movement.position()),
                    );
                    self.fps_log.frame(Instant::now());
                }
            }
            _ => {}
        }
    }

    /// Off screen (Android): the native window is going away, nothing is drawn until resumed.
    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.suspended = true;
        self.flush_config(None);
        // the app may be closed from the background: the hub's picture now
        self.save_player_snapshot();
        if let Some(ctx) = &mut self.ui_context {
            ctx.clear_fingers();
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        // Controller axes, from the vendored winit's Android joystick events. On desktop Motion is the raw
        // mouse (axes 0/1, the left stick's numbers), which would walk the character after the mouse.
        if cfg!(any(target_os = "android", target_os = "horizon"))
            && let winit::event::DeviceEvent::Motion { axis, value } = event
        {
            self.gamepad.axis(axis, value);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(target_os = "horizon")]
        {
            // The native loop owns the 60 Hz deadline and accounts for work and
            // FIFO waits. Request this frame before that loop considers sleeping.
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::Poll);
        }
        #[cfg(not(target_os = "horizon"))]
        {
            let now = Instant::now();
            if now >= self.next_frame {
                self.next_frame = now + FRAME_INTERVAL;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
        }
    }
}

fn pick_account_background(paths: &[String]) -> Option<String> {
    if paths.is_empty() {
        return None;
    }
    let idx = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0)
        % paths.len();
    Some(paths[idx].clone())
}

/// config.json from the working directory, then the game until the window closes. Logging is the caller's.
pub fn run(event_loop: EventLoop<()>) {
    let config = Config::load_or_default("config.json");
    nostalre_profiling::debug::init(
        config.debug.trace_packet,
        config.debug.trace_effects,
        config.debug.trace_input,
        config.debug.trace_texture_load,
        config.debug.trace_sprite_scale,
    );

    let mut app = App::new(config);
    event_loop.run_app(&mut app).unwrap();
    #[cfg(target_os = "horizon")]
    switch::record_stage("[Shutdown] Client event loop returned");
    // every way out of the game (the window closed, Exit, a script's end) leaves the world here
    app.save_player_snapshot();
    #[cfg(target_os = "horizon")]
    switch::record_stage("[Shutdown] Character snapshot finished; dropping client/video resources");
    drop(app);
    #[cfg(target_os = "horizon")]
    switch::record_stage("[Shutdown] Client/video resources dropped");
}

fn init_logging<W>(writer: W)
where
    W: for<'w> tracing_subscriber::fmt::MakeWriter<'w> + Send + Sync + 'static,
{
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(writer)
        // Android keeps the library loaded between game sessions: the second run finds it set up
        .try_init()
        .ok();
}

pub fn desktop_main() {
    init_logging(std::io::stdout);
    run(EventLoop::new().unwrap());
}

/// Entry point when Android's NativeActivity loads the library (Ragnarok Solo's game screen). The app writes
/// config.json into the activity's internal data directory first, and the log goes to client.log next to it.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;

    // a panic must end the ":game" process too, or the next game session finds winit's event loop taken; one
    // in packet dispatch is caught there and only drops the packet
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        report(info);
        if !nostalre_network::panic_is_guarded() {
            std::process::exit(1);
        }
    }));

    if let Some(dir) = app.internal_data_path() {
        let _ = std::env::set_current_dir(&dir);
        // env.txt (KEY=VALUE lines, optional): WGPU_BACKEND, RUST_LOG... for a device without a shell to set them
        if let Ok(text) = std::fs::read_to_string(dir.join("env.txt")) {
            for (key, value) in text.lines().filter_map(|line| line.trim().split_once('=')) {
                // SAFETY: before the game starts a thread. Android's own threads already run, but none of them
                // reads these variables; set_var races only with a concurrent getenv of the same environment.
                unsafe { std::env::set_var(key.trim(), value.trim()) };
            }
        }
        if let Ok(log) = std::fs::File::create(dir.join("client.log")) {
            init_logging(std::sync::Mutex::new(log));
        }
    }
    run(EventLoop::builder().with_android_app(app).build().unwrap());
    // winit allows one event loop per process: the activity runs in its own (":game"), ended with the game
    std::process::exit(0);
}

#[cfg(target_os = "horizon")]
fn main() {
    switch::main();
}
