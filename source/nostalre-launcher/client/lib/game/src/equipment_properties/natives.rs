//! The functions kRO's client gives `EquipmentProperties.lub` (registered by 0x67ab20): 105 recorders, all one
//! function over (name, argument count, value argument, extra argument) (0x678980), and 18 getters reading the
//! character (`docs/nostalre-kro-exe-equip-window.md`, "Equipment Attributes" 2).
use super::RunState;
use super::order::record_key;
use mlua::{Lua, Value, Variadic};

/// One recorder: its name, how many arguments it reads, which holds the value and which the 2-dim key
/// (1-based, 0 for none). From the thunks 0x66a1f0..0x66a870 and their bodies
pub struct Recorder {
    pub name: &'static str,
    pub count: usize,
    pub value_arg: usize,
    pub extra_arg: usize,
}

const fn r(name: &'static str, count: usize, value_arg: usize, extra_arg: usize) -> Recorder {
    Recorder {
        name,
        count,
        value_arg,
        extra_arg,
    }
}

pub const RECORDERS: [Recorder; 105] = [
    r("AddAttrTolerace", 2, 2, 0),
    r("addattrtolerace", 3, 2, 0),
    r("AddBowAttackDamage", 2, 2, 0),
    r("AddCRIPercent_Race", 2, 2, 0),
    r("AddDamage_CRI", 2, 2, 0),
    r("AddDamage_Property", 3, 3, 0),
    r("AddDamage_SKID", 3, 3, 2),
    r("AddDamage_Size", 3, 3, 0),
    r("AddEXPPercent_KillRace", 2, 2, 0),
    r("AddExtParam", 3, 3, 0),
    r("AddGuideAttack", 1, 1, 0),
    r("AddHPdrain", 2, 1, 2),
    r("AddHealModifyPercent", 1, 1, 0),
    r("AddHealValue", 1, 1, 0),
    r("AddIgnore_MRES_RacePercent", 2, 2, 0),
    r("AddIgnore_RES_RacePercent", 2, 2, 0),
    r("AddMDamage_Property", 3, 3, 0),
    r("AddMDamage_Size", 3, 3, 0),
    r("AddMdamage_Class", 2, 2, 0),
    r("AddMdamage_Race", 2, 2, 0),
    r("AddMeleeAttackDamage", 2, 2, 0),
    r("AddMeleeAttackReflect", 1, 1, 0),
    r("AddNeverknockback", 1, 1, 0),
    r("AddRaceTolerace", 2, 2, 0),
    r("AddRangeAttackDamage", 2, 2, 0),
    r("AddReceiveItem_Equip", 1, 1, 0),
    r("AddReflectMagic", 1, 1, 0),
    r("AddReflectTolerace", 1, 1, 0),
    r("AddSFCTEquipAmount", 2, 1, 2),
    r("AddSFCTEquipPermill", 2, 1, 2),
    r("AddSPconsumption", 1, 1, 0),
    r("addspconsumption", 2, 1, 2),
    r("AddSPdrain", 2, 1, 2),
    r("AddSkillDelay", 2, 2, 1),
    r("AddSkillMDamage", 2, 2, 0),
    r("AddSkillSP", 2, 2, 1),
    r("AddSpecificSpellCastTime", 2, 2, 1),
    r("AddSpellCastTime", 1, 1, 0),
    r("AddSpellDelay", 1, 1, 0),
    r("BodyAttribute", 1, 0, 0),
    r("Clairvoyance", 1, 1, 0),
    r("ClassAddDamage", 3, 3, 0),
    r("ClassSubDamage", 3, 3, 0),
    r("Condition", 3, 0, 0),
    r("DisableSkill", 2, 2, 1),
    r("EnableSkill", 2, 2, 1),
    r("IndestructibleArmor", 1, 1, 0),
    r("IndestructibleWeapon", 1, 1, 0),
    r("Magicimmune", 1, 1, 0),
    r("NoDispell", 1, 1, 0),
    r("NoJamstone", 1, 1, 0),
    r("NoMadogearfuel", 1, 1, 0),
    r("PerfectDamage", 1, 1, 0),
    r("RaceAddDamage", 2, 2, 0),
    r("RaceAddDamageSelf", 2, 2, 0),
    r("RaceSubDamage", 2, 2, 0),
    r("RaceSubDamageSelf", 2, 2, 0),
    r("Reincarnation", 1, 1, 0),
    r("SetIgnoreDEFClass", 1, 0, 0),
    r("SetIgnoreDEFRace", 1, 0, 0),
    r("SetIgnoreDefClass_Percent", 2, 2, 0),
    r("SetIgnoreDefRace_Percent", 2, 2, 0),
    r("SetIgnoreMdefClass", 2, 2, 0),
    r("SetIgnoreMdefRace", 2, 2, 0),
    r("SetInvestigate", 0, 0, 0),
    r("SplashAttack", 1, 1, 0),
    r("SubAttrTolerace", 2, 2, 0),
    r("subattrtolerace", 3, 2, 0),
    r("SubBowAttackDamage", 2, 2, 0),
    r("SubCRIPercent_Race", 2, 2, 0),
    r("SubDamage_CRI", 2, 2, 0),
    r("SubDamage_Property", 3, 3, 0),
    r("SubDamage_SKID", 3, 3, 2),
    r("SubDamage_Size", 3, 3, 0),
    r("SubEXPPercent_KillRace", 2, 2, 0),
    r("SubExtParam", 3, 3, 0),
    r("SubGuideAttack", 1, 1, 0),
    r("SubHPdrain", 2, 1, 2),
    r("SubHealModifyPercent", 1, 1, 0),
    r("SubHealValue", 1, 1, 0),
    r("SubIgnore_MRES_RacePercent", 2, 2, 0),
    r("SubIgnore_RES_RacePercent", 2, 2, 0),
    r("SubMDamage_Property", 3, 3, 0),
    r("SubMDamage_Size", 3, 3, 0),
    r("SubMdamage_Class", 2, 2, 0),
    r("SubMdamage_Race", 2, 2, 0),
    r("SubMeleeAttackDamage", 2, 2, 0),
    r("SubMeleeAttackReflect", 1, 1, 0),
    r("SubNeverknockback", 0, 0, 0),
    r("SubRaceTolerace", 2, 2, 0),
    r("SubRangeAttackDamage", 2, 2, 0),
    r("SubReceiveItem_Equip", 1, 1, 0),
    r("SubReflectMagic", 1, 1, 0),
    r("SubReflectTolerace", 1, 1, 0),
    r("SubSFCTEquipAmount", 2, 1, 2),
    r("SubSFCTEquipPermill", 2, 1, 2),
    r("SubSPconsumption", 1, 1, 0),
    r("subspconsumption", 2, 1, 2),
    r("SubSPdrain", 2, 1, 2),
    r("SubSkillDelay", 2, 2, 1),
    r("SubSkillMDamage", 2, 2, 0),
    r("SubSkillSP", 2, 2, 1),
    r("SubSpecificSpellCastTime", 2, 2, 1),
    r("SubSpellCastTime", 1, 1, 0),
    r("SubSpellDelay", 1, 1, 0),
];

