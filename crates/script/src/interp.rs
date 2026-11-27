// SPDX-License-Identifier: MIT
//! The tree-walking interpreter.

use crate::parser::{BinOp, Expr, ExprKind, FnDef, Script, Stmt, StmtKind};
use crate::ScriptError;
use orbe_core::Variant;
use orbe_math::Vec2;
use std::collections::BTreeMap;
use std::rc::Rc;

type Scope = BTreeMap<String, Variant>;
type Res<T> = Result<T, ScriptError>;

/// Resource bounds for one [`Vm::run`] or [`Vm::call`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Most evaluation steps (statements, expressions and calls).
    pub steps: u64,
    /// Deepest function call nesting.
    pub call_depth: usize,
    /// Longest array, dictionary or string a script may build.
    pub max_len: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            steps: 1_000_000,
            call_depth: 64,
            max_len: 100_000,
        }
    }
}

/// The functions the embedding game offers to scripts.
pub trait Host {
    /// Calls the host function `name`. Returns `None` when the host has no such function, so the
    /// script gets an "unknown function" error; `Some(Err(message))` fails the script.
    fn call(&mut self, name: &str, args: &[Variant]) -> Option<Result<Variant, String>>;
}

/// A host that offers nothing.
pub struct NoHost;

impl Host for NoHost {
    fn call(&mut self, _name: &str, _args: &[Variant]) -> Option<Result<Variant, String>> {
        None
    }
}

/// A script and its global variables.
pub struct Vm {
    functions: BTreeMap<String, Rc<FnDef>>,
    body: Vec<Stmt>,
    globals: Scope,
    limits: Limits,
}

impl Vm {
    /// A machine for `script` with the default [`Limits`].
    pub fn new(script: Script) -> Vm {
        Vm::with_limits(script, Limits::default())
    }

    /// A machine for `script` with explicit limits.
    pub fn with_limits(script: Script, limits: Limits) -> Vm {
        Vm {
            functions: script.functions,
            body: script.body,
            globals: Scope::new(),
            limits,
        }
    }

    /// Runs the script's top-level statements; its globals persist for later [`Vm::call`]s.
    pub fn run(&mut self, host: &mut dyn Host) -> Res<()> {
        let body = std::mem::take(&mut self.body);
        let result = self.with_ctx(host, |ctx| ctx.exec_block_in_place(&body).map(|_| ()));
        self.body = body;
        result
    }

    /// Calls the script function `name` with `args`.
    pub fn call(&mut self, name: &str, args: &[Variant], host: &mut dyn Host) -> Res<Variant> {
        self.with_ctx(host, |ctx| ctx.call_user(name, args, 0))
    }

    /// The value of a global variable.
    pub fn global(&self, name: &str) -> Option<&Variant> {
        self.globals.get(name)
    }

    /// Sets a global variable.
    pub fn set_global(&mut self, name: &str, value: Variant) {
        self.globals.insert(name.to_string(), value);
    }

    /// Whether the script defines the function `name`.
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    fn with_ctx<T>(
        &mut self,
        host: &mut dyn Host,
        f: impl FnOnce(&mut Ctx<'_>) -> Res<T>,
    ) -> Res<T> {
        let mut ctx = Ctx {
            functions: &self.functions,
            host,
            frames: vec![vec![std::mem::take(&mut self.globals)]],
            steps: 0,
            depth: 0,
            limits: self.limits,
        };
        let result = f(&mut ctx);
        // Frame 0 is always the globals, whatever happened.
        ctx.frames.truncate(1);
        if let Some(mut frame) = ctx.frames.pop() {
            frame.truncate(1);
            if let Some(g) = frame.pop() {
                self.globals = g;
            }
        }
        result
    }
}

enum Flow {
    Normal,
    Break,
    Continue,
    Return(Variant),
}

struct Ctx<'a> {
    functions: &'a BTreeMap<String, Rc<FnDef>>,
    host: &'a mut dyn Host,
    /// Frame 0 holds the globals; each call pushes a frame of scopes.
    frames: Vec<Vec<Scope>>,
    steps: u64,
    /// Rust recursion in progress (statements and expressions), bounded by [`MAX_DEPTH`].
    depth: u32,
    limits: Limits,
}

