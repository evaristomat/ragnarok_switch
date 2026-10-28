//! Drives one session against a running server without the game: login, character select (optionally creating a
//! character first), map entry, a few steps, then idles. Prints every packet the client could not frame, could not
//! parse, or parsed but does not handle, so a packet version can be checked headless
//! (docs/nostalre-protocol-plan.md, step 3.1).
//!
//! cargo run -p nostalre-network --example session_probe -- [--login 127.0.0.1:6900]
//!     [--user tester] [--pass tester] [--slot 0] [--create NAME [--doram] [--female]] [--seconds 20]
//!     [--warp MAP] [--chat TEXT]... [--equip] [--raise-trait] [--fight]
//!
//! `--warp` sends `@warp MAP` (a GM account) after the walk; `--chat` says TEXT (GM commands too) after the warp, in
//! order; `--equip` wears the first equipment an item pickup reports (say `@item 1201` first); `--raise-trait` raises
//! POW by one with 0x0b24 (a 4th job with trait points: say `@job 4252` and `@traitpoint 10` first); `--fight` spawns
//! a Poring and attacks a monster in reach;
//! `--delete-check SLOT` reserves that slot's character for deletion and cancels it, before selecting `--slot`;
//! `--dump NAME` prints the bytes and events of every packet whose name contains NAME.
//!
//! `--social STEP`, in order, after `--raise-trait` and before `--fight`:
//! - party: `party:NAME` makes one, `invite:NAME`, `party-option[:EXP,PICKUP,SHARE]` (1,1,0 when bare),
//!   `party-chat:TEXT`, `party-leave`;
//! - guild: `guild:NAME` makes one (`@guild`, a GM), `guild-invite:NAME` (a party member), `guild-leave`,
//!   `guild-expel:NAME`, `emblem-upload:BMP` (to the web server, then 0x0b46), `emblem-download`;
//! - others: `whisper:NAME:TEXT` (again until NAME is online), `friend:NAME`, `say:TEXT` (a GM command),
//!   `unequip-all` (takes off everything worn and waits for 0x0bae),
//!   `homun-feed`, `hatch` (`@hatch`, the first egg), `pet-feed`, `drop-all` (`@dropall`), `pickup` (a floor item);
//! - `shop-quit:X,Y,ITEM`: at the shop NPC on cell X,Y (seen during the steps: warp there with `say:@warp` first),
//!   buys one ITEM, then reopens the shop, closes it with 0x09d4 and checks the same buy is refused;
//! - `reuse:ITEM`: gets two of an item with a reuse delay (607, Yggdrasil Berry), uses one (checking it was used)
//!   and the other at once, and waits for the server's "seconds left" (0x07e2);
//! - the live pass (3.6b item 6, `session_probe/play.rs`): `shop-sell:X,Y,ITEM`, `cart:ITEM` (`@cart 1`, in and
//!   out), `storage:ITEM` (`@storage`, in, out, close), `vend:ITEM,PRICE` (a merchant: its cart, a shop, until a
//!   sale) and `vend-buy:VENDOR_AID`, `trade:PARTNER_AID,ITEM` (the partner probe accepts and confirms by itself),
//!   `repair:ITEM` (a Blacksmith, the item damaged by `@item2`), `card:CARD,EQUIP`, `identify:ITEM` (a Magnifier),
//!   `warp-portal:X,Y` (an Acolyte line), `die-respawn`, `merc:SCROLL`, `chatroom-open[:TITLE]` and
//!   `chatroom-join:OWNER_AID`, `cart-load:ITEM` (one left in the cart, for a game run), `refine:X,Y,ITEM` (a
//!   refiner's window, a safe +1), `walk:X,Y`, `talk:X,Y` (the dialog answered with the first choices; the quests it
//!   gave or took), `server-move:X,Y` (onto a warp to the second map server: 0x0ac7 followed);
//! - `wait:SECONDS`, `wait-guild:SECONDS` (until we are in a guild).
//!
//! Party, guild and friend invitations are accepted, and the social events and our option bits (mounts, costumes)
//! are printed, all along.
//!
//! Exits non-zero when the session did not reach the map or any packet was skipped, dropped or left unhandled.
//! scripts/probe.sh runs it on its own account (prober), so it never logs out a game running on tester; an account
//! without characters gets one, Prober.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nostalre_game::event::GameEvent;
use nostalre_network::connection::Connection;
use nostalre_network::emblem_http::{self, WebAuth};
use nostalre_network::handler::dispatch_packet;
use nostalre_network::session::Session;
#[path = "session_probe/play.rs"]
mod play;

use nostalre_network::{
    build_char_enter_packet, build_login_packet, build_map_loaded_packet,
    build_request_move_packet, build_select_char_packet, build_zone_enter_packet, ip_u32_to_string,
    sender,
};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::{Layer, Registry};

