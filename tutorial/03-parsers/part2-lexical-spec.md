# Part 2 — The lexical grammar

[← Step 3 overview](guide.md)

You write section 2 of `mini-parser/spec/maxim.md`: what Maxim's
tokens are, character by character, and the rules that need more than
characters to decide.

After each task:

```bash
cargo check-grammar lexical
```

A `✗` names the rule and the sample it gets wrong: `FLOAT must not
match "1."` means your `FLOAT` matches `1.` and shouldn't.

---

## The idea: tokens are regular

Almost every token of Maxim can be described without nesting: a name
is a letter then letters and digits, a line comment runs to the end of
the line. Languages like that are **regular**: a regular expression
describes them, and a lexer recognizes them reading each character
once, without a stack. Each of these token rules could be expanded
into one big regular expression.

One token isn't regular: the block comment, because block comments
nest. No regular expression can check that every `/*` has its `*/`,
any more than it can check parentheses: that takes counting. So
`BLOCK_COMMENT` will refer to itself, the only lexical rule that does.
A lexer still reads it in one pass, with a counter instead of a stack
(Part 4). The checker allows a lexical rule to refer to itself, but not
before reading a character: a rule that starts with itself, like
`A ::= A "x" | "x"`, is **left-recursive**, and a lexer that followed
it would call itself forever without moving.

Two rules connect the token rules to a lexer that reads a text from
start to end:

- **Longest match** (also called **maximal munch**): at each position,
  the lexer takes the longest token it can. `->` is one token, not `-`
  then `>`; `iffy` is a name, not the keyword `if` then `fy`.
- **Priority**: when two rules match the same longest text, the first
  listed wins. That's how a classic lexer generator (lex, flex)
  separates keywords from names: list the keywords first. Maxim's spec
  doesn't need it, thanks to the `-` operator, as you'll see.

Write these two rules as the first paragraph of section 2 (replace the
TODO comment there). Read the solution once you have.

<details>
<summary>Solution</summary>

```markdown
A program is a text in UTF-8. The lexer reads it from start to end and
cuts it into tokens, each time taking the **longest** token that
matches at the current position: `->` is one token, not `-` then `>`.
When two token rules match the same longest text, the one listed first
in this section wins.
```

</details>

---

## 2a. Whitespace and comments

Define `WHITESPACE` and `COMMENT` in section 2.1.

- **Whitespace** is one or more spaces, tabs and line breaks. A line
  break is `\n`, or `\r\n` (Windows). A `\r` alone is an error.
- **Comments** are `// …` to the end of the line (the line break isn't
  part of the comment), and `/* … */`, possibly on several lines.
- **Block comments nest**, as in Rust: inside one, `/*` opens a nested
  comment, and `*/` closes the innermost. `/* a /* b */ c */` is one
  comment; `/* a /* b */` isn't finished.

Characters are written `#x9` (a tab), `#xA` (`\n`), `#xD` (`\r`). Write
helper rules if you like, `LINE_COMMENT` for example: any `UPPER_CASE`
name is a lexical rule.

Start without nesting if you like: a comment that ends at the first
`*/`. The checker will tell you which nested samples fail, and the
rule only grows from there.

<details>
<summary>Hint 1: the trap in block comments</summary>

A rule like `"/*" ANY* "*/"`, with `ANY` any character, matches too
much: `/* a */ */` would be one comment, because `ANY*` can contain
`*/`. The checker tests exactly that. The text inside must not contain
`*/`, and with nesting, not `/*` either: those start and end comments.

The W3C way to say it, with `-`: "anything, except anything that
contains `/*` or `*/`". The XML spec writes its comments like that.
"Anything" is the class of every character, `[#x0-#x10FFFF]`.

</details>

<details>
<summary>Hint 2: nesting is recursion</summary>

A block comment is `/*`, then texts and block comments, then `*/`. So
`BLOCK_COMMENT` uses itself, as `Expr` will in the syntactic grammar.
With `COMMENT_TEXT` the text from Hint 1, a first attempt:

