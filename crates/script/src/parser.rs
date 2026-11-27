// SPDX-License-Identifier: MIT
//! Tokens to a syntax tree.

use crate::lexer::{lex, Tok, Token};
use crate::ScriptError;
use orbe_core::Variant;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Deepest nesting of expressions and blocks.
pub const MAX_NESTING: u32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub line: u32,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Lit(Variant),
    Var(String),
    Array(Vec<Expr>),
    Dict(Vec<(Expr, Expr)>),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub line: u32,
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    Let(String, Expr),
    /// `name[i][j] op= value`; `op` is `None` for plain `=`.
    Assign {
        name: String,
        indices: Vec<Expr>,
        op: Option<BinOp>,
        value: Expr,
    },
    If(Vec<(Expr, Vec<Stmt>)>, Option<Vec<Stmt>>),
    While(Expr, Vec<Stmt>),
    For(String, Expr, Vec<Stmt>),
    Return(Option<Expr>),
    Break,
    Continue,
    Expr(Expr),
}

#[derive(Debug)]
pub struct FnDef {
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

/// A parsed script: its functions and its top-level statements.
#[derive(Debug)]
pub struct Script {
    pub(crate) functions: BTreeMap<String, Rc<FnDef>>,
    pub(crate) body: Vec<Stmt>,
}

impl Script {
    /// Parses `source`.
    pub fn parse(source: &str) -> Result<Script, ScriptError> {
        let tokens = lex(source)?;
        let mut p = Parser {
            tokens,
            pos: 0,
            depth: 0,
        };
        let mut functions = BTreeMap::new();
        let mut body = Vec::new();
        loop {
            p.skip_separators();
            if p.peek() == &Tok::Eof {
                break;
            }
            if p.peek() == &Tok::Fn {
                let line = p.line();
                p.pos += 1;
                let name = p.ident("a function name")?;
                p.expect(&Tok::LParen, "`(`")?;
                let mut params = Vec::new();
                p.skip_newlines();
                while p.peek() != &Tok::RParen {
                    params.push(p.ident("a parameter name")?);
                    p.skip_newlines();
                    if p.peek() == &Tok::Comma {
                        p.pos += 1;
                        p.skip_newlines();
                    } else {
                        break;
                    }
                }
                p.expect(&Tok::RParen, "`)`")?;
                let fbody = p.block()?;
                if functions
                    .insert(
                        name.clone(),
                        Rc::new(FnDef {
                            params,
                            body: fbody,
                        }),
                    )
                    .is_some()
                {
                    return Err(ScriptError::new(
                        line,
                        format!("function `{name}` is defined twice"),
                    ));
                }
            } else {
                body.push(p.statement()?);
            }
        }
        Ok(Script { functions, body })
    }

    /// Names of the functions the script defines, sorted.
    pub fn function_names(&self) -> Vec<&str> {
        self.functions.keys().map(String::as_str).collect()
    }
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    depth: u32,
}

impl Parser {
    fn peek(&self) -> &Tok {
        static EOF: Tok = Tok::Eof;
        self.tokens.get(self.pos).map_or(&EOF, |t| &t.tok)
    }

    fn line(&self) -> u32 {
        self.tokens
            .get(self.pos)
            .or_else(|| self.tokens.last())
            .map_or(0, |t| t.line)
    }

    fn skip_newlines(&mut self) {
        while self.peek() == &Tok::Newline {
            self.pos += 1;
        }
    }

    fn skip_separators(&mut self) {
        while matches!(self.peek(), Tok::Newline | Tok::Semi) {
            self.pos += 1;
        }
    }

    fn err<T>(&self, message: impl Into<String>) -> Result<T, ScriptError> {
        Err(ScriptError::new(self.line(), message))
    }

    fn expect(&mut self, tok: &Tok, what: &str) -> Result<(), ScriptError> {
        if self.peek() == tok {
            self.pos += 1;
            Ok(())
        } else {
            self.err(format!("expected {what}"))
        }
    }

    fn ident(&mut self, what: &str) -> Result<String, ScriptError> {
        if let Tok::Ident(name) = self.peek() {
            let name = name.clone();
            self.pos += 1;
            Ok(name)
        } else {
            self.err(format!("expected {what}"))
        }
    }

    fn enter(&mut self) -> Result<(), ScriptError> {
        self.depth += 1;
        if self.depth > MAX_NESTING {
            return self.err("nested too deeply");
        }
        Ok(())
    }

