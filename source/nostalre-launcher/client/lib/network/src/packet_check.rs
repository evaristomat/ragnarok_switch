//! Checks of the packet crate against the server's own table (`rathena_table`): which struct the crate parses an id
//! with, and whether its fields lie where rAthena's do. Used by this crate's tests and by the status report
//! (`examples/packet_status.rs`, docs/nostalre-protocol-plan.md "Converting a packet").

use packets::packets::Packet;
use packets::packets_parser;

use crate::rathena_table::{PacketTable, ServerField, WireLen};
use crate::solo_packets::PacketZcHpInfo;

/// The struct the crate speaks `id` with at `packetver`: it parses the id, and that is the struct's id at this
/// version (a struct also parses the older ids it had). None otherwise. Parsed from zeros, so only the type and its
/// layout mean anything.
pub fn crate_packet(id: u16, packetver: u32) -> Option<Box<dyn Packet>> {
    const LEN: usize = 4096;
    let mut buffer = vec![0u8; LEN];
    buffer[..2].copy_from_slice(&id.to_le_bytes());
    buffer[2..4].copy_from_slice(&(LEN as u16).to_le_bytes());
    if id.to_le_bytes() == PacketZcHpInfo::ID {
        return Some(Box::new(PacketZcHpInfo::from(
            &buffer[..PacketZcHpInfo::LEN],
        )));
    }
    let packet = std::panic::catch_unwind(|| packets_parser::parse(&buffer, packetver)).ok()?;
    (packet.name() != "Unknown" && current_id(packet.as_ref(), packetver) == Some(id))
        .then_some(packet)
}

/// A packet's id at a version (`Packet::id` spells the wire bytes in hex: "0x3604" is 0x0436).
pub fn current_id(packet: &dyn Packet, packetver: u32) -> Option<u16> {
    let wire = u16::from_str_radix(packet.id(packetver).trim_start_matches("0x"), 16).ok()?;
    Some(wire.swap_bytes())
}

/// Whether the client handles or builds packets of this struct: handler.rs or sender.rs names it outside its tests.
pub fn used_by_client(struct_name: &str) -> bool {
    [include_str!("handler.rs"), include_str!("sender.rs")]
        .iter()
        .any(|source| names(&without_tests(source), struct_name))
}

/// The sender.rs functions that build a request of this struct (named outside the tests).
pub fn builders_of(struct_name: &str) -> Vec<String> {
    without_tests(include_str!("sender.rs"))
        .split("\npub fn ")
        .skip(1)
        .filter(|body| names(body, struct_name))
        .filter_map(|body| body.split('(').next().map(str::to_string))
        .collect()
}

/// Every `pub fn build_*` in sender.rs (outside the tests).
pub fn builder_names() -> Vec<String> {
    without_tests(include_str!("sender.rs"))
        .split("\npub fn ")
        .skip(1)
        .filter_map(|body| body.split('(').next())
        .filter(|name| name.starts_with("build_"))
        .map(str::to_string)
        .collect()
}

/// A source file without its `mod tests` (from `#[cfg(test)]` to the module's closing brace at column 0).
/// The source without its `mod tests`.
pub(crate) fn without_tests(source: &str) -> String {
    match source.split_once("\n#[cfg(test)]\nmod tests {") {
        Some((before, rest)) => {
            let after = rest.split_once("\n}\n").map_or("", |(_, after)| after);
            format!("{before}\n{after}")
        }
        None => source.to_string(),
    }
}

fn names(source: &str, word: &str) -> bool {
    let ident = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    source.match_indices(word).any(|(at, _)| {
        !ident(source[..at].chars().next_back()) && !ident(source[at + word.len()..].chars().next())
    })
}

/// Differences that read the same bytes: (id, our field, why it is right).
pub const ACCEPTED: &[(u16, &str, &str)] = &[
    (
        0x0235,
        "skill_list[].atype",
        "the server's inf and unknown (always 0), read as one little-endian int like the player's skill list",
    ),
    (
        0x0361,
        "head_dir",
        "the server reads its low byte (head direction 0 to 2); the high byte is the zero padding before dir",
    ),
    (0x0b39, "item_info[].option_data", OPTIONS),
    (0x0b41, "option_data", OPTIONS),
    (0x0b42, "option_data", OPTIONS),
    (0x0b44, "option_data", OPTIONS),
    (0x0b45, "option_data", OPTIONS),
    (0x0b3d, "item_list[].option_data", OPTIONS),
    (0x0b40, "item_list[].option_data", OPTIONS),
];

