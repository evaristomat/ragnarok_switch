//! Reader for compiled Lua chunks (`.lub`), Lua 5.0 and 5.1.
//!
//! The data tables the client ships as `.lub` are straight-line chunks that
//! build one global table out of constants, so only the opcodes they use are
//! executed. Anything else is an error and the caller falls back. A few tables
//! are lists of calls instead (`AddHeadStyle(...)`, the stylist's): a call to a
//! function the chunk did not define is recorded with its arguments, not run.

use std::collections::HashMap;
use std::rc::Rc;

pub const SIGNATURE: &[u8] = b"\x1bLua";

const LUA50: u8 = 0x50;
const LUA51: u8 = 0x51;

const OP_LOADK: u32 = 1;
const OP_GETGLOBAL: u32 = 5;
const OP_GETTABLE: u32 = 6;
const OP_SETGLOBAL: u32 = 7;
const OP_SETTABLE: u32 = 9;
const OP_NEWTABLE: u32 = 10;
/// 5.1 only: a boolean field, a table constructor's list part, and a function definition (`function main() ... end`)
const OP_LOADBOOL_51: u32 = 2;
const OP_SETLIST_51: u32 = 34;
const OP_CLOSURE_51: u32 = 36;
/// 5.1 only: a call (recorded, see `LuaState::calls`)
const OP_CALL_51: u32 = 28;
/// Lua 5.1's `LFIELDS_PER_FLUSH`: list items per SETLIST block
const FIELDS_PER_FLUSH: usize = 50;
const OP_RETURN_50: u32 = 27;
const OP_RETURN_51: u32 = 30;
/// Lua's own limit on nested functions (`LUAI_MAXCCALLS`); the client's tables nest one or two deep
const MAX_NESTING: usize = 200;

#[derive(Debug)]
pub enum LubError {
    NotACompiledChunk,
    UnsupportedVersion(u8),
    UnsupportedLayout,
    UnexpectedEof,
    BadConstant(u8),
    UnsupportedOpcode(u32),
    TypeError,
    /// The chunks ran without defining a table a reader needs
    MissingTable(&'static str),
}

impl std::fmt::Display for LubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LubError::NotACompiledChunk => write!(f, "not a compiled lua chunk"),
            LubError::UnsupportedVersion(v) => write!(f, "unsupported lua version {v:#x}"),
            LubError::UnsupportedLayout => write!(f, "unsupported chunk layout"),
            LubError::UnexpectedEof => write!(f, "unexpected end of chunk"),
            LubError::BadConstant(t) => write!(f, "unknown constant type {t}"),
            LubError::UnsupportedOpcode(op) => write!(f, "unsupported opcode {op}"),
            LubError::TypeError => write!(f, "value is not of the expected type"),
            LubError::MissingTable(name) => write!(f, "no table {name}"),
        }
    }
}

impl std::error::Error for LubError {}

pub fn is_compiled_chunk(data: &[u8]) -> bool {
    data.starts_with(SIGNATURE)
}

#[derive(Clone, Debug)]
pub enum Value {
    Nil,
    Bool(bool),
    Number(f64),
    Str(Rc<[u8]>),
    Table(usize),
}

impl Value {
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Number(u64),
    Str(Rc<[u8]>),
}

impl Key {
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Key::Number(bits) => Some(f64::from_bits(*bits)),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Key::Str(s) => Some(s),
            _ => None,
        }
    }

    fn from_value(value: &Value) -> Result<Self, LubError> {
        match value {
            Value::Number(n) => Ok(Key::Number(n.to_bits())),
            Value::Str(s) => Ok(Key::Str(s.clone())),
            _ => Err(LubError::TypeError),
        }
    }
}

pub type Table = HashMap<Key, Value>;

/// A call a chunk made to a global function it does not define: the function's name and its arguments
#[derive(Clone, Debug)]
pub struct Call {
    pub function: Rc<[u8]>,
    pub args: Vec<Value>,
}

#[derive(Default)]
pub struct LuaState {
    globals: HashMap<Rc<[u8]>, Value>,
    tables: Vec<Table>,
    calls: Vec<Call>,
}

