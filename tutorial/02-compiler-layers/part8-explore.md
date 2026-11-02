# Part 8 — Explore, and clean up

[← Step 2 overview](guide.md)

No new code in this part. Every layer works, so you can look inside
each one, then inside LLVM, all the way down to the processor's
instructions. Then you clean up, and take stock.

Commands write their files into `target/`, which git ignores.

---

## Every layer, one command each

| Layer                      | Command                                                     |
|----------------------------|-------------------------------------------------------------|
| tokens                     | `cargo mini --emit=tokens mini/programs/inputs.mini`              |
| parse tree (generated)     | `cargo mini --emit=cst mini/programs/inputs.mini`                 |
| AST (hand-written parser)  | `cargo mini --emit=ast mini/programs/inputs.mini`                 |
| AST (generated parser)     | `cargo mini --emit=ast --parser=yacc mini/programs/inputs.mini`   |
| IR                         | `cargo mini --emit=ir mini/programs/inputs.mini`                  |
| optimized IR, step by step | `cargo mini --emit=ir -O --print-after-each mini/programs/inputs.mini` |
| JavaScript                 | `cargo mini -O mini/programs/inputs.mini`                         |
| LLVM IR                    | `cargo mini --emit=llvm -O mini/programs/inputs.mini`             |
| run (interpreter)          | `cargo mini run mini/programs/inputs.mini 4 100`                  |
| run (native)               | `cargo mini build -O mini/programs/inputs.mini -o target/inputs && ./target/inputs 4 100` |

Real compilers have the same switches, because their developers need to
look at each layer as much as you do: `rustc -Zunpretty=ast-tree`,
`rustc --emit=mir` and `--emit=llvm-ir`, `clang -Xclang -ast-dump`,
`clang -emit-llvm`, `go build -gcflags=-S`.

---

## LLVM's optimizer, compared with yours

Generate the LLVM IR **without** Mini's optimizer, and let LLVM's do
the work instead:

```bash
cargo mini --emit=llvm mini/programs/inputs.mini > target/inputs.ll
clang -O2 -S -emit-llvm -Wno-override-module target/inputs.ll -o target/inputs.opt.ll
```

`-S -emit-llvm` stops after optimization and writes LLVM IR instead of
machine code. In `target/inputs.opt.ll`, find `@mini_main`:

```llvm
define i64 @mini_main(i64 %x, i64 %y) local_unnamed_addr #0 {
entry:
  %0 = mul i64 %x, 5
  ret i64 %0
}
```

The same result as `cargo mini -O`. Which LLVM pass did it? LLVM can print
the function after each pass that changed it, like your
`--print-after-each`:

```bash
clang -O2 -S -emit-llvm -Wno-override-module target/inputs.ll -o /dev/null \
  -mllvm -print-changed=quiet -mllvm -filter-print-funcs=mini_main
```

```text
*** IR Dump After EarlyCSEPass on mini_main ***
define i64 @mini_main(i64 %x, i64 %y) {
entry:
  %0 = mul i64 %x, 5
  ret i64 %0
}
*** IR Dump After GlobalOptPass on [module] ***
...
```

One pass, `EarlyCSE`, did the work of your three: it folds constants,
simplifies, merges common subexpressions, and removes the instructions
it made dead, all in one walk. The later passes only add attributes
(`local_unnamed_addr`, `#0`) that describe the function to its callers.
(`-mllvm` passes an option to LLVM itself, through clang.)

How many passes does `-O2` run?

```bash
clang -O2 -S -emit-llvm -Wno-override-module target/inputs.ll -o /dev/null \
  -mllvm -print-pipeline-passes | tr ',' '\n' | wc -l
```

Over a hundred, some of them several times, organized exactly like
your pass manager: a list of passes, some nested in loops.

---

## Down to the machine

`-S` without `-emit-llvm` goes one layer further: assembly.

```bash
clang -O2 -S -Wno-override-module target/inputs.ll -o target/inputs.s
```

```text
mini_main:
	leaq	(%rdi,%rdi,4), %rax
	retq
```

