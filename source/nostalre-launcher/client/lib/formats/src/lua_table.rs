use std::collections::HashMap;

use crate::lub;
use crate::lub::{Key, LuaState, LubError, Value};
use std::rc::Rc;

pub fn decode_euc_kr(data: &[u8]) -> String {
    // Translation overlays explicitly mark UTF-8. Keep unmarked GRF resource
    // names in CP949: guessing UTF-8/Western encodings corrupts Korean paths.
    if let Some(utf8) = data.strip_prefix(b"\xef\xbb\xbf") {
        return String::from_utf8_lossy(utf8).into_owned();
    }
    let (decoded, _, _) = encoding_rs::EUC_KR.decode(data);
    decoded.into_owned()
}

pub fn parse_item_res_table(data: &[u8]) -> HashMap<u32, String> {
    let content = decode_euc_kr(data);
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let parts: Vec<&str> = line.split('#').collect();
        if parts.len() >= 2 {
            if let Ok(id) = parts[0].parse::<u32>() {
                let name = parts[1];
                if !name.is_empty() {
                    map.insert(id, name.to_string());
                }
            }
        }
    }
    map
}

pub fn parse_item_name_table(data: &[u8]) -> HashMap<u32, String> {
    let content = decode_euc_kr(data);
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let parts: Vec<&str> = line.split('#').collect();
        if parts.len() >= 2 {
            if let Ok(id) = parts[0].parse::<u32>() {
                let name = parts[1].replace("_", " ");
                if !name.is_empty() {
                    map.insert(id, name.to_string());
                }
            }
        }
    }
    map
}

pub fn parse_item_description_table(data: &[u8]) -> HashMap<u32, Vec<String>> {
    let content = decode_euc_kr(data);
    let mut map: HashMap<u32, Vec<String>> = HashMap::new();
    let mut current_id: Option<u32> = None;

    for token in content.split('#') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(id) = trimmed.parse::<u32>() {
            current_id = Some(id);
        } else if let Some(id) = current_id {
            let lines: Vec<String> = token
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.to_string())
                .collect();
            if !lines.is_empty() {
                map.entry(id).or_default().extend(lines);
            }
        }
    }
    map
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuestDisplay {
    pub title: String,
    pub icon_name: String,
    pub image_name: String,
    pub summary: String,
    pub description: String,
}

/// Parses `data/questid2display.txt`: a `#`-delimited stream of 6-field records
/// `id # title # icon # image # summary # description`. Newlines inside a field
/// are cosmetic and whole `//` lines are comments.
pub fn parse_questid2display(data: &[u8]) -> HashMap<u32, QuestDisplay> {
    let content = decode_euc_kr(data);
    let body: String = content
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let tokens: Vec<&str> = body.split('#').collect();
    let mut map = HashMap::new();
    for chunk in tokens.chunks(6) {
        if chunk.len() < 6 {
            break;
        }
        let Ok(id) = chunk[0].trim().parse::<u32>() else {
            continue;
        };
        map.insert(
            id,
            QuestDisplay {
                title: chunk[1].trim().to_string(),
                icon_name: chunk[2].trim().to_string(),
                image_name: chunk[3].trim().to_string(),
                summary: chunk[4].trim().to_string(),
                description: chunk[5].trim().to_string(),
            },
        );
    }
    map
}

pub fn parse_id_set_table(data: &[u8]) -> std::collections::HashSet<u32> {
    let content = decode_euc_kr(data);
    let mut set = std::collections::HashSet::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let parts: Vec<&str> = line.split('#').collect();
        if !parts.is_empty() {
            if let Ok(id) = parts[0].parse::<u32>() {
                set.insert(id);
            }
        }
    }
    set
}

pub fn parse_skill_name_table(data: &[u8]) -> HashMap<String, String> {
    let content = decode_euc_kr(data);
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let parts: Vec<&str> = line.split('#').collect();
        if parts.len() >= 2 {
            let name = parts[0].trim();
            let display = parts[1].trim();
            if !name.is_empty() && !display.is_empty() {
                map.insert(name.to_string(), display.replace("_", " ").to_string());
            }
        }
    }
    map
}

