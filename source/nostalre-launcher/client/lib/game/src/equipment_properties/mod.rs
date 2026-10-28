//! Equipment Attributes: the player's own `EquipmentProperties.lub` run in a Lua 5.1 VM, as kRO's client runs it
//! (`EquipmentProperties::CEquipmentPropertiesMgr`), adding up what the worn gear gives into the Order table's
//! lines. `docs/nostalre-equipment-attributes.md` is the design; `docs/nostalre-kro-exe-equip-window.md`,
//! "Equipment Attributes", what kRO's exe does.
mod jobs;
mod natives;
pub mod order;
pub mod property;

use crate::item::ItemOption;
use mlua::{Function, Lua, Table};
use order::{Op, Order};
use property::{Container, Prop, SepNames};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

/// The four files, in the order kRO's 0x67ab20 loads them; a path under `data/` of the archive, and where the
/// pinned install keeps its own (English) copy, which kRO's client reads first
const FILES: [(&str, Option<&str>); 4] = [
    (
        "data/luafiles514/lua files/equipmentproperties/equipmentproperties.lub",
        None,
    ),
    ("data/luafiles514/lua files/datainfo/enumvar.lub", None),
    (
        "data/luafiles514/lua files/datainfo/addrandomoptionnametable.lub",
        Some("data/luafiles514/lua files/datainfo/addrandomoptionnametable.lub"),
    ),
    (
        "data/luafiles514/lua files/equipmentproperties/equipmentpropertiesorder.lub",
        Some("data/luafiles514/lua files/EquipmentProperties/EquipmentPropertiesOrder.lub"),
    ),
];

/// The ExtParam ids the refine bonus records under that are not among the `Stat` names (0x65b120)
const EXT_RES: i32 = 244;
const EXT_MRES: i32 = 245;
const EXT_P_ATK: i32 = 242;
const EXT_S_MATK: i32 = 243;

/// The `EnumVAR` names 0x67db90 asks for (by their djb2 hashes), the stats a `Stat` array gives
#[derive(Clone, Copy)]
enum StatName {
    Def,
    Mdef,
    Atk,
    Matk,
    MaxHp,
    MaxSp,
    Str,
    Agi,
    Vit,
    Int,
    Dex,
    Luk,
    Pow,
    Sta,
    Wis,
    Spl,
    Con,
    Crt,
}

impl StatName {
    const ALL: [(StatName, &'static str); 18] = [
        (StatName::Def, "VAR_ITEMDEFPOWER"),
        (StatName::Mdef, "VAR_MDEFPOWER"),
        (StatName::Atk, "VAR_ATTPOWER"),
        (StatName::Matk, "VAR_ATTMPOWER"),
        (StatName::MaxHp, "VAR_MAXHPAMOUNT"),
        (StatName::MaxSp, "VAR_MAXSPAMOUNT"),
        (StatName::Str, "VAR_STRAMOUNT"),
        (StatName::Agi, "VAR_AGIAMOUNT"),
        (StatName::Vit, "VAR_VITAMOUNT"),
        (StatName::Int, "VAR_INTAMOUNT"),
        (StatName::Dex, "VAR_DEXAMOUNT"),
        (StatName::Luk, "VAR_LUKAMOUNT"),
        (StatName::Pow, "VAR_POWAMOUNT"),
        (StatName::Sta, "VAR_STAAMOUNT"),
        (StatName::Wis, "VAR_WISAMOUNT"),
        (StatName::Spl, "VAR_SPLAMOUNT"),
        (StatName::Con, "VAR_CONAMOUNT"),
        (StatName::Crt, "VAR_CRTAMOUNT"),
    ];
}

/// What one index of a `Stat` array holds (0x6817e0)
#[derive(Clone, Copy)]
enum StatSlot {
    Stat(StatName),
    Key(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Armor,
    MeleeWeapon,
    RangedWeapon,
    Ammo,
    Special,
}

impl Kind {
    fn from_type(name: &str) -> Option<Self> {
        match name {
            "armor" => Some(Kind::Armor),
            "Mweapon" => Some(Kind::MeleeWeapon),
            "Rweapon" => Some(Kind::RangedWeapon),
            "ammo" => Some(Kind::Ammo),
            "special" => Some(Kind::Special),
            _ => None,
        }
    }

