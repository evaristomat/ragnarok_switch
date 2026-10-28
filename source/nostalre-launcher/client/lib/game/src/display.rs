use serde::{Deserialize, Serialize};

/// Which HUD the game screen draws: kRO's windows, or our frameless one (`docs/nostalre-hud.md`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HudLayout {
    Classic,
    #[default]
    Modern,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayOptions {
    pub show_other_damage: bool,
    pub show_other_cast_bars: bool,
    pub hide_name_player: bool,
    pub hide_name_monster: bool,
    pub hide_name_npc: bool,
    pub show_level_aura: bool,
}

impl Default for DisplayOptions {
    fn default() -> Self {
        Self {
            show_other_damage: true,
            show_other_cast_bars: true,
            hide_name_player: false,
            hide_name_monster: false,
            hide_name_npc: false,
            show_level_aura: true,
        }
    }
}

/// The graphics options changed together: the classic Graphic Settings window, the menu's Settings, the chat
/// commands (`/effect`, `/fog`...)
#[derive(Debug, Clone, Default)]
pub struct GraphicsSettings {
    /// Percent
    pub ui_scale: f32,
    pub fullscreen: bool,
    pub fog: bool,
    pub show_skill_effects: bool,
    pub display: DisplayOptions,
    pub snap: crate::cursor::MouseSnapPrefs,
    pub refuse_trade: bool,
    pub refuse_party_invite: bool,
    /// Bold name plates
    pub accessibility: bool,
    pub filter_world: bool,
    pub filter_effects: bool,
    pub filter_sprites: bool,
    pub sprite_upscale: bool,
}

/// The sound options changed together: the classic Sound window, the menu's Settings
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SoundSettings {
    /// 0 to 1
    pub bgm_volume: f32,
    pub sfx_volume: f32,
    pub bgm_enabled: bool,
    pub sfx_enabled: bool,
    pub stereo: bool,
    pub play_when_unfocused: bool,
}