On x86-64 Linux, the first argument arrives in register `%rdi`, and the
result goes back in `%rax`. `lea` ("load effective address") was made
for computing addresses like `base + index × 4`, but nothing stops a
compiler from using it for arithmetic: `%rdi + %rdi × 4` is `x × 5`, in
one instruction that's cheaper than a multiplication. This choice is
made by LLVM's **instruction selection**, after all the IR passes: the
layers continue below LLVM IR, through a machine-level IR, register
allocation and scheduling, down to bytes.

Now a division:

```bash
cargo mini --emit=llvm mini/programs/division.mini > target/division.ll
clang -O2 -S -emit-llvm -Wno-override-module target/division.ll -o target/division.opt.ll
clang -O2 -S -Wno-override-module target/division.ll -o target/division.s
```

In `division.opt.ll`, `@mini_div` has been **inlined** into
`@mini_main`, and its checks turned into a `switch`. Where two paths
meet again, you'll find a **φ (phi)** instruction: SSA's answer to "this
value comes from two places":

```llvm
mini_div.exit:
  %common.ret.op.i = phi i64 [ %negated.i, %minus_one.i ], [ %quotient.i, %divide.i ]
```

"If we came from `%minus_one.i`, the value is `%negated.i`; from
`%divide.i`, it's `%quotient.i`." A Mini with `if … then … else` would
need phis in its own IR.

In `division.s`, look for `idivq` and `divl`. LLVM added a fast path:
when both numbers fit in 32 bits (the `orq`/`shrq $32` test), it uses
the 32-bit `divl`, which is much faster than a 64-bit `idivq` on many
processors.

---

## Undefined behaviour, live

Part 7 claimed that LLVM's `sdiv` would be dangerous for Mini. Check it.
Take a program whose division is unused, but must still fail:

```bash
echo '0 * (x / y)' > target/ub.mini
cargo mini --emit=llvm target/ub.mini > target/ub.ll
```

Make a copy where the call to `@mini_div` is replaced by a plain `sdiv`,
and build both copies at `-O0` and `-O2`:

```bash
sed 's/call i64 @mini_div(i64 %x, i64 %y)/sdiv i64 %x, %y/' target/ub.ll > target/ub-raw.ll
for O in -O0 -O2; do
  clang $O -Wno-override-module target/ub.ll mini/runtime/mini_rt.c -o target/ub$O
  clang $O -Wno-override-module target/ub-raw.ll mini/runtime/mini_rt.c -o target/ub-raw$O
done
for exe in target/ub-O0 target/ub-O2 target/ub-raw-O0 target/ub-raw-O2; do echo "$exe:"; ./$exe 7 0; done
```

```text
target/ub-O0:
error: division by zero
target/ub-O2:
error: division by zero
target/ub-raw-O0:
Floating point exception (core dumped)
target/ub-raw-O2:
0
```

With `@mini_div`, Mini's meaning holds at every optimization level.
With a raw `sdiv`, the *same program* crashes at `-O0` (the processor
traps on a division by zero) and prints `0` at `-O2`. LLVM saw an
`sdiv` whose result is multiplied by 0; since division by zero is
undefined, it may assume the divisor isn't zero, so the `sdiv` can't
fail, so it's dead: deleted. Both behaviours are "correct" for LLVM.
That's what undefined behaviour means, and why a frontend must never
hand LLVM an operation whose UB its own language defines.

---

## How rustc does it

Rust has the same problem with overflow, and solves it with the same
tools. Write this into `target/five.rs`:

```rust
#[inline(never)]
pub fn times_five(x: i64) -> i64 {
    x * 5
}
```

```bash
rustc --edition 2024 --crate-type=lib --emit=llvm-ir -C opt-level=0 -o target/five-debug.ll target/five.rs
rustc --edition 2024 --crate-type=lib --emit=llvm-ir -O -o target/five-release.ll target/five.rs
```

In a debug build, `x * 5` checks for overflow, with an LLVM intrinsic,
and panics:

```llvm
  %0 = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %x, i64 5)
  %_2.1 = extractvalue { i64, i1 } %0, 1
  br i1 %_2.1, label %panic, label %bb1
```

In a release build, Rust's `*` wraps, like Mini's, and the IR is the
same as yours: `%_0 = mul i64 %x, 5`, no `nsw`. Notice the function's
name too: `@_RNvCs…4five10times_five`. That's Rust's name mangling.

