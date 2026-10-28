use crate::char_name::CharNameCache;
use crate::cooldown::CooldownTracker;
use crate::event::{CharacterInfo, Ranking};
use crate::hotkey::HotkeyBar;
use crate::inventory::{CartData, InventoryData, StorageData};
use crate::mail::MailState;
use crate::skill::{ItemSkills, SkillList};
use crate::sprite_path::{dual_wield_type, weapon_view_id_to_type};
use crate::trade::TradeData;
use models::enums::EnumWithStringValue;
use models::enums::item::EquipmentLocation;
use models::enums::weapon::WeaponType;

/// Times are local-clock milliseconds; `end_ms` is `None` for permanent statuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveStatus {
    pub efst: i16,
    pub val1: i32,
    pub start_ms: u64,
    pub end_ms: Option<u64>,
    pub icon_loaded: bool,
}

/// One trait stat (POW to CRT): its points, the bonus of equipment and statuses, and the trait points the next
/// point costs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TraitStat {
    pub base: u16,
    pub bonus: i16,
    pub cost: u16,
}

/// The 4th jobs' stats, rAthena's `SP_POW` (219) to `SP_UCRT` (252): the six trait stats, what they add up to, the
/// trait points to spend and AP. Every character gets them on a Renewal server; they stay 0 below the 4th jobs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Traits {
    /// POW, STA, WIS, SPL, CON, CRT, in status id order
    pub stats: [TraitStat; 6],
    pub p_atk: i32,
    pub s_matk: i32,
    pub res: i32,
    pub mres: i32,
    pub h_plus: i32,
    pub c_rate: i32,
    pub points: u32,
    pub ap: u32,
    pub max_ap: u32,
}

/// Status ids of the trait stats and of their costs (`SP_POW`, `SP_UPOW`)
pub const TRAIT_STAT_FIRST_ID: u16 = 219;
const TRAIT_COST_FIRST_ID: u16 = 247;

impl Traits {
    /// The trait stat a status id names (219 to 224), as an index into `stats`
    pub fn stat_index(status_id: u16) -> Option<usize> {
        let index = status_id.checked_sub(TRAIT_STAT_FIRST_ID)? as usize;
        (index < 6).then_some(index)
    }

    /// A parameter change (0x00b0 values, 0x00be costs, 0x00bc raises) for a trait status id; other ids are ignored
    fn apply_parameter(&mut self, status: models::enums::status::StatusTypes, value: i32) {
        use models::enums::EnumWithNumberValue;
        use models::enums::status::StatusTypes;
        let id = status.value() as u16;
        if let Some(i) = Self::stat_index(id) {
            self.stats[i].base = value as u16;
            return;
        }
        if let Some(i) = id.checked_sub(TRAIT_COST_FIRST_ID).filter(|i| *i < 6) {
            self.stats[i as usize].cost = value as u16;
            return;
        }
        match status {
            StatusTypes::Patk => self.p_atk = value,
            StatusTypes::Smatk => self.s_matk = value,
            StatusTypes::Res => self.res = value,
            StatusTypes::Mres => self.mres = value,
            StatusTypes::Hplus => self.h_plus = value,
            StatusTypes::Crate => self.c_rate = value,
            StatusTypes::Tstatuspoint => self.points = value as u32,
            StatusTypes::Ap => self.ap = value as u32,
            StatusTypes::Maxap => self.max_ap = value as u32,
            _ => {}
        }
    }
}

