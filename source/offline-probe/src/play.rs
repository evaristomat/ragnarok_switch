//! The probe's live-pass steps (step 3.6b item 6): one game service each, played against the server and checked by
//! its answer. Every packet on the way is framed, parsed and handled, or the probe is not clean.

use super::*;
use models::enums::skill_enums::SkillEnum;
use models::enums::vanish::VanishType;
use nostalre_game::sprite_path::is_hidden;

/// Seconds the server takes to answer a request
const REPLY: u64 = 3;
/// Seconds for an answer that waits on a skill or a script
const SLOW_REPLY: u64 = 5;
/// Seconds a two-player step waits for the other probe
const PARTNER_WAIT: u64 = 60;

const MAGNIFIER: u32 = 611;
const BLUE_GEMSTONE: u32 = 717;
/// What a Blacksmith's Repair Weapon may take: Iron, Steel, Iron Ore, Rough Oridecon
const REPAIR_MATERIALS: [u32; 4] = [998, 999, 1002, 756];
/// rAthena's mercenary action 2 dismisses it
const MERCENARY_DISMISS: i8 = 2;
const WARP_PORTAL_LEVEL: i16 = 4;

impl Social {
    pub(super) async fn play(
        &mut self,
        zone: &mut Probe,
        step: &str,
        me: &str,
    ) -> Result<(), String> {
        let pv = zone.packetver;
        let (verb, arg) = step.split_once(':').unwrap_or((step, ""));
        let numbers: Vec<u32> = arg.split(',').filter_map(|v| v.parse().ok()).collect();
        let report = match (verb, &numbers[..]) {
            ("shop-equip", &[x,y,item])=>{
                let npc=self.npc_at(x,y)?;self.open_shop(zone,npc,Deal::Buy).await?;
                expect_zero("paid equipment purchase",self.buy(zone,item).await?)?;
                self.idle(zone,Duration::from_millis(300)).await;
                zone.send(sender::build_npc_trade_quit_packet(pv)).await;
                let(index,location)=*self.equipment_picks.get(&item).ok_or("purchased equipment pickup absent")?;
                let worn=self.request(zone,sender::build_equip_item_packet(index,location,pv),REPLY,"equip paid purchase",|e|match e {GameEvent::InventoryEquipResult {index:i,success,..} if *i==index=>Some(*success),_=>None}).await?;
                ensure(worn,"server refused purchased equipment")?;
                format!("paid normal shop price and equipped item {item}")
            }
            // a buy with the shop open goes through; after 0x09d4 the server has forgotten the shop and refuses
            // the same buy
            ("shop-quit", &[x, y, item]) => {
                let npc = self.npc_at(x, y)?;
                self.open_shop(zone, npc, Deal::Buy).await?;
                let open = self.buy(zone, item).await?;
                self.open_shop(zone, npc, Deal::Buy).await?;
                zone.send(sender::build_npc_trade_quit_packet(pv)).await;
                let quit = self.buy(zone, item).await?;
                ensure(
                    open == 0 && quit != 0,
                    &format!(
                        "buy with the shop open {open}, after 0x09d4 {quit}: expected 0, then refused"
                    ),
                )?;
                format!("buy with the shop open {open}, after 0x09d4 {quit}")
            }
            ("shop-sell", &[x, y, item]) => {
                let index = self.get_item(zone, me, item, 1).await?;
                let npc = self.npc_at(x, y)?;
                self.open_shop(zone, npc, Deal::Sell).await?;
                let sell = sender::build_sell_item_list_packet(&[(index as i16, 1)], pv);
                let result = self
                    .request(zone, sell, REPLY, "sell result", |e| match e {
                        GameEvent::NpcShopSellResult { result } => Some(*result),
                        _ => None,
                    })
                    .await?;
                expect_zero("sell", result)?;
                "sold".into()
            }
            // two of an item with a status-bound reuse delay: the first is used, the second at once is refused
            // with 0x07e2's seconds left
            ("reuse", &[item]) => {
                let index = self.get_item(zone, me, item, 2).await?;
                let use_item = sender::build_use_item_packet(index, self.aid, pv);
                let used = self
                    .request(zone, use_item.clone(), REPLY, "use result", |e| match e {
                        GameEvent::InventoryUseItemResult { success, .. } => Some(*success),
                        _ => None,
                    })
                    .await?;
                ensure(used, "the first use was refused")?;
                // past `item_use_interval`, which refuses without a word, but well inside the item's own delay
                self.idle(zone, Duration::from_millis(500)).await;
                let (msg_id, value) = self
                    .request(
                        zone,
                        use_item,
                        REPLY,
                        "message with a value (0x07e2)",
                        |e| match e {
                            GameEvent::ServerMsgValue { msg_id, value } => Some((*msg_id, *value)),
                            _ => None,
                        },
                    )
                    .await?;
                format!("used once, then message {msg_id} with {value}")
            }
            ("cart", &[item]) => {
                let cart_index = self.put_in_cart(zone, me, item).await?;
                let back = sender::build_move_item_cart_to_body_packet(cart_index, 1, pv);
                self.request(zone, back, REPLY, "the item back from the cart", |e| {
                    matches!(e, GameEvent::InventoryItemPickup { item_id, .. } if *item_id == item)
                        .then_some(())
                })
                .await?;
                "into the cart and back".into()
            }
            // leaves one ITEM in the cart, for a game run that sells it (a cart is kept across logins)
            ("cart-load", &[item]) => {
                let cart_index = self.put_in_cart(zone, me, item).await?;
                format!("item {item} at cart index {cart_index}")
            }
            ("storage", &[item]) => {
                let index = self.get_item(zone, me, item, 1).await?;
                self.request(zone, chat(me, "@storage", pv), REPLY, "storage", |e| {
                    matches!(e, GameEvent::StorageOpened { .. }).then_some(())
                })
                .await?;
                let store = sender::build_move_item_body_to_store_packet(index, 1, pv);
                let stored = self
                    .request(zone, store, REPLY, "stored item", |e| match e {
                        GameEvent::StorageItemAdded { index, item_id, .. } if *item_id == item => {
                            Some(*index)
                        }
                        _ => None,
                    })
                    .await?;
                let back = sender::build_move_item_store_to_body_packet(stored, 1, pv);
                self.request(zone, back, REPLY, "the item back from storage", |e| {
                    matches!(e, GameEvent::StorageItemRemoved { index, .. } if *index == stored)
                        .then_some(())
                })
                .await?;
                let close = sender::build_close_store_packet(pv);
                self.request(zone, close, REPLY, "storage closed", |e| {
                    matches!(e, GameEvent::StorageClosed).then_some(())
                })
                .await?;
                "stored, taken back, closed".into()
            }
            // a merchant's shop from the cart; done when a buyer takes the item
            ("vend", &[item, price]) => {
                let cart_index = self.put_in_cart(zone, me, item).await?;
                self.cast_for(
                    zone,
                    SkillEnum::McVending,
                    1,
                    None,
                    REPLY,
                    "vending setup",
                    |e| matches!(e, GameEvent::OpenVendingSetup { .. }).then_some(()),
                )
                .await?;
                let open = sender::build_req_openstore2_packet(
                    "Probe shop",
                    &[(cart_index as i16, 1, price as i32)],
                    pv,
                );
                self.request(zone, open, REPLY, "own stock", |e| {
                    matches!(e, GameEvent::VendingOwnStock { .. }).then_some(())
                })
                .await?;
                self.expect(zone, PARTNER_WAIT, "a sale", |e| {
                    matches!(e, GameEvent::VendingStockDecrement { .. }).then_some(())
                })
                .await?;
                // sold out: the client closes the shop, as the game does, or the server keeps us vending
                zone.send(sender::build_req_closestore_packet(pv)).await;
                "shop open, the item sold, the shop closed".into()
            }
            ("vend-buy", &[vendor]) => {
                // the shop sign comes as the shop opens, or with the vendor coming into sight
                if !self.vendors.contains(&vendor) {
                    self.expect(zone, PARTNER_WAIT, "the vendor's shop", |e| {
                        matches!(e, GameEvent::VendingBoardShown { aid, .. } if *aid == vendor)
                            .then_some(())
                    })
                    .await?;
                }
                let browse = sender::build_req_buy_frommc_packet(vendor, pv);
                let (unique_id, first) = self
                    .request(zone, browse, REPLY, "the vendor's list", |e| match e {
                        GameEvent::VendingShopList {
                            unique_id, items, ..
                        } => items.first().map(|i| (*unique_id, i.clone())),
                        _ => None,
                    })
                    .await?;
                let purchase = sender::build_purchase_frommc2_packet(
                    vendor,
                    unique_id,
                    &[(1, first.index)],
                    pv,
                );
                self.request(zone, purchase, REPLY, "the bought item", |e| {
                    matches!(e, GameEvent::InventoryItemPickup { item_id, .. } if *item_id == first.item_id)
                        .then_some(())
                })
                .await?;
                format!("bought item {} for {}z", first.item_id, first.price)
            }
            // the partner probe accepts and confirms on its own (`note`)
            ("trade", &[partner, item]) => {
                let index = self.get_item(zone, me, item, 1).await?;
                let ask = sender::build_req_exchange_item_packet(partner, pv);
                let result = self
                    .request(zone, ask, PARTNER_WAIT, "trade answer", |e| match e {
                        GameEvent::ExchangeAckResult { result, .. } => Some(*result),
                        _ => None,
                    })
                    .await?;
                ensure(result == TRADE_ACCEPTED, &format!("trade answer {result}"))?;
                let add = sender::build_add_exchange_item_packet(index, 1, pv);
                let added = self
                    .request(zone, add, REPLY, "item added", |e| match e {
                        GameEvent::ExchangeAddResult { result, .. } => Some(*result),
                        _ => None,
                    })
                    .await?;
                expect_zero("adding the item", added)?;
                let ok = sender::build_conclude_exchange_item_packet(pv);
                self.request(zone, ok, PARTNER_WAIT, "the partner's OK", |e| {
                    matches!(e, GameEvent::ExchangeConcluded { who: PARTNER }).then_some(())
                })
                .await?;
                let exec = sender::build_exec_exchange_item_packet(pv);
                let done = self
                    .request(zone, exec, PARTNER_WAIT, "trade completed", |e| match e {
                        GameEvent::ExchangeCompleted { result } => Some(*result),
                        _ => None,
                    })
                    .await?;
                expect_zero("the trade", done)?;
                "traded".into()
            }
            // a Blacksmith repairs a damaged weapon of its own (attribute 1 through `@item2`)
            ("repair", &[item]) => {
                for material in REPAIR_MATERIALS {
                    zone.send(chat(me, &format!("@item {material} 5"), pv))
                        .await;
                }
                zone.send(chat(me, &format!("@item2 {item} 1 1 0 1 0 0 0 0"), pv))
                    .await;
                let row = self
                    .cast_for(
                        zone,
                        SkillEnum::BsRepairweapon,
                        1,
                        None,
                        SLOW_REPLY,
                        "repair list",
                        |e| match e {
                            GameEvent::RepairItemList { items } => {
                                items.iter().find(|r| r.item_id == item).cloned()
                            }
                            _ => None,
                        },
                    )
                    .await?;
                let pick = sender::build_req_itemrepair_packet(
                    row.index,
                    row.item_id,
                    row.refine,
                    row.cards,
                    pv,
                );
                let ok = self
                    .request(zone, pick, REPLY, "repair result", |e| match e {
                        GameEvent::RepairItemResult { ok, .. } => Some(*ok),
                        _ => None,
                    })
                    .await?;
                ensure(ok, "the repair failed")?;
                "repaired".into()
            }
            ("card", &[card, equip]) => {
                let equip_index = self.get_item(zone, me, equip, 1).await?;
                let card_index = self.get_item(zone, me, card, 1).await?;
                let list = sender::build_card_composition_list_packet(card_index, pv);
                self.request(zone, list, REPLY, "equipment for the card", |e| match e {
                    GameEvent::CardInsertItemList { equip_indices, .. } => {
                        equip_indices.contains(&equip_index).then_some(())
                    }
                    _ => None,
                })
                .await?;
                let insert = sender::build_card_composition_packet(card_index, equip_index, pv);
                let result = self
                    .request(zone, insert, REPLY, "card result", |e| match e {
                        GameEvent::CardInsertResult { result, .. } => Some(*result),
                        _ => None,
                    })
                    .await?;
                expect_zero("the card", result)?;
                "card inserted".into()
            }
            // an unidentified item (`@item2 ... identify 0`) and a Magnifier
            ("identify", &[item]) => {
                let unidentified = chat(me, &format!("@item2 {item} 1 0 0 0 0 0 0 0"), pv);
                let unknown = self
                    .request(
                        zone,
                        unidentified,
                        REPLY,
                        "the unidentified item",
                        |e| match e {
                            GameEvent::InventoryItemPickup {
                                index,
                                item_id,
                                is_identified: false,
                                ..
                            } if *item_id == item => Some(*index),
                            _ => None,
                        },
                    )
                    .await?;
                let magnifier = self.get_item(zone, me, MAGNIFIER, 1).await?;
                // an item skill: the server hands it over (0x0147) and the client casts it
                let use_magnifier = sender::build_use_item_packet(magnifier, self.aid, pv);
                let (skill, level) = self
                    .request(
                        zone,
                        use_magnifier,
                        REPLY,
                        "the Magnifier's skill",
                        |e| match e {
                            GameEvent::AutoCastSkill { skill, level, .. } => Some((*skill, *level)),
                            _ => None,
                        },
                    )
                    .await?;
                self.cast_for(
                    zone,
                    skill,
                    level,
                    None,
                    REPLY,
                    "identify list",
                    |e| match e {
                        GameEvent::ItemIdentifyList { indices } => {
                            indices.contains(&unknown).then_some(())
                        }
                        _ => None,
                    },
                )
                .await?;
                let pick = sender::build_req_itemidentify_packet(unknown as i16, pv);
                let ok = self
                    .request(zone, pick, REPLY, "identify result", |e| match e {
                        GameEvent::ItemIdentifyResult { ok, .. } => Some(*ok),
                        _ => None,
                    })
                    .await?;
                ensure(ok, "identification failed")?;
                "identified".into()
            }
            // an Acolyte's Warp Portal (with a Blue Gemstone) on the cell X,Y: the list, then the portal
            ("warp-portal", &[x, y]) => {
                self.get_item(zone, me, BLUE_GEMSTONE, 1).await?;
                let cast = sender::build_use_skill_to_ground_packet(
                    SkillEnum::AlWarp,
                    WARP_PORTAL_LEVEL,
                    x as i16,
                    y as i16,
                    pv,
                );
                let destinations = self
                    .request(zone, cast, SLOW_REPLY, "warp list", |e| match e {
                        GameEvent::WarpList { destinations, .. } => Some(destinations.clone()),
                        _ => None,
                    })
                    .await?;
                let first = destinations.first().ok_or("an empty warp list")?.clone();
                let select = sender::build_select_warppoint_packet(SkillEnum::AlWarp, &first, pv);
                self.request(zone, select, REPLY, "the portal", |e| {
                    matches!(e, GameEvent::SkillUnitEntered { x: ux, y: uy, .. }
                        if (*ux as u32, *uy as u32) == (x, y))
                    .then_some(())
                })
                .await?;
                format!("{} destinations, a portal to {first}", destinations.len())
            }
            ("die-respawn", &[]) => {
                let aid = self.aid;
                self.request(zone, chat(me, "@die", pv), REPLY, "our death", |e| {
                    matches!(e, GameEvent::EntityVanished { gid, vanish_type: VanishType::Die }
                        if *gid == aid)
                    .then_some(())
                })
                .await?;
                let respawn = sender::build_return_savepoint_packet(pv);
                let map = self
                    .request(zone, respawn, SLOW_REPLY, "the save point", |e| match e {
                        GameEvent::MapChanged { map_name, x, y } => {
                            Some(format!("{map_name} {x},{y}"))
                        }
                        _ => None,
                    })
                    .await?;
                format!("died, back at {map}")
            }
            ("merc", &[scroll]) => {
                let index = self.get_item(zone, me, scroll, 1).await?;
                let use_scroll = sender::build_use_item_packet(index, self.aid, pv);
                let (gid, name) = self
                    .request(zone, use_scroll, SLOW_REPLY, "mercenary", |e| match e {
                        GameEvent::MercenaryInfoReceived { info, .. } => {
                            Some((info.gid, info.name.clone()))
                        }
                        _ => None,
                    })
                    .await?;
                let dismiss = sender::build_mercenary_command_packet(MERCENARY_DISMISS, pv);
                self.request(zone, dismiss, REPLY, "the mercenary gone", |e| {
                    matches!(e, GameEvent::EntityVanished { gid: g, .. } if *g == gid).then_some(())
                })
                .await?;
                format!("{name} hired and dismissed")
            }
            // a room for two; done when the guest has come and gone, and the room is closed
            ("chatroom-open", &[]) => {
                let title = if arg.is_empty() { "Probe room" } else { arg };
                let create = sender::build_create_chatroom_packet(title, 2, true, "", pv);
                let flag = self
                    .request(zone, create, REPLY, "room created", |e| match e {
                        GameEvent::ChatRoomCreateResult { flag } => Some(*flag),
                        _ => None,
                    })
                    .await?;
                expect_zero("the room", flag)?;
                let guest = self
                    .expect(zone, PARTNER_WAIT, "a guest", |e| match e {
                        GameEvent::ChatRoomMemberJoined { name, .. } => Some(name.clone()),
                        _ => None,
                    })
                    .await?;
                self.expect(zone, PARTNER_WAIT, "the guest leaving", |e| {
                    matches!(e, GameEvent::ChatRoomMemberLeft { name, .. } if *name == guest)
                        .then_some(())
                })
                .await?;
                let leave = sender::build_exit_room_packet(pv);
                self.request(zone, leave, REPLY, "the room closed", |e| {
                    matches!(e, GameEvent::ChatRoomDestroy { .. }).then_some(())
                })
                .await?;
                format!("{guest} came and left, the room closed")
            }
            ("chatroom-join", &[owner]) => {
                let room = match self.rooms.get(&owner) {
                    Some(room) => *room,
                    None => {
                        self.expect(zone, PARTNER_WAIT, "the room", |e| match e {
                            GameEvent::ChatRoomUpsert {
                                owner_aid, room_id, ..
                            } if *owner_aid == owner => Some(*room_id),
                            _ => None,
                        })
                        .await?
                    }
                };
                let enter = sender::build_req_enter_room_packet(room, pv);
                let members = self
                    .request(zone, enter, REPLY, "entering the room", |e| match e {
                        GameEvent::ChatRoomEntered { members, .. } => Some(members.len()),
                        _ => None,
                    })
                    .await?;
                self.idle(zone, Duration::from_secs(1)).await;
                let leave = sender::build_exit_room_packet(pv);
                self.request(zone, leave, REPLY, "leaving the room", |e| {
                    matches!(e, GameEvent::ChatRoomMemberLeft { name, .. } if name == me)
                        .then_some(())
                })
                .await?;
                format!("joined room {room} ({members} in it), left")
            }
            // a refiner on X,Y (its dialog, then rAthena's refine window): ITEM in, its first material, a safe +1
            ("refine", &[x, y, item]) => {
                let index = self.get_item(zone, me, item, 1).await?;
                let npc = self.npc_at(x, y)?;
                zone.send(sender::build_contact_npc_packet(npc, pv)).await;
                self.answer_dialog(zone).await?;
                self.expect(zone, REPLY, "the refine window", |e| {
                    matches!(e, GameEvent::RefineWindowOpened).then_some(())
                })
                .await?;
                let select = sender::build_refining_select_item_packet(index, pv);
                let material = self
                    .request(zone, select, REPLY, "the item's materials", |e| match e {
                        GameEvent::RefineMaterials { materials, .. } => {
                            materials.first().map(|m| m.item_id)
                        }
                        _ => None,
                    })
                    .await?;
                self.get_item(zone, me, material, 1).await?;
                let refine = sender::build_req_refining_packet(index, material, false, pv);
                let (result, level) = self
                    .request(zone, refine, SLOW_REPLY, "refine result", |e| match e {
                        GameEvent::ItemRefiningResult { result, refine, .. } => {
                            Some((*result, *refine))
                        }
                        _ => None,
                    })
                    .await?;
                ensure(result == 0, &format!("a safe refine failed ({result})"))?;
                zone.send(sender::build_close_refining_ui_packet(pv)).await;
                format!("+{level} with material {material}")
            }
            // a walk onto the warp cell X,Y, to a map only the other map server holds (run-host.sh's
            // SECOND_MAP_SERVER; `@warp` refuses such maps): 0x0ac7, then the game's way over, a connection to that
            // server and the same login
            ("server-move", &[x, y]) => {
                let walk = sender::build_request_move_packet(x as u16, y as u16, pv);
                let (map, addr) = self
                    .request(
                        zone,
                        walk,
                        SLOW_REPLY,
                        "a map-server move (0x0ac7)",
                        |e| match e {
                            GameEvent::ZoneServerChanged { map_name, ip, port } => Some((
                                map_name.clone(),
                                format!("{}:{port}", ip_u32_to_string(*ip)),
                            )),
                            _ => None,
                        },
                    )
                    .await?;
                *zone = zone.follow(&addr).await;
                let (x, y) = self
                    .request(
                        zone,
                        self.zone_enter.clone(),
                        SLOW_REPLY,
                        "entering the next server",
                        |e| match e {
                            GameEvent::MapEntered { x, y, .. } => Some((*x, *y)),
                            _ => None,
                        },
                    )
                    .await?;
                zone.send(build_map_loaded_packet(pv)).await;
                format!("{map} on {addr}, at {x},{y}")
            }
            // walks to X,Y, then stays a moment for what the cell sets off (touch areas)
            ("walk", &[x, y]) => {
                let walk = sender::build_request_move_packet(x as u16, y as u16, pv);
                self.request(zone, walk, REPLY, "the walk", |e| {
                    matches!(e, GameEvent::PlayerMoved { .. }).then_some(())
                })
                .await?;
                self.idle(zone, Duration::from_secs(4)).await;
                format!("at {x},{y}")
            }
            // talks to the NPC on X,Y and answers its dialog; the quests it gave or took
            ("fight-objective", &[quest_id,mob,required,seconds]) => {
                println!("CAMPAIGN_START quest={quest_id} skill={:?} hp={:?}/{} position={:?}",self.combat_skill,self.hp,self.max_hp,self.position);
                let mut navigation=CampaignNavigation::load(quest_id);
                let deadline=Instant::now()+Duration::from_secs(seconds as u64);
                let mut last_walk=Instant::now()-Duration::from_secs(10);
                let mut last_skill=Instant::now()-Duration::from_secs(10);
                let mut search=0usize;let mut ticks=0;let mut last_target=0;
                loop {
                    let done=self.campaign_log.get(quest_id).is_some_and(|q|q.objectives.iter().any(|o|o.current>=required as i16));
                    let stage=nostalre_game::campaign_data::STAGES.iter().find(|s|s.id==quest_id).ok_or("campaign stage absent")?;
                    if !done && mob!=1916 && !self.map_name.is_empty() && self.map_name!=stage.area {
                        // Exploration can cross a normal warp. Use the actual exposed
                        // campaign travel menu to return, keeping all earned counters.
                        self.npc_choices=vec![4];zone.send(chat(me,"@campanha",pv)).await;
                        self.answer_dialog(zone).await?;self.idle(zone,Duration::from_secs(2)).await;
                        last_target=0;continue;
                    }
                    if done {break;}
                    ensure(Instant::now()<deadline,&format!("objective {quest_id} timed out"))?;
                    if self.dead {
                        ensure(mob!=1916,"player died in the owned boss attempt")?;
                        println!("CAMPAIGN_RESPAWN quest={quest_id}");
                        zone.send(sender::build_return_savepoint_packet(pv)).await;self.idle(zone,Duration::from_secs(3)).await;
                        for choice in [3,4] {self.npc_choices=vec![choice];zone.send(chat(me,"@campanha",pv)).await;self.answer_dialog(zone).await?;}
                        self.idle(zone,Duration::from_secs(3)).await;last_target=0;continue;
                    }
                    if mob!=1916 && ticks>0 && ticks%12==0 && self.hp.is_some_and(|hp|hp<self.max_hp/3) {
                        self.npc_choices=vec![3];zone.send(chat(me,"@campanha",pv)).await;
                        self.answer_dialog(zone).await?;self.idle(zone,Duration::from_secs(3)).await;
                        last_target=0;
                    }
                    if self.heal_skill_level>0 {
                        if last_skill.elapsed()>Duration::from_secs(2) && self.hp.is_some_and(|hp|hp<self.max_hp*7/10) {zone.send(sender::build_use_skill_packet(SkillEnum::AlHeal,self.heal_skill_level,self.aid,pv)).await;last_skill=Instant::now();}
                    }
                    // Clear nearby native monsters on hunts instead of blindly walking
                    // through aggressive packs. Boss tests engage only their owned MVP.
                    let target=self.units.iter().filter(|u|u.job as u32==mob || (mob!=1916 && self.hp.is_some_and(|hp|hp<self.max_hp*3/4) && u.cell.0.abs_diff(self.position.0).max(u.cell.1.abs_diff(self.position.1))<=2 && (1000..4000).contains(&u.job) && !(1078..=1085).contains(&u.job))).min_by_key(|u|(u.job as u32!=mob,u.cell.0.abs_diff(self.position.0).max(u.cell.1.abs_diff(self.position.1)))).map(|u|(u.id,u.cell));
                    if let Some((id,(x,y)))=target {
                        let magic=matches!(self.combat_skill,Some((SkillEnum::MgFirebolt|SkillEnum::AlHolylight,_)));
                        if self.position.0.abs_diff(x).max(self.position.1.abs_diff(y))>if magic {8} else {1} {
                            if last_walk.elapsed()>Duration::from_secs(2) {let (x,y)=navigation.as_mut().and_then(|n|n.toward(self.position,(x,y))).unwrap_or((x,y));zone.send(sender::build_request_move_packet(x,y,pv)).await;last_walk=Instant::now();}
                        } else if magic {
                            if let Some((skill,level))=self.combat_skill {let cooldown=if skill==SkillEnum::AlHolylight {Duration::from_millis(1500)} else {Duration::from_secs(6)};if last_skill.elapsed()>cooldown {zone.send(sender::build_use_skill_packet(skill,level,id,pv)).await;last_skill=Instant::now();}}
                        } else {
                            if id!=last_target || ticks%4==0 {zone.send(sender::build_action_request_packet(id,7,pv)).await;last_target=id;}
                            if let Some((SkillEnum::SmBash,level))=self.combat_skill {if last_skill.elapsed()>Duration::from_millis(1200) {zone.send(sender::build_use_skill_packet(SkillEnum::SmBash,level,id,pv)).await;last_skill=Instant::now();}}
                        }
                    } else if last_walk.elapsed()>Duration::from_secs(2) {
                        // Finite exploration around the chapter's authored hunting destination.
                        let stage=nostalre_game::campaign_data::STAGES.iter().find(|s|s.id==quest_id).ok_or("campaign stage absent")?;
                        let offsets=[(0,0),(8,0),(0,8),(-8,0),(0,-8),(16,8),(-16,8),(8,-16),(-8,-16)];
                        let(dx,dy)=offsets[search%offsets.len()];search+=1;
                        let (x,y)=navigation.as_mut().and_then(|n|n.explore(self.position)).unwrap_or(((stage.ax as i32+dx) as u16,(stage.ay as i32+dy) as u16));
                        zone.send(sender::build_request_move_packet(x,y,pv)).await;last_walk=Instant::now();
                    }
                    if ticks%10==0 {println!("CAMPAIGN_COMBAT quest={quest_id} hp={:?}/{} units={:?} progress={:?}",self.hp,self.max_hp,self.units.iter().map(|u|(u.job,u.cell)).collect::<Vec<_>>(),self.campaign_log.get(quest_id).map(|q|&q.objectives));}
                    ticks+=1;
                    if mob==1916 {ensure(!self.dead && self.hp!=Some(0),"player died in real campaign combat")?;}
                    if ticks%3==0 && self.hp.is_some_and(|hp|hp<self.max_hp/3) {zone.send(sender::build_use_skill_packet(SkillEnum::NvFirstaid,1,self.aid,pv)).await;}
                    self.idle(zone,Duration::from_secs(1)).await;
                    if last_walk.elapsed()>Duration::from_secs(2) {
                        for (&drop,&(x,y)) in &self.floor_positions {if x.abs_diff(self.position.0).max(y.abs_diff(self.position.1))<=2 {zone.send(sender::build_pickup_item_packet(drop,pv)).await;}}
                    }
                }
                self.idle(zone,Duration::from_secs(3)).await;
                for (&drop,&(x,y)) in &self.floor_positions {if x.abs_diff(self.position.0).max(y.abs_diff(self.position.1))<=2 {zone.send(sender::build_pickup_item_packet(drop,pv)).await;}}
                format!("campaign objective {quest_id}: {required} real kills, no GM progress command")
            }
            ("talk", &[x, y]) => {
                let npc = self.npc_at(x, y)?;
                let before = self.quests.len();
                zone.send(sender::build_contact_npc_packet(npc, pv)).await;
                let answers = self.answer_dialog(zone).await?;
                let quests = self.quests[before..].to_vec();
                format!("{answers} answers, quests {quests:?}")
            }
            // a skill of one's own on oneself (SKILL,LEVEL), or on a monster MOB spawned for it (SKILL,LEVEL,MOB):
            // the server takes it when its effect or damage comes back under the skill's id
            // Auto Shadow Spell at LEVEL with a copied skill (Reproduce): the menu of copied skills (0x0442), the first
            // picked (0x0443), and the autocast status it starts on us
            ("shadow-spell", &[level]) => {
                let me = self.aid;
                let skill = SkillEnum::ScAutoshadowspell;
                let (why, skills) = self
                    .cast_for(
                        zone,
                        skill,
                        level as i16,
                        None,
                        SLOW_REPLY,
                        "the skill menu",
                        |e| match e {
                            GameEvent::SkillSelectList { why, skills } => {
                                Some((*why, skills.clone()))
                            }
                            _ => None,
                        },
                    )
                    .await?;
                let first = *skills.first().ok_or("an empty skill menu")?;
                let pick = sender::build_skill_select_response_packet(why, Some(first), pv);
                let efst = self
                    .request(zone, pick, REPLY, "the autocast status", |e| match e {
                        GameEvent::StatusEffectChanged {
                            gid,
                            efst,
                            active: true,
                            ..
                        } if *gid == me => Some(*efst),
                        _ => None,
                    })
                    .await?;
                format!("menu {why} {skills:?}, picked {first:?}, status {efst}")
            }
            // a crafting skill (Mix Cooking, Create Bomb...) at LEVEL: its list (0x025a), ITEM picked from it (0x025b),
            // then the item made or the skill's message (0x07e6) when the try fails
            ("cook", &[id, level, item]) => {
                let (mk_type, items) = self.cooking_list(zone, id, level).await?;
                ensure(
                    items.contains(&item),
                    &format!("{item} not in the list {items:?}"),
                )?;
                let pick = sender::build_req_cooking_packet(mk_type, item, pv);
                let outcome = self
                    .request(zone, pick, REPLY, "the cooking result", |e| match e {
                        GameEvent::InventoryItemPickup { item_id, .. } if *item_id == item => {
                            Some("made".to_string())
                        }
                        GameEvent::ServerSkillMsg { msg_id, .. } => {
                            Some(format!("message {msg_id}"))
                        }
                        _ => None,
                    })
                    .await?;
                format!("list {mk_type} {items:?}, {item}: {outcome}")
            }
            // the same list closed with item 0, then cast again: it comes back only if the server let go of the menu
            ("cook-cancel", &[id, level]) => {
                let mut kinds = Vec::new();
                for _ in 0..2 {
                    let (mk_type, _) = self.cooking_list(zone, id, level).await?;
                    zone.send(sender::build_req_cooking_packet(mk_type, 0, pv))
                        .await;
                    kinds.push(mk_type);
                    self.idle(zone, Duration::from_secs(2)).await;
                }
                format!("the list twice, closed with item 0 each time: {kinds:?}")
            }
            // Change Material or Four Spirit Analysis (SKILL,LEVEL) on ITEM,AMOUNT pairs got with `@item`: the item list
            // window (0x07e3), the chosen items by inventory index (0x07e4), then what the server makes of them
            ("item-list", &[id, level, ref pairs @ ..])
                if !pairs.is_empty() && pairs.len() % 2 == 0 =>
            {
                let mut chosen = Vec::new();
                for pair in pairs.chunks(2) {
                    let index = self.get_item(zone, me, pair[0], pair[1]).await?;
                    chosen.push((index, pair[1] as u16));
                }
                let kind = self.item_list_window(zone, id, level).await?;
                let inputs: Vec<u32> = pairs.chunks(2).map(|p| p[0]).collect();
                let answer = sender::build_itemlistwin_res_packet(kind, true, &chosen, pv);
                let outcome = self
                    .request(
                        zone,
                        answer,
                        SLOW_REPLY,
                        "what the window made",
                        |e| match e {
                            GameEvent::InventoryItemPickup { item_id, count, .. }
                                if !inputs.contains(item_id) =>
                            {
                                Some(format!("made {count} of {item_id}"))
                            }
                            GameEvent::ServerSkillMsg { msg_id, .. } => {
                                Some(format!("message {msg_id}"))
                            }
                            GameEvent::SkillFailed { cause, .. } => {
                                Some(format!("failed, cause {cause}"))
                            }
                            _ => None,
                        },
                    )
                    .await?;
                format!("window {kind}, sent {chosen:?}: {outcome}")
            }
            // the window cancelled (0x07e4, OK 0), then cast again: it comes back only if the server let go of the menu
            ("item-list-cancel", &[id, level]) => {
                let mut kinds = Vec::new();
                for _ in 0..2 {
                    let kind = self.item_list_window(zone, id, level).await?;
                    zone.send(sender::build_itemlistwin_res_packet(kind, false, &[], pv))
                        .await;
                    kinds.push(kind);
                    self.idle(zone, Duration::from_secs(2)).await;
                }
                format!("the window twice, cancelled each time: {kinds:?}")
            }
            ("cast", &[id, level]) => self.cast(zone, id, level, None).await?,
            ("cast", &[id, level, mob]) => {
                zone.send(chat(me, &format!("@monster {mob}"), pv)).await;
                let target = self
                    .expect(zone, REPLY, &format!("monster {mob}"), |e| match e {
                        GameEvent::EntitySpawned { gid, job, .. } if u32::from(*job) == mob => {
                            Some(*gid)
                        }
                        _ => None,
                    })
                    .await?;
                self.cast(zone, id, level, Some(target)).await?
            }
            _ => return Err(format!("unknown step {step}")),
        };
        println!("social: {step}: {report}");
        Ok(())
    }