/// Offsets rAthena's table lists for a request its parse function never reads: (id, offset, why)
pub const ACCEPTED_PARSED_OFFSETS: &[(u16, usize, &str)] = &[(
    0x0a99,
    4,
    "the table keeps the old position's offset (`parseable_packet(0x0A99,4,...,2,4)`) for the 4-byte packet; \
     clif_parse_equipswitch_remove reads only the index at 2",
)];

const OPTIONS: &str =
    "the five item options (index, value, param), kept as raw bytes until the client shows them";

/// How the crate's struct for one of the server's packets compares with it.
pub struct Check {
    pub crate_name: String,
    /// The crate's length is the server's (requests: not checked here, see the sender tests).
    pub len_ok: bool,
    pub ours: WireLen,
    /// Field differences (`layout_differences`, and offsets the server reads a request at where we have no field),
    /// without the `ACCEPTED` ones.
    pub differences: Vec<String>,
}

impl Check {
    pub fn matches(&self) -> bool {
        self.len_ok && self.differences.is_empty()
    }
}

/// Compares the crate's struct for `id` with the server's: sent by the server (`by_server`) or a request it parses.
/// None when the crate does not speak the id.
pub fn check(table: &PacketTable, id: u16, by_server: bool) -> Option<Check> {
    let pv = table.packetver;
    let packet = crate_packet(id, pv)?;
    let ours = crate_len(id, pv).unwrap_or(WireLen::Fixed(packet.base_len(pv)));
    let len_ok = if by_server {
        let server = table.sent_len(id);
        let one_size = table.sent_one_size(id);
        server == Some(ours) || one_size.is_some_and(|n| ours == WireLen::Fixed(n))
    } else {
        true
    };
    let layout = packet.layout(pv);
    let mut differences: Vec<String> = table
        .layout(id, by_server)
        .map(|server| match is_read_widths(server) {
            true => width_differences(server, &layout),
            false => layout_differences(server, &layout),
        })
        .unwrap_or_default()
        .into_iter()
        .filter(|d| {
            !ACCEPTED
                .iter()
                .any(|(i, field, _)| *i == id && d.starts_with(&format!("`{field}`")))
        })
        .collect();
    if !by_server {
        let offsets = table.parsed_offsets(id);
        for offset in offsets.iter().filter(|o| {
            !layout.iter().any(|(_, start, _)| start == *o)
                && !ACCEPTED_PARSED_OFFSETS
                    .iter()
                    .any(|(i, accepted, _)| *i == id && accepted == *o)
        }) {
            differences.push(format!(
                "the server reads a field at {offset}, ours has none there"
            ));
        }
    }
    Some(Check {
        crate_name: packet.name().to_string(),
        len_ok,
        ours,
        differences,
    })
}

/// The crate's length for a packet the server sends, from its parser tables.
pub fn crate_len(id: u16, packetver: u32) -> Option<WireLen> {
    let bytes = id.to_le_bytes();
    if packets_parser::is_variable_length(bytes, packetver) {
        return Some(WireLen::Variable);
    }
    packets_parser::packet_len(bytes, packetver).map(WireLen::Fixed)
}

/// A request rAthena parses without a struct: the dump gives only the widths its parse function reads at its
/// offsets (`pos[n]` fields), not the whole packet.
fn is_read_widths(server: &[ServerField]) -> bool {
    server.iter().all(|f| f.name.starts_with("pos["))
}

/// Where the crate's fields miss what the server reads: a field of ours at each offset, as wide as the read.
fn width_differences(server: &[ServerField], ours: &[(String, usize, usize)]) -> Vec<String> {
    server
        .iter()
        .filter_map(
            |f| match ours.iter().find(|(_, offset, _)| *offset == f.offset) {
                Some((_, _, len)) if *len == f.size => None,
                Some((name, _, len)) => Some(format!(
                    "`{name}` at {} is {len} bytes, the server reads {}",
                    f.offset, f.size
                )),
                None => Some(format!(
                    "the server reads {} bytes at {}, ours has no field there",
                    f.size, f.offset
                )),
            },
        )
        .collect()
}

