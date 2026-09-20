//! `.vy` text -> [`Patch`]. Hand-written: the grammar is twelve line-oriented
//! rules, and a parser this small can afford what a combinator can't — errors
//! that say what to write instead.
//!
//! Lexing is on demand, because a patch mixes two kinds of word: expression
//! tokens (`ss*(1-t)`) and raw text the grammar expects at known positions
//! (`transicao/*.png`, `/hands`, `HDMI-A-1`). The parser asks the [`Cursor`]
//! for whichever the position calls for, so `*` is a multiply in one place and
//! a glob in another without either needing quotes.

use super::vocabulary::{self, Family};
use super::{
    Body, Comp, Diagnostic, Effect, Expr, Item, Mod, Node, Out, OutKind, Patch, Scalar, Scene,
    Source, Term, Transition, What, did_you_mean,
};
use crate::input::Value;
use crate::sugar::Blend;

/// Words that mean something at the start of a node, so can't name one.
const RESERVED: &[&str] = &["osc", "ramp", "rust", "add", "out", "smooth"];

/// Parses a whole patch, reporting every broken statement, not just the first —
/// whoever wrote it (a person, a model) fixes them in one pass.
pub fn parse(text: &str) -> Result<Patch, Vec<Diagnostic>> {
    let mut patch = Patch::default();
    let mut errors = Vec::new();
    for (line, statement) in statements(text) {
        let mut cursor = Cursor {
            text: &statement,
            at: 0,
            line,
        };
        if let Err(e) = cursor.statement(&mut patch) {
            errors.push(e);
        }
    }
    if errors.is_empty() {
        Ok(patch)
    } else {
        Err(errors)
    }
}

/// Splits the text into statements: comments dropped, blank lines skipped, and
/// a line starting with `|` or `+` folded into the statement above it. Each
/// statement keeps the number of its first line.
fn statements(text: &str) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some((_, above)) if line.starts_with('|') || line.starts_with('+') => {
                above.push(' ');
                above.push_str(line);
            }
            _ => out.push((index + 1, line.to_owned())),
        }
    }
    out
}

/// Everything before the first `#` that isn't inside a string.
fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f32, Unit),
    Ident(String),
    Str(String),
    Sym(&'static str),
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    /// Scene units, or a plain number. `s` and `hz` read as this too: they
    /// say what a number *is*, they don't scale it.
    Plain,
    Px,
}

impl Tok {
    fn show(&self) -> String {
        match self {
            Tok::Num(n, _) => format!("`{n}`"),
            Tok::Ident(s) => format!("`{s}`"),
            Tok::Str(s) => format!("\"{s}\""),
            Tok::Sym(s) => format!("`{s}`"),
            Tok::End => "the end of the line".into(),
        }
    }
}

struct Cursor<'a> {
    text: &'a str,
    at: usize,
    line: usize,
}

type Parsed<T> = Result<T, Diagnostic>;

impl<'a> Cursor<'a> {
    fn error(&self, message: impl Into<String>) -> Diagnostic {
        Diagnostic::error(self.line, message)
    }

    /// What is left of the statement — borrowed from the text, not the cursor,
    /// so the cursor can move while a slice of it is held.
    fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    fn skip_space(&mut self) {
        self.at += self.rest().len() - self.rest().trim_start().len();
    }

    /// The next whitespace-delimited word, raw — for the positions where the
    /// grammar expects text: a path, a glob, an address, a connector.
    fn word(&mut self) -> Option<String> {
        self.skip_space();
        let rest = self.rest();
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        if end == 0 {
            return None;
        }
        self.at += end;
        Some(rest[..end].to_owned())
    }

