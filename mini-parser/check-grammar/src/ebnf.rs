//! Reading the spec: its ` ```ebnf ` blocks, in the W3C's EBNF notation.

use std::fmt;

/// A grammar: rules, in the order of the spec.
pub struct Grammar {
    pub rules: Vec<Rule>,
}

/// `name ::= expr`
pub struct Rule {
    pub name: String,
    pub expr: Expr,
    /// Where the rule is, in the spec file.
    pub line: usize,
}

/// A grammar expression, with its source text for messages.
pub struct Expr {
    /// Unique in the grammar: a key for caches.
    pub id: usize,
    pub kind: ExprKind,
    pub text: String,
    pub line: usize,
}

pub enum ExprKind {
    /// `A | B`
    Alt(Vec<Expr>),
    /// `A B`
    Seq(Vec<Expr>),
    /// `A - B`
    Diff(Box<Expr>, Box<Expr>),
    /// `A?`
    Opt(Box<Expr>),
    /// `A*`
    Star(Box<Expr>),
    /// `A+`
    Plus(Box<Expr>),
    /// `"text"` or `'text'`
    Lit(String),
    /// `[a-z_]`, `[^"]`, `#xA`
    Class(CharClass),
    /// A rule's name.
    Ref(String),
}

/// A set of characters.
pub struct CharClass {
    pub negated: bool,
    pub ranges: Vec<(char, char)>,
}

impl CharClass {
    pub fn contains(&self, c: char) -> bool {
        let inside = self.ranges.iter().any(|&(lo, hi)| lo <= c && c <= hi);
        inside != self.negated
    }
}

/// An error in the spec, at a line of the spec file.
pub struct SpecError {
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.msg)
    }
}

/// Read every ` ```ebnf ` block of a Markdown file.
pub fn parse_spec(markdown: &str) -> Result<Grammar, Vec<SpecError>> {
    let mut rules = Vec::new();
    let mut errors = Vec::new();
    let mut ids = 0;
    let mut block: Option<(usize, String)> = None;
    for (index, line) in markdown.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        match &mut block {
            None if trimmed == "```ebnf" => block = Some((line_number + 1, String::new())),
            None => {}
            Some((start, text)) => {
                if trimmed.starts_with("```") {
                    match parse_block(text, *start, &mut ids) {
                        Ok(block_rules) => rules.extend(block_rules),
                        Err(error) => errors.push(error),
                    }
                    block = None;
                } else {
                    text.push_str(line);
                    text.push('\n');
                }
            }
        }
    }
    if let Some((start, _)) = block {
        errors.push(SpecError {
            line: start - 1,
            msg: "this ```ebnf block never ends".to_string(),
        });
    }
    if errors.is_empty() {
        Ok(Grammar { rules })
    } else {
        Err(errors)
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Name(String),
    Define,
    Pipe,
    Minus,
    Question,
    Star,
    Plus,
    LParen,
    RParen,
    Lit(String),
    Class(bool, Vec<(char, char)>),
    Eof,
}

struct Token {
    tok: Tok,
    start: usize,
    end: usize,
    line: usize,
}

fn tokenize(text: &str, first_line: usize) -> Result<Vec<Token>, SpecError> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut line = first_line;
    let error = |line: usize, msg: String| SpecError { line, msg };
    while i < chars.len() {
        let (start, c) = chars[i];
        let next = chars.get(i + 1).map(|&(_, c)| c);
        let single = |tok: Tok| (tok, 1);
        let (tok, len) = match c {
            '\n' => {
                line += 1;
                i += 1;
                continue;
            }
            c if c.is_whitespace() => {
                i += 1;
                continue;
            }
            '/' if next == Some('*') => {
                let mut j = i + 2;
                while j + 1 < chars.len() && !(chars[j].1 == '*' && chars[j + 1].1 == '/') {
                    if chars[j].1 == '\n' {
                        line += 1;
                    }
                    j += 1;
                }
                if j + 1 >= chars.len() {
                    return Err(error(line, "a `/*` comment never ends".to_string()));
                }
                i = j + 2;
                continue;
            }
            ':' if text[start..].starts_with("::=") => (Tok::Define, 3),
            '|' => single(Tok::Pipe),
            '-' => single(Tok::Minus),
            '?' => single(Tok::Question),
            '*' => single(Tok::Star),
            '+' => single(Tok::Plus),
            '(' => single(Tok::LParen),
            ')' => single(Tok::RParen),
            '"' | '\'' => {
                let mut j = i + 1;
                while j < chars.len() && chars[j].1 != c && chars[j].1 != '\n' {
                    j += 1;
                }
                if j >= chars.len() || chars[j].1 != c {
                    return Err(error(line, format!("a {c}…{c} text never ends")));
                }
                let lit: String = chars[i + 1..j].iter().map(|&(_, c)| c).collect();
                if lit.is_empty() {
                    return Err(error(
                        line,
                        "an empty text matches nothing useful".to_string(),
                    ));
                }
                (Tok::Lit(lit), j - i + 1)
            }
            '#' if next == Some('x') => {
                let mut j = i + 2;
                while j < chars.len() && chars[j].1.is_ascii_hexdigit() {
                    j += 1;
                }
                let hex: String = chars[i + 2..j].iter().map(|&(_, c)| c).collect();
                let c = u32::from_str_radix(&hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| error(line, format!("`#x{hex}` is not a character")))?;
                (Tok::Class(false, vec![(c, c)]), j - i)
            }
            '[' => {
                let (negated, ranges, len) =
                    char_class(&chars[i..]).map_err(|msg| error(line, msg))?;
                (Tok::Class(negated, ranges), len)
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut j = i;
                while j < chars.len() && (chars[j].1.is_ascii_alphanumeric() || chars[j].1 == '_') {
                    j += 1;
                }
                let name: String = chars[i..j].iter().map(|&(_, c)| c).collect();
                (Tok::Name(name), j - i)
            }
            other => return Err(error(line, format!("`{other}` has no meaning in EBNF"))),
        };
        let end = chars.get(i + len).map_or(text.len(), |&(offset, _)| offset);
        tokens.push(Token {
            tok,
            start,
            end,
            line,
        });
        i += len;
    }
    tokens.push(Token {
        tok: Tok::Eof,
        start: text.len(),
        end: text.len(),
        line,
    });
    Ok(tokens)
}