pub fn parse_skill_description_table(data: &[u8]) -> HashMap<String, Vec<String>> {
    let content = decode_euc_kr(data);
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let mut current_name: Option<String> = None;

    for token in content.split('#') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.contains(' ') && !trimmed.contains('\n') && trimmed.contains('_') {
            current_name = Some(trimmed.to_string());
        } else if let Some(ref name) = current_name {
            let lines: Vec<String> = token
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.to_string())
                .collect();
            if !lines.is_empty() {
                map.entry(name.clone()).or_default().extend(lines);
            }
        }
    }
    map
}

pub fn parse_level_use_skill_sp_table(data: &[u8]) -> HashMap<String, Vec<i16>> {
    let content = decode_euc_kr(data);
    let mut map = HashMap::new();
    let mut current_name: Option<String> = None;
    let mut sp_list: Vec<i16> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line == "@" {
            if let Some(name) = current_name.take() {
                if !sp_list.is_empty() {
                    map.insert(name, std::mem::take(&mut sp_list));
                }
            }
            sp_list.clear();
            continue;
        }
        let stripped = line.trim_end_matches('#');
        if let Ok(sp) = stripped.parse::<i16>() {
            sp_list.push(sp);
        } else if !stripped.is_empty() && stripped.contains('_') {
            if let Some(name) = current_name.take() {
                if !sp_list.is_empty() {
                    map.insert(name, std::mem::take(&mut sp_list));
                }
            }
            sp_list.clear();
            current_name = Some(stripped.to_string());
        }
    }
    if let Some(name) = current_name {
        if !sp_list.is_empty() {
            map.insert(name, sp_list);
        }
    }
    map
}

pub fn build_accessory_table(id_content: &str, name_content: &str) -> HashMap<u16, String> {
    let name_to_id = parse_assignments(id_content);
    let name_to_suffix = parse_table_entries(name_content, "ACCESSORY_IDs.ACCESSORY_");

    let mut table = HashMap::new();
    for (name, id) in &name_to_id {
        if let Some(suffix) = name_to_suffix.get(name) {
            table.insert(*id as u16, suffix.clone());
        }
    }
    table
}

pub fn build_accessory_table_from_lub(
    id_chunk: &[u8],
    name_chunk: &[u8],
) -> Result<HashMap<u16, String>, LubError> {
    named_ids(id_chunk, name_chunk, "AccNameTable")
}

/// The garments' sprite folders by view (`RobeNameTable`, indexed by `SPRITE_ROBE_IDs`): the view rAthena sends as
/// `LOOK_ROBE`.
pub fn parse_robe_names(
    id_chunk: &[u8],
    name_chunk: &[u8],
) -> Result<HashMap<u16, String>, LubError> {
    named_ids(id_chunk, name_chunk, "RobeNameTable")
}

/// `table` of `name_chunk`, whose keys are the ids `id_chunk` names: id -> name.
fn named_ids(
    id_chunk: &[u8],
    name_chunk: &[u8],
    table: &'static str,
) -> Result<HashMap<u16, String>, LubError> {
    let mut state = LuaState::new();
    lub::load_chunk(id_chunk, &mut state)?;
    lub::load_chunk(name_chunk, &mut state)?;
    let table = state
        .global_table(table)
        .ok_or(LubError::MissingTable(table))?;
    Ok(table
        .iter()
        .filter_map(|(key, value)| Some((key_id(key)?, decode_euc_kr(value.as_bytes()?))))
        .collect())
}

/// Where a garment is drawn, as kRO's `_New_DrawOnTop` has it: over the body, but on the frames `behind` lists for
/// the sex (0 female, 1 male), the job and the action. A job `jobs` lists uses the rows of the job it names
/// (`GetSpriteInherite2Job`).
#[derive(Debug, Default, Clone)]
pub struct RobeLayers {
    pub jobs: HashMap<u16, u16>,
    pub behind: [HashMap<u16, HashMap<u16, Vec<u16>>>; 2],
}

