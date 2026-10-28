//! Scripted runs on the desktop: with `NOSTALRE_SCRIPT=<file>` the client performs the file's steps on its own once
//! in game, so a change can be watched without a hand on the controls (Ragnarok Solo's
//! `scripts/run-nostalre-script.sh` writes the files and takes the screenshots `shot` asks for).
//!
//! One step per line; `#` starts a comment:
//!
//! ```text
//! wait SECONDS        pause
//! chat TEXT           send TEXT as chat; GM commands start with @
//! wait_map NAME       until the player stands on map NAME (gives up after 30 s)
//! walk DX DY          walk to the cell DX, DY away from the player's
//! zoom NOTCHES        camera zoom in mouse-wheel notches
//! turn DEGREES        camera rotation
//! equip ITEM_ID       wear the first such item in the inventory where it goes
//! attack              attack the nearest monster, repeatedly, as a click on it does
//! talk                talk to the nearest NPC in reach, as a click on it does
//! hotkey ACTION       the window a hotkey opens, by its action name (`ToggleStatus`, `ToggleBasicInfo`...), as
//!                     the classic layout opens it whatever the HUD
//! menu ACTION         the hotkey as the player presses it: with the frameless HUD, the menu's screen for it
//! skill ID LEVEL      use skill ID at LEVEL, as its hotbar slot does (on the player, or by targeting)
//! skill-at ID LEVEL   use skill ID at LEVEL as its hotbar slot does, then click as its kind asks: the nearest
//!                     monster (an attack), the player (a support skill), the monster's cell or the player's (ground)
//! click ID DX DY      a left click DX, DY (UI pixels) from the top-left corner of the window with widget id ID,
//!                     where it was drawn last (fixed popups too)
//! type TEXT           TEXT typed into the focused text field
//! dclick ID DX DY     the same as a double-click
//! drag ID DX DY ID2 DX2 DY2
//!                     press at the first point, move to the second, let go there (an item into a slot)
//! shot NAME           log "script shot NAME" for the wrapper's screenshot, then wait a second for it
//! snap NAME           the same screenshot without the wait, for a shot timed to a skill's effects
//! escape              press Escape once, as the keyboard does (closes the window on top: a skill's menu)
//! key NAME            press a key the menu reads: enter, up, down, left, right, delete, pageup, pagedown, or a
//!                     letter (key s)
//! wait_hit SECONDS    until the player's last skill shows its result (a damage, no-damage or ground skill packet
//!                     from the server), SECONDS at most: shots timed to a skill's release
//! charselect          back to character select, as the system menu's button
//! quit                close the client
//! ```
//!
//! Every step is logged as "script: <step>" and the end as "script done".

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use winit::event_loop::ActiveEventLoop;
use winit::keyboard::KeyCode;

use models::enums::skill_enums::SkillEnum;
use nostalre_game::cursor::PendingSkillTarget;
use nostalre_game::entity::EntityCategory;
use nostalre_game::targeting::{TargetClass, skill_target_class};
use nostalre_ui::state::StateCache;

use crate::App;

const MAP_TIMEOUT: Duration = Duration::from_secs(30);
const SHOT_SETTLE: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq)]
enum Step {
    Wait(Duration),
    Chat(String),
    WaitMap(String),
    Walk(i32, i32),
    Zoom(f32),
    Turn(f32),
    Shot(String),
    Equip(u32),
    Attack,
    Talk,
    /// The action's window, as the classic layout opens it
    Hotkey(nostalre_game::keybinding::HotkeyAction),
    /// The action's key as the player presses it: the frameless HUD's menu screen for it
    Menu(nostalre_game::keybinding::HotkeyAction),
    Skill(SkillEnum, i16),
    SkillAt(SkillEnum, i16),
    /// The point, and whether it is a double-click
    Click(WindowPoint, bool),
    Drag(WindowPoint, WindowPoint),
    Type(String),
    /// A `shot` without the settle, the steps after it going on at once
    Snap(String),
    Escape,
    /// A key the menu reads (`key enter`, `key down`, `key pagedown`...)
    Key(ScriptKey),
    WaitHit(Duration),
    CharSelect,
    Quit,
}

