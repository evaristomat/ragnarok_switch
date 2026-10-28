//! `EquipmentPropertiesOrder`: every line the window can show, in its categories' order, and the keys the
//! recorders reach them by (kRO's 0x67e1a0; `docs/nostalre-kro-exe-equip-window.md`, "Equipment Attributes" 5).
use mlua::{Table, Value};
use std::collections::HashMap;

/// A line's `val` op (`Operation` of the Order lub): how two records of it merge
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    One,
    Max,
    DrainAdd,
}

impl Op {
    fn from_lua(n: i64) -> Option<Self> {
        match n {
            0 => Some(Op::Add),
            1 => Some(Op::One),
            2 => Some(Op::Max),
            3 => Some(Op::DrainAdd),
            _ => None,
        }
    }
}

/// What names a 2-dim line's key (`sep`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sep {
    SkillName,
    AddDamageSkillName,
    Number,
    SkillGroup,
}

impl Sep {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "GetSkillName" => Some(Sep::SkillName),
            "GetAddDamageSkillName" => Some(Sep::AddDamageSkillName),
            "GetNumber" => Some(Sep::Number),
            "GetSkillGroup" => Some(Sep::SkillGroup),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Line {
    /// The category, 0-based (the Order table's 1..7)
    pub category: usize,
    pub name: String,
    /// `func[1]`: records by it count as they are, by `func[2]` negated
    pub plus: String,
    /// The argument holding the value, 0 for none
    pub val_arg: i64,
    pub op: Op,
    pub syms: Option<[String; 2]>,
    pub sep: Option<Sep>,
    pub uniq1: Option<String>,
    pub uniq2: Option<String>,
    pub min: Option<i32>,
    pub max: Option<i32>,
    pub div: Option<i32>,
    pub decimals: usize,
}

#[derive(Debug, Default)]
pub struct Order {
    /// The categories' names, in order
    pub categories: Vec<String>,
    /// Every line, in category then line order: a line's place here is its key in the containers
    pub lines: Vec<Line>,
    by_key: HashMap<String, usize>,
    by_option: HashMap<i64, usize>,
}

/// A recorder's key: its name, then each condition argument as " arg<i>:<v>" in argument order
pub fn record_key(name: &str, args: impl Iterator<Item = (i64, i64)>) -> String {
    let mut key = name.to_string();
    for (i, v) in args {
        key.push_str(&format!(" arg{i}:{v}"));
    }
    key
}

fn int_pairs(table: &Table) -> mlua::Result<Vec<(i64, i64)>> {
    let mut pairs = Vec::new();
    for pair in table.pairs::<i64, f64>() {
        let (k, v) = pair?;
        pairs.push((k, v as i64));
    }
    pairs.sort_unstable();
    Ok(pairs)
}

fn strings(table: &Table) -> mlua::Result<Vec<String>> {
    table.sequence_values::<String>().collect()
}

impl Order {
    pub fn line_by_key(&self, key: &str) -> Option<usize> {
        self.by_key.get(key).copied()
    }

    pub fn line_by_option(&self, option: i64) -> Option<usize> {
        self.by_option.get(&option).copied()
    }

    /// The Order lub's tables as kRO reads them; a malformed line fails the whole table, as kRO's check does
    pub fn read(order: &Table) -> mlua::Result<Self> {
        let mut parsed = Order::default();
        let mut categories: Vec<(i64, Table)> =
            order.pairs::<i64, Table>().collect::<mlua::Result<_>>()?;
        categories.sort_by_key(|(i, _)| *i);
        for (category, (_, table)) in categories.into_iter().enumerate() {
            parsed.categories.push(table.get("name")?);
            let lines: Table = table.get("order")?;
            let mut lines: Vec<(i64, Table)> = lines.pairs().collect::<mlua::Result<_>>()?;
            lines.sort_by_key(|(i, _)| *i);
            for (_, line) in lines {
                parsed.read_line(category, &line)?;
            }
        }
        Ok(parsed)
    }

    fn read_line(&mut self, category: usize, line: &Table) -> mlua::Result<()> {
        let bad = |what: &str| mlua::Error::runtime(format!("Order line: {what}"));
        let funcs = strings(&line.get::<Table>("func")?)?;
        if !(1..=2).contains(&funcs.len()) {
            return Err(bad("func"));
        }
        let val = int_pairs(&line.get::<Table>("val")?)?;
        let [(val_arg, op)] = val[..] else {
            return Err(bad("val"));
        };
        let op = Op::from_lua(op).ok_or_else(|| bad("op"))?;
        let cond = match line.get::<Value>("cond")? {
            Value::Table(t) => int_pairs(&t)?,
            _ => Vec::new(),
        };
        let syms = match line.get::<Value>("sym")? {
            Value::Table(t) => match &strings(&t)?[..] {
                [] => None,
                [a, b] => Some([a.clone(), b.clone()]),
                _ => return Err(bad("sym")),
            },
            _ => None,
        };
        let sep = match line.get::<Value>("sep")? {
            Value::Table(t) => {
                let mut seps = t.pairs::<i64, String>();
                let sep = seps.next().transpose()?;
                if seps.next().is_some() {
                    return Err(bad("sep"));
                }
                sep.map(|(_, name)| Sep::from_name(&name))
                    .map(|sep| sep.ok_or_else(|| bad("sep name")))
                    .transpose()?
            }
            _ => None,
        };
        let mut value_options = HashMap::new();
        if let Value::Table(t) = line.get::<Value>("valOpt")? {
            for pair in t.pairs::<i64, f64>() {
                let (k, v) = pair?;
                value_options.insert(k, v as i32);
            }
        }
        let index = self.lines.len();
        for func in &funcs {
            let key = record_key(func, cond.iter().copied());
            if self.by_key.insert(key, index).is_some() {
                return Err(bad("key defined twice"));
            }
        }
        if let Some(option) = line.get::<Option<f64>>("rndOpt")?
            && self.by_option.insert(option as i64, index).is_some()
        {
            return Err(bad("rndOpt value is already defined"));
        }
        self.lines.push(Line {
            category,
            name: line.get("name")?,
            plus: funcs[0].clone(),
            val_arg,
            op,
            syms,
            sep,
            uniq1: line.get("uniq1")?,
            uniq2: line.get::<Option<Value>>("uniq2")?.and_then(|v| match v {
                Value::String(s) => s.to_str().ok().map(|s| s.to_string()),
                Value::Integer(n) => Some(n.to_string()),
                Value::Number(n) => Some((n as i64).to_string()),
                _ => None,
            }),
            min: value_options.get(&0).copied(),
            max: value_options.get(&1).copied(),
            div: value_options.get(&2).copied(),
            decimals: value_options.get(&3).map_or(0, |&d| d.max(0) as usize),
        });
        Ok(())
    }
}