pub struct Character {
    pub inventory: InventoryData,
    pub cart: CartData,
    pub storage: StorageData,
    pub trade: TradeData,
    pub refine: crate::refine::RefineSession,
    pub stylist: crate::stylist::StylistSession,
    pub titles: crate::title::Titles,
    pub equip_switch: crate::equip_switch::EquipSwitch,
    pub mail: MailState,
    pub skills: SkillList,
    pub item_skills: ItemSkills,
    pub hotkeys: HotkeyBar,
    pub cooldowns: CooldownTracker,
    pub name: String,
    pub skill_point: u32,
    pub status_point: u32,
    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
    pub base_level: u16,
    pub job_level: u32,
    pub base_exp: u64,
    pub job_exp: u64,
    pub next_base_exp: u64,
    pub next_job_exp: u64,
    pub str: u8,
    pub agi: u8,
    pub vit: u8,
    pub int: u8,
    pub dex: u8,
    pub luk: u8,
    pub str_bonus: i16,
    pub agi_bonus: i16,
    pub vit_bonus: i16,
    pub int_bonus: i16,
    pub dex_bonus: i16,
    pub luk_bonus: i16,
    pub str_cost: u16,
    pub agi_cost: u16,
    pub vit_cost: u16,
    pub int_cost: u16,
    pub dex_cost: u16,
    pub luk_cost: u16,
    pub atk1: i32,
    pub atk2: i32,
    pub matk1: i32,
    pub matk2: i32,
    pub def1: i32,
    pub def2: i32,
    pub mdef1: i32,
    pub mdef2: i32,
    pub hit: i32,
    pub flee1: i32,
    pub flee2: i32,
    pub critical: i32,
    pub aspd: i32,
    pub effect_state: i32,
    pub cart_design: Option<u8>,
    pub active_statuses: Vec<ActiveStatus>,
    /// Married partner's name (from ZC_COUPLENAME); empty when unpartnered.
    pub partner_name: String,
    /// Names of the blacksmiths/alchemists who made the forged and created
    /// items we hold, and of the ranked.
    pub char_names: CharNameCache,
    /// A ranking waiting for its names.
    pub pending_ranking: Option<Ranking>,
    /// Right- and left-hand view ids from the last weapon look change. The left
    /// value is either a shield or an off-hand weapon and only the inventory
    /// tells which, so the pair is kept to be re-resolved when the equipment
    /// list lands after the look change.
    pub hand_look: (u16, u16),
    pub traits: Traits,
}

impl Default for Character {
    fn default() -> Self {
        Self::new()
    }
}

impl Character {
    pub fn new() -> Self {
        Self {
            inventory: InventoryData::new(),
            cart: CartData::new(),
            storage: StorageData::new(),
            trade: TradeData::new(),
            refine: Default::default(),
            stylist: Default::default(),
            titles: Default::default(),
            equip_switch: Default::default(),
            mail: MailState::new(),
            skills: SkillList::new(),
            item_skills: ItemSkills::default(),
            hotkeys: HotkeyBar::new(),
            cooldowns: CooldownTracker::new(),
            name: String::new(),
            skill_point: 0,
            status_point: 0,
            hp: 0,
            max_hp: 0,
            sp: 0,
            max_sp: 0,
            base_level: 0,
            job_level: 0,
            base_exp: 0,
            job_exp: 0,
            next_base_exp: 0,
            next_job_exp: 0,
            str: 0,
            agi: 0,
            vit: 0,
            int: 0,
            dex: 0,
            luk: 0,
            str_bonus: 0,
            agi_bonus: 0,
            vit_bonus: 0,
            int_bonus: 0,
            dex_bonus: 0,
            luk_bonus: 0,
            str_cost: 0,
            agi_cost: 0,
            vit_cost: 0,
            int_cost: 0,
            dex_cost: 0,
            luk_cost: 0,
            atk1: 0,
            atk2: 0,
            matk1: 0,
            matk2: 0,
            def1: 0,
            def2: 0,
            mdef1: 0,
            mdef2: 0,
            hit: 0,
            flee1: 0,
            flee2: 0,
            critical: 0,
            aspd: 0,
            effect_state: 0,
            cart_design: None,
            active_statuses: Vec::new(),
            partner_name: String::new(),
            char_names: CharNameCache::default(),
            pending_ranking: None,
            hand_look: (0, 0),
            traits: Traits::default(),
        }
    }

