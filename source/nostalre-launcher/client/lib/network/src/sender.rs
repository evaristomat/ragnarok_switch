use crate::session::Session;
use models::enums::skill_enums::SkillEnum;
use packets::packets::*;
use nostalre_game::event::{BarterPurchase, FameKind};
use nostalre_game::stylist::StyleChoice;

pub fn build_login_packet(username: &str, password: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCaLogin::new(packetver);
    pkt.set_version(20);

    let mut id = [0 as char; 24];
    for (i, c) in username.chars().take(23).enumerate() {
        id[i] = c;
    }
    pkt.set_id(id);

    let mut passwd = [0 as char; 24];
    for (i, c) in password.chars().take(23).enumerate() {
        passwd[i] = c;
    }
    pkt.set_passwd(passwd);
    pkt.set_client_type(0);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_char_enter_packet(session: &Session) -> Vec<u8> {
    let mut pkt = PacketChEnter::new(session.packetver);
    pkt.set_aid(session.account_id);
    pkt.set_auth_code(session.login_id1);
    pkt.set_user_level(session.login_id2);
    pkt.set_client_type(0);
    pkt.set_sex(session.sex);
    pkt.fill_raw_with_packetver(Some(session.packetver));
    pkt.raw
}

pub fn build_select_char_packet(slot: u8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketChSelectChar::new(packetver);
    pkt.set_char_num(slot);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_select_accessible_map_packet(slot: u8, map_index: u8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketChSelectAccessibleMapname::new(packetver);
    pkt.set_char_num(slot);
    pkt.set_map_list_num(map_index);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// From packetver 20151001 a new character carries its starting job and sex; before, only Novices of the
/// account's sex are made.
pub fn build_make_char_packet(
    name: &str,
    slot: u8,
    hair_style: u16,
    hair_color: u16,
    job: u16,
    sex: u8,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketChMakeChar3::new(packetver);
    pkt.set_name(name_to_char24(name));
    pkt.set_char_num(slot);
    pkt.set_head_pal(hair_color as i16);
    pkt.set_head(hair_style as i16);
    pkt.set_class(job as i32);
    pkt.set_sex(sex);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_delete_char_reserve_packet(gid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketChDeleteChar3Reserved::new(packetver);
    pkt.set_gid(gid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_delete_char_confirm_packet(gid: u32, birthdate: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketChDeleteChar3::new(packetver);
    pkt.set_gid(gid);
    pkt.set_birth(birthdate_to_char6(birthdate));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_delete_char_cancel_packet(gid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketChDeleteChar3Cancel::new(packetver);
    pkt.set_gid(gid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_request_move_packet(dest_x: u16, dest_y: u16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzRequestMove::new(packetver);
    pkt.set_dest(crate::helpers::encode_pos(dest_x, dest_y, 0));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_map_loaded_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzNotifyActorinit::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_chat_packet(msg: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzPlayerChat::new(packetver);
    let msg_null = format!("{msg}\0");
    pkt.set_packet_length((4 + msg_null.len()) as i16);
    pkt.set_msg(msg_null);
    pkt.fill_raw_with_packetver(Some(packetver));
    #[cfg(target_os = "horizon")]
    crate::offline_text::replace_payload(&mut pkt.raw, 4, msg);
    pkt.raw
}

pub fn build_emotion_packet(emote_type: u8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqEmotion::new(packetver);
    pkt.set_atype(emote_type);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_action_request_packet(target_gid: u32, action: u8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzRequestAct::new(packetver);
    pkt.set_target_gid(target_gid);
    pkt.set_action(action);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_change_direction_packet(head_dir: u8, dir: u8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzChangeDirection::new(packetver);
    pkt.set_head_dir(head_dir as i16);
    pkt.set_dir(dir);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rathena_table::{self, WireLen};

    const PV: u32 = rathena_table::PACKETVER;

    const BARTER_LINE: BarterPurchase = BarterPurchase {
        item_id: 984,
        shop_index: 1,
        amount: 2,
        inv_index: 5,
    };

    /// One request per `sender.rs` builder: (the server's handler, or a login/char packet's name; the builder; its
    /// bytes). `every_builder_has_a_request_case` keeps it complete.
    fn request_cases() -> Vec<(&'static str, &'static str, Vec<u8>)> {
        let pv = PV;
        macro_rules! case {
            ($handler:expr, $builder:ident($($arg:expr),* $(,)?)) => {
                ($handler, stringify!($builder), $builder($($arg),*))
            };
        }
        let mut session = Session::new(pv);
        session.store_login(2000000, 1, 2, 1);
        session.store_zone_info(150000, "prontera.gat".into());
        let skill = SkillEnum::MgFirebolt;
        vec![
            case!("CA_LOGIN", build_login_packet("tester", "tester", pv)),
            case!("CH_SELECT_CHAR", build_select_char_packet(0, pv)),
            case!(
                "CH_MAKE_CHAR",
                build_make_char_packet("Spike", 1, 1, 0, 4218, 1, pv)
            ),
            case!(
                "clif_parse_WantToConnection",
                build_zone_enter_packet(&session)
            ),
            case!("clif_parse_LoadEndAck", build_map_loaded_packet(pv)),
            case!(
                "clif_parse_blocking_playcancel",
                build_blocking_play_cancel_packet(pv)
            ),
            case!(
                "clif_parse_WalkToXY",
                build_request_move_packet(150, 150, pv)
            ),
            case!("clif_parse_TickSend", build_request_time_packet(1000, pv)),
            case!(
                "clif_parse_ChangeDir",
                build_change_direction_packet(0, 2, pv)
            ),
            case!(
                "clif_parse_ActionRequest",
                build_action_request_packet(110000000, 7, pv)
            ),
            case!(
                "clif_parse_GetCharNameRequest",
                build_reqname_packet(110000000, pv)
            ),
            case!(
                "clif_parse_SolveCharName",
                build_solve_char_name_packet(150000, pv)
            ),
            case!(
                "clif_parse_GlobalMessage",
                build_chat_packet("Spike : hello", pv)
            ),
            case!("clif_parse_Emotion", build_emotion_packet(1, pv)),
            case!("clif_parse_Restart", build_restart_packet(pv)),
            case!(
                "clif_parse_TakeItem",
                build_pickup_item_packet(110000000, pv)
            ),
            case!("clif_parse_DropItem", build_drop_item_packet(2, 1, pv)),
            case!("clif_parse_UseItem", build_use_item_packet(2, 2000000, pv)),
            case!(
                "clif_parse_MoveToKafra",
                build_move_item_body_to_store_packet(2, 1, pv)
            ),
            case!(
                "clif_parse_MoveFromKafra",
                build_move_item_store_to_body_packet(2, 1, pv)
            ),
            case!(
                "clif_parse_StoragePassword",
                build_ack_store_password_packet(false, "1234", "", pv)
            ),
            case!(
                "clif_parse_UseSkillToId",
                build_use_skill_packet(skill, 10, 110000000, pv)
            ),
            case!(
                "clif_parse_UseSkillToPos",
                build_use_skill_to_ground_packet(skill, 10, 150, 150, pv)
            ),
            case!(
                "clif_parse_UseSkillToPosMoreInfo",
                build_use_skill_to_ground_with_talkbox_packet(skill, 1, 150, 150, "hi", pv)
            ),
            case!(
                "clif_parse_NpcClicked",
                build_contact_npc_packet(110000000, pv)
            ),
            case!(
                "clif_parse_NpcNextClicked",
                build_npc_next_packet(110000000, pv)
            ),
            case!(
                "clif_parse_NpcSelectMenu",
                build_npc_menu_select_packet(110000000, 1, pv)
            ),
            case!(
                "clif_parse_NpcCloseClicked",
                build_npc_close_packet(110000000, pv)
            ),
            case!("clif_parse_StatusUp", build_stat_change_packet(13, 1, pv)),
            case!(
                "clif_parse_traitstatus_up",
                build_trait_stat_change_packet(219, 1, pv)
            ),
            case!(
                "clif_parse_WisMessage",
                build_whisper_packet("Other", "hi", pv)
            ),
            case!(
                "clif_parse_FriendsListAdd",
                build_add_friend_packet("Other", pv)
            ),
            case!(
                "clif_parse_PartyInvite2",
                build_party_invite_by_name_packet("Other", pv)
            ),
            case!("clif_parse_HomMenu", build_homun_menu_packet(1, pv)),
            // Items, protocol step 3.2
            case!("clif_parse_EquipItem", build_equip_item_packet(2, 2, pv)),
            case!("clif_parse_UnequipItem", build_unequip_item_packet(2, pv)),
            case!(
                "clif_parse_unequipall",
                build_unequip_all_packet(u32::MAX, pv)
            ),
            case!(
                "clif_parse_change_title",
                build_change_title_packet(1000, pv)
            ),
            case!(
                "clif_parse_equipswitch_add",
                build_equip_switch_add_packet(2, 0x10, pv)
            ),
            case!(
                "clif_parse_equipswitch_remove",
                build_equip_switch_remove_packet(2, pv)
            ),
            case!(
                "clif_parse_equipswitch_request",
                build_equip_switch_packet(pv)
            ),
            case!(
                "clif_parse_equipswitch_request_single",
                build_equip_switch_single_packet(2, pv)
            ),
            case!(
                "clif_parse_PutItemToCart",
                build_move_item_body_to_cart_packet(2, 1, pv)
            ),
            case!(
                "clif_parse_GetItemFromCart",
                build_move_item_cart_to_body_packet(2, 1, pv)
            ),
            case!(
                "clif_parse_MoveFromKafraToCart",
                build_move_item_store_to_cart_packet(2, 1, pv)
            ),
            case!(
                "clif_parse_MoveToKafraFromCart",
                build_move_item_cart_to_store_packet(2, 1, pv)
            ),
            case!("clif_parse_CloseKafra", build_close_store_packet(pv)),
            case!(
                "clif_parse_NpcBuyListSend",
                build_purchase_item_list_packet(&[(1, 501)], pv)
            ),
            case!(
                "clif_parse_NpcSellListSend",
                build_sell_item_list_packet(&[(2, 1)], pv)
            ),
            case!(
                "clif_parse_TradeRequest",
                build_req_exchange_item_packet(2000001, pv)
            ),
            case!("clif_parse_TradeAck", build_ack_exchange_item_packet(3, pv)),
            case!(
                "clif_parse_TradeAddItem",
                build_add_exchange_item_packet(2, 1, pv)
            ),
            case!(
                "clif_parse_TradeOk",
                build_conclude_exchange_item_packet(pv)
            ),
            case!(
                "clif_parse_TradeCancel",
                build_cancel_exchange_item_packet(pv)
            ),
            case!(
                "clif_parse_TradeCommit",
                build_exec_exchange_item_packet(pv)
            ),
            case!("clif_parse_RemoveOption", build_cartoff_packet(pv)),
            case!("clif_parse_ChangeCart", build_change_cart_packet(1, pv)),
            case!(
                "clif_parse_NoviceExplosionSpirits",
                build_novice_prayer_packet(pv)
            ),
            case!(
                "clif_parse_SelectCart",
                build_select_cart_packet(2_000_000, 11, pv)
            ),
            case!(
                "clif_parse_UseCard",
                build_card_composition_list_packet(2, pv)
            ),
            case!(
                "clif_parse_InsertCard",
                build_card_composition_packet(2, 3, pv)
            ),
            case!(
                "clif_parse_ItemIdentify",
                build_req_itemidentify_packet(2, pv)
            ),
            case!(
                "clif_parse_SelectArrow",
                build_req_makingarrow_packet(1750, pv)
            ),
            case!(
                "clif_parse_ProduceMix",
                build_req_makingitem_packet(1201, [0; 3], pv)
            ),
            // Mix Cooking's list (6), a Bun (12062)
            case!("clif_parse_Cooking", build_req_cooking_packet(6, 12062, pv)),
            case!(
                "clif_parse_WeaponRefine",
                build_req_weaponrefine_packet(2, pv)
            ),
            case!(
                "clif_parse_RepairItem",
                build_req_itemrepair_packet(2, 1201, 0, [0; 4], pv)
            ),
            case!(
                "clif_parse_refineui_add",
                build_refining_select_item_packet(2, pv)
            ),
            case!(
                "clif_parse_refineui_refine",
                build_req_refining_packet(2, 984, true, pv)
            ),
            case!(
                "clif_parse_refineui_close",
                build_close_refining_ui_packet(pv)
            ),
            case!(
                "clif_parse_NPCMarketPurchase",
                build_market_purchase_packet(&[(2, 1010)], pv)
            ),
            case!("clif_parse_NPCMarketClosed", build_market_close_packet(pv)),
            case!("clif_parse_NPCShopClosed", build_npc_trade_quit_packet(pv)),
            case!(
                "clif_parse_barter_buy",
                build_barter_purchase_packet(false, &[BARTER_LINE], pv)
            ),
            case!(
                "clif_parse_barter_extended_buy",
                build_barter_purchase_packet(true, &[BARTER_LINE], pv)
            ),
            case!(
                "clif_parse_barter_close",
                build_barter_close_packet(false, pv)
            ),
            case!(
                "clif_parse_stylist_buy",
                build_stylist_buy_packet(&StyleChoice([1, 2, 0, 0, 5, 0, 0]), pv)
            ),
            case!("clif_parse_stylist_close", build_stylist_close_packet(pv)),
            case!(
                "clif_parse_barter_extended_close",
                build_barter_close_packet(true, pv)
            ),
            case!(
                "clif_parse_OpenVending",
                build_req_openstore2_packet("Shop", &[(2, 1, 100)], pv)
            ),
            case!("clif_parse_CloseVending", build_req_closestore_packet(pv)),
            case!(
                "clif_parse_VendingListReq",
                build_req_buy_frommc_packet(2000001, pv)
            ),
            case!(
                "clif_parse_PurchaseReq2",
                build_purchase_frommc2_packet(2000001, 1, &[(1, 2)], pv)
            ),
            // Character server
            case!(
                "chclif_parse_reqtoconnect",
                build_char_enter_packet(&session)
            ),
            case!("PING", build_char_ping_packet(2000000, pv)),
            case!(
                "CH_SELECT_ACCESSIBLE_MAPNAME",
                build_select_accessible_map_packet(0, 1, pv)
            ),
            case!(
                "CH_DELETE_CHAR3_RESERVED",
                build_delete_char_reserve_packet(150000, pv)
            ),
            case!(
                "CH_DELETE_CHAR3",
                build_delete_char_confirm_packet(150000, "000101", pv)
            ),
            case!(
                "CH_DELETE_CHAR3_CANCEL",
                build_delete_char_cancel_packet(150000, pv)
            ),
            // World, NPCs, chat rooms
            case!("clif_parse_Restart", build_return_savepoint_packet(pv)),
            case!("clif_parse_progressbar", build_progress_done_packet(pv)),
            case!("clif_parse_StopAttack", build_cancel_lockon_packet(pv)),
            case!(
                "clif_parse_AutoRevive",
                build_standing_resurrection_packet(pv)
            ),
            case!("clif_parse_QuitGame", build_req_disconnect_packet(pv)),
            case!(
                "clif_parse_NpcAmountInput",
                build_npc_input_number_packet(110000000, 5, pv)
            ),
            case!(
                "clif_parse_NpcStringInput",
                build_npc_input_string_packet(110000000, "hi", pv)
            ),
            case!(
                "clif_parse_NpcBuySellSelected",
                build_npc_deal_type_packet(110000000, 0, pv)
            ),
            case!(
                "clif_parse_ChatAddMember",
                build_req_enter_room_packet(1, pv)
            ),
            case!(
                "clif_parse_CreateChatRoom",
                build_create_chatroom_packet("Room", 5, true, "", pv)
            ),
            case!(
                "clif_parse_ChatRoomStatusChange",
                build_change_chatroom_packet("Room", 5, true, "", pv)
            ),
            case!(
                "clif_parse_ChangeChatOwner",
                build_change_chat_owner_packet("Other", pv)
            ),
            case!(
                "clif_parse_KickFromChat",
                build_expel_chat_member_packet("Other", pv)
            ),
            case!("clif_parse_ChatLeave", build_exit_room_packet(pv)),
            case!(
                "clif_parse_RequestMemo",
                build_remember_warppoint_packet(pv)
            ),
            case!("clif_parse_LessEffect", build_lesseffect_packet(true, pv)),
            case!("clif_parse_HowManyConnections", build_user_count_packet(pv)),
            case!(
                "clif_parse_GMReqNoChat",
                build_give_manner_point_packet(2000001, true, 1, pv)
            ),
            case!(
                "clif_parse_GMRc",
                build_give_manner_byname_packet("Other", pv)
            ),
            case!("clif_parse_Check", build_status_gm_packet("Other", pv)),
            case!(
                "clif_parse_GMReqAccountName",
                build_account_name_packet(2000001, pv)
            ),
            case!(
                "clif_parse_ranklist",
                build_ranking_request_packet(FameKind::Alchemist, pv)
            ),
            case!("clif_parse_configuration", build_config_packet(0, 1, pv)),
            case!(
                "clif_parse_questStateAck",
                build_active_quest_packet(1000, true, pv)
            ),
            case!(
                "clif_parse_FeelSaveOk",
                build_agree_star_place_packet(0, pv)
            ),
            case!("clif_parse_NoviceDoriDori", build_doridori_packet(pv)),
            // Chat and whispers
            case!(
                "clif_parse_GuildMessage",
                build_guild_chat_packet("Spike : hi", pv)
            ),
            case!(
                "clif_parse_PartyMessage",
                build_party_chat_packet("Spike : hi", pv)
            ),
            case!(
                "clif_parse_PMIgnore",
                build_setting_whisper_pc_packet("Other", true, pv)
            ),
            case!(
                "clif_parse_PMIgnoreAll",
                build_setting_whisper_state_packet(true, pv)
            ),
            // Mail
            case!(
                "clif_parse_Mail_refreshinbox",
                build_mail_get_list_packet(pv)
            ),
            case!("clif_parse_Mail_read", build_mail_open_packet(1, pv)),
            case!("clif_parse_Mail_delete", build_mail_delete_packet(1, pv)),
            case!(
                "clif_parse_Mail_getattach",
                build_mail_get_item_packet(1, pv)
            ),
            case!(
                "clif_parse_Mail_winopen",
                build_mail_reset_item_packet(0, pv)
            ),
            case!(
                "clif_parse_Mail_setattach",
                build_mail_add_item_packet(2, 1, pv)
            ),
            case!(
                "clif_parse_Mail_return",
                build_req_mail_return_packet(1, "Other", pv)
            ),
            case!(
                "clif_parse_Mail_send",
                build_mail_send_packet("Other", "Title", "Body", pv)
            ),
            // Skills, hotkeys
            case!("clif_parse_SkillUp", build_upgrade_skill_packet(skill, pv)),
            case!(
                "clif_parse_Hotkey",
                build_shortcut_key_change_packet(0, 1, 19, 1, pv)
            ),
            case!(
                "clif_parse_UseSkillMap",
                build_select_warppoint_packet(SkillEnum::AlWarp, "prontera", pv)
            ),
            case!(
                "clif_parse_AutoSpell",
                build_select_autospell_packet(Some(SkillEnum::MgColdbolt), pv)
            ),
            case!(
                "clif_parse_SkillSelectMenu",
                build_skill_select_response_packet(1, Some(SkillEnum::MgFirebolt), pv)
            ),
            case!(
                "clif_parse_ItemListWindowSelected",
                build_itemlistwin_res_packet(0, true, &[(5, 45), (9, 35)], pv)
            ),
            // Items (the rest)
            case!("clif_parse_RemoveOption", build_remove_option_packet(pv)),
            case!(
                "clif_parse_OpenVending",
                build_req_cancel_openstore_packet(pv)
            ),
            // Party, family, friends
            case!(
                "clif_parse_CreateParty",
                build_make_party_packet("Party", pv)
            ),
            case!(
                "clif_parse_CreateParty2",
                build_make_party2_packet("Party", 0, 0, pv)
            ),
            case!(
                "clif_parse_PartyInvite",
                build_req_join_party_packet(2000001, pv)
            ),
            case!(
                "clif_parse_ReplyPartyInvite2",
                build_join_party_reply_packet(1, true, pv)
            ),
            case!("clif_parse_LeaveParty", build_leave_party_packet(pv)),
            case!(
                "clif_parse_RemovePartyMember",
                build_expel_party_member_packet(2000001, "Other", pv)
            ),
            case!(
                "clif_parse_PartyChangeOption",
                build_change_party_option_packet(1, 1, 0, pv)
            ),
            case!(
                "clif_parse_GuildChangeEmblem2",
                build_add_new_emblem_packet(7, 3, pv)
            ),
            case!(
                "clif_parse_PartyChangeLeader",
                build_change_party_leader_packet(2000001, pv)
            ),
            case!(
                "clif_parse_Adopt_request",
                build_adopt_request_packet(2000001, pv)
            ),
            case!(
                "clif_parse_Adopt_reply",
                build_adopt_reply_packet(2000001, 2000002, true, pv)
            ),
            case!(
                "CZ_REQ_JOIN_COUPLE",
                build_marry_request_packet(2000001, pv)
            ),
            case!(
                "CZ_JOIN_COUPLE",
                build_marry_reply_packet(2000001, 150001, true, pv)
            ),
            case!(
                "clif_parse_FriendsListReply",
                build_ack_add_friend_packet(2000001, 150001, true, pv)
            ),
            case!(
                "clif_parse_FriendsListRemove",
                build_delete_friend_packet(2000001, 150001, pv)
            ),
            // Companions
            case!(
                "clif_parse_HomMoveTo",
                build_companion_move_packet(110000001, 150, 150, pv)
            ),
            case!(
                "clif_parse_HomAttack",
                build_companion_attack_packet(110000001, 110000002, pv)
            ),
            case!(
                "clif_parse_HomMoveToMaster",
                build_companion_move_to_owner_packet(110000001, pv)
            ),
            case!(
                "clif_parse_mercenary_action",
                build_mercenary_command_packet(1, pv)
            ),
            case!(
                "clif_parse_ChangeHomunculusName",
                build_rename_homun_packet("Lif", pv)
            ),
            case!(
                "clif_parse_CatchPet",
                build_trycapture_packet(110000001, pv)
            ),
            case!("clif_parse_PetMenu", build_command_pet_packet(0, pv)),
            case!(
                "clif_parse_ChangePetName",
                build_rename_pet_packet("Poring", pv)
            ),
            case!("clif_parse_SelectEgg", build_select_petegg_packet(2, pv)),
            case!("clif_parse_SendEmotion", build_pet_act_packet(0, pv)),
            // Guild
            case!(
                "clif_parse_GuildCheckMaster",
                build_req_guild_menuinterface(pv)
            ),
            case!("clif_parse_GuildRequestInfo", build_req_guild_menu(0, pv)),
            case!(
                "clif_parse_GuildChangeNotice",
                build_guild_notice(1, "Subject", "Notice", pv)
            ),
            case!(
                "clif_parse_GuildLeave",
                build_req_leave_guild(1, 2000000, 150000, "bye", pv)
            ),
            case!(
                "clif_parse_GuildExpulsion",
                build_req_ban_guild(1, 2000001, 150001, "bye", pv)
            ),
            case!(
                "clif_parse_GuildChangeMemberPosition",
                build_req_change_memberpos(2000001, 150001, 1, pv)
            ),
            case!(
                "clif_parse_GuildChangePositionInfo",
                build_reg_change_guild_positioninfo(&[], pv)
            ),
            case!(
                "clif_parse_CreateGuild",
                build_make_guild(150000, "Guild", pv)
            ),
            case!(
                "clif_parse_GuildBreak",
                build_req_disorganize_guild("Guild", pv)
            ),
            case!(
                "clif_parse_GuildRequestEmblem",
                build_req_guild_emblem_img(1, pv)
            ),
            case!(
                "clif_parse_GuildChangeEmblem",
                build_register_guild_emblem(vec![0; 64], pv)
            ),
            case!(
                "clif_parse_GuildInvite",
                build_req_join_guild(2000001, 2000000, 150000, pv)
            ),
            case!(
                "clif_parse_GuildReplyInvite",
                build_ans_join_guild(1, true, pv)
            ),
            case!(
                "clif_parse_GuildRequestAlliance",
                build_req_ally_guild(2000001, 2000000, 150000, pv)
            ),
            case!(
                "clif_parse_GuildReplyAlliance",
                build_ally_guild(2000001, true, pv)
            ),
            case!(
                "clif_parse_GuildOpposition",
                build_req_hostile_guild(2000001, pv)
            ),
            case!(
                "clif_parse_GuildDelAlliance",
                build_req_delete_related_guild(2, 0, pv)
            ),
        ]
    }

    /// Builders whose request is still wrong at 20250716: (builder, why). The work list of the request side; it
    /// only shrinks (a builder that comes right fails `requests_match_the_server_at_each_version` until it leaves).
    const NOT_YET_REQUESTS: &[(&str, &str)] = &[
        ("build_marry_request_packet", MARRIAGE),
        ("build_marry_reply_packet", MARRIAGE),
        ("build_mail_get_list_packet", MAIL),
        ("build_mail_open_packet", MAIL),
        ("build_mail_delete_packet", MAIL),
        ("build_mail_get_item_packet", MAIL),
        ("build_mail_reset_item_packet", MAIL),
        ("build_mail_add_item_packet", MAIL),
        ("build_req_mail_return_packet", MAIL),
        ("build_mail_send_packet", MAIL),
    ];
    const MAIL: &str = "RODEX replaced this mail request at 20250716; RODEX is left out (docs/nostalre-protocol-plan.md, section 5)";
    const MARRIAGE: &str = "rAthena registers the length only, no handler: weddings run through NPC scripts, the request is dropped";

    /// What is wrong with one request: its handler is not in the server's table (no current id), or it goes out
    /// under another id or length than the server parses. None when it matches.
    fn request_mismatch(
        table: &rathena_table::PacketTable,
        handler: &str,
        raw: &[u8],
    ) -> Option<String> {
        let Some((id, parsed)) = table.parsed_by_handler(handler) else {
            return Some(format!("{handler}: no current id in the server's table"));
        };
        let sent_id = u16::from_le_bytes([raw[0], raw[1]]);
        let len_ok = match parsed.len {
            WireLen::Fixed(n) => raw.len() == n,
            WireLen::Variable => {
                raw.len() >= 4 && u16::from_le_bytes([raw[2], raw[3]]) as usize == raw.len()
            }
        };
        (sent_id != id || !len_ok).then(|| {
            format!(
                "{handler}: sent 0x{sent_id:04x} ({} bytes), server wants 0x{id:04x} ({:?})",
                raw.len(),
                parsed.len
            )
        })
    }

    /// Every request the client sends goes out under the id the server's table gives the client for the request's
    /// handler (login and char requests: by packet name), at the length it parses, with a variable request
    /// declaring its own length (lib/network/data/rathena-<version>.tsv). `NOT_YET_REQUESTS` are expected to
    /// differ, and must.
    #[test]
    fn requests_match_the_server() {
        let table = rathena_table::table();
        let mut wrong = Vec::new();
        for (handler, builder, raw) in request_cases() {
            let mismatch = request_mismatch(table, handler, &raw);
            let not_yet = NOT_YET_REQUESTS.iter().any(|(b, _)| *b == builder);
            match (mismatch, not_yet) {
                (Some(m), false) => wrong.push(format!("{builder}: {m}")),
                (None, true) => wrong.push(format!(
                    "{builder}: matches now, take it off NOT_YET_REQUESTS"
                )),
                _ => {}
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    /// Every `pub fn build_*` in sender.rs has a case in `request_cases`, so a new or changed request cannot go out
    /// unchecked (equipping was sent under a dead id for want of one); `NOT_YET_REQUESTS` names only builders.
    #[test]
    fn every_builder_has_a_request_case() {
        let cased: Vec<&str> = request_cases().iter().map(|(_, b, _)| *b).collect();
        let builders = crate::packet_check::builder_names();
        let mut wrong: Vec<String> = builders
            .iter()
            .filter(|b| !cased.contains(&b.as_str()))
            .map(|b| format!("{b}: no case in request_cases"))
            .collect();
        wrong.extend(
            NOT_YET_REQUESTS
                .iter()
                .filter(|(b, _)| !builders.iter().any(|name| name == b))
                .map(|(b, _)| format!("{b}: in NOT_YET_REQUESTS but not a builder")),
        );
        assert!(!builders.is_empty());
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    /// Variable requests whose body the server's table cannot describe (no struct, only the widths read at a few
    /// offsets), with how they are checked instead: (handler, how). A list inside one is invisible to the layout
    /// checks; the shop buy list went out with 16-bit item ids that way.
    const CHECKED_BY_HAND: &[(&str, &str)] = &[
        ("clif_parse_GlobalMessage", TEXT),
        ("clif_parse_PartyMessage", TEXT),
        ("clif_parse_GuildMessage", TEXT),
        (
            "clif_parse_WisMessage",
            "a 24-byte name at 4, then text the server reads as a string",
        ),
        (
            "clif_parse_GuildChangeEmblem",
            "the rest is the emblem's bytes",
        ),
        (
            "clif_parse_OpenVending",
            "entries checked in changed_request_layouts_at_20250716",
        ),
        (
            "clif_parse_GuildChangePositionInfo",
            "entries checked in changed_request_layouts_at_20250716",
        ),
        (
            "clif_parse_ItemListWindowSelected",
            "entries checked in changed_request_layouts_at_20250716",
        ),
    ];
    const TEXT: &str = "the rest is text the server reads as a string";

    /// Every variable request the client sends that the server's table has no full layout for is in
    /// `CHECKED_BY_HAND`, and every entry there is such a request.
    #[test]
    fn requests_without_a_server_layout_are_checked_by_hand() {
        let table = rathena_table::table();
        let mut blind = Vec::new();
        for (handler, builder, _) in request_cases() {
            if NOT_YET_REQUESTS.iter().any(|(b, _)| *b == builder) {
                continue;
            }
            let Some((id, parsed)) = table.parsed_by_handler(handler) else {
                continue;
            };
            let full_layout = table
                .layout(id, false)
                .is_some_and(|fields| fields.iter().any(|f| !f.name.starts_with("pos[")));
            if parsed.len == WireLen::Variable && !full_layout && !blind.contains(&handler) {
                blind.push(handler);
            }
        }
        let mut wrong: Vec<String> = blind
            .iter()
            .filter(|h| !CHECKED_BY_HAND.iter().any(|(c, _)| c == *h))
            .map(|h| format!("{h}: variable, no server layout: check it by hand and list it in CHECKED_BY_HAND"))
            .collect();
        wrong.extend(
            CHECKED_BY_HAND
                .iter()
                .filter(|(c, _)| !blind.contains(c))
                .map(|(c, _)| {
                    format!(
                        "{c}: in CHECKED_BY_HAND but the table describes it (or nothing sends it)"
                    )
                }),
        );
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    /// Field offsets the server reads at 20250716, checked where they changed: the map login's sex moved by four
    /// bytes, the talkbox message shrank to 21 bytes, item ids and equip locations are 32-bit; and the entries of
    /// the variable requests in `CHECKED_BY_HAND`, which the server's table cannot describe.
    #[test]
    fn changed_request_layouts_at_20250716() {
        let pv = 20250716;
        let long = |raw: &[u8], at: usize| u32::from_le_bytes(raw[at..at + 4].try_into().unwrap());
        // CZ_PARTY_JOIN_REQ_ACK: GRID.L accept.B
        assert_eq!(
            build_join_party_reply_packet(7, true, pv),
            [0xc7, 0x02, 7, 0, 0, 0, 1]
        );
        // CZ_REQ_ADD_NEW_EMBLEM: guild_id.L version.L
        assert_eq!(
            build_add_new_emblem_packet(7, 3, pv),
            [0x46, 0x0b, 7, 0, 0, 0, 3, 0, 0, 0]
        );
        // clif_parse_PartyChangeOption: exp.L pickup.B share.B, all three set at once
        assert_eq!(
            build_change_party_option_packet(1, 1, 0, pv),
            [0xd7, 0x07, 1, 0, 0, 0, 1, 0]
        );
        // CZ_SHORTCUT_KEY_CHANGE2: tab.W index.W isSkill.B id.L count.W; tab 0 is the bar (clif_parse_Hotkey)
        assert_eq!(
            build_shortcut_key_change_packet(4, 1, 5, 10, pv),
            [0x21, 0x0b, 0, 0, 4, 0, 1, 5, 0, 0, 0, 10, 0]
        );
        // clif_parse_NpcBuyListSend: entries of amount.W itemId.L from offset 4
        let buy = build_purchase_item_list_packet(&[(3, 501), (1, 100_000)], pv);
        assert_eq!(buy.len(), 16);
        assert_eq!(u16::from_le_bytes([buy[2], buy[3]]), 16);
        assert_eq!(
            (u16::from_le_bytes([buy[4], buy[5]]), long(&buy, 6)),
            (3, 501)
        );
        assert_eq!(
            (u16::from_le_bytes([buy[10], buy[11]]), long(&buy, 12)),
            (1, 100_000)
        );
        // clif_parse_OpenVending: name at 4, flag.B at 84, entries of index.W amount.W price.L from 85
        // (vending_openvending)
        let vend = build_req_openstore2_packet("Shop", &[(2, 3, 1000), (5, 1, 70)], pv);
        assert_eq!(vend.len(), 85 + 2 * 8);
        assert_eq!(&vend[4..8], b"Shop");
        let entry = |at: usize| {
            let word = |at: usize| u16::from_le_bytes([vend[at], vend[at + 1]]);
            (word(at), word(at + 2), long(&vend, at + 4))
        };
        assert_eq!((entry(85), entry(93)), ((2, 3, 1000), (5, 1, 70)));
        // clif_parse_GuildChangePositionInfo: 40-byte entries from 4, position.L mode.L at +4, exp share.L at +12,
        // name at +16 (guild_change_position)
        let position = nostalre_game::guild::GuildPosition {
            id: 3,
            name: "Officer".into(),
            right: 0x11,
            ranking: 3,
            pay_rate: 10,
        };
        let guild = build_reg_change_guild_positioninfo(&[position], pv);
        assert_eq!(guild.len(), 4 + 40);
        assert_eq!(
            (long(&guild, 4), long(&guild, 8), long(&guild, 16)),
            (3, 0x11, 10)
        );
        assert_eq!(&guild[20..27], b"Officer");
        // clif_parse_ItemListWindowSelected: length.W at 2, kind.L at 4, OK.L at 8, entries of index.W amount.W from
        // 12 (skill_changematerial, skill_elementalanalysis)
        let list = build_itemlistwin_res_packet(1, true, &[(5, 45), (9, 35)], pv);
        assert_eq!(list.len(), 12 + 2 * 4);
        let word = |at: usize| u16::from_le_bytes([list[at], list[at + 1]]);
        assert_eq!(word(2), 20);
        assert_eq!((long(&list, 4), long(&list, 8)), (1, 1));
        assert_eq!((word(12), word(14), word(16), word(18)), (5, 45, 9, 35));
        let cancel = build_itemlistwin_res_packet(0, false, &[], pv);
        assert_eq!((cancel.len(), long(&cancel, 8)), (12, 0));
        // clif_parse_EquipItem: index.W position.L
        let equip = build_equip_item_packet(5, 0x8000, pv);
        assert_eq!(
            (u16::from_le_bytes([equip[2], equip[3]]), long(&equip, 4)),
            (5, 0x8000)
        );

        let mut session = Session::new(pv);
        session.store_login(2000000, 0x11223344, 2, 1);
        session.store_zone_info(150000, "prontera.gat".into());
        let enter = build_zone_enter_packet(&session);
        // clif_parse_WantToConnection offsets 2, 6, 10, 14, 22: account, character, login id, tick, sex
        assert_eq!(u32::from_le_bytes(enter[2..6].try_into().unwrap()), 2000000);
        assert_eq!(u32::from_le_bytes(enter[6..10].try_into().unwrap()), 150000);
        assert_eq!(
            u32::from_le_bytes(enter[10..14].try_into().unwrap()),
            0x11223344
        );
        assert_eq!(enter[22], 1);

        let talk = build_use_skill_to_ground_with_talkbox_packet(
            SkillEnum::MgFirebolt,
            1,
            150,
            151,
            "a message longer than twenty bytes",
            pv,
        );
        assert_eq!(&talk[10..30], b"a message longer tha");
        assert_eq!(talk[30], 0);

        let made = build_make_char_packet("Spike", 1, 3, 4, 4218, 0, pv);
        // CH_MAKE_CHAR: name 2, slot 26, hair colour 27, hair style 29, job 31, sex 35
        assert_eq!(&made[2..7], b"Spike");
        assert_eq!((made[26], made[27], made[29]), (1, 4, 3));
        assert_eq!(u16::from_le_bytes([made[31], made[32]]), 4218);
        assert_eq!(made[35], 0);
    }

    // rAthena @20250716: parseable_packet(0x0361, 5, clif_parse_ChangeDir, 2, 4)
    #[test]
    fn change_direction_wire_layout() {
        let raw = build_change_direction_packet(2, 5, PV);
        assert_eq!(raw.len(), 5);
        assert_eq!(u16::from_le_bytes([raw[0], raw[1]]), 0x0361);
        assert_eq!(i16::from_le_bytes([raw[2], raw[3]]), 2);
        assert_eq!(raw[4], 5);
    }

    // rAthena @20250716: parseable_packet(0x035f, 5, clif_parse_WalkToXY, 2)
    #[test]
    fn request_move_wire_layout() {
        let raw = build_request_move_packet(10, 20, PV);
        assert_eq!(raw.len(), 5);
        assert_eq!(u16::from_le_bytes([raw[0], raw[1]]), 0x035f);
    }

    // rAthena @20250716: parseable_packet(0x0439, 8, clif_parse_UseItem, 2, 4)
    #[test]
    fn use_item_wire_layout() {
        let raw = build_use_item_packet(10, 222, PV);
        assert_eq!(raw.len(), 8);
        assert_eq!(u16::from_le_bytes([raw[0], raw[1]]), 0x0439);
        assert_eq!(u16::from_le_bytes([raw[2], raw[3]]), 10);
        assert_eq!(u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]), 222);
    }

    #[test]
    fn restart_packet_types() {
        assert_eq!(*build_return_savepoint_packet(PV).last().unwrap(), 0);
        assert_eq!(*build_restart_packet(PV).last().unwrap(), 1);
    }

    #[test]
    fn standing_resurrection_builds() {
        assert!(!build_standing_resurrection_packet(PV).is_empty());
    }

    // A vendor's items are bought with FROMMC2 (0x0801), which carries the shop's unique id
    #[test]
    fn purchase_frommc_carries_the_shop_id() {
        let raw = build_purchase_frommc2_packet(111, 222, &[(2i16, 5i16)], PV);
        assert_eq!(u16::from_le_bytes([raw[0], raw[1]]), 0x0801);
        assert_eq!(raw.len(), 16); // header2+len2+aid4+uid4 + 1*4
    }

    #[test]
    fn adopt_reply_round_trips_through_parser() {
        let raw = build_adopt_reply_packet(111, 222, true, PV);
        assert_eq!(u16::from_le_bytes([raw[0], raw[1]]), 0x01f7);
        let parsed = packets::packets_parser::parse(&raw, PV);
        let pkt = parsed
            .as_any()
            .downcast_ref::<PacketCzJoinBaby>()
            .expect("expected PacketCzJoinBaby");
        assert_eq!(pkt.aid, 111);
        assert_eq!(pkt.gid, 222);
        assert_eq!(pkt.answer, 1);
    }

    /// The written message rides along with the placement, in the wide field the
    /// server reads it from.
    #[test]
    fn talkbox_ground_cast_carries_its_message() {
        let raw = build_use_skill_to_ground_with_talkbox_packet(
            SkillEnum::RgGraffiti,
            1,
            155,
            182,
            "hi there",
            PV,
        );
        assert_eq!(raw.len(), 31);
        assert_eq!(&raw[..2], &[0x67, 0x03]);

        let parsed = packets::packets_parser::parse(&raw, PV);
        let pkt = parsed
            .as_any()
            .downcast_ref::<PacketCzUseSkillTogroundWithtalkbox>()
            .expect("expected PacketCzUseSkillTogroundWithtalkbox");
        assert_eq!(pkt.skid, SkillEnum::RgGraffiti.id() as u16);
        assert_eq!(pkt.selected_level, 1);
        assert_eq!((pkt.x_pos, pkt.y_pos), (155, 182));
        let message: String = pkt.contents_21.iter().take_while(|c| **c != '\0').collect();
        assert_eq!(message, "hi there");
    }
}

pub fn build_zone_enter_packet(session: &Session) -> Vec<u8> {
    let mut pkt = PacketCzEnter2::new(session.packetver);
    pkt.set_aid(session.account_id);
    pkt.set_gid(session.char_id);
    pkt.set_auth_code(session.login_id1);
    pkt.set_client_time(0);
    pkt.set_sex(session.sex);
    pkt.fill_raw_with_packetver(Some(session.packetver));
    pkt.raw
}

pub fn build_restart_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzRestart::new(packetver);
    pkt.set_atype(1);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_return_savepoint_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzRestart::new(packetver);
    pkt.set_atype(0);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_progress_done_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzProgress::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_cancel_lockon_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzCancelLockon::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_standing_resurrection_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzStandingResurrection::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_disconnect_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqDisconnect::new(packetver);
    pkt.set_atype(0);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_request_time_packet(client_time: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzRequestTime::new(packetver);
    pkt.set_client_time(client_time);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_reqname_packet(entity_id: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqname::new(packetver);
    pkt.set_aid(entity_id);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_solve_char_name_packet(char_id: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqnameBygid::new(packetver);
    pkt.set_gid(char_id);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_char_ping_packet(account_id: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzPing::new(packetver);
    pkt.set_aid(account_id);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_contact_npc_packet(npc_id: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzContactnpc::new(packetver);
    pkt.set_naid(npc_id);
    pkt.set_atype(1);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_npc_next_packet(npc_id: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqNextScript::new(packetver);
    pkt.set_naid(npc_id);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_npc_close_packet(npc_id: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzCloseDialog::new(packetver);
    pkt.set_naid(npc_id);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_npc_menu_select_packet(npc_id: u32, choice: u8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzChooseMenu::new(packetver);
    pkt.set_naid(npc_id);
    pkt.set_num(choice);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_npc_input_number_packet(npc_id: u32, value: i32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzInputEditdlg::new(packetver);
    pkt.set_naid(npc_id);
    pkt.set_value(value);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_npc_input_string_packet(npc_id: u32, text: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzInputEditdlgstr::new(packetver);
    let msg = format!("{text}\0");
    pkt.set_packet_length((8 + msg.len()) as i16);
    pkt.set_naid(npc_id);
    pkt.set_msg(msg);
    pkt.fill_raw_with_packetver(Some(packetver));
    #[cfg(target_os = "horizon")]
    crate::offline_text::replace_payload(&mut pkt.raw, 8, text);
    pkt.raw
}

pub fn build_npc_deal_type_packet(npc_id: u32, deal_type: u8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzAckSelectDealtype::new(packetver);
    pkt.set_naid(npc_id);
    pkt.set_atype(deal_type);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_enter_room_packet(room_id: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqEnterRoom::new(packetver);
    pkt.set_room_id(room_id);
    pkt.set_passwd(['\0'; 8]);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

fn passwd_chars(password: &str) -> [char; 8] {
    let mut buf = ['\0'; 8];
    for (i, c) in password.chars().take(8).enumerate() {
        buf[i] = c;
    }
    buf
}

pub fn build_create_chatroom_packet(
    title: &str,
    limit: i16,
    public: bool,
    password: &str,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzCreateChatroom::new(packetver);
    pkt.set_packet_length((15 + title.len()) as i16);
    pkt.set_size(limit);
    pkt.set_atype(public as u8);
    pkt.set_passwd(passwd_chars(password));
    pkt.set_title(title.to_string());
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_change_chatroom_packet(
    title: &str,
    limit: i16,
    public: bool,
    password: &str,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzChangeChatroom::new(packetver);
    pkt.set_packet_length((15 + title.len()) as i16);
    pkt.set_size(limit);
    pkt.set_atype(public as u8);
    pkt.set_passwd(passwd_chars(password));
    pkt.set_title(title.to_string());
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

fn name_chars(name: &str) -> [char; 24] {
    let mut buf = ['\0'; 24];
    for (i, c) in name.chars().take(24).enumerate() {
        buf[i] = c;
    }
    buf
}

pub fn build_change_chat_owner_packet(name: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqRoleChange::new(packetver);
    pkt.set_role(0);
    pkt.set_name(name_chars(name));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_expel_chat_member_packet(name: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqExpelMember::new(packetver);
    pkt.set_name(name_chars(name));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_exit_room_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzExitRoom::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_remember_warppoint_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzRememberWarppoint::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_lesseffect_packet(is_less: bool, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzLesseffect::new(packetver);
    pkt.set_is_less(is_less as i32);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_user_count_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqUserCount::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_give_manner_point_packet(
    target_aid: u32,
    positive: bool,
    point: i16,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzReqGiveMannerPoint::new(packetver);
    pkt.set_other_aid(target_aid);
    pkt.set_atype(if positive {
        nostalre_game::gm::MANNER_TYPE_PLUS
    } else {
        nostalre_game::gm::MANNER_TYPE_MINUS
    });
    pkt.set_point(point.clamp(0, 30000));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_give_manner_byname_packet(name: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqGiveMannerByname::new(packetver);
    pkt.set_char_name(name_to_char24(name));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_status_gm_packet(name: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqStatusGm::new(packetver);
    pkt.set_char_name(name_to_char24(name));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_account_name_packet(aid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqAccountname::new(packetver);
    pkt.set_aid(aid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// CZ_REQ_RANKING 0x097c: rAthena parses only this one from 20120503 (the 0x0217 family drops the client).
pub fn build_ranking_request_packet(kind: FameKind, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqRanking::new(packetver);
    pkt.set_ranking_type(kind.rank_type());
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_guild_chat_packet(msg: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzGuildChat::new(packetver);
    let msg_null = format!("{msg}\0");
    pkt.set_packet_length((4 + msg_null.len()) as i16);
    pkt.set_msg(msg_null);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_whisper_packet(receiver: &str, msg: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzWhisper::new(packetver);
    pkt.set_receiver(name_to_char24(receiver));
    let msg_null = format!("{msg}\0");
    pkt.set_packet_length((28 + msg_null.len()) as i16);
    pkt.set_msg(msg_null);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// `block` = add the player to the ignore list (`/ex`); `false` removes them
/// (`/in`). The wire `type` byte is 0 for add, 1 for remove.
pub fn build_setting_whisper_pc_packet(name: &str, block: bool, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzSettingWhisperPc::new(packetver);
    pkt.set_name(name_to_char24(name));
    pkt.set_atype(if block { 0 } else { 1 });
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// `block_all` = ignore every incoming whisper (`/exall`); `false` accepts all
/// (`/inall`).
pub fn build_setting_whisper_state_packet(block_all: bool, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzSettingWhisperState::new(packetver);
    pkt.set_atype(if block_all { 0 } else { 1 });
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_purchase_item_list_packet(items: &[(i16, u32)], packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzPcPurchaseItemlist::new(packetver);
    let item_list: Vec<CzPurchaseItem> = items
        .iter()
        .map(|(count, item_id)| {
            let mut item = CzPurchaseItem::new(packetver);
            item.set_count(*count);
            item.set_itid_32(*item_id);
            item.fill_raw_with_packetver(Some(packetver));
            item
        })
        .collect();
    let entry = CzPurchaseItem::base_len(packetver);
    pkt.set_packet_length((4 + items.len() * entry) as i16);
    pkt.set_item_list(item_list);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Buys `(count, item id)` from the open market shop
pub fn build_market_purchase_packet(items: &[(i16, u32)], packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzNpcMarketPurchase::new(packetver);
    let list: Vec<PacketCzNpcMarketPurchaseSub> = items
        .iter()
        .map(|(count, item_id)| {
            let mut item = PacketCzNpcMarketPurchaseSub::new(packetver);
            item.set_item_id(*item_id);
            item.set_qty(i32::from(*count));
            item.fill_raw_with_packetver(Some(packetver));
            item
        })
        .collect();
    let entry = PacketCzNpcMarketPurchaseSub::base_len(packetver);
    pkt.set_packet_length((4 + items.len() * entry) as i16);
    pkt.set_list(list);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Changes the looks picked at the stylist (0x0afc)
pub fn build_stylist_buy_packet(choice: &StyleChoice, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqStyleChange2::new(packetver);
    let [
        head_palette,
        head_style,
        body_palette,
        top,
        mid,
        bottom,
        body_style,
    ] = choice.0;
    pkt.set_head_palette(head_palette);
    pkt.set_head_style(head_style);
    pkt.set_body_palette(body_palette);
    pkt.set_top_accessory(top);
    pkt.set_mid_accessory(mid);
    pkt.set_bottom_accessory(bottom);
    pkt.set_body_style(body_style);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_stylist_close_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqStyleClose::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Trades for items at the open barter shop: simple (0x0b0f) or extended (0x0b57)
pub fn build_barter_purchase_packet(
    extended: bool,
    items: &[BarterPurchase],
    packetver: u32,
) -> Vec<u8> {
    if extended {
        let mut pkt = PacketCzNpcExpandedBarterMarketPurchase::new(packetver);
        let list: Vec<PacketCzNpcExpandedBarterMarketPurchaseSub> = items
            .iter()
            .map(|p| {
                let mut line = PacketCzNpcExpandedBarterMarketPurchaseSub::new(packetver);
                line.set_item_id(p.item_id);
                line.set_shop_index(p.shop_index as i32);
                line.set_amount(p.amount as i32);
                line.fill_raw_with_packetver(Some(packetver));
                line
            })
            .collect();
        let entry = PacketCzNpcExpandedBarterMarketPurchaseSub::base_len(packetver);
        pkt.set_packet_length((4 + items.len() * entry) as i16);
        pkt.set_list(list);
        pkt.fill_raw_with_packetver(Some(packetver));
        return pkt.raw;
    }
    let mut pkt = PacketCzNpcBarterMarketPurchase::new(packetver);
    let list: Vec<PacketCzNpcBarterMarketPurchaseSub> = items
        .iter()
        .map(|p| {
            let mut line = PacketCzNpcBarterMarketPurchaseSub::new(packetver);
            line.set_item_id(p.item_id);
            line.set_amount(p.amount as i32);
            line.set_inv_index(p.inv_index);
            line.set_shop_index(p.shop_index as i32);
            line.fill_raw_with_packetver(Some(packetver));
            line
        })
        .collect();
    let entry = PacketCzNpcBarterMarketPurchaseSub::base_len(packetver);
    pkt.set_packet_length((4 + items.len() * entry) as i16);
    pkt.set_list(list);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Closes the barter shop: simple (0x0b12) or extended (0x0b58); until then the server refuses NPC talk
pub fn build_barter_close_packet(extended: bool, packetver: u32) -> Vec<u8> {
    if extended {
        let mut pkt = PacketCzNpcExpandedBarterMarketClose::new(packetver);
        pkt.fill_raw_with_packetver(Some(packetver));
        return pkt.raw;
    }
    let mut pkt = PacketCzNpcBarterMarketClose::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Closes the market shop: until then the server keeps the player trading (no item use, no drops)
pub fn build_market_close_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzNpcMarketClose::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Closes a buy or sell window (0x09d4, from 20131223): the server forgets the shop. Until then it ignores the item
/// list window and the equip switch; the older way, an empty list, is answered as a failed deal
pub fn build_npc_trade_quit_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzNpcTradeQuit::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_sell_item_list_packet(items: &[(i16, i16)], packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzPcSellItemlist::new(packetver);
    let item_list: Vec<CzSellItem> = items
        .iter()
        .map(|(index, count)| {
            let mut item = CzSellItem::new(packetver);
            item.set_index(*index);
            item.set_count(*count);
            item.fill_raw_with_packetver(Some(packetver));
            item
        })
        .collect();
    pkt.set_packet_length((4 + items.len() * 4) as i16);
    pkt.set_item_list(item_list);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_use_item_packet(index: u16, account_id: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzUseItem::new(packetver);
    pkt.set_index(index);
    pkt.set_aid(account_id);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_equip_item_packet(index: u16, location: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqWearEquip::new(packetver);
    pkt.set_index(index);
    pkt.set_wear_location_32(location);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_unequip_item_packet(index: u16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqTakeoffEquip::new(packetver);
    pkt.set_index(index);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Sent after each map property (0x099b) as kRO's exe does (its handler 0x7af240 ends with it); the server answers
/// with its load confirm and, on a map with the `specialpopup` flag, the popup (`clif_parse_blocking_playcancel`)
pub fn build_blocking_play_cancel_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzBlockingPlayCancel::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Takes off everything worn in `location` (an equip-position mask) at once (0x0bf5)
pub fn build_unequip_all_packet(location: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqTakeoffEquipAll::new(packetver);
    pkt.set_location(location);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Put an inventory item into the swap equipment set (0x0a97, `clif_parse_equipswitch_add`)
pub fn build_equip_switch_add_packet(index: u16, location: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqEquipswitchAdd::new(packetver);
    pkt.set_index(index);
    pkt.set_position(location);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Take an item out of the swap equipment set (0x0a99, `clif_parse_equipswitch_remove`)
pub fn build_equip_switch_remove_packet(index: u16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqEquipswitchRemove::new(packetver);
    pkt.set_index(index);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// CHANGE: wear the swap equipment set (0x0a9c, `clif_parse_equipswitch_request`)
pub fn build_equip_switch_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqEquipswitch::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Swap one item (0x0ace, `clif_parse_equipswitch_request_single`)
pub fn build_equip_switch_single_packet(index: u16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqEquipswitchSingle::new(packetver);
    pkt.set_index(index);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Wear one of the character's titles, 0 to wear none (0x0a2e, `clif_parse_change_title`)
pub fn build_change_title_packet(title_id: i32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqChangeTitle::new(packetver);
    pkt.set_title_id(title_id);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_drop_item_packet(index: u16, count: i16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzItemThrow::new(packetver);
    pkt.set_index(index);
    pkt.set_count(count);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_pickup_item_packet(itaid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzItemPickup::new(packetver);
    pkt.set_itaid(itaid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_move_item_body_to_cart_packet(index: u16, count: i16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzMoveItemFromBodyToCart::new(packetver);
    pkt.set_index(index as i16);
    pkt.set_count(count as i32);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_move_item_cart_to_body_packet(index: u16, count: i16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzMoveItemFromCartToBody::new(packetver);
    pkt.set_index(index as i16);
    pkt.set_count(count as i32);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_move_item_store_to_cart_packet(index: u16, count: i16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzMoveItemFromStoreToCart::new(packetver);
    pkt.set_index(index as i16);
    pkt.set_count(count as i32);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_move_item_cart_to_store_packet(index: u16, count: i16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzMoveItemFromCartToStore::new(packetver);
    pkt.set_index(index as i16);
    pkt.set_count(count as i32);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_move_item_body_to_store_packet(index: u16, count: i16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzMoveItemFromBodyToStore::new(packetver);
    pkt.set_index(index as i16);
    pkt.set_count(count as i32);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_move_item_store_to_body_packet(index: u16, count: i16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzMoveItemFromStoreToBody::new(packetver);
    pkt.set_index(index as i16);
    pkt.set_count(count as i32);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_close_store_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzCloseStore::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// `CZ_ACK_STORE_PASSWORD.Type`.
const STORE_PASSWORD_CHANGE: i16 = 2;
const STORE_PASSWORD_CHECK: i16 = 3;

pub fn build_ack_store_password_packet(
    change: bool,
    password: &str,
    new_password: &str,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzAckStorePassword::new(packetver);
    pkt.set_atype(if change {
        STORE_PASSWORD_CHANGE
    } else {
        STORE_PASSWORD_CHECK
    });
    pkt.set_password(to_char_array::<16>(password));
    pkt.set_new_password(to_char_array::<16>(new_password));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_exchange_item_packet(target_aid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqExchangeItem::new(packetver);
    pkt.set_aid(target_aid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_ack_exchange_item_packet(result: u8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzAckExchangeItem::new(packetver);
    pkt.set_result(result);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_add_exchange_item_packet(index: u16, count: i32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzAddExchangeItem::new(packetver);
    pkt.set_index(index as i16);
    pkt.set_count(count);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_conclude_exchange_item_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzConcludeExchangeItem::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_cancel_exchange_item_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzCancelExchangeItem::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_exec_exchange_item_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzExecExchangeItem::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_mail_get_list_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzMailGetList::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_mail_open_packet(mail_id: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzMailOpen::new(packetver);
    pkt.set_mail_id(mail_id as i32);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_mail_delete_packet(mail_id: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzMailDelete::new(packetver);
    pkt.set_mail_id(mail_id as i32);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_mail_get_item_packet(mail_id: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzMailGetItem::new(packetver);
    pkt.set_mail_id(mail_id as i32);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_mail_reset_item_packet(ty: u8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzMailResetItem::new(packetver);
    pkt.set_atype(ty as i16);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_mail_add_item_packet(index: u16, amount: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzMailAddItem::new(packetver);
    pkt.set_index(index as i16);
    pkt.set_count(amount as i32);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_mail_return_packet(mail_id: u32, receiver: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqMailReturn::new(packetver);
    pkt.set_mail_id(mail_id as i32);
    pkt.set_receive_name(name_chars(receiver));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// 0x248 <len>.W <recipient>.24B <title>.40B <body len>.B <body>.?B
///
/// Hand-encoded: the generated `PacketCzMailSend` types `msg_len` as a u32, but
/// the wire field is a single byte.
pub fn build_mail_send_packet(to: &str, title: &str, body: &str, _packetver: u32) -> Vec<u8> {
    let mut cut = body.len().min(nostalre_game::mail::MAIL_BODY_MAX);
    while cut > 0 && !body.is_char_boundary(cut) {
        cut -= 1;
    }
    let body_bytes = &body.as_bytes()[..cut];

    let pkt_len = 69 + body_bytes.len();
    let mut buf = Vec::with_capacity(pkt_len);
    buf.extend_from_slice(&0x248i16.to_le_bytes());
    buf.extend_from_slice(&(pkt_len as i16).to_le_bytes());

    let mut recv = [0u8; 24];
    for (i, b) in to.as_bytes().iter().take(23).enumerate() {
        recv[i] = *b;
    }
    buf.extend_from_slice(&recv);

    let mut header = [0u8; 40];
    for (i, b) in title.as_bytes().iter().take(39).enumerate() {
        header[i] = *b;
    }
    buf.extend_from_slice(&header);

    buf.push(body_bytes.len() as u8);
    buf.extend_from_slice(body_bytes);
    buf
}

pub fn build_cartoff_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqCartoff::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_change_cart_packet(num: i16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqChangecart::new(packetver);
    pkt.set_num(num);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// 0x0980: the cart picked from Cart Decoration's list, under the account id 0x097f gave (`clif_parse_SelectCart`
/// takes 10 to 12 only)
pub fn build_select_cart_packet(identity: u32, cart: u8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzSelectcart::new(packetver);
    pkt.set_identity(identity);
    pkt.set_atype(cart);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// 0x01ed: a Super Novice's prayer said (`clif_parse_NoviceExplosionSpirits`), no body
pub fn build_novice_prayer_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzChopokgi::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_upgrade_skill_packet(skill: SkillEnum, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzUpgradeSkilllevel::new(packetver);
    pkt.set_skid(skill.id() as u16);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_stat_change_packet(status_id: u16, amount: u8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzStatusChange::new(packetver);
    pkt.set_status_id(status_id);
    pkt.set_change_amount(amount);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Raises a trait stat (POW to CRT, status ids 219 to 224) by `amount`: 0x0b24, from 20200916
pub fn build_trait_stat_change_packet(status_id: u16, amount: u16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzAdvancedStatusChange::new(packetver);
    pkt.set_status_type(status_id as i16);
    pkt.set_amount(amount as i16);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_shortcut_key_change_packet(
    index: u16,
    is_skill: i8,
    id: u32,
    count: i16,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzShortcutKeyChange::new(packetver);
    pkt.set_index(index);
    let mut key = ShortCutKey::new(packetver);
    key.set_is_skill(is_skill);
    key.set_id(id);
    key.set_count(count);
    key.fill_raw_with_packetver(Some(packetver));
    pkt.set_short_cut_key(key);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_card_composition_list_packet(card_index: u16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqItemcompositionList::new(packetver);
    pkt.set_card_index(card_index as i16);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_card_composition_packet(card_index: u16, equip_index: u16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqItemcomposition::new(packetver);
    pkt.set_card_index(card_index as i16);
    pkt.set_equip_index(equip_index as i16);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_use_skill_packet(
    skill: SkillEnum,
    level: i16,
    target_id: u32,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzUseSkill::new(packetver);
    pkt.set_selected_level(level);
    pkt.set_skid(skill.id() as u16);
    pkt.set_target_id(target_id);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_use_skill_to_ground_packet(
    skill: SkillEnum,
    level: i16,
    x: i16,
    y: i16,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzUseSkillToground::new(packetver);
    pkt.set_selected_level(level);
    pkt.set_skid(skill.id() as u16);
    pkt.set_x_pos(x);
    pkt.set_y_pos(y);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_use_skill_to_ground_with_talkbox_packet(
    skill: SkillEnum,
    level: i16,
    x: i16,
    y: i16,
    message: &str,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzUseSkillTogroundWithtalkbox::new(packetver);
    pkt.set_selected_level(level);
    pkt.set_skid(skill.id() as u16);
    pkt.set_x_pos(x);
    pkt.set_y_pos(y);
    // the message field is 21 bytes, ending in a NUL
    let mut bytes = [0u8; 21];
    let src = message.as_bytes();
    let n = src.len().min(20);
    bytes[..n].copy_from_slice(&src[..n]);
    pkt.set_contents_21(std::array::from_fn(|i| bytes[i] as char));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_select_warppoint_packet(skill: SkillEnum, map_name: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzSelectWarppoint::new(packetver);
    pkt.set_skid(skill.id() as u16);
    let mut bytes = [0u8; 16];
    let src = map_name.as_bytes();
    let n = src.len().min(15);
    bytes[..n].copy_from_slice(&src[..n]);
    pkt.set_map_name(std::array::from_fn(|i| bytes[i] as char));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_remove_option_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqCartoff::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_doridori_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzDoridori::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

fn name_to_char24(name: &str) -> [char; 24] {
    let mut buf = [0 as char; 24];
    for (i, c) in name.chars().take(23).enumerate() {
        buf[i] = c;
    }
    buf
}

/// Birthdate as the 6 raw digits (YYMMDD) the server compares against; non-digits
/// are dropped and the century is trimmed, so "2001-05-14", "20010514" and
/// "010514" all send "010514".
fn birthdate_to_char6(birthdate: &str) -> [char; 6] {
    let digits: Vec<char> = birthdate.chars().filter(|c| c.is_ascii_digit()).collect();
    let start = digits.len().saturating_sub(6);
    let mut buf = [0 as char; 6];
    for (i, c) in digits[start..].iter().enumerate() {
        buf[i] = *c;
    }
    buf
}

pub fn build_make_party_packet(name: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzMakeGroup::new(packetver);
    pkt.set_group_name(name_to_char24(name));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_join_party_packet(target_aid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqJoinGroup::new(packetver);
    pkt.set_aid(target_aid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// CZ_PARTY_JOIN_REQ_ACK, the answer to ZC_PARTY_JOIN_REQ 0x02c6 (the old 0x00ff still parses).
pub fn build_join_party_reply_packet(party_grid: u32, accept: bool, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzPartyJoinReqAck::new(packetver);
    pkt.set_grid(party_grid);
    pkt.set_b_accept(accept);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_adopt_request_packet(target_aid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqJoinBaby::new(packetver);
    pkt.set_aid(target_aid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_adopt_reply_packet(
    father_aid: u32,
    mother_aid: u32,
    accept: bool,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzJoinBaby::new(packetver);
    pkt.set_aid(father_aid);
    pkt.set_gid(mother_aid);
    pkt.set_answer(if accept { 1 } else { 0 });
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_marry_request_packet(target_aid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqJoinCouple::new(packetver);
    pkt.set_aid(target_aid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_marry_reply_packet(
    proposer_aid: u32,
    proposer_gid: u32,
    accept: bool,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzJoinCouple::new(packetver);
    pkt.set_aid(proposer_aid);
    pkt.set_gid(proposer_gid);
    pkt.set_answer(if accept { 1 } else { 0 });
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_leave_party_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqLeaveGroup::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_expel_party_member_packet(aid: u32, name: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqExpelGroupMember::new(packetver);
    pkt.set_aid(aid);
    pkt.set_character_name(name_to_char24(name));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// CZ_GROUPINFO_CHANGE_V2: the server sets all three at once (`party_changeoption`), so a change of one passes
/// the other two as they are.
pub fn build_change_party_option_packet(
    exp_option: u32,
    item_pickup_rule: u8,
    item_division_rule: u8,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzGroupinfoChangeV2::new(packetver);
    pkt.set_exp_option(exp_option);
    pkt.set_item_pickup_rule(item_pickup_rule);
    pkt.set_item_division_rule(item_division_rule);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_make_party2_packet(
    name: &str,
    item_pickup_rule: u8,
    item_division_rule: u8,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzMakeGroup2::new(packetver);
    pkt.set_group_name(name_to_char24(name));
    pkt.set_item_pickup_rule(item_pickup_rule);
    pkt.set_item_division_rule(item_division_rule);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_party_invite_by_name_packet(name: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzPartyJoinReq::new(packetver);
    pkt.set_character_name(name_to_char24(name));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_change_party_leader_packet(aid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzChangeGroupMaster::new(packetver);
    pkt.set_aid(aid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_add_friend_packet(name: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzAddFriends::new(packetver);
    pkt.set_name(name_to_char24(name));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_ack_add_friend_packet(
    req_aid: u32,
    req_gid: u32,
    accept: bool,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzAckReqAddFriends::new(packetver);
    pkt.set_req_aid(req_aid);
    pkt.set_req_gid(req_gid);
    pkt.set_result(if accept { 1 } else { 0 });
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_agree_star_place_packet(which: i8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzAgreeStarplace::new(packetver);
    pkt.set_which(which);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_delete_friend_packet(aid: u32, gid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzDeleteFriends::new(packetver);
    pkt.set_aid(aid);
    pkt.set_gid(gid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_party_chat_packet(msg: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzRequestChatParty::new(packetver);
    let msg_null = format!("{msg}\0");
    pkt.set_packet_length((4 + msg_null.len()) as i16);
    pkt.set_msg(msg_null);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

// --- Skill-triggered production / selection windows ---

pub fn build_req_itemidentify_packet(index: i16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqItemidentify::new(packetver);
    pkt.set_index(index);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_makingarrow_packet(item_id: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqMakingarrow::new(packetver);
    pkt.set_id_32(item_id);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_makingitem_packet(item_id: u32, materials: [u32; 3], packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqmakingitem::new(packetver);
    let mut info = MakableitemInfo::new(packetver);
    info.set_itid_32(item_id);
    info.set_material_id_32(materials);
    info.fill_raw_with_packetver(Some(packetver));
    pkt.set_info(info);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// 0x025b: the item picked from a 0x025a list of kind `mk_type`, item 0 to close the list
pub fn build_req_cooking_packet(mk_type: u16, item_id: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqMakingitem::new(packetver);
    pkt.set_mk_type(mk_type as i16);
    pkt.set_id_32(item_id);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_weaponrefine_packet(index: i32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqWeaponrefine::new(packetver);
    pkt.set_index(index);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Puts the inventory item at `index` into the refine window; the server answers with its materials
pub fn build_refining_select_item_packet(index: u16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzRefiningSelectItem::new(packetver);
    pkt.set_index(index);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Refines the item at `index` with `material`, Blacksmith Blessings guarding it when `blessing`
pub fn build_req_refining_packet(
    index: u16,
    material: u32,
    blessing: bool,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzReqRefining::new(packetver);
    pkt.set_index(index);
    pkt.set_item_id(material);
    pkt.set_blacksmith_blessing(blessing as u8);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_close_refining_ui_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzCloseRefiningUi::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_itemrepair_packet(
    index: i16,
    item_id: u32,
    refine: u8,
    cards: [u32; 4],
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzReqItemrepair2::new(packetver);
    let mut info = RepairitemInfo2::new(packetver);
    info.set_index(index);
    info.set_itid(item_id);
    info.set_refining_level(refine);
    let mut slot = EQUIPSLOTINFO::new(packetver);
    slot.set_card1_32(cards[0]);
    slot.set_card2_32(cards[1]);
    slot.set_card3_32(cards[2]);
    slot.set_card4_32(cards[3]);
    slot.fill_raw_with_packetver(Some(packetver));
    info.set_slot(slot);
    info.fill_raw_with_packetver(Some(packetver));
    pkt.set_target_item_info(info);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// `None` is the list's cancel, which the server reads as skill id 0.
/// 0x0443: the skill picked from a 0x0442 menu, 0 to close it (rAthena then ends the menu skill)
pub fn build_skill_select_response_packet(
    why: i32,
    skill: Option<SkillEnum>,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzSkillSelectResponse::new(packetver);
    pkt.set_why(why);
    pkt.set_skid(skill.map_or(0, |s| s.id() as u16));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// 0x07e4: the item list window's answer (`clif_parse_ItemListWindowSelected`): its kind, 1 for OK or 0 for Cancel,
/// then each chosen item's inventory index and amount. rAthena reads a Cancel or an empty list as the end of the menu
pub fn build_itemlistwin_res_packet(
    kind: i32,
    confirmed: bool,
    items: &[(u16, u16)],
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzItemlistwinRes::new(packetver);
    let list: Vec<PacketCzItemlistwinResSub> = items
        .iter()
        .map(|&(index, count)| {
            let mut line = PacketCzItemlistwinResSub::new(packetver);
            line.set_index(index);
            line.set_count(count);
            line.fill_raw_with_packetver(Some(packetver));
            line
        })
        .collect();
    let entry = PacketCzItemlistwinResSub::base_len(packetver);
    pkt.set_packet_length((12 + items.len() * entry) as i16);
    pkt.set_atype(kind);
    pkt.set_action(i32::from(confirmed));
    pkt.set_material_list(list);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_select_autospell_packet(skill: Option<SkillEnum>, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzSelectautospell::new(packetver);
    pkt.set_skid(skill.map_or(0, |s| s.id() as i32));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

// --- Vending ---

pub fn build_req_openstore2_packet(
    shop_name: &str,
    items: &[(i16, i16, i32)],
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzReqOpenstore2::new(packetver);
    let mut name = ['\0'; 80];
    for (i, c) in shop_name.chars().take(79).enumerate() {
        name[i] = c;
    }
    pkt.set_store_name(name);
    pkt.set_result(true);
    let store_list: Vec<StoreItem> = items
        .iter()
        .map(|(index, count, price)| {
            let mut s = StoreItem::new(packetver);
            s.set_index(*index);
            s.set_count(*count);
            s.set_price(*price);
            s.fill_raw_with_packetver(Some(packetver));
            s
        })
        .collect();
    // header(2) + len(2) + name(80) + result(1) + N*8
    pkt.set_packet_length((85 + items.len() * 8) as i16);
    pkt.set_store_list(store_list);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_cancel_openstore_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqOpenstore2::new(packetver);
    pkt.set_store_name(['\0'; 80]);
    pkt.set_result(false);
    pkt.set_packet_length(85);
    pkt.set_store_list(Vec::new());
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_closestore_packet(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqClosestore::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_buy_frommc_packet(aid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqBuyFrommc::new(packetver);
    pkt.set_aid(aid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

/// Buy from a vending merchant: `unique_id` is the shop's, from its item list.
pub fn build_purchase_frommc2_packet(
    aid: u32,
    unique_id: u32,
    items: &[(i16, i16)],
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzPcPurchaseItemlistFrommc2::new(packetver);
    pkt.set_aid(aid);
    pkt.set_unique_id(unique_id);
    let item_list: Vec<CzPurchaseItemFrommc> = items
        .iter()
        .map(|(count, index)| {
            let mut item = CzPurchaseItemFrommc::new(packetver);
            item.set_count(*count);
            item.set_index(*index);
            item.fill_raw_with_packetver(Some(packetver));
            item
        })
        .collect();
    // header(2) + len(2) + aid(4) + uniqueId(4) + N*4
    pkt.set_packet_length((12 + items.len() * 4) as i16);
    pkt.set_item_list(item_list);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

// --- Homunculus / Mercenary ---

pub fn build_companion_move_packet(gid: u32, x: u16, y: u16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzRequestMovenpc::new(packetver);
    pkt.set_gid(gid);
    pkt.set_dest(crate::helpers::encode_pos(x, y, 0));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_companion_attack_packet(gid: u32, target_gid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzRequestActnpc::new(packetver);
    pkt.set_gid(gid);
    pkt.set_target_gid(target_gid);
    pkt.set_action(0);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_companion_move_to_owner_packet(gid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzRequestMovetoowner::new(packetver);
    pkt.set_gid(gid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_config_packet(config: i32, value: i32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzConfig::new(packetver);
    pkt.set_config(config);
    pkt.set_value(value);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_homun_menu_packet(command: i8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzCommandMer::new(packetver);
    pkt.set_atype(0);
    pkt.set_command(command);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_mercenary_command_packet(command: i8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzMerCommand::new(packetver);
    pkt.set_command(command);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_rename_homun_packet(name: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzRenameMer::new(packetver);
    pkt.set_name(name_to_char24(name));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_trycapture_packet(gid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzTrycaptureMonster::new(packetver);
    pkt.set_target_aid(gid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_command_pet_packet(csub: i8, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzCommandPet::new(packetver);
    pkt.set_c_sub(csub);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_rename_pet_packet(name: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzRenamePet::new(packetver);
    pkt.set_sz_name(name_to_char24(name));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_select_petegg_packet(index: u16, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzSelectPetegg::new(packetver);
    pkt.set_index(index as i16);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_pet_act_packet(data: i32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzPetAct::new(packetver);
    pkt.set_data(data);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_active_quest_packet(quest_id: u32, active: bool, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzActiveQuest::new(packetver);
    pkt.set_quest_id(quest_id);
    pkt.set_active(active);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_guild_menuinterface(packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqGuildMenuinterface::new(packetver);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_guild_menu(atype: i32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqGuildMenu::new(packetver);
    pkt.set_atype(atype);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

fn to_char_array<const N: usize>(s: &str) -> [char; N] {
    let mut buf = [0 as char; N];
    for (i, c) in s.chars().take(N - 1).enumerate() {
        buf[i] = c;
    }
    buf
}

pub fn build_guild_notice(gdid: u32, subject: &str, notice: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzGuildNotice::new(packetver);
    pkt.set_gdid(gdid);
    pkt.set_subject(to_char_array::<60>(subject));
    pkt.set_notice(to_char_array::<120>(notice));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_leave_guild(
    gdid: u32,
    aid: i32,
    gid: i32,
    reason: &str,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzReqLeaveGuild::new(packetver);
    pkt.set_gdid(gdid);
    pkt.set_aid(aid);
    pkt.set_gid(gid);
    pkt.set_reason_desc(to_char_array::<40>(reason));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_ban_guild(gdid: u32, aid: i32, gid: i32, reason: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqBanGuild::new(packetver);
    pkt.set_gdid(gdid);
    pkt.set_aid(aid);
    pkt.set_gid(gid);
    pkt.set_reason_desc(to_char_array::<40>(reason));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_change_memberpos(aid: i32, gid: i32, position_id: i32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqChangeMemberpos::new(packetver);
    let mut row = MemberPositionInfo::new(packetver);
    row.set_aid(aid);
    row.set_gid(gid);
    row.set_position_id(position_id);
    row.fill_raw_with_packetver(Some(packetver));
    pkt.set_packet_length((4 + 12) as i16);
    pkt.set_member_info(vec![row]);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_reg_change_guild_positioninfo(
    rows: &[nostalre_game::guild::GuildPosition],
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzRegChangeGuildPositioninfo::new(packetver);
    let list: Vec<GuildRegPositionInfo> = rows
        .iter()
        .map(|p| {
            let mut r = GuildRegPositionInfo::new(packetver);
            r.set_position_id(p.id);
            r.set_right(p.right);
            r.set_ranking(p.ranking);
            r.set_pay_rate(p.pay_rate);
            r.set_pos_name(to_char_array::<24>(&p.name));
            r.fill_raw_with_packetver(Some(packetver));
            r
        })
        .collect();
    pkt.set_packet_length((4 + rows.len() * 40) as i16);
    pkt.set_member_list(list);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_make_guild(gid: u32, name: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqMakeGuild::new(packetver);
    pkt.set_gid(gid);
    pkt.set_gname(name_to_char24(name));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_disorganize_guild(key: &str, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqDisorganizeGuild::new(packetver);
    pkt.set_key(to_char_array::<40>(key));
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_guild_emblem_img(gdid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqGuildEmblemImg::new(packetver);
    pkt.set_gdid(gdid as i32);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_register_guild_emblem(bmp: Vec<u8>, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzRegisterGuildEmblemImg::new(packetver);
    pkt.set_packet_length((4 + bmp.len()) as i16);
    // fill_raw rebuilds img_raw from the (empty) img string, so append the body after.
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw.extend_from_slice(&bmp);
    pkt.raw
}

/// CZ_REQ_ADD_NEW_EMBLEM: the emblem `version` the web server stored (`emblem_http::upload`) is the guild's now.
pub fn build_add_new_emblem_packet(gdid: u32, version: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqAddNewEmblem::new(packetver);
    pkt.set_guild_id(gdid);
    pkt.set_version(version);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_join_guild(target_aid: u32, my_aid: u32, my_gid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqJoinGuild::new(packetver);
    pkt.set_aid(target_aid);
    pkt.set_my_aid(my_aid);
    pkt.set_my_gid(my_gid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_ans_join_guild(gdid: u32, accept: bool, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzJoinGuild::new(packetver);
    pkt.set_gdid(gdid);
    pkt.set_answer(if accept { 1 } else { 0 });
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_ally_guild(target_aid: u32, my_aid: u32, my_gid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqAllyGuild::new(packetver);
    pkt.set_aid(target_aid);
    pkt.set_my_aid(my_aid);
    pkt.set_my_gid(my_gid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_ally_guild(other_aid: u32, accept: bool, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzAllyGuild::new(packetver);
    pkt.set_other_aid(other_aid);
    pkt.set_answer(if accept { 1 } else { 0 });
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_hostile_guild(target_aid: u32, packetver: u32) -> Vec<u8> {
    let mut pkt = PacketCzReqHostileGuild::new(packetver);
    pkt.set_aid(target_aid);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}

pub fn build_req_delete_related_guild(
    opponent_gdid: u32,
    relation: i32,
    packetver: u32,
) -> Vec<u8> {
    let mut pkt = PacketCzReqDeleteRelatedGuild::new(packetver);
    pkt.set_opponent_gdid(opponent_gdid);
    pkt.set_relation(relation);
    pkt.fill_raw_with_packetver(Some(packetver));
    pkt.raw
}