    fn stat_slots(self) -> &'static [StatSlot] {
        use StatName::*;
        use StatSlot::{Key, Stat};
        match self {
            Kind::Armor => &[
                Stat(Def),
                Stat(Str),
                Stat(Int),
                Stat(Vit),
                Stat(Dex),
                Stat(Agi),
                Stat(Luk),
                Stat(MaxHp),
                Stat(MaxSp),
                Stat(Mdef),
                Key(2),
                Stat(Pow),
                Stat(Spl),
                Stat(Sta),
                Stat(Wis),
                Stat(Con),
                Stat(Crt),
            ],
            Kind::MeleeWeapon => &[
                Key(0),
                Key(1),
                Stat(Atk),
                Stat(Matk),
                Stat(Str),
                Stat(Int),
                Stat(Vit),
                Stat(Dex),
                Stat(Agi),
                Stat(Luk),
                Key(2),
                Stat(Pow),
                Stat(Spl),
                Stat(Sta),
                Stat(Wis),
                Stat(Con),
                Stat(Crt),
            ],
            Kind::RangedWeapon => &[
                Key(1),
                Stat(Atk),
                Stat(Str),
                Stat(Int),
                Stat(Vit),
                Stat(Dex),
                Stat(Agi),
                Stat(Luk),
                Key(2),
                Stat(Pow),
                Stat(Spl),
                Stat(Sta),
                Stat(Wis),
                Stat(Con),
                Stat(Crt),
            ],
            Kind::Ammo => &[Key(0), Stat(Atk)],
            Kind::Special => &[],
        }
    }
}

#[derive(Debug, Default)]
struct ItemData {
    kind: Option<Kind>,
    /// The non-zero stats as (ExtParam id, value)
    stats: Vec<(i32, i32)>,
    /// Key 0 (element), 1 (weapon class), 2 (level)
    keys: [i32; 3],
    combos: Vec<u32>,
}

/// One worn item as the getters see it
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WornItem {
    pub item_id: u32,
    pub refine: u8,
    pub grade: u8,
    pub cards: [u32; 4],
    /// The cards are not cards: a forged or crafted item's slots name its maker
    pub cards_are_maker: bool,
    pub options: [ItemOption; 5],
    pub wear: u32,
}

/// The character as the scripts read it
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub worn: Vec<WornItem>,
    pub job: u16,
    pub base_level: i32,
    pub job_level: i32,
    pub sp: i32,
    pub max_sp: i32,
    /// STR to LUK without equipment
    pub base_stats: [i32; 6],
    /// POW to CRT without equipment
    pub trait_stats: [i32; 6],
    pub skills: HashMap<i32, i32>,
    /// The map's name without `.rsw`
    pub map: String,
    /// The hatched pet's egg, which kRO counts as an Expansion: only its `OnStartEquip` runs (0x65afe0)
    pub pet_egg: Option<u32>,
    /// The pet's intimacy as last received (0x01a2, 0x01a4 type 1), which `GetPetRelationship` reads
    pub pet_intimacy: i16,
}

/// kRO's pet egg test (0xd8ebe0): the item packets carry an egg's type as armor, so the id decides
pub fn is_pet_egg(item_id: u32) -> bool {
    (9001..=9499).contains(&item_id)
}

/// `GetPetRelationship()`'s level for an intimacy: kRO's 0xd82050, the pet's intimacy band
pub fn pet_relationship(intimacy: i16) -> i32 {
    crate::pet::IntimacyState::from_value(intimacy).index() as i32
}

impl Snapshot {
    /// `get(id)` (0x68bf30): the values its data asks for; the rest kRO answers from state nostalRE has no
    /// counterpart for, or answers 0 itself
    fn get(&self, id: i32) -> i32 {
        match id {
            7 => self.sp,
            8 => self.max_sp,
            11 => self.base_level,
            55 => self.job_level,
            32..=37 => self.base_stats[(id - 32) as usize],
            255..=260 => self.trait_stats[(id - 255) as usize],
            _ => 0,
        }
    }
}

