//! Where would a recursive descent parser need more than one token of
//! lookahead? The classic LL(1) analysis: FIRST and FOLLOW sets, then
//! every choice whose options can start with the same token.

use std::collections::BTreeSet;

use crate::earley::{Bnf, Sym};

/// One place where one token isn't enough to choose.
pub struct Conflict {
    /// The spec rule it's in.
    pub rule: String,
    /// What the problem is, in words.
    pub what: String,
}

/// The token that ends the input, in FOLLOW sets.
const END: usize = usize::MAX;

pub fn conflicts(bnf: &Bnf, start: usize) -> Vec<Conflict> {
    let first = first_sets(bnf);
    let follow = follow_sets(bnf, &first, start);
    let name = |t: usize| {
        if t == END {
            "the end of the file".to_string()
        } else {
            format!("`{}`", bnf.terminals[t])
        }
    };
    let mut conflicts = Vec::new();
    for (nt, info) in bnf.nts.iter().enumerate() {
        let prods = &bnf.by_lhs[nt];
        if prods.len() < 2 {
            continue;
        }
        // What tokens can each option start with? An option that can be
        // empty also "starts" with what follows the whole choice.
        let predict: Vec<BTreeSet<usize>> = prods
            .iter()
            .map(|&p| {
                let (mut set, nullable) = first_of(bnf, &first, &bnf.prods[p].rhs);
                if nullable {
                    set.extend(&follow[nt]);
                }
                set
            })
            .collect();
        let mut shared = BTreeSet::new();
        for a in 0..predict.len() {
            for b in a + 1..predict.len() {
                shared.extend(predict[a].intersection(&predict[b]).copied());
            }
        }
        if shared.is_empty() {
            continue;
        }
        let tokens: Vec<String> = shared.into_iter().map(name).collect();
        let tokens = tokens.join(", ");
        let empty_option = prods.iter().any(|&p| bnf.prods[p].rhs.is_empty());
        let what = if info.made && empty_option {
            format!(
                "at `{}`, {tokens} can start the repeated or optional part, and also what comes after it",
                info.name
            )
        } else if info.made {
            format!(
                "at `{}`, {tokens} can start more than one option",
                info.name
            )
        } else {
            format!("{tokens} can start more than one of its alternatives")
        };
        conflicts.push(Conflict {
            rule: info.rule.clone(),
            what,
        });
    }
    conflicts
}

/// Left-recursive rules: `A ::= A …`. Recursive descent would call
/// itself forever on them.
pub fn left_recursive(bnf: &Bnf) -> Vec<String> {
    let mut found = BTreeSet::new();
    for nt in 0..bnf.nts.len() {
        // Can `nt` derive a sequence that starts with `nt`?
        let mut seen = BTreeSet::new();
        let mut todo = vec![nt];
        while let Some(current) = todo.pop() {
            for &p in &bnf.by_lhs[current] {
                for sym in &bnf.prods[p].rhs {
                    let Sym::N(next) = *sym else { break };
                    if next == nt {
                        found.insert(bnf.nts[nt].rule.clone());
                    }
                    if seen.insert(next) {
                        todo.push(next);
                    }
                    if !bnf.nullable[next] {
                        break;
                    }
                }
            }
        }
    }
    found.into_iter().collect()
}

fn first_sets(bnf: &Bnf) -> Vec<BTreeSet<usize>> {
    let mut first = vec![BTreeSet::new(); bnf.nts.len()];
    loop {
        let mut changed = false;
        for prod in &bnf.prods {
            let (set, _) = first_of(bnf, &first, &prod.rhs);
            let before = first[prod.lhs].len();
            first[prod.lhs].extend(set);
            changed |= first[prod.lhs].len() != before;
        }
        if !changed {
            return first;
        }
    }
}

/// The tokens a sequence can start with, and whether it can be empty.
fn first_of(bnf: &Bnf, first: &[BTreeSet<usize>], rhs: &[Sym]) -> (BTreeSet<usize>, bool) {
    let mut set = BTreeSet::new();
    for sym in rhs {
        match *sym {
            Sym::T(t) => {
                set.insert(t);
                return (set, false);
            }
            Sym::N(nt) => {
                set.extend(&first[nt]);
                if !bnf.nullable[nt] {
                    return (set, false);
                }
            }
        }
    }
    (set, true)
}

fn follow_sets(bnf: &Bnf, first: &[BTreeSet<usize>], start: usize) -> Vec<BTreeSet<usize>> {
    let mut follow = vec![BTreeSet::new(); bnf.nts.len()];
    follow[start].insert(END);
    loop {
        let mut changed = false;
        for prod in &bnf.prods {
            for (k, sym) in prod.rhs.iter().enumerate() {
                let Sym::N(nt) = *sym else { continue };
                let (mut set, nullable) = first_of(bnf, first, &prod.rhs[k + 1..]);
                if nullable {
                    set.extend(follow[prod.lhs].clone());
                }
                let before = follow[nt].len();
                follow[nt].extend(set);
                changed |= follow[nt].len() != before;
            }
        }
        if !changed {
            return follow;
        }
    }
}
