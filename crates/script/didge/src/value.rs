// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Lua behind genet's value-level script contract.
//!
//! [`LuaValueEngine`] implements [`ValueEngine`]: neutral [`ScriptValue`]s in
//! and out, host functions over them, and evaluation under a [`Budget`]. It is
//! the Lua counterpart of `script-rhai`'s `RhaiValueEngine`.
//!
//! ## Sandbox
//!
//! Each [`eval`](ValueEngine::eval) builds a fresh Lua state from the stored
//! globals and host functions, so a rule cannot leak a binding into the next
//! and the engine stays `Send`. The state loads `base`, `math`, `string` and
//! `table` only: no `io` and no `coroutine`. Rules use Lua's own maths,
//! `math.sin(x)` and `x ^ y`: didgeridoo computes both with libm, so a rule
//! gives the same bits natively and on wasm32. `math` has no `random`,
//! because didgeridoo is built with that feature off.
//!
//! ## Values
//!
//! A missing fact is `nil` and reads as [`ScriptValue::Unit`]. A table crosses
//! as a list when its keys are exactly `1..=n` (and when empty), as a map when
//! every key is a string, and is refused otherwise. A `Unit` in a list is
//! refused, since `nil` would end the list; in a map it leaves the key out.
//!
//! The string limit is the host's: [`LuaValueEngine::with_max_string_bytes`].

use std::collections::BTreeMap;

use piccolo::{
    Callback, CallbackReturn, Closure, Context, Error, Executor, Fuel, Lua, Stack, StashedExecutor,
    Table, Value,
};
use script_engine_api::{Budget, HostFunction, ScriptValue, ValueEngine, ValueError};

use crate::DEFAULT_MAX_STRING_BYTES;

/// Nesting deeper than this does not cross the boundary, either way.
const MAX_DEPTH: usize = 64;

/// Values read out of a script in one conversion, shared tables counted each
/// time they are met, so a table that holds itself twice per level cannot
/// make the copy outrun the fuel.
const MAX_NODES: usize = 1 << 20;

/// Fuel handed to one `Executor::step` call.
const FUEL_PER_STEP: i32 = 4096;

/// The name compiled chunks carry in error messages.
const CHUNK_NAME: &str = "didge rule";

/// Put before the source to read it as a value, on the same line so error
/// line numbers stay the author's.
const RETURN_PREFIX: &str = "return ";

const UNIT_IN_LIST: &str = "a unit in a list has no Lua form: nil would end the list";

/// Names that parse as identifiers but cannot be written as a variable.
const RESERVED: [&str; 22] = [
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

/// A Lua-backed [`ValueEngine`] whose maths is the same bits on every target.
pub struct LuaValueEngine {
    globals: BTreeMap<String, ScriptValue>,
    functions: BTreeMap<String, (usize, HostFunction)>,
    max_string_bytes: usize,
}

impl LuaValueEngine {
    pub fn new() -> Self {
        Self {
            globals: BTreeMap::new(),
            functions: BTreeMap::new(),
            max_string_bytes: DEFAULT_MAX_STRING_BYTES,
        }
    }

    /// The longest string, in bytes, a rule may build.
    pub fn max_string_bytes(&self) -> usize {
        self.max_string_bytes
    }

    /// Replace the string limit with the host's own.
    pub fn with_max_string_bytes(mut self, bytes: usize) -> Self {
        self.max_string_bytes = bytes;
        self
    }

    /// A state with the sandbox's libraries, the globals and the host functions.
    fn state(&self) -> Result<Lua, ValueError> {
        let mut lua = Lua::empty();
        lua.enter(|ctx| {
            ctx.set_max_string_bytes(self.max_string_bytes);
            piccolo::stdlib::load_base(ctx);
            piccolo::stdlib::load_math(ctx);
            piccolo::stdlib::load_string(ctx);
            piccolo::stdlib::load_table(ctx);
            let table = ctx.globals();
            for (name, value) in &self.globals {
                put(ctx, table, name, to_value(ctx, value, 0)?)?;
            }
            for (name, (arity, function)) in &self.functions {
                let (name_in, arity, function) = (name.clone(), *arity, function.clone());
                let callback = Callback::from_fn(&ctx, move |ctx, _, stack| {
                    call_host(ctx, stack, &name_in, arity, &function)
                });
                put(ctx, table, name, Value::Function(callback.into()))?;
            }
            Ok::<(), ValueError>(())
        })?;
        Ok(lua)
    }
}

impl Default for LuaValueEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn identifier(name: &str) -> Result<(), ValueError> {
    let mut chars = name.chars();
    let head = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if head && chars.all(|c| c.is_ascii_alphanumeric() || c == '_') && !RESERVED.contains(&name) {
        Ok(())
    } else {
        Err(ValueError::Script(format!("`{name}` is not an identifier")))
    }
}

/// Refuse at set time what would be refused when the value is built.
fn check(value: &ScriptValue, depth: usize) -> Result<(), ValueError> {
    if depth > MAX_DEPTH {
        return Err(ValueError::Value("nested too deeply".into()));
    }
    match value {
        ScriptValue::List(items) => items.iter().try_for_each(|item| {
            if matches!(item, ScriptValue::Unit) {
                Err(ValueError::Value(UNIT_IN_LIST.into()))
            } else {
                check(item, depth + 1)
            }
        }),
        ScriptValue::Map(entries) => entries.values().try_for_each(|item| check(item, depth + 1)),
        _ => Ok(()),
    }
}

fn put<'gc>(
    ctx: Context<'gc>,
    table: Table<'gc>,
    key: &str,
    value: Value<'gc>,
) -> Result<(), ValueError> {
    let key = Value::String(ctx.intern(key.as_bytes()));
    table
        .set_raw(&ctx, key, value)
        .map(|_| ())
        .map_err(|e| ValueError::Value(e.to_string()))
}