```ebnf
BLOCK_COMMENT ::= "/*" ( COMMENT_TEXT | BLOCK_COMMENT )* "*/"
```

</details>

<details>
<summary>Hint 3: two texts in a row</summary>

The first attempt still matches `/* a */ */`. `( COMMENT_TEXT |
BLOCK_COMMENT )*` allows two texts in a row: ` a *` then `/ `. Neither
contains `*/`, but together they do.

Texts and comments must alternate: `/*`, a text, then any number of
(a comment, a text), then `*/`. A text can be empty, so `/*/**/*/`
still works.

</details>

<details>
<summary>Hint 4: where a text meets a delimiter</summary>

Two samples are left, `/* a*/* b */ */` and `/* a/*/`. A lexer reads
from left to right, so in `*/*` the `*/` comes first and closes the
comment, and in `/*/` the `/*` comes first and opens one. Your rule
lets the text end with the first character of the delimiter after it:
` a*` then `/* b */`.

So the text before a nested comment can't end with `*`, and the text
before the final `*/` can't end with `/`. Two more rules, each a
`COMMENT_TEXT` minus what it can't end with.

</details>

<details>
<summary>Solution</summary>

```ebnf
WHITESPACE    ::= ( " " | #x9 | #xA | #xD #xA )+
COMMENT       ::= LINE_COMMENT | BLOCK_COMMENT
LINE_COMMENT  ::= "//" [^#xA#xD]*
BLOCK_COMMENT ::= "/*" ( TEXT_BEFORE_OPEN BLOCK_COMMENT )* TEXT_BEFORE_CLOSE "*/"
ANY           ::= [#x0-#x10FFFF]
COMMENT_TEXT  ::= ANY* - ( ANY* ( "/*" | "*/" ) ANY* )
TEXT_BEFORE_OPEN  ::= COMMENT_TEXT - ( ANY* "*" )
TEXT_BEFORE_CLOSE ::= COMMENT_TEXT - ( ANY* "/" )
```

`( TEXT_BEFORE_OPEN BLOCK_COMMENT )* TEXT_BEFORE_CLOSE` is the
alternation of Hint 3, written as pairs (a text, then a comment) and a
last text. The parentheses in `ANY* - ( ANY* "*" )` matter: `-` binds
tighter than juxtaposition.

Then add the prose:

```markdown
Whitespace and comments separate tokens, and are otherwise ignored.
```

before the grammar, and after it:

```markdown
Lines end with `\n` or `\r\n`; a `\r` alone is an error.

Block comments nest, as in Rust: `/* a /* b */ c */` is one comment.
Inside one, `/*` opens a nested comment and `*/` closes the innermost,
reading from left to right: in `*/*`, the `*/` closes first. The text
between them contains neither, and can't end with the `*` of a `*/` or
the `/` of a `/*` (the two `TEXT_BEFORE_` rules). `BLOCK_COMMENT` is the
only lexical rule that refers to itself: no regular expression can
count nesting.
```

Without nesting, the comment rule could be a regular expression, and
it's worth seeing once: `"/*" ( [^*] | "*"+ [^*/] )* "*"+ "/"` reads
"characters that aren't `*`, or runs of stars not followed by `/`;
then stars, then `/`". It's what most lexers did before nesting became
common (C, Java, JavaScript don't nest; Rust, Swift, Scala and Haskell
do).

</details>

---

## 2b. Names and keywords

Define `KEYWORD`, `LOWER` and `UPPER` in section 2.2.

- The keywords: `module import type fn cast component let match if
  else`.
- A **lowercase name** starts with a lowercase ASCII letter or `_`,
  then has ASCII letters, digits and `_`. It isn't a keyword, and it
  isn't `_` alone (that's the wildcard, a token of its own). `_tmp` is
  a name.
- A **capitalized name** starts with an uppercase ASCII letter, then
  the same characters.

<details>
<summary>Hint 1: no keywords</summary>

`A - B`: what `A` matches, but not `B`. `B` can be an alternative:
`( KEYWORD | "_" )`.

</details>

<details>
<summary>Hint 2: precedence</summary>

`-` binds tighter than juxtaposition: `[a-z_] IDENT_CHAR* - KEYWORD`
means `[a-z_] ( IDENT_CHAR* - KEYWORD )`, which excludes nothing
useful (`fn` is `f` then `n`, and `n` isn't a keyword). The checker
will tell you `LOWER must not match "fn"`. Put the sequence in
parentheses.

</details>

<details>
<summary>Solution</summary>

```ebnf
KEYWORD    ::= "module" | "import" | "type" | "fn" | "cast" | "component"
             | "let" | "match" | "if" | "else"
