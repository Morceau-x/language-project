# Part 7 — Backends: JavaScript and LLVM

[← Step 2 overview](guide.md)

A **backend** turns the IR into something another tool runs. Mini gets
two: JavaScript again, this time from the IR, and **LLVM IR**, which
clang turns into a native executable. Both are short, because the IR
already did the hard part.

**Rust you'll use:** [`fmt::Write` and `writeln!`](rust-concepts.md#11-writing-text-fmtwrite-and-writeln),
[`HashMap`](rust-concepts.md#5-hashmap-and-hashset),
[running programs and writing files](rust-concepts.md#13-running-other-programs-stdprocesscommand)
(in the provided code).

---

## 7a. JavaScript, from the IR

Step 1's code generator printed nested JavaScript expressions, with
parentheses everywhere to keep the grouping. From three-address code,
each instruction becomes one `const`:

```js
function main($x, $y) {
  const $0 = wrap(2n + 3n);
  const $1 = wrap($x * $0);
  const $2 = wrap($y * 0n);
  const $3 = wrap($1 + $2);
  return $3;
}
```

No grouping to get right: every intermediate value has a name. (Node's
JIT compiler sees through the names: they cost nothing at runtime.)

Two other things changed since step 1.

**Exact integers.** A JavaScript `Number` is a 64-bit float, exact only
up to 2^53. `9007199254740993` silently becomes `9007199254740992` (step
1's stretch exercise 5). `BigInt` is exact at any size: `2n`, `$x * 5n`.
Mini's integers wrap at 64 bits, so each result goes through
`wrap(n) = BigInt.asIntN(64, n)`. And BigInt division already rounds
toward zero, so `Math.trunc` is gone: the `div` helper only checks for
zero.

**Name mangling.** A Mini input could be called `new`, `wrap` or
`main`. Printed as is, `function main(new)` is a syntax error, and an
input called `wrap` would hide the helper. So every generated name
starts with `$`, a character Mini names can't contain: `$new` can't
clash with anything. Compilers that target C or JavaScript all do some
version of this; C++ compilers do it to every function name
([try `c++filt`](https://en.wikipedia.org/wiki/Name_mangling)).

Open `src/backend/js.rs` and read `PRELUDE`, `emit_main` and
`emit_epilogue`: the helpers, the function itself, and the code that
reads the inputs from the command line. Then write the two functions in
between:

- `value(function, v)`: `5n` for a constant (`(-5n)` if negative: the
  parentheses keep `-` followed by a negative number from printing as
  `--`, the step-1 trap), `$x` for an input, `$3` for a temporary.
- `emit_inst(function, inst, out)`: one line per instruction:

  | Instruction        | JavaScript                        |
  |--------------------|-----------------------------------|
  | `%3 = neg a`       | `  const $3 = wrap(-a);`          |
  | `%3 = add a, b`    | `  const $3 = wrap(a + b);`       |
  | (same for `sub`, `mul`) |                              |
  | `%3 = div a, b`    | `  const $3 = div(a, b);`         |

<details>
<summary>Hint</summary>

`op.symbol()` (from step 1's `ast.rs`) gives `+`, `-`, `*`. Match the
division first, like step 1's code generator did:
`Op::Binary(BinOp::Div, a, b) => ...`, then `Op::Binary(op, a, b) => ...`.
`writeln!(out, ...)` returns the `fmt::Result` the function needs.

</details>

```bash
cargo test -p mini --lib backend::js
```

<details>
<summary>Solution</summary>

```rust
fn emit_inst(function: &Function, inst: &Inst, out: &mut String) -> fmt::Result {
    let dest = format!("${}", inst.dest.0);
    match inst.op {
        Op::Neg(a) => writeln!(out, "  const {dest} = wrap(-{});", value(function, a)),
        Op::Binary(BinOp::Div, a, b) => writeln!(
            out,
            "  const {dest} = div({}, {});",
            value(function, a),
            value(function, b)
        ),
        Op::Binary(op, a, b) => writeln!(
            out,
            "  const {dest} = wrap({} {} {});",
            value(function, a),
            op.symbol(),
            value(function, b)
        ),
    }
}
```

```rust
fn value(function: &Function, value: Value) -> String {
    match value {
        // Negative constants get parentheses, so that `-` followed by
        // one can never print as `--`, JavaScript's decrement operator.
        Value::Const(c) if c < 0 => format!("({c}n)"),
        Value::Const(c) => format!("{c}n"),
        Value::Input(i) => format!("${}", function.inputs[i]),
        Value::Temp(t) => format!("${}", t.0),
    }
}
```

</details>

Run it: `mini` prints JavaScript by default, and `node -` reads the
program from its standard input, then takes the inputs:

```bash
mini -O mini/programs/inputs.mini | node - 4 100
mini mini/programs/division.mini | node - 10 0; echo "exit status: $?"
```

```text
20
error: division by zero
exit status: 1
```

---

## The idea: LLVM

**LLVM** is a compiler toolkit. Its front ends (clang for C and C++,
rustc, swiftc, Zig, Julia…) translate their language to **LLVM IR**;
LLVM optimizes it and generates machine code for dozens of processors.
Writing an LLVM backend is how a small language gets a world-class
optimizer and code generator for free.

LLVM IR exists in three equivalent forms: in memory (inside LLVM),
binary (`.bc` files), and **text** (`.ll` files). Mini writes text. No
Rust bindings, no LLVM library to install: clang reads `.ll` files
like it reads `.c` files.

Here is what you'll generate for `x * (2 + 3) + y * 0`, before
optimization. It should look familiar:

```llvm
define i64 @mini_main(i64 %x, i64 %y) {
entry:
  %0 = add i64 2, 3
  %1 = mul i64 %x, %0
  %2 = mul i64 %y, 0
  %3 = add i64 %1, %2
  ret i64 %3
}
```

The basics, in the order you meet them:

- `define` a function, with its return type and parameters. `@` marks
  global names (functions, global constants), `%` local ones (values,
  parameters, labels).
- **Types are written everywhere.** `i64` is a 64-bit integer. It has no
  sign: the *instruction* decides how to read the bits. `sdiv` divides
  signed numbers, `udiv` unsigned ones; `add` is the same for both.
- A function body is made of **basic blocks**: a label (`entry:`), then
  instructions, ending with a **terminator** that says where to go
  next: `ret`, a branch `br`, or `unreachable`. Mini's functions have a
  single block; `@mini_div` (below) has five.
- It's **SSA**, like your IR: each `%` value is assigned once.
- `%x` and `%is_zero` are *named* values: any name you like. `%0`, `%1`
  are *numbered* values, and they must be numbered in order, with no
  gaps. (Unnamed blocks count too, which is why `entry:` is named.)

That last rule is the only real difference from your IR. After dead
code elimination, your temporaries have gaps: `%1`, then `%7`. LLVM
would reject `%7` right after `%1`. So the backend gives each temporary
the next free number, its **slot**, as it writes the instruction, and
uses that slot from then on.

### Undefined behaviour

Open `src/backend/llvm.rs` and read `PRELUDE`. It defines `@mini_div`,
and every Mini division calls it. Why not use LLVM's `sdiv` directly?

Because `sdiv` has **undefined behaviour** (UB) when the divisor is 0,
and for `i64::MIN / -1`. Undefined doesn't mean "crashes": it means
LLVM's optimizer may **assume it never happens**, and transform the
program on that basis. Mini defines both cases (an error, and wrapping),
so it must check before LLVM sees an `sdiv`. Part 8 shows what happens
otherwise.

`@mini_div` is a small function with five basic blocks: compare
(`icmp eq`), branch (`br i1 %is_zero, label %zero, label %nonzero`),
call the runtime's error function, and tell LLVM that the call never
returns (`unreachable`). Read it until each line makes sense.

The same reasoning explains `add`: LLVM's plain `add` wraps around on
overflow, exactly like Mini. `add nsw` ("no signed wrap") would be UB
on overflow instead. That's what clang emits for C's signed
arithmetic, and why signed overflow is UB in C.

---

## 7b. LLVM IR, from the IR

Two functions in `src/backend/llvm.rs`:

- `value(function, slots, v)`: a constant as its number (`5`, `-5`: no
  `n`, no parentheses), an input as `%x`, a temporary as `%` and its
  slot: `slots[&t]`.
- `emit_main(function, out)`: the function, as in the example above:

  | Instruction       | LLVM                                          |
  |-------------------|-----------------------------------------------|
  | `%7 = neg a`      | `  %N = sub i64 0, a`                         |
  | `%7 = add a, b`   | `  %N = add i64 a, b` (`instruction_name(op)` for `sub`, `mul`) |
  | `%7 = div a, b`   | `  %N = call i64 @mini_div(i64 a, i64 b)`     |
  | `ret v`           | `  ret i64 v`                                 |

  where `N` is the instruction's slot: the number of instructions
  written before it. LLVM has no negation instruction: `0 - a` is how
  clang writes it too.

<details>
<summary>Hint 1: the shape</summary>

```rust
let params: Vec<String> = /* "i64 %x", "i64 %y" */;
writeln!(out, "define i64 @mini_main({}) {{", params.join(", "))?;
writeln!(out, "entry:")?;
let mut slots: HashMap<Temp, usize> = HashMap::new();
for inst in &function.body {
    let slot = slots.len();
    // build the instruction's text with format!, write "  %{slot} = ..."
    slots.insert(inst.dest, slot);
}
// the ret line, then "}"
```

</details>

<details>
<summary>Hint 2: order inside the loop</summary>

Insert the slot **after** formatting the operands: an instruction
never uses its own result, so it doesn't matter here, but it's the
natural order (operands are defined before the instruction).

</details>

```bash
cargo test -p mini --lib backend::llvm
```

<details>
<summary>Solution</summary>

```rust
fn emit_main(function: &Function, out: &mut String) -> fmt::Result {
    let params: Vec<String> = function
        .inputs
        .iter()
        .map(|name| format!("i64 %{name}"))
        .collect();
    writeln!(out, "define i64 @mini_main({}) {{", params.join(", "))?;
    writeln!(out, "entry:")?;
    let mut slots: HashMap<Temp, usize> = HashMap::new();
    for inst in &function.body {
        let slot = slots.len();
        let instruction = match inst.op {
            Op::Neg(a) => format!("sub i64 0, {}", value(function, &slots, a)),
            Op::Binary(BinOp::Div, a, b) => format!(
                "call i64 @mini_div(i64 {}, i64 {})",
                value(function, &slots, a),
                value(function, &slots, b)
            ),
            Op::Binary(op, a, b) => format!(
                "{} i64 {}, {}",
                instruction_name(op),
                value(function, &slots, a),
                value(function, &slots, b)
            ),
        };
        writeln!(out, "  %{slot} = {instruction}")?;
        slots.insert(inst.dest, slot);
    }
    writeln!(out, "  ret i64 {}", value(function, &slots, function.result))?;
    writeln!(out, "}}")
}
```

```rust
fn value(function: &Function, slots: &HashMap<Temp, usize>, value: Value) -> String {
    match value {
        Value::Const(c) => c.to_string(),
        Value::Input(i) => format!("%{}", function.inputs[i]),
        Value::Temp(t) => format!("%{}", slots[&t]),
    }
}
```

</details>

```bash
mini --emit=llvm mini/programs/inputs.mini
```

---

## The runtime, and linking

The LLVM module needs one more piece before it's a program: something
must read the inputs from the command line, call `@mini_main`, and
print the result. That code is the same for every Mini program, so it's
written once, in C: `runtime/mini_rt.c`. Read it. Every language has a
runtime like this: C has `crt0` and libc, Rust has the code in `std`
that runs before your `main`, Go has a whole runtime with a garbage
collector.

The runtime doesn't know how many inputs a program takes, so the
backend adds **glue** at the end of the module (`emit_glue`, provided;
look at the end of the `--emit=llvm` output):

- `@mini_input_count` and `@mini_input_names`: global constants the
  runtime reads to check the command line and print a usage message;
- `@mini_entry(ptr %inputs)`: takes the inputs as an array, loads each
  one (`getelementptr` computes the address of element `i`, `load`
  reads it), and calls `@mini_main` with them.

`mini build` does the rest (`build_native` in `src/driver.rs`): it
writes the `.ll` file and the runtime to a temporary folder, and runs

```bash
clang -O2 -Wno-override-module program.ll mini_rt.c -o OUTPUT
```

clang compiles the C runtime to LLVM IR, optimizes both modules,
generates machine code for each, and the **linker** joins them with the
C library (for `printf` and `strtoll`) into one executable. The
`-Wno-override-module` silences a warning: the `.ll` file doesn't say
which processor it's for, and clang fills in yours.

```bash
mini build -O mini/programs/division.mini -o target/division
./target/division 10 3
./target/division -9223372036854775808 -1
./target/division 10 0; echo "exit status: $?"
./target/division 10
```

```text
built target/division
3
-9223372036854775808
error: division by zero
exit status: 1
usage: ./target/division total count
```

A native executable, compiled from Mini through your IR, your
optimizer, your backend, and LLVM.

---

## Check: every road, the same answers

The main test in `tests/end_to_end.rs` runs a list of programs through
the interpreter, Node.js and native code, each with and without the
optimizer, and checks that they all agree, including on the division
that `dce` must not remove, the `x / x` whose value `fold` already
knows, and the overflows:

```bash
cargo test -p mini --test end_to_end
```

And now, everything:

```bash
cargo test -p mini --no-fail-fast
```

All 128 tests should pass.

---

**Next:** [Part 8 — Explore, and clean up](part8-explore.md)
