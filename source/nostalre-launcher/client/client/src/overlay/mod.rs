use crate::App;
use crate::config::{AccessibilityConfig, BarStyle};
use nostalre_game::character::is_fourth_job;
use nostalre_game::cursor::RenderEntry;
use nostalre_game::entity::{Entity, EntityCategory, EntityType};
use nostalre_game::sprite_path::JT_HIDDEN_NPC;
use nostalre_game::targeting::{GM_TEXT_COLOR, pk_name_color};
use nostalre_renderer::{UiDrawCall, UiTextureRef};
use nostalre_ui_component::game::chat_room_board;
use nostalre_ui_component::game::sign_board;
use nostalre_ui_component::game::vending_board;
use nostalre_ui_component::helper::dialog_container::DialogContainer;

/// Screen-space `[x0, y0, x1, y1]` of a vendor's shop board, centered above its
/// head. Shared by the overlay renderer and the hover hit-test so they agree.
pub(crate) fn vending_board_rect(entry: &RenderEntry) -> [f32; 4] {
    vending_board::board_rect(
        entry.screen_anchor[0],
        entry.screen_anchor[1],
        entry.head_offset,
    )
}

/// Screen-space `[x0, y0, x1, y1]` of a chat room board, centered above the
/// owner's head. Shared by the overlay renderer and the hover hit-test.
pub(crate) fn chat_room_board_rect(entry: &RenderEntry) -> [f32; 4] {
    chat_room_board::board_rect(
        entry.screen_anchor[0],
        entry.screen_anchor[1],
        entry.head_offset,
    )
}

/// Below this the bar draws `fill_color_low`.
const BAR_LOW_RATIO: f32 = 0.25;
const GUILD_NAME_COLOR: [f32; 4] = [0.8, 1.0, 0.753, 1.0];
const NPC_NAME_COLOR: [f32; 4] = [0.584, 0.722, 0.969, 1.0];
const NPC_NAME_DROP: f32 = 18.0;
const MONSTER_NAME_COLOR: [f32; 4] = [1.0, 0.765, 0.765, 1.0];
const MOB_INFO_COLOR: [f32; 4] = [0.9, 0.9, 0.9, 1.0];
const EMBLEM_HOVER_SIZE: f32 = 24.0;
const EMBLEM_HEAD_SIZE: f32 = 24.0;
const TALKBOX_BUBBLE_LIFT: f32 = 20.0;
/// How far a sign's window rises over its point, in sprite pixels at the player's scale: 0xadffe0 raises it by the
/// screen height · 81/480 · the player's +0x5c, which the actor draw (0x74f960) sets to its sprite scale over
/// height · 1.34/768 (0x4ee0f6), so the height cancels
const SIGN_LIFT: f32 = 81.0 * 768.0 / (480.0 * 1.34);
/// Floor-item hover label: its box's top edge sits `DROP` pixels below the item's
/// ground anchor, scaled by perspective, and the text `INSET` pixels inside that.
const FLOOR_ITEM_NAME_COLOR: [f32; 4] = [1.0, 0.937, 0.584, 1.0];
const FLOOR_ITEM_LABEL_DROP: f32 = 20.0;
const FLOOR_ITEM_LABEL_TEXT_INSET: f32 = 4.0;
const OUTLINE_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
/// At plate font size a glyph's own edge alpha is as low as 0.2, so a single ring
/// reads grey next to the letter's vertical stems. Repeating it accumulates towards
/// opaque black without widening the outline.
const OUTLINE_PASSES: usize = 3;

impl App {
    fn plate_text_width(&self, text: &str, font_atlas: &nostalre_renderer::FontAtlas) -> f32 {
        if self.config.accessibility.bold_name_plates {
            bold_width(text, font_atlas)
        } else {
            font_atlas.measure_text(text)
        }
    }

    fn build_plate_text(
        &self,
        text: &str,
        x: f32,
        y: f32,
        color: [f32; 4],
        font_atlas: &nostalre_renderer::FontAtlas,
        calls: &mut Vec<UiDrawCall>,
    ) {
        if self.config.accessibility.bold_name_plates {
            build_outlined_bold_text(text, x, y, color, font_atlas, calls);
        } else {
            build_outlined_text(text, x, y, color, font_atlas, calls);
        }
    }

    pub(crate) fn build_world_overlays(
        &self,
        render_list: &[RenderEntry],
        floor_item_render_list: &[RenderEntry],
        hovered_entity_id: Option<u32>,
        hovered_floor_item_id: Option<u32>,
    ) -> Vec<UiDrawCall> {
        let mut calls = Vec::new();

        self.build_npc_cutins(&mut calls);
        self.build_sign_boards(render_list, &mut calls);
        self.build_hovered_entity_overlay(hovered_entity_id, render_list, &mut calls);
        self.build_npc_names(render_list, &mut calls);
        self.build_guild_emblems(render_list, &mut calls);
        self.build_player_bars(hovered_entity_id, render_list, &mut calls);
        self.build_persistent_bars(hovered_entity_id, render_list, &mut calls);
        self.build_cast_bars(render_list, &mut calls);
        self.build_chat_bubbles(render_list, &mut calls);
        self.build_talkbox_bubbles(&mut calls);
        self.build_vending_boards(render_list, &mut calls);
        self.build_chat_room_boards(render_list, &mut calls);
        self.build_floor_item_tooltip(hovered_floor_item_id, floor_item_render_list, &mut calls);
        self.build_debug_pick_bounds(render_list, floor_item_render_list, &mut calls);

        calls
    }

    /// Controller users have no hover to ask for names. Request visible NPCs once,
    /// sharing the existing per-entity flag with mouse hover and the Y talk hint.
    pub(crate) fn request_visible_npc_names(&mut self, render_list: &[RenderEntry]) {
        if self.config.display.hide_name_npc {
            return;
        }
        let Some(renderer) = &self.renderer else {
            return;
        };
        let screen = [
            renderer.device.surface_config.width as f32 / renderer.dpi_scale,
            renderer.device.surface_config.height as f32 / renderer.dpi_scale,
        ];
        for entry in render_list {
            let Some(entity) = self.game.world.entities.get_mut(entry.id) else {
                continue;
            };
            if !npc_name_visible(entity, entry, screen)
                || entity.name_requested
                || entity.name.is_some()
            {
                continue;
            }
            entity.name_requested = true;
            self.channel
                .send_packet(nostalre_network::build_reqname_packet(
                    entity.id,
                    self.active_packetver,
                ));
        }
    }

