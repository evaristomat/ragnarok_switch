use crate::App;
use models::enums::skill_enums::SkillEnum;
use nostalre_game::autocounter;
use nostalre_game::companion::OwnerCommand;
use nostalre_game::cursor::{CompanionSkillTarget, PendingSkillTarget};
use nostalre_game::entity::{EntityCategory, EntityState, EntityType};
use nostalre_game::movement::direction_from_delta;
use nostalre_game::path::try_move_to;
use nostalre_game::targeting::{
    TargetClass, can_attack, cures_silence, shift_allows_monster, skill_target_allowed,
    skill_target_class,
};
use nostalre_network::{
    build_change_direction_packet, build_contact_npc_packet, build_req_buy_frommc_packet,
    build_req_enter_room_packet, build_request_move_packet, build_use_skill_packet,
};

impl App {
    pub(crate) fn handle_left_click(&mut self) {
        if self.try_pet_modal_click() {
            return;
        }
        if self.try_marriage_target_click() {
            return;
        }
        if nostalre_profiling::debug::trace_input() {
            tracing::info!(
                "handle_left_click: pending_companion={:?} hovered_entity={:?}",
                self.game.pending_casts.pending_companion_skill.is_some(),
                self.game.hover.hovered_entity_id
            );
        }
        if self.windows.npc_dialog.dialog.is_open() || self.windows.npc_shop.shop.is_open() {
            return;
        }
        if autocounter::player_in_autocounter(&self.game.world.entities) {
            self.dispel_autocounter();
            return;
        }
        if self.game.casting_blocks_action()
            || self
                .game
                .world
                .entities
                .player()
                .is_some_and(|e| e.state() == EntityState::SkillExec)
        {
            return;
        }
        if let Some(entity_id) = self.game.hover.target_id()
            && Some(entity_id) == self.game.world.entities.player_id()
            && self
                .game
                .world
                .entities
                .get(entity_id)
                .is_some_and(|e| e.vending_board.is_some())
        {
            self.close_own_shop();
            return;
        }
        if let Some(room_id) = self.game.hover.hovered_chat_room {
            self.channel
                .send_packet(build_req_enter_room_packet(room_id, self.active_packetver));
            return;
        }
        if let Some(is_mercenary) = self.game.pending_casts.pending_companion_patrol.take() {
            if let Some((cx, cy)) = self.hovered_cell() {
                self.push_owner_command_to(
                    is_mercenary,
                    OwnerCommand::patrol(cx, cy),
                    self.input.shift_pressed,
                );
            }
            return;
        }
        if let Some(pending) = self.game.pending_casts.pending_companion_skill.take() {
            let reserved = self.input.shift_pressed;
            let target = match pending.target {
                CompanionSkillTarget::Entity => self.game.hover.hovered_entity_id,
                CompanionSkillTarget::SkillUnit => self.game.hover.hovered_skill_unit_id,
                CompanionSkillTarget::Ground => {
                    if let Some((cx, cy)) = self.hovered_cell() {
                        self.push_owner_command_to(
                            pending.is_mercenary,
                            OwnerCommand::skill_area(
                                pending.skill.id() as u16,
                                pending.level as u8,
                                cx,
                                cy,
                            ),
                            reserved,
                        );
                    }
                    return;
                }
            };
            if let Some(target) = target {
                self.push_owner_command_to(
                    pending.is_mercenary,
                    OwnerCommand::skill_object(
                        pending.skill.id() as u16,
                        pending.level as u8,
                        target,
                    ),
                    reserved,
                );
            }
            return;
        }
        if self.is_local_player_incapacitated() {
            return;
        }
        if let Some(pending) = self.game.pending_casts.pending_skill_target {
            if self.hidden_blocks_skill(pending.skill()) {
                self.game.pending_casts.pending_skill_target = None;
                return;
            }
            if self.skill_on_cooldown(pending.skill()) {
                return;
            }
            self.game.pending_casts.pending_skill_target = None;
            let mut skill_cast = false;
            match pending {
                PendingSkillTarget::Entity { skill, level } => {
                    let class = self
                        .game
                        .resolve_cast_skill(skill)
                        .map(|(target_type, _)| skill_target_class(target_type))
                        .unwrap_or(TargetClass::Offensive);
                    let player_id = self.game.world.entities.player_id();
                    let shift = self.input.shift_pressed;
                    let noshift = self.game.prefs.noshift_mode;
                    let valid_target = self.game.hover.hovered_entity_id.filter(|&id| {
                        self.game.world.entities.get(id).is_some_and(|e| {
                            (skill_target_allowed(
                                class,
                                e,
                                &self.game.session.map_properties,
                                player_id,
                            ) || cures_silence(skill, e))
                                && shift_allows_monster(class, e, shift, noshift)
                        })
                    });
                    if let Some(entity_id) = valid_target {
                        skill_cast = self.cast_skill_on_unit(skill, level, entity_id);
                    }
                }
                PendingSkillTarget::SkillUnit { skill, level } => {
                    if let Some(unit_id) = self.game.hover.hovered_skill_unit_id
                        && let Some(cell) = self.game.world.trap_units.get(&unit_id).map(|t| t.cell)
                    {
                        let reach = self.skill_reach(skill, (cell.0 as i32, cell.1 as i32));
                        if reach.in_range() {
                            self.channel.send_packet(build_use_skill_packet(
                                skill,
                                level,
                                unit_id,
                                self.active_packetver,
                            ));
                        } else {
                            self.game.pending_casts.pending_skill_unit_cast =
                                Some((skill, level, unit_id));
                            reach.walk(self);
                        }
                        skill_cast = true;
                    }
                }
                PendingSkillTarget::Ground { skill, level } => {
                    if let Some((cx, cy)) = self.hovered_cell() {
                        self.cast_skill_on_cell(skill, level, cx as i16, cy as i16);
                    }
                    skill_cast = true;
                }
            }
            if skill_cast {
                self.game.combat.attack_target_id = None;
            }
            return;
        }
        if let Some(item_id) = self.game.hover.hovered_floor_item_id {
            if self.player_hidden() {
                return;
            }
            self.game.combat.attack_target_id = None;
            self.game.pending_casts.pending_pickup_item_id = self
                .game
                .world
                .floor_items
                .contains_key(&item_id)
                .then_some(item_id);
            self.check_pending_pickup();
            return;
        }
        if let Some(entity_id) = self.game.hover.target_id()
            && Some(entity_id) != self.game.world.entities.player_id()
            && self
                .game
                .world
                .entities
                .get(entity_id)
                .is_some_and(|e| e.vending_board.is_some())
        {
            self.channel.send_packet(build_req_buy_frommc_packet(
                entity_id,
                self.active_packetver,
            ));
            return;
        }
        if let Some(entity_id) = self.game.hover.hovered_entity_id
            && let Some(entity) = self.game.world.entities.get(entity_id)
            && entity.category() == EntityCategory::Npc
        {
            self.channel
                .send_packet(build_contact_npc_packet(entity_id, self.active_packetver));
            return;
        }
        if let Some(entity_id) = self.game.hover.hovered_entity_id
            && let Some(entity) = self.game.world.entities.get(entity_id)
        {
            let player_id = self.game.world.entities.player_id();
            let should_attack = match entity.entity_type {
                EntityType::Monster => !self.input.shift_pressed && !entity.is_pet,
                EntityType::Player => {
                    can_attack(entity, &self.game.session.map_properties, player_id)
                        && (!self.game.session.map_properties.no_lockon()
                            || self.input.shift_pressed
                            || self.game.prefs.noshift_mode)
                }
                _ => false,
            };
            if should_attack {
                self.initiate_attack(entity_id);
                return;
            }
        }
        if let Some(unit_id) = self.game.hover.hovered_skill_unit_id
            && self.game.world.trap_units.contains_key(&unit_id)
        {
            self.initiate_attack(unit_id);
            return;
        }
        self.stop_attacking();
        self.game.pending_casts.pending_pickup_item_id = None;
        self.game.pending_casts.pending_ground_cast = None;
        self.game.pending_casts.pending_skill_unit_cast = None;
        // The server moves us in a straight line and rejects any move of ours, which
        // snaps the character back: click-to-move (and the continuous walk that
        // routes through here) waits.
        if self.game.player_moves_on_its_own() {
            return;
        }
        if self.player_hide_move_blocked() {
            return;
        }
        let (dest_x, dest_y) = match self.hovered_cell() {
            Some(c) => c,
            None => return,
        };
        if self.turn_while_sitting(dest_x, dest_y) {
            return;
        }
        let locked_state = self
            .game
            .world
            .entities
            .player()
            .filter(|e| e.is_move_locked())
            .map(|e| e.state());
        if let Some(state) = locked_state {
            self.game.combat.queued_move = Some((dest_x, dest_y));
            self.game.combat.queued_move_state = Some(state);
            return;
        }
        let gat = match &self.game.session.gat {
            Some(g) => g,
            None => return,
        };

        let (src_x, src_y) = self
            .game
            .world
            .entities
            .player()
            .map(|e| e.movement.cell_position())
            .unwrap_or((0, 0));

        let move_action = match try_move_to(gat, src_x, src_y, dest_x, dest_y) {
            Some(a) => a,
            None => return,
        };

        self.channel.send_packet(build_request_move_packet(
            move_action.dest_x,
            move_action.dest_y,
            self.active_packetver,
        ));
    }

