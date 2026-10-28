use std::io;
use std::panic;

use packets::packets::Packet;
use packets::packets_parser;
use nostalre_profiling::debug::{self, PacketTrace};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

use crate::rathena_table::{self, WireLen};
use crate::solo_packets::{PacketZcHpInfo,PacketZcSkillScale};

const ZC_AID_PACKET_ID: u16 = 0x0283;

#[derive(Debug)]
pub enum ConnectionError {
    Io(io::Error),
    Disconnected,
}

impl From<io::Error> for ConnectionError {
    fn from(err: io::Error) -> Self {
        ConnectionError::Io(err)
    }
}

/// Periodic keepalive/time packets excluded from packet tracing regardless of mode.
fn is_muted_packet(name: &str) -> bool {
    matches!(
        name,
        "PacketCzRequestTime" | "PacketCzPing" | "PacketZcNotifyTime" | "PacketZcPingLive"
    )
}

/// Packets whose payload carries a password: traced by name and length only.
fn carries_secret(name: &str) -> bool {
    matches!(name, "PacketCaLogin" | "PacketCzAckStorePassword")
}

pub struct Connection {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
    recv_buffer: Vec<u8>,
    expect_aid_preamble: bool,
}

impl Connection {
    pub async fn connect(addr: &str, expect_aid_preamble: bool) -> io::Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        #[cfg(target_os = "horizon")]
        if let Err(error) = stream.set_nodelay(true) {
            tracing::warn!("TCP_NODELAY unavailable: {error}");
        }
        let (reader, writer) = stream.into_split();
        Ok(Self {
            reader,
            writer,
            recv_buffer: Vec::with_capacity(4096),
            expect_aid_preamble,
        })
    }

    pub async fn send_packet(&mut self, data: &[u8], packetver: u32) -> io::Result<()> {
        if debug::packet_trace() == PacketTrace::All {
            let name =
                panic::catch_unwind(|| packets_parser::parse(data, packetver).name().to_string())
                    .unwrap_or_else(|_| "<parse panic>".to_string());
            if !is_muted_packet(&name) {
                if carries_secret(&name) {
                    tracing::info!("send packet: {} ({} bytes)", name, data.len());
                } else {
                    let preview: Vec<String> =
                        data.iter().take(16).map(|b| format!("{b:02x}")).collect();
                    tracing::info!(
                        "send packet: {} ({} bytes) [{}]",
                        name,
                        data.len(),
                        preview.join(" ")
                    );
                }
            }
        }
        self.writer.write_all(data).await
    }

    fn estimate_packet_len(data: &[u8]) -> usize {
        if data.len() >= 4 {
            let len = u16::from_le_bytes([data[2], data[3]]) as usize;
            if len >= 4 && len <= data.len() {
                return len;
            }
        }
        4.min(data.len())
    }

    pub async fn recv_packets(
        &mut self,
        packetver: u32,
    ) -> Result<Vec<Box<dyn Packet>>, ConnectionError> {
        let mut buf = [0u8; 4096];
        let n = self.reader.read(&mut buf).await?;
        if n == 0 {
            return Err(ConnectionError::Disconnected);
        }
        self.recv_buffer.extend_from_slice(&buf[..n]);

        let trace = debug::packet_trace();

        if trace == PacketTrace::All {
            tracing::info!(
                "TCP read: {n} bytes, buffer_total={}, first_16={:02x?}",
                self.recv_buffer.len(),
                &buf[..n.min(16)]
            );
        }

        if self.expect_aid_preamble {
            if self.recv_buffer.len() < 4 {
                return Ok(Vec::new());
            }
            if let Some(aid) = Self::take_bare_aid_preamble(&mut self.recv_buffer)
                && trace == PacketTrace::All
            {
                tracing::info!("consumed account_id preamble: {aid}");
            }
            self.expect_aid_preamble = false;
        }

        let packets = Self::drain_packets(&mut self.recv_buffer, packetver, trace);
        Ok(packets)
    }

    /// Drops the headerless account id a server may greet with. Zone servers from
    /// packetver 20070521 greet with `ZC_AID` instead, which the parser handles.
    fn take_bare_aid_preamble(buffer: &mut Vec<u8>) -> Option<u32> {
        if u16::from_le_bytes([buffer[0], buffer[1]]) == ZC_AID_PACKET_ID {
            return None;
        }
        let aid = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
        buffer.drain(..4);
        Some(aid)
    }

    /// Total wire length of the packet at the head of `data`.
    fn frame_len(data: &[u8]) -> FrameLen {
        if data.len() < 2 {
            return FrameLen::Incomplete;
        }
        match rathena_table::table().sent_len(u16::from_le_bytes([data[0], data[1]])) {
            Some(WireLen::Fixed(len)) if len >= 2 => FrameLen::Known(len),
            Some(WireLen::Variable) => Self::declared_len(data),
            _ => FrameLen::Unusable,
        }
    }

    /// The length a variable-length packet declares in the two bytes after its id.
    fn declared_len(data: &[u8]) -> FrameLen {
        if data.len() < 4 {
            return FrameLen::Incomplete;
        }
        match u16::from_le_bytes([data[2], data[3]]) as usize {
            declared if declared >= 4 => FrameLen::Known(declared),
            _ => FrameLen::Unusable,
        }
    }

    fn drain_packets(
        buffer: &mut Vec<u8>,
        packetver: u32,
        trace: PacketTrace,
    ) -> Vec<Box<dyn Packet>> {
        let mut packets: Vec<Box<dyn Packet>> = Vec::new();
        let mut offset = 0;

        while offset < buffer.len() {
            let remaining = &buffer[offset..];
            let expected = match Self::frame_len(remaining) {
                FrameLen::Known(len) => len,
                FrameLen::Incomplete => {
                    if trace == PacketTrace::All {
                        tracing::info!("partial header ({} bytes), awaiting more", remaining.len());
                    }
                    break;
                }
                FrameLen::Unusable => {
                    let skip = Self::estimate_packet_len(remaining);
                    tracing::info!(
                        "skipping unknown packet 0x{:02x}{:02x} ({skip} bytes), buffer_remaining={}",
                        remaining.get(1).copied().unwrap_or(0),
                        remaining[0],
                        remaining.len()
                    );
                    if trace == PacketTrace::All {
                        tracing::debug!("unknown packet dump: {:02x?}", &remaining[..skip]);
                    }
                    offset += skip;
                    continue;
                }
            };
            if expected > remaining.len() {
                if trace == PacketTrace::All {
                    tracing::info!(
                        "packet 0x{:02x}{:02x} needs {expected} bytes, have {} — awaiting more",
                        remaining[1],
                        remaining[0],
                        remaining.len()
                    );
                }
                break;
            }

            let parse_buf = &remaining[..expected];
            if parse_buf[..2]==PacketZcSkillScale::ID {
                packets.push(Box::new(PacketZcSkillScale::from(parse_buf)));offset+=expected;continue;
            }
            if parse_buf[..2] == PacketZcHpInfo::ID {
                packets.push(Box::new(PacketZcHpInfo::from(parse_buf)));
                offset += expected;
                continue;
            }
            match panic::catch_unwind(|| packets_parser::parse(parse_buf, packetver)) {
                Ok(packet) => {
                    if packet.name() == "Unknown" {
                        tracing::info!(
                            "skipping unparsed packet 0x{:02x}{:02x} ({expected} bytes)",
                            parse_buf[1],
                            parse_buf[0]
                        );
                    } else {
                        if trace == PacketTrace::All && !is_muted_packet(packet.name()) {
                            tracing::info!(
                                "recv {} ({expected} bytes, remaining={}) {:02x?}",
                                packet.name(),
                                remaining.len(),
                                &parse_buf[..expected.min(64)]
                            );
                        }
                        packets.push(packet);
                    }
                }
                Err(_) => {
                    tracing::warn!(
                        "packet parse panic at offset {offset}, packet 0x{:02x}{:02x}, len {expected}",
                        parse_buf[1],
                        parse_buf[0]
                    );
                }
            }
            offset += expected;
        }

        buffer.drain(..offset);
        packets
    }
}