    /// Native blue, outlined names below the feet, before any conversation.
    fn build_npc_names(&self, render_list: &[RenderEntry], calls: &mut Vec<UiDrawCall>) {
        if self.config.display.hide_name_npc {
            return;
        }
        let Some(renderer) = &self.renderer else {
            return;
        };
        let screen = [
            renderer.device.surface_config.width as f32 / renderer.dpi_scale,
            renderer.device.surface_config.height as f32 / renderer.dpi_scale,
        ];
        let mut occupied = Vec::new();
        if let Some(player) = render_list
            .iter()
            .find(|entry| self.game.world.entities.is_player(entry.id))
        {
            // Leave room for the player's native gauges and hover name as well.
            let half_width = self.config.accessibility.hp_bar.width / 2.0 + 4.0;
            occupied.push([
                player.screen_anchor[0] - half_width,
                player.pick_bounds[3] + 4.0,
                player.screen_anchor[0] + half_width,
                player.pick_bounds[3] + 40.0,
            ]);
        }
        for entry in render_list {
            let Some(entity) = self.game.world.entities.get(entry.id) else {
                continue;
            };
            if !npc_name_visible(entity, entry, screen) {
                continue;
            }
            let Some(name) = plate_name(entity).filter(|name| !name.trim().is_empty()) else {
                continue;
            };
            let width = self.plate_text_width(&name, &renderer.font_atlas);
            let x = entry.screen_anchor[0] - width / 2.0;
            let y = npc_name_baseline(
                x,
                width,
                entry.pick_bounds[3] + NPC_NAME_DROP,
                renderer.font_atlas.line_height,
                &occupied,
            );
            occupied.push([x, y - renderer.font_atlas.line_height, x + width, y + 3.0]);
            self.build_plate_text(
                &name,
                x,
                y,
                NPC_NAME_COLOR,
                &renderer.font_atlas,
                calls,
            );
        }
    }

    /// The map's sign boards from kRO's table, placed as its exe places them on map load (0xade5b0): the cell's
    /// ground (0x9f2780) raised by the sign's lift; their textures loaded with them
    pub(crate) fn place_map_signs(&mut self, map_name: &str) {
        let signs: Vec<_> = self
            .game
            .data_table
            .sign_boards
            .iter()
            .filter(|sign| nostalre_game::same_map(&sign.map, map_name))
            .filter_map(|sign| {
                let (x, y) = sign.cell;
                let mut world = self.cell_world_pos(x as f32 + 0.5, y as f32 + 0.5)?;
                // −Y is up
                world[1] -= sign.lift;
                Some((sign.clone(), world))
            })
            .collect();
        if let (Some(grf), Some(renderer)) = (&self.grf, &mut self.renderer)
            && !signs.is_empty()
        {
            let mut paths: Vec<&str> = signs.iter().map(|(sign, _)| sign.image.as_str()).collect();
            paths.push(sign_board::BOARD_TEX);
            renderer.preload_textures(&paths, grf);
        }
        self.game.session.map_signs = signs;
    }

    /// The map's title on entering it, its banner's texture loaded (0x9293e0)
    pub(crate) fn show_map_title(&mut self, map_name: &str) {
        let title = self.game.data_table.map_titles.on_enter(map_name).cloned();
        if let (Some(title), Some(grf), Some(renderer)) = (&title, &self.grf, &mut self.renderer) {
            renderer.preload_textures(&[title.background.as_str()], grf);
        }
        self.game.session.map_title = title.map(nostalre_game::map_title::MapTitleBanner::new);
    }

    /// Each sign on screen by an NPC (`stands_by_npc`) centred on its point's projection, then raised as kRO's
    /// `C3DCooRendering` raises its window (`SIGN_LIFT`, scaled as the player's sprite is)
    fn build_sign_boards(&self, render_list: &[RenderEntry], calls: &mut Vec<UiDrawCall>) {
        let Some(renderer) = &self.renderer else {
            return;
        };
        if self.game.session.map_signs.is_empty() {
            return;
        }
        let dpi = renderer.dpi_scale;
        let (screen_w, screen_h) = (
            renderer.device.surface_config.width as f32 / dpi,
            renderer.device.surface_config.height as f32 / dpi,
        );
        let player = self.game.world.entities.player_id();
        let lift = render_list
            .iter()
            .find(|entry| Some(entry.id) == player)
            .map_or(0.0, |entry| SIGN_LIFT * entry.sprite_scale);
        let reach = sign_board::REACH;
        let on_screen = |x: f32, y: f32| {
            (-reach..=screen_w + reach).contains(&x) && (-reach..=screen_h + reach).contains(&y)
        };
        let npc_cells: Vec<_> = self
            .game
            .world
            .entities
            .iter()
            .filter(|entity| entity.entity_type == EntityType::Npc)
            .map(|entity| entity.movement.cell_position())
            .collect();
        let signs: Vec<_> = self
            .game
            .session
            .map_signs
            .iter()
            .filter(|(sign, _)| {
                nostalre_game::sign_board::stands_by_npc(sign, npc_cells.iter().copied())
            })
            .filter_map(|(sign, [x, y, z])| {
                let (sx, sy) = renderer
                    .camera
                    .world_to_screen(*x, *y, *z, screen_w, screen_h)?;
                on_screen(sx, sy - lift).then_some((sign, sx, sy - lift))
            })
            .collect();
        let size = |path: &str| renderer.texture_cache.texture_size(path);
        sign_board::draw_signs(calls, &renderer.font_atlas, size, &signs);
    }

    /// NPC cutin illustrations, anchored along the bottom of the screen at their
    /// native size: slot 0 left, 1 middle, 2 right. Drawn before the hover overlay
    /// so it sits under the NPC dialog.
    fn build_npc_cutins(&self, calls: &mut Vec<UiDrawCall>) {
        let Some(renderer) = &self.renderer else {
            return;
        };
        let screen_w = renderer.device.surface_config.width as f32 / renderer.dpi_scale;
        let screen_h = renderer.device.surface_config.height as f32 / renderer.dpi_scale;
        for (slot, image) in self.game.npc_cutins.iter().enumerate() {
            let Some(image) = image else { continue };
            let path = crate::events::marriage::cutin_texture_path(image);
            let Some((w, h)) = renderer.texture_cache.texture_size(&path) else {
                continue;
            };
            let (w, h) = (w as f32, h as f32);
            let x = match slot {
                0 => 0.0,
                1 => ((screen_w - w) / 2.0).max(0.0),
                _ => (screen_w - w).max(0.0),
            };
            let y = (screen_h - h).max(0.0);
            let (verts, indices) = nostalre_ui::draw::quad_vertices(x, y, w, h, [1.0; 4]);
            calls.push(UiDrawCall {
                vertices: verts.into(),
                indices: indices.into(),
                texture: UiTextureRef::Named(path.into()),
            });
        }
    }

