//! The syntactic grammar as plain BNF, and an Earley parser for it.
//!
//! Earley parsing accepts any context-free grammar, ambiguous or not,
//! left-recursive or not: whatever you write in the spec, it can tell
//! whether a program follows it. And it can count the ways it does:
//! more than one is an ambiguity.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::ebnf::{Expr, ExprKind, Grammar};

/// A grammar symbol: a terminal (a token kind) or a nonterminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sym {
    T(usize),
    N(usize),
}

/// `lhs ::= rhs`, with only sequences: the EBNF operators become extra
/// nonterminals.
pub struct Prod {
    pub lhs: usize,
    pub rhs: Vec<Sym>,
}

/// A nonterminal: a rule of the spec, or one made for an EBNF operator.
pub struct Nonterminal {
    /// The rule's name, or for a made one, the EBNF it stands for.
    pub name: String,
    /// The spec rule it comes from.
    pub rule: String,
    /// Made for an EBNF operator: invisible in trees.
    pub made: bool,
}

pub struct Bnf {
    pub nts: Vec<Nonterminal>,
    pub prods: Vec<Prod>,
    pub by_lhs: Vec<Vec<usize>>,
    pub nullable: Vec<bool>,
    pub terminals: Vec<String>,
    term_index: HashMap<String, usize>,
    nt_index: HashMap<String, usize>,
}

impl Bnf {
    /// Translate the rules reachable from `starts`. Names in `tokens` are
    /// terminals; a quoted text `"fn"` is the terminal `"fn"`.
    pub fn new(grammar: &Grammar, starts: &[&str], tokens: &[&str]) -> Bnf {
        let mut bnf = Bnf {
            nts: Vec::new(),
            prods: Vec::new(),
            by_lhs: Vec::new(),
            nullable: Vec::new(),
            terminals: Vec::new(),
            term_index: HashMap::new(),
            nt_index: HashMap::new(),
        };
        let rules: HashMap<&str, &Expr> = grammar
            .rules
            .iter()
            .map(|rule| (rule.name.as_str(), &rule.expr))
            .collect();
        let mut todo: Vec<String> = starts.iter().map(|s| s.to_string()).collect();
        let mut done = HashSet::new();
        while let Some(name) = todo.pop() {
            if !done.insert(name.clone()) {
                continue;
            }
            let Some(expr) = rules.get(name.as_str()) else {
                continue;
            };
            let lhs = bnf.rule_nt(&name);
            let alts: Vec<&Expr> = match &expr.kind {
                ExprKind::Alt(alts) => alts.iter().collect(),
                _ => vec![expr],
            };
            for alt in alts {
                let rhs = bnf.seq(alt, &name, tokens, &mut todo);
                bnf.add(lhs, rhs);
            }
        }
        bnf.compute_nullable();
        bnf
    }

    pub fn nt(&self, name: &str) -> Option<usize> {
        self.nt_index.get(name).copied()
    }

    pub fn term(&self, name: &str) -> Option<usize> {
        self.term_index.get(name).copied()
    }

    fn rule_nt(&mut self, name: &str) -> usize {
        if let Some(&nt) = self.nt_index.get(name) {
            return nt;
        }
        let nt = self.new_nt(name, name, false);
        self.nt_index.insert(name.to_string(), nt);
        nt
    }

    fn new_nt(&mut self, name: &str, rule: &str, made: bool) -> usize {
        self.nts.push(Nonterminal {
            name: name.to_string(),
            rule: rule.to_string(),
            made,
        });
        self.by_lhs.push(Vec::new());
        self.nts.len() - 1
    }

    fn terminal(&mut self, name: String) -> usize {
        if let Some(&t) = self.term_index.get(&name) {
            return t;
        }
        self.terminals.push(name.clone());
        self.term_index.insert(name, self.terminals.len() - 1);
        self.terminals.len() - 1
    }

    fn add(&mut self, lhs: usize, rhs: Vec<Sym>) {
        self.prods.push(Prod { lhs, rhs });
        self.by_lhs[lhs].push(self.prods.len() - 1);
    }