    /// The next expression token.
    fn token(&mut self) -> Parsed<Tok> {
        self.skip_space();
        let rest = self.rest();
        let mut chars = rest.chars();
        let Some(first) = chars.next() else {
            return Ok(Tok::End);
        };
        let second = chars.next();

        if first.is_ascii_digit() || (first == '.' && second.is_some_and(|c| c.is_ascii_digit())) {
            let digits = rest
                .find(|c: char| !c.is_ascii_digit() && c != '.')
                .unwrap_or(rest.len());
            let unit_len = rest[digits..]
                .find(|c: char| !c.is_ascii_alphabetic())
                .unwrap_or(rest.len() - digits);
            let (number, unit) = (&rest[..digits], &rest[digits..digits + unit_len]);
            let value = number
                .parse()
                .map_err(|_| self.error(format!("`{number}` is not a number")))?;
            let unit = match unit {
                "" | "s" | "hz" => Unit::Plain,
                "px" => Unit::Px,
                other => {
                    return Err(self
                        .error(format!("unknown unit `{other}` on `{number}{other}`"))
                        .help(
                            "units are: s  hz  px  — no unit = scene units (shorter edge is 1)",
                        ));
                }
            };
            self.at += digits + unit_len;
            return Ok(Tok::Num(value, unit));
        }
        if first.is_alphabetic() || first == '_' {
            let end = rest
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(rest.len());
            self.at += end;
            return Ok(Tok::Ident(rest[..end].to_owned()));
        }
        if first == '"' {
            let Some(end) = rest[1..].find('"') else {
                return Err(self
                    .error("this string never closes")
                    .help("add the closing \""));
            };
            self.at += end + 2;
            return Ok(Tok::Str(rest[1..=end].to_owned()));
        }
        if first == '-' && second == Some('>') {
            self.at += 2;
            return Ok(Tok::Sym("->"));
        }
        let sym = match first {
            '+' => "+",
            '-' => "-",
            '*' => "*",
            '@' => "@",
            '|' => "|",
            '=' => "=",
            ':' => ":",
            '&' => "&",
            '(' => "(",
            ')' => ")",
            other => return Err(self.error(format!("unexpected `{other}`"))),
        };
        self.at += 1;
        Ok(Tok::Sym(sym))
    }

    fn peek(&mut self) -> Parsed<Tok> {
        let at = self.at;
        let tok = self.token();
        self.at = at;
        tok
    }

    /// Consumes the next token if it is the symbol `sym`.
    fn eat(&mut self, sym: &str) -> Parsed<bool> {
        let hit = matches!(self.peek()?, Tok::Sym(s) if s == sym);
        if hit {
            self.token()?;
        }
        Ok(hit)
    }

    /// Consumes the next token if it is the bare word `word`.
    fn eat_word(&mut self, word: &str) -> Parsed<bool> {
        let hit = matches!(self.peek()?, Tok::Ident(w) if w == word);
        if hit {
            self.token()?;
        }
        Ok(hit)
    }

    fn ident(&mut self, what: &str) -> Parsed<String> {
        match self.token()? {
            Tok::Ident(name) => Ok(name),
            other => Err(self.error(format!("expected {what}, found {}", other.show()))),
        }
    }

    fn end(&mut self) -> Parsed<()> {
        match self.token()? {
            Tok::End => Ok(()),
            other => Err(self.error(format!("unexpected {} here", other.show()))),
        }
    }

    // -----------------------------------------------------------------------
    // Statements
    // -----------------------------------------------------------------------

    fn statement(&mut self, patch: &mut Patch) -> Parsed<()> {
        let start = self.at;
        let first = self.token()?;
        let second = self.peek()?;
        match (first, second) {
            (Tok::Ident(name), Tok::Sym("=")) => {
                self.token()?;
                if RESERVED.contains(&name.as_str()) || vocabulary::source(&name).is_some() {
                    return Err(self
                        .error(format!(
                            "`{name}` is a word of the language; a node can't take it as its name"
                        ))
                        .help(format!("rename it: `my_{name} = ...`")));
                }
                let body = self.body()?;
                self.end()?;
                patch.nodes.push(Node {
                    name,
                    line: self.line,
                    body,
                });
            }
            (Tok::Ident(name), Tok::Sym(":")) => {
                self.token()?;
                let comp = self.comp()?;
                self.end()?;
                patch.scenes.push(Scene {
                    name,
                    line: self.line,
                    comp,
                });
            }
            (Tok::Ident(from), Tok::Sym("->")) => {
                let transition = self.transition(Some(from))?;
                patch.transitions.push(transition);
            }
            (Tok::Sym("*"), Tok::Sym("->")) => {
                let transition = self.transition(None)?;
                patch.transitions.push(transition);
            }
            (Tok::Ident(word), _) if word == "out" => {
                if patch.out.is_some() {
                    return Err(self.error("a patch has one output; this is a second `out`"));
                }
                patch.out = Some(self.out()?);
            }
            (first, _) => {
                self.at = start;
                return Err(self
                    .error(format!("a line can't start with {} like this", first.show()))
                    .help(
                        "a line is one of:  name = node  ·  name : scene  ·  a -> b cond cut  ·  out ...\n\
                         (a line starting with | or + continues the node above it)",
                    ));
            }
        }
        Ok(())
    }