    fn block(&mut self) -> Result<Vec<Stmt>, ScriptError> {
        self.enter()?;
        self.skip_newlines();
        self.expect(&Tok::LBrace, "`{`")?;
        let mut stmts = Vec::new();
        loop {
            self.skip_separators();
            match self.peek() {
                Tok::RBrace => {
                    self.pos += 1;
                    break;
                }
                Tok::Eof => return self.err("missing `}`"),
                Tok::Fn => return self.err("functions can only be defined at the top level"),
                _ => stmts.push(self.statement()?),
            }
        }
        self.depth -= 1;
        Ok(stmts)
    }

    fn end_of_statement(&mut self) -> Result<(), ScriptError> {
        match self.peek() {
            Tok::Newline | Tok::Semi => {
                self.pos += 1;
                Ok(())
            }
            Tok::RBrace | Tok::Eof => Ok(()),
            _ => self.err("expected the end of the statement"),
        }
    }

    fn statement(&mut self) -> Result<Stmt, ScriptError> {
        let line = self.line();
        let kind = match self.peek().clone() {
            Tok::Let => {
                self.pos += 1;
                let name = self.ident("a variable name")?;
                self.expect(&Tok::Assign, "`=`")?;
                let value = self.expr()?;
                self.end_of_statement()?;
                StmtKind::Let(name, value)
            }
            Tok::If => {
                self.pos += 1;
                let mut arms = Vec::new();
                let cond = self.expr()?;
                arms.push((cond, self.block()?));
                let mut otherwise = None;
                loop {
                    let save = self.pos;
                    self.skip_newlines();
                    if self.peek() != &Tok::Else {
                        self.pos = save;
                        break;
                    }
                    self.pos += 1;
                    if self.peek() == &Tok::If {
                        self.pos += 1;
                        let cond = self.expr()?;
                        arms.push((cond, self.block()?));
                    } else {
                        otherwise = Some(self.block()?);
                        break;
                    }
                }
                StmtKind::If(arms, otherwise)
            }
            Tok::While => {
                self.pos += 1;
                let cond = self.expr()?;
                StmtKind::While(cond, self.block()?)
            }
            Tok::For => {
                self.pos += 1;
                let name = self.ident("a loop variable")?;
                self.expect(&Tok::In, "`in`")?;
                let iter = self.expr()?;
                StmtKind::For(name, iter, self.block()?)
            }
            Tok::Return => {
                self.pos += 1;
                let value = if matches!(
                    self.peek(),
                    Tok::Newline | Tok::Semi | Tok::RBrace | Tok::Eof
                ) {
                    None
                } else {
                    Some(self.expr()?)
                };
                self.end_of_statement()?;
                StmtKind::Return(value)
            }
            Tok::Break => {
                self.pos += 1;
                self.end_of_statement()?;
                StmtKind::Break
            }
            Tok::Continue => {
                self.pos += 1;
                self.end_of_statement()?;
                StmtKind::Continue
            }
            _ => {
                let target = self.expr()?;
                let op = match self.peek() {
                    Tok::Assign => Some(None),
                    Tok::PlusAssign => Some(Some(BinOp::Add)),
                    Tok::MinusAssign => Some(Some(BinOp::Sub)),
                    Tok::StarAssign => Some(Some(BinOp::Mul)),
                    Tok::SlashAssign => Some(Some(BinOp::Div)),
                    _ => None,
                };
                let stmt = if let Some(op) = op {
                    self.pos += 1;
                    let value = self.expr()?;
                    let (name, indices) = to_target(target)
                        .ok_or_else(|| ScriptError::new(line, "cannot assign to this"))?;
                    StmtKind::Assign {
                        name,
                        indices,
                        op,
                        value,
                    }
                } else {
                    StmtKind::Expr(target)
                };
                self.end_of_statement()?;
                stmt
            }
        };
        Ok(Stmt { kind, line })
    }

    fn expr(&mut self) -> Result<Expr, ScriptError> {
        self.enter()?;
        let e = self.binary(0)?;
        self.depth -= 1;
        Ok(e)
    }

