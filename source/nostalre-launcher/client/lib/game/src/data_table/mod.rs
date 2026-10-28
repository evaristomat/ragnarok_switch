use std::collections::HashMap;

use models::enums::skill_enums::SkillEnum;

use crate::data_table::accessory_table::AccessoryTable;
use crate::data_table::card_illustration_table::CardIllustrationTable;
use crate::data_table::card_name_table::CardNameTable;
use crate::data_table::item_description_table::ItemDescriptionTable;
use crate::data_table::item_name_table::ItemNameTable;
use crate::data_table::item_resource_table::ItemResourceTable;
use crate::data_table::item_slot_count_table::ItemSlotCountTable;
use crate::data_table::map_name_table::MapNameTable;
use crate::data_table::map_position_table::MapPositionTable;
use crate::data_table::msg_string_table::MsgStringTable;
use crate::data_table::name_table::NameTable;
use crate::data_table::quest_display_table::QuestDisplayTable;
use crate::data_table::robe_table::RobeTable;
use crate::data_table::skill_description_table::SkillDescriptionTable;
use crate::data_table::skill_name_table::SkillNameTable;
use crate::data_table::skill_use_level_table::SkillUseLevelTable;

pub mod accessory_table;
pub mod card_illustration_table;
pub mod card_name_table;
pub mod fog_table;
pub mod indoor_table;
pub mod item_description_table;
pub mod item_name_table;
pub mod item_resource_table;
pub mod item_slot_count_table;
pub mod map_cloud_table;
pub mod map_name_table;
pub mod map_position_table;
pub mod msg_string_table;
pub mod name_table;
pub mod quest_display_table;
mod quest_classic_pt_br;
pub mod robe_table;
pub mod skill_description_table;
pub mod skill_name_table;
pub mod skill_use_level_table;
#[derive(Default)]
pub struct DataTable {
    pub name: Option<NameTable>,
    pub accessory: Option<AccessoryTable>,
    pub item_name: Option<ItemNameTable>,
    pub item_resource: Option<ItemResourceTable>,
    pub item_slot_count: Option<ItemSlotCountTable>,
    pub card_name: Option<CardNameTable>,
    pub card_illustration: Option<CardIllustrationTable>,
    pub item_description: Option<ItemDescriptionTable>,
    pub skill_name: Option<SkillNameTable>,
    pub skill_description: Option<SkillDescriptionTable>,
    pub skill_use_level: Option<SkillUseLevelTable>,
    pub pet_talk: Option<nostalre_formats::pettalk::PetTalkTable>,
    pub quest_display: Option<QuestDisplayTable>,
    pub msg_string: Option<MsgStringTable>,
    pub map_position: Option<MapPositionTable>,
    pub map_name: Option<MapNameTable>,
    /// Loaded with the first garment drawn (six lub chunks, about 12 ms on the desktop).
    pub robe: std::cell::OnceCell<RobeTable>,
    /// kRO's stylist lists (`load_styling_shop`); empty when the archive does not have them
    pub styling_shop: nostalre_formats::lua_table::StylingShopTable,
    /// A headgear item's view (`headgearviewtable.txt`), for drawing one the player does not wear yet
    pub headgear_view: HashMap<u32, u16>,
    /// kRO's sign boards over map cells (`load_sign_boards`)
    pub sign_boards: Vec<crate::sign_board::SignBoard>,
    /// The titles shown on entering a map (`mapInfo.lub`)
    pub map_titles: crate::map_title::MapTitles,
    /// The special pop-ups by id (`spopup.lub`)
    pub special_popups: HashMap<u32, crate::special_popup::SpecialPopup>,
    /// Title names and the achievements that give them (`titletable.lub`, `achievements.lub`)
    pub titles: crate::title::TitleTables,
    /// The damage skins in id order (`load_damage_skins`); empty without the tables
    pub damage_skins: Vec<crate::damage_skin::DamageSkin>,
}

impl DataTable {
    pub fn new() -> Self {
        Self {
            name: None,
            accessory: None,
            item_name: None,
            item_resource: None,
            item_slot_count: None,
            card_name: None,
            card_illustration: None,
            item_description: None,
            skill_name: None,
            skill_description: None,
            skill_use_level: None,
            pet_talk: None,
            quest_display: None,
            msg_string: None,
            map_position: None,
            map_name: None,
            robe: std::cell::OnceCell::new(),
            styling_shop: Default::default(),
            headgear_view: HashMap::new(),
            sign_boards: Vec::new(),
            map_titles: Default::default(),
            special_popups: HashMap::new(),
            titles: Default::default(),
            damage_skins: Vec::new(),
        }
    }