/// kRO's location index for a wear mask (0x67a8a0)
pub fn location_of(mask: u32) -> i32 {
    match mask {
        0x1 | 0x101 | 0x201 | 0x301 => 1,
        0x2 | 0x22 => 4,
        0x4 => 5,
        0x8 => 7,
        0x10 => 2,
        0x20 => 3,
        0x40 => 6,
        0x80 => 8,
        0x100 | 0x300 => 10,
        0x200 => 11,
        0x400 | 0xc00 | 0x1400 | 0x1c00 => 20,
        0x800 | 0x1800 => 21,
        0x1000 => 22,
        0x2000 => 23,
        0x10000 => 30,
        0x20000 => 31,
        0x40000 => 32,
        0x80000 => 33,
        0x100000 => 34,
        0x200000 => 35,
        _ => 0,
    }
}

/// The one mask a location index reads (0x679780)
fn mask_of(location: i32) -> u32 {
    match location {
        1 => 0x1,
        2 => 0x10,
        3 => 0x20,
        4 => 0x2,
        5 => 0x4,
        6 => 0x40,
        7 => 0x8,
        8 => 0x80,
        10 => 0x100,
        11 => 0x200,
        20 => 0x400,
        21 => 0x800,
        22 => 0x1000,
        23 => 0x2000,
        30 => 0x10000,
        31 => 0x20000,
        32 => 0x40000,
        33 => 0x80000,
        34 => 0x100000,
        35 => 0x200000,
        _ => 0,
    }
}

/// Costume and shadow gear get no refine or grade bonus (0xd8e630)
fn is_costume_or_shadow(mask: u32) -> bool {
    mask & 0x3f3c00 != 0
}

/// What the natives work on during one run
struct RunState {
    order: Rc<Order>,
    snapshot: Snapshot,
    /// Each worn item's static keys, by location
    keys: HashMap<i32, [i32; 3]>,
    /// `GetLocation()`: the item's, a card's parent's, 0 for a set
    location: i32,
    container: Container,
    temp: HashMap<i32, i32>,
}

impl RunState {
    fn worn_at(&self, location: i32) -> Option<&WornItem> {
        let mask = mask_of(location);
        (mask != 0)
            .then(|| self.snapshot.worn.iter().find(|w| w.wear & mask != 0))
            .flatten()
    }

    fn static_key(&self, location: i32, key: usize) -> i32 {
        self.keys.get(&location).map_or(0, |keys| keys[key])
    }

    /// One recorder call (0x65a710): its key names the Order line; by the line's second name it counts negated
    fn record(&mut self, key: &str, name: &str, extra: Option<i32>, value: Option<i32>) {
        let Some(index) = self.order.line_by_key(key) else {
            return;
        };
        let line = &self.order.lines[index];
        let sign = if line.plus == name { 1 } else { -1 };
        let prop = match (value, extra) {
            (None, None) if line.op == Op::One && line.val_arg == 0 => Prop::Flag,
            (None, _) => return,
            (Some(value), Some(extra)) => {
                Prop::TwoDim(BTreeMap::from([(extra, value.wrapping_mul(sign))]))
            }
            (Some(value), None) if line.op != Op::One => Prop::OneDim(value.wrapping_mul(sign)),
            (Some(_), None) => Prop::Flag,
        };
        let order = Rc::clone(&self.order);
        self.container.insert(index, prop, &order.lines);
    }

    fn record_ext_param(&mut self, ext: i32, value: i32) {
        let key = order::record_key("AddExtParam", [(1, 0), (2, i64::from(ext))].into_iter());
        self.record(&key, "AddExtParam", None, Some(value));
    }

    /// A random option (0x6591f0): through its `rndOpt` line, its value times the line's DIV
    fn record_option(&mut self, option: &ItemOption, id: u16, value: i32) {
        let Some(index) = self.order.line_by_option(i64::from(id)) else {
            return;
        };
        let line = &self.order.lines[index];
        if line.plus == "SetWeaponAttr" {
            return;
        }
        let value = value.wrapping_mul(line.div.unwrap_or(1));
        let prop = if line.sep.is_some() {
            Prop::TwoDim(BTreeMap::from([(i32::from(option.param), value)]))
        } else if line.op != Op::One {
            Prop::OneDim(value)
        } else {
            Prop::Flag
        };
        let order = Rc::clone(&self.order);
        self.container.insert(index, prop, &order.lines);
    }
}

