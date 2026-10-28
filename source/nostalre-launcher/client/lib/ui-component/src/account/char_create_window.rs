use crate::Window;
use crate::helper::window_chrome;
use nostalre_game::event::GameEvent;
use nostalre_ui::frame::{ButtonTextures, TextInputBg, UiFrame, WidgetId};
use nostalre_ui::rect::Rect;
use nostalre_ui::text_input::TextInput;

// login_interface/win_make2.bmp, compact 150x286
const WIN_W: f32 = 150.0;
const HEADER_H: f32 = 17.0;
const CONTENT_H: f32 = 240.0;
const FOOTER_H: f32 = 29.0;
const WIN_H: f32 = HEADER_H + CONTENT_H + FOOTER_H;
const NAME_X: f32 = 40.0;
const NAME_Y: f32 = 142.0;
const NAME_W: f32 = 96.0;
const NAME_H: f32 = 18.0;
const PREVIEW_X: f32 = 40.0;
const PREVIEW_Y: f32 = -10.0;
const PREVIEW_ANCHOR_X: f32 = 32.0;
const PREVIEW_ANCHOR_Y: f32 = 115.0;
const STYLE_Y: f32 = 185.0;
const COLOR_Y: f32 = 225.0;
const ARROW_L_X: f32 = 15.0;
const ARROW_R_X: f32 = 125.0;

const ARROW_W: f32 = 13.0;
const ARROW_H: f32 = 13.0;

const BTN_W: f32 = 42.0;
const BTN_H: f32 = 20.0;
const BTN_BOTTOM: f32 = 4.0;
const MAKE_RIGHT: f32 = 50.0;
const CANCEL_RIGHT: f32 = 4.0;

const NAME_MAX_LEN: usize = 23;
const HEAD_MIN: u16 = 1;
const HEAD_MAX: u16 = 25;
const HAIR_COLOR_COUNT: u16 = 9;

pub const CHAR_CREATE_WINDOW_ID: WidgetId = WidgetId(4500);
const NAME_ID: WidgetId = WidgetId(4501);
const STYLE_L_ID: WidgetId = WidgetId(4502);
const STYLE_R_ID: WidgetId = WidgetId(4503);
const COLOR_L_ID: WidgetId = WidgetId(4504);
const MAKE_ID: WidgetId = WidgetId(4505);
const CANCEL_ID: WidgetId = WidgetId(4506);
const COLOR_R_ID: WidgetId = WidgetId(4507);
const JOB_ID: WidgetId = WidgetId(4509);
const SEX_ID: WidgetId = WidgetId(4510);
#[cfg(any(target_os = "horizon", test))]
const NAME_KEYBOARD_ID: WidgetId = WidgetId(4511);
#[cfg(any(target_os = "horizon", test))]
const KEYBOARD_W: f32 = 396.0;
#[cfg(any(target_os = "horizon", test))]
const KEYBOARD_H: f32 = 232.0;

pub const JOB_NOVICE: u16 = 0;
pub const JOB_SUMMONER: u16 = 4218;
const CHOICE_W: f32 = 56.0;
const CHOICE_H: f32 = 16.0;

const WIN2_TEXTURE: &str = nostalre_resources::ui::login::WIN_MAKE2;
const NAME_EDIT_TEXTURE: &str = nostalre_resources::ui::login::NAME_EDIT;
const ARROW_L_TEXTURE: &str = nostalre_resources::ui::SCROLL1LEFT;
const ARROW_R_TEXTURE: &str = nostalre_resources::ui::SCROLL1RIGHT;

const MAKE_BTN: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_MAKE,
    hover: nostalre_resources::ui::BTN_MAKE_A,
    pressed: nostalre_resources::ui::BTN_MAKE_B,
};
const CANCEL_BTN: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_CANCEL,
    hover: nostalre_resources::ui::BTN_CANCEL_A,
    pressed: nostalre_resources::ui::BTN_CANCEL_B,
};
const ARROW_L_BTN: ButtonTextures = ButtonTextures {
    normal: ARROW_L_TEXTURE,
    hover: ARROW_L_TEXTURE,
    pressed: ARROW_L_TEXTURE,
};
const ARROW_R_BTN: ButtonTextures = ButtonTextures {
    normal: ARROW_R_TEXTURE,
    hover: ARROW_R_TEXTURE,
    pressed: ARROW_R_TEXTURE,
};

const ERROR_COLOR: [f32; 4] = [0.8, 0.2, 0.2, 1.0];

