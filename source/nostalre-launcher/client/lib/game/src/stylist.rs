//! The stylist's session (rAthena's stylist UI: 0x0ae2 type 1, 0x0afc, 0x0a47, 0x0a48): what the player picked in
//! each of kRO's styling shop lists, and whether a change is on its way.

pub use nostalre_formats::lua_table::{StyleGoods, StylingShopTable};
use std::collections::HashMap;

/// The looks a stylist request carries, in its order (0x0afc)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleSlot {
    HeadPalette,
    HeadStyle,
    BodyPalette,
    TopAccessory,
    MidAccessory,
    BottomAccessory,
    BodyStyle,
}

impl StyleSlot {
    pub const ALL: [StyleSlot; 7] = [
        StyleSlot::HeadPalette,
        StyleSlot::HeadStyle,
        StyleSlot::BodyPalette,
        StyleSlot::TopAccessory,
        StyleSlot::MidAccessory,
        StyleSlot::BottomAccessory,
        StyleSlot::BodyStyle,
    ];

    /// The styling shop list this slot picks from; a Doram has lists of its own for hair and clothes
    pub fn list(self, doram: bool) -> &'static str {
        match (self, doram) {
            (StyleSlot::HeadPalette, _) => "HeadPalette",
            (StyleSlot::HeadStyle, false) => "HeadStyle",
            (StyleSlot::HeadStyle, true) => "DoramHeadStyle",
            (StyleSlot::BodyPalette, false) => "BodyPalette",
            (StyleSlot::BodyPalette, true) => "DoramBodyPalette",
            (StyleSlot::TopAccessory, _) => "TopAccessory",
            (StyleSlot::MidAccessory, _) => "MidAccessory",
            (StyleSlot::BottomAccessory, _) => "BottomAccessory",
            (StyleSlot::BodyStyle, false) => "SecondCostume",
            (StyleSlot::BodyStyle, true) => "DoramSecondCostume",
        }
    }
}

/// What a stylist request sends: per slot, the chosen entry's place in its list from 1; 0 leaves that look. A hair
/// colour of -1 is rAthena's "back to colour 0".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StyleChoice(pub [i16; 7]);

/// The hair colour pick that sends the hair back to colour 0 (rAthena's `Index: -1`)
pub const RESET_COLOR: i16 = -1;

impl StyleChoice {
    pub fn get(&self, slot: StyleSlot) -> i16 {
        self.0[slot as usize]
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&pick| pick == 0)
    }
}

/// The looks the preview draws: the player's own, each pick's in its place
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StyleLook {
    pub hair: u16,
    pub hair_color: u16,
    pub cloth_color: u16,
    pub head_top: u16,
    pub head_mid: u16,
    pub head_bottom: u16,
}

#[derive(Debug, Default)]
pub struct StylistSession {
    open: bool,
    choice: StyleChoice,
    waiting: bool,
    result: Option<bool>,
    /// The player's look now, kept up by the preview: a pick of what the player already has is no change
    current: StyleLook,
}

impl StylistSession {
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

    pub fn choice(&self) -> StyleChoice {
        self.choice
    }

    pub fn set_current(&mut self, look: StyleLook) {
        self.current = look;
    }

    /// Whether `goods` in the slot's list is the look the player has now (hairstyle and colours; a headgear is
    /// mailed, so another of one worn is still a purchase)
    pub fn is_current(&self, slot: StyleSlot, goods: &StyleGoods) -> bool {
        let now = match slot {
            StyleSlot::HeadStyle => self.current.hair,
            StyleSlot::HeadPalette => self.current.hair_color,
            StyleSlot::BodyPalette => self.current.cloth_color,
            _ => return false,
        };
        u16::try_from(goods.goods) == Ok(now)
    }

    /// Picks the entry at `place` (from 1) of the slot's list, or clears the slot with 0
    pub fn pick(&mut self, slot: StyleSlot, place: i16) {
        self.choice.0[slot as usize] = place;
    }