impl LuaState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn global_table(&self, name: &str) -> Option<&Table> {
        match self.globals.get(name.as_bytes()) {
            Some(Value::Table(index)) => self.tables.get(*index),
            _ => None,
        }
    }

    pub fn tables(&self) -> impl Iterator<Item = &Table> {
        self.tables.iter()
    }

    /// The calls the chunks made, in order
    pub fn calls(&self) -> &[Call] {
        &self.calls
    }

    /// The table a value refers to, for tables nested in others
    pub fn table(&self, value: &Value) -> Option<&Table> {
        match value {
            Value::Table(index) => self.tables.get(*index),
            _ => None,
        }
    }
}

/// Runs one chunk. State is shared so a chunk can read globals an earlier one
/// left behind — `accname.lub` indexes `ACCESSORY_IDs` that way.
pub fn load_chunk(data: &[u8], state: &mut LuaState) -> Result<(), LubError> {
    let mut reader = Reader::new(data);
    let version = read_header(&mut reader)?;
    let function = read_function(&mut reader, version, 0)?;
    execute(&function, version, state)
}

/// A Lua 5.1 chunk rewritten for this build's `size_t`, the one header field a 64-bit Lua 5.1 refuses kRO's
/// 32-bit chunks over (`lundump.c` `checkHeader`): every string length is widened (or narrowed) to it, the rest
/// is copied as is
pub fn to_host_chunk(data: &[u8]) -> Result<Vec<u8>, LubError> {
    let mut reader = Reader::new(data);
    if read_header(&mut reader)? != LUA51 {
        return Err(LubError::UnsupportedVersion(LUA50));
    }
    let host_size_t = std::mem::size_of::<usize>();
    let mut out = Vec::with_capacity(data.len() + data.len() / 8);
    out.extend_from_slice(&data[..reader.pos]);
    out[8] = host_size_t as u8;
    copy_function(&mut reader, &mut out, host_size_t, 0)?;
    Ok(out)
}

fn copy_string(reader: &mut Reader, out: &mut Vec<u8>, host_size_t: usize) -> Result<(), LubError> {
    let bytes = reader.string()?;
    let len = bytes.as_ref().map_or(0, |b| b.len() + 1);
    out.extend_from_slice(&(len as u64).to_le_bytes()[..host_size_t]);
    if let Some(bytes) = bytes {
        out.extend_from_slice(&bytes);
        out.push(0);
    }
    Ok(())
}

fn copy_function(
    reader: &mut Reader,
    out: &mut Vec<u8>,
    host_size_t: usize,
    depth: usize,
) -> Result<(), LubError> {
    if depth > MAX_NESTING {
        return Err(LubError::UnsupportedLayout);
    }
    copy_string(reader, out, host_size_t)?;
    // Lines defined, upvalues, parameters, vararg, stack size
    out.extend_from_slice(reader.take(12)?);
    let code = reader.count_of(4)?;
    out.extend_from_slice(&(code as i32).to_le_bytes());
    out.extend_from_slice(reader.take(code * 4)?);
    let constants = reader.count_of(1)?;
    out.extend_from_slice(&(constants as i32).to_le_bytes());
    for _ in 0..constants {
        let kind = reader.u8()?;
        out.push(kind);
        match kind {
            0 => {}
            1 => out.push(reader.u8()?),
            3 => out.extend_from_slice(reader.take(8)?),
            4 => copy_string(reader, out, host_size_t)?,
            other => return Err(LubError::BadConstant(other)),
        }
    }
    let nested = reader.count_of(1)?;
    out.extend_from_slice(&(nested as i32).to_le_bytes());
    for _ in 0..nested {
        copy_function(reader, out, host_size_t, depth + 1)?;
    }
    let lines = reader.count_of(4)?;
    out.extend_from_slice(&(lines as i32).to_le_bytes());
    out.extend_from_slice(reader.take(lines * 4)?);
    let locals = reader.count_of(reader.min_string() + 8)?;
    out.extend_from_slice(&(locals as i32).to_le_bytes());
    for _ in 0..locals {
        copy_string(reader, out, host_size_t)?;
        out.extend_from_slice(reader.take(8)?);
    }
    let upvalues = reader.count_of(reader.min_string())?;
    out.extend_from_slice(&(upvalues as i32).to_le_bytes());
    for _ in 0..upvalues {
        copy_string(reader, out, host_size_t)?;
    }
    Ok(())
}