#[derive(Default)]
struct Tally {
    handled: BTreeMap<String, usize>,
    unhandled: BTreeMap<String, usize>,
    panicked: BTreeMap<String, usize>,
    /// "skipping unknown packet" / "skipping unparsed packet" / parse panics, as connection.rs logs them
    skipped: BTreeMap<String, usize>,
}

/// Counts the framing and parsing problems `Connection` reports through `tracing`.
struct SkipCounter(Arc<Mutex<Tally>>);

impl<S: tracing::Subscriber> Layer<S> for SkipCounter {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        struct Message(String);
        impl Visit for Message {
            fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.0 = format!("{value:?}");
                }
            }
        }
        let mut message = Message(String::new());
        event.record(&mut message);
        let m = message.0;
        if m.starts_with("skipping") || m.starts_with("packet parse panic") {
            let key: String = m
                .split(" (")
                .next()
                .unwrap_or(&m)
                .split(", buffer")
                .next()
                .unwrap_or(&m)
                .into();
            *self.0.lock().unwrap().skipped.entry(key).or_default() += 1;
        }
    }
}

struct Args {
    packetver: u32,
    login: String,
    user: String,
    pass: String,
    slot: u8,
    create: Option<String>,
    doram: bool,
    female: bool,
    seconds: u64,
    warp: Option<String>,
    chat: Vec<String>,
    equip: bool,
    raise_trait: bool,
    fight: bool,
    dump: Option<String>,
    social: Vec<String>,
    delete_check: Option<u8>,
}

fn args() -> Args {
    let mut a = Args {
        packetver: nostalre_network::rathena_table::PACKETVER,
        login: "127.0.0.1:6900".into(),
        user: "tester".into(),
        pass: "tester".into(),
        slot: 0,
        create: None,
        doram: false,
        female: false,
        seconds: 20,
        warp: None,
        chat: Vec::new(),
        equip: false,
        raise_trait: false,
        fight: false,
        dump: None,
        social: Vec::new(),
        delete_check: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().unwrap_or_else(|| panic!("{flag} needs a value"));
        match flag.as_str() {
            "--login" => a.login = value(),
            "--user" => a.user = value(),
            "--pass" => a.pass = value(),
            "--slot" => a.slot = value().parse().expect("slot"),
            "--create" => a.create = Some(value()),
            "--doram" => a.doram = true,
            "--female" => a.female = true,
            "--seconds" => a.seconds = value().parse().expect("seconds"),
            "--warp" => a.warp = Some(value()),
            "--chat" => a.chat.push(value()),
            "--equip" => a.equip = true,
            "--raise-trait" => a.raise_trait = true,
            "--fight" => a.fight = true,
            "--dump" => a.dump = Some(value()),
            "--social" => a.social.push(value()),
            "--delete-check" => a.delete_check = Some(value().parse().expect("slot")),
            other => panic!("unknown argument {other}"),
        }
    }
    a
}

struct Probe {
    conn: Connection,
    packetver: u32,
    tally: Arc<Mutex<Tally>>,
    dump: Option<String>,
    /// Party members by name, from every list and join since login: the social steps invite them.
    party: BTreeMap<String, u32>,
    /// Our character id and guild, kept the same way (the guild's id comes at login, or when one is joined).
    gid: u32,
    guild: Option<u32>,
    /// The server closed the connection (a second login on the account kicks the first): nothing more comes.
    closed: bool,
    /// When we last sent anything: the server drops a client silent for longer than its stall time
    last_sent: Instant,
}

impl Probe {
    async fn connect(addr: &str, expect_aid: bool, a: &Args, tally: &Arc<Mutex<Tally>>) -> Self {
        let conn = Connection::connect(addr, expect_aid)
            .await
            .unwrap_or_else(|e| panic!("connect {addr}: {e}"));
        Self {
            conn,
            packetver: a.packetver,
            tally: tally.clone(),
            dump: a.dump.clone(),
            party: BTreeMap::new(),
            gid: 0,
            guild: None,
            closed: false,
            last_sent: Instant::now(),
        }
    }

    /// The next map server's connection after a map-server move: the same tally, version and what we know
    async fn follow(&self, addr: &str) -> Self {
        let conn = Connection::connect(addr, true)
            .await
            .unwrap_or_else(|e| panic!("connect {addr}: {e}"));
        Self {
            conn,
            packetver: self.packetver,
            tally: self.tally.clone(),
            dump: self.dump.clone(),
            party: self.party.clone(),
            gid: self.gid,
            guild: self.guild,
            closed: false,
            last_sent: Instant::now(),
        }
    }

    async fn send(&mut self, data: Vec<u8>) {
        if self.closed {
            return;
        }
        self.conn
            .send_packet(&data, self.packetver)
            .await
            .expect("send");
        self.last_sent = Instant::now();
    }

    /// A time request after 10 s of silence, as the game sends one every few seconds: a long wait on the other
    /// probe must not look like a dead client
    async fn keep_alive(&mut self, started: Instant) {
        const QUIET: Duration = Duration::from_secs(10);
        if self.last_sent.elapsed() >= QUIET {
            let tick = started.elapsed().as_millis() as u32;
            self.send(sender::build_request_time_packet(tick, self.packetver))
                .await;
        }
    }

