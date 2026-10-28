//! Test-only scenarios through the actual App, normalized pad input and offline servers.
//! Proposal checks describe requested future behavior; a failure is not a shipped regression.
use std::{collections::{HashMap, VecDeque}, time::{Duration, Instant}};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use winit::event_loop::ActiveEventLoop;
use crate::{App, NetworkCommand};
use crate::input::gamepad::Button;
use models::enums::skill_enums::SkillEnum;
use nostalre_game::{app_state::AppState, entity::{EntityState, EntityType}, event::GameEvent, hotkey::HotkeySlotContent};

enum Step {
    Act(fn(&mut Probe, &mut App, &ActiveEventLoop)),
    Wait(f32),
    Until(&'static str, fn(&Probe, &App) -> bool, f32),
    Check(&'static str, &'static str, fn(&Probe, &App) -> bool),
}

#[derive(Clone, Default)]
struct Counts { skills: u32, moves: u32, attacks: u32, normal_hits: u32, damage: HashMap<u32,u32>, nodamage: HashMap<u32,u32>, ground: u32, failures: Vec<u8> }

pub(crate) struct Probe {
    pub finished: bool,
    started: bool,
    steps: VecDeque<Step>, wake: Option<Instant>, deadline: Option<Instant>,
    script: Option<crate::script::Script>,
    outgoing: Option<mpsc::UnboundedReceiver<NetworkCommand>>,
    forward: Option<mpsc::UnboundedSender<NetworkCommand>>,
    count: Counts, base: Counts, sp: u32, hp: u32, position: (u16,u16), target: Option<u32>,
    findings: Vec<Value>, frames_ms: Vec<f64>, frame_started: Option<Instant>,
    input_at: Option<Instant>, command_latencies_ms: Vec<f64>,
    camera: (f32,f32), cursor: (f64,f64), vendor: Option<(u32,u32,i16,i32)>,
    potion_count: i32, zeny: i32, seen_town_chat: bool, town_survey: usize,
}

fn button(app: &mut App, b: Button, down: bool) { app.gamepad.validation_button(b, down); }
fn neutral(app: &mut App) {
    app.gamepad.axis(0,0.0); app.gamepad.axis(1,0.0);
    app.gamepad.axis(11,0.0); app.gamepad.axis(14,0.0);
    for b in [Button::L1,Button::R1,Button::L2,Button::R2,Button::R3,Button::A,Button::B,Button::X,Button::Y,Button::Right] { button(app,b,false); }
}
fn advance_tutorial_dialog(_: &mut Probe, app:&mut App, el:&ActiveEventLoop) {
    use nostalre_game::npc_dialog::NpcDialogState;
    let dialog=&mut app.windows.npc_dialog.dialog;
    let npc_id=dialog.npc_id;
    let event=if dialog.state==NpcDialogState::WaitingForMenu {
        dialog.close_menu();Some(GameEvent::RequestNpcMenuSelect{npc_id,choice:1})
    } else if dialog.next_button {
        dialog.advance_next();Some(GameEvent::RequestNpcNext{npc_id})
    } else if dialog.close_button {
        dialog.close();Some(GameEvent::RequestNpcClose{npc_id})
    } else {None};
    if let Some(event)=event {app.handle_ui_events(vec![event],el);}
}
fn advance_job_changer(p: &mut Probe, app: &mut App, el: &ActiveEventLoop) {
    if app.windows.npc_dialog.dialog.text.contains("pontos de habilidade")
        || app.game.world.entities.player().is_some_and(|e| e.job == 1) {
        return;
    }
    advance_tutorial_dialog(p, app, el);
}
fn monsters(app:&App)->Vec<u32> { app.game.world.entities.iter().filter(|e| e.entity_type==EntityType::Monster && e.is_alive() && !e.is_pet).map(|e|e.id).collect() }
fn survey_town_vendors(p:&mut Probe, app:&mut App, _: &ActiveEventLoop) {
    if app.game.world.entities.iter().any(|e|e.vending_board.is_some()) {return;}
    const POINTS:[(u16,u16);9]=[(130,170),(156,170),(182,170),(130,194),(156,194),(182,194),(130,218),(156,218),(182,218)];
    if let Some((x,y))=POINTS.get(p.town_survey) {app.run_chat_command(&format!("@warp prontera {x} {y}"));}
    p.town_survey+=1;
}
fn request(app:&mut App, el:&ActiveEventLoop, skill:SkillEnum) { app.handle_ui_events(vec![GameEvent::RequestUseSkill{skill,level:1}],el); }
fn survey_town_chat(p:&mut Probe, app:&mut App, _: &ActiveEventLoop) {
    p.seen_town_chat |= app.game.chat_rooms.iter().next().is_some();
    if p.seen_town_chat { return; }
    // Room announcements are spatial: survey the plaza instead of requiring
    // a randomly placed room to be visible from one fixed test coordinate.
    const POINTS: [(u16,u16);9]=[(130,170),(156,170),(182,170),(130,194),(156,194),(182,194),(130,218),(156,218),(182,218)];
    if let Some((x,y))=POINTS.get(p.town_survey) {app.run_chat_command(&format!("@warp prontera {x} {y}"));}
    p.town_survey+=1;
}
fn mark(p:&mut Probe, app:&mut App, _: &ActiveEventLoop) { p.base=p.count.clone(); p.sp=app.game.character.sp; p.hp=app.game.character.hp; p.position=app.game.world.entities.player().unwrap().movement.cell_position(); p.target=app.game.combat.attack_target_id; }
fn prepare_bash(p:&mut Probe,app:&mut App,el:&ActiveEventLoop) {
    app.stop_attacking(); mark(p,app,el);
    request(app,el,SkillEnum::SmBash); live_bash(app);
}
fn live_bash(app:&mut App) {
    let Some(from)=app.game.world.entities.player().map(|e|e.movement.cell_position()) else {return;};
    let target=app.game.world.entities.iter().filter(|e|e.entity_type==EntityType::Monster&&e.is_alive()&&!e.is_pet)
        .min_by_key(|e|{let p=e.movement.cell_position();from.0.abs_diff(p.0).max(from.1.abs_diff(p.1))}).map(|e|e.id);
    if let Some(id)=target {app.game.pending_casts.pending_skill_target=None;app.cast_skill_on_unit(SkillEnum::SmBash,1,id);}
}

impl Probe {
    pub fn from_env()->Option<Self> {
        std::env::var_os("NOSTALRE_GAMEPLAY_VALIDATION")?;
        Some(Self{finished:false,started:false,steps:VecDeque::new(),wake:None,deadline:None,script:None,outgoing:None,forward:None,count:Counts::default(),base:Counts::default(),sp:0,hp:0,position:(0,0),target:None,findings:vec![],frames_ms:vec![],frame_started:None,input_at:None,command_latencies_ms:vec![],camera:(0.0,0.0),cursor:(0.0,0.0),vendor:None,potion_count:0,zeny:0,seen_town_chat:false,town_survey:0})
    }
    fn record(&mut self,name:&str,kind:&str,passed:bool,app:&App) {
        let row=json!({"name":name,"kind":kind,"passed":passed,"sp":app.game.character.sp,"hp":app.game.character.hp,"before_sp":self.sp,"before_hp":self.hp,
            "skill_requests_since_mark":self.count.skills-self.base.skills,"move_requests_since_mark":self.count.moves-self.base.moves,
            "normal_damage_events":self.count.normal_hits,"skill_damage_events":self.count.damage,"skill_success_events":self.count.nodamage,
            "ground_events":self.count.ground,"server_failure_causes":self.count.failures,
            "target":app.game.combat.attack_target_id,"selected_target":app.game.combat.selected_target_id,
            "camera":app.renderer.as_ref().map(|r|(r.camera.yaw,r.camera.pitch)),"cursor":app.input.mouse_position,
            "ui_blockers":{"menu":app.windows.menu.is_open(),"system":app.windows.system_menu.open,
                "item":app.windows.item_info_window.is_open(),"vendor":app.windows.vending_shop_window.is_open(),
                "vendor_setup":app.windows.vending_setup_window.is_open(),"npc":app.windows.npc_dialog.dialog.is_open(),
                "shop":app.windows.npc_shop.shop.is_open(),"quests":app.windows.quest_window.is_open(),
                "detail":app.windows.quest_detail_window.is_open(),"chat":app.windows.chat_window.active,
                "confirm":app.windows.confirm_dialog.state.is_some()},
            "position":app.game.world.entities.player().map(|e|e.movement.cell_position())});
        println!("GAMEPLAY-CHECK {row}"); self.findings.push(row);
    }
    fn build_steps()->VecDeque<Step> {
        if std::env::var_os("NOSTALRE_CLASSIC_PORT_VALIDATION").is_some() {
            return Self::classic_port_steps();
        }
        if std::env::var_os("NOSTALRE_QUEST_FLOW_VALIDATION").is_some() {
            return Self::quest_flow_steps();
        }
        if std::env::var_os("NOSTALRE_QUEST_GUIDE_VALIDATION").is_some() {
            return Self::quest_guide_steps();
        }
        if std::env::var_os("NOSTALRE_NPC_PORTAL_VALIDATION").is_some() {
            return Self::npc_portal_steps();
        }
        if std::env::var_os("NOSTALRE_IZLUDE_PORTAL_VALIDATION").is_some() {
            return Self::portal_steps();
        }
        use Step::*;
        let mut steps=vec![
            Act(|_,a,_| {a.run_chat_command("@warp prontera 150 180");}), Wait(1.0),
            Act(|_,a,_| {for c in ["@killmonster","@jobchange 23","@lvup 49","@int 50","@vit 30","@str 5","@dex 20","@allskill","@heal"] {a.run_chat_command(c);} }),
            Until("server granted test skills",|_,a| [SkillEnum::SmBash,SkillEnum::SmEndure,SkillEnum::MgColdbolt,SkillEnum::MgFirewall,SkillEnum::AlHeal].iter().all(|s|a.game.character.skills.skills().iter().any(|x|x.skill==*s && x.level>0)),6.0), Wait(0.5),
            Act(|_,a,_| {
                for (slot,skill) in [(1,SkillEnum::SmEndure),(3,SkillEnum::SmBash),(9,SkillEnum::AlHeal)] { a.game.character.hotkeys.set_slot(slot,HotkeySlotContent::Skill{skill,level:1}); }
            }),
            Act(mark), Act(|_,a,_| {a.gamepad.axis(0,0.08);a.gamepad.axis(1,-0.08);}), Wait(0.35),
            Check("small stick drift sends no walk", "regression",|p,_|p.count.moves==p.base.moves),
            Act(|_,a,_|neutral(a)), Act(mark),
            Act(|_,a,_|button(a,Button::L1,true)), Wait(0.05),
            Act(|p,a,_|{p.input_at=Some(Instant::now());button(a,Button::X,true);}), Wait(0.1), Act(|_,a,_|neutral(a)),
            Until("Endure acknowledged and cooldown received",|p,a|p.count.nodamage.get(&8).unwrap_or(&0)>p.base.nodamage.get(&8).unwrap_or(&0) && a.skill_on_cooldown(SkillEnum::SmEndure),3.0),
            Check("L then X casts self skill and spends SP", "regression",|p,a|a.game.character.sp<p.sp && p.count.skills>p.base.skills),
            Act(mark), Act(|_,a,el|request(a,el,SkillEnum::SmEndure)), Wait(0.15),
            Check("cooldown suppresses duplicate self skill", "regression",|p,_|p.count.skills==p.base.skills),
            Until("Endure cooldown nearly expires",|_,a| {let r=a.game.character.cooldowns.remaining_secs(SkillEnum::SmEndure,a.start_time.elapsed().as_secs_f32());r>0.02&&r<0.18},10.5),
            Act(mark), Act(|_,a,el|request(a,el,SkillEnum::SmEndure)), Wait(0.55),
            Check("one early press executes after cooldown", "proposal",|p,_|p.count.skills==p.base.skills+1),
            Act(|_,a,el|request(a,el,SkillEnum::SmEndure)), Wait(0.25),
            Check("fresh press after cooldown casts", "regression",|p,_|p.count.skills>p.base.skills),
            Act(|_,a,_| {a.stop_attacking(); neutral(a);}), Wait(0.1), Act(mark),
            Act(|_,a,_| {button(a,Button::R1,true);button(a,Button::X,true);}), Wait(0.15),
            Check("simultaneous R and X selects mapped Heal", "proposal",|_,a|matches!(a.game.pending_casts.pending_skill_target,Some(nostalre_game::cursor::PendingSkillTarget::Entity{skill:SkillEnum::AlHeal,..}))),
            Act(|_,a,_|neutral(a)), Act(|_,a,el|request(a,el,SkillEnum::MgColdbolt)), Wait(0.1),
            Check("offensive skill opens target selection", "regression",|_,a|a.game.pending_casts.pending_skill_target.is_some()),
            Act(|_,a,_|button(a,Button::B,true)), Wait(0.1), Act(|_,a,_|neutral(a)),
            Check("B cancels skill targeting", "regression",|_,a|a.game.pending_casts.pending_skill_target.is_none()&&a.game.pending_casts.pending_skill.is_none()),
            Act(|_,a,_|a.run_chat_command("@monster PUPA 2")),
            Until("two living targets spawned",|_,a|monsters(a).len()>=2,3.0), Act(mark),
            Act(|p,a,_| {p.input_at=Some(Instant::now());button(a,Button::X,true);}), Wait(0.15), Act(|_,a,_|neutral(a)),
            Until("X causes real normal attack damage",|p,_|p.count.normal_hits>p.base.normal_hits,5.0),
            Act(|p,a,_| {let nearest=a.game.combat.attack_target_id;let other=monsters(a).into_iter().find(|id|Some(*id)!=nearest);a.stop_attacking();if let Some(other)=other {a.initiate_attack(other);}p.target=other;}),
            Act(|_,a,_|button(a,Button::X,true)), Wait(0.1), Act(|_,a,_|neutral(a)),
            Check("X preserves explicitly selected living target", "proposal",|p,a|p.target.is_some()&&a.game.combat.attack_target_id==p.target),
            Act(mark), Act(|_,a,_|button(a,Button::Right,true)), Wait(0.1), Act(|_,a,_|neutral(a)),
            Check("D-pad cycles combat target", "proposal",|p,a|p.target.is_some()&&a.game.combat.attack_target_id.is_some()&&a.game.combat.attack_target_id!=p.target),
            Act(mark), Act(|_,a,_|{a.gamepad.axis(0,0.11);a.gamepad.axis(1,0.0);}), Wait(0.15), Act(|_,a,_|neutral(a)),
            Check("tiny above-threshold tilt preserves attack", "proposal",|p,a|p.target.is_some()&&a.game.combat.attack_target_id==p.target),
            Act(|_,a,_|{a.stop_attacking();a.run_chat_command("@killmonster");a.run_chat_command("@monster PUPA 1");a.run_chat_command("@heal");}),
            Until("fresh living Bash target exists",|_,a|!monsters(a).is_empty(),3.0),Wait(0.25),Act(mark),
            Act(|_,a,_|button(a,Button::L1,true)),Wait(0.05),Act(|_,a,_|button(a,Button::A,true)),Wait(0.15),Act(|_,a,_|neutral(a)),
            Act(|_,a,_|live_bash(a)),
            Until("Bash damage acknowledged",|p,_|p.count.damage.get(&5).unwrap_or(&0)>p.base.damage.get(&5).unwrap_or(&0),4.0),
            Check("offensive hotbar skill spends SP", "regression",|p,a|a.game.character.sp<p.sp),
            Wait(1.2),Act(|_,a,_|{a.stop_attacking();a.run_chat_command("@killmonster");a.run_chat_command("@monster PUPA 1");}),
            Until("fresh target for SP refusal exists",|_,a|!monsters(a).is_empty(),3.0),Wait(0.25),
            Act(|_,a,_|a.run_chat_command("@heal 0 -999999")),
            Until("SP drained by server",|_,a|a.game.character.sp<8,2.0), Act(prepare_bash),
            Until("insufficient SP refusal received",|p,_|p.count.failures.len()>p.base.failures.len()&&p.count.failures[p.base.failures.len()..].contains(&1),3.0),
            Check("SP refusal clears pending entity cast", "regression",|_,a|a.game.pending_casts.pending_skill_target.is_none()&&a.game.pending_casts.pending_skill.is_none()),
            Act(|_,a,_|a.run_chat_command("@heal")),Wait(0.15),Act(prepare_bash),
            Until("Bash works after SP recovery",|p,_|p.count.damage.get(&5).unwrap_or(&0)>p.base.damage.get(&5).unwrap_or(&0),4.0),
            Act(|_,a,_|{a.stop_attacking();a.run_chat_command("@killmonster");a.run_chat_command("@heal");a.run_chat_command("@heal -100 0");}),
            Until("server confirmed injury before Heal",|_,a|a.game.character.hp+50<a.game.character.max_hp,3.0),Act(mark),
            Act(|_,a,el|{request(a,el,SkillEnum::AlHeal);a.validation_click_skill();}),
            Until("Heal acknowledged on player",|p,_|p.count.nodamage.get(&28).unwrap_or(&0)>p.base.nodamage.get(&28).unwrap_or(&0),3.0),
            Check("support skill restores HP and spends SP", "regression",|p,a|a.game.character.hp>p.hp&&a.game.character.sp<p.sp),
            Wait(0.6), Act(mark), Act(|_,a,el|{request(a,el,SkillEnum::MgFirewall);a.validation_click_skill();}),
            Until("ground skill appears in real world",|p,_|p.count.ground>p.base.ground,5.0),
            Check("ground skill spends SP", "regression",|p,a|a.game.character.sp<p.sp),
            Wait(1.0), Act(|_,a,el|{request(a,el,SkillEnum::MgColdbolt);a.run_chat_command("@warp izlude_c 122 100");}),
            Until("warp to Izlude completed",|_,a|a.game.session.current_map.as_deref()==Some("izlude_c"),10.0),Wait(0.1),
            Check("warp clears previous target and pending skills", "regression",|_,a|a.game.combat.attack_target_id.is_none()&&a.game.pending_casts.pending_skill_target.is_none()&&a.game.pending_casts.pending_skill.is_none()&&a.game.pending_casts.pending_ground_cast.is_none()),
            Act(|_,a,_|a.run_chat_command("@kill")),
            Until("server confirmed player death",|_,a|a.game.character.hp==0&&a.game.world.entities.player().is_some_and(|e|e.state()==EntityState::Dead),3.0),Act(mark),
            Act(|_,a,_|{a.gamepad.axis(0,1.0);}),Wait(0.35),Act(|_,a,_|neutral(a)),
            Check("dead avatar cannot move in world", "regression",|p,a|a.game.world.entities.player().unwrap().movement.cell_position()==p.position),
            Check("dead pad movement sends no useless request", "regression",|p,_|p.count.moves==p.base.moves),
            Act(mark),Act(|_,a,el|request(a,el,SkillEnum::SmEndure)),Wait(0.2),
            Check("dead avatar sends no skill request", "regression",|p,_|p.count.skills==p.base.skills),
            Act(|_,a,_|a.run_chat_command("@alive")),
            Until("server confirmed resurrection",|_,a|a.game.character.hp>0&&a.game.world.entities.player().is_some_and(|e|e.state()!=EntityState::Dead),3.0),
            Act(mark), Act(|_,a,_|{let yaw=a.renderer.as_ref().unwrap().camera.yaw;a.gamepad.axis(0,f64::from(yaw.cos()));a.gamepad.axis(1,f64::from(yaw.sin()));}),Wait(0.45),Act(|_,a,_|neutral(a)),Wait(0.5),
            Check("movement recovers after resurrection", "regression",|p,a|a.game.world.entities.player().unwrap().movement.cell_position()!=p.position),
            Act(|_,a,_|{neutral(a);a.stop_attacking();a.run_chat_command("@heal");a.run_chat_command("@warp prontera 150 180");}),
            Until("return to Prontera for stress",|_,a|a.game.session.current_map.as_deref()==Some("prontera"),10.0),Wait(0.5),
            Act(|_,a,_|{a.windows.menu.open();}),Wait(0.1),Act(mark),
            Act(|_,a,_|a.gamepad.axis(0,1.0)),Wait(0.3),Act(|_,a,_|neutral(a)),
            Check("left stick cannot walk under menu", "regression",|p,a|p.count.moves==p.base.moves&&a.game.world.entities.player().unwrap().movement.cell_position()==p.position),
            Act(|_,a,_|a.windows.menu.close()),Wait(0.4),
            Act(|_,a,_|{a.run_chat_command("@killmonster");a.run_chat_command("@monster RED_PLANT 8");a.run_chat_command("@heal");}),
            Until("durable stationary stress targets spawned",|_,a|monsters(a).len()>=8,3.0),
            Act(mark),Act(|_,a,el|{request(a,el,SkillEnum::MgColdbolt);a.validation_click_skill();}),
            Until("Cold Bolt cast and damage received",|p,_|p.count.damage.get(&14).unwrap_or(&0)>p.base.damage.get(&14).unwrap_or(&0),6.0),
            Check("Cold Bolt consumes SP", "regression",|p,a|a.game.character.sp<p.sp),Wait(1.2),
        ];
        for _ in 0..8 {
            steps.extend([
                Act(|_,a,_|{a.stop_attacking();a.run_chat_command("@heal");}),Wait(0.1),Act(mark),
                Act(|_,a,el|{request(a,el,SkillEnum::MgColdbolt);a.validation_click_skill();}),
                Until("repeated Cold Bolt resolves",|p,_|p.count.damage.get(&14).unwrap_or(&0)>p.base.damage.get(&14).unwrap_or(&0),6.0),Wait(1.2),
            ]);
        }
        steps.extend([
            Act(|_,a,_|{a.stop_attacking();a.run_chat_command("@heal");}),Wait(0.4),Act(mark),
            Act(|_,a,el|{request(a,el,SkillEnum::MgColdbolt);a.validation_click_skill();}),
            Until("cast in progress before death",|_,a|a.game.world.entities.player().is_some_and(|e|e.state()==EntityState::Casting),3.0),
            Act(|_,a,_|a.run_chat_command("@kill")),
            Until("death during cast acknowledged",|_,a|a.game.character.hp==0&&a.game.session.player_dead,3.0),Wait(2.0),
            Check("death cancels in-flight cast without damage", "regression",|p,_|p.count.damage.get(&14).unwrap_or(&0)==p.base.damage.get(&14).unwrap_or(&0)),
            Act(|_,a,_|a.run_chat_command("@alive")),
            Until("resurrected after interrupted cast",|_,a|a.game.character.hp>0&&!a.game.session.player_dead,3.0),Wait(0.2),Act(mark),
            Act(|_,a,el|{request(a,el,SkillEnum::MgColdbolt);a.validation_click_skill();}),
            Until("Cold Bolt recovers after death interruption",|p,_|p.count.damage.get(&14).unwrap_or(&0)>p.base.damage.get(&14).unwrap_or(&0),6.0),Wait(1.2),
            Act(|_,a,_|{a.stop_attacking();a.run_chat_command("@killmonster");}),Wait(0.3),
            Act(|_,a,el|{request(a,el,SkillEnum::SmBash);}),Wait(0.1),
            Check("no target leaves an explicit cancellable selector", "regression",|_,a|a.game.pending_casts.pending_skill_target.is_some()&&a.game.combat.attack_target_id.is_none()),
            Act(|_,a,_|button(a,Button::B,true)),Wait(0.1),Act(|_,a,_|neutral(a)),
            Act(|_,a,_|{a.run_chat_command("@heal");a.run_chat_command("@warp int_land03 81 104");}),
            Until("return to ship completed",|_,a|a.game.session.current_map.as_deref()==Some("int_land03"),10.0),Wait(0.2),
        ]);
        steps.into()
    }
    fn classic_port_steps()->VecDeque<Step> {
        use Step::*;
        let mut steps = vec![
            Act(|_,a,_|a.run_chat_command("@populate off")),Wait(1.0),
            Check("saved Classic configuration recovers our controller HUD", "regression",|_,a|a.config.hud_layout==crate::config::HudLayout::Modern&&a.windows.hud.is_modern()),
            Until("classic Shion name arrives without hover or dialogue",|_,a|a.game.world.entities.iter().any(|e|e.name.as_deref()==Some("Shion")),8.0),
            Check("classic Shion has server yellow quest marker", "regression",|_,a|a.game.quest_markers.values().any(|m|m.x==53&&m.y==114&&m.color==1)),
            Check("classic novice objective is yellow on minimap", "regression",|_,a|a.windows.minimap_window.entity_markers.iter().any(|m|m.x==53.0&&m.y==114.0&&matches!(m.marker_type,nostalre_ui_component::game::minimap_window::MarkerType::Square([1.0,0.85,0.1])))),
            Act(|_,_,_|println!("script shot CLASSIC-NPC-NAMES")),Wait(1.0),
            Act(|_,a,_|a.run_chat_command("@setquest 1001")),
            Until("classic acolyte journal received via protocol",|_,a|a.game.quest_log.get(1001).is_some(),4.0),
            Check("classic journal uses PT-BR text", "regression",|_,a|a.game.data_table.quest_display.as_ref().is_some_and(|t|t.title(1001)=="Mudança de Classe: Noviço"&&t.summary(1001).contains("você"))),
            Act(|_,a,_|a.run_chat_command("@setquest 1002")),
            Until("second classic quest arrives via protocol",|_,a|a.game.quest_log.get(1002).is_some(),4.0),
            Check("campaign keeps priority when a side quest is accepted", "regression",|_,a|a.game.quest_log.followed().is_some_and(|q|(900000..900100).contains(&q.id))),
            Act(|_,a,el|{assert!(a.game.quest_log.follow(1001));a.handle_ui_events(vec![GameEvent::OpenQuestDetail{quest_id:1001}],el);}),Wait(0.4),
            Check("manual quest choice has priority", "regression",|_,a|a.game.quest_log.followed().is_some_and(|q|q.id==1001)),
            Act(|_,_,_|println!("script shot CLASSIC-JOURNAL-PTBR")),Wait(1.0),
            Act(|_,a,_|button(a,Button::B,true)),Wait(0.1),Act(|_,a,_|button(a,Button::B,false)),Wait(0.1),
            Act(|_,a,_|{a.run_chat_command("@erasequest 1001");a.run_chat_command("@erasequest 1002");}),
            Until("classic fixture quests removed",|_,a|a.game.quest_log.get(1001).is_none()&&a.game.quest_log.get(1002).is_none(),4.0),
            Check("no Renewal academy destinations were imported", "regression",|_,_|nostalre_game::quest_guidance::for_quest(&Default::default(),5153).is_none()&&nostalre_game::quest_guidance::novice_hint("iz_ac01",0,1).is_none()),
        ];
        steps.extend(Self::menu_job_changer_steps());
        steps.extend(Self::infinite_wing_steps());
        steps.extend(Self::classic_pre5_steps());
        steps.into()
    }