/// A point DX, DY (UI pixels) from a window's top-left corner
#[derive(Clone, Copy, Debug, PartialEq)]
struct WindowPoint {
    window: u32,
    dx: f32,
    dy: f32,
}

impl WindowPoint {
    fn parse<'a>(words: &mut impl Iterator<Item = &'a str>) -> Option<Self> {
        Some(Self {
            window: words.next()?.parse().ok()?,
            dx: words.next()?.parse().ok()?,
            dy: words.next()?.parse().ok()?,
        })
    }
}

/// Where a window-relative point is on screen, the window as drawn last
fn on_screen(ui: &StateCache, point: WindowPoint) -> Option<(f32, f32)> {
    let origin = ui.drawn_window_origin(nostalre_ui::frame::WidgetId(point.window));
    if origin.is_none() {
        tracing::warn!("script: no window {} on screen", point.window);
    }
    origin.map(|[x, y]| (x + point.dx, y + point.dy))
}

/// A drag's two screen points and its stage
struct Drag {
    from: (f32, f32),
    to: (f32, f32),
    frame: u8,
}

impl Step {
    /// Talks to the map server or acts in the world: held while not in game (after `charselect`)
    fn needs_game(&self) -> bool {
        matches!(
            self,
            Step::Chat(_)
                | Step::Walk(..)
                | Step::Equip(_)
                | Step::Attack
                | Step::Talk
                | Step::Hotkey(_)
                | Step::Skill(..)
                | Step::SkillAt(..)
        )
    }
}

/// A comment starts a line, or is a `#` standing alone; one before a word is a GM char command (`#option <name>`).
fn strip_comment(line: &str) -> &str {
    let line = line.trim();
    if line.starts_with('#') {
        return "";
    }
    let at = line
        .match_indices('#')
        .find(|(i, _)| {
            line[..*i].ends_with(char::is_whitespace)
                && line[i + 1..].chars().next().is_none_or(char::is_whitespace)
        })
        .map_or(line.len(), |(i, _)| i);
    line[..at].trim_end()
}