    /// Reads packets as `idle` does (answering invitations, keeping what the steps need) until `want` picks an
    /// event; `what` names it when none comes within `secs`
    async fn expect<T>(
        &mut self,
        zone: &mut Probe,
        secs: u64,
        what: &str,
        mut want: impl FnMut(&GameEvent) -> Option<T>,
    ) -> Result<T, String> {
        let started = Instant::now();
        let deadline = started + Duration::from_secs(secs);
        while !zone.closed && Instant::now() < deadline {
            zone.keep_alive(started).await;
            if let Some(found) = self.pump(zone, &mut want).await {
                return Ok(found);
            }
        }
        Err(closed_or(zone, &format!("no {what}")))
    }

    /// Sends `packet` and waits for the answer `want` picks
    pub(super) async fn request<T>(
        &mut self,
        zone: &mut Probe,
        packet: Vec<u8>,
        secs: u64,
        what: &str,
        want: impl FnMut(&GameEvent) -> Option<T>,
    ) -> Result<T, String> {
        zone.send(packet).await;
        self.expect(zone, secs, what, want).await
    }

    /// Casts `skill` at `level` on `target` (oneself without one) and waits for the answer `want` picks; the server's
    /// refusal of the skill (0x0110) fails at once, with its cause
    #[allow(clippy::too_many_arguments)]
    async fn cast_for<T>(
        &mut self,
        zone: &mut Probe,
        skill: SkillEnum,
        level: i16,
        target: Option<u32>,
        secs: u64,
        what: &str,
        mut want: impl FnMut(&GameEvent) -> Option<T>,
    ) -> Result<T, String> {
        let target = target.unwrap_or(self.aid);
        let cast = sender::build_use_skill_packet(skill, level, target, zone.packetver);
        self.request(zone, cast, secs, what, |e| match e {
            GameEvent::SkillFailed {
                skill: s, cause, ..
            } if *s == Some(skill) => Some(Err(format!("{skill:?} refused, cause {cause}"))),
            _ => want(e).map(Ok),
        })
        .await?
    }

