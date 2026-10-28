pub mod char_create_window;
pub mod char_select_window;
pub mod login_server_list_window;
pub mod login_window;
pub mod server_list_window;

use nostalre_ui::draw::{self, DrawCall, TexturePath, TextureRef};
use nostalre_ui::frame::UiFrame;

/// Draws a full-screen stretched background behind an account-flow screen. When
/// `texture` is `None` nothing is drawn and the renderer's clear color shows.
pub fn draw_background(ui: &mut UiFrame, texture: Option<TexturePath>) {
    let Some(name) = texture else {
        return;
    };
    let (verts, indices) = draw::quad_vertices(
        0.0,
        0.0,
        ui.ctx.screen_width,
        ui.ctx.screen_height,
        [1.0, 1.0, 1.0, 1.0],
    );
    ui.draw_calls.push(DrawCall {
        vertices: verts.into(),
        indices: indices.into(),
        texture: TextureRef::Named(name),
    });
}

/// The account-flow screen while Ragnarok Solo's hub has already chosen the character: the background and a line
/// naming who is entering, in place of the login, server and character windows the client goes through by itself.
pub fn draw_entering(ui: &mut UiFrame, texture: Option<TexturePath>, line: &str) {
    draw_background(ui, texture);
    let x = ((ui.ctx.screen_width - ui.atlas.measure_text(line)) / 2.0).max(0.0);
    let y = ui.ctx.screen_height * 0.85;
    ui.text_with_outline(
        x,
        y,
        line,
        [1.0, 1.0, 1.0, 1.0],
        &draw::TextOutline::cross([0.0, 0.0, 0.0, 0.85]),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    use nostalre_ui::context::UiContext;
    use nostalre_ui::state::StateCache;
    use nostalre_ui::test_support::test_frame;

    #[test]
    fn draw_background_pushes_fullscreen_quad_or_nothing() {
        let mut ctx = UiContext::new(1024.0, 768.0);
        let mut state = StateCache::new();

        let mut ui = test_frame(&mut ctx, &mut state);
        draw_background(&mut ui, None);
        assert!(ui.draw_calls.is_empty());

        let mut ui = test_frame(&mut ctx, &mut state);
        draw_background(&mut ui, Some("bg.bmp".into()));
        assert_eq!(ui.draw_calls.len(), 1);
        let call = &ui.draw_calls[0];
        match &call.texture {
            TextureRef::Named(n) => {
                assert_eq!(n, "bg.bmp");
                assert!(
                    matches!(n, std::borrow::Cow::Borrowed(_)),
                    "a constant name, not copied"
                );
            }
            _ => panic!("expected named texture"),
        }
        // a widget quad allocates nothing: vertices inline, indices the shared slice
        assert!(matches!(
            call.vertices,
            nostalre_renderer::Vertices::Quad(_)
        ));
        assert!(matches!(call.indices, std::borrow::Cow::Borrowed(_)));
        let xs: Vec<f32> = ui.draw_calls[0]
            .vertices
            .iter()
            .map(|v| v.position[0])
            .collect();
        assert!(xs.iter().cloned().fold(f32::MIN, f32::max) >= 1024.0);
    }
}
