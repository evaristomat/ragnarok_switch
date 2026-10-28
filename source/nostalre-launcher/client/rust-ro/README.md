# rust-ro crates

Five crates of [rust-ro](https://github.com/nmeylan/rust-ro), Nicolas Meylan's Ragnarok server in Rust, that the
client is built on: `models` (job, skill, item and status enums), `packets` (packet definitions and parser),
`movement`, and the proc-macro crates `enum_macro` and `accessor` they use.

Copied unchanged from commit `2e673a2446f1ae98d9edc49ba2d7bc9fe248c4d8` (`lib/<crate>`), which nostalro-client used
as a git dependency, so nostalRE can extend them: 3rd-job IDs, Renewal skills, newer packet versions. MIT licence,
in `LICENSE.md`. Changes since the copy are ours and listed in `../NOTICE`.

`packets-tool/` is rust-ro's packet generator (`tools/packets` at the same commit) with its `packets_db`: the
`packets` crate's four generated files come from it (`packets-tool/README.md`).