/// Deepest combined nesting of statements and expressions being evaluated at once; it keeps the
/// interpreter's own stack use bounded however a script nests calls and expressions.
const MAX_DEPTH: u32 = 160;

fn truthy(v: &Variant) -> bool {
    !matches!(v, Variant::Nil | Variant::Bool(false))
}

fn num(v: &Variant) -> Option<f64> {
    match v {
        Variant::Int(i) => Some(*i as f64),
        Variant::Float(f) => Some(*f),
        _ => None,
    }
}

fn values_equal(a: &Variant, b: &Variant) -> bool {
    match (num(a), num(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

fn finite(f: f64, line: u32) -> Res<Variant> {
    if f.is_finite() {
        Ok(Variant::Float(f))
    } else {
        Err(ScriptError::new(
            line,
            "arithmetic produced a non-finite number",
        ))
    }
}

impl Ctx<'_> {
    fn step(&mut self, line: u32) -> Res<()> {
        self.steps += 1;
        if self.steps > self.limits.steps {
            Err(ScriptError::new(line, "script ran too long"))
        } else {
            Ok(())
        }
    }

    fn enter(&mut self, line: u32) -> Res<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.depth -= 1;
            Err(ScriptError::new(line, "nested too deeply"))
        } else {
            Ok(())
        }
    }

    fn check_len(&self, len: usize, line: u32) -> Res<()> {
        if len > self.limits.max_len {
            Err(ScriptError::new(line, "value is too large"))
        } else {
            Ok(())
        }
    }

    // ---- variables ----

    fn find_mut(&mut self, name: &str) -> Option<&mut Variant> {
        let top = self.frames.len().checked_sub(1)?;
        // The current frame first, then the globals.
        let in_top = self
            .frames
            .get(top)
            .is_some_and(|f| f.iter().any(|s| s.contains_key(name)));
        let frame_index = if in_top { top } else { 0 };
        self.frames
            .get_mut(frame_index)?
            .iter_mut()
            .rev()
            .find_map(|s| s.get_mut(name))
    }

    fn lookup(&mut self, name: &str) -> Option<Variant> {
        self.find_mut(name).map(|v| v.clone())
    }

    fn define(&mut self, name: &str, value: Variant) {
        if let Some(scope) = self.frames.last_mut().and_then(|f| f.last_mut()) {
            scope.insert(name.to_string(), value);
        }
    }

    // ---- statements ----

    /// Runs `stmts` in the current innermost scope (the top level and function bodies).
    fn exec_block_in_place(&mut self, stmts: &[Stmt]) -> Res<Flow> {
        for s in stmts {
            match self.exec(s)? {
                Flow::Normal => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    fn exec_block(&mut self, stmts: &[Stmt]) -> Res<Flow> {
        if let Some(f) = self.frames.last_mut() {
            f.push(Scope::new());
        }
        let r = self.exec_block_in_place(stmts);
        if let Some(f) = self.frames.last_mut() {
            f.pop();
        }
        r
    }

    fn exec(&mut self, s: &Stmt) -> Res<Flow> {
        self.enter(s.line)?;
        let r = self.exec_inner(s);
        self.depth -= 1;
        r
    }

    fn exec_inner(&mut self, s: &Stmt) -> Res<Flow> {
        self.step(s.line)?;
        match &s.kind {
            StmtKind::Let(name, e) => {
                let v = self.eval(e)?;
                self.define(name, v);
            }
            StmtKind::Assign {
                name,
                indices,
                op,
                value,
            } => {
                let rhs = self.eval(value)?;
                let mut idx = Vec::with_capacity(indices.len());
                for i in indices {
                    idx.push(self.eval(i)?);
                }
                self.assign(name, &idx, *op, rhs, s.line)?;
            }
            StmtKind::If(arms, otherwise) => {
                for (cond, body) in arms {
                    if truthy(&self.eval(cond)?) {
                        return self.exec_block(body);
                    }
                }
                if let Some(body) = otherwise {
                    return self.exec_block(body);
                }
            }
            StmtKind::While(cond, body) => {
                while truthy(&self.eval(cond)?) {
                    self.step(s.line)?;
                    match self.exec_block(body)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Normal | Flow::Continue => {}
                    }
                }
            }
            StmtKind::For(name, iter, body) => {
                let items: Vec<Variant> = match self.eval(iter)? {
                    Variant::Array(a) => a,
                    Variant::Dict(d) => d.into_keys().map(Variant::Str).collect(),
                    Variant::Str(t) => t.chars().map(|c| Variant::Str(c.to_string())).collect(),
                    other => {
                        return Err(ScriptError::new(
                            s.line,
                            format!("cannot loop over a {}", other.type_name()),
                        ))
                    }
                };
                for item in items {
                    self.step(s.line)?;
                    if let Some(f) = self.frames.last_mut() {
                        f.push(Scope::new());
                    }
                    self.define(name, item);
                    let r = self.exec_block_in_place(body);
                    if let Some(f) = self.frames.last_mut() {
                        f.pop();
                    }
                    match r? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Normal | Flow::Continue => {}
                    }
                }
            }
            StmtKind::Return(e) => {
                let v = match e {
                    Some(e) => self.eval(e)?,
                    None => Variant::Nil,
                };
                return Ok(Flow::Return(v));
            }
            StmtKind::Break => return Ok(Flow::Break),
            StmtKind::Continue => return Ok(Flow::Continue),
            StmtKind::Expr(e) => {
                self.eval(e)?;
            }
        }
        Ok(Flow::Normal)
    }

    fn assign(
        &mut self,
        name: &str,
        indices: &[Variant],
        op: Option<BinOp>,
        rhs: Variant,
        line: u32,
    ) -> Res<()> {
        let max_len = self.limits.max_len;
        let Some(var) = self.find_mut(name) else {
            return Err(ScriptError::new(
                line,
                format!("`{name}` is not defined (use `let`)"),
            ));
        };
        let slot = place(var, indices, line)?;
        let new = match op {
            None => rhs,
            Some(op) => binop(op, slot.clone(), rhs, line, max_len)?,
        };
        *slot = new;
        Ok(())
    }

    // ---- expressions ----

    fn eval(&mut self, e: &Expr) -> Res<Variant> {
        self.enter(e.line)?;
        let r = self.eval_inner(e);
        self.depth -= 1;
        r
    }

    fn eval_inner(&mut self, e: &Expr) -> Res<Variant> {
        self.step(e.line)?;
        match &e.kind {
            ExprKind::Lit(v) => Ok(v.clone()),
            ExprKind::Var(name) => self
                .lookup(name)
                .ok_or_else(|| ScriptError::new(e.line, format!("`{name}` is not defined"))),
            ExprKind::Array(items) => {
                self.check_len(items.len(), e.line)?;
                let mut out = Vec::with_capacity(items.len());
                for i in items {
                    out.push(self.eval(i)?);
                }
                Ok(Variant::Array(out))
            }
            ExprKind::Dict(pairs) => {
                self.check_len(pairs.len(), e.line)?;
                let mut out = BTreeMap::new();
                for (k, v) in pairs {
                    let Variant::Str(key) = self.eval(k)? else {
                        return Err(ScriptError::new(e.line, "dictionary keys must be strings"));
                    };
                    out.insert(key, self.eval(v)?);
                }
                Ok(Variant::Dict(out))
            }
            ExprKind::Neg(inner) => match self.eval(inner)? {
                Variant::Int(i) => i
                    .checked_neg()
                    .map(Variant::Int)
                    .ok_or_else(|| ScriptError::new(e.line, "integer overflow")),
                Variant::Float(f) => Ok(Variant::Float(-f)),
                Variant::Vec2(v) => Ok(Variant::Vec2(Vec2::new(-v.x, -v.y))),
                other => Err(ScriptError::new(
                    e.line,
                    format!("cannot negate a {}", other.type_name()),
                )),
            },
            ExprKind::Not(inner) => Ok(Variant::Bool(!truthy(&self.eval(inner)?))),
            ExprKind::Binary(BinOp::And, a, b) => {
                let left = self.eval(a)?;
                if truthy(&left) {
                    self.eval(b)
                } else {
                    Ok(left)
                }
            }
            ExprKind::Binary(BinOp::Or, a, b) => {
                let left = self.eval(a)?;
                if truthy(&left) {
                    Ok(left)
                } else {
                    self.eval(b)
                }
            }
            ExprKind::Binary(op, a, b) => {
                let (x, y) = (self.eval(a)?, self.eval(b)?);
                binop(*op, x, y, e.line, self.limits.max_len)
            }
            ExprKind::Index(base, index) => {
                let (b, i) = (self.eval(base)?, self.eval(index)?);
                read_index(&b, &i, e.line)
            }
            ExprKind::Call(name, args) => {
                let mut values = Vec::with_capacity(args.len());
                for a in args {
                    values.push(self.eval(a)?);
                }
                self.call(name, &values, e.line)
            }
        }
    }

    fn call(&mut self, name: &str, args: &[Variant], line: u32) -> Res<Variant> {
        if self.functions.contains_key(name) {
            return self.call_user(name, args, line);
        }
        if let Some(r) = self.builtin(name, args, line) {
            return r;
        }
        match self.host.call(name, args) {
            Some(Ok(v)) => Ok(v),
            Some(Err(m)) => Err(ScriptError::new(line, m)),
            None => Err(ScriptError::new(line, format!("unknown function `{name}`"))),
        }
    }

    fn call_user(&mut self, name: &str, args: &[Variant], line: u32) -> Res<Variant> {
        let def = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| ScriptError::new(line, format!("unknown function `{name}`")))?;
        if args.len() != def.params.len() {
            return Err(ScriptError::new(
                line,
                format!(
                    "`{name}` takes {} argument(s), got {}",
                    def.params.len(),
                    args.len()
                ),
            ));
        }
        if self.frames.len() > self.limits.call_depth {
            return Err(ScriptError::new(line, "calls are nested too deeply"));
        }
        self.step(line)?;
        let mut scope = Scope::new();
        for (p, a) in def.params.iter().zip(args) {
            scope.insert(p.clone(), a.clone());
        }
        self.frames.push(vec![scope]);
        let depth = self.frames.len();
        let r = self.exec_block_in_place(&def.body);
        self.frames.truncate(depth - 1);
        match r? {
            Flow::Return(v) => Ok(v),
            _ => Ok(Variant::Nil),
        }
    }

    /// The built-in functions; `None` when `name` is not one of them.
    fn builtin(&mut self, name: &str, args: &[Variant], line: u32) -> Option<Res<Variant>> {
        let fail = |m: &str| Some(Err(ScriptError::new(line, format!("{name}: {m}"))));
        let r = match (name, args) {
            ("len", [v]) => match v {
                Variant::Array(a) => Variant::Int(a.len() as i64),
                Variant::Dict(d) => Variant::Int(d.len() as i64),
                Variant::Str(s) => Variant::Int(s.chars().count() as i64),
                _ => return fail("expects an array, dictionary or string"),
            },
            ("str", [v]) => Variant::Str(to_text(v)),
            ("int", [v]) => match v {
                Variant::Int(i) => Variant::Int(*i),
                Variant::Float(f) if f.is_finite() => Variant::Int(f.trunc() as i64),
                Variant::Bool(b) => Variant::Int(i64::from(*b)),
                Variant::Str(s) => s.trim().parse::<i64>().map_or(Variant::Nil, Variant::Int),
                _ => return fail("cannot convert"),
            },
            ("float", [v]) => match v {
                Variant::Int(i) => Variant::Float(*i as f64),
                Variant::Float(f) => Variant::Float(*f),
                Variant::Str(s) => match s.trim().parse::<f64>() {
                    Ok(f) if f.is_finite() => Variant::Float(f),
                    _ => Variant::Nil,
                },
                _ => return fail("cannot convert"),
            },
            ("type", [v]) => Variant::Str(v.type_name().to_string()),
            ("vec2", [x, y]) => match (num(x), num(y)) {
                (Some(x), Some(y)) if x.is_finite() && y.is_finite() => {
                    Variant::Vec2(Vec2::new(x as f32, y as f32))
                }
                _ => return fail("expects two numbers"),
            },
            ("range", [n]) => match n {
                Variant::Int(n) => return Some(self.range(0, *n, 1, line)),
                _ => return fail("expects integers"),
            },
            ("range", [Variant::Int(a), Variant::Int(b)]) => {
                return Some(self.range(*a, *b, 1, line))
            }
            ("range", [Variant::Int(a), Variant::Int(b), Variant::Int(s)]) => {
                return Some(self.range(*a, *b, *s, line))
            }
            ("append", [Variant::Array(a), v]) => {
                if let Err(e) = self.check_len(a.len() + 1, line) {
                    return Some(Err(e));
                }
                let mut a = a.clone();
                a.push(v.clone());
                Variant::Array(a)
            }
            ("keys", [Variant::Dict(d)]) => {
                Variant::Array(d.keys().cloned().map(Variant::Str).collect())
            }
            ("has", [Variant::Dict(d), Variant::Str(k)]) => Variant::Bool(d.contains_key(k)),
            ("abs", [v]) => match v {
                Variant::Int(i) => match i.checked_abs() {
                    Some(a) => Variant::Int(a),
                    None => return fail("overflow"),
                },
                Variant::Float(f) => Variant::Float(f.abs()),
                _ => return fail("expects a number"),
            },
            ("min", [a, b]) | ("max", [a, b]) => {
                let (Some(x), Some(y)) = (num(a), num(b)) else {
                    return fail("expects two numbers");
                };
                let pick_a = if name == "min" { x <= y } else { x >= y };
                if pick_a {
                    a.clone()
                } else {
                    b.clone()
                }
            }
            ("floor", [v]) | ("ceil", [v]) | ("sqrt", [v]) | ("sin", [v]) | ("cos", [v]) => {
                let Some(x) = num(v) else {
                    return fail("expects a number");
                };
                let y = match name {
                    "floor" => x.floor(),
                    "ceil" => x.ceil(),
                    "sqrt" => x.sqrt(),
                    "sin" => x.sin(),
                    _ => x.cos(),
                };
                return Some(finite(y, line));
            }
            ("pow", [a, b]) => {
                let (Some(x), Some(y)) = (num(a), num(b)) else {
                    return fail("expects two numbers");
                };
                return Some(finite(x.powf(y), line));
            }
            (
                "len" | "str" | "int" | "float" | "type" | "vec2" | "range" | "append" | "keys"
                | "has" | "abs" | "min" | "max" | "floor" | "ceil" | "sqrt" | "sin" | "cos" | "pow",
                _,
            ) => return fail("wrong arguments"),
            _ => return None,
        };
        Some(Ok(r))
    }

    fn range(&self, from: i64, to: i64, step: i64, line: u32) -> Res<Variant> {
        if step == 0 {
            return Err(ScriptError::new(line, "range: step cannot be zero"));
        }
        let diff = i128::from(to) - i128::from(from);
        let s = i128::from(step);
        let count: u128 = if (diff > 0 && s > 0) || (diff < 0 && s < 0) {
            ((diff.abs() + s.abs() - 1) / s.abs()) as u128
        } else {
            0
        };
        if count > self.limits.max_len as u128 {
            return Err(ScriptError::new(line, "range: too many items"));
        }
        let mut out = Vec::with_capacity(count as usize);
        let mut v = from;
        for _ in 0..count {
            out.push(Variant::Int(v));
            v = v.saturating_add(step);
        }
        Ok(Variant::Array(out))
    }
}