    /// Casts skill `id` at `level` on `target` (oneself without one): the cast's time when it has one, then its
    /// effect or damage
    async fn cast(
        &mut self,
        zone: &mut Probe,
        id: u32,
        level: u32,
        target: Option<u32>,
    ) -> Result<String, String> {
        let skill = SkillEnum::try_from_value(id)?;
        let me = self.aid;
        let mut cast_ms = 0;
        let outcome = self
            .cast_for(
                zone,
                skill,
                level as i16,
                target,
                SLOW_REPLY,
                &format!("{skill:?}'s effect"),
                |e| match e {
                    GameEvent::SkillCasting {
                        gid,
                        skill: s,
                        delay_ms,
                        ..
                    } if *gid == me && *s == skill => {
                        cast_ms = *delay_ms;
                        None
                    }
                    GameEvent::SkillNoDamage {
                        skill: s, src_gid, ..
                    } if *src_gid == me && *s == skill => Some("its effect".to_string()),
                    GameEvent::SkillDamage {
                        skill: s,
                        src_gid,
                        damage,
                        ..
                    } if *src_gid == me && *s == skill => Some(format!("{damage} damage")),
                    _ => None,
                },
            )
            .await?;
        Ok(format!("{skill:?}: cast {cast_ms} ms, {outcome}"))
    }