struct Function {
    constants: Vec<Value>,
    code: Vec<u32>,
    upvalues: u8,
    /// Upvalue counts of the functions defined inside: a CLOSURE is followed by that many pseudo-instructions
    nested_upvalues: Vec<u8>,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    /// A 64-bit `luac` writes 8-byte string lengths; kRO's are 4.
    wide_size_t: bool,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            wide_size_t: false,
        }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], LubError> {
        let end = self.pos.checked_add(count).ok_or(LubError::UnexpectedEof)?;
        let slice = self
            .data
            .get(self.pos..end)
            .ok_or(LubError::UnexpectedEof)?;
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, LubError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, LubError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn i32(&mut self) -> Result<i32, LubError> {
        Ok(self.u32()? as i32)
    }

    fn count(&mut self) -> Result<usize, LubError> {
        usize::try_from(self.i32()?).map_err(|_| LubError::UnsupportedLayout)
    }

    /// A count of items at least `min_size` bytes each: a corrupt one larger than the rest of the chunk fails
    /// here instead of reserving or looping for it.
    fn count_of(&mut self, min_size: usize) -> Result<usize, LubError> {
        let count = self.count()?;
        let remaining = self.data.len() - self.pos;
        if count
            .checked_mul(min_size)
            .is_none_or(|bytes| bytes > remaining)
        {
            return Err(LubError::UnexpectedEof);
        }
        Ok(count)
    }

    /// The smallest dumped string: its length alone
    fn min_string(&self) -> usize {
        if self.wide_size_t { 8 } else { 4 }
    }

    fn f64(&mut self) -> Result<f64, LubError> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    /// A dumped string is a size_t length counting the trailing NUL, then the
    /// bytes; length 0 means `nil`.
    fn string(&mut self) -> Result<Option<Rc<[u8]>>, LubError> {
        let len = if self.wide_size_t {
            let raw = u64::from_le_bytes(self.take(8)?.try_into().unwrap());
            usize::try_from(raw).map_err(|_| LubError::UnsupportedLayout)?
        } else {
            self.count()?
        };
        if len == 0 {
            return Ok(None);
        }
        let bytes = self.take(len)?;
        Ok(Some(Rc::from(&bytes[..len - 1])))
    }
}

fn read_header(reader: &mut Reader) -> Result<u8, LubError> {
    if reader.take(4)? != SIGNATURE {
        return Err(LubError::NotACompiledChunk);
    }
    let version = reader.u8()?;
    if version == LUA51 {
        reader.u8()?;
    } else if version != LUA50 {
        return Err(LubError::UnsupportedVersion(version));
    }
    let little_endian = reader.u8()?;
    let size_int = reader.u8()?;
    let size_size_t = reader.u8()?;
    let size_instruction = reader.u8()?;
    if little_endian != 1 || size_int != 4 || !matches!(size_size_t, 4 | 8) || size_instruction != 4
    {
        return Err(LubError::UnsupportedLayout);
    }
    reader.wide_size_t = size_size_t == 8;
    if version == LUA50 {
        let size_op = reader.u8()?;
        let size_a = reader.u8()?;
        let size_b = reader.u8()?;
        let size_c = reader.u8()?;
        if (size_op, size_a, size_b, size_c) != (6, 8, 9, 9) {
            return Err(LubError::UnsupportedLayout);
        }
        if reader.u8()? != 8 {
            return Err(LubError::UnsupportedLayout);
        }
        reader.f64()?;
    } else {
        if reader.u8()? != 8 {
            return Err(LubError::UnsupportedLayout);
        }
        if reader.u8()? != 0 {
            return Err(LubError::UnsupportedLayout);
        }
    }
    Ok(version)
}

fn read_function(reader: &mut Reader, version: u8, depth: usize) -> Result<Function, LubError> {
    if depth > MAX_NESTING {
        return Err(LubError::UnsupportedLayout);
    }
    reader.string()?;
    reader.i32()?;
    if version == LUA51 {
        reader.i32()?;
    }
    let upvalues = reader.u8()?;
    reader.u8()?;
    reader.u8()?;
    reader.u8()?;

    if version == LUA50 {
        read_debug(reader)?;
        let constants = read_constants(reader)?;
        let nested_upvalues = read_nested_functions(reader, version, depth)?;
        let code = read_code(reader)?;
        Ok(Function {
            constants,
            code,
            upvalues,
            nested_upvalues,
        })
    } else {
        let code = read_code(reader)?;
        let constants = read_constants(reader)?;
        let nested_upvalues = read_nested_functions(reader, version, depth)?;
        read_debug(reader)?;
        Ok(Function {
            constants,
            code,
            upvalues,
            nested_upvalues,
        })
    }
}

