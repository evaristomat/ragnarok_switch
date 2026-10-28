//! Effect art loads a little each frame instead of all at once on the first map: there it took seconds on the
//! RP6, the input queue went unread and Android called the game not responding (2026-10-21). First what draws
//! wrong when missing (the hand-written effects' textures draw white, their sprites not at all), then the common
//! effects (hits, heals, level up, warps), then the player's own skills once the skill list comes, then the rest.
//! An STR effect played before its turn loads when it plays (`game_updates`), with the names tried after its
//! first.
//!
//! Switch prepares whole assets on one bounded worker, including decoding and
//! device uploads. Frames only publish completed entries; the time budget below
//! is retained for the desktop path and a worker-start failure fallback.
use crate::App;
use models::enums::effect_id::EffectId;
use models::enums::skill_enums::SkillEnum;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

#[cfg(any(target_os = "horizon", test))]
#[path = "effect_background_worker.rs"]
mod background;
#[cfg(any(target_os = "horizon", test))]
use background::BackgroundWorker;

#[cfg(any(target_os = "horizon", test))]
enum Prepared {
    Texture(Option<(nostalre_renderer::wgpu::BindGroup, u32, u32)>),
    Sprite(Option<nostalre_renderer::EffectSpriteEntry>),
    Str(Option<nostalre_renderer::StrEffectEntry>),
}
#[cfg(any(target_os = "horizon", test))]
struct Completed {
    asset: Asset,
    filtering: bool,
    prepared: Prepared,
}

#[cfg(any(target_os = "horizon", test))]
fn prepare_asset(
    asset: Asset,
    filtering: bool,
    grf: &nostalre_formats::grf::GrfArchive,
    device: &nostalre_renderer::wgpu::Device,
    queue: &nostalre_renderer::wgpu::Queue,
    layout: &nostalre_renderer::wgpu::BindGroupLayout,
) -> Completed {
    use nostalre_renderer::{EffectSpriteCache, Renderer, StrEffectCache};
    let prepared = match &asset {
        Asset::Texture(path) => Prepared::Texture(Renderer::prepare_effect_texture(
            path, grf, device, queue, layout, filtering,
        )),
        Asset::Sprite(path) => Prepared::Sprite(EffectSpriteCache::prepare(
            path, grf, device, queue, layout, filtering,
        )),
        Asset::Str(names) => Prepared::Str(StrEffectCache::prepare(
            names[0],
            &names[1..],
            grf,
            layout,
            device,
            queue,
            filtering,
        )),
    };
    Completed {
        asset,
        filtering,
        prepared,
    }
}

/// A frame's share of loading; one item goes each frame even when it alone takes longer
const FRAME_BUDGET: Duration = Duration::from_millis(4);

/// Effects nearly every session shows soon: hits, heals, levels, refining and brewing, warps, items, MVPs
const COMMON: [EffectId; 23] = [
    EffectId::Hit1,
    EffectId::Hit2,
    EffectId::Hitline7,
    EffectId::Heal,
    EffectId::Heal2,
    EffectId::Heal3,
    EffectId::Heal4,
    EffectId::Angel,
    EffectId::Angel2,
    EffectId::Angel3,
    EffectId::Joblvup,
    EffectId::Joblvup50,
    EffectId::Refineok,
    EffectId::Refinefail,
    EffectId::Warpzone2,
    EffectId::Portal,
    EffectId::Portal2,
    EffectId::Readyportal2,
    EffectId::Teleportation2,
    EffectId::Entry2,
    EffectId::GetItem,
    EffectId::Mvp,
    EffectId::Lockon,
];

#[derive(Clone)]
enum Asset {
    Texture(String),
    Sprite(&'static str),
    /// An STR effect: its first name, then the names tried after it
    Str(&'static [&'static str]),
}

