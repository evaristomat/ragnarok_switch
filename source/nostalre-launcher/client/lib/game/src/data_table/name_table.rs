use std::collections::HashMap;

use nostalre_formats::builtin_name_table::BUILTIN_NAME_TABLE;
use nostalre_formats::grf::GrfArchive;
use nostalre_formats::lua_table;

use crate::entity::EntityCategory;
use crate::sprite_path::entity_category_from_job;

const NPC_IDENTITY_PATHS: &[&str] = &[
    nostalre_resources::lua::NPC_IDENTITY_514_LUB,
    nostalre_resources::lua::NPC_IDENTITY_LUB,
];

const JOB_NAME_PATHS: &[&str] = &[
    nostalre_resources::lua::JOB_NAME_514_LUB,
    nostalre_resources::lua::JOB_NAME_LUB,
];

pub struct NameTable {
    entries: HashMap<u16, String>,
}

impl NameTable {
    pub fn load() -> Self {
        Self {
            entries: BUILTIN_NAME_TABLE
                .iter()
                .map(|&(id, name)| (id, name.to_string()))
                .collect(),
        }
    }

    /// kRO's own names (`npcidentity` and `jobname`) over the built-in table, which keeps the ids kRO's data lacks:
    /// kRO draws by its own table (rAthena's `HIDDEN_MOB`, 2337, is `INVISIBLE` there, not a mosquito). Players are
    /// never drawn through this table.
    pub fn load_from_grf(grf: &GrfArchive) -> Self {
        let mut table = Self::load();
        let read = |paths: &[&str]| paths.iter().find_map(|path| grf.read_file(path).ok());
        let (Some(identity), Some(job_name)) = (read(NPC_IDENTITY_PATHS), read(JOB_NAME_PATHS))
        else {
            return table;
        };
        match lua_table::parse_job_name_lub(&[&identity, &job_name]) {
            Ok(names) => {
                let taken = table.take_game_names(names);
                tracing::info!(
                    "Name table: {} built in, {taken} from the game data",
                    BUILTIN_NAME_TABLE.len()
                );
            }
            Err(error) => {
                tracing::warn!("Job name tables unreadable, built-in names only: {error}")
            }
        }
        table
    }

    /// Takes the game data's names, players aside; returns how many.
    fn take_game_names(&mut self, names: HashMap<u16, String>) -> usize {
        let mut taken = 0;
        for (id, name) in names {
            if entity_category_from_job(id) != EntityCategory::Player {
                self.entries.insert(id, name);
                taken += 1;
            }
        }
        taken
    }

    pub fn ids(&self) -> impl Iterator<Item = u16> + '_ {
        self.entries.keys().copied()
    }

    pub fn get_name(&self, job_id: u16) -> Option<&str> {
        self.entries.get(&job_id).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_table_has_common_entries() {
        let table = NameTable::load();
        assert_eq!(table.get_name(1002), Some("Poring"));
        assert_eq!(table.get_name(46), Some("1_ETC_01"));
        assert_eq!(table.get_name(1885), Some("GOPINICH"));
        assert_eq!(table.get_name(566), Some("MYSTCASE"));
        assert_eq!(table.get_name(6001), Some("LIF"));
        assert!(table.get_name(60000).is_none());
    }

    /// kRO's table wins where both name an id (it is what kRO draws), fills the ids the built-in one lacks, and
    /// never adds a player job.
    #[test]
    fn game_data_names_win() {
        let mut table = NameTable::load();
        let kro = HashMap::from([
            (20921, "chimera_fulgor".to_string()),
            (2337, "INVISIBLE".to_string()),
            (4252, "DRAGON_KNIGHT".to_string()),
        ]);
        assert_eq!(table.take_game_names(kro), 2);
        assert_eq!(table.get_name(20921), Some("chimera_fulgor"));
        assert_eq!(table.get_name(2337), Some("INVISIBLE"));
        assert!(table.get_name(4252).is_none());
    }

    /// Players are drawn by `sprite_job`, never through this table: a player row would be dead weight.
    #[test]
    fn builtin_table_has_no_player_jobs() {
        use crate::entity::EntityCategory;
        use crate::sprite_path::entity_category_from_job;
        let players: Vec<u16> = BUILTIN_NAME_TABLE
            .iter()
            .map(|&(id, _)| id)
            .filter(|&id| entity_category_from_job(id) == EntityCategory::Player)
            .collect();
        assert!(players.is_empty(), "player rows: {players:?}");
    }
}
