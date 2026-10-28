use super::input_dialog::{InputDialog, InputDialogConfig, InputDialogLayout, InputDialogResult};
use crate::helper::dialog_container::DialogContainer;
use crate::helper::scrollbar::{self, ScrollbarIds};
use crate::helper::window_chrome::draw_textured_quad;
use crate::{BuildCtx, InGameWindow, Window};
use nostalre_game::data_table::DataTable;
use nostalre_game::event::GameEvent;
use nostalre_game::npc_dialog::{NpcDialogData, NpcDialogState};
use nostalre_game::npc_text::{self, NpcLink, NpcTextPart};
use nostalre_ui::draw::{self, DrawCall, TextureRef};
use nostalre_ui::frame::{ButtonTextures, TextInputBg, UiFrame, WidgetId, WindowOrder};
use nostalre_ui::rect::Rect;
use nostalre_ui::rich_text::{self, RichLayout, RichSpan, RunKind};
use nostalre_ui::text_input::TextInput;

const OVERLAY_ID: WidgetId = WidgetId(600);
pub const NPC_DIALOG_WINDOW_ID: WidgetId = WidgetId(610);
pub const NPC_MENU_WINDOW_ID: WidgetId = WidgetId(611);
const NPC_DEAL_WINDOW_ID: WidgetId = WidgetId(612);
const NEXT_BTN_ID: WidgetId = WidgetId(601);
const CLOSE_BTN_ID: WidgetId = WidgetId(602);
const INPUT_ID: WidgetId = WidgetId(603);
const OK_BTN_ID: WidgetId = WidgetId(604);
const CANCEL_BTN_ID: WidgetId = WidgetId(605);
const MENU_OK_BTN_ID: WidgetId = WidgetId(606);
const BUY_BTN_ID: WidgetId = WidgetId(607);
const SELL_BTN_ID: WidgetId = WidgetId(608);
const DEAL_CANCEL_BTN_ID: WidgetId = WidgetId(609);
const MENU_BASE_ID: u32 = 620;
const SCROLL_UP_ID: WidgetId = WidgetId(640);
const SCROLL_DOWN_ID: WidgetId = WidgetId(641);
const SCROLL_THUMB_ID: WidgetId = WidgetId(642);
/// A link in the say window's text; links past the module's last id share it (`interact` keeps no state by id)
const LINK_BASE_ID: u32 = 650;
const LAST_LINK_ID: u32 = 699;

const DIALOG_W: f32 = 280.0;
const DIALOG_H: f32 = 180.0;
const DIALOG_DEFAULT_X: f32 = 200.0;
const DIALOG_DEFAULT_Y: f32 = 100.0;
const MENU_W: f32 = 280.0;
const MENU_H: f32 = 120.0;
const MENU_DEFAULT_X: f32 = 200.0;
const MENU_DEFAULT_Y: f32 = 300.0;
const PADDING: f32 = 8.0;
const TEXT_LINE_HEIGHT: f32 = 16.0;
const MENU_ITEM_HEIGHT: f32 = 20.0;
const MENU_VISIBLE_ROWS: usize = 4;
const FALLBACK_BTN_W: f32 = 42.0;
const FALLBACK_BTN_H: f32 = 20.0;
/// A link in NPC text as the 2025 rig draws it (2026-10-19): #4A94F7, underlined 3 pixels under the text
const LINK_COLOR: [f32; 4] = [74.0 / 255.0, 148.0 / 255.0, 247.0 / 255.0, 1.0];
const UNDERLINE_DROP: f32 = 3.0;
/// An item icon in NPC text (`^i[id]`) at its own size, its line as tall (the 2025 rig)
const TEXT_ICON: f32 = 24.0;

const NEXT_BTN: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_NEXT,
    hover: nostalre_resources::ui::BTN_NEXT_A,
    pressed: nostalre_resources::ui::BTN_NEXT_B,
};

const CLOSE_BTN: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_CLOSE,
    hover: nostalre_resources::ui::BTN_CLOSE_A,
    pressed: nostalre_resources::ui::BTN_CLOSE_B,
};

const OK_BTN: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_OK,
    hover: nostalre_resources::ui::BTN_OK_A,
    pressed: nostalre_resources::ui::BTN_OK_B,
};

const CANCEL_BTN: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_CANCEL,
    hover: nostalre_resources::ui::BTN_CANCEL_A,
    pressed: nostalre_resources::ui::BTN_CANCEL_B,
};

const BUY_BTN: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_BUY,
    hover: nostalre_resources::ui::BTN_BUY_A,
    pressed: nostalre_resources::ui::BTN_BUY_B,
};

const SELL_BTN: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_SELL,
    hover: nostalre_resources::ui::BTN_SELL_A,
    pressed: nostalre_resources::ui::BTN_SELL_B,
};

const WIN_TEXTURE: &str = nostalre_resources::ui::WIN_MSGBOX;
const BTN_BOTTOM: f32 = 4.0;
const BTN_FIRST_RIGHT: f32 = 5.0;
const BTN_SPACING: f32 = 3.0;

fn cancel_drag_on_press(ui: &mut UiFrame, window: WidgetId, rects: &[Rect]) {
    if !ui.ctx.mouse_clicked {
        return;
    }
    let (mx, my) = (ui.ctx.mouse_x, ui.ctx.mouse_y);
    if rects.iter().any(|r| r.contains(mx, my)) {
        ui.cancel_window_drag(window);
    }
}

pub struct NpcDialog {
    pub has_grf_textures: bool,
    pub dialog: NpcDialogData,
    pub string_input: TextInput,
    number_input_dialog: Option<InputDialog>,
    btn_size: (f32, f32),
    container: DialogContainer,
    win_size: (f32, f32),
    say_text: SayText,
    /// The deal popup's button the keys picked, as they stand: 0 Buy, 1 Sell, 2 Cancel
    deal_choice: usize,
}

