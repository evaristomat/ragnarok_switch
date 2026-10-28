//! rAthena's packet table at the packet versions we speak, dumped from the server's own headers
//! (`scripts/packetdb-dump.sh` writes `data/rathena-<packetver>.tsv`). It is the truth for framing: every id the
//! server can send has a length here, whether or not the packet crate parses it. `data/rathena-<packetver>-layout.tsv`
//! holds the fields of the struct behind each id, for checking the crate's layouts (`packet_check`).

use std::collections::HashMap;
use std::sync::LazyLock;

/// Wire length of a packet: fixed, or read from the two bytes after the id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireLen {
    Fixed(usize),
    Variable,
}

impl WireLen {
    fn from_table(value: i32) -> Self {
        if value < 0 {
            WireLen::Variable
        } else {
            WireLen::Fixed(value as usize)
        }
    }
}

/// A packet the server parses: its length and where its fields start.
#[derive(Debug, Clone)]
pub struct Parsed {
    pub len: WireLen,
    /// rAthena's handler (`clif_parse_WalkToXY`) or, for login and char server requests, the packet's name.
    pub handler: String,
    pub offsets: Vec<usize>,
    /// The id this version's client sends for the handler; the others are older ids the server still accepts.
    pub current: bool,
}

/// A packet the server sends.
#[derive(Debug, Clone)]
pub struct Sent {
    pub len: WireLen,
    pub name: String,
    /// Some server code sends it (the table also lists ids nothing sends any more).
    pub used: bool,
    /// A variable packet that always has this size: it declares its length, the server sends `sizeof`.
    pub one_size: Option<usize>,
}

/// One field of the server's struct for a packet, flattened like the crate's `layout()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerField {
    pub name: String,
    pub offset: usize,
    /// 0 for an open array (the rest of a variable packet).
    pub size: usize,
    pub array: bool,
}

pub struct PacketTable {
    pub packetver: u32,
    sent: HashMap<u16, Sent>,
    parsed: HashMap<u16, Parsed>,
    /// (id, sent by the server) -> the struct's fields
    layouts: HashMap<(u16, bool), Vec<ServerField>>,
}