/// Where the crate's fields (`ours`, from `Packet::layout`) differ from the server's struct: every field of ours must
/// lie inside one of the server's and be that field, or a part of a server array; every server field must be
/// covered, up to an open field of ours. Empty when they agree.
pub fn layout_differences(server: &[ServerField], ours: &[(String, usize, usize)]) -> Vec<String> {
    // an open field of ours (a string, raw bytes) holds whatever the server puts from its offset on
    let open_from = ours
        .iter()
        .filter(|(_, _, len)| *len == 0)
        .map(|(_, offset, _)| *offset)
        .min()
        .unwrap_or(usize::MAX);
    let ours: Vec<&(String, usize, usize)> = ours.iter().filter(|(_, _, len)| *len > 0).collect();
    let mut differences = Vec::new();
    for (name, offset, len) in &ours {
        let end = offset + len;
        let home = server.iter().find(|f| {
            f.offset <= *offset
                && (f.size == 0 || end <= f.offset + f.size)
                && (f.size > 0 || f.array)
        });
        match home {
            None => {
                let crossed: Vec<&str> = server
                    .iter()
                    .filter(|f| f.offset < end && *offset < f.offset + f.size.max(1))
                    .map(|f| f.name.as_str())
                    .collect();
                differences.push(format!(
                    "`{name}` [{offset}..{end}) crosses the server's {}",
                    if crossed.is_empty() {
                        "end".to_string()
                    } else {
                        crossed.join(", ")
                    }
                ));
            }
            Some(f) if f.size > 0 && (f.offset, f.size) != (*offset, *len) && !f.array => {
                differences.push(format!(
                    "`{name}` [{offset}..{end}) is part of the server's `{}` [{}..{})",
                    f.name,
                    f.offset,
                    f.offset + f.size
                ));
            }
            Some(_) => {}
        }
    }
    for f in server.iter().filter(|f| f.size > 0 && f.offset < open_from) {
        let covered = ours
            .iter()
            .any(|(_, o, l)| *o < f.offset + f.size && f.offset < o + l);
        if !covered {
            differences.push(format!(
                "the server's `{}` [{}..{}) has no field of ours",
                f.name,
                f.offset,
                f.offset + f.size
            ));
        }
    }
    differences
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str, offset: usize, size: usize, array: bool) -> ServerField {
        ServerField {
            name: name.into(),
            offset,
            size,
            array,
        }
    }

    fn ours(fields: &[(&str, usize, usize)]) -> Vec<(String, usize, usize)> {
        fields
            .iter()
            .map(|(n, o, l)| (n.to_string(), *o, *l))
            .collect()
    }

    #[test]
    fn a_misplaced_field_is_named() {
        // the 2018 unit packets: rAthena puts the 4-byte shield before the 2-byte accessory
        let server = [
            field("weapon", 27, 4, false),
            field("shield", 31, 4, false),
            field("accessory", 35, 2, false),
        ];
        let wrong = ours(&[("weapon", 27, 4), ("accessory", 31, 2), ("shield", 33, 4)]);
        assert_eq!(
            layout_differences(&server, &wrong),
            [
                "`accessory` [31..33) is part of the server's `shield` [31..35)",
                "`shield` [33..37) crosses the server's shield, accessory"
            ]
        );
        let right = ours(&[("weapon", 27, 4), ("shield", 31, 4), ("accessory", 35, 2)]);
        assert!(layout_differences(&server, &right).is_empty());
    }

    // 0x025a: rAthena's list type, then 32-bit item ids; the crate read 16-bit ids from the list type on and its
    // layout ended there, so nothing past it was compared
    #[test]
    fn a_layout_ending_early_misses_the_server_fields_after_it() {
        let server = [
            field("makeItem", 4, 2, false),
            field("items[].itemId", 6, 4, false),
        ];
        assert_eq!(
            layout_differences(&server, &ours(&[("id_list[]", 4, 2)])),
            ["the server's `items[].itemId` [6..10) has no field of ours"]
        );
        let raw = ours(&[("make_item", 4, 2), ("items", 6, 0)]);
        assert!(
            layout_differences(&server, &raw).is_empty(),
            "an open tail holds anything"
        );
    }

    #[test]
    fn arrays_may_be_split_and_open_tails_hold_anything() {
        let server = [
            field("slot.card", 0, 16, true),
            field("Flag", 16, 1, false),
            field("message", 17, 0, true),
        ];
        let split = ours(&[
            ("card1", 0, 4),
            ("card2", 4, 4),
            ("card3", 8, 4),
            ("card4", 12, 4),
            ("flag", 16, 1),
            ("message", 17, 0),
        ]);
        assert!(layout_differences(&server, &split).is_empty());
        let missing = ours(&[("card1", 0, 4), ("flag", 16, 1)]);
        assert!(
            layout_differences(&server, &missing).is_empty(),
            "a split array need not be complete"
        );
        let gap = ours(&[("card1", 0, 16), ("other", 17, 1)]);
        assert_eq!(
            layout_differences(
                &[
                    field("a", 0, 16, true),
                    field("b", 16, 1, false),
                    field("c", 17, 1, false)
                ],
                &gap
            ),
            ["the server's `b` [16..17) has no field of ours"]
        );
    }
}