IDENT_CHAR ::= [a-zA-Z0-9_]
LOWER      ::= ( [a-z_] IDENT_CHAR* ) - ( KEYWORD | "_" )
UPPER      ::= [A-Z] IDENT_CHAR*
```

```markdown
A `LOWER` name is a variable, a function, a field or a type variable.
An `UPPER` name is a type, a constructor or a module. `_` alone is not a
name but a token of its own.
```

Thanks to `-`, the spec says what a name is with no priority rule. A
lexer still needs one: it reads `iffy` and must decide between a name
and a keyword. Part 5 does it the usual way: read the longest name,
then look it up in the keyword list.

</details>

---

## 2c. Numbers

Define `INT` and `FLOAT` in section 2.3.

- An **integer** is digits, with `_` allowed after the first digit as
  a separator: `1_000_000`, even `1_`.
- A **float** is an integer, a `.`, and an integer: `3.14`. Not `1.`,
  not `.5`, no exponents (`1e5`).
- No sign: `-1` is the operator `-` applied to `1`. If `-1` were one
  token, `a-1` would be two values side by side.

<details>
<summary>Solution</summary>

```ebnf
DIGITS ::= [0-9] [0-9_]*
INT    ::= DIGITS
FLOAT  ::= DIGITS "." DIGITS
```

```markdown
`_` separates digits for readability: `1_000_000`. There are no
negative literals: `-1` is the operator `-` applied to `1`. `1.` and
`.5` aren't numbers.
```

</details>

---

## 2d. Strings

Define the four string tokens in section 2.4: `STRING`, `STRING_START`,
`STRING_MIDDLE` and `STRING_END`.

### The idea: a string with holes is several tokens

`"${count} items"` contains an expression, `count`, and the expression
can be anything: `"${List.length(xs) * 2} halves"`, even another
string, `"${"inner"}"`. Expressions nest; tokens can't. So the lexer
cuts the string at its holes, and the parser parses the expression in
between like any other:

```text
"Hello, ${name}, you have ${n + 1} messages"

STRING_START    "Hello, ${
LOWER           name
STRING_MIDDLE   }, you have ${
LOWER           n
"+"             +
INT             1
STRING_END      } messages"
```

JavaScript does the same with its template literals; its spec calls
the pieces `TemplateHead`, `TemplateMiddle` and `TemplateTail`. A
string without holes is one `STRING` token.

### The rules

- A string is **on one line**, between double quotes.
- **Escapes**: `\n`, `\t`, `\"`, `\\`, `\$`, and `\u{1F389}` with 1 to
  6 hexadecimal digits.
- A **hole** opens with `${`. A `$` followed by anything else is just a
  `$` (`"costs $5"`), and `\$` writes a `$` that would open a hole.

<details>
<summary>Hint 1: start simple</summary>

Write `STRING_TEXT`, what's between the delimiters, as "characters that
aren't `"`, `\` or a line break, or escapes". Then
`STRING ::= '"' STRING_TEXT '"'`, and the three others the same way
with `}` and `${` as delimiters. The checker will then say that
`STRING` must not match `"${x}"`: your text allows `${`.

</details>

<details>
<summary>Hint 2: no `${` inside</summary>

The same trick as for block comments: the text, except texts that
contain `${` somewhere. With `X` for "a character or an escape":
`X* - ( X* "${" X* )`. Write `X` out, or as a helper rule.

