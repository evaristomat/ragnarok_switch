//! Status of every packet at a packet version, as Markdown: what the server sends and parses (rAthena's table,
//! lib/network/data/), which struct the packet crate speaks it with, whether length and field layout match the
//! server's (`packet_check::check`), and what the client maps it to: the events `dispatch_packet` makes of a server
//! packet (copies filled with 0x00, 0x01, 0x02 and 0xff), the `sender.rs` builders of a request. The work list of the protocol
//! move (docs/nostalre-protocol-plan.md, "Converting a packet"); scripts/packet-status.sh writes it to
//! nostalre-client/docs/packet-status-<packetver>.md.
//!
//! `cargo run -p nostalre-network --example packet_status [-- PACKETVER]` prints it; the tests below keep the
//! committed report current and its flags explained.

use crate::handler::dispatch_packet;
use crate::packet_check::{Check, builders_of, check, crate_packet, current_id, used_by_client};
use crate::rathena_table::{self, PACKETVER, WireLen};
use crate::solo_packets::PacketZcHpInfo;
use packets::packets::Packet;
use packets::packets_parser;
use std::fmt::Write;

struct Row {
    id: u16,
    by_server: bool,
    server_name: String,
    len: WireLen,
    check: Option<Check>,
}

impl Row {
    fn used(&self) -> bool {
        self.check
            .as_ref()
            .is_some_and(|c| used_by_client(&c.crate_name))
    }

    fn matches(&self) -> bool {
        self.check.as_ref().is_some_and(Check::matches)
    }
}

/// A copy of the crate's packet for `id` with every byte after the id (and a variable packet's length) `fill`
fn filled_packet(id: u16, pv: u32, fill: u8) -> Option<Box<dyn Packet>> {
    const LEN: usize = 4096;
    let mut buffer = vec![fill; LEN];
    buffer[..2].copy_from_slice(&id.to_le_bytes());
    if id.to_le_bytes() == PacketZcHpInfo::ID {
        // not in the generated parser: the connection frames it by hand (connection.rs)
        return Some(Box::new(PacketZcHpInfo::from(
            &buffer[..PacketZcHpInfo::LEN],
        )));
    }
    if packets_parser::is_variable_length(id.to_le_bytes(), pv) {
        buffer[2..4].copy_from_slice(&(LEN as u16).to_le_bytes());
    }
    let packet = std::panic::catch_unwind(|| packets_parser::parse(&buffer, pv)).ok()?;
    (current_id(packet.as_ref(), pv) == Some(id)).then_some(packet)
}

/// The events the handler makes of the packet, in copies filled with 0x00, 0x01, 0x02 and 0xff (handlers that answer
/// some values only, a result or a type, show up with one of them; lists come out full of entries; 0xff is past the
/// end of most enums, so a conversion that panics on an unknown value shows), and whether a copy panicked.
pub fn handled_events(id: u16, pv: u32) -> (Vec<String>, bool) {
    let mut names: Vec<String> = Vec::new();
    let mut panicked = false;
    for packet in [0, 1, 2, 0xff]
        .into_iter()
        .map(|fill| filled_packet(id, pv, fill))
        .into_iter()
        .flatten()
    {
        let events = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatch_packet(packet.as_ref())
        }));
        let Ok(events) = events else {
            panicked = true;
            continue;
        };
        for e in events {
            let name: String = format!("{e:?}")
                .chars()
                .take_while(|c| c.is_alphanumeric())
                .collect();
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    if names.len() > 1 {
        names.retain(|n| n != "Acknowledged");
    }
    (names, panicked)
}

/// `handled_events` for the report: `ignored` when the handler only acknowledges the packet, nothing when neither
/// copy makes an event; **panics** when either copy panicked (the client drops such a packet whole).
fn handled_as(id: u16, pv: u32) -> String {
    let (names, panicked) = handled_events(id, pv);
    let mapped = match names.as_slice() {
        [] if panicked => return PANICS.into(),
        [] => NO_EVENT.into(),
        [only] if only == "Acknowledged" => "ignored".into(),
        _ => names.join(", "),
    };
    match panicked {
        true => format!("{mapped}, {PANICS}"),
        false => mapped,
    }
}

