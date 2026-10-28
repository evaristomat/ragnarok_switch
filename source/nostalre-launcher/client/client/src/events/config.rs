use crate::App;
use crate::config::Config;
use std::time::Duration;

/// How long `save_config_soon` waits for the changes to stop
const CONFIG_SAVE_DELAY: Duration = Duration::from_millis(500);
use nostalre_game::event::SelfConfigKind;
use nostalre_ui_component::menu::SettingsValues;

impl App {
    pub(super) fn handle_self_config_changed(&mut self, kind: SelfConfigKind, enabled: bool) {
        let config = &mut self.game.prefs.self_config;
        match kind {
            SelfConfigKind::RefusePartyInvite => config.refuse_party_invite = enabled,
            SelfConfigKind::OpenEquipmentWindow => config.open_equipment_window = enabled,
            SelfConfigKind::Call => config.call_enabled = enabled,
            SelfConfigKind::PetAutofeed => config.pet_autofeed = enabled,
            SelfConfigKind::HomunculusAutofeed => config.homun_autofeed = enabled,
            SelfConfigKind::HideCostumes => config.hide_costumes = enabled,
        }
        self.windows
            .equipment_window
            .set_self_config(config.open_equipment_window, config.hide_costumes);
    }
}

/// What the menu's Settings shows of `config`
fn settings_values(config: &Config) -> SettingsValues {
    SettingsValues {
        graphics: config.graphics(),
        sound: config.sound(),
        keys: config.keybindings.clone(),
        chat_lines: config.shortcut_commands.clone(),
    }
}

impl App {
    /// The menu's Settings reads the config each time it opens; its resets go back to a fresh config's values
    pub(crate) fn fill_menu_settings(&mut self) {
        if self.windows.menu.wants_settings() {
            let now = settings_values(&self.config);
            let defaults = settings_values(&Config::default());
            self.windows.menu.set_settings(now, defaults);
        }
    }

    /// The Keys page or the classic Keys window changed a key
    pub(crate) fn apply_key_bindings(&mut self, keys: nostalre_game::keybinding::KeyBindings) {
        self.config.keybindings = keys;
        self.windows.menu.set_key_labels(&self.config.keybindings);
        self.save_config_soon();
    }

    /// Writes `config.json` once the changes stop for a moment (a volume held down steps many times), or at
    /// once on close and suspend (`flush_config`)
    pub(crate) fn save_config_soon(&mut self) {
        self.config_save_due = Some(std::time::Instant::now() + CONFIG_SAVE_DELAY);
    }

    /// The save `save_config_soon` asked for, when it is due (`now`) or at once (`None`)
    pub(crate) fn flush_config(&mut self, now: Option<std::time::Instant>) {
        if self
            .config_save_due
            .is_some_and(|due| now.is_none_or(|now| now >= due))
        {
            self.config_save_due = None;
            self.config.save("config.json");
        }
    }

    pub(crate) fn apply_sound_settings(
        &mut self,
        settings: nostalre_game::display::SoundSettings,
        persist: bool,
    ) {
        self.config.bgm_volume = settings.bgm_volume;
        self.config.sfx_volume = settings.sfx_volume;
        self.config.bgm_enabled = settings.bgm_enabled;
        self.config.sfx_enabled = settings.sfx_enabled;
        self.config.custom.sound.stereo = settings.stereo;
        self.config.custom.sound.play_when_unfocused = settings.play_when_unfocused;
        self.sound.set_stereo(settings.stereo);
        self.sound.set_volumes(
            self.config.effective_bgm_volume(),
            self.config.effective_sfx_volume(),
        );
        self.apply_sound_pause();
        if persist {
            self.save_config_soon();
        }
    }

    pub(crate) fn open_sound_options(&mut self) {
        self.windows.sound_options.set_values(self.config.sound());
        self.windows.sound_options.toggle();
    }

    pub(crate) fn open_graphic_options(&mut self) {
        if !self.windows.graphic_options.open {
            let settings = self.config.graphics();
            self.windows.graphic_options.set_values(&settings);
            self.windows
                .graphic_options
                .set_hud_layout(self.config.hud_layout);
        }
        self.windows.graphic_options.toggle();
    }