    /// An item's display name, "Item #id" when no table names it; an unidentified item's is its unidentified one
    /// A skill's name as the tables give it, its aegis name when they lack it
    pub fn skill_name_for(&self, skill: SkillEnum) -> &str {
        skill_name_table::format_skill_display_name(skill, self.skill_name.as_ref())
    }

    pub fn item_name_for(&self, item_id: u32, identified: bool) -> String {
        self.item_name.as_ref().map_or_else(
            || format!("Item #{item_id}"),
            |t| t.get_name_or_id_for(item_id, identified),
        )
    }

    pub fn item_icon_path(&self, item_id: u32) -> Option<String> {
        self.item_resource.as_ref()?.item_icon_path(item_id)
    }

    /// A msgstringtable line with its arguments filled in
    pub fn msg(&self, id: u16, args: &[&str]) -> Option<String> {
        self.msg_string.as_ref().and_then(|t| t.format(id, args))
    }

    /// A msgstringtable line with its arguments filled in, `fallback` when the table lacks it
    pub fn msg_or(&self, id: u16, args: &[&str], fallback: &str) -> String {
        self.msg(id, args).unwrap_or_else(|| fallback.to_string())
    }
}

/// kRO's iteminfo: its current table if the loose files have it (`System/itemInfo_true.lub`), else the GRF's older
/// one; then other servers' official tables, then the English translation's, each for the items the ones before
/// lack (an official table knows an item's look, the translation's often not). Read once; the item tables and the
/// hand looks fill in from it what the GRF's text tables lack.
pub fn load_item_info(
    grf: &nostalre_formats::grf::GrfArchive,
) -> std::collections::HashMap<u32, nostalre_formats::lua_table::ItemInfo> {
    use nostalre_resources::lua::{
        ITEM_INFO_514_LUB, ITEM_INFO_EN_514_LUB, ITEM_INFO_IRO_514_LUB, ITEM_INFO_TRUE_514_LUB,
    };
    let parse = nostalre_formats::lua_table::parse_item_info;
    let mut items = read_lub(grf, &[ITEM_INFO_TRUE_514_LUB, ITEM_INFO_514_LUB], parse);
    let kro = items.len();
    for table in [ITEM_INFO_IRO_514_LUB, ITEM_INFO_EN_514_LUB] {
        let before = items.len();
        add_missing_items(&mut items, read_lub(grf, &[table], parse));
        tracing::info!(table, added = items.len() - before, "iteminfo extended");
    }
    tracing::info!(items = items.len(), kro, "iteminfo loaded");
    items
}

fn add_missing_items(
    items: &mut HashMap<u32, nostalre_formats::lua_table::ItemInfo>,
    extra: HashMap<u32, nostalre_formats::lua_table::ItemInfo>,
) {
    for (id, item) in extra {
        items.entry(id).or_insert(item);
    }
}

pub fn load_styling_shop(
    grf: &nostalre_formats::grf::GrfArchive,
) -> nostalre_formats::lua_table::StylingShopTable {
    read_lub(
        grf,
        &[nostalre_resources::lua::STYLING_SHOP_514_LUB],
        nostalre_formats::lua_table::parse_styling_shop,
    )
}

/// A table the pinned client keeps beside its archive, as Lua source: its `SystemEN` copy (WARP0716's English
/// ones), else its `System` one; a compiled one (an official install's) is not read, with a warning
pub fn read_install_table(install_dir: Option<&std::path::Path>, file: &str) -> Option<Vec<u8>> {
    let dir = install_dir?;
    let (path, source) = ["SystemEN", "System"].iter().find_map(|folder| {
        let path = dir.join(folder).join(file);
        std::fs::read(&path).ok().map(|source| (path, source))
    })?;
    if nostalre_formats::lub::is_compiled_chunk(&source) {
        tracing::warn!(path = %path.display(), "compiled table, only Lua source is read");
        return None;
    }
    Some(source)
}