    fn build_hovered_entity_overlay(
        &self,
        hovered_entity_id: Option<u32>,
        render_list: &[RenderEntry],
        calls: &mut Vec<UiDrawCall>,
    ) {
        let (entity_id, renderer) = match (hovered_entity_id, &self.renderer) {
            (Some(id), Some(r)) => (id, r),
            _ => return,
        };
        let entity = match self.game.world.entities.get(entity_id) {
            Some(e) => e,
            None => return,
        };
        // NPC names have their own persistent pass; hovering must not draw twice.
        if entity.category() == EntityCategory::Npc {
            return;
        }
        let entry = match render_list.iter().find(|e| e.id == entity_id) {
            Some(e) => e,
            None => return,
        };

        let hp_style = self.entity_hp_style(entity);
        let sp_style = &self.config.accessibility.sp_bar;
        let mut bar_y = entry.pick_bounds[3] + 5.0;
        let mut bar_height = hp_style.height;
        let hp_ratio = (entity.category().has_health_bar()
            && !self.game.world.entities.hides_overhead_ui(entity_id))
        .then(|| self.entity_hp_ratio(entity_id))
        .flatten();
        if let Some(ratio) = hp_ratio {
            let (_x, y) = render_hp_bar(entry, ratio, hp_style, calls);
            bar_y = y;
            if let Some(sp_ratio) = self.entity_sp_ratio(entity_id) {
                let sp_y = y + hp_style.height;
                render_bar(entry.screen_anchor[0], sp_y, sp_ratio, sp_style, calls);
                bar_y = sp_y;
                bar_height = sp_style.height;
            }
            if let Some(ap) = self
                .game
                .world
                .entities
                .is_player(entity_id)
                .then(|| self.player_ap_ratio())
                .flatten()
            {
                let ap_style = &self.config.accessibility.ap_bar;
                let ap_y = bar_y + bar_height;
                render_bar(entry.screen_anchor[0], ap_y, ap, ap_style, calls);
                bar_y = ap_y;
                bar_height = ap_style.height;
            }
        }
        if let Some(name) = plate_name(entity)
            && !self.name_hidden(entity)
        {
            let text_width = self.plate_text_width(&name, &renderer.font_atlas);
            let text_x = entry.screen_anchor[0] - text_width / 2.0;
            let name_y = bar_y + bar_height + 13.0;
            let mut text_y = name_y;
            self.build_plate_text(
                &name,
                text_x,
                text_y,
                entity_name_color(entity),
                &renderer.font_atlas,
                calls,
            );

            let mut leftmost_x = text_x;
            if let Some((line, color)) = second_plate_line(entity) {
                let line_width = self.plate_text_width(&line, &renderer.font_atlas);
                let line_x = entry.screen_anchor[0] - line_width / 2.0;
                leftmost_x = leftmost_x.min(line_x);
                text_y += renderer.font_atlas.line_height;
                self.build_plate_text(&line, line_x, text_y, color, &renderer.font_atlas, calls);
            }

            if entity.guild_id != 0 && entity.guild_emblem_version != 0 {
                let block_center_y = (name_y + text_y) / 2.0;
                let emblem_x = leftmost_x - EMBLEM_HOVER_SIZE - 3.0;
                let emblem_y = block_center_y - EMBLEM_HOVER_SIZE / 2.0;
                push_emblem(
                    entity.guild_id,
                    entity.guild_emblem_version,
                    emblem_x,
                    emblem_y,
                    EMBLEM_HOVER_SIZE,
                    renderer,
                    calls,
                );
            }
        }
    }

    /// Guild emblems above the head of every guilded character and castle
    /// guardian, shown only on WoE/siege maps. Castle flags carry theirs on the
    /// banner itself, not overhead.
    fn build_guild_emblems(&self, render_list: &[RenderEntry], calls: &mut Vec<UiDrawCall>) {
        if !self.game.session.map_properties.is_siege() {
            return;
        }
        let Some(renderer) = &self.renderer else {
            return;
        };
        for entry in render_list {
            let Some(entity) = self.game.world.entities.get(entry.id) else {
                continue;
            };
            let wears_emblem = entity.entity_type == EntityType::Player
                || nostalre_game::sprite_path::is_guardian(entity.job);
            if !wears_emblem
                || entity.guild_id == 0
                || entity.guild_emblem_version == 0
                || nostalre_game::sprite_path::is_hidden(entity.effect_state)
            {
                continue;
            }
            let head_top = entry.screen_anchor[1] - entry.head_offset;
            let emblem_x = entry.screen_anchor[0] - EMBLEM_HEAD_SIZE / 2.0;
            let emblem_y = head_top - EMBLEM_HEAD_SIZE - 4.0;
            push_emblem(
                entity.guild_id,
                entity.guild_emblem_version,
                emblem_x,
                emblem_y,
                EMBLEM_HEAD_SIZE,
                renderer,
                calls,
            );
        }
    }

    fn build_player_bars(
        &self,
        hovered_entity_id: Option<u32>,
        render_list: &[RenderEntry],
        calls: &mut Vec<UiDrawCall>,
    ) {
        if self.renderer.is_none() || self.game.world.entities.player().is_none() {
            return;
        }
        if hovered_entity_id == self.game.world.entities.player_id() {
            return;
        }
        let ratio = self.game.character.hp_percentage();
        if let Some(entry) = render_list
            .iter()
            .find(|e| Some(e.id) == self.game.world.entities.player_id())
        {
            let hp_style = &self.config.accessibility.hp_bar;
            let sp_style = &self.config.accessibility.sp_bar;
            let (_x, y) = render_hp_bar(entry, ratio, hp_style, calls);
            let sp_y = y + hp_style.height;
            render_bar(
                entry.screen_anchor[0],
                sp_y,
                self.game.character.sp_percentage(),
                sp_style,
                calls,
            );
            if let Some(ap) = self.player_ap_ratio() {
                let ap_style = &self.config.accessibility.ap_bar;
                render_bar(
                    entry.screen_anchor[0],
                    sp_y + sp_style.height,
                    ap,
                    ap_style,
                    calls,
                );
            }
        }
    }

