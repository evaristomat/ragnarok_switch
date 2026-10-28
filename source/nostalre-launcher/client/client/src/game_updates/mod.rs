mod animation;
mod combat;
mod companion;
mod items;
mod movement;

use crate::App;
use models::enums::EnumWithStringValue;
use nostalre_formats::act::SpriteAnimationState;
use nostalre_game::app_state::AppState;
use nostalre_game::damage_number::DamageNumber;
use nostalre_renderer::effect::EffectUpdateCtx;

/// Reserved sprite key for the (single) character-creation preview.
pub(crate) const CREATE_PREVIEW_GID: u32 = u32::MAX;
/// Reserved sprite key for the stylist's preview: the player with the picks on
pub(crate) const STYLIST_PREVIEW_GID: u32 = u32::MAX - 1;

impl App {
    fn update_account_sprites(&mut self, _delta: f32, elapsed: f32) {
        if self.game.session.app_state != AppState::CharacterCreate {
            return;
        }
        let appearance = match &self.char_create_window {
            Some(w) => (w.hair_style, w.hair_color),
            None => return,
        };
        if self.char_create_built_appearance != Some(appearance) {
            let sex = self
                .game
                .session
                .login_session
                .as_ref()
                .map(|s| s.sex)
                .unwrap_or(0);
            self.load_player_sprite(
                CREATE_PREVIEW_GID,
                0,
                sex,
                appearance.0,
                appearance.1,
                0,
                None,
                0,
                0,
                0,
                0,
                0,
            );
            self.char_create_built_appearance = Some(appearance);
            self.account_anims
                .insert(CREATE_PREVIEW_GID, SpriteAnimationState::new(0));
        }
        let dir = ((elapsed / 0.5) as u32 % 8) as u8;
        if let Some(anim) = self.account_anims.get_mut(&CREATE_PREVIEW_GID) {
            anim.set_direction(dir);
        }
    }

    /// Rebuilds the stylist's preview sprite when its look changes: the player's own with the picks on
    fn update_stylist_preview(&mut self) {
        if self.windows.stylist_window.preview().is_none() {
            if self.stylist_built_look.take().is_some() {
                self.game.sprite_caches.sprites.remove(&STYLIST_PREVIEW_GID);
            }
            return;
        }
        let Some(player) = self.game.world.entities.player() else {
            return;
        };
        let base = nostalre_game::stylist::StyleLook {
            hair: player.head,
            hair_color: player.hair_color,
            cloth_color: player.cloth_color,
            head_top: player.head_top,
            head_mid: player.head_mid,
            head_bottom: player.head_bottom,
        };
        let job = nostalre_game::sprite_path::visual_job(
            player.job,
            player.effect_state,
            player.status_mount,
        );
        let (sex, weapon, weapon_look, shield) =
            (player.sex, player.weapon, player.weapon_look, player.shield);
        self.game.character.stylist.set_current(base);
        let data = &self.game.data_table;
        let look = self.game.character.stylist.preview(
            base,
            &data.styling_shop,
            nostalre_game::sprite_path::is_doram(player.job),
            &data.headgear_view,
        );
        if self.stylist_built_look == Some(look) {
            return;
        }
        self.load_player_sprite(
            STYLIST_PREVIEW_GID,
            job,
            sex,
            look.hair,
            look.hair_color,
            look.cloth_color,
            weapon,
            weapon_look,
            look.head_top,
            look.head_mid,
            look.head_bottom,
            shield,
        );
        self.stylist_built_look = Some(look);
    }

