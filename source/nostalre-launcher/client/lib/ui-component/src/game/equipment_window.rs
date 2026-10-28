use super::damage_indicator_tab::{self, DamageIndicatorTab, DamageTabAction};
use super::inventory_window::INV_WINDOW_ID;
use super::swap_panel::{self, SwapPanelAction, SwapView};
use super::title_list::TitleList;
use crate::helper::tab_control::{self, TAB_H, Tab, TabControl};
use crate::helper::window_chrome::{
    SYS_BASE_OFF_TEX, SYS_BASE_ON_TEX, TITLEBAR_TEX, draw_footer, draw_sys_button,
    draw_textured_quad, draw_titlebar, fill, text_color,
};
use crate::{BuildCtx, InGameWindow, Window};
use nostalre_game::damage_number::DamageDirection;
use nostalre_game::data_table::DataTable;
use nostalre_game::display_name::format_equipment_display_name_colored;
use nostalre_game::event::{GameEvent, SelfConfigKind};
use nostalre_game::inventory::{EquipmentLocation, InventoryData};
use nostalre_game::item::Item;
use nostalre_game::sprite_path::OPTION_REMOVABLE_MASK;
use nostalre_game::title::Titles;
use nostalre_ui::draw::{self, DrawCall, TextureRef};
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

const ITEM_INVERT_TEX: &str = nostalre_resources::ui::basic::ITEM_INVERT;
const BTN_OFF_TEX: &str = nostalre_resources::ui::basic::BTN_OFF;
/// kRO's "Unequip All" button: as drawn, under the mouse, greyed (2025 rig, 2026-10-19)
const UNEQUIP_ALL_TEX: [&str; 3] = [
    nostalre_resources::ui::basic::BTN_E_OFF_A,
    nostalre_resources::ui::basic::BTN_E_OFF_B,
    nostalre_resources::ui::basic::BTN_E_OFF_C,
];
const UNEQUIP_ALL_SIZE: (f32, f32) = (40.0, 20.0);
/// Where kRO puts it in the window (0x8a5854: 0xeb, 0xa7)
const UNEQUIP_ALL_POS: (f32, f32) = (235.0, 167.0);
/// The strip under the slots that holds the button: the bottom of `equipwin_bg2.bmp` (157 high, 130 of slots)
const FOOTER_H: f32 = 27.0;