/// `ATTR_TOLERACE_ALLBUTNOTHING` also takes its value off Neutral's line (0x6591f0)
const OPTION_ALL_BUT_NEUTRAL: u16 = 35;
const OPTION_NEUTRAL: u16 = 25;

pub struct EquipmentProperties {
    lua: Lua,
    order: Rc<Order>,
    /// Each `StatName`'s ExtParam id, from `EnumVAR[name][2]`
    ext_ids: [i32; 18],
    items: HashMap<u32, ItemData>,
    combos: HashMap<u32, Vec<u32>>,
    /// `RefiningBonus[table][level][refine]`
    refining: HashMap<(String, i32), Vec<f64>>,
    /// `GradeBonus.NORMAL_WEAPON[grade]`: the refine ATK and MATK multiples
    weapon_grade: HashMap<u8, (f64, f64)>,
    skill_groups: HashMap<i32, Vec<i32>>,
    ad_skill: HashMap<i32, i32>,
    item_table: Table,
    combo_table: Table,
}

/// The `GRADE_*` names of `GradeBonus`, by grade (0x6800c0)
const GRADE_NAMES: [&str; 8] = [
    "GRADE_NORMAL",
    "GRADE_MAGIC",
    "GRADE_RARE",
    "GRADE_EPIC",
    "GRADE_UNIQUE",
    "GRADE_LEGEND",
    "GRADE_ANCIENT",
    "GRADE_AWAKEN",
];

fn load_file(lua: &Lua, name: &str, data: &[u8]) -> mlua::Result<()> {
    if nostalre_formats::lub::is_compiled_chunk(data) {
        let chunk = nostalre_formats::lub::to_host_chunk(data)
            .map_err(|e| mlua::Error::runtime(format!("{name}: {e}")))?;
        lua.load(&chunk[..])
            .set_name(name)
            .set_mode(mlua::ChunkMode::Binary)
            .exec()
    } else {
        lua.load(data)
            .set_name(name)
            .set_mode(mlua::ChunkMode::Text)
            .exec()
    }
}

fn int_list(table: &Table) -> mlua::Result<Vec<i32>> {
    table
        .sequence_values::<f64>()
        .map(|v| v.map(|v| v as i32))
        .collect()
}

impl EquipmentProperties {
    /// The four files from the install, else the archive; none when one is missing or fails, as kRO's window
    /// then has nothing to show
    pub fn load(
        grf: &nostalre_formats::grf::GrfArchive,
        install_dir: Option<&std::path::Path>,
    ) -> Option<Self> {
        let started = std::time::Instant::now();
        let result = Self::load_files(|(archived, installed)| {
            installed
                .zip(install_dir)
                .and_then(|(file, dir)| std::fs::read(dir.join(file)).ok())
                .or_else(|| grf.read_file(archived).ok())
        });
        match result {
            Ok(loaded) => {
                tracing::info!(
                    items = loaded.items.len(),
                    lines = loaded.order.lines.len(),
                    ms = started.elapsed().as_millis() as u64,
                    "equipment properties"
                );
                Some(loaded)
            }
            Err(e) => {
                tracing::warn!("equipment properties not loaded: {e}");
                None
            }
        }
    }

    fn load_files(read: impl Fn((&str, Option<&str>)) -> Option<Vec<u8>>) -> mlua::Result<Self> {
        // The chunks are the player's install, as trusted as its archive; binary chunks need the unsafe state
        let lua = unsafe {
            Lua::unsafe_new_with(
                mlua::StdLib::MATH | mlua::StdLib::STRING | mlua::StdLib::TABLE,
                mlua::LuaOptions::new(),
            )
        };
        natives::register(&lua)?;
        for file in FILES {
            let data =
                read(file).ok_or_else(|| mlua::Error::runtime(format!("{} missing", file.0)))?;
            load_file(&lua, file.0, &data)?;
        }
        Self::read_tables(lua)
    }

