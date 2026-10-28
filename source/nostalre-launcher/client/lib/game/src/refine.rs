//! The refine window's session (rAthena's refine UI, 0x0aa0 to 0x0aa4): the item in it, what the server offers to
//! refine it with, what the player chose, and whether a refine is on its way.

use crate::event::RefineMaterial;
use crate::inventory::InventoryData;
use crate::item::Item;

/// Guards a refine from breaking and downgrading (rAthena's `ITEMID_BLACKSMITH_BLESSING`)
pub const BLACKSMITH_BLESSING: u32 = 6635;
/// The material hexagons the window has
pub const MATERIAL_SLOTS: usize = 4;

/// Why the chosen refine cannot be asked for
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefineBlock {
    NoMaterialChosen,
    NoMaterial,
    NoZeny,
    NoBlessing,
}

/// A refine the server will take: the item, the material, whether blessings guard it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefineRequest {
    pub index: u16,
    pub material: u32,
    pub blessing: bool,
}

#[derive(Debug, Default)]
pub struct RefineSession {
    open: bool,
    item: Option<u16>,
    blessings: u8,
    materials: Vec<RefineMaterial>,
    selected: Option<usize>,
    use_blessing: bool,
    waiting: bool,
    result: Option<i16>,
    /// An item was offered that cannot go in, for the window to say so
    refused: bool,
}

/// Whether `item` can go into the refine window: an unworn equipment the server will list materials for (it
/// ignores unidentified and damaged ones without an answer)
pub fn refinable(item: &Item) -> bool {
    item.is_equipment()
        && !item.is_ammunition()
        && !item.is_equipped()
        && item.is_identified
        && !item.is_damaged
}

impl RefineSession {
    pub fn open(&mut self) {
        *self = Self {
            open: true,
            ..Self::default()
        };
    }

