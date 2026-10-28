//! `custom.skill.body_effect`: what a unit's body shows while a status lasts or a counter is up, and the counters'
//! kRO looks the rules fall back to (`docs/nostalre-body-effects.md`).

use crate::config::{HexColor, status_key};
use models::enums::EnumWithStringValue;
use nostalre_game::character::classic_job_name;
use nostalre_game::effect::effects::body_aura::{
    self, BodyAuraSpec, GlowLayer, GroundLayer, GroundLook, MoteLayer, OrbLayer, SpotLook,
};
use serde::{Deserialize, Serialize};

/// What the server counts on a unit and kRO draws around it
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Counter {
    /// A Monk's spirit spheres, a Gunslinger's coins (0x01d0, 0x01e1)
    Spirits,
    /// A Rune Knight's Millennium Shields (0x0440)
    MillenniumShield,
    /// A Soul Reaper's souls (0x0b73)
    SoulEnergy,
    /// A Kagerou's or Oboro's charms, one counter an element (0x08cf)
    WaterCharm,
    EarthCharm,
    FireCharm,
    WindCharm,
}

impl Counter {
    /// The charms by the element 0x08cf numbers them (1 water to 4 wind)
    const CHARMS: [Counter; 4] = [
        Self::WaterCharm,
        Self::EarthCharm,
        Self::FireCharm,
        Self::WindCharm,
    ];

    /// Every charm counter's count after 0x08cf: `count` for `element`'s, 0 for the others (a unit holds one element
    /// at a time), 0 for all at element 0 (the last one gone)
    pub fn charms(element: u8, count: u8) -> [(Counter, u8); 4] {
        std::array::from_fn(|i| {
            let charm = Self::CHARMS[i];
            (
                charm,
                if usize::from(element) == i + 1 {
                    count
                } else {
                    0
                },
            )
        })
    }
}

/// When a rule shows: `{"status": "EFST_<NAME>"}` or `{"counter": "soul_energy"}`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// By name: `EFST_AURABLADE`, `AURABLADE` or `Aurablade` alike
    Status(String),
    Counter(Counter),
}

/// One body effect: when, for whom, and its layers (each optional, each field defaulted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyEffectRule {
    #[serde(flatten)]
    pub trigger: Trigger,
    /// Only these jobs, by name or by the line they grew from (`Monk` takes a Sura too); empty is anyone
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jobs: Vec<String>,
    /// From this count up (a look per count range: the rule with the highest `min_count` reached wins)
    #[serde(default)]
    pub min_count: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orbs: Option<OrbsRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground: Option<GroundRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glow: Option<GlowRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motes: Option<MotesRule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpotLookName {
    Glow,
    Spark,
    Soft,
    GreenFlame,
    RedFlame,
    WaterOrb,
    StoneOrb,
    FireOrb,
    LightningOrb,
    FireCharm,
    WaterCharm,
    EarthCharm,
    WindCharm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroundLookName {
    Ring,
    Disc,
}

/// Orbs circling the body; `count` fixed, or without it one a counted unit (1 for a status), up to `max`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OrbsRule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u8>,
    pub max: u8,
    /// How much each count past `max` swells the orbs and lights a halo round them
    pub swell: f32,
    pub radius: f32,
    pub height: f32,
    pub speed: f32,
    /// Degrees a second each orb turns about its own middle (negative, the other way)
    pub spin: f32,
    pub size: f32,
    pub wobble: f32,
    /// Spread evenly around the circle; otherwise 72 degrees apart, as kRO lays a Monk's five
    pub even: bool,
    /// A wheel standing at the body's back, turning with it, instead of a ring round it
    pub upright: bool,
    /// How far behind the body the circle's middle sits
    pub back: f32,
    /// How much darker an upright wheel's orbs go toward its bottom (0 none, 1 black there)
    pub shade: f32,
    /// How much smaller they go toward the bottom (0 none, 0.25 a quarter there)
    pub shrink: f32,
    pub look: SpotLookName,
    pub inner: HexColor,
    pub outer: HexColor,
    pub intensity: f32,
}