enum FrameLen {
    Known(usize),
    /// Too few bytes to read the length: wait for the next read.
    Incomplete,
    /// No length to work from — unknown id, or a declared length below the header.
    Unusable,
}

#[cfg(test)]
mod tests {
    use super::*;
    use packets::packets::{
        PacketChSendMapInfo, PacketZcAcceptEnter2, PacketZcAid, PacketZcNotifyPlayerchat,
        PacketZcNotifyTime, PacketZcReqWearEquipAckV5,
    };

    fn greeting_stream(packetver: u32, aid_bytes: &[u8]) -> Vec<u8> {
        let mut enter = PacketZcAcceptEnter2::new(packetver);
        enter.set_x_size(5);
        enter.set_y_size(5);
        enter.fill_raw_with_packetver(Some(packetver));
        let mut stream = aid_bytes.to_vec();
        stream.extend_from_slice(&enter.raw);
        stream
    }

    #[test]
    fn an_undefined_fixed_packet_is_skipped_without_losing_the_next() {
        let packetver = 20250716;
        // The roulette's ZC_ACK_OPEN_ROULETTE: no crate struct, framed by rAthena's table (23 bytes).
        let mut buffer = vec![0x1a, 0x0a];
        buffer.extend_from_slice(&[0; 21]);
        let mut time = PacketZcNotifyTime::new(packetver);
        time.fill_raw_with_packetver(Some(packetver));
        buffer.extend_from_slice(&time.raw);
        let framed = Connection::drain_packets(&mut buffer, packetver, PacketTrace::None);
        assert_eq!(framed.len(), 1);
        assert!(framed[0].as_any().is::<PacketZcNotifyTime>());
        assert!(buffer.is_empty());
    }