fn parse(text: &str) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = strip_comment(line);
        if line.is_empty() {
            continue;
        }
        let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        let number = |what: &str| -> Result<f32, String> {
            rest.parse()
                .map_err(|_| format!("line {}: {word} needs {what}", n + 1))
        };
        let step = match word {
            "wait" => Step::Wait(
                Duration::try_from_secs_f32(number("seconds")?)
                    .map_err(|_| format!("line {}: wait needs seconds, 0 or more", n + 1))?,
            ),
            "chat" => Step::Chat(rest.to_string()),
            "wait_map" => Step::WaitMap(rest.to_string()),
            "walk" => {
                let mut cells = rest.split_whitespace().map(str::parse::<i32>);
                match (cells.next(), cells.next()) {
                    (Some(Ok(dx)), Some(Ok(dy))) => Step::Walk(dx, dy),
                    _ => return Err(format!("line {}: walk needs DX DY", n + 1)),
                }
            }
            "zoom" => Step::Zoom(number("notches")?),
            "turn" => Step::Turn(number("degrees")?),
            "shot" => Step::Shot(rest.to_string()),
            "snap" => Step::Snap(rest.to_string()),
            "escape" => Step::Escape,
            "key" => Step::Key(
                ScriptKey::parse(rest)
                    .ok_or_else(|| format!("line {}: unknown key {rest}", n + 1))?,
            ),
            "wait_hit" => Step::WaitHit(
                Duration::try_from_secs_f32(number("seconds")?)
                    .map_err(|_| format!("line {}: wait_hit needs seconds, 0 or more", n + 1))?,
            ),
            "equip" => Step::Equip(
                rest.parse()
                    .map_err(|_| format!("line {}: equip needs ITEM_ID", n + 1))?,
            ),
            "attack" => Step::Attack,
            "talk" => Step::Talk,
            "hotkey" | "menu" => {
                let action = serde_json::from_value(serde_json::Value::String(rest.to_string()))
                    .map_err(|_| format!("line {}: unknown hotkey action {rest}", n + 1))?;
                if word == "menu" {
                    Step::Menu(action)
                } else {
                    Step::Hotkey(action)
                }
            }
            "click" | "dclick" => {
                let mut v = rest.split_whitespace();
                match WindowPoint::parse(&mut v) {
                    Some(point) => Step::Click(point, word == "dclick"),
                    None => return Err(format!("line {}: {word} needs ID DX DY", n + 1)),
                }
            }
            "drag" => {
                let mut v = rest.split_whitespace();
                match (WindowPoint::parse(&mut v), WindowPoint::parse(&mut v)) {
                    (Some(from), Some(to)) => Step::Drag(from, to),
                    _ => return Err(format!("line {}: drag needs ID DX DY ID2 DX2 DY2", n + 1)),
                }
            }
            "skill" | "skill-at" => {
                let mut v = rest.split_whitespace();
                let parsed = (
                    v.next()
                        .and_then(|s| s.parse::<u32>().ok())
                        .and_then(|id| SkillEnum::try_from_value(id).ok()),
                    v.next().and_then(|s| s.parse::<i16>().ok()),
                );
                match parsed {
                    (Some(skill), Some(level)) if word == "skill" => Step::Skill(skill, level),
                    (Some(skill), Some(level)) => Step::SkillAt(skill, level),
                    _ => {
                        return Err(format!(
                            "line {}: skill needs a known SKILL_ID and LEVEL",
                            n + 1
                        ));
                    }
                }
            }
            "type" => Step::Type(rest.to_string()),
            "charselect" => Step::CharSelect,
            "quit" => Step::Quit,
            other => return Err(format!("line {}: unknown step {other}", n + 1)),
        };
        steps.push(step);
    }
    Ok(steps)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScriptKey {
    Enter,
    Up,
    Down,
    Left,
    Right,
    Delete,
    PageUp,
    PageDown,
    /// A letter key, by its code (`key s`: the menu's "to bar")
    Letter(KeyCode),
}

const LETTERS: [KeyCode; 26] = [
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
];

impl ScriptKey {
    fn parse(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "enter" => Self::Enter,
            "up" => Self::Up,
            "down" => Self::Down,
            "left" => Self::Left,
            "right" => Self::Right,
            "delete" => Self::Delete,
            "pageup" => Self::PageUp,
            "pagedown" => Self::PageDown,
            letter if letter.len() == 1 && letter.as_bytes()[0].is_ascii_lowercase() => {
                Self::Letter(LETTERS[usize::from(letter.as_bytes()[0] - b'a')])
            }
            _ => return None,
        })
    }
}

pub(crate) struct Script {
    steps: VecDeque<Step>,
    /// The current step holds until then (`wait`, `shot`), or until the map (`wait_map`).
    until: Option<Instant>,
    map: Option<String>,
    /// A `click` (and whether it is a double one), made before the next frame's UI is built (the UI context
    /// forgets clicks at the end of a frame)
    click: Option<(f32, f32, bool)>,
    /// `type`'s text, given to the next frame's UI as typed characters
    typed: Option<String>,
    /// `escape`, given to the next frame's UI as the key
    escape: bool,
    /// `key`, given to the next frame's UI
    key: Option<ScriptKey>,
    /// Whether the player's last skill showed its result (`wait_hit`), from its `skill-at` on
    skill_result: bool,
    /// `wait_hit` holds until `skill_result`, or `until`
    awaiting_result: bool,
    /// A `drag` under way, one stage a frame
    drag: Option<Drag>,
    /// Steps begin once in game; out of it (`charselect`) only those that need no game go on
    started: bool,
}