/// A Lua number as kRO's natives read it: cut to an integer, an error for anything else (0x678980's "arg{0} is
/// must number")
fn int_arg(args: &[Value], i: usize) -> mlua::Result<i32> {
    match args.get(i) {
        Some(Value::Integer(n)) => Ok(*n as i32),
        Some(Value::Number(n)) => Ok(*n as i32),
        _ => Err(mlua::Error::runtime(format!("arg{} is must number", i + 1))),
    }
}

fn record(lua: &Lua, spec: &Recorder, args: &[Value]) -> mlua::Result<()> {
    let mut values = [0i32; 3];
    for (i, value) in values.iter_mut().enumerate().take(spec.count) {
        *value = int_arg(args, i)?;
    }
    let arg = |n: usize| (n > 0).then(|| values[n - 1]);
    let key = record_key(
        spec.name,
        (1..=spec.count)
            .filter(|&i| i != spec.value_arg && i != spec.extra_arg)
            .map(|i| (i as i64, i64::from(values[i - 1]))),
    );
    let mut state = lua
        .app_data_mut::<RunState>()
        .ok_or_else(|| mlua::Error::runtime("no run"))?;
    state.record(&key, spec.name, arg(spec.extra_arg), arg(spec.value_arg));
    Ok(())
}

fn with_state<R>(lua: &Lua, f: impl FnOnce(&mut RunState) -> R) -> mlua::Result<R> {
    let mut state = lua
        .app_data_mut::<RunState>()
        .ok_or_else(|| mlua::Error::runtime("no run"))?;
    Ok(f(&mut state))
}

