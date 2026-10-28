//! A picture of our character for the Android app's home screen, saved each time the player leaves the map world
//! (back to character select, the window closing, the app going to the background). The hub reads
//! `snapshots/<char_id>.png` from the working directory (the app's files dir on Android).

use std::path::{Path, PathBuf};
use std::time::Instant;

use nostalre_game::app_state::AppState;
use nostalre_renderer::portrait::{CpuSheet, compose_portrait};

use crate::App;

/// The folder the pictures go in, under the working directory.
const SNAPSHOT_DIR: &str = "snapshots";

impl App {
    /// Saves our character as the game draws it standing (`portrait::compose_portrait`) to
    /// `snapshots/<char_id>.png`, over the last one. Nothing outside the map world.
    pub(crate) fn save_player_snapshot(&self) {
        if self.game.session.app_state != AppState::InGame {
            return;
        }
        let Some(char_id) = self
            .game
            .session
            .login_session
            .as_ref()
            .map(|s| s.char_id)
            .filter(|&id| id != 0)
        else {
            return;
        };
        let Some(gid) = self.game.world.entities.player_id() else {
            return;
        };
        let Some(job) = self.game.world.entities.get(gid).map(|e| e.job) else {
            return;
        };
        let Some(look) = self.unit_look(gid) else {
            return;
        };
        let start = Instant::now();
        let (head_job, orc_face, is_gm, robe) = self.unit_look_extras(gid, look.job);
        let Some(sprite) = self.player_entity_sprite(
            look.job,
            head_job,
            look.sex,
            look.head,
            look.hair_color,
            look.cloth_color,
            look.weapon,
            look.weapon_look,
            look.head_top,
            look.head_mid,
            look.head_bottom,
            look.shield,
            robe,
            orc_face,
            is_gm,
            CpuSheet::from_data,
        ) else {
            tracing::warn!("snapshot: no sprite for char {char_id} (job {})", look.job);
            return;
        };
        let portrait = compose_portrait(&sprite, nostalre_game::sprite_path::baby_body_scale(job));
        let written = portrait
            .encode_png()
            .map_err(std::io::Error::other)
            .and_then(|png| write_snapshot(Path::new(SNAPSHOT_DIR), char_id, &png));
        match written {
            Ok(path) => tracing::info!(
                "snapshot: {} in {} ms",
                path.display(),
                start.elapsed().as_millis()
            ),
            Err(e) => tracing::warn!("snapshot: char {char_id} not saved: {e}"),
        }
    }
}

/// The picture of character `char_id` in `dir`.
fn snapshot_path(dir: &Path, char_id: u32) -> PathBuf {
    dir.join(format!("{char_id}.png"))
}

/// Writes `png` as character `char_id`'s picture in `dir` (made if missing), through a temporary file renamed over
/// the old one, so the hub never reads half a picture.
fn write_snapshot(dir: &Path, char_id: u32, png: &[u8]) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = snapshot_path(dir, char_id);
    let partial = dir.join(format!("{char_id}.png.part"));
    std::fs::write(&partial, png)?;
    std::fs::rename(&partial, &path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_named_by_the_character_id() {
        assert_eq!(
            snapshot_path(Path::new("snapshots"), 150000),
            Path::new("snapshots/150000.png")
        );
    }

    #[test]
    fn writing_makes_the_folder_and_overwrites_the_last_picture() {
        let dir = std::env::temp_dir().join(format!("nostalre-snapshot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = write_snapshot(&dir, 150000, b"first").unwrap();
        assert_eq!(path, dir.join("150000.png"));
        write_snapshot(&dir, 150000, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["150000.png"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
