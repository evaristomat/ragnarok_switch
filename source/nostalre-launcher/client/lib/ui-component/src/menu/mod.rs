//! The frameless menu (`docs/nostalre-menu.md`): the HUD layout's full-screen menu, as old Final Fantasy
//! games have one. Text straight on the darkened world, the commands down the right, each screen in the same
//! frame. Escape opens it once nothing else wants the key; the game goes on underneath.
mod bar_picker;
mod compact;
mod equipment;
mod frame;
mod home;
mod item_detail;
mod items;
mod settings;
mod skills;
mod status;
mod travel;
mod travel_catalog;

use crate::hud::Place;
use crate::{BuildCtx, InGameWindow, Window};
use bar_picker::BarPicker;
pub(crate) use frame::HANDHELD;
use frame::Keys;
use nostalre_game::event::GameEvent;
use nostalre_game::keybinding::{HotkeyAction, KeyBindings, KeyChord};
use nostalre_ui::frame::{UiFrame, WidgetId, WindowOrder};
use nostalre_ui::rect::Rect;
pub use settings::SettingsValues;

pub const MENU_WINDOW_ID: WidgetId = WidgetId(6300);
const MENU_BG_ID: WidgetId = WidgetId(6301);
const CRUMB_ID: WidgetId = WidgetId(6302);
/// The skill-bar picker and the drop question, a window over the screen so the screen under it takes no taps
const MODAL_ID: WidgetId = WidgetId(6303);
/// The commands, 6310 to 6329
const CMD_BASE_ID: u32 = 6310;
/// The footer's hints, 6330 to 6339
const HINT_BASE_ID: u32 = 6330;
/// A screen's tabs, 6340 to 6349
const TAB_BASE_ID: u32 = 6340;
/// The status rows' raise buttons, 6350 to 6369
const ACTION_BASE_ID: u32 = 6350;
/// An item's card slots, 6370 to 6379
const CARD_BASE_ID: u32 = 6370;
/// A screen's list rows, 6400 to 6699
const ROW_BASE_ID: u32 = 6400;
/// Settings' page list (6500 to 6519), its "Reset page" row, its Keys page's rows (6550 to 6599): in the rows'
/// range, past the longest options page
const SETTINGS_PAGE_BASE_ID: u32 = 6500;
const SETTINGS_RESET_ID: WidgetId = WidgetId(6520);
const SETTINGS_KEY_BASE_ID: u32 = 6550;
/// The skill tree's nodes, 6700 to 6949
const NODE_BASE_ID: u32 = 6700;
const NODE_IDS: u32 = 250;
/// The skill-bar picker's slots, 6950 to 6999
const SLOT_BASE_ID: u32 = 6950;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Items,
    Equipment,
    Skills,
    Status,
    Quests,
    Companions,
    Party,
    Guild,
    Friends,
    WorldMap,
    GmTravel,
    JobChanger,
    Campaign,
    Settings,
    CharacterSelect,
    ExitGame,
}

/// The commands, the system ones after the gap
const COMMANDS: [Command; 16] = [
    Command::Items,
    Command::Equipment,
    Command::Skills,
    Command::Status,
    Command::Quests,
    Command::Companions,
    Command::Party,
    Command::Guild,
    Command::Friends,
    Command::WorldMap,
    Command::GmTravel,
    Command::JobChanger,
    Command::Campaign,
    Command::Settings,
    Command::CharacterSelect,
    Command::ExitGame,
];
const FIRST_SYSTEM: usize = 13;

