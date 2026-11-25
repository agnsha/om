# Compiler architecture

Om's compiler is written in Rust and split into crates so each stage can be
read, tested and replaced on its own.

```text
            ┌───────────┐   ┌────────────┐   ┌──────────────────┐   ┌──────────┐
 file.om ─▶ │ om-lexer  │─▶ │ om-parser  │─▶ │ om-codegen       │─▶ │ omc      │─▶ executable
            │ tokens    │   │ AST        │   │ check + lower to │   │ link with│
            │ + spans   │   │            │   │ Cranelift IR     │   │ cc       │
            └───────────┘   └────────────┘   └──────────────────┘   └──────────┘
```

## om-lexer

A hand-written, single-pass lexer over bytes. Each `Token` carries a `Span`
(byte offsets) so every later error can point at source text. Keywords are
recognised after reading an identifier.

## om-parser

A recursive-descent parser. Statements and blocks use plain functions; binary
operators use *precedence climbing* (`Parser::binary`), which handles
associativity and precedence in one loop. The grammar is at the top of
`crates/om-parser/src/lib.rs`, and the AST types are in `ast.rs`.

Semicolons are optional: a block's final expression without `;` becomes its
`tail`, which is the block's value.

## om-codegen

Phase 0 checks types while lowering to Cranelift IR, in two passes:

1. **Declare** every function's signature, so calls can refer to functions
   defined later in the file.
2. **Define** each body: walk the AST, compute each expression's `Ty`, and emit
   Cranelift instructions. Variables become Cranelift `Variable`s, which the
   frontend turns into SSA form for us.

Control flow maps to Cranelift blocks: `if` creates then, else and merge
blocks, and the merge block takes a block parameter for the `if`'s value.
`&&` and `||` short-circuit the same way. After `return`, lowering continues in
an unreachable block so code after it still type-checks.

Om functions are exported as `om_<name>`; Om's `main` becomes the C `main`
symbol so the object links like a C program. Output goes through libc's `puts`
and a two-line C runtime (`RUNTIME_C`) for printing integers, because
Cranelift cannot call variadic functions like `printf` portably.

## omc

The driver. It renders diagnostics (`file:line:col` with a caret under the
span), writes the object file and runtime to a temp directory, and runs the
system C compiler to link.

## Planned changes

- **Phase 1:** move type checking into an `om-check` crate that produces a typed
  AST, so code generation never reports user errors.
- **Phase 2:** an ownership pass over the typed AST (moves and scoped borrows).
- **Phase 3:** an `om-ir` crate (SSA, monomorphized) between checking and
  Cranelift, which also makes an LLVM backend possible in phase 5.

## Adding a feature, step by step

Example: adding a `for` loop.

1. Add the keyword to `TokenKind` and the match in `Lexer::ident`, with a lexer test.
2. Add a `Stmt::For` variant in `ast.rs` and parse it in `Parser::stmt`, with a parser test.
3. Lower it in `Lower::stmt` in `om-codegen` (it is a `while` with an increment).
4. Add an example program and its `.expected` output in `examples/`; the
   end-to-end test picks it up automatically.
