//! The data in a Lua source file (as the pinned client's `SystemEN` tables are: `mapInfo.lub`, `spopup.lub`): its
//! top-level assignments of strings, numbers, booleans and tables, a name of a table field (`SPUI_IDS.SPU_CHAT`)
//! taking that field's value. Functions and `if` blocks are skipped, not run: these files define a `main` that
//! hands the tables to the client, which reads the tables themselves instead.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum LuaValue {
    Nil,
    Bool(bool),
    Number(f64),
    Str(String),
    Table(LuaTable),
}

/// A table's entries in the order written; positional ones keyed from 1
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LuaTable {
    pub entries: Vec<(LuaKey, LuaValue)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LuaKey {
    Index(i64),
    Name(String),
}

impl LuaValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            LuaValue::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            LuaValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            LuaValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_table(&self) -> Option<&LuaTable> {
        match self {
            LuaValue::Table(t) => Some(t),
            _ => None,
        }
    }
}

impl LuaTable {
    /// A field by name (`["name"]` or `name =`)
    pub fn get(&self, name: &str) -> Option<&LuaValue> {
        self.entries.iter().find_map(|(key, value)| match key {
            LuaKey::Name(n) if n == name => Some(value),
            _ => None,
        })
    }

    /// The positional values in order (`[1]`, `[2]`, ... as written)
    pub fn list(&self) -> impl Iterator<Item = &LuaValue> {
        self.entries.iter().filter_map(|(key, value)| match key {
            LuaKey::Index(_) => Some(value),
            LuaKey::Name(_) => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Name(String),
    Str(String),
    Number(f64),
    Symbol(char),
}

/// The file's top-level globals; text in EUC-KR read as such (an ASCII file reads the same)
pub fn globals(source: &[u8]) -> HashMap<String, LuaValue> {
    globals_with(source, HashMap::new())
}

/// As `globals`, with `seed` defined first: tables another file defines that this one's field references read
/// (`damageskininfo.lub`'s `[DSList.DT_Default]`, `DSList` being `damageskinlist.lub`'s)
pub fn globals_with(source: &[u8], seed: HashMap<String, LuaValue>) -> HashMap<String, LuaValue> {
    let text = crate::lua_table::decode_euc_kr(source);
    let tokens = tokenize(&text);
    let mut parser = Parser {
        tokens,
        at: 0,
        globals: seed,
    };
    parser.chunk();
    parser.globals
}

fn tokenize(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '-' && chars.get(i + 1) == Some(&'-') {
            i += 2;
            if let Some(level) = long_bracket(&chars, i) {
                i = skip_long(&chars, i, level);
            } else {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
        } else if c == '"' || c == '\'' {
            let (s, next) = quoted(&chars, i);
            tokens.push(Token::Str(s));
            i = next;
        } else if c == '[' && long_bracket(&chars, i).is_some() {
            let level = long_bracket(&chars, i).unwrap();
            let start = i + level + 2;
            let end = skip_long(&chars, i, level);
            let body: String = chars[start..end.saturating_sub(level + 2).max(start)]
                .iter()
                .collect();
            tokens.push(Token::Str(
                body.strip_prefix('\n').unwrap_or(&body).to_string(),
            ));
            i = end;
        } else if c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit))
        {
            let start = i;
            if c == '0' && matches!(chars.get(i + 1), Some('x' | 'X')) {
                i += 2;
                while i < chars.len() && chars[i].is_ascii_hexdigit() {
                    i += 1;
                }
                let hex: String = chars[start + 2..i].iter().collect();
                tokens.push(Token::Number(
                    i64::from_str_radix(&hex, 16).unwrap_or(0) as f64
                ));
            } else {
                while i < chars.len()
                    && (chars[i].is_ascii_alphanumeric()
                        || chars[i] == '.'
                        || ((chars[i] == '-' || chars[i] == '+')
                            && matches!(chars[i - 1], 'e' | 'E')))
                {
                    i += 1;
                }
                let number: String = chars[start..i].iter().collect();
                tokens.push(Token::Number(number.parse().unwrap_or(0.0)));
            }
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(Token::Name(chars[start..i].iter().collect()));
        } else {
            tokens.push(Token::Symbol(c));
            i += 1;
        }
    }
    tokens
}

/// The level of a long bracket (`[[`, `[=[`...) opening at `i`
fn long_bracket(chars: &[char], i: usize) -> Option<usize> {
    if chars.get(i) != Some(&'[') {
        return None;
    }
    let level = chars[i + 1..].iter().take_while(|&&c| c == '=').count();
    (chars.get(i + 1 + level) == Some(&'[')).then_some(level)
}

/// Past the long bracket of `level` opening at `i`
fn skip_long(chars: &[char], i: usize, level: usize) -> usize {
    let close: Vec<char> = std::iter::once(']')
        .chain(std::iter::repeat_n('=', level))
        .chain(std::iter::once(']'))
        .collect();
    let mut j = i + level + 2;
    while j + close.len() <= chars.len() {
        if chars[j..j + close.len()] == close[..] {
            return j + close.len();
        }
        j += 1;
    }
    chars.len()
}

fn quoted(chars: &[char], i: usize) -> (String, usize) {
    let quote = chars[i];
    let mut s = String::new();
    let mut j = i + 1;
    while j < chars.len() && chars[j] != quote {
        if chars[j] == '\\' && j + 1 < chars.len() {
            j += 1;
            s.push(match chars[j] {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                other => other,
            });
        } else {
            s.push(chars[j]);
        }
        j += 1;
    }
    (s, j + 1)
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
    globals: HashMap<String, LuaValue>,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.at).cloned();
        self.at += 1;
        token
    }

    fn is_symbol(&self, c: char) -> bool {
        self.peek() == Some(&Token::Symbol(c))
    }

    fn chunk(&mut self) {
        while let Some(token) = self.next() {
            match token {
                Token::Name(name)
                    if matches!(name.as_str(), "function" | "if" | "do" | "for" | "while") =>
                {
                    self.at -= 1;
                    self.skip_block();
                }
                Token::Name(name) if name == "local" => {}
                Token::Name(name) if self.is_symbol('=') && !self.is_double_equals() => {
                    self.at += 1;
                    let value = self.value();
                    self.globals.insert(name, value);
                }
                _ => {}
            }
        }
    }

    fn is_double_equals(&self) -> bool {
        self.tokens.get(self.at + 1) == Some(&Token::Symbol('='))
    }

    /// Past a block opened at the current token and closed by its `end`
    fn skip_block(&mut self) {
        let mut depth = 0;
        while let Some(token) = self.next() {
            if let Token::Name(name) = token {
                match name.as_str() {
                    "function" | "if" | "do" => depth += 1,
                    // `for ... do` and `while ... do` open with their `do`
                    "end" => {
                        depth -= 1;
                        if depth <= 0 {
                            return;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn value(&mut self) -> LuaValue {
        match self.next() {
            Some(Token::Str(s)) => LuaValue::Str(s),
            Some(Token::Number(n)) => LuaValue::Number(n),
            Some(Token::Symbol('-')) => match self.next() {
                Some(Token::Number(n)) => LuaValue::Number(-n),
                _ => LuaValue::Nil,
            },
            Some(Token::Symbol('{')) => LuaValue::Table(self.table()),
            Some(Token::Name(name)) => match name.as_str() {
                "true" => LuaValue::Bool(true),
                "false" => LuaValue::Bool(false),
                "nil" => LuaValue::Nil,
                "function" => {
                    self.at -= 1;
                    self.skip_block();
                    LuaValue::Nil
                }
                _ => self.reference(name),
            },
            _ => LuaValue::Nil,
        }
    }

    /// `name.field...`: the value a global table holds there
    fn reference(&mut self, name: String) -> LuaValue {
        let mut value = self.globals.get(&name).cloned().unwrap_or(LuaValue::Nil);
        while self.is_symbol('.') {
            self.at += 1;
            let Some(Token::Name(field)) = self.next() else {
                return LuaValue::Nil;
            };
            value = value
                .as_table()
                .and_then(|t| t.get(&field))
                .cloned()
                .unwrap_or(LuaValue::Nil);
        }
        value
    }

    fn table(&mut self) -> LuaTable {
        let mut table = LuaTable::default();
        let mut index = 1;
        loop {
            if self.is_symbol('}') || self.peek().is_none() {
                self.at += 1;
                return table;
            }
            if self.is_symbol(',') || self.is_symbol(';') {
                self.at += 1;
                continue;
            }
            let key = if self.is_symbol('[') {
                self.at += 1;
                let key = self.value();
                self.at += 1; // ]
                self.at += 1; // =
                match key {
                    LuaValue::Number(n) => Some(LuaKey::Index(n as i64)),
                    LuaValue::Str(s) => Some(LuaKey::Name(s)),
                    _ => None,
                }
            } else if let Some(Token::Name(name)) = self.peek().cloned()
                && self.tokens.get(self.at + 1) == Some(&Token::Symbol('='))
                && self.tokens.get(self.at + 2) != Some(&Token::Symbol('='))
            {
                self.at += 2;
                Some(LuaKey::Name(name))
            } else {
                let key = LuaKey::Index(index);
                index += 1;
                Some(key)
            };
            let value = self.value();
            if let Some(key) = key {
                table.entries.push((key, value));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_strings_numbers_and_field_references_are_read_and_functions_skipped() {
        let source = br#"-- a comment
SPUI_IDS = { SPU_CHAT = 0, SPU_FLOATING = 1 }
popuptbl = {
	[1] = {
		uitype = SPUI_IDS.SPU_FLOATING,
		textcolor = 16763904,
		Description = { "First line.", "Second \"quoted\" line." }
	},
}
function main()
	for k, v in pairs(popuptbl) do
		if v then result = AddMsg(k) end
	end
	return true, "good"
end
mapTbl = { ["hero_in1.rsw"] = { notifyEnter = true, signName = { mainTitle = "Spring Garden" }, size = -2.5, hex = 0x1F } }
"#;
        let globals = globals(source);
        let popup = globals["popuptbl"].as_table().unwrap().entries[0].clone();
        assert_eq!(popup.0, LuaKey::Index(1));
        let popup = popup.1;
        let popup = popup.as_table().unwrap();
        assert_eq!(popup.get("uitype"), Some(&LuaValue::Number(1.0)));
        assert_eq!(popup.get("textcolor"), Some(&LuaValue::Number(16763904.0)));
        let lines: Vec<_> = popup
            .get("Description")
            .and_then(LuaValue::as_table)
            .unwrap()
            .list()
            .filter_map(LuaValue::as_str)
            .collect();
        assert_eq!(lines, ["First line.", "Second \"quoted\" line."]);
        assert!(!globals.contains_key("result"));

        let map = globals["mapTbl"]
            .as_table()
            .unwrap()
            .get("hero_in1.rsw")
            .unwrap();
        let map = map.as_table().unwrap();
        assert_eq!(map.get("notifyEnter"), Some(&LuaValue::Bool(true)));
        assert_eq!(map.get("size"), Some(&LuaValue::Number(-2.5)));
        assert_eq!(map.get("hex"), Some(&LuaValue::Number(31.0)));
        let title = map
            .get("signName")
            .and_then(LuaValue::as_table)
            .unwrap()
            .get("mainTitle");
        assert_eq!(title.and_then(LuaValue::as_str), Some("Spring Garden"));
    }

    #[test]
    fn a_seeded_table_answers_field_references() {
        let seed = HashMap::from([(
            "DSList".to_string(),
            LuaValue::Table(LuaTable {
                entries: vec![(LuaKey::Name("DT_Han".into()), LuaValue::Number(2.0))],
            }),
        )]);
        let globals = globals_with(
            b"DSLName = { [DSList.DT_Han] = { DSMillionUnit = 2 } }",
            seed,
        );
        let names = globals["DSLName"].as_table().unwrap();
        assert_eq!(names.entries[0].0, LuaKey::Index(2));
    }

    #[test]
    fn long_strings_and_comments() {
        let globals = globals(b"--[[ a\nblock ]]\nx = [[\nline]]\n");
        assert_eq!(globals["x"], LuaValue::Str("line".into()));
    }
}