    /// Reads and dispatches packets until `want` picks an event or the time runs out.
    async fn until<T>(
        &mut self,
        within: Duration,
        mut want: impl FnMut(&GameEvent) -> Option<T>,
    ) -> Option<T> {
        let deadline = Instant::now() + within;
        loop {
            if self.closed {
                return None;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            let Ok(read) = tokio::time::timeout(left, self.conn.recv_packets(self.packetver)).await
            else {
                return None;
            };
            let packets = match read {
                Ok(packets) => packets,
                Err(e) => {
                    println!("connection ended: {e:?}");
                    self.closed = true;
                    return None;
                }
            };
            let mut found = None;
            for packet in &packets {
                let name = packet.name().to_string();
                let dispatched = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    dispatch_packet(packet.as_ref())
                }));
                if let (Some(want), Ok(events)) = (&self.dump, &dispatched)
                    && name.contains(want.as_str())
                {
                    let hex: Vec<String> =
                        packet.raw().iter().map(|b| format!("{b:02x}")).collect();
                    println!(
                        "{name} [{}] {}\n  -> {events:?}",
                        packet.raw().len(),
                        hex.join(" ")
                    );
                }
                let mut tally = self.tally.lock().unwrap();
                match dispatched {
                    Err(_) => *tally.panicked.entry(name).or_default() += 1,
                    Ok(events) if events.is_empty() => {
                        *tally.unhandled.entry(name).or_default() += 1
                    }
                    Ok(events) => {
                        *tally.handled.entry(name).or_default() += 1;
                        for event in &events {
                            match event {
                                GameEvent::PartyMemberList { members, .. } => self
                                    .party
                                    .extend(members.iter().map(|m| (m.name.clone(), m.aid))),
                                GameEvent::PartyMemberAdded { name, aid, .. } => {
                                    self.party.insert(name.clone(), *aid);
                                }
                                GameEvent::GuildIdentityUpdated { gdid, .. }
                                | GameEvent::GuildInfo { gdid, .. } => {
                                    self.guild = (*gdid != 0).then_some(*gdid);
                                }
                                GameEvent::GuildMemberLeft { gid, .. }
                                | GameEvent::GuildMemberExpelled { gid, .. }
                                    if *gid == self.gid =>
                                {
                                    self.guild = None;
                                }
                                _ => {}
                            }
                            if found.is_none() {
                                found = want(event);
                            }
                        }
                    }
                }
            }
            if found.is_some() {
                return found;
            }
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let a = args();
    let tally = Arc::new(Mutex::new(Tally::default()));
    let filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into());
    let subscriber = Registry::default()
        .with(tracing_subscriber::fmt::layer().with_filter(filter))
        .with(SkipCounter(tally.clone()));
    tracing::subscriber::set_global_default(subscriber).expect("tracing");
    let wait = Duration::from_secs(10);
    let pv = a.packetver;

    // login server
    let mut login = Probe::connect(&a.login, false, &a, &tally).await;
    login.send(build_login_packet(&a.user, &a.pass, pv)).await;
    let accepted = login
        .until(wait, |e| match e {
            GameEvent::LoginAccepted {
                account_id,
                login_id1,
                login_id2,
                sex,
                servers,
                web_auth_token,
            } => Some(Ok((
                *account_id,
                *login_id1,
                *login_id2,
                *sex,
                servers.clone(),
                web_auth_token.clone(),
            ))),
            GameEvent::LoginRefused { error_code } => Some(Err(*error_code)),
            _ => None,
        })
        .await;
    let (account_id, login_id1, login_id2, sex, servers, web_auth_token) = match accepted {
        Some(Ok(v)) => v,
        Some(Err(code)) => return finish(&tally, &format!("login refused ({code})")),
        None => return finish(&tally, "no login reply"),
    };
    println!(
        "login: account {account_id}, {} char server(s)",
        servers.len()
    );
    let mut session = Session::new(pv);
    session.store_login(account_id, login_id1, login_id2, sex);

    // char server
    let Some(server) = servers.first() else {
        return finish(&tally, "no char server");
    };
    let char_addr = format!("{}:{}", ip_u32_to_string(server.ip), server.port);
    let mut char = Probe::connect(&char_addr, true, &a, &tally).await;
    char.send(build_char_enter_packet(&session)).await;
    let Some(characters) = char
        .until(wait, |e| match e {
            GameEvent::CharacterListReceived { characters } => Some(characters.clone()),
            _ => None,
        })
        .await
    else {
        return finish(&tally, "no character list");
    };
    for c in &characters {
        println!(
            "  slot {} {} class {} lv {} on {}",
            c.slot, c.name, c.class, c.base_level, c.map
        );
    }
    if let Some(check) = a.delete_check {
        let Some(c) = characters.iter().find(|c| c.slot == check as i8) else {
            return finish(
                &tally,
                &format!("no character in slot {check} to reserve for deletion"),
            );
        };
        if let Err(e) = delete_reserve_and_cancel(&mut char, c.gid, wait).await {
            return finish(&tally, &e);
        }
        println!("delete-check: {} reserved for deletion, then kept", c.name);
    }
    let mut slot = a.slot;
    let mut char_name = characters
        .iter()
        .find(|c| c.slot == slot as i8)
        .map(|c| c.name.clone());
    // an account without characters (a fresh prober) gets one
    let create = a
        .create
        .clone()
        .or_else(|| characters.is_empty().then(|| "Prober".to_string()));
    if let Some(name) = &create {
        slot = (0..15)
            .find(|s| !characters.iter().any(|c| c.slot == *s as i8))
            .expect("a free slot");
        let job = if a.doram { 4218 } else { 0 };
        // the account's sex unless asked otherwise: the server takes the packet's from 20151001
        let char_sex = if a.female { 0 } else { sex };
        char.send(sender::build_make_char_packet(
            name, slot, 1, 0, job, char_sex, pv,
        ))
        .await;
        match char
            .until(wait, |e| match e {
                GameEvent::CharacterCreated { character } => Some(Ok(character.clone())),
                GameEvent::CharacterCreateFailed { error_code } => Some(Err(*error_code)),
                _ => None,
            })
            .await
        {
            Some(Ok(c)) => {
                println!(
                    "created {} in slot {} (class {}, sex {})",
                    c.name, c.slot, c.class, c.sex
                );
                char_name = Some(c.name);
            }
            Some(Err(code)) => return finish(&tally, &format!("creation refused ({code})")),
            None => return finish(&tally, "no creation reply"),
        }
    }
    char.send(build_select_char_packet(slot, pv)).await;
    let Some((char_id, map, ip, port)) = char
        .until(wait, |e| match e {
            GameEvent::ZoneServerConnectInfo {
                char_id,
                map_name,
                ip,
                port,
            } => Some((*char_id, map_name.clone(), *ip, *port)),
            _ => None,
        })
        .await
    else {
        return finish(&tally, "no map server handoff");
    };
    println!("select slot {slot}: char {char_id} on {map}");
    session.store_zone_info(char_id, map);

    // map server
    let mut zone = Probe::connect(
        &format!("{}:{}", ip_u32_to_string(ip), port),
        true,
        &a,
        &tally,
    )
    .await;
    zone.send(build_zone_enter_packet(&session)).await;
    let Some((x, y)) = zone
        .until(wait, |e| match e {
            GameEvent::MapEntered { x, y, .. } => Some((*x, *y)),
            _ => None,
        })
        .await
    else {
        return finish(&tally, "no map entry");
    };
    println!("entered at {x},{y}");
    zone.send(build_map_loaded_packet(pv)).await;
    zone.until(Duration::from_secs(2), |_| None::<()>).await;
    // a short walk; rAthena refuses one onto a wall without a word, so other directions are tried
    let mut moved = None;
    for (dx, dy) in [(2, 2), (-2, -2), (2, -2), (-2, 2)] {
        let (Some(tx), Some(ty)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) else {
            continue;
        };
        zone.send(build_request_move_packet(tx, ty, pv)).await;
        moved = zone
            .until(Duration::from_secs(2), |e| {
                matches!(e, GameEvent::PlayerMoved { .. }).then_some(())
            })
            .await;
        if moved.is_some() {
            break;
        }
    }
    println!(
        "walk: {}",
        if moved.is_some() {
            "moved"
        } else {
            "no move reply"
        }
    );
    let (mut px, mut py) = (x, y);
    if let Some(map) = &a.warp {
        let name = char_name.as_deref().unwrap_or("");
        zone.send(chat(name, &format!("@warp {map}"), pv)).await;
        let changed = zone
            .until(wait, |e| match e {
                GameEvent::MapChanged { map_name, x, y } => Some(format!("{map_name} {x},{y}")),
                _ => None,
            })
            .await;
        let Some(changed) = changed else {
            return finish(&tally, "the warp got no map change");
        };
        println!("warped to {changed}");
        if let Some((cx, cy)) = changed.rsplit(' ').next().and_then(|c| c.split_once(',')) {
            (px, py) = (cx.parse().unwrap_or(px), cy.parse().unwrap_or(py));
        }
        zone.send(build_map_loaded_packet(pv)).await;
    }
    for text in &a.chat {
        let name = char_name.as_deref().unwrap_or("");
        zone.send(chat(name, text, pv)).await;
        println!("said {text}");
    }
    if a.equip {
        let picked = zone
            .until(wait, |e| match e {
                GameEvent::InventoryItemPickup {
                    index, location, ..
                } if *location != 0 => Some((*index, *location)),
                _ => None,
            })
            .await;
        let Some((index, location)) = picked else {
            return finish(&tally, "no equipment picked up to wear");
        };
        zone.send(sender::build_equip_item_packet(index, location, pv))
            .await;
        let worn = zone
            .until(wait, |e| match e {
                GameEvent::InventoryEquipResult {
                    index,
                    wear_location,
                    success,
                    ..
                } => Some(format!(
                    "index {index} at {wear_location:#x}, success {success}"
                )),
                _ => None,
            })
            .await;
        match worn {
            Some(worn) => println!("equip: {worn}"),
            None => return finish(&tally, "the equip request got no reply"),
        }
    }
    if a.raise_trait {
        const POW: u16 = 219;
        zone.send(sender::build_trait_stat_change_packet(POW, 1, pv))
            .await;
        // the server answers 0x00bc with the new value, or refuses it (no event)
        let raised = zone
            .until(wait, |e| match e {
                GameEvent::ParameterChanged { var_id: POW, value } => Some(*value),
                _ => None,
            })
            .await;
        match raised {
            Some(pow) => println!("trait: POW raised to {pow}"),
            None => return finish(&tally, "POW was not raised (refused, or no reply)"),
        }
    }
    zone.gid = char_id;
    // rAthena's web server beside the login server, for the emblem steps
    let web = WebAuth {
        addr: format!(
            "{}:8888",
            a.login.rsplit_once(':').map_or("127.0.0.1", |(h, _)| h)
        ),
        token: web_auth_token,
        aid: account_id,
        world: server.name.clone(),
    };
    let mut social = Social {
        aid: account_id,
        zone_enter: build_zone_enter_packet(&session),
        web,
        guild_members: BTreeMap::new(),
        whisper_ack: None,
        unequip_all: None,
        floor_items: Vec::new(),
        eggs: Vec::new(),
        units: Vec::new(),
        quests: Vec::new(),
        trade_partner: false,
        vendors: BTreeSet::new(),
        rooms: BTreeMap::new(),
    };
    for step in &a.social {
        if let Err(e) = social
            .step(&mut zone, step, char_name.as_deref().unwrap_or(""))
            .await
        {
            return finish(&tally, &format!("social step {step}: {e}"));
        }
    }
    let mut target = None;
    if a.fight {
        // a Poring next to the character, so there is something in reach
        let name = char_name.as_deref().unwrap_or("");
        zone.send(chat(name, "@monster 1002", pv)).await;
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(a.seconds) {
        zone.send(sender::build_request_time_packet(
            start.elapsed().as_millis() as u32,
            pv,
        ))
        .await;
        let monster = zone
            .until(Duration::from_secs(5), |e| match e {
                // a monster within reach: rAthena drops an attack whose walk there is too long
                GameEvent::EntitySpawned { gid, job, x, y, .. }
                    if a.fight
                        && (1000..4000).contains(job)
                        && x.abs_diff(px).max(y.abs_diff(py)) <= 8 =>
                {
                    Some(*gid)
                }
                _ => None,
            })
            .await;
        if let (None, Some(gid)) = (target, monster) {
            println!("attacking monster {gid}");
            zone.send(sender::build_action_request_packet(gid, 7, pv))
                .await;
            target = Some(gid);
        }
    }
    finish(
        &tally,
        if moved.is_some() {
            ""
        } else {
            "the walk got no reply"
        },
    );
}

