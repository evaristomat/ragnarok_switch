//! Update only resources owned by the offline profile, preserving saves and custom includes.
use std::io::{self, Write};
use std::path::Path;

const RESOURCES: [&str; 21] = [
    "npc/custom/nostalre_job_changer.txt",
    "npc/custom/ragswitch_campaign/main.txt",
    "db/ragswitch_campaign_quests.yml",
    "conf/battle/nostalre_rates.conf",
    "db/ragswitch_classic_items.yml",
    "npc/custom/ragswitch_starter_gift.txt",
    "db/import/map_cache.dat",
    "npc/pre-re/scripts_athena.conf",
    "npc/pre-re/scripts_warps.conf",
    "db/population_engine.yml",
    "db/population_vendors.yml",
    "db/population_chat.yml",
    "conf/battle/population_engine.conf",
    "npc/cities/izlude.txt",
    "npc/cities/prontera.txt",
    "npc/kafras/cool_event_corp.txt",
    "npc/kafras/dts_warper.txt",
    "npc/kafras/functions_kafras.txt",
    "npc/kafras/kafras.txt",
    "npc/pre-re/guides/guides_izlude.txt",
    "npc/pre-re/guides/guides_prontera.txt",
];

fn patch_item_import(original: &[u8]) -> io::Result<Vec<u8>> {
    let directive = b"  - Path: db/ragswitch_classic_items.yml";
    if original.split(|b| *b == b'\n').any(|line| line.trim_ascii() == directive.trim_ascii()) {
        return Ok(original.to_vec());
    }
    if !original.windows(b"Footer:".len()).any(|w| w == b"Footer:")
        || !original.windows(b"  Imports:".len()).any(|w| w == b"  Imports:") {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Item database import footer missing"));
    }
    let mut updated = original.to_vec();
    if !updated.ends_with(b"\n") { updated.push(b'\n'); }
    updated.extend_from_slice(directive);
    updated.push(b'\n');
    Ok(updated)
}

const TRAINING_GROUNDS: &str = "npc/pre-re/jobs/novice/novice.txt";
const QUEST_MARKER_RULES: &str = include_str!("quest_marker_rules.tsv");

/// Insert only our OnInit marker lines. NPC logic, translation bytes and user
/// changes outside these lines survive an upgrade of an already installed SD.
pub(crate) fn patch_quest_markers(original: &[u8]) -> io::Result<Vec<u8>> {
    // Rebuild only our tagged lines so fixes to managed rules also reach SDs
    // that have already received an earlier revision.
    let mut bytes=Vec::with_capacity(original.len());
    for line in original.split_inclusive(|b|*b==b'\n') {
        if !line.windows(b"// ragswitch-pre-quest-guide-v1".len()).any(|part|part==b"// ragswitch-pre-quest-guide-v1") {
            bytes.extend_from_slice(line);
        }
    }
    for rule in QUEST_MARKER_RULES.lines().filter(|line|!line.starts_with("//") && !line.is_empty()) {
        let (name,condition)=rule.split_once('\t').unwrap();
        let header=format!("\tscript\t{name}\t");
        let start=bytes.windows(header.len()).position(|part|part==header.as_bytes())
            .ok_or_else(||io::Error::new(io::ErrorKind::InvalidData,format!("Tutorial NPC missing: {name}")))?;
        let end=start+bytes[start..].windows(2).position(|part|part==b"\n}")
            .ok_or_else(||io::Error::new(io::ErrorKind::InvalidData,"Tutorial NPC has no closing line"))?;
        // A template without a map cannot register a visible marker.
        let guard=if name.starts_with("::") {"if (strnpcinfo(4) != \"\") "} else {""};
        let marker=format!("\t{guard}questinfo( QTYPE_QUEST2, QMARK_YELLOW, \"{condition}\" ); // ragswitch-pre-quest-guide-v1");
        let block=&bytes[start..end];
        if block.windows(marker.len()).any(|part|part==marker.as_bytes()) {continue;}
        let newline=if block.windows(2).any(|part|part==b"\r\n") {"\r\n"} else {"\n"};
        let init=b"\nOnInit:";
        let (at,insert)=if let Some(label)=block.windows(init.len()).position(|part|part==init) {
            let label_end=start+label+init.len();
            let at=label_end+bytes[label_end..end].iter().position(|b|*b==b'\n')
                .ok_or_else(||io::Error::new(io::ErrorKind::InvalidData,"Invalid OnInit line"))?+1;
            (at,format!("{marker}{newline}"))
        } else {
            (end,format!("{newline}\tend;{newline}OnInit:{newline}{marker}{newline}\tend;"))
        };
        bytes.splice(at..at,insert.bytes());
    }
    Ok(bytes)
}

