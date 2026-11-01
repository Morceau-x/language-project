%grmtools{yacckind: Grmtools}
// Mini's grammar in yacc syntax, for grmtools. At build time it becomes
// an LALR(1) parser (see build.rs), driven by the lexer from mini.l.
//
// Each alternative has an *action* in braces: Rust code that computes the
// value of the rule from the values of its parts, `$1`, `$2`, `$3`. Here
// every value is a parse tree (`Cst`). `$lexer` gives access to the source
// text of a token.

%start Expr

// Part 3b: %avoid_insert and the precedence declarations go here.

%%
Expr -> Cst<'input>:
      'INT' { node(vec![token($lexer, $1)]) }
    // Part 3b: the other alternatives go here.
    ;
%%
// Code here is in scope in the actions above. `node` and `token` are
// defined in mod.rs.
use super::{node, token};
use crate::cst::Cst;
