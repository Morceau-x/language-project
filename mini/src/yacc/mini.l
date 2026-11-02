%grmtools {allow_wholeline_comments}
%%
// The lexer for the generated front end. Each rule is a regular
// expression and the name of the token it makes. When several rules
// match, the longest match wins; on a tie, the first rule wins. `;`
// instead of a name means "skip it".

[0-9]+ "INT"
[a-zA-Z_][a-zA-Z0-9_]* "IDENT"
\+ "+"
- "-"
\* "*"
/ "/"
\( "("
\) ")"
\s+ ;