    /// HP ratio for an entity, sourcing companion HP from companion state (which is not
    /// mirrored onto `Entity.hp`), monster HP from `Entity.mob_info` and party-member
    /// HP from `Entity.hp`.
    pub(crate) fn entity_hp_ratio(&self, entity_id: u32) -> Option<f32> {
        if self.game.world.entities.get(entity_id).is_some_and(ambient_population_actor) {
            return None;
        }
        if self.game.world.entities.is_player(entity_id) {
            return Some(self.game.character.hp_percentage());
        }
        if let Some((hp, _)) = self.companion_gauges(entity_id) {
            return Some(hp);
        }
        self.game.world.entities.get(entity_id).and_then(|e| {
            e.mob_info
                .as_ref()
                .and_then(|info| info.hp_ratio())
                .or_else(|| e.hp_percentage())
        })
    }

    /// The player's AP ratio, shown under SP for 4th jobs: empty while max AP is 0 (below base level 200), as kRO
    /// draws it (the rig, October 15)
    fn player_ap_ratio(&self) -> Option<f32> {
        let traits = &self.game.character.traits;
        is_fourth_job(self.game.world.entities.player_job())
            .then(|| ap_ratio(traits.ap, traits.max_ap))
    }

    /// SP ratio for an entity that shows an SP bar below it: the player and the
    /// player's companions. Other entities (party members, monsters) return None.
    fn entity_sp_ratio(&self, entity_id: u32) -> Option<f32> {
        if self.game.world.entities.is_player(entity_id) {
            return Some(self.game.character.sp_percentage());
        }
        self.companion_gauges(entity_id).and_then(|(_, sp)| sp)
    }

    /// A companion's HP bar is the player's whatever its body: an elemental or a summon is a monster body under kRO's
    /// one companion gauge (0x53f160, built for the homunculus, the mercenary, the elemental and the summons alike)
    fn entity_hp_style(&self, entity: &Entity) -> &BarStyle {
        let accessibility = &self.config.accessibility;
        if self.companion_gauges(entity.id).is_some() {
            &accessibility.hp_bar
        } else {
            hp_bar_style(accessibility, entity.entity_type)
        }
    }

    /// HP and SP of the player's companion behind `entity_id`, kept apart from `Entity` by the companion's own
    /// packets: a homunculus, a mercenary or an elemental (HP and SP), or a summon (HP only). kRO draws them all
    /// under the unit from one gauge widget (0x82bed0: types 8 to 10 with SP, the ABR and Bionic 0xd and 0xe without)
    fn companion_gauges(&self, entity_id: u32) -> Option<(f32, Option<f32>)> {
        let companions = &self.game.companions;
        let homunculus = companions.homunculus.as_ref().filter(|h| h.gid == entity_id);
        let mercenary = companions.mercenary.as_ref().filter(|m| m.gid == entity_id);
        let elemental = companions.elemental.as_ref().filter(|e| e.gid == entity_id);
        homunculus
            .map(|h| (h.hp_percentage(), Some(h.sp_percentage())))
            .or_else(|| mercenary.map(|m| (m.hp_percentage(), Some(m.sp_percentage()))))
            .or_else(|| elemental.map(|e| (e.hp_percentage(), Some(e.sp_percentage()))))
            .or_else(|| {
                companions
                    .summons
                    .get(&entity_id)
                    .map(|s| (s.hp_percentage(), None))
            })
    }

    /// Always-on HP bars below party members, the player's mercenary/homunculus and
    /// monsters the server reports HP for. The player is handled by `build_player_bars`;
    /// the hovered entity by the hover overlay — both are skipped here to avoid drawing
    /// twice.
    fn build_persistent_bars(
        &self,
        hovered_entity_id: Option<u32>,
        render_list: &[RenderEntry],
        calls: &mut Vec<UiDrawCall>,
    ) {
        if self.renderer.is_none() {
            return;
        }
        let player_id = self.game.world.entities.player_id();
        for entry in render_list {
            if Some(entry.id) == player_id || Some(entry.id) == hovered_entity_id {
                continue;
            }
            let is_companion = self.companion_gauges(entry.id).is_some();
            let is_party_member = self
                .game
                .party
                .as_ref()
                .is_some_and(|p| p.members.iter().any(|m| m.aid == entry.id));
            let Some(entity) = self.game.world.entities.get(entry.id) else {
                continue;
            };
            if !persistent_bar_eligible(entity, is_companion, is_party_member) {
                continue;
            }
            let Some(ratio) = self.entity_hp_ratio(entry.id) else {
                continue;
            };
            let hp_style = self.entity_hp_style(entity);
            let (_x, y) = render_hp_bar(entry, ratio, hp_style, calls);
            if let Some(sp_ratio) = self.entity_sp_ratio(entry.id) {
                render_bar(
                    entry.screen_anchor[0],
                    y + hp_style.height,
                    sp_ratio,
                    &self.config.accessibility.sp_bar,
                    calls,
                );
            }
        }
    }

    fn build_cast_bars(&self, render_list: &[RenderEntry], calls: &mut Vec<UiDrawCall>) {
        use nostalre_game::effect::casting_skill;
        let style = &self.config.accessibility.cast_bar;
        let cast_bar_height = style.height;
        if let Some(bar) = &self.game.session.progress_bar
            && let Some(entry) = render_list
                .iter()
                .find(|e| self.game.world.entities.is_player(e.id))
        {
            render_bar(
                entry.screen_anchor[0],
                entry.screen_anchor[1] - entry.head_offset - cast_bar_height - 2.0,
                bar.fraction(),
                style,
                calls,
            );
        }
        for entry in render_list {
            if (self.config.display.show_other_cast_bars
                || self.game.world.entities.is_player(entry.id))
                && let Some(entity) = self.game.world.entities.get(entry.id)
                && let Some(progress) = entity.cast_progress()
                && !entity
                    .active_skill
                    .is_some_and(|skill| casting_skill(skill).hide_cast_bar)
            {
                let cast_bar_y = entry.screen_anchor[1] - entry.head_offset - cast_bar_height - 2.0;
                render_bar(entry.screen_anchor[0], cast_bar_y, progress, style, calls);
            }
        }
    }

    fn name_hidden(&self, entity: &Entity) -> bool {
        let category = entity.category();
        if !category.has_name_plate() || entity.job == JT_HIDDEN_NPC {
            return true;
        }
        let display = &self.config.display;
        match category {
            EntityCategory::Player => {
                display.hide_name_player || self.game.session.map_properties.is_siege()
            }
            EntityCategory::Monster | EntityCategory::Pet => display.hide_name_monster,
            EntityCategory::Npc => display.hide_name_npc,
            _ => false,
        }
    }

