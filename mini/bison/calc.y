/* calc.y: Mini's grammar for GNU Bison, the classic parser generator
 * (Bison is the GNU version of yacc, "yet another compiler compiler").
 *
 * Bison reads this file and writes calc.tab.c: a C program containing an
 * LALR(1) parser for the grammar below. Each rule has an *action*, C code
 * that runs when the rule is recognized, so this parser is a calculator:
 * it computes the value while it parses.
 *
 *   bison calc.y                           writes calc.tab.c
 *   clang calc.tab.c -o calc               compiles it
 *   echo '1 + 2 * 3' | ./calc              prints 7
 *   echo '1 + 2 * 3' | YYDEBUG=1 ./calc    shows each step of the parse
 *   bison --report=all calc.y              writes calc.output: the automaton
 *
 * The file has three parts, separated by %%: declarations, grammar
 * rules, and C code copied as is.
 */

%{
#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>

int yylex(void);
void yyerror(const char *message);
%}

/* Every semantic value ($$, $1, $2...) is a long long. */
%define api.value.type {long long}
/* Compile the tracing code in; YYDEBUG=1 switches it on (see main). */
%define parse.trace
/* Syntax errors say what was expected. */
%define parse.error verbose

/* How the trace (YYDEBUG=1) prints a value. */
%printer { fprintf(yyo, "%lld", $$); } <>

/* Tokens with a value. Single characters like '+' need no declaration. */
%token INT

/* Part 2d: precedence declarations go here. */

%%

/* Part 2a: Mini's grammar goes here. For now, a program is one integer. */
program:
    INT                     { printf("%lld\n", $1); }
  ;

%%

/* The lexer, written by hand: Bison only generates parsers. (Its partner
 * flex generates lexers from regular expressions, the way mini.l does for
 * grmtools in Part 3.) It returns the next token's kind, and leaves the
 * token's value in yylval. */
int yylex(void) {
    int c = getchar();
    while (c == ' ' || c == '\t' || c == '\n' || c == '\r') {
        c = getchar();
    }
    if (c == EOF) {
        return YYEOF;
    }
    if (isdigit(c)) {
        long long value = 0;
        while (isdigit(c)) {
            value = value * 10 + (c - '0');
            c = getchar();
        }
        ungetc(c, stdin);
        yylval = value;
        return INT;
    }
    /* Any other character is a token of its own: '+' is token number 43,
       its character code. Unknown ones like '$' make a syntax error. */
    return c;
}

void yyerror(const char *message) {
    fprintf(stderr, "error: %s\n", message);
}

int main(void) {
    yydebug = getenv("YYDEBUG") != NULL;
    return yyparse();
}