/// `titletable.lub` in the pinned client's own `data` folder, which kRO's client reads before its archive (the
/// archive's copy is compiled and Korean, with other ids)
const TITLE_TABLE: &str = "data/luafiles514/lua files/datainfo/titletable.lub";

/// A file of the pinned client's `data` folder, as Lua source; a compiled one is not read, with a warning
fn read_install_data_file(install_dir: Option<&std::path::Path>, file: &str) -> Option<Vec<u8>> {
    let path = install_dir?.join(file);
    let source = std::fs::read(&path).ok()?;
    if nostalre_formats::lub::is_compiled_chunk(&source) {
        tracing::warn!(path = %path.display(), "compiled table, only Lua source is read");
        return None;
    }
    Some(source)
}

/// A folder name as the pinned install's unzip wrote it: CP949 bytes read as Latin-1 (`À¯ÀúÀÎÅÍÆäÀÌ½º`), or
/// already UTF-8
fn install_folder_name(name: &str) -> String {
    match name.chars().map(|c| u8::try_from(u32::from(c)).ok()).collect::<Option<Vec<u8>>>() {
        Some(bytes) if !name.is_ascii() => encoding_rs::EUC_KR.decode(&bytes).0.into_owned(),
        _ => name.to_string(),
    }
}

fn find_folder(dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let found = entry.file_name().to_string_lossy().into_owned();
        let found = install_folder_name(&found);
        (found == name || found.eq_ignore_ascii_case(name)).then(|| entry.path())
    })
}

const UI_FOLDER: &str = "유저인터페이스";

/// The pinned install's own copies of one `유저인터페이스` folder, which kRO's client reads before its archive (the
/// English art), as (archive path, bytes); none without the install or the folder
pub fn install_ui_textures(
    install_dir: Option<&std::path::Path>,
    folder: &str,
) -> Vec<(String, Vec<u8>)> {
    let Some(dir) = install_dir
        .and_then(|dir| find_folder(&dir.join("data/texture"), UI_FOLDER))
        .and_then(|ui| find_folder(&ui, folder))
    else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let bytes = std::fs::read(entry.path()).ok()?;
            Some((
                format!("data/texture/{UI_FOLDER}/{}/{name}", folder.to_lowercase()),
                bytes,
            ))
        })
        .collect()
}

/// The map titles and special pop-ups of the pinned client's folder; none when it has neither
pub fn load_install_tables(data: &mut DataTable, install_dir: Option<&std::path::Path>) {
    if let Some(source) = read_install_table(install_dir, "mapInfo.lub") {
        data.map_titles = crate::map_title::MapTitles::parse(&source);
    }
    if let Some(source) = read_install_table(install_dir, "spopup.lub") {
        data.special_popups = crate::special_popup::parse(&source);
    }
    data.titles = crate::title::TitleTables::parse(
        read_install_data_file(install_dir, TITLE_TABLE).as_deref(),
        read_install_table(install_dir, "achievements.lub").as_deref(),
    );
    tracing::info!(
        titles = data.map_titles.len(),
        popups = data.special_popups.len(),
        title_names = data.titles.len(),
        "map titles, special pop-ups and title names"
    );
}

/// The damage skins: the archive's compiled `damageskinlist.lub` counts them, the pinned client's own
/// `damageskininfo.lub` (English, Lua source) describes them; none without both
pub fn load_damage_skins(
    grf: &nostalre_formats::grf::GrfArchive,
    install_dir: Option<&std::path::Path>,
) -> Vec<crate::damage_skin::DamageSkin> {
    let mut list = nostalre_formats::lub::LuaState::new();
    let listed = grf
        .read_file(DAMAGE_SKIN_LIST)
        .ok()
        .and_then(|chunk| nostalre_formats::lub::load_chunk(&chunk, &mut list).ok());
    let info = read_install_data_file(install_dir, DAMAGE_SKIN_INFO);
    let skins = match (listed, info) {
        (Some(()), Some(info)) => {
            crate::damage_skin::parse(&crate::damage_skin::skin_ids(&list), &info)
        }
        _ => Vec::new(),
    };
    tracing::info!(skins = skins.len(), "damage skins");
    skins
}

const DAMAGE_SKIN_LIST: &str = "data/luafiles514/lua files/damageskin/damageskinlist.lub";
const DAMAGE_SKIN_INFO: &str = "data/luafiles514/lua files/damageskin/damageskininfo.lub";