impl Script {
    pub(crate) fn from_env() -> Option<Self> {
        #[cfg(target_os = "horizon")]
        let one_shot = std::path::Path::new("diagnostic-run.once").is_file();
        #[cfg(not(target_os = "horizon"))]
        let one_shot = false;
        let path = std::env::var_os("NOSTALRE_SCRIPT").or_else(||
            one_shot.then(|| std::ffi::OsString::from("diagnostic-script.txt")))?;
        let steps = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| parse(&text));
        match steps {
            Ok(steps) => {
                if one_shot {
                    if let Err(error) = std::fs::remove_file("diagnostic-run.once") {
                        tracing::error!("Diagnostic script not started: cannot consume one-shot marker: {error}");
                        return None;
                    }
                }
                tracing::info!("script {}: {} steps", path.to_string_lossy(), steps.len());
                Some(Self {
                    steps: steps.into(),
                    until: None,
                    started: false,
                    map: None,
                    click: None,
                    typed: None,
                    escape: false,
                    key: None,
                    skill_result: false,
                    awaiting_result: false,
                    drag: None,
                })
            }
            Err(e) => {
                tracing::error!("script {}: {e}", path.to_string_lossy());
                None
            }
        }
    }
}

impl App {
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn validation_click_skill(&mut self) {
        // A real player chooses a visible live monster, never the corpse left
        // by the preceding iteration of a scripted stress scenario.
        if let Some(PendingSkillTarget::Entity { skill, level }) = self.game.pending_casts.pending_skill_target {
            let hostile = self.game.resolve_cast_skill(skill).is_some_and(|(kind, _)| skill_target_class(kind) != TargetClass::Supportive);
            if hostile {
                let from = self.game.world.entities.player().unwrap().movement.cell_position();
                let target = self.game.world.entities.iter()
                    .filter(|e| e.category() == EntityCategory::Monster && e.is_alive() && !e.is_pet)
                    .min_by_key(|e| {let (x,y)=e.movement.cell_position();x.abs_diff(from.0).max(y.abs_diff(from.1))})
                    .map(|e|e.id);
                if let Some(target) = target {
                    self.game.pending_casts.pending_skill_target = None;
                    self.cast_skill_on_unit(skill, level, target);
                }
                return;
            }
        }
        self.click_for_skill();
    }

    /// A skill's result from the server (its damage, no-damage or ground packet): the player's ends `wait_hit`
    pub(crate) fn note_script_skill_result(&mut self, src_gid: u32) {
        if let Some(script) = &mut self.script
            && self.game.world.entities.player_id() == Some(src_gid)
        {
            script.skill_result = true;
        }
    }

    /// The script's pending `click` and `type`, for the UI about to be built
    pub(crate) fn apply_script_input(&mut self) {
        let (Some(script), Some(ui_ctx)) = (self.script.as_mut(), self.ui_context.as_mut()) else {
            return;
        };
        if let Some((x, y, double)) = script.click.take() {
            ui_ctx.mouse_x = x;
            ui_ctx.mouse_y = y;
            ui_ctx.mouse_clicked = true;
            ui_ctx.mouse_pressed = true;
            ui_ctx.mouse_double_clicked = double;
        }
        if let Some(text) = script.typed.take() {
            ui_ctx.typed_chars.extend(text.chars());
        }
        if std::mem::take(&mut script.escape) {
            ui_ctx.key_escape = true;
        }
        match script.key.take() {
            Some(ScriptKey::Enter) => ui_ctx.key_enter = true,
            Some(ScriptKey::Up) => ui_ctx.key_up = true,
            Some(ScriptKey::Down) => ui_ctx.key_down = true,
            Some(ScriptKey::Left) => ui_ctx.key_left = true,
            Some(ScriptKey::Right) => ui_ctx.key_right = true,
            Some(ScriptKey::Delete) => ui_ctx.key_delete = true,
            Some(ScriptKey::PageUp) => ui_ctx.pressed_codes.push(KeyCode::PageUp),
            Some(ScriptKey::PageDown) => ui_ctx.pressed_codes.push(KeyCode::PageDown),
            Some(ScriptKey::Letter(code)) => ui_ctx.pressed_codes.push(code),
            None => {}
        }
        if let Some(drag) = &mut script.drag {
            // pressed on the source, moved past the drag threshold, onto the target, let go there
            let (x, y, down) = match drag.frame {
                0 => (drag.from.0, drag.from.1, true),
                1 => (
                    (drag.from.0 + drag.to.0) / 2.0,
                    (drag.from.1 + drag.to.1) / 2.0,
                    true,
                ),
                2 => (drag.to.0, drag.to.1, true),
                _ => (drag.to.0, drag.to.1, false),
            };
            ui_ctx.mouse_x = x;
            ui_ctx.mouse_y = y;
            ui_ctx.mouse_down = down;
            ui_ctx.mouse_clicked = drag.frame == 0;
            ui_ctx.mouse_pressed = drag.frame == 0;
            drag.frame += 1;
            if !down {
                script.drag = None;
            }
        }
    }