    /// Casts crafting skill `id` at `level`: its list's kind and items (0x025a)
    async fn cooking_list(
        &mut self,
        zone: &mut Probe,
        id: u32,
        level: u32,
    ) -> Result<(u16, Vec<u32>), String> {
        let skill = SkillEnum::try_from_value(id)?;
        self.cast_for(
            zone,
            skill,
            level as i16,
            None,
            SLOW_REPLY,
            "the cooking list",
            |e| match e {
                GameEvent::CookingList { mk_type, item_ids } => Some((*mk_type, item_ids.clone())),
                _ => None,
            },
        )
        .await
    }

    /// SKILL at LEVEL and the item list window's kind it opens (0x07e3)
    async fn item_list_window(
        &mut self,
        zone: &mut Probe,
        id: u32,
        level: u32,
    ) -> Result<i32, String> {
        let skill = SkillEnum::try_from_value(id)?;
        self.cast_for(
            zone,
            skill,
            level as i16,
            None,
            SLOW_REPLY,
            "the item list window",
            |e| match e {
                GameEvent::ItemListWindowOpened { kind } => Some(*kind),
                _ => None,
            },
        )
        .await
    }

    /// `@item ITEM COUNT` and the inventory index it lands on
    async fn get_item(
        &mut self,
        zone: &mut Probe,
        me: &str,
        item: u32,
        count: u32,
    ) -> Result<u16, String> {
        let gm_item = chat(me, &format!("@item {item} {count}"), zone.packetver);
        self.request(zone, gm_item, REPLY, &format!("item {item}"), |e| match e {
            GameEvent::InventoryItemPickup { index, item_id, .. } if *item_id == item => {
                Some(*index)
            }
            _ => None,
        })
        .await
    }

