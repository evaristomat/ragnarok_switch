//! Textures of nostalRE's own, built into the client: art kRO's archives lack. Each is served as an archive file
//! (`GrfArchive::set_override`), so the effects name it as they name kRO's. Made with `scripts/charm-sprite.py`.

use crate::effect_texture_path;

/// A Kagerou's or Oboro's charms: pixel-art vortex orbs, one an element (the user's, October 14)
pub const CHARM_FIRE: &str = "nostalre/charm_fire.png";
pub const CHARM_WATER: &str = "nostalre/charm_water.png";
pub const CHARM_EARTH: &str = "nostalre/charm_earth.png";
pub const CHARM_WIND: &str = "nostalre/charm_wind.png";

const OWN: &[(&str, &[u8])] = &[
    (CHARM_FIRE, include_bytes!("../assets/charm_fire.png")),
    (CHARM_WATER, include_bytes!("../assets/charm_water.png")),
    (CHARM_EARTH, include_bytes!("../assets/charm_earth.png")),
    (CHARM_WIND, include_bytes!("../assets/charm_wind.png")),
];

/// Every own texture's archive path and bytes
pub fn files() -> impl Iterator<Item = (String, &'static [u8])> {
    OWN.iter()
        .map(|&(name, bytes)| (effect_texture_path(name), bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_own_texture_is_a_square_png_where_the_effects_look() {
        for (path, bytes) in files() {
            assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "{path} is a PNG");
            let (w, h) = (&bytes[16..20], &bytes[20..24]);
            assert_eq!(w, h, "{path} is square");
            assert!(path.starts_with("data/texture/"), "{path}");
        }
        assert_eq!(
            files().next().unwrap().0,
            effect_texture_path(CHARM_FIRE),
            "found under the name the looks use"
        );
    }
}
