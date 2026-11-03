//! Matching a text against a lexical rule.
//!
//! The lexical grammar is a set of regular expressions with names, plus
//! `A - B`. A text matches a rule if the rule can match all of it. The
//! matcher computes, for an expression and a start position, every
//! position where a match can end; `A - B` then simply keeps the ends
//! of `A` that aren't ends of `B`.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::ebnf::{Expr, ExprKind, Grammar};

/// Does `text` match the rule `rule`, all of it?
pub fn matches(grammar: &Grammar, rule: &str, text: &str) -> bool {
    let Some(rule) = grammar.rules.iter().find(|r| r.name == rule) else {
        return false;
    };
    let mut matcher = Matcher {
        grammar,
        chars: text.chars().collect(),
        memo: HashMap::new(),
        busy: HashSet::new(),
    };
    let len = matcher.chars.len();
    matcher.ends(&rule.expr, 0).contains(&len)
}

struct Matcher<'g> {
    grammar: &'g Grammar,
    chars: Vec<char>,
    memo: HashMap<(usize, usize), BTreeSet<usize>>,
    /// Matches in progress: a rule that refers to itself would loop.
    busy: HashSet<(usize, usize)>,
}

impl Matcher<'_> {
    /// Where a match of `expr` starting at `pos` can end.
    fn ends(&mut self, expr: &Expr, pos: usize) -> BTreeSet<usize> {
        let key = (expr.id, pos);
        if let Some(ends) = self.memo.get(&key) {
            return ends.clone();
        }
        if !self.busy.insert(key) {
            return BTreeSet::new();
        }
        let ends = self.compute(expr, pos);
        self.busy.remove(&key);
        self.memo.insert(key, ends.clone());
        ends
    }

    fn compute(&mut self, expr: &Expr, pos: usize) -> BTreeSet<usize> {
        match &expr.kind {
            ExprKind::Lit(text) => {
                let lit: Vec<char> = text.chars().collect();
                let end = pos + lit.len();
                if self.chars.get(pos..end) == Some(&lit[..]) {
                    BTreeSet::from([end])
                } else {
                    BTreeSet::new()
                }
            }
            ExprKind::Class(class) => match self.chars.get(pos) {
                Some(&c) if class.contains(c) => BTreeSet::from([pos + 1]),
                _ => BTreeSet::new(),
            },
            ExprKind::Ref(name) => {
                let grammar = self.grammar;
                match grammar.rules.iter().find(|r| &r.name == name) {
                    Some(rule) => self.ends(&rule.expr, pos),
                    None => BTreeSet::new(),
                }
            }
            ExprKind::Seq(items) => {
                let mut positions = BTreeSet::from([pos]);
                for item in items {
                    let mut next = BTreeSet::new();
                    for p in positions {
                        next.extend(self.ends(item, p));
                    }
                    positions = next;
                }
                positions
            }
            ExprKind::Alt(alts) => {
                let mut ends = BTreeSet::new();
                for alt in alts {
                    ends.extend(self.ends(alt, pos));
                }
                ends
            }
            ExprKind::Opt(inner) => {
                let mut ends = self.ends(inner, pos);
                ends.insert(pos);
                ends
            }
            ExprKind::Star(inner) => self.repeat(inner, BTreeSet::from([pos])),
            ExprKind::Plus(inner) => {
                let first = self.ends(inner, pos);
                self.repeat(inner, first)
            }
            ExprKind::Diff(a, b) => {
                let excluded = self.ends(b, pos);
                self.ends(a, pos).difference(&excluded).copied().collect()
            }
        }
    }

    /// Everything reachable from `start` by repeating `inner`.
    fn repeat(&mut self, inner: &Expr, start: BTreeSet<usize>) -> BTreeSet<usize> {
        let mut all = start.clone();
        let mut frontier: Vec<usize> = start.into_iter().collect();
        while let Some(p) = frontier.pop() {
            for end in self.ends(inner, p) {
                if all.insert(end) {
                    frontier.push(end);
                }
            }
        }
        all
    }
}