/// A character class: whether it's negated, its ranges, and its length
/// in the source, in characters.
type ClassParts = (bool, Vec<(char, char)>, usize);

/// `[…]` at the start of `chars`.
fn char_class(chars: &[(usize, char)]) -> Result<ClassParts, String> {
    let mut i = 1;
    let negated = chars.get(i).is_some_and(|&(_, c)| c == '^');
    if negated {
        i += 1;
    }
    let mut ranges = Vec::new();
    // One character of the class: `a` or `#xN`.
    let one = |i: &mut usize| -> Result<char, String> {
        match chars.get(*i).map(|&(_, c)| c) {
            Some('#') if chars.get(*i + 1).map(|&(_, c)| c) == Some('x') => {
                let mut j = *i + 2;
                while chars.get(j).is_some_and(|&(_, c)| c.is_ascii_hexdigit()) {
                    j += 1;
                }
                let hex: String = chars[*i + 2..j].iter().map(|&(_, c)| c).collect();
                *i = j;
                u32::from_str_radix(&hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| format!("`#x{hex}` is not a character"))
            }
            Some('\n') | None => Err("a `[` class never ends".to_string()),
            Some(c) => {
                *i += 1;
                Ok(c)
            }
        }
    };
    while chars.get(i).map(|&(_, c)| c) != Some(']') {
        let lo = one(&mut i)?;
        let hi = if chars.get(i).map(|&(_, c)| c) == Some('-')
            && chars.get(i + 1).is_some_and(|&(_, c)| c != ']')
        {
            i += 1;
            one(&mut i)?
        } else {
            lo
        };
        if hi < lo {
            return Err(format!("the range `{lo}-{hi}` is backwards"));
        }
        ranges.push((lo, hi));
    }
    if ranges.is_empty() {
        return Err("an empty class `[]` matches nothing".to_string());
    }
    Ok((negated, ranges, i + 1))
}

struct Parser<'a> {
    text: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    ids: &'a mut usize,
}

fn parse_block(text: &str, first_line: usize, ids: &mut usize) -> Result<Vec<Rule>, SpecError> {
    let tokens = tokenize(text, first_line)?;
    let mut p = Parser {
        text,
        tokens,
        pos: 0,
        ids,
    };
    let mut rules = Vec::new();
    while p.peek() != &Tok::Eof {
        let line = p.tokens[p.pos].line;
        let Tok::Name(name) = p.peek().clone() else {
            return Err(p.error("expected a rule: `name ::= …`"));
        };
        p.pos += 1;
        if p.peek() != &Tok::Define {
            return Err(p.error(&format!("expected `::=` after `{name}`")));
        }
        p.pos += 1;
        if p.at_item_end() {
            return Err(p.error(&format!("the rule `{name}` is empty")));
        }
        let expr = p.alt()?;
        if !p.at_rule_start() && p.peek() != &Tok::Eof {
            return Err(p.error("this doesn't fit here"));
        }
        rules.push(Rule { name, expr, line });
    }
    Ok(rules)
}