impl Default for OrbsRule {
    fn default() -> Self {
        Self {
            count: None,
            max: body_aura::MAX_ORBS,
            swell: 0.0,
            radius: 7.0,
            height: 11.0,
            speed: 1.0,
            spin: 0.0,
            size: 1.6,
            wobble: 0.5,
            even: true,
            upright: false,
            back: 0.0,
            shade: 0.0,
            shrink: 0.0,
            look: SpotLookName::Glow,
            inner: HexColor::rgb(255, 255, 255),
            outer: HexColor::rgb(90, 50, 255),
            intensity: 0.8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GroundRule {
    pub look: GroundLookName,
    pub radius: f32,
    /// Degrees a second
    pub spin: f32,
    /// How far the radius swells and shrinks, a share of it
    pub breath: f32,
    pub inner: HexColor,
    pub outer: HexColor,
    pub intensity: f32,
}

impl Default for GroundRule {
    fn default() -> Self {
        Self {
            look: GroundLookName::Ring,
            radius: 6.0,
            spin: 30.0,
            breath: 0.1,
            inner: HexColor::rgb(255, 255, 255),
            outer: HexColor::rgb(90, 50, 255),
            intensity: 0.6,
        }
    }
}

/// An outline glow behind the body, `size` pixels wide, breathing by `breath`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GlowRule {
    pub color: HexColor,
    pub size: f32,
    pub breath: f32,
    pub intensity: f32,
}

impl Default for GlowRule {
    fn default() -> Self {
        Self {
            color: HexColor::rgb(90, 50, 255),
            size: 5.0,
            breath: 1.5,
            intensity: 0.7,
        }
    }
}

/// Motes rising off the body: `rate` a second, each rising `rise` in `life` seconds, from within `spread`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MotesRule {
    pub rate: f32,
    pub rise: f32,
    pub life: f32,
    pub spread: f32,
    pub size: f32,
    pub look: SpotLookName,
    pub inner: HexColor,
    pub outer: HexColor,
    pub intensity: f32,
}

impl Default for MotesRule {
    fn default() -> Self {
        Self {
            rate: 8.0,
            rise: 20.0,
            life: 1.2,
            spread: 5.0,
            size: 1.0,
            look: SpotLookName::Spark,
            inner: HexColor::rgb(255, 255, 255),
            outer: HexColor::rgb(90, 50, 255),
            intensity: 0.8,
        }
    }
}

fn rgb(color: &HexColor) -> [f32; 3] {
    let [r, g, b, _] = color.rgba();
    [r, g, b]
}

impl SpotLookName {
    fn look(self) -> SpotLook {
        match self {
            Self::Glow => SpotLook::Glow,
            Self::Spark => SpotLook::Spark,
            Self::Soft => SpotLook::Soft,
            Self::GreenFlame => SpotLook::GreenFlame,
            Self::RedFlame => SpotLook::RedFlame,
            Self::WaterOrb => SpotLook::WaterOrb,
            Self::StoneOrb => SpotLook::StoneOrb,
            Self::FireOrb => SpotLook::FireOrb,
            Self::LightningOrb => SpotLook::LightningOrb,
            Self::FireCharm => SpotLook::FireCharm,
            Self::WaterCharm => SpotLook::WaterCharm,
            Self::EarthCharm => SpotLook::EarthCharm,
            Self::WindCharm => SpotLook::WindCharm,
        }
    }
}

impl BodyEffectRule {
    pub fn spec(&self) -> BodyAuraSpec {
        BodyAuraSpec {
            orbs: self.orbs.as_ref().map(|o| OrbLayer {
                count: o.count,
                max: o.max,
                swell: o.swell,
                radius: o.radius,
                height: o.height,
                speed: o.speed,
                spin: o.spin,
                size: o.size,
                wobble: o.wobble,
                even: o.even,
                upright: o.upright,
                back: o.back,
                shade: o.shade,
                shrink: o.shrink,
                look: o.look.look(),
                inner: rgb(&o.inner),
                outer: rgb(&o.outer),
                intensity: o.intensity,
                tiers: &[],
            }),
            ground: self.ground.as_ref().map(|g| GroundLayer {
                look: match g.look {
                    GroundLookName::Ring => GroundLook::Ring,
                    GroundLookName::Disc => GroundLook::Disc,
                },
                radius: g.radius,
                spin: g.spin,
                breath: g.breath,
                inner: rgb(&g.inner),
                outer: rgb(&g.outer),
                intensity: g.intensity,
            }),
            glow: self.glow.as_ref().map(|g| GlowLayer {
                color: g.color.rgb8(),
                size: g.size,
                breath: g.breath,
                intensity: g.intensity,
            }),
            motes: self.motes.as_ref().map(|m| MoteLayer {
                rate: m.rate,
                rise: m.rise,
                life: m.life,
                spread: m.spread,
                size: m.size,
                look: m.look.look(),
                inner: rgb(&m.inner),
                outer: rgb(&m.outer),
                intensity: m.intensity,
            }),
        }
    }