    fn build_chat_bubbles(&self, render_list: &[RenderEntry], calls: &mut Vec<UiDrawCall>) {
        for entry in render_list {
            let entity = match self.game.world.entities.get(entry.id) {
                Some(e) => e,
                None => continue,
            };
            let bubble = match &entity.chat_bubble {
                Some(b) => b,
                None => continue,
            };
            self.build_speech_bubble(
                &bubble.message,
                entry.screen_anchor[0],
                entry.screen_anchor[1] - entry.head_offset - 10.0,
                calls,
            );
        }
    }

    /// Talkie Box text, over the box rather than over an actor: skill units are
    /// not entities, so their anchor is the position captured with the text.
    fn build_talkbox_bubbles(&self, calls: &mut Vec<UiDrawCall>) {
        let Some(renderer) = &self.renderer else {
            return;
        };
        let screen_w = renderer.device.surface_config.width as f32 / renderer.dpi_scale;
        let screen_h = renderer.device.surface_config.height as f32 / renderer.dpi_scale;
        for (world, bubble) in self.game.world.talkbox_bubbles.values() {
            let Some((sx, sy)) = renderer
                .camera
                .world_to_screen(world[0], world[1], world[2], screen_w, screen_h)
            else {
                continue;
            };
            self.build_speech_bubble(&bubble.message, sx, sy - TALKBOX_BUBBLE_LIFT, calls);
        }
    }

    /// A centred speech bubble whose bottom edge sits at `bottom_y`.
    fn build_speech_bubble(
        &self,
        message: &str,
        center_x: f32,
        bottom_y: f32,
        calls: &mut Vec<UiDrawCall>,
    ) {
        let Some(renderer) = &self.renderer else {
            return;
        };
        let padding = 4.0;
        let lines = nostalre_ui::draw::word_wrap(
            message,
            150.0,
            |t| renderer.font_atlas.measure_text(t),
            false,
        );

        let line_h = renderer.font_atlas.line_height;
        let total_h = line_h * lines.len() as f32 + padding * 2.0;
        let widest = lines
            .iter()
            .map(|l| renderer.font_atlas.measure_text(l))
            .fold(0.0_f32, f32::max);
        let box_w = widest + padding * 2.0;
        let box_x = center_x - box_w / 2.0;
        let box_y = bottom_y - total_h;

        let (bg_verts, bg_idx) =
            nostalre_ui::draw::quad_vertices(box_x, box_y, box_w, total_h, [0.0, 0.0, 0.0, 0.8]);
        calls.push(UiDrawCall {
            vertices: bg_verts.into(),
            indices: bg_idx.into(),
            texture: UiTextureRef::White,
        });

        for (i, line) in lines.iter().enumerate() {
            let line_w = renderer.font_atlas.measure_text(line);
            let lx = center_x - line_w / 2.0;
            let ly = box_y + padding + line_h / 2.0 + line_h * i as f32;
            let (verts, indices) = nostalre_ui::draw::text_vertices(
                line,
                lx,
                ly,
                [1.0, 1.0, 1.0, 1.0],
                &renderer.font_atlas,
            );
            if !verts.is_empty() {
                calls.push(UiDrawCall {
                    vertices: verts.into(),
                    indices: indices.into(),
                    texture: UiTextureRef::FontAtlas,
                });
            }
        }
    }

