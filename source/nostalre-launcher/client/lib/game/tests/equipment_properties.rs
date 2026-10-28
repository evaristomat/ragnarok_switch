//! Equipment Attributes against the pinned install (`../data/warpgate/gate` beside the checkout); skipped
//! without it. The expected lines are the rig's (`docs/nostalre-kro-exe-equip-window.md`, "Equipment
//! Attributes" 3, the worked example).
use nostalre_formats::grf::GrfArchive;
use nostalre_game::equipment_properties::{EquipmentProperties, SkillNames, Snapshot, WornItem};
use std::path::PathBuf;

fn install() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../data/warpgate/gate");
    dir.join("data.grf").exists().then_some(dir)
}

struct Ids;
impl SkillNames for Ids {
    fn skill_name(&self, skill: i32) -> String {
        format!("skill {skill}")
    }
}

fn load() -> Option<EquipmentProperties> {
    let dir = install()?;
    let grf = GrfArchive::open(&dir.join("data.grf")).unwrap();
    Some(EquipmentProperties::load(&grf, Some(&dir)).expect("loads"))
}

fn worn(item_id: u32, refine: u8, wear: u32) -> WornItem {
    WornItem {
        item_id,
        refine,
        wear,
        ..Default::default()
    }
}

#[test]
fn a_refined_sword_gives_the_rigs_atk_and_matk() {
    let Some(properties) = load() else {
        eprintln!("skipping: no pinned install");
        return;
    };
    let snapshot = Snapshot {
        worn: vec![worn(1101, 2, 0x2)],
        ..Default::default()
    };
    let total = properties.compute(&snapshot);
    let lines = properties.tab_lines(&total, 0, &Ids);
    assert!(lines.contains(&"+29#ATK".to_string()), "{lines:?}");
    assert!(lines.contains(&"+4#MATK".to_string()), "{lines:?}");
}

#[test]
fn a_card_and_a_set_count() {
    let Some(properties) = load() else {
        return;
    };
    // Poring Card in a Sword: LUK +2, Perfect Dodge 10/10
    let mut sword = worn(1101, 0, 0x2);
    sword.cards[0] = 4001;
    let total = properties.compute(&Snapshot {
        worn: vec![sword],
        ..Default::default()
    });
    let stats = properties.tab_lines(&total, 1, &Ids);
    assert!(stats.contains(&"+2#LUK".to_string()), "{stats:?}");
    let ability = properties.tab_lines(&total, 2, &Ids);
    assert!(
        ability.iter().any(|l| l.ends_with("#Perfect Dodge")),
        "{ability:?}"
    );
}

#[test]
fn a_script_reads_the_characters_stats() {
    let Some(properties) = load() else {
        return;
    };
    // Item 2161 gives MATK +10 and MaxHP +800 above 119 base INT (get(35); rAthena's `readparam(bInt)>=120`)
    let mut snapshot = Snapshot {
        worn: vec![worn(2161, 0, 0x10)],
        ..Default::default()
    };
    // STR, AGI, VIT, INT, DEX, LUK
    snapshot.base_stats[3] = 120;
    let rich = properties.tab_lines(&properties.compute(&snapshot), 0, &Ids);
    snapshot.base_stats[3] = 100;
    let poor = properties.tab_lines(&properties.compute(&snapshot), 0, &Ids);
    assert!(rich.contains(&"+800#MAXHP".to_string()), "{rich:?}");
    assert!(!poor.contains(&"+800#MAXHP".to_string()), "{poor:?}");
}

#[test]
fn a_hatched_poring_counts_by_its_relationship() {
    let Some(properties) = load() else {
        return;
    };
    // Item 9001, the Poring egg: LUK +2 and CRI +1 (10 tenths) at relationship 3, LUK +3 at 4, nothing below
    let lines = |intimacy| {
        let snapshot = Snapshot {
            pet_egg: Some(9001),
            pet_intimacy: intimacy,
            ..Default::default()
        };
        properties.tab_lines(&properties.compute(&snapshot), 0, &Ids)
    };
    assert_eq!(lines(800), ["+2#LUK", "+1#CRI"]);
    assert!(lines(950).contains(&"+3#LUK".to_string()));
    // kRO's band 3 ends at 900 inclusive
    assert!(lines(900).contains(&"+2#LUK".to_string()));
    assert!(lines(500).is_empty());
}