/// The say window's text laid out, kept while its text, width and font stay
#[derive(Default)]
struct SayText {
    text: String,
    width: f32,
    line_height: f32,
    layout: RichLayout,
    links: Vec<NpcLink>,
    /// An item icon's texture, by link
    icons: Vec<Option<String>>,
}

impl SayText {
    fn update(&mut self, text: &str, width: f32, color: [f32; 4], ui: &UiFrame, data: &DataTable) {
        let line_height = ui.atlas.line_height;
        if self.text == text && self.width == width && self.line_height == line_height {
            return;
        }
        self.links.clear();
        self.icons.clear();
        let parts = npc_text::parse(text);
        let spans: Vec<RichSpan> = parts
            .iter()
            .map(|part| match part {
                NpcTextPart::Text(text) => RichSpan::Text(text),
                NpcTextPart::Link { label, link } => {
                    self.links.push(link.clone());
                    self.icons.push(None);
                    RichSpan::Link {
                        label,
                        link: self.links.len() - 1,
                    }
                }
                NpcTextPart::ItemIcon(id) => {
                    self.links.push(NpcLink::Item(*id));
                    self.icons.push(data.item_icon_path(*id));
                    RichSpan::Image {
                        link: self.links.len() - 1,
                    }
                }
                NpcTextPart::Bold(bold) => RichSpan::Bold(*bold),
            })
            .collect();
        self.layout = rich_text::layout(
            &spans,
            width,
            |t| ui.atlas.measure_text(t),
            TEXT_ICON,
            color,
        );
        self.text = text.to_string();
        self.width = width;
        self.line_height = line_height;
    }

    /// The text's height
    fn height(&self) -> f32 {
        self.layout.height(TEXT_LINE_HEIGHT, TEXT_ICON)
    }

    /// Draws the text from `top`, then the underlines, then the icons: the renderer merges neighbouring draws of
    /// one texture, so the text stays one draw however many links it has. A line holding an icon is the icon's
    /// height, its text at the bottom. A click on a link gives what it opens.
    fn draw(&self, ui: &mut UiFrame, data: &DataTable, x: f32, top: f32) -> Option<GameEvent> {
        let mut opened = None;
        let mut underlines = Vec::new();
        let mut icons = Vec::new();
        for run in &self.layout.runs {
            let run_x = x + run.x;
            let line_top = top + self.layout.line_top(run.line, TEXT_LINE_HEIGHT, TEXT_ICON);
            let taller = if self.layout.image_lines.contains(&run.line) {
                TEXT_ICON - TEXT_LINE_HEIGHT
            } else {
                0.0
            };
            let run_baseline = line_top + taller + ui.atlas.line_height;
            let top = line_top + taller;
            let link = match run.kind {
                RunKind::Text => {
                    ui.colored_text(run_x, run_baseline, &run.text, run.color);
                    continue;
                }
                RunKind::Link(link) | RunKind::Image(link) => link,
            };
            let id = (LINK_BASE_ID as usize + link).min(LAST_LINK_ID as usize) as u32;
            let rect = Rect::new(run_x, top, run.width, TEXT_LINE_HEIGHT);
            let response = ui.interact(WidgetId(id), rect);
            if response.hovered() {
                ui.any_interactive_hovered = true;
            }
            if let RunKind::Image(_) = run.kind {
                if let Some(path) = &self.icons[link] {
                    icons.push((run_x, line_top, path));
                }
            } else {
                ui.colored_text(run_x, run_baseline, &run.text, LINK_COLOR);
                let (v, i) = draw::quad_vertices(
                    run_x,
                    run_baseline + UNDERLINE_DROP,
                    run.width,
                    1.0,
                    LINK_COLOR,
                );
                underlines.push(DrawCall {
                    vertices: v.into(),
                    indices: i.into(),
                    texture: TextureRef::White,
                });
            }
            if response.clicked() {
                opened = link_event(&self.links[link], data);
            }
        }
        ui.draw_calls.extend(underlines);
        for (x, y, path) in icons {
            draw_textured_quad(ui, x, y, TEXT_ICON, TEXT_ICON, path.clone());
        }
        opened
    }
}

/// What a click on a link opens: an item's description, navigation as the server's guide (0x08e2) gives it, or
/// nothing yet (a web page, a tip box, a quest)
fn link_event(link: &NpcLink, data: &DataTable) -> Option<GameEvent> {
    match link {
        NpcLink::Item(id) => Some(GameEvent::ShowItemInfoDirect {
            item: Box::new(npc_text::linked_item(*id, data)),
        }),
        NpcLink::Navigation(navigation) => Some(GameEvent::NavigationGuide {
            navigation: navigation.clone(),
        }),
        NpcLink::Inert => None,
    }
}

impl Default for NpcDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl NpcDialog {
    /// End a conversation on a session transition, including input popups that
    /// may not have received another UI frame before the old map disappeared.
    pub fn reset_conversation(&mut self) {
        self.dialog.close();
        self.string_input = TextInput::new(70, false);
        self.number_input_dialog = None;
        self.say_text = SayText::default();
        self.deal_choice = 0;
    }

    pub fn new() -> Self {
        Self {
            has_grf_textures: false,
            dialog: NpcDialogData::new(),
            string_input: TextInput::new(70, false),
            number_input_dialog: None,
            btn_size: (FALLBACK_BTN_W, FALLBACK_BTN_H),
            container: DialogContainer::new(),
            win_size: (280.0, 120.0),
            say_text: SayText::default(),
            deal_choice: 0,
        }
    }
}