    #[test]
    fn zone_greeting_frames_whatever_form_the_aid_takes() {
        let packetver = 20250716;
        let mut aid = PacketZcAid::new(packetver);
        aid.set_aid(2000000);
        aid.fill_raw_with_packetver(Some(packetver));

        let mut buffer = greeting_stream(packetver, &aid.raw);
        assert!(Connection::take_bare_aid_preamble(&mut buffer).is_none());
        let framed = Connection::drain_packets(&mut buffer, packetver, PacketTrace::None);
        assert_eq!(framed.len(), 2);
        assert_eq!(
            framed[0]
                .as_any()
                .downcast_ref::<PacketZcAid>()
                .unwrap()
                .aid,
            2000000
        );
        assert!(framed[1].as_any().is::<PacketZcAcceptEnter2>());
        assert!(buffer.is_empty());

        let mut buffer = greeting_stream(packetver, &2000000u32.to_le_bytes());
        assert_eq!(
            Connection::take_bare_aid_preamble(&mut buffer),
            Some(2000000)
        );
        let framed = Connection::drain_packets(&mut buffer, packetver, PacketTrace::None);
        assert_eq!(framed.len(), 1);
        assert!(framed[0].as_any().is::<PacketZcAcceptEnter2>());
        assert!(buffer.is_empty());
    }

    /// A packet arriving in two TCP reads must be held until complete, then
    /// parsed once — whether its length comes from a lookup or from its header.
    #[test]
    fn split_packets_are_held_until_complete() {
        let packetver = 20250716;

        let mut fixed = PacketZcNotifyTime::new(packetver);
        fixed.set_time(1234);
        fixed.fill_raw_with_packetver(Some(packetver));

        let mut variable = PacketZcNotifyPlayerchat::new(packetver);
        variable.set_msg("hello\0".to_string());
        variable.set_packet_length((PacketZcNotifyPlayerchat::base_len(packetver) + 6) as i16);
        variable.fill_raw_with_packetver(Some(packetver));

        for stream in [fixed.raw.clone(), variable.raw.clone()] {
            for split in 1..stream.len() {
                let mut buffer = stream[..split].to_vec();
                let framed = Connection::drain_packets(&mut buffer, packetver, PacketTrace::None);
                assert!(
                    framed.is_empty(),
                    "parsed a packet from {split} of {} bytes",
                    stream.len()
                );
                assert_eq!(buffer.len(), split, "dropped bytes of an incomplete packet");

                buffer.extend_from_slice(&stream[split..]);
                let framed = Connection::drain_packets(&mut buffer, packetver, PacketTrace::None);
                assert_eq!(framed.len(), 1, "split at {split} did not reassemble");
                assert_eq!(framed[0].raw(), &stream);
                assert!(buffer.is_empty());
            }
        }
    }

    /// The length framing advances by must equal the length parsing consumes,
    /// including for packets whose layout is packetver-gated.
    #[test]
    fn frame_len_matches_what_parsing_consumes() {
        let packetver = 20250716;
        let mut ack = PacketZcReqWearEquipAckV5::new(packetver);
        ack.set_index(3);
        ack.fill_raw_with_packetver(Some(packetver));

        let expected = match Connection::frame_len(&ack.raw) {
            FrameLen::Known(len) => len,
            _ => panic!("no framing length at {packetver}"),
        };
        assert_eq!(expected, ack.raw.len());
        assert_eq!(expected, PacketZcReqWearEquipAckV5::base_len(packetver));
    }

    /// The map server's address (0x0ac5, 156 bytes with its DNS host) is held until it is complete and framed whole.
    #[test]
    fn the_zone_server_notice_is_framed_whole() {
        let packetver = 20250716;
        let mut notice = PacketChSendMapInfo::new(packetver);
        notice.set_gid(150000);
        notice.set_map_server_port(6121);
        notice.fill_raw_with_packetver(Some(packetver));
        let stream = notice.raw.clone();

        let expected = match Connection::frame_len(&stream) {
            FrameLen::Known(len) => len,
            _ => panic!("no framing length"),
        };
        assert_eq!(expected, 156);
        assert_eq!(expected, stream.len());

        let mut short = stream[..stream.len() - 6].to_vec();
        assert!(Connection::drain_packets(&mut short, packetver, PacketTrace::None).is_empty());

        let mut buffer = stream.clone();
        let framed = Connection::drain_packets(&mut buffer, packetver, PacketTrace::None);
        assert_eq!(framed.len(), 1);
        let zonesvr = framed[0]
            .as_any()
            .downcast_ref::<PacketChSendMapInfo>()
            .unwrap();
        assert_eq!(zonesvr.gid, 150000);
        assert_eq!(zonesvr.map_server_port, 6121);
        assert!(buffer.is_empty());
    }

    /// A packet split mid-stream must not stall the packets that precede it.
    #[test]
    fn a_trailing_partial_packet_does_not_hold_back_the_complete_ones() {
        let packetver = 20250716;
        let mut time = PacketZcNotifyTime::new(packetver);
        time.set_time(7);
        time.fill_raw_with_packetver(Some(packetver));

        let mut buffer = time.raw.clone();
        buffer.extend_from_slice(&time.raw[..3]);

        let framed = Connection::drain_packets(&mut buffer, packetver, PacketTrace::None);
        assert_eq!(framed.len(), 1);
        assert_eq!(buffer, time.raw[..3]);
    }
}