    fn body(&mut self) -> Parsed<Body> {
        match self.peek()? {
            Tok::Ident(word) if word == "rust" => {
                self.token()?;
                Ok(Body::Leaf(self.ident("the leaf's function name")?))
            }
            Tok::Ident(word) if word == "osc" => {
                // `osc /address` is an input; `osc <hz> <amp>` an oscillator.
                let at = self.at;
                self.token()?;
                self.skip_space();
                if !self.rest().starts_with('/') {
                    self.at = at;
                    return Ok(Body::Scalar(Scalar::Value(self.atom()?)));
                }
                let address = self.word().unwrap_or_default();
                let debounce = if self.eat_word("debounce")? {
                    Some(self.atom()?)
                } else {
                    None
                };
                Ok(Body::Scalar(Scalar::Input { address, debounce }))
            }
            Tok::Ident(word) if word == "ramp" => {
                self.token()?;
                let gate = self.ident("the gate that drives the ramp")?;
                let (mut up, mut down) = (Expr::Num(1.0), Expr::Num(1.0));
                loop {
                    if self.eat_word("up")? {
                        up = self.atom()?;
                    } else if self.eat_word("down")? {
                        down = self.atom()?;
                    } else {
                        break;
                    }
                }
                Ok(Body::Scalar(Scalar::Ramp { gate, up, down }))
            }
            Tok::Num(..) | Tok::Sym("(") | Tok::Sym("-") => {
                Ok(Body::Scalar(Scalar::Value(self.atom()?)))
            }
            Tok::Ident(word) if word == "smooth" => Ok(Body::Scalar(Scalar::Value(self.atom()?))),
            _ => Ok(Body::Signal(self.comp()?)),
        }
    }

    // -----------------------------------------------------------------------
    // Compositions
    // -----------------------------------------------------------------------

    fn comp(&mut self) -> Parsed<Comp> {
        let mut items = vec![self.item()?];
        while self.eat("+")? {
            items.push(self.item()?);
        }
        let effect = if self.eat("|")? {
            Some(self.effect()?)
        } else {
            None
        };
        Ok(Comp { items, effect })
    }

    fn item(&mut self) -> Parsed<Item> {
        let name = match self.token()? {
            Tok::Ident(name) => name,
            other => {
                return Err(self
                    .error(format!(
                        "expected a source or a node's name, found {}",
                        other.show()
                    ))
                    .help(format!("sources: {}", source_names())));
            }
        };
        let what = match vocabulary::source(&name) {
            Some(word) => What::Source(self.source(word)?),
            None => What::Wire(name),
        };
        let mut item = Item {
            what,
            alpha: None,
            at: None,
            blend: Blend::Over,
        };
        loop {
            if self.eat("*")? {
                item.alpha = Some(self.atom()?);
            } else if self.eat("@")? {
                item.at = Some(self.atom()?);
            } else if self.eat_word("add")? {
                item.blend = Blend::Add;
            } else {
                break;
            }
        }
        Ok(item)
    }