    fn read_tables(lua: Lua) -> mlua::Result<Self> {
        let globals = lua.globals();
        let enum_var: Table = globals.get("EnumVAR")?;
        let mut ext_ids = [0; StatName::ALL.len()];
        for (stat, name) in StatName::ALL {
            let entry: Table = enum_var.get(name)?;
            ext_ids[stat as usize] = entry.get::<f64>(2)? as i32;
        }

        let item_table: Table = globals.get("Item")?;
        let mut items = HashMap::new();
        for pair in item_table.pairs::<f64, Table>() {
            let (id, entry) = pair?;
            let mut data = ItemData {
                kind: entry
                    .get::<Option<String>>("Type")?
                    .as_deref()
                    .and_then(Kind::from_type),
                ..Default::default()
            };
            if let Some(kind) = data.kind {
                let stats = match entry.get::<Option<Table>>("Stat")? {
                    Some(stat) => int_list(&stat)?,
                    None => Vec::new(),
                };
                let slots = kind.stat_slots();
                if stats.len() != slots.len() {
                    tracing::warn!(id, count = stats.len(), "Item has invalid 'Stat' table");
                    continue;
                }
                for (slot, value) in slots.iter().zip(stats) {
                    match *slot {
                        StatSlot::Key(k) => data.keys[k] = value,
                        StatSlot::Stat(_) if value == 0 => {}
                        StatSlot::Stat(name) => data.stats.push((ext_ids[name as usize], value)),
                    }
                }
            }
            if let Some(combos) = entry.get::<Option<Table>>("Combiitem")? {
                data.combos = int_list(&combos)?.into_iter().map(|c| c as u32).collect();
            }
            items.insert(id as u32, data);
        }

        let combo_table: Table = globals.get("Combiitem")?;
        let mut combos = HashMap::new();
        for pair in combo_table.pairs::<f64, Table>() {
            let (id, entry) = pair?;
            let list = int_list(&entry.get::<Table>("Item")?)?;
            combos.insert(id as u32, list.into_iter().map(|i| i as u32).collect());
        }

        let mut refining = HashMap::new();
        for pair in globals
            .get::<Table>("RefiningBonus")?
            .pairs::<String, Table>()
        {
            let (name, levels) = pair?;
            for level in levels.pairs::<f64, Table>() {
                let (level, values) = level?;
                let values = values
                    .sequence_values::<f64>()
                    .collect::<mlua::Result<_>>()?;
                refining.insert((name.clone(), level as i32), values);
            }
        }

        let mut weapon_grade = HashMap::new();
        if let Some(weapon) = globals
            .get::<Table>("GradeBonus")?
            .get::<Option<Table>>("NORMAL_WEAPON")?
        {
            for (grade, name) in GRADE_NAMES.iter().enumerate() {
                if let Some(bonus) = weapon.get::<Option<Table>>(*name)? {
                    let atk = bonus
                        .get::<Option<f64>>("REFINING_WEAPON_ATK_MULTIPLE")?
                        .unwrap_or(0.0);
                    let matk = bonus
                        .get::<Option<f64>>("REFINING_WEAPON_MATK_MULTIPLE")?
                        .unwrap_or(0.0);
                    weapon_grade.insert(grade as u8, (atk, matk));
                }
            }
        }

        let mut skill_groups = HashMap::new();
        for pair in globals.get::<Table>("SkillGroup")?.pairs::<f64, Table>() {
            let (id, skills) = pair?;
            skill_groups.insert(id as i32, int_list(&skills)?);
        }
        let mut ad_skill = HashMap::new();
        if let Some(table) = globals.get::<Option<Table>>("ADSkill")? {
            for pair in table.pairs::<f64, f64>() {
                let (from, to) = pair?;
                ad_skill.insert(from as i32, to as i32);
            }
        }

        let order = Order::read(&globals.get::<Table>("EquipmentPropertiesOrder")?)?;
        drop(globals);
        Ok(Self {
            lua,
            order: Rc::new(order),
            ext_ids,
            items,
            combos,
            refining,
            weapon_grade,
            skill_groups,
            ad_skill,
            item_table,
            combo_table,
        })
    }

    pub fn order(&self) -> &Order {
        &self.order
    }

    fn refining_bonus(&self, table: &str, level: i32, refine: u8) -> f64 {
        self.refining
            .get(&(table.to_string(), level))
            .and_then(|values| values.get(usize::from(refine).checked_sub(1)?))
            .copied()
            .unwrap_or(0.0)
    }