/// The `sender.rs` functions that build a request
fn built_by(struct_name: &str) -> String {
    match builders_of(struct_name).as_slice() {
        [] => NO_BUILDER.into(),
        builders => builders
            .iter()
            .map(|b| format!("`{b}`"))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn len_text(len: WireLen) -> String {
    match len {
        WireLen::Fixed(n) => n.to_string(),
        WireLen::Variable => "var".into(),
    }
}

/// Every packet the server sends or parses, with what the client maps a used one to (empty when unused).
fn rows() -> (Vec<Row>, Vec<String>) {
    let pv = PACKETVER;
    let t = rathena_table::table();
    let sent = t.sent().map(|(id, p)| (id, true, p.name.clone(), p.len));
    let parsed = t
        .parsed()
        .filter(|(_, p)| p.current)
        .map(|(id, p)| (id, false, p.handler.clone(), p.len));
    let mut rows: Vec<Row> = sent
        .chain(parsed)
        .map(|(id, by_server, server_name, len)| Row {
            id,
            by_server,
            server_name,
            len,
            check: check(t, id, by_server),
        })
        .collect();
    rows.sort_by_key(|r| (!r.by_server, r.id));
    let maps_to: Vec<String> = rows
        .iter()
        .map(|r| match &r.check {
            // what a builder sends shows its builders (rAthena's table lists 0x0065 as sent), the rest its events
            Some(c) if r.used() && (!r.by_server || !builders_of(&c.crate_name).is_empty()) => {
                built_by(&c.crate_name)
            }
            Some(_) if r.used() => handled_as(r.id, pv),
            _ => String::new(),
        })
        .collect();

    (rows, maps_to)
}

/// The used packets the client parses with no event or has no builder for: (id, flag).
pub fn flagged() -> Vec<(u16, String)> {
    let (rows, maps_to) = rows();
    rows.iter()
        .zip(maps_to)
        .filter(|(_, m)| m == NO_EVENT || m == NO_BUILDER || m.contains(PANICS))
        .map(|(r, m)| (r.id, m))
        .collect()
}

const NO_EVENT: &str = "**no event**";
const PANICS: &str = "**panics**";
const NO_BUILDER: &str = "**no builder**";

/// A handler for a packet the server does not send at `pv` (an older id or packet), whose events no packet it sends
/// makes: the feature waits for the version's packet. (id, crate struct, its events, the server's packets of the
/// same name at `pv`)
pub struct Orphan {
    pub id: u16,
    pub crate_name: String,
    pub events: Vec<String>,
    pub replaced_by: Vec<(u16, String)>,
}

/// rAthena's name for a crate struct, without a version suffix: `PacketZcAddMemberToGroup2` -> `ZC_ADD_MEMBER_TO_GROUP`
fn base_name(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.trim_start_matches("Packet").chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_uppercase());
    }
    let trimmed = out.trim_end_matches(|c: char| c.is_ascii_digit());
    trimmed.strip_suffix("_V").unwrap_or(trimmed).to_string()
}

/// Every orphaned handler
pub fn orphans() -> Vec<Orphan> {
    let pv = PACKETVER;
    let t = rathena_table::table();
    let sent: Vec<(u16, String)> = t.sent().map(|(id, p)| (id, p.name.clone())).collect();
    let live: Vec<String> = sent
        .iter()
        .flat_map(|(id, _)| handled_events(*id, pv).0)
        .collect();
    (0u16..0x0d00)
        .filter(|id| !sent.iter().any(|(s, _)| s == id))
        .filter_map(|id| crate_packet(id, pv).map(|p| (id, p.name().to_string())))
        .filter(|(_, name)| used_by_client(name) && builders_of(name).is_empty())
        .filter_map(|(id, crate_name)| {
            let events: Vec<String> = handled_events(id, pv)
                .0
                .into_iter()
                .filter(|e| e != "Acknowledged" && !live.contains(e))
                .collect();
            let base = base_name(&crate_name);
            let replaced_by = sent
                .iter()
                .filter(|(_, n)| n.trim_end_matches(|c: char| c.is_ascii_digit()) == base)
                .cloned()
                .collect();
            (!events.is_empty()).then_some(Orphan {
                id,
                crate_name,
                events,
                replaced_by,
            })
        })
        .collect()
}