pub struct CharCreateWindow {
    pub name: TextInput,
    pub slot: u8,
    pub hair_style: u16,
    pub hair_color: u16,
    pub error_message: Option<String>,
    pub has_grf_textures: bool,
    /// Starting job, chosen per character: Novice (Human) or Summoner (Doram).
    pub job: u16,
    /// 0 female, 1 male.
    pub sex: u8,
    win_origin: (f32, f32),
    #[cfg(any(target_os = "horizon", test))]
    name_uppercase: bool,
}

impl CharCreateWindow {
    /// The window for `slot`, its sex starting from the account's.
    pub fn new(slot: u8, account_sex: u8) -> Self {
        Self {
            name: TextInput::new(NAME_MAX_LEN, false),
            slot,
            hair_style: HEAD_MIN,
            hair_color: 0,
            error_message: None,
            has_grf_textures: false,
            job: JOB_NOVICE,
            sex: account_sex,
            win_origin: (0.0, 0.0),
            #[cfg(any(target_os = "horizon", test))]
            name_uppercase: false,
        }
    }

    /// Screen-space feet anchor for the rotating preview sprite.
    pub fn preview_anchor(&self) -> [f32; 2] {
        [
            self.win_origin.0 + PREVIEW_X + PREVIEW_ANCHOR_X,
            self.win_origin.1 + HEADER_H + PREVIEW_Y + PREVIEW_ANCHOR_Y,
        ]
    }

    fn cycle_head(&mut self, delta: i32) {
        let span = (HEAD_MAX - HEAD_MIN + 1) as i32;
        let cur = (self.hair_style - HEAD_MIN) as i32;
        self.hair_style = HEAD_MIN + (cur + delta).rem_euclid(span) as u16;
    }

    fn cycle_color(&mut self, delta: i32) {
        let n = HAIR_COLOR_COUNT as i32;
        self.hair_color = (self.hair_color as i32 + delta).rem_euclid(n) as u16;
    }

    fn try_submit(&mut self, submit: bool, events: &mut Vec<GameEvent>) {
        if !submit {
            return;
        }
        if self.name.text.trim().is_empty() {
            self.error_message = Some(crate::pt!("Please enter a name.", "Digite um nome.").into());
        } else {
            events.push(GameEvent::RequestMakeCharacter {
                name: self.name.text.clone(),
                slot: self.slot,
                hair_style: self.hair_style,
                hair_color: self.hair_color,
                job: JOB_NOVICE,
                sex: self.sex,
            });
        }
    }