/// [`RobeLayers`] from `_new_2dlayerdir_f` (`LAYER_ALL_TABLE_LIST`) and the two `_new_biglayerdir` tables, every
/// garment's (kRO lists no small ones). `job_ids` defines the `JOBID` they are indexed by.
pub fn parse_robe_layers(
    job_ids: &[u8],
    jobs: &[u8],
    female: &[u8],
    male: &[u8],
) -> Result<RobeLayers, LubError> {
    let mut state = LuaState::new();
    // jobinheritlist calls functions after its JOBID table, which the VM does not run: the table is what is needed
    match lub::load_chunk(job_ids, &mut state) {
        Ok(()) | Err(LubError::UnsupportedOpcode(_)) => {}
        Err(e) => return Err(e),
    }
    state
        .global_table("JOBID")
        .ok_or(LubError::MissingTable("JOBID"))?;
    for chunk in [jobs, female, male] {
        lub::load_chunk(chunk, &mut state)?;
    }
    let number = |value: &Value| value.as_number().and_then(to_id);
    let jobs = state
        .global_table("LAYER_ALL_TABLE_LIST")
        .ok_or(LubError::MissingTable("LAYER_ALL_TABLE_LIST"))?
        .iter()
        .filter_map(|(job, inherits)| Some((key_id(job)?, number(inherits)?)))
        .collect();
    let rows = |name: &'static str| -> Result<HashMap<u16, HashMap<u16, Vec<u16>>>, LubError> {
        let table = state
            .global_table(name)
            .ok_or(LubError::MissingTable(name))?;
        Ok(table
            .iter()
            .filter_map(|(job, actions)| {
                let actions = state
                    .table(actions)?
                    .iter()
                    .filter_map(|(action, frames)| {
                        let mut frames: Vec<u16> =
                            state.table(frames)?.values().filter_map(number).collect();
                        frames.sort_unstable();
                        Some((key_id(action)?, frames))
                    })
                    .collect();
                Some((key_id(job)?, actions))
            })
            .collect())
    };
    Ok(RobeLayers {
        jobs,
        behind: [rows("_New_BigLayerDir_F")?, rows("_New_BigLayerDir_M")?],
    })
}

fn key_id(key: &Key) -> Option<u16> {
    key.as_number().and_then(to_id)
}

pub fn parse_jt_identity_lub(chunk: &[u8]) -> Result<HashMap<u16, String>, LubError> {
    let mut state = LuaState::new();
    lub::load_chunk(chunk, &mut state)?;

    let mut identities = HashMap::new();
    for table in state.tables() {
        for (key, value) in table {
            if let (Some(name), Some(id)) = (key.as_bytes(), value.as_number().and_then(to_id)) {
                identities.insert(id, decode_euc_kr(name));
            }
        }
    }
    Ok(identities)
}

/// Resolves `JobNameTable` to job id -> sprite name. Its keys are `jobtbl.JT_*`
/// lookups, so the identity chunks that define `jobtbl` have to be loaded into
/// the same state first, in the order given.
pub fn parse_job_name_lub(chunks: &[&[u8]]) -> Result<HashMap<u16, String>, LubError> {
    let mut state = LuaState::new();
    for chunk in chunks {
        lub::load_chunk(chunk, &mut state)?;
    }

    let mut names = HashMap::new();
    let Some(table) = state.global_table("JobNameTable") else {
        return Ok(names);
    };
    for (key, value) in table {
        if let (Some(id), Some(name)) = (key.as_number().and_then(to_id), value.as_bytes()) {
            names.insert(id, decode_euc_kr(name));
        }
    }
    Ok(names)
}

/// One item of kRO's iteminfo (`tbl`): what the client draws and names it by
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ItemInfo {
    /// For a weapon, the view it is drawn as: rAthena sends a weapon without a view id as its item id, which the
    /// client resolves through this
    pub class_num: Option<u16>,
    pub identified_resource: Option<String>,
    pub unidentified_resource: Option<String>,
    pub identified_name: Option<String>,
    pub unidentified_name: Option<String>,
}

/// kRO's iteminfo per item id: the current table (`System/itemInfo_true.lub`) has the 32-bit ids the text tables
/// in the GRF predate
pub fn parse_item_info(chunk: &[u8]) -> Result<HashMap<u32, ItemInfo>, LubError> {
    let mut state = LuaState::new();
    lub::load_chunk(chunk, &mut state)?;
    let table = state.global_table("tbl").ok_or(LubError::TypeError)?;
    let mut items = HashMap::new();
    for (key, entry) in table {
        let (Some(id), Some(entry)) = (
            key.as_number()
                .filter(|n| *n >= 0.0 && *n <= u32::MAX as f64),
            state.table(entry),
        ) else {
            continue;
        };
        let field = |name: &str| entry.get(&Key::Str(Rc::from(name.as_bytes())));
        let text = |name: &str| field(name).and_then(Value::as_bytes).map(decode_euc_kr);
        items.insert(
            id as u32,
            ItemInfo {
                class_num: field("ClassNum").and_then(Value::as_number).and_then(to_id),
                identified_resource: text("identifiedResourceName"),
                unidentified_resource: text("unidentifiedResourceName"),
                identified_name: text("identifiedDisplayName"),
                unidentified_name: text("unidentifiedDisplayName"),
            },
        );
    }
    Ok(items)
}

