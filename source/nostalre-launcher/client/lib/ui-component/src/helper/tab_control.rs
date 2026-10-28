//! kRO's row of tabs over a window's body (`UITabControl`, the 2025 exe's 0x837010), drawn in code as kRO's is:
//! style 1 (tabs on top), every tab as wide as the widest label plus 15 (0x85f1f0), a label too long for its
//! share of the parent's width cut with "..." (0x864690, 0xa24ee0)
use crate::helper::window_chrome::fill;
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

/// The control's height: its row argument plus one (0x85fca0; the equipment window passes 0x11)
pub const TAB_H: f32 = 18.0;
/// Added to the label's width (the third argument, 0xf)
const LABEL_PAD: f32 = 15.0;
/// kRO's defaults with no colours given (0x844710): white tabs, grey edges, black labels
const FILL: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
pub const EDGE: [f32; 4] = [195.0 / 255.0, 195.0 / 255.0, 195.0 / 255.0, 1.0];
const LABEL: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
/// kRO's label font height, from which its label sits 3 (chosen) or 4 pixels down in the tab
const FONT_H: f32 = 12.0;

pub struct Tab {
    pub label: &'static str,
    /// A tab whose window part is not built yet: drawn, never chosen
    pub enabled: bool,
}

/// The label kRO shows: whole when it fits `limit`, else cut from the end with "..." until it does
pub fn fitted_label(label: &str, limit: f32, measure: impl Fn(&str) -> f32) -> String {
    if measure(label) <= limit {
        return label.to_string();
    }
    let mut chars: Vec<char> = label.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let cut = format!("{}...", chars.iter().collect::<String>());
        if measure(&cut) <= limit {
            return cut;
        }
    }
    "...".to_string()
}

/// One tab's width: the widest fitted label plus the pad, the same for all (0x85f1f0)
pub fn tab_width(label_widths: &[f32]) -> f32 {
    label_widths
        .iter()
        .map(|w| w.ceil() + LABEL_PAD)
        .fold(0.0, f32::max)
}

/// The labels as shown and the tabs' width, worked out once for a parent width and font: building them each
/// frame would allocate every frame the window is open
#[derive(Default)]
pub struct TabControl {
    /// The parent's width, the font's line height (bits) and the tab count the layout was made for
    made_for: Option<(u32, u32, usize)>,
    labels: Vec<String>,
    label_widths: Vec<f32>,
    tab_w: f32,
}

impl TabControl {
    fn lay_out(&mut self, ui: &UiFrame, parent_w: f32, tabs: &[Tab]) {
        let key = (
            parent_w.to_bits(),
            ui.atlas.line_height.to_bits(),
            tabs.len(),
        );
        if self.made_for == Some(key) {
            return;
        }
        let limit = parent_w / tabs.len().max(1) as f32 - LABEL_PAD;
        let measure = |s: &str| ui.atlas.measure_text(s);
        self.labels = tabs
            .iter()
            .map(|t| fitted_label(t.label, limit, measure))
            .collect();
        self.label_widths = self.labels.iter().map(|l| measure(l).ceil()).collect();
        self.tab_w = tab_width(&self.label_widths);
        self.made_for = Some(key);
    }

    /// Draws the tabs at (`x`, `y`) of a parent `parent_w` wide; returns the tab clicked, if any, and the control's
    /// width. Neighbours share their edge (each starts one pixel inside the last); the chosen tab is open at the
    /// bottom, the others stand one pixel lower with a line under them (0x857910, 0x847930, 0x85b1e0, 0x8474f0).
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        &mut self,
        ui: &mut UiFrame,
        base_id: u32,
        x: f32,
        y: f32,
        parent_w: f32,
        tabs: &[Tab],
        selected: usize,
    ) -> (Option<usize>, f32) {
        self.lay_out(ui, parent_w, tabs);
        let (w, h) = (self.tab_w, TAB_H);
        let mut clicked = None;
        for (i, tab) in tabs.iter().enumerate() {
            let label = self.labels[i].as_str();
            let tx = x + i as f32 * (w - 1.0);
            let chosen = i == selected;
            let top = if chosen { y } else { y + 1.0 };
            fill(ui, tx + 1.0, top + 1.0, w - 2.0, h - 1.0, FILL);
            fill(ui, tx + 2.0, top, w - 4.0, 1.0, EDGE);
            fill(ui, tx + 1.0, top + 1.0, 1.0, 1.0, EDGE);
            fill(ui, tx, top + 2.0, 1.0, h - 2.0, EDGE);
            fill(ui, tx + w - 2.0, top + 1.0, 1.0, 1.0, EDGE);
            fill(ui, tx + w - 1.0, top + 2.0, 1.0, h - 2.0, EDGE);
            if !chosen {
                fill(ui, tx, y + h - 1.0, w, 1.0, EDGE);
            }
            let gap = w - self.label_widths[i];
            let label_x = tx + (gap / 2.0).floor() + gap.rem_euclid(2.0).floor();
            let label_top = ((h - FONT_H) / 2.0).floor() + if chosen { 0.0 } else { 1.0 };
            ui.text(label_x, y + label_top + ui.atlas.ascent, label, LABEL);

            let response = ui.interact(WidgetId(base_id + i as u32), Rect::new(tx, y, w, h));
            if response.hovered() {
                if tab.enabled && !chosen {
                    ui.any_interactive_hovered = true;
                }
                if label != tab.label {
                    ui.tooltip(tx, y + h + 2.0, tab.label);
                }
            }
            if tab.enabled && !chosen && response.clicked() {
                clicked = Some(i);
            }
        }
        (clicked, tabs.len() as f32 * (w - 1.0) + 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Six pixels a character, as a plain stand-in for the font
    fn measure(s: &str) -> f32 {
        s.chars().count() as f32 * 6.0
    }

    #[test]
    fn a_label_too_long_for_its_share_is_cut_with_dots() {
        assert_eq!(fitted_label("General", 55.0, measure), "General");
        assert_eq!(fitted_label("Damage Indicator", 55.0, measure), "Damage...");
        assert!(measure(&fitted_label("Damage Indicator", 55.0, measure)) <= 55.0);
    }

    #[test]
    fn every_tab_is_as_wide_as_the_widest_label_and_the_pad() {
        let widths = ["General", "Costume", "Title", "Damage..."].map(measure);
        assert_eq!(tab_width(&widths), 9.0 * 6.0 + 15.0);
    }
}
