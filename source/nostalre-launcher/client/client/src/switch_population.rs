//! One-time import of the new population preset on existing SD installations.
//! The generic installer deliberately preserves files already on the SD.
use std::io::{self, Write};
use std::path::Path;

pub fn ensure_population_import(root: &Path) -> io::Result<bool> {
    let battle = root.join("conf/battle_athena.conf");
    let original = std::fs::read_to_string(&battle)?;
    let population = "conf/battle/population_engine.conf";
    if original.lines().any(|line| {
        line.trim().split_once(':').is_some_and(|(key, value)|
            key.trim() == "import" && value.trim() == population)
    }) {
        return Ok(false);
    }
    // Don't install an import pointing at a missing resource after an interrupted
    // payload extraction. Retrying the update can complete extraction first.
    if !root.join(population).is_file() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "Perfil de populacao ausente"));
    }
    let backup = root.join("conf/battle_athena.conf.pre-population-50");
    match std::fs::OpenOptions::new().write(true).create_new(true).open(&backup) {
        Ok(mut file) => { file.write_all(original.as_bytes())?; file.sync_all()?; }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let added = "// RagSWITCH: populacao automatica; ajustes pessoais continuam em conf/import.\nimport: conf/battle/population_engine.conf\n";
    // Preserve the user's import as the final override, wherever it was placed.
    let mut updated = String::with_capacity(original.len() + added.len() + 1);
    let mut inserted = false;
    for line in original.split_inclusive('\n') {
        if !inserted && line.trim().split_once(':').is_some_and(|(key, value)|
            key.trim() == "import" && value.trim() == "conf/import/battle_conf.txt") {
            updated.push_str(added);
            inserted = true;
        }
        updated.push_str(line);
    }
    if !inserted {
        if !updated.ends_with('\n') { updated.push('\n'); }
        updated.push_str(added);
    }
    let temporary = root.join("conf/.population-50.installing");
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(updated.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(temporary, battle)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new(text: &str) -> Self {
            let root = std::env::temp_dir().join(format!("population-install-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            std::fs::create_dir_all(root.join("conf/battle")).unwrap();
            std::fs::create_dir_all(root.join("conf/import")).unwrap();
            std::fs::write(root.join("conf/battle_athena.conf"), text).unwrap();
            std::fs::write(root.join("conf/battle/population_engine.conf"), "population_engine_enable: on\npopulation_engine_max_count: 50\n").unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

    #[test]
    fn existing_sd_gets_population_without_changing_save_or_personal_overrides() {
        let old = "// existing settings\nimport: conf/battle/player.conf\nimport: conf/import/battle_conf.txt\n";
        let f = Fixture::new(old);
        std::fs::write(f.0.join("ragnarok.db"), b"two saved characters").unwrap();
        std::fs::write(f.0.join("conf/import/battle_conf.txt"), "base_exp_rate: 250\n").unwrap();
        assert!(ensure_population_import(&f.0).unwrap());
        let updated = std::fs::read_to_string(f.0.join("conf/battle_athena.conf")).unwrap();
        assert!(updated.find("import: conf/battle/population_engine.conf").unwrap() < updated.find("import: conf/import/battle_conf.txt").unwrap());
        assert_eq!(std::fs::read_to_string(f.0.join("conf/battle_athena.conf.pre-population-50")).unwrap(), old);
        assert_eq!(std::fs::read(f.0.join("ragnarok.db")).unwrap(), b"two saved characters");
        assert_eq!(std::fs::read_to_string(f.0.join("conf/import/battle_conf.txt")).unwrap(), "base_exp_rate: 250\n");
        assert!(!ensure_population_import(&f.0).unwrap());
    }
    #[test]
    fn commented_import_is_not_treated_as_enabled() {
        let f = Fixture::new("//import: conf/battle/population_engine.conf\r\n");
        assert!(ensure_population_import(&f.0).unwrap());
        assert!(!ensure_population_import(&f.0).unwrap());
    }
    #[test]
    fn missing_payload_does_not_break_existing_installation() {
        let old = "import: conf/import/battle_conf.txt\n";
        let f = Fixture::new(old);
        std::fs::remove_file(f.0.join("conf/battle/population_engine.conf")).unwrap();
        assert_eq!(ensure_population_import(&f.0).unwrap_err().kind(), io::ErrorKind::NotFound);
        assert_eq!(std::fs::read_to_string(f.0.join("conf/battle_athena.conf")).unwrap(), old);
        assert!(!f.0.join("conf/battle_athena.conf.pre-population-50").exists());
    }
    #[test]
    fn restart_after_interrupted_migration_preserves_original_backup() {
        let f = Fixture::new("import: conf/import/battle_conf.txt");
        std::fs::write(f.0.join("conf/battle_athena.conf.pre-population-50"), "original backup").unwrap();
        assert!(ensure_population_import(&f.0).unwrap());
        assert_eq!(std::fs::read_to_string(f.0.join("conf/battle_athena.conf.pre-population-50")).unwrap(), "original backup");
    }
}