impl Window for NpcDialog {
    fn has_grf_textures(&self) -> bool {
        self.has_grf_textures
    }
    fn set_has_grf_textures(&mut self, value: bool) {
        self.has_grf_textures = value;
    }

    fn set_texture_sizes(&mut self, size_fn: &dyn Fn(&str) -> Option<(u32, u32)>) {
        if let Some((w, h)) = size_fn(NEXT_BTN.normal) {
            self.btn_size = (w as f32, h as f32);
        }
        self.container.set_texture_sizes(size_fn);
        if let Some((w, h)) = size_fn(WIN_TEXTURE) {
            self.win_size = (w as f32, h as f32);
        }
    }

    fn window_size(&self) -> (f32, f32) {
        (DIALOG_W, DIALOG_H)
    }

    fn grf_texture_paths() -> Vec<&'static str> {
        let mut paths = DialogContainer::grf_texture_paths();
        paths.extend_from_slice(&[
            WIN_TEXTURE,
            NEXT_BTN.normal,
            NEXT_BTN.hover,
            NEXT_BTN.pressed,
            CLOSE_BTN.normal,
            CLOSE_BTN.hover,
            CLOSE_BTN.pressed,
            OK_BTN.normal,
            OK_BTN.hover,
            OK_BTN.pressed,
            CANCEL_BTN.normal,
            CANCEL_BTN.hover,
            CANCEL_BTN.pressed,
            BUY_BTN.normal,
            BUY_BTN.hover,
            BUY_BTN.pressed,
            SELL_BTN.normal,
            SELL_BTN.hover,
            SELL_BTN.pressed,
        ]);
        paths.extend_from_slice(&scrollbar::grf_texture_paths());
        paths
    }
}

impl InGameWindow for NpcDialog {
    fn owns_keyboard(&self, _ctx: &BuildCtx) -> bool {
        self.dialog.is_open()
    }

