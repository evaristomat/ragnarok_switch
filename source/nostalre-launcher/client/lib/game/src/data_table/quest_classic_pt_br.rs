//! PT-BR journal display overlay for the independent classic edition.
//! Does not create quests or import Renewal academy destinations.
use std::collections::HashMap;
use nostalre_formats::lua_table::QuestDisplay;

pub(super) fn apply(entries: &mut HashMap<u32, QuestDisplay>) {
    for stage in crate::campaign_data::STAGES {
        for (id,done) in [(stage.id,false),(stage.id+500,true)] {
            let entry=entries.entry(id).or_default();
            entry.title=format!("{}{}",stage.title,if done {" — Concluída"} else {""});
            entry.summary=if done {stage.summary} else {stage.objective}.into();
            entry.description=if done {stage.summary} else {stage.summary.split("\n\n").next().unwrap_or("")}.into();
        }
    }
    entries.insert(900999,QuestDisplay {title:"Ecos de Rune-Midgard — Campanha concluída".into(),summary:"A ameaça imediata de Morocc terminou. Seu mundo permanece aberto.".into(),description:"Campanha concluída. Continue explorando, enfrente MVPs ou renasça. A recompensa final já foi registrada e não se repete.".into(),..Default::default()});
    for (id, translated) in nostalre_formats::lua_table::parse_questid2display(include_bytes!("quest_community_pt_br.txt")) {
        let entry = entries.entry(id).or_default();
        entry.title = translated.title;
        entry.summary = translated.summary;
        if !translated.description.is_empty() { entry.description = translated.description; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translates_classic_quests_preserving_art_and_custom_entries() {
        let old = QuestDisplay { title: "Custom".into(), icon_name: "custom-icon".into(), image_name: "custom-art".into(), ..Default::default() };
        let mut entries = HashMap::from([(1001, old.clone()), (999_999, old.clone())]);
        apply(&mut entries);
        assert_eq!(entries[&1001].title, "Mudança de Classe: Noviço");
        assert!(entries[&1001].summary.contains("você"));
        assert_eq!(entries[&1001].icon_name, "custom-icon");
        assert_eq!(entries[&1001].image_name, "custom-art");
        assert_eq!(entries[&999_999], old);
        assert_eq!(entries.len(), 1631 + crate::campaign_data::STAGES.len()*2 + 1);
        assert!(entries.values().all(|q| !q.title.contains('\u{fffd}') && !q.summary.contains('\u{fffd}')));
    }
}