/// What the social steps know (our guild's members, the last whisper ack, floor items, eggs); invitations are
/// accepted as they come.
struct Social {
    aid: u32,
    /// The map server's login (0x0436), the same for the next one after a map-server move
    zone_enter: Vec<u8>,
    web: WebAuth,
    guild_members: BTreeMap<String, (u32, u32)>,
    whisper_ack: Option<u8>,
    /// The answer to `unequip-all`: refused or not
    unequip_all: Option<bool>,
    /// Floor items in sight, for `pickup`.
    floor_items: Vec<u32>,
    eggs: Vec<u16>,
    /// Units in sight since the steps began, the latest last: for the shop and talk steps
    units: Vec<Unit>,
    /// Quests added (+id) and removed (-id), in order, for the quest steps
    quests: Vec<i64>,
    /// Asked to trade: the trade is confirmed as soon as the other side confirms it
    trade_partner: bool,
    /// Shops open in sight, by their vendor's account, and chat rooms by their owner's: they can come before the
    /// step that wants them
    vendors: BTreeSet<u32>,
    rooms: BTreeMap<u32, u32>,
}

/// A timed deletion reserved (0x0827, answered by 0x0828) and cancelled (0x082b, 0x082c): the character stays
async fn delete_reserve_and_cancel(
    char: &mut Probe,
    gid: u32,
    wait: Duration,
) -> Result<(), String> {
    const DELETE_OK: u32 = 1;
    let pv = char.packetver;
    char.send(sender::build_delete_char_reserve_packet(gid, pv))
        .await;
    let reserved = char
        .until(wait, |e| match e {
            GameEvent::CharacterDeleteReserved {
                result,
                delete_reserved_date,
                ..
            } => Some((*result, *delete_reserved_date)),
            _ => None,
        })
        .await
        .ok_or("no answer to the deletion reserve")?;
    if reserved.0 != DELETE_OK {
        return Err(format!("deletion reserve refused ({})", reserved.0));
    }
    println!("delete-check: reserved, {} s to wait", reserved.1);
    char.send(sender::build_delete_char_cancel_packet(gid, pv))
        .await;
    let cancelled = char
        .until(wait, |e| match e {
            GameEvent::CharacterDeleteCancelled { result, .. } => Some(*result),
            _ => None,
        })
        .await
        .ok_or("no answer to the deletion cancel")?;
    if cancelled != DELETE_OK {
        return Err(format!("deletion cancel refused ({cancelled})"));
    }
    Ok(())
}