impl Command {
    fn label(self) -> &'static str {
        match self {
            Self::Items => crate::pt!("Items", "Itens"),
            Self::Equipment => crate::pt!("Equipment", "Equipamentos"),
            Self::Skills => crate::pt!("Skills", "Habilidades"),
            Self::Status => "Status",
            Self::Quests => crate::pt!("Quests", "Missões"),
            Self::Companions => crate::pt!("Companions", "Companheiros"),
            Self::Party => crate::pt!("Party", "Grupo"),
            Self::Guild => crate::pt!("Guild", "Clã"),
            Self::Friends => crate::pt!("Friends", "Amigos"),
            Self::WorldMap => crate::pt!("World Map", "Mapa-múndi"),
            Self::GmTravel => "GM / Viagens",
            Self::JobChanger => "Job Changer",
            Self::Campaign => "Campanha / Equipe",
            Self::Settings => crate::pt!("Settings", "Opções"),
            Self::CharacterSelect => crate::pt!("Character Select", "Selecionar personagem"),
            Self::ExitGame => crate::pt!("Exit Game", "Sair do jogo"),
        }
    }

    fn help(self) -> &'static str {
        match self {
            Self::Items => crate::pt!(
                "Use and wear what is in your bag.",
                "Use e equipe os itens do inventário."
            ),
            Self::Equipment => crate::pt!(
                "What you wear, slot by slot, and what in your bag fits.",
                "Seus equipamentos e os itens compatíveis do inventário."
            ),
            Self::Skills => crate::pt!(
                "Your skill tree: spend skill points.",
                "Árvore de habilidades: distribua seus pontos."
            ),
            Self::Status => crate::pt!(
                "Stats, traits and status points.",
                "Atributos, características e pontos."
            ),
            Self::Quests => crate::pt!(
                "Active and finished quests.",
                "Missões ativas e concluídas."
            ),
            Self::Companions => crate::pt!(
                "Your pet, homunculus and mercenary.",
                "Seu mascote, homúnculo e mercenário."
            ),
            Self::Party => crate::pt!(
                "Members, invites and loot sharing.",
                "Membros, convites e divisão de itens."
            ),
            Self::Guild => crate::pt!(
                "Members, skills, alliances and emblem.",
                "Membros, habilidades, alianças e emblema."
            ),
            Self::Friends => crate::pt!(
                "Friend list and whispers.",
                "Lista de amigos e mensagens privadas."
            ),
            Self::WorldMap => crate::pt!(
                "The world map and where you are.",
                "Mapa-múndi e sua localização."
            ),
            Self::GmTravel => {
                "Viajar para cidades, dungeons e campos com os comandos GM do servidor."
            }
            Self::Campaign => "Retomar a história, viajar ao objetivo e recrutar ou reagrupar companheiros.",
            Self::JobChanger => "Abrir o Mestre das Classes. Respeita níveis e pontos de habilidade distribuídos.",
            Self::Settings => crate::pt!(
                "Display, graphics, sound, keys and chat lines.",
                "Tela, gráficos, som, teclas e frases de chat."
            ),
            Self::CharacterSelect => crate::pt!(
                "Leave the map for character select.",
                "Voltar à seleção de personagens."
            ),
            Self::ExitGame => crate::pt!("Close nostalRE.", "Fechar o nostalRE."),
        }
    }

    /// The window a command opens, whose key the list shows
    fn hotkey(self) -> Option<HotkeyAction> {
        Some(match self {
            Self::Items => HotkeyAction::ToggleInventory,
            Self::Equipment => HotkeyAction::ToggleEquipment,
            Self::Skills => HotkeyAction::ToggleSkillTree,
            Self::Status => HotkeyAction::ToggleStatus,
            Self::Quests => HotkeyAction::ToggleQuest,
            Self::Party => HotkeyAction::ToggleParty,
            Self::Guild => HotkeyAction::ToggleGuild,
            Self::Friends => HotkeyAction::ToggleFriends,
            Self::WorldMap => HotkeyAction::ToggleWorldMap,
            _ => return None,
        })
    }

    fn screen(self) -> Option<Screen> {
        Some(match self {
            Self::Items => Screen::Items,
            Self::Equipment => Screen::Equipment,
            Self::Skills => Screen::Skills,
            Self::Status => Screen::Status,
            Self::GmTravel => Screen::GmTravel,
            Self::Settings => Screen::Settings,
            _ => return None,
        })
    }

    /// Asks before it acts
    fn confirm(self) -> Option<&'static str> {
        match self {
            Self::CharacterSelect => Some(crate::pt!(
                "Return to character select?",
                "Voltar à seleção de personagens?"
            )),
            Self::ExitGame => Some(crate::pt!("Close nostalRE?", "Fechar o nostalRE?")),
            _ => None,
        }
    }
}

/// Where a command sits in the list
fn command_index(is: impl Fn(Command) -> bool) -> usize {
    COMMANDS.iter().position(|&c| is(c)).unwrap_or(0)
}