    /// The symbols for `expr`, making nonterminals for its operators.
    fn seq(
        &mut self,
        expr: &Expr,
        rule: &str,
        tokens: &[&str],
        todo: &mut Vec<String>,
    ) -> Vec<Sym> {
        match &expr.kind {
            ExprKind::Seq(items) => items
                .iter()
                .flat_map(|item| self.seq(item, rule, tokens, todo))
                .collect(),
            ExprKind::Lit(text) => vec![Sym::T(self.terminal(format!("\"{text}\"")))],
            ExprKind::Ref(name) if tokens.contains(&name.as_str()) => {
                vec![Sym::T(self.terminal(name.clone()))]
            }
            ExprKind::Ref(name) => {
                todo.push(name.clone());
                vec![Sym::N(self.rule_nt(name))]
            }
            ExprKind::Alt(alts) => {
                let nt = self.new_nt(&expr.text, rule, true);
                for alt in alts {
                    let rhs = self.seq(alt, rule, tokens, todo);
                    self.add(nt, rhs);
                }
                vec![Sym::N(nt)]
            }
            ExprKind::Opt(inner) => {
                let nt = self.new_nt(&expr.text, rule, true);
                self.add(nt, vec![]);
                let rhs = self.seq(inner, rule, tokens, todo);
                self.add(nt, rhs);
                vec![Sym::N(nt)]
            }
            ExprKind::Star(inner) | ExprKind::Plus(inner) => {
                // `A*` is `S ::= ε | A S`; `A+` is `A S`.
                let star_text = match expr.kind {
                    ExprKind::Star(_) => expr.text.clone(),
                    _ => format!("{}*", inner.text),
                };
                let nt = self.new_nt(&star_text, rule, true);
                self.add(nt, vec![]);
                let mut rhs = self.seq(inner, rule, tokens, todo);
                rhs.push(Sym::N(nt));
                self.add(nt, rhs);
                match expr.kind {
                    ExprKind::Star(_) => vec![Sym::N(nt)],
                    _ => {
                        let mut once = self.seq(inner, rule, tokens, todo);
                        once.push(Sym::N(nt));
                        once
                    }
                }
            }
            // Characters and `-` belong to the lexical grammar: the checks
            // report them before this runs.
            ExprKind::Class(_) | ExprKind::Diff(..) => vec![],
        }
    }

    fn compute_nullable(&mut self) {
        self.nullable = vec![false; self.nts.len()];
        loop {
            let mut changed = false;
            for prod in &self.prods {
                if !self.nullable[prod.lhs]
                    && prod.rhs.iter().all(|sym| match sym {
                        Sym::T(_) => false,
                        Sym::N(nt) => self.nullable[*nt],
                    })
                {
                    self.nullable[prod.lhs] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }
}

/// An Earley item: a production, how much of it is matched, and where
/// the match started.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Item {
    prod: u32,
    dot: u32,
    origin: u32,
}

/// The result of recognizing a token sequence.
pub struct Chart<'b> {
    bnf: &'b Bnf,
    tokens: Vec<Option<usize>>,
    sets: Vec<HashSet<Item>>,
}

/// Run the Earley algorithm from `start` over `tokens` (terminal ids;
/// `None` for a token the grammar never mentions).
pub fn recognize<'b>(bnf: &'b Bnf, start: usize, tokens: &[Option<usize>]) -> Chart<'b> {
    let n = tokens.len();
    let mut sets: Vec<HashSet<Item>> = vec![HashSet::new(); n + 1];
    let mut lists: Vec<Vec<Item>> = vec![Vec::new(); n + 1];
    let add = |sets: &mut Vec<HashSet<Item>>, lists: &mut Vec<Vec<Item>>, at: usize, item: Item| {
        if sets[at].insert(item) {
            lists[at].push(item);
        }
    };
    for &prod in &bnf.by_lhs[start] {
        add(
            &mut sets,
            &mut lists,
            0,
            Item {
                prod: prod as u32,
                dot: 0,
                origin: 0,
            },
        );
    }
    for i in 0..=n {
        let mut k = 0;
        while k < lists[i].len() {
            let item = lists[i][k];
            k += 1;
            let prod = &bnf.prods[item.prod as usize];
            match prod.rhs.get(item.dot as usize) {
                None => {
                    // Complete: advance the items waiting for this lhs.
                    let origin = item.origin as usize;
                    let mut j = 0;
                    while j < lists[origin].len() {
                        let waiting = lists[origin][j];
                        j += 1;
                        let wprod = &bnf.prods[waiting.prod as usize];
                        if wprod.rhs.get(waiting.dot as usize) == Some(&Sym::N(prod.lhs)) {
                            add(
                                &mut sets,
                                &mut lists,
                                i,
                                Item {
                                    dot: waiting.dot + 1,
                                    ..waiting
                                },
                            );
                        }
                    }
                }
                Some(Sym::N(nt)) => {
                    for &p in &bnf.by_lhs[*nt] {
                        add(
                            &mut sets,
                            &mut lists,
                            i,
                            Item {
                                prod: p as u32,
                                dot: 0,
                                origin: i as u32,
                            },
                        );
                    }
                    // Aycock and Horspool's fix: a nullable symbol can be
                    // skipped right away.
                    if bnf.nullable[*nt] {
                        add(
                            &mut sets,
                            &mut lists,
                            i,
                            Item {
                                dot: item.dot + 1,
                                ..item
                            },
                        );
                    }
                }
                Some(Sym::T(t)) => {
                    if i < n && tokens[i] == Some(*t) {
                        add(
                            &mut sets,
                            &mut lists,
                            i + 1,
                            Item {
                                dot: item.dot + 1,
                                ..item
                            },
                        );
                    }
                }
            }
        }
    }
    Chart {
        bnf,
        tokens: tokens.to_vec(),
        sets,
    }
}