    /// Weapon type and shield view the two hands add up to, off-hand weapons
    /// folded into the combined dual-wield type.
    pub fn resolve_hand_look(&self) -> (Option<WeaponType>, u16) {
        let (right, left) = self.hand_look;
        let right_type = weapon_view_id_to_type(right);
        let left_is_weapon = self
            .inventory
            .equipped_in_slot(EquipmentLocation::HandLeft)
            .is_some_and(|item| item.is_weapon());
        if !left_is_weapon {
            return (right_type, left);
        }
        let weapon = match (right_type, weapon_view_id_to_type(left)) {
            (Some(r), Some(l)) => dual_wield_type(r, l).or(Some(r)),
            (None, Some(l)) => Some(l),
            _ => right_type,
        };
        (weapon, 0)
    }

    /// Sentinel the server sends for infinite-duration statuses: any tick `<= 0` is
    /// rewritten to this before transmission, so the icon must show no countdown.
    pub const PERMANENT_STATUS_TICK: u64 = 9999;

    /// `life_ms == 0` or [`PERMANENT_STATUS_TICK`] means permanent (no expiry).
    /// Re-applying an existing status updates timing in place.
    pub fn apply_status(
        &mut self,
        efst: i16,
        val1: i32,
        now_ms: u64,
        life_ms: u64,
        icon_loaded: bool,
    ) {
        let end_ms = if life_ms == 0 || life_ms == Self::PERMANENT_STATUS_TICK {
            None
        } else {
            Some(now_ms + life_ms)
        };
        if let Some(existing) = self.active_statuses.iter_mut().find(|s| s.efst == efst) {
            existing.val1 = val1;
            existing.start_ms = now_ms;
            existing.end_ms = end_ms;
            existing.icon_loaded = icon_loaded;
        } else {
            self.active_statuses.push(ActiveStatus {
                efst,
                val1,
                start_ms: now_ms,
                end_ms,
                icon_loaded,
            });
        }
    }

    /// Moves the acked add request off the inventory and onto my side of the deal.
    /// The server never reports the deduction: it deletes silently on commit and
    /// re-sends a pickup for every staged item on cancel.
    pub fn commit_trade_add(&mut self) {
        let Some((index, count)) = self.trade.take_pending_add() else {
            return;
        };
        if index == crate::trade::TRADE_ZENY_INDEX {
            self.trade.add_my_zeny(count as i64);
            self.inventory.zeny = (self.inventory.zeny - count).max(0);
            return;
        }
        let Some(src) = self.inventory.get_item(index) else {
            return;
        };
        let mut item = src.clone();
        item.count = count as i16;
        self.trade.add_my_item(item);
        self.inventory.subtract_item_count(index, count as i16);
    }

    /// Char ids of item makers and the ranked whose name we still need and may ask for now.
    pub fn pending_char_names(&mut self, now_ms: u64) -> Vec<u32> {
        let mut char_ids: Vec<u32> = self
            .inventory
            .all_items()
            .iter()
            .chain(self.cart.all_items())
            .chain(self.storage.all_items())
            .chain(self.trade.my_items())
            .chain(self.trade.other_items())
            .filter_map(|item| item.producer_char_id())
            .chain(
                self.pending_ranking
                    .iter()
                    .flat_map(|r| r.entries.iter().map(|(id, _)| *id)),
            )
            .collect();
        char_ids.retain(|id| self.char_names.should_request(*id, now_ms));
        char_ids
    }

    pub fn clear_status(&mut self, efst: i16) {
        self.active_statuses.retain(|s| s.efst != efst);
    }

    pub fn has_status(&self, efst: i16) -> bool {
        self.active_statuses.iter().any(|s| s.efst == efst)
    }

    pub fn prune_expired(&mut self, now_ms: u64) {
        self.active_statuses
            .retain(|s| s.end_ms.is_none_or(|end| now_ms < end));
    }