    /// An open dialog always claims Escape, even in the states that do nothing
    /// with it: the server holds the character until it gets a reply, so the key
    /// must not reach a window behind it.
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.dialog.is_open()
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let npc_id = self.dialog.npc_id;
        if self.dialog.close_button {
            self.dialog.close();
            return vec![GameEvent::RequestNpcClose { npc_id }];
        }
        match self.dialog.state {
            NpcDialogState::WaitingForMenu => {
                self.dialog.close();
                vec![GameEvent::RequestNpcMenuSelect {
                    npc_id,
                    choice: 255,
                }]
            }
            NpcDialogState::WaitingForDealType => {
                self.dialog.close();
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let _character = &mut *ctx.character;
        let _data = ctx.data;
        if !self.dialog.is_open() {
            return Vec::new();
        }

        self.dialog.restore_unanswered_next(std::time::Instant::now());
        let mut events = Vec::new();
        let state = self.dialog.state;

        let menu_up = state == NpcDialogState::WaitingForMenu;

        if ui.ctx.key_enter && !menu_up && self.dialog.next_button {
            events.push(GameEvent::RequestNpcNext {
                npc_id: self.dialog.npc_id,
            });
            self.dialog.advance_next();
            return events;
        }
        if ui.ctx.key_enter && !menu_up && self.dialog.close_button {
            events.push(GameEvent::RequestNpcClose {
                npc_id: self.dialog.npc_id,
            });
            self.dialog.close();
            return events;
        }

        match state {
            NpcDialogState::WaitingForMenu => {
                let total_items = self.dialog.menu_items.len();
                if ui.ctx.key_up && self.dialog.selected_menu_index > 0 {
                    self.dialog.selected_menu_index -= 1;
                }
                if ui.ctx.key_down && self.dialog.selected_menu_index + 1 < total_items {
                    self.dialog.selected_menu_index += 1;
                }
                if ui.ctx.key_up || ui.ctx.key_down {
                    let offset = &mut self.dialog.menu_scroll_offset;
                    if *offset > self.dialog.selected_menu_index {
                        *offset = self.dialog.selected_menu_index;
                    } else if self.dialog.selected_menu_index >= *offset + MENU_VISIBLE_ROWS
                        && total_items > MENU_VISIBLE_ROWS
                    {
                        *offset = self.dialog.selected_menu_index + 1 - MENU_VISIBLE_ROWS;
                    }
                    let max_offset = total_items.saturating_sub(MENU_VISIBLE_ROWS);
                    *offset = (*offset).min(max_offset);
                }
                if ui.ctx.key_enter {
                    let choice = (self.dialog.selected_menu_index + 1) as u8;
                    events.push(GameEvent::RequestNpcMenuSelect {
                        npc_id: self.dialog.npc_id,
                        choice,
                    });
                    self.dialog.close_menu();
                    return events;
                }
            }
            NpcDialogState::WaitingForNumberInput => {}
            NpcDialogState::WaitingForStringInput => {
                if ui.ctx.key_enter {
                    let text = self.string_input.text.clone();
                    events.push(GameEvent::RequestNpcInputString {
                        npc_id: self.dialog.npc_id,
                        text,
                    });
                    self.string_input.text.clear();
                    self.string_input.cursor_pos = 0;
                    self.dialog.close();
                    return events;
                }
            }
            _ => {}
        }

        let prev_grf = ui.has_grf_textures;
        ui.has_grf_textures = self.has_grf_textures;
        self.container.has_grf_textures = self.has_grf_textures;

        let screen = Rect::new(0.0, 0.0, ui.ctx.screen_width, ui.ctx.screen_height);
        ui.interact(OVERLAY_ID, screen);

        if state != NpcDialogState::WaitingForDealType {
            self.deal_choice = 0;
        }
        if state == NpcDialogState::WaitingForDealType {
            let result = self.build_deal_type_popup(ui);
            ui.has_grf_textures = prev_grf;
            return result;
        }

        let say_visible = self.dialog.say_visible || state == NpcDialogState::WaitingForStringInput;

        let padding = PADDING;
        let dialog_w = DIALOG_W;

        if say_visible {
            let text_area_w = dialog_w - padding * 2.0;
            let text_color = self.container.text_color();
            self.say_text
                .update(&self.dialog.text, text_area_w, text_color, ui, ctx.data);
            let text_h = self.say_text.height();

            let input_h = if state == NpcDialogState::WaitingForStringInput {
                30.0
            } else {
                0.0
            };

            let (btn_w, btn_h) = self.btn_size;
            let has_button = self.dialog.next_button
                || self.dialog.close_button
                || state == NpcDialogState::WaitingForStringInput;
            let btn_area_h = if has_button { btn_h + padding } else { 0.0 };

            let dialog_h = (padding + text_h + input_h + btn_area_h + padding).max(DIALOG_H);

            ui.ensure_in_z_order_with(NPC_DIALOG_WINDOW_ID, WindowOrder::Foreground);
            let win = ui.window_at(
                NPC_DIALOG_WINDOW_ID,
                dialog_w,
                dialog_h,
                dialog_h,
                DIALOG_DEFAULT_X,
                DIALOG_DEFAULT_Y,
            );
            ui.interact(NPC_DIALOG_WINDOW_ID, win);
            let (dx, dy) = (win.x, win.y);

            self.container.draw(
                &mut ui.draw_calls,
                dx,
                dy,
                dialog_w,
                dialog_h,
                [1.0, 1.0, 1.0, 0.95],
            );

            let first_baseline = dy + padding + ui.atlas.line_height;
            events.extend(
                self.say_text
                    .draw(ui, ctx.data, dx + padding, dy + padding),
            );
            let text_y = first_baseline + text_h;

            if state == NpcDialogState::WaitingForStringInput {
                let input_y = text_y + padding;
                let input_rect =
                    Rect::new(dx + padding, input_y, text_area_w - btn_w - padding, 22.0);
                if ui.focused() != Some(INPUT_ID) {
                    ui.set_focus(INPUT_ID);
                }
                ui.text_input(
                    INPUT_ID,
                    input_rect,
                    &mut self.string_input,
                    TextInputBg::Default,
                );

                let ok_rect = Rect::new(dx + dialog_w - padding - btn_w, input_y, btn_w, btn_h);
                cancel_drag_on_press(ui, NPC_DIALOG_WINDOW_ID, &[input_rect, ok_rect]);
                let ok = ui.button(OK_BTN_ID, ok_rect, &OK_BTN, "OK");
                if ok.clicked() {
                    let text = self.string_input.text.clone();
                    events.push(GameEvent::RequestNpcInputString {
                        npc_id: self.dialog.npc_id,
                        text,
                    });
                    self.string_input.text.clear();
                    self.string_input.cursor_pos = 0;
                    self.dialog.close();
                    ui.has_grf_textures = prev_grf;
                    return events;
                }
            }

            let dialog_rect = Rect::new(dx, dy, dialog_w, dialog_h);
            let btns = dialog_rect.buttons_bottom_right(
                2,
                btn_w,
                btn_h,
                BTN_BOTTOM,
                BTN_FIRST_RIGHT,
                BTN_SPACING,
            );

            let shown =
                usize::from(self.dialog.next_button) + usize::from(self.dialog.close_button);
            if shown > 0 {
                cancel_drag_on_press(ui, NPC_DIALOG_WINDOW_ID, &btns[..shown]);
            }

            if self.dialog.next_button {
                let response = ui.button(NEXT_BTN_ID, btns[0], &NEXT_BTN, crate::pt!("Next", "Próximo"));
                if response.clicked() {
                    events.push(GameEvent::RequestNpcNext {
                        npc_id: self.dialog.npc_id,
                    });
                    self.dialog.advance_next();
                }
            }
            if self.dialog.close_button {
                let slot = if self.dialog.next_button {
                    btns[1]
                } else {
                    btns[0]
                };
                let response = ui.button(CLOSE_BTN_ID, slot, &CLOSE_BTN, crate::pt!("Close", "Fechar"));
                if response.clicked() {
                    events.push(GameEvent::RequestNpcClose {
                        npc_id: self.dialog.npc_id,
                    });
                    self.dialog.close();
                }
            }
        } // say_visible

        if state == NpcDialogState::WaitingForMenu {
            let menu_events = self.build_menu_window(ui);
            events.extend(menu_events);
        }

        if state == NpcDialogState::WaitingForNumberInput {
            if self.number_input_dialog.is_none() {
                let mut dialog = InputDialog::new(
                    InputDialogConfig {
                        layout: InputDialogLayout::Text {
                            label: Some("Input number".to_string()),
                            show_cancel: false,
                        },
                        escape_cancels: false,
                        default_value: String::new(),
                        max_len: 10,
                        numeric_only: true,
                        max_value: None,
                    },
                    WidgetId(INPUT_ID.0),
                );
                dialog.init_container(&self.container);
                self.number_input_dialog = Some(dialog);
            }
            let dialog = self.number_input_dialog.as_mut().unwrap();
            dialog.init_container(&self.container);
            if let InputDialogResult::Submitted = dialog.build(ui) {
                let value: i32 = dialog.value_i32().unwrap_or(0);
                events.push(GameEvent::RequestNpcInputNumber {
                    npc_id: self.dialog.npc_id,
                    value,
                });
                self.number_input_dialog = None;
                self.dialog.close();
            }
        } else {
            self.number_input_dialog = None;
        }

        ui.has_grf_textures = prev_grf;
        events
    }
}

impl NpcDialog {
    fn build_menu_window(&mut self, ui: &mut UiFrame) -> Vec<GameEvent> {
        let mut events = Vec::new();
        let mut submit = false;
        let (btn_w, btn_h) = self.btn_size;
        let menu_w = MENU_W;
        let menu_h = MENU_H;
        let padding = PADDING;
        let menu_item_h = MENU_ITEM_HEIGHT;
        let text_area_w = menu_w - padding * 2.0;

        let total_items = self.dialog.menu_items.len();
        let needs_scroll = total_items > MENU_VISIBLE_ROWS;
        let content_h = MENU_VISIBLE_ROWS as f32 * menu_item_h;

        ui.ensure_in_z_order_with(NPC_MENU_WINDOW_ID, WindowOrder::Foreground);
        let win = ui.window_at(
            NPC_MENU_WINDOW_ID,
            menu_w,
            menu_h,
            menu_h,
            MENU_DEFAULT_X,
            MENU_DEFAULT_Y,
        );
        ui.interact(NPC_MENU_WINDOW_ID, win);
        let (dx, menu_y) = (win.x, win.y);

        self.container.draw(
            &mut ui.draw_calls,
            dx,
            menu_y,
            menu_w,
            menu_h,
            [1.0, 1.0, 1.0, 0.95],
        );

        let content_rect = Rect::new(dx + padding, menu_y + padding, text_area_w, content_h);
        cancel_drag_on_press(ui, NPC_MENU_WINDOW_ID, &[content_rect]);

        let text_color = self.container.text_color();
        let offset = self.dialog.menu_scroll_offset;
        let end_idx = (offset + MENU_VISIBLE_ROWS).min(total_items);
        let item_text_w = if needs_scroll {
            text_area_w - scrollbar::SCROLLBAR_W
        } else {
            text_area_w
        };

        for idx in offset..end_idx {
            let row = idx - offset;
            let item_y = menu_y + padding + row as f32 * menu_item_h;
            let item_rect = Rect::new(dx + padding, item_y, item_text_w, menu_item_h);
            let widget_id = WidgetId(MENU_BASE_ID + idx as u32);
            let response = ui.interact(widget_id, item_rect);
            if response.hovered() {
                ui.any_interactive_hovered = true;
            }

            let is_selected = idx == self.dialog.selected_menu_index;

            if is_selected {
                let highlight = [0.3, 0.3, 0.5, 0.5];
                let (v, i) = draw::quad_vertices(
                    item_rect.x,
                    item_rect.y,
                    item_rect.w,
                    item_rect.h,
                    highlight,
                );
                ui.draw_calls.push(DrawCall {
                    vertices: v.into(),
                    indices: i.into(),
                    texture: TextureRef::White,
                });
            }

            let label = format!("{}. {}", idx + 1, &self.dialog.menu_items[idx]);
            ui.colored_text(
                dx + padding + (4.0),
                item_y + ui.atlas.line_height - (4.0),
                &label,
                text_color,
            );

            if response.clicked() || response.double_clicked() {
                self.dialog.selected_menu_index = idx;
            }
            if response.double_clicked() {
                submit = true;
            }
        }

        if needs_scroll {
            let max_scroll = total_items - MENU_VISIBLE_ROWS;
            let scroll_ids = ScrollbarIds {
                up: SCROLL_UP_ID,
                down: SCROLL_DOWN_ID,
                thumb: SCROLL_THUMB_ID,
            };
            let scroll_x = dx + menu_w - scrollbar::SCROLLBAR_W - padding;
            self.dialog.menu_scroll_offset = scrollbar::scrollbar(
                ui,
                scroll_ids,
                offset,
                MENU_VISIBLE_ROWS,
                max_scroll,
                content_rect,
                scroll_x,
                menu_y + padding,
                content_h,
            );
        }

        let menu_rect = Rect::new(dx, menu_y, menu_w, menu_h);
        let menu_btns = menu_rect.buttons_bottom_right(
            2,
            btn_w,
            btn_h,
            BTN_BOTTOM,
            BTN_FIRST_RIGHT,
            BTN_SPACING,
        );

        cancel_drag_on_press(ui, NPC_MENU_WINDOW_ID, &menu_btns);

        let cancel = ui.button(CANCEL_BTN_ID, menu_btns[0], &CANCEL_BTN, crate::pt!("Cancel", "Cancelar"));
        let ok = ui.button(MENU_OK_BTN_ID, menu_btns[1], &OK_BTN, "OK");

        if ok.clicked() || submit {
            let choice = (self.dialog.selected_menu_index + 1) as u8;
            events.push(GameEvent::RequestNpcMenuSelect {
                npc_id: self.dialog.npc_id,
                choice,
            });
            self.dialog.close_menu();
        }
        if cancel.clicked() {
            events.push(GameEvent::RequestNpcMenuSelect {
                npc_id: self.dialog.npc_id,
                choice: 255,
            });
            self.dialog.close();
        }

        events
    }