fn read_code(reader: &mut Reader) -> Result<Vec<u32>, LubError> {
    let count = reader.count_of(4)?;
    (0..count).map(|_| reader.u32()).collect()
}

fn read_constants(reader: &mut Reader) -> Result<Vec<Value>, LubError> {
    let count = reader.count_of(1)?;
    let mut constants = Vec::with_capacity(count);
    for _ in 0..count {
        let value = match reader.u8()? {
            0 => Value::Nil,
            1 => Value::Bool(reader.u8()? != 0),
            3 => Value::Number(reader.f64()?),
            4 => reader.string()?.map(Value::Str).unwrap_or(Value::Nil),
            other => return Err(LubError::BadConstant(other)),
        };
        constants.push(value);
    }
    Ok(constants)
}

fn read_nested_functions(
    reader: &mut Reader,
    version: u8,
    depth: usize,
) -> Result<Vec<u8>, LubError> {
    let count = reader.count_of(1)?;
    (0..count)
        .map(|_| read_function(reader, version, depth + 1).map(|f| f.upvalues))
        .collect()
}

fn read_debug(reader: &mut Reader) -> Result<(), LubError> {
    let lines = reader.count_of(4)?;
    reader.take(lines * 4)?;
    let locals = reader.count_of(reader.min_string() + 8)?;
    for _ in 0..locals {
        reader.string()?;
        reader.i32()?;
        reader.i32()?;
    }
    let upvalues = reader.count_of(reader.min_string())?;
    for _ in 0..upvalues {
        reader.string()?;
    }
    Ok(())
}

/// 5.1 moved the operand fields around and changed how a constant is spelled in
/// an RK operand, so both are read through this.
struct Encoding {
    version: u8,
}

impl Encoding {
    fn a(&self, instruction: u32) -> usize {
        let shift = if self.version == LUA50 { 24 } else { 6 };
        ((instruction >> shift) & 0xff) as usize
    }

    fn b(&self, instruction: u32) -> u32 {
        let shift = if self.version == LUA50 { 15 } else { 23 };
        (instruction >> shift) & 0x1ff
    }

    fn c(&self, instruction: u32) -> u32 {
        let shift = if self.version == LUA50 { 6 } else { 14 };
        (instruction >> shift) & 0x1ff
    }

    fn bx(&self, instruction: u32) -> usize {
        let shift = if self.version == LUA50 { 6 } else { 14 };
        ((instruction >> shift) & 0x3ffff) as usize
    }

    /// `Some(constant index)` when the operand names a constant.
    fn constant_index(&self, operand: u32) -> Option<usize> {
        if self.version == LUA50 {
            (operand >= 250).then(|| (operand - 250) as usize)
        } else {
            (operand & 0x100 != 0).then_some((operand & 0xff) as usize)
        }
    }

    fn return_opcode(&self) -> u32 {
        if self.version == LUA50 {
            OP_RETURN_50
        } else {
            OP_RETURN_51
        }
    }
}