pub(crate) struct EffectLoader {
    queue: VecDeque<Asset>,
    /// Every STR effect's names by its first, for one loaded when it plays
    str_names: HashMap<&'static str, &'static [&'static str]>,
    started: Option<Instant>,
    loaded: usize,
    next_background: Option<Instant>,
    #[cfg(any(target_os = "horizon", test))]
    worker: Option<BackgroundWorker<(Asset, bool), Completed>>,
    #[cfg(any(target_os = "horizon", test))]
    inflight: Option<Asset>,
    #[cfg(any(target_os = "horizon", test))]
    worker_failed: bool,
}

impl EffectLoader {
    #[cfg(test)]
    pub(crate) fn background_worker_healthy(&self) -> bool {
        self.worker.is_some() && !self.worker_failed
    }
    pub(crate) fn new() -> Self {
        let all_strs = nostalre_game::effect::effect_str_names();
        let str_names = all_strs.iter().map(|names| (names[0], *names)).collect();
        let mut queue: VecDeque<Asset> = nostalre_game::effect::effect_texture_paths()
            .into_iter()
            .map(Asset::Texture)
            .collect();
        let mut sprites: Vec<&'static str> = nostalre_game::effect::effect_spr_paths();
        sprites.extend(nostalre_game::effect::custom_effect_sprite_paths());
        sprites.extend(nostalre_game::effect::skill_unit_sprite_paths());
        sprites.sort();
        sprites.dedup();
        queue.extend(sprites.into_iter().map(Asset::Sprite));
        let common = COMMON
            .into_iter()
            .filter_map(nostalre_game::effect::str_names_of);
        queue.extend(common.map(Asset::Str));
        // STRs already have an on-demand path (live_str_names). Uploading every
        // registered skill's animation here kept >1 GiB alive before map swaps.
        // Switch warms common effects; the full alias registry remains above.
        #[cfg(not(any(target_os = "horizon", test)))]
        queue.extend(all_strs.into_iter().map(Asset::Str));
        Self {
            queue,
            str_names,
            started: None,
            loaded: 0,
            next_background: None,
            #[cfg(any(target_os = "horizon", test))]
            worker: None,
            #[cfg(any(target_os = "horizon", test))]
            inflight: None,
            #[cfg(any(target_os = "horizon", test))]
            worker_failed: false,
        }
    }

    /// The names an STR effect is also found under, after `name`
    pub(crate) fn other_names(&self, name: &str) -> &'static [&'static str] {
        self.str_names.get(name).map_or(&[], |names| &names[1..])
    }

    /// The player's skills come first: their STR effects go to the front of what is left
    pub(crate) fn put_skills_first(&mut self, skills: impl Iterator<Item = SkillEnum>) {
        if self.queue.is_empty() {
            return;
        }
        for skill in skills {
            let ids = nostalre_game::effect::skill_effect_ids(skill);
            for names in ids
                .into_iter()
                .filter_map(nostalre_game::effect::str_names_of)
            {
                self.queue.push_front(Asset::Str(names));
            }
        }
    }
}

impl App {
    /// A frame's share of the effect art; nothing once it is all in
    pub(crate) fn load_effects_in_background(&mut self) {
        #[cfg(any(target_os = "horizon", test))]
        self.load_effects_asynchronously();
        #[cfg(not(any(target_os = "horizon", test)))]
        self.load_effects_synchronously();
    }

    /// Desktop path and the fallback if a worker cannot be started.
    fn load_effects_synchronously(&mut self) {
        let (Some(renderer), Some(grf)) = (self.renderer.as_mut(), self.grf.as_ref()) else {
            return;
        };
        let loader = &mut self.effect_loader;
        if loader.queue.is_empty() {
            return;
        }
        let start = Instant::now();
        let started = *loader.started.get_or_insert(start);
        while let Some(asset) = loader.queue.pop_front() {
            match asset {
                Asset::Texture(path) => {
                    renderer.load_effect_texture(&path, grf);
                }
                Asset::Sprite(path) => {
                    self.effect_sprites.load(
                        path,
                        grf,
                        &renderer.device.device,
                        &renderer.device.queue,
                        &renderer.texture_cache.bind_group_layout,
                    );
                }
                Asset::Str(names) => {
                    self.str_effects.load(
                        names[0],
                        &names[1..],
                        grf,
                        &mut renderer.texture_cache,
                        &renderer.device.device,
                        &renderer.device.queue,
                    );
                }
            }
            loader.loaded += 1;
            if start.elapsed() >= FRAME_BUDGET {
                break;
            }
        }
        if loader.queue.is_empty() {
            tracing::info!(
                "Effect art loaded in the background: {} items in {:.1} s",
                loader.loaded,
                started.elapsed().as_secs_f32()
            );
        }
    }