/// A unit in sight: its id, cell and option bits (a cloaked NPC shares its cell with the one shown)
struct Unit {
    id: u32,
    cell: (u16, u16),
    effect_state: i32,
}

/// rAthena's trade answer (0x00e6 sent, 0x01f5 back): accepted
const TRADE_ACCEPTED: u8 = 3;
/// 0x00ec: the partner, not us, pressed OK
const PARTNER: u8 = 1;

/// "Name : text", a chat line as the game sends it.
fn said(me: &str, text: &str) -> String {
    format!("{me} : {text}")
}

/// A public chat line (GM commands are said this way).
fn chat(me: &str, text: &str, pv: u32) -> Vec<u8> {
    sender::build_chat_packet(&said(me, text), pv)
}

/// `why`, unless the server closed the connection first.
fn closed_or(zone: &Probe, why: &str) -> String {
    if zone.closed {
        "the connection is closed".into()
    } else {
        why.into()
    }
}

fn print_social(e: &GameEvent) {
    let text = format!("{e:?}");
    const SHOWN: [&str; 8] = [
        "Whisper",
        "Party",
        "Guild",
        "Friend",
        "EntityGuild",
        "Homun",
        "FloorItem",
        "Pet",
    ];
    if SHOWN.iter().any(|p| text.starts_with(p)) {
        println!("  event {text}");
    }
}