/// weapontable's views past the 30 base ones
#[derive(Debug, Default)]
pub struct WeaponTable {
    /// Each view as the look that draws it: `WeaponNameTable` names most after a classic item (`_1410`, the Lance),
    /// whose art the client finds by item id; the rest take their base view (`Expansion_Weapon_IDs`)
    pub expansions: HashMap<u16, u16>,
    /// The views named after art of their own instead: the file suffix (`_강아지풀_녹색`, a Doram's green foxtail),
    /// drawn as `<job>_<sex><suffix>`
    pub art_names: HashMap<u16, String>,
}

pub fn parse_weapon_table(chunk: &[u8]) -> Result<WeaponTable, LubError> {
    let mut state = LuaState::new();
    lub::load_chunk(chunk, &mut state)?;
    let bases = state
        .global_table("Expansion_Weapon_IDs")
        .ok_or(LubError::TypeError)?;
    let names = state.global_table("WeaponNameTable");
    let mut table = WeaponTable::default();
    for (key, base) in bases {
        let Some(view) = key.as_number().and_then(to_id) else {
            continue;
        };
        let name = names
            .and_then(|names| names.get(key))
            .and_then(Value::as_bytes);
        let classic_item = name.and_then(|name| {
            std::str::from_utf8(name.strip_prefix(b"_")?)
                .ok()?
                .parse()
                .ok()
        });
        if classic_item.is_none()
            && let Some(name) = name
        {
            table.art_names.insert(view, decode_euc_kr(name));
        }
        if let Some(look) = classic_item.or_else(|| base.as_number().and_then(to_id)) {
            table.expansions.insert(view, look);
        }
    }
    Ok(table)
}

/// kRO's styling shop (`stylingshop/stylingshopinfo.lub`): its lists by the name each call adds to (`HeadStyle`,
/// `HeadPalette`, `TopAccessory`, `DoramHeadStyle`...), in order. A choice's place in its list, from 1, is what the
/// stylist request sends (rAthena's `Index` in `stylist.yml`).
#[derive(Debug, Default)]
pub struct StylingShopTable {
    pub lists: HashMap<String, Vec<StyleGoods>>,
}

/// One choice of the styling shop: the look or item it gives and what it costs (zeny, or an item, or the box that
/// holds one)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleGoods {
    pub goods: u32,
    pub zeny: u32,
    pub item: Option<u32>,
    pub box_item: Option<u32>,
    /// Offered to a Doram too (the human lists' `isDoram`; the Doram lists are theirs alone)
    pub doram_too: bool,
    /// A hairstyle that takes a hair colour (`isColoring`)
    pub dyeable: bool,
}

/// The table is a list of calls to `StylingShop.AddX(goods, payment, flags...)`, functions the client defines in
/// `stylingshopinfo_f.lub`: the reader records them (`LuaState::calls`) instead of running them. The flags: a
/// hairstyle's `isColoring, isDoram` (every style has its nine palettes, and the six that carry flags are all
/// colourable: the second is the one that varies, and a Doram has a hairstyle list of its own), a Doram
/// hairstyle's `isColoring` (`stylingshopinfo_f.lub`'s parameters), palettes none, the rest `isDoram`.
pub fn parse_styling_shop(chunk: &[u8]) -> Result<StylingShopTable, LubError> {
    let mut state = LuaState::new();
    lub::load_chunk(chunk, &mut state)?;
    let mut lists: HashMap<String, Vec<StyleGoods>> = HashMap::new();
    let whole = |v: &Value| {
        v.as_number()
            .filter(|n| *n >= 0.0 && *n <= u32::MAX as f64)
            .map(|n| n as u32)
    };
    for call in state.calls() {
        let Some(list) = call.function.strip_prefix(&b"StylingShop.Add"[..]) else {
            continue;
        };
        let Some(goods) = call.args.first().and_then(whole) else {
            continue;
        };
        let list = String::from_utf8_lossy(list).into_owned();
        // most calls leave the flags out: nil, which kRO reads as not for Doram and dyeable
        let flag = |i: usize| matches!(call.args.get(i), Some(Value::Bool(true)));
        let not_false = |i: usize| !matches!(call.args.get(i), Some(Value::Bool(false)));
        let payment = call.args.get(1).and_then(|v| state.table(v));
        let cost = |name: &str| {
            payment
                .and_then(|t| t.get(&Key::Str(Rc::from(name.as_bytes()))))
                .and_then(whole)
        };
        let (doram_too, dyeable) = match list.as_str() {
            "HeadStyle" => (flag(3), not_false(2)),
            "DoramHeadStyle" => (true, not_false(2)),
            "HeadPalette" | "TopAccessory" | "MidAccessory" | "BottomAccessory"
            | "SecondCostume" => (flag(2), true),
            _ => (true, true),
        };
        lists.entry(list).or_default().push(StyleGoods {
            goods,
            zeny: cost("zeny").unwrap_or(0),
            item: cost("itid"),
            box_item: cost("boxitid"),
            doram_too,
            dyeable,
        });
    }
    Ok(StylingShopTable { lists })
}

