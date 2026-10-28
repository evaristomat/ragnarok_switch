//! The approved integrated handheld menu: one small surface over the live world.
use super::*;
use crate::hud::compact as s;
use nostalre_ui::frame::WindowOrder;

pub(super) fn bounds(w: f32, h: f32, inventory: bool) -> Rect {
    let (pw, ph) = if inventory {
        (
            (w * 0.68).max(500.0).min(w - 24.0),
            (h * 0.66).max(270.0).min(h - 24.0),
        )
    } else {
        (420.0_f32.min(w - 24.0), 300.0_f32.min(h - 24.0))
    };
    Rect::new(((w - pw) * 0.5).floor(), ((h - ph) * 0.5).floor(), pw, ph)
}

fn command_bounds(body: Rect, index: usize) -> Rect {
    let rows = COMMANDS.len().div_ceil(2);
    let stride = (body.h / rows as f32).min(25.0);
    Rect::new(
        body.x + (index / rows) as f32 * (body.w * 0.5 + 3.0),
        body.y + (index % rows) as f32 * stride,
        body.w * 0.5 - 6.0,
        stride - 2.0,
    )
}

impl Menu {
    pub(super) fn build_compact(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let mut events = Vec::new();
        let inventory = self.screen == Screen::Items;
        let travel = self.screen == Screen::GmTravel;
        let r = bounds(
            ui.ctx.screen_width,
            ui.ctx.screen_height,
            inventory || travel,
        );
        let screen = Rect::new(0.0, 0.0, ui.ctx.screen_width, ui.ctx.screen_height);
        ui.ensure_in_z_order_with(MENU_WINDOW_ID, WindowOrder::Foreground);
        ui.enter_window(MENU_WINDOW_ID, screen);
        ui.interact(MENU_BG_ID, screen);
        s::fill(ui, screen, [0.0, 0.0, 0.0, 0.14]);
        s::panel(ui, r);
        let title = if inventory {
            "Inventário"
        } else if travel {
            "GM / Viagens"
        } else {
            "Menu"
        };
        let name = format!(
            "{} · {}",
            ctx.character.name,
            nostalre_game::character::job_class_name(ctx.job_class)
        );
        s::text(
            ui,
            r.x + 14.0,
            s::baseline(ui, r.y + 12.0),
            &s::fit(ui, &name, r.w - 110.0),
            s::DIM,
        );
        s::text(ui, r.x + 14.0, s::baseline(ui, r.y + 27.0), title, s::TEXT);
        let back = Rect::new(r.x + r.w - 67.0, r.y + 17.0, 53.0, 22.0);
        s::text(ui, back.x, s::baseline(ui, back.y), "B  Voltar", s::DIM);
        let mut keys = Keys::read(ui);
        if ui.interact(CRUMB_ID, back).clicked() {
            keys.back = true;
        }
        if keys.back {
            events.extend(self.on_escape(ctx));
            return events;
        }
        let body = Rect::new(r.x + 14.0, r.y + 55.0, r.w - 28.0, r.h - 91.0);
        s::rule(ui, r.x + 14.0, r.y + r.h - 33.0, r.w - 28.0);
        if travel {
            if self.travel.build(ui, ctx, body, &keys, &mut events) {
                self.close();
            }
            s::text(
                ui,
                r.x + 14.0,
                s::baseline(ui, r.y + r.h - 24.0),
                &s::fit(
                    ui,
                    "Direcional  Selecionar / página    A  Abrir / viajar    L/R  Categoria    B  Voltar",
                    r.w - 28.0,
                ),
                s::DIM,
            );
        } else if inventory {
            let picker_keys = keys;
            if self.picker.is_some() {
                keys = Keys::default();
            }
            let mut to_bar = None;
            self.items.build_compact(
                ui,
                ctx,
                body,
                &keys,
                &mut self.textures,
                &mut to_bar,
                &mut events,
            );
            if let Some(what) = to_bar {
                self.picker = Some(BarPicker::new(what));
            }
            let (help, _) = self.items.footer(ctx);
            if self.items.asking() {
                // The amount and its confirmation are isolated from the grid underneath.
                s::panel(ui, body);
                for (i, line) in s::wrap(ui, &help, body.w - 24.0).iter().take(4).enumerate() {
                    s::text(
                        ui,
                        body.x + 12.0,
                        s::baseline(ui, body.y + 18.0 + i as f32 * s::line(ui)),
                        line,
                        s::TEXT,
                    );
                }
                s::text(
                    ui,
                    body.x + 12.0,
                    s::baseline(ui, body.y + body.h - 28.0),
                    "Direcional  Quantidade     A  Confirmar     B  Cancelar",
                    s::DIM,
                );
            }
            if self.picker.is_some() || self.items.asking() {
                ui.ensure_in_z_order_with(MODAL_ID, WindowOrder::Foreground);
                ui.enter_window(MODAL_ID, body);
            }
            if let Some(picker) = &mut self.picker {
                // A newly opened picker never consumes the press that opened it.
                let picker_keys = if to_bar.is_some() {
                    Keys::default()
                } else {
                    picker_keys
                };
                if picker.build(ui, ctx, body, &picker_keys, &mut self.textures, &mut events) {
                    self.picker = None;
                }
            }
            let hints = if self.picker.is_some() {
                "Direcional  Selecionar    A  Atribuir    L/R  Conjunto    B  Voltar"
            } else if self.items.asking() {
                "A  Confirmar descarte    B  Cancelar"
            } else {
                "Direcional  Selecionar    A  Usar/equipar    X  Atalho    Y  Descartar"
            };
            s::text(
                ui,
                r.x + 14.0,
                s::baseline(ui, r.y + r.h - 24.0),
                &s::fit(ui, hints, r.w - 90.0),
                s::TEXT,
            );
            s::right(
                ui,
                r.x + r.w - 14.0,
                s::baseline(ui, r.y + r.h - 24.0),
                &format!("{} z", ctx.character.inventory.zeny),
                s::DIM,
            );
        } else {
            let rows = COMMANDS.len().div_ceil(2);
            if keys.up {
                self.cursor = self.cursor.checked_sub(1).unwrap_or(COMMANDS.len() - 1);
            }
            if keys.down {
                self.cursor = (self.cursor + 1) % COMMANDS.len();
            }
            if keys.left {
                self.cursor = self.cursor.saturating_sub(rows);
            }
            if keys.right {
                self.cursor = (self.cursor + rows).min(COMMANDS.len() - 1);
            }
            if self.confirm.is_some() {
                if keys.enter {
                    self.confirm_answered(true, &mut events);
                    return events;
                }
                let help = self.confirm.unwrap().confirm().unwrap_or_default();
                s::text(ui, body.x, s::baseline(ui, body.y + 20.0), help, s::TEXT);
                s::text(
                    ui,
                    body.x,
                    s::baseline(ui, body.y + 50.0),
                    "A  Confirmar    B  Cancelar",
                    s::DIM,
                );
            } else {
                let mut clicked = None;
                for (i, c) in COMMANDS.iter().enumerate() {
                    let row = command_bounds(body, i);
                    let resp = ui.interact(WidgetId(CMD_BASE_ID + i as u32), row);
                    if self.cursor == i {
                        s::slot(ui, row, true);
                    }
                    s::text(
                        ui,
                        row.x + 7.0,
                        s::baseline(ui, row.y + 4.0),
                        c.label(),
                        s::TEXT,
                    );
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                }
                if keys.enter || clicked.is_some() {
                    self.pick(COMMANDS[clicked.unwrap_or(self.cursor)], ctx, &mut events);
                }
            }
            s::text(
                ui,
                r.x + 14.0,
                s::baseline(ui, r.y + r.h - 24.0),
                "Direcional  Selecionar    A  Abrir    B / +  Fechar",
                s::DIM,
            );
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_commands_including_job_changer_and_exit_fit_inside_two_columns() {
        for (w, h) in [(853.0, 480.0), (1280.0, 720.0), (640.0, 360.0)] {
            let panel = bounds(w, h, false);
            let body = Rect::new(panel.x + 14.0, panel.y + 55.0, panel.w - 28.0, panel.h - 91.0);
            for index in 0..COMMANDS.len() {
                let row = command_bounds(body, index);
                assert!(row.x >= body.x && row.y >= body.y);
                assert!(row.x + row.w <= body.x + body.w && row.y + row.h <= body.y + body.h);
                assert!(row.h >= 18.0);
            }
        }
    }
    #[test]
    fn inventory_is_a_centered_panel_at_handheld_scales() {
        for (w, h) in [(853.0, 480.0), (1280.0, 720.0), (640.0, 360.0)] {
            let r = bounds(w, h, true);
            assert!(r.x >= 12.0 && r.y >= 12.0 && r.x + r.w <= w - 12.0 && r.y + r.h <= h - 12.0);
            assert!(r.w <= w * 0.79 && r.h <= h * 0.76);
        }
    }
}