impl Social {
    async fn step(&mut self, zone: &mut Probe, step: &str, me: &str) -> Result<(), String> {
        let pv = zone.packetver;
        if zone.closed {
            return Err("the connection is closed".into());
        }
        let (verb, arg) = step.split_once(':').unwrap_or((step, ""));
        let packet = match verb {
            "wait" => {
                let secs = arg.parse().map_err(|_| "seconds")?;
                self.idle(zone, Duration::from_secs_f32(secs)).await;
                None
            }
            "wait-guild" => {
                let secs = arg.parse().map_err(|_| "seconds")?;
                let joined = self
                    .idle_until(zone, Duration::from_secs_f32(secs), |_, z| {
                        z.guild.is_some()
                    })
                    .await;
                return joined
                    .then_some(())
                    .ok_or_else(|| closed_or(zone, "no guild joined"));
            }
            "party" => Some(sender::build_make_party2_packet(arg, 1, 0, pv)),
            "invite" => Some(sender::build_party_invite_by_name_packet(arg, pv)),
            "party-leave" => Some(sender::build_leave_party_packet(pv)),
            "party-option" => {
                // EXP,PICKUP,SHARE (1,1,0 when empty): experience sharing, then the two item rules
                let rules: Vec<u32> = arg.split(',').filter_map(|v| v.parse().ok()).collect();
                let (exp, pickup, share) = match rules[..] {
                    [e, p, s] => (e, p as u8, s as u8),
                    _ => (1, 1, 0),
                };
                Some(sender::build_change_party_option_packet(
                    exp, pickup, share, pv,
                ))
            }
            "party-chat" => Some(sender::build_party_chat_packet(&said(me, arg), pv)),
            "whisper" => {
                // again until the target is online (ack 0, 1 while it is not), for a game still starting
                let (to, text) = arg.split_once(':').ok_or("whisper:NAME:TEXT")?;
                let deadline = Instant::now() + Duration::from_secs(240);
                while Instant::now() < deadline && !zone.closed {
                    self.whisper_ack = None;
                    zone.send(sender::build_whisper_packet(to, text, pv)).await;
                    self.idle_until(zone, Duration::from_secs(5), |s, _| s.whisper_ack.is_some())
                        .await;
                    if self.whisper_ack == Some(0) {
                        println!("social: {step}");
                        return Ok(());
                    }
                    self.idle(zone, Duration::from_secs(2)).await;
                }
                return Err(closed_or(zone, &format!("{to} never took the whisper")));
            }
            "friend" => Some(sender::build_add_friend_packet(arg, pv)),
            "say" => Some(chat(me, arg, pv)),
            "unequip-all" => {
                // everything worn, costumes too (0x0bf5's mask from 20230906)
                self.unequip_all = None;
                zone.send(sender::build_unequip_all_packet(u32::MAX, pv))
                    .await;
                self.idle_until(zone, Duration::from_secs(3), |s, _| s.unequip_all.is_some())
                    .await;
                return match self.unequip_all {
                    Some(false) => Ok(()),
                    Some(true) => Err("unequip-all refused".into()),
                    None => Err(closed_or(zone, "no answer to unequip-all")),
                };
            }
            "pet-feed" => Some(sender::build_command_pet_packet(1, pv)),
            "hatch" => {
                // `@hatch` opens the egg list; the first egg in it hatches
                self.eggs.clear();
                zone.send(chat(me, "@hatch", pv)).await;
                self.idle_until(zone, Duration::from_secs(3), |s, _| !s.eggs.is_empty())
                    .await;
                let egg = self.eggs.first().copied().ok_or("no egg to hatch")?;
                Some(sender::build_select_petegg_packet(egg, pv))
            }
            "homun-feed" => Some(sender::build_homun_menu_packet(1, pv)),
            "drop-all" => Some(chat(me, "@dropall", pv)),
            "pickup" => {
                let item = self.floor_items.pop().ok_or("no item on the floor")?;
                Some(sender::build_pickup_item_packet(item, pv))
            }
            "guild" => Some(chat(me, &format!("@guild {arg}"), pv)),
            "guild-invite" => {
                let aid = *zone.party.get(arg).ok_or("not a party member")?;
                Some(sender::build_req_join_guild(aid, self.aid, zone.gid, pv))
            }
            "guild-leave" => {
                let gdid = zone.guild.ok_or("not in a guild")?;
                Some(sender::build_req_leave_guild(
                    gdid,
                    self.aid as i32,
                    zone.gid as i32,
                    "bye",
                    pv,
                ))
            }
            "emblem-upload" => {
                let gdid = zone.guild.ok_or("not in a guild")?;
                let bmp = std::fs::read(arg).map_err(|e| format!("{arg}: {e}"))?;
                let version = emblem_http::upload(&self.web, gdid, &bmp)?;
                println!("social: the web server stored the emblem as version {version}");
                Some(sender::build_add_new_emblem_packet(gdid, version, pv))
            }
            "emblem-download" => {
                let gdid = zone.guild.ok_or("not in a guild")?;
                let image = emblem_http::download(&self.web, gdid)?;
                let magic = String::from_utf8_lossy(&image[..image.len().min(2)]).into_owned();
                println!(
                    "social: guild {gdid}'s emblem from the web server: {} bytes, {magic}",
                    image.len()
                );
                None
            }
            "guild-expel" => {
                let gdid = zone.guild.ok_or("not in a guild")?;
                // the member list as it is now: it came when we joined
                zone.send(sender::build_req_guild_menu(1, pv)).await;
                self.idle(zone, Duration::from_secs(2)).await;
                let (aid, gid) = *self.guild_members.get(arg).ok_or("not a guild member")?;
                Some(sender::build_req_ban_guild(
                    gdid, aid as i32, gid as i32, "expelled", pv,
                ))
            }
            _ => return self.play(zone, step, me).await,
        };
        if let Some(packet) = packet {
            zone.send(packet).await;
            println!("social: {step}");
            self.idle(zone, Duration::from_secs(2)).await;
        }
        Ok(())
    }

