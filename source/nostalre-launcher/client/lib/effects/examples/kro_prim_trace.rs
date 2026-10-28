//! A ported kRO effect's quads, each update, in the form the exe's effect runs record theirs
//! (`out/kro-effect-draws/*.jsonl`: one line a triangle, its world vertices, colour, uv, texture and blend), for
//! `scripts/kro-exe/check_prim_port.py` to set beside them.
//!
//! Run with: `cargo run -p nostalre-effects --example kro_prim_trace -- ID [UPDATES]`

use models::enums::EnumWithNumberValue;
use models::enums::effect_id::EffectId;
use nostalre_effects::kro_prim::{KroQuad, trace};

fn vertex(q: &KroQuad, i: usize) -> String {
    let [x, y, z] = q.corners[i];
    let [u, v] = q.uv[i];
    format!(r#"["world", {x}, {y}, {z}, "{:08x}", {u}, {v}]"#, q.argb)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let id: usize = args
        .first()
        .and_then(|a| a.parse().ok())
        .expect("an effect id");
    let updates: usize = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(600);
    let Some(frames) = trace(EffectId::from_value(id), updates) else {
        eprintln!("effect {id} is not ported");
        std::process::exit(1);
    };
    for (at, quads) in frames.iter().enumerate() {
        for q in quads {
            // kRO's two triangles, in its order
            for tri in q.tris {
                let verts: Vec<String> = tri.iter().map(|&i| vertex(q, i)).collect();
                println!(
                    r#"{{"at": {at}, "vertices": [{}], "texture": "{}", "blend": [{}, {}]}}"#,
                    verts.join(", "),
                    // kRO's own path, JSON-escaped: a nested texture keeps its folders under `effect\`
                    if q.texture.contains('/') {
                        q.texture.replace('/', "\\\\")
                    } else {
                        format!("effect\\\\{}", q.texture)
                    },
                    q.blend.0,
                    q.blend.1
                );
            }
        }
    }
}