/// kRO's sign boards (`signboardlist.lub`); none when the archive does not have them
pub fn load_sign_boards(
    grf: &nostalre_formats::grf::GrfArchive,
) -> Vec<crate::sign_board::SignBoard> {
    let rows = read_lub(
        grf,
        &[nostalre_resources::lua::SIGN_BOARD_LIST_514_LUB],
        nostalre_formats::lua_table::parse_sign_board_list,
    );
    crate::sign_board::sign_boards(&rows)
}

/// The first of `paths` the archive has, parsed; empty (with a warning) when it does not read
pub(crate) fn read_lub<T: Default>(
    grf: &nostalre_formats::grf::GrfArchive,
    paths: &[&str],
    parse: fn(&[u8]) -> Result<T, nostalre_formats::lub::LubError>,
) -> T {
    let Some((path, chunk)) = paths
        .iter()
        .find_map(|path| grf.read_file(path).ok().map(|chunk| (path, chunk)))
    else {
        return T::default();
    };
    parse(&chunk).unwrap_or_else(|error| {
        tracing::warn!("{path} unreadable: {error}");
        T::default()
    })
}

/// An `id#view#` table of rAthena's views (scripts/import-english.py); empty when the archive does not have it
pub fn read_view_table(grf: &nostalre_formats::grf::GrfArchive, path: &str) -> HashMap<u32, u16> {
    grf.read_file(path)
        .map(|data| parse_view_table(&data))
        .unwrap_or_default()
}

fn parse_view_table(data: &[u8]) -> HashMap<u32, u16> {
    String::from_utf8_lossy(data)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('#');
            Some((
                fields.next()?.trim().parse().ok()?,
                fields.next()?.trim().parse().ok()?,
            ))
        })
        .collect()
}

/// Adds `field` of every item `table` lacks
pub(crate) fn fill_missing(
    table: &mut std::collections::HashMap<u32, String>,
    items: &std::collections::HashMap<u32, nostalre_formats::lua_table::ItemInfo>,
    field: fn(&nostalre_formats::lua_table::ItemInfo) -> &Option<String>,
) {
    for (id, item) in items {
        if let Some(value) = field(item).as_ref().filter(|v| !v.is_empty()) {
            table.entry(*id).or_insert_with(|| value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use nostalre_formats::lua_table::ItemInfo;

    use super::{add_missing_items, parse_view_table, read_install_table};

    #[test]
    fn an_install_table_comes_from_systemen_first_and_only_as_source() {
        let dir = std::env::temp_dir().join(format!("nostalre-install-{}", std::process::id()));
        for folder in ["SystemEN", "System"] {
            std::fs::create_dir_all(dir.join(folder)).unwrap();
        }
        std::fs::write(dir.join("SystemEN/mapInfo.lub"), "mapTbl = {}").unwrap();
        std::fs::write(dir.join("System/mapInfo.lub"), "other = {}").unwrap();
        std::fs::write(dir.join("System/spopup.lub"), b"\x1bLuaQ compiled").unwrap();

        let read = |file| read_install_table(Some(&dir), file);
        assert_eq!(read("mapInfo.lub").as_deref(), Some(&b"mapTbl = {}"[..]));
        assert_eq!(read("spopup.lub"), None);
        assert_eq!(read("missing.lub"), None);
        assert_eq!(read_install_table(None, "mapInfo.lub"), None);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn view_table_skips_lines_it_cannot_read() {
        assert_eq!(
            parse_view_table(b"460003#4#\nnot a line\n400103#1056#\n"),
            HashMap::from([(460_003, 4), (400_103, 1056)])
        );
    }

    fn resource(name: &str) -> ItemInfo {
        ItemInfo {
            identified_resource: Some(name.to_string()),
            ..ItemInfo::default()
        }
    }

    #[test]
    fn english_item_info_adds_only_items_kro_lacks() {
        let mut items = HashMap::from([(1201, resource("나이프"))]);
        add_missing_items(
            &mut items,
            HashMap::from([(1201, resource("knife")), (1270141, resource("new_sword"))]),
        );
        assert_eq!(items[&1201].identified_resource.as_deref(), Some("나이프"));
        assert_eq!(
            items[&1270141].identified_resource.as_deref(),
            Some("new_sword")
        );
    }
}