Paste any of these `.ll` files into [Compiler Explorer](https://godbolt.org)
(language "LLVM IR") to see the assembly for other processors, and
compare with the C, Rust or Go compilers on the same site.

---

## Clean up

1. **Delete the old code generator.** `src/codegen.rs` turned the AST
   straight into JavaScript; `backend/js.rs` replaced it. Delete the
   file, and its `pub mod codegen;` line in `src/lib.rs`. (It's in git
   if you ever want to look at it again.)
2. **Delete the scaffolding `allow`** in `src/lib.rs`: the comment
   lines and `#![allow(dead_code, unused_imports, unused_variables, clippy::ptr_arg)]`.
3. **Lint and format:**

   ```bash
   cargo clippy -p mini --all-targets
   ```

   ```bash
   cargo fmt
   ```

   With the `allow` gone, clippy may point at unused imports in your
   own step-1 code (`use std::ops::Deref;`, for example): delete them.
4. **Final check:** `cargo test -p mini`, 121 tests (128, minus the
   seven that tested `codegen.rs`), all green.

---

## What you can now do

- [ ] Explain why a compiler has many layers, and what each of Mini's
      does, from tokens to machine code
- [ ] Read a Bison grammar, its automaton, and a shift/reduce trace;
      explain a shift/reduce conflict and resolve it with precedence
      declarations
- [ ] Generate a parser in Rust with a build script, and connect it to
      hand-written code
- [ ] Explain the difference between a parse tree and an AST, and
      prune one into the other
- [ ] Design a three-address, SSA intermediate representation, lower an
      AST into it, and interpret it
- [ ] Write optimization passes, and know what they may remove (pure
      computations nobody uses) and what they may not (anything that
      can fail)
- [ ] Generate LLVM IR, link it with a runtime, and read what LLVM's
      optimizer and code generator make of it
- [ ] Explain undefined behaviour, and how a frontend keeps its
      language's meaning on top of it
- [ ] Test a compiler differentially and with random programs
- [ ] Use lifetimes in types, `HashMap`/`HashSet`, slice patterns,
      traits and trait objects, closures, and build scripts in Rust

---

## Stretch exercises

Optional, roughly from easiest to hardest. Test first, as always.

1. **More simplifications.** `sub 0, x` is `neg x`, and `neg (neg x)`
   is `x`. The second one needs to know how `x` was computed: give
   `simplify` access to the instructions seen so far (a
   `HashMap<Temp, Op>`). The first one *replaces* an instruction instead
   of a value: what changes in `replace_uses`?
2. **Strength reduction.** `mul x, 2` could be `add x, x`. Should Mini
   do it? Look at what LLVM makes of `x * 2`, `x * 8` and `x * 7` in
   assembly before deciding.
3. **`--emit=asm`.** Add a layer to `main.rs` that runs
   `clang -O2 -S` on the LLVM IR and prints the assembly.
4. **Errors that point at the source, natively.** The native program
   says `error: division by zero`, without the location the interpreter
   gives. Pass the line and column to `@mini_division_by_zero` (the
   backend needs the source text to compute them, or a precomputed
   table), and print them in the runtime. Do the same in JavaScript.
5. **A third backend: C.** Emit C instead of LLVM IR, and compile it
   with clang. Careful: C's signed overflow is undefined, so use
   `uint64_t` arithmetic and convert back. Compare the assembly with the
   LLVM backend's.
6. **A textual IR parser.** Parse the IR's text format back into a
   `Function`. Then optimizer tests can be written as IR text, before
   and after, the way LLVM's own tests are (its `lit` and `FileCheck`
   tools). A property to test it: printing then parsing gives the same
   function.
7. **The AST, straight from the grammar.** Write a second grammar whose
   actions build `Expr` directly (`Expr '+' Expr { Ok(Expr::binary(BinOp::Add, $1?, $3?)) }`),
   with no CST. Which version would you rather maintain?
8. **Pratt parsing.** Replace `expr` and `term` in the hand-written
   parser with one loop driven by a table of operator precedences, the
   hand-written equivalent of `%left`. The differential tests will tell
   you whether you got it right.

---

**Done?** Head back to the [tutorial overview](../README.md) for what
comes next.