    fn classic_pre5_steps()->Vec<Step> {
        use Step::*;
        let mut steps=vec![
            Check("actual GRF item names are Portuguese", "regression",|_,a|a.game.data_table.item_name_for(501,true)=="Poção Vermelha"&&a.game.data_table.item_name_for(1201,true)=="Faca"),
            Act(|_,a,_|a.run_chat_command("@warp new_1-1 142 112")),Wait(0.7),
            Act(|_,a,_|a.game.combat.queued_move=Some((148,112))),
            Until("novice exit uses actual classic reception portal",|_,a|a.game.session.current_map.as_deref()==Some("new_1-2"),8.0),
            Act(|_,a,_|a.game.combat.queued_move=Some((100,26))),
            Until("walking reaches classic receptionist",|_,a|a.game.world.entities.player().is_some_and(|e|e.movement.cell_position()==(100,26)),8.0),
            Act(|_,a,el|{let id=a.game.world.entities.iter().find(|e|e.entity_type==EntityType::Npc&&e.movement.cell_position()==(100,29)).unwrap().id;a.handle_ui_events(vec![GameEvent::RequestNpcContact{npc_id:id}],el);}),
            Until("classic receptionist answers at correct position",|_,a|a.windows.npc_dialog.dialog.say_visible,4.0),
            Act(|_,_,_|println!("script shot CLASSIC-RECEPTION")),Wait(0.5),
            Act(|_,a,el|{let npc_id=a.windows.npc_dialog.dialog.npc_id;a.windows.npc_dialog.dialog.close();a.handle_ui_events(vec![GameEvent::RequestNpcClose{npc_id}],el);}),Wait(0.3),
        ];
        for _ in 0..3 { steps.extend([
            Act(|_,a,_|{a.run_chat_command("@where");a.run_chat_command("@go 5");}),
            Until("go Izlude arrives at classic center",|_,a|a.game.session.current_map.as_deref()==Some("izlude")&&a.game.world.entities.player().is_some_and(|e|e.movement.cell_position()==(128,114)),8.0),
            Act(|_,a,_|a.run_chat_command("@warp izlude 52 136")),Wait(0.4),
            Act(|_,a,_|a.game.combat.queued_move=Some((52,140))),
            Until("classic Swordman guild entrance portal works",|_,a|a.game.session.current_map.as_deref()==Some("izlude_in"),8.0),Wait(0.2),
            Act(|_,a,_|a.game.combat.queued_move=Some((74,158))),
            Until("classic Swordman guild return portal works",|_,a|a.game.session.current_map.as_deref()==Some("izlude"),8.0),Wait(0.3),Act(mark),
            Act(|_,a,_|{let yaw=a.renderer.as_ref().unwrap().camera.yaw;a.gamepad.axis(0,f64::from(yaw.cos()));a.gamepad.axis(1,f64::from(yaw.sin()));}),Wait(0.4),Act(|_,a,_|neutral(a)),Wait(0.3),
            Check("Izlude portal return permits real controller movement", "regression",|p,a|a.game.world.entities.player().unwrap().movement.cell_position()!=p.position),
        ]); }
        steps.extend([
            Act(|_,a,_|a.run_chat_command("@warp prontera 150 180")),Wait(1.0),
            Act(|p,a,_|{let c=&a.renderer.as_ref().unwrap().camera;p.camera=(c.yaw,c.pitch);p.cursor=a.input.mouse_position;a.gamepad.axis(11,0.8);a.gamepad.axis(14,0.6);}),Wait(0.4),Act(|_,a,_|neutral(a)),
            Check("right stick rotates and tilts camera without moving cursor", "regression",|p,a|{let c=&a.renderer.as_ref().unwrap().camera;(c.yaw-p.camera.0).abs()>0.01&&(c.pitch-p.camera.1).abs()>0.01&&a.input.mouse_position==p.cursor}),
            Act(|_,a,_|{for c in ["@killmonster","@jobchange 23","@lvup 49","@allskill","@heal","@monster PUPA 2"] {a.run_chat_command(c);}}),
            Until("two nearby living monsters available",|_,a|monsters(a).len()>=2,5.0),Wait(1.5),
            Act(|_,a,_|button(a,Button::R2,true)),Wait(0.1),Act(|_,a,_|neutral(a)),Act(|p,a,el|{mark(p,a,el);p.target=a.game.combat.selected_target_id;p.camera=(a.renderer.as_ref().unwrap().camera.yaw,a.renderer.as_ref().unwrap().camera.pitch);}),
            Check("ZR selects monster without starting attack", "regression",|p,a|p.target.is_some()&&a.game.combat.attack_target_id.is_none()),
            Act(|_,a,_|button(a,Button::R2,true)),Wait(0.1),Act(|_,a,_|neutral(a)),
            Check("ZR cycles monsters without rotating camera", "regression",|p,a|a.game.combat.selected_target_id.is_some()&&a.game.combat.selected_target_id!=p.target&&(a.renderer.as_ref().unwrap().camera.yaw-p.camera.0).abs()<0.01),
            Act(|_,a,_|button(a,Button::L2,true)),Wait(0.1),Act(|_,a,_|neutral(a)),
            Check("ZL cycles back to previous selected monster", "regression",|p,a|a.game.combat.selected_target_id==p.target),
            Act(|_,_,_|println!("script shot CLASSIC-CONTROLLER-TARGET")),Wait(0.5),
            Act(|_,a,_|{a.game.character.hotkeys.set_slot(3,HotkeySlotContent::Skill{skill:SkillEnum::SmBash,level:1});}),Act(mark),
            Act(|_,a,_|{button(a,Button::L1,true);button(a,Button::A,true);}),Wait(0.15),Act(|_,a,_|neutral(a)),
            Until("controller offensive skill damages selected monster",|p,_|p.count.damage.get(&5).unwrap_or(&0)>p.base.damage.get(&5).unwrap_or(&0),7.0),
            Check("controller offensive skill preserves SP", "regression",|p,a|a.game.character.sp==p.sp),
            Check("server advertises zero SP on offensive skill", "regression",|_,a|a.game.character.skills.get_skill(SkillEnum::SmBash).is_some_and(|s|s.sp_cost==0)),
            // A second cast needs a live target and must wait for the first cast's delay.
            Act(|_,a,_|{a.stop_attacking();a.run_chat_command("@monster PUPA 1");}),Wait(1.5),
            Act(|_,a,_|{a.game.combat.selected_target_id=None;button(a,Button::R2,true);}),Wait(0.1),Act(|_,a,_|neutral(a)),
            Act(|_,a,_|{a.stop_attacking();a.run_chat_command("@heal 0 -100000");}),
            Until("SP damage can still reduce actual SP to zero",|_,a|a.game.character.sp==0,2.0),Act(mark),
            Act(|_,a,_|{button(a,Button::L1,true);button(a,Button::A,true);}),Wait(0.15),Act(|_,a,_|neutral(a)),
            Until("controller offensive skill works with zero SP",|p,_|p.count.damage.get(&5).unwrap_or(&0)>p.base.damage.get(&5).unwrap_or(&0),7.0),
            Act(|_,a,_|{a.stop_attacking();a.run_chat_command("@heal");a.game.character.hotkeys.set_slot(3,HotkeySlotContent::Skill{skill:SkillEnum::MgFirewall,level:1});}),Wait(0.3),Act(mark),
            Act(|_,a,_|{button(a,Button::L1,true);button(a,Button::A,true);}),Wait(0.15),Act(|_,a,_|neutral(a)),Wait(0.15),
            Check("ground skill waits for deliberate confirmation", "regression",|p,a|matches!(a.game.pending_casts.pending_skill_target,Some(nostalre_game::cursor::PendingSkillTarget::Ground{skill:SkillEnum::MgFirewall,..}))&&p.count.ground==p.base.ground),
            Act(|p,a,_|{p.camera=(a.renderer.as_ref().unwrap().camera.yaw,a.renderer.as_ref().unwrap().camera.pitch);p.cursor=a.input.mouse_position;a.gamepad.axis(11,0.2);}),Wait(0.15),Act(|_,a,_|neutral(a)),
            Check("right stick aims ground spell while camera stays still", "regression",|p,a|a.input.mouse_position!=p.cursor&&a.renderer.as_ref().unwrap().camera.yaw==p.camera.0),
            Act(|_,_,_|println!("script shot CLASSIC-GROUND-TARGET")),Wait(0.5),
            Act(|_,a,_|button(a,Button::A,true)),Wait(0.1),Act(|_,a,_|neutral(a)),
            Until("A confirms a real ground spell on server",|p,_|p.count.ground>p.base.ground,5.0),
            Check("ground skill preserves SP", "regression",|p,a|a.game.character.sp==p.sp),
            Act(|_,a,_|{a.run_chat_command("@heal");a.game.character.hotkeys.set_slot(9,HotkeySlotContent::Skill{skill:SkillEnum::AlHeal,level:1});}),Wait(0.3),
            Act(|_,a,_|{button(a,Button::R1,true);button(a,Button::X,true);}),Wait(0.15),Act(|_,a,_|neutral(a)),
            Check("support spell retains manual ally selection", "regression",|_,a|matches!(a.game.pending_casts.pending_skill_target,Some(nostalre_game::cursor::PendingSkillTarget::Entity{skill:SkillEnum::AlHeal,..}))),
            Act(|_,a,_|button(a,Button::B,true)),Wait(0.1),Act(|_,a,_|neutral(a)),
            Check("B cancels support spell selection", "regression",|_,a|a.game.pending_casts.pending_skill_target.is_none()),
            Act(|_,a,_|{a.stop_attacking();a.run_chat_command("@heal");}),Wait(0.3),Act(mark),
            Act(|_,a,el|request(a,el,SkillEnum::TfHiding)),
            Until("Hiding activates on actual server",|p,_|p.count.nodamage.get(&51).unwrap_or(&0)>p.base.nodamage.get(&51).unwrap_or(&0),3.0),Wait(6.0),
            Check("maintained Hiding preserves SP", "regression",|p,a|a.game.character.sp==p.sp),
            Act(|_,a,el|request(a,el,SkillEnum::TfHiding)),Wait(0.5),
            // Isolate the long Asura cast from the earlier ground spells and plaza counter.
            Act(|_,a,_|a.run_chat_command("@warp prontera 182 170")),Wait(1.0),
            Act(|_,a,_|{for c in ["@killmonster","@jobchange 15","@allskill","@heal","@monster PUPA 1"] {a.run_chat_command(c);}}),Wait(1.0),
            Act(|_,a,_|{a.run_chat_command("@spiritball 5");a.run_chat_command("@useskill 270 5 self");}),Wait(1.0),
            Act(|_,a,_|a.run_chat_command("@spiritball 5")),Wait(0.3),Act(mark),
            Act(|_,a,_|{let id=monsters(a).into_iter().next().unwrap();a.cast_skill_on_unit(SkillEnum::MoExtremityfist,1,id);}),
            Until("Asura Strike damages actual monster",|p,_|p.count.damage.get(&271).unwrap_or(&0)>p.base.damage.get(&271).unwrap_or(&0),8.0),
            Check("Asura Strike preserves SP", "regression",|p,a|a.game.character.sp==p.sp),
            // Wait for each reset before teleporting: commands sent while the map
            // changes can be discarded by the server's loading-state guard.
            Act(|_,a,_|{a.stop_attacking();a.run_chat_command("@killmonster");a.run_chat_command("@jobchange 0");}),
            Until("combat fixture returns to novice class",|_,a|a.game.world.entities.player().is_some_and(|e|e.job==0),4.0),
            Act(|_,a,_|{a.run_chat_command("@lvup -999");a.run_chat_command("@joblvup -999");a.run_chat_command("@skillpoint -9999");}),
            Until("combat fixture resets Base and Job levels",|_,a|a.game.character.base_level==1&&a.game.character.job_level==1,4.0),
            Act(|_,a,_|a.run_chat_command("@warp prontera 149 178")),Wait(1.2),
            Act(|_,a,_|a.run_chat_command("@zeny 10000")),Wait(0.3),
            Until("merchant fixture has purchase money",|_,a|a.game.character.inventory.zeny>=10000,3.0),
            Act(|_,a,_|a.run_chat_command("@populate on")),Wait(24.0),
            // Shop announcements are spatial, like the conversation rooms checked below.
            Act(|p,_,_|p.town_survey=0),
            Act(survey_town_vendors),Wait(0.5),Act(survey_town_vendors),Wait(0.5),Act(survey_town_vendors),Wait(0.5),
            Act(survey_town_vendors),Wait(0.5),Act(survey_town_vendors),Wait(0.5),Act(survey_town_vendors),Wait(0.5),
            Act(survey_town_vendors),Wait(0.5),Act(survey_town_vendors),Wait(0.5),Act(survey_town_vendors),Wait(0.5),
            Until("native merchant boards visible in town",|_,a|a.game.world.entities.iter().any(|e|e.vending_board.is_some()),5.0),
            Check("ambient bots have no HP bars even for GM", "regression",|_,a|a.game.world.entities.iter().filter(|e|e.entity_type==EntityType::Player&&(95_000_000..100_000_000).contains(&e.id)).all(|e|a.entity_hp_ratio(e.id).is_none())),
            Check("town actors face different directions", "regression",|_,a|a.game.world.entities.iter().filter(|e|e.entity_type==EntityType::Player&&(95_000_000..100_000_000).contains(&e.id)).map(|e|e.direction).collect::<std::collections::HashSet<_>>().len()>=3),
            Check("shop titles use ordinary market wording", "regression",|_,a|a.game.world.entities.iter().filter_map(|e|e.vending_board.as_deref()).all(|s|!s.contains("aventura")&&!s.contains("jornada")&&!s.contains("viagem"))),
            // Only a subset of the randomly placed shops is inside packet sight range.
            // Check every visible title exactly; raw-packet tests cover both encodings.
            Check("shop titles match Portuguese catalog after packet decoding", "regression",|_,a|a.game.world.entities.iter().any(|e|e.vending_board.is_some())&&a.game.world.entities.iter().filter_map(|e|e.vending_board.as_deref()).all(|s|["Poções","Poções e asas","Consumíveis","Flechas e poções","Utilidades","Asas e lupas","Suprimentos","Poções / flechas"].contains(&s))),

            Act(|p,a,_|{let from=a.game.world.entities.player().unwrap().movement.cell_position();p.target=a.game.world.entities.iter().filter(|e|e.vending_board.is_some()).min_by_key(|e|{let pos=e.movement.cell_position();from.0.abs_diff(pos.0).max(from.1.abs_diff(pos.1))}).map(|e|e.id);p.vendor=None;}),
            Act(|p,a,el|a.handle_ui_events(vec![GameEvent::RequestBuyFromVendor{aid:p.target.unwrap()}],el)),
            Until("bot opens real vending shop with potion stock",|p,a|p.vendor.is_some()&&a.windows.vending_shop_window.is_open(),5.0),
            Act(|_,_,_|println!("script shot CLASSIC-MERCHANT-SHOP")),Wait(0.5),
            Act(|p,a,el|{p.potion_count=a.game.character.inventory.count_of(501);p.zeny=a.game.character.inventory.zeny;if let Some((aid,unique_id,index,_))=p.vendor {a.handle_ui_events(vec![GameEvent::RequestPurchaseFromVendor{aid,unique_id,items:vec![(1,index)]}],el);}}),
            Until("real bot purchase adds one potion to inventory",|p,a|a.game.character.inventory.count_of(501)==p.potion_count+1,5.0),
            Check("real bot purchase deducts advertised zeny price", "regression",|p,a|p.vendor.is_some_and(|v|a.game.character.inventory.zeny==p.zeny-v.3)),
            Act(|_,a,_|a.windows.vending_shop_window.close()),
            Act(survey_town_chat),Wait(0.8),
            Act(survey_town_chat),Wait(0.8),
            Act(survey_town_chat),Wait(0.8),
            Act(survey_town_chat),Wait(0.8),
            Act(survey_town_chat),Wait(0.8),
            Act(survey_town_chat),Wait(0.8),
            Act(survey_town_chat),Wait(0.8),
            Act(survey_town_chat),Wait(0.8),
            Act(survey_town_chat),Wait(0.8),
            Check("native bot conversation rooms visible", "regression",|p,a|p.seen_town_chat||a.game.chat_rooms.iter().next().is_some()),
            Act(|_,a,_|{a.windows.vending_shop_window.close();a.run_chat_command("@populate off");a.run_chat_command("@warp new_1-1 53 111");}),
            Until("pre5 scenarios restore novice fixture",|_,a|a.game.session.current_map.as_deref()==Some("new_1-1")&&a.game.character.base_level==1,8.0),
        ]);
        steps
    }

