# Part 5 — An intermediate representation

[← Step 2 overview](guide.md)

Step 1's code generator walked the AST and printed JavaScript directly.
Real compilers put at least one more layer in between: an
**intermediate representation** (IR), a form of the program designed
for the compiler rather than for the person who wrote it. rustc has
several (HIR, THIR, MIR) before handing LLVM its own, LLVM IR. This part designs
Mini's IR, translates the AST into it, and writes an interpreter for it.

**Rust you'll use:** [`HashMap`](rust-concepts.md#5-hashmap-and-hashset),
[deriving `Hash`, `PartialOrd`, `Ord`](rust-concepts.md#6-deriving-hash-partialord-ord),
[wrapping arithmetic](rust-concepts.md#7-wrapping-checked-and-overflowing-arithmetic),
[`fmt::Write` and `writeln!`](rust-concepts.md#11-writing-text-fmtwrite-and-writeln),
[`Vec` methods](rust-concepts.md#12-more-vec-methods).

---

## The idea: a flat list of simple steps

The AST is shaped like the source: nested, recursive, made for parsing.
A machine runs flat sequences of simple steps. The IR sits in between:

```text
x * (2 + 3) + y * 0            fn main(%x, %y) {
                                 %0 = add 2, 3
                                 %1 = mul %x, %0
                                 %2 = mul %y, 0
                                 %3 = add %1, %2
                                 ret %3
                               }
```

Two ideas shape it, and LLVM IR is built on the same two:

- **Three-address code.** An instruction applies one operator to at
  most two operands, and names its result: `%1 = mul %x, %0`. No
  nesting: every intermediate value has a name. (The name comes from
  the three "addresses": two operands and a result.)
- **SSA form**, static single assignment. Each temporary is assigned
  exactly once, so `%1` means the same value everywhere it appears. A
  pass that knows `%0` is `5` can replace every `%0` by `5` without
  checking whether something changed `%0` in between: nothing can.
  That makes optimizations simple and safe. Mini has no variables to
  reassign and no branches, so its IR is in SSA form without trying.
  (With `if`, a value can come from two places; SSA then needs φ
  ("phi") instructions. You'll meet one in LLVM's output in Part 8.)

Why bother, for a language this small?

- **One IR, many front ends and back ends.** Two front ends build the
  AST; lowering turns it into IR; three back ends (an interpreter,
  JavaScript, LLVM) read the IR. Without the IR, each back end would
  have to understand the AST, and each optimization would be written
  once per back end.
- **Optimizations are easier on flat SSA**: Part 6.
- **Code generation gets easier.** The step-1 code generator had to put
  parentheses back to keep the grouping. In three-address code, there's
  nothing to group: each instruction becomes one line of output.

### The types

Open `src/ir.rs` and read the types:

```rust
pub struct Function { pub inputs: Vec<String>, pub body: Vec<Inst>, pub result: Value }
pub struct Inst     { pub dest: Temp, pub op: Op, pub span: Span }
pub enum Op         { Neg(Value), Binary(BinOp, Value, Value) }
pub enum Value      { Const(i64), Input(usize), Temp(Temp) }
pub struct Temp(pub u32);
```

- A `Value` is an operand: what an instruction reads. Constants and
  inputs need no instruction.
- `Value::Input(1)` is the second input, whose name is `inputs[1]`.
- Each instruction has a `span`: the source it came from. Debug
  information, in a way: the interpreter uses it to point at the
  division that failed, long after the AST is gone.
- A temporary has its own number, instead of being "the result of the
  instruction at index 3". Part 6 deletes instructions; with indices,
  every reference after a deleted instruction would have to be
  renumbered.

The derives matter too: `Op` and `Value` are `Copy` (small, cheap to
copy, no ownership to think about), and `Hash` lets Part 6 use them as
`HashMap` keys.

---

## 5a. The IR printer

You'll read a lot of IR in Parts 6 to 8, so first, print it. Two
functions in `src/ir.rs`:

- `Function::show(value)`: how an operand is written. A constant as its
  number (`-7`), an input as `%` and its name (`%x`), a temporary as `%`
  and its number (`%3`). `Temp` already implements `Display` as `%3`.
- `fmt` in `impl fmt::Display for Function`: the whole function, in the
  format the doc comment shows. A header with the inputs, one indented
  line per instruction (`mnemonic(op)` gives `add`, `sub`, `mul`, `div`),
  the `ret` line, and `}`.

<details>
<summary>Hint 1: the header</summary>

Build a `Vec<String>` of `%x`, `%y` with `.iter().map(...).collect()`,
then `.join(", ")`. In a format string, `{{` prints `{`.

</details>

<details>
<summary>Hint 2: one line per instruction</summary>

`match inst.op` with two arms. `writeln!(f, "  {} = neg {}", inst.dest, self.show(a))?`
writes one line and returns early on an error.

</details>

```bash
cargo test -p mini --lib -- ir::tests::show ir::tests::display
```

<details>
<summary>Solution</summary>

```rust
pub fn show(&self, value: Value) -> String {
    match value {
        Value::Const(c) => c.to_string(),
        Value::Input(i) => format!("%{}", self.inputs[i]),
        Value::Temp(t) => t.to_string(),
    }
}
```

```rust
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let inputs: Vec<String> = self.inputs.iter().map(|name| format!("%{name}")).collect();
    writeln!(f, "fn main({}) {{", inputs.join(", "))?;
    for inst in &self.body {
        match inst.op {
            Op::Neg(a) => writeln!(f, "  {} = neg {}", inst.dest, self.show(a))?,
            Op::Binary(op, a, b) => writeln!(
                f,
                "  {} = {} {}, {}",
                inst.dest,
                mnemonic(op),
                self.show(a),
                self.show(b)
            )?,
        }
    }
    writeln!(f, "  ret {}", self.show(self.result))?;
    writeln!(f, "}}")
}
```

</details>

---

## 5b. Lowering

**Lowering** translates from a higher-level representation to a
lower-level one: here, the AST to the IR. Open `src/lower.rs`. `lower`
(provided) does it in two passes over the tree: first collect the
inputs, then emit the instructions.

### `collect_inputs`

Walk the tree and add each name to `names`, in the order of first
appearance, without duplicates: for `y * x + y`, that's `y`, then `x`.
That order is the order of the inputs on the command line.

<details>
<summary>Hint</summary>

Recursion with a `match` on `&expr.kind`, like your S-expression
printer. For a `Var`, `names.contains(name)` says whether it's already
there; `names.push(name.clone())` adds it. (A `Vec` is fine for a
handful of names. With thousands, you'd keep a `HashSet` alongside.)

</details>

### `Lowerer::lower_expr`

Emit the instructions that compute `expr`, and return the `Value` that
holds its result:

- a literal is just a `Value::Const`: no instruction;
- a name is a `Value::Input`: `self.input(name)` (provided) finds its
  index;
- negation and binary operations: lower the operands **first**, then
  emit one instruction with `self.emit(op, expr.span)` (provided), which
  returns the new temporary.

Lower the left operand before the right one. It matters: in
`x / 0 + y / 0`, the first division to fail must be the left one, as
in the source.

<details>
<summary>Hint</summary>

```rust
ExprKind::Binary { op, lhs, rhs } => {
    let lhs = self.lower_expr(lhs);
    let rhs = self.lower_expr(rhs);
    self.emit(Op::Binary(*op, lhs, rhs), expr.span)
}
```

This is a **post-order** walk: children before their parent. Compare
with your S-expression printer, which is pre-order: it prints the
operator before the children.

</details>

```bash
cargo test -p mini --lib lower
```

<details>
<summary>Solution</summary>

```rust
fn collect_inputs(expr: &Expr, names: &mut Vec<String>) {
    match &expr.kind {
        ExprKind::Int(_) => {}
        ExprKind::Var(name) => {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        ExprKind::Neg(operand) => collect_inputs(operand, names),
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_inputs(lhs, names);
            collect_inputs(rhs, names);
        }
    }
}
```

```rust
fn lower_expr(&mut self, expr: &Expr) -> Value {
    match &expr.kind {
        ExprKind::Int(value) => Value::Const(*value),
        ExprKind::Var(name) => self.input(name),
        ExprKind::Neg(operand) => {
            let operand = self.lower_expr(operand);
            self.emit(Op::Neg(operand), expr.span)
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let lhs = self.lower_expr(lhs);
            let rhs = self.lower_expr(rhs);
            self.emit(Op::Binary(*op, lhs, rhs), expr.span)
        }
    }
}
```

</details>

Now the IR layer works:

```bash
mini --emit=ir mini/programs/inputs.mini
```

```bash
mini --emit=ir mini/programs/arithmetic.mini
```

---

## 5c. Mini's arithmetic, in one place

`eval_binary(op, a, b)` in `src/ir.rs` computes one operation, with the
rules from [the meaning](guide.md#the-meaning): wrapping on overflow,
division rounding toward zero, and `None` for a division by zero.

This function *is* Mini's arithmetic. The interpreter calls it to run
programs; in Part 6, the constant folder calls it to compute at compile
time. Since both call the same function, a constant folded at compile
time always equals what the program would compute at runtime. Compilers
that get this wrong produce programs that behave differently with and
without optimization.

<details>
<summary>Hint</summary>

Rust's integers have a method for each policy: `a + b` panics on
overflow in debug builds, `a.wrapping_add(b)` wraps, `a.checked_add(b)`
returns `None`. `wrapping_div` exists too: it wraps `i64::MIN / -1` to
`i64::MIN`, but still panics on a zero divisor, so check that first.
A match guard does it: `BinOp::Div if b == 0 => None`.

</details>

```bash
cargo test -p mini --lib eval_binary
```

<details>
<summary>Solution</summary>

```rust
pub fn eval_binary(op: BinOp, a: i64, b: i64) -> Option<i64> {
    match op {
        BinOp::Add => Some(a.wrapping_add(b)),
        BinOp::Sub => Some(a.wrapping_sub(b)),
        BinOp::Mul => Some(a.wrapping_mul(b)),
        BinOp::Div if b == 0 => None,
        BinOp::Div => Some(a.wrapping_div(b)),
    }
}
```

</details>

---

## 5d. The interpreter

Open `src/interp.rs`. The interpreter runs the IR directly, one
instruction after another, keeping the value of each temporary in a
`HashMap<Temp, i64>`. It's the simplest back end, and the most
important one: it **defines** what a program means. In Part 6, every
optimization is checked against it; in Part 7, every back end.

Two functions:

- `Machine::value(v)`: the current value of an operand. A constant is
  itself, an input is in `self.inputs`, a temporary is in `self.temps`.
- `run(function, inputs)`: create a `Machine`, run each instruction,
  store its result under its `dest`, and return the value of
  `function.result`. A division by zero returns
  `Err(Diagnostic::new("division by zero", inst.span))`.

<details>
<summary>Hint 1: <code>value</code></summary>

`self.temps[&t]` looks up a key, and panics if it's missing. Here a
missing temporary would be a bug in the compiler (SSA guarantees every
temporary is computed before it's used), so a panic is right.

</details>

<details>
<summary>Hint 2: <code>run</code></summary>

```rust
let mut machine = Machine { inputs, temps: HashMap::new() };
for inst in &function.body {
    let result = match inst.op {
        Op::Neg(a) => /* wrapping negation */,
        Op::Binary(op, a, b) => /* eval_binary, or return the error */,
    };
    machine.temps.insert(inst.dest, result);
}
Ok(machine.value(function.result))
```

`assert_eq!(inputs.len(), function.inputs.len(), "one value per input")`
at the start documents the contract: the caller (`main.rs`) checks the
number of inputs before calling.

</details>

```bash
cargo test -p mini --lib interp
```

<details>
<summary>Solution</summary>

```rust
pub fn run(function: &Function, inputs: &[i64]) -> Result<i64, Diagnostic> {
    assert_eq!(inputs.len(), function.inputs.len(), "one value per input");
    let mut machine = Machine {
        inputs,
        temps: HashMap::new(),
    };
    for inst in &function.body {
        let result = match inst.op {
            Op::Neg(a) => machine.value(a).wrapping_neg(),
            Op::Binary(op, a, b) => match eval_binary(op, machine.value(a), machine.value(b)) {
                Some(result) => result,
                None => return Err(Diagnostic::new("division by zero", inst.span)),
            },
        };
        machine.temps.insert(inst.dest, result);
    }
    Ok(machine.value(function.result))
}
```

```rust
fn value(&self, value: Value) -> i64 {
    match value {
        Value::Const(c) => c,
        Value::Input(i) => self.inputs[i],
        Value::Temp(t) => self.temps[&t],
    }
}
```

A real interpreter would use a `Vec` indexed by temporary number
instead of a `HashMap`: faster, since temporaries are small dense
integers. The `HashMap` keeps working when Part 6 leaves gaps in the
numbering.

</details>

---

## Check

Mini programs run now:

```bash
mini run mini/programs/inputs.mini 4 100
mini run mini/programs/division.mini 10 3
mini run mini/programs/division.mini 10 0
```

```text
20
3
error: division by zero
 --> mini/programs/division.mini:1:1
  |
1 | total / count
  | ^^^^^^^^^^^^^
```

The runtime error points at the source, thanks to the span carried by
the instruction. A negative input works too, since everything after the
file name is an input: `mini run mini/programs/division.mini -7 2`.

One property test checks lowering and the interpreter together,
against a separate evaluator that works directly on the AST
(`evaluate` in `tests/properties.rs`), on random programs and inputs:

```bash
cargo test -p mini --test properties the_ir_means_what_the_ast_means
```

---

**Next:** [Part 6 — An optimizer](part6-optimizer.md)