    /// Lets go of every pick the player now has: a refused purchase may have made some of its changes (the server
    /// makes them one by one and stops at the first it cannot pay), which another purchase would charge again. A
    /// headgear is mailed and cannot be seen this way.
    pub fn drop_current_picks(&mut self, table: &StylingShopTable, doram: bool) {
        for slot in StyleSlot::ALL {
            if self
                .picked(table, slot, doram)
                .is_some_and(|goods| self.is_current(slot, goods))
            {
                self.pick(slot, 0);
            }
        }
    }

    /// The styling shop entry a slot's pick names, for its look and cost
    pub fn picked<'t>(
        &self,
        table: &'t StylingShopTable,
        slot: StyleSlot,
        doram: bool,
    ) -> Option<&'t StyleGoods> {
        let place = usize::try_from(self.choice.get(slot))
            .ok()?
            .checked_sub(1)?;
        table.lists.get(slot.list(doram))?.get(place)
    }

    /// `base` with every pick in its place; a headgear is drawn by its view (`headgear_view`, item id to view)
    pub fn preview(
        &self,
        base: StyleLook,
        table: &StylingShopTable,
        doram: bool,
        headgear_view: &HashMap<u32, u16>,
    ) -> StyleLook {
        let goods = |slot| {
            self.picked(table, slot, doram)
                .and_then(|g| u16::try_from(g.goods).ok())
        };
        let view = |slot| {
            self.picked(table, slot, doram)
                .and_then(|g| headgear_view.get(&g.goods).copied())
        };
        let mut look = base;
        if let Some(hair) = goods(StyleSlot::HeadStyle) {
            look.hair = hair;
        }
        if self.choice.get(StyleSlot::HeadPalette) == RESET_COLOR {
            look.hair_color = 0;
        } else if let Some(color) = goods(StyleSlot::HeadPalette) {
            look.hair_color = color;
        }
        if let Some(color) = goods(StyleSlot::BodyPalette) {
            look.cloth_color = color;
        }
        for (slot, field) in [
            (StyleSlot::TopAccessory, &mut look.head_top),
            (StyleSlot::MidAccessory, &mut look.head_mid),
            (StyleSlot::BottomAccessory, &mut look.head_bottom),
        ] {
            if let Some(view) = view(slot) {
                *field = view;
            }
        }
        look
    }

    /// A change was asked for: the window waits for its answer
    pub fn start(&mut self) {
        self.waiting = true;
    }

    pub fn is_waiting(&self) -> bool {
        self.waiting
    }

    /// The server's answer (0x0a47), for the window to say; it closes after a change made, as kRO's does, and
    /// keeps the picks after a refused one
    pub fn finish(&mut self, success: bool) {
        if self.waiting {
            self.waiting = false;
            self.result = Some(success);
        }
    }

    pub fn take_result(&mut self) -> Option<bool> {
        self.result.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn goods(goods: u32) -> StyleGoods {
        StyleGoods {
            goods,
            zeny: 100_000,
            item: None,
            box_item: None,
            doram_too: true,
            dyeable: true,
        }
    }

    #[test]
    fn a_pick_is_a_place_in_its_list_and_a_doram_picks_from_its_own() {
        let mut table = StylingShopTable::default();
        table
            .lists
            .insert("HeadStyle".into(), vec![goods(1), goods(2), goods(3)]);
        table.lists.insert("DoramHeadStyle".into(), vec![goods(9)]);
        let mut session = StylistSession::default();
        session.open();
        assert!(session.choice().is_empty());

        session.pick(StyleSlot::HeadStyle, 2);
        assert_eq!(session.choice().0, [0, 2, 0, 0, 0, 0, 0]);
        assert_eq!(
            session
                .picked(&table, StyleSlot::HeadStyle, false)
                .map(|g| g.goods),
            Some(2)
        );
        assert_eq!(session.picked(&table, StyleSlot::HeadStyle, true), None);
        session.pick(StyleSlot::HeadStyle, 1);
        assert_eq!(
            session
                .picked(&table, StyleSlot::HeadStyle, true)
                .map(|g| g.goods),
            Some(9)
        );

        session.pick(StyleSlot::HeadPalette, RESET_COLOR);
        assert_eq!(
            session.picked(&table, StyleSlot::HeadPalette, false),
            None,
            "the reset colour is no entry"
        );
    }

    #[test]
    fn the_preview_is_the_players_look_with_each_pick_in_its_place() {
        let mut table = StylingShopTable::default();
        table.lists.insert("HeadStyle".into(), vec![goods(12)]);
        table.lists.insert("HeadPalette".into(), vec![goods(4)]);
        table
            .lists
            .insert("MidAccessory".into(), vec![goods(2201), goods(2239)]);
        let headgear_view = HashMap::from([(2239, 47)]);
        let base = StyleLook {
            hair: 3,
            hair_color: 2,
            cloth_color: 1,
            head_top: 5,
            ..StyleLook::default()
        };
        let mut session = StylistSession::default();
        session.open();
        assert_eq!(session.preview(base, &table, false, &headgear_view), base);

        session.pick(StyleSlot::HeadStyle, 1);
        session.pick(StyleSlot::HeadPalette, 1);
        session.pick(StyleSlot::MidAccessory, 2);
        let look = session.preview(base, &table, false, &headgear_view);
        assert_eq!(
            (
                look.hair,
                look.hair_color,
                look.head_mid,
                look.head_top,
                look.cloth_color
            ),
            (12, 4, 47, 5, 1)
        );
        session.pick(StyleSlot::HeadPalette, RESET_COLOR);
        assert_eq!(
            session
                .preview(base, &table, false, &headgear_view)
                .hair_color,
            0
        );
    }

    #[test]
    fn the_look_the_player_has_is_current_and_a_headgear_never_is() {
        let mut session = StylistSession::default();
        session.set_current(StyleLook {
            hair: 12,
            hair_color: 4,
            head_mid: 47,
            ..StyleLook::default()
        });
        assert!(session.is_current(StyleSlot::HeadStyle, &goods(12)));
        assert!(!session.is_current(StyleSlot::HeadStyle, &goods(13)));
        assert!(session.is_current(StyleSlot::HeadPalette, &goods(4)));
        assert!(!session.is_current(StyleSlot::MidAccessory, &goods(47)));
    }

    #[test]
    fn a_pick_the_player_now_has_is_let_go() {
        let mut table = StylingShopTable::default();
        table.lists.insert("HeadStyle".into(), vec![goods(12)]);
        table.lists.insert("BodyPalette".into(), vec![goods(3)]);
        let mut session = StylistSession::default();
        session.open();
        session.pick(StyleSlot::HeadStyle, 1);
        session.pick(StyleSlot::BodyPalette, 1);
        // the hairstyle went through before the refusal, the clothes colour did not
        session.set_current(StyleLook {
            hair: 12,
            cloth_color: 1,
            ..StyleLook::default()
        });
        session.drop_current_picks(&table, false);
        assert_eq!(session.choice().get(StyleSlot::HeadStyle), 0);
        assert_eq!(session.choice().get(StyleSlot::BodyPalette), 1);
    }

    #[test]
    fn an_answer_is_kept_for_the_window_and_a_refusal_keeps_the_picks() {
        let mut session = StylistSession::default();
        session.open();
        session.pick(StyleSlot::MidAccessory, 3);
        session.finish(true);
        assert_eq!(session.take_result(), None, "no answer without a request");
        session.start();
        session.finish(false);
        assert!(session.is_open() && !session.is_waiting());
        assert_eq!(session.take_result(), Some(false));
        assert_eq!(session.choice().get(StyleSlot::MidAccessory), 3);

        session.start();
        session.finish(true);
        assert_eq!(session.take_result(), Some(true));
    }
}
