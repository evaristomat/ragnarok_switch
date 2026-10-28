//! The version-gated packet families at the client's version, pinned by hand: their lengths as rAthena's table has
//! them (`data/rathena-20250716.tsv`), and the ids they take answered by the handler. A `packets_db` edit
//! that moves a gate fails here by name, beside the layout check.

use crate::packet_status::handled_events;
use packets::packets::*;

const PV: u32 = crate::rathena_table::PACKETVER;

#[test]
fn version_gated_lengths() {
    for (packet, ours, server) in [
        (
            "0x0999 ZC_REQ_WEAR_EQUIP_ACK",
            PacketZcReqWearEquipAckV5::base_len(PV),
            11,
        ),
        (
            "0x099a ZC_REQ_TAKEOFF_EQUIP_ACK",
            PacketZcReqTakeoffEquipAckV5::base_len(PV),
            9,
        ),
        (
            "0x01a2 ZC_PROPERTY_PET",
            PacketZcPropertyPet::base_len(PV),
            37,
        ),
        ("0x029b ZC_MER_INIT", PacketZcMerInit::base_len(PV), 80),
        ("0x09f9 ZC_ADD_QUEST", PacketZcAddQuest::base_len(PV), 143),
        (
            "0x0b20 ZC_SHORTCUT_KEY_LIST",
            PacketZcShortcutKeyListV2::base_len(PV),
            271,
        ),
        (
            "0x0ba4 ZC_PROPERTY_HOMUN",
            PacketZcPropertyHomun::base_len(PV),
            85,
        ),
        ("0x0acc ZC_NOTIFY_EXP", PacketZcNotifyExp::base_len(PV), 18),
        (
            "0x0b41 ZC_ITEM_PICKUP_ACK",
            PacketZcItemPickupAckV7::base_len(PV),
            70,
        ),
        (
            "0x0add ZC_ITEM_FALL_ENTRY",
            PacketZcItemFallEntryV5::base_len(PV),
            24,
        ),
        ("0x08c8 ZC_NOTIFY_ACT", PacketZcNotifyAct3::base_len(PV), 34),
        (
            "0x0a30 ZC_ACK_REQNAMEALL",
            PacketZcAckReqnameall2::base_len(PV),
            106,
        ),
        (
            "0x01d7 ZC_SPRITE_CHANGE",
            PacketZcSpriteChange2::base_len(PV),
            15,
        ),
    ] {
        assert_eq!(ours, server, "{packet}");
    }
}

/// The ids the 2012 families moved to by 20220406, and the packets added by 20250716: each parses to a packet the
/// handler makes an event of (not only acknowledges).
#[test]
fn version_gated_ids_are_answered() {
    let ids: &[u16] = &[
        0x09ca, // skill unit on the ground
        0x0add, // item on the floor
        0x01d7, // look change
        0x0983, // status change
        0x09f8, 0x09f9, 0x09fa, // quest list, added quest, hunt update
        0x0b09, 0x0b39, // item lists (inventory, cart, storage)
        0x0abe, // warp list
        0x0999, 0x099a, // equip and unequip
        0x0b1a, // skill cast
        0x0ae4, // party member
        0x099b, // map property
        0x0acc, // experience
        0x09ff, 0x09fe, 0x09fd, // units standing, new, walking
        0x0b8c, // NPC music
        0x0bae, // unequip all answered
        0x0bbe, // special pop-up
        0x0b79, // expanded barter list
        0x0ba2, 0x0ba3, 0x0bb5, // dialog window size and position
    ];
    let silent: Vec<String> = ids
        .iter()
        .filter(|id| {
            handled_events(**id, PV)
                .0
                .iter()
                .all(|event| event == "Acknowledged")
        })
        .map(|id| format!("0x{id:04x}"))
        .collect();
    assert!(
        silent.is_empty(),
        "not answered at {PV}: {}",
        silent.join(", ")
    );
}