/// A parse tree: nodes for the spec's rules, leaves for tokens.
pub enum Tree {
    Token(usize),
    Node { nt: usize, children: Vec<Tree> },
}

impl Chart<'_> {
    fn has(&self, prod: usize, dot: usize, origin: usize, at: usize) -> bool {
        self.sets[at].contains(&Item {
            prod: prod as u32,
            dot: dot as u32,
            origin: origin as u32,
        })
    }

    /// Did `start` match all the tokens?
    pub fn accepts(&self, start: usize) -> bool {
        let n = self.tokens.len();
        self.bnf.by_lhs[start]
            .iter()
            .any(|&prod| self.has(prod, self.bnf.prods[prod].rhs.len(), 0, n))
    }

    /// A parse tree of all the tokens from `start`, and whether there is
    /// more than one.
    pub fn tree(&self, start: usize) -> Option<(Tree, bool)> {
        let n = self.tokens.len();
        let mut counter = Counter {
            chart: self,
            nt_memo: HashMap::new(),
            seq_memo: HashMap::new(),
            busy: HashSet::new(),
        };
        let count = counter.nt(start, 0, n);
        if count == 0 {
            return None;
        }
        let tree = counter.build(start, 0, n)?;
        Some((tree, count > 1))
    }
}

/// Counts parse trees, up to 2 ("more than one"), and builds one.
struct Counter<'c, 'b> {
    chart: &'c Chart<'b>,
    nt_memo: HashMap<(usize, usize, usize), u8>,
    seq_memo: HashMap<(usize, usize, usize, usize, usize), u8>,
    busy: HashSet<(usize, usize, usize)>,
}

impl Counter<'_, '_> {
    /// How many ways `nt` matches tokens `i..j`.
    fn nt(&mut self, nt: usize, i: usize, j: usize) -> u8 {
        if let Some(&count) = self.nt_memo.get(&(nt, i, j)) {
            return count;
        }
        if !self.busy.insert((nt, i, j)) {
            return 0;
        }
        let mut total = 0u8;
        for &prod in &self.chart.bnf.by_lhs[nt] {
            let len = self.chart.bnf.prods[prod].rhs.len();
            if self.chart.has(prod, len, i, j) {
                total = total.saturating_add(self.seq(prod, 0, i, i, j)).min(2);
            }
        }
        self.busy.remove(&(nt, i, j));
        self.nt_memo.insert((nt, i, j), total);
        total
    }

    /// How many ways the rest of `prod`, from symbol `k`, matches tokens
    /// `pos..j`, in a match of `prod` that started at `origin`.
    fn seq(&mut self, prod: usize, k: usize, origin: usize, pos: usize, j: usize) -> u8 {
        let key = (prod, k, origin, pos, j);
        if let Some(&count) = self.seq_memo.get(&key) {
            return count;
        }
        let rhs = &self.chart.bnf.prods[prod].rhs;
        let count = match rhs.get(k) {
            None => u8::from(pos == j),
            Some(&Sym::T(t)) => {
                if pos < j
                    && self.chart.tokens[pos] == Some(t)
                    && self.chart.has(prod, k + 1, origin, pos + 1)
                {
                    self.seq(prod, k + 1, origin, pos + 1, j)
                } else {
                    0
                }
            }
            Some(&Sym::N(nt)) => {
                let mut total = 0u8;
                for mid in pos..=j {
                    if total >= 2 {
                        break;
                    }
                    if !self.chart.has(prod, k + 1, origin, mid) {
                        continue;
                    }
                    let here = self.nt(nt, pos, mid);
                    if here > 0 {
                        let rest = self.seq(prod, k + 1, origin, mid, j);
                        total = total.saturating_add(here.saturating_mul(rest)).min(2);
                    }
                }
                total
            }
        };
        self.seq_memo.insert(key, count);
        count
    }