fn execute(function: &Function, version: u8, state: &mut LuaState) -> Result<(), LubError> {
    let encoding = Encoding { version };
    let mut registers = vec![Value::Nil; 256];
    // the global a register was loaded from, for recording a call through it
    let mut loaded_from: Vec<Option<Rc<[u8]>>> = vec![None; 256];

    let constant = |index: usize| -> Result<Value, LubError> {
        function
            .constants
            .get(index)
            .cloned()
            .ok_or(LubError::TypeError)
    };
    let register = |registers: &Vec<Value>, index: u32| -> Result<Value, LubError> {
        registers
            .get(index as usize)
            .cloned()
            .ok_or(LubError::TypeError)
    };

    let mut pc = 0;
    while let Some(&instruction) = function.code.get(pc) {
        pc += 1;
        let opcode = instruction & 0x3f;
        if opcode == encoding.return_opcode() {
            break;
        }
        let a = encoding.a(instruction);
        if a >= registers.len() {
            return Err(LubError::TypeError);
        }
        let rk = |registers: &Vec<Value>, operand: u32| -> Result<Value, LubError> {
            match encoding.constant_index(operand) {
                Some(index) => constant(index),
                None => register(registers, operand),
            }
        };
        match opcode {
            OP_LOADK => registers[a] = constant(encoding.bx(instruction))?,
            OP_NEWTABLE => {
                state.tables.push(Table::new());
                registers[a] = Value::Table(state.tables.len() - 1);
            }
            OP_SETTABLE => {
                let Value::Table(index) = &registers[a] else {
                    return Err(LubError::TypeError);
                };
                let index = *index;
                let key = Key::from_value(&rk(&registers, encoding.b(instruction))?)?;
                let value = rk(&registers, encoding.c(instruction))?;
                state
                    .tables
                    .get_mut(index)
                    .ok_or(LubError::TypeError)?
                    .insert(key, value);
            }
            OP_GETTABLE => {
                let base = register(&registers, encoding.b(instruction))?;
                // a field of a global no chunk defined (`StylingShop.AddHeadStyle`): only ever called, so its
                // dotted name is what a call records
                if let (Value::Nil, Some(name), Value::Str(field)) = (
                    &base,
                    &loaded_from[encoding.b(instruction) as usize],
                    rk(&registers, encoding.c(instruction))?,
                ) {
                    let dotted = [name.as_ref(), b".", field.as_ref()].concat();
                    registers[a] = Value::Nil;
                    loaded_from[a] = Some(Rc::from(dotted));
                    continue;
                }
                let Value::Table(index) = base else {
                    return Err(LubError::TypeError);
                };
                let key = Key::from_value(&rk(&registers, encoding.c(instruction))?)?;
                registers[a] = state
                    .tables
                    .get(index)
                    .ok_or(LubError::TypeError)?
                    .get(&key)
                    .cloned()
                    .unwrap_or(Value::Nil);
            }
            OP_GETGLOBAL => {
                let Value::Str(name) = constant(encoding.bx(instruction))? else {
                    return Err(LubError::TypeError);
                };
                registers[a] = state.globals.get(&name).cloned().unwrap_or(Value::Nil);
                loaded_from[a] = Some(name);
                continue;
            }
            OP_CALL_51 if version == LUA51 => {
                // B 0 (arguments up to the stack top) only follows another call, which these files do not make
                let b = encoding.b(instruction) as usize;
                if b == 0 || a + b > registers.len() {
                    return Err(LubError::UnsupportedLayout);
                }
                let function = loaded_from[a].clone().ok_or(LubError::UnsupportedLayout)?;
                state.calls.push(Call {
                    function,
                    args: registers[a + 1..a + b].to_vec(),
                });
            }
            OP_SETGLOBAL => {
                let Value::Str(name) = constant(encoding.bx(instruction))? else {
                    return Err(LubError::TypeError);
                };
                state.globals.insert(name, registers[a].clone());
            }
            OP_LOADBOOL_51 if version == LUA51 => {
                registers[a] = Value::Bool(encoding.b(instruction) != 0);
                if encoding.c(instruction) != 0 {
                    pc += 1;
                }
            }
            OP_SETLIST_51 if version == LUA51 => {
                let Value::Table(index) = &registers[a] else {
                    return Err(LubError::TypeError);
                };
                let index = *index;
                // B 0 (up to the stack top) only follows a call, which these files do not make
                let count = encoding.b(instruction) as usize;
                if count == 0 || a + count >= registers.len() {
                    return Err(LubError::UnsupportedLayout);
                }
                let block = match encoding.c(instruction) as usize {
                    0 => {
                        let next = *function.code.get(pc).ok_or(LubError::UnexpectedEof)?;
                        pc += 1;
                        next as usize
                    }
                    c => c,
                };
                let table = state.tables.get_mut(index).ok_or(LubError::TypeError)?;
                let first = block.checked_sub(1).ok_or(LubError::TypeError)? * FIELDS_PER_FLUSH;
                for i in 1..=count {
                    let key = Key::Number(((first + i) as f64).to_bits());
                    table.insert(key, registers[a + i].clone());
                }
            }
            OP_CLOSURE_51 if version == LUA51 => {
                // Functions are not run: the value stands in, and its upvalue pseudo-instructions are skipped
                let upvalues = *function
                    .nested_upvalues
                    .get(encoding.bx(instruction))
                    .ok_or(LubError::TypeError)?;
                registers[a] = Value::Nil;
                pc += upvalues as usize;
            }
            other => return Err(LubError::UnsupportedOpcode(other)),
        }
        loaded_from[a] = None;
    }
    Ok(())
}