    /// Reads packets for `time`, keeping what the steps need and answering invitations.
    async fn idle(&mut self, zone: &mut Probe, time: Duration) {
        self.idle_until(zone, time, |_, _| false).await;
    }

    /// Reads packets until `done` or `within` runs out (or the connection closes); whether `done` came.
    async fn idle_until(
        &mut self,
        zone: &mut Probe,
        within: Duration,
        done: impl Fn(&Self, &Probe) -> bool,
    ) -> bool {
        let deadline = Instant::now() + within;
        while !done(self, zone) {
            if zone.closed || Instant::now() >= deadline {
                return false;
            }
            zone.keep_alive(deadline - within).await;
            self.pump(zone, |_| None::<()>).await;
        }
        true
    }

    /// One read of the server: every event noted (what the steps need kept, invitations answered) and offered to
    /// `want`, which picks the first it is after
    async fn pump<T>(
        &mut self,
        zone: &mut Probe,
        mut want: impl FnMut(&GameEvent) -> Option<T>,
    ) -> Option<T> {
        let pv = zone.packetver;
        let mut replies = Vec::new();
        let found = zone
            .until(Duration::from_millis(300), |e| {
                print_social(e);
                replies.extend(self.note(e, pv));
                want(e)
            })
            .await;
        for (what, packet) in replies {
            println!("social: answering with the {what}");
            zone.send(packet).await;
        }
        found
    }