    fn build_vending_boards(&self, render_list: &[RenderEntry], calls: &mut Vec<UiDrawCall>) {
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };
        for entry in render_list {
            let entity = match self.game.world.entities.get(entry.id) {
                Some(e) => e,
                None => continue,
            };
            let board = match &entity.vending_board {
                Some(b) => b,
                None => continue,
            };

            let has_grf = renderer
                .texture_cache
                .texture_size(vending_board::VENDING_ICON_TEX)
                .is_some();
            let mut container = DialogContainer::new();
            container.has_grf_textures = has_grf;
            container.set_texture_sizes(&|name| renderer.texture_cache.texture_size(name));

            vending_board::draw_board(
                calls,
                &container,
                &renderer.font_atlas,
                entry.screen_anchor[0],
                entry.screen_anchor[1],
                entry.head_offset,
                board,
            );
        }
    }

    fn build_chat_room_boards(&self, render_list: &[RenderEntry], calls: &mut Vec<UiDrawCall>) {
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };
        for room in self.game.chat_rooms.iter() {
            let entry = match render_list.iter().find(|e| e.id == room.owner_aid) {
                Some(e) => e,
                None => continue,
            };

            let has_grf = renderer
                .texture_cache
                .texture_size(chat_room_board::CHAT_OPEN_TEX)
                .is_some();
            let mut container = DialogContainer::new();
            container.has_grf_textures = has_grf;
            container.set_texture_sizes(&|name| renderer.texture_cache.texture_size(name));

            chat_room_board::draw_board(
                calls,
                &container,
                &renderer.font_atlas,
                entry.screen_anchor[0],
                entry.screen_anchor[1],
                entry.head_offset,
                room.atype,
                &room.title,
                room.cur_count,
                room.max_count,
            );
        }
    }

    fn build_floor_item_tooltip(
        &self,
        hovered_floor_item_id: Option<u32>,
        floor_item_render_list: &[RenderEntry],
        calls: &mut Vec<UiDrawCall>,
    ) {
        let fi_id = match hovered_floor_item_id {
            Some(id) => id,
            None => return,
        };
        let floor_item = match self.game.world.floor_items.get(&fi_id) {
            Some(fi) => fi,
            None => return,
        };
        let fi_entry = match floor_item_render_list.iter().find(|e| e.id == fi_id) {
            Some(e) => e,
            None => return,
        };
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };

        let Some(label) = floor_item.label(self.game.data_table.msg_string.as_ref()) else {
            return;
        };
        let text_w = self.plate_text_width(&label, &renderer.font_atlas);
        let text_x = fi_entry.screen_anchor[0] - text_w / 2.0;
        let text_y = fi_entry.screen_anchor[1]
            + FLOOR_ITEM_LABEL_DROP * fi_entry.sprite_scale
            + FLOOR_ITEM_LABEL_TEXT_INSET;

        self.build_plate_text(
            &label,
            text_x,
            text_y,
            FLOOR_ITEM_NAME_COLOR,
            &renderer.font_atlas,
            calls,
        );
    }

    fn build_debug_pick_bounds(
        &self,
        render_list: &[RenderEntry],
        floor_item_render_list: &[RenderEntry],
        calls: &mut Vec<UiDrawCall>,
    ) {
        if !self.game.debug_show_pick_bounds {
            return;
        }
        let debug_color = [1.0, 0.0, 0.0, 0.7];
        let line_thickness = 1.0;
        for entry in render_list.iter().chain(floor_item_render_list.iter()) {
            let [left, top, right, bottom] = entry.pick_bounds;
            let w = right - left;
            let h = bottom - top;
            for (x, y, bw, bh) in [
                (left, top, w, line_thickness),
                (left, bottom - line_thickness, w, line_thickness),
                (left, top, line_thickness, h),
                (right - line_thickness, top, line_thickness, h),
            ] {
                let (v, i) = nostalre_ui::draw::quad_vertices(x, y, bw, bh, debug_color);
                calls.push(UiDrawCall {
                    vertices: v.into(),
                    indices: i.into(),
                    texture: UiTextureRef::White,
                });
            }
            let dot = 3.0;
            let (v, i) = nostalre_ui::draw::quad_vertices(
                entry.screen_anchor[0] - dot,
                entry.screen_anchor[1] - dot,
                dot * 2.0,
                dot * 2.0,
                debug_color,
            );
            calls.push(UiDrawCall {
                vertices: v.into(),
                indices: i.into(),
                texture: UiTextureRef::White,
            });
        }
    }

    pub(crate) fn build_skill_overlay(&self) -> Vec<UiDrawCall> {
        let (pending, renderer) = match (
            &self.game.pending_casts.pending_skill_target,
            &self.renderer,
        ) {
            (Some(p), Some(r)) => (p, r),
            _ => return Vec::new(),
        };

        let mut calls = Vec::new();
        let (mx, my) = self.input.mouse_position;
        let text = format!("Lv {}", pending.level());
        let text_x = mx as f32 + 20.0;
        let text_y = my as f32 + 2.0;
        self.build_plate_text(
            &text,
            text_x,
            text_y,
            [1.0, 1.0, 1.0, 1.0],
            &renderer.font_atlas,
            &mut calls,
        );

        let skill = pending.skill();
        if self.game.resolve_cast_skill(skill).is_some() {
            let display_name = self.game.data_table.skill_name_for(skill);
            let level = pending.level();
            let banner_text = if level > 0 {
                format!("{display_name}(Lv {level})")
            } else {
                display_name.to_string()
            };
            let padding = 4.0;
            let line_h = renderer.font_atlas.line_height;
            let text_w = renderer.font_atlas.measure_text(&banner_text);
            let box_w = text_w + padding * 2.0;
            let box_h = line_h + padding * 2.0;
            let screen_w = renderer.device.surface_config.width as f32 / renderer.dpi_scale;
            let box_x = ((screen_w - box_w) / 2.0).floor();
            let box_y = 80.0;

            let (bg_verts, bg_idx) =
                nostalre_ui::draw::quad_vertices(box_x, box_y, box_w, box_h, [0.0, 0.0, 0.0, 0.8]);
            calls.push(UiDrawCall {
                vertices: bg_verts.into(),
                indices: bg_idx.into(),
                texture: UiTextureRef::White,
            });

            let tx = box_x + padding;
            let ty = box_y + padding + line_h / 2.0;
            let (verts, indices) = nostalre_ui::draw::text_vertices(
                &banner_text,
                tx,
                ty,
                [0.0, 1.0, 0.0, 1.0],
                &renderer.font_atlas,
            );
            if !verts.is_empty() {
                calls.push(UiDrawCall {
                    vertices: verts.into(),
                    indices: indices.into(),
                    texture: UiTextureRef::FontAtlas,
                });
            }
        }

        calls
    }
}

/// Only drawn NPC bodies on screen receive a persistent name or a name request.
fn npc_name_visible(entity: &Entity, entry: &RenderEntry, screen: [f32; 2]) -> bool {
    entity.category() == EntityCategory::Npc
        && !nostalre_game::sprite_path::is_undrawn_actor(entity.job)
        && !nostalre_game::sprite_path::is_hidden(entity.effect_state)
        && entry.pick_bounds[2] >= 0.0
        && entry.pick_bounds[0] <= screen[0]
        && entry.pick_bounds[3] >= 0.0
        && entry.pick_bounds[1] <= screen[1]
}

/// Keep adjacent native names legible without cards or leader lines.
fn npc_name_baseline(x: f32, width: f32, mut y: f32, line_h: f32, occupied: &[[f32; 4]]) -> f32 {
    while let Some(bottom) = occupied
        .iter()
        .filter(|r| {
            x < r[2] + 2.0
                && x + width > r[0] - 2.0
                && y - line_h < r[3] + 2.0
                && y + 3.0 > r[1] - 2.0
        })
        .map(|r| r[3])
        .max_by(f32::total_cmp)
    {
        y = bottom + line_h + 2.0;
    }
    y
}

fn push_emblem(
    guild_id: u32,
    version: i32,
    x: f32,
    y: f32,
    size: f32,
    renderer: &nostalre_renderer::Renderer,
    calls: &mut Vec<UiDrawCall>,
) {
    let key = nostalre_game::guild::emblem_texture_key(guild_id, version);
    if renderer.texture_cache.texture_size(&key).is_none() {
        return;
    }
    let (verts, indices) = nostalre_ui::draw::quad_vertices(x, y, size, size, [1.0; 4]);
    calls.push(UiDrawCall {
        vertices: verts.into(),
        indices: indices.into(),
        texture: UiTextureRef::Named(key.into()),
    });
}

/// The name line: a player's title before its name as kRO's balloon has it ("[%s] %s", 0x82e1d0)
fn plate_name(entity: &Entity) -> Option<std::borrow::Cow<'_, str>> {
    let name = entity.name.as_deref()?;
    Some(match &entity.title {
        Some(title) => format!("[{title}] {name}").into(),
        None => name.into(),
    })
}

/// Text drawn under an entity's name: the guild tag, or the monster info the server
/// sends when `show_mob_info` is on. A monster never has both.
fn second_plate_line(entity: &Entity) -> Option<(String, [f32; 4])> {
    if let Some(guild_name) = &entity.guild_name {
        let text = match &entity.position_name {
            Some(pos) if !pos.is_empty() => format!("<{guild_name}> [{pos}]"),
            _ => format!("<{guild_name}>"),
        };
        let color = if entity.is_gm {
            GM_TEXT_COLOR
        } else {
            GUILD_NAME_COLOR
        };
        return Some((text, color));
    }
    entity
        .mob_info
        .as_ref()
        .map(|info| (info.label(), MOB_INFO_COLOR))
}

