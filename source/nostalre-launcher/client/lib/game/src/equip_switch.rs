//! The swap equipment set: inventory items registered to be worn in place of the worn ones at one press of
//! CHANGE. The item lists carry no mark for them: the client learns the set from the server's list at map load
//! (0x0a9b) and its answers to each add and removal (0x0a98, 0x0a9a), as kRO's does (0xd1c350, 0xd1b620, 0xd1c1b0;
//! `docs/nostalre-kro-exe-equip-window.md`, "Swap Equipment").

/// kRO's client lets CHANGE be pressed once in this long, timed from its own last request (0xc928fe: 0x2710 ms)
pub const CHANGE_COOLDOWN_MS: u64 = 10_000;

#[derive(Debug, Default)]
pub struct EquipSwitch {
    /// Each registered item's inventory index and the locations it takes
    entries: Vec<(u16, u32)>,
    /// When the last CHANGE was sent, in the client's milliseconds
    last_change_ms: Option<u64>,
    /// The panel is open, and opens with the equipment window, until its Cancel (kRO's 0x1602278): the inventory's
    /// Ctrl and double-click adds to the set only then (0x949fc0)
    pub panel_open: bool,
}

impl EquipSwitch {
    /// The server's whole set (0x0a9b): it replaces what was known
    pub fn set_list(&mut self, entries: Vec<(u16, u32)>) {
        self.entries = entries;
    }

    /// An item the server took into the set (0x0a98 flag 0); it takes its locations from whatever held them
    pub fn add(&mut self, index: u16, location: u32) {
        self.entries
            .retain(|&(i, l)| i != index && l & location == 0);
        self.entries.push((index, location));
    }

    /// An item the server took out of the set (0x0a9a)
    pub fn remove(&mut self, index: u16) {
        self.entries.retain(|&(i, _)| i != index);
    }

    pub fn contains(&self, index: u16) -> bool {
        self.entries.iter().any(|&(i, _)| i == index)
    }

    /// The registered item that takes any of `location`
    pub fn item_in(&self, location: u32) -> Option<u16> {
        self.entries
            .iter()
            .find(|&&(_, l)| l & location != 0)
            .map(|&(i, _)| i)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The seconds kRO's client still asks the player to wait before CHANGE, or none when it may be sent
    pub fn change_wait_secs(&self, now_ms: u64) -> Option<u64> {
        let elapsed = now_ms.saturating_sub(self.last_change_ms?);
        (elapsed < CHANGE_COOLDOWN_MS).then(|| 10 - elapsed / 1000)
    }

    /// CHANGE was sent at `now_ms`; a single item's swap does not restart the wait (0xc929d8)
    pub fn changed(&mut self, now_ms: u64) {
        self.last_change_ms = Some(now_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_added_item_takes_its_locations_from_the_one_there() {
        let mut set = EquipSwitch::default();
        set.set_list(vec![(5, 0x2), (6, 0x10)]);
        set.add(7, 0x22);
        assert_eq!(set.item_in(0x2), Some(7));
        assert_eq!(set.item_in(0x20), Some(7));
        assert_eq!(set.item_in(0x10), Some(6));
        assert!(!set.contains(5));
        set.remove(7);
        assert_eq!(set.item_in(0x2), None);
    }

    #[test]
    fn change_waits_ten_seconds_counted_down_whole() {
        let mut set = EquipSwitch::default();
        assert_eq!(set.change_wait_secs(1_000), None);
        set.changed(1_000);
        assert_eq!(set.change_wait_secs(1_000), Some(10));
        assert_eq!(set.change_wait_secs(4_500), Some(7));
        assert_eq!(set.change_wait_secs(11_000), None);
    }
}