    fn turn_while_sitting(&mut self, dest_x: i32, dest_y: i32) -> bool {
        let pv = self.active_packetver;
        let Some(entity) = self.game.world.entities.player_mut() else {
            return false;
        };
        if entity.state() != EntityState::Sitting {
            return false;
        }
        let (src_x, src_y) = entity.movement.cell_position();
        let dx = dest_x as f32 - src_x as f32;
        let dy = dest_y as f32 - src_y as f32;
        if let Some(target) = direction_from_delta(dx, dy) {
            entity.look_at_direction(target);
            let (head_dir, dir) = (entity.head_dir, entity.direction);
            self.channel
                .send_packet(build_change_direction_packet(head_dir, dir, pv));
        }
        true
    }
}

/// The caster's cell, a skill's range and the cell it is cast at, for the cast or the walk into range
struct SkillReach {
    from: (u16, u16),
    to: (i32, i32),
    range: i32,
}

impl SkillReach {
    fn in_range(&self) -> bool {
        (i32::from(self.from.0) - self.to.0)
            .abs()
            .max((i32::from(self.from.1) - self.to.1).abs())
            <= self.range
    }

    fn walk(&self, app: &mut App) -> bool {
        app.try_move_toward(self.to.0, self.to.1, self.from.0, self.from.1, self.range)
    }
}

