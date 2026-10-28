//! Every effect id as nostalRE plays it, for setting beside kRO's runs (scripts/kro-exe/compare_effects.py).
//!
//! Run with: `cargo run -p nostalre-effects --example dump_effects > ../out/ours-effects.jsonl`
//!
//! One JSON line per effect id: its spec (`str`, `spr`, `spr_burst`, `custom`, `noop` or `none`) with its file and
//! duration (a custom one's `custom_duration_ms`), and for a custom effect (`make_effect`) what it does when stepped
//! as kRO's runs step theirs: 16 ms
//! updates from the origin, at most 600, each update's draws (`collect_draws`, in their Debug form: variant,
//! texture, blend, colour and sizes) and sounds, and the update it died at.

use std::panic::{AssertUnwindSafe, catch_unwind};

use models::enums::EnumWithNumberValue;
use models::enums::effect_id::EffectId;

use nostalre_effects::effect_trait::{CameraView, EffectRenderCtx, EffectUpdateCtx};
use nostalre_effects::factory::make_effect;
use nostalre_effects::spec::EffectAnchor;
use nostalre_effects::table::custom_duration_ms;
use nostalre_effects::{EffectDrawList, EffectSpec, EffectStatus, effect_spec};

const STEP: f32 = 0.016;
const CAP: usize = 600;

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn spec_fields(spec: Option<EffectSpec>) -> String {
    match spec {
        None => r#""spec": "none""#.to_string(),
        Some(EffectSpec::Noop) => r#""spec": "noop""#.to_string(),
        Some(EffectSpec::Custom) => r#""spec": "custom""#.to_string(), // its duration: custom_duration_ms
        Some(EffectSpec::Str {
            file,
            duration_ms,
            repeat,
        }) => format!(
            r#""spec": "str", "file": {}, "duration_ms": {duration_ms}, "repeat": {repeat}"#,
            quote(file)
        ),
        Some(EffectSpec::Spr {
            sprite,
            duration_ms,
            repeat,
            ..
        }) => format!(
            r#""spec": "spr", "file": {}, "duration_ms": {duration_ms}, "repeat": {repeat}"#,
            quote(sprite)
        ),
        Some(EffectSpec::SprBurst {
            sprite,
            duration_ms,
            ..
        }) => {
            format!(
                r#""spec": "spr_burst", "file": {}, "duration_ms": {duration_ms}"#,
                quote(sprite)
            )
        }
    }
}

/// A custom effect stepped from the origin.
struct Run {
    /// Each update that drew: `[update, [draw, ...]]`, as JSON.
    frames: String,
    sounds: Vec<&'static str>,
    /// The update it answered `Dead`, if it did within the cap.
    died: Option<usize>,
    overlay: Option<&'static str>,
}

fn run_custom(id: EffectId) -> Option<Run> {
    let mut effect = make_effect(id, EffectAnchor::Point([0.0, 0.0, 0.0]), None, None, None)?;
    let overlay = effect.str_overlay();
    let render = EffectRenderCtx {
        camera: CameraView {
            eye: [0.0, 300.0, -300.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
        },
        screen_w: 640.0,
        screen_h: 480.0,
        elapsed: 0.0,
    };
    let mut frames = Vec::new();
    let mut sounds = Vec::new();
    let mut died = None;
    for t in 0..CAP {
        let status = effect.update(&EffectUpdateCtx {
            delta: STEP,
            camera_target: None,
            caster_yaw: None,
        });
        if let Some(s) = effect.take_sfx_request() {
            sounds.push(s);
        }
        let mut list = EffectDrawList::new();
        effect.collect_draws(
            &mut list,
            &EffectRenderCtx {
                elapsed: STEP * (t + 1) as f32,
                ..render
            },
        );
        if !list.is_empty() {
            let draws: Vec<String> = list
                .primitives
                .iter()
                .chain(list.behind.iter())
                .map(|d| quote(&format!("{d:?}")))
                .collect();
            frames.push(format!(r#"[{t}, [{}]]"#, draws.join(", ")));
        }
        if status == EffectStatus::Dead {
            died = Some(t);
            break;
        }
    }
    Some(Run {
        frames: frames.join(", "),
        sounds,
        died,
        overlay,
    })
}

fn main() {
    for id in EffectId::ALL {
        let spec = effect_spec(id);
        let custom = matches!(spec, Some(EffectSpec::Custom));
        let mut line = format!(
            r#"{{"id": {}, "name": {}, {}"#,
            id.value(),
            quote(&format!("{id:?}")),
            spec_fields(spec)
        );
        if custom {
            match catch_unwind(AssertUnwindSafe(|| run_custom(id))) {
                Ok(Some(Run {
                    frames,
                    sounds,
                    died,
                    overlay,
                })) => {
                    let sounds: Vec<String> = sounds.iter().map(|s| quote(s)).collect();
                    // the holder (renderer effect/holder.rs) also drops it at its duration (u32::MAX: never)
                    line.push_str(&format!(
                        r#", "made": true, "duration_ms": {}, "died": {}, "overlay": {}, "sounds": [{}], "frames": [{frames}]"#,
                        custom_duration_ms(id),
                        died.map_or("null".to_string(), |d| d.to_string()),
                        overlay.map_or("null".to_string(), quote),
                        sounds.join(", ")
                    ));
                }
                Ok(None) => line.push_str(r#", "made": false"#),
                Err(_) => line.push_str(r#", "made": false, "panicked": true"#),
            }
        }
        line.push('}');
        println!("{line}");
    }
}