    fn fires_on(&self, trigger: &Trigger) -> bool {
        match (&self.trigger, trigger) {
            (Trigger::Status(a), Trigger::Status(b)) => status_key(a) == status_key(b),
            (a, b) => a == b,
        }
    }

    fn fits(&self, job: u16, count: u8) -> bool {
        count >= self.min_count
            && (self.jobs.is_empty() || self.jobs.iter().any(|name| is_job(name, job)))
    }
}

/// `name` is the job's own or the line it grew from, in any spelling
fn is_job(name: &str, job: u16) -> bool {
    let key = |s: &str| s.to_ascii_lowercase().replace([' ', '_'], "");
    let own = nostalre_game::character::job_class_name(job);
    let line = classic_job_name(job).map(|j| j.as_str());
    [Some(own), line]
        .into_iter()
        .flatten()
        .any(|n| key(n) == key(name))
}

/// What `trigger` shows on a unit of `job` at `count`: the user's rules first, then a counter's built-in look. Of the
/// rules that fit, the one with the highest `min_count` wins, a job's own before anyone's.
pub fn body_effect(
    rules: &[BodyEffectRule],
    trigger: &Trigger,
    job: u16,
    count: u8,
) -> Option<BodyAuraSpec> {
    rules
        .iter()
        .filter(|r| r.fires_on(trigger) && r.fits(job, count))
        .max_by_key(|r| (r.min_count, !r.jobs.is_empty()))
        .map(BodyEffectRule::spec)
        .or_else(|| match trigger {
            Trigger::Counter(counter) => built_in(*counter, job, count),
            Trigger::Status(_) => None,
        })
}

/// A counter's built-in look: kRO's, but for the charms, ours
fn built_in(counter: Counter, job: u16, count: u8) -> Option<BodyAuraSpec> {
    use models::enums::class::JobName;
    if count == 0 {
        return None;
    }
    let orbs = match counter {
        Counter::Spirits => match classic_job_name(job) {
            Some(JobName::Champion) => body_aura::CHOOKGI2,
            Some(JobName::Gunslinger) => body_aura::CHOOKGI3,
            _ => body_aura::CHOOKGI,
        },
        Counter::MillenniumShield => body_aura::MILLENNIUM_SHIELD,
        Counter::SoulEnergy => body_aura::SOUL_ENERGY,
        Counter::WaterCharm => body_aura::charms(SpotLook::WaterCharm),
        Counter::EarthCharm => body_aura::charms(SpotLook::EarthCharm),
        Counter::FireCharm => body_aura::charms(SpotLook::FireCharm),
        Counter::WindCharm => body_aura::charms(SpotLook::WindCharm),
    };
    Some(body_aura::orbs_only(orbs))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONK: u16 = 15;
    const CHAMPION: u16 = 4016;
    const SURA: u16 = 4070;
    const GUNSLINGER: u16 = 24;

    fn rules(json: &str) -> Vec<BodyEffectRule> {
        serde_json::from_str(json).unwrap()
    }

    fn status(name: &str) -> Trigger {
        Trigger::Status(name.into())
    }

    #[test]
    fn a_rule_reads_its_trigger_and_defaults_every_field_it_leaves_out() {
        let r = rules(
            r##"[{"status": "EFST_AURABLADE", "glow": {"color": "#FF0000"}},
                 {"counter": "soul_energy", "min_count": 10, "orbs": {"count": 3, "outer": "#00FF00"}}]"##,
        );
        assert_eq!(r[0].trigger, status("EFST_AURABLADE"));
        let glow = r[0].spec().glow.unwrap();
        assert_eq!(glow.color, [255, 0, 0]);
        assert_eq!(glow.size, GlowRule::default().size);
        assert!(r[0].spec().orbs.is_none());
        assert_eq!(r[1].trigger, Trigger::Counter(Counter::SoulEnergy));
        let orbs = r[1].spec().orbs.unwrap();
        assert_eq!((orbs.count, orbs.outer), (Some(3), [0.0, 1.0, 0.0]));
        assert_eq!(orbs.radius, OrbsRule::default().radius);
        let back: Vec<BodyEffectRule> = rules(&serde_json::to_string(&r).unwrap());
        assert_eq!(back[1].trigger, r[1].trigger);
    }

