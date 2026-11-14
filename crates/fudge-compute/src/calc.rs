//! Hand-written recursive-descent calculator.

use crate::format::fmt_num;
use crate::{AnswerKind, ComputeAnswer};
use thiserror::Error;

const MAX_DEPTH: usize = 64;

#[derive(Debug, Error, PartialEq)]
pub enum CalcError {
    #[error("unexpected character '{0}'")]
    BadChar(char),
    #[error("syntax error")]
    Syntax,
    #[error("math error")]
    Math,
    #[error("expression too deeply nested")]
    TooDeep,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
    Bang,
    Ident(String),
}

#[derive(Debug, Clone, Copy)]
enum Fun {
    Sqrt,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Ln,
    Log,
    Log2,
    Abs,
    Round,
    Floor,
    Ceil,
    Exp,
}

fn fun_of(s: &str) -> Option<Fun> {
    Some(match s {
        "sqrt" => Fun::Sqrt,
        "sin" => Fun::Sin,
        "cos" => Fun::Cos,
        "tan" => Fun::Tan,
        "asin" => Fun::Asin,
        "acos" => Fun::Acos,
        "atan" => Fun::Atan,
        "ln" => Fun::Ln,
        "log" => Fun::Log,
        "log2" => Fun::Log2,
        "abs" => Fun::Abs,
        "round" => Fun::Round,
        "floor" => Fun::Floor,
        "ceil" => Fun::Ceil,
        "exp" => Fun::Exp,
        _ => return None,
    })
}

fn const_of(s: &str) -> Option<f64> {
    match s {
        "pi" => Some(std::f64::consts::PI),
        "e" => Some(std::f64::consts::E),
        "tau" => Some(std::f64::consts::TAU),
        _ => None,
    }
}

fn is_known_ident(s: &str) -> bool {
    fun_of(s).is_some() || const_of(s).is_some()
}

#[derive(Debug, Clone, Copy)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

