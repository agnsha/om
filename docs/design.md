# Om design

> Draft 1. This is a plan, not a description of the current compiler. Speed
> figures are targets that phase 5 will measure.

## Summary

Om is a compiled systems language for backend services. It aims to:

- **match Rust's runtime speed** on general code (same LLVM optimizer, no GC, no hidden costs);
- **beat Rust on compile time and simplicity** (Cranelift dev builds, no lifetime annotations);
- **beat idiomatic Rust on one workload**: allocation-heavy request handling, using a memory region per request.

Om will not be faster than Rust at everything. In release mode both compile to
similar LLVM IR, so for loops over numbers, hashing or SIMD the realistic result
is parity.

## Language overview

Om keeps what makes Rust fast and safe: static types, single ownership, no GC,
no exceptions, monomorphized generics, errors as values. It drops what makes
Rust slow to learn and to compile: lifetime annotations, a complex trait solver,
and procedural macros.

```om
struct User { id: u64, name: str }

enum Shape { Circle(f64), Rect(f64, f64) }

fn area(s: &Shape) -> f64 {
    match s {
        Circle(r)  => 3.14159 * r * r,
        Rect(w, h) => w * h,
    }
}

fn parse_port(text: &str) -> Result[u16, Error] {
    let n = text.to_int()?
    if n > 65535 { return Err(Error.range("port")) }
    Ok(n as u16)
}
```

- Integer types with explicit widths, `f64`, `bool`, `str`, structs, enums with data, arrays and slices.
- Generics use square brackets (`Vec[T]`) so the parser never confuses them with `<`.
- Interfaces (Om's traits) with explicit implementations and no blanket impls.
- Local type inference only: function signatures are always written out.
- No implicit copies of large values and no implicit allocation.

## Memory model: ownership without lifetimes

Rust's borrow checker lets a reference be stored anywhere, which is why
lifetimes appear in signatures. Om uses a narrower rule, drawn from research
languages such as Hylo and Vale: **references are second-class**.

- A function can take `&T` or `&mut T` and use it.
- A reference cannot be stored in a struct or returned past its arguments.

This removes nearly all lifetime annotations and makes the checker a fast,
local pass per function. Three kinds of ownership cover what the rule takes away:

1. **Owned values** move by default, as in Rust. Small `copy` types are copied.
2. **Boxes and collections** (`Box[T]`, `Vec[T]`, `Map[K, V]`) free themselves when their owner goes out of scope.
3. **Regions** are named arenas. Everything allocated in a region is freed at once when it ends, and the compiler stops pointers into it from escaping.

```om
fn build_report(rows: &Vec[Row]) -> String {
    region scratch {
        let parts = Vec.in(scratch)
        for r in rows { parts.push(r.format_in(scratch)) }
        parts.join("\n")          // result copied out
    }                             // scratch freed in one step
}
```

**Trade-off:** some designs that are natural in Rust, such as a struct holding a
borrowed slice of a large buffer, need an index, a region or a copy in Om.

## Where Om can be faster than Rust

| Area | Why |
|---|---|
| Debug compile time | Cranelift instead of LLVM for dev builds; linear type checking; per-function incremental builds |
| Allocation-heavy servers | Region per request: allocation is a pointer bump, the free is one reset. Rust can do this with `bumpalo`, but it is opt-in and fights the borrow checker. |
| Developer time | No lifetimes, one owned string type, short errors, one `om` tool |

## Where it cannot

| Area | Why |
|---|---|
| Raw compute | Same backend as Rust in release mode, so parity at best |
| Libraries Rust has tuned for years | `tokio`, `serde`, `hyper` carry a decade of work; Om's first library will be slower in places |
| Expressiveness | Second-class references make some zero-copy designs harder |
| Ecosystem | Starts from zero; C interop is the bridge |

## Om vs Rust

| Area | Rust | Om plan | Who wins |
|---|---|---|---|
| Runtime speed, compute | LLVM, highly tuned | LLVM in release | Tie at best |
| Runtime speed, servers | malloc per object unless you opt in | Region per request by default | Om, if benchmarks agree |
| Debug compile time | Slow on large crates | Cranelift + linear front end | Om |
| Memory safety | Borrow checker with lifetimes | Scoped borrows + checked regions | Both safe |
| Learning curve | Lifetimes, traits, macros | No lifetimes, simple interfaces | Om |
| Expressiveness | References anywhere | References as parameters | Rust |
| Ecosystem | crates.io, rust-analyzer | Starts from zero | Rust |

## Backend services

A request handler builds strings, parses JSON, makes lists, writes a response
and discards everything. That lifecycle is a region. Om's planned server
library gives each request its own region.

```om
use om.http

fn get_user(req: &Request, r: Region) -> Response {
    let id = req.param("id")?.to_int()?
    let user = db.find_user(id)?
    Response.json(user.to_json_in(r))
}

fn main() {
    let app = http.Router.new()
    app.get("/users/:id", get_user)
    app.listen("0.0.0.0:8080")
}
```

- Predictable latency: no GC pauses at the 99th percentile.
- Small static binaries for containers.
- Structured concurrency: a task group per request.
- C interop for OpenSSL, SQLite and other existing libraries.

## Compiler

Written in Rust, in small crates: `om-lexer`, `om-parser`, `om-check`, `om-ir`,
`om-codegen` (Cranelift, later LLVM), `omc` (the CLI) and `om-std`.
Self-hosting is a late, optional goal. See [architecture.md](architecture.md).

## Roadmap

| Phase | Work | Result |
|---|---|---|
| 0 | Workspace, lexer, parser, Cranelift backend, CLI, tests, CI, docs, website | Native hello world ✅ |
| 1 | Integer types, `for`, `om-check` crate, better errors | fizzbuzz, primes, small programs |
| 2 | Structs, moves, copy types, scoped borrows, ownership checker | Memory safety without GC or lifetimes |
| 3 | Enums, `match`, generics, interfaces, `Result` and `?` | Algebraic data types and generics |
| 4 | Strings, `Vec`, `Map`, files, regions, C interop, `om test`, `om fmt` | Usable for CLI tools |
| 5 | LLVM release backend; benchmarks vs Rust, C, Go with method and raw data | Measured speed |
| 6 | HTTP library with per-request regions, structured concurrency, JSON | A deployed web service in Om |

## Risks

| Risk | Response |
|---|---|
| Scope is huge | Every phase ends with something that runs |
| Speed claims fail | Claim targets only until phase 5, then publish measured results, losses included |
| Ownership rules too strict | Write real programs (JSON parser, HTTP server) and relax rules where they hurt |
| LLVM setup pain | Cranelift only until phase 5; LLVM behind a feature flag |