impl PacketTable {
    fn parse(packetver: u32, tsv: &str, layout_tsv: &str) -> Self {
        let mut sent = HashMap::new();
        let mut parsed = HashMap::new();
        for line in tsv.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
            let cols: Vec<&str> = line.split('\t').collect();
            let id = u16::from_str_radix(cols[0].trim_start_matches("0x"), 16).expect("packet id");
            let len = WireLen::from_table(cols[1].parse().expect("packet length"));
            match cols[2] {
                "zc" => {
                    let sent_packet = Sent {
                        len,
                        name: cols[3].to_string(),
                        used: !cols[4].ends_with(" unused"),
                        one_size: cols.get(5).and_then(|v| v.parse().ok()),
                    };
                    sent.insert(id, sent_packet);
                }
                "cz" => {
                    let offsets = cols
                        .get(5)
                        .map(|o| o.split(',').filter_map(|v| v.trim().parse().ok()).collect())
                        .unwrap_or_default();
                    let current = !cols[4].ends_with(" older");
                    parsed.insert(
                        id,
                        Parsed {
                            len,
                            handler: cols[3].to_string(),
                            offsets,
                            current,
                        },
                    );
                }
                other => panic!("unknown direction {other} in packet table"),
            }
        }
        let mut layouts: HashMap<(u16, bool), Vec<ServerField>> = HashMap::new();
        for line in layout_tsv
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
        {
            let cols: Vec<&str> = line.split('\t').collect();
            let id = u16::from_str_radix(cols[0].trim_start_matches("0x"), 16).expect("packet id");
            let field = ServerField {
                name: cols[3].to_string(),
                offset: cols[4].parse().expect("field offset"),
                size: cols[5].parse().expect("field size"),
                array: cols[6] == "1",
            };
            layouts
                .entry((id, cols[1] == "zc"))
                .or_default()
                .push(field);
        }
        Self {
            packetver,
            sent,
            parsed,
            layouts,
        }
    }

    /// The fields of the struct the server sends (`sent`) or parses this id with, when it has one.
    pub fn layout(&self, id: u16, sent: bool) -> Option<&[ServerField]> {
        self.layouts.get(&(id, sent)).map(Vec::as_slice)
    }

    /// The requests the server parses: id and how.
    pub fn parsed(&self) -> impl Iterator<Item = (u16, &Parsed)> {
        self.parsed.iter().map(|(id, p)| (*id, p))
    }

    /// Length of a packet the server sends with this id.
    pub fn sent_len(&self, id: u16) -> Option<WireLen> {
        self.sent.get(&id).map(|p| p.len)
    }

    /// For a variable packet the server always sends at one size (`sizeof`), that size.
    pub fn sent_one_size(&self, id: u16) -> Option<usize> {
        self.sent.get(&id).and_then(|p| p.one_size)
    }

    /// The field offsets the server reads a request at.
    pub fn parsed_offsets(&self, id: u16) -> &[usize] {
        self.parsed.get(&id).map_or(&[], |p| p.offsets.as_slice())
    }

    /// The packets some server code sends.
    pub fn sent(&self) -> impl Iterator<Item = (u16, &Sent)> {
        self.sent
            .iter()
            .filter(|(_, p)| p.used)
            .map(|(id, p)| (*id, p))
    }

    /// The request this version's client sends for a handler (or login/char packet name).
    pub fn parsed_by_handler(&self, handler: &str) -> Option<(u16, &Parsed)> {
        self.parsed
            .iter()
            .find(|(_, p)| p.current && p.handler == handler)
            .map(|(id, p)| (*id, p))
    }
}

/// The one packet version the client speaks: the pinned kRO client's (docs/nostalre-plan.md, milestone 8), against
/// rAthena built at the same `PACKETVER`
pub const PACKETVER: u32 = 20250716;

/// `scripts/packetdb-dump.sh`'s table and layouts at [`PACKETVER`]
const TABLE_TSV: &str = include_str!("../data/rathena-20250716.tsv");
const LAYOUT_TSV: &str = include_str!("../data/rathena-20250716-layout.tsv");

static TABLE: LazyLock<PacketTable> =
    LazyLock::new(|| PacketTable::parse(PACKETVER, TABLE_TSV, LAYOUT_TSV));