fn to_text(v: &Variant) -> String {
    match v {
        Variant::Nil => "nil".to_string(),
        Variant::Bool(b) => b.to_string(),
        Variant::Int(i) => i.to_string(),
        Variant::Float(f) => f.to_string(),
        Variant::Str(s) => s.clone(),
        Variant::Vec2(v) => format!("({}, {})", v.x, v.y),
        Variant::Array(a) => format!("[{}]", a.iter().map(to_text).collect::<Vec<_>>().join(", ")),
        Variant::Dict(d) => format!(
            "{{{}}}",
            d.iter()
                .map(|(k, v)| format!("{k}: {}", to_text(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => format!("<{}>", other.type_name()),
    }
}

fn index_of(v: &Variant, len: usize, line: u32) -> Res<usize> {
    match v {
        Variant::Int(i) if *i >= 0 && (*i as u64) < len as u64 => Ok(*i as usize),
        Variant::Int(_) => Err(ScriptError::new(line, "index out of range")),
        _ => Err(ScriptError::new(line, "an index must be an integer")),
    }
}

fn read_index(base: &Variant, index: &Variant, line: u32) -> Res<Variant> {
    match base {
        Variant::Array(a) => Ok(a
            .get(index_of(index, a.len(), line)?)
            .cloned()
            .unwrap_or_default()),
        Variant::Str(s) => {
            let chars: Vec<char> = s.chars().collect();
            let i = index_of(index, chars.len(), line)?;
            Ok(Variant::Str(
                chars.get(i).map(char::to_string).unwrap_or_default(),
            ))
        }
        Variant::Dict(d) => match index {
            Variant::Str(k) => Ok(d.get(k).cloned().unwrap_or_default()),
            _ => Err(ScriptError::new(line, "dictionary keys must be strings")),
        },
        Variant::Vec2(v) => match index {
            Variant::Int(0) => Ok(Variant::Float(f64::from(v.x))),
            Variant::Int(1) => Ok(Variant::Float(f64::from(v.y))),
            _ => Err(ScriptError::new(line, "index out of range")),
        },
        other => Err(ScriptError::new(
            line,
            format!("cannot index a {}", other.type_name()),
        )),
    }
}

/// The slot `indices` leads to inside `var`; a missing dictionary key is created as `nil`.
fn place<'v>(var: &'v mut Variant, indices: &[Variant], line: u32) -> Res<&'v mut Variant> {
    let Some((first, rest)) = indices.split_first() else {
        return Ok(var);
    };
    let next = match var {
        Variant::Array(a) => {
            let i = index_of(first, a.len(), line)?;
            a.get_mut(i)
                .ok_or_else(|| ScriptError::new(line, "index out of range"))?
        }
        Variant::Dict(d) => match first {
            Variant::Str(k) => d.entry(k.clone()).or_default(),
            _ => return Err(ScriptError::new(line, "dictionary keys must be strings")),
        },
        other => {
            return Err(ScriptError::new(
                line,
                format!("cannot assign into a {}", other.type_name()),
            ))
        }
    };
    place(next, rest, line)
}

fn binop(op: BinOp, a: Variant, b: Variant, line: u32, max_len: usize) -> Res<Variant> {
    let bad = |a: &Variant, b: &Variant| {
        ScriptError::new(
            line,
            format!(
                "cannot apply {op:?} to a {} and a {}",
                a.type_name(),
                b.type_name()
            ),
        )
    };
    match op {
        BinOp::Eq => return Ok(Variant::Bool(values_equal(&a, &b))),
        BinOp::Ne => return Ok(Variant::Bool(!values_equal(&a, &b))),
        BinOp::And | BinOp::Or => return Ok(Variant::Bool(truthy(&a) && truthy(&b))),
        _ => {}
    }
    match (&a, &b) {
        (Variant::Int(x), Variant::Int(y)) => {
            let r = match op {
                BinOp::Add => x.checked_add(*y),
                BinOp::Sub => x.checked_sub(*y),
                BinOp::Mul => x.checked_mul(*y),
                BinOp::Div | BinOp::Rem if *y == 0 => {
                    return Err(ScriptError::new(line, "division by zero"))
                }
                BinOp::Div => x.checked_div(*y),
                BinOp::Rem => x.checked_rem(*y),
                BinOp::Lt => return Ok(Variant::Bool(x < y)),
                BinOp::Le => return Ok(Variant::Bool(x <= y)),
                BinOp::Gt => return Ok(Variant::Bool(x > y)),
                BinOp::Ge => return Ok(Variant::Bool(x >= y)),
                _ => None,
            };
            r.map(Variant::Int)
                .ok_or_else(|| ScriptError::new(line, "integer overflow"))
        }
        (Variant::Str(x), Variant::Str(y)) => match op {
            BinOp::Add => {
                if x.len() + y.len() > max_len {
                    return Err(ScriptError::new(line, "value is too large"));
                }
                Ok(Variant::Str(format!("{x}{y}")))
            }
            BinOp::Lt => Ok(Variant::Bool(x < y)),
            BinOp::Le => Ok(Variant::Bool(x <= y)),
            BinOp::Gt => Ok(Variant::Bool(x > y)),
            BinOp::Ge => Ok(Variant::Bool(x >= y)),
            _ => Err(bad(&a, &b)),
        },
        (Variant::Array(x), Variant::Array(y)) if op == BinOp::Add => {
            if x.len() + y.len() > max_len {
                return Err(ScriptError::new(line, "value is too large"));
            }
            Ok(Variant::Array(x.iter().chain(y).cloned().collect()))
        }
        (Variant::Vec2(x), Variant::Vec2(y)) => match op {
            BinOp::Add => Ok(Variant::Vec2(Vec2::new(x.x + y.x, x.y + y.y))),
            BinOp::Sub => Ok(Variant::Vec2(Vec2::new(x.x - y.x, x.y - y.y))),
            _ => Err(bad(&a, &b)),
        },
        (Variant::Vec2(v), s) | (s, Variant::Vec2(v)) if num(s).is_some() => {
            let k = num(s).unwrap_or(0.0) as f32;
            match op {
                BinOp::Mul if k.is_finite() => Ok(Variant::Vec2(Vec2::new(v.x * k, v.y * k))),
                BinOp::Div if matches!(a, Variant::Vec2(_)) && k != 0.0 && k.is_finite() => {
                    Ok(Variant::Vec2(Vec2::new(v.x / k, v.y / k)))
                }
                _ => Err(bad(&a, &b)),
            }
        }
        _ => {
            let (Some(x), Some(y)) = (num(&a), num(&b)) else {
                return Err(bad(&a, &b));
            };
            match op {
                BinOp::Add => finite(x + y, line),
                BinOp::Sub => finite(x - y, line),
                BinOp::Mul => finite(x * y, line),
                BinOp::Div | BinOp::Rem if y == 0.0 => {
                    Err(ScriptError::new(line, "division by zero"))
                }
                BinOp::Div => finite(x / y, line),
                BinOp::Rem => finite(x % y, line),
                BinOp::Lt => Ok(Variant::Bool(x < y)),
                BinOp::Le => Ok(Variant::Bool(x <= y)),
                BinOp::Gt => Ok(Variant::Bool(x > y)),
                BinOp::Ge => Ok(Variant::Bool(x >= y)),
                _ => Err(bad(&a, &b)),
            }
        }
    }
}