fn entity_name_color(entity: &Entity) -> [f32; 4] {
    if let Some(color) = pk_name_color(entity.effect_state) {
        return color;
    }
    if entity.is_gm {
        return GM_TEXT_COLOR;
    }
    match entity.entity_type {
        EntityType::Player | EntityType::Homunculus | EntityType::Mercenary => [1.0, 1.0, 1.0, 1.0],
        EntityType::Monster => MONSTER_NAME_COLOR,
        EntityType::Npc => NPC_NAME_COLOR,
    }
}

fn hp_bar_style(accessibility: &AccessibilityConfig, entity_type: EntityType) -> &BarStyle {
    match entity_type {
        EntityType::Monster => &accessibility.monster_hp_bar,
        _ => &accessibility.hp_bar,
    }
}

fn render_bar(center_x: f32, y: f32, ratio: f32, style: &BarStyle, draw_calls: &mut Vec<UiDrawCall>) {
    let (width, height) = (style.width, style.height);
    let border_x = center_x - width / 2.0;
    let (border_verts, border_idx) =
        nostalre_ui::draw::quad_vertices(border_x, y, width, height, style.border_color.rgba());
    draw_calls.push(UiDrawCall {
        vertices: border_verts.into(),
        indices: border_idx.into(),
        texture: UiTextureRef::White,
    });
    let (bg_verts, bg_idx) = nostalre_ui::draw::quad_vertices(
        border_x + 1.0,
        y + 1.0,
        width - 2.0,
        height - 2.0,
        style.background_color.rgba(),
    );
    draw_calls.push(UiDrawCall {
        vertices: bg_verts.into(),
        indices: bg_idx.into(),
        texture: UiTextureRef::White,
    });
    let fill_ratio = ratio.clamp(0.0, 1.0);
    let fill_w = (width - 2.0) * fill_ratio;
    let fill_color = if fill_ratio >= BAR_LOW_RATIO {
        style.fill_color
    } else {
        style.fill_color_low
    };
    let (fill_verts, fill_idx) = nostalre_ui::draw::quad_vertices(
        border_x + 1.0,
        y + 1.0,
        fill_w,
        height - 2.0,
        fill_color.rgba(),
    );
    draw_calls.push(UiDrawCall {
        vertices: fill_verts.into(),
        indices: fill_idx.into(),
        texture: UiTextureRef::White,
    });
}

/// How full the AP bar is: empty while there is no AP to have
fn ap_ratio(ap: u32, max_ap: u32) -> f32 {
    if max_ap == 0 {
        return 0.0;
    }
    (ap as f32 / max_ap as f32).clamp(0.0, 1.0)
}

/// Whether an entity carries a bar with no pointer over it. Monsters qualify once the
/// server has reported HP for them, which it pushes to the area on every hit.
fn ambient_population_actor(entity: &Entity) -> bool {
    entity.entity_type==EntityType::Player && (95_000_000..100_000_000).contains(&entity.id)
}

fn persistent_bar_eligible(entity: &Entity, is_companion: bool, is_party_member: bool) -> bool {
    if ambient_population_actor(entity) { return false; }
    let has_mob_hp = entity
        .mob_info
        .as_ref()
        .is_some_and(|info| info.hp_ratio().is_some())
        // ZC_HP_INFO: the server sends it to those who hit the monster, like the original client's bar
        || entity.hp_percentage().is_some();
    if !is_companion && !is_party_member && !has_mob_hp {
        return false;
    }
    !nostalre_game::sprite_path::is_hidden(entity.effect_state) && entity.is_alive()
}

fn render_hp_bar(
    entry: &RenderEntry,
    ratio: f32,
    style: &BarStyle,
    draw_calls: &mut Vec<UiDrawCall>,
) -> (f32, f32) {
    let center_x = entry.screen_anchor[0];
    let y = entry.pick_bounds[3];
    render_bar(center_x, y, ratio, style, draw_calls);
    (center_x, y)
}

fn build_outlined_text(
    text: &str,
    x: f32,
    y: f32,
    color: [f32; 4],
    font_atlas: &nostalre_renderer::FontAtlas,
    calls: &mut Vec<UiDrawCall>,
) {
    nostalre_ui::draw::push_outlined_text(
        text,
        x,
        y,
        color,
        &nostalre_ui::draw::TextOutline::ring(OUTLINE_COLOR),
        font_atlas,
        calls,
    );
}

fn bold_text(text: &str) -> String {
    text.chars()
        .map(nostalre_renderer::font_atlas::bold_char)
        .collect()
}

fn bold_width(text: &str, font_atlas: &nostalre_renderer::FontAtlas) -> f32 {
    font_atlas.measure_text(&bold_text(text))
}