    fn source(&mut self, word: &'static vocabulary::SourceWord) -> Parsed<Source> {
        let mut source = Source {
            kind: word.kind,
            args: Vec::new(),
            text: None,
            mods: Vec::new(),
        };
        for arg in word.args {
            source.args.push(self.atom().map_err(|e| {
                self.error(format!(
                    "`{}` takes {}: {}",
                    word.name,
                    signature(word),
                    e.message
                ))
                .help(format!("missing or bad `{arg}`"))
            })?);
        }
        if let Some(what) = word.text {
            source.text = Some(match word.family {
                Family::Text => match self.token()? {
                    Tok::Str(text) => text,
                    other => {
                        return Err(self
                            .error(format!(
                                "`text` takes a quoted string, found {}",
                                other.show()
                            ))
                            .help("text \"NORTE\" size .08"));
                    }
                },
                _ => self.word().ok_or_else(|| {
                    self.error(format!("`{}` takes a {what}", word.name))
                        .help(format!("{} {}", word.name, example_text(word.name)))
                })?,
            });
        }
        // Modifiers, in any order, for as long as the next word is one.
        while let Tok::Ident(name) = self.peek()? {
            if name == "add" {
                break;
            }
            let Some(modifier) = vocabulary::modifier(&name) else {
                let known = vocabulary::MODS.iter().map(|m| m.name);
                let mut e = self.error(format!("`{name}` is not a modifier"));
                e = match did_you_mean(&name, known) {
                    Some(close) => e.help(format!("did you mean `{close}`?")),
                    None => e.help(format!(
                        "modifiers for `{}`: {}\n(to stack another node, put `+` before it)",
                        word.name,
                        mods_for(word.family)
                    )),
                };
                return Err(e);
            };
            if !modifier.families.contains(&word.family) {
                return Err(self
                    .error(format!(
                        "`{}` doesn't apply to `{}`",
                        modifier.name, word.name
                    ))
                    .help(format!(
                        "modifiers for `{}`: {}",
                        word.name,
                        mods_for(word.family)
                    )));
            }
            self.token()?;
            let mut args = Vec::new();
            for arg in modifier.args {
                args.push(self.atom().map_err(|e| {
                    self.error(format!(
                        "`{}` takes {}: {}",
                        modifier.name,
                        modifier.args.join(" "),
                        e.message
                    ))
                    .help(format!("missing or bad `{arg}`"))
                })?);
            }
            source.mods.push(Mod {
                kind: modifier.kind,
                args,
            });
        }
        Ok(source)
    }

    fn effect(&mut self) -> Parsed<Effect> {
        let name = self.ident("an effect after `|`")?;
        let Some(word) = vocabulary::effect(&name) else {
            let known = vocabulary::EFFECTS.iter().map(|e| e.name);
            let e = self.error(format!("`{name}` is not an effect"));
            return Err(match did_you_mean(&name, known) {
                Some(close) => e.help(format!("did you mean `{close}`?")),
                None => e.help("effects: feedback decay <x> angle <x> scale <x>"),
            });
        };
        let mut params = Vec::new();
        while let Tok::Ident(param) = self.peek()? {
            if !word.params.iter().any(|(p, _)| *p == param) {
                let known: Vec<&str> = word.params.iter().map(|(p, _)| *p).collect();
                return Err(self
                    .error(format!("`{}` has no `{param}`", word.name))
                    .help(format!("its parameters: {}", known.join(" "))));
            }
            self.token()?;
            params.push((param, self.atom()?));
        }
        Ok(Effect {
            kind: word.kind,
            params,
        })
    }

    // -----------------------------------------------------------------------
    // Numbers
    // -----------------------------------------------------------------------

    /// Anything a number socket takes without parentheses: a number, a node's
    /// name, `-x`, `smooth(x)`, `osc hz amp`, or a parenthesized expression.
    /// Binary operators live only inside parentheses — outside them `+` and `*`
    /// belong to the composition.
    fn atom(&mut self) -> Parsed<Expr> {
        match self.token()? {
            Tok::Num(n, Unit::Plain) => Ok(Expr::Num(n)),
            Tok::Num(n, Unit::Px) => Ok(Expr::Px(n)),
            Tok::Sym("-") => Ok(Expr::Neg(Box::new(self.atom()?))),
            Tok::Sym("(") => {
                let inner = self.expr()?;
                if !self.eat(")")? {
                    return Err(self
                        .error("this `(` never closes")
                        .help("add the closing )"));
                }
                Ok(inner)
            }
            Tok::Ident(word) if word == "smooth" => {
                if !self.eat("(")? {
                    return Err(self.error("`smooth` is a function").help("smooth(t)"));
                }
                let inner = self.expr()?;
                if !self.eat(")")? {
                    return Err(self.error("this `smooth(` never closes").help("smooth(t)"));
                }
                Ok(Expr::Smooth(Box::new(inner)))
            }
            Tok::Ident(word) if word == "osc" => Ok(Expr::Osc {
                hz: Box::new(self.atom()?),
                amp: Box::new(self.atom()?),
            }),
            Tok::Ident(name) => Ok(Expr::Wire(name)),
            other => Err(self.error(format!("expected a number, found {}", other.show()))),
        }
    }