    pub fn build(&mut self, ui: &mut UiFrame) -> Vec<GameEvent> {
        let mut events = Vec::new();
        let prev_grf = ui.has_grf_textures;
        ui.has_grf_textures = self.has_grf_textures;
        #[cfg(not(any(target_os = "horizon", test)))]
        let win = ui.window_account(CHAR_CREATE_WINDOW_ID, WIN_W, WIN_H, HEADER_H);
        #[cfg(any(target_os = "horizon", test))]
        let win = {
            // Keep the name and preview beside the keyboard, including at 150% UI scale.
            let x = ((ui.ctx.screen_width - WIN_W - 16.0 - KEYBOARD_W) / 2.0).max(0.0).floor();
            let y = ((ui.ctx.screen_height - WIN_H) / 2.0).max(0.0).floor();
            ui.window_fixed(CHAR_CREATE_WINDOW_ID, WIN_W, WIN_H, x, y)
        };
        self.win_origin = (win.x, win.y);
        let (ox, oy) = (win.x, win.y);
        let has_grf = self.has_grf_textures;
        let content_y = oy + HEADER_H;
        let footer_y = content_y + CONTENT_H;

        window_chrome::draw_titlebar(ui, ox, oy, WIN_W, HEADER_H, has_grf);
        ui.text(
            ox + 18.0,
            oy + 2.0 + ui.atlas.ascent,
            crate::pt!("New Character", "Novo personagem"),
            window_chrome::text_color(has_grf),
        );

        if has_grf {
            window_chrome::draw_textured_quad(ui, ox, content_y, WIN_W, CONTENT_H, WIN2_TEXTURE);
        } else {
            window_chrome::draw_container(ui, ox, content_y, WIN_W, CONTENT_H, has_grf);
        }
        window_chrome::draw_footer(ui, ox, footer_y, WIN_W, FOOTER_H, has_grf);

        let name_bg = if has_grf {
            TextInputBg::Texture(NAME_EDIT_TEXTURE)
        } else {
            TextInputBg::Default
        };
        let name_rect = Rect::new(ox + NAME_X, content_y + NAME_Y - NAME_H, NAME_W, NAME_H);
        ui.text_input(NAME_ID, name_rect, &mut self.name, name_bg);

        let style_l = Rect::new(
            ox + ARROW_L_X,
            content_y + STYLE_Y - ARROW_H,
            ARROW_W,
            ARROW_H,
        );
        let style_r = Rect::new(
            ox + ARROW_R_X,
            content_y + STYLE_Y - ARROW_H,
            ARROW_W,
            ARROW_H,
        );
        if ui.button(STYLE_L_ID, style_l, &ARROW_L_BTN, "<").clicked() {
            self.cycle_head(-1);
        }
        if ui.button(STYLE_R_ID, style_r, &ARROW_R_BTN, ">").clicked() {
            self.cycle_head(1);
        }
        let color_l = Rect::new(
            ox + ARROW_L_X,
            content_y + COLOR_Y - ARROW_H,
            ARROW_W,
            ARROW_H,
        );
        let color_r = Rect::new(
            ox + ARROW_R_X,
            content_y + COLOR_Y - ARROW_H,
            ARROW_W,
            ARROW_H,
        );
        if ui.button(COLOR_L_ID, color_l, &ARROW_L_BTN, "<").clicked() {
            self.cycle_color(-1);
        }
        if ui.button(COLOR_R_ID, color_r, &ARROW_R_BTN, ">").clicked() {
            self.cycle_color(1);
        }

        // Classic creation is human novice only.
        self.job = JOB_NOVICE;
        let job_rect = Rect::new(ox + 8.0, content_y + 4.0, CHOICE_W, CHOICE_H);
        ui.text_button(JOB_ID, job_rect, "Humano");
        let sex = if self.sex == 1 { crate::pt!("Male", "Masculino") } else { crate::pt!("Female", "Feminino") };
        let sex_rect = Rect::new(
            ox + WIN_W - 8.0 - CHOICE_W,
            content_y + 4.0,
            CHOICE_W,
            CHOICE_H,
        );
        if ui.text_button(SEX_ID, sex_rect, sex).clicked() {
            self.sex = 1 - self.sex.min(1);
        }

        if let Some(msg) = &self.error_message {
            ui.text(
                ox + 8.0,
                content_y + NAME_Y + NAME_H + ui.atlas.line_height,
                msg,
                ERROR_COLOR,
            );
        }

        let btn_y = footer_y + FOOTER_H - BTN_BOTTOM - BTN_H;
        let make = Rect::new(ox + WIN_W - MAKE_RIGHT - BTN_W, btn_y, BTN_W, BTN_H);
        let cancel = Rect::new(ox + WIN_W - CANCEL_RIGHT - BTN_W, btn_y, BTN_W, BTN_H);
        let submit = ui.button(MAKE_ID, make, &MAKE_BTN, crate::pt!("Make", "Criar")).clicked() || ui.ctx.key_enter;
        self.try_submit(submit, &mut events);
        if ui
            .button(CANCEL_ID, cancel, &CANCEL_BTN, crate::pt!("Cancel", "Cancelar"))
            .clicked()
            || ui.ctx.key_escape
        {
            events.push(GameEvent::CancelCreateCharacter);
        }
        ui.has_grf_textures = prev_grf;
        #[cfg(any(target_os = "horizon", test))]
        self.build_name_keyboard(ui);
        events
    }

