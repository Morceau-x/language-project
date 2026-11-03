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

Every token of Maxim can be described without nesting: a name is a
letter then letters and digits, a comment runs to the end of the line.
Languages like that are **regular**: a regular expression describes
them, and a lexer recognizes them reading each character once, without
a stack. That's why the lexical grammar never refers to itself: each
token rule can be expanded into one big regular expression. (The
checker rejects lexical rules that refer to themselves.)

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
  Block comments don't nest: `/* a /* b */` is one comment.

Characters are written `#x9` (a tab), `#xA` (`\n`), `#xD` (`\r`). Write
helper rules if you like, `LINE_COMMENT` for example: any `UPPER_CASE`
name is a lexical rule.

<details>
<summary>Hint 1: the trap in block comments</summary>

A rule like `"/*" ANY* "*/"`, with `ANY` any character, matches too
much: `/* a */ */` would be one comment, because `ANY*` can contain
`*/`. The checker tests exactly that. The comment must end at the
*first* `*/`.

</details>

<details>
<summary>Hint 2: two ways to forbid `*/` inside</summary>

The regular expression way: inside, a character that isn't `*`, or
stars followed by a character that isn't `*` or `/`; at the end, stars
then `/`.

The W3C way, with `-`: "anything, except anything that contains `*/`".
The XML spec writes its comments like that. "Anything" is the class of
every character, `[#x0-#x10FFFF]`.

</details>

<details>
<summary>Solution</summary>

```ebnf
WHITESPACE    ::= ( " " | #x9 | #xA | #xD #xA )+
COMMENT       ::= LINE_COMMENT | BLOCK_COMMENT
LINE_COMMENT  ::= "//" [^#xA#xD]*
BLOCK_COMMENT ::= "/*" ( [^*] | "*"+ [^*/] )* "*"+ "/"
```

`( [^*] | "*"+ [^*/] )*` reads: characters that aren't `*`, or runs of
stars that aren't followed by `/`. Then the end: stars, then `/`.

The W3C way works too:

```ebnf
ANY           ::= [#x0-#x10FFFF]
BLOCK_COMMENT ::= "/*" ( ANY* - ( ANY* "*/" ANY* ) ) "*/"
```

Then add the prose:

```markdown
Lines end with `\n` or `\r\n`; a `\r` alone is an error. Block
comments don't nest: the first `*/` ends one.
```

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
2. **`BOL_LPAREN`.** The layout rule of Part 1: a call's `(` must be on
   the same line as what it calls. The parser needs to know which `(`
   start a line; the lexer tells it with a different token. "A line
   break in the whitespace and comments just before it" is the precise
   condition: a block comment can contain one too.
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
2. **A `(` that starts a line**, that is, with a line break in the
   whitespace and comments just before it, is the token `BOL_LPAREN`
   ("beginning of line"), not `"("`. The syntax uses the difference: a
   call's `(` must be on the same line as what it calls.
3. **A `<` followed by a letter**, where an expression can start, opens
   an HTML element, which this specification leaves out: it's an
   error. An expression can start after a token that can't end one,
   and at the start of a line. The tokens that can end an expression
   are `LOWER`, `UPPER`, `"_"`, `INT`, `FLOAT`, `STRING`, `STRING_END`,
   `")"`, `"]"`, `"}"` and `"?"`. So `a <b` compares, but
   `let x = <b` and a line starting with `<b` are HTML.
```

Rule 3 is the same problem as JavaScript's `/`: one token of context
(the previous one) decides. Part 5 implements all three.

</details>

---

## Check

```bash
cargo check-grammar lexical
```

```text
Lexical grammar (section 2)
  ✓ WHITESPACE: 10 samples
  ✓ COMMENT: 14 samples
  …
  ✓ STRING_END: 6 samples
```

The other sections still fail: that's Part 3.

---

**Next:** [Part 3 — The syntactic grammar](part3-syntax-spec.md)