Why does `\${` survive? Because `\$` is one escape: the text `\${`
can't be cut as "`X*`, then `${`".

</details>

<details>
<summary>Solution</summary>

Before the grammar, what a string is:

```markdown
A string is on one line, between double quotes. A `${` opens a *hole*:
an expression whose value is inserted in the string. The hole ends at
the `}` that matches its `${`.
```

```ebnf
ESCAPE        ::= "\" ( [nt"\$] | "u{" HEX HEX? HEX? HEX? HEX? HEX? "}" )
HEX           ::= [0-9a-fA-F]
STRING_TEXT   ::= ( [^"\#xA#xD] | ESCAPE )*
                - ( ( [^"\#xA#xD] | ESCAPE )* "${" ( [^"\#xA#xD] | ESCAPE )* )
STRING        ::= '"' STRING_TEXT '"'
STRING_START  ::= '"' STRING_TEXT "${"
STRING_MIDDLE ::= "}" STRING_TEXT "${"
STRING_END    ::= "}" STRING_TEXT '"'
```

In W3C EBNF, a backslash is an ordinary character: `"\"` is a
backslash, and `[^"\#xA#xD]` is "not a quote, a backslash or a line
break".

The rules accept `\u{D800}`, a surrogate, which isn't a valid
character. A grammar can say it, but painfully; specs say it in prose:

```markdown
`"Hello, ${name}!"` is three tokens around the hole's: `STRING_START`
(`"Hello, ${`), `LOWER` (`name`), and `STRING_END` (`}!"`). A string
with two holes has a `STRING_MIDDLE` between them.

A `$` not followed by `{` is just a `$`; `\$` escapes one that is. In
`\u{…}`, the digits must name a Unicode scalar value (not a surrogate,
at most `10FFFF`).
```

</details>

---

## 2e. Punctuation

Define `PUNCTUATION` in section 2.5: every operator and bracket of the
tour in Part 1, and `=>`. Nothing checks this rule against samples:
the syntactic grammar will use the punctuation by quoting it.

<details>
<summary>Solution</summary>

```ebnf
PUNCTUATION ::= "(" | ")" | "{" | "}" | "[" | "]" | "<" | ">" | "," | ":" | "::"
              | "." | ".." | "=" | "==" | "!=" | "<=" | ">=" | "|" | "||" | "|>"
              | "&&" | "!" | "->" | "=>" | "?" | "+" | "++" | "-" | "*" | "**"
              | "/" | "~/" | "%"
```

```markdown
`=>` is not used by the syntax: it's there so that the old lambda
syntax `(x) => x` gets a helpful error.
```

`&` and `~` alone aren't tokens: `a & b` is an error.

</details>

---

## 2f. The rules that need context

Three things the lexer decides can't be written as regular
expressions, because they depend on what came before. Describe them in
prose in section 2.6. Real specs do the same: Go's spec explains in
prose where the lexer inserts semicolons, JavaScript's when a `/`
starts a regular expression.

1. **The `}` that closes a hole.** In `"${ f({ a }) }"`, which `}`
   ends the hole and starts the `STRING_END`? The lexer has to count
   the braces opened inside the hole. And a hole must fit on one line
   (the string does).
2. **`NEWLINE`.** The line break rule of Part 1, Scala's: a line break
   ends a statement when the token before it can end one and the token
   after it can start one. The syntactic grammar will use a `NEWLINE`
   token to separate statements, match arms and declarations. Say:
   - which tokens can end a statement: the same as the ones that can
     end an expression, in rule 3 below;
   - which tokens can start a statement, a match arm or a declaration:
     what an expression, a pattern, `let` or a declaration can start
     with;
   - the exception: inside `( … )` and `[ … ]`, line breaks are
     whitespace, so arguments and list items can go on several lines.
     But a lambda's body is a block inside the parentheses, and its
     statements still need their line breaks;
   - what several line breaks in a row make, and a block comment that
     spans lines.
3. **HTML.** In the full language, `<div>` starts an HTML element
   where an expression can start, and is a comparison elsewhere:
   `a <b` compares. An expression can start after a token that can't
   end one (`=`, `(`, `,`, an operator…), and at the start of a line.
   This spec leaves HTML out, so the rule says where it's an error.
   List the tokens that can end an expression: the name tokens, the
   literals, `_`, the closing brackets and `?`.

<details>
<summary>Solution</summary>

```markdown
Three rules can't be written in the lexical grammar, because they
depend on what comes before a token.

1. **A `}` that closes a hole** isn't a `"}"` token: it starts a
   `STRING_MIDDLE` or a `STRING_END`. Inside a hole, braces nest, so in
   `"${ { a } }"` the first `}` is a `"}"` token and the second closes
   the hole. A hole must fit on one line: its whitespace and comments
   can't contain a line break.
2. **A line break is a `NEWLINE` token**, which separates statements,
   match arms and declarations, when three things hold (the rule of
   the Scala language):
   - the token before it can end a statement: `LOWER`, `UPPER`, `"_"`,
     `INT`, `FLOAT`, `STRING`, `STRING_END`, `")"`, `"]"`, `"}"` or
     `"?"`;
   - the token after it can start a statement, a match arm or a
     declaration: `LOWER`, `UPPER`, `"_"`, `INT`, `FLOAT`, `STRING`,
     `STRING_START`, `"("`, `"["`, `"{"`, `"-"`, `"!"`, `"fn"`, `"if"`,
     `"match"`, `"let"`, `"import"`, `"type"`, `"cast"` or
     `"component"`;
   - it isn't inside parentheses or brackets, unless it's inside braces
     inside them: in `f(x,` newline `y)`, the line break is whitespace,
     but in `f(fn() {` newline `a` newline `b })`, the second is a
     `NEWLINE`.

   Otherwise it's whitespace. Several line breaks in a row, with
   whitespace and comments between them, make one `NEWLINE`, and a
   block comment that spans lines counts as a line break. So a line
   that ends with an operator continues on the next one, and so does a
   line that starts with `|>`, `.`, `|` or `else`; but a line that
   starts with `-` or `(` starts something new.
3. **A `<` followed by a letter**, where an expression can start, opens
   an HTML element, which this specification leaves out: it's an
   error. An expression can start after a token that can't end one,
   and at the start of a line. The tokens that can end an expression
   are `LOWER`, `UPPER`, `"_"`, `INT`, `FLOAT`, `STRING`, `STRING_END`,
   `")"`, `"]"`, `"}"` and `"?"`. So `a <b` compares, but
   `let x = <b` and a line starting with `<b` are HTML.
```

Rule 3 is the same problem as JavaScript's `/`: one token of context
(the previous one) decides. Rule 2 needs more: the tokens on both
sides, and a stack of open brackets. Go's rule is simpler, it only
looks at the token before the line break; that's why in Go,
`x` newline `+ y` is an error rather than a continuation. Scala looks
at the next token too, so a line can start with `.` or `|>`.

Part 5 implements rules 1 and 3 in the lexer, and Part 7 rule 2, where
the parser reads the tokens.

</details>

<details>
<summary>Why not keep the line breaks and let the grammar decide?</summary>

The syntactic grammar could take line breaks as tokens and allow them
everywhere they're allowed, but "everywhere" is almost everywhere:
between `+` and its right operand, after `(`, after `,`, before `|>`…
Every rule would be twice as long. Splitting the decision out keeps
the grammar readable, as splitting out whitespace did. Scala, Kotlin
and Swift all do it this way.

</details>

---

## Check

```bash
cargo check-grammar lexical
```

```text
Lexical grammar (section 2)
  ✓ WHITESPACE: 10 samples
  ✓ COMMENT: 24 samples
  …
  ✓ STRING_END: 6 samples
```

The other sections still fail: that's Part 3.

---

**Next:** [Part 3 — The syntactic grammar](part3-syntax-spec.md)