    fn binary(&mut self, min_prec: u8) -> Result<Expr, ScriptError> {
        let mut left = self.unary()?;
        loop {
            let (op, prec) = match self.peek() {
                Tok::Or => (BinOp::Or, 1),
                Tok::And => (BinOp::And, 2),
                Tok::Eq => (BinOp::Eq, 3),
                Tok::Ne => (BinOp::Ne, 3),
                Tok::Lt => (BinOp::Lt, 4),
                Tok::Le => (BinOp::Le, 4),
                Tok::Gt => (BinOp::Gt, 4),
                Tok::Ge => (BinOp::Ge, 4),
                Tok::Plus => (BinOp::Add, 5),
                Tok::Minus => (BinOp::Sub, 5),
                Tok::Star => (BinOp::Mul, 6),
                Tok::Slash => (BinOp::Div, 6),
                Tok::Percent => (BinOp::Rem, 6),
                _ => break,
            };
            if prec < min_prec {
                break;
            }
            let line = self.line();
            self.pos += 1;
            let right = self.binary(prec + 1)?;
            left = Expr {
                kind: ExprKind::Binary(op, Box::new(left), Box::new(right)),
                line,
            };
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, ScriptError> {
        let line = self.line();
        match self.peek() {
            Tok::Minus => {
                self.pos += 1;
                self.enter()?;
                let e = self.unary()?;
                self.depth -= 1;
                Ok(Expr {
                    kind: ExprKind::Neg(Box::new(e)),
                    line,
                })
            }
            Tok::Not => {
                self.pos += 1;
                self.enter()?;
                let e = self.unary()?;
                self.depth -= 1;
                Ok(Expr {
                    kind: ExprKind::Not(Box::new(e)),
                    line,
                })
            }
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> Result<Expr, ScriptError> {
        let mut e = self.primary()?;
        while self.peek() == &Tok::LBracket {
            let line = self.line();
            self.pos += 1;
            self.skip_newlines();
            let index = self.expr()?;
            self.skip_newlines();
            self.expect(&Tok::RBracket, "`]`")?;
            e = Expr {
                kind: ExprKind::Index(Box::new(e), Box::new(index)),
                line,
            };
        }
        Ok(e)
    }

    fn primary(&mut self) -> Result<Expr, ScriptError> {
        let line = self.line();
        let kind = match self.peek().clone() {
            Tok::Int(i) => {
                self.pos += 1;
                ExprKind::Lit(Variant::Int(i))
            }
            Tok::Float(f) => {
                self.pos += 1;
                ExprKind::Lit(Variant::Float(f))
            }
            Tok::Str(s) => {
                self.pos += 1;
                ExprKind::Lit(Variant::Str(s))
            }
            Tok::True => {
                self.pos += 1;
                ExprKind::Lit(Variant::Bool(true))
            }
            Tok::False => {
                self.pos += 1;
                ExprKind::Lit(Variant::Bool(false))
            }
            Tok::Nil => {
                self.pos += 1;
                ExprKind::Lit(Variant::Nil)
            }
            Tok::Ident(name) => {
                self.pos += 1;
                if self.peek() == &Tok::LParen {
                    self.pos += 1;
                    let args = self.list(&Tok::RParen)?;
                    ExprKind::Call(name, args)
                } else {
                    ExprKind::Var(name)
                }
            }
            Tok::LParen => {
                self.pos += 1;
                self.skip_newlines();
                let e = self.expr()?;
                self.skip_newlines();
                self.expect(&Tok::RParen, "`)`")?;
                return Ok(e);
            }
            Tok::LBracket => {
                self.pos += 1;
                ExprKind::Array(self.list(&Tok::RBracket)?)
            }
            Tok::LBrace => {
                self.pos += 1;
                let mut pairs = Vec::new();
                self.skip_newlines();
                while self.peek() != &Tok::RBrace {
                    let key = self.expr()?;
                    self.skip_newlines();
                    self.expect(&Tok::Colon, "`:`")?;
                    self.skip_newlines();
                    let value = self.expr()?;
                    pairs.push((key, value));
                    self.skip_newlines();
                    if self.peek() == &Tok::Comma {
                        self.pos += 1;
                        self.skip_newlines();
                    } else {
                        break;
                    }
                }
                self.expect(&Tok::RBrace, "`}`")?;
                ExprKind::Dict(pairs)
            }
            _ => return self.err("expected an expression"),
        };
        Ok(Expr { kind, line })
    }

    /// Comma-separated expressions up to `close`, which is consumed.
    fn list(&mut self, close: &Tok) -> Result<Vec<Expr>, ScriptError> {
        let mut items = Vec::new();
        self.skip_newlines();
        while self.peek() != close {
            items.push(self.expr()?);
            self.skip_newlines();
            if self.peek() == &Tok::Comma {
                self.pos += 1;
                self.skip_newlines();
            } else {
                break;
            }
        }
        if self.peek() == close {
            self.pos += 1;
            Ok(items)
        } else {
            self.err("expected `,` or a closing bracket")
        }
    }
}

fn to_target(e: Expr) -> Option<(String, Vec<Expr>)> {
    match e.kind {
        ExprKind::Var(name) => Some((name, Vec::new())),
        ExprKind::Index(base, index) => {
            let (name, mut indices) = to_target(*base)?;
            indices.push(*index);
            Some((name, indices))
        }
        _ => None,
    }
}
