# packets-tool

rust-ro's packet generator (`tools/packets` in [rust-ro](https://github.com/nmeylan/rust-ro), MIT, copied from
commit `2e673a24` with only its paths changed). It reads `packets_db` and writes the four generated files of the
`packets` crate: `packets.rs`, `packets_impl.rs`, `packets_parser.rs`, `packets_print.rs`.

```sh
cargo run -p packets-tool   # from nostalre-client/; then commit packets_db and the regenerated files together
```

## packets_db

A packet is an id line and a C-like struct:

```
0x0437, 0x0085:20040705, 0x00a7:20041129, 0x035f:20101124
struct PACKET_CZ_REQUEST_MOVE {
  short PacketId
  #V RANGE 20070108 20090408 unsigned byte pad14[3]
  unsigned byte dest[3]
 }
```

- **Id ladder.** The first id has no version; `id:version` entries take over from that packet version on (the
  highest one at or below the client's version wins). Below the lowest dated id the generator still uses that
  lowest dated id, never the undated first one (it is only a label): date every id a version should send
  (`0x0bf5, 0x0bad:20210818, 0x0bf5:20230906`).
- **Gated fields.** `#V GTE v`, `#V GT v`, `#V LT v`, `#V LTE v` and `#V RANGE a b` (a ≤ version < b) put a field in
  the packet only at those versions. `fill_raw_with_packetver(Some(pv))` honours them; plain `fill_raw()` writes
  every gated field, so builders always pass the version.
- **Nested structs** (`struct NAME field[...] { ... }` with a `// Size n` comment) and `[...]` arrays of unknown
  count make variable-length packets; `short PacketLength` as the second field is the length.
- **Raw tails.** `unsigned byte name[...]` keeps a variable tail as bytes (`Vec<u8>`), for entries of varying size
  the handler reads itself (0x0b79's barter items). `char` and `unsigned char` make a `String`, which loses trailing
  zero bytes and fails the round-trip test.

Every generated packet has `layout(packetver)`, its fields as they lie on the wire. The server is the truth for
layouts: `scripts/packetdb-dump.sh` dumps rAthena's table and struct layouts at our packet version into
`lib/network/data/`, and `nostalre-network`'s tests check the crate against them
(`rathena_table::tests::the_packets_the_client_uses_match_the_server`). How to convert a packet:
`docs/nostalre-protocol-plan.md` in the repository root, "Converting a packet".
