//! Packets Ragnarok Solo's server sends at packet versions the generated `packets` crate does not list them for.

use std::any::Any;

use packets::packets::Packet;

/// ZC_HP_INFO (0x0977): a monster's HP for its bar. rAthena sends it from packet version 20120404; Solo's
/// server sends it to every version.
#[derive(Clone, Debug)]
pub struct PacketZcHpInfo {
    pub raw: Vec<u8>,
    pub gid: u32,
    pub hp: i32,
    pub max_hp: i32,
}

impl PacketZcHpInfo {
    pub const ID: [u8; 2] = [0x77, 0x09];
    pub const LEN: usize = 14;

    /// `buffer` holds exactly [`Self::LEN`] bytes, starting with [`Self::ID`].
    pub fn from(buffer: &[u8]) -> Self {
        let u32_at = |at: usize| {
            u32::from_le_bytes([buffer[at], buffer[at + 1], buffer[at + 2], buffer[at + 3]])
        };
        Self {
            raw: buffer.to_vec(),
            gid: u32_at(2),
            hp: u32_at(6) as i32,
            max_hp: u32_at(10) as i32,
        }
    }
}

impl Packet for PacketZcHpInfo {
    fn id(&self, _packetver: u32) -> &str {
        "0x7709"
    }
    fn name(&self) -> &str {
        "PacketZcHpInfo"
    }
    fn base_len(&self, _packetver: u32) -> usize {
        Self::LEN
    }
    fn display(&self) {
        println!("{self:?}");
    }
    fn debug(&self) {
        println!("{self:?}");
    }
    fn pretty_debug(&self) {
        println!("{self:#?}");
    }
    fn fill_raw(&mut self) {}
    fn fill_raw_with_packetver(&mut self, _packetver: Option<u32>) {}
    fn raw(&self) -> &Vec<u8> {
        &self.raw
    }
    fn raw_mut(&mut self) -> &mut Vec<u8> {
        &mut self.raw
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn layout(&self, _packetver: u32) -> Vec<(String, usize, usize)> {
        [("packet_id", 0, 2), ("gid", 2, 4), ("hp", 6, 4), ("max_hp", 10, 4)]
            .map(|(name, offset, len)| (name.to_string(), offset, len))
            .to_vec()
    }
    fn to_json(&self, _packetver: u32) -> String {
        format!(
            r#"{{"gid":{},"hp":{},"max_hp":{}}}"#,
            self.gid, self.hp, self.max_hp
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_hp_info() {
        let mut raw = vec![0x77, 0x09];
        raw.extend(110_015_601u32.to_le_bytes());
        raw.extend(45i32.to_le_bytes());
        raw.extend(60i32.to_le_bytes());
        let packet = PacketZcHpInfo::from(&raw);
        assert_eq!(
            (packet.gid, packet.hp, packet.max_hp),
            (110_015_601, 45, 60)
        );
    }
}

/// ZC_SKILL_SCALE: the existing ground-cast warning sent by classic boss skills.
#[derive(Debug)]
pub struct PacketZcSkillScale {pub raw:Vec<u8>,pub gid:u32,pub skill_id:u16,pub level:u16,pub x:i16,pub y:i16,pub cast_ms:u32}
impl PacketZcSkillScale {
    pub const ID:[u8;2]=[0x41,0x0a];
    pub const LEN:usize=18;
    pub fn from(raw:&[u8])->Self {
        let word=|n|u16::from_le_bytes([raw[n],raw[n+1]]);
        let dword=|n|u32::from_le_bytes(raw[n..n+4].try_into().unwrap());
        Self {raw:raw.to_vec(),gid:dword(2),skill_id:word(6),level:word(8),x:word(10) as i16,y:word(12) as i16,cast_ms:dword(14)}
    }
}
impl Packet for PacketZcSkillScale {
    fn id(&self,_:u32)->&str {"0x410a"}
    fn name(&self)->&str {"PacketZcSkillScale"}
    fn base_len(&self,_:u32)->usize {Self::LEN}
    fn display(&self){println!("{self:?}");}
    fn debug(&self){println!("{self:?}");}
    fn pretty_debug(&self){println!("{self:#?}");}
    fn fill_raw(&mut self){}
    fn fill_raw_with_packetver(&mut self,_:Option<u32>){}
    fn raw(&self)->&Vec<u8>{&self.raw}
    fn raw_mut(&mut self)->&mut Vec<u8>{&mut self.raw}
    fn as_any(&self)->&dyn Any{self}
    fn as_any_mut(&mut self)->&mut dyn Any{self}
    fn layout(&self,_:u32)->Vec<(String,usize,usize)>{[("packet_id",0,2),("gid",2,4),("skill_id",6,2),("level",8,2),("x",10,2),("y",12,2),("cast_ms",14,4)].map(|(s,p,n)|(s.into(),p,n)).to_vec()}
    fn to_json(&self,_:u32)->String{format!("{{\"gid\":{},\"skill_id\":{},\"level\":{},\"x\":{},\"y\":{},\"cast_ms\":{}}}",self.gid,self.skill_id,self.level,self.x,self.y,self.cast_ms)}
}

#[cfg(test)]
mod skill_scale_tests {
    use super::*;
    #[test]
    fn boss_warning_reaches_the_existing_ground_cast_marker() {
        let mut raw=PacketZcSkillScale::ID.to_vec();raw.extend(110_015_619u32.to_le_bytes());
        raw.extend(661u16.to_le_bytes());raw.extend(1u16.to_le_bytes());raw.extend(58i16.to_le_bytes());raw.extend(193i16.to_le_bytes());raw.extend(800u32.to_le_bytes());
        assert_eq!(raw.len(),PacketZcSkillScale::LEN);
        let packet=PacketZcSkillScale::from(&raw);
        let events=crate::handler::dispatch_packet(&packet);
        match &events[..] {
            [nostalre_game::event::GameEvent::SkillCasting {gid,skill,x,y,delay_ms,..}]=>assert_eq!((*gid,skill.id(),*x,*y,*delay_ms),(110_015_619,661,58,193,800)),
            _=>panic!("missing boss ground warning: {events:?}"),
        }
    }
}