    /// The id of the unit of this category closest to the player, in cells
    fn nearest_entity(&self, category: EntityCategory) -> Option<u32> {
        let (px, py) = self.game.world.entities.player()?.movement.cell_position();
        self.game
            .world
            .entities
            .iter()
            .filter(|e| e.category() == category)
            .min_by_key(|e| {
                let (x, y) = e.movement.cell_position();
                x.abs_diff(px).max(y.abs_diff(py))
            })
            .map(|e| e.id)
    }

    /// The click a skill waiting for its target gets from `skill-at`, through the click's own cast
    fn click_for_skill(&mut self) {
        let Some(pending) = self.game.pending_casts.pending_skill_target.take() else {
            return;
        };
        let monster = self.nearest_entity(EntityCategory::Monster);
        let player = self.game.world.entities.player_id();
        match pending {
            PendingSkillTarget::Entity { skill, level } => {
                let support = self.game.resolve_cast_skill(skill).is_some_and(|(kind, _)| {
                    skill_target_class(kind) == TargetClass::Supportive
                });
                match if support { player } else { monster } {
                    Some(target) => {
                        self.cast_skill_on_unit(skill, level, target);
                    }
                    None => tracing::warn!("script: no target for {skill:?}"),
                }
            }
            PendingSkillTarget::Ground { skill, level } => {
                let cell = monster
                    .or(player)
                    .and_then(|id| self.game.world.entities.get(id))
                    .map(|e| e.movement.cell_position());
                match cell {
                    Some((x, y)) => self.cast_skill_on_cell(skill, level, x as i16, y as i16),
                    None => tracing::warn!("script: no cell for {skill:?}"),
                }
            }
            PendingSkillTarget::SkillUnit { skill, .. } => {
                tracing::warn!("script: {skill:?} is cast on a trap, which skill-at does not pick")
            }
        }
    }