fn getter(
    lua: &Lua,
    name: &str,
    f: impl Fn(&mut RunState, &[Value]) -> mlua::Result<i32> + 'static,
) -> mlua::Result<()> {
    let function = lua.create_function(move |lua, args: Variadic<Value>| {
        let mut state = lua
            .app_data_mut::<RunState>()
            .ok_or_else(|| mlua::Error::runtime("no run"))?;
        f(&mut state, &args)
    })?;
    lua.globals().set(name, function)
}

pub fn register(lua: &Lua) -> mlua::Result<()> {
    let globals = lua.globals();
    for spec in &RECORDERS {
        let function =
            lua.create_function(move |lua, args: Variadic<Value>| record(lua, spec, &args))?;
        globals.set(spec.name, function)?;
    }
    getter(lua, "GetLocation", |s, _| Ok(s.location))?;
    getter(lua, "GetRefineLevel", |s, a| {
        Ok(s.worn_at(int_arg(a, 0)?).map_or(0, |w| i32::from(w.refine)))
    })?;
    getter(lua, "GetEquipGradeLevel", |s, a| {
        Ok(s.worn_at(int_arg(a, 0)?).map_or(0, |w| i32::from(w.grade)))
    })?;
    getter(lua, "GetItemIDLocation", |s, a| {
        Ok(s.worn_at(int_arg(a, 0)?).map_or(0, |w| w.item_id as i32))
    })?;
    getter(lua, "GetEquipArmorLv", |s, a| {
        Ok(s.static_key(int_arg(a, 0)?, 2))
    })?;
    getter(lua, "GetEquipWeaponLv", |s, a| {
        Ok(s.static_key(int_arg(a, 0)?, 2))
    })?;
    getter(lua, "GetWeaponClass", |s, a| {
        Ok(s.static_key(int_arg(a, 0)?, 1).max(0))
    })?;
    getter(lua, "GetSkillLevel", |s, a| {
        let skill = int_arg(a, 0)?;
        Ok(s.snapshot.skills.get(&skill).copied().unwrap_or(0))
    })?;
    getter(lua, "GetPureJob", |s, _| {
        Ok(super::jobs::pure_job(s.snapshot.job))
    })?;
    getter(lua, "GetCategoryJob", |s, a| {
        Ok(super::jobs::category_job(int_arg(a, 0)?, s.snapshot.job))
    })?;
    getter(lua, "CheckJobGroup2", |s, _| {
        Ok(super::jobs::job_group2(s.snapshot.job))
    })?;
    getter(lua, "GetPetRelationship", |s, _| {
        Ok(super::pet_relationship(s.snapshot.pet_intimacy))
    })?;
    // nostalRE has no PC cafe
    getter(lua, "IsPremiumPcCafe", |_, _| Ok(0))?;
    getter(lua, "get", |s, a| Ok(s.snapshot.get(int_arg(a, 0)?)))?;
    getter(lua, "GetEquipTempValue", |s, a| {
        Ok(s.temp.get(&int_arg(a, 0)?).copied().unwrap_or(0))
    })?;
    getter(lua, "SetEquipTempValue", |s, a| {
        let (key, value) = (int_arg(a, 0)?, int_arg(a, 1)?);
        s.temp.insert(key, value);
        Ok(0)
    })?;
    globals.set(
        "GetMapName",
        lua.create_function(|lua, ()| with_state(lua, |s| s.snapshot.map.clone()))?,
    )?;
    globals.set(
        "C_Logger",
        lua.create_function(|_, text: Value| match text {
            Value::String(s) => {
                tracing::debug!(text = %s.to_string_lossy(), "EquipmentProperties log");
                Ok(())
            }
            _ => Err(mlua::Error::runtime("Logger arg1 Error")),
        })?,
    )?;
    Ok(())
}