    #[test]
    fn a_misspelt_look_or_counter_fails_the_load() {
        assert!(serde_json::from_str::<Vec<BodyEffectRule>>(r#"[{"counter": "souls"}]"#).is_err());
        assert!(
            serde_json::from_str::<Vec<BodyEffectRule>>(
                r#"[{"status": "X", "orbs": {"look": "star"}}]"#
            )
            .is_err()
        );
    }

    #[test]
    fn a_status_rule_matches_by_any_spelling_and_statuses_have_no_built_in_look() {
        let r = rules(r#"[{"status": "EFST_AURABLADE", "motes": {}}]"#);
        for name in ["AURABLADE", "Aurablade", "aura_blade"] {
            assert!(body_effect(&r, &status(name), MONK, 1).is_some(), "{name}");
        }
        assert!(body_effect(&r, &status("Lightblade"), MONK, 1).is_none());
        assert!(body_effect(&[], &status("Aurablade"), MONK, 1).is_none());
    }

    #[test]
    fn the_counters_fall_back_to_their_built_in_look_by_job() {
        let spirits = Trigger::Counter(Counter::Spirits);
        let orbs = |job| body_effect(&[], &spirits, job, 5).unwrap().orbs.unwrap();
        assert_eq!(orbs(MONK), body_aura::CHOOKGI);
        assert_eq!(orbs(CHAMPION), body_aura::CHOOKGI2);
        assert_eq!(orbs(SURA), body_aura::CHOOKGI, "a Sura grew from the Monk");
        assert_eq!(orbs(GUNSLINGER), body_aura::CHOOKGI3);
        let shields = body_effect(&[], &Trigger::Counter(Counter::MillenniumShield), 4060, 3);
        assert_eq!(shields.unwrap().orbs, Some(body_aura::MILLENNIUM_SHIELD));
        assert!(
            body_effect(&[], &spirits, MONK, 0).is_none(),
            "nothing at 0"
        );
        let souls = body_effect(&[], &Trigger::Counter(Counter::SoulEnergy), 4240, 13);
        assert_eq!(souls.unwrap().orbs, Some(body_aura::SOUL_ENERGY));
        let fire = body_effect(&[], &Trigger::Counter(Counter::FireCharm), 4211, 10).unwrap();
        let fire = fire.orbs.unwrap();
        assert_eq!(
            (fire.look, fire.max, fire.even),
            (SpotLook::FireCharm, 10, true),
            "all ten, evenly"
        );
        assert!(
            fire.upright && fire.back > 0.0,
            "on a wheel at the body's back"
        );
    }

    #[test]
    fn a_charm_packet_sets_its_element_and_takes_the_others_off() {
        let counts = |element, count| Counter::charms(element, count).map(|(_, n)| n);
        assert_eq!(counts(3, 7), [0, 0, 7, 0], "fire");
        assert_eq!(Counter::charms(3, 7)[2].0, Counter::FireCharm);
        assert_eq!(counts(0, 0), [0; 4], "the last one gone");
        assert_eq!(counts(9, 5), [0; 4], "no such element");
    }

    #[test]
    fn a_users_rule_replaces_the_built_in_look_where_it_fits() {
        let r = rules(r#"[{"counter": "spirits", "jobs": ["Monk"], "ground": {}}]"#);
        let spirits = Trigger::Counter(Counter::Spirits);
        assert!(
            body_effect(&r, &spirits, SURA, 5).unwrap().ground.is_some(),
            "a Sura is of the Monk's line"
        );
        let champion = body_effect(&r, &spirits, CHAMPION, 5).unwrap();
        assert_eq!(
            champion.orbs,
            Some(body_aura::CHOOKGI2),
            "a Champion keeps kRO's"
        );
    }

    #[test]
    fn the_highest_min_count_reached_wins_and_a_jobs_own_before_anyones() {
        let r = rules(
            r##"[{"counter": "soul_energy", "orbs": {"outer": "#00FF00"}},
                 {"counter": "soul_energy", "min_count": 10, "orbs": {"outer": "#FF0000"}},
                 {"counter": "soul_energy", "min_count": 10, "jobs": ["Soul Reaper"], "orbs": {"outer": "#0000FF"}}]"##,
        );
        let souls = Trigger::Counter(Counter::SoulEnergy);
        let outer = |job, count| {
            body_effect(&r, &souls, job, count)
                .unwrap()
                .orbs
                .unwrap()
                .outer
        };
        assert_eq!(outer(4240, 5), [0.0, 1.0, 0.0]);
        assert_eq!(outer(4240, 10), [0.0, 0.0, 1.0]);
        assert_eq!(outer(MONK, 20), [1.0, 0.0, 0.0]);
    }
}