/// The screen a window key opens with the frameless HUD
fn screen_for(action: HotkeyAction) -> Option<Screen> {
    COMMANDS
        .iter()
        .find(|c| c.hotkey() == Some(action))
        .and_then(|c| c.screen())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Screen {
    #[default]
    Home,
    Items,
    Equipment,
    Skills,
    Status,
    GmTravel,
    Settings,
}

#[derive(Default)]
pub struct Menu {
    open: bool,
    screen: Screen,
    /// The picked command
    cursor: usize,
    /// A system command waiting for Enter
    confirm: Option<Command>,
    /// Each command's key, as the player bound it
    key_labels: Vec<(HotkeyAction, String)>,
    /// Where the player stands, set by the client before `build`
    pub place: Option<Place>,
    items: items::Items,
    equipment: equipment::Equipment,
    skills: skills::Skills,
    status: status::Status,
    settings: settings::Settings,
    travel: travel::Travel,
    textures: frame::Textures,
    /// A skill or item on its way onto the skill bar
    picker: Option<BarPicker>,
}

impl Menu {
    pub fn character_preview(&self) -> Option<((f32, f32), usize)> {
        (self.open && self.screen == Screen::Items && self.picker.is_none())
            .then(|| self.items.character_preview())
            .flatten()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
        self.set_screen(Screen::Home);
        self.confirm = None;
    }

    /// Another page: what the last one was asking goes away with it
    fn set_screen(&mut self, screen: Screen) {
        self.screen = screen;
        self.picker = None;
        self.items.back();
        self.equipment.back();
        self.settings.leave();
        self.travel.leave();
    }

    /// Settings is open and wants the client's values ([`Menu::set_settings`])
    pub fn wants_settings(&self) -> bool {
        self.open && self.screen == Screen::Settings && self.settings.needs_values()
    }

    /// What Settings shows, and what its resets go back to
    pub fn set_settings(&mut self, now: SettingsValues, defaults: SettingsValues) {
        self.settings.set_values(now, defaults);
    }

    /// Settings' Keys page waits for a key: the client hands it the next press instead of acting on it
    pub fn is_capturing_key(&self) -> bool {
        self.open && self.screen == Screen::Settings && self.settings.is_capturing()
    }

    /// Settings is taking text (a chat line): the keys are the text's, not the game's
    pub fn is_editing_text(&self) -> bool {
        self.open && self.screen == Screen::Settings && self.settings.is_editing()
    }

    /// The press the Keys page waited for; the bindings after it, when they changed
    pub fn capture_key(&mut self, chord: KeyChord) -> Option<GameEvent> {
        self.settings.capture(chord)
    }

    /// Escape while the Keys page waits: the key stays as it was
    pub fn cancel_key_capture(&mut self) {
        self.settings.cancel_capture();
    }

    /// Art the screens drew that the client has not been asked to load yet
    pub fn take_wanted_textures(&mut self) -> Vec<String> {
        self.textures.take()
    }

    /// The keys the commands show, from the player's bindings
    pub fn set_key_labels(&mut self, bindings: &KeyBindings) {
        self.key_labels = COMMANDS
            .iter()
            .filter_map(|c| c.hotkey())
            .filter_map(|a| Some((a, bindings.get(a)?.short_display())))
            .collect();
    }

    /// A window key with the frameless HUD: the menu's screen for that window, or the menu closed when that
    /// screen is the one open; false for a key whose window has no screen
    pub fn toggle_for(&mut self, action: HotkeyAction) -> bool {
        let Some(screen) = screen_for(action) else {
            return false;
        };
        if self.open && self.screen == screen {
            self.close();
        } else {
            self.open_on(action);
        }
        true
    }

    /// The menu open on the screen for `action`'s window (the level-up note's Status or Skills)
    pub fn open_on(&mut self, action: HotkeyAction) {
        if let Some(screen) = screen_for(action) {
            self.open();
            self.set_screen(screen);
            self.cursor = command_index(|c| c.screen() == Some(screen));
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.confirm = None;
        self.set_screen(Screen::Home);
    }

    fn pick(&mut self, command: Command, ctx: &BuildCtx, events: &mut Vec<GameEvent>) {
        self.cursor = command_index(|c| c == command);
        if command.confirm().is_some() {
            self.confirm = Some(command);
            self.picker = None;
            return;
        }
        self.confirm = None;
        if let Some(screen) = command.screen() {
            self.set_screen(screen);
            return;
        }
        if command == Command::Campaign {
            events.push(GameEvent::RequestSendChat {message:"@campanha".into()});
            self.close();return;
        }
        if command == Command::JobChanger {
            events.push(GameEvent::RequestSendChat { message: "@jobchanger".into() });
            self.close();
            return;
        }
        let actions: Vec<HotkeyAction> = match command {
            Command::Companions => [
                (ctx.homunculus.is_some(), HotkeyAction::ToggleHomunculus),
                (ctx.mercenary.is_some(), HotkeyAction::ToggleMercenary),
                (ctx.pet.gid.is_some(), HotkeyAction::TogglePet),
            ]
            .into_iter()
            .filter_map(|(has, action)| has.then_some(action))
            .collect(),
            _ => command.hotkey().into_iter().collect(),
        };
        // no companion: nothing to open, the menu stays
        if actions.is_empty() {
            return;
        }
        events.extend(
            actions.into_iter().map(|action| {
                GameEvent::OpenWindow(Box::new(GameEvent::RunHotkeyAction { action }))
            }),
        );
        self.close();
    }

    fn key_label(&self, command: Command) -> Option<&str> {
        let action = command.hotkey()?;
        self.key_labels
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, label)| label.as_str())
    }

    fn confirm_answered(&mut self, yes: bool, events: &mut Vec<GameEvent>) {
        if let Some(command) = self.confirm.take().filter(|_| yes) {
            events.push(match command {
                Command::ExitGame => GameEvent::QuitGame,
                _ => GameEvent::BackToCharacterSelect,
            });
            self.close();
        }
    }
}