    /// A cart (`@cart 1`) with one `item` in it; the item's cart index
    async fn put_in_cart(&mut self, zone: &mut Probe, me: &str, item: u32) -> Result<u16, String> {
        let pv = zone.packetver;
        zone.send(chat(me, "@cart 1", pv)).await;
        let index = self.get_item(zone, me, item, 1).await?;
        let to_cart = sender::build_move_item_body_to_cart_packet(index, 1, pv);
        self.request(zone, to_cart, REPLY, "cart item", |e| match e {
            GameEvent::CartItemAdded { index, item_id, .. } if *item_id == item => Some(*index),
            _ => None,
        })
        .await
    }

    /// The unit one can click on X,Y, the latest to appear there; hidden ones (a cloaked NPC) are passed over, as the
    /// game's picking does. Seen during the steps: warp there with `say:@warp` first
    pub(super) fn npc_at(&self, x: u32, y: u32) -> Result<u32, String> {
        self.units
            .iter()
            .rev()
            .find(|u| {
                (u32::from(u.cell.0), u32::from(u.cell.1)) == (x, y) && !is_hidden(u.effect_state)
                    && !(95_000_000..100_000_000).contains(&u.id) && !(1000..4000).contains(&u.job)
            })
            .map(|u| u.id)
            .ok_or_else(|| format!("no unit on {x},{y}"))
    }