    /// One parse tree of `nt` over tokens `i..j`.
    fn build(&mut self, nt: usize, i: usize, j: usize) -> Option<Tree> {
        for &prod in &self.chart.bnf.by_lhs[nt] {
            let len = self.chart.bnf.prods[prod].rhs.len();
            if self.chart.has(prod, len, i, j) && self.seq(prod, 0, i, i, j) > 0 {
                let children = self.build_seq(prod, 0, i, i, j)?;
                return Some(Tree::Node { nt, children });
            }
        }
        None
    }

    fn build_seq(
        &mut self,
        prod: usize,
        k: usize,
        origin: usize,
        pos: usize,
        j: usize,
    ) -> Option<Vec<Tree>> {
        let rhs = self.chart.bnf.prods[prod].rhs.clone();
        match rhs.get(k) {
            None => Some(Vec::new()),
            Some(Sym::T(_)) => {
                let mut rest = self.build_seq(prod, k + 1, origin, pos + 1, j)?;
                rest.insert(0, Tree::Token(pos));
                Some(rest)
            }
            Some(&Sym::N(nt)) => {
                for mid in pos..=j {
                    if self.chart.has(prod, k + 1, origin, mid)
                        && self.nt(nt, pos, mid) > 0
                        && self.seq(prod, k + 1, origin, mid, j) > 0
                    {
                        let child = self.build(nt, pos, mid)?;
                        let mut rest = self.build_seq(prod, k + 1, origin, mid, j)?;
                        rest.insert(0, child);
                        return Some(rest);
                    }
                }
                None
            }
        }
    }
}

/// A node of a tree, seen the way the spec's rules make it: the
/// nonterminals made for EBNF operators are spliced into their parent.
pub struct Grouping {
    /// The token ranges of the spec's rules, and the ranges that a
    /// repetition implies: `A (op A)*` groups from the left.
    pub spans: BTreeSet<(usize, usize)>,
    /// The tree with one `⟨ ⟩` per node of two children or more.
    pub display: String,
}

pub fn grouping(bnf: &Bnf, tree: &Tree, texts: &[String]) -> Grouping {
    let mut spans = BTreeSet::new();
    let (display, _, _) = visit(bnf, tree, texts, &mut spans);
    Grouping { spans, display }
}

/// A child once made nonterminals are spliced: its display and range.
type Part = (String, usize, usize);

/// Returns the node's display and its token range.
fn visit(bnf: &Bnf, tree: &Tree, texts: &[String], spans: &mut BTreeSet<(usize, usize)>) -> Part {
    match tree {
        Tree::Token(i) => (texts[*i].clone(), *i, *i + 1),
        Tree::Node { .. } => {
            let mut parts: Vec<Part> = Vec::new();
            flatten(bnf, tree, texts, spans, &mut parts, true);
            let (Some(first), Some(last)) = (parts.first(), parts.last()) else {
                return (String::new(), usize::MAX, usize::MAX);
            };
            let (start, end) = (first.1, last.2);
            spans.insert((start, end));
            // A repetition groups from the left: `a - b - c` holds
            // `a - b`.
            for k in 2..parts.len() {
                spans.insert((start, parts[k - 1].2));
            }
            let display = if parts.len() == 1 {
                parts.pop().unwrap().0
            } else {
                let inner: Vec<String> = parts.into_iter().map(|p| p.0).collect();
                format!("⟨{}⟩", inner.join(" "))
            };
            (display, start, end)
        }
    }
}

/// The children of a node, with made nonterminals spliced in.
fn flatten(
    bnf: &Bnf,
    tree: &Tree,
    texts: &[String],
    spans: &mut BTreeSet<(usize, usize)>,
    out: &mut Vec<Part>,
    top: bool,
) {
    match tree {
        Tree::Node { nt, children } if top || bnf.nts[*nt].made => {
            for child in children {
                flatten(bnf, child, texts, spans, out, false);
            }
        }
        _ => {
            let part = visit(bnf, tree, texts, spans);
            if part.1 != usize::MAX {
                out.push(part);
            }
        }
    }
}
