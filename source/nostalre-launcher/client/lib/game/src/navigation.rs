//! An NPC's navigation guide (0x08e2, `clif_navigateTo`, the `navigateto` script command): where it points, as
//! rAthena fills it and kRO's handler (0x7ff390) reads it before its navigation window (0x685be0) takes over

/// What the guide points at: rAthena's three kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavTarget {
    /// A cell of a map (0)
    Cell { x: i16, y: i16 },
    /// A map, no cell (1; rAthena: "will fail if you are already on the map")
    Map,
    /// A monster on a map (3)
    Monster { mob_id: u16 },
}

impl NavTarget {
    /// rAthena's kinds, with what each carries; any other is none it sends
    pub fn from_packet(kind: u8, x: i16, y: i16, mob_id: u16) -> Option<Self> {
        match kind {
            0 => Some(Self::Cell { x, y }),
            1 => Some(Self::Map),
            3 => Some(Self::Monster { mob_id }),
            _ => None,
        }
    }
}

/// The ways of travel the guide may use, from `flag`'s decimal digits as kRO's handler splits them (0x7ff3b8): the
/// hundreds a Kafra, the tens a scroll, the ones an airship (rAthena's `NAV_KAFRA_ONLY` 100, `NAV_SCROLL_ONLY` 10,
/// `NAV_AIRSHIP_ONLY` 1; a digit above 1 counts as 1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NavServices {
    pub kafra: bool,
    pub scroll: bool,
    pub airship: bool,
}

impl NavServices {
    pub fn from_flag(flag: u8) -> Self {
        let kafra = flag >= 100;
        let rest = if kafra { flag - 100 } else { flag };
        let scroll = rest >= 10;
        let rest = if scroll { rest - 10 } else { rest };
        Self {
            kafra,
            scroll,
            airship: rest >= 1,
        }
    }
}

/// The guide the server last gave
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Navigation {
    pub map: String,
    pub target: NavTarget,
    pub services: NavServices,
    /// rAthena's `hideWindow`: kRO opens no navigation window (`navigateto`'s default)
    pub hide_window: bool,
}

/// How near the player must stand to a guide's cell for it to be done (ours: kRO's own end is its window's)
const ARRIVED_CELLS: u16 = 3;

impl Navigation {
    /// The guide's cell on `map`, when it points at one there
    pub fn cell_on(&self, map: &str) -> Option<(i16, i16)> {
        match self.target {
            NavTarget::Cell { x, y } if crate::same_map(&self.map, map) => Some((x, y)),
            _ => None,
        }
    }

    /// Whether the player, standing on `map` at `cell`, has reached the guide's cell
    pub fn reached(&self, map: &str, cell: (u16, u16)) -> bool {
        self.cell_on(map).is_some_and(|(x, y)| {
            cell.0
                .abs_diff(x.max(0) as u16)
                .max(cell.1.abs_diff(y.max(0) as u16))
                <= ARRIVED_CELLS
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kinds_rathena_sends_and_no_other() {
        assert_eq!(
            NavTarget::from_packet(0, 252, 297, 0),
            Some(NavTarget::Cell { x: 252, y: 297 })
        );
        assert_eq!(NavTarget::from_packet(1, 0, 0, 0), Some(NavTarget::Map));
        assert_eq!(
            NavTarget::from_packet(3, 0, 0, 1002),
            Some(NavTarget::Monster { mob_id: 1002 })
        );
        assert_eq!(NavTarget::from_packet(2, 1, 1, 0), None);
    }

    #[test]
    fn the_flag_splits_by_its_digits() {
        let s = NavServices::from_flag;
        assert_eq!(s(0), NavServices::default());
        assert_eq!(
            s(101),
            NavServices {
                kafra: true,
                scroll: false,
                airship: true
            },
            "NAV_KAFRA_AND_AIRSHIP, navigateto's default"
        );
        assert_eq!(
            s(110),
            NavServices {
                kafra: true,
                scroll: true,
                airship: false
            }
        );
        assert_eq!(
            s(255),
            NavServices {
                kafra: true,
                scroll: true,
                airship: true
            },
            "111 to 255 are all"
        );
        assert_eq!(s(19), s(11), "a digit above 1 counts as 1");
    }

    #[test]
    fn same_map_keys_names_as_map_key_does_without_building_keys() {
        for (a, b) in [
            ("payon", "PAYON.gat"),
            ("payon.rsw", " payon "),
            ("prt_fild08", "prt_fild08.GAT"),
        ] {
            assert!(crate::same_map(a, b), "{a} {b}");
            assert_eq!(crate::map_key(a), crate::map_key(b));
        }
        assert!(!crate::same_map("payon", "payon2"));
        assert!(!crate::same_map(".gat", "x"));
        assert!(
            !crate::same_map("가.gat", "가가"),
            "no panic on a multi-byte name"
        );
    }

    #[test]
    fn a_guide_is_reached_within_three_cells_of_its_cell_on_its_map() {
        let nav = Navigation {
            map: "payon".into(),
            target: NavTarget::Cell { x: 252, y: 297 },
            services: NavServices::default(),
            hide_window: true,
        };
        assert_eq!(nav.cell_on("payon.gat"), Some((252, 297)));
        assert!(nav.reached("payon", (250, 299)));
        assert!(!nav.reached("payon", (248, 297)));
        assert!(!nav.reached("prontera", (252, 297)));
    }
}
