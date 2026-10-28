//! Garments (`LOOK_ROBE`): the sprite folder each view names, and the frames that draw it behind the body.

use std::collections::HashMap;

use nostalre_formats::grf::GrfArchive;
use nostalre_formats::lua_table::{self, RobeLayers};
use nostalre_resources::lua;

#[derive(Default)]
pub struct RobeTable {
    names: HashMap<u16, String>,
    layers: RobeLayers,
}

impl RobeTable {
    pub fn load(grf: &GrfArchive) -> Self {
        let read = |path: &str| grf.read_file(path).unwrap_or_default();
        let names =
            lua_table::parse_robe_names(&read(lua::ROBE_ID_514_LUB), &read(lua::ROBE_NAME_514_LUB))
                .unwrap_or_else(|e| {
                    tracing::warn!("garment names unreadable: {e}");
                    HashMap::new()
                });
        let layers = lua_table::parse_robe_layers(
            &read(lua::JOB_INHERIT_LIST_514_LUB),
            &read(lua::ROBE_LAYER_JOBS_514_LUB),
            &read(lua::ROBE_LAYERS_FEMALE_514_LUB),
            &read(lua::ROBE_LAYERS_MALE_514_LUB),
        )
        .unwrap_or_else(|e| {
            tracing::warn!("garment layers unreadable (drawn over the body): {e}");
            RobeLayers::default()
        });
        tracing::info!(
            garments = names.len(),
            layer_jobs = layers.behind[1].len(),
            "garment tables loaded"
        );
        Self { names, layers }
    }

    /// The sprite folder of the garment `view` names.
    pub fn name(&self, view: u16) -> Option<&str> {
        self.names.get(&view).map(String::as_str)
    }

    /// Every garment's sprite folder.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.values().map(String::as_str)
    }

    /// Per action, the frames that draw a garment behind the body of `job` (the job drawn, its mount included).
    pub fn behind(&self, job: u16, sex: u8) -> Option<&HashMap<u16, Vec<u16>>> {
        let job = self.layers.jobs.get(&job).copied().unwrap_or(job);
        self.layers.behind[usize::from(sex == 1)].get(&job)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_takes_the_rows_of_the_job_it_inherits() {
        let table = RobeTable {
            names: HashMap::from([(1, "천사날개".to_string())]),
            layers: RobeLayers {
                jobs: HashMap::from([(4008, 7)]),
                behind: [
                    HashMap::new(),
                    HashMap::from([(7, HashMap::from([(0, vec![0, 1, 2])]))]),
                ],
            },
        };
        assert_eq!(table.name(1), Some("천사날개"));
        assert_eq!(
            table.behind(4008, 1).and_then(|a| a.get(&0)),
            Some(&vec![0, 1, 2])
        );
        assert_eq!(table.behind(7, 1), table.behind(4008, 1));
        assert_eq!(table.behind(7, 0), None);
        assert_eq!(table.behind(9, 1), None);
    }
}