    fn infinite_wing_steps()->Vec<Step> {
        use Step::*;
        let mut steps=vec![
            Until("existing character receives one infinite fly wing on login",|_,a|a.game.character.inventory.count_of(12887)==1,4.0),
            Check("infinite wing has Portuguese inventory name", "regression",|_,a|a.game.data_table.item_name_for(12887,true)=="Asa de Mosca Infinita"),
            Check("infinite wing describes no expiration or consumption", "regression",|_,a|a.game.data_table.item_description.as_ref().and_then(|t|t.get(12887,true)).is_some_and(|lines|lines.iter().any(|l|l.contains("não expira")))),
            Act(|_,a,el|{let index=a.game.character.inventory.find_by_item_id(12887).unwrap().index;a.handle_ui_events(vec![GameEvent::ShowItemInfo{index}],el);}),Wait(0.3),
            Act(|_,_,_|println!("script shot CLASSIC-INFINITE-WING")),Wait(1.0),
            Act(|_,a,_|button(a,Button::B,true)),Wait(0.1),Act(|_,a,_|button(a,Button::B,false)),Wait(0.2),
            Check("B closes item description and restores world controls", "regression",|_,a|!a.windows.item_info_window.is_open()),
            Act(|_,a,_|a.run_chat_command("@warp prt_fild08 170 180")),
            Until("field loaded for actual wing item use",|_,a|a.game.session.current_map.as_deref()==Some("prt_fild08")&&a.game.world.entities.player().is_some_and(|e|e.movement.cell_position()==(170,180)),10.0),Wait(0.3),
        ];
        for _ in 0..3 {
            steps.extend([
                Act(mark),
                Act(|_,a,el|{let index=a.game.character.inventory.find_by_item_id(12887).unwrap().index;a.handle_ui_events(vec![GameEvent::RequestUseItem{index}],el);}),
                Until("infinite wing actually teleports on same map",|p,a|a.game.session.current_map.as_deref()==Some("prt_fild08")&&a.game.world.entities.player().is_some_and(|e|e.movement.cell_position()!=p.position),5.0),
                Until("infinite wing remains after real item use",|_,a|a.game.character.inventory.count_of(12887)==1,4.0),Wait(0.4),
            ]);
        }
        steps.extend([
            Act(|_,a,_|a.run_chat_command("@warp new_1-1 53 111")),
            Until("wing test returns to classic novice route",|_,a|a.game.session.current_map.as_deref()==Some("new_1-1")&&a.game.world.entities.player().is_some_and(|e|e.movement.cell_position()==(53,111)),10.0),
        ]);
        steps
    }

