//! Destinations for the independent Pre-Renewal novice training grounds.
use crate::quest::QuestLog;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Destination { pub map: &'static str, pub x: u16, pub y: u16, pub name: &'static str }
#[derive(Debug)]
pub struct Guidance { pub title: &'static str, pub objective: String, pub destination: Destination }
pub fn for_quest(log: &QuestLog, id: u32) -> Option<Guidance> {
    let stage=crate::campaign_data::STAGES.iter().find(|stage|stage.id==id)?;
    let quest=log.get(id)?;
    let complete=stage.hunt && !quest.objectives.is_empty() && quest.objectives.iter().all(|o|o.required>0 && o.current>=o.required);
    let destination=if stage.hunt && !complete {
        Destination {map:stage.area,x:stage.ax,y:stage.ay,name:"Área do objetivo"}
    } else { Destination {map:stage.map,x:stage.x,y:stage.y,name:stage.npc} };
    Some(Guidance {title:stage.title,objective:if complete {format!("Volte para entregar: {}.",stage.npc)} else {stage.objective.into()},destination})
}
pub fn next_on_map(target: &Destination, map: &str) -> Option<Destination> {
    let map=map.strip_suffix(".gat").unwrap_or(map);
    if map==target.map {return Some(target.clone());}
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<(String,String),Option<Destination>>>>=std::sync::OnceLock::new();
    let cache=CACHE.get_or_init(Default::default);
    let key=(map.to_owned(),target.map.to_owned());
    if let Some(found)=cache.lock().unwrap().get(&key) {return found.clone();}
    // BFS over active classic warp declarations. Never project a foreign map's cells here.
    let mut queue=std::collections::VecDeque::from([(map,None)]);
    let mut seen=std::collections::HashSet::from([map]);
    while let Some((here,first))=queue.pop_front() {
        for &(from,x,y,to) in crate::campaign_data::ROUTES {
            if here!=from || !seen.insert(to) {continue;}
            let hop=first.clone().unwrap_or(Destination {map:from,x,y,name:"Portal para o objetivo"});
            if to==target.map {cache.lock().unwrap().insert(key,Some(hop.clone()));return Some(hop);}
            queue.push_back((to,Some(hop)));
        }
    }
    cache.lock().unwrap().insert(key,None);
    None
}
pub fn novice_hint(map: &str, job: u16, job_level: u32) -> Option<Guidance> {
    if job != 0 { return None; }
    let map = map.strip_suffix(".gat").unwrap_or(map);
    let (map, x, y, name, objective) = match map {
        "new_1-1" => ("new_1-1", 53, 114, "Shion", "Fale com Shion, ao lado do ponto inicial, para comecar o treinamento."),
        "new_1-2" => ("new_1-2", 100, 29, "Recepcionista", "Fale com a recepcionista e siga as aulas do Campo de Treinamento."),
        "new_1-3" => ("new_1-3", 96, 174, "Examinador", "Conclua o treinamento e fale com o examinador para escolher seu destino."),
        "prontera" if job_level >= 10 => ("prontera", 150, 180, "Mestre das Classes", "Classe 10: escolha sua primeira classe com o Mestre das Classes."),
        _ => return None,
    };
    Some(Guidance { title: "Aprendiz Pre-Renewal", objective: objective.into(), destination: Destination {map, x, y, name} })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn novice_is_guided_through_classic_grounds() {
        assert_eq!(novice_hint("new_1-1", 0, 1).unwrap().destination.name, "Shion");
        assert_eq!(novice_hint("new_1-2.gat", 0, 5).unwrap().destination.y, 29);
        assert_eq!(novice_hint("prontera", 0, 10).unwrap().destination.x, 150);
        assert!(novice_hint("iz_ac01", 0, 1).is_none());
        assert!(novice_hint("prontera", 1, 10).is_none());
    }
}

pub fn region(map:&str)->&str {
    if map.starts_with("new_") || map.starts_with("prt_") {"prontera"}
    else if map.starts_with("gef_") {"geffen"}
    else if map.starts_with("pay_") {"payon"}
    else if map.starts_with("nif_") {"niflheim"}
    else if map.starts_with("nameless_") {"veins"}
    else if map.starts_with("kh_") {"yuno"}
    else if map.starts_with("lhz_") {"lighthalzen"}
    else if map.starts_with("moc_") {"morocc"}
    else if map=="ve_in" {"veins"}
    else {map}
}
#[cfg(test)]
mod campaign_tests {
    use super::*;
    use crate::quest::{Quest,QuestObjective};
    #[test]
    fn hunt_returns_to_contact_only_after_server_counter_is_complete() {
        let mut log=QuestLog::default();
        log.add(Quest {id:900005,active:true,objectives:vec![QuestObjective {current:7,required:8,..Default::default()}],..Default::default()});
        assert_eq!(for_quest(&log,900005).unwrap().destination.map,"prt_fild08");
        log.quests[0].objectives[0].current=8;
        let guide=for_quest(&log,900005).unwrap();
        assert_eq!(guide.destination.map,"prontera");assert!(guide.objective.starts_with("Volte para entregar"));
    }
    #[test]
    fn accepting_a_sidequest_does_not_steal_campaign_but_manual_choice_does() {
        let mut log=QuestLog::default();
        for id in [900005,1238] {log.add(Quest{id,active:true,..Default::default()});}
        assert_eq!(log.followed().unwrap().id,900005);
        assert!(log.follow(1238));assert_eq!(log.followed().unwrap().id,1238);
    }
    #[test]
    fn distant_objectives_resolve_a_portal_in_the_current_map_only() {
        let destination=Destination{map:"payon",x:158,y:246,name:"Guarda"};
        let hop=next_on_map(&destination,"prontera.gat").unwrap();
        assert_eq!(hop.map,"prontera");assert_ne!((hop.x,hop.y),(158,246));
        assert_eq!(next_on_map(&destination,"prontera"),Some(hop));
        assert!(next_on_map(&destination,"missing_map").is_none());
    }
}