impl App {
    fn skill_reach(&self, skill: SkillEnum, to: (i32, i32)) -> SkillReach {
        let from = self
            .game
            .world
            .entities
            .player()
            .map(|e| e.movement.cell_position())
            .unwrap_or((0, 0));
        SkillReach {
            from,
            to,
            range: self
                .game
                .resolve_cast_skill(skill)
                .map(|(_, range)| range as i32)
                .unwrap_or(1),
        }
    }

    /// Casts `skill` on a unit as a click on it does: at once in range, else walking there first. Whether it went
    /// out now
    pub(crate) fn cast_skill_on_unit(&mut self, skill: SkillEnum, level: i16, entity_id: u32) -> bool {
        let Some(cell) = self
            .game
            .world
            .entities
            .get(entity_id)
            .map(|e| e.movement.cell_position())
        else {
            return false;
        };
        let reach = self.skill_reach(skill, (cell.0 as i32, cell.1 as i32));
        if reach.in_range() {
            self.channel.send_packet(build_use_skill_packet(
                skill,
                level,
                entity_id,
                self.active_packetver,
            ));
            if skill == SkillEnum::BsRepairweapon {
                self.game.pending_casts.pending_repair_target = Some(entity_id);
            }
            return true;
        }
        if reach.walk(self) {
            self.game.pending_casts.pending_skill = Some(skill);
            self.game.pending_casts.pending_skill_level = Some(level);
            self.game.combat.attack_target_id = Some(entity_id);
        }
        false
    }

    /// Casts a ground skill at a cell as a click on it does: at once in range, else walking there first
    pub(crate) fn cast_skill_on_cell(&mut self, skill: SkillEnum, level: i16, x: i16, y: i16) {
        let reach = self.skill_reach(skill, (x as i32, y as i32));
        if reach.in_range() {
            self.cast_on_ground(skill, level, x, y);
        } else {
            self.game.pending_casts.pending_ground_cast = Some((skill, level, x, y));
            reach.walk(self);
        }
    }
}