/// A footer button with a normal and an over-and-press bitmap (Swap Equipment, Equipment Attributes); true when
/// clicked
struct FooterButton {
    id: WidgetId,
    pos: (f32, f32),
    /// A bitmap button takes its bitmap's size
    size: (f32, f32),
    tex: [&'static str; 2],
    tooltip: &'static str,
    /// What the button says without the archive's art
    fallback: &'static str,
}

const SWAP_BUTTON: FooterButton = FooterButton {
    id: EQ_SWAP_BTN_ID,
    pos: SWAP_BTN_POS,
    size: (40.0, 20.0),
    tex: SWAP_BTN_TEX,
    tooltip: "Swap Equipment",
    fallback: "Swap",
};

const ATTRIBUTES_BUTTON: FooterButton = FooterButton {
    id: EQ_ATTRIBUTES_BTN_ID,
    pos: ATTRIBUTES_BTN_POS,
    size: (23.0, 20.0),
    tex: ATTRIBUTES_BTN_TEX,
    tooltip: "Equipment Attributes",
    fallback: "Attr",
};

fn footer_button(
    ui: &mut UiFrame,
    button: &FooterButton,
    win_x: f32,
    win_y: f32,
    grf: bool,
) -> bool {
    let (w, h) = button.size;
    let rect = Rect::new(win_x + button.pos.0, win_y + button.pos.1, w, h);
    let response = ui.interact(button.id, rect);
    if response.hovered() {
        ui.any_interactive_hovered = true;
        ui.tooltip(rect.x, rect.y + h + 2.0, button.tooltip);
    }
    if grf {
        let tex = button.tex[usize::from(response.hovered())];
        draw_textured_quad(ui, rect.x, rect.y, w, h, tex);
    } else {
        fill(ui, rect.x, rect.y, w, h, [0.55, 0.65, 0.85, 1.0]);
        ui.text(
            rect.x + 4.0,
            rect.y + 14.0,
            button.fallback,
            [0.0, 0.0, 0.0, 1.0],
        );
    }
    response.clicked()
}

/// An item in a slot: its icon, and its name wrapped from its first baseline `name` down, in kRO's 70-wide box
/// (0x897d60, the swap window's 0x7f8810); `alpha` below 1 for a swap slot's stand-in, the item worn there
pub(super) fn draw_slot_item(
    ui: &mut UiFrame,
    item: &Item,
    display_name: &str,
    icon_rect: Rect,
    name: Option<(f32, f32)>,
    text_color: [f32; 4],
    alpha: f32,
) {
    if let Some(icon_path) = item.icon_path() {
        let (v, idx) = draw::quad_vertices(
            icon_rect.x,
            icon_rect.y,
            icon_rect.w,
            icon_rect.h,
            [1.0, 1.0, 1.0, alpha],
        );
        ui.draw_calls.push(DrawCall {
            vertices: v.into(),
            indices: idx.into(),
            texture: TextureRef::Named(icon_path.into()),
        });
    }
    let Some((name_x, first_baseline)) = name else {
        return;
    };
    let lines = draw::colored_word_wrap(display_name, NAME_W, |t| ui.atlas.measure_text(t), true);
    let color = [
        text_color[0],
        text_color[1],
        text_color[2],
        text_color[3] * alpha,
    ];
    for (j, line) in lines.iter().enumerate() {
        let baseline = first_baseline + j as f32 * NAME_LINE_STEP;
        ui.colored_text(name_x, baseline, line, color);
    }
}

pub const EQ_WINDOW_ID: WidgetId = WidgetId(900);
const EQ_CLOSE_BTN_ID: WidgetId = WidgetId(901);
const EQ_MINI_BTN_ID: WidgetId = WidgetId(902);
const EQ_REMOVE_OPTION_BTN_ID: WidgetId = WidgetId(903);
const EQ_CART_SLOT_ID: WidgetId = WidgetId(904);
const EQ_UNEQUIP_ALL_ID: WidgetId = WidgetId(905);
const EQ_TAB_BASE_ID: u32 = 906;
const EQ_CONFIG_CHECK_ID: WidgetId = WidgetId(925);
/// The title list's scroll bar and rows (up to 7 visible)
const EQ_TITLE_LIST_BASE_ID: u32 = 930;
/// The Damage Indicator tab's cells, radios and buttons
const EQ_DAMAGE_TAB_BASE_ID: u32 = 945;
/// The swap panel's slots, CHANGE and Cancel; the footer's Swap Equipment button
const EQ_SWAP_PANEL_BASE_ID: u32 = 965;
const EQ_SWAP_BTN_ID: WidgetId = WidgetId(990);
/// The drag source a set item carries out of the swap panel, to the inventory or onto the window
pub const EQ_SWAP_DRAG_ID: WidgetId = WidgetId(960);
/// kRO's Swap Equipment button beside Unequip All (0x8a5832: 0xc1, 0xa7; `btn_e_change_a`, `_b` for over and
/// press)
const SWAP_BTN_POS: (f32, f32) = (193.0, 167.0);
const SWAP_BTN_TEX: [&str; 2] = [
    nostalre_resources::ui::basic::BTN_E_CHANGE_A,
    nostalre_resources::ui::basic::BTN_E_CHANGE_B,
];
/// kRO's Equipment Attributes button left of it (0x246; (0xa7, 0xa7) static data; `btn_e_ability_a`, `_b` for
/// over and press), on General and Costume
const EQ_ATTRIBUTES_BTN_ID: WidgetId = WidgetId(991);
const ATTRIBUTES_BTN_POS: (f32, f32) = (167.0, 167.0);
const ATTRIBUTES_BTN_TEX: [&str; 2] = [
    nostalre_resources::ui::basic::BTN_E_ABILITY_A,
    nostalre_resources::ui::basic::BTN_E_ABILITY_B,
];
/// The Title tab in the window (0x8b3325, the list at 0x8a56ca; `+0xf4` = the tab row's 20): the white panel, the
/// worn title's line (centred, 13 high), the list's grey frame and the list
const TITLE_PANEL: (f32, f32, f32, f32) = (1.0, 35.0, 278.0, 131.0);
const TITLE_WORN: (f32, f32, f32) = (1.0, 40.0, 278.0);
const TITLE_LIST_FRAME: (f32, f32, f32, f32) = (2.0, 58.0, 276.0, 107.0);
const TITLE_LIST_FRAME_COLOR: [f32; 4] = [100.0 / 255.0, 100.0 / 255.0, 100.0 / 255.0, 1.0];
const TITLE_LIST: (f32, f32, f32, f32) = (3.0, 59.0, 274.0, 105.0);
/// Show Equip on General, View Costumes on Costume: one place, one id (0x8a4bed, 0x8a4c87); the box and its label
/// where the rig shows them (2026-10-20)
const CONFIG_CHECK_RECT: (f32, f32, f32, f32) = (10.0, 173.0, 80.0, 10.0);
const CONFIG_CHECK_BOX: f32 = 10.0;
const CONFIG_LABEL_X: f32 = 23.0;
const CONFIG_LABEL_BASELINE: f32 = 181.0;
const EQ_SLOT_BASE_ID: u32 = 910;

const TITLE_H: f32 = 17.0;
/// The row of tabs under the title bar: the slots start this much lower (the exe's UIEquipWnd +0xf4, 0x14)
const TAB_ROW_H: f32 = 20.0;
/// The Costume tab's art lies over the General one's from 3 pixels higher (0x897dca: +0xf4 + 0xe, against +0x11)
const COSTUME_BG_RISE: f32 = 3.0;
const WIN_W: f32 = 280.0;
const CONTENT_H: f32 = 130.0;
const MINI_BTN_SIZE: f32 = 11.0;

const SIDE_COL_W: f32 = 115.0;
const CENTER_COL_W: f32 = 50.0;
const ICON_SIZE: f32 = 24.0;
const SLOT_ROWS: usize = 5;
/// A slot's icon and name, from the window's left edge, left column then right (0x897d60: icons at 6 and 0xfb,
/// names at 0x1c + 5 and 0xa7 + 5, both left-aligned in a box 0x46 wide)
const ICON_X: [f32; 2] = [6.0, 251.0];
const NAME_X: [f32; 2] = [33.0, 172.0];
const NAME_W: f32 = 70.0;
/// The name's first baseline under the slot's top, and the step to the next line (the rig, 2026-10-20: kRO's
/// 12-pixel lines; the exe passes the first line's middle, 0x19 under the tabs for the top row)
const NAME_BASELINE: f32 = 10.0;
const NAME_LINE_STEP: f32 = 12.0;
const REMOVE_OPTION_BTN_SIZE: f32 = 36.0;

/// The own equipment window's art (0x8b3190; `equipwin_bg.bmp` is another player's)
const EQUIP_BG_TEX: &str = nostalre_resources::ui::basic::EQUIPWIN_BG2;
/// The Costume tab's slots (0x897d9f)
const COSTUME_BG_TEX: &str = nostalre_resources::ui::basic::EQUIPWIN_SPECIAL;
const CLOSE_OFF_TEX: &str = nostalre_resources::ui::basic::SYS_CLOSE_OFF;
const CLOSE_ON_TEX: &str = nostalre_resources::ui::basic::SYS_CLOSE_ON;
const MINI_OFF_TEX: &str = nostalre_resources::ui::basic::SYS_MINI_OFF;
const MINI_ON_TEX: &str = nostalre_resources::ui::basic::SYS_MINI_ON;

pub(super) struct EquipSlot {
    pub(super) location: EquipmentLocation,
    pub(super) col: u8, // 0=left, 1=right, 2=center
    pub(super) row: u8,
}

const fn slot(location: EquipmentLocation, col: u8, row: u8) -> EquipSlot {
    EquipSlot { location, col, row }
}

const GENERAL_SLOTS: &[EquipSlot] = &[
    slot(EquipmentLocation::HeadTop, 0, 0),
    slot(EquipmentLocation::HeadLow, 0, 1),
    slot(EquipmentLocation::HandRight, 0, 2),
    slot(EquipmentLocation::Garment, 0, 3),
    slot(EquipmentLocation::AccessoryLeft, 0, 4),
    slot(EquipmentLocation::HeadMid, 1, 0),
    slot(EquipmentLocation::Armor, 1, 1),
    slot(EquipmentLocation::HandLeft, 1, 2),
    slot(EquipmentLocation::Shoes, 1, 3),
    slot(EquipmentLocation::AccessoryRight, 1, 4),
    slot(EquipmentLocation::Ammo, 2, 0),
];

/// The second tab: costumes and shadow gear in one, each location in the place of the general one kRO maps it to
/// (0xd55850, then the bit number: 0x897d60)
const COSTUME_SLOTS: &[EquipSlot] = &[
    slot(EquipmentLocation::CostumeHeadTop, 0, 0),
    slot(EquipmentLocation::CostumeHeadLow, 0, 1),
    slot(EquipmentLocation::ShadowWeapon, 0, 2),
    slot(EquipmentLocation::CostumeGarment, 0, 3),
    slot(EquipmentLocation::ShadowAccR, 0, 4),
    slot(EquipmentLocation::CostumeHeadMid, 1, 0),
    slot(EquipmentLocation::ShadowArmor, 1, 1),
    slot(EquipmentLocation::ShadowShield, 1, 2),
    slot(EquipmentLocation::ShadowShoes, 1, 3),
    slot(EquipmentLocation::ShadowAccL, 1, 4),
];

/// The window's tabs in kRO's order (0x8a565a..0x8a5980); Title and Damage Indicator are drawn but not built yet
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EquipTab {
    General,
    Costume,
    Title,
    DamageIndicator,
}

impl EquipTab {
    const ALL: [Self; 4] = [
        Self::General,
        Self::Costume,
        Self::Title,
        Self::DamageIndicator,
    ];
    const TABS: [Tab; 4] = [
        Tab {
            label: "General",
            enabled: true,
        },
        Tab {
            label: crate::pt!("Costume", "Visual"),
            enabled: true,
        },
        Tab {
            label: "Title",
            enabled: true,
        },
        Tab {
            label: "Damage Indicator",
            enabled: true,
        },
    ];

    fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    fn index(self) -> usize {
        self as usize
    }

    fn slots(self) -> &'static [EquipSlot] {
        match self {
            Self::General => GENERAL_SLOTS,
            Self::Costume => COSTUME_SLOTS,
            Self::Title | Self::DamageIndicator => &[],
        }
    }

    /// What its Unequip All takes off (0x8c05b4): the ten general slots and the ammunition, or the costumes and the
    /// shadow gear (with 0x4000, rAthena's unused `EQP_COSTUME_FLOOR`)
    fn unequip_all_mask(self) -> u32 {
        match self {
            Self::General => 0x83ff,
            Self::Costume => 0x3f7c00,
            Self::Title | Self::DamageIndicator => 0,
        }
    }

    /// The locations of its ten slots: Unequip All is greyed while none is worn (0x8c0604, 0xd9aa70 / 0xd9a960;
    /// ammunition alone does not count)
    fn slot_mask(self) -> u32 {
        match self {
            Self::General => 0x3ff,
            Self::Costume => 0x3f7c00,
            Self::Title | Self::DamageIndicator => 0,
        }
    }
}

pub struct EquipmentWindow {
    pub has_grf_textures: bool,
    pub open: bool,
    minimized: bool,
    bg_size: (f32, f32),
    costume_bg_size: (f32, f32),
    character_center: Option<[f32; 2]>,
    paperdoll_insert_index: Option<usize>,
    cart_slot_center: Option<[f32; 2]>,
    /// An "Unequip All" sent and not answered: kRO greys the button meanwhile (0x8c05f8)
    unequip_all_pending: bool,
    tab: EquipTab,
    tabs: TabControl,
    /// The swap panel drawn under the slots this frame (`EquipSwitch::panel_open` on General or Costume)
    swap_shown: bool,
    title_list: TitleList,
    damage_tab: DamageIndicatorTab,
    /// The damage skin and direction in use, the Damage Indicator tab's starting picks
    damage_skin: u32,
    damage_direction: DamageDirection,
    /// The server's settings the two check boxes show (0x02d9): a click asks to change one and the box follows the
    /// answer, as kRO's does (0xcbb0f0)
    show_equip: bool,
    hide_costumes: bool,
}

impl Default for EquipmentWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl EquipmentWindow {
    pub fn new() -> Self {
        Self {
            has_grf_textures: false,
            open: false,
            minimized: false,
            bg_size: (0.0, 0.0),
            costume_bg_size: (0.0, 0.0),
            character_center: None,
            paperdoll_insert_index: None,
            cart_slot_center: None,
            unequip_all_pending: false,
            tab: EquipTab::General,
            tabs: TabControl::default(),
            swap_shown: false,
            title_list: TitleList::default(),
            damage_tab: DamageIndicatorTab::default(),
            damage_skin: 0,
            damage_direction: DamageDirection::Default,
            show_equip: false,
            hide_costumes: false,
        }
    }

    /// The server's "show my equipment to others" and "hide costumes" settings
    pub fn set_self_config(&mut self, show_equip: bool, hide_costumes: bool) {
        self.show_equip = show_equip;
        self.hide_costumes = hide_costumes;
    }

    /// The damage skin and direction in use (the config's, or the last Apply's)
    pub fn set_damage_indicator(&mut self, skin: u32, direction: DamageDirection) {
        self.damage_skin = skin;
        self.damage_direction = direction;
        self.damage_tab.reset(skin, direction);
    }

    /// The server answered an "Unequip All" (0x0bae)
    pub fn unequip_all_answered(&mut self) {
        self.unequip_all_pending = false;
    }

    pub fn character_center(&self) -> Option<[f32; 2]> {
        self.character_center
    }

    pub fn paperdoll_insert_index(&self) -> Option<usize> {
        self.paperdoll_insert_index
    }