    fn menu_job_changer_steps()->Vec<Step> {
        use Step::*;
        let mut steps = vec![
            Act(mark),
            Act(|_,a,_|{a.windows.menu=Default::default();a.windows.menu.open();}),Wait(0.2),
        ];
        // Navigate the real menu through normalized controller input.
        for _ in 0..11 {
            steps.extend([Act(|_,a,_|button(a,Button::Down,true)),Wait(0.05),
                          Act(|_,a,_|button(a,Button::Down,false)),Wait(0.05)]);
        }
        steps.extend([
            Act(|_,_,_|println!("script shot CLASSIC-MENU-JOBCHANGER")),Wait(1.0),
            Act(|_,a,_|button(a,Button::A,true)),Wait(0.1),Act(|_,a,_|button(a,Button::A,false)),Wait(0.2),
            Until("menu Job Changer opens actual remote NPC dialog",|_,a|a.windows.npc_dialog.dialog.say_visible&&a.windows.npc_dialog.dialog.text.contains("Mestre das Classes"),4.0),
            Check("menu closes so Job Changer dialog is readable", "regression",|_,a|!a.windows.menu.is_open()),
            Check("server enforces Job 10 before first class", "regression",|_,a|a.windows.npc_dialog.dialog.text.contains("necessários")&&a.windows.npc_dialog.dialog.text.contains("Job")&&a.game.character.job_level==1&&a.game.world.entities.player().is_some_and(|e|e.job==0)),
            Check("opening Job Changer does not warp the player", "regression",|p,a|a.game.session.current_map.as_deref()==Some("new_1-1")&&a.game.world.entities.player().is_some_and(|e|e.movement.cell_position()==p.position)),
            Act(|_,_,_|println!("script shot CLASSIC-JOBCHANGER-REQUIREMENTS")),Wait(1.0),
            Act(advance_tutorial_dialog),Wait(0.3),
            Until("Job Changer closes cleanly and returns control",|_,a|!a.windows.npc_dialog.dialog.is_open(),4.0),
            Act(|_,a,_|a.run_chat_command("@joblvup 9")),
            Until("novice reaches Job 10 for remote progression test",|_,a|a.game.character.job_level==10,4.0),
            Act(|_,a,_|a.run_chat_command("@jobchanger")),Wait(0.4),
        ]);
        for _ in 0..8 {steps.extend([Act(advance_job_changer),Wait(0.2)]);}
        steps.extend([
            Until("remote confirmation enforces spent skill points",|_,a|a.windows.npc_dialog.dialog.text.contains("pontos de habilidade"),4.0),
            Check("unspent skill points cannot bypass class requirements", "regression",|_,a|a.game.world.entities.player().is_some_and(|e|e.job==0)),
            Act(advance_tutorial_dialog),Wait(0.3),
            Act(|_,a,_|a.run_chat_command("@skillpoint -9999")),Wait(0.2),
            Act(|_,a,_|a.run_chat_command("@jobchanger")),Wait(0.4),
        ]);
        for _ in 0..8 {steps.extend([Act(advance_job_changer),Wait(0.2)]);}
        steps.extend([
            Until("remote Job Changer confirms actual Swordman class",|_,a|a.game.world.entities.player().is_some_and(|e|e.job==1),4.0),
            Check("successful remote class change stays on original map", "regression",|p,a|a.game.session.current_map.as_deref()==Some("new_1-1")&&a.game.world.entities.player().is_some_and(|e|e.movement.cell_position()==p.position)),
            Act(|_,_,_|println!("script shot CLASSIC-JOBCHANGER-SUCCESS")),Wait(1.0),
            Act(advance_tutorial_dialog),Wait(0.3),
            // Restore this disposable graphical fixture before the existing route.
            Act(|_,a,_|a.run_chat_command("@jobchange 0")),Wait(0.2),
            Act(|_,a,_|a.run_chat_command("@joblvup -999")),Wait(0.2),
            Act(|_,a,_|a.run_chat_command("@skillpoint -9999")),Wait(0.2),
            Until("disposable novice fixture restored after class-change test",|_,a|a.game.character.job_level==1&&a.game.world.entities.player().is_some_and(|e|e.job==0),4.0),
        ]);
        steps
    }