    /// `OnStartEquip` of an `Item` or `Combiitem` entry, in a protected call: a script error (kRO's data calls
    /// `AddDamage_HIT`, which no client registers) keeps what it recorded before it (0x65aa90)
    fn call_start(&self, table: &Table, id: u32) {
        let Ok(Some(entry)) = table.get::<Option<Table>>(id) else {
            return;
        };
        if let Ok(start) = entry.get::<Function>("OnStartEquip")
            && let Err(e) = start.call::<()>(())
        {
            tracing::debug!(id, "OnStartEquip: {e}");
        }
    }

    fn with_state<R>(&self, f: impl FnOnce(&mut RunState) -> R) -> R {
        let mut state = self.lua.app_data_mut::<RunState>().expect("run state");
        f(&mut state)
    }

    /// Start a new container for the item at `location`
    fn begin(&self, location: i32) {
        self.with_state(|s| {
            s.container = Container::default();
            s.location = location;
            s.temp.clear();
        });
    }

    fn take_container(&self) -> Container {
        self.with_state(|s| std::mem::take(&mut s.container))
    }

    /// One worn item's run (0x65b120): its `Stat`, the refine and grade bonus, its script, its random options, then
    /// its cards
    fn run_worn(&self, location: i32, worn: &WornItem) -> Container {
        self.begin(location);
        let data = self.items.get(&worn.item_id);
        if let Some(data) = data {
            self.with_state(|s| {
                for &(ext, value) in &data.stats {
                    s.record_ext_param(ext, value);
                }
            });
            if !is_costume_or_shadow(worn.wear) && worn.refine > 0 {
                self.record_refine_bonus(data, worn);
            }
        }
        self.call_start(&self.item_table, worn.item_id);
        self.with_state(|s| {
            for option in worn.options.iter().filter(|o| o.id != 0) {
                s.record_option(option, option.id, i32::from(option.value));
                if option.id == OPTION_ALL_BUT_NEUTRAL {
                    s.record_option(option, OPTION_NEUTRAL, -i32::from(option.value));
                }
            }
        });
        let mut container = self.take_container();
        for &card in self.cards(worn) {
            self.begin(location);
            self.call_start(&self.item_table, card);
            container.merge(&self.take_container(), &self.order.lines);
        }
        container
    }