fn to_value<'gc>(
    ctx: Context<'gc>,
    value: &ScriptValue,
    depth: usize,
) -> Result<Value<'gc>, ValueError> {
    if depth > MAX_DEPTH {
        return Err(ValueError::Value("nested too deeply".into()));
    }
    Ok(match value {
        ScriptValue::Unit => Value::Nil,
        ScriptValue::Bool(value) => Value::Boolean(*value),
        ScriptValue::Int(value) => Value::Integer(*value),
        ScriptValue::Float(value) => Value::Number(*value),
        ScriptValue::Str(value) => Value::String(ctx.intern(value.as_bytes())),
        ScriptValue::List(items) => {
            let table = Table::new(&ctx);
            for (index, item) in items.iter().enumerate() {
                if matches!(item, ScriptValue::Unit) {
                    return Err(ValueError::Value(UNIT_IN_LIST.into()));
                }
                let key = Value::Integer(index as i64 + 1);
                table
                    .set_raw(&ctx, key, to_value(ctx, item, depth + 1)?)
                    .map_err(|e| ValueError::Value(e.to_string()))?;
            }
            Value::Table(table)
        },
        ScriptValue::Map(entries) => {
            let table = Table::new(&ctx);
            for (key, item) in entries {
                if !matches!(item, ScriptValue::Unit) {
                    put(ctx, table, key, to_value(ctx, item, depth + 1)?)?;
                }
            }
            Value::Table(table)
        },
    })
}

fn from_value<'gc>(
    value: Value<'gc>,
    depth: usize,
    nodes: &mut usize,
) -> Result<ScriptValue, ValueError> {
    if depth > MAX_DEPTH {
        return Err(ValueError::Value("nested too deeply".into()));
    }
    spend(nodes, 1)?;
    match value {
        Value::Nil => Ok(ScriptValue::Unit),
        Value::Boolean(value) => Ok(ScriptValue::Bool(value)),
        Value::Integer(value) => Ok(ScriptValue::Int(value)),
        Value::Number(value) => Ok(ScriptValue::Float(value)),
        Value::String(text) => text
            .to_str()
            .map(|text| ScriptValue::Str(text.to_owned()))
            .map_err(|_| ValueError::Value("a string that is not UTF-8".into())),
        Value::Table(table) => from_table(table, depth, nodes),
        other => Err(ValueError::Value(format!(
            "lua value of type {}",
            other.type_name()
        ))),
    }
}

fn spend(nodes: &mut usize, count: usize) -> Result<(), ValueError> {
    *nodes = nodes.saturating_add(count);
    if *nodes > MAX_NODES {
        Err(ValueError::Value("too many values to carry".into()))
    } else {
        Ok(())
    }
}

fn from_table<'gc>(
    table: Table<'gc>,
    depth: usize,
    nodes: &mut usize,
) -> Result<ScriptValue, ValueError> {
    let pairs: Vec<_> = table.iter().collect();
    spend(nodes, pairs.len())?;
    if pairs
        .iter()
        .all(|(key, _)| matches!(key, Value::Integer(_)))
    {
        // Keys 1..=n with no gap: n distinct keys all inside that range.
        let mut slots: Vec<Option<ScriptValue>> = vec![None; pairs.len()];
        for (key, item) in pairs {
            let Value::Integer(key) = key else {
                unreachable!("every key was checked to be an integer")
            };
            let slot = key
                .checked_sub(1)
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| slots.get_mut(index))
                .ok_or_else(|| ValueError::Value("a table with gaps in its positions".into()))?;
            *slot = Some(from_value(item, depth + 1, nodes)?);
        }
        Ok(ScriptValue::List(slots.into_iter().flatten().collect()))
    } else if pairs.iter().all(|(key, _)| matches!(key, Value::String(_))) {
        let mut entries = BTreeMap::new();
        for (key, item) in pairs {
            let Value::String(key) = key else {
                unreachable!("every key was checked to be a string")
            };
            let key = key
                .to_str()
                .map_err(|_| ValueError::Value("a key that is not UTF-8".into()))?;
            entries.insert(key.to_owned(), from_value(item, depth + 1, nodes)?);
        }
        Ok(ScriptValue::Map(entries))
    } else {
        Err(ValueError::Value(
            "a table whose keys are not all positions or all strings".into(),
        ))
    }
}