/// One row of kRO's `SignBoardList` (`signboardlist.lub`) as `GetSignBoardListTable` hands it to the exe (0xadefa0,
/// format `d>sddddsss`): the fields as written, before the exe's defaults; a missing string is `None`, a missing
/// number 0 (Lua's `tonumber` of nil)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignBoardRow {
    pub map: String,
    pub x: i32,
    pub y: i32,
    pub height: i32,
    pub kind: i32,
    pub image: Option<String>,
    pub text: Option<String>,
    pub color: Option<String>,
}

/// `SignBoardList`'s rows in order, from the first (kRO's defaults) to the first missing one
pub fn parse_sign_board_list(chunk: &[u8]) -> Result<Vec<SignBoardRow>, LubError> {
    let mut state = LuaState::new();
    lub::load_chunk(chunk, &mut state)?;
    let list = state
        .global_table("SignBoardList")
        .ok_or(LubError::MissingTable("SignBoardList"))?;
    let index = |n: usize| Key::Number((n as f64).to_bits());
    let mut rows = Vec::new();
    while let Some(row) = list
        .get(&index(rows.len() + 1))
        .and_then(|v| state.table(v))
    {
        let string = |i| {
            row.get(&index(i))
                .and_then(Value::as_bytes)
                .map(decode_euc_kr)
        };
        let number = |i| {
            row.get(&index(i))
                .and_then(Value::as_number)
                .map_or(0, |n| n as i32)
        };
        rows.push(SignBoardRow {
            map: string(1).unwrap_or_default(),
            x: number(2),
            y: number(3),
            height: number(4),
            kind: number(5),
            image: string(6),
            text: string(7),
            color: string(8),
        });
    }
    Ok(rows)
}

/// SP per level, by skill name, of the skills whose level the player picks (`bSeperateLv`) in kRO's
/// `SKILL_INFO_LIST` (`skillinfolist.lub`). Its keys index `SKID` and its prerequisites `JOBID`, so the chunks
/// defining those (`skillid.lub`, `jobinheritlist.lub`) come first, in the order given.
pub fn parse_skill_info_sp_amounts(
    chunks: &[&[u8]],
) -> Result<HashMap<String, Vec<i16>>, LubError> {
    let mut state = LuaState::new();
    for chunk in chunks {
        lub::load_chunk(chunk, &mut state)?;
    }
    let list = state
        .global_table("SKILL_INFO_LIST")
        .ok_or(LubError::MissingTable("SKILL_INFO_LIST"))?;
    let index = |n: usize| Key::Number((n as f64).to_bits());
    let field = |name: &str| Key::Str(Rc::from(name.as_bytes()));
    let mut skills = HashMap::new();
    for entry in list.values().filter_map(|v| state.table(v)) {
        if !matches!(entry.get(&field("bSeperateLv")), Some(Value::Bool(true))) {
            continue;
        }
        let Some(name) = entry.get(&index(1)).and_then(Value::as_bytes) else {
            continue;
        };
        let amounts = entry.get(&field("SpAmount")).and_then(|v| state.table(v));
        let sp = (1..)
            .map_while(|level| amounts?.get(&index(level))?.as_number())
            .map(|sp| sp as i16)
            .collect();
        skills.insert(decode_euc_kr(name), sp);
    }
    Ok(skills)
}

fn to_id(number: f64) -> Option<u16> {
    (number >= 0.0 && number <= u16::MAX as f64).then_some(number as u16)
}