fn replace_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let temporary = path.with_extension("offline-installing");
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(temporary, path)
}

fn update_owned(path: &Path, bytes: &[u8]) -> io::Result<bool> {
    let old = match std::fs::read(path) {
        Ok(old) => Some(old),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    if old.as_deref() == Some(bytes) { return Ok(false); }
    if let Some(old) = old {
        // Keep the first pre-migration copy even after retries or later updates.
        let backup = path.with_extension(format!("{}.pre-offline-profile", path.extension().unwrap_or_default().to_string_lossy()));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(backup) {
            Ok(mut file) => { file.write_all(&old)?; file.sync_all()?; }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {},
            Err(e) => return Err(e),
        }
    }
    replace_file(path, bytes)?;
    Ok(true)
}

fn ensure_include(path: &Path, directive: &[u8], final_override: bool) -> io::Result<bool> {
    let old = std::fs::read(path)?;
    let matching = |line: &[u8]| {
        // The config and NPC lists may contain CP1252 comments: keep their original bytes.
        let line = line.split(|b| *b == b'\r').next().unwrap_or(line);
        std::str::from_utf8(line).ok().is_some_and(|line| {
            let line = line.split("//").next().unwrap_or("");
            line.split_once(':').is_some_and(|(key, value)| {
                let expected = std::str::from_utf8(directive).unwrap();
                let (want_key, want_value) = expected.split_once(':').unwrap();
                key.trim() == want_key.trim() && value.trim() == want_value.trim()
            })
        })
    };
    let count = old.split(|b| *b == b'\n').filter(|line| matching(line)).count();
    let last_active = old.split(|b| *b == b'\n').rev().find(|line| {
        let trimmed = line.trim_ascii();
        !trimmed.is_empty() && !trimmed.starts_with(b"//")
    });
    if count == 1 && (!final_override || last_active.is_some_and(matching)) { return Ok(false); }
    let mut updated = Vec::with_capacity(old.len() + directive.len() + 2);
    for line in old.split_inclusive(|b| *b == b'\n') {
        if !matching(line) { updated.extend_from_slice(line); }
    }
    if !updated.ends_with(b"\n") { updated.push(b'\n'); }
    updated.extend_from_slice(directive);
    updated.push(b'\n');
    update_owned(path, &updated)
}

const CAMPAIGN_HOOKS:&str=include_str!("campaign_hooks.tsv");
fn npc_body(bytes:&[u8],start:usize)->io::Result<(usize,usize)> {
    let open=start+bytes[start..].iter().position(|b|*b==b'{').ok_or_else(||io::Error::new(io::ErrorKind::InvalidData,"NPC body missing"))?;
    let(mut depth,mut quote,mut line,mut comment,mut escaped,mut i)=(0,false,false,false,false,open);
    while i<bytes.len() {
        let b=bytes[i];let pair=bytes.get(i..i+2);
        if line {if b==b'\n' {line=false;}}
        else if comment {if pair==Some(b"*/") {comment=false;i+=1;}}
        else if quote {if escaped {escaped=false;} else if b==b'\\' {escaped=true;} else if b==b'"' {quote=false;}}
        else if pair==Some(b"//") {line=true;i+=1;}
        else if pair==Some(b"/*") {comment=true;i+=1;}
        else if b==b'"' {quote=true;}
        else if b==b'{' {depth+=1;}
        else if b==b'}' {depth-=1;if depth==0 {return Ok((open+1,i));}}
        i+=1;
    }
    Err(io::Error::new(io::ErrorKind::InvalidData,"NPC body unclosed"))
}
fn patch_campaign_contacts(original:&[u8], path:&str)->io::Result<Vec<u8>> {
    let mut bytes=original.to_vec();
    for row in CAMPAIGN_HOOKS.lines() {
        let mut cells=row.split('\t');
        let file=cells.next().unwrap();let npc=cells.next().unwrap();let condition=cells.next().unwrap();
        if file!=path {continue;}
        let header=format!("\tscript\t{npc}\t");
        let mut offset=0;let mut start=None;
        for line in bytes.split_inclusive(|b|*b==b'\n') {
            if !line.trim_ascii_start().starts_with(b"//") && line.windows(header.len()).any(|p|p==header.as_bytes()) {start=Some(offset);break;}
            offset+=line.len();
        }
        let start=start.ok_or_else(||io::Error::new(io::ErrorKind::InvalidData,format!("Campaign contact missing: {npc}")))?;
        let(open,end)=npc_body(&bytes,start)?;
        let hook=format!("\n\tif (callfunc(\"RS_CampaignContact\", \"{npc}\")) close; // RS-campaign-contact-v1\n");
        if !bytes[start..end].windows(b"RS-campaign-contact-v1".len()).any(|p|p==b"RS-campaign-contact-v1") {bytes.splice(open..open,hook.bytes());}
        let(_,end)=npc_body(&bytes,start)?;
        if !bytes[start..end].windows(b"RS-campaign-marker-v1".len()).any(|p|p==b"RS-campaign-marker-v1") {
            let marker=format!("\tif (strnpcinfo(4) != \"\") questinfo(QTYPE_QUEST2,QMARK_YELLOW,\"{condition}\"); // RS-campaign-marker-v1\n");
            if let Some(init)=bytes[start..end].windows(b"\nOnInit:".len()).position(|p|p==b"\nOnInit:") {
                let label=start+init+b"\nOnInit:".len();let at=label+bytes[label..end].iter().position(|b|*b==b'\n').unwrap()+1;
                bytes.splice(at..at,marker.bytes());
            } else {bytes.splice(end..end,format!("\n\tend;\nOnInit:\n{marker}\tend;\n").bytes());}
        }
        let(_,end)=npc_body(&bytes,start)?;
        if !bytes[start..end].windows(b"RS-campaign-touch-v1".len()).any(|p|p==b"RS-campaign-touch-v1") {
            if let Some(touch)=bytes[start..end].windows(b"\nOnTouch_:".len()).position(|p|p==b"\nOnTouch_:") {
                let at=start+touch+b"\nOnTouch_:".len();
                bytes.splice(at..at,format!("\n\tif ({condition}) end; // RS-campaign-touch-v1").bytes());
            }
        }
    }
    Ok(bytes)
}
fn patch_quest_import(old:&[u8])->Vec<u8> {
    let mut data=old.to_vec();let include=b"  - Path: db/ragswitch_campaign_quests.yml";
    if !data.windows(include.len()).any(|p|p==include) {if !data.ends_with(b"\n"){data.push(b'\n');}data.extend_from_slice(include);data.push(b'\n');}
    data
}

pub fn sync_offline_profile(source: &Path, target: &Path) -> io::Result<usize> {
    // Read all required payloads first. Never enable an include pointing to absent data.
    let payloads = RESOURCES.iter().map(|name| std::fs::read(source.join(name))).collect::<io::Result<Vec<_>>>()?;
    let training=std::fs::read(source.join(TRAINING_GROUNDS))?;
    let training=patch_quest_markers(&training)?;
    let item_import=patch_item_import(&std::fs::read(target.join("db/item_db.yml"))?)?;
    let mut contacts=Vec::new();
    let mut seen=std::collections::HashSet::new();
    for row in CAMPAIGN_HOOKS.lines() {
        let path=row.split('\t').next().unwrap();
        if !seen.insert(path) {continue;}
        let original=if path==TRAINING_GROUNDS {training.clone()} else if RESOURCES.contains(&path) {std::fs::read(source.join(path))?} else {std::fs::read(target.join(path))?};
        let patched=patch_campaign_contacts(&original,path)?;
        let patched=if path==TRAINING_GROUNDS {patch_quest_markers(&patched)?} else {patched};
        contacts.push((path,patched));
    }
    let quest_import=patch_quest_import(&std::fs::read(target.join("db/quest_db.yml"))?);
    let mut changed = 0;
    for (name, bytes) in RESOURCES.iter().zip(payloads.iter()) {
        let bytes=contacts.iter().find(|(path,_)|path==name).map(|(_,data)|data).unwrap_or(bytes);
        changed += usize::from(update_owned(&target.join(name), bytes)?);
    }
    changed += usize::from(ensure_include(&target.join("npc/scripts_custom.conf"), b"npc: npc/custom/nostalre_job_changer.txt", false)?);
    changed += usize::from(ensure_include(&target.join("npc/scripts_custom.conf"), b"npc: npc/custom/ragswitch_starter_gift.txt", false)?);
    changed += usize::from(update_owned(&target.join("db/item_db.yml"), &item_import)?);
    // The explicitly requested 15x preset wins over older personal EXP/drop overrides.
    changed += usize::from(ensure_include(&target.join("conf/battle_athena.conf"), b"import: conf/battle/nostalre_rates.conf", true)?);

    changed += usize::from(ensure_include(&target.join("npc/scripts_custom.conf"), b"npc: npc/custom/ragswitch_campaign/main.txt", false)?);
    changed += usize::from(update_owned(&target.join("db/quest_db.yml"), &quest_import)?);
    for (path,bytes) in contacts {changed += usize::from(update_owned(&target.join(path), &bytes)?);}
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn campaign_guard_upgrade_preserves_touch_dialog_and_guards_its_old_trigger() {
        let original=b"// custom CP1252: caf\xe9\r\npayon,158,246,3\tscript\tGuardsman#payon\t708,3,3,{\r\n end;\r\nOnTouch_:\r\n mes \"native dialogue\";\r\n nude; close;\r\n}\r\n";
        let patched=patch_campaign_contacts(original,"npc/cities/payon.txt").unwrap();
        assert!(patched.starts_with(b"// custom CP1252: caf\xe9\r\n"));
        let text=String::from_utf8_lossy(&patched);
        assert!(text.contains("if (RS_CampStage == 6 || RS_CampStage == 7) end; // RS-campaign-touch-v1"));
        assert!(text.contains("mes \"native dialogue\";\r\n nude; close;"));
        assert_eq!(text.matches("OnInit:").count(),1);
        assert_eq!(patch_campaign_contacts(&patched,"npc/cities/payon.txt").unwrap(),patched);
    }
    #[test]
    fn bundled_classic_training_grounds_accept_safe_idempotent_upgrade() {
        let bundled = include_bytes!("../../switch/romfs/server/npc/pre-re/jobs/novice/novice.txt");
        let patched = patch_quest_markers(bundled).unwrap();
        assert_eq!(patch_quest_markers(&patched).unwrap(), patched);
        assert_eq!(patched.windows(b"ragswitch-pre-quest-guide-v1".len()).filter(|w| *w == b"ragswitch-pre-quest-guide-v1").count(), 3);
        assert!(!patched.windows(b"iz_ac01".len()).any(|w| w == b"iz_ac01"));
    }
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("offline-profile-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            for directory in ["source/npc/custom", "source/conf/battle", "target/npc", "target/conf/import"] { std::fs::create_dir_all(root.join(directory)).unwrap(); }
            std::fs::write(root.join("source/npc/custom/nostalre_job_changer.txt"), b"bundled npc").unwrap();
            std::fs::write(root.join("source/conf/battle/nostalre_rates.conf"), b"base_exp_rate: 1000\n").unwrap();
            std::fs::create_dir_all(root.join("source/db")).unwrap();
            std::fs::create_dir_all(root.join("target/db")).unwrap();
            std::fs::write(root.join("source/db/ragswitch_classic_items.yml"), b"bundled item\n").unwrap();
            std::fs::write(root.join("source/npc/custom/ragswitch_starter_gift.txt"), b"bundled gift\n").unwrap();
            for name in RESOURCES {
                let path=root.join("source").join(name);
                if !path.exists() {
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(path,b"bundled classic resource\n").unwrap();
                }
            }
            std::fs::write(root.join("target/db/item_db.yml"), b"// custom\xe1\r\nHeader:\r\n  Type: ITEM_DB\r\n  Version: 3\r\nFooter:\r\n  Imports:\r\n  - Path: db/import/item_db.yml\r\n").unwrap();
            std::fs::write(root.join("target/npc/scripts_custom.conf"), b"// coment\xe1rio\r\nnpc: npc/custom/my-npc.txt\r\n").unwrap();
            std::fs::write(root.join("target/conf/battle_athena.conf"), b"import: conf/battle/population_engine.conf\nimport: conf/import/battle_conf.txt").unwrap();
            let training=root.join("target").join(TRAINING_GROUNDS);
            std::fs::create_dir_all(training.parent().unwrap()).unwrap();
            let mut script=b"// tradu\xe7\xe3o e personaliza\xe7\xe3o\r\n".to_vec();
            for line in QUEST_MARKER_RULES.lines().filter(|line|!line.starts_with("//") && !line.is_empty()) {
                let (name,_)=line.split_once('\t').unwrap();
                script.extend(format!("izlude,1,1,0\tscript\t{name}\t123,{{\r\n\tclose;\r\n}}\r\n").bytes());
            }
            std::fs::write(&training,&script).unwrap();
            let source_training=root.join("source").join(TRAINING_GROUNDS);
            std::fs::create_dir_all(source_training.parent().unwrap()).unwrap();
            std::fs::write(source_training,script).unwrap();
            std::fs::write(root.join("target/db/quest_db.yml"),b"Header:\n  Type: QUEST_DB\n  Version: 3\nFooter:\n  Imports:\n").unwrap();
            for row in CAMPAIGN_HOOKS.lines() {
                let mut values=row.split('\t');let path=values.next().unwrap();let npc=values.next().unwrap();
                for base in ["source","target"] {
                    if base=="target" && RESOURCES.contains(&path) {continue;}
                    let file=root.join(base).join(path);
                    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                    let mut bytes=std::fs::read(&file).unwrap_or_default();
                    let header=format!("\tscript\t{npc}\t");
                    if !bytes.windows(header.len()).any(|p|p==header.as_bytes()) {
                        bytes.extend(format!("\nprontera,1,1,0\tscript\t{npc}\t123,{{\nclose;\n}}\n").bytes());
                        std::fs::write(file,bytes).unwrap();
                    }
                }
            }
            Self(root)
        }
        fn run(&self) -> io::Result<usize> { sync_offline_profile(&self.0.join("source"), &self.0.join("target")) }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

    #[test]
    fn upgrade_preserves_bank_personal_files_population_and_cp1252_includes() {
        let f = Fixture::new();
        std::fs::write(f.0.join("target/ragnarok.db"), b"saved players").unwrap();
        std::fs::write(f.0.join("target/conf/import/battle_conf.txt"), b"custom setting").unwrap();
        assert!(f.run().unwrap() >= 28);
        let npc = std::fs::read(f.0.join("target/npc/scripts_custom.conf")).unwrap();
        assert!(npc.starts_with(b"// coment\xe1rio\r\nnpc: npc/custom/my-npc.txt\r\n"));
        assert_eq!(std::fs::read(f.0.join("target/ragnarok.db")).unwrap(), b"saved players");
        assert_eq!(std::fs::read(f.0.join("target/conf/import/battle_conf.txt")).unwrap(), b"custom setting");
        assert_eq!(f.run().unwrap(), 0);
    }

    #[test]
    fn new_payload_updates_owned_script_and_keeps_first_backup() {
        let f = Fixture::new(); f.run().unwrap();
        let script = f.0.join("source/npc/custom/ragswitch_starter_gift.txt");
        std::fs::write(&script, b"npc version 2").unwrap(); assert_eq!(f.run().unwrap(), 1);
        std::fs::write(&script, b"npc version 3").unwrap(); assert_eq!(f.run().unwrap(), 1);
        assert_eq!(std::fs::read(f.0.join("target/npc/custom/ragswitch_starter_gift.txt.pre-offline-profile")).unwrap(), b"bundled gift\n");
    }

    #[test]
    fn commented_or_duplicate_includes_are_enabled_once_with_rates_last() {
        let f = Fixture::new();
        std::fs::write(f.0.join("target/conf/battle_athena.conf"), b"//import: conf/battle/nostalre_rates.conf\nimport: conf/battle/nostalre_rates.conf\nimport: conf/import/battle_conf.txt\nimport: conf/battle/nostalre_rates.conf\nbase_exp_rate: 100\n").unwrap();
        f.run().unwrap();
        let text = std::fs::read_to_string(f.0.join("target/conf/battle_athena.conf")).unwrap();
        assert_eq!(text.lines().filter(|line| *line == "import: conf/battle/nostalre_rates.conf").count(), 1);
        assert!(text.ends_with("base_exp_rate: 100\nimport: conf/battle/nostalre_rates.conf\n"));
        assert_eq!(f.run().unwrap(), 0);
    }

    #[test]
    fn missing_payload_leaves_existing_config_untouched() {
        let f = Fixture::new();
        let old = std::fs::read(f.0.join("target/npc/scripts_custom.conf")).unwrap();
        std::fs::remove_file(f.0.join("source/conf/battle/nostalre_rates.conf")).unwrap();
        assert_eq!(f.run().unwrap_err().kind(), io::ErrorKind::NotFound);
        assert_eq!(std::fs::read(f.0.join("target/npc/scripts_custom.conf")).unwrap(), old);
        assert!(!f.0.join("target/npc/custom/nostalre_job_changer.txt").exists());
    }

    #[test]
    fn infinite_wing_upgrade_preserves_custom_database_and_is_idempotent() {
        let f=Fixture::new();
        let before=std::fs::read(f.0.join("target/db/item_db.yml")).unwrap();
        f.run().unwrap();
        let after=std::fs::read(f.0.join("target/db/item_db.yml")).unwrap();
        assert!(after.starts_with(&before));
        assert_eq!(patch_item_import(&after).unwrap(), after);
        assert!(f.0.join("target/db/ragswitch_classic_items.yml").is_file());
        assert!(f.0.join("target/npc/custom/ragswitch_starter_gift.txt").is_file());
        assert_eq!(f.run().unwrap(),0);
    }

    #[test]
    fn quest_marker_upgrade_preserves_cp1252_logic_and_custom_changes_and_is_idempotent() {
        let f=Fixture::new();
        let path=f.0.join("target").join(TRAINING_GROUNDS);
        let before=std::fs::read(&path).unwrap();
        f.run().unwrap();
        let after=std::fs::read(&path).unwrap();
        assert!(after.starts_with(b"// tradu\xe7\xe3o e personaliza\xe7\xe3o\r\n"));
        assert_eq!(after.windows(b"ragswitch-pre-quest-guide-v1".len()).filter(|p|*p==b"ragswitch-pre-quest-guide-v1").count(),3);
        assert_eq!(std::fs::read(path.with_extension("txt.pre-offline-profile")).unwrap(),before);
        assert_eq!(patch_quest_markers(&after).unwrap(),after);
        let mut customized=after.clone();customized.extend(b"// personal NPC kept\r\n");
        assert_eq!(patch_quest_markers(&customized).unwrap(),customized);
        assert_eq!(patch_campaign_contacts(&after,TRAINING_GROUNDS).unwrap(),after);
    }

    #[test]
    fn quest_marker_upgrade_checks_all_npcs_before_writing_anything() {
        let f=Fixture::new();
        let path=f.0.join("target").join(TRAINING_GROUNDS);
        let before=std::fs::read(&path).unwrap();
        std::fs::write(f.0.join("source").join(TRAINING_GROUNDS),b"invalid bundle without expected NPCs").unwrap();
        assert_eq!(f.run().unwrap_err().kind(),io::ErrorKind::InvalidData);
        assert!(!f.0.join("target/npc/custom/nostalre_job_changer.txt").exists());
        assert_eq!(std::fs::read(path).unwrap(),before);
    }
}