    /// Plays an NPC's dialog to its end as a player who always takes the first answer: Next, the first menu entry,
    /// 1 or "Probe" when asked, Close. Done at the close, or when the NPC says nothing more for `REPLY` seconds
    /// (`close2`, `end`)
    pub(super) async fn answer_dialog(&mut self, zone: &mut Probe) -> Result<usize, String> {
        let pv = zone.packetver;
        let mut answers = 0;
        let choices = std::env::var("NOSTALRE_PROBE_NPC_CHOICES").unwrap_or_default();
        let picked=std::mem::take(&mut self.npc_choices);
        let picked=if picked.is_empty() {choices.split(',').filter_map(|v|v.parse::<u8>().ok()).collect()} else {picked};
        let mut choices=picked.into_iter();
        loop {
            // the answer, and whether it ends the dialog
            let reply = self
                .expect(zone, REPLY, "dialog", |e| match e {
                    GameEvent::NpcDialogNext { npc_id } => {
                        Some((sender::build_npc_next_packet(*npc_id, pv), false))
                    }
                    GameEvent::NpcDialogMenu { npc_id, items } => {
                        let choice = choices.next().unwrap_or(1);
                        println!("NPC menu: {items:?}; choice {choice}");
                        Some((sender::build_npc_menu_select_packet(*npc_id, choice, pv), false))
                    }
                    GameEvent::NpcInputNumber { npc_id } => {
                        Some((sender::build_npc_input_number_packet(*npc_id, 1, pv), false))
                    }
                    GameEvent::NpcInputString { npc_id } => Some((
                        sender::build_npc_input_string_packet(*npc_id, "Probe", pv),
                        false,
                    )),
                    GameEvent::NpcDialogClose { npc_id } => {
                        Some((sender::build_npc_close_packet(*npc_id, pv), true))
                    }
                    _ => None,
                })
                .await;
            match reply {
                Ok((packet, closes)) => {
                    zone.send(packet).await;
                    answers += 1;
                    if closes {
                        return Ok(answers);
                    }
                }
                // quiet: the script ended without a close button
                Err(_) if !zone.closed => return Ok(answers),
                Err(e) => return Err(e),
            }
        }
    }