    fn quest_flow_steps()->VecDeque<Step> {
        use Step::*;
        let mut steps=vec![
            Act(|_,a,_|a.run_chat_command("@warp izlude 120 205")),
            Until("arrived near tutorial delivery NPC",|_,a|a.game.session.current_map.as_deref()==Some("izlude")&&a.game.world.entities.player().is_some_and(|e|e.movement.cell_position()==(120,205)),10.0),
            Act(|_,a,_|{
                for id in [7474,7475,7476,7477] {
                    if a.game.quest_log.get(id).is_some() {a.run_chat_command(&format!("@erasequest {id}"));}
                }
            }),Wait(0.3),
            Act(|_,a,_|{
                for id in [7474,7475,7476,7477] {a.run_chat_command(&format!("@setquest {id}"));}
            }),
            Until("server created parent and all three visit receipts",|_,a|[7474,7475,7476,7477].iter().all(|id|a.game.quest_log.get(*id).is_some()),4.0),
            Act(|_,a,el|a.handle_ui_events(vec![GameEvent::RequestToggleQuestActive{quest_id:7474,active:false},GameEvent::RequestToggleQuestActive{quest_id:7476,active:false}],el)),
            Until("server hid parent and second receipt exactly like captured save",|_,a|[7474,7476].iter().all(|id|a.game.quest_log.get(*id).is_some_and(|q|!q.active)),4.0),
            Act(|_,a,el|a.handle_ui_events(vec![GameEvent::OpenQuestDetail{quest_id:7477}],el)),Wait(0.3),
            Check("Portuguese checkpoint title replaces One step behind", "regression",|_,a|a.game.data_table.quest_display.as_ref().unwrap().title(7477)=="Passeio: ponte visitada"),
            Check("hidden parent and active receipts still point to delivery", "regression",|_,a|nostalre_game::quest_guidance::for_quest(&a.game.quest_log,7477).is_some_and(|g|g.objective.contains("3/3")&&g.destination.x==120&&g.destination.y==207)),
            Until("integrated controller HUD has the actual character portrait",|_,a|a.windows.hud.portrait().is_some()&&a.game.world.entities.player_id().is_some_and(|id|a.game.sprite_caches.sprites.contains_key(&id)),4.0),
            Act(|_,_,_|println!("script shot QUEST-RECEIPT-PTBR")),Wait(1.0),
            Act(|_,a,_|button(a,Button::Down,true)),Wait(0.05),Act(|_,a,_|button(a,Button::Down,false)),Wait(0.1),
            Act(|_,a,_|button(a,Button::B,true)),Wait(0.05),Act(|_,a,_|button(a,Button::B,false)),Wait(0.1),
            Check("B closes localized detail without opening chat", "regression",|_,a|!a.windows.quest_detail_window.is_open()&&!a.windows.chat_window.active),
            Until("real Information Staff entity loaded",|_,a|a.game.world.entities.iter().any(|e|e.entity_type==EntityType::Npc&&e.movement.cell_position()==(120,207)),4.0),
            Act(|_,a,el|{
                let npc_id=a.game.world.entities.iter().find(|e|e.entity_type==EntityType::Npc&&e.movement.cell_position()==(120,207)).unwrap().id;
                a.handle_ui_events(vec![GameEvent::RequestNpcContact{npc_id}],el);
            }),
            Until("delivery script recognizes all three visits with correct Portuguese accents",|_,a|a.windows.npc_dialog.dialog.say_visible&&a.windows.npc_dialog.dialog.text.contains("você")&&a.windows.npc_dialog.dialog.text.contains("locais"),4.0),
            Act(|_,_,_|println!("script shot QUEST-DELIVERY-PTBR")),Wait(1.0),
        ];
        // Advance the actual translated dialogue, choosing its first menu entry.
        // All quest completion/removal and rewards below must come from the C server.
        for _ in 0..40 {steps.extend([Act(advance_tutorial_dialog),Wait(0.12)]);}
        steps.extend([
            Until("delivery completes parent and removes all three receipts",|_,a|[7474,7475,7476,7477].iter().all(|id|a.game.quest_log.get(*id).is_none()),5.0),
            Check("tour actually awarded novice potions", "regression",|_,a|a.game.character.inventory.count_of(569)>=20),
            Act(|_,a,_|a.run_chat_command("@warp iz_ac01 100 36")),
            Until("academy reception map loaded",|_,a|a.game.session.current_map.as_deref()==Some("iz_ac01"),10.0),
            Check("next tutorial step is academy reception", "regression",|_,a|nostalre_game::quest_guidance::novice_hint("iz_ac01",0,a.game.character.job_level).is_some_and(|g|g.destination.x==100&&g.destination.y==39)),
            Until("receptionist entity loaded",|_,a|a.game.world.entities.iter().any(|e|e.entity_type==EntityType::Npc&&e.movement.cell_position()==(100,39)),4.0),
            Act(|_,a,el|{
                let npc_id=a.game.world.entities.iter().find(|e|e.entity_type==EntityType::Npc&&e.movement.cell_position()==(100,39)).unwrap().id;
                a.handle_ui_events(vec![GameEvent::RequestNpcContact{npc_id}],el);
            }),
            Until("actual receptionist dialog displayed",|_,a|a.windows.npc_dialog.dialog.say_visible,4.0),
        ]);
        for _ in 0..30 {steps.extend([Act(advance_tutorial_dialog),Wait(0.12)]);}
        steps.extend([
            Act(|_,a,_|a.run_chat_command("@joblvup 9")),
            Until("novice reaches actual first class job level",|_,a|a.game.character.job_level==10,4.0),
            Check("job level 10 points to class instructors instead of level 140 content", "regression",|_,a|nostalre_game::quest_guidance::novice_hint("iz_ac01",0,a.game.character.job_level).is_some_and(|g|g.destination.map=="iz_ac02"&&g.objective.contains("primeira classe"))),
            Act(|_,_,_|println!("script shot QUEST-ACADEMY-PTBR")),Wait(1.0),
            Act(|_,a,_|a.run_chat_command("@warp int_land03 81 104")),
            Until("quest flow returned to baseline ship",|_,a|a.game.session.current_map.as_deref()==Some("int_land03"),10.0),Wait(0.2),
        ]);
        steps.into()
    }