fn parse_assignments(content: &str) -> HashMap<String, u32> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("--") || line.is_empty() {
            continue;
        }
        if let Some((name_part, value_part)) = line.split_once('=') {
            let name = name_part.trim().trim_start_matches("ACCESSORY_");
            let value_str = value_part.trim().trim_end_matches(',').trim();
            if let Ok(id) = value_str.parse::<u32>() {
                map.insert(name.to_string(), id);
            }
        }
    }
    map
}

fn parse_table_entries(content: &str, prefix: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("--") || line.is_empty() {
            continue;
        }
        let Some(rest) = line.strip_prefix('[') else {
            continue;
        };
        let Some((key_part, value_part)) = rest.split_once(']') else {
            continue;
        };
        let name = key_part
            .trim()
            .strip_prefix(prefix)
            .unwrap_or(key_part.trim());

        let Some((_, val_rest)) = value_part.split_once('=') else {
            continue;
        };
        let val = val_rest.trim().trim_end_matches(',').trim();
        if val.starts_with('"') && val.ends_with('"') {
            let inner = &val[1..val.len() - 1];
            map.insert(name.to_string(), inner.to_string());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_translation_tables_preserve_portuguese_accents_and_ids() {
        let names = parse_item_name_table("\u{feff}501#Poção_Vermelha#\n".as_bytes());
        assert_eq!(names.get(&501).map(String::as_str), Some("Poção Vermelha"));
        let descriptions = parse_item_description_table("\u{feff}501#\nRecuperação de HP.\n#\n".as_bytes());
        assert_eq!(descriptions.get(&501).unwrap(), &["Recuperação de HP."]);
        // Resource names from unmarked original archives must still use CP949.
        let (korean, _, _) = encoding_rs::EUC_KR.encode("포션");
        assert_eq!(decode_euc_kr(&korean), "포션");
    }

    #[test]
    fn the_styling_shop_lists_kros_choices_in_order_with_their_costs_and_flags() {
        use crate::lub::test_chunk::{CallSpec, call_chunk};
        let call = |function, goods, cost, flags| CallSpec {
            function,
            goods,
            cost,
            flags,
        };
        let chunk = call_chunk(
            "StylingShop",
            &[
                call("AddHeadStyle", 1.0, ("zeny", 100_000.0), &[true, true]),
                call("AddHeadStyle", 2.0, ("zeny", 100_000.0), &[true, false]),
                call("AddHeadStyle", 3.0, ("zeny", 100_000.0), &[false, true]),
                call("AddHeadStyle", 7.0, ("zeny", 100_000.0), &[]),
                call("AddBodyPalette", 2.0, ("itid", 6046.0), &[]),
                call("AddMidAccessory", 2239.0, ("zeny", 10_000.0), &[false]),
                call("AddDoramHeadStyle", 3.0, ("zeny", 100_000.0), &[false]),
            ],
        );
        let table = parse_styling_shop(&chunk).unwrap();
        let hair = &table.lists["HeadStyle"];
        assert_eq!(hair.iter().map(|g| g.goods).collect::<Vec<_>>(), [1, 2, 3, 7]);
        assert!(
            hair[3].dyeable && !hair[3].doram_too,
            "flags left out: dyeable, humans only"
        );
        assert_eq!(
            (hair[0].zeny, hair[0].doram_too, hair[0].dyeable),
            (100_000, true, true)
        );
        assert!(
            hair[1].dyeable && !hair[1].doram_too,
            "style 2: colourable, not for Doram"
        );
        assert!(!hair[2].dyeable, "a false first flag takes no colour");
        let dye = &table.lists["BodyPalette"][0];
        assert_eq!((dye.goods, dye.item, dye.zeny), (2, Some(6046), 0));
        assert!(!table.lists["MidAccessory"][0].doram_too);
        let doram = &table.lists["DoramHeadStyle"][0];
        assert_eq!(
            (doram.goods, doram.doram_too, doram.dyeable),
            (3, true, false)
        );
    }

    #[test]
    fn parse_accessory_id_and_name() {
        let id_content = r#"
ACCESSORY_GOGGLES = 1,
ACCESSORY_RIBBON = 4,
ACCESSORY_HEADBAND = 6,
"#;
        let name_content = r#"
[ACCESSORY_IDs.ACCESSORY_GOGGLES] = "_고글",
[ACCESSORY_IDs.ACCESSORY_RIBBON] = "_리본",
[ACCESSORY_IDs.ACCESSORY_HEADBAND] = "_머리띠",
"#;
        let table = build_accessory_table(id_content, name_content);
        assert_eq!(table.get(&1).unwrap(), "_고글");
        assert_eq!(table.get(&4).unwrap(), "_리본");
        assert_eq!(table.get(&6).unwrap(), "_머리띠");
        assert_eq!(table.len(), 3);
    }

    #[test]
    fn skips_comments_and_empty_lines() {
        let id_content = "-- comment\nACCESSORY_TEST = 5,\n\n";
        let name_content = "-- comment\n[ACCESSORY_IDs.ACCESSORY_TEST] = \"_테스트\",\n";
        let table = build_accessory_table(id_content, name_content);
        assert_eq!(table.get(&5).unwrap(), "_테스트");
    }

    #[test]
    fn unmatched_entries_are_skipped() {
        let id_content = "ACCESSORY_A = 1,\nACCESSORY_B = 2,\n";
        let name_content = "[ACCESSORY_IDs.ACCESSORY_A] = \"_a\",\n";
        let table = build_accessory_table(id_content, name_content);
        assert_eq!(table.len(), 1);
        assert!(table.get(&2).is_none());
    }

    #[test]
    fn parse_item_description_table_multiple_items() {
        let data = b"501#\nA red potion.\n^FFFFFF_^000000\nClass:^0000FF Restorative^000000\nWeight:^009900 7^000000\n#\n502#\nAn orange potion.\nWeight:^009900 10^000000\n#\n";
        let table = parse_item_description_table(data);
        assert_eq!(table.len(), 2);
        let desc_501 = table.get(&501).unwrap();
        assert_eq!(desc_501.len(), 4);
        assert_eq!(desc_501[0], "A red potion.");
        assert_eq!(desc_501[1], "^FFFFFF_^000000");
        assert_eq!(desc_501[2], "Class:^0000FF Restorative^000000");
        assert_eq!(desc_501[3], "Weight:^009900 7^000000");
        let desc_502 = table.get(&502).unwrap();
        assert_eq!(desc_502.len(), 2);
        assert_eq!(desc_502[0], "An orange potion.");
    }

    #[test]
    fn parse_item_description_table_missing_item_returns_none() {
        let data = b"501#\nSome desc\n#\n";
        let table = parse_item_description_table(data);
        assert!(table.get(&999).is_none());
    }

    #[test]
    fn parse_level_use_skill_sp_table_parses_blocks() {
        let data = b"SM_BASH#\n8#\n8#\n15#\n@\nSM_PROVOKE#\n4#\n5#\n6#\n@\n";
        let table = parse_level_use_skill_sp_table(data);
        assert_eq!(table.len(), 2);
        assert_eq!(table["SM_BASH"], vec![8, 8, 15]);
        assert_eq!(table["SM_PROVOKE"], vec![4, 5, 6]);
    }

    #[test]
    fn parse_level_use_skill_sp_table_skips_comments() {
        let data = b"// comment\nSM_BASH#\n8#\n@\n";
        let table = parse_level_use_skill_sp_table(data);
        assert_eq!(table.len(), 1);
        assert_eq!(table["SM_BASH"], vec![8]);
    }

    #[test]
    fn parse_questid2display_records_skip_comments_keep_color_codes() {
        let data = b"// header comment\n1000#First Quest#SG_FEEL#que_noimage#Summary one#\nHunt ^FF0000ten^000000 Porings.#\n2000#Second#ico_new#que_img#S2#D2#\n";
        let table = parse_questid2display(data);
        assert_eq!(table.len(), 2);
        let q = table.get(&1000).unwrap();
        assert_eq!(q.title, "First Quest");
        assert_eq!(q.icon_name, "SG_FEEL");
        assert_eq!(q.image_name, "que_noimage");
        assert_eq!(q.summary, "Summary one");
        assert_eq!(q.description, "Hunt ^FF0000ten^000000 Porings.");
        assert_eq!(table.get(&2000).unwrap().icon_name, "ico_new");
    }

    #[test]
    fn parse_id_set_table_extracts_ids() {
        let data = b"4001#\n4002#\n// comment\n\n4003#\n";
        let set = parse_id_set_table(data);
        assert_eq!(set.len(), 3);
        assert!(set.contains(&4001));
        assert!(set.contains(&4002));
        assert!(set.contains(&4003));
        assert!(!set.contains(&9999));
    }
}