    /// The card slots kRO runs: every non-zero one; a forged or crafted item's slots hold its maker's id, which
    /// kRO's client would run as cards too (its gap), so nostalRE runs none of them
    fn cards<'a>(&self, worn: &'a WornItem) -> &'a [u32] {
        if worn.cards_are_maker {
            return &[];
        }
        let used = worn
            .cards
            .iter()
            .rposition(|&c| c != 0)
            .map_or(0, |i| i + 1);
        &worn.cards[..used]
    }

    fn record_refine_bonus(&self, data: &ItemData, worn: &WornItem) {
        let level = data.keys[2];
        let refine = worn.refine;
        let stat = |name: StatName| self.ext_ids[name as usize];
        match data.kind {
            Some(Kind::Armor) => {
                let def = self.refining_bonus("ARMOR_DEF", level, refine) as i32;
                let res = self.refining_bonus("ARMOR_RES", level, refine) as i32;
                self.with_state(|s| {
                    s.record_ext_param(stat(StatName::Def), def);
                    s.record_ext_param(EXT_RES, res);
                    s.record_ext_param(EXT_MRES, res);
                });
            }
            Some(Kind::MeleeWeapon | Kind::RangedWeapon) => {
                let v = self.refining_bonus("WEAPON_ATK", level, refine);
                let (atk_mult, matk_mult) = self
                    .weapon_grade
                    .get(&worn.grade)
                    .copied()
                    .unwrap_or_default();
                let atk = (v * atk_mult / 10.0 + v) as i32;
                let matk = (v * matk_mult / 10.0 + v) as i32;
                let adv = self.refining_bonus("WEAPON_ADV_ATK", level, refine) as i32;
                let melee = data.kind == Some(Kind::MeleeWeapon);
                self.with_state(|s| {
                    s.record_ext_param(stat(StatName::Atk), atk);
                    if melee {
                        s.record_ext_param(stat(StatName::Matk), matk);
                    }
                    s.record_ext_param(EXT_P_ATK, adv);
                    s.record_ext_param(EXT_S_MATK, adv);
                });
            }
            _ => {}
        }
    }

    /// The sets the worn items and their cards complete (0x678c50): each listed id worn at least once
    fn active_combos(&self, snapshot: &Snapshot) -> BTreeSet<u32> {
        let mut worn_ids = BTreeSet::new();
        for worn in &snapshot.worn {
            worn_ids.insert(worn.item_id);
            worn_ids.extend(self.cards(worn).iter().copied().filter(|&c| c != 0));
        }
        // The pet's egg counts toward sets too (0x678c50 from 0x683cc0)
        worn_ids.extend(snapshot.pet_egg);
        worn_ids
            .iter()
            .filter_map(|id| self.items.get(id))
            .flat_map(|data| data.combos.iter().copied())
            .filter(|combo| {
                self.combos
                    .get(combo)
                    .is_some_and(|ids| ids.iter().all(|id| worn_ids.contains(id)))
            })
            .collect()
    }

    /// Everything the worn gear gives, merged as kRO's window merges it (0x67a2d0)
    pub fn compute(&self, snapshot: &Snapshot) -> Container {
        let mut by_location: BTreeMap<i32, &WornItem> = BTreeMap::new();
        for worn in &snapshot.worn {
            by_location.insert(location_of(worn.wear), worn);
        }
        let keys = by_location
            .iter()
            .map(|(&loc, worn)| {
                let keys = self.items.get(&worn.item_id).map_or([0; 3], |d| d.keys);
                (loc, keys)
            })
            .collect();
        self.lua.set_app_data(RunState {
            order: Rc::clone(&self.order),
            snapshot: snapshot.clone(),
            keys,
            location: 0,
            container: Container::default(),
            temp: HashMap::new(),
        });
        let mut total = Container::default();
        for (&location, worn) in &by_location {
            let mut container = self.run_worn(location, worn);
            container.uniq_pass(&self.order.lines, true);
            total.merge(&container, &self.order.lines);
        }
        // Then the Expansions (+0x2c), here the pet, then the sets (+0x34), as 0x67a2d0 merges them
        if let Some(egg) = snapshot.pet_egg {
            self.begin(0);
            self.call_start(&self.item_table, egg);
            total.merge(&self.take_container(), &self.order.lines);
        }
        for combo in self.active_combos(snapshot) {
            self.begin(0);
            self.call_start(&self.combo_table, combo);
            total.merge(&self.take_container(), &self.order.lines);
        }
        total.uniq_pass(&self.order.lines, false);
        self.lua.remove_app_data::<RunState>();
        total
    }

    /// A tab's lines: Total (0) every category's, tab k only category k's, in the Order table's order
    pub fn tab_lines(&self, total: &Container, tab: usize, names: &dyn SkillNames) -> Vec<String> {
        let sep = Names {
            names,
            properties: self,
        };
        let mut out = Vec::new();
        for (&index, prop) in &total.props {
            let line = &self.order.lines[index];
            if tab != 0 && line.category + 1 != tab {
                continue;
            }
            let text = property::text(line, prop, &sep);
            out.extend(
                text.split('\n')
                    .filter(|t| !t.is_empty())
                    .map(str::to_string),
            );
        }
        out
    }
}

/// The skill names the window's 2-dim lines need
pub trait SkillNames {
    fn skill_name(&self, skill: i32) -> String;
}

struct Names<'a> {
    names: &'a dyn SkillNames,
    properties: &'a EquipmentProperties,
}

impl SepNames for Names<'_> {
    fn skill_name(&self, skill: i32) -> String {
        self.names.skill_name(skill)
    }

    fn add_damage_skill(&self, key: i32) -> i32 {
        self.properties.ad_skill.get(&key).copied().unwrap_or(key)
    }

    fn skill_group(&self, key: i32) -> Option<&[i32]> {
        self.properties.skill_groups.get(&key).map(Vec::as_slice)
    }
}

/// One window line: the value, the label, and whether kRO marks it negative (`{~}`; 0x9c7b00)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowLine {
    pub value: String,
    pub label: String,
    pub negative: bool,
}

pub fn window_line(text: &str) -> WindowLine {
    let negative = text.contains(property::NEGATIVE_MARK);
    let text = text.replace(property::NEGATIVE_MARK, "");
    match text.split_once('#') {
        Some((value, label)) => WindowLine {
            value: value.to_string(),
            label: label.to_string(),
            negative,
        },
        None => WindowLine {
            value: String::new(),
            label: text,
            negative,
        },
    }
}
