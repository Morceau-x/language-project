# Part 6 — An optimizer

[← Step 2 overview](guide.md)

An optimizer rewrites the IR into a program that computes **exactly the
same thing**, errors included, with less work. Mini's has three
classic passes, plus the machinery that runs them:

```text
x * (2 + 3) + y * 0             after -O
%0 = add 2, 3                   %1 = mul %x, 5
%1 = mul %x, %0                 ret %1
%2 = mul %y, 0
%3 = add %1, %2
ret %3
```

**Rust you'll use:** [traits](rust-concepts.md#8-traits-as-interfaces),
[trait objects](rust-concepts.md#9-trait-objects-boxdyn-trait),
[closures as arguments](rust-concepts.md#10-closures-fn-fnmut-fnonce),
[`HashMap` and `HashSet`](rust-concepts.md#5-hashmap-and-hashset),
[more patterns](rust-concepts.md#4-slice-patterns-and-more-patterns)
(`@` bindings, `|` in patterns, guards),
[property testing](rust-concepts.md#16-property-based-testing-with-proptest).

---

## The idea: small passes, run until nothing changes

Open `src/opt/mod.rs`. A **pass** is one small rewrite of the whole
function:

```rust
pub trait Pass {
    fn name(&self) -> &'static str;
    /// Rewrite `function`, and say whether anything changed.
    fn run(&self, function: &mut Function) -> bool;
}
```

Mini's three passes:

| Pass   | File      | Does                                                                      | Example                                  |
|--------|-----------|---------------------------------------------------------------------------|------------------------------------------|
| `fold` | `fold.rs` | **constant folding** and **algebraic simplification**                     | `add 2, 3` is `5`; `mul %x, 1` is `%x`  |
| `cse`  | `cse.rs`  | **common subexpression elimination**: reuse a value already computed      | the second `add %x, 1` is the first one |
| `dce`  | `dce.rs`  | **dead code elimination**: prune instructions whose result nobody uses    | `%0 = add 2, 3` once nothing reads `%0` |

Each pass does one thing, and leaves work for the others. `fold` and
`cse` never delete anything: they *replace uses*. When `fold` learns
that `%0` is `5`, it rewrites every operand `%0` into `5`, and leaves
`%0 = add 2, 3` where it is, now unused. Pruning it is `dce`'s job.

Because each pass can create work for another, the **pass manager**
runs them in a loop, until a whole round changes nothing: a
**fixpoint**. `(x + 1) - (x + 1)` needs two rounds: `cse` turns it into
`%0 - %0`, and only then can `fold` see that it's `0`.

LLVM is organized the same way, at a much larger scale: `clang -O2`
runs around a hundred passes, many of them several times. You'll watch
some of them in Part 8.

### "Replace all uses with"

Read `replace_uses` in `src/opt/mod.rs`. It's the engine behind `fold`
and `cse`:

```rust
fn replace_uses(
    function: &mut Function,
    mut replacement: impl FnMut(&Inst) -> Option<Value>,
) -> bool
```

It walks the instructions in order. For each one, it first rewrites the
operands with the replacements found so far, then calls `replacement`,
which says whether this instruction's result can be replaced by a
value. Since a temporary is always defined before it's used (SSA), one
walk is enough to follow chains: in `(2 + 3) * 4`, `%0` becomes `5`,
so `%1 = mul %0, 4` is seen as `mul 5, 4`, which becomes `20`.

`Fold` is then one line:

```rust
fn run(&self, function: &mut Function) -> bool {
    replace_uses(function, |inst| simplify(inst.op))
}
```

The closure is the pass's only knowledge; the walking and rewriting are
shared. LLVM calls this operation `replaceAllUsesWith`, and it's
everywhere in its source.

---

## 6a. `simplify`

Open `src/opt/fold.rs`. `simplify(op)` returns the value `op` computes,
when it can be known without running the program: a constant, or one of
the instruction's own operands. Otherwise `None`.

1. **Constant folding.** When all operands are constants, compute the
   result, with `eval_binary` (Part 5c): the folder must compute exactly
   what the program would have. `neg 5` is `-5`.
2. **Algebraic identities**, where `x` is any value:

   | Instruction                | Value |
   |----------------------------|-------|
   | `add x, 0` and `add 0, x`  | `x`   |
   | `sub x, 0`                 | `x`   |
   | `sub x, x`                 | `0`   |
   | `mul x, 1` and `mul 1, x`  | `x`   |
   | `mul x, 0` and `mul 0, x`  | `0`   |
   | `div x, 1`                 | `x`   |
   | `div x, x`                 | `1`   |
   | `div 0, x`                 | `0`   |

The last two rows look wrong: when `x` is 0, `div x, x` isn't `1`,
it's an error. (And `div 1, 0`, which `eval_binary` can't compute, has
no value at all: constant folding returns `None` for it.)

Here is why the last two rows are still right. `fold` only replaces the
*uses* of the division's result; the instruction itself stays, and
part 6d makes sure `dce` never removes an instruction that can fail. So
when `x` is 0, the division still runs and the program still fails,
before anything uses the `1`. When `x` isn't 0, the value is `1`. The
replacement only has to be right **when the division succeeds**. That's
the doc comment's point: the *value* of an instruction and its *effect*
(failing) are separate questions, answered by separate passes.

<details>
<summary>Hint 1: patterns with constants</summary>

Start the function with `use BinOp::{Add, Div, Mul, Sub};` and
`use Value::Const;`: then patterns can say `Add` and `Const(0)` instead
of `BinOp::Add` and `Value::Const(0)`. (A `use` inside a function only
applies there.) A pattern can match nested values directly:

```rust
Op::Binary(op, Const(a), Const(b)) => eval_binary(op, a, b).map(Const),
```

`.map(Const)` turns `Some(5)` into `Some(Const(5))`: a tuple variant
is also a function.

</details>

<details>
<summary>Hint 2: two patterns, one arm</summary>

`|` combines patterns that bind the same names:

```rust
Op::Binary(Add, x, Const(0)) | Op::Binary(Add, Const(0), x) => Some(x),
```

For `sub x, x`, a guard compares the two operands:
`Op::Binary(Sub, x, y) if x == y => ...`.

</details>

<details>
<summary>Hint 3: order</summary>

Arms are tried in order. Put constant folding first, so that `div 1, 0`
reaches the constant arm (which gives `None`) and never the `div 0, x`
or `div x, x` arms.

</details>

```bash
cargo test -p mini --lib simplify
```

<details>
<summary>Solution</summary>

```rust
pub fn simplify(op: Op) -> Option<Value> {
    use BinOp::{Add, Div, Mul, Sub};
    use Value::Const;
    match op {
        // Constant folding, with the same arithmetic as the interpreter.
        Op::Neg(Const(a)) => Some(Const(a.wrapping_neg())),
        Op::Binary(op, Const(a), Const(b)) => eval_binary(op, a, b).map(Const),
        // Algebraic identities.
        Op::Binary(Add, x, Const(0)) | Op::Binary(Add, Const(0), x) => Some(x),
        Op::Binary(Sub, x, Const(0)) => Some(x),
        Op::Binary(Sub, x, y) if x == y => Some(Const(0)),
        Op::Binary(Mul, x, Const(1)) | Op::Binary(Mul, Const(1), x) => Some(x),
        Op::Binary(Mul, _, Const(0)) | Op::Binary(Mul, Const(0), _) => Some(Const(0)),
        Op::Binary(Div, x, Const(1)) => Some(x),
        // Only right when the division succeeds, which is all we need.
        Op::Binary(Div, x, y) if x == y => Some(Const(1)),
        Op::Binary(Div, Const(0), _) => Some(Const(0)),
        _ => None,
    }
}
```

</details>

The fold tests then pass too:

```bash
cargo test -p mini --lib opt::fold
```

---

## 6b. `Op::can_trap`

Back in `src/ir.rs`: can running this instruction fail? Only a division
can, when its divisor is zero. A division by a constant other than zero
can't fail; by zero, it always does; by anything else, it might.

```bash
cargo test -p mini --lib can_trap
```

<details>
<summary>Solution</summary>

```rust
pub fn can_trap(self) -> bool {
    match self {
        Op::Binary(BinOp::Div, _, Value::Const(divisor)) => divisor == 0,
        Op::Binary(BinOp::Div, _, _) => true,
        _ => false,
    }
}
```

</details>

---

## 6c. Common subexpression elimination

Open `src/opt/cse.rs`. In SSA form, two instructions with the same
operator and the same operands compute the same value: nothing can have
changed in between. So when an instruction repeats an earlier one, its
result can be replaced by the earlier result.

Keep a `HashMap<Op, Temp>` from each instruction seen so far to its
result. `Op` can be a key because it derives `Hash` and `Eq` (and so do
`Value` and `BinOp`, which it contains).

One subtlety: `add %x, %y` and `add %y, %x` are the same value. Before
looking an instruction up, put it in a **canonical form**: for `add`
and `mul`, whose operands can be swapped, sort the operands. `Value`
derives `Ord`, so `b < a` compares them. (The order itself doesn't
matter, only that both forms end up the same.)

Two functions:

- `canonical(op)`: `op` with the operands of `add` and `mul` sorted.
- `Cse::run`: `replace_uses` with a closure that looks up the canonical
  form. Found: replace with the earlier temporary. Not found: remember
  this one.

<details>
<summary>Hint 1: <code>canonical</code></summary>

An `@` binding names a matched value: `op @ (BinOp::Add | BinOp::Mul)`
matches either operator and calls it `op`. With a guard:

```rust
Op::Binary(op @ (BinOp::Add | BinOp::Mul), a, b) if b < a => Op::Binary(op, b, a),
_ => op,
```

</details>

<details>
<summary>Hint 2: <code>run</code></summary>

The closure needs to modify `seen` while `replace_uses` calls it. That
makes it an `FnMut` closure, exactly what `replace_uses` accepts.

```rust
let mut seen: HashMap<Op, Temp> = HashMap::new();
replace_uses(function, |inst| {
    let key = canonical(inst.op);
    match seen.get(&key) {
        // ...
    }
})
```

`seen.get(&key)` returns `Option<&Temp>`; the pattern `Some(&earlier)`
copies the `Temp` out.

</details>

```bash
cargo test -p mini --lib cse
```

<details>
<summary>Solution</summary>

```rust
fn run(&self, function: &mut Function) -> bool {
    let mut seen: HashMap<Op, Temp> = HashMap::new();
    replace_uses(function, |inst| {
        let key = canonical(inst.op);
        match seen.get(&key) {
            Some(&earlier) => Some(Value::Temp(earlier)),
            None => {
                seen.insert(key, inst.dest);
                None
            }
        }
    })
}
```

```rust
fn canonical(op: Op) -> Op {
    match op {
        Op::Binary(op @ (BinOp::Add | BinOp::Mul), a, b) if b < a => Op::Binary(op, b, a),
        _ => op,
    }
}
```

Is it safe to merge two divisions, `x / y` and `x / y`? If `y` is 0,
the first one fails before the second is reached, with or without CSE.
Yes.

</details>

---

## 6d. Dead code elimination

Open `src/opt/dce.rs`. An instruction is **live** if the result uses
it, or if a live instruction uses it, or **if it can fail**. Everything
else is dead, and goes.

Liveness flows backwards, from uses to definitions, so walk the
instructions **in reverse**, with a `HashSet<Temp>` of live
temporaries:

1. Start with the result's temporary, if it is one.
2. For each instruction, last to first: if its `dest` is live, or it
   can trap, it's live: mark its operands' temporaries live too.
3. Keep only the live instructions: `function.body.retain(...)`.
4. Return whether the body got shorter.

<details>
<summary>Hint</summary>

`function.body.iter().rev()` walks backwards. `inst.op.operands()`
gives the operands; `if let Value::Temp(temp) = operand` picks the
temporaries. Since every instruction that `can_trap` is marked live in
step 2, step 3 only needs `live.contains(&inst.dest)`.

</details>

```bash
cargo test -p mini --lib dce
```

<details>
<summary>Solution</summary>

```rust
fn run(&self, function: &mut Function) -> bool {
    // Walk backwards from the result: an instruction is live if a live
    // instruction (or the result) uses it, or if it can fail.
    let mut live = HashSet::new();
    if let Value::Temp(temp) = function.result {
        live.insert(temp);
    }
    for inst in function.body.iter().rev() {
        if live.contains(&inst.dest) || inst.op.can_trap() {
            live.insert(inst.dest);
            for operand in inst.op.operands() {
                if let Value::Temp(temp) = operand {
                    live.insert(temp);
                }
            }
        }
    }
    let before = function.body.len();
    function.body.retain(|inst| live.contains(&inst.dest));
    function.body.len() != before
}
```

</details>

---

## 6e. The pass manager

Back in `src/opt/mod.rs`. `PassManager::run` runs every pass in order,
then again, until a whole round changes nothing. After each pass that
changed something, it calls `trace` with the pass's name and the
function: that's how `--print-after-each` works. And in case two
buggy passes keep undoing each other, it stops after `MAX_ROUNDS`
rounds.

The passes are stored as `Vec<Box<dyn Pass>>`: a list of values of
*different* types (`Fold`, `Cse`, `Dce`), all used through the `Pass`
trait. `pass.run(function)` calls the right `run` for each one.

<details>
<summary>Hint</summary>

```rust
for _ in 0..MAX_ROUNDS {
    let mut changed = false;
    for pass in &self.passes {
        // run it; if it changed something, remember and trace
    }
    // stop when nothing changed
}
```

`trace` is a `&mut dyn FnMut(&str, &Function)`: call it like a
function, `trace(pass.name(), function)`.

</details>

```bash
cargo test -p mini --lib opt
```

<details>
<summary>Solution</summary>

```rust
pub fn run(&self, function: &mut Function, trace: &mut dyn FnMut(&str, &Function)) {
    for _ in 0..MAX_ROUNDS {
        let mut changed = false;
        for pass in &self.passes {
            if pass.run(function) {
                changed = true;
                trace(pass.name(), function);
            }
        }
        if !changed {
            return;
        }
    }
}
```

</details>

---

## Watch it work

```bash
cargo mini --emit=ir -O --print-after-each mini/programs/repeated.mini
```

```text
; after cse
fn main(%x) {
  %0 = add %x, 1
  %1 = add %x, 1
  %2 = mul %0, %0
  %3 = add %x, 1
  %4 = add %x, 1
  %5 = mul %0, %0
  %6 = sub %2, %2
  %7 = div %x, 2
  %8 = add %6, %7
  ret %8
}

; after dce
fn main(%x) {
  %0 = add %x, 1
  %2 = mul %0, %0
  %6 = sub %2, %2
  %7 = div %x, 2
  %8 = add %6, %7
  ret %8
}

; after fold
...
; after dce
fn main(%x) {
  %7 = div %x, 2
  ret %7
}
```

`(x + 1) * (x + 1) - (x + 1) * (x + 1) + x / 2` is `x / 2`. Follow
each step. Round 1: `fold` finds nothing; `cse` makes `%6 = sub %2, %2`
and leaves duplicates behind; `dce` prunes them. Round 2: `fold` sees
`sub %2, %2` is `0`, then `add 0, %7` is `%7`; `dce` prunes what's
left. Round 3 changes nothing: done. The gaps in the numbering show
what was pruned.

Try the other samples, and your own programs:

```bash
cargo mini --emit=ir -O mini/programs/inputs.mini
cargo mini --emit=ir -O mini/programs/arithmetic.mini
```

---

## Testing an optimizer: property tests

How do you know an optimizer is right? Examples help, but the bugs
hide in the cases nobody wrote down. Open `tests/properties.rs` and
read `optimizing_preserves_meaning`:

```rust
proptest! {
    #[test]
    fn optimizing_preserves_meaning(source in program(), inputs in inputs()) {
        // lower, copy, optimize the copy
        prop_assert_eq!(
            interp::run(&optimized, inputs),
            interp::run(&original, inputs),
            ...
        );
    }
}
```

`program()` is a **strategy**: a recipe for random values, here random
Mini programs built from small numbers, two names and the four
operators. proptest runs the test on 256 of them, each with random
inputs that favour the dangerous values: 0, -1, `i64::MIN`, `i64::MAX`.
The optimized program must give the same result, or fail with the same
error at the same place.

```bash
cargo test -p mini --test properties
```

**Exercise: break the optimizer.** In `Dce::run`, remove
`|| inst.op.can_trap()`, and run the property tests again. proptest
finds a program that now gives a result where it should fail, then
**shrinks** it: it tries smaller programs and simpler inputs, as long
as the test still fails, and reports the smallest it found:

```text
minimal failing input: source = "y + x / x + 0", inputs = [
    0,
    0,
]
```

along with the IR before and after optimization. Put `can_trap` back.
proptest saved the failing case in
`tests/properties.proptest-regressions` and replays it first on every
run, so the bug can't come back unnoticed: commit that file, or delete
it.

This is the same idea as Part 4's differential test, with a twist: the
optimizer is compared with itself, *switched off*. Real compiler teams
fuzz their compilers this way all the time (Csmith for C compilers,
`cargo fuzz` for Rust code) and have found hundreds of bugs in GCC and
LLVM.

---

**Next:** [Part 7 — Backends: JavaScript and LLVM](part7-backends.md)