/// Chunks for tests: kRO's call-list tables, `StylingShop.AddX(goods, {zeny = n}, flags...)` per call
#[cfg(test)]
pub(crate) mod test_chunk {
    use super::*;

    const OP_LOADBOOL: u32 = OP_LOADBOOL_51;

    fn abc(op: u32, a: u32, b: u32, c: u32) -> u32 {
        op | a << 6 | c << 14 | b << 23
    }

    fn abx(op: u32, a: u32, bx: u32) -> u32 {
        op | a << 6 | bx << 14
    }

    /// One call: the function's field name, its first argument, the cost table's one field, its flags
    pub(crate) struct CallSpec<'a> {
        pub function: &'a str,
        pub goods: f64,
        pub cost: (&'a str, f64),
        pub flags: &'a [bool],
    }

    pub(crate) fn call_chunk(table: &str, calls: &[CallSpec]) -> Vec<u8> {
        enum K<'a> {
            S(&'a str),
            N(f64),
        }
        let mut constants: Vec<K> = vec![K::S(table)];
        let mut code = Vec::new();
        for call in calls {
            let at = constants.len() as u32;
            constants.extend([
                K::S(call.function),
                K::N(call.goods),
                K::S(call.cost.0),
                K::N(call.cost.1),
            ]);
            code.extend([
                abx(OP_GETGLOBAL, 0, 0),
                abc(OP_GETTABLE, 0, 0, 0x100 | at),
                abx(OP_LOADK, 1, at + 1),
                abc(OP_NEWTABLE, 2, 0, 1),
                abc(OP_SETTABLE, 2, 0x100 | (at + 2), 0x100 | (at + 3)),
            ]);
            for (i, flag) in call.flags.iter().enumerate() {
                code.push(abc(OP_LOADBOOL, 3 + i as u32, *flag as u32, 0));
            }
            code.push(abc(OP_CALL_51, 0, 3 + call.flags.len() as u32, 1));
        }
        code.push(OP_RETURN_51 | 1 << 23);

        let mut chunk = SIGNATURE.to_vec();
        chunk.extend([LUA51, 0, 1, 4, 4, 4, 8, 0]);
        chunk.extend(0u32.to_le_bytes());
        chunk.extend([0; 8]);
        chunk.extend([0, 0, 0, 5]);
        chunk.extend((code.len() as i32).to_le_bytes());
        for instruction in code {
            chunk.extend(instruction.to_le_bytes());
        }
        chunk.extend((constants.len() as i32).to_le_bytes());
        for constant in constants {
            match constant {
                K::S(text) => {
                    chunk.push(4);
                    chunk.extend((text.len() as u32 + 1).to_le_bytes());
                    chunk.extend(text.as_bytes());
                    chunk.push(0);
                }
                K::N(n) => {
                    chunk.push(3);
                    chunk.extend(n.to_le_bytes());
                }
            }
        }
        for _ in 0..4 {
            chunk.extend(0i32.to_le_bytes()); // nested functions, lines, locals, upvalues
        }
        chunk
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_through_an_undefined_global_is_recorded_with_its_arguments() {
        use test_chunk::{CallSpec, call_chunk};
        let chunk = call_chunk(
            "StylingShop",
            &[CallSpec {
                function: "AddHeadStyle",
                goods: 7.0,
                cost: ("zeny", 100_000.0),
                flags: &[true, false],
            }],
        );
        let mut state = LuaState::new();
        load_chunk(&chunk, &mut state).unwrap();
        let [call] = state.calls() else {
            panic!("one call expected, got {}", state.calls().len());
        };
        assert_eq!(&*call.function, b"StylingShop.AddHeadStyle");
        assert_eq!(call.args.len(), 4);
        assert_eq!(call.args[0].as_number(), Some(7.0));
        let cost = state.table(&call.args[1]).unwrap();
        assert_eq!(
            cost.get(&Key::Str(Rc::from(&b"zeny"[..])))
                .and_then(Value::as_number),
            Some(100_000.0)
        );
        assert!(matches!(
            call.args[2..],
            [Value::Bool(true), Value::Bool(false)]
        ));
    }

    #[test]
    fn text_lua_is_not_a_compiled_chunk() {
        assert!(!is_compiled_chunk(b"ACCESSORY_GOGGLES = 1,\n"));
        let mut state = LuaState::new();
        assert!(matches!(
            load_chunk(b"ACCESSORY_GOGGLES = 1,\n", &mut state),
            Err(LubError::NotACompiledChunk)
        ));
    }

    /// A Lua 5.1 header as `luac` writes it on a 32-bit little-endian build.
    fn header_51() -> Vec<u8> {
        let mut chunk = SIGNATURE.to_vec();
        chunk.extend([LUA51, 0, 1, 4, 4, 4, 8, 0]);
        chunk
    }

    /// A function's fields up to its code: no source name, lines 0, no upvalues or parameters.
    fn function_start(chunk: &mut Vec<u8>) {
        chunk.extend(0u32.to_le_bytes());
        chunk.extend([0; 8]);
        chunk.extend([0, 0, 0, 2]);
    }

    fn count(chunk: &mut Vec<u8>, n: i32) {
        chunk.extend(n.to_le_bytes());
    }

    /// A chunk that sets `T = {}`: NEWTABLE, SETGLOBAL, RETURN.
    fn table_chunk() -> Vec<u8> {
        let mut chunk = header_51();
        function_start(&mut chunk);
        count(&mut chunk, 3);
        for instruction in [OP_NEWTABLE, OP_SETGLOBAL, OP_RETURN_51 | 1 << 23] {
            chunk.extend(instruction.to_le_bytes());
        }
        count(&mut chunk, 1);
        chunk.push(4);
        chunk.extend(2u32.to_le_bytes());
        chunk.extend(b"T\0");
        for _ in 0..4 {
            count(&mut chunk, 0); // nested functions, lines, locals, upvalues
        }
        chunk
    }

    #[test]
    fn a_well_formed_chunk_runs() {
        let mut state = LuaState::new();
        load_chunk(&table_chunk(), &mut state).unwrap();
        assert!(state.global_table("T").is_some());
    }

    #[test]
    fn a_32_bit_chunk_is_rewritten_for_the_hosts_size_t() {
        let chunk = to_host_chunk(&table_chunk()).unwrap();
        let wide = std::mem::size_of::<usize>();
        assert_eq!(chunk[8] as usize, wide);
        // Two strings (the source name and "T"), each length now `wide` bytes
        assert_eq!(chunk.len(), table_chunk().len() + 2 * (wide - 4));
        let mut state = LuaState::new();
        load_chunk(&chunk, &mut state).unwrap();
        assert!(state.global_table("T").is_some());
    }

    /// Checked on the reader: the chunk test below passes even with a 51 GB reservation where the kernel overcommits.
    #[test]
    fn a_count_past_the_end_fails_before_anything_is_reserved() {
        let data = [i32::MAX.to_le_bytes(), [0; 4]].concat();
        assert!(matches!(
            Reader::new(&data).count_of(1),
            Err(LubError::UnexpectedEof)
        ));
        let data = [1i32.to_le_bytes(), [0; 4]].concat();
        assert_eq!(Reader::new(&data).count_of(4).unwrap(), 1);
        assert!(Reader::new(&data).count_of(5).is_err());
    }

    /// The constants' count sits right after the three instructions.
    #[test]
    fn a_corrupt_count_is_an_error() {
        let constants_at = header_51().len() + 16 + 4 + 12;
        for (at, what) in [
            (constants_at, "constants"),
            (header_51().len() + 16, "code"),
        ] {
            let mut chunk = table_chunk();
            chunk[at..at + 4].copy_from_slice(&i32::MAX.to_le_bytes());
            let mut state = LuaState::new();
            assert!(
                matches!(load_chunk(&chunk, &mut state), Err(LubError::UnexpectedEof)),
                "{what}"
            );
        }
    }

    #[test]
    fn deep_nesting_is_an_error_not_a_stack_overflow() {
        const DEPTH: usize = 100_000;
        let mut chunk = header_51();
        for _ in 0..DEPTH {
            function_start(&mut chunk);
            count(&mut chunk, 0); // code
            count(&mut chunk, 0); // constants
            count(&mut chunk, 1); // one nested function
        }
        for _ in 0..=DEPTH {
            for _ in 0..3 {
                count(&mut chunk, 0); // lines, locals, upvalues
            }
        }
        let mut state = LuaState::new();
        assert!(matches!(
            load_chunk(&chunk, &mut state),
            Err(LubError::UnsupportedLayout)
        ));
    }
}