    /// Keeps what the steps need from `e`; the answer to an invitation, or to a guild we just made or joined.
    fn note(&mut self, e: &GameEvent, pv: u32) -> Option<(&'static str, Vec<u8>)> {
        match e {
            GameEvent::GuildIdentityUpdated { gdid, .. } if *gdid != 0 => {
                // its info and member list come on request, as the game asks on map entry
                let menus = [0, 1]
                    .iter()
                    .flat_map(|t| sender::build_req_guild_menu(*t, pv))
                    .collect();
                return Some(("guild menus", menus));
            }
            GameEvent::GuildMembers { members } => self
                .guild_members
                .extend(members.iter().map(|m| (m.name.clone(), (m.aid, m.gid)))),
            GameEvent::GuildMemberLeft { gid, .. } | GameEvent::GuildMemberExpelled { gid, .. } => {
                self.guild_members.retain(|_, (_, g)| g != gid)
            }
            GameEvent::WhisperAck { result } => self.whisper_ack = Some(*result),
            GameEvent::UnequipAllAnswered { refused } => self.unequip_all = Some(*refused),
            GameEvent::MapChanged { .. } => {
                self.units.clear();
                self.vendors.clear();
                self.rooms.clear();
                return Some(("map loaded", build_map_loaded_packet(pv)));
            }
            GameEvent::VendingBoardShown { aid, .. } => {
                self.vendors.insert(*aid);
            }
            GameEvent::VendingBoardHidden { aid } => {
                self.vendors.remove(aid);
            }
            GameEvent::ChatRoomUpsert {
                owner_aid, room_id, ..
            } => {
                self.rooms.insert(*owner_aid, *room_id);
            }
            GameEvent::EntitySpawned {
                gid,
                x,
                y,
                effect_state,
                ..
            } => {
                self.units.retain(|u| u.id != *gid);
                self.units.push(Unit {
                    id: *gid,
                    cell: (*x, *y),
                    effect_state: *effect_state,
                });
            }
            GameEvent::EntityOptionChanged {
                gid, effect_state, ..
            } => {
                if *gid == self.aid {
                    println!("option {effect_state:#x}");
                }
                if let Some(unit) = self.units.iter_mut().find(|u| u.id == *gid) {
                    unit.effect_state = *effect_state;
                }
            }
            GameEvent::EntityVanished { gid, .. } => self.units.retain(|u| u.id != *gid),
            GameEvent::QuestAdded { quest } => self.quests.push(i64::from(quest.id)),
            GameEvent::QuestRemoved { quest_id } => self.quests.push(-i64::from(*quest_id)),
            GameEvent::ExchangeRequested { name, .. } => {
                println!("social: {name} asks to trade");
                self.trade_partner = true;
                return Some((
                    "trade request",
                    sender::build_ack_exchange_item_packet(TRADE_ACCEPTED, pv),
                ));
            }
            GameEvent::ExchangeConcluded { who: PARTNER } if self.trade_partner => {
                let confirm = [
                    sender::build_conclude_exchange_item_packet(pv),
                    sender::build_exec_exchange_item_packet(pv),
                ]
                .concat();
                return Some(("trade OK", confirm));
            }
            GameEvent::ExchangeCompleted { .. } | GameEvent::ExchangeCanceled => {
                self.trade_partner = false
            }
            GameEvent::FloorItemAppeared { id, .. } => self.floor_items.push(*id),
            GameEvent::FloorItemDisappeared { id } => self.floor_items.retain(|i| i != id),
            GameEvent::PetEggList { indices } => self.eggs = indices.clone(),
            GameEvent::PartyInviteReceived { party_grid, .. } => {
                let reply = sender::build_join_party_reply_packet(*party_grid, true, pv);
                return Some(("party invite", reply));
            }
            GameEvent::GuildInviteReceived { gdid, .. } => {
                return Some((
                    "guild invite",
                    sender::build_ans_join_guild(*gdid, true, pv),
                ));
            }
            GameEvent::FriendRequestReceived {
                req_aid, req_gid, ..
            } => {
                let reply = sender::build_ack_add_friend_packet(*req_aid, *req_gid, true, pv);
                return Some(("friend request", reply));
            }
            _ => {}
        }
        None
    }
}

/// Prints the tally; exits non-zero on `failure` or on any packet that was not framed, parsed and handled.
fn finish(tally: &Arc<Mutex<Tally>>, failure: &str) {
    let t = tally.lock().unwrap();
    let list = |title: &str, m: &BTreeMap<String, usize>| {
        if !m.is_empty() {
            println!("{title}:");
            for (k, n) in m {
                println!("  {k} x{n}");
            }
        }
    };
    list("handled", &t.handled);
    list("UNHANDLED (parsed, no event)", &t.unhandled);
    list("PANICKED in dispatch", &t.panicked);
    list("SKIPPED by framing or parsing", &t.skipped);
    let clean = failure.is_empty()
        && t.unhandled.is_empty()
        && t.panicked.is_empty()
        && t.skipped.is_empty();
    if !failure.is_empty() {
        println!("FAILED: {failure}");
    }
    println!("{}", if clean { "clean" } else { "not clean" });
    std::process::exit(if clean { 0 } else { 1 });
}