/// rAthena's packet table at [`PACKETVER`]
pub fn table() -> &'static PacketTable {
    &TABLE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet_check::{check, used_by_client};

    /// Every packet the client handles or builds (handler.rs, sender.rs) has the server's length and lies on the
    /// wire as the server's struct, or for a request, has a field at every offset the server reads. Packets the
    /// client does not use yet are in the status report (scripts/packet-status.sh). `NOT_YET` lists the ones the
    /// client uses that still differ: the work list of the protocol move (docs/nostalre-protocol-plan.md).
    #[test]
    fn the_packets_the_client_uses_match_the_server() {
        const NOT_YET: &[u16] = &[];
        let mut wrong = Vec::new();
        let pv = PACKETVER;
        let t = table();
        let requests = t
            .parsed()
            .filter(|(_, p)| p.current)
            .map(|(id, _)| (id, false));
        for (id, by_server) in t.sent().map(|(id, _)| (id, true)).chain(requests) {
            let Some(c) = check(t, id, by_server) else {
                continue;
            };
            if NOT_YET.contains(&id) || !used_by_client(&c.crate_name) {
                continue;
            }
            if !c.len_ok {
                wrong.push(format!(
                    "{pv} 0x{id:04x} {}: length {:?}, server {:?}",
                    c.crate_name,
                    c.ours,
                    t.sent_len(id)
                ));
            }
            wrong.extend(
                c.differences
                    .iter()
                    .map(|d| format!("{pv} 0x{id:04x} {}: {d}", c.crate_name)),
            );
        }
        wrong.sort();
        wrong.dedup();
        assert!(
            wrong.is_empty(),
            "{} differences:\n{}",
            wrong.len(),
            wrong.join("\n")
        );
    }

    /// Golden bytes: every packet the client uses, in the shape the server sends it, parses and serializes back to
    /// the same bytes, so the crate's parser and builder agree on the layout
    /// checked above. Fields alternate 0 and 1 (bools must stay 0 or 1); an open list holds one entry.
    #[test]
    fn the_packets_the_client_uses_round_trip() {
        let pv = PACKETVER;
        let t = table();
        let mut wrong = Vec::new();
        let mut checked = 0;
        for (id, sent) in t.sent() {
            let Some(c) = check(t, id, true) else {
                continue;
            };
            if !used_by_client(&c.crate_name) || !c.matches() {
                continue;
            }
            let len = match (sent.len, sent.one_size) {
                (WireLen::Fixed(n), _) | (WireLen::Variable, Some(n)) => n,
                (WireLen::Variable, None) => {
                    let layout = t.layout(id, true).unwrap_or(&[]);
                    let open = layout.iter().filter(|f| f.name.contains("[]"));
                    let end = open.map(|f| f.offset + f.size).max();
                    match end {
                        Some(end) => end,
                        None => continue,
                    }
                }
            };
            let mut bytes: Vec<u8> = (0..len).map(|i| (i % 2) as u8).collect();
            bytes[..2].copy_from_slice(&id.to_le_bytes());
            if sent.len == WireLen::Variable {
                bytes[2..4].copy_from_slice(&(len as u16).to_le_bytes());
            }
            checked += 1;
            let round = std::panic::catch_unwind(|| {
                let mut packet = packets::packets_parser::parse(&bytes, pv);
                packet.fill_raw_with_packetver(Some(pv));
                packet.raw().clone()
            });
            match round {
                Ok(raw) if raw == bytes => {}
                Ok(raw) => {
                    let at = raw
                        .iter()
                        .zip(&bytes)
                        .position(|(a, b)| a != b)
                        .unwrap_or(raw.len().min(len));
                    wrong.push(format!(
                        "0x{id:04x} {}: {} bytes back of {len}, first difference at {at}",
                        c.crate_name,
                        raw.len()
                    ));
                }
                Err(_) => wrong.push(format!("0x{id:04x} {}: panicked", c.crate_name)),
            }
        }
        wrong.sort();
        assert!(
            wrong.is_empty(),
            "{} packets do not round-trip at {pv}:\n{}",
            wrong.len(),
            wrong.join("\n")
        );
        assert!(checked > 150, "only {checked} packets checked at {pv}");
    }

    /// The table and its layouts were dumped at [`PACKETVER`]: moving the version takes a new dump
    #[test]
    fn the_table_is_dumped_at_the_clients_version() {
        let dumped_at = format!("PACKETVER={PACKETVER} ");
        for tsv in [TABLE_TSV, LAYOUT_TSV] {
            assert!(tsv.lines().next().unwrap().contains(&dumped_at));
        }
    }

    #[test]
    fn the_table_knows_the_login_burst() {
        let t = table();
        assert_eq!(t.sent_len(0x0ac4), Some(WireLen::Variable)); // AC_ACCEPT_LOGIN
        assert_eq!(t.sent_len(0x0ac5), Some(WireLen::Fixed(156))); // HC_NOTIFY_ZONESVR
        assert_eq!(t.sent_len(0x01d7), Some(WireLen::Fixed(15))); // ZC_SPRITE_CHANGE, struct size wins
        let (id, walk) = t.parsed_by_handler("clif_parse_WalkToXY").unwrap();
        assert_eq!(
            (id, walk.len, walk.offsets.as_slice()),
            (0x035f, WireLen::Fixed(5), &[2][..])
        );
    }
}