    pub fn init_from_info(&mut self, info: &CharacterInfo) {
        self.name = info.name.clone();
        self.hp = info.hp;
        self.max_hp = info.max_hp;
        self.sp = info.sp;
        self.max_sp = info.max_sp;
        self.base_level = info.base_level;
        self.job_level = info.job_level;
        self.str = info.str;
        self.agi = info.agi;
        self.vit = info.vit;
        self.int = info.int;
        self.dex = info.dex;
        self.luk = info.luk;
        self.effect_state = info.effect_state;
        self.inventory.zeny = info.zeny;
    }

    pub fn hp_percentage(&self) -> f32 {
        if self.max_hp > 0 {
            self.hp as f32 / self.max_hp as f32
        } else {
            0.0
        }
    }

    pub fn sp_percentage(&self) -> f32 {
        if self.max_sp > 0 {
            self.sp as f32 / self.max_sp as f32
        } else {
            0.0
        }
    }

    pub fn base_exp_percentage(&self) -> f32 {
        if self.next_base_exp > 0 {
            self.base_exp as f32 / self.next_base_exp as f32
        } else {
            0.0
        }
    }

    pub fn job_exp_percentage(&self) -> f32 {
        if self.next_job_exp > 0 {
            self.job_exp as f32 / self.next_job_exp as f32
        } else {
            0.0
        }
    }

    /// Share of a level that `amount` experience represents, in percent.
    pub fn exp_gain_percentage(&self, amount: i64, is_base: bool) -> f32 {
        let next = if is_base {
            self.next_base_exp
        } else {
            self.next_job_exp
        };
        if next == 0 {
            return 0.0;
        }
        amount as f32 / next as f32 * 100.0
    }

    pub fn apply_parameter_changed(&mut self, var_id: u16, value: i64) -> Option<u16> {
        use models::enums::EnumWithNumberValue;
        use models::enums::status::StatusTypes;
        let Ok(status) = StatusTypes::try_from_value(var_id as usize) else {
            return None;
        };
        // Only experience is 64-bit (0x0acb); the rest fit the 32-bit packets.
        let exp = value as u64;
        let value = value as i32;
        match status {
            StatusTypes::Speed => return Some(value as u16),
            StatusTypes::Hp => self.hp = value as u32,
            StatusTypes::Maxhp => self.max_hp = value as u32,
            StatusTypes::Sp => self.sp = value as u32,
            StatusTypes::Maxsp => self.max_sp = value as u32,
            StatusTypes::Baseexp => self.base_exp = exp,
            StatusTypes::Jobexp => self.job_exp = exp,
            StatusTypes::Baselevel => self.base_level = value as u16,
            StatusTypes::Str => self.str = value as u8,
            StatusTypes::Agi => self.agi = value as u8,
            StatusTypes::Vit => self.vit = value as u8,
            StatusTypes::Int => self.int = value as u8,
            StatusTypes::Dex => self.dex = value as u8,
            StatusTypes::Luk => self.luk = value as u8,
            StatusTypes::Joblevel => self.job_level = value as u32,
            StatusTypes::Weight => self.inventory.weight = value,
            StatusTypes::Maxweight => self.inventory.max_weight = value,
            StatusTypes::Zeny => self.inventory.zeny = value,
            StatusTypes::Skillpoint => self.skill_point = value as u32,
            StatusTypes::Statuspoint => self.status_point = value as u32,
            StatusTypes::Nextbaseexp => self.next_base_exp = exp,
            StatusTypes::Nextjobexp => self.next_job_exp = exp,
            StatusTypes::StrNextLevelIncreaseCost => self.str_cost = value as u16,
            StatusTypes::AgiNextLevelIncreaseCost => self.agi_cost = value as u16,
            StatusTypes::VitNextLevelIncreaseCost => self.vit_cost = value as u16,
            StatusTypes::IntNextLevelIncreaseCost => self.int_cost = value as u16,
            StatusTypes::DexNextLevelIncreaseCost => self.dex_cost = value as u16,
            StatusTypes::LukNextLevelIncreaseCost => self.luk_cost = value as u16,
            StatusTypes::Atk1 => self.atk1 = value,
            StatusTypes::Atk2 => self.atk2 = value,
            StatusTypes::Matk1 => self.matk1 = value,
            StatusTypes::Matk2 => self.matk2 = value,
            StatusTypes::Def1 => self.def1 = value,
            StatusTypes::Def2 => self.def2 = value,
            StatusTypes::Mdef1 => self.mdef1 = value,
            StatusTypes::Mdef2 => self.mdef2 = value,
            StatusTypes::Hit => self.hit = value,
            StatusTypes::Flee1 => self.flee1 = value,
            StatusTypes::Flee2 => self.flee2 = value,
            StatusTypes::Critical => self.critical = value,
            StatusTypes::Aspd => self.aspd = value,
            _ => self.traits.apply_parameter(status, value),
        }
        None
    }