    /// Always available during creation on Switch: no library applet or L3 is required.
    /// Both a touch and the controller's right-stick cursor use these normal UI buttons.
    #[cfg(any(target_os = "horizon", test))]
    fn build_name_keyboard(&mut self, ui: &mut UiFrame) {
        let x = self.win_origin.0 + WIN_W + 16.0;
        let y = ((ui.ctx.screen_height - KEYBOARD_H) / 2.0).max(0.0).floor();
        let win = ui.window_fixed(NAME_KEYBOARD_ID, KEYBOARD_W, KEYBOARD_H, x, y);
        window_chrome::draw_container(ui, win.x, win.y, win.w, win.h, false);
        let color = window_chrome::text_color(false);
        ui.text(x + 8.0, y + 8.0 + ui.atlas.ascent, "Nome do personagem", color);
        ui.text(x + 8.0, y + 25.0 + ui.atlas.ascent, "Toque ou use o analogico direito + A", color);
        for (row, keys) in ["1234567890", "qwertyuiop", "asdfghjkl_", "zxcvbnm.-"].iter().enumerate() {
            for (column, ch) in keys.chars().enumerate() {
                let ch = if self.name_uppercase { ch.to_ascii_uppercase() } else { ch };
                let rect = Rect::new(x + 8.0 + column as f32 * 38.0, y + 48.0 + row as f32 * 34.0, 36.0, 32.0);
                if ui.text_button(WidgetId(4520 + row as u32 * 10 + column as u32), rect, &ch.to_string()).clicked() {
                    self.append_name_char(ch);
                }
            }
        }
        let row_y = y + 188.0;
        let shift = if self.name_uppercase { "Minusculas" } else { "Maiusculas" };
        if ui.text_button(WidgetId(4560), Rect::new(x + 8.0, row_y, 120.0, 32.0), shift).clicked() {
            self.name_uppercase = !self.name_uppercase;
        }
        if ui.text_button(WidgetId(4561), Rect::new(x + 138.0, row_y, 120.0, 32.0), "Espaco").clicked() {
            self.append_name_char(' ');
        }
        if ui.text_button(WidgetId(4562), Rect::new(x + 268.0, row_y, 120.0, 32.0), "Apagar").clicked() {
            self.name.text.pop();
            self.name.move_cursor_to_end();
            self.name.clear_selection();
            self.error_message = None;
        }
    }

    #[cfg(any(target_os = "horizon", test))]
    fn append_name_char(&mut self, ch: char) {
        if self.name.text.len() + ch.len_utf8() <= self.name.max_len {
            self.name.text.push(ch);
            self.name.move_cursor_to_end();
            self.name.clear_selection();
            self.error_message = None;
        }
    }
}

impl Window for CharCreateWindow {
    fn has_grf_textures(&self) -> bool {
        self.has_grf_textures
    }
    fn set_has_grf_textures(&mut self, value: bool) {
        self.has_grf_textures = value;
    }

    fn window_size(&self) -> (f32, f32) {
        (WIN_W, WIN_H)
    }

    fn grf_texture_paths() -> Vec<&'static str> {
        vec![
            WIN2_TEXTURE,
            NAME_EDIT_TEXTURE,
            ARROW_L_TEXTURE,
            ARROW_R_TEXTURE,
            window_chrome::TITLEBAR_TEX,
            window_chrome::FOOTER_TEX,
            window_chrome::SYS_BASE_OFF_TEX,
            window_chrome::SYS_BASE_ON_TEX,
            MAKE_BTN.normal,
            MAKE_BTN.hover,
            MAKE_BTN.pressed,
            CANCEL_BTN.normal,
            CANCEL_BTN.hover,
            CANCEL_BTN.pressed,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use nostalre_ui::context::UiContext;
    use nostalre_ui::state::StateCache;
    use nostalre_ui::test_support::test_frame;

    #[test]
    fn head_cycle_wraps() {
        let mut win = CharCreateWindow::new(0, 0);
        assert_eq!(win.hair_style, HEAD_MIN);
        win.cycle_head(-1);
        assert_eq!(win.hair_style, HEAD_MAX);
        win.cycle_head(1);
        assert_eq!(win.hair_style, HEAD_MIN);
    }

    #[test]
    fn color_cycle_wraps_both_ways() {
        let mut win = CharCreateWindow::new(0, 0);
        win.cycle_color(-1);
        assert_eq!(win.hair_color, HAIR_COLOR_COUNT - 1);
        win.cycle_color(1);
        assert_eq!(win.hair_color, 0);
    }

    #[test]
    fn make_emits_request() {
        let mut win = CharCreateWindow::new(1, 0);
        win.name.text = "Novice".into();
        let mut state = StateCache::new();

        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_enter = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = win.build(&mut ui);

        assert!(events.iter().any(|e| matches!(
            e,
            GameEvent::RequestMakeCharacter { slot: 1, name, .. } if name == "Novice"
        )));
    }

    #[test]
    fn make_with_empty_name_sets_error_and_no_event() {
        let mut win = CharCreateWindow::new(0, 0);
        let mut state = StateCache::new();

        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_enter = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = win.build(&mut ui);

        assert!(win.error_message.is_some());
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, GameEvent::RequestMakeCharacter { .. }))
        );
    }
}