    fn build_deal_type_popup(&mut self, ui: &mut UiFrame) -> Vec<GameEvent> {
        let mut events = Vec::new();
        let (btn_w, btn_h) = self.btn_size;
        let (dialog_w, dialog_h) = self.win_size;

        let dx = ((ui.ctx.screen_width - dialog_w) / 2.0).floor();
        let dy = (ui.ctx.screen_height / 1.5).floor();

        ui.ensure_in_z_order_with(NPC_DEAL_WINDOW_ID, WindowOrder::Foreground);
        let win = ui.window_fixed(NPC_DEAL_WINDOW_ID, dialog_w, dialog_h, dx, dy);
        ui.interact(NPC_DEAL_WINDOW_ID, win);

        if self.has_grf_textures {
            let (v, i) = draw::quad_vertices(dx, dy, dialog_w, dialog_h, [1.0, 1.0, 1.0, 1.0]);
            ui.draw_calls.push(DrawCall {
                vertices: v.into(),
                indices: i.into(),
                texture: TextureRef::Named(WIN_TEXTURE.into()),
            });
        } else {
            let (v, i) = draw::quad_vertices(dx, dy, dialog_w, dialog_h, [0.2, 0.2, 0.28, 1.0]);
            ui.draw_calls.push(DrawCall {
                vertices: v.into(),
                indices: i.into(),
                texture: TextureRef::White,
            });
            let border_color = [0.5, 0.5, 0.6, 1.0];
            for (bx, by, bw, bh) in [
                (dx, dy, dialog_w, 1.0),
                (dx, dy + dialog_h - 1.0, dialog_w, 1.0),
                (dx, dy, 1.0, dialog_h),
                (dx + dialog_w - 1.0, dy, 1.0, dialog_h),
            ] {
                let (v, i) = draw::quad_vertices(bx, by, bw, bh, border_color);
                ui.draw_calls.push(DrawCall {
                    vertices: v.into(),
                    indices: i.into(),
                    texture: TextureRef::White,
                });
            }
        }

        let container = Rect::new(dx, dy, dialog_w, dialog_h);
        let btns = container.buttons_bottom_right(
            3,
            btn_w,
            btn_h,
            BTN_BOTTOM,
            BTN_FIRST_RIGHT,
            BTN_SPACING,
        );

        let message = "Please select a Deal type";
        let (text_y, text_x) =
            container.text_dialog_alignment(PADDING, btns[0].y, ui.atlas.line_height);
        let text_color = if self.has_grf_textures {
            [0.0, 0.0, 0.0, 1.0]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };
        ui.text(text_x, text_y, message, text_color);

        let cancel = ui.button(DEAL_CANCEL_BTN_ID, btns[0], &CANCEL_BTN, crate::pt!("Cancel", "Cancelar"));
        let sell = ui.button(SELL_BTN_ID, btns[1], &SELL_BTN, crate::pt!("Sell", "Vender"));
        let buy = ui.button(BUY_BTN_ID, btns[2], &BUY_BTN, crate::pt!("Buy", "Comprar"));

        // the arrow keys (a controller's D-pad) walk the buttons as they stand, Enter (A) takes the picked one
        if ui.ctx.key_left {
            self.deal_choice = self.deal_choice.saturating_sub(1);
        }
        if ui.ctx.key_right {
            self.deal_choice = (self.deal_choice + 1).min(2);
        }
        if ui.ctx.pad_labels {
            crate::helper::focus_ring(ui, btns[2 - self.deal_choice]);
        }
        let chosen = if buy.clicked() {
            Some(0)
        } else if sell.clicked() {
            Some(1)
        } else if cancel.clicked() {
            Some(2)
        } else {
            ui.ctx.key_enter.then_some(self.deal_choice)
        };
        if let Some(choice) = chosen {
            if choice < 2 {
                events.push(GameEvent::RequestNpcDealType {
                    npc_id: self.dialog.npc_id,
                    deal_type: choice as u8,
                });
            }
            self.dialog.close();
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InGameWindow;
    use crate::game::chat_window::CHAT_WINDOW_ID;
    use crate::game::inventory_window::INV_WINDOW_ID;
    use crate::game::minimap_window::MINIMAP_WINDOW_ID;
    use nostalre_game::character::Character;
    use nostalre_game::data_table::DataTable;

    use nostalre_ui::context::UiContext;
    use nostalre_ui::state::StateCache;
    use nostalre_ui::test_support::test_frame;

    #[test]
    fn enter_triggers_next() {
        let mut npc = NpcDialog::new();
        npc.dialog.open_text(100, "Hello");
        npc.dialog.wait_for_next(100);

        let mut character = Character::new();
        let data = DataTable::new();
        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_enter = true;
        let mut ui = test_frame(&mut ctx, &mut state);

        let events = npc.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            GameEvent::RequestNpcNext { npc_id: 100 }
        ));
        assert_eq!(npc.dialog.state, NpcDialogState::DisplayingText);
    }