impl Window for Menu {
    fn has_grf_textures(&self) -> bool {
        false
    }

    fn set_has_grf_textures(&mut self, _value: bool) {}

    fn grf_texture_paths() -> Vec<&'static str> {
        Vec::new()
    }
}

impl InGameWindow for Menu {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    /// Escape steps back: out of a question, a screen's inner list, a screen, then the menu
    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if self.confirm.take().is_some() || self.picker.take().is_some() {
            return Vec::new();
        }
        let left_inner = match self.screen {
            Screen::Equipment => self.equipment.back(),
            Screen::Items => self.items.back(),
            Screen::Settings => self.settings.back(),
            Screen::GmTravel => self.travel.back(),
            _ => false,
        };
        if left_inner {
            return Vec::new();
        }
        if self.screen == Screen::Home {
            self.close();
        } else {
            self.set_screen(Screen::Home);
        }
        Vec::new()
    }

    fn owns_keyboard(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let mut events = Vec::new();
        if !self.open {
            return events;
        }
        if self.screen == Screen::GmTravel
            || ((ui.ctx.pad_labels || cfg!(target_os = "horizon"))
                && matches!(self.screen, Screen::Home | Screen::Items))
        {
            return self.build_compact(ui, ctx);
        }
        let screen = Rect::new(0.0, 0.0, ui.ctx.screen_width, ui.ctx.screen_height);
        ui.ensure_in_z_order_with(MENU_WINDOW_ID, WindowOrder::Foreground);
        ui.enter_window(MENU_WINDOW_ID, screen);
        ui.interact(MENU_BG_ID, screen);

        let layout = frame::Layout::new(ui);
        frame::veil(ui);
        let mut keys = Keys::read(ui);

        let (crumb, info) = self.top_line(ctx);
        if frame::top_line(ui, &layout, crumb, &info) {
            self.set_screen(Screen::Home);
            self.confirm = None;
        }

        let active = self.screen != Screen::Home;
        let highlight = COMMANDS
            .iter()
            .position(|c| c.screen() == Some(self.screen))
            .unwrap_or(self.cursor);
        let labels = COMMANDS.map(|c| (c.label(), self.key_label(c)));
        if let Some(i) = frame::commands(ui, &layout, &labels, FIRST_SYSTEM, highlight, active) {
            self.pick(COMMANDS[i], ctx, &mut events);
            if !self.open {
                return events;
            }
        }

        let (help, hints) = if let Some(command) = self.confirm {
            (
                command.confirm().unwrap_or_default().to_string(),
                vec![frame::Hint::ENTER_YES, frame::Hint::ESC_NO],
            )
        } else if let Some(picker) = &self.picker {
            picker.footer(ctx)
        } else {
            match self.screen {
                Screen::Home => (
                    COMMANDS[self.cursor].help().to_string(),
                    vec![frame::Hint::MOVE, frame::Hint::OPEN, frame::Hint::CLOSE],
                ),
                Screen::Items => self.items.footer(ctx),
                Screen::Equipment => self.equipment.footer(ctx),
                Screen::Skills => self.skills.footer(ctx),
                Screen::Status => self.status.footer(),
                Screen::Settings => self.settings.footer(),
                Screen::GmTravel => (String::new(), vec![frame::Hint::BACK]),
            }
        };
        if let Some(key) = frame::footer(ui, &layout, &help, &hints) {
            keys.press(key);
        }
        if keys.back {
            events.extend(self.on_escape(ctx));
            if !self.open {
                return events;
            }
            // the page stepped back to is drawn this frame, without the press
            keys = Keys::default();
        }

        if self.confirm.is_some() {
            if keys.enter {
                self.confirm_answered(true, &mut events);
                return events;
            }
            // the page stays in sight under the question, the keys go to the question
            keys = Keys::default();
        }
        let picker_keys = keys;
        if self.picker.is_some() {
            keys = Keys::default();
        }
        let mut to_bar = None;

        let area = layout.content;
        match self.screen {
            Screen::Home => {
                if keys.up {
                    self.cursor = self.cursor.checked_sub(1).unwrap_or(COMMANDS.len() - 1);
                }
                if keys.down {
                    self.cursor = (self.cursor + 1) % COMMANDS.len();
                }
                if keys.enter {
                    self.pick(COMMANDS[self.cursor], ctx, &mut events);
                    if !self.open {
                        return events;
                    }
                }
                home::build(ui, ctx, area, self.place.as_ref());
            }
            Screen::Items => {
                let textures = &mut self.textures;
                let out = &mut to_bar;
                self.items
                    .build(ui, ctx, area, &keys, textures, out, &mut events)
            }
            Screen::Equipment => {
                let textures = &mut self.textures;
                if self
                    .equipment
                    .build(ui, ctx, area, &keys, textures, &mut events)
                {
                    self.close();
                    return events;
                }
            }
            Screen::Skills => {
                let (textures, out) = (&mut self.textures, &mut to_bar);
                self.skills
                    .build(ui, ctx, area, &keys, textures, out, &mut events)
            }
            Screen::Status => self.status.build(ui, ctx, area, &keys, &mut events),
            Screen::Settings => self.settings.build(ui, area, &keys, &mut events),
            Screen::GmTravel => {
                if self.travel.build(ui, ctx, area, &keys, &mut events) {
                    self.close();
                }
            }
        }
        if let Some(what) = to_bar {
            self.picker = Some(BarPicker::new(what));
        }
        if self.picker.is_some() || self.items.asking() {
            // over the page: the page under it takes no taps from the next frame on
            ui.ensure_in_z_order_with(MODAL_ID, WindowOrder::Foreground);
            ui.enter_window(MODAL_ID, area);
        }
        if let Some(picker) = &mut self.picker
            && picker.build(ui, ctx, area, &picker_keys, &mut self.textures, &mut events)
        {
            self.picker = None;
        }
        events
    }
}