    pub fn cart_slot_center(&self) -> Option<[f32; 2]> {
        self.cart_slot_center
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_visible(&self) -> bool {
        self.open && !self.minimized
    }

    pub fn is_minimized(&self) -> bool {
        self.minimized
    }

    pub fn set_minimized(&mut self, value: bool) {
        self.minimized = value;
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
}

impl EquipmentWindow {
    /// The window's height: down to the swap panel's foot while it shows, else the footer's (0x8c0432)
    fn full_h(&self) -> f32 {
        if self.swap_shown {
            swap_panel::PANEL_Y + swap_panel::PANEL_SIZE.1
        } else {
            TITLE_H + TAB_ROW_H + self.body_h()
        }
    }

    /// The slots and the footer strip under the tabs: the art's height, or ours without it
    fn body_h(&self) -> f32 {
        if self.bg_size.1 > 0.0 {
            self.bg_size.1
        } else {
            CONTENT_H + FOOTER_H
        }
    }

    /// The Title tab: the worn title on white, centred, over the list of the character's titles in a grey frame
    /// (0x8b3325; list 0x8a56ca); returns the title a double-click asks to wear
    fn title_tab(
        &mut self,
        ui: &mut UiFrame,
        win_x: f32,
        win_y: f32,
        titles: &Titles,
        data: &DataTable,
    ) -> Option<i32> {
        let (x, y, w, h) = TITLE_PANEL;
        fill(ui, win_x + x, win_y + y, w, h, [1.0, 1.0, 1.0, 1.0]);
        let (x, y, w, h) = TITLE_LIST_FRAME;
        for (fx, fy, fw, fh) in [
            (x, y, w, 1.0),
            (x, y + h - 1.0, w, 1.0),
            (x, y, 1.0, h),
            (x + w - 1.0, y, 1.0, h),
        ] {
            fill(ui, win_x + fx, win_y + fy, fw, fh, TITLE_LIST_FRAME_COLOR);
        }
        let worn = data.titles.name(titles.current);
        let (x, y, w) = TITLE_WORN;
        let worn_x = win_x + x + ((w - ui.atlas.measure_text(worn)) * 0.5).floor();
        ui.text(
            worn_x,
            win_y + y + ui.atlas.ascent,
            worn,
            [0.0, 0.0, 0.0, 1.0],
        );
        let (x, y, w, h) = TITLE_LIST;
        self.title_list.build(
            ui,
            EQ_TITLE_LIST_BASE_ID,
            Rect::new(win_x + x, win_y + y, w, h),
            titles.owned(),
            |id| data.titles.name(id),
        )
    }

    /// The tab's check box: Show Equip asks the server to show the equipment to others (0x02d8 type 0), View
    /// Costumes asks it to show costumes (type 5 off; on hides them)
    fn config_check(
        &mut self,
        ui: &mut UiFrame,
        win_x: f32,
        win_y: f32,
        grf: bool,
    ) -> Option<GameEvent> {
        // The setting, its label, whether the box is ticked, and what a click asks for
        let (kind, label, checked, enabled) = match self.tab {
            EquipTab::General => (
                SelfConfigKind::OpenEquipmentWindow,
                "Show Equip",
                self.show_equip,
                !self.show_equip,
            ),
            EquipTab::Costume => (
                SelfConfigKind::HideCostumes,
                "View Costumes",
                !self.hide_costumes,
                !self.hide_costumes,
            ),
            EquipTab::Title | EquipTab::DamageIndicator => return None,
        };
        let (x, y, w, h) = CONFIG_CHECK_RECT;
        let box_rect = Rect::new(win_x + x, win_y + y, CONFIG_CHECK_BOX, CONFIG_CHECK_BOX);
        ui.checkbox_display(box_rect, checked, &crate::helper::CHECKBOX);
        ui.text(
            win_x + CONFIG_LABEL_X,
            win_y + CONFIG_LABEL_BASELINE,
            label,
            text_color(grf),
        );
        let response = ui.interact(EQ_CONFIG_CHECK_ID, Rect::new(win_x + x, win_y + y, w, h));
        if response.hovered() {
            ui.any_interactive_hovered = true;
        }
        response
            .clicked()
            .then_some(GameEvent::RequestSetConfig { kind, enabled })
    }

    /// kRO's button at the footer's right end (`right`, the window's right edge): greyed while nothing is worn
    /// that it takes off or while its last request waits for the answer; a click asks for all of it
    fn unequip_all_button(
        &mut self,
        ui: &mut UiFrame,
        win_x: f32,
        win_y: f32,
        worn: bool,
        grf: bool,
    ) -> Option<GameEvent> {
        let (w, h) = UNEQUIP_ALL_SIZE;
        let rect = Rect::new(win_x + UNEQUIP_ALL_POS.0, win_y + UNEQUIP_ALL_POS.1, w, h);
        let enabled = worn && !self.unequip_all_pending;
        let response = ui.interact(EQ_UNEQUIP_ALL_ID, rect);
        let hovered = enabled && response.hovered();
        if hovered {
            ui.any_interactive_hovered = true;
        }
        if grf {
            let state = match (enabled, hovered) {
                (false, _) => 2,
                (true, true) => 1,
                (true, false) => 0,
            };
            draw_textured_quad(ui, rect.x, rect.y, w, h, UNEQUIP_ALL_TEX[state]);
        } else {
            let color = if enabled {
                [0.55, 0.65, 0.85, 1.0]
            } else {
                [0.6, 0.6, 0.6, 1.0]
            };
            let (v, idx) = draw::quad_vertices(rect.x, rect.y, w, h, color);
            ui.draw_calls.push(DrawCall {
                vertices: v.into(),
                indices: idx.into(),
                texture: TextureRef::White,
            });
            ui.text(rect.x + 4.0, rect.y + 14.0, "All", [0.0, 0.0, 0.0, 1.0]);
        }
        if response.hovered() {
            ui.tooltip(rect.x, rect.y + h + 2.0, "Unequip All");
        }
        if enabled && response.clicked() {
            self.unequip_all_pending = true;
            return Some(GameEvent::RequestUnequipAll {
                location: self.tab.unequip_all_mask(),
            });
        }
        None
    }
}

impl Window for EquipmentWindow {
    fn has_grf_textures(&self) -> bool {
        self.has_grf_textures
    }
    fn set_has_grf_textures(&mut self, value: bool) {
        self.has_grf_textures = value;
    }

    fn set_texture_sizes(&mut self, size_fn: &dyn Fn(&str) -> Option<(u32, u32)>) {
        if let Some((w, h)) = size_fn(EQUIP_BG_TEX) {
            self.bg_size = (w as f32, h as f32);
        }
        if let Some((w, h)) = size_fn(COSTUME_BG_TEX) {
            self.costume_bg_size = (w as f32, h as f32);
        }
    }

    fn window_size(&self) -> (f32, f32) {
        (WIN_W, self.full_h())
    }

    fn grf_texture_paths() -> Vec<&'static str> {
        vec![
            TITLEBAR_TEX,
            EQUIP_BG_TEX,
            COSTUME_BG_TEX,
            ITEM_INVERT_TEX,
            BTN_OFF_TEX,
            SYS_BASE_OFF_TEX,
            SYS_BASE_ON_TEX,
            CLOSE_OFF_TEX,
            CLOSE_ON_TEX,
            MINI_OFF_TEX,
            MINI_ON_TEX,
            crate::helper::window_chrome::FOOTER_TEX,
            UNEQUIP_ALL_TEX[0],
            UNEQUIP_ALL_TEX[1],
            UNEQUIP_ALL_TEX[2],
        ]
        .into_iter()
        .chain(damage_indicator_tab::TEXTURES)
        .chain(swap_panel::TEXTURES)
        .chain(SWAP_BTN_TEX)
        .chain(ATTRIBUTES_BTN_TEX)
        .collect()
    }
}

impl InGameWindow for EquipmentWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.open = false;
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let character = &mut *ctx.character;
        let data = ctx.data;
        let inventory = &character.inventory;
        let producers = &character.char_names;
        self.character_center = None;
        self.paperdoll_insert_index = None;
        self.cart_slot_center = None;

        if !self.open {
            // kRO makes the window anew on each opening, on its first tab with the damage picks as in use (0x8c0145,
            // the ctor 0x88d740)
            self.tab = EquipTab::General;
            self.damage_tab
                .reset(self.damage_skin, self.damage_direction);
            return Vec::new();
        }

        let mut events = Vec::new();

        let prev_grf = ui.has_grf_textures;
        ui.has_grf_textures = self.has_grf_textures;

        let grf = self.has_grf_textures;
        let text_color = text_color(grf);

        let win_w = WIN_W;
        let content_h = CONTENT_H;
        // The swap panel stays open across tabs and openings but shows on General and Costume only (0x8bf98e)
        self.swap_shown = character.equip_switch.panel_open
            && matches!(self.tab, EquipTab::General | EquipTab::Costume);
        let win_h = if self.minimized {
            TITLE_H
        } else {
            self.full_h()
        };

        let default_x = 0.0;
        let default_y = 210.0;
        let win = ui.window_at(EQ_WINDOW_ID, win_w, win_h, TITLE_H, default_x, default_y);

        let win_rect = Rect::new(win.x, win.y, win_w, win_h);
        ui.interact(EQ_WINDOW_ID, win_rect);

        draw_titlebar(ui, win.x, win.y, win_w, TITLE_H, grf);
        ui.text(
            win.x + (17.0),
            win.y + (TITLE_H) - (3.0),
            crate::pt!("Equipment", "Equipamentos"),
            text_color,
        );