    pub fn apply_status_changed(&mut self, status_type: u32, base: i32, bonus: i32) {
        use models::enums::EnumWithNumberValue;
        use models::enums::status::StatusTypes;
        if let Ok(status) = StatusTypes::try_from_value(status_type as usize) {
            match status {
                StatusTypes::Str => {
                    self.str = base as u8;
                    self.str_bonus = bonus as i16;
                }
                StatusTypes::Agi => {
                    self.agi = base as u8;
                    self.agi_bonus = bonus as i16;
                }
                StatusTypes::Vit => {
                    self.vit = base as u8;
                    self.vit_bonus = bonus as i16;
                }
                StatusTypes::Int => {
                    self.int = base as u8;
                    self.int_bonus = bonus as i16;
                }
                StatusTypes::Dex => {
                    self.dex = base as u8;
                    self.dex_bonus = bonus as i16;
                }
                StatusTypes::Luk => {
                    self.luk = base as u8;
                    self.luk_bonus = bonus as i16;
                }
                _ => {
                    if let Some(i) = Traits::stat_index(status_type as u16) {
                        self.traits.stats[i].base = base as u16;
                        self.traits.stats[i].bonus = bonus as i16;
                    }
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.inventory.clear();
        self.cart.clear();
        self.skills.clear();
        self.item_skills.clear();
        self.hotkeys.clear();
        self.cooldowns.clear();
        self.name.clear();
        self.skill_point = 0;
        self.status_point = 0;
        self.hp = 0;
        self.max_hp = 0;
        self.sp = 0;
        self.max_sp = 0;
        self.base_level = 0;
        self.job_level = 0;
        self.base_exp = 0;
        self.job_exp = 0;
        self.next_base_exp = 0;
        self.next_job_exp = 0;
        self.str = 0;
        self.agi = 0;
        self.vit = 0;
        self.int = 0;
        self.dex = 0;
        self.luk = 0;
        self.str_bonus = 0;
        self.agi_bonus = 0;
        self.vit_bonus = 0;
        self.int_bonus = 0;
        self.dex_bonus = 0;
        self.luk_bonus = 0;
        self.str_cost = 0;
        self.agi_cost = 0;
        self.vit_cost = 0;
        self.int_cost = 0;
        self.dex_cost = 0;
        self.luk_cost = 0;
        self.atk1 = 0;
        self.atk2 = 0;
        self.matk1 = 0;
        self.matk2 = 0;
        self.def1 = 0;
        self.def2 = 0;
        self.mdef1 = 0;
        self.mdef2 = 0;
        self.hit = 0;
        self.flee1 = 0;
        self.flee2 = 0;
        self.critical = 0;
        self.aspd = 0;
        self.effect_state = 0;
        self.cart_design = None;
        self.active_statuses.clear();
        self.partner_name.clear();
        self.char_names.clear();
        self.traits = Traits::default();
    }
}

/// A 4th job: one with AP (`BaseAp` in rAthena's `job_basepoints.yml`; Sky Emperor to Spirit Handler too, which
/// are not `JOBL_FOURTH`). Trait stats exist for everyone on a Renewal server, only these earn trait points.
pub fn is_fourth_job(class_id: u16) -> bool {
    crate::job_tables::has_ap(class_id)
}

/// The classic job whose rules (motions, sounds, effects) `job` follows: [`classic_job`](crate::sprite_path::classic_job).
pub fn classic_job_name(job: u16) -> Option<models::enums::class::JobName> {
    use models::enums::EnumWithNumberValue;
    models::enums::class::JobName::try_from_value(crate::sprite_path::classic_job(job) as usize)
        .ok()
}

pub fn job_class_name(class_id: u16) -> &'static str {
    use models::enums::EnumWithNumberValue;
    use models::enums::class::JobName;
    JobName::try_from_value(class_id as usize)
        .map(|j| j.as_str())
        .unwrap_or("Adventurer")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::EquipmentItemData;

    #[test]
    fn job_class_names() {
        assert_eq!(job_class_name(0), "Novice");
        assert_eq!(job_class_name(4008), "Lord Knight");
        assert_eq!(job_class_name(4013), "Assassin Cross");
        assert_eq!(job_class_name(22), "Wedding");
        assert_eq!(job_class_name(4054), "Rune Knight");
        assert_eq!(job_class_name(4060), "Rune Knight");
        assert_eq!(job_class_name(4098), "Baby Ranger");
        assert_eq!(job_class_name(4252), "Dragon Knight");
        assert_eq!(job_class_name(4308), "Spirit Handler");
        assert_eq!(job_class_name(9999), "Adventurer");
    }

    #[test]
    fn exp_gain_percentage_follows_the_next_level_requirement() {
        use models::enums::EnumWithNumberValue;
        use models::enums::status::StatusTypes;
        let mut char = Character::new();

        assert_eq!(char.exp_gain_percentage(1500, true), 0.0);

        char.apply_parameter_changed(StatusTypes::Nextbaseexp.value() as u16, 300_000);
        char.apply_parameter_changed(StatusTypes::Nextjobexp.value() as u16, 100_000);

        assert_eq!(char.exp_gain_percentage(1500, true), 0.5);
        assert_eq!(char.exp_gain_percentage(1500, false), 1.5);
    }

    #[test]
    fn experience_above_32_bits_is_kept() {
        use models::enums::EnumWithNumberValue;
        use models::enums::status::StatusTypes;
        let mut char = Character::new();
        char.apply_parameter_changed(StatusTypes::Baseexp.value() as u16, 3_000_000_000);
        char.apply_parameter_changed(StatusTypes::Nextbaseexp.value() as u16, 6_000_000_000);
        assert_eq!(char.base_exp_percentage(), 0.5);
        assert_eq!(char.exp_gain_percentage(60_000_000, true), 1.0);
    }

    #[test]
    fn staged_trade_item_leaves_the_inventory_and_frees_its_slot() {
        use crate::data_table::item_name_table::ItemNameTable;
        use crate::inventory::NormalItemData;
        use models::enums::item::ItemType;
        use std::collections::HashMap;

        let names = HashMap::from([
            (991u32, "Crystal Blue".to_string()),
            (992u32, "Wind of Verdure".to_string()),
        ]);
        let mut data = crate::data_table::DataTable::default();
        data.item_name = Some(ItemNameTable::from_entries(names.clone(), names));

        let mut char = Character::new();
        char.inventory.apply_normal_items(
            vec![NormalItemData {
                index: 5,
                item_id: 991,
                item_type: 3,
                is_identified: true,
                count: 20,
                wear_state: 0,
                slot: [0; 4],
            }],
            &data,
        );

        char.trade.begin("Bob".into(), 2000, 50, 50);
        char.trade.set_pending_add(5, 20);
        char.commit_trade_add();

        assert_eq!(char.trade.my_items().len(), 1);
        assert_eq!(char.trade.my_items()[0].item_id, 991);
        assert!(
            char.inventory.get_item(5).is_none(),
            "the server deletes a traded item without telling us"
        );

        let (name, _, _) = char
            .inventory
            .apply_item_pickup(
                5,
                992,
                157,
                ItemType::Etc,
                true,
                false,
                0,
                [0; 4],
                0,
                0,
                &data,
                &char.char_names,
            )
            .unwrap();
        assert_eq!(name, "Wind of Verdure");
        assert_eq!(char.inventory.get_item(5).unwrap().count, 157);
    }

    #[test]
    fn dual_wield_look_survives_an_emptied_inventory() {
        let data = crate::data_table::DataTable::default();
        let mut char = Character::new();
        let daggers = vec![
            EquipmentItemData {
                grade: 0,
                options: Default::default(),
                index: 1,
                item_id: 1201,
                item_type: 5,
                is_identified: true,
                location: 2,
                wear_state: 2,
                is_damaged: false,
                refining_level: 0,
                slot: [0; 4],
            },
            EquipmentItemData {
                grade: 0,
                options: Default::default(),
                index: 2,
                item_id: 1202,
                item_type: 5,
                is_identified: true,
                location: 32,
                wear_state: 32,
                is_damaged: false,
                refining_level: 0,
                slot: [0; 4],
            },
        ];
        char.inventory.apply_equipment_items(daggers.clone(), &data);
        char.hand_look = (1201, 1202);
        assert_eq!(
            char.resolve_hand_look(),
            (Some(WeaponType::DoubleDd), 0),
            "both daggers known"
        );

        char.inventory.clear();
        assert_eq!(char.resolve_hand_look(), (Some(WeaponType::Dagger), 1202));

        char.inventory.apply_equipment_items(daggers, &data);
        assert_eq!(char.resolve_hand_look(), (Some(WeaponType::DoubleDd), 0));
    }

    #[test]
    fn shield_look_is_kept_when_the_off_hand_is_not_a_weapon() {
        let data = crate::data_table::DataTable::default();
        let mut char = Character::new();
        char.inventory.apply_equipment_items(
            vec![EquipmentItemData {
                grade: 0,
                options: Default::default(),
                index: 1,
                item_id: 2104,
                item_type: 4,
                is_identified: true,
                location: 32,
                wear_state: 32,
                is_damaged: false,
                refining_level: 0,
                slot: [0; 4],
            }],
            &data,
        );
        char.hand_look = (1101, 2);
        assert_eq!(char.resolve_hand_look(), (Some(WeaponType::Sword1H), 2));
    }

    #[test]
    fn parameter_changed_updates_stats_and_returns_speed() {
        let mut char = Character::new();

        assert!(char.apply_parameter_changed(5, 500).is_none());
        assert_eq!(char.hp, 500);

        char.apply_parameter_changed(6, 1000);
        assert_eq!(char.max_hp, 1000);
        assert!((char.hp_percentage() - 0.5).abs() < 0.01);

        char.apply_parameter_changed(7, 80);
        assert_eq!(char.sp, 80);

        char.apply_parameter_changed(20, 999999);
        assert_eq!(char.inventory.zeny, 999999);

        let result = char.apply_parameter_changed(0, 150);
        assert_eq!(result, Some(150));

        char.apply_parameter_changed(13, 42);
        assert_eq!(char.str, 42);

        char.apply_parameter_changed(12, 5);
        assert_eq!(char.skill_point, 5);
    }

    #[test]
    fn init_from_info_populates_zeny_from_char_select() {
        let info = CharacterInfo {
            gid: 1,
            name: "Knight".into(),
            class: 7,
            base_level: 50,
            base_exp: 0,
            job_level: 42,
            map: "prontera".into(),
            slot: 0,
            head: 1,
            hair_color: 0,
            cloth_color: 0,
            weapon: 2,
            head_top: 0,
            head_mid: 0,
            head_bottom: 0,
            shield: 0,
            robe: 0,
            sex: 1,
            hp: 3000,
            max_hp: 3500,
            sp: 100,
            max_sp: 150,
            str: 50,
            agi: 30,
            vit: 40,
            int: 10,
            dex: 20,
            luk: 10,
            effect_state: 0,
            zeny: 123_456,
        };

        let mut char = Character::new();
        char.init_from_info(&info);
        assert_eq!(char.inventory.zeny, 123_456);
    }

    #[test]
    fn status_changed_updates_base_stats() {
        let mut char = Character::new();
        char.apply_status_changed(13, 50, 5);
        assert_eq!(char.str, 50);
        assert_eq!(char.str_bonus, 5);
        char.apply_status_changed(14, 30, 0);
        assert_eq!(char.agi, 30);
        char.apply_status_changed(17, 99, -2);
        assert_eq!(char.dex, 99);
        assert_eq!(char.dex_bonus, -2);
    }

    #[test]
    fn status_lifecycle_apply_clear_and_prune() {
        let mut char = Character::new();
        char.apply_status(10, 0, 1_000, 30_000, true);
        char.apply_status(2, 0, 1_000, 0, true);
        assert_eq!(char.active_statuses.len(), 2);

        char.apply_status(10, 0, 5_000, 60_000, true);
        assert_eq!(char.active_statuses.len(), 2);
        assert_eq!(char.active_statuses[0].efst, 10);
        assert_eq!(char.active_statuses[0].end_ms, Some(65_000));

        char.clear_status(2);
        assert_eq!(char.active_statuses.len(), 1);

        char.prune_expired(64_000);
        assert_eq!(char.active_statuses.len(), 1);
        char.prune_expired(65_000);
        assert!(char.active_statuses.is_empty());
    }

    #[test]
    fn permanent_tick_sentinel_never_expires() {
        let mut char = Character::new();
        char.apply_status(35, 0, 1_000, Character::PERMANENT_STATUS_TICK, true);
        assert_eq!(char.active_statuses[0].end_ms, None);
        char.prune_expired(u64::MAX);
        assert_eq!(char.active_statuses.len(), 1);
    }

    #[test]
    fn trait_stats_take_rathena_status_ids() {
        let mut char = Character::new();
        // 0x0141: POW 5 + 2, CRT 3
        char.apply_status_changed(219, 5, 2);
        char.apply_status_changed(224, 3, 0);
        // 0x00b0: P.ATK, C.RATE, trait points, AP and max AP
        for (id, value) in [(225, 7), (230, 4), (231, 12), (232, 40), (233, 200)] {
            char.apply_parameter_changed(id, value);
        }
        // 0x00be: the costs, SP_UPOW 247 to SP_UCRT 252
        char.apply_parameter_changed(247, 1);
        char.apply_parameter_changed(252, 3);
        // 0x00bc: a raise answers with the new value
        char.apply_parameter_changed(220, 9);

        let t = &char.traits;
        assert_eq!(
            t.stats[0],
            TraitStat {
                base: 5,
                bonus: 2,
                cost: 1
            }
        );
        assert_eq!(t.stats[1].base, 9);
        assert_eq!(
            t.stats[5],
            TraitStat {
                base: 3,
                bonus: 0,
                cost: 3
            }
        );
        assert_eq!((t.p_atk, t.c_rate), (7, 4));
        assert_eq!((t.points, t.ap, t.max_ap), (12, 40, 200));
        // the old stats are untouched
        assert_eq!((char.str, char.luk, char.str_cost), (0, 0, 0));
    }
}