    fn expr(&mut self) -> Parsed<Expr> {
        let mut left = self.product()?;
        loop {
            if self.eat("+")? {
                left = Expr::Add(Box::new(left), Box::new(self.product()?));
            } else if self.eat("-")? {
                left = Expr::Sub(Box::new(left), Box::new(self.product()?));
            } else {
                return Ok(left);
            }
        }
    }

    fn product(&mut self) -> Parsed<Expr> {
        let mut left = self.atom()?;
        while self.eat("*")? {
            left = Expr::Mul(Box::new(left), Box::new(self.atom()?));
        }
        Ok(left)
    }

    // -----------------------------------------------------------------------
    // Transitions and the output
    // -----------------------------------------------------------------------

    /// Everything after `from`: `-> to  cond  cut|fade Ns  [sound]`.
    fn transition(&mut self, from: Option<String>) -> Parsed<Transition> {
        const SHAPE: &str =
            "a -> b  <cond>  cut        (cond:  node rise|off|done  ·  node = N  ·  c & c)";
        self.token()?; // ->
        let to = self.ident("the scene to go to")?;
        let mut cond = Vec::new();
        loop {
            let node = match self.token()? {
                Tok::Ident(word) if word == "cut" || word == "fade" => {
                    return Err(self
                        .error("a transition needs a condition before `cut`/`fade`")
                        .help(SHAPE));
                }
                Tok::Ident(node) => node,
                other => {
                    return Err(self
                        .error(format!("expected a condition, found {}", other.show()))
                        .help(SHAPE));
                }
            };
            cond.push(match self.token()? {
                Tok::Ident(w) if w == "rise" => Term::Rise(node),
                Tok::Ident(w) if w == "off" => Term::Off(node),
                Tok::Ident(w) if w == "done" => Term::Done(node),
                Tok::Sym("=") => match self.token()? {
                    Tok::Num(n, _) => Term::Is(node, Value::Num(n)),
                    Tok::Ident(symbol) => Term::Is(node, Value::Sym(symbol)),
                    other => {
                        return Err(self.error(format!(
                            "`{node} =` compares with a number or a word, found {}",
                            other.show()
                        )));
                    }
                },
                other => {
                    return Err(self
                        .error(format!("`{node}` what? found {}", other.show()))
                        .help(SHAPE));
                }
            });
            if !self.eat("&")? {
                break;
            }
        }
        let fade = match self.token()? {
            Tok::Ident(w) if w == "cut" => None,
            Tok::Ident(w) if w == "fade" => Some(self.atom().map_err(|e| {
                self.error(format!("`fade` takes a duration: {}", e.message))
                    .help("fade 1.2s")
            })?),
            other => {
                return Err(self
                    .error(format!(
                        "a transition ends in `cut` or `fade Ns`, found {}",
                        other.show()
                    ))
                    .help(SHAPE));
            }
        };
        let sound = self.word();
        self.end()?;
        Ok(Transition {
            from,
            to,
            cond,
            fade,
            sound,
            line: self.line,
        })
    }

    /// Everything after `out`: `<window|kms> [args]  [keystone <file>]  [remote <port>]`.
    fn out(&mut self) -> Parsed<Out> {
        const SHAPE: &str = "out window 960x600  keystone config/keystone.json  remote 9001\n\
                             out kms HDMI-A-1 1920x1200 60  picture 1200x1200  keystone config/keystone.json  remote 9001";
        let kind = match self.word().as_deref() {
            Some("window") => OutKind::Window,
            Some("kms") => OutKind::Kms,
            Some(other) => {
                return Err(self
                    .error(format!("`{other}` is not an output"))
                    .help(SHAPE));
            }
            None => return Err(self.error("`out` needs an output").help(SHAPE)),
        };
        let mut out = Out {
            kind,
            connector: None,
            size: None,
            hz: None,
            keystone: None,
            picture: None,
            remote: None,
            line: self.line,
        };
        while let Some(word) = self.word() {
            match word.as_str() {
                "picture" => {
                    let size = self.word().unwrap_or_default();
                    out.picture = Some(parse_size(&size).ok_or_else(|| {
                        self.error(format!("`picture` takes a size, found `{size}`"))
                            .help("picture 1200x1200   (the face's own size; the keystone lands it in the output)")
                    })?);
                }
                "keystone" => {
                    out.keystone = Some(
                        self.word()
                            .ok_or_else(|| self.error("`keystone` takes a file").help(SHAPE))?,
                    );
                }
                "remote" => {
                    let port = self.word().unwrap_or_default();
                    out.remote = Some(port.parse().map_err(|_| {
                        self.error(format!("`remote` takes a UDP port, found `{port}`"))
                            .help(SHAPE)
                    })?);
                }
                _ => {
                    if let Some(size) = parse_size(&word) {
                        out.size = Some(size);
                    } else if let Ok(hz) = word.parse() {
                        out.hz = Some(hz);
                    } else {
                        out.connector = Some(word);
                    }
                }
            }
        }
        Ok(out)
    }
}

