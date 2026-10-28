use crate::App;
use models::enums::EnumWithNumberValue;
use models::enums::effect_id::EffectId;
use models::enums::status::StatusTypes;
use nostalre_game::damage_number::DamageNumber;

impl App {
    pub(super) fn handle_recovery(&mut self, var_id: u16, amount: i32) {
        let Some(gid) = self.game.world.entities.player_id() else {
            return;
        };
        let (color, effect) = match StatusTypes::try_from_value(var_id as usize) {
            Ok(StatusTypes::Sp) => ([0.0, 0.0, 1.0], EffectId::Sptime),
            _ => ([0.0, 1.0, 0.0], EffectId::Hptime),
        };
        self.game
            .combat
            .damage_numbers
            .add(DamageNumber::effect_number(gid, amount, color, 0.0));
        self.effect_queue.spawn_on(effect, gid);
    }

    pub(super) fn handle_parameter_changed(&mut self, var_id: u16, value: i64) {
        if let Some(speed) = self.game.character.apply_parameter_changed(var_id, value)
            && let Some(entity) = self.game.world.entities.player_mut()
        {
            tracing::info!(
                previous = entity.speed,
                speed,
                "player movement speed from server"
            );
            entity.speed = speed;
            entity.movement.set_speed(speed);
        }
        if let Ok(StatusTypes::Baselevel) = StatusTypes::try_from_value(var_id as usize)
            && let Some(gid) = self.game.world.entities.player_id()
        {
            self.refresh_level_aura(gid);
        }
    }
}

impl App {
    /// The server took the title change (0x0a2f result 0): worn, and on the character's own name (0xcfb210);
    /// a refusal changes nothing and says nothing, as kRO's client
    pub(super) fn handle_title_change_answered(&mut self, ok: bool, title_id: i32) {
        if !ok {
            return;
        }
        self.game.character.titles.current = title_id;
        let title = self.game.data_table.titles.name(title_id);
        if let Some(entity) = self.game.world.entities.player_mut() {
            entity.title = (!title.is_empty()).then(|| title.to_string());
        }
    }
}