    /// Runs the script's steps that are due; called every frame (steps start once in game).
    pub(crate) fn run_script(&mut self, now: Instant, event_loop: &ActiveEventLoop) {
        let in_game = self.game.session.app_state == nostalre_game::app_state::AppState::InGame;
        let Some(script) = &mut self.script else {
            return;
        };
        script.started |= in_game;
        if !script.started {
            return;
        }
        if let Some(map) = &script.map {
            let on_map = self.game.session.current_map.as_deref().is_some_and(|m| {
                m.trim_end_matches(".gat") == map.as_str()
                    || m.trim_end_matches(".rsw") == map.as_str()
            });
            let late = script.until.is_some_and(|t| now >= t);
            if !on_map && !late {
                return;
            }
            if late && !on_map {
                tracing::warn!("script: not on {map} after {}s", MAP_TIMEOUT.as_secs());
            }
            script.map = None;
            script.until = None;
        }
        if script.awaiting_result {
            let late = script.until.is_some_and(|t| now >= t);
            if !script.skill_result && !late {
                return;
            }
            if !script.skill_result {
                tracing::warn!("script: no result of the last skill");
            }
            script.awaiting_result = false;
            script.until = None;
        }
        if script.until.is_some_and(|t| now < t) {
            return;
        }
        script.until = None;
        if !in_game && script.steps.front().is_some_and(Step::needs_game) {
            return;
        }
        let Some(step) = script.steps.pop_front() else {
            tracing::info!("script done");
            self.script = None;
            return;
        };
        tracing::info!("script: {step:?}");
        if matches!(step, Step::Skill(..) | Step::SkillAt(..)) {
            script.skill_result = false;
        }
        match step {
            Step::Wait(d) => script.until = Some(now + d),
            Step::WaitMap(map) => {
                script.map = Some(map);
                script.until = Some(now + MAP_TIMEOUT);
            }
            Step::Shot(name) => {
                #[cfg(target_os = "horizon")]
                if name == "BENCHMARK-BEGIN" { self.fps_log.begin_window(now); }
                tracing::info!("script shot {name}");
                script.until = Some(now + SHOT_SETTLE);
            }
            Step::Snap(name) => {
                #[cfg(target_os = "horizon")]
                if name == "BENCHMARK-BEGIN" { self.fps_log.begin_window(now); }
                tracing::info!("script shot {name}");
            }
            Step::Escape => script.escape = true,
            Step::Key(key) => script.key = Some(key),
            Step::WaitHit(timeout) => {
                script.awaiting_result = true;
                script.until = Some(now + timeout);
            }
            Step::Chat(text) => self.run_chat_command(&text),
            Step::Walk(dx, dy) => {
                if let Some(player) = self.game.world.entities.player() {
                    let (x, y) = player.movement.cell_position();
                    self.game.combat.queued_move =
                        Some(((x as i32 + dx).max(0), (y as i32 + dy).max(0)));
                }
            }
            Step::Zoom(notches) => {
                let control = self.camera_control();
                if let Some(renderer) = &mut self.renderer {
                    renderer.camera.apply_wheel(notches, control);
                }
            }
            Step::Turn(degrees) => {
                let control = self.camera_control();
                if let Some(renderer) = &mut self.renderer {
                    renderer.camera.rotate_by(degrees, control);
                }
            }
            Step::Equip(item_id) => {
                let item = self
                    .game
                    .character
                    .inventory
                    .find_by_item_id(item_id)
                    .map(|item| (item.index, item.location));
                match item {
                    Some((index, location)) => {
                        self.channel
                            .send_packet(nostalre_network::sender::build_equip_item_packet(
                                index,
                                location,
                                self.active_packetver,
                            ))
                    }
                    None => tracing::warn!("script: no item {item_id} to equip"),
                }
            }
            Step::Attack => match self.nearest_entity(EntityCategory::Monster) {
                Some(gid) => self.initiate_attack(gid),
                None => tracing::warn!("script: no monster to attack"),
            },
            // the controller's and the touch Talk's pick: in reach, no warp portal
            Step::Talk => {
                if !self.talk_to_nearest_npc() {
                    tracing::warn!("script: no NPC to talk to");
                }
            }
            // the window itself, as the classic layout opens it, so steps clicking into it work with either HUD
            Step::Hotkey(action) => self.run_action(action),
            Step::Menu(action) => self.dispatch_action(action),
            Step::Skill(skill, level) => self.handle_ui_events(
                vec![nostalre_game::event::GameEvent::RequestUseSkill { skill, level }],
                event_loop,
            ),
            Step::Type(text) => script.typed = Some(text),
            Step::Click(point, double) => {
                if let Some((x, y)) = on_screen(&self.ui_state_cache, point) {
                    tracing::info!("script: click at {x}, {y}");
                    script.click = Some((x, y, double));
                }
            }
            Step::Drag(from, to) => {
                if let (Some(from), Some(to)) = (
                    on_screen(&self.ui_state_cache, from),
                    on_screen(&self.ui_state_cache, to),
                ) {
                    tracing::info!("script: drag from {from:?} to {to:?}");
                    script.drag = Some(Drag { from, to, frame: 0 });
                }
            }
            Step::SkillAt(skill, level) => {
                self.handle_ui_events(
                    vec![nostalre_game::event::GameEvent::RequestUseSkill { skill, level }],
                    event_loop,
                );
                self.click_for_skill();
            }
            Step::CharSelect => self.handle_ui_events(
                vec![nostalre_game::event::GameEvent::BackToCharacterSelect],
                event_loop,
            ),
            Step::Quit => {
                #[cfg(test)]
                if std::env::var_os("NOSTALRE_NPC_QUIT_VALIDATION").is_some() {
                    assert!(self.windows.npc_dialog.dialog.is_open(), "exit must exercise an open NPC dialog");
                    println!("NPC-EXIT-CHECK dialog_open=true request=QuitGame");
                    self.handle_ui_events(vec![nostalre_game::event::GameEvent::QuitGame], event_loop);
                    return;
                }
                tracing::info!("script done");
                event_loop.exit();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(window: u32, dx: f32, dy: f32) -> WindowPoint {
        WindowPoint { window, dx, dy }
    }

    #[test]
    fn a_chat_step_keeps_a_char_command() {
        assert_eq!(
            parse("# a comment\nchat #option Prober 0 0 0 # its option\n").unwrap(),
            [Step::Chat("#option Prober 0 0 0".into())]
        );
    }

    #[test]
    fn only_steps_without_the_game_run_out_of_it() {
        let held: Vec<bool> =
            parse("wait 1\nshot a\ncharselect\nchat hi\nequip 1201\nattack\ntalk\nquit\n")
                .unwrap()
                .iter()
                .map(Step::needs_game)
                .collect();
        assert_eq!(held, [false, false, false, true, true, true, true, false]);
    }

    #[test]
    fn a_negative_wait_is_a_mistake_in_the_file_not_a_crash() {
        assert!(parse("wait -0.1\n").is_err());
    }

    #[test]
    fn parses_every_step_and_skips_comments() {
        let steps = parse(
            "# tour\nwait 1.5\nchat @warp prontera  # go\nwait_map prontera\nwalk 3 -2\nzoom -2\nturn 45\nshot p-0\nequip 630014\nattack\ntalk\nhotkey ToggleStatus\nskill 41 1\nskill-at 2560 5\nclick 1500 100 121.5\ndclick 800 4 5\ndrag 4900 18 39 2300 17 65\ntype 1,000\nsnap p-1\nescape\nwait_hit 2\ncharselect\nquit\n",
        )
        .unwrap();
        assert_eq!(
            steps,
            vec![
                Step::Wait(Duration::from_secs_f32(1.5)),
                Step::Chat("@warp prontera".into()),
                Step::WaitMap("prontera".into()),
                Step::Walk(3, -2),
                Step::Zoom(-2.0),
                Step::Turn(45.0),
                Step::Shot("p-0".into()),
                Step::Equip(630014),
                Step::Attack,
                Step::Talk,
                Step::Hotkey(nostalre_game::keybinding::HotkeyAction::ToggleStatus),
                Step::Skill(SkillEnum::McVending, 1),
                Step::SkillAt(SkillEnum::RlCMarker, 5),
                Step::Click(point(1500, 100.0, 121.5), false),
                Step::Click(point(800, 4.0, 5.0), true),
                Step::Drag(point(4900, 18.0, 39.0), point(2300, 17.0, 65.0)),
                Step::Type("1,000".into()),
                Step::Snap("p-1".into()),
                Step::Escape,
                Step::WaitHit(Duration::from_secs(2)),
                Step::CharSelect,
                Step::Quit,
            ]
        );
    }

    #[test]
    fn a_bad_line_names_its_number() {
        assert_eq!(
            parse("wait 1\nwalk 3\n").unwrap_err(),
            "line 2: walk needs DX DY"
        );
        assert_eq!(parse("jump\n").unwrap_err(), "line 1: unknown step jump");
        assert_eq!(
            parse("hotkey Fly\n").unwrap_err(),
            "line 1: unknown hotkey action Fly"
        );
        assert_eq!(
            parse("click 1500 3\n").unwrap_err(),
            "line 1: click needs ID DX DY"
        );
    }
}