    fn quest_guide_steps()->VecDeque<Step> {
        use Step::*;
        vec![
            Act(|_,a,_|a.run_chat_command("@warp izlude 132 56")),
            Until("arrived beside Gein",|_,a|a.game.session.current_map.as_deref()==Some("izlude")&&a.game.world.entities.player().is_some_and(|e|e.movement.cell_position()==(132,56)),10.0),
            Until("Gein entity loaded",|_,a|a.game.world.entities.iter().any(|e|e.entity_type==EntityType::Npc&&e.movement.cell_position()==(132,49)),4.0),Wait(0.3),
            Check("level-140 Gein quest has no yellow marker on novice", "regression",|_,a|{
                let id=a.game.world.entities.iter().find(|e|e.entity_type==EntityType::Npc&&e.movement.cell_position()==(132,49)).unwrap().id;
                a.game.character.base_level<140&&!a.game.quest_markers.get(&id).is_some_and(|m|m.color==1)
            }),
            Act(|_,a,el|{
                let npc_id=a.game.world.entities.iter().find(|e|e.entity_type==EntityType::Npc&&e.movement.cell_position()==(132,49)).unwrap().id;
                a.handle_ui_events(vec![GameEvent::RequestNpcContact{npc_id}],el);
            }),
            Until("Gein first dialog page arrived",|_,a|a.windows.npc_dialog.dialog.say_visible&&a.windows.npc_dialog.dialog.next_button,4.0),
            Act(|_,a,el|{let npc_id=a.windows.npc_dialog.dialog.npc_id;a.windows.npc_dialog.dialog.advance_next();a.handle_ui_events(vec![GameEvent::RequestNpcNext{npc_id}],el);}),
            Until("Gein actually asks novice to return at level 140",|_,a|a.windows.npc_dialog.dialog.say_visible&&a.windows.npc_dialog.dialog.text.contains("140"),4.0),
            Act(|_,a,_|a.run_chat_command("@jobchange 4054")),
            Until("disposable test job permits level 140",|_,a|a.game.world.entities.player().is_some_and(|e|e.job==4054),4.0),
            Act(|_,a,_|a.run_chat_command("@lvup 138")),
            Until("server raised disposable novice to level 140",|_,a|a.game.character.base_level>=140,4.0),
            Until("Gein yellow marker becomes available at level 140",|_,a|{
                a.game.world.entities.iter().find(|e|e.entity_type==EntityType::Npc&&e.movement.cell_position()==(132,49))
                    .is_some_and(|e|a.game.quest_markers.get(&e.id).is_some_and(|m|m.color==1))
            },4.0),
            Act(|_,a,_|{a.run_chat_command("@lvup -138");a.run_chat_command("@jobchange 0");a.run_chat_command("@warp izlude 110 178");}),
            Until("returned to central Izlude for quest HUD",|_,a|a.game.world.entities.player().is_some_and(|e|e.movement.cell_position()==(110,178)),5.0),
            Act(|_,a,_|{for id in [7474,7475,7476,7477,1238] {a.run_chat_command(&format!("@erasequest {id}"));}a.run_chat_command("@setquest 7474");if let Some(ui)=a.ui_context.as_mut(){ui.pad_labels=true;}}),
            Until("server granted real tutorial quest",|_,a|a.game.quest_log.get(7474).is_some_and(|q|q.active)&&[7475,7476,7477].iter().all(|id|a.game.quest_log.get(*id).is_none()),4.0),Wait(0.2),
            Check("accepted tutorial draws a blue destination", "regression",|_,a|a.windows.minimap_window.entity_markers.iter().any(|m|matches!(m.marker_type,nostalre_ui_component::game::minimap_window::MarkerType::Square(_))&&m.x==179.0&&m.y==75.0)),
            Check("controller HUD uses the enlarged minimap", "regression",|_,a|a.windows.minimap_window.round.is_some_and(|r|r.w>=80.0&&r.w<=100.0)),
            Act(|_,_,_|println!("script shot QUEST-ACTIVE")),Wait(1.0),
            Act(|_,a,_|{for id in [7475,7476,7477] {a.run_chat_command(&format!("@setquest {id}"));}}),
            Until("three checkpoint states acknowledged by server",|_,a|[7475,7476,7477].iter().all(|id|a.game.quest_log.get(*id).is_some()),4.0),Wait(0.2),
            Check("three checkpoints redirect blue marker to reward NPC", "regression",|_,a|a.windows.minimap_window.entity_markers.iter().any(|m|matches!(m.marker_type,nostalre_ui_component::game::minimap_window::MarkerType::Square(_))&&m.x==120.0&&m.y==207.0)),
            Act(|_,_,_|println!("script shot QUEST-RETURN")),Wait(1.0),
            Act(|_,a,el|{a.run_chat_command("@setquest 1238");a.handle_ui_events(vec![GameEvent::ToggleQuestWindow],el);}),
            Until("side quest acknowledged for controller selection",|_,a|a.game.quest_log.get(1238).is_some(),4.0),Wait(0.1),
            Act(|_,a,_|button(a,Button::Down,true)),Wait(0.05),Act(|_,a,_|button(a,Button::Down,false)),Wait(0.05),
            Act(|_,a,_|button(a,Button::Down,true)),Wait(0.05),Act(|_,a,_|button(a,Button::Down,false)),Wait(0.05),
            Act(|_,a,_|button(a,Button::Down,true)),Wait(0.05),Act(|_,a,_|button(a,Button::Down,false)),Wait(0.05),
            Act(|_,a,_|button(a,Button::Down,true)),Wait(0.05),Act(|_,a,_|button(a,Button::Down,false)),Wait(0.05),
            Act(|_,_,_|println!("script shot QUEST-MENU")),Wait(1.0),
            Act(|_,a,_|button(a,Button::A,true)),Wait(0.05),Act(|_,a,_|button(a,Button::A,false)),Wait(0.1),
            Check("D-pad and A follow the chosen side quest", "regression",|_,a|a.game.quest_log.followed_id==Some(1238)&&a.windows.quest_detail_window.quest_id()==Some(1238)),
            Act(mark),Act(|_,a,_|a.gamepad.axis(0,1.0)),Wait(0.3),Act(|_,a,_|neutral(a)),
            Check("reading a quest cannot move the avatar behind the panel", "regression",|p,a|p.count.moves==p.base.moves&&a.game.world.entities.player().unwrap().movement.cell_position()==p.position),
            Act(|_,a,_|button(a,Button::B,true)),Wait(0.05),Act(|_,a,_|button(a,Button::B,false)),Wait(0.1),
            Check("B closes the quest detail", "regression",|_,a|!a.windows.quest_detail_window.is_open()),
            Act(|_,a,_|a.run_chat_command("@warp int_land03 81 104")),
            Until("quest guide test returned to ship",|_,a|a.game.session.current_map.as_deref()==Some("int_land03"),10.0),Wait(0.2),
        ].into()
    }
    fn npc_portal_steps()->VecDeque<Step> {
        use Step::*;
        let mut steps=Vec::new();
        for _ in 0..3 {
        steps.extend([
            // kRO deliberately rejects a third identical chat line. Separate
            // repeated fixture warps with a real read-only command.
            Act(|_,a,_|{a.run_chat_command("@where");a.run_chat_command("@warp izlude 113 175");}),
            Until("arrived beside Shop Helper Leonie",|_,a|a.game.session.current_map.as_deref()==Some("izlude")&&a.game.world.entities.player().is_some_and(|e|e.movement.cell_position()==(113,175)),10.0),
            Until("actual Shop Helper entity appeared",|_,a|a.game.world.entities.iter().any(|e|e.entity_type==EntityType::Npc&&e.movement.cell_position()==(115,181)),4.0),
            Act(|_,a,_|{a.game.combat.queued_move=Some((110,182));}),
            Until("walk toward portal acknowledged before NPC contact",|_,a|a.game.world.entities.player().is_some_and(|e|e.movement.is_moving()),2.0),
            Act(|_,a,el|{
                let npc=a.game.world.entities.iter().find(|e|e.entity_type==EntityType::Npc&&e.movement.cell_position()==(115,181)).expect("actual Shop Helper");
                let npc_id=npc.id;
                a.handle_ui_events(vec![GameEvent::RequestNpcContact{npc_id}],el);
            }),
            Until("Leonie conversation displayed by actual UI",|_,a|a.windows.npc_dialog.dialog.say_visible,4.0),
            Check("Leonie dialog is visible before crossing the portal", "regression",|_,a|a.game.session.current_map.as_deref()==Some("izlude")&&a.windows.npc_dialog.dialog.is_open()),
            Until("entered shop while Leonie dialog was open",|_,a|a.game.session.current_map.as_deref()==Some("izlude_in"),10.0),Wait(0.3),
            Check("map transition dismisses previous NPC conversation", "regression",|_,a|!a.windows.npc_dialog.dialog.is_open()),
            Act(|_,a,_|{a.game.combat.queued_move=Some((65,84));}),
            Until("returned outside after interrupted NPC conversation",|_,a|a.game.session.current_map.as_deref()==Some("izlude"),10.0),Wait(0.3),Act(mark),
            Act(|_,a,_|{let yaw=a.renderer.as_ref().unwrap().camera.yaw;a.gamepad.axis(0,f64::from(yaw.cos()));a.gamepad.axis(1,f64::from(yaw.sin()));}),Wait(0.4),Act(|_,a,_|neutral(a)),Wait(0.3),
            Check("movement works after NPC portal round trip", "regression",|p,a|a.game.world.entities.player().unwrap().movement.cell_position()!=p.position),
        ]);
        }
        steps.extend([
            Act(|_,a,_|a.run_chat_command("@warp int_land03 81 104")),
            Until("NPC portal test returned to ship",|_,a|a.game.session.current_map.as_deref()==Some("int_land03"),10.0),Wait(0.2),
        ]);
        steps.into()
    }
    fn portal_steps()->VecDeque<Step> {
        use Step::*;
        let repeats=std::env::var("NOSTALRE_IZLUDE_PORTAL_ROUNDS").ok().and_then(|n|n.parse::<usize>().ok()).unwrap_or(1);
        if repeats==1 {return vec![
            Act(|_,a,_|a.run_chat_command("@warp izlude_in 65 87")),
            Until("entered actual Izlude weapon shop",|_,a|a.game.session.current_map.as_deref()==Some("izlude_in"),10.0),Wait(0.3),
            Act(|_,a,_|{a.game.combat.queued_move=Some((65,84));}),
            Until("weapon shop exit portal changed map",|_,a|a.game.session.current_map.as_deref()==Some("izlude"),10.0),Wait(0.3),
            Act(|_,a,_|{
                let cell=a.game.world.entities.player().unwrap().movement.cell_position();
                let gat=a.game.session.gat.as_ref().unwrap();
                println!("IZLUDE-PORTAL map={:?} cell={cell:?} client_gat={}x{} walkable={}",a.game.session.current_map,gat.width,gat.height,gat.is_walkable(cell.0.into(),cell.1.into()));
            }),
            Check("shop exit lands on a walkable client cell", "regression",|_,a|{
                let cell=a.game.world.entities.player().unwrap().movement.cell_position();a.game.session.gat.as_ref().unwrap().is_walkable(cell.0.into(),cell.1.into())
            }),Act(mark),
            Act(|_,a,_|{let yaw=a.renderer.as_ref().unwrap().camera.yaw;a.gamepad.axis(0,f64::from(yaw.cos()));a.gamepad.axis(1,f64::from(yaw.sin()));}),Wait(0.8),Act(|_,a,_|neutral(a)),Wait(0.4),
            Check("avatar can walk after leaving weapon shop", "regression",|p,a|a.game.world.entities.player().unwrap().movement.cell_position()!=p.position),
            Act(|_,a,_|a.run_chat_command("@warp int_land03 81 104")),
            Until("portal test returned to ship",|_,a|a.game.session.current_map.as_deref()==Some("int_land03"),10.0),Wait(0.2),
        ].into();}
        let mut steps=vec![
            Check("saved avatar starts on walkable geometry", "regression",|_,a|{let p=a.game.world.entities.player().unwrap().movement.cell_position();a.game.session.gat.as_ref().unwrap().is_walkable(p.0.into(),p.1.into())}),Act(mark),
            Act(|_,a,_|{let yaw=a.renderer.as_ref().unwrap().camera.yaw;a.gamepad.axis(0,f64::from(yaw.cos()));a.gamepad.axis(1,f64::from(yaw.sin()));}),Wait(0.35),Act(|_,a,_|neutral(a)),Wait(0.3),
            Check("saved trapped avatar can walk without editing DB", "regression",|p,a|a.game.world.entities.player().unwrap().movement.cell_position()!=p.position),
            Act(|_,a,_|a.run_chat_command("@warp izlude 110 178")),Until("arrived outside weapon shop",|_,a|a.game.session.current_map.as_deref()==Some("izlude")&&a.game.world.entities.player().is_some_and(|e|e.movement.cell_position()==(110,178)),10.0),Wait(0.3),
        ];
        for _ in 0..repeats {
            steps.extend([
                Act(|_,a,_|{a.game.combat.queued_move=Some((110,182));}),
                Until("entered weapon shop through actual portal",|_,a|a.game.session.current_map.as_deref()==Some("izlude_in"),10.0),Wait(0.3),
                Act(|_,a,_|{a.game.combat.queued_move=Some((65,84));}),
                Until("exited weapon shop through actual portal",|_,a|a.game.session.current_map.as_deref()==Some("izlude"),10.0),Wait(0.3),
                Check("repeated shop exit is walkable", "regression",|_,a|{let p=a.game.world.entities.player().unwrap().movement.cell_position();a.game.session.gat.as_ref().unwrap().is_walkable(p.0.into(),p.1.into())}),Act(mark),
                Act(|_,a,_|{let yaw=a.renderer.as_ref().unwrap().camera.yaw;a.gamepad.axis(0,f64::from(yaw.cos()));a.gamepad.axis(1,f64::from(yaw.sin()));}),Wait(0.35),Act(|_,a,_|neutral(a)),Wait(0.3),
                Check("repeated exit allows physical-stick movement", "regression",|p,a|a.game.world.entities.player().unwrap().movement.cell_position()!=p.position),
            ]);
        }
        steps.extend([
            Act(|_,a,_|a.run_chat_command("@warp int_land03 81 104")),
            Until("portal test returned to ship",|_,a|a.game.session.current_map.as_deref()==Some("int_land03"),10.0),Wait(0.2),
        ]);
        steps.into()
    }
    pub fn before_frame(&mut self,app:&mut App,el:&ActiveEventLoop) {
        self.frame_started=Some(Instant::now());
        if self.finished { return; }
        if !self.started {
            if app.game.session.app_state!=AppState::InGame {return;}
            self.started=true;self.script=app.script.take();
            self.forward=app.channel.cmd_tx.take();let(tx,rx)=mpsc::unbounded_channel();app.channel.cmd_tx=Some(tx);self.outgoing=Some(rx);
            self.steps=Self::build_steps();println!("GAMEPLAY-BEGIN actual App / offline servers / Vulkan / disposable save");
        }
        self.drain_commands(app);
        if self.wake.is_some_and(|at|Instant::now()<at) {return;} self.wake=None;
        let Some(step)=self.steps.pop_front() else {
            self.finished=true;app.script=self.script.take();self.write_report();return;
        };
        match step {
            Step::Act(f)=>f(self,app,el),
            Step::Wait(s)=>self.wake=Some(Instant::now()+Duration::from_secs_f32(s)),
            Step::Check(name,kind,f)=>{let ok=f(self,app);self.record(name,kind,ok,app);},
            Step::Until(name,f,seconds)=>{
                if f(self,app) {self.deadline=None;self.record(name,"regression",true,app);}
                else if Instant::now()>=*self.deadline.get_or_insert_with(||Instant::now()+Duration::from_secs_f32(seconds)) {self.deadline=None;self.record(name,"regression",false,app);}
                else {self.steps.push_front(Step::Until(name,f,seconds));}
            }
        }
    }
    fn drain_commands(&mut self,app:&App) {
        let move_packet=nostalre_network::build_request_move_packet(1,1,app.active_packetver);
        let skill_packet=nostalre_network::build_use_skill_packet(SkillEnum::SmEndure,1,2000000,app.active_packetver);
        let attack_packet=nostalre_network::build_action_request_packet(0,7,app.active_packetver);
        if let Some(rx)=&mut self.outgoing {while let Ok(command)=rx.try_recv() {
            if let NetworkCommand::SendPacket(bytes)=&command {if bytes.len()>=2 {
                let mut gameplay=false;
                if bytes[..2]==move_packet[..2] {self.count.moves+=1;gameplay=true;}
                if bytes[..2]==skill_packet[..2] {self.count.skills+=1;gameplay=true;}
                if bytes[..2]==attack_packet[..2] {self.count.attacks+=1;gameplay=true;}
                if gameplay {if let Some(at)=self.input_at.take() {self.command_latencies_ms.push(at.elapsed().as_secs_f64()*1000.0);}}
            }}
            if let Some(tx)=&self.forward {tx.send(command).unwrap();}
        }}
    }
    pub fn after_frame(&mut self,app:&mut App) {
        self.drain_commands(app);
        if let Some(at)=self.frame_started.take() {if self.started&&!self.finished {self.frames_ms.push(at.elapsed().as_secs_f64()*1000.0);}}
    }
    pub fn observe(&mut self,event:&GameEvent) {
        match event {
            GameEvent::VendingShopList{aid,unique_id,items}=>{self.vendor=items.iter().find(|i|i.item_id==501).map(|i|(*aid,*unique_id,i.index,i.price));},
            GameEvent::EntityAction{gid,damage,left_damage,..} if *gid==2000000&&(*damage>0||*left_damage>0)=>self.count.normal_hits+=1,
            GameEvent::SkillDamage{src_gid,skill,damage,..} if *src_gid==2000000&&*damage>0=>*self.count.damage.entry(skill.id()).or_default()+=1,
            GameEvent::SkillNoDamage{src_gid,skill,..} if *src_gid==2000000=>*self.count.nodamage.entry(skill.id()).or_default()+=1,
            GameEvent::GroundSkill{src_gid,..} if *src_gid==2000000=>self.count.ground+=1,
            GameEvent::SkillUnitEntered{creator_aid,..} if *creator_aid==2000000=>self.count.ground+=1,
            GameEvent::SkillFailed{cause,..}=>self.count.failures.push(*cause),
            _=>{}
        }
    }
    fn write_report(&self) {
        let mut frames=self.frames_ms.clone();frames.sort_by(f64::total_cmp);
        let percentile=|p:f64|if frames.is_empty(){0.0}else{frames[((frames.len()-1)as f64*p)as usize]};
        let report=json!({"finished":self.finished,"backend":"Actual PC client, Mesa software Vulkan, not emulated Switch",
            "checks":self.findings,"sampled_cpu_render_frames":frames.len(),"cpu_render_ms_p50":percentile(0.5),"cpu_render_ms_p95":percentile(0.95),"cpu_render_ms_p99":percentile(0.99),
            "normalized_input_to_command_ms":self.command_latencies_ms,"switch_60fps_verified":false,
            "qualification":"Proposal failures identify unimplemented requested behavior. Regression failures require investigation. No physical Joy-Con or console timing claim."});
        std::fs::write(std::env::var("NOSTALRE_GAMEPLAY_VALIDATION").unwrap(),serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        println!("GAMEPLAY-END checks={} failures={}",self.findings.len(),self.findings.iter().filter(|v|v["passed"]==false).count());
    }
}
