use crate::{App, input};
use nostalre_game::cursor::{RenderEntry, RenderEntryKind};
use nostalre_game::gr2_model;
use nostalre_renderer::Renderer;
use nostalre_renderer::sprite_projection::pick_bounds_from_drawn;

/// Box for an entity whose sprite or model has not loaded yet.
fn placeholder_bounds(screen_anchor: [f32; 2]) -> ([f32; 4], f32) {
    let half = 50.0;
    (
        [
            screen_anchor[0] - half,
            screen_anchor[1] - 100.0,
            screen_anchor[0] + half,
            screen_anchor[1],
        ],
        100.0,
    )
}

impl App {
    pub(crate) fn screen_dims(
        &self,
    ) -> Option<(
        &Renderer,
        &nostalre_formats::map_coordinates::MapCoordinates,
        f32,
        f32,
    )> {
        let renderer = self.renderer.as_ref()?;
        let coords = self.game.session.map_coords.as_ref()?;
        let screen_w = renderer.device.surface_config.width as f32 / renderer.dpi_scale;
        let screen_h = renderer.device.surface_config.height as f32 / renderer.dpi_scale;
        Some((renderer, coords, screen_w, screen_h))
    }

    pub(crate) fn push_projected(
        list: &mut Vec<RenderEntry>,
        kind: RenderEntryKind,
        id: u32,
        projected: Option<([f32; 2], f32, u8, f32, [f32; 2])>,
        flat_depth_gradient: Option<[f32; 2]>,
        camera_dir: Option<u8>,
        bounds: impl FnOnce([f32; 2], f32, u8, f32) -> ([f32; 4], f32),
    ) {
        let Some((screen_anchor, depth, projected_dir, sprite_scale, depth_gradient)) = projected
        else {
            return;
        };
        let (pick_bounds, head_offset) = bounds(screen_anchor, depth, projected_dir, sprite_scale);
        list.push(RenderEntry {
            kind,
            id,
            screen_anchor,
            depth,
            depth_gradient,
            flat_depth_gradient: flat_depth_gradient.unwrap_or(depth_gradient),
            camera_dir: camera_dir.unwrap_or(projected_dir),
            sprite_scale,
            pick_bounds,
            head_offset,
        });
    }

    pub(crate) fn compute_render_list(&self) -> Vec<RenderEntry> {
        nostalre_profiling::profile_function!();
        let mut render_list = Vec::new();
        if let Some((renderer, coords, screen_w, screen_h)) = self.screen_dims() {
            for entity in self.game.world.entities.iter() {
                let projected = input::entity_screen_params(
                    entity.movement.position(),
                    self.game.session.gat.as_ref(),
                    coords,
                    &renderer.camera,
                    screen_w,
                    screen_h,
                );
                let flat_depth_gradient = input::entity_ground_gradient(
                    entity.movement.position(),
                    self.game.session.gat.as_ref(),
                    coords,
                    &renderer.camera,
                    screen_w,
                    screen_h,
                );
                Self::push_projected(
                    &mut render_list,
                    RenderEntryKind::Entity,
                    entity.id,
                    projected,
                    Some(flat_depth_gradient),
                    None,
                    |screen_anchor, depth, camera_dir, sprite_scale| match self
                        .game
                        .sprite_caches
                        .sprites
                        .get(&entity.id)
                    {
                        Some(sprite) => (
                            sprite.compute_pick_bounds(
                                &entity.animation,
                                Some(camera_dir),
                                entity.head_dir,
                                screen_anchor,
                                depth,
                                sprite_scale,
                                screen_w,
                            ),
                            sprite.compute_head_offset(
                                &entity.animation,
                                Some(camera_dir),
                                entity.head_dir,
                                screen_anchor,
                                depth,
                                sprite_scale,
                            ),
                        ),
                        // GR2 entities carry no sprite: bound the posed model
                        // the same way the sprite path bounds its drawn quads.
                        None => renderer
                            .gr2_models
                            .get(&entity.id)
                            .zip(self.game.sprite_caches.gr2_models.get(&entity.id))
                            .and_then(|(model, instance)| {
                                model.asset().project_screen_bounds(
                                    gr2_model::model_world_transform(
                                        entity.movement.position(),
                                        self.game.session.gat.as_ref(),
                                        coords,
                                        entity.direction,
                                    ),
                                    instance.palette(),
                                    &renderer.camera,
                                    screen_w,
                                    screen_h,
                                )
                            })
                            .map_or_else(
                                || placeholder_bounds(screen_anchor),
                                |drawn| {
                                    let bounds =
                                        pick_bounds_from_drawn(drawn, screen_anchor, screen_w);
                                    (bounds, bounds[3] - bounds[1])
                                },
                            ),
                    },
                );
            }
        }
        render_list.sort_by(|a, b| {
            b.depth
                .partial_cmp(&a.depth)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        render_list
    }
}