    pub(crate) fn run_game_updates(&mut self, delta: f32, elapsed: f32) {
        nostalre_profiling::profile_function!();
        self.update_account_sprites(delta, elapsed);
        self.update_stylist_preview();
        let now_ms = self.start_time.elapsed().as_millis() as u64;
        self.game.character.prune_expired(now_ms);
        self.update_movement(delta, elapsed);
        self.update_camera(delta);
        if let Some(renderer) = &self.renderer {
            self.game.combat.damage_numbers.view_yaw = renderer.camera.yaw;
        }
        self.process_continuous_walk(delta);
        self.update_entity_state(delta);
        self.flush_queued_move();
        self.update_companion_ai(delta);
        self.game.combat.damage_numbers.update(delta);
        self.process_scheduled_hits();
        self.process_caster_replays();
        self.update_floor_items(elapsed);
        self.request_char_names(now_ms);
        self.show_ranking_once_named();
        self.update_arrows(delta);
        self.check_pending_pickup();
        self.check_pending_attack(delta);
        self.update_progress_bar(delta);
        self.update_cast_marks(delta);
        self.check_pending_skill();
        self.check_pending_ground_skill();
        self.check_pending_skill_unit_cast();
        self.poll_gr2_loads();
        self.load_missing_entity_sprites();
        self.update_sprite_animation(delta);
        self.update_gr2_models(elapsed);
        self.update_running_footprints(delta);
        self.update_hover(delta);
        self.update_cart_animations(delta);
        self.update_falcon_visuals(delta);
        self.update_pet_roulette(delta);
        self.update_fades(delta);
        self.update_talkbox_bubbles(delta);
        self.update_show_digit(delta);
        self.game.schedulers.day_night.tick(delta);
        if self.game.schedulers.day_night.take_dirty()
            && let Some(renderer) = &mut self.renderer
        {
            renderer.set_day_night(
                self.game.schedulers.day_night.world_diffuse(),
                self.game.schedulers.day_night.sprite_light(),
            );
        }
        let camera = self.renderer.as_ref().map(|r| &r.camera);
        let is_visible = |pos: [f32; 3]| {
            camera.is_some_and(|c| c.is_world_pos_visible(pos[0], pos[1], pos[2], 0.25))
        };
        {
            nostalre_profiling::profile_scope!("ambient-effects");
            self.game
                .schedulers
                .ambient_effects
                .update(delta, &is_visible, &mut self.effect_queue);
        }
        self.game.schedulers.map_cloud.update(
            self.game.world.entities.player_id(),
            self.config.show_skill_effects,
            &mut self.effect_queue,
        );

        let entities = &self.game.world.entities;
        let resolve_caster_yaw = |id: u32| entities.get(id).map(|e| e.facing_degrees.to_radians());
        let gat = self.game.session.gat.as_ref();
        let map_coords = self.game.session.map_coords.as_ref();
        let resolve_entity_pos = |id: u32| {
            let (gat, coords) = (gat?, map_coords?);
            let (cx, cy) = entities.get(id)?.movement.position();
            let (wx, _, wz) = coords.cell_to_world(cx + 0.5, cy + 0.5);
            Some([wx, gat.get_height(cx + 0.5, cy + 0.5), wz])
        };
        if !self.effect_queue.pending.is_empty() {
            let t = self.start_time.elapsed().as_millis();
            for req in &self.effect_queue.pending {
                if nostalre_profiling::debug::trace_effects() {
                    tracing::debug!(
                        "[effect-timing t={t}ms] queue drain -> spawn {} (attach={:?})",
                        req.effect_id.as_str(),
                        req.attach,
                    );
                }
            }
        }
        self.game.schedulers.repeat_sounds.update(
            delta,
            &resolve_entity_pos,
            &mut self.sound_queue,
        );
        self.effect_holder
            .drain_queue(&mut self.effect_queue, &resolve_entity_pos);
        {
            nostalre_profiling::profile_scope!("effects-update");
            self.effect_holder.update(
                &EffectUpdateCtx {
                    delta,
                    camera_target: None,
                    caster_yaw: None,
                },
                &resolve_caster_yaw,
                &resolve_entity_pos,
            );
            let str_effects = &self.str_effects;
            self.effect_holder
                .end_played_strs(&|name| str_effects.seconds(name));
        }
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.camera.shake_offset = self.effect_holder.camera_shake_offset().into();
        }

        if let (Some(renderer), Some(grf)) = (self.renderer.as_mut(), self.grf.as_ref()) {
            for name in self.effect_holder.live_str_names() {
                if self.str_effects.get(&name).is_none() {
                    self.str_effects.load(
                        &name,
                        self.effect_loader.other_names(&name),
                        grf,
                        &mut renderer.texture_cache,
                        &renderer.device.device,
                        &renderer.device.queue,
                    );
                }
            }
        }

        for (entity_id, req) in self.effect_holder.drain_number_requests() {
            self.game
                .combat
                .damage_numbers
                .add(DamageNumber::effect_number(
                    entity_id, req.value, req.color, 0.0,
                ));
        }

        self.sync_trap_models();
    }

    /// Build/remove the deployed-trap RSM models to match `game.trap_units`.
    fn sync_trap_models(&mut self) {
        let (Some(renderer), Some(grf)) = (self.renderer.as_mut(), self.grf.as_ref()) else {
            return;
        };
        let live: std::collections::HashSet<u32> =
            self.game.world.trap_units.keys().copied().collect();
        renderer.retain_skill_unit_models(&live);
        if self.game.world.trap_units.is_empty() {
            return;
        }
        let scale_factor = self
            .game
            .session
            .map_coords
            .as_ref()
            .map(|c| c.zoom() / 10.0)
            .unwrap_or(1.0);
        for (&aid, trap) in &self.game.world.trap_units {
            if renderer.has_skill_unit_model(aid) {
                continue;
            }
            let Some(name) = nostalre_game::effect::trap_model_name(trap.unit_id) else {
                continue;
            };
            let world = trap.world;
            let path = format!("data\\model\\{name}");
            match grf.read_file(&path) {
                Ok(bytes) => match nostalre_formats::rsm::RsmFile::parse(&bytes) {
                    Ok(rsm) => renderer.add_skill_unit_model(aid, &rsm, grf, world, scale_factor),
                    Err(e) => tracing::warn!("trap model parse failed {path}: {e}"),
                },
                Err(e) => tracing::warn!("trap model read failed {path}: {e}"),
            }
        }
    }
}
