//! Controller-friendly GM travel, using the bundled server's verified Warper destinations.
use super::{ROW_BASE_ID, TAB_BASE_ID, frame::Keys, travel_catalog::GROUPS};
use crate::{BuildCtx, hud::compact as s};
use nostalre_game::event::GameEvent;
use nostalre_ui::{
    context::PhysicalKeyCode,
    frame::{UiFrame, WidgetId},
    rect::Rect,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Destination {
    pub label: &'static str,
    pub map: &'static str,
    pub x: u16,
    pub y: u16,
    pub go: Option<u8>,
}
impl Destination {
    fn title(self) -> String {
        GROUPS
            .iter()
            .find(|g| {
                g.destinations
                    .iter()
                    .any(|d| d.map == self.map && d.x == self.x && d.y == self.y)
            })
            .filter(|g| g.label != self.label)
            .map_or_else(
                || self.label.to_string(),
                |g| format!("{} · {}", g.label, self.label),
            )
    }
    fn command(self) -> String {
        self.go.map_or_else(
            || format!("@warp {} {} {}", self.map, self.x, self.y),
            |id| format!("@go {id}"),
        )
    }
}
pub(super) struct TravelGroup {
    pub category: usize,
    pub label: &'static str,
    pub destinations: &'static [Destination],
}

#[derive(Default)]
pub(super) struct Travel {
    category: usize,
    cursors: [usize; 4],
    group: Option<usize>,
    floor: usize,
    offset: usize,
    recent: Vec<Destination>,
}
const TABS: [&str; 4] = ["Cidades", "Dungeons", "Campos", "Recentes"];

impl Travel {
    pub fn back(&mut self) -> bool {
        if self.group.take().is_some() {
            self.offset = 0;
            true
        } else {
            false
        }
    }
    pub fn leave(&mut self) {
        self.group = None;
        self.offset = 0;
    }
    fn groups(&self) -> Vec<usize> {
        GROUPS
            .iter()
            .enumerate()
            .filter(|(_, g)| g.category == self.category)
            .map(|(i, _)| i)
            .collect()
    }
    fn count(&self) -> usize {
        if let Some(index) = self.group {
            GROUPS[index].destinations.len()
        } else if self.category == 3 {
            self.recent.len()
        } else {
            self.groups().len()
        }
    }
    fn cursor(&self) -> usize {
        if self.group.is_some() {
            self.floor
        } else {
            self.cursors[self.category]
        }
    }
    fn set_cursor(&mut self, index: usize) {
        let index = index.min(self.count().saturating_sub(1));
        if self.group.is_some() {
            self.floor = index;
        } else {
            self.cursors[self.category] = index;
        }
    }
    fn picked_group(&self) -> Option<usize> {
        self.group
            .or_else(|| self.groups().get(self.cursor()).copied())
    }
    fn destination(&self) -> Option<Destination> {
        if self.category == 3 {
            self.recent.get(self.cursor()).copied()
        } else if let Some(group) = self.group {
            GROUPS[group].destinations.get(self.floor).copied()
        } else {
            let g = GROUPS.get(self.picked_group()?)?;
            (g.destinations.len() == 1).then(|| g.destinations[0])
        }
    }
    /// Enter opens a floor list or submits one normal chat command to the server.
    fn activate(&mut self) -> Option<GameEvent> {
        if let Some(dest) = self.destination() {
            self.recent
                .retain(|d| d.map != dest.map || d.x != dest.x || d.y != dest.y);
            self.recent.insert(0, dest);
            self.recent.truncate(8);
            return Some(GameEvent::RequestSendChat {
                message: dest.command(),
            });
        }
        if let Some(group) = self.picked_group() {
            self.group = Some(group);
            self.floor = 0;
            self.offset = 0;
        }
        None
    }
    pub fn build(
        &mut self,
        ui: &mut UiFrame,
        ctx: &BuildCtx,
        area: Rect,
        keys: &Keys,
        events: &mut Vec<GameEvent>,
    ) -> bool {
        let mut tx = area.x;
        s::text(ui, tx, s::baseline(ui, area.y), "L", s::DIM);
        tx += 17.0;
        let mut category = self.category;
        for (i, label) in TABS.iter().enumerate() {
            let w = s::measure(ui, label);
            let r = Rect::new(tx, area.y, w, 20.0);
            if ui.interact(WidgetId(TAB_BASE_ID + i as u32), r).clicked() {
                category = i;
            }
            s::text(
                ui,
                tx,
                s::baseline(ui, area.y),
                label,
                if self.category == i { s::TEXT } else { s::DIM },
            );
            if self.category == i {
                s::fill(ui, Rect::new(tx, area.y + 17.0, w, 1.3), s::ACCENT);
            }
            tx += w + 16.0;
        }
        s::text(ui, tx, s::baseline(ui, area.y), "R", s::DIM);
        if keys.prev_tab {
            category = (self.category + 3) % 4;
        }
        if keys.next_tab {
            category = (self.category + 1) % 4;
        }
        if category != self.category {
            self.category = category;
            self.group = None;
            self.floor = 0;
            self.offset = 0;
        }
        s::rule(ui, area.x, area.y + 23.0, area.w);
        let list = Rect::new(
            area.x,
            area.y + 33.0,
            (area.w * 0.55).floor(),
            area.h - 49.0,
        );
        let rows = (list.h / 22.0).floor().max(1.0) as usize;
        let len = self.count();
        let mut index = self.cursor();
        if keys.up {
            index = if index == 0 {
                len.saturating_sub(1)
            } else {
                index - 1
            };
        }
        if keys.down && len > 0 {
            index = (index + 1) % len;
        }
        if keys.left || ui.ctx.pressed_codes.contains(&PhysicalKeyCode::Home) {
            index = index.saturating_sub(rows);
        }
        if keys.right || ui.ctx.pressed_codes.contains(&PhysicalKeyCode::End) {
            index = (index + rows).min(len.saturating_sub(1));
        }
        let scroll = ui.take_scroll(list);
        if scroll > 0.0 {
            index = index.saturating_sub(1);
        }
        if scroll < 0.0 {
            index = (index + 1).min(len.saturating_sub(1));
        }
        self.set_cursor(index);
        let index = self.cursor();
        if index < self.offset {
            self.offset = index;
        }
        if index >= self.offset + rows {
            self.offset = index + 1 - rows;
        }
        self.offset = self.offset.min(len.saturating_sub(rows));
        let groups = self.groups();
        let mut activate = keys.enter;
        for (slot, i) in (self.offset..len).take(rows).enumerate() {
            let r = Rect::new(list.x, list.y + slot as f32 * 22.0, list.w, 21.0);
            let response = ui.interact(WidgetId(ROW_BASE_ID + slot as u32), r);
            if i == index {
                s::slot(ui, r, true);
            }
            let (label, note) = if let Some(group) = self.group {
                (
                    GROUPS[group].destinations[i].label.to_string(),
                    String::new(),
                )
            } else if self.category == 3 {
                (self.recent[i].title(), String::new())
            } else {
                let g = &GROUPS[groups[i]];
                (
                    g.label.to_string(),
                    if g.destinations.len() > 1 {
                        format!("{}  >", g.destinations.len())
                    } else {
                        String::new()
                    },
                )
            };
            let reserve = s::measure(ui, &note) + 16.0;
            s::text(
                ui,
                r.x + 6.0,
                s::baseline(ui, r.y + 3.0),
                &s::fit(ui, &label, r.w - 12.0 - reserve),
                s::TEXT,
            );
            s::right(
                ui,
                r.x + r.w - 5.0,
                s::baseline(ui, r.y + 3.0),
                &note,
                s::DIM,
            );
            if response.clicked() {
                self.set_cursor(i);
            }
            if response.double_clicked() {
                self.set_cursor(i);
                activate = true;
            }
        }
        let right = Rect::new(
            list.x + list.w + 16.0,
            list.y,
            area.w - list.w - 16.0,
            list.h,
        );
        s::fill(
            ui,
            Rect::new(right.x - 8.0, right.y, 0.7, right.h),
            [0.55, 0.7, 0.8, 0.25],
        );
        if let Some(dest) = self.destination() {
            let heading = dest.title();
            let mut y = right.y;
            for line in s::wrap(ui, &heading, right.w).iter().take(2) {
                s::text(ui, right.x, s::baseline(ui, y), line, s::TEXT);
                y += s::line(ui);
            }
            y += 8.0;
            s::text(
                ui,
                right.x,
                s::baseline(ui, y),
                &s::fit(ui, dest.label, right.w),
                s::DIM,
            );
            y += s::line(ui) + 8.0;
            let map = crate::hud::place_name(ctx.data, dest.map);
            for line in s::wrap(ui, map, right.w).iter().take(2) {
                s::text(ui, right.x, s::baseline(ui, y), line, s::DIM);
                y += s::line(ui);
            }
            y += 10.0;
            s::text(
                ui,
                right.x,
                s::baseline(ui, y),
                &s::fit(ui, &dest.command(), right.w),
                s::ACCENT,
            );
            let button = Rect::new(right.x, right.y + right.h - 29.0, right.w, 25.0);
            s::slot(ui, button, false);
            s::center(
                ui,
                button.x + button.w * 0.5,
                s::baseline(ui, button.y + 5.0),
                "A  Viajar",
                s::TEXT,
            );
            if ui.interact(WidgetId(ROW_BASE_ID + 200), button).clicked() {
                activate = true;
            }
        } else if let Some(group) = self.picked_group().filter(|_| self.category != 3) {
            let g = &GROUPS[group];
            let mut y = right.y;
            for line in s::wrap(ui, g.label, right.w).iter().take(2) {
                s::text(ui, right.x, s::baseline(ui, y), line, s::TEXT);
                y += s::line(ui);
            }
            y += 10.0;
            let kind = if self.category == 1 {
                "andares / áreas"
            } else {
                "áreas"
            };
            s::text(
                ui,
                right.x,
                s::baseline(ui, y),
                &format!("{} {kind}", g.destinations.len()),
                s::DIM,
            );
            y += s::line(ui) + 12.0;
            for line in s::wrap(ui, "Escolha o destino dentro desta região.", right.w) {
                s::text(ui, right.x, s::baseline(ui, y), &line, s::DIM);
                y += s::line(ui);
            }
            let button = Rect::new(right.x, right.y + right.h - 29.0, right.w, 25.0);
            s::slot(ui, button, false);
            s::center(
                ui,
                button.x + button.w * 0.5,
                s::baseline(ui, button.y + 5.0),
                "A  Escolher área",
                s::TEXT,
            );
            if ui.interact(WidgetId(ROW_BASE_ID + 200), button).clicked() {
                activate = true;
            }
        } else {
            for (i, line) in s::wrap(ui, "Seus últimos destinos aparecerão aqui.", right.w)
                .iter()
                .enumerate()
            {
                s::text(
                    ui,
                    right.x,
                    s::baseline(ui, right.y + i as f32 * s::line(ui)),
                    line,
                    s::DIM,
                );
            }
        }
        let path = self
            .group
            .map(|g| format!("{} / {}", TABS[self.category], GROUPS[g].label))
            .unwrap_or_else(|| TABS[self.category].into());
        s::text(
            ui,
            area.x,
            s::baseline(ui, area.y + area.h - 12.0),
            &s::fit(ui, &path, area.w - 65.0),
            s::DIM,
        );
        s::right(
            ui,
            area.x + area.w,
            s::baseline(ui, area.y + area.h - 12.0),
            &format!("{} / {}", if len == 0 { 0 } else { index + 1 }, len),
            s::DIM,
        );
        if activate && let Some(event) = self.activate() {
            events.push(event);
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cities_use_real_go_ids_and_dungeons_require_a_floor() {
        let mut t = Travel::default();
        t.cursors[0] = t
            .groups()
            .iter()
            .position(|&i| GROUPS[i].label == "Izlude")
            .unwrap();
        assert!(
            matches!(t.activate(),Some(GameEvent::RequestSendChat{message}) if message=="@go 5")
        );
        t.category = 1;
        t.cursors[1] = t
            .groups()
            .iter()
            .position(|&i| GROUPS[i].label == "Caverna de Byalan")
            .unwrap();
        assert!(t.activate().is_none());
        assert!(t.group.is_some());
        t.floor = 2;
        assert!(
            matches!(t.activate(),Some(GameEvent::RequestSendChat{message}) if message=="@warp iz_dun02 236 204")
        );
        assert!(t.back());
        assert!(t.group.is_none());
        t.category = 3;
        t.cursors[3] = 0;
        assert_eq!(t.destination().unwrap().map, "iz_dun02");
    }
    #[test]
    fn empty_recents_never_send_a_command_and_recents_deduplicate() {
        let mut t = Travel {
            category: 3,
            ..Travel::default()
        };
        assert!(t.activate().is_none());
        t.category = 0;
        for _ in 0..12 {
            assert!(t.activate().is_some());
        }
        assert_eq!(t.recent.len(), 1);
        for g in GROUPS {
            for d in g.destinations {
                assert!(d.x > 0 && d.y > 0);
                assert!(d.map.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
            }
        }
    }
}