/// Run host function `name` on the call's arguments. The arguments are copied
/// out, never borrowed, so the caller's tables are untouched.
fn call_host<'gc>(
    ctx: Context<'gc>,
    mut stack: Stack<'gc, '_>,
    name: &str,
    arity: usize,
    function: &HostFunction,
) -> Result<CallbackReturn<'gc>, Error<'gc>> {
    if stack.len() != arity {
        let given = stack.len();
        let message = format!("`{name}` takes {arity} arguments, got {given}");
        return Err(ValueError::Script(message).into());
    }
    let mut nodes = 0;
    let args = (0..arity)
        .map(|index| from_value(stack.get(index), 0, &mut nodes))
        .collect::<Result<Vec<_>, _>>()?;
    let returned = function(&args).map_err(ValueError::Host)?;
    let value = to_value(ctx, &returned, 0)?;
    stack.clear();
    stack.push_back(value);
    Ok(CallbackReturn::Return)
}

/// The host's own [`ValueError`] when one was carried through Lua, else the
/// script's error in Lua's words.
fn script_error(error: Error<'_>) -> ValueError {
    match &error {
        Error::Runtime(carried) => carried
            .downcast::<ValueError>()
            .cloned()
            .unwrap_or_else(|| ValueError::Script(error.to_string())),
        Error::Lua(_) => ValueError::Script(error.to_string()),
    }
}

/// Step `executor` until it finishes or `limit` fuel is spent; `None` has no
/// limit. Running out is [`ValueError::Budget`].
fn run(lua: &mut Lua, executor: &StashedExecutor, limit: Option<u64>) -> Result<(), ValueError> {
    let mut left = limit;
    loop {
        let slice = left
            .and_then(|left| i32::try_from(left).ok())
            .unwrap_or(FUEL_PER_STEP)
            .min(FUEL_PER_STEP);
        let mut fuel = Fuel::with(slice);
        let finished = lua
            .enter(|ctx| ctx.fetch(executor).step(ctx, &mut fuel))
            .map_err(|e| ValueError::Script(e.to_string()))?;
        if finished {
            return Ok(());
        }
        if let Some(remaining) = left {
            // A callback may overdraw the slice; count at least one unit so an
            // interrupt cannot spin forever.
            let spent = i64::from(slice) - i64::from(fuel.remaining());
            let remaining = remaining.saturating_sub(u64::try_from(spent.max(1)).unwrap_or(1));
            if remaining == 0 {
                return Err(ValueError::Budget);
            }
            left = Some(remaining);
        }
    }
}

impl ValueEngine for LuaValueEngine {
    fn set_global(&mut self, name: &str, value: ScriptValue) -> Result<(), ValueError> {
        identifier(name)?;
        if self.functions.contains_key(name) {
            return Err(ValueError::Host(format!("`{name}` is already registered")));
        }
        check(&value, 0)?;
        self.globals.insert(name.to_string(), value);
        Ok(())
    }

    fn register(
        &mut self,
        name: &str,
        arity: usize,
        function: HostFunction,
    ) -> Result<(), ValueError> {
        identifier(name)?;
        // A global and a function share Lua's one namespace.
        if self.functions.contains_key(name) || self.globals.contains_key(name) {
            return Err(ValueError::Host(format!("`{name}` is already registered")));
        }
        self.functions.insert(name.to_string(), (arity, function));
        Ok(())
    }

    fn eval(&mut self, source: &str, budget: Budget) -> Result<ScriptValue, ValueError> {
        let limit = match budget {
            Budget::Unbounded => None,
            // An empty budget runs nothing.
            Budget::Steps(0) => return Err(ValueError::Budget),
            Budget::Steps(steps) => Some(steps),
        };
        let mut lua = self.state()?;
        let executor = lua.enter(|ctx| {
            // A bare value first, as Lua's own prompt does; otherwise the
            // source as written, whose compile error is the one reported.
            let as_value = format!("{RETURN_PREFIX}{source}");
            let closure = match Closure::load(ctx, Some(CHUNK_NAME), as_value.as_bytes()) {
                Ok(closure) => closure,
                Err(_) => Closure::load(ctx, Some(CHUNK_NAME), source.as_bytes())
                    .map_err(|e| ValueError::Script(e.to_string()))?,
            };
            Ok::<_, ValueError>(ctx.stash(Executor::start(ctx, closure.into(), ())))
        })?;
        run(&mut lua, &executor, limit)?;
        lua.enter(|ctx| {
            let result = ctx.fetch(&executor).take_result::<Value>(ctx);
            // A main chunk that yields leaves the executor suspended, not
            // finished; that is a script error, not a result.
            match result.map_err(|e| ValueError::Script(e.to_string()))? {
                Ok(value) => from_value(value, 0, &mut 0),
                Err(error) => Err(script_error(error)),
            }
        })
    }
}

#[cfg(test)]
#[path = "value_tests.rs"]
mod tests;
