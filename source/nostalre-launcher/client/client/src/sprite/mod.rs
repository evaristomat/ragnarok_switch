pub(crate) mod cart;
mod cursor;
mod effects;
pub(crate) mod falcon;
mod floor_item;
pub(crate) mod gr2_loader;
mod snapshot;

pub(crate) use cart::CartVisual;
pub(crate) use falcon::FalconVisual;

use crate::App;
use models::enums::weapon::WeaponType;
use nostalre_formats::act::ActFile;
use nostalre_formats::spr::SpriteData;
use nostalre_game::data_table::accessory_table::AccessoryTable;
use nostalre_game::data_table::robe_table::RobeTable;
use nostalre_game::entity::EntityType;
use nostalre_game::gr2_model::{self, Gr2Asset, Gr2ModelInstance};
use nostalre_game::sprite_loader;
use nostalre_game::sprite_path::{entity_sprite_base_path, is_undrawn_actor};
use nostalre_profiling::profile_function;
use nostalre_renderer::{
    EntitySprite, Gr2ModelAsset, Gr2ModelDraw, RobeLayer, SpriteTextures, build_entity_sprite_cached,
    upload_glyph_textures, upload_sprite_textures,
};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Mutex;

/// What a player unit wears, as its sprite is built from it.
pub(crate) struct UnitLook {
    pub(crate) job: u16,
    pub(crate) sex: u8,
    pub(crate) head: u16,
    pub(crate) hair_color: u16,
    pub(crate) cloth_color: u16,
    pub(crate) weapon: Option<WeaponType>,
    pub(crate) weapon_look: u16,
    pub(crate) head_top: u16,
    pub(crate) head_mid: u16,
    pub(crate) head_bottom: u16,
    pub(crate) shield: u16,
}

impl App {
    pub(crate) fn upload_sprite(&self, data: &SpriteData) -> Option<SpriteTextures> {
        let renderer = self.renderer.as_ref()?;
        Some(upload_sprite_textures(
            &data.images,
            data.indexed_count,
            &renderer.device.device,
            &renderer.device.queue,
            &renderer.texture_cache.bind_group_layout,
        ))
    }

    pub(crate) fn upload_glyph_sprite(&self, data: &SpriteData) -> Option<SpriteTextures> {
        let renderer = self.renderer.as_ref()?;
        Some(upload_glyph_textures(
            &data.images,
            data.indexed_count,
            &renderer.device.device,
            &renderer.device.queue,
            &renderer.texture_cache.bind_group_layout,
        ))
    }

    pub(crate) fn reload_player_sprite(&mut self, gid: u32) {
        profile_function!();
        let Some(look) = self.unit_look(gid) else {
            return;
        };
        self.load_player_sprite(
            gid,
            look.job,
            look.sex,
            look.head,
            look.hair_color,
            look.cloth_color,
            look.weapon,
            look.weapon_look,
            look.head_top,
            look.head_mid,
            look.head_bottom,
            look.shield,
        );
    }

    /// What a player unit wears now, its body as drawn (mounts and costumes included).
    pub(crate) fn unit_look(&self, gid: u32) -> Option<UnitLook> {
        let entity = self.game.world.entities.get(gid)?;
        Some(UnitLook {
            job: nostalre_game::sprite_path::visual_job(
                entity.job,
                entity.effect_state,
                entity.status_mount,
            ),
            sex: entity.sex,
            head: entity.head,
            hair_color: entity.hair_color,
            cloth_color: entity.cloth_color,
            weapon: entity.weapon,
            weapon_look: entity.weapon_look,
            head_top: entity.head_top,
            head_mid: entity.head_mid,
            head_bottom: entity.head_bottom,
            shield: entity.shield,
        })
    }

    /// The parts of a player's look that come from the unit itself whatever it is drawn with: the job its head
    /// follows, an orcish face, the GM body, its garment.
    pub(crate) fn unit_look_extras(&self, gid: u32, job: u16) -> (u16, bool, bool, u16) {
        self.game
            .world
            .entities
            .get(gid)
            .map(|e| {
                (
                    e.job,
                    nostalre_game::sprite_path::is_orcish(e.effect_state),
                    e.is_gm,
                    e.robe,
                )
            })
            .unwrap_or((job, false, false, 0))
    }

