//! What the recorders leave per Order line, how two of them merge and the text they make (kRO's
//! `CEquipmentProperties`, `..1Dim`, `..2Dim`: merge 0x666970 / 0x666a10, insert 0x667350, text 0x6688f0 /
//! 0x667730 / 0x667d40). Values are integers, as kRO's.
use super::order::{Line, Op, Sep};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prop {
    /// A line with no value: it shows or not
    Flag,
    OneDim(i32),
    /// Key (a skill, a group, a percent) to value
    TwoDim(BTreeMap<i32, i32>),
}

impl Prop {
    fn merge(&mut self, other: &Prop, op: Op) {
        match (self, other) {
            (Prop::OneDim(a), Prop::OneDim(b)) => match op {
                Op::Add => *a = a.wrapping_add(*b),
                Op::One => *a = a.wrapping_add(1),
                Op::Max => *a = (*a).max(*b),
                Op::DrainAdd => {}
            },
            (Prop::TwoDim(a), Prop::TwoDim(b)) => {
                for (&k, &v) in b {
                    match op {
                        Op::Add => {
                            let e = a.entry(k).or_default();
                            *e = e.wrapping_add(v);
                        }
                        Op::One => *a.entry(k).or_default() += 1,
                        // A new key starts at 0, so a negative value alone stays 0 (std::map's operator[])
                        Op::Max => {
                            let e = a.entry(k).or_default();
                            *e = (*e).max(v);
                        }
                        // Both the percent and the chance add up, while there is one entry (0x666a10 case 3)
                        Op::DrainAdd => {
                            if a.len() == 1 {
                                let (k0, v0) = a.pop_first().unwrap();
                                a.insert(k0.wrapping_add(k), v0.wrapping_add(v));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// One equipment's (or the total's) properties, by Order line
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Container {
    pub props: BTreeMap<usize, Prop>,
    /// The line last added under each `uniq2` (0x667350, 0x666c40)
    last_uniq2: HashMap<String, usize>,
}

impl Container {
    /// A record joins the line's property, or starts it (0x667350)
    pub fn insert(&mut self, line: usize, prop: Prop, lines: &[Line]) {
        match self.props.get_mut(&line) {
            Some(existing) => existing.merge(&prop, lines[line].op),
            None => {
                self.props.insert(line, prop);
            }
        }
        if let Some(uniq2) = &lines[line].uniq2 {
            self.last_uniq2.insert(uniq2.clone(), line);
        }
    }

    pub fn merge(&mut self, other: &Container, lines: &[Line]) {
        for (&line, prop) in &other.props {
            self.insert(line, prop.clone(), lines);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.props.is_empty()
    }

    /// Of the lines naming the same thing, which stay (0x668b40): by `uniq1` the first in Order order; for one
    /// item (`per_item`), by `uniq2` among lines without a `uniq1`, the last one added
    pub fn uniq_pass(&mut self, lines: &[Line], per_item: bool) {
        let mut seen = HashSet::new();
        let last_uniq2 = &self.last_uniq2;
        self.props.retain(|&index, _| {
            let line = &lines[index];
            match (&line.uniq1, &line.uniq2) {
                (Some(uniq1), _) if !uniq1.is_empty() => seen.insert(uniq1.as_str()),
                (_, Some(uniq2)) if per_item && !uniq2.is_empty() => last_uniq2
                    .get(uniq2)
                    .is_some_and(|&last| lines[last].name == line.name),
                _ => true,
            }
        });
    }
}

/// The names a 2-dim line's key is shown by
pub trait SepNames {
    fn skill_name(&self, skill: i32) -> String;
    /// `ADSkill[key]`, the key itself when the table has none
    fn add_damage_skill(&self, key: i32) -> i32;
    /// `SkillGroup[key]`'s skills, none for a group the table lacks
    fn skill_group(&self, key: i32) -> Option<&[i32]>;
}

/// The number as kRO's window prints it: clamped by `valOpt` MIN and MAX, divided by DIV, cut to FLOAT
/// decimals by `floor` on the signed value; with two symbols a negative one is printed without its sign.
/// None for a zero
fn number(line: &Line, value: i32) -> Option<(String, bool)> {
    let mut v = value;
    if let Some(min) = line.min {
        v = v.max(min);
    }
    if let Some(max) = line.max {
        v = v.min(max);
    }
    let mut d = f64::from(v);
    if let Some(div) = line.div.filter(|&div| div != 0) {
        d /= f64::from(div);
    }
    let sign = if d < 0.0 && line.syms.is_some() {
        -1.0
    } else {
        1.0
    };
    let scale = 10f64.powi(line.decimals as i32);
    let cut = (d * scale).floor() / scale;
    if cut == 0.0 || cut.is_nan() {
        return None;
    }
    Some((format!("{:.*}", line.decimals, sign * cut), cut <= 0.0))
}

/// The marker kRO's line widget turns into the negative colour
pub const NEGATIVE_MARK: &str = "{~}";

fn fill(line: &Line, val: &str, sep: &str, negative: bool) -> String {
    let mut text = line.name.replace("{val}", val).replace("{sep}", sep);
    if let Some(syms) = &line.syms {
        text = text.replace("{sym}", &syms[negative as usize]);
        if negative {
            text.insert_str(0, NEGATIVE_MARK);
        }
    }
    text
}

fn sep_text(line: &Line, key: i32, names: &dyn SepNames) -> Result<Option<String>, ()> {
    let Some(sep) = line.sep else {
        return Ok(Some(String::new()));
    };
    let name = match sep {
        Sep::SkillName => names.skill_name(key),
        Sep::AddDamageSkillName => match names.add_damage_skill(key) {
            0 => return Ok(None),
            skill => names.skill_name(skill),
        },
        Sep::Number => return Ok(Some(key.to_string())),
        Sep::SkillGroup => {
            if key == 0 {
                return Ok(Some(String::new()));
            }
            let skills = names.skill_group(key).ok_or(())?;
            let mut joined = skills
                .iter()
                .map(|&s| names.skill_name(s))
                .collect::<Vec<_>>()
                .join(", ");
            if !joined.is_empty() {
                joined.push(' ');
            }
            return Ok(Some(joined));
        }
    };
    Ok((!name.is_empty()).then_some(name))
}

/// The property's text as the window asks it (clamped, as worn); a 2-dim one has a line per key, joined by "\n"
pub fn text(line: &Line, prop: &Prop, names: &dyn SepNames) -> String {
    match prop {
        Prop::Flag => match &line.syms {
            None => line.name.clone(),
            Some(syms) => line.name.replace("{sym}", &syms[0]),
        },
        Prop::OneDim(v) => {
            if *v == 0 {
                return String::new();
            }
            number(line, *v).map_or_else(String::new, |(val, negative)| {
                fill(line, &val, "", negative)
            })
        }
        Prop::TwoDim(entries) => {
            let mut out = Vec::new();
            for (&key, &value) in entries {
                let sep = match sep_text(line, key, names) {
                    Ok(Some(sep)) => sep,
                    Ok(None) => continue,
                    // A group the table lacks leaves the whole line out (0x667d40's error flag)
                    Err(()) => return String::new(),
                };
                if let Some((val, negative)) = number(line, value) {
                    out.push(fill(line, &val, &sep, negative));
                }
            }
            out.join("\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(name: &str, op: Op, syms: bool) -> Line {
        Line {
            category: 0,
            name: name.into(),
            plus: "AddExtParam".into(),
            val_arg: 3,
            op,
            syms: syms.then(|| ["+".into(), "-".into()]),
            sep: None,
            uniq1: None,
            uniq2: None,
            min: None,
            max: None,
            div: None,
            decimals: 0,
        }
    }

    struct NoNames;
    impl SepNames for NoNames {
        fn skill_name(&self, skill: i32) -> String {
            format!("Skill{skill}")
        }
        fn add_damage_skill(&self, key: i32) -> i32 {
            if key == 2056 { 0 } else { key }
        }
        fn skill_group(&self, key: i32) -> Option<&[i32]> {
            (key == 1004).then_some(&[19, 14][..])
        }
    }

    #[test]
    fn a_negative_value_takes_the_second_symbol_and_the_mark() {
        let l = line("{sym}{val}#ATK", Op::Add, true);
        assert_eq!(text(&l, &Prop::OneDim(29), &NoNames), "+29#ATK");
        assert_eq!(text(&l, &Prop::OneDim(-5), &NoNames), "{~}-5#ATK");
        assert_eq!(text(&l, &Prop::OneDim(0), &NoNames), "");
    }

    #[test]
    fn div_and_float_cut_by_floor_on_the_signed_value() {
        let mut l = line("{val}s#{sym} Delay", Op::Add, true);
        l.div = Some(1000);
        l.decimals = 1;
        assert_eq!(text(&l, &Prop::OneDim(1550), &NoNames), "1.5s#+ Delay");
        // floor(-2.5) is -3: kRO prints 3 with the second symbol
        l.decimals = 0;
        assert_eq!(text(&l, &Prop::OneDim(-2500), &NoNames), "{~}3s#- Delay");
    }

    #[test]
    fn clamp_by_min_and_max() {
        let mut l = line("{val}%#ignore", Op::Add, false);
        l.min = Some(0);
        l.max = Some(100);
        assert_eq!(text(&l, &Prop::OneDim(150), &NoNames), "100%#ignore");
        assert_eq!(text(&l, &Prop::OneDim(-3), &NoNames), "");
    }

    #[test]
    fn merges_follow_the_op() {
        let lines = [line("a", Op::Add, false), line("m", Op::Max, false)];
        let mut c = Container::default();
        c.insert(0, Prop::OneDim(3), &lines);
        c.insert(0, Prop::OneDim(4), &lines);
        c.insert(1, Prop::OneDim(3), &lines);
        c.insert(1, Prop::OneDim(2), &lines);
        assert_eq!(c.props[&0], Prop::OneDim(7));
        assert_eq!(c.props[&1], Prop::OneDim(3));
    }

    #[test]
    fn drains_add_the_percent_and_the_chance() {
        let lines = [line("d", Op::DrainAdd, false)];
        let mut c = Container::default();
        c.insert(0, Prop::TwoDim(BTreeMap::from([(5, 10)])), &lines);
        c.insert(0, Prop::TwoDim(BTreeMap::from([(3, 2)])), &lines);
        assert_eq!(c.props[&0], Prop::TwoDim(BTreeMap::from([(8, 12)])));
    }

    #[test]
    fn the_first_uniq1_line_stays_and_the_last_uniq2_one_for_an_item() {
        let mut lines = [
            line("100%", Op::One, false),
            line("x%", Op::Add, false),
            line("a", Op::Add, false),
            line("b", Op::Add, false),
        ];
        lines[0].uniq1 = Some("IgnoreDEF".into());
        lines[1].uniq1 = Some("IgnoreDEF".into());
        lines[2].uniq2 = Some("1".into());
        lines[3].uniq2 = Some("1".into());
        let mut c = Container::default();
        c.insert(0, Prop::Flag, &lines);
        c.insert(1, Prop::OneDim(5), &lines);
        c.insert(3, Prop::OneDim(1), &lines);
        c.insert(2, Prop::OneDim(1), &lines);
        let mut total = c.clone();
        total.uniq_pass(&lines, false);
        assert_eq!(total.props.keys().copied().collect::<Vec<_>>(), [0, 2, 3]);
        c.uniq_pass(&lines, true);
        assert_eq!(c.props.keys().copied().collect::<Vec<_>>(), [0, 2]);
    }

    #[test]
    fn two_dim_lines_name_their_keys() {
        let mut l = line("{val}%#{sym} Damage of {sep}", Op::Add, true);
        l.syms = Some(["Increased".into(), "Decreased".into()]);
        l.sep = Some(Sep::AddDamageSkillName);
        let prop = Prop::TwoDim(BTreeMap::from([(2055, 10), (2056, 5)]));
        assert_eq!(
            text(&l, &prop, &NoNames),
            "10%#Increased Damage of Skill2055"
        );
        l.sep = Some(Sep::SkillGroup);
        let prop = Prop::TwoDim(BTreeMap::from([(1004, 500), (77, 1)]));
        assert_eq!(text(&l, &prop, &NoNames), "");
        let prop = Prop::TwoDim(BTreeMap::from([(1004, 500)]));
        assert_eq!(
            text(&l, &prop, &NoNames),
            "500%#Increased Damage of Skill19, Skill14 "
        );
    }
}