    /// Talks to a shop NPC and picks a deal, until its list comes
    async fn open_shop(&mut self, zone: &mut Probe, npc: u32, deal: Deal) -> Result<(), String> {
        let pv = zone.packetver;
        let contact = sender::build_contact_npc_packet(npc, pv);
        self.request(zone, contact, REPLY, "deal type choice", |e| {
            matches!(e, GameEvent::NpcDealTypeSelect { npc_id } if *npc_id == npc).then_some(())
        })
        .await?;
        let pick = sender::build_npc_deal_type_packet(npc, deal as u8, pv);
        self.request(zone, pick, REPLY, "shop list", |e| match (deal, e) {
            (Deal::Buy, GameEvent::NpcShopBuyList { .. })
            | (Deal::Sell, GameEvent::NpcShopSellList { .. }) => Some(()),
            _ => None,
        })
        .await
    }

    /// Buys one `item`; the server's answer (0 bought)
    async fn buy(&mut self, zone: &mut Probe, item: u32) -> Result<u8, String> {
        let buy = sender::build_purchase_item_list_packet(&[(1, item)], zone.packetver);
        self.request(zone, buy, REPLY, "buy answer", |e| match e {
            GameEvent::NpcShopBuyResult { result } => Some(*result),
            _ => None,
        })
        .await
    }
}