/// The status report as Markdown (docs/packet-status-<pv>.md).
pub fn report() -> String {
    let pv = PACKETVER;
    let (rows, maps_to) = rows();
    let mut out = String::new();
    let count = |f: &dyn Fn(&Row) -> bool| rows.iter().filter(|r| f(r)).count();
    writeln!(out, "# Packet status at {pv}\n").unwrap();
    writeln!(
        out,
        "Generated by `scripts/packet-status.sh` from rAthena's table (`lib/network/data/rathena-{pv}*.tsv`) and the \
         packet crate; do not edit. How to convert a packet: docs/nostalre-protocol-plan.md, \"Converting a packet\".\n"
    )
    .unwrap();
    writeln!(out, "| | Packets |\n|---|---|").unwrap();
    writeln!(out, "| The server sends or parses | {} |", rows.len()).unwrap();
    writeln!(
        out,
        "| The crate speaks them | {} |",
        count(&|r| r.check.is_some())
    )
    .unwrap();
    writeln!(
        out,
        "| ... with the server's length and layout | {} |",
        count(&Row::matches)
    )
    .unwrap();
    writeln!(out, "| The client uses them | {} |", count(&Row::used)).unwrap();
    writeln!(
        out,
        "| ... but they still differ | {} |",
        count(&|r| r.used() && !r.matches())
    )
    .unwrap();
    writeln!(
        out,
        "| The crate does not speak them (framed and skipped) | {} |",
        count(&|r| r.check.is_none())
    )
    .unwrap();
    let with = |text: &str| maps_to.iter().filter(|m| m.as_str() == text).count();
    writeln!(
        out,
        "| Used, but parsed with no event / no builder | {} / {} |\n",
        with(NO_EVENT),
        with(NO_BUILDER)
    )
    .unwrap();
    writeln!(
        out,
        "\"Client\" is what the client makes of a packet it uses: for a server packet, the events the handler makes of \
         copies filled with 0x00, 0x01, 0x02 and 0xff (`ignored`: acknowledged only; a handler can make more for \
         other values, a list's storage type or a result code; **panics**: a copy panicked); for a request, the `sender.rs` functions that build it.\n"
    )
    .unwrap();
    for (title, by_server) in [
        ("Sent by the server", true),
        ("Parsed by the server (requests)", false),
    ] {
        writeln!(out, "## {title}\n").unwrap();
        writeln!(
            out,
            "| Id | rAthena | Length | Crate struct | Length | Layout | Client |"
        )
        .unwrap();
        writeln!(out, "|---|---|---|---|---|---|---|").unwrap();
        for (r, maps_to) in rows
            .iter()
            .zip(&maps_to)
            .filter(|(r, _)| r.by_server == by_server)
        {
            let (name, length, layout) = match &r.check {
                None => ("-", "", String::new()),
                Some(c) => (
                    c.crate_name.as_str(),
                    if c.len_ok { "ok" } else { "**differs**" },
                    match c.differences.as_slice() {
                        [] => "ok".into(),
                        [first, rest @ ..] => {
                            let more = if rest.is_empty() {
                                String::new()
                            } else {
                                format!(" (+{})", rest.len())
                            };
                            format!("**{}**{more}", first.replace('|', "\\|"))
                        }
                    },
                ),
            };
            writeln!(
                out,
                "| 0x{:04x} | {} | {} | {name} | {length} | {layout} | {maps_to} |",
                r.id,
                r.server_name,
                len_text(r.len),
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }
    writeln!(out, "## Handled, but not sent at this version\n").unwrap();
    writeln!(
        out,
        "Handlers for a packet the server does not send at {pv} whose events none of its packets make: the feature \
         waits for the version's packet (`ORPHANED` in this module's tests says which step).\n"
    )
    .unwrap();
    writeln!(
        out,
        "| Id | Crate struct | Events | The server sends instead |\n|---|---|---|---|"
    )
    .unwrap();
    for o in orphans() {
        let instead: Vec<String> = o
            .replaced_by
            .iter()
            .map(|(id, n)| format!("0x{id:04x} {n}"))
            .collect();
        writeln!(
            out,
            "| 0x{:04x} | {} | {} | {} |",
            o.id,
            o.crate_name,
            o.events.join(", "),
            instead.join(", ")
        )
        .unwrap();
    }
    writeln!(out).unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Used packets that are flagged on purpose: (id, why).
    const EXPLAINED: &[(u16, &str)] = &[
        (
            0x023c,
            "storage password result: the handler answers the result codes 4 to 8 only, neither filled copy",
        ),
        (
            0x081e,
            "elemental gauge: the handler answers rAthena's four gauges (5 to 8, SP_HP to SP_MAXSP) only, no filled copy",
        ),
        (
            0x0b6c,
            "summon gauge: the handler answers HP and max HP (5 and 6, SP_HP and SP_MAXHP) only, no filled copy",
        ),
    ];

    /// Handlers waiting for the version's packet (`orphans`): (id, step and the packet that replaces it). The
    /// work list of the features whose packets moved; it only shrinks, and a new orphan needs a line.
    const ORPHANED: &[(u16, &str)] = &[
        (
            0x01e2,
            "marriage: rAthena sends no proposal, weddings run through NPC scripts",
        ),
        (0x01e4, "marriage: as 0x01e2"),
        (0x0240, "the old mail: RODEX replaced it, RODEX is left out"),
        (0x0242, "the old mail: RODEX replaced it, RODEX is left out"),
        (0x0245, "the old mail: RODEX replaced it, RODEX is left out"),
        (0x0249, "the old mail: RODEX replaced it, RODEX is left out"),
        (0x024a, "the old mail: RODEX replaced it, RODEX is left out"),
        (0x0255, "the old mail: RODEX replaced it, RODEX is left out"),
        (0x0257, "the old mail: RODEX replaced it, RODEX is left out"),
    ];

    /// Every orphaned handler is on `ORPHANED`, and every entry there is still one.
    #[test]
    fn every_orphaned_handler_is_on_the_work_list() {
        let found: Vec<u16> = orphans().iter().map(|o| o.id).collect();
        let mut wrong: Vec<String> = found
            .iter()
            .filter(|id| !ORPHANED.iter().any(|(o, _)| o == *id))
            .map(|id| format!("0x{id:04x}: handled, not sent, and on no step: add it to ORPHANED"))
            .collect();
        wrong.extend(
            ORPHANED
                .iter()
                .filter(|(id, _)| !found.contains(id))
                .map(|(id, _)| format!("0x{id:04x}: no longer orphaned, take it off ORPHANED")),
        );
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    /// Every server packet the handler names is one the server sends or waits on `ORPHANED`. An arm for a packet
    /// that never comes is dead code that `orphans` cannot see when a live packet makes the same
    /// events (the 20120307 unit entries did, 80 arms in all).
    #[test]
    fn every_handled_packet_is_one_the_server_sends() {
        let live: std::collections::HashSet<String> = rathena_table::table()
            .sent()
            .map(|(id, _)| id)
            .chain(ORPHANED.iter().map(|(id, _)| *id))
            .filter_map(|id| crate_packet(id, PACKETVER))
            .map(|p| p.name().to_string())
            .collect();
        let crate_source = include_str!("../../../rust-ro/packets/src/packets.rs");
        let is_packet = |name: &str| crate_source.contains(&format!("pub struct {name} {{"));
        let code = crate::packet_check::without_tests(include_str!("handler.rs"));
        let named: std::collections::BTreeSet<&str> = code
            .match_indices("Packet")
            .filter(|(at, _)| !code[..*at].ends_with(|c: char| c.is_alphanumeric() || c == '_'))
            .map(|(at, _)| {
                let rest = &code[at..];
                &rest[..rest
                    .find(|c: char| !c.is_alphanumeric() && c != '_')
                    .unwrap_or(rest.len())]
            })
            .filter(|name| is_packet(name))
            .collect();
        assert!(
            named.len() > 250,
            "the scan found {} packets in handler.rs",
            named.len()
        );
        let mut dead: Vec<&str> = named
            .into_iter()
            .filter(|name| !live.contains(*name))
            .collect();
        dead.sort();
        dead.dedup();
        assert!(
            dead.is_empty(),
            "{} handled, not sent, not on ORPHANED:\n{}",
            dead.len(),
            dead.join("\n")
        );
    }

    /// The committed report is what the code makes now: a packet or handler change comes with its regenerated
    /// report (scripts/packet-status.sh), so the diff shows what moved.
    #[test]
    fn the_status_report_is_current() {
        assert!(
            // Git's Windows checkout may use CRLF. Compare the generated
            // content without mistaking that checkout convention for stale packets.
            report() == include_str!("../../../docs/packet-status-20250716.md").replace("\r\n", "\n"),
            "docs/packet-status-20250716.md is stale: run scripts/packet-status.sh and commit it"
        );
    }

    /// Every used packet maps to something: a server packet to an event (or `ignored`), a request to a builder, and
    /// no handler panics on a filled copy. A new flag is a packet the client drops or cannot send; fix it, or
    /// explain it in `EXPLAINED`.
    #[test]
    fn every_used_packet_maps_to_an_event_or_a_builder() {
        let unexplained: Vec<String> = flagged()
            .into_iter()
            .filter(|(id, _)| !EXPLAINED.iter().any(|(e, _)| e == id))
            .map(|(id, flag)| format!("0x{id:04x}: {flag}"))
            .collect();
        assert!(unexplained.is_empty(), "{}", unexplained.join("\n"));
    }
}