    /// Rebuilds a unit's sprite from what it holds now: a player's, ours or another's, with its hands, clothes and
    /// mounts; any other unit's from its job.
    pub(crate) fn reload_unit_sprite(&mut self, gid: u32) {
        if let Some(e) = self.game.world.entities.get(gid) {
            if !self.game.world.entities.is_player(gid) {
                self.game.sprite_caches.sprites.remove(&gid);
                self.game.sprite_caches.failed_sprite_loads.remove(&gid);
                return;
            }
            let (entity_type, job) = (e.entity_type, e.job);
            self.load_entity_sprite(gid, entity_type, job);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn load_player_sprite(
        &mut self,
        gid: u32,
        job: u16,
        sex: u8,
        head: u16,
        hair_color: u16,
        cloth_color: u16,
        weapon: Option<WeaponType>,
        weapon_look: u16,
        head_top: u16,
        head_mid: u16,
        head_bottom: u16,
        shield_id: u16,
    ) {
        profile_function!();
        // the head follows the unit's own job: a Doram in a costume it has no art for keeps its head (kRO)
        let (head_job, orc_face, is_gm, robe) = self.unit_look_extras(gid, job);
        let sprite = self.build_player_entity_sprite(
            job,
            head_job,
            sex,
            head,
            hair_color,
            cloth_color,
            weapon,
            weapon_look,
            head_top,
            head_mid,
            head_bottom,
            shield_id,
            robe,
            orc_face,
            is_gm,
        );
        self.keep_player_sprite(gid, job, sprite);
    }

    /// Keeps a built player sprite under `gid` (an entity's, or a character select slot's).
    pub(crate) fn keep_player_sprite(
        &mut self,
        gid: u32,
        job: u16,
        sprite: Option<Rc<EntitySprite>>,
    ) {
        match sprite {
            Some(sprite) => {
                self.game.sprite_caches.sprites.insert(gid, sprite);
            }
            None => tracing::warn!("no player sprite for gid={gid} job={job}"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_player_entity_sprite(
        &self,
        job: u16,
        head_job: u16,
        sex: u8,
        head: u16,
        hair_color: u16,
        cloth_color: u16,
        weapon: Option<WeaponType>,
        weapon_look: u16,
        head_top: u16,
        head_mid: u16,
        head_bottom: u16,
        shield_id: u16,
        robe: u16,
        orc_face: bool,
        is_gm: bool,
    ) -> Option<Rc<EntitySprite>> {
        profile_function!();
        let renderer = self.renderer.as_ref()?;
        let (device, queue, layout) = (
            &renderer.device.device,
            &renderer.device.queue,
            &renderer.texture_cache.bind_group_layout,
        );
        let sprite = self.player_entity_sprite(
            job,
            head_job,
            sex,
            head,
            hair_color,
            cloth_color,
            weapon,
            weapon_look,
            head_top,
            head_mid,
            head_bottom,
            shield_id,
            robe,
            orc_face,
            is_gm,
            |data| {
                let textures = renderer.texture_cache.sprite_sheets.upload(&data, device, queue, layout);
                (textures, data.act)
            },
        )?;
        Some(Rc::new(sprite))
    }

    /// A player's layers, each part's images made into a sheet by `sheet`: uploaded for the game, kept as pixels
    /// for its picture (`snapshot`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn player_entity_sprite<T>(
        &self,
        job: u16,
        head_job: u16,
        sex: u8,
        head: u16,
        hair_color: u16,
        cloth_color: u16,
        weapon: Option<WeaponType>,
        weapon_look: u16,
        head_top: u16,
        head_mid: u16,
        head_bottom: u16,
        shield_id: u16,
        robe: u16,
        orc_face: bool,
        is_gm: bool,
        mut sheet: impl FnMut(SpriteData) -> (T, ActFile),
    ) -> Option<EntitySprite<T>> {
        let grf = self.grf.as_ref()?;
        let empty_table = AccessoryTable::empty();
        let accessory_table = self
            .game
            .data_table
            .accessory
            .as_ref()
            .unwrap_or(&empty_table);
        let data = sprite_loader::load_player_sprite_data(
            grf,
            accessory_table,
            job,
            head_job,
            sex,
            head,
            hair_color,
            cloth_color,
            weapon,
            weapon_look,
            head_top,
            head_mid,
            head_bottom,
            shield_id,
            orc_face,
            is_gm,
        )?;
        let robe = self.robe_layer(robe, job, sex, &mut sheet);
        Some(
            EntitySprite::from_parts(
                data.body,
                data.head,
                data.weapon,
                data.weapon_trail,
                data.headgear_top,
                data.headgear_mid,
                data.headgear_bottom,
                data.shield,
                data.shadow,
                sheet,
            )
            .with_robe(robe)
            .with_layer_order(data.layer_order)
            .with_taekwon_sex(nostalre_game::entity::is_taekwon_job(job).then_some(sex)),
        )
    }

    /// The garment `robe` names, drawn for `job` (the job drawn, mount included); None for none or without art.
    fn robe_layer<T>(
        &self,
        robe: u16,
        job: u16,
        sex: u8,
        sheet: &mut impl FnMut(SpriteData) -> (T, ActFile),
    ) -> Option<RobeLayer<T>> {
        if robe == 0 {
            return None;
        }
        let grf = self.grf.as_ref()?;
        let table = self
            .game
            .data_table
            .robe
            .get_or_init(|| RobeTable::load(grf));
        let Some(name) = table.name(robe) else {
            // once per view: every look change of its wearer rebuilds the sprite
            static UNKNOWN: Mutex<BTreeSet<u16>> = Mutex::new(BTreeSet::new());
            if UNKNOWN.lock().is_ok_and(|mut seen| seen.insert(robe)) {
                tracing::warn!("garment {robe} is not in the client's robe table: not drawn");
            }
            return None;
        };
        let (textures, act) = sheet(sprite_loader::load_robe_sprite(grf, name, job, sex)?);
        Some(RobeLayer {
            textures,
            act,
            behind: table.behind(job, sex).cloned().unwrap_or_default(),
        })
    }

    /// Load the head/body sprite of every current guild member into a dedicated
    /// cache keyed by member GID, for the face icons in the guild roster. Kept
    /// separate from `game.sprites` so it never collides with on-screen entities.
    pub(crate) fn load_guild_member_sprites(&mut self) {
        profile_function!();
        self.game.sprite_caches.guild_head_sprites.clear();
        let members: Vec<(u32, u16, u8, u16, u16)> = match &self.game.guild {
            Some(g) => g
                .members
                .iter()
                .map(|m| {
                    let sex = self
                        .game
                        .world
                        .entities
                        .get(m.gid)
                        .or_else(|| {
                            self.game
                                .world
                                .entities
                                .get(self.game.world.entities.resolve_key(m.aid))
                        })
                        .map(|e| e.sex)
                        .unwrap_or_else(|| m.sex.max(0) as u8);
                    (
                        m.gid,
                        m.job.max(0) as u16,
                        sex,
                        m.head.max(0) as u16,
                        m.head_palette.max(0) as u16,
                    )
                })
                .collect(),
            None => return,
        };
        for (gid, job, sex, head, hair_color) in members {
            if let Some(sprite) = self.build_player_entity_sprite(
                job, job, sex, head, hair_color, 0, None, 0, 0, 0, 0, 0, 0, false, false,
            ) {
                self.game
                    .sprite_caches
                    .guild_head_sprites
                    .insert(gid, sprite);
            }
        }
    }

    pub(crate) fn load_mercenary_sprite(&mut self, gid: u32, job: u16) {
        let (grf, renderer) = match (&self.grf, &self.renderer) {
            (Some(g), Some(r)) => (g, r),
            _ => return,
        };
        let name_table = match &self.game.data_table.name {
            Some(t) => t,
            None => return,
        };
        let data = match sprite_loader::load_mercenary_sprite_data(grf, name_table, job) {
            Some(d) => d,
            None => {
                tracing::warn!("load_mercenary_sprite: no sprite data for gid={gid} job={job}");
                return;
            }
        };
        let sprite = Rc::new(
            build_entity_sprite_cached(
                &renderer.device.device,
                &renderer.device.queue,
                &renderer.texture_cache,
                data.body,
                data.head,
                data.weapon,
                data.weapon_trail,
                data.headgear_top,
                data.headgear_mid,
                data.headgear_bottom,
                data.shield,
                data.shadow,
            )
            .with_layer_order(data.layer_order),
        );
        self.game.sprite_caches.sprites.insert(gid, sprite);
    }

    /// Loads the sprite `job` draws for a unit; a player's comes from the unit itself (`reload_player_sprite`).
    pub(crate) fn load_entity_sprite(&mut self, gid: u32, entity_type: EntityType, job: u16) {
        profile_function!();
        let (grf, renderer) = match (&self.grf, &self.renderer) {
            (Some(g), Some(r)) => (g, r),
            _ => return,
        };

        match entity_type {
            // a player's look is its own: hands, clothes and mounts come from the unit
            EntityType::Player => self.reload_player_sprite(gid),
            EntityType::Mercenary => {
                self.load_mercenary_sprite(gid, job);
            }
            EntityType::Npc | EntityType::Monster | EntityType::Homunculus => {
                let name_table = match &self.game.data_table.name {
                    Some(t) => t,
                    None => {
                        tracing::warn!("No name table for job {job}");
                        return;
                    }
                };
                let gr2_name = name_table
                    .get_name(job)
                    .filter(|n| gr2_model::is_gr2_name(n))
                    .map(|n| n.to_string());
                if let Some(name) = gr2_name {
                    self.load_gr2_entity_model(gid, &name);
                    return;
                }
                let cache_key = match entity_sprite_base_path(name_table, job) {
                    Some(p) => p,
                    None => {
                        if !is_undrawn_actor(job) {
                            tracing::warn!("No sprite path for id {job}");
                        }
                        return;
                    }
                };

                if let Some(cached) = self.game.sprite_caches.sprite_cache.get(&cache_key) {
                    self.game
                        .sprite_caches
                        .sprites
                        .insert(gid, Rc::clone(cached));
                    return;
                }

                let data = match sprite_loader::load_entity_sprite_data(grf, name_table, job) {
                    Some(d) => d,
                    None => return,
                };
                let sprite = Rc::new(build_entity_sprite_cached(
                    &renderer.device.device,
                    &renderer.device.queue,
                    &renderer.texture_cache,
                    data.body,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    data.shadow,
                ));
                self.game
                    .sprite_caches
                    .sprite_cache
                    .insert(cache_key, Rc::clone(&sprite));
                self.game.sprite_caches.sprites.insert(gid, sprite);
            }
        }
    }

    /// Swaps a pet's body sprite to its accessory ACT variant. The accessory is an
    /// ACT-only swap that reuses the mob's base SPR (the accessory frames live in
    /// that SPR). Falls back to the plain mob sprite when the accessory is unset or
    /// its ACT/SPR is missing.
    pub(crate) fn load_pet_sprite(&mut self, gid: u32, job: u16, accessory_view: u16) {
        let load_plain = |app: &mut Self| {
            app.load_entity_sprite(gid, EntityType::Monster, job);
        };
        let Some(act_path) = nostalre_game::pet_tables::pet_accessory_act(accessory_view) else {
            load_plain(self);
            return;
        };
        if let Some(cached) = self.game.sprite_caches.sprite_cache.get(act_path) {
            let cached = Rc::clone(cached);
            self.game.sprite_caches.sprites.insert(gid, cached);
            return;
        }
        let base_path = self
            .game
            .data_table
            .name
            .as_ref()
            .and_then(|nt| nostalre_game::sprite_path::entity_sprite_base_path(nt, job));
        let Some(base_path) = base_path else {
            load_plain(self);
            return;
        };
        let spr_path = format!("{base_path}.spr");
        let (grf, renderer) = match (&self.grf, &self.renderer) {
            (Some(g), Some(r)) => (g, r),
            _ => return,
        };
        let Some(data) = sprite_loader::load_sprite_data(grf, &spr_path, act_path) else {
            load_plain(self);
            return;
        };
        let sprite = Rc::new(build_entity_sprite_cached(
            &renderer.device.device,
            &renderer.device.queue,
            &renderer.texture_cache,
            data,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ));
        self.game
            .sprite_caches
            .sprite_cache
            .insert(act_path.to_string(), Rc::clone(&sprite));
        self.game.sprite_caches.sprites.insert(gid, sprite);
    }

    /// Load a `.gr2` name-table entity (emperium, guardian, guild flag…) as an
    /// animated 3D model instead of a sprite. Geometry, textures, skeleton and
    /// clips are cached per model file; only the transform and bone palette are
    /// per entity.
    pub(crate) fn load_gr2_entity_model(&mut self, gid: u32, model_name: &str) {
        let path = gr2_model::gr2_model_path(model_name);
        if self.game.sprite_caches.gr2_assets.contains_key(&path) {
            self.spawn_gr2_instance(gid, &path);
            return;
        }
        if let Some(loader) = &mut self.gr2_loader {
            loader.request(gid, model_name, &path);
        }
    }

    /// Instance every entity whose model finished reading on the loader thread.
    pub(crate) fn poll_gr2_loads(&mut self) {
        let Some(loader) = &mut self.gr2_loader else {
            return;
        };
        for (path, loaded, gids) in loader.take_ready() {
            let uploaded = match loaded {
                Some(loaded) => self.insert_gr2_asset(path.clone(), loaded),
                None => false,
            };
            for gid in gids {
                if !uploaded {
                    self.game.sprite_caches.failed_sprite_loads.insert(gid);
                    continue;
                }
                if self.game.world.entities.get(gid).is_some() {
                    self.spawn_gr2_instance(gid, &path);
                }
            }
        }
    }

    /// Upload a decoded model and cache it under `path`. False when it produced
    /// nothing drawable.
    fn insert_gr2_asset(&mut self, path: String, loaded: gr2_loader::Gr2LoadedAsset) -> bool {
        let Some(renderer) = &mut self.renderer else {
            return false;
        };
        let Some(asset) = Gr2ModelAsset::from_parts(
            &renderer.device.device,
            &renderer.device.queue,
            &renderer.texture_cache,
            loaded.geometry,
            loaded.textures,
            loaded.emblem_texture_index,
        ) else {
            tracing::warn!("gr2 model {path} produced no renderable geometry");
            return false;
        };
        renderer.gr2_assets.insert(path.clone(), Rc::new(asset));
        self.game.sprite_caches.gr2_assets.insert(
            path,
            Rc::new(Gr2Asset {
                pose: loaded.pose,
                clips: loaded.clips,
            }),
        );
        true
    }

    /// Give `gid` its own transform and bone palette over the cached model.
    fn spawn_gr2_instance(&mut self, gid: u32, path: &str) {
        let Some(cpu_asset) = self.game.sprite_caches.gr2_assets.get(path).cloned() else {
            return;
        };
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let Some(asset) = renderer.gr2_assets.get(path).cloned() else {
            return;
        };
        let draw = Gr2ModelDraw::new(&renderer.device.device, &renderer.gr2_pipeline, asset);
        renderer.gr2_models.insert(gid, draw);
        self.game
            .sprite_caches
            .gr2_models
            .insert(gid, Gr2ModelInstance::new(cpu_asset));
        self.apply_guild_emblem_to_model(gid);
    }

    pub(crate) fn remove_gr2_model(&mut self, gid: u32) {
        self.game.sprite_caches.gr2_models.remove(&gid);
        if let Some(renderer) = &mut self.renderer {
            renderer.gr2_models.remove(&gid);
        }
    }

    pub(crate) fn load_missing_entity_sprites(&mut self) {
        profile_function!();
        let mut missing: Vec<_> = self
            .game
            .world
            .entities
            .iter()
            .filter(|e| {
                self.game.world.entities.player_id() != Some(e.id)
                    && !self.game.sprite_caches.sprites.contains_key(&e.id)
                    && !self.game.sprite_caches.gr2_models.contains_key(&e.id)
                    && !self.game.sprite_caches.failed_sprite_loads.contains(&e.id)
                    && !self
                        .gr2_loader
                        .as_ref()
                        .is_some_and(|loader| loader.is_waiting(e.id))
            })
            .map(|e| (e.id, e.entity_type, e.job))
            .collect();
        let origin = self.game.world.entities.player().map(|p| p.movement.cell_position()).unwrap_or((0,0));
        missing.sort_by_key(|(gid,_,_)| self.game.world.entities.get(*gid).map(|e| { let (x,y)=e.movement.cell_position(); (x.abs_diff(origin.0) as u32).pow(2)+(y.abs_diff(origin.1) as u32).pow(2) }).unwrap_or(u32::MAX));
        // One actor upload per frame: queued looks use the latest entity state.
        for (gid, entity_type, job) in missing.iter().take(1) {
            tracing::info!(
                "Retrying sprite load for entity gid={gid} job={job} type={entity_type:?}"
            );
            self.load_entity_sprite(*gid, *entity_type, *job);
            if let Some(e) = self.game.world.entities.get(*gid) {
                if e.is_pet && e.pet_accessory != 0 { let (job, accessory)=(e.job,e.pet_accessory); self.load_pet_sprite(*gid,job,accessory); }
            }
            if !self.game.sprite_caches.sprites.contains_key(gid)
                && !self.game.sprite_caches.gr2_models.contains_key(gid)
                && !self
                    .gr2_loader
                    .as_ref()
                    .is_some_and(|loader| loader.is_waiting(*gid))
            {
                self.game.sprite_caches.failed_sprite_loads.insert(*gid);
            }
        }
    }
}
