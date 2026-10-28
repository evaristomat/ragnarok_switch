from pathlib import Path
out = Path(__file__).resolve().parent.parent / "client/lib/game/src/job_tables.rs"
out.write_text('// RagSWITCH Pre-Renewal: no AP/traits; all playable classic jobs cap at Base 99.\npub fn has_ap(_job: u16) -> bool { false }\npub fn max_base_level(_job: u16) -> u16 { 99 }\n#[cfg(test)]\nmod tests {\n    use super::*;\n    #[test]\n    fn classic_level_and_ap_limits() {\n        for job in [0, 1, 7, 4008, 23, 24, 25, 4046, 4049] {\n            assert_eq!(max_base_level(job), 99);\n            assert!(!has_ap(job));\n        }\n    }\n}\n', encoding="utf-8")