        let btn_size = MINI_BTN_SIZE;
        let mini_rect = Rect::new(
            win.x + win_w - btn_size * 2.0 - (6.0),
            win.y + ((TITLE_H) - btn_size) / 2.0,
            btn_size,
            btn_size,
        );
        let mini_resp = ui.interact(EQ_MINI_BTN_ID, mini_rect);
        if mini_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(
            ui,
            mini_rect,
            (btn_size, btn_size),
            mini_resp.hovered(),
            grf,
            MINI_ON_TEX,
            MINI_OFF_TEX,
            Some('_'),
        );
        if mini_resp.clicked() {
            self.minimized = !self.minimized;
        }

        let close_rect = Rect::new(
            win.x + win_w - btn_size - (3.0),
            win.y + ((TITLE_H) - btn_size) / 2.0,
            btn_size,
            btn_size,
        );
        let close_resp = ui.interact(EQ_CLOSE_BTN_ID, close_rect);
        if close_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(
            ui,
            close_rect,
            (btn_size, btn_size),
            close_resp.hovered(),
            grf,
            CLOSE_ON_TEX,
            CLOSE_OFF_TEX,
            Some('x'),
        );
        if close_resp.clicked() {
            self.open = false;
            ui.has_grf_textures = prev_grf;
            return events;
        }

        if self.minimized {
            ui.has_grf_textures = prev_grf;
            return events;
        }

        let tab_y = win.y + TITLE_H;
        if grf {
            fill(ui, win.x, tab_y, win_w, TAB_ROW_H, [1.0, 1.0, 1.0, 1.0]);
        } else {
            crate::helper::fallback::window_body(ui, win.x, tab_y, win_w, TAB_ROW_H);
        }
        let (clicked_tab, tabs_w) = self.tabs.build(
            ui,
            EQ_TAB_BASE_ID,
            win.x,
            tab_y,
            win_w,
            &EquipTab::TABS,
            self.tab.index(),
        );
        if let Some(tab) = clicked_tab.and_then(EquipTab::from_index) {
            self.tab = tab;
            self.title_list.reset();
        }
        // The line under the tabs runs on to the window's edge (0x8b3239)
        fill(
            ui,
            win.x + tabs_w,
            tab_y + TAB_H - 1.0,
            win_w - tabs_w,
            1.0,
            tab_control::EDGE,
        );

        let content_y = tab_y + TAB_ROW_H;
        if grf {
            draw_textured_quad(ui, win.x, content_y, win_w, self.bg_size.1, EQUIP_BG_TEX);
            if self.tab == EquipTab::Costume {
                let (w, h) = self.costume_bg_size;
                draw_textured_quad(ui, win.x, content_y - COSTUME_BG_RISE, w, h, COSTUME_BG_TEX);
            }
        } else {
            crate::helper::fallback::window_body(ui, win.x, content_y, win_w, content_h);
        }
        if self.tab == EquipTab::Title {
            // No paperdoll, slots or footer controls on this tab (0x8b3190, 0x8bf825)
            if let Some(title_id) = self.title_tab(ui, win.x, win.y, &character.titles, data) {
                events.push(GameEvent::RequestChangeTitle { title_id });
            }
            ui.has_grf_textures = prev_grf;
            return events;
        }
        if self.tab == EquipTab::DamageIndicator {
            // As the Title tab: none of the equipment's parts (0x8bf8e1)
            match self.damage_tab.build(
                ui,
                EQ_DAMAGE_TAB_BASE_ID,
                win.x,
                win.y,
                &data.damage_skins,
                self.damage_skin,
                grf,
            ) {
                Some(DamageTabAction::Apply { skin, direction }) => {
                    events.push(GameEvent::ApplyDamageIndicator { skin, direction });
                }
                // Close shuts the window (0xc9)
                Some(DamageTabAction::Close) => self.open = false,
                None => {}
            }
            ui.has_grf_textures = prev_grf;
            return events;
        }

        let slot_h = content_h / SLOT_ROWS as f32;
        let character_x = win.x + SIDE_COL_W + CENTER_COL_W / 2.0;
        let character_y = content_y + content_h - slot_h;
        self.character_center = Some([character_x, character_y]);
        self.paperdoll_insert_index = Some(ui.draw_calls.len());

        let general = self.tab == EquipTab::General;
        if general
            && character.cart_design.is_none()
            && character.effect_state & OPTION_REMOVABLE_MASK != 0
        {
            let btn_x = win.x + SIDE_COL_W + (CENTER_COL_W - REMOVE_OPTION_BTN_SIZE) / 2.0;
            let btn_y = win.y + content_h - slot_h;
            let btn_rect = Rect::new(btn_x, btn_y, REMOVE_OPTION_BTN_SIZE, REMOVE_OPTION_BTN_SIZE);
            let resp = ui.interact(EQ_REMOVE_OPTION_BTN_ID, btn_rect);
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            if grf {
                let (v, idx) = draw::quad_vertices(
                    btn_rect.x,
                    btn_rect.y,
                    btn_rect.w,
                    btn_rect.h,
                    [1.0, 1.0, 1.0, 1.0],
                );
                ui.draw_calls.push(DrawCall {
                    vertices: v.into(),
                    indices: idx.into(),
                    texture: TextureRef::Named(BTN_OFF_TEX.into()),
                });
            } else {
                let color = if resp.hovered() {
                    [0.6, 0.2, 0.2, 1.0]
                } else {
                    [0.4, 0.15, 0.15, 1.0]
                };
                let (v, idx) =
                    draw::quad_vertices(btn_rect.x, btn_rect.y, btn_rect.w, btn_rect.h, color);
                ui.draw_calls.push(DrawCall {
                    vertices: v.into(),
                    indices: idx.into(),
                    texture: TextureRef::White,
                });
                ui.text(
                    btn_rect.x + 8.0,
                    btn_rect.y + 12.0,
                    crate::pt!("Off", "Desativado"),
                    [1.0, 1.0, 1.0, 1.0],
                );
            }
            if resp.clicked() {
                events.push(GameEvent::RequestRemoveOption);
            }
        }

        if general && character.cart_design.is_some() {
            let slot_w = CENTER_COL_W;
            let slot_x = win.x + SIDE_COL_W;
            let slot_y = content_y + content_h - slot_h;
            let cart_rect = Rect::new(slot_x, slot_y, slot_w, slot_h);
            self.cart_slot_center = Some([slot_x + slot_w / 2.0, content_y + content_h - 4.0]);
            let resp = ui.interact(EQ_CART_SLOT_ID, cart_rect);
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            if resp.clicked() {
                character.cart.open();
            }
            if resp.right_clicked() {
                events.push(GameEvent::RequestRemoveOption);
            }
            if resp.hovered() {
                ui.tooltip(
                    cart_rect.x,
                    cart_rect.y - ui.atlas.line_height - 4.0,
                    "Cart (click to open, right-click to remove)",
                );
            }
        }