impl Menu {
    /// The breadcrumb (a screen's name) and what the top line says on the right
    fn top_line(&self, ctx: &BuildCtx) -> (Option<&'static str>, String) {
        let c = &*ctx.character;
        let crumb = COMMANDS
            .iter()
            .find(|cmd| cmd.screen() == Some(self.screen))
            .map(|cmd| cmd.label())
            .filter(|_| self.screen != Screen::Home);
        let info = match self.screen {
            Screen::Home | Screen::Settings | Screen::GmTravel => self
                .place
                .as_ref()
                .map(|p| {
                    format!(
                        "{} · {}, {}",
                        crate::hud::place_name(ctx.data, &p.map),
                        p.x,
                        p.y
                    )
                })
                .unwrap_or_default(),
            Screen::Items => items::summary(c),
            Screen::Equipment => format!(
                "{} · {}",
                c.name,
                nostalre_game::character::job_class_name(ctx.job_class)
            ),
            Screen::Skills => self.skills.summary(c.skill_point),
            Screen::Status => {
                crate::pt_format!("Status points {}", "Pontos de atributo {}", c.status_point)
            }
        };
        (crumb, info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostalre_game::character::Character;
    use nostalre_game::data_table::DataTable;
    use nostalre_ui::context::UiContext;
    use nostalre_ui::state::StateCache;
    use nostalre_ui::test_support::test_frame;

    fn frame_with(menu: &mut Menu, set: impl FnOnce(&mut UiContext)) -> Vec<GameEvent> {
        frame_on(menu, &mut Character::new(), set)
    }

    fn frame_on(
        menu: &mut Menu,
        character: &mut Character,
        set: impl FnOnce(&mut UiContext),
    ) -> Vec<GameEvent> {
        let data = DataTable::new();
        let mut ctx = BuildCtx::test(character, &data);
        let mut state = StateCache::new();
        let mut ui_ctx = UiContext::new(1280.0, 720.0);
        set(&mut ui_ctx);
        let mut ui = test_frame(&mut ui_ctx, &mut state);
        menu.build(&mut ui, &mut ctx)
    }

    fn potions(count: i16) -> Character {
        use nostalre_game::item::{Item, ItemType};
        let mut character = Character::new();
        let mut item = Item::from_tables(501, ItemType::Healing, &DataTable::new());
        item.index = 2;
        item.count = count;
        item.is_identified = true;
        character.inventory.add_item(item);
        character
    }

    fn press(code: nostalre_ui::context::PhysicalKeyCode) -> impl FnOnce(&mut UiContext) {
        move |ui| ui.pressed_codes.push(code)
    }

    #[test]
    fn the_system_commands_follow_the_gap() {
        assert_eq!(COMMANDS[FIRST_SYSTEM], Command::Settings);
        assert!(
            COMMANDS[..FIRST_SYSTEM]
                .iter()
                .all(|c| c.confirm().is_none())
        );
    }

    #[test]
    fn a_window_key_opens_its_screen_and_closes_it_again() {
        let mut menu = Menu::default();
        assert!(menu.toggle_for(HotkeyAction::ToggleInventory));
        assert!(menu.is_open());
        assert_eq!(menu.screen, Screen::Items);
        assert!(menu.toggle_for(HotkeyAction::ToggleStatus));
        assert_eq!(menu.screen, Screen::Status);
        assert!(menu.toggle_for(HotkeyAction::ToggleStatus));
        assert!(!menu.is_open());
        assert!(!menu.toggle_for(HotkeyAction::ToggleQuest));
    }

    #[test]
    fn an_item_goes_on_the_slot_picked_for_it() {
        use nostalre_ui::context::PhysicalKeyCode as Code;
        let mut menu = Menu::default();
        menu.toggle_for(HotkeyAction::ToggleInventory);
        let mut character = potions(5);
        frame_on(&mut menu, &mut character, press(Code::KeyS));
        assert!(menu.picker.is_some());
        frame_on(&mut menu, &mut character, |ui| ui.key_right = true);
        let events = frame_on(&mut menu, &mut character, |ui| ui.key_enter = true);
        assert!(matches!(
            events.as_slice(),
            [GameEvent::RequestHotkeyChange {
                index: 1,
                is_skill: false,
                id: 501,
                ..
            }]
        ));
        assert!(menu.picker.is_none());
        assert!(menu.is_open());
    }

    #[test]
    fn the_skill_bar_question_goes_with_its_screen_and_alt_s_is_not_s() {
        use nostalre_ui::context::PhysicalKeyCode as Code;
        let mut menu = Menu::default();
        menu.toggle_for(HotkeyAction::ToggleInventory);
        let mut character = potions(5);
        frame_on(&mut menu, &mut character, |ui| {
            ui.pressed_codes.push(Code::KeyS);
            ui.alt_pressed = true;
        });
        assert!(menu.picker.is_none());
        frame_on(&mut menu, &mut character, press(Code::KeyS));
        assert!(menu.picker.is_some());
        menu.toggle_for(HotkeyAction::ToggleSkillTree);
        assert!(menu.picker.is_none());

        // nor does it outlive the menu, or wait under a question Enter would answer
        menu.toggle_for(HotkeyAction::ToggleInventory);
        frame_on(&mut menu, &mut character, press(Code::KeyS));
        menu.close();
        menu.open();
        assert!(menu.picker.is_none());
        menu.toggle_for(HotkeyAction::ToggleInventory);
        frame_on(&mut menu, &mut character, press(Code::KeyS));
        let data = DataTable::new();
        let ctx = BuildCtx::test(&mut character, &data);
        menu.pick(Command::ExitGame, &ctx, &mut Vec::new());
        assert!(menu.picker.is_none());
    }

    #[test]
    fn a_drop_question_for_an_item_gone_goes_away() {
        let mut menu = Menu::default();
        menu.toggle_for(HotkeyAction::ToggleInventory);
        let mut character = potions(30);
        frame_on(&mut menu, &mut character, |ui| ui.key_delete = true);
        assert!(menu.items.asking());
        character.inventory.remove_item(2);
        assert!(frame_on(&mut menu, &mut character, |ui| ui.key_enter = true).is_empty());
        assert!(!menu.items.asking());
    }

    #[test]
    fn a_stack_asks_how_many_to_drop() {
        let mut menu = Menu::default();
        menu.toggle_for(HotkeyAction::ToggleInventory);
        let mut character = potions(30);
        assert!(frame_on(&mut menu, &mut character, |ui| ui.key_delete = true).is_empty());
        frame_on(&mut menu, &mut character, |ui| {
            ui.typed_chars = vec!['1', '2']
        });
        let events = frame_on(&mut menu, &mut character, |ui| ui.key_enter = true);
        assert!(matches!(
            events.as_slice(),
            [GameEvent::RequestDropItem {
                index: 2,
                count: 12
            }]
        ));
        // a single item asks too
        let mut one = potions(1);
        assert!(frame_on(&mut menu, &mut one, |ui| ui.key_delete = true).is_empty());
        let events = frame_on(&mut menu, &mut one, |ui| ui.key_enter = true);
        assert!(matches!(
            events.as_slice(),
            [GameEvent::RequestDropItem { index: 2, count: 1 }]
        ));
    }

    #[test]
    fn enter_on_a_window_command_opens_that_window_and_closes_the_menu() {
        let mut menu = Menu::default();
        menu.open();
        menu.cursor = COMMANDS.iter().position(|&c| c == Command::Quests).unwrap();
        let events = frame_with(&mut menu, |ui| ui.key_enter = true);
        // opened, never closed: the client leaves an open window as it is
        let [GameEvent::OpenWindow(window)] = events.as_slice() else {
            panic!("{events:?}");
        };
        assert!(matches!(
            **window,
            GameEvent::RunHotkeyAction {
                action: HotkeyAction::ToggleQuest
            }
        ));
        assert!(!menu.is_open());
    }

    #[test]
    fn escape_steps_back_from_a_screen_then_closes() {
        let mut menu = Menu::default();
        menu.open();
        frame_with(&mut menu, |ui| ui.key_enter = true);
        assert_eq!(menu.screen, Screen::Items);
        let mut character = Character::new();
        let data = DataTable::new();
        let mut ctx = BuildCtx::test(&mut character, &data);
        menu.on_escape(&mut ctx);
        assert_eq!(menu.screen, Screen::Home);
        assert!(menu.is_open());
        menu.on_escape(&mut ctx);
        assert!(!menu.is_open());
    }

    /// The menu on Settings, its values loaded
    fn settings_open() -> Menu {
        let mut menu = Menu::default();
        menu.open();
        menu.cursor = command_index(|c| c == Command::Settings);
        frame_with(&mut menu, |ui| ui.key_enter = true);
        assert!(menu.wants_settings());
        menu.set_settings(SettingsValues::default(), SettingsValues::default());
        assert!(!menu.wants_settings());
        menu
    }

    #[test]
    fn a_setting_applies_at_once_and_the_menu_stays() {
        let mut menu = settings_open();
        let events = frame_with(&mut menu, |ui| ui.key_enter = true);
        assert!(matches!(
            events.as_slice(),
            [GameEvent::GraphicsSettingsChanged { persist: true, .. }]
        ));
        assert!(menu.is_open());
        // read again the next time the screen opens
        menu.set_screen(Screen::Home);
        menu.set_screen(Screen::Settings);
        assert!(menu.wants_settings());
    }

    #[test]
    fn the_keys_page_waits_for_a_key_and_escape_keeps_the_old_one() {
        use nostalre_ui::context::PhysicalKeyCode as Code;
        let mut menu = settings_open();
        for _ in 0..5 {
            frame_with(&mut menu, press(Code::PageDown));
        }
        frame_with(&mut menu, |ui| ui.key_enter = true);
        assert!(menu.is_capturing_key());
        let mut character = Character::new();
        let data = DataTable::new();
        let mut ctx = BuildCtx::test(&mut character, &data);
        menu.on_escape(&mut ctx);
        assert!(!menu.is_capturing_key());
        assert!(menu.is_open());

        frame_with(&mut menu, |ui| ui.key_enter = true);
        let alt_i = KeyChord::new("KeyI", true, false, false);
        let Some(GameEvent::KeyBindingsChanged(keys)) = menu.capture_key(alt_i.clone()) else {
            panic!("no change");
        };
        assert_eq!(keys.get(HotkeyAction::ToggleInventory), Some(&alt_i));
    }

    #[test]
    fn exit_asks_first() {
        let mut menu = Menu::default();
        menu.open();
        menu.cursor = COMMANDS
            .iter()
            .position(|&c| c == Command::ExitGame)
            .unwrap();
        assert!(frame_with(&mut menu, |ui| ui.key_enter = true).is_empty());
        let events = frame_with(&mut menu, |ui| ui.key_enter = true);
        assert!(matches!(events.as_slice(), [GameEvent::QuitGame]));
    }

    #[test]
    fn compact_inventory_assigns_the_modifier_slot_and_confirms_before_dropping() {
        use nostalre_ui::context::PhysicalKeyCode as Code;
        let mut menu = Menu::default();
        menu.open_on(HotkeyAction::ToggleInventory);
        let mut character = potions(20);
        let pad = |ui: &mut UiContext| {
            ui.pad_labels = true;
        };
        frame_on(&mut menu, &mut character, pad);
        assert!(menu.character_preview().is_some());
        frame_on(&mut menu, &mut character, |ui| {
            pad(ui);
            ui.pressed_codes.push(Code::KeyS);
        });
        assert!(menu.picker.is_some());
        assert!(menu.character_preview().is_none());
        frame_on(&mut menu, &mut character, |ui| {
            pad(ui);
            ui.key_down = true;
        });
        frame_on(&mut menu, &mut character, |ui| {
            pad(ui);
            ui.pressed_codes.push(Code::PageDown);
        });
        let events = frame_on(&mut menu, &mut character, |ui| {
            pad(ui);
            ui.key_enter = true;
        });
        assert!(matches!(
            events.as_slice(),
            [GameEvent::RequestHotkeyChange {
                index: 16,
                is_skill: false,
                id: 501,
                ..
            }]
        ));
        let events = frame_on(&mut menu, &mut character, |ui| {
            pad(ui);
            ui.key_delete = true;
        });
        assert!(events.is_empty());
        assert!(menu.items.asking());
        let events = frame_on(&mut menu, &mut character, |ui| {
            pad(ui);
            ui.key_enter = true;
        });
        assert!(matches!(
            events.as_slice(),
            [GameEvent::RequestDropItem {
                index: 2,
                count: 20
            }]
        ));
    }

    #[test]
    fn job_changer_opens_the_server_dialog_and_closes_the_controller_menu() {
        let mut menu = Menu::default();
        menu.open();
        menu.cursor = command_index(|c| c == Command::JobChanger);
        let events = frame_with(&mut menu, |ui| {
            ui.pad_labels = true;
            ui.key_enter = true;
        });
        assert!(matches!(events.as_slice(), [GameEvent::RequestSendChat { message }] if message == "@jobchanger"));
        assert!(!menu.is_open());
    }

    #[test]
    fn gm_travel_opens_from_home_and_closes_after_one_server_command() {
        let mut menu = Menu::default();
        menu.open();
        menu.cursor = command_index(|c| c == Command::GmTravel);
        frame_with(&mut menu, |ui| {
            ui.pad_labels = true;
            ui.key_enter = true;
        });
        assert_eq!(menu.screen, Screen::GmTravel);
        let steps=travel_catalog::GROUPS.iter().filter(|g|g.category==0).position(|g|g.label=="Alberta").unwrap();
        for _ in 0..steps {
            frame_with(&mut menu, |ui| { ui.pad_labels=true; ui.key_down=true; });
        }
        let events = frame_with(&mut menu, |ui| {
            ui.pad_labels = true;
            ui.key_enter = true;
        });
        assert!(
            matches!(events.as_slice(),[GameEvent::RequestSendChat{message}] if message=="@go 4")
        );
        assert!(!menu.is_open());
    }
}