/// Only the bold ring thickens with the UI scale: at a regular weight the side bearings
/// are too narrow to hold it, and letters merge into their neighbours.
fn build_outlined_bold_text(
    text: &str,
    x: f32,
    y: f32,
    color: [f32; 4],
    font_atlas: &nostalre_renderer::FontAtlas,
    calls: &mut Vec<UiDrawCall>,
) {
    let outline = nostalre_ui::draw::TextOutline {
        color: OUTLINE_COLOR,
        ring: &nostalre_ui::draw::OUTLINE_RING_8,
        passes: OUTLINE_PASSES,
        thickness_px: font_atlas.dpi_scale.round(),
    };
    nostalre_ui::draw::push_outlined_text(
        &bold_text(text),
        x,
        y,
        color,
        &outline,
        font_atlas,
        calls,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostalre_game::targeting::EFFECT_STATE_RED_NAME;

    #[test]
    fn the_ap_bar_is_empty_without_ap_and_full_at_max() {
        assert_eq!(ap_ratio(0, 0), 0.0);
        assert_eq!(ap_ratio(100, 200), 0.5);
        assert_eq!(ap_ratio(200, 200), 1.0);
    }

    fn player() -> Entity {
        Entity::new(
            1,
            EntityType::Player,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            150,
        )
    }

    fn monster() -> Entity {
        Entity::new(
            2,
            EntityType::Monster,
            1002,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            150,
        )
    }

    fn npc() -> Entity {
        Entity::new(3, EntityType::Npc, 46, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 150)
    }

    #[test]
    fn crowded_npc_names_stay_below_feet_and_clear_the_player_and_other_names() {
        let player = [590.0, 370.0, 690.0, 410.0];
        let first_y = npc_name_baseline(550.0, 110.0, 396.0, 15.0, &[player]);
        assert!(first_y - 15.0 >= player[3] + 2.0);
        let first = [550.0, first_y - 15.0, 660.0, first_y + 3.0];
        let second_y = npc_name_baseline(600.0, 130.0, 396.0, 15.0, &[player, first]);
        assert!(second_y - 15.0 >= first[3] + 2.0);
        assert_eq!(
            npc_name_baseline(100.0, 100.0, 396.0, 15.0, &[player, first]),
            396.0
        );
    }

    #[test]
    fn persistent_npc_names_do_not_reveal_hidden_triggers_or_other_actors() {
        use nostalre_game::sprite_path::{
            JT_EFFECTLAUNCHER, JT_HIDDEN_WARP_NPC, JT_INVISIBLE, JT_WARPNPC, OPTION_HIDE,
        };
        let entry = RenderEntry {
            kind: nostalre_game::cursor::RenderEntryKind::Entity,
            id: 3,
            screen_anchor: [100.0, 100.0],
            depth: 0.5,
            depth_gradient: [0.0; 2],
            flat_depth_gradient: [0.0; 2],
            camera_dir: 0,
            sprite_scale: 1.0,
            pick_bounds: [80.0, 50.0, 120.0, 100.0],
            head_offset: 50.0,
        };
        let screen = [1280.0, 720.0];
        let mut actor = npc();
        assert!(npc_name_visible(&actor, &entry, screen));
        for job in [
            JT_WARPNPC,
            JT_EFFECTLAUNCHER,
            JT_HIDDEN_NPC,
            JT_HIDDEN_WARP_NPC,
            JT_INVISIBLE,
            0,
            1002,
        ] {
            actor.job = job;
            assert!(!npc_name_visible(&actor, &entry, screen), "job {job}");
        }
        actor = npc();
        actor.effect_state = OPTION_HIDE;
        assert!(!npc_name_visible(&actor, &entry, screen));
        actor.effect_state = 0;
        for bounds in [
            [-80.0, 50.0, -20.0, 100.0],
            [1300.0, 50.0, 1340.0, 100.0],
            [80.0, -100.0, 120.0, -20.0],
            [80.0, 740.0, 120.0, 790.0],
        ] {
            let outside = RenderEntry {
                pick_bounds: bounds,
                ..entry
            };
            assert!(!npc_name_visible(&actor, &outside, screen));
        }
    }

    #[test]
    fn ambient_bots_do_not_show_party_style_hp_even_when_server_supplies_it() {
        let mut bot=player();bot.id=95_000_001;bot.hp=Some(100);bot.max_hp=Some(100);
        assert!(!persistent_bar_eligible(&bot,false,false));
        assert!(!persistent_bar_eligible(&bot,false,true));
        let mut member=player();member.hp=Some(100);member.max_hp=Some(100);
        assert!(persistent_bar_eligible(&member,false,true));
    }

    #[test]
    fn monster_keeps_its_bar_off_hover_once_the_server_reports_hp() {
        let mut mob = monster();
        assert!(!persistent_bar_eligible(&mob, false, false));

        mob.mob_info = nostalre_game::mob_info::MobInfo::parse("Lv. 5 | HP: 100/200");
        assert!(persistent_bar_eligible(&mob, false, false));

        mob.mob_info = nostalre_game::mob_info::MobInfo::parse("Lv. 5");
        assert!(!persistent_bar_eligible(&mob, false, false));

        mob.mob_info = nostalre_game::mob_info::MobInfo::parse("HP: 0/200");
        mob.enter_dead();
        assert!(!persistent_bar_eligible(&mob, false, false));
    }

    #[test]
    fn stealth_takes_the_persistent_bar_away() {
        use nostalre_game::sprite_path::{OPTION_CHASEWALK, OPTION_CLOAK, OPTION_HIDE};

        let mut member = player();
        assert!(persistent_bar_eligible(&member, false, true));
        for option in [OPTION_HIDE, OPTION_CLOAK, OPTION_CLOAK | OPTION_CHASEWALK] {
            member.effect_state = option;
            assert!(!persistent_bar_eligible(&member, false, true));
        }
    }

    #[test]
    fn the_accessibility_path_is_wider_and_more_heavily_outlined() {
        let atlas = nostalre_renderer::FontAtlas::from_embedded(14.0, 1.0);
        assert!(bold_width("Poring", &atlas) > atlas.measure_text("Poring"));

        let mut plain = Vec::new();
        build_outlined_text("Poring", 10.0, 10.0, [1.0; 4], &atlas, &mut plain);
        assert_eq!(plain.len(), 2, "the ring is merged into one call");
        assert_eq!(
            plain[0].vertices.len(),
            plain[1].vertices.len() * nostalre_ui::draw::OUTLINE_RING_8.len()
        );

        let mut bold = Vec::new();
        build_outlined_bold_text("Poring", 10.0, 10.0, [1.0; 4], &atlas, &mut bold);
        assert_eq!(bold.len(), 2);
        assert_eq!(
            bold[0].vertices.len(),
            bold[1].vertices.len() * nostalre_ui::draw::OUTLINE_RING_8.len() * OUTLINE_PASSES
        );
    }

    #[test]
    fn a_title_goes_before_the_name_in_brackets() {
        let mut entity = Entity::new(3, EntityType::Npc, 46, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 150);
        assert_eq!(plate_name(&entity), None);
        entity.name = Some("Lidia".into());
        assert_eq!(plate_name(&entity).as_deref(), Some("Lidia"));
        entity.title = Some("VIP".into());
        assert_eq!(plate_name(&entity).as_deref(), Some("[VIP] Lidia"));
    }

    #[test]
    fn name_color_per_type_with_gm_and_pk_overrides() {
        let mut e = player();
        assert_eq!(entity_name_color(&e), [1.0, 1.0, 1.0, 1.0]);
        e.is_gm = true;
        assert_eq!(entity_name_color(&e), GM_TEXT_COLOR);
        e.effect_state = EFFECT_STATE_RED_NAME;
        assert_eq!(entity_name_color(&e), [1.0, 0.0, 0.0, 1.0]);

        assert_eq!(entity_name_color(&npc()), NPC_NAME_COLOR);
        assert_eq!(entity_name_color(&monster()), MONSTER_NAME_COLOR);
    }
}
