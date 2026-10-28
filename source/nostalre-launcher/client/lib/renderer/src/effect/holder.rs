use std::sync::Arc;

use models::enums::EnumWithNumberValue;
use models::enums::effect_id::EffectId;
use nostalre_effects::effects::body_aura::BodyAuraEffect;
use nostalre_effects::spec::EffectAnchor;
use nostalre_effects::{
    Afterimage, AlphaKeyframe, Attach, BodyAction, CameraShake, Effect as GameEffect,
    EffectDrawList, EffectQueue, EffectRenderCtx, EffectSpec, EffectStatus, EffectUpdateCtx,
    NumberRequest, SpawnRequest, SprBodyRecolor, SprBurstParams, StrLayer, custom_duration_ms,
    effect_spec, make_effect, spawn_camera_shake, str_layer, str_variant,
};
use nostalre_formats::act::SpriteAnimationState;

use nostalre_effects::sfx::{SfxEmission, SfxPos, SfxSchedule, effect_sound, emit};

use crate::effect::str_pipeline::StrEmitterInput;
use crate::effect_sprite::BurstParticle;

pub trait ExternalCustomBackend: Send + Sync {
    fn spawn(
        &self,
        effect_id: u16,
        from: [f32; 3],
        to: [f32; 3],
        hit_count: u8,
        target_size: Option<[f32; 2]>,
    ) -> u64;
    fn update(&self, handle: u64, dt: f32, caster_yaw: Option<f32>) -> bool;
    fn collect(&self, handle: u64, ctx: &EffectRenderCtx, out: &mut EffectDrawList);
    fn str_overlay(&self, _handle: u64) -> Option<String> {
        None
    }
    fn take_camera_shake(&self, _handle: u64) -> Option<CameraShake> {
        None
    }
    fn take_sfx(&self, _handle: u64) -> Option<String> {
        None
    }
    fn drop_handle(&self, handle: u64);
    fn drop_all(&self);
}

#[derive(Default)]
struct ShakeController {
    amplitude: f32,
    elapsed: f32,
    duration: f32,
}

impl ShakeController {
    fn trigger(&mut self, shake: CameraShake) {
        let remaining = if self.duration > 0.0 {
            self.amplitude * (1.0 - (self.elapsed / self.duration).clamp(0.0, 1.0))
        } else {
            0.0
        };
        self.amplitude = shake.amplitude.max(remaining);
        self.duration = (shake.duration_ms as f32 / 1000.0).max(1e-3);
        self.elapsed = 0.0;
    }

    fn tick(&mut self, dt: f32) {
        if self.duration > 0.0 {
            self.elapsed += dt;
        }
    }

    fn offset(&self) -> glam::Vec3 {
        if self.duration <= 0.0 || self.elapsed >= self.duration {
            return glam::Vec3::ZERO;
        }
        let amp = self.amplitude * (1.0 - self.elapsed / self.duration);
        let frame = (self.elapsed * 60.0) as u32;
        let j = |salt: u32| {
            let x = frame
                .wrapping_mul(2_654_435_761)
                .wrapping_add(salt.wrapping_mul(40_503))
                .wrapping_add(0x9E37_79B9);
            let x = x ^ (x >> 15);
            ((x % 100_000) as f32 / 100_000.0) * 2.0 - 1.0
        };
        glam::Vec3::new(j(1) * amp, j(2) * amp * 0.5, j(3) * amp)
    }
}

pub struct StrSnapshot<'a> {
    pub name: std::borrow::Cow<'a, str>,
    pub position: [f32; 3],
    /// From `position` in the camera's own axes ([`nostalre_effects::StrPiece`])
    pub view_offset: [f32; 3],
    pub scale: f32,
    pub anim_time: f32,
    pub repeat: bool,
    pub layer: StrLayer,
    pub turn: nostalre_effects::StrTurn,
    pub alpha: f32,
}

impl StrSnapshot<'_> {
    pub fn emitter(&self) -> StrEmitterInput<'_> {
        StrEmitterInput {
            str_name: &self.name,
            position: self.position,
            view_offset: self.view_offset,
            scale: self.scale,
            anim_time: self.anim_time,
            repeat: self.repeat,
            turn: self.turn,
            alpha: self.alpha,
        }
    }
}

pub struct SprSnapshot {
    pub sprite: String,
    pub position: [f32; 3],
    pub anim_time: f32,
    pub duration_ms: f32,
    pub size_scale: f32,
    pub anim_speed: f32,
    pub repeat: bool,
    pub tint: [f32; 4],
    pub action_index: usize,
    pub no_depth: bool,
    pub clip_offset: [i32; 2],
}

pub struct SprBurstSnapshot {
    pub sprite: String,
    pub size_scale: f32,
    pub alpha_max: f32,
    pub anim_speed: f32,
    pub size_shrink: bool,
    pub twinkle: bool,
    pub particles: Vec<BurstParticle>,
}

#[derive(Clone, Copy)]
struct BurstParticleSim {
    pos: [f32; 3],
    velocity: [f32; 3],
    age: f32,
    lifetime: f32,
    age_frames: f32,
    lon_deg: f32,
    lat_deg: f32,
    curve_timer_frames: f32,
    curve_count: u32,
    alpha: f32,
    alpha_speed: f32,
    alpha_max: f32,
    keyframe_idx: usize,
}

struct BurstState {
    sprite: String,
    params: SprBurstParams,
    particles: Vec<BurstParticleSim>,
    has_emitted: bool,
    cooldown_timer: f32,
    body_recolor: Option<SprBodyRecolor>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EffectHandle(u64);

#[derive(Clone, Debug)]
pub enum SpawnOutcome {
    Custom,
    Str { name: String },
    Spr,
    SprBurst,
    CustomNotImpl,
    NoSpec,
    Noop,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpawnStatus {
    Rendering,
    StrFileMissing,
    CustomNotImpl,
    NoSpec,
    Noop,
}

enum HeldPayload {
    Custom(Box<dyn GameEffect>),
    CustomExternal {
        handle: u64,
    },
    Str {
        name: String,
        repeat: bool,
    },
    Spr {
        sprite: String,
        size_scale: f32,
        anim_speed: f32,
        repeat: bool,
        tint: [f32; 4],
        pos_y: f32,
        action_index: usize,
        no_depth: bool,
        clip_offset: [i32; 2],
    },
    SprBurst(BurstState),
}

struct HeldEffect {
    handle: EffectHandle,
    effect_id: EffectId,
    payload: HeldPayload,
    attach: Attach,
    age: f32,
    duration: f32,
    key: Option<u32>,
    sfx_schedule: Option<SfxSchedule>,
    sfx_last_frame: i32,
    sfx_rng: u32,
}

pub struct AfterimageSnapshot {
    entity_id: u32,
    pub anim: SpriteAnimationState,
    pub camera_dir: Option<u8>,
    pub head_dir: u8,
    pub world_pos: (f32, f32),
    pub anchor: [f32; 2],
    pub depth: f32,
    pub scale: f32,
    pub tint: [u8; 3],
    pub alpha: f32,
    fade_per_sec: f32,
}

impl AfterimageSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        entity_id: u32,
        anim: SpriteAnimationState,
        camera_dir: Option<u8>,
        head_dir: u8,
        world_pos: (f32, f32),
        anchor: [f32; 2],
        depth: f32,
        scale: f32,
        ai: &Afterimage,
    ) -> Self {
        Self {
            entity_id,
            anim,
            camera_dir,
            head_dir,
            world_pos,
            anchor,
            depth,
            scale,
            tint: ai.tint,
            alpha: ai.start_alpha,
            fade_per_sec: ai.fade_per_frame * 60.0,
        }
    }
}

#[derive(Default)]
pub struct EffectHolder {
    next_id: u64,
    effects: Vec<HeldEffect>,
    last_spawn: Option<SpawnOutcome>,
    external_backend: Option<Arc<dyn ExternalCustomBackend>>,
    shake: ShakeController,
    afterimages: Vec<AfterimageSnapshot>,
    pending_sfx: Vec<SfxEmission>,
    ground_sampler: Option<nostalre_effects::effect_trait::GroundSampler>,
    /// Units whose effects run but are not drawn (`set_unseen_owners`)
    unseen_owners: Vec<u32>,
    /// Kept between frames for `end_played_strs`, so asking the effects for their STRs allocates nothing
    played_pieces: Vec<nostalre_effects::StrPiece>,
}

