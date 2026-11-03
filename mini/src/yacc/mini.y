%grmtools{yacckind: Grmtools}
// Mini's grammar in yacc syntax, for grmtools. At build time it becomes
// an LR(1) parser (see build.rs), driven by the lexer from mini.l.
//
// Each alternative has an *action* in braces: Rust code that computes the
// value of the rule from the values of its parts, `$1`, `$2`, `$3`. Here
// every value is a parse tree (`Cst`). `$lexer` gives access to the source
// text of a token.

%start Expr

%avoid_insert "INT" "IDENT"
%left '+' '-'
%left '*' '/'
%nonassoc NEG
%expect-unused 'NEG'

%%

Expr -> Cst<'input>:
      Expr '+' Expr { node(vec![$1, token($lexer, $2), $3]) }
    | Expr '-' Expr { node(vec![$1, token($lexer, $2), $3]) }
    | Expr '*' Expr { node(vec![$1, token($lexer, $2), $3]) }
    | Expr '/' Expr { node(vec![$1, token($lexer, $2), $3]) }
    | '-' Expr %prec NEG { node(vec![token($lexer, $1), $2]) }
    | '(' Expr ')' { node(vec![token($lexer, $1), $2, token($lexer, $3)]) }
    | 'INT' { node(vec![token($lexer, $1)]) }
    | 'IDENT' { node(vec![token($lexer, $1)]) }
    ;

%%
// Code here is in scope in the actions above. `node` and `token` are
// defined in mod.rs.
use super::{node, token};
use crate::cst::Cst;