/// `1920x1200`
fn parse_size(word: &str) -> Option<[u32; 2]> {
    let (w, h) = word.split_once('x')?;
    Some([w.parse().ok()?, h.parse().ok()?])
}

fn signature(word: &vocabulary::SourceWord) -> String {
    let args: Vec<&str> = word.args.iter().copied().chain(word.text).collect();
    args.join(" ")
}

fn source_names() -> String {
    let names: Vec<&str> = vocabulary::SOURCES.iter().map(|s| s.name).collect();
    names.join(" ")
}

fn mods_for(family: Family) -> String {
    let names: Vec<&str> = vocabulary::MODS
        .iter()
        .filter(|m| m.families.contains(&family))
        .map(|m| m.name)
        .collect();
    names.join(" ")
}

fn example_text(source: &str) -> &'static str {
    match source {
        "frames" => "transicao/*.png",
        _ => "agua.mp4",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{ModKind, SourceKind};

    fn ok(text: &str) -> Patch {
        parse(text).unwrap_or_else(|errors| {
            let all: Vec<String> = errors.iter().map(ToString::to_string).collect();
            panic!("should parse:\n{}", all.join("\n"));
        })
    }

    fn first_error(text: &str) -> Diagnostic {
        parse(text).expect_err("should not parse").remove(0)
    }

    #[test]
    fn a_source_takes_its_args_then_modifiers_in_any_order() {
        let patch = ok("aura = circle .06 soft 1 hue 160 drift 6 wave .25 .05hz .08hz");
        let Body::Signal(comp) = &patch.nodes[0].body else {
            panic!("a signal");
        };
        let What::Source(source) = &comp.items[0].what else {
            panic!("a source");
        };
        assert_eq!(source.kind, SourceKind::Circle);
        assert_eq!(source.args, vec![Expr::Num(0.06)]);
        assert_eq!(source.mods.len(), 4);
        assert_eq!(source.modifier(ModKind::Wave).unwrap().args.len(), 3);
    }

    #[test]
    fn a_composition_reads_stack_alpha_time_and_blend() {
        let patch = ok("touch = ss*(1-t) + trans@smooth(t) + ring add");
        let Body::Signal(comp) = &patch.nodes[0].body else {
            panic!("a signal");
        };
        assert_eq!(comp.items.len(), 3);
        assert_eq!(
            comp.items[0].alpha,
            Some(Expr::Sub(
                Box::new(Expr::Num(1.0)),
                Box::new(Expr::Wire("t".into()))
            ))
        );
        assert_eq!(
            comp.items[1].at,
            Some(Expr::Smooth(Box::new(Expr::Wire("t".into()))))
        );
        assert_eq!(comp.items[2].blend, Blend::Add);
    }

    #[test]
    fn globs_and_addresses_are_taken_raw() {
        let patch = ok("f = frames transicao/*.png loop\nhands = osc /hands debounce .08");
        let Body::Signal(comp) = &patch.nodes[0].body else {
            panic!("a signal");
        };
        let What::Source(source) = &comp.items[0].what else {
            panic!("a source");
        };
        assert_eq!(source.text.as_deref(), Some("transicao/*.png"));
        assert!(matches!(
            &patch.nodes[1].body,
            Body::Scalar(Scalar::Input { address, debounce: Some(_) }) if address == "/hands"
        ));
    }

    #[test]
    fn osc_is_an_input_with_an_address_and_an_oscillator_without() {
        let patch = ok("lfo = osc .07hz .03\nfolhas = circle .1 y osc .07hz .03");
        assert!(matches!(
            &patch.nodes[0].body,
            Body::Scalar(Scalar::Value(Expr::Osc { .. }))
        ));
        let Body::Signal(comp) = &patch.nodes[1].body else {
            panic!("a signal");
        };
        let What::Source(source) = &comp.items[0].what else {
            panic!("a source");
        };
        assert!(matches!(
            source.modifier(ModKind::Y).unwrap().args[0],
            Expr::Osc { .. }
        ));
    }

    #[test]
    fn a_continuation_line_folds_into_the_node_above() {
        let patch = ok(
            "aura = circle .06 soft 1   # the glow\n     | feedback decay .3 angle .2 scale .9\n",
        );
        assert_eq!(patch.nodes.len(), 1);
        let Body::Signal(comp) = &patch.nodes[0].body else {
            panic!("a signal");
        };
        assert_eq!(comp.effect.as_ref().unwrap().params.len(), 3);
    }

    #[test]
    fn scenes_and_transitions() {
        let patch = ok("idle : ss\n\
             idle -> touch  hands rise  cut\n\
             touch -> idle  t = 0 & hands off  cut\n\
             touch -> play  t = 1  cut  confirma.wav\n\
             play -> idle  play done  fade 1.2s\n\
             * -> grid  mode = grid  cut");
        assert_eq!(patch.scenes[0].name, "idle");
        let t = &patch.transitions;
        assert_eq!(t[0].cond, vec![Term::Rise("hands".into())]);
        assert_eq!(
            t[1].cond,
            vec![
                Term::Is("t".into(), Value::Num(0.0)),
                Term::Off("hands".into())
            ]
        );
        assert_eq!(t[2].sound.as_deref(), Some("confirma.wav"));
        assert_eq!(t[3].fade, Some(Expr::Num(1.2)));
        assert_eq!(t[4].from, None);
        assert_eq!(
            t[4].cond,
            vec![Term::Is("mode".into(), Value::Sym("grid".into()))]
        );
    }

    #[test]
    fn the_out_line() {
        let patch = ok(
            "out kms HDMI-A-1 1920x1200 60  picture 1200x1200  keystone config/keystone.json  remote 9001",
        );
        let out = patch.out.unwrap();
        assert_eq!(out.kind, OutKind::Kms);
        assert_eq!(out.connector.as_deref(), Some("HDMI-A-1"));
        assert_eq!(out.size, Some([1920, 1200]));
        assert_eq!(out.picture, Some([1200, 1200]));
        assert_eq!(out.hz, Some(60.0));
        assert_eq!(out.keystone.as_deref(), Some("config/keystone.json"));
        assert_eq!(out.remote, Some(9001));
    }

    #[test]
    fn the_brief_s_cube_face_parses_whole() {
        let patch = ok(include_str!("../../tests/fixtures/map-show.vy"));
        assert_eq!(patch.nodes.len(), 14);
        assert_eq!(patch.scenes.len(), 3);
        assert_eq!(patch.transitions.len(), 8);
        assert!(patch.out.is_some());
    }

    #[test]
    fn a_typo_in_a_modifier_suggests_the_word() {
        let e = first_error("a = circle .1 sofft 1");
        assert!(e.message.contains("sofft"), "{e}");
        assert_eq!(e.help.as_deref(), Some("did you mean `soft`?"));
    }

    #[test]
    fn a_modifier_on_the_wrong_family_says_what_fits() {
        let e = first_error("a = circle .1 loop");
        assert!(e.message.contains("doesn't apply"), "{e}");
        assert!(e.help.unwrap().contains("soft"));
    }

    #[test]
    fn every_broken_line_is_reported_not_just_the_first() {
        let errors = parse("a = circle\nb = circle .1\nc -> d").unwrap_err();
        assert_eq!(errors.len(), 2);
        assert_eq!((errors[0].line, errors[1].line), (1, 3));
    }

    #[test]
    fn a_node_cannot_be_named_after_a_word_of_the_language() {
        let e = first_error("circle = rect 1 1");
        assert!(e.message.contains("word of the language"), "{e}");
    }

    #[test]
    fn unknown_units_are_named() {
        let e = first_error("a = circle 3cm");
        assert!(e.message.contains("cm"), "{e}");
    }
}