    #[cfg(any(target_os = "horizon", test))]
    fn load_effects_asynchronously(&mut self) {
        use std::sync::mpsc::TrySendError;
        if self.effect_loader.worker_failed {
            self.load_effects_synchronously();
            return;
        }
        let (Some(renderer), Some(grf)) = (self.renderer.as_mut(), self.grf.as_ref()) else {
            return;
        };
        let filtering = self.config.custom.filtering.effects;
        let loader = &mut self.effect_loader;
        if loader.queue.is_empty()
            && loader
                .worker
                .as_ref()
                .is_none_or(|worker| !worker.is_busy())
        {
            return;
        }
        if loader.worker.is_none() {
            let grf = grf.clone();
            let device = renderer.device.device.clone();
            let queue = renderer.device.queue.clone();
            let layout = renderer.texture_cache.bind_group_layout.clone();
            match BackgroundWorker::new(
                "effect-preload",
                move |(asset, filtering): (Asset, bool)| {
                    prepare_asset(asset, filtering, &grf, &device, &queue, &layout)
                },
            ) {
                Ok(worker) => loader.worker = Some(worker),
                Err(error) => {
                    tracing::error!(
                        "Cannot start effect preload worker; using synchronous fallback: {error}"
                    );
                    loader.worker_failed = true;
                    self.load_effects_synchronously();
                    return;
                }
            }
        }
        match loader.worker.as_mut().unwrap().poll() {
            Ok(Some(completed)) => {
                loader.inflight = None;
                if completed.filtering != filtering {
                    // A setting changed while this resource was being prepared.
                    // Retry with the current sampler instead of publishing stale art.
                    loader.queue.push_front(completed.asset);
                } else {
                    match (completed.asset, completed.prepared) {
                        (Asset::Texture(path), Prepared::Texture(ready)) => {
                            renderer.install_effect_texture(&path, ready);
                        }
                        (Asset::Sprite(path), Prepared::Sprite(ready)) => {
                            self.effect_sprites.install_prepared(path, ready);
                        }
                        (Asset::Str(names), Prepared::Str(ready)) => {
                            self.str_effects.install_prepared(names[0], ready);
                        }
                        _ => unreachable!("prepared effect kind differs from its job"),
                    }
                    loader.loaded += 1;
                }
            }
            Ok(None) => {}
            Err(error) => {
                tracing::error!(
                    "Effect preload worker disconnected; using synchronous fallback: {error}"
                );
                if let Some(asset) = loader.inflight.take() {
                    loader.queue.push_front(asset);
                }
                loader.worker = None;
                loader.worker_failed = true;
                self.load_effects_synchronously();
                return;
            }
        }
        if loader.worker.as_ref().unwrap().is_busy() {
            return;
        }
        let now = Instant::now();
        if loader.next_background.is_some_and(|next| now < next) { return; }
        let moving = self.game.world.entities.player().is_some_and(|p| p.movement.is_moving());
        loader.next_background = Some(now + Duration::from_millis(if moving { 100 } else { 16 }));
        // Cached duplicate aliases cost no upload. Bound that bookkeeping too.
        for _ in 0..32 {
            let Some(asset) = loader.queue.pop_front() else {
                if let Some(started) = loader.started {
                    tracing::info!(
                        "Effect art prepared off the frame thread: {} items in {:.1} s",
                        loader.loaded,
                        started.elapsed().as_secs_f32()
                    );
                }
                return;
            };
            let cached = match &asset {
                Asset::Texture(path) => renderer.texture_cache.get(path).is_some(),
                Asset::Sprite(path) => self.effect_sprites.get(path).is_some(),
                Asset::Str(names) => self.str_effects.is_resolved(names[0]),
            };
            if cached {
                loader.loaded += 1;
                continue;
            }
            loader.started.get_or_insert_with(Instant::now);
            let retry = asset.clone();
            match loader.worker.as_mut().unwrap().submit((asset, filtering)) {
                Ok(()) => loader.inflight = Some(retry),
                Err(TrySendError::Full((asset, _))) => loader.queue.push_front(asset),
                Err(TrySendError::Disconnected((asset, _))) => {
                    loader.queue.push_front(asset);
                    loader.worker_failed = true;
                    tracing::error!(
                        "Effect preload worker closed; synchronous fallback on the next frame"
                    );
                }
            }
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_strs_are_warmed_and_all_aliases_remain_available_on_demand() {
        let loader = EffectLoader::new();
        let strs = loader
            .queue
            .iter()
            .filter(|a| matches!(a, Asset::Str(_)))
            .count();
        let common = COMMON.into_iter().filter_map(nostalre_game::effect::str_names_of).count();
        assert_eq!(strs, common);
        assert!(strs < loader.str_names.len());
        for names in nostalre_game::effect::effect_str_names() {
            assert_eq!(loader.other_names(names[0]), &names[1..]);
        }
        assert_eq!(
            loader.other_names("newfirewall"),
            &["firewall1", "firewall2"]
        );
        assert!(loader.other_names("no such effect").is_empty());
    }

    #[test]
    fn a_skill_goes_to_the_front() {
        let mut loader = EffectLoader::new();
        loader.put_skills_first(std::iter::once(SkillEnum::AlAngelus));
        assert!(matches!(loader.queue.front(), Some(Asset::Str(_))));
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires actual kRO GRF and a Vulkan adapter; no Switch FPS claim"]
    fn gpu_worker_prepares_textures_sprites_and_str_in_both_filter_modes() {
        use nostalre_formats::grf::GrfArchive;
        use nostalre_renderer::{EffectSpriteCache, StrEffectCache, TextureCache, block_on, wgpu};
        use std::sync::Arc;
        let path = std::env::var("NOSTALRE_GPU_GRF").expect("read-only test archive");
        let grf = Arc::new(GrfArchive::open_layered(&[path], None).unwrap());
        let texture = nostalre_game::effect::effect_texture_paths()
            .into_iter()
            .find(|path| grf.file_exists(path))
            .expect("effect texture fixture");
        let sprite = nostalre_game::effect::effect_spr_paths()
            .into_iter()
            .chain(nostalre_game::effect::custom_effect_sprite_paths())
            .find(|path| {
                grf.file_exists(&format!("{path}.spr")) && grf.file_exists(&format!("{path}.act"))
            })
            .expect("SPR/ACT effect fixture");
        let str_names = nostalre_game::effect::effect_str_names()
            .into_iter()
            .find(|names| grf.file_exists(&nostalre_resources::texture::effect::str_file(names[0])))
            .expect("STR effect fixture");
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter =
            block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).unwrap();
        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let mut textures = TextureCache::new(&device);
        let worker_device = device.clone();
        let worker_queue = queue.clone();
        let worker_layout = textures.bind_group_layout.clone();
        let mut worker = BackgroundWorker::new("gpu-effect-coverage", move |(asset, filtering)| {
            let scope = worker_device.push_error_scope(wgpu::ErrorFilter::Validation);
            let prepared = prepare_asset(
                asset,
                filtering,
                &grf,
                &worker_device,
                &worker_queue,
                &worker_layout,
            );
            worker_queue.submit([]);
            worker_device.poll(wgpu::PollType::Poll).unwrap();
            assert!(
                block_on(scope.pop()).is_none(),
                "worker created invalid GPU resources"
            );
            prepared
        })
        .unwrap();
        for filtering in [false, true] {
            textures.remove(&texture);
            let mut sprites = EffectSpriteCache::new();
            sprites.set_filtering(filtering);
            let mut strs = StrEffectCache::new();
            strs.set_filtering(filtering);
            let mut installed = [false; 3];
            for asset in [
                Asset::Texture(texture.clone()),
                Asset::Sprite(sprite),
                Asset::Str(str_names),
            ] {
                assert!(worker.submit((asset, filtering)).is_ok());
                let deadline = Instant::now() + Duration::from_secs(20);
                let completed = loop {
                    if let Some(ready) = worker.poll().unwrap() {
                        break ready;
                    }
                    assert!(Instant::now() < deadline, "GPU asset worker timed out");
                    std::thread::sleep(Duration::from_millis(1));
                };
                assert_eq!(completed.filtering, filtering);
                match (completed.asset, completed.prepared) {
                    (Asset::Texture(path), Prepared::Texture(Some((group, width, height)))) => {
                        assert!(width > 0 && height > 0);
                        textures.insert(&path, group, width, height);
                        installed[0] = textures.get(&path).is_some();
                    }
                    (Asset::Sprite(path), Prepared::Sprite(Some(entry))) => {
                        assert!(!entry.act.actions.is_empty());
                        installed[1] = sprites.install_prepared(path, Some(entry))
                            && sprites.get(path).is_some();
                    }
                    (Asset::Str(names), Prepared::Str(Some(entry))) => {
                        assert!(entry.bind_groups.iter().flatten().any(Option::is_some));
                        installed[2] = strs.install_prepared(names[0], Some(entry))
                            && strs.is_resolved(names[0]);
                    }
                    _ => panic!("real effect fixture did not produce a GPU asset"),
                }
            }
            assert_eq!(installed, [true; 3]);
        }
        println!(
            "PASS: actual GRF texture, SPR/ACT and STR prepared on worker with nearest/linear filtering and Vulkan validation"
        );
    }
}