    #[test]
    fn clicking_a_link_opens_what_it_points_at() {
        let mut npc = NpcDialog::new();
        npc.dialog.open_text(
            100,
            "Bring <ITEM>[Knife]<INFO>1201</INFO></ITEM> to the <NAVI>[Tool Shop]<INFO>alberta,98,154,0,000,0</INFO></NAVI>.",
        );
        let mut state = StateCache::new();
        let frame =
            |state: &mut StateCache, npc: &mut NpcDialog, ctx: &mut UiContext| -> Vec<GameEvent> {
                let mut character = Character::new();
                let data = DataTable::new();
                let mut ui = test_frame(ctx, state);
                let z = ui.get_z_order();
                ui.compute_hovered_window(&z);
                npc.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data))
            };
        let mut idle = UiContext::new(800.0, 600.0);
        assert!(frame(&mut state, &mut npc, &mut idle).is_empty());

        let click_link = |state: &mut StateCache, npc: &mut NpcDialog, link: usize| {
            let run = npc
                .say_text
                .layout
                .runs
                .iter()
                .find(|r| r.kind == RunKind::Link(link))
                .unwrap()
                .clone();
            let mut click = UiContext::new(800.0, 600.0);
            click.mouse_x = DIALOG_DEFAULT_X + PADDING + run.x + run.width / 2.0;
            click.mouse_y = DIALOG_DEFAULT_Y
                + PADDING
                + run.line as f32 * TEXT_LINE_HEIGHT
                + TEXT_LINE_HEIGHT / 2.0;
            click.mouse_clicked = true;
            frame(state, npc, &mut click)
        };
        assert!(matches!(
            click_link(&mut state, &mut npc, 0).as_slice(),
            [GameEvent::ShowItemInfoDirect { item }] if item.item_id == 1201
        ));
        assert!(matches!(
            click_link(&mut state, &mut npc, 1).as_slice(),
            [GameEvent::NavigationGuide { navigation }] if navigation.map == "alberta"
        ));
        assert!(npc.dialog.is_open());
    }

    #[test]
    fn escape_cancels_menu() {
        let mut npc = NpcDialog::new();
        npc.dialog.show_menu(100, vec!["Buy".into(), "Sell".into()]);

        let mut character = Character::new();
        let data = DataTable::new();
        let mut ctx = crate::BuildCtx::test(&mut character, &data);

        assert!(npc.wants_escape(&ctx));
        let events = npc.on_escape(&mut ctx);
        assert_eq!(events.len(), 1);
        match &events[0] {
            GameEvent::RequestNpcMenuSelect { npc_id, choice } => {
                assert_eq!(*npc_id, 100);
                assert_eq!(*choice, 255);
            }
            other => panic!("expected RequestNpcMenuSelect, got {other:?}"),
        }
        assert!(!npc.dialog.is_open());
    }

    #[test]
    fn escape_cancels_deal_type() {
        let mut npc = NpcDialog::new();
        npc.dialog.show_deal_type(100);

        let mut character = Character::new();
        let data = DataTable::new();
        let mut ctx = crate::BuildCtx::test(&mut character, &data);

        assert!(npc.wants_escape(&ctx));
        let events = npc.on_escape(&mut ctx);
        assert!(events.is_empty());
        assert!(!npc.dialog.is_open());
    }

    #[test]
    fn menu_with_many_items_keeps_scroll_offset_bounded() {
        let mut npc = NpcDialog::new();
        let items: Vec<String> = (1..=10).map(|i| format!("Item {i}")).collect();
        npc.dialog.show_menu(100, items);

        let mut character = Character::new();
        let data = DataTable::new();
        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut ui = test_frame(&mut ctx, &mut state);

        npc.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert_eq!(npc.dialog.menu_scroll_offset, 0);
        assert_eq!(npc.dialog.selected_menu_index, 0);

        let mut ctx2 = UiContext::new(800.0, 600.0);
        ctx2.key_down = true;
        let mut ui2 = test_frame(&mut ctx2, &mut state);

        for _ in 0..6 {
            npc.build(&mut ui2, &mut crate::BuildCtx::test(&mut character, &data));
        }
        assert_eq!(npc.dialog.selected_menu_index, 6);
        assert_eq!(npc.dialog.menu_scroll_offset, 3);
    }

    #[test]
    fn menu_defaults_to_fixed_rect_and_drags_by_body_only() {
        let mut npc = NpcDialog::new();
        npc.dialog.show_menu(100, vec!["Buy".into(), "Sell".into()]);

        let mut character = Character::new();
        let data = DataTable::new();
        let mut state = StateCache::new();

        let mut ctx = UiContext::new(800.0, 600.0);
        let mut ui = test_frame(&mut ctx, &mut state);
        npc.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert_eq!(
            state.extract_window_positions().get(&NPC_MENU_WINDOW_ID.0),
            Some(&[MENU_DEFAULT_X, MENU_DEFAULT_Y])
        );

        let mut press_body = UiContext::new(800.0, 600.0);
        press_body.mouse_x = 210.0;
        press_body.mouse_y = 400.0;
        press_body.mouse_clicked = true;
        press_body.mouse_down = true;
        let mut ui = test_frame(&mut press_body, &mut state);
        npc.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));

        let mut drag = UiContext::new(800.0, 600.0);
        drag.mouse_x = 250.0;
        drag.mouse_y = 440.0;
        drag.mouse_down = true;
        let mut ui = test_frame(&mut drag, &mut state);
        npc.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert_eq!(
            state.extract_window_positions().get(&NPC_MENU_WINDOW_ID.0),
            Some(&[MENU_DEFAULT_X + 40.0, MENU_DEFAULT_Y + 40.0])
        );

        let mut release = UiContext::new(800.0, 600.0);
        let mut ui = test_frame(&mut release, &mut state);
        npc.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));

        let mut press_row = UiContext::new(800.0, 600.0);
        press_row.mouse_x = 340.0;
        press_row.mouse_y = 355.0;
        press_row.mouse_clicked = true;
        press_row.mouse_down = true;
        let mut ui = test_frame(&mut press_row, &mut state);
        npc.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));

        let mut drag_row = UiContext::new(800.0, 600.0);
        drag_row.mouse_x = 400.0;
        drag_row.mouse_y = 415.0;
        drag_row.mouse_down = true;
        let mut ui = test_frame(&mut drag_row, &mut state);
        npc.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert_eq!(
            state.extract_window_positions().get(&NPC_MENU_WINDOW_ID.0),
            Some(&[MENU_DEFAULT_X + 40.0, MENU_DEFAULT_Y + 40.0]),
            "pressing a menu row must not drag the window"
        );
    }

    #[test]
    fn menu_row_stays_clickable_after_another_window_is_raised() {
        let mut npc = NpcDialog::new();
        npc.dialog.show_menu(100, vec!["Buy".into(), "Sell".into()]);
        let mut state = StateCache::new();

        let frame = |state: &mut StateCache, npc: &mut NpcDialog, ctx: &mut UiContext| -> bool {
            let mut character = Character::new();
            let data = DataTable::new();
            let mut ui = test_frame(ctx, state);
            let z = ui.get_z_order();
            ui.compute_hovered_window(&z);
            ui.window_at(INV_WINDOW_ID, 280.0, 240.0, 15.0, 190.0, 290.0);
            npc.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
            ui.any_hovered
        };

        let mut raise_inventory = UiContext::new(800.0, 600.0);
        raise_inventory.mouse_x = 300.0;
        raise_inventory.mouse_y = 500.0;
        raise_inventory.mouse_clicked = true;
        frame(&mut state, &mut npc, &mut raise_inventory);

        let mut click_row = UiContext::new(800.0, 600.0);
        click_row.mouse_x = 300.0;
        click_row.mouse_y = 330.0;
        click_row.mouse_clicked = true;
        let claimed = frame(&mut state, &mut npc, &mut click_row);
        assert_eq!(npc.dialog.selected_menu_index, 1);
        assert!(claimed, "the menu body must claim the pointer");
    }

    #[test]
    fn deal_type_keys_walk_the_buttons_and_enter_takes_one() {
        let mut npc = NpcDialog::new();
        npc.dialog.show_deal_type(100);
        let mut state = StateCache::new();
        let mut frame = |keys: fn(&mut UiContext)| {
            let mut ctx = UiContext::new(800.0, 600.0);
            keys(&mut ctx);
            let mut character = Character::new();
            let data = DataTable::new();
            let mut ui = test_frame(&mut ctx, &mut state);
            npc.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data))
        };

        assert!(frame(|c| c.key_right = true).is_empty());
        let events = frame(|c| c.key_enter = true);
        assert!(
            matches!(
                events.as_slice(),
                [GameEvent::RequestNpcDealType {
                    npc_id: 100,
                    deal_type: 1
                }]
            ),
            "Right then Enter sells, got {events:?}"
        );
    }

    #[test]
    fn deal_type_buttons_work_under_a_raised_window() {
        let mut npc = NpcDialog::new();
        npc.dialog.show_deal_type(100);
        let mut state = StateCache::new();

        let frame =
            |state: &mut StateCache, npc: &mut NpcDialog, ctx: &mut UiContext| -> Vec<GameEvent> {
                let mut character = Character::new();
                let data = DataTable::new();
                let mut ui = test_frame(ctx, state);
                let z = ui.get_z_order();
                ui.compute_hovered_window(&z);
                ui.window_at(CHAT_WINDOW_ID, 560.0, 200.0, 15.0, 0.0, 400.0);
                ui.window_at(MINIMAP_WINDOW_ID, 200.0, 200.0, 15.0, 600.0, 0.0);
                npc.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data))
            };

        let mut raise_chat = UiContext::new(800.0, 600.0);
        raise_chat.mouse_x = 50.0;
        raise_chat.mouse_y = 550.0;
        raise_chat.mouse_clicked = true;
        frame(&mut state, &mut npc, &mut raise_chat);

        let mut click_buy = UiContext::new(800.0, 600.0);
        click_buy.mouse_x = 410.0;
        click_buy.mouse_y = 500.0;
        click_buy.mouse_clicked = true;
        let events = frame(&mut state, &mut npc, &mut click_buy);
        assert!(
            matches!(
                events.as_slice(),
                [GameEvent::RequestNpcDealType {
                    npc_id: 100,
                    deal_type: 0
                }]
            ),
            "expected a buy deal type, got {events:?}"
        );
    }

    #[test]
    fn double_clicking_a_menu_row_selects_it_and_replies() {
        let mut npc = NpcDialog::new();
        npc.dialog.show_menu(100, vec!["Buy".into(), "Sell".into()]);

        let mut state = StateCache::new();

        let frame =
            |state: &mut StateCache, npc: &mut NpcDialog, ctx: &mut UiContext| -> Vec<GameEvent> {
                let mut character = Character::new();
                let data = DataTable::new();
                let mut ui = test_frame(ctx, state);
                let z = ui.get_z_order();
                ui.compute_hovered_window(&z);
                npc.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data))
            };

        let mut idle = UiContext::new(800.0, 600.0);
        frame(&mut state, &mut npc, &mut idle);

        let mut double_click = UiContext::new(800.0, 600.0);
        double_click.mouse_x = 300.0;
        double_click.mouse_y = 330.0;
        double_click.mouse_clicked = true;
        double_click.mouse_double_clicked = true;
        let events = frame(&mut state, &mut npc, &mut double_click);

        assert!(
            matches!(
                events.as_slice(),
                [GameEvent::RequestNpcMenuSelect {
                    npc_id: 100,
                    choice: 2
                }]
            ),
            "expected the second entry to be picked, got {events:?}"
        );
        assert!(!npc.dialog.is_open());
    }

    #[test]
    fn menu_mouse_wheel_scrolls_and_persists() {
        let mut npc = NpcDialog::new();
        let items: Vec<String> = (1..=10).map(|i| format!("Item {i}")).collect();
        npc.dialog.show_menu(100, items);

        let mut character = Character::new();
        let data = DataTable::new();
        let mut state = StateCache::new();

        let mut ctx = UiContext::new(800.0, 600.0);
        let mut ui = test_frame(&mut ctx, &mut state);
        npc.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert_eq!(npc.dialog.menu_scroll_offset, 0);

        let mut ctx2 = UiContext::new(800.0, 600.0);
        ctx2.mouse_x = 300.0;
        ctx2.mouse_y = 340.0;
        ctx2.scroll_delta = -1.0; // scroll down
        let mut ui2 = test_frame(&mut ctx2, &mut state);
        npc.build(&mut ui2, &mut crate::BuildCtx::test(&mut character, &data));
        assert_eq!(
            npc.dialog.menu_scroll_offset, 1,
            "mouse wheel should scroll down"
        );

        let mut ctx3 = UiContext::new(800.0, 600.0);
        let mut ui3 = test_frame(&mut ctx3, &mut state);
        npc.build(&mut ui3, &mut crate::BuildCtx::test(&mut character, &data));
        assert_eq!(
            npc.dialog.menu_scroll_offset, 1,
            "scroll offset must persist across frames"
        );
    }
}