impl EffectHolder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Terrain height lookup handed to every effect built from here on. Set once
    /// per map; effects that do not place geometry off-anchor ignore it.
    pub fn set_ground_sampler(
        &mut self,
        sampler: Option<nostalre_effects::effect_trait::GroundSampler>,
    ) {
        self.ground_sampler = sampler;
    }

    /// The units whose attached effects are not drawn this frame, kept alive to show again: another player hidden
    /// or cloaked, as kRO's draw skips them (0x992480, 0x6c6ad0)
    pub fn set_unseen_owners(&mut self, owners: impl IntoIterator<Item = u32>) {
        self.unseen_owners.clear();
        self.unseen_owners.extend(owners);
    }

    /// Ends the custom effects whose STRs end by themselves ([`nostalre_effects::Effect::ends_with_its_strs`])
    /// once each has played to its file's end; `seconds` is a file's length from the renderer's STR cache, none
    /// while it is not loaded (the effect then holds to its own end)
    pub fn end_played_strs(&mut self, seconds: &dyn Fn(&str) -> Option<f32>) {
        let pieces = &mut self.played_pieces;
        self.effects.retain(|e| {
            let HeldPayload::Custom(c) = &e.payload else {
                return true;
            };
            if !c.ends_with_its_strs() {
                return true;
            }
            pieces.clear();
            c.str_pieces(pieces);
            let played = !pieces.is_empty()
                && pieces.iter().all(|p| {
                    !p.repeat && seconds(p.file).is_some_and(|length| p.anim_time >= length)
                });
            !played
        });
    }

    fn drawn(&self, attach: &Attach) -> bool {
        !matches!(attach, Attach::Entity(id) if self.unseen_owners.contains(id))
    }

    pub fn last_spawn(&self) -> Option<&SpawnOutcome> {
        self.last_spawn.as_ref()
    }

    pub fn effect_count(&self) -> usize {
        self.effects.len()
    }

    pub fn custom_count(&self) -> usize {
        self.effects
            .iter()
            .filter(|e| matches!(e.payload, HeldPayload::Custom(_)))
            .count()
    }

    pub fn debug_live(&self) -> Vec<(EffectId, f32, f32)> {
        self.effects
            .iter()
            .filter(|e| matches!(e.payload, HeldPayload::Custom(_)))
            .map(|e| (e.effect_id, e.age, e.duration))
            .collect()
    }

    pub fn set_external_backend(&mut self, backend: Option<Arc<dyn ExternalCustomBackend>>) {
        if let Some(old) = &self.external_backend {
            self.effects
                .retain(|e| !matches!(e.payload, HeldPayload::CustomExternal { .. }));
            old.drop_all();
        }
        self.external_backend = backend;
    }

    pub fn spawn(
        &mut self,
        effect_id: EffectId,
        attach: Attach,
        override_duration_ms: Option<u32>,
    ) -> Option<EffectHandle> {
        self.spawn_request(
            SpawnRequest {
                override_duration_ms,
                ..SpawnRequest::new(effect_id, attach)
            },
            &|_| None,
        )
    }

    fn spawn_request(
        &mut self,
        req: SpawnRequest,
        resolve_entity: &dyn Fn(u32) -> Option<[f32; 3]>,
    ) -> Option<EffectHandle> {
        let SpawnRequest {
            effect_id,
            attach,
            override_duration_ms,
            cap_duration_ms,
            hit_count,
            target_size,
            key,
            size_scale: size_scale_override,
            body_aura,
        } = req;
        let Some(spec) = effect_spec(effect_id) else {
            self.last_spawn = Some(SpawnOutcome::NoSpec);
            return None;
        };
        if let Some(shake) = spawn_camera_shake(effect_id) {
            self.shake.trigger(shake);
        }
        if matches!(spec, EffectSpec::Noop) {
            self.last_spawn = Some(SpawnOutcome::Noop);
            return None;
        }
        let payload = match &spec {
            EffectSpec::Str { file, repeat, .. } => {
                let name = match str_variant(effect_id, self.next_id) {
                    "" => *file,
                    variant => variant,
                };
                self.last_spawn = Some(SpawnOutcome::Str {
                    name: name.to_string(),
                });
                HeldPayload::Str {
                    name: name.to_string(),
                    repeat: *repeat,
                }
            }
            EffectSpec::Spr {
                sprite,
                size_scale,
                anim_speed,
                repeat,
                tint,
                pos_y,
                action_index,
                no_depth,
                clip_offset,
                ..
            } => {
                self.last_spawn = Some(SpawnOutcome::Spr);
                HeldPayload::Spr {
                    sprite: (*sprite).to_string(),
                    size_scale: *size_scale * size_scale_override.unwrap_or(1.0),
                    anim_speed: *anim_speed,
                    repeat: *repeat,
                    tint: *tint,
                    pos_y: *pos_y,
                    action_index: *action_index,
                    no_depth: *no_depth,
                    clip_offset: *clip_offset,
                }
            }
            EffectSpec::SprBurst {
                sprite,
                burst,
                body_recolor,
                ..
            } => {
                self.last_spawn = Some(SpawnOutcome::SprBurst);
                let mut params = *burst;
                params.size *= size_scale_override.unwrap_or(1.0);
                HeldPayload::SprBurst(BurstState {
                    sprite: (*sprite).to_string(),
                    params,
                    particles: Vec::new(),
                    has_emitted: false,
                    cooldown_timer: 0.0,
                    body_recolor: *body_recolor,
                })
            }
            EffectSpec::Noop => unreachable!("Noop handled above"),
            // a body effect is built from the rule that asked for it, not from its id
            EffectSpec::Custom if let Some(spec) = body_aura => {
                self.last_spawn = Some(SpawnOutcome::Custom);
                let anchor = attach_to_anchor(attach, resolve_entity);
                HeldPayload::Custom(Box::new(BodyAuraEffect::new(
                    anchor.point(),
                    spec,
                    hit_count.unwrap_or(1),
                )))
            }
            EffectSpec::Custom => {
                if let Some(backend) = &self.external_backend {
                    let (from, to) = match attach {
                        Attach::WorldPos(p) => (p, p),
                        Attach::Trail { from, to } => (from, to),
                        // Snapshot the entity position at spawn — the external
                        // backend has no per-frame entity table.
                        Attach::Entity(id) => {
                            let p = resolve_entity(id).unwrap_or([0.0; 3]);
                            (p, p)
                        }
                        Attach::Projectile { .. } | Attach::Link { .. } => ([0.0; 3], [0.0; 3]),
                    };
                    let handle = backend.spawn(
                        effect_id.value() as u16,
                        from,
                        to,
                        hit_count.unwrap_or(0),
                        target_size,
                    );
                    if handle != 0 {
                        self.last_spawn = Some(SpawnOutcome::Custom);
                        HeldPayload::CustomExternal { handle }
                    } else {
                        self.last_spawn = Some(SpawnOutcome::CustomNotImpl);
                        if nostalre_profiling::debug::trace_effects() {
                            tracing::info!(
                                "EffectHolder: external backend has no impl for {:?}",
                                effect_id
                            );
                        }
                        return None;
                    }
                } else {
                    let anchor = attach_to_anchor(attach, resolve_entity);
                    match make_effect(
                        effect_id,
                        anchor,
                        hit_count,
                        target_size,
                        override_duration_ms,
                    ) {
                        Some(mut e) => {
                            if let Some(sampler) = &self.ground_sampler {
                                e.set_ground_sampler(sampler.clone());
                            }
                            self.last_spawn = Some(SpawnOutcome::Custom);
                            HeldPayload::Custom(e)
                        }
                        None => {
                            self.last_spawn = Some(SpawnOutcome::CustomNotImpl);
                            if nostalre_profiling::debug::trace_effects() {
                                tracing::info!("EffectHolder: no factory impl for {:?}", effect_id);
                            }
                            return None;
                        }
                    }
                }
            }
        };

        let duration_ms = override_duration_ms.unwrap_or_else(|| match spec {
            EffectSpec::Str { duration_ms, .. }
            | EffectSpec::Spr { duration_ms, .. }
            | EffectSpec::SprBurst { duration_ms, .. } => duration_ms,
            EffectSpec::Custom => custom_duration_ms(effect_id),
            EffectSpec::Noop => unreachable!("Noop handled above"),
        });
        let duration_ms = cap_duration_ms.map_or(duration_ms, |cap| duration_ms.min(cap));
        let duration = if duration_ms == u32::MAX {
            f32::INFINITY
        } else {
            duration_ms as f32 / 1000.0
        };

        let handle = EffectHandle(self.next_id);
        self.next_id += 1;
        self.effects.push(HeldEffect {
            handle,
            effect_id,
            payload,
            attach,
            age: 0.0,
            duration,
            key,
            sfx_schedule: effect_sound(effect_id),
            sfx_last_frame: -1,
            sfx_rng: (self.next_id as u32).wrapping_mul(2654435761) ^ (effect_id.value() as u32)
                | 1,
        });
        Some(handle)
    }

    pub fn drain_queue(
        &mut self,
        queue: &mut EffectQueue,
        resolve_entity: &dyn Fn(u32) -> Option<[f32; 3]>,
    ) {
        for key in queue.drain_despawns() {
            self.despawn_by_key(key);
        }
        for req in queue.drain() {
            self.spawn_request(req, resolve_entity);
        }
        for (key, message) in queue.drain_messages() {
            for e in self.effects.iter_mut().filter(|e| e.key == Some(key)) {
                if let HeldPayload::Custom(c) = &mut e.payload {
                    c.on_message(message);
                }
            }
        }
    }

    pub fn despawn(&mut self, handle: EffectHandle) {
        let backend = self.external_backend.as_ref().cloned();
        self.effects.retain(|e| {
            if e.handle != handle {
                return true;
            }
            if let (HeldPayload::CustomExternal { handle: h }, Some(b)) = (&e.payload, &backend) {
                b.drop_handle(*h);
            }
            false
        });
    }

    /// Remove every live instance of `effect_id` attached to `entity_id`. Used
    /// to hand a caster off from one body effect to the next (High Jump's leap
    /// is deleted as the landing spawns).
    pub fn despawn_effect_on_entity(&mut self, effect_id: EffectId, entity_id: u32) {
        let backend = self.external_backend.as_ref().cloned();
        self.effects.retain(|e| {
            if e.effect_id != effect_id
                || !matches!(e.attach, Attach::Entity(id) if id == entity_id)
            {
                return true;
            }
            if let (HeldPayload::CustomExternal { handle: h }, Some(b)) = (&e.payload, &backend) {
                b.drop_handle(*h);
            }
            false
        });
    }

    pub fn despawn_by_key(&mut self, key: u32) {
        let backend = self.external_backend.as_ref().cloned();
        self.effects.retain(|e| {
            if e.key != Some(key) {
                return true;
            }
            if let (HeldPayload::CustomExternal { handle: h }, Some(b)) = (&e.payload, &backend) {
                b.drop_handle(*h);
            }
            false
        });
    }

    pub fn clear(&mut self) {
        if let Some(b) = &self.external_backend {
            b.drop_all();
        }
        self.effects.clear();
        self.shake = ShakeController::default();
        self.afterimages.clear();
        self.pending_sfx.clear();
    }

    pub fn update(
        &mut self,
        ctx: &EffectUpdateCtx,
        resolve_caster_yaw: &dyn Fn(u32) -> Option<f32>,
        resolve_entity_pos: &dyn Fn(u32) -> Option<[f32; 3]>,
    ) {
        let dt = ctx.delta;
        let backend = self.external_backend.clone();
        let mut shake_requests: Vec<CameraShake> = Vec::new();
        let mut sfx_out: Vec<SfxEmission> = Vec::new();
        self.effects.retain_mut(|e| {
            nostalre_profiling::profile_scope!(
                "effect_update",
                format!("{} {:?}", e.effect_id.value(), e.effect_id)
            );
            e.age += dt;
            let expired = e.age >= e.duration;
            let attach = e.attach;
            let caster_yaw = match attach {
                Attach::Entity(id) => resolve_caster_yaw(id),
                _ => ctx.caster_yaw,
            };
            let alive = match &mut e.payload {
                HeldPayload::Custom(c) => {
                    if let Attach::Link { caster, target } = attach {
                        match (resolve_entity_pos(caster), resolve_entity_pos(target)) {
                            (Some(a), Some(b)) => c.set_link_endpoints(a, b),
                            _ => return false,
                        }
                    }
                    if let Attach::Entity(id) = attach
                        && let Some(p) = resolve_entity_pos(id)
                    {
                        c.set_position(p);
                    }
                    let per_ctx = EffectUpdateCtx { caster_yaw, ..*ctx };
                    let running = c.update(&per_ctx) == EffectStatus::Running;
                    if let Some(s) = c.take_camera_shake() {
                        shake_requests.push(s);
                    }
                    if let Some(w) = c.take_sfx_request()
                        && let Some(pos) = resolve_position(&attach, resolve_entity_pos)
                    {
                        sfx_out.push(SfxEmission {
                            name: w.to_string(),
                            world_pos: pos,
                            pos: SfxPos::World,
                        });
                    }
                    running
                }
                HeldPayload::CustomExternal { handle } => backend
                    .as_ref()
                    .map(|b| {
                        let running = b.update(*handle, dt, caster_yaw);
                        if let Some(s) = b.take_camera_shake(*handle) {
                            shake_requests.push(s);
                        }
                        if let Some(w) = b.take_sfx(*handle)
                            && let Some(pos) = resolve_position(&attach, resolve_entity_pos)
                        {
                            sfx_out.push(SfxEmission {
                                name: w,
                                world_pos: pos,
                                pos: SfxPos::World,
                            });
                        }
                        running
                    })
                    .unwrap_or(false),
                HeldPayload::Str { .. } => true,
                HeldPayload::Spr { .. } => true,
                HeldPayload::SprBurst(b) => {
                    update_burst(b, &e.attach, dt, ctx.camera_target, resolve_entity_pos);
                    true
                }
            };
            if let Some(sched) = e.sfx_schedule {
                let cur_frame = (e.age * 60.0) as i32;
                if cur_frame > e.sfx_last_frame {
                    if let Some(pos) = resolve_position(&attach, resolve_entity_pos) {
                        emit(
                            sched,
                            e.sfx_last_frame,
                            cur_frame,
                            &mut e.sfx_rng,
                            pos,
                            &mut sfx_out,
                        );
                    }
                    e.sfx_last_frame = cur_frame;
                }
            }
            if !alive || expired {
                if let (HeldPayload::CustomExternal { handle }, Some(b)) = (&e.payload, &backend) {
                    b.drop_handle(*handle);
                }
                return false;
            }
            true
        });
        self.pending_sfx.append(&mut sfx_out);
        for s in shake_requests {
            self.shake.trigger(s);
        }
        self.shake.tick(dt);
        self.tick_afterimages(dt);
    }

    fn tick_afterimages(&mut self, dt: f32) {
        for img in &mut self.afterimages {
            img.alpha -= img.fade_per_sec * dt;
        }
        self.afterimages.retain(|i| i.alpha > 0.0);
    }

    pub fn afterimage_params_for_entity(&self, entity_id: u32) -> Option<Afterimage> {
        self.effects.iter().rev().find_map(|e| {
            if let (Attach::Entity(id), HeldPayload::Custom(c)) = (e.attach, &e.payload)
                && id == entity_id
            {
                c.body_afterimage()
            } else {
                None
            }
        })
    }

    pub fn push_afterimage(&mut self, snapshot: AfterimageSnapshot) {
        self.afterimages.push(snapshot);
    }

    pub fn afterimages_for_entity(
        &self,
        entity_id: u32,
    ) -> impl Iterator<Item = &AfterimageSnapshot> {
        self.afterimages
            .iter()
            .filter(move |i| i.entity_id == entity_id)
    }

    pub fn camera_shake_offset(&self) -> [f32; 3] {
        self.shake.offset().to_array()
    }

    /// Drop everything anchored to `entity_id`. Called when the entity dies, so
    /// hit markers and buffs do not keep playing on the corpse.
    pub fn remove_entity_effects(&mut self, entity_id: u32) {
        self.effects
            .retain(|e| e.attach != Attach::Entity(entity_id));
    }

    pub fn reposition_by_key(&mut self, key: u32, world_pos: [f32; 3]) -> bool {
        let mut moved = false;
        for e in self.effects.iter_mut().filter(|e| e.key == Some(key)) {
            e.attach = Attach::WorldPos(world_pos);
            if let HeldPayload::Custom(c) = &mut e.payload {
                c.set_position(world_pos);
            }
            moved = true;
        }
        moved
    }

    pub fn body_channels_for_entity(&self, entity_id: u32) -> crate::sprite::BodyChannels {
        let mut ch = crate::sprite::BodyChannels::default();
        for e in &self.effects {
            if let (Attach::Entity(id), HeldPayload::SprBurst(b)) = (e.attach, &e.payload)
                && id == entity_id
                && let Some(r) = b.body_recolor
            {
                let frame = (e.age * 60.0) as u32;
                if (r.window_frames.0..=r.window_frames.1).contains(&frame) && frame % 2 == 0 {
                    ch.tint = Some(r.rgb);
                }
            }
            let (id, c) = match (e.attach, &e.payload) {
                (Attach::Entity(id), HeldPayload::Custom(c)) => (id, c),
                (Attach::Link { caster, .. }, HeldPayload::Custom(c)) => (caster, c),
                _ => continue,
            };
            if id != entity_id {
                continue;
            }
            if let Some(j) = c.body_edge_jitter() {
                for (acc, v) in ch.edge_jitter.iter_mut().zip(j) {
                    *acc += v;
                }
            }
            match c.body_weapon_light() {
                nostalre_effects::WeaponLight::None => {}
                light => ch.weapon_light = light,
            }
            if let Some(t) = c.body_tint() {
                ch.tint = Some(t.rgb);
            }
            if c.body_additive() {
                ch.additive = true;
            }
            if let Some(s) = c.body_scale() {
                ch.scale *= s;
            }
            if let Some(yaw) = c.body_yaw() {
                ch.yaw += yaw;
            }
            if let Some(angle) = c.body_angle() {
                ch.angle += angle;
            }
            if let Some(v) = c.body_vertical() {
                ch.lift_px += v.lift_px;
                ch.alpha *= v.alpha;
                ch.squeeze *= v.squeeze;
            }
            if let Some(mut copies) = c.body_copies() {
                ch.copies.append(&mut copies);
            }
        }
        ch
    }

    /// Whether a red body-hit flash is running on this actor. A hovering Star
    /// Gladiator treats it as an upright pose.
    pub fn has_red_body_flash(&self, entity_id: u32) -> bool {
        self.effects.iter().any(|e| {
            e.attach == Attach::Entity(entity_id)
                && matches!(
                    e.effect_id,
                    EffectId::RedHit | EffectId::Redlightbody | EffectId::MadnessRed
                )
        })
    }

    pub fn take_body_action_for_entity(&mut self, entity_id: u32) -> Option<BodyAction> {
        for e in &mut self.effects {
            if let (Attach::Entity(id), HeldPayload::Custom(c)) = (e.attach, &mut e.payload)
                && id == entity_id
                && let Some(action) = c.take_body_action()
            {
                return Some(action);
            }
        }
        None
    }

    pub fn drain_number_requests(&mut self) -> Vec<(u32, NumberRequest)> {
        let mut out = Vec::new();
        for e in &mut self.effects {
            if let (Attach::Entity(id), HeldPayload::Custom(c)) = (e.attach, &mut e.payload)
                && let Some(req) = c.take_number_request()
            {
                out.push((id, req));
            }
        }
        out
    }

    /// Sound requests emitted by effects this frame: `(wave path, world pos)`.
    pub fn drain_sfx(&mut self) -> Vec<SfxEmission> {
        std::mem::take(&mut self.pending_sfx)
    }

    pub fn collect_custom_draws(&self, out: &mut EffectDrawList, ctx: &EffectRenderCtx) {
        for e in self.effects.iter().filter(|e| self.drawn(&e.attach)) {
            nostalre_profiling::profile_scope!(
                "effect_collect",
                format!("{} {:?}", e.effect_id.value(), e.effect_id)
            );
            match &e.payload {
                HeldPayload::Custom(c) => c.collect_draws(out, ctx),
                HeldPayload::CustomExternal { handle } => {
                    if let Some(b) = &self.external_backend {
                        b.collect(*handle, ctx, out);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn collect_spr_burst_emitters(
        &self,
        resolve_entity: &dyn Fn(u32) -> Option<[f32; 3]>,
    ) -> Vec<SprBurstSnapshot> {
        self.effects
            .iter()
            .filter_map(|e| {
                let HeldPayload::SprBurst(b) = &e.payload else {
                    return None;
                };
                if !self.drawn(&e.attach) {
                    return None;
                }
                let _ = resolve_position(&e.attach, resolve_entity)?;
                let has_keyframes = !b.params.alpha_keyframes.is_empty();
                let particles = b
                    .particles
                    .iter()
                    .map(|p| BurstParticle {
                        pos: p.pos,
                        age: p.age,
                        lifetime: p.lifetime,
                        alpha_override: has_keyframes.then_some(p.alpha),
                    })
                    .collect();
                Some(SprBurstSnapshot {
                    sprite: b.sprite.clone(),
                    size_scale: b.params.size,
                    alpha_max: b.params.alpha_max,
                    anim_speed: b.params.anim_speed,
                    size_shrink: b.params.size_shrink,
                    twinkle: b.params.twinkle,
                    particles,
                })
            })
            .collect()
    }

    pub fn collect_spr_emitters(
        &self,
        resolve_entity: &dyn Fn(u32) -> Option<[f32; 3]>,
    ) -> Vec<SprSnapshot> {
        self.effects
            .iter()
            .filter_map(|e| {
                let HeldPayload::Spr {
                    sprite,
                    size_scale,
                    anim_speed,
                    repeat,
                    tint,
                    pos_y,
                    action_index,
                    no_depth,
                    clip_offset,
                } = &e.payload
                else {
                    return None;
                };
                if !self.drawn(&e.attach) {
                    return None;
                }
                let mut pos = resolve_position(&e.attach, resolve_entity)?;
                pos[1] += pos_y;
                let duration_ms = if e.duration.is_finite() {
                    e.duration * 1000.0
                } else {
                    1000.0
                };
                Some(SprSnapshot {
                    sprite: sprite.clone(),
                    position: pos,
                    anim_time: e.age,
                    duration_ms,
                    size_scale: *size_scale,
                    anim_speed: *anim_speed,
                    repeat: *repeat,
                    tint: *tint,
                    action_index: *action_index,
                    no_depth: *no_depth,
                    clip_offset: *clip_offset,
                })
            })
            .collect()
    }

    /// Every STR the held effects play, once each, for loading what is not loaded yet
    pub fn live_str_names(&self) -> Vec<std::borrow::Cow<'_, str>> {
        use std::borrow::Cow;
        let mut names = Vec::new();
        let mut pieces = Vec::new();
        for e in &self.effects {
            match &e.payload {
                HeldPayload::Str { name, .. } => names.push(Cow::Borrowed(name.as_str())),
                HeldPayload::Custom(c) => {
                    names.extend(c.str_overlay().map(Cow::Borrowed));
                    c.str_pieces(&mut pieces);
                    names.extend(pieces.drain(..).map(|p| Cow::Borrowed(p.file)));
                }
                HeldPayload::CustomExternal { handle } => names.extend(
                    self.external_backend
                        .as_ref()
                        .and_then(|b| b.str_overlay(*handle))
                        .map(Cow::Owned),
                ),
                _ => {}
            }
        }
        names.sort();
        names.dedup();
        names
    }

    /// The STRs the held effects play this frame: each STR effect, a custom effect's overlay, and the STRs a
    /// custom effect keeps by slot
    pub fn collect_str_emitters(
        &self,
        resolve_entity: &dyn Fn(u32) -> Option<[f32; 3]>,
    ) -> Vec<StrSnapshot<'_>> {
        use std::borrow::Cow;
        let mut out = Vec::new();
        let mut pieces = Vec::new();
        for e in self.effects.iter().filter(|e| self.drawn(&e.attach)) {
            let whole: Option<(Cow<'_, str>, bool)> = match &e.payload {
                HeldPayload::Str { name, repeat } => Some((Cow::Borrowed(name.as_str()), *repeat)),
                HeldPayload::Custom(c) => {
                    c.str_pieces(&mut pieces);
                    c.str_overlay().map(|name| (Cow::Borrowed(name), false))
                }
                HeldPayload::CustomExternal { handle } => self
                    .external_backend
                    .as_ref()
                    .and_then(|b| b.str_overlay(*handle))
                    .map(|name| (Cow::Owned(name), false)),
                _ => None,
            };
            if whole.is_none() && pieces.is_empty() {
                continue;
            }
            let Some(position) = resolve_position(&e.attach, resolve_entity) else {
                pieces.clear();
                continue;
            };
            if let Some((name, repeat)) = whole {
                out.push(StrSnapshot {
                    name,
                    position,
                    view_offset: [0.0; 3],
                    scale: 1.0,
                    anim_time: e.age,
                    repeat,
                    layer: str_layer(e.effect_id),
                    turn: nostalre_effects::StrTurn::None,
                    alpha: 1.0,
                });
            }
            out.extend(pieces.drain(..).map(|p| StrSnapshot {
                name: Cow::Borrowed(p.file),
                position: p.at.unwrap_or(position),
                view_offset: p.view_offset,
                scale: p.scale,
                anim_time: p.anim_time,
                repeat: p.repeat,
                layer: if p.under_sprites {
                    StrLayer::UnderSprites
                } else {
                    StrLayer::World
                },
                turn: p.turn,
                alpha: p.alpha,
            }));
        }
        out
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn last_spawn_outcome(&self) -> Option<&SpawnOutcome> {
        self.last_spawn.as_ref()
    }

    pub fn last_spawn_status(&self, str_in_cache: impl Fn(&str) -> bool) -> Option<SpawnStatus> {
        Some(match self.last_spawn.as_ref()? {
            SpawnOutcome::Spr => SpawnStatus::Rendering,
            SpawnOutcome::SprBurst => SpawnStatus::Rendering,
            SpawnOutcome::Custom => SpawnStatus::Rendering,
            SpawnOutcome::Str { name } => {
                if str_in_cache(name) {
                    SpawnStatus::Rendering
                } else {
                    SpawnStatus::StrFileMissing
                }
            }
            SpawnOutcome::CustomNotImpl => SpawnStatus::CustomNotImpl,
            SpawnOutcome::NoSpec => SpawnStatus::NoSpec,
            SpawnOutcome::Noop => SpawnStatus::Noop,
        })
    }
}

fn resolve_position(
    attach: &Attach,
    resolve_entity: &dyn Fn(u32) -> Option<[f32; 3]>,
) -> Option<[f32; 3]> {
    match attach {
        Attach::WorldPos(p) => Some(*p),
        Attach::Entity(id) => resolve_entity(*id),
        Attach::Projectile { from, .. } => resolve_entity(*from),
        Attach::Trail { from, .. } => Some(*from),
        Attach::Link { caster, .. } => resolve_entity(*caster),
    }
}

fn attach_to_anchor(
    attach: Attach,
    resolve_entity: &dyn Fn(u32) -> Option<[f32; 3]>,
) -> EffectAnchor {
    match attach {
        Attach::WorldPos(p) => EffectAnchor::Point(p),
        Attach::Entity(id) => EffectAnchor::Point(resolve_entity(id).unwrap_or([0.0; 3])),
        Attach::Projectile { from, to } => {
            let from_pos = resolve_entity(from).unwrap_or([0.0; 3]);
            match resolve_entity(to) {
                Some(to_pos) => EffectAnchor::Trail {
                    from: from_pos,
                    to: to_pos,
                },
                None => EffectAnchor::Point(from_pos),
            }
        }
        Attach::Trail { from, to } => EffectAnchor::Trail { from, to },
        Attach::Link { caster, target } => EffectAnchor::Trail {
            from: resolve_entity(caster).unwrap_or([0.0; 3]),
            to: resolve_entity(target).unwrap_or([0.0; 3]),
        },
    }
}

fn update_burst(
    b: &mut BurstState,
    attach: &Attach,
    dt: f32,
    camera_target: Option<[f32; 3]>,
    resolve_entity_pos: &dyn Fn(u32) -> Option<[f32; 3]>,
) {
    let anchor = if b.params.follow_camera
        && let Some(p) = camera_target
    {
        p
    } else {
        resolve_position(attach, resolve_entity_pos).unwrap_or([0.0; 3])
    };

    if !b.has_emitted {
        spawn_burst(b, anchor);
        b.has_emitted = true;
        b.cooldown_timer = 0.0;
    }

    let gravity = b.params.gravity_world_per_sec2;
    let curve = b.params.curve;
    let alpha_keyframes = b.params.alpha_keyframes;
    let (speed_lo, speed_hi) = b.params.speed_range;
    let dt_frames = dt * 60.0;
    b.particles.retain_mut(|p| {
        p.age += dt;
        if p.age >= p.lifetime {
            return false;
        }
        p.age_frames += dt_frames;

        if let Some(cp) = curve {
            p.curve_timer_frames -= dt_frames;
            if p.curve_timer_frames <= 0.0 {
                p.lon_deg =
                    wrap_deg(p.lon_deg + rand_range(-cp.angle_jitter_deg, cp.angle_jitter_deg));
                p.lat_deg =
                    wrap_deg(p.lat_deg + rand_range(-cp.angle_jitter_deg, cp.angle_jitter_deg));
                let speed = if cp.speed_resample {
                    rand_range(speed_lo, speed_hi)
                } else {
                    velocity_magnitude(p.velocity) / 60.0
                };
                let (vx, vy, vz) = direction_from_lon_lat(p.lon_deg, p.lat_deg);
                let world_speed = speed * 60.0;
                p.velocity = [vx * world_speed, vy * world_speed, vz * world_speed];
                let (lo, hi) = cp.subsequent_period_frames;
                p.curve_timer_frames += rand_period_frames(lo, hi);
                p.curve_count = p.curve_count.saturating_add(1);
            }
        }

        if gravity != 0.0 {
            p.velocity[1] += gravity * dt;
        }
        p.pos[0] += p.velocity[0] * dt;
        p.pos[1] += p.velocity[1] * dt;
        p.pos[2] += p.velocity[2] * dt;

        if !alpha_keyframes.is_empty() {
            while let Some(kf) = alpha_keyframes.get(p.keyframe_idx)
                && p.age_frames >= kf.at_frame as f32
            {
                p.alpha = kf.alpha_init;
                p.alpha_max = kf.alpha_max;
                p.alpha_speed = p.alpha_max / 1.5;
                p.keyframe_idx += 1;
            }
            if p.alpha_speed >= 0.0 && p.alpha >= p.alpha_max {
                p.alpha_speed = -p.alpha_speed.abs();
            } else if p.alpha_speed < 0.0 && p.alpha <= 0.0 {
                p.alpha_speed = p.alpha_speed.abs();
            }
            p.alpha = (p.alpha + p.alpha_speed * dt_frames).clamp(0.0, p.alpha_max);
        }

        true
    });

    if let Some(period_frames) = b.params.period_frames {
        b.cooldown_timer += dt;
        let period_secs = (period_frames as f32 / 60.0).max(1e-4);
        while b.cooldown_timer >= period_secs {
            b.cooldown_timer -= period_secs;
            spawn_burst(b, anchor);
        }
    }
}

fn spawn_burst(b: &mut BurstState, anchor: [f32; 3]) {
    let (lo, hi) = b.params.burst_count_range;
    let count = if hi <= lo {
        lo
    } else {
        lo + (rand_u32() % (hi - lo + 1))
    };
    let (slo, shi) = b.params.speed_range;
    let lifetime = (b.params.particle_lifetime_ms / 1000.0).max(1e-3);
    let radius = b.params.spawn_radius_xz;
    let cone = b.params.cone_latitude_deg;
    for _ in 0..count {
        let speed = rand_range(slo, shi);
        let (ox, oz) = if radius > 0.0 {
            let r_norm = ((rand_u32() % 1000) as f32 / 1000.0).sqrt();
            let theta = (rand_u32() % 360_000) as f32 / 1000.0 * std::f32::consts::PI / 180.0;
            (radius * r_norm * theta.cos(), radius * r_norm * theta.sin())
        } else {
            (0.0, 0.0)
        };
        let (lon_deg, lat_deg, velocity) = match cone {
            None => (0.0, -90.0, [0.0, -speed * 60.0, 0.0]),
            Some((lat_min, lat_max)) => {
                let lon_deg = (rand_u32() % 360_000) as f32 / 1000.0;
                let lat_deg = rand_range(lat_min, lat_max);
                let (vx, vy, vz) = direction_from_lon_lat(lon_deg, lat_deg);
                let speed_world = speed * 60.0;
                (
                    lon_deg,
                    lat_deg,
                    [vx * speed_world, vy * speed_world, vz * speed_world],
                )
            }
        };
        let curve_timer_frames = b
            .params
            .curve
            .map(|cp| {
                let (lo, hi) = cp.initial_period_frames;
                rand_period_frames(lo, hi)
            })
            .unwrap_or(0.0);
        let (alpha, alpha_max, alpha_speed, keyframe_idx) =
            init_alpha_state(b.params.alpha_keyframes);
        b.particles.push(BurstParticleSim {
            pos: [
                anchor[0] + ox,
                anchor[1] + b.params.pos_y_start,
                anchor[2] + oz,
            ],
            velocity,
            age: 0.0,
            lifetime,
            age_frames: 0.0,
            lon_deg,
            lat_deg,
            curve_timer_frames,
            curve_count: 0,
            alpha,
            alpha_speed,
            alpha_max,
            keyframe_idx,
        });
    }
}

fn direction_from_lon_lat(lon_deg: f32, lat_deg: f32) -> (f32, f32, f32) {
    let lon = lon_deg.to_radians();
    let elev = (lat_deg - 90.0).to_radians();
    let cos_e = elev.cos();
    let sin_e = elev.sin();
    (cos_e * lon.cos(), -sin_e, cos_e * lon.sin())
}

fn velocity_magnitude(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn wrap_deg(d: f32) -> f32 {
    let mut x = d % 360.0;
    if x < 0.0 {
        x += 360.0;
    }
    x
}

fn rand_range(lo: f32, hi: f32) -> f32 {
    if hi <= lo {
        return lo;
    }
    let t = (rand_u32() % 1_000_000) as f32 / 1_000_000.0;
    lo + t * (hi - lo)
}

fn rand_period_frames(lo: u32, hi: u32) -> f32 {
    if hi <= lo {
        return lo as f32;
    }
    (lo + (rand_u32() % (hi - lo + 1))) as f32
}

fn init_alpha_state(keyframes: &[AlphaKeyframe]) -> (f32, f32, f32, usize) {
    if keyframes.is_empty() {
        return (0.0, 1.0, 0.0, 0);
    }
    let mut idx = 0;
    let mut alpha = 0.0;
    let mut alpha_max = 1.0;
    while let Some(kf) = keyframes.get(idx)
        && kf.at_frame == 0
    {
        alpha = kf.alpha_init;
        alpha_max = kf.alpha_max;
        idx += 1;
    }
    let alpha_speed = alpha_max / 1.5;
    (alpha, alpha_max, alpha_speed, idx)
}

fn rand_u32() -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    std::time::Instant::now().hash(&mut h);
    std::thread::current().id().hash(&mut h);
    h.finish() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::enums::effect_id::EffectId;
    use nostalre_effects::Attach;

    fn ctx(dt: f32) -> EffectUpdateCtx {
        EffectUpdateCtx {
            delta: dt,
            camera_target: None,
            caster_yaw: None,
        }
    }

    #[test]
    fn spawning_an_str_effect_runs_until_duration_expires() {
        let mut h = EffectHolder::new();
        let handle = h
            .spawn(
                EffectId::Bubble,
                Attach::WorldPos([0.0, 0.0, 0.0]),
                Some(2000),
            )
            .expect("spawn");
        assert_eq!(h.len(), 1);

        h.update(&ctx(1.0), &|_| None, &|_| None);
        assert_eq!(h.len(), 1, "still alive at 1s of a 2s effect");

        h.update(&ctx(1.5), &|_| None, &|_| None);
        assert!(h.is_empty(), "should have expired after total 2.5s");

        h.despawn(handle);
    }

    #[test]
    fn a_capped_effect_ends_at_its_own_length_or_the_cap_whichever_is_first() {
        let Some(EffectSpec::Str { duration_ms, .. }) = effect_spec(EffectId::Bubble) else {
            panic!("Bubble is an STR effect");
        };
        let natural = duration_ms as f32 / 1000.0;
        for (cap_ms, lives) in [(duration_ms / 2, natural / 2.0), (duration_ms * 3, natural)] {
            let mut q = EffectQueue::default();
            q.spawn_on_capped(EffectId::Bubble, 7, cap_ms);
            let mut h = EffectHolder::new();
            h.drain_queue(&mut q, &|_| Some([0.0; 3]));
            h.update(&ctx(lives - 0.05), &|_| None, &|_| Some([0.0; 3]));
            assert_eq!(h.len(), 1, "alive just before {lives}s (cap {cap_ms} ms)");
            h.update(&ctx(0.1), &|_| None, &|_| Some([0.0; 3]));
            assert!(h.is_empty(), "gone just after {lives}s (cap {cap_ms} ms)");
        }
    }

    #[test]
    fn a_class_ending_with_its_strs_ends_once_they_have_played() {
        let mut h = EffectHolder::new();
        h.spawn(EffectId::Kro1911, Attach::WorldPos([0.0; 3]), None)
            .expect("Petitio's class");
        h.update(&ctx(0.2), &|_| None, &|_| None);
        h.end_played_strs(&|_| None);
        assert_eq!(h.len(), 1, "held while its files' lengths are not known");
        h.end_played_strs(&|_| Some(0.5));
        assert_eq!(h.len(), 1, "still playing");
        h.end_played_strs(&|_| Some(0.1));
        assert!(h.is_empty(), "both its STRs played");
    }

    #[test]
    fn a_body_effect_is_built_from_its_rule_held_and_taken_off_by_key() {
        use nostalre_effects::effects::body_aura::{BodyAuraSpec, GlowLayer};
        let spec = BodyAuraSpec {
            glow: Some(GlowLayer {
                color: [90, 50, 255],
                size: 5.0,
                breath: 1.5,
                intensity: 0.7,
            }),
            ..Default::default()
        };
        let mut q = EffectQueue::default();
        q.spawn_body_aura(7, 3, spec, 1);
        let mut h = EffectHolder::new();
        h.drain_queue(&mut q, &|id| (id == 7).then_some([10.0, 0.0, 5.0]));
        assert_eq!(h.len(), 1);
        for _ in 0..20 {
            h.update(&ctx(60.0), &|_| None, &|id| {
                (id == 7).then_some([10.0, 0.0, 5.0])
            });
        }
        assert_eq!(h.len(), 1, "held twenty minutes: only its key takes it off");
        let copies = h.body_channels_for_entity(7).copies;
        assert_eq!(copies.len(), 1);
        assert_eq!(copies[0].tint, [90, 50, 255]);
        q.despawn(3);
        h.drain_queue(&mut q, &|_| None);
        assert!(h.is_empty());
    }

    #[test]
    fn entity_attached_effect_follows_master_each_frame() {
        use std::sync::{Arc, Mutex};
        struct FollowFake {
            seen: Arc<Mutex<Vec<[f32; 3]>>>,
        }
        impl GameEffect for FollowFake {
            fn update(&mut self, _: &EffectUpdateCtx) -> EffectStatus {
                EffectStatus::Running
            }
            fn collect_draws(&self, _: &mut EffectDrawList, _: &EffectRenderCtx) {}
            fn set_position(&mut self, pos: [f32; 3]) {
                self.seen.lock().unwrap().push(pos);
            }
        }
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut h = EffectHolder::new();
        h.effects.push(HeldEffect {
            handle: EffectHandle(1),
            effect_id: EffectId::Beginspell,
            payload: HeldPayload::Custom(Box::new(FollowFake { seen: seen.clone() })),
            attach: Attach::Entity(7),
            age: 0.0,
            duration: f32::INFINITY,
            key: None,
            sfx_schedule: None,
            sfx_last_frame: -1,
            sfx_rng: 1,
        });
        h.update(&ctx(1.0 / 60.0), &|_| None, &|id| {
            (id == 7).then_some([10.0, 0.0, 5.0])
        });
        h.update(&ctx(1.0 / 60.0), &|_| None, &|id| {
            (id == 7).then_some([11.0, 0.0, 5.0])
        });
        assert_eq!(
            *seen.lock().unwrap(),
            vec![[10.0, 0.0, 5.0], [11.0, 0.0, 5.0]],
        );
    }

    #[test]
    fn custom_effect_self_emitted_sfx_reaches_drain() {
        struct SfxFake {
            pending: Option<&'static str>,
        }
        impl GameEffect for SfxFake {
            fn update(&mut self, _: &EffectUpdateCtx) -> EffectStatus {
                EffectStatus::Running
            }
            fn collect_draws(&self, _: &mut EffectDrawList, _: &EffectRenderCtx) {}
            fn take_sfx_request(&mut self) -> Option<&'static str> {
                self.pending.take()
            }
        }
        let mut h = EffectHolder::new();
        h.effects.push(HeldEffect {
            handle: EffectHandle(1),
            effect_id: EffectId::Portal4,
            payload: HeldPayload::Custom(Box::new(SfxFake {
                pending: Some("effect\\윈드워크.wav"),
            })),
            attach: Attach::Entity(7),
            age: 0.0,
            duration: f32::INFINITY,
            key: None,
            sfx_schedule: None,
            sfx_last_frame: -1,
            sfx_rng: 1,
        });
        h.update(&ctx(1.0 / 60.0), &|_| None, &|id| {
            (id == 7).then_some([10.0, 0.0, 5.0])
        });
        assert_eq!(
            h.drain_sfx(),
            vec![SfxEmission {
                name: "effect\\윈드워크.wav".to_string(),
                world_pos: [10.0, 0.0, 5.0],
                pos: SfxPos::World,
            }],
        );
    }

    #[test]
    fn despawn_by_key_drops_only_matching_effects() {
        let mut h = EffectHolder::new();
        let mut push_keyed = |handle: u64, key: Option<u32>| {
            h.effects.push(HeldEffect {
                handle: EffectHandle(handle),
                effect_id: EffectId::Beginspell,
                payload: HeldPayload::Str {
                    name: "x".to_string(),
                    repeat: false,
                },
                attach: Attach::WorldPos([0.0; 3]),
                age: 0.0,
                duration: f32::INFINITY,
                key,
                sfx_schedule: None,
                sfx_last_frame: -1,
                sfx_rng: 1,
            });
        };
        push_keyed(1, Some(7));
        push_keyed(2, Some(7));
        push_keyed(3, Some(9));
        push_keyed(4, None);
        assert_eq!(h.len(), 4);

        h.despawn_by_key(7);
        assert_eq!(h.len(), 2);
        h.despawn_by_key(123);
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn reposition_by_key_moves_existing_unit_without_duplicating() {
        use std::sync::{Arc, Mutex};
        struct MoveFake {
            seen: Arc<Mutex<Vec<[f32; 3]>>>,
        }
        impl GameEffect for MoveFake {
            fn update(&mut self, _: &EffectUpdateCtx) -> EffectStatus {
                EffectStatus::Running
            }
            fn collect_draws(&self, _: &mut EffectDrawList, _: &EffectRenderCtx) {}
            fn set_position(&mut self, pos: [f32; 3]) {
                self.seen.lock().unwrap().push(pos);
            }
        }
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut h = EffectHolder::new();
        h.effects.push(HeldEffect {
            handle: EffectHandle(1),
            effect_id: EffectId::Beginspell,
            payload: HeldPayload::Custom(Box::new(MoveFake { seen: seen.clone() })),
            attach: Attach::WorldPos([105.0, 0.0, 154.0]),
            age: 0.0,
            duration: f32::INFINITY,
            key: Some(1312),
            sfx_schedule: None,
            sfx_last_frame: -1,
            sfx_rng: 1,
        });

        assert!(h.reposition_by_key(1312, [112.0, 0.0, 154.0]));
        assert_eq!(h.len(), 1);
        assert_eq!(*seen.lock().unwrap(), vec![[112.0, 0.0, 154.0]]);
        assert!(!h.reposition_by_key(9999, [0.0; 3]));
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn quakebody_attached_to_entity_jitters_and_tints_only_that_entity() {
        let mut h = EffectHolder::new();
        h.spawn(EffectId::Quakebody4, Attach::Entity(7), None)
            .expect("spawn");
        h.update(&ctx(25.0 / 60.0), &|_| None, &|_| None);

        let ch = h.body_channels_for_entity(7);
        assert_ne!(ch.edge_jitter, [0.0; 4]);
        assert!(ch.tint.is_some());
        let other = h.body_channels_for_entity(99);
        assert_eq!(other.edge_jitter, [0.0; 4]);
        assert!(other.tint.is_none());
    }

    #[test]
    fn twohand_quicken_emits_and_decays_afterimage_for_attached_entity() {
        let mut h = EffectHolder::new();
        h.spawn(EffectId::Twohandquicken, Attach::Entity(7), None)
            .expect("spawn");

        let ai = h
            .afterimage_params_for_entity(7)
            .expect("Quicken leaves a trail");
        assert_eq!(ai.tint, [200, 200, 0]);
        assert!(
            h.afterimage_params_for_entity(99).is_none(),
            "only the attached entity trails"
        );

        h.push_afterimage(AfterimageSnapshot::new(
            7,
            SpriteAnimationState::new(0),
            Some(0),
            0,
            (10.0, 20.0),
            [10.0, 20.0],
            0.5,
            1.0,
            &ai,
        ));
        assert_eq!(h.afterimages_for_entity(7).count(), 1);
        assert!(h.afterimages_for_entity(99).next().is_none());

        h.update(&ctx(1.0), &|_| None, &|_| None);
        assert_eq!(h.afterimages_for_entity(7).count(), 0);
    }

    #[test]
    fn screen_quake_spawn_triggers_and_settles_camera_shake() {
        let mut h = EffectHolder::new();
        assert_eq!(h.camera_shake_offset(), [0.0, 0.0, 0.0]);
        h.spawn(EffectId::ScreenQuake, Attach::WorldPos([0.0; 3]), None);
        h.update(&ctx(0.05), &|_| None, &|_| None);
        assert_ne!(h.camera_shake_offset(), [0.0, 0.0, 0.0]);
        h.update(&ctx(3.0), &|_| None, &|_| None);
        assert_eq!(h.camera_shake_offset(), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn factory_dispatched_warp_spawns_and_reports_rendering() {
        let mut h = EffectHolder::new();
        let handle = h.spawn(EffectId::Warp, Attach::WorldPos([0.0, 0.0, 0.0]), Some(500));
        assert!(handle.is_some());
        assert_eq!(h.last_spawn_status(|_| false), Some(SpawnStatus::Rendering));
    }

    #[test]
    fn factory_unimplemented_custom_falls_back_to_placeholder() {
        let mut h = EffectHolder::new();
        let handle = h.spawn(
            EffectId::Icewall,
            Attach::WorldPos([0.0, 0.0, 0.0]),
            Some(500),
        );
        assert!(handle.is_some());
        assert_eq!(h.last_spawn_status(|_| false), Some(SpawnStatus::Rendering));
    }

    #[test]
    fn str_spawn_status_depends_on_cache_lookup() {
        let mut h = EffectHolder::new();
        h.spawn(
            EffectId::Bubble,
            Attach::WorldPos([0.0, 0.0, 0.0]),
            Some(1000),
        )
        .expect("spawn");
        assert_eq!(h.last_spawn_status(|_| true), Some(SpawnStatus::Rendering));
        assert_eq!(
            h.last_spawn_status(|_| false),
            Some(SpawnStatus::StrFileMissing)
        );
    }

    #[test]
    fn custom_effect_with_str_overlay_emits_snapshot() {
        struct HybridFake;
        impl GameEffect for HybridFake {
            fn update(&mut self, _: &EffectUpdateCtx) -> EffectStatus {
                EffectStatus::Running
            }
            fn collect_draws(&self, _: &mut EffectDrawList, _: &EffectRenderCtx) {}
            fn str_overlay(&self) -> Option<&'static str> {
                Some("stormgust")
            }
        }
        let mut h = EffectHolder::new();
        h.effects.push(HeldEffect {
            handle: EffectHandle(1),
            effect_id: EffectId::Beginspell,
            payload: HeldPayload::Custom(Box::new(HybridFake)),
            attach: Attach::WorldPos([1.0, 2.0, 3.0]),
            age: 0.5,
            duration: 10.0,
            key: None,
            sfx_schedule: None,
            sfx_last_frame: -1,
            sfx_rng: 1,
        });
        let snaps = h.collect_str_emitters(&|_| None);
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].name, "stormgust");
        assert_eq!(snaps[0].position, [1.0, 2.0, 3.0]);
        assert!((snaps[0].anim_time - 0.5).abs() < 1e-6);
    }

    #[test]
    fn custom_effect_without_overlay_emits_no_str_snapshot() {
        let mut h = EffectHolder::new();
        h.spawn(EffectId::Warp, Attach::WorldPos([0.0, 0.0, 0.0]), Some(500))
            .expect("spawn");
        let snaps = h.collect_str_emitters(&|_| None);
        assert!(snaps.is_empty());
    }

    #[test]
    fn spr_spawn_emits_snapshot_with_sprite_and_anim_time() {
        let mut h = EffectHolder::new();
        h.spawn(
            EffectId::Torch,
            Attach::WorldPos([10.0, 20.0, 30.0]),
            Some(2000),
        )
        .expect("spawn");
        h.update(&ctx(0.25), &|_| None, &|_| None);
        let snaps = h.collect_spr_emitters(&|_| None);
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].sprite, "data/sprite/이팩트/torch_01");
        assert_eq!(snaps[0].position, [10.0, 20.0, 30.0]);
        assert!((snaps[0].anim_time - 0.25).abs() < 1e-6);
        assert_eq!(snaps[0].action_index, 0);
    }

    #[test]
    fn spr_snapshot_carries_non_zero_act_action() {
        let mut h = EffectHolder::new();
        h.spawn(
            EffectId::Vallentine2,
            Attach::WorldPos([0.0, 0.0, 0.0]),
            Some(1000),
        )
        .expect("spawn");
        h.update(&ctx(0.1), &|_| None, &|_| None);
        let snaps = h.collect_spr_emitters(&|_| None);
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].action_index, 1);
    }

    #[test]
    fn spr_burst_spawns_initial_particles_and_drifts_them() {
        let mut h = EffectHolder::new();
        h.spawn(EffectId::Smoke, Attach::WorldPos([0.0, 0.0, 0.0]), None)
            .expect("spawn");
        h.update(&ctx(0.1), &|_| None, &|_| None);
        let snaps = h.collect_spr_burst_emitters(&|_| None);
        assert_eq!(snaps.len(), 1);
        let snap = &snaps[0];
        assert_eq!(snap.sprite, "data/sprite/이팩트/굴뚝연기");
        assert!(
            (1..=4).contains(&snap.particles.len()),
            "burst count out of range: {}",
            snap.particles.len()
        );
        for sp in &snap.particles {
            assert!(
                sp.pos[1] < -9.0,
                "particle should have drifted past pos_y_start=-9: {:?}",
                sp.pos
            );
            assert!((sp.age - 0.1).abs() < 1e-5, "age should track dt");
        }
    }

    #[test]
    fn steal_bursts_ten_gravity_arc_particles_that_scatter_in_xz() {
        let mut h = EffectHolder::new();
        h.spawn(EffectId::Steal, Attach::WorldPos([0.0, 0.0, 0.0]), None)
            .expect("spawn");
        h.update(&ctx(0.05), &|_| None, &|_| None);
        let snaps = h.collect_spr_burst_emitters(&|_| None);
        assert_eq!(snaps.len(), 1);
        let snap = &snaps[0];
        assert_eq!(snap.sprite, "data/sprite/이팩트/particle7");
        assert!(snap.size_shrink, "Steal particles must shrink to 0");
        assert!(!snap.twinkle, "Steal does not twinkle");
        assert_eq!(
            snap.particles.len(),
            10,
            "Steal spawns exactly 10 particles"
        );
        let any_xz_motion = snap
            .particles
            .iter()
            .any(|sp| sp.pos[0].abs() > 0.05 || sp.pos[2].abs() > 0.05);
        assert!(
            any_xz_motion,
            "cone scatter should give at least one particle non-zero XZ drift after one tick"
        );
    }

    #[test]
    fn entity_anchored_burst_emits_at_resolved_entity_position() {
        let mut h = EffectHolder::new();
        h.spawn(EffectId::Steal, Attach::Entity(7), None)
            .expect("spawn");
        let resolve = |id: u32| (id == 7).then_some([100.0, 50.0, 200.0]);
        h.update(&ctx(0.05), &|_| None, &resolve);
        let snaps = h.collect_spr_burst_emitters(&resolve);
        let snap = &snaps[0];
        for sp in &snap.particles {
            assert!(
                (sp.pos[0] - 100.0).abs() < 5.0 && (sp.pos[2] - 200.0).abs() < 5.0,
                "particle must spawn on the target entity, not the map origin: {:?}",
                sp.pos
            );
        }
    }

    #[test]
    fn firefly_propagates_twinkle_and_cone_flags() {
        let mut h = EffectHolder::new();
        h.spawn(EffectId::Firefly, Attach::WorldPos([0.0, 0.0, 0.0]), None)
            .expect("spawn");
        h.update(&ctx(0.05), &|_| None, &|_| None);
        let snaps = h.collect_spr_burst_emitters(&|_| None);
        assert_eq!(snaps.len(), 1);
        assert!(snaps[0].twinkle);
        assert!(!snaps[0].size_shrink);
    }

    #[test]
    fn firefly_spawn_directions_span_full_sphere() {
        let mut h = EffectHolder::new();
        for _ in 0..40 {
            h.spawn(EffectId::Firefly, Attach::WorldPos([0.0, 0.0, 0.0]), None)
                .expect("spawn");
        }
        h.update(&ctx(1.0 / 60.0), &|_| None, &|_| None);
        let snaps = h.collect_spr_burst_emitters(&|_| None);
        let mut saw_up = false;
        let mut saw_down = false;
        for s in &snaps {
            for p in &s.particles {
                let dy = p.pos[1] - (-10.0);
                if dy < -0.05 {
                    saw_up = true;
                }
                if dy > 0.05 {
                    saw_down = true;
                }
            }
        }
        assert!(saw_up, "firefly must sometimes drift upward (vy<0)");
        assert!(saw_down, "firefly must sometimes drift downward (vy>0)");
    }

    #[test]
    fn firefly_pt_curve_perturbs_velocity_within_30_frames() {
        let mut h = EffectHolder::new();
        h.spawn(EffectId::Firefly, Attach::WorldPos([0.0, 0.0, 0.0]), None)
            .expect("spawn");
        h.update(&ctx(1.0 / 60.0), &|_| None, &|_| None);
        let snap0 = h
            .collect_spr_burst_emitters(&|_| None)
            .into_iter()
            .next()
            .expect("snapshot");
        let p0 = snap0.particles[0].pos;
        for _ in 0..30 {
            h.update(&ctx(1.0 / 60.0), &|_| None, &|_| None);
        }
        let snap1 = h
            .collect_spr_burst_emitters(&|_| None)
            .into_iter()
            .next()
            .expect("snapshot still alive");
        let p1 = snap1.particles[0].pos;
        let dist =
            ((p1[0] - p0[0]).powi(2) + (p1[1] - p0[1]).powi(2) + (p1[2] - p0[2]).powi(2)).sqrt();
        assert!(
            dist > 0.5,
            "particle should drift across 0.5s window: {dist}"
        );
        assert!(dist.is_finite());
    }

    #[test]
    fn firefly_alpha_keyframes_drive_per_particle_alpha_override() {
        let mut h = EffectHolder::new();
        h.spawn(EffectId::Firefly, Attach::WorldPos([0.0, 0.0, 0.0]), None)
            .expect("spawn");
        h.update(&ctx(1.0 / 60.0), &|_| None, &|_| None);
        let snap = h
            .collect_spr_burst_emitters(&|_| None)
            .into_iter()
            .next()
            .expect("snapshot");
        let a_early = snap.particles[0]
            .alpha_override
            .expect("keyframes should populate alpha_override");
        assert!(
            a_early >= 0.0 && a_early <= 200.0 / 255.0 + 1e-3,
            "early alpha within ceiling: {a_early}",
        );

        for _ in 0..39 {
            h.update(&ctx(1.0 / 60.0), &|_| None, &|_| None);
        }
        let mut peak_bright: f32 = 0.0;
        for _ in 0..20 {
            h.update(&ctx(1.0 / 60.0), &|_| None, &|_| None);
            if let Some(snap) = h.collect_spr_burst_emitters(&|_| None).into_iter().next()
                && let Some(a) = snap.particles[0].alpha_override
            {
                peak_bright = peak_bright.max(a);
            }
        }
        assert!(
            peak_bright > 80.0 / 255.0,
            "bright phase peak should exceed the dim ceiling: {peak_bright}",
        );
    }

    #[test]
    fn a_units_servants_live_and_hand_out_their_strs_by_slot() {
        let mut h = EffectHolder::new();
        let mut q = EffectQueue::new();
        q.spawn_on_keyed_counted(EffectId::Kro1694, 7, 1, 3);
        h.drain_queue(&mut q, &|_| Some([0.0; 3]));
        assert_eq!(h.len(), 1);
        let ctx = EffectUpdateCtx {
            delta: 1.0 / 60.0,
            camera_target: None,
            caster_yaw: None,
        };
        h.update(&ctx, &|_| None, &|_| Some([10.0, 0.0, 10.0]));
        let strs = h.collect_str_emitters(&|_| Some([10.0, 0.0, 10.0]));
        assert_eq!(strs.len(), 5, "three coming, two shooting off");
        assert!(strs.iter().all(|s| s.layer == StrLayer::UnderSprites));
        q.send(1, nostalre_effects::EffectMessage::Count(5));
        h.drain_queue(&mut q, &|_| None);
        let names = h.live_str_names();
        assert!(names.iter().any(|n| n.ends_with("servantweapon_cast")));
    }

    #[test]
    fn an_aimed_kro_class_made_on_a_link_draws_turned_toward_its_target() {
        let mut h = EffectHolder::new();
        let mut q = EffectQueue::new();
        q.spawn_link(EffectId::Kro2114, 7, 8);
        let at = |id: u32| Some(if id == 7 { [0.0; 3] } else { [30.0, 0.0, 0.0] });
        h.drain_queue(&mut q, &at);
        let ctx = EffectUpdateCtx {
            delta: 1.0 / 60.0,
            camera_target: None,
            caster_yaw: None,
        };
        h.update(&ctx, &|_| None, &at);
        let strs = h.collect_str_emitters(&at);
        let [shot] = &strs[..] else {
            panic!("one STR: {}", strs.len());
        };
        assert_eq!(shot.name, "frenzy_shot/frenzy_shot/frenzy_shot");
        assert_eq!(shot.position, [0.0, -4.0, 0.0]);
        assert!(matches!(
            shot.turn,
            nostalre_effects::StrTurn::Toward { .. }
        ));
    }

    #[test]
    fn a_hidden_owners_effects_run_undrawn_and_show_again() {
        let mut h = EffectHolder::new();
        let mut q = EffectQueue::new();
        q.spawn_on_keyed_counted(EffectId::Kro1135, 7, 1, 7);
        q.spawn_on_keyed_counted(EffectId::Kro1694, 7, 2, 5);
        h.drain_queue(&mut q, &|_| Some([0.0; 3]));
        let at = |_| Some([10.0, 0.0, 10.0]);
        let ctx = EffectRenderCtx {
            camera: nostalre_effects::CameraView {
                eye: [0.0, -50.0, -50.0],
                target: [0.0; 3],
                up: [0.0, -1.0, 0.0],
            },
            screen_w: 800.0,
            screen_h: 600.0,
            elapsed: 0.0,
        };
        let drawn = |h: &EffectHolder| {
            let mut draws = EffectDrawList::new();
            h.collect_custom_draws(&mut draws, &ctx);
            (draws.primitives.len(), h.collect_str_emitters(&at).len())
        };
        assert_eq!(drawn(&h), (7, 5), "seven coins and glows, five servants");
        h.set_unseen_owners([7]);
        assert_eq!(drawn(&h), (0, 0));
        assert_eq!(h.len(), 2, "kept alive");
        h.set_unseen_owners([]);
        assert_eq!(drawn(&h), (7, 5));
    }

    #[test]
    fn drain_queue_pulls_pending_requests() {
        let mut h = EffectHolder::new();
        let mut q = EffectQueue::new();
        q.spawn_at(EffectId::Bubble, [1.0, 2.0, 3.0]);
        q.spawn_at(EffectId::Gaspush, [0.0, 0.0, 0.0]);
        h.drain_queue(&mut q, &|_| None);
        assert_eq!(h.len(), 2);
        assert!(q.pending.is_empty());
    }
}