#[derive(Debug)]
enum Expr {
    Num(f64),
    Const(f64),
    Neg(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
    Pct(Box<Expr>),
    Fact(Box<Expr>),
    Func(Fun, Box<Expr>),
}

fn lex_number(ch: &[char], start: usize) -> Result<(f64, usize), CalcError> {
    let n = ch.len();
    let mut i = start;
    if ch[i] == '0' && i + 2 < n {
        let radix = match ch[i + 1] {
            'x' | 'X' => 16,
            'b' | 'B' => 2,
            _ => 0,
        };
        if radix != 0 && ch[i + 2].is_digit(radix) {
            i += 2;
            let mut s = String::new();
            while i < n && ch[i].is_digit(radix) {
                s.push(ch[i]);
                i += 1;
            }
            let v = u64::from_str_radix(&s, radix).map_err(|_| CalcError::Syntax)?;
            return Ok((v as f64, i));
        }
    }
    let mut s = String::new();
    let mut int_digits = 0;
    while i < n && ch[i].is_ascii_digit() {
        s.push(ch[i]);
        i += 1;
        int_digits += 1;
    }
    if (1..=3).contains(&int_digits) && ch[start] != '0' {
        while i + 3 < n
            && ch[i] == ','
            && ch[i + 1..=i + 3].iter().all(|c| c.is_ascii_digit())
            && !(i + 4 < n && ch[i + 4].is_ascii_digit())
        {
            s.extend(&ch[i + 1..=i + 3]);
            i += 4;
        }
    }
    if i < n && ch[i] == '.' {
        s.push('.');
        i += 1;
        while i < n && ch[i].is_ascii_digit() {
            s.push(ch[i]);
            i += 1;
        }
    }
    if s == "." {
        return Err(CalcError::Syntax);
    }
    if i < n && (ch[i] == 'e' || ch[i] == 'E') {
        let mut j = i + 1;
        let mut exp = String::from("e");
        if j < n && (ch[j] == '+' || ch[j] == '-') {
            exp.push(ch[j]);
            j += 1;
        }
        if j < n && ch[j].is_ascii_digit() {
            while j < n && ch[j].is_ascii_digit() {
                exp.push(ch[j]);
                j += 1;
            }
            s.push_str(&exp);
            i = j;
        }
    }
    let v: f64 = s.parse().map_err(|_| CalcError::Syntax)?;
    Ok((v, i))
}

fn lex(input: &str) -> Result<Vec<Tok>, CalcError> {
    let ch: Vec<char> = input.chars().collect();
    let n = ch.len();
    let mut i = 0;
    let mut out: Vec<Tok> = Vec::new();
    while i < n {
        let c = ch[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '+' => {
                out.push(Tok::Plus);
                i += 1;
            }
            '-' | '\u{2212}' => {
                out.push(Tok::Minus);
                i += 1;
            }
            '*' | '\u{d7}' | '\u{b7}' => {
                out.push(Tok::Star);
                i += 1;
            }
            '/' | '\u{f7}' => {
                out.push(Tok::Slash);
                i += 1;
            }
            '%' => {
                out.push(Tok::Percent);
                i += 1;
            }
            '^' => {
                out.push(Tok::Caret);
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            '!' => {
                out.push(Tok::Bang);
                i += 1;
            }
            '0'..='9' | '.' => {
                let (v, next) = lex_number(&ch, i)?;
                out.push(Tok::Num(v));
                i = next;
            }
            c if c.is_alphabetic() => {
                let mut s = String::new();
                while i < n && ch[i].is_alphabetic() {
                    s.extend(ch[i].to_lowercase());
                    i += 1;
                }
                if s == "log" && i < n && ch[i] == '2' {
                    s.push('2');
                    i += 1;
                }
                let prev_is_value = matches!(
                    out.last(),
                    Some(Tok::Num(_)) | Some(Tok::RParen) | Some(Tok::Bang)
                );
                if s == "x" && prev_is_value {
                    out.push(Tok::Star);
                } else {
                    out.push(Tok::Ident(s));
                }
            }
            other => return Err(CalcError::BadChar(other)),
        }
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn peek_at(&self, off: usize) -> Option<&Tok> {
        self.toks.get(self.pos + off)
    }

    fn enter(&mut self) -> Result<(), CalcError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            Err(CalcError::TooDeep)
        } else {
            Ok(())
        }
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn parse_expr(&mut self) -> Result<Expr, CalcError> {
        let mut l = self.parse_term()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Plus) => Op::Add,
                Some(Tok::Minus) => Op::Sub,
                _ => break,
            };
            self.pos += 1;
            let r = self.parse_term()?;
            l = Expr::Bin(op, Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn parse_term(&mut self) -> Result<Expr, CalcError> {
        let mut l = self.parse_unary()?;
        loop {
            let tok = self.peek().cloned();
            let op = match tok {
                Some(Tok::Star) => Op::Mul,
                Some(Tok::Slash) => Op::Div,
                Some(Tok::Percent) => Op::Mod,
                Some(Tok::Ident(ref s)) if s == "of" => {
                    if !matches!(l, Expr::Pct(_)) {
                        return Err(CalcError::Syntax);
                    }
                    Op::Mul
                }
                // implicit multiplication: 2(3+4), 2pi, 2 sqrt(4)
                Some(Tok::LParen) => {
                    let r = self.parse_unary()?;
                    l = Expr::Bin(Op::Mul, Box::new(l), Box::new(r));
                    continue;
                }
                Some(Tok::Ident(ref s)) if is_known_ident(s) => {
                    let r = self.parse_unary()?;
                    l = Expr::Bin(Op::Mul, Box::new(l), Box::new(r));
                    continue;
                }
                _ => break,
            };
            self.pos += 1;
            let r = self.parse_unary()?;
            l = Expr::Bin(op, Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn parse_unary(&mut self) -> Result<Expr, CalcError> {
        self.enter()?;
        let r = match self.peek() {
            Some(Tok::Minus) => {
                self.pos += 1;
                self.parse_unary().map(|e| Expr::Neg(Box::new(e)))
            }
            Some(Tok::Plus) => {
                self.pos += 1;
                self.parse_unary()
            }
            _ => self.parse_power(),
        };
        self.leave();
        r
    }

    fn parse_power(&mut self) -> Result<Expr, CalcError> {
        let base = self.parse_postfix()?;
        if matches!(self.peek(), Some(Tok::Caret)) {
            self.pos += 1;
            let exp = self.parse_unary()?;
            return Ok(Expr::Bin(Op::Pow, Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn percent_is_postfix(&self) -> bool {
        match self.peek_at(1) {
            None => true,
            Some(Tok::Num(_)) | Some(Tok::LParen) => false,
            Some(Tok::Ident(s)) => s == "of",
            Some(_) => true,
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, CalcError> {
        let mut e = self.parse_primary()?;
        loop {
            match self.peek() {
                Some(Tok::Bang) => {
                    self.pos += 1;
                    e = Expr::Fact(Box::new(e));
                }
                Some(Tok::Percent) if self.percent_is_postfix() => {
                    self.pos += 1;
                    e = Expr::Pct(Box::new(e));
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn expect_rparen(&mut self) -> Result<(), CalcError> {
        if matches!(self.peek(), Some(Tok::RParen)) {
            self.pos += 1;
            Ok(())
        } else {
            Err(CalcError::Syntax)
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, CalcError> {
        let tok = self.peek().cloned().ok_or(CalcError::Syntax)?;
        match tok {
            Tok::Num(v) => {
                self.pos += 1;
                Ok(Expr::Num(v))
            }
            Tok::LParen => {
                self.pos += 1;
                self.enter()?;
                let e = self.parse_expr();
                self.leave();
                let e = e?;
                self.expect_rparen()?;
                Ok(e)
            }
            Tok::Ident(name) => {
                if let Some(f) = fun_of(&name) {
                    self.pos += 1;
                    let arg = if matches!(self.peek(), Some(Tok::LParen)) {
                        self.pos += 1;
                        self.enter()?;
                        let e = self.parse_expr();
                        self.leave();
                        let e = e?;
                        self.expect_rparen()?;
                        e
                    } else {
                        self.parse_unary()?
                    };
                    Ok(Expr::Func(f, Box::new(arg)))
                } else if let Some(c) = const_of(&name) {
                    self.pos += 1;
                    Ok(Expr::Const(c))
                } else {
                    Err(CalcError::Syntax)
                }
            }
            _ => Err(CalcError::Syntax),
        }
    }
}

fn eval(e: &Expr) -> Result<f64, CalcError> {
    let v = match e {
        Expr::Num(v) | Expr::Const(v) => *v,
        Expr::Neg(x) => -eval(x)?,
        Expr::Pct(x) => eval(x)? / 100.0,
        Expr::Fact(x) => {
            let v = eval(x)?;
            if v < 0.0 || v.fract() != 0.0 || v > 170.0 {
                return Err(CalcError::Math);
            }
            let mut f = 1.0f64;
            let mut k = 2.0;
            while k <= v {
                f *= k;
                k += 1.0;
            }
            f
        }
        Expr::Func(f, x) => {
            let v = eval(x)?;
            match f {
                Fun::Sqrt => v.sqrt(),
                Fun::Sin => v.sin(),
                Fun::Cos => v.cos(),
                Fun::Tan => v.tan(),
                Fun::Asin => v.asin(),
                Fun::Acos => v.acos(),
                Fun::Atan => v.atan(),
                Fun::Ln => v.ln(),
                Fun::Log => v.log10(),
                Fun::Log2 => v.log2(),
                Fun::Abs => v.abs(),
                Fun::Round => v.round(),
                Fun::Floor => v.floor(),
                Fun::Ceil => v.ceil(),
                Fun::Exp => v.exp(),
            }
        }
        Expr::Bin(op, l, r) => {
            // "80 + 15%" means 80 + 15% of 80
            if let (Op::Add | Op::Sub, Expr::Pct(p)) = (op, &**r) {
                let a = eval(l)?;
                let pct = a * eval(p)? / 100.0;
                return finite(if matches!(op, Op::Add) { a + pct } else { a - pct });
            }
            let a = eval(l)?;
            let b = eval(r)?;
            match op {
                Op::Add => a + b,
                Op::Sub => a - b,
                Op::Mul => a * b,
                Op::Div => {
                    if b == 0.0 {
                        return Err(CalcError::Math);
                    }
                    a / b
                }
                Op::Mod => {
                    if b == 0.0 {
                        return Err(CalcError::Math);
                    }
                    a % b
                }
                Op::Pow => a.powf(b),
            }
        }
    };
    finite(v)
}

fn finite(v: f64) -> Result<f64, CalcError> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(CalcError::Math)
    }
}

fn is_trivial(e: &Expr) -> bool {
    match e {
        Expr::Num(_) | Expr::Const(_) => true,
        Expr::Neg(x) => is_trivial(x),
        _ => false,
    }
}

/// Evaluate an expression. Bare numbers/constants are rejected (not a calculation).
pub fn calculate(input: &str) -> Result<f64, CalcError> {
    let toks = lex(input)?;
    if toks.is_empty() {
        return Err(CalcError::Syntax);
    }
    let mut p = Parser {
        toks,
        pos: 0,
        depth: 0,
    };
    let e = p.parse_expr()?;
    if p.pos != p.toks.len() {
        return Err(CalcError::Syntax);
    }
    if is_trivial(&e) {
        return Err(CalcError::Syntax);
    }
    eval(&e)
}

pub fn evaluate(input: &str) -> Option<ComputeAnswer> {
    let v = calculate(input).ok()?;
    let s = fmt_num(v);
    Some(ComputeAnswer {
        kind: AnswerKind::Math,
        title: format!("= {s}"),
        subtitle: input.trim().to_string(),
        copy_text: s,
        score: 90,
        needs_network: false,
        color_hex: None,
    })
}
