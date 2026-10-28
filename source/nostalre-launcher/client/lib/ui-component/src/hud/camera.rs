//! The camera's readout while a controller or the on-screen controls move it (the design canvas, "Camera on
//! controller and touch").
use super::style;
use nostalre_ui::draw;
use nostalre_ui::frame::UiFrame;

const TOP: f32 = 14.0;
const UNDER_PLATE: f32 = 10.0;
const PILL_PAD: f32 = 14.0;

/// Where the camera is, as the readout shows it
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CameraReadout {
    /// 0 to 360, north 0
    pub turn_deg: f32,
    /// Down from level
    pub tilt_deg: f32,
    pub distance: f32,
}

/// "CAMERA  Turn 32° · Tilt 45° · Zoom 300" in a dark pill at the top centre, under the target's plate (whose
/// bottom edge `plate` is) when there is one
pub(super) fn readout(ui: &mut UiFrame, r: &CameraReadout, plate: Option<f32>) {
    let text = format!(
        "Turn {:.0}°  ·  Tilt {:.0}°  ·  Zoom {:.0}",
        r.turn_deg, r.tilt_deg, r.distance
    );
    let heading_w = style::heading_width(ui, "Camera");
    let w = (PILL_PAD * 2.0 + heading_w + 12.0 + style::measure(ui, &text)).ceil();
    let h = style::line(ui) + 10.0;
    let x = ((ui.ctx.screen_width - w) / 2.0).floor();
    let top = plate.map_or(TOP, |bottom| bottom + UNDER_PLATE);
    let back = draw::rounded_rect(x, top, w, h, h * 0.5, style::shade(0.55));
    style::push_white(ui, back);
    let baseline = style::baseline(ui, top + 5.0);
    style::heading(ui, x + PILL_PAD, baseline, "Camera");
    style::text(
        ui,
        x + PILL_PAD + heading_w + 12.0,
        baseline,
        &text,
        style::TEXT,
    );
}