        let highlight_location: Option<u32> = ui
            .drag_info()
            .filter(|(src, _)| *src == INV_WINDOW_ID)
            .and_then(|(_, idx)| inventory.get_item(idx as u16))
            .filter(|item| item.is_equipment() && !item.is_equipped())
            .map(|item| item.equip_location());

        let side_col_w = SIDE_COL_W;
        let icon = ICON_SIZE;

        for (i, slot) in self.tab.slots().iter().enumerate() {
            let (slot_x, slot_w) = match slot.col {
                0 => (win.x, side_col_w),                      // Left column
                1 => (win.x + win_w - side_col_w, side_col_w), // Right column
                _ => (win.x + side_col_w, (CENTER_COL_W)),     // Center column
            };
            let slot_y = content_y + slot.row as f32 * slot_h;

            let widget_id = WidgetId(EQ_SLOT_BASE_ID + i as u32);

            let icon_pad = (slot_h - icon) / 2.0;
            let (icon_x, name_x) = match slot.col {
                0 | 1 => {
                    let col = slot.col as usize;
                    (win.x + ICON_X[col], Some(win.x + NAME_X[col]))
                }
                _ => (slot_x + (slot_w - icon) / 2.0, None),
            };
            let icon_y = slot_y + icon_pad;
            let icon_rect = Rect::new(icon_x, icon_y, icon, icon);
            let response = ui.interact(widget_id, icon_rect);

            if let Some(item) = inventory.equipped_in_slot(slot.location) {
                let display_name =
                    format_equipment_display_name_colored(item, data, producers, text_color);
                let name = name_x.map(|x| (x, slot_y + NAME_BASELINE));
                draw_slot_item(ui, item, &display_name, icon_rect, name, text_color, 1.0);

                let mut clicked = false;

                if response.clicked() {
                    ui.drag_source(
                        EQ_WINDOW_ID,
                        item.index as usize,
                        item.icon_path(),
                        (ICON_SIZE, ICON_SIZE),
                    );
                    clicked = true;
                }

                if response.right_clicked() {
                    events.push(GameEvent::ShowItemInfo { index: item.index });
                    clicked = true;
                }
                if response.double_clicked() {
                    events.push(GameEvent::RequestUnequipItem { index: item.index });
                    clicked = true;
                }
                if !clicked && response.hovered() {
                    let tooltip = if item.count > 1 {
                        format!("{}: {} ea.", display_name, item.count)
                    } else {
                        draw::strip_color_codes(&display_name)
                    };
                    ui.tooltip(
                        icon_rect.x,
                        icon_rect.y - ui.atlas.line_height - 16.0,
                        &tooltip,
                    );
                }
            }

            if let Some(loc) = highlight_location
                && loc & InventoryData::slot_mask(slot.location) != 0
            {
                if grf {
                    let (v, idx) = draw::quad_vertices(
                        icon_rect.x,
                        icon_rect.y,
                        icon_rect.w,
                        icon_rect.h,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                    ui.draw_calls.push(DrawCall {
                        vertices: v.into(),
                        indices: idx.into(),
                        texture: TextureRef::Named(ITEM_INVERT_TEX.into()),
                    });
                } else {
                    let (v, idx) = draw::quad_vertices(
                        icon_rect.x,
                        icon_rect.y,
                        icon_rect.w,
                        icon_rect.h,
                        [0.20, 0.42, 0.88, 0.35],
                    );
                    ui.draw_calls.push(DrawCall {
                        vertices: v.into(),
                        indices: idx.into(),
                        texture: TextureRef::White,
                    });
                }
            }
        }

        if self.swap_shown {
            // The panel lies over the footer and its controls (0x8c0432: Move(0, 0xa7))
            let view = SwapView {
                inventory,
                set: &character.equip_switch,
                data,
                producers,
                text_color,
            };
            let actions = swap_panel::build(
                ui,
                EQ_SWAP_PANEL_BASE_ID,
                win.x,
                win.y + swap_panel::PANEL_Y,
                self.tab.slots(),
                self.tab == EquipTab::Costume,
                &view,
            );
            for action in actions {
                match action {
                    SwapPanelAction::Event(event) => events.push(event),
                    SwapPanelAction::Cancel => character.equip_switch.panel_open = false,
                }
            }
        } else {
            if !grf {
                draw_footer(ui, win.x, content_y + content_h, win_w, FOOTER_H, grf);
            }
            if let Some(event) = self.config_check(ui, win.x, win.y, grf) {
                events.push(event);
            }
            let slot_mask = self.tab.slot_mask();
            let worn = inventory
                .all_items()
                .iter()
                .any(|item| item.wear_state & slot_mask != 0);
            if let Some(event) = self.unequip_all_button(ui, win.x, win.y, worn, grf) {
                events.push(event);
            }
            if footer_button(ui, &SWAP_BUTTON, win.x, win.y, grf) {
                character.equip_switch.panel_open = true;
            }
            if footer_button(ui, &ATTRIBUTES_BUTTON, win.x, win.y, grf) {
                events.push(GameEvent::ToggleEquipmentProperties);
            }
        }

        let content_rect = Rect::new(win.x, content_y, win_w, content_h);
        match ui.drop_zone(content_rect) {
            Some((INV_WINDOW_ID, item_index))
                if let Some(item) = inventory.get_item(item_index as u16)
                    && item.is_equipment()
                    && !item.is_equipped() =>
            {
                events.push(GameEvent::RequestEquipItem {
                    index: item.index,
                    location: item.equip_location(),
                });
            }
            // A set item dropped on the window swaps alone (0x895ef0)
            Some((EQ_SWAP_DRAG_ID, item_index)) => {
                events.push(GameEvent::RequestEquipSwitchSingle {
                    index: item_index as u16,
                });
            }
            _ => {}
        }

        ui.has_grf_textures = prev_grf;
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostalre_game::character::Character;
    use nostalre_game::data_table::DataTable;
    use nostalre_game::item::Item;
    use nostalre_ui::context::UiContext;
    use nostalre_ui::state::StateCache;
    use nostalre_ui::test_support::test_frame;

    fn item(index: u16, item_id: u32, location: u32, wear_state: u32) -> Item {
        Item {
            grade: 0,
            options: Default::default(),
            index,
            item_id,
            item_type: models::enums::item::ItemType::Armor,
            count: 1,
            is_identified: true,
            is_damaged: false,
            refining_level: 0,
            slot: [0; 4],
            location,
            wear_state,
            name: format!("Item {item_id}"),
            resource_name: None,
        }
    }

    fn knife(wear_state: u32) -> Item {
        Item {
            item_type: models::enums::item::ItemType::Weapon,
            ..item(2, 1201, 2, wear_state)
        }
    }

    /// One frame with a click on the "Unequip All" button (the window at its default place, no archive art)
    fn click(
        window: &mut EquipmentWindow,
        character: &mut Character,
        state: &mut StateCache,
    ) -> Vec<GameEvent> {
        let at = (
            UNEQUIP_ALL_POS.0 + UNEQUIP_ALL_SIZE.0 / 2.0,
            UNEQUIP_ALL_POS.1 + UNEQUIP_ALL_SIZE.1 / 2.0,
        );
        click_at(window, character, state, at)
    }

    /// One frame with a click at `at` in the window
    fn click_at(
        window: &mut EquipmentWindow,
        character: &mut Character,
        state: &mut StateCache,
        at: (f32, f32),
    ) -> Vec<GameEvent> {
        frame(window, character, state, &DataTable::new(), at, false)
    }

    /// One frame of the window at its default place with `data`, clicked (double-clicked when `double`) at `at`
    fn frame(
        window: &mut EquipmentWindow,
        character: &mut Character,
        state: &mut StateCache,
        data: &DataTable,
        at: (f32, f32),
        double: bool,
    ) -> Vec<GameEvent> {
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = at.0;
        ctx.mouse_y = 210.0 + at.1;
        ctx.mouse_clicked = true;
        ctx.mouse_double_clicked = double;
        let mut ui = test_frame(&mut ctx, state);
        let z = ui.get_z_order();
        ui.compute_hovered_window(&z);
        window.build(&mut ui, &mut crate::BuildCtx::test(character, data))
    }

    fn open_on(tab: EquipTab) -> EquipmentWindow {
        let mut window = EquipmentWindow::new();
        window.open = true;
        window.tab = tab;
        window
    }

    #[test]
    fn unequip_all_takes_off_the_general_tab_and_waits_for_its_answer() {
        let mut window = open_on(EquipTab::General);
        let mut character = Character::new();
        character.inventory.add_item(knife(2));
        let mut state = StateCache::new();

        assert!(matches!(
            click(&mut window, &mut character, &mut state).as_slice(),
            [GameEvent::RequestUnequipAll { location: 0x83ff }]
        ));
        assert!(click(&mut window, &mut character, &mut state).is_empty());
        window.unequip_all_answered();
        assert_eq!(click(&mut window, &mut character, &mut state).len(), 1);
    }

    #[test]
    fn unequip_all_with_nothing_worn_does_nothing() {
        let mut window = open_on(EquipTab::General);
        let mut character = Character::new();
        character.inventory.add_item(knife(0));
        let mut state = StateCache::new();
        assert!(click(&mut window, &mut character, &mut state).is_empty());
    }

    #[test]
    fn ammunition_alone_leaves_unequip_all_greyed() {
        let mut window = open_on(EquipTab::General);
        let mut character = Character::new();
        character.inventory.add_item(Item {
            item_type: models::enums::item::ItemType::Ammo,
            ..item(3, 1750, 0x8000, 0x8000)
        });
        let mut state = StateCache::new();
        assert!(click(&mut window, &mut character, &mut state).is_empty());
    }

    #[test]
    fn the_costume_tab_takes_off_costumes_and_shadow_gear() {
        let mut window = open_on(EquipTab::Costume);
        let mut character = Character::new();
        character
            .inventory
            .add_item(item(4, 24000, 0x01_0000, 0x01_0000));
        let mut state = StateCache::new();
        assert!(matches!(
            click(&mut window, &mut character, &mut state).as_slice(),
            [GameEvent::RequestUnequipAll { location: 0x3f7c00 }]
        ));
    }

    #[test]
    fn the_costume_tab_does_not_count_general_gear() {
        let mut window = open_on(EquipTab::Costume);
        let mut character = Character::new();
        character.inventory.add_item(knife(2));
        let mut state = StateCache::new();
        assert!(click(&mut window, &mut character, &mut state).is_empty());
    }

    #[test]
    fn every_location_of_the_costume_tab_has_one_slot() {
        let mask = COSTUME_SLOTS
            .iter()
            .fold(0, |m, slot| m | InventoryData::slot_mask(slot.location));
        assert_eq!(mask, EquipTab::Costume.slot_mask() & !0x4000);
        assert_eq!(COSTUME_SLOTS.len(), 10);
    }

    fn click_config_check(window: &mut EquipmentWindow) -> Vec<GameEvent> {
        let (x, y, _, h) = CONFIG_CHECK_RECT;
        let mut character = Character::new();
        let mut state = StateCache::new();
        click_at(window, &mut character, &mut state, (x + 5.0, y + h / 2.0))
    }

    #[test]
    fn show_equip_asks_the_server_and_waits_for_its_answer() {
        let mut window = open_on(EquipTab::General);
        assert!(matches!(
            click_config_check(&mut window).as_slice(),
            [GameEvent::RequestSetConfig {
                kind: SelfConfigKind::OpenEquipmentWindow,
                enabled: true
            }]
        ));
        assert!(!window.show_equip, "the box follows the server's answer");
        window.set_self_config(true, false);
        assert!(matches!(
            click_config_check(&mut window).as_slice(),
            [GameEvent::RequestSetConfig {
                kind: SelfConfigKind::OpenEquipmentWindow,
                enabled: false
            }]
        ));
    }

    #[test]
    fn view_costumes_unticked_hides_them() {
        let mut window = open_on(EquipTab::Costume);
        assert!(matches!(
            click_config_check(&mut window).as_slice(),
            [GameEvent::RequestSetConfig {
                kind: SelfConfigKind::HideCostumes,
                enabled: true
            }]
        ));
        window.set_self_config(false, true);
        assert!(matches!(
            click_config_check(&mut window).as_slice(),
            [GameEvent::RequestSetConfig {
                kind: SelfConfigKind::HideCostumes,
                enabled: false
            }]
        ));
    }

    fn title_data() -> DataTable {
        let mut data = DataTable::new();
        data.titles = nostalre_game::title::TitleTables::parse(
            Some(b"title_tbl = { [1000] = \"Junction of life\", [1034] = \"VIP\" }"),
            Some(
                b"achievement_tbl = { [200000] = { reward = { title = 1000 } }, [130000] = { reward = { title = 1034 } } }",
            ),
        );
        data
    }

    /// One frame with a click (a double-click when `double`) on a row of the Title tab's list
    fn click_title_row(
        window: &mut EquipmentWindow,
        character: &mut Character,
        row: usize,
        double: bool,
    ) -> Vec<GameEvent> {
        let at = (TITLE_LIST.0 + 10.0, TITLE_LIST.1 + row as f32 * 15.0 + 7.0);
        let mut state = StateCache::new();
        frame(window, character, &mut state, &title_data(), at, double)
    }

    fn skin(name: &str) -> nostalre_game::damage_skin::DamageSkin {
        nostalre_game::damage_skin::DamageSkin {
            name: name.into(),
            icon: format!("data/texture/{name}.bmp"),
            number: format!("data/sprite/{name}"),
            msg: format!("data/sprite/{name}_msg"),
            blue: format!("data/sprite/{name}_bmsg"),
            million_unit: 1,
        }
    }

    /// One click at `at` on the Damage Indicator tab with two skins
    fn click_damage_tab(window: &mut EquipmentWindow, at: (f32, f32)) -> Vec<GameEvent> {
        let mut data = DataTable::new();
        data.damage_skins = vec![skin("Normal"), skin("Colour")];
        let mut character = Character::new();
        let mut state = StateCache::new();
        frame(window, &mut character, &mut state, &data, at, false)
    }

    #[test]
    fn apply_takes_the_picked_skin_and_direction() {
        let mut window = open_on(EquipTab::DamageIndicator);
        window.set_damage_indicator(0, DamageDirection::Default);
        assert!(click_damage_tab(&mut window, (6.0 + 68.0 + 10.0, 80.0)).is_empty());
        assert!(click_damage_tab(&mut window, (148.0 + 5.0, 45.0)).is_empty());
        assert!(matches!(
            click_damage_tab(&mut window, (180.0, 177.0)).as_slice(),
            [GameEvent::ApplyDamageIndicator {
                skin: 1,
                direction: DamageDirection::Center
            }]
        ));
    }

    #[test]
    fn the_picks_start_as_in_use_each_time_the_window_opens() {
        let mut window = open_on(EquipTab::DamageIndicator);
        window.set_damage_indicator(1, DamageDirection::Right);
        click_damage_tab(&mut window, (6.0 + 10.0, 80.0));
        window.open = false;
        click_damage_tab(&mut window, (0.0, 0.0));
        window.open = true;
        window.tab = EquipTab::DamageIndicator;
        assert!(matches!(
            click_damage_tab(&mut window, (180.0, 177.0)).as_slice(),
            [GameEvent::ApplyDamageIndicator {
                skin: 1,
                direction: DamageDirection::Right
            }]
        ));
    }

    #[test]
    fn close_on_the_damage_tab_shuts_the_window() {
        let mut window = open_on(EquipTab::DamageIndicator);
        assert!(click_damage_tab(&mut window, (240.0, 177.0)).is_empty());
        assert!(!window.open);
    }

    #[test]
    fn a_double_click_on_disable_title_takes_the_title_off() {
        let mut window = open_on(EquipTab::Title);
        let mut character = Character::new();
        assert!(matches!(
            click_title_row(&mut window, &mut character, 0, true).as_slice(),
            [GameEvent::RequestChangeTitle { title_id: 0 }]
        ));
    }

    #[test]
    fn the_titles_follow_disable_title_by_name_and_a_double_click_wears_one() {
        let mut window = open_on(EquipTab::Title);
        let mut character = Character::new();
        character
            .titles
            .set_rewarded(&title_data().titles, &[130_000, 200_000]);
        assert_eq!(character.titles.owned(), [1000, 1034]);
        assert!(click_title_row(&mut window, &mut character, 2, false).is_empty());
        assert!(matches!(
            click_title_row(&mut window, &mut character, 2, true).as_slice(),
            [GameEvent::RequestChangeTitle { title_id: 1034 }]
        ));
    }

    #[test]
    fn the_title_tab_has_no_unequip_all() {
        let mut window = open_on(EquipTab::Title);
        let mut character = Character::new();
        character.inventory.add_item(knife(2));
        let mut state = StateCache::new();
        assert!(click(&mut window, &mut character, &mut state).is_empty());
    }

    /// One frame clicked at `at`, double-clicked when `double`, with Ctrl held when `ctrl`
    fn click_with(
        window: &mut EquipmentWindow,
        character: &mut Character,
        at: (f32, f32),
        double: bool,
        ctrl: bool,
    ) -> Vec<GameEvent> {
        let data = DataTable::new();
        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = at.0;
        ctx.mouse_y = 210.0 + at.1;
        ctx.mouse_clicked = true;
        ctx.mouse_double_clicked = double;
        ctx.ctrl_pressed = ctrl;
        let mut ui = test_frame(&mut ctx, &mut state);
        let z = ui.get_z_order();
        ui.compute_hovered_window(&z);
        window.build(&mut ui, &mut crate::BuildCtx::test(character, &data))
    }

    #[test]
    fn swap_equipment_opens_the_panel_under_the_slots() {
        let mut window = open_on(EquipTab::General);
        let mut character = Character::new();
        click_with(
            &mut window,
            &mut character,
            (SWAP_BTN_POS.0 + 10.0, 177.0),
            false,
            false,
        );
        assert!(character.equip_switch.panel_open);
        click_with(&mut window, &mut character, (0.0, 0.0), false, false);
        assert_eq!(window.window_size().1, 346.0);
    }

    /// The panel's CHANGE and Cancel, in the window (the panel at 0xa7)
    const CHANGE_AT: (f32, f32) = (140.0, 167.0 + 165.0);
    const CANCEL_AT: (f32, f32) = (249.0, 167.0 + 165.0);

    #[test]
    fn change_asks_for_the_swap_and_cancel_shuts_the_panel() {
        let mut window = open_on(EquipTab::Costume);
        let mut character = Character::new();
        character.equip_switch.panel_open = true;
        assert!(matches!(
            click_with(&mut window, &mut character, CHANGE_AT, false, false).as_slice(),
            [GameEvent::RequestEquipSwitch]
        ));
        click_with(&mut window, &mut character, CANCEL_AT, false, false);
        assert!(!character.equip_switch.panel_open);
    }

    #[test]
    fn the_title_tab_hides_the_panel_but_keeps_it_open() {
        let mut window = open_on(EquipTab::Title);
        let mut character = Character::new();
        character.equip_switch.panel_open = true;
        assert!(click_with(&mut window, &mut character, CHANGE_AT, false, false).is_empty());
        assert!(character.equip_switch.panel_open);
        assert_eq!(
            window.window_size().1,
            TITLE_H + TAB_ROW_H + CONTENT_H + FOOTER_H
        );
    }

    #[test]
    fn a_set_item_double_clicked_leaves_the_set_or_with_ctrl_swaps_alone() {
        let mut window = open_on(EquipTab::General);
        let mut character = Character::new();
        character.inventory.add_item(item(4, 2301, 0x10, 0));
        character.equip_switch.set_list(vec![(4, 0x10)]);
        character.equip_switch.panel_open = true;
        // The armor's row on the panel's right
        let at = (200.0, 167.0 + 57.0);
        assert!(matches!(
            click_with(&mut window, &mut character, at, true, false).as_slice(),
            [GameEvent::RequestEquipSwitchRemove { index: 4 }]
        ));
        assert!(matches!(
            click_with(&mut window, &mut character, at, true, true).as_slice(),
            [GameEvent::RequestEquipSwitchSingle { index: 4 }]
        ));
    }

    #[test]
    fn closing_the_window_goes_back_to_the_first_tab() {
        let mut window = open_on(EquipTab::Costume);
        window.open = false;
        let mut character = Character::new();
        let mut state = StateCache::new();
        click(&mut window, &mut character, &mut state);
        assert_eq!(window.tab, EquipTab::General);
    }
}
