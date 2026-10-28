//! The offline server's translated scripts use Windows-1252. Read the original
//! packet bytes: the generated packet's lossy UTF-8 String has already lost accents.
pub(crate) fn decode(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    let bytes = &bytes[..end];
    if let Ok(text) = std::str::from_utf8(bytes) { return text.to_owned(); }
    encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned()
}

pub(crate) fn replace_payload(packet: &mut Vec<u8>, header_len: usize, text: &str) {
    let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(text);
    packet.truncate(header_len);
    packet.extend_from_slice(&bytes);
    packet.push(0);
    let length = u16::try_from(packet.len()).expect("text packet exceeds protocol limit");
    packet[2..4].copy_from_slice(&length.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dialogue_accents_survive_western_and_utf8_packets() {
        assert_eq!(decode(b"Po\xe7\xe3o e recupera\xe7\xe3o\0trailing"), "Poção e recuperação");
        assert_eq!(decode("Poção e recuperação\0".as_bytes()), "Poção e recuperação");
    }
    #[test]
    fn input_uses_single_byte_text_and_its_actual_wire_length() {
        let mut packet=vec![0xd5,0,0,0,1,2,3,4];
        replace_payload(&mut packet,8,"poção");
        assert_eq!(&packet[8..],b"po\xe7\xe3o\0");
        assert_eq!(u16::from_le_bytes([packet[2],packet[3]]) as usize,packet.len());
        assert_eq!(&packet[4..8],&[1,2,3,4]);
    }
}