    pub(crate) fn apply_graphics_settings(
        &mut self,
        settings: nostalre_game::display::GraphicsSettings,
        persist: bool,
    ) {
        let nostalre_game::display::GraphicsSettings {
            ui_scale,
            fullscreen,
            fog,
            show_skill_effects,
            display,
            snap,
            refuse_trade,
            refuse_party_invite,
            accessibility,
            filter_world,
            filter_effects,
            filter_sprites,
            sprite_upscale,
        } = settings;
        let fullscreen_changed = fullscreen != self.config.fullscreen;
        let aura_changed = display.show_level_aura != self.config.display.show_level_aura;
        let ui_scale_changed = ui_scale != self.config.dpi_scale;
        let world_filter_changed = filter_world != self.config.custom.filtering.world;
        let effect_filter_changed = filter_effects != self.config.custom.filtering.effects;
        let sprite_filter_changed = filter_sprites != self.config.custom.filtering.sprites;
        let upscale_changed = sprite_upscale != self.config.custom.filtering.sprite_upscale;

        self.config.dpi_scale = ui_scale;
        self.config.fullscreen = fullscreen;
        self.config.fog = fog;
        self.config.show_skill_effects = show_skill_effects;
        self.config.display = display;
        self.config.snap = snap;
        self.config.refuse_trade = refuse_trade;
        self.config.refuse_party_invite = refuse_party_invite;
        self.config.accessibility.bold_name_plates = accessibility;
        self.config.custom.filtering.world = filter_world;
        self.config.custom.filtering.effects = filter_effects;
        self.config.custom.filtering.sprites = filter_sprites;
        self.config.custom.filtering.sprite_upscale = sprite_upscale;
        self.game.prefs.self_config.refuse_party_invite = refuse_party_invite;

        if let Some(window) = &self.window {
            if fullscreen_changed {
                window.set_fullscreen(
                    fullscreen.then(|| winit::window::Fullscreen::Borderless(None)),
                );
            }
        }
        if ui_scale_changed {
            let new_dpi = ui_scale / 100.0;
            if let Some(renderer) = &mut self.renderer {
                renderer.set_dpi_scale(new_dpi);
                let phys_w = renderer.device.surface_config.width as f32;
                let phys_h = renderer.device.surface_config.height as f32;
                if let Some(ui_ctx) = &mut self.ui_context {
                    ui_ctx.dpi_scale = new_dpi;
                    ui_ctx.screen_width = phys_w / new_dpi;
                    ui_ctx.screen_height = phys_h / new_dpi;
                }
            }
        }
        if let Some(renderer) = &mut self.renderer {
            renderer.set_fog(if fog { self.map_fog } else { None });
            if world_filter_changed {
                renderer.set_world_filtering(filter_world, self.grf.as_deref());
            }
            if effect_filter_changed {
                renderer.set_effect_filtering(filter_effects, self.grf.as_deref());
            }
        }
        if effect_filter_changed {
            self.str_effects.set_filtering(filter_effects);
            self.effect_sprites.set_filtering(filter_effects);
            if let (Some(renderer), Some(grf)) = (&self.renderer, &self.grf) {
                self.effect_sprites.reload(
                    grf,
                    &renderer.device.device,
                    &renderer.device.queue,
                    &renderer.texture_cache.bind_group_layout,
                );
            }
        }
        if sprite_filter_changed || upscale_changed {
            let sharpen = sprite_upscale && filter_sprites;
            if let Some(renderer) = &mut self.renderer {
                renderer.set_sprite_upscale(sharpen);
            }
            if nostalre_profiling::debug::trace_sprite_scale() {
                tracing::info!("[sprite-scale] shader upscale={sharpen}");
            }
        }
        if sprite_filter_changed {
            nostalre_renderer::sprite::set_filtering(filter_sprites);
            // `load_missing_entity_sprites` rebuilds every entity but the player
            // on the next frame.
            self.game.sprite_caches.sprites.clear();
            self.game.sprite_caches.sprite_cache.clear();
            self.game.sprite_caches.guild_head_sprites.clear();
            if let Some(gid) = self.game.world.entities.player_id() {
                self.reload_player_sprite(gid);
            }
        }
        if upscale_changed {
            let message = match (sprite_upscale, filter_sprites) {
                (true, true) => "Sprite upscale: on",
                (true, false) => "Sprite upscale applies once sprite filtering is on.",
                (false, _) => "Sprite upscale: off",
            };
            self.windows.chat_window.add_system(message.to_string());
        }
        self.effect_queue.set_effects_enabled(show_skill_effects);
        if aura_changed {
            let gids: Vec<u32> = self.game.world.entities.iter().map(|e| e.id).collect();
            for gid in gids {
                self.refresh_level_aura(gid);
            }
        }
        if persist {
            self.save_config_soon();
        }
    }
}

impl App {
    /// Keeps the Damage Indicator's choice and draws with it from now on
    pub(crate) fn apply_damage_indicator(
        &mut self,
        skin: u32,
        direction: nostalre_game::damage_number::DamageDirection,
    ) {
        self.config.damage_skin = skin;
        self.config.damage_dir = direction.saved();
        self.config.save("config.json");
        self.game.combat.damage_numbers.direction = direction;
        self.windows
            .equipment_window
            .set_damage_indicator(skin, direction);
        if let Some(grf) = self.grf.clone() {
            self.load_damage_skin_sprites(&grf);
        }
    }
}