impl Parser<'_> {
    fn peek(&self) -> &Tok {
        &self.tokens[self.pos].tok
    }

    fn error(&self, msg: &str) -> SpecError {
        SpecError {
            line: self.tokens[self.pos].line,
            msg: msg.to_string(),
        }
    }

    /// At `name ::=`: the next rule starts here.
    fn at_rule_start(&self) -> bool {
        matches!(self.peek(), Tok::Name(_))
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| t.tok == Tok::Define)
    }

    fn at_item_end(&self) -> bool {
        matches!(self.peek(), Tok::Pipe | Tok::RParen | Tok::Eof) || self.at_rule_start()
    }

    fn make(&mut self, kind: ExprKind, start: usize) -> Expr {
        let first = &self.tokens[start];
        let last = &self.tokens[self.pos - 1];
        *self.ids += 1;
        Expr {
            id: *self.ids,
            kind,
            text: self.text[first.start..last.end].to_string(),
            line: first.line,
        }
    }

    fn alt(&mut self) -> Result<Expr, SpecError> {
        let start = self.pos;
        let mut alts = vec![self.seq()?];
        while self.peek() == &Tok::Pipe {
            self.pos += 1;
            if self.at_item_end() {
                return Err(self.error("expected something after `|`"));
            }
            alts.push(self.seq()?);
        }
        Ok(if alts.len() == 1 {
            alts.pop().unwrap()
        } else {
            self.make(ExprKind::Alt(alts), start)
        })
    }

    fn seq(&mut self) -> Result<Expr, SpecError> {
        let start = self.pos;
        let mut items = Vec::new();
        while !self.at_item_end() {
            items.push(self.diff()?);
        }
        Ok(if items.len() == 1 {
            items.pop().unwrap()
        } else {
            self.make(ExprKind::Seq(items), start)
        })
    }

    fn diff(&mut self) -> Result<Expr, SpecError> {
        let start = self.pos;
        let lhs = self.postfix()?;
        if self.peek() != &Tok::Minus {
            return Ok(lhs);
        }
        self.pos += 1;
        if self.at_item_end() {
            return Err(self.error("expected something after `-`"));
        }
        let rhs = self.postfix()?;
        Ok(self.make(ExprKind::Diff(Box::new(lhs), Box::new(rhs)), start))
    }

    fn postfix(&mut self) -> Result<Expr, SpecError> {
        let start = self.pos;
        let mut expr = self.primary()?;
        loop {
            let kind = match self.peek() {
                Tok::Question => ExprKind::Opt(Box::new(expr)),
                Tok::Star => ExprKind::Star(Box::new(expr)),
                Tok::Plus => ExprKind::Plus(Box::new(expr)),
                _ => return Ok(expr),
            };
            self.pos += 1;
            expr = self.make(kind, start);
        }
    }

    fn primary(&mut self) -> Result<Expr, SpecError> {
        let start = self.pos;
        let kind = match self.peek().clone() {
            Tok::Name(name) => ExprKind::Ref(name),
            Tok::Lit(text) => ExprKind::Lit(text),
            Tok::Class(negated, ranges) => ExprKind::Class(CharClass { negated, ranges }),
            Tok::LParen => {
                self.pos += 1;
                if self.at_item_end() {
                    return Err(self.error("expected something inside `( )`"));
                }
                let inner = self.alt()?;
                if self.peek() != &Tok::RParen {
                    return Err(self.error("expected `)`"));
                }
                self.pos += 1;
                // Keep the parentheses in the text, for messages.
                let mut grouped = self.make(ExprKind::Seq(vec![]), start);
                grouped.kind = inner.kind;
                grouped.id = inner.id;
                return Ok(grouped);
            }
            Tok::Define => return Err(self.error("`::=` must follow a rule's name")),
            _ => return Err(self.error("expected a name, a \"text\", a [class] or `(`")),
        };
        self.pos += 1;
        Ok(self.make(kind, start))
    }
}

impl Expr {
    /// Every rule name this expression refers to.
    pub fn refs(&self, out: &mut Vec<(String, usize)>) {
        match &self.kind {
            ExprKind::Alt(items) | ExprKind::Seq(items) => {
                for item in items {
                    item.refs(out);
                }
            }
            ExprKind::Diff(a, b) => {
                a.refs(out);
                b.refs(out);
            }
            ExprKind::Opt(a) | ExprKind::Star(a) | ExprKind::Plus(a) => a.refs(out),
            ExprKind::Ref(name) => out.push((name.clone(), self.line)),
            ExprKind::Lit(_) | ExprKind::Class(_) => {}
        }
    }

    /// Visit this expression and every expression inside it.
    pub fn walk(&self, visit: &mut impl FnMut(&Expr)) {
        visit(self);
        match &self.kind {
            ExprKind::Alt(items) | ExprKind::Seq(items) => {
                for item in items {
                    item.walk(visit);
                }
            }
            ExprKind::Diff(a, b) => {
                a.walk(visit);
                b.walk(visit);
            }
            ExprKind::Opt(a) | ExprKind::Star(a) | ExprKind::Plus(a) => a.walk(visit),
            ExprKind::Ref(_) | ExprKind::Lit(_) | ExprKind::Class(_) => {}
        }
    }
}
