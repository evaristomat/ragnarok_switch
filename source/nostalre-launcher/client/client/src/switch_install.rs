//! Resource installation with one target-directory listing instead of one
//! existence stat/open per resource. Existing saves and custom files win.
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;

pub struct InstallProgress {
    pub files: usize,
    pub copied: usize,
    pub directory_lists: usize,
    pub target_metadata_queries: usize,
    pub repair_partial: bool,
}
impl InstallProgress {
    pub fn new(repair_partial: bool) -> Self {
        Self { files: 0, copied: 0, directory_lists: 0, target_metadata_queries: 0, repair_partial }
    }
}

pub fn install_tree(
    source: &Path, target: &Path, progress: &mut InstallProgress,
    on_file: &mut impl FnMut(usize, &Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;
    // Horizon's SD filesystem matches ASCII names without case sensitivity.
    let names: HashMap<OsString, OsString> = std::fs::read_dir(target)?
        .map(|entry| entry.map(|entry| { let name=entry.file_name(); (name.to_ascii_lowercase(),name) }))
        .collect::<Result<_, _>>()?;
    progress.directory_lists += 1;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        let present = names.get(&name.to_ascii_lowercase());
        let destination = target.join(present.unwrap_or(&name));
        if entry.file_type()?.is_dir() {
            install_tree(&entry.path(), &destination, progress, on_file)?;
        } else {
            let exists = present.is_some();
            let is_save = name.to_ascii_lowercase() == "ragnarok.db";
            let incomplete = if exists && progress.repair_partial && !is_save {
                progress.target_metadata_queries += 1;
                destination.metadata()?.len() != entry.metadata()?.len()
            } else { false };
            if !exists || incomplete {
                let temporary = destination.with_file_name(format!(".{}.installing", name.to_string_lossy()));
                std::fs::copy(entry.path(), &temporary)?;
                std::fs::rename(&temporary, &destination)?;
                progress.copied += 1;
            }
            progress.files += 1;
            on_file(progress.files, &destination)?;
        }
    }
    Ok(())
}

#[cfg(test)] mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("install_test_{}_{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
            std::fs::create_dir_all(root.join("source/npc")).unwrap();
            std::fs::write(root.join("source/ragnarok.db"), b"seed").unwrap();
            std::fs::write(root.join("source/npc/test.txt"), b"resource").unwrap();
            Self(root)
        }
        fn run(&self, repair: bool) -> InstallProgress {
            let mut progress = InstallProgress::new(repair);
            install_tree(&self.0.join("source"), &self.0.join("target"), &mut progress,&mut |_,_|Ok(())).unwrap();
            progress
        }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _=std::fs::remove_dir_all(&self.0); } }
    #[test] fn first_install_copies_and_completed_install_uses_no_file_metadata_queries() {
        let f=Fixture::new();assert_eq!(f.run(true).copied,2);
        let result=f.run(false);assert_eq!(result.files,2);assert_eq!(result.copied,0);
        assert_eq!(result.target_metadata_queries,0);assert_eq!(result.directory_lists,2);
    }
    #[test] fn missing_resource_repaired_without_overwriting_character_database_or_custom_script() {
        let f=Fixture::new();f.run(true);
        std::fs::write(f.0.join("target/ragnarok.db"),b"players and their saves").unwrap();
        std::fs::write(f.0.join("target/npc/test.txt"),b"custom script").unwrap();
        std::fs::write(f.0.join("source/npc/new.txt"),b"new resource").unwrap();
        let p=f.run(false);assert_eq!(p.copied,1);
        assert_eq!(std::fs::read(f.0.join("target/ragnarok.db")).unwrap(),b"players and their saves");
        assert_eq!(std::fs::read(f.0.join("target/npc/test.txt")).unwrap(),b"custom script");
        std::fs::remove_file(f.0.join("target/npc/test.txt")).unwrap();
        assert_eq!(f.run(false).copied,1);
    }
    #[test] fn interrupted_copy_is_repaired_but_database_is_always_preserved() {
        let f=Fixture::new();f.run(true);
        std::fs::write(f.0.join("target/ragnarok.db"),b"players").unwrap();
        std::fs::write(f.0.join("target/npc/test.txt"),b"truncated").unwrap();
        assert_eq!(f.run(true).copied,1);
        assert_eq!(std::fs::read(f.0.join("target/npc/test.txt")).unwrap(),b"resource");
        assert_eq!(std::fs::read(f.0.join("target/ragnarok.db")).unwrap(),b"players");
    }
    #[test] fn existing_sd_names_with_different_case_preserve_custom_data() {
        let f=Fixture::new();f.run(true);
        std::fs::rename(f.0.join("target/npc"),f.0.join("target/NPC")).unwrap();
        std::fs::rename(f.0.join("target/NPC/test.txt"),f.0.join("target/NPC/TEST.TXT")).unwrap();
        std::fs::write(f.0.join("target/NPC/TEST.TXT"),b"custom").unwrap();
        assert_eq!(f.run(false).copied,0);
        assert_eq!(std::fs::read(f.0.join("target/NPC/TEST.TXT")).unwrap(),b"custom");
    }
    #[test] fn cancellation_propagates_and_restart_finishes_the_install() {
        let f=Fixture::new();let mut p=InstallProgress::new(true);
        let rc=install_tree(&f.0.join("source"),&f.0.join("target"),&mut p,&mut |_,_|Err(std::io::ErrorKind::Interrupted.into()));
        assert_eq!(rc.unwrap_err().kind(),std::io::ErrorKind::Interrupted);
        assert_eq!(f.run(true).files,2);
    }
}