    pub fn close(&mut self) {
        *self = Self::default();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn item(&self) -> Option<u16> {
        self.item
    }

    /// What the server offers for the item at `index` (sent when it goes in, and again after each refine). The
    /// chosen material stays chosen while it is still offered.
    pub fn set_materials(&mut self, index: u16, blessings: u8, mut materials: Vec<RefineMaterial>) {
        materials.truncate(MATERIAL_SLOTS);
        let chosen = self.selected().map(|m| m.item_id);
        if self.item != Some(index) {
            self.use_blessing = false;
        }
        self.item = Some(index);
        self.selected = chosen.and_then(|id| materials.iter().position(|m| m.item_id == id));
        self.materials = materials;
        self.blessings = blessings;
        if blessings == 0 {
            self.use_blessing = false;
        }
    }

    /// The item comes out of the window (Back, or it broke)
    pub fn clear_item(&mut self) {
        let open = self.open;
        *self = Self::default();
        self.open = open;
    }

    /// An item that cannot go in was offered
    pub fn refuse(&mut self) {
        self.refused = true;
    }

    pub fn take_refused(&mut self) -> bool {
        std::mem::take(&mut self.refused)
    }

    pub fn materials(&self) -> &[RefineMaterial] {
        &self.materials
    }

    /// Blacksmith Blessings a refine at this level takes; 0 when none can guard it
    pub fn blessings(&self) -> u8 {
        self.blessings
    }

    pub fn selected(&self) -> Option<&RefineMaterial> {
        self.selected.and_then(|i| self.materials.get(i))
    }

    pub fn selected_slot(&self) -> Option<usize> {
        self.selected
    }

    pub fn select(&mut self, slot: usize) {
        if slot < self.materials.len() {
            self.selected = Some(slot);
        }
    }

    pub fn toggle_blessing(&mut self) {
        if self.blessings > 0 {
            self.use_blessing = !self.use_blessing;
        }
    }

    pub fn use_blessing(&self) -> bool {
        self.use_blessing
    }

    /// The refine the current choice makes, or what it lacks: the material, its zeny, the blessings
    pub fn request(&self, inventory: &InventoryData) -> Result<RefineRequest, RefineBlock> {
        let index = self.item.ok_or(RefineBlock::NoMaterialChosen)?;
        let material = self.selected().ok_or(RefineBlock::NoMaterialChosen)?;
        if inventory.count_of(material.item_id) == 0 {
            return Err(RefineBlock::NoMaterial);
        }
        if i64::from(inventory.zeny) < i64::from(material.zeny) {
            return Err(RefineBlock::NoZeny);
        }
        if self.use_blessing && inventory.count_of(BLACKSMITH_BLESSING) < i32::from(self.blessings)
        {
            return Err(RefineBlock::NoBlessing);
        }
        Ok(RefineRequest {
            index,
            material: material.item_id,
            blessing: self.use_blessing,
        })
    }

    /// A refine was asked for: the window waits for its result
    pub fn start(&mut self) {
        self.waiting = true;
    }

    /// The server's refine result (`ITEMREFINING_*`), for the window to play
    pub fn finish(&mut self, result: i16) {
        if self.open && self.waiting {
            self.waiting = false;
            self.result = Some(result);
        }
    }

    pub fn take_result(&mut self) -> Option<i16> {
        self.result.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::enums::item::ItemType;

    const PHRACON: RefineMaterial = RefineMaterial {
        item_id: 1010,
        chance: 100,
        zeny: 50,
    };
    const ORIDECON: RefineMaterial = RefineMaterial {
        item_id: 984,
        chance: 90,
        zeny: 1000,
    };

    fn inventory(zeny: i32, items: &[(u32, i16)]) -> InventoryData {
        let mut inventory = InventoryData::new();
        inventory.zeny = zeny;
        for (n, (item_id, count)) in items.iter().enumerate() {
            inventory.add_item(Item {
                grade: 0,
                options: Default::default(),
                index: n as u16 + 2,
                item_id: *item_id,
                item_type: ItemType::Etc,
                count: *count,
                is_identified: true,
                is_damaged: false,
                refining_level: 0,
                slot: [0; 4],
                location: 0,
                wear_state: 0,
                name: String::new(),
                resource_name: None,
            });
        }
        inventory
    }

    #[test]
    fn only_an_unworn_identified_whole_equipment_goes_in() {
        let knife = Item {
            item_type: ItemType::Weapon,
            location: 2,
            ..inventory(0, &[(1201, 1)]).all_items()[0].clone()
        };
        assert!(refinable(&knife));
        for item in [
            Item {
                wear_state: 2,
                ..knife.clone()
            },
            Item {
                is_identified: false,
                ..knife.clone()
            },
            Item {
                is_damaged: true,
                ..knife.clone()
            },
            Item {
                item_type: ItemType::Ammo,
                ..knife.clone()
            },
            Item {
                item_type: ItemType::Etc,
                ..knife.clone()
            },
        ] {
            assert!(!refinable(&item), "{item:?}");
        }
    }

    #[test]
    fn a_choice_asks_for_its_material_or_says_what_is_missing() {
        let mut session = RefineSession::default();
        session.open();
        session.set_materials(7, 1, vec![PHRACON, ORIDECON]);
        let carried = inventory(100, &[(1010, 5), (984, 1)]);
        assert_eq!(
            session.request(&carried),
            Err(RefineBlock::NoMaterialChosen)
        );

        session.select(0);
        assert_eq!(
            session.request(&carried),
            Ok(RefineRequest {
                index: 7,
                material: 1010,
                blessing: false
            })
        );
        session.select(1);
        assert_eq!(session.request(&carried), Err(RefineBlock::NoZeny));
        assert_eq!(
            session.request(&inventory(5000, &[])),
            Err(RefineBlock::NoMaterial)
        );

        session.toggle_blessing();
        assert_eq!(
            session.request(&inventory(5000, &[(984, 1)])),
            Err(RefineBlock::NoBlessing)
        );
        assert!(
            session
                .request(&inventory(5000, &[(984, 1), (BLACKSMITH_BLESSING, 1)]))
                .is_ok()
        );
    }

    #[test]
    fn the_choice_survives_the_list_sent_after_a_refine_but_not_another_item() {
        let mut session = RefineSession::default();
        session.open();
        session.set_materials(7, 2, vec![PHRACON, ORIDECON]);
        session.select(1);
        session.toggle_blessing();

        session.set_materials(7, 2, vec![ORIDECON]);
        assert_eq!(session.selected(), Some(&ORIDECON));
        assert!(session.use_blessing());

        session.set_materials(8, 0, vec![PHRACON]);
        assert_eq!(session.selected(), None);
        assert!(!session.use_blessing());
    }

    #[test]
    fn a_refusal_is_said_once_and_material_counts_add_up_every_stack() {
        let mut session = RefineSession::default();
        session.open();
        session.refuse();
        assert!(session.take_refused());
        assert!(!session.take_refused());

        let carried = inventory(0, &[(984, 3), (984, 2), (1010, 1)]);
        assert_eq!(carried.count_of(984), 5);
        assert_eq!(carried.count_of(BLACKSMITH_BLESSING), 0);
    }

    #[test]
    fn a_result_is_kept_only_for_a_refine_the_window_asked_for() {
        let mut session = RefineSession::default();
        session.open();
        session.finish(0);
        assert_eq!(session.take_result(), None);

        session.start();
        session.finish(2);
        session.finish(3); // one result per refine asked for
        assert_eq!(session.take_result(), Some(2));
        assert_eq!(session.take_result(), None);
    }
}