/// rAthena's 0x00c5 deal types
#[derive(Clone, Copy)]
enum Deal {
    Buy = 0,
    Sell = 1,
}

fn ensure(ok: bool, failure: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(failure.into()) }
}

fn expect_zero(what: &str, result: u8) -> Result<(), String> {
    ensure(result == 0, &format!("{what}: result {result}"))
}

// Collision-aware test driver: walks normal protocol paths and discovers monsters
// only from normal visibility packets. No spawn, warp or quest-progress commands.
struct CampaignNavigation { width:usize,height:usize,cells:Vec<u8>,pending:Vec<(u16,u16)> }
impl CampaignNavigation {
    fn load(quest:u32)->Option<Self> {
        let data=std::fs::read(format!("campaign-walk-{quest}.bin")).ok()?;
        let width=u16::from_le_bytes(data.get(..2)?.try_into().ok()?) as usize;
        let height=u16::from_le_bytes(data.get(2..4)?.try_into().ok()?) as usize;
        if data.len()!=4+width*height {return None;}
        let mut nav=Self {width,height,cells:data[4..].to_vec(),pending:Vec::new()};
        for y in (8..height).step_by(16) {for x in (8..width).step_by(16) {
            if nav.walkable(x,y) {nav.pending.push((x as u16,y as u16));}
        }}
        Some(nav)
    }
    fn walkable(&self,x:usize,y:usize)->bool {x<self.width && y<self.height && matches!(self.cells[y*self.width+x],0|2|3|4|6)}
    fn toward(&self,from:(u16,u16),to:(u16,u16))->Option<(u16,u16)> {
        let start=from.1 as usize*self.width+from.0 as usize;
        let goal=to.1 as usize*self.width+to.0 as usize;
        if !self.walkable(from.0 as usize,from.1 as usize) || !self.walkable(to.0 as usize,to.1 as usize) {return None;}
        let mut previous=vec![usize::MAX;self.cells.len()];previous[start]=start;
        let mut queue=std::collections::VecDeque::from([start]);
        while let Some(cell)=queue.pop_front() {
            if cell==goal {break;}
            let x=cell%self.width;let y=cell/self.width;
            for (nx,ny) in [(x.wrapping_sub(1),y),(x+1,y),(x,y.wrapping_sub(1)),(x,y+1)] {
                if !self.walkable(nx,ny) {continue;}
                let next=ny*self.width+nx;
                if previous[next]==usize::MAX {previous[next]=cell;queue.push_back(next);}
            }
        }
        if previous[goal]==usize::MAX {return None;}
        let mut path=vec![goal];while *path.last()?!=start {path.push(previous[*path.last()?]);}
        let next=path[path.len().saturating_sub(9)];Some(((next%self.width) as u16,(next/self.width) as u16))
    }
    fn explore(&mut self,from:(u16,u16))->Option<(u16,u16)> {
        self.pending.retain(|&(x,y)|x.abs_diff(from.0).max(y.abs_diff(from.1))>4);
        loop {
            let index=self.pending.iter().enumerate().min_by_key(|(_,p)|p.0.abs_diff(from.0) as u32+p.1.abs_diff(from.1) as u32).map(|(i,_)|i)?;
            if let Some(next)=self.toward(from,self.pending[index]) {return Some(next);}
            self.pending.swap_remove(index);
        }
    }
}
