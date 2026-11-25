//! Type checking and native code generation for Om using Cranelift.
//!
//! The checker runs while lowering: each expression is type-checked as it is
//! translated to Cranelift IR. A later phase moves checking into its own crate
//! (`om-check`) and adds an Om IR between the AST and Cranelift.
//!
//! ## Value representation
//!
//! Every Om value is *flattened* into a list of scalar Cranelift values:
//! an `i64` is one `I64`, a struct or tuple is its fields' scalars in order, and
//! an enum is an `I64` tag followed by the scalars of every variant's payload.
//! Values live in SSA registers, never on the heap, so structs and enums cost
//! nothing beyond their fields and Cranelift can keep them in registers.
//!
//! ## Generics
//!
//! Generic functions, structs and enums are *monomorphized*, as in Rust: each
//! use with new type arguments produces a specialised copy. Type arguments are
//! inferred from the arguments of a call and from the type the context expects.
//!
//! ## References
//!
//! `&mut T` parameters use *mutable value semantics* (as in Swift `inout` and
//! Hylo): the callee receives the value, may change it, and hands the new value
//! back as an extra return value that the caller writes to the original place.
//! There are no pointers, so there is nothing for a lifetime to describe.
//!
//! The Om `main` becomes the C `main` symbol, and output goes through a small C
//! runtime ([`RUNTIME_C`]) because Cranelift cannot call variadic functions portably.

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{types, AbiParam, Block as ClBlock, InstBuilder, MemFlagsData, Signature, TrapCode, Value};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use om_lexer::Span;
use om_parser::ast::*;
use std::collections::{HashMap, HashSet};
use std::fmt;

/// Types and functions every Om program can use without importing them.
mod prelude;

/// The prelude's Om source: `Option`, `Result`, the standard traits and number methods.
pub fn prelude_source() -> String {
    prelude::source()
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompileError {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CompileError {}

type CResult<T> = Result<T, CompileError>;

fn err<T>(message: impl Into<String>, span: Span) -> CResult<T> {
    Err(CompileError { message: message.into(), span })
}

fn internal(e: impl fmt::Display) -> CompileError {
    CompileError { message: e.to_string(), span: Span::default() }
}

/// Integer types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntTy {
    I8,
    I16,
    I32,
    I64,
    Isize,
    U8,
    U16,
    U32,
    U64,
    Usize,
}

impl IntTy {
    const ALL: [IntTy; 10] = [
        IntTy::I8,
        IntTy::I16,
        IntTy::I32,
        IntTy::I64,
        IntTy::Isize,
        IntTy::U8,
        IntTy::U16,
        IntTy::U32,
        IntTy::U64,
        IntTy::Usize,
    ];

    fn name(self) -> &'static str {
        match self {
            IntTy::I8 => "i8",
            IntTy::I16 => "i16",
            IntTy::I32 => "i32",
            IntTy::I64 => "i64",
            IntTy::Isize => "isize",
            IntTy::U8 => "u8",
            IntTy::U16 => "u16",
            IntTy::U32 => "u32",
            IntTy::U64 => "u64",
            IntTy::Usize => "usize",
        }
    }

    fn from_name(name: &str) -> Option<IntTy> {
        IntTy::ALL.into_iter().find(|t| t.name() == name)
    }

    fn bits(self) -> u32 {
        match self {
            IntTy::I8 | IntTy::U8 => 8,
            IntTy::I16 | IntTy::U16 => 16,
            IntTy::I32 | IntTy::U32 => 32,
            _ => 64,
        }
    }

    fn signed(self) -> bool {
        matches!(self, IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64 | IntTy::Isize)
    }

    fn cl(self) -> types::Type {
        types::Type::int(self.bits() as u16).unwrap()
    }

    fn min(self) -> i128 {
        if self.signed() {
            -(1i128 << (self.bits() - 1))
        } else {
            0
        }
    }

    fn max(self) -> i128 {
        if self.signed() {
            (1i128 << (self.bits() - 1)) - 1
        } else {
            (1i128 << self.bits()) - 1
        }
    }
}

/// Om types, with all generic parameters already substituted.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    Int(IntTy),
    F32,
    F64,
    Bool,
    Char,
    Str,
    Unit,
    /// The type of an expression that never finishes, such as `return` or an endless `loop`.
    Never,
    Tuple(Vec<Ty>),
    Struct(String, Vec<Ty>),
    Enum(String, Vec<Ty>),
    /// `Ptr[T]`: a raw heap pointer, used inside the prelude to build `Vec`, `String` and `Box`.
    Ptr(Box<Ty>),
}

fn join<T: fmt::Display>(items: &[T]) -> String {
    items.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", ")
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Int(t) => f.write_str(t.name()),
            Ty::F32 => f.write_str("f32"),
            Ty::F64 => f.write_str("f64"),
            Ty::Bool => f.write_str("bool"),
            Ty::Char => f.write_str("char"),
            Ty::Str => f.write_str("str"),
            Ty::Unit => f.write_str("()"),
            Ty::Never => f.write_str("!"),
            Ty::Tuple(ts) if ts.len() == 1 => write!(f, "({},)", ts[0]),
            Ty::Tuple(ts) => write!(f, "({})", join(ts)),
            Ty::Struct(n, a) | Ty::Enum(n, a) if a.is_empty() => f.write_str(n),
            Ty::Struct(n, a) | Ty::Enum(n, a) => write!(f, "{n}[{}]", join(a)),
            Ty::Ptr(t) => write!(f, "Ptr[{t}]"),
        }
    }
}

impl Ty {
    const I64: Ty = Ty::Int(IntTy::I64);

    fn is_float(&self) -> bool {
        matches!(self, Ty::F32 | Ty::F64)
    }

    /// The name methods and trait impls are filed under: `i64`, `Point`, `Option`.
    fn key(&self) -> Option<String> {
        match self {
            Ty::Int(t) => Some(t.name().into()),
            Ty::F32 => Some("f32".into()),
            Ty::F64 => Some("f64".into()),
            Ty::Bool => Some("bool".into()),
            Ty::Char => Some("char".into()),
            Ty::Str => Some("str".into()),
            Ty::Struct(n, _) | Ty::Enum(n, _) => Some(n.clone()),
            Ty::Ptr(_) => Some("Ptr".into()),
            _ => None,
        }
    }

    /// Type arguments of a struct or enum type.
    fn args(&self) -> &[Ty] {
        match self {
            Ty::Struct(_, a) | Ty::Enum(_, a) => a,
            Ty::Ptr(t) => std::slice::from_ref(&**t),
            _ => &[],
        }
    }

    /// The name of a struct or enum type, without type arguments.
    fn base(&self) -> Option<&str> {
        match self {
            Ty::Struct(n, _) | Ty::Enum(n, _) => Some(n),
            _ => None,
        }
    }
}

fn te_string(t: &TypeExpr) -> String {
    if t.name == TypeExpr::TUPLE {
        return format!("({})", t.args.iter().map(te_string).collect::<Vec<_>>().join(", "));
    }
    if t.args.is_empty() {
        t.name.clone()
    } else {
        format!("{}[{}]", t.name, t.args.iter().map(te_string).collect::<Vec<_>>().join(", "))
    }
}

/// Type variables bound so far, plus `Self` inside an `impl`.
type Subst = HashMap<String, Ty>;

/// Flattened scalar values of one Om value.
type Vals = Vec<Value>;

/// Generic parameter names and field types of a struct definition.
type StructShape = (Vec<String>, Vec<(String, TypeExpr)>);
/// Type parameters, bounds and type of a trait impl.
type ImplShape = (Vec<String>, Vec<Bound>, TypeExpr);
/// Generic parameter names and variant payload types of an enum definition.
type EnumShape = (Vec<String>, Vec<(String, Vec<TypeExpr>)>);

struct FnParam {
    name: String,
    ty: Ty,
    mode: ParamMode,
}

/// One concrete (monomorphized) function.
struct FnInfo {
    id: FuncId,
    params: Vec<FnParam>,
    ret: Ty,
}

/// A function or method as written, possibly generic.
#[derive(Clone)]
struct FnDef {
    f: Function,
    /// For methods: the `impl` block's type parameters and type.
    impl_generics: Vec<String>,
    /// Bounds on the impl's type parameters.
    impl_bounds: Vec<Bound>,
    impl_ty: Option<TypeExpr>,
    /// Defined in the prelude: compiled only when used.
    prelude: bool,
}

impl FnDef {
    fn all_generics(&self) -> Vec<String> {
        self.impl_generics.iter().chain(&self.f.generics).cloned().collect()
    }
}

/// The type of a constant expression, or `None` if it is not a constant.
fn const_type(e: &Expr, want: &Ty) -> Option<Ty> {
    match &e.kind {
        ExprKind::Int(n) => Some(match want {
            Ty::Int(t) if (t.min()..=t.max()).contains(n) => want.clone(),
            _ => Ty::I64,
        }),
        ExprKind::Float(_) if *want == Ty::F32 => Some(Ty::F32),
        ExprKind::Float(_) => Some(Ty::F64),
        ExprKind::Bool(_) => Some(Ty::Bool),
        ExprKind::Char(_) => Some(Ty::Char),
        ExprKind::Str(_) => Some(Ty::Str),
        ExprKind::Cast(inner, t) if matches!(inner.kind, ExprKind::Int(_) | ExprKind::Float(_)) => builtin_ty(&t.name),
        ExprKind::Unary(UnOp::Neg, inner) => match const_type(inner, want)? {
            t @ (Ty::F32 | Ty::F64) => Some(t),
            Ty::Int(t) if t.signed() => Some(Ty::Int(t)),
            _ => None,
        },
        _ => None,
    }
}

fn sint_cc(op: BinOp) -> IntCC {
    match op {
        BinOp::Eq => IntCC::Equal,
        BinOp::Ne => IntCC::NotEqual,
        BinOp::Lt => IntCC::SignedLessThan,
        BinOp::Le => IntCC::SignedLessThanOrEqual,
        BinOp::Gt => IntCC::SignedGreaterThan,
        _ => IntCC::SignedGreaterThanOrEqual,
    }
}

fn uint_cc(op: BinOp) -> IntCC {
    match op {
        BinOp::Eq => IntCC::Equal,
        BinOp::Ne => IntCC::NotEqual,
        BinOp::Lt => IntCC::UnsignedLessThan,
        BinOp::Le => IntCC::UnsignedLessThanOrEqual,
        BinOp::Gt => IntCC::UnsignedGreaterThan,
        _ => IntCC::UnsignedGreaterThanOrEqual,
    }
}

/// A plain or suffixed integer literal: its value, and its suffix type if any.
/// Call `f` on `e` and every expression inside it.
fn walk_expr(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(e);
    match &e.kind {
        ExprKind::Unary(_, x)
        | ExprKind::Field(x, _)
        | ExprKind::Ref(_, x)
        | ExprKind::Try(x)
        | ExprKind::Cast(x, _) => walk_expr(x, f),
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) | ExprKind::ListRepeat(a, b) => {
            walk_expr(a, f);
            walk_expr(b, f);
        }
        ExprKind::Call(_, args) => args.iter().for_each(|a| walk_expr(&a.value, f)),
        ExprKind::MethodCall(r, _, args) => {
            walk_expr(r, f);
            args.iter().for_each(|a| walk_expr(&a.value, f));
        }
        ExprKind::StructLit(_, fs) => fs.iter().for_each(|(_, x)| walk_expr(x, f)),
        ExprKind::Tuple(xs) | ExprKind::List(xs) => xs.iter().for_each(|x| walk_expr(x, f)),
        ExprKind::Interp(parts) => parts.iter().for_each(|p| {
            if let InterpPart::Expr(x) = p {
                walk_expr(x, f)
            }
        }),
        ExprKind::If(c, a, b) => {
            walk_expr(c, f);
            walk_block(a, f);
            if let Some(b) = b {
                walk_block(b, f);
            }
        }
        ExprKind::Match(x, arms) => {
            walk_expr(x, f);
            for a in arms {
                if let Some(g) = &a.guard {
                    walk_expr(g, f);
                }
                walk_expr(&a.body, f);
            }
        }
        ExprKind::Loop(b, _) | ExprKind::Block(b) => walk_block(b, f),
        _ => {}
    }
}

fn walk_block(b: &Block, f: &mut dyn FnMut(&Expr)) {
    b.stmts.iter().for_each(|s| walk_stmt(s, f));
    if let Some(t) = &b.tail {
        walk_expr(t, f);
    }
}

fn walk_stmt(s: &Stmt, f: &mut dyn FnMut(&Expr)) {
    match s {
        Stmt::Let { value, .. } => walk_expr(value, f),
        Stmt::Assign { target, value, .. } => {
            walk_expr(target, f);
            walk_expr(value, f);
        }
        Stmt::While { cond, body, .. } => {
            walk_expr(cond, f);
            walk_block(body, f);
        }
        Stmt::For { start, end, body, .. } => {
            walk_expr(start, f);
            walk_expr(end, f);
            walk_block(body, f);
        }
        Stmt::ForEach { iter, body, .. } => {
            walk_expr(iter, f);
            walk_block(body, f);
        }
        Stmt::Return { value: Some(v), .. } | Stmt::Break { value: Some(v), .. } => walk_expr(v, f),
        Stmt::Defer { body, .. } => walk_expr(body, f),
        Stmt::Expr(e) => walk_expr(e, f),
        _ => {}
    }
}

fn literal_int(e: &Expr) -> Option<(i128, Option<&TypeExpr>)> {
    match &e.kind {
        ExprKind::Int(n) => Some((*n, None)),
        ExprKind::Cast(inner, t) => match inner.kind {
            ExprKind::Int(n) if IntTy::from_name(&t.name).is_some() => Some((n, Some(t))),
            _ => None,
        },
        _ => None,
    }
}

/// An unsuffixed number literal, possibly negated: its type comes from the other operand.
fn is_literal(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Float(_) => true,
        ExprKind::Unary(UnOp::Neg, inner) => is_literal(inner),
        _ => false,
    }
}

/// The built-in type with this name, if there is one.
fn builtin_ty(name: &str) -> Option<Ty> {
    Some(match name {
        "f32" => Ty::F32,
        "f64" => Ty::F64,
        "char" => Ty::Char,
        "bool" => Ty::Bool,
        "str" => Ty::Str,
        n => Ty::Int(IntTy::from_name(n)?),
    })
}

/// Variants of the prelude enums that can be written without their enum name.
fn prelude_variant(name: &str) -> Option<&'static str> {
    match name {
        "Some" => Some("Option::Some"),
        "None" => Some("Option::None"),
        "Ok" => Some("Result::Ok"),
        "Err" => Some("Result::Err"),
        _ => None,
    }
}

/// Type-check `program` and compile it to a native object file for the host machine.
pub fn compile_to_object(program: &Program) -> CResult<Vec<u8>> {
    compile_with(program, false)
}

/// Like [`compile_to_object`]; `check_prelude` also compiles every non-generic
/// prelude function, used or not (for testing the prelude itself).
pub fn compile_with(program: &Program, check_prelude: bool) -> CResult<Vec<u8>> {
    let src = prelude_source();
    let prelude = om_parser::parse(&src).map_err(|e| {
        let line = src[..e.span.start.min(src.len())].lines().count();
        internal(format!("prelude line {line}: {e}"))
    })?;
    let prelude_impls = prelude.impls.len();
    let mut all = prelude;
    all.functions.extend(program.functions.iter().cloned());
    all.structs.extend(program.structs.iter().cloned());
    all.enums.extend(program.enums.iter().cloned());
    all.impls.extend(program.impls.iter().cloned());
    all.consts.extend(program.consts.iter().cloned());
    all.traits.extend(program.traits.iter().cloned());
    let program = &all;

    // `OM_TARGET` (a target triple such as `x86_64-pc-windows-msvc`) cross-compiles
    // the object file; by default it targets the machine running the compiler.
    let isa_builder = match std::env::var("OM_TARGET") {
        Ok(t) if !t.is_empty() => cranelift_codegen::isa::lookup_by_name(&t)
            .map_err(|e| internal(format!("unsupported target `{t}`: {e}")))?,
        _ => cranelift_native::builder().map_err(|e| internal(format!("unsupported host: {e}")))?,
    };
    let windows = isa_builder.triple().operating_system == target_lexicon::OperatingSystem::Windows;
    let mut flags = settings::builder();
    flags.set("opt_level", "speed").map_err(internal)?;
    // COFF has no GOT, so Windows code uses absolute addresses that the linker relocates.
    flags.set("is_pic", if windows { "false" } else { "true" }).map_err(internal)?;
    if windows {
        // Windows grows the stack one guard page at a time: big frames must touch each page.
        flags.set("enable_probestack", "true").map_err(internal)?;
        flags.set("probestack_strategy", "inline").map_err(internal)?;
    }
    // Structs returned from functions can need more return registers than the ABI has.
    flags.set("enable_multi_ret_implicit_sret", "true").map_err(internal)?;
    let isa = isa_builder.finish(settings::Flags::new(flags)).map_err(internal)?;
    let builder = ObjectBuilder::new(isa, "om", cranelift_module::default_libcall_names()).map_err(internal)?;
    let mut cg = Codegen {
        module: ObjectModule::new(builder),
        defs: HashMap::new(),
        fns: HashMap::new(),
        queue: Vec::new(),
        strings: HashMap::new(),
        structs: HashMap::new(),
        enums: HashMap::new(),
        consts: HashMap::new(),
        traits: HashMap::new(),
        impls: HashMap::new(),
        glue: HashMap::new(),
        glue_queue: Vec::new(),
        origins: HashMap::new(),
        glue_origins: HashMap::new(),
        cur_origin: None,
        needs_drop_cache: Default::default(),
    };

    cg.declare_types(program)?;
    for t in IntTy::ALL {
        let lit = |n: i128| Expr { kind: ExprKind::Int(n), span: Span::default() };
        cg.consts.insert(format!("{}::MIN", t.name()), (Ty::Int(t), lit(t.min())));
        cg.consts.insert(format!("{}::MAX", t.name()), (Ty::Int(t), lit(t.max())));
        cg.consts.insert(format!("{}::BITS", t.name()), (Ty::Int(IntTy::U32), lit(t.bits() as i128)));
    }
    for (name, ty, x) in [
        ("f64::MAX", Ty::F64, f64::MAX),
        ("f64::MIN", Ty::F64, f64::MIN),
        ("f64::EPSILON", Ty::F64, f64::EPSILON),
        ("f64::INFINITY", Ty::F64, f64::INFINITY),
        ("f64::NEG_INFINITY", Ty::F64, f64::NEG_INFINITY),
        ("f64::NAN", Ty::F64, f64::NAN),
        ("f32::MAX", Ty::F32, f32::MAX as f64),
        ("f32::MIN", Ty::F32, f32::MIN as f64),
        ("f32::EPSILON", Ty::F32, f32::EPSILON as f64),
        ("f32::INFINITY", Ty::F32, f64::INFINITY),
        ("f32::NAN", Ty::F32, f64::NAN),
    ] {
        cg.consts.insert(name.into(), (ty, Expr { kind: ExprKind::Float(x), span: Span::default() }));
    }
    cg.consts.insert("char::MAX".into(), (Ty::Char, Expr { kind: ExprKind::Char(char::MAX), span: Span::default() }));
    for t in &program.traits {
        let mut seen = HashSet::new();
        for m in &t.methods {
            if !seen.insert(&m.name) {
                return err(format!("trait `{}` declares `{}` twice", t.name, m.name), m.span);
            }
        }
        if cg.traits.insert(t.name.clone(), t.clone()).is_some() {
            return err(format!("trait `{}` is defined twice", t.name), t.span);
        }
    }
    for c in &program.consts {
        let ty = cg.resolve(&c.ty, &Subst::new())?;
        match const_type(&c.value, &ty) {
            Some(t) if t == ty => {}
            Some(t) => return err(format!("constant `{}` is declared as {ty} but its value is {t}", c.name), c.span),
            None => return err("a `const` value must be a literal (number, bool or string)", c.value.span),
        }
        if cg.consts.insert(c.name.clone(), (ty, c.value.clone())).is_some() {
            return err(format!("constant `{}` is defined twice", c.name), c.span);
        }
    }

    for f in &program.functions {
        let def = FnDef { f: f.clone(), impl_generics: vec![], impl_bounds: vec![], impl_ty: None, prelude: false };
        cg.add_def(f.name.clone(), def, f.span)?;
    }
    for (i, imp) in program.impls.iter().enumerate() {
        cg.add_impl(imp, i < prelude_impls)?;
    }
    let Some(main) = program.functions.iter().find(|f| f.name == "main") else {
        return err("program has no `fn main()`", Span::default());
    };
    if !main.params.is_empty() || !main.generics.is_empty() {
        return err("`main` takes no parameters", main.span);
    }

    // Instantiate every non-generic function now, so all of them are checked even
    // if unused. Generic ones are instantiated on first use with concrete types.
    let mut keys: Vec<String> = cg
        .defs
        .iter()
        .filter(|(_, d)| (check_prelude || !d.prelude) && d.all_generics().is_empty())
        .map(|(k, _)| k.clone())
        .collect();
    keys.sort();
    for k in keys {
        cg.instantiate(&k, Subst::new(), Span::default())?;
    }
    let mut ctx = cg.module.make_context();
    let mut fctx = FunctionBuilderContext::new();
    loop {
        let (r, what) = if let Some((inst, key, subst)) = cg.queue.pop() {
            cg.cur_origin = if cg.defs[&key].prelude { cg.origins.get(&inst).copied() } else { None };
            (cg.define(&inst, &key, subst, &mut ctx, &mut fctx), format!("inside the library's `{key}`"))
        } else if let Some((id, ty, kind)) = cg.glue_queue.pop() {
            cg.cur_origin = cg.glue_origins.get(&id).copied();
            let verb = match kind {
                Glue::Drop => "dropping",
                Glue::Clone => "cloning",
                Glue::Write(..) => "printing",
            };
            (cg.define_glue(id, &ty, kind, &mut ctx, &mut fctx), format!("while {verb} {ty}"))
        } else {
            break;
        };
        if let Err(mut e) = r {
            // An error inside library code is reported at the user's line that needed it.
            if let Some(o) = cg.cur_origin {
                e.span = o;
                e.message = format!("{} ({what}, needed here)", e.message);
            }
            return Err(e);
        }
        cg.cur_origin = None;
    }
    cg.module.finish().emit().map_err(|e| internal(format!("writing object file: {e}")))
}

struct Codegen {
    module: ObjectModule,
    /// Functions and methods as written, by name (`f` or `Type::method`).
    defs: HashMap<String, FnDef>,
    /// Concrete functions, by instance name (`f`, `f<i64>`, `Option<i64>::unwrap`).
    fns: HashMap<String, FnInfo>,
    /// Instances declared but not yet compiled: (instance, definition, type arguments).
    queue: Vec<(String, String, Subst)>,
    strings: HashMap<String, DataId>,
    structs: HashMap<String, StructShape>,
    enums: HashMap<String, EnumShape>,
    consts: HashMap<String, (Ty, Expr)>,
    traits: HashMap<String, TraitDef>,
    /// Trait impls by (trait, type key): the impl's type parameters, bounds and type.
    impls: HashMap<(String, String), ImplShape>,
    /// Generated drop and clone functions, by type.
    glue: HashMap<(Ty, Glue), FuncId>,
    glue_queue: Vec<(FuncId, Ty, Glue)>,
    /// Where user code first needed each function instance or glue function, so an
    /// error inside generic library code can point at the user's line.
    origins: HashMap<String, Span>,
    glue_origins: HashMap<FuncId, Span>,
    /// The user's line responsible for the library code being compiled now.
    cur_origin: Option<Span>,
    needs_drop_cache: std::cell::RefCell<HashMap<Ty, bool>>,
}

/// Compiler-generated functions for a type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Glue {
    Drop,
    Clone,
    /// Print a value: (nested inside another value, to stderr).
    Write(bool, bool),
}

/// A piece of a panic message: literal text, then a runtime value to print.
type PanicPart<'s> = (Option<&'s str>, Option<(Value, Ty)>);

fn string_ty() -> Ty {
    Ty::Struct("String".into(), vec![])
}

fn is_string(ty: &Ty) -> bool {
    matches!(ty, Ty::Struct(n, a) if n == "String" && a.is_empty())
}

const PRIVATE_STRUCTS: &[&str] = &["Vec", "String", "Box"];

const BUILTINS: &[&str] = &["print", "eprint", "panic", "assert"];

impl Codegen {
    fn add_def(&mut self, key: String, def: FnDef, span: Span) -> CResult<()> {
        let generics = def.all_generics();
        for b in def.impl_bounds.iter().chain(&def.f.bounds) {
            self.check_bound(b, &generics)?;
        }
        if BUILTINS.contains(&key.as_str()) {
            return err(format!("`{key}` is a built-in function and cannot be redefined"), span);
        }
        if self.defs.insert(key.clone(), def).is_some() {
            return err(format!("function `{key}` is defined twice"), span);
        }
        Ok(())
    }

    fn check_bound(&self, b: &Bound, generics: &[String]) -> CResult<()> {
        if !generics.contains(&b.param) {
            return err(format!("`{}` is not a type parameter here", b.param), b.span);
        }
        if !self.traits.contains_key(&b.trait_name) {
            return err(format!("cannot find trait `{}`", b.trait_name), b.span);
        }
        Ok(())
    }

    /// Register an inherent or trait `impl` block's methods.
    fn add_impl(&mut self, imp: &ImplBlock, prelude: bool) -> CResult<()> {
        let name = &imp.ty.name;
        let builtin = builtin_ty(name).is_some();
        let generics = match (self.structs.get(name), self.enums.get(name)) {
            (Some((g, _)), _) | (_, Some((g, _))) => g.clone(),
            _ if builtin && (prelude || imp.trait_name.is_some()) => vec![],
            _ if builtin => {
                return err(
                    format!("cannot add methods to built-in type `{name}` directly; implement a trait for it"),
                    imp.ty.span,
                )
            }
            _ => return err(format!("cannot find struct or enum `{name}` for this `impl`"), imp.ty.span),
        };
        let listed: Vec<&str> = imp.ty.args.iter().map(|a| a.name.as_str()).collect();
        if imp.generics != generics || listed != generics.iter().map(String::as_str).collect::<Vec<_>>() {
            let tr = imp.trait_name.as_ref().map(|(t, _)| format!("{t} for ")).unwrap_or_default();
            let want = if generics.is_empty() {
                format!("impl {tr}{name}")
            } else {
                format!("impl[{0}] {tr}{name}[{0}]", generics.join(", "))
            };
            return err(format!("this impl must be written `{want}`"), imp.ty.span);
        }
        for b in &imp.bounds {
            self.check_bound(b, &imp.generics)?;
        }
        let def = |f: &Function| FnDef {
            f: f.clone(),
            impl_generics: imp.generics.clone(),
            impl_bounds: imp.bounds.clone(),
            impl_ty: Some(imp.ty.clone()),
            prelude,
        };
        let Some((tn, tspan)) = &imp.trait_name else {
            for m in &imp.methods {
                self.add_def(format!("{name}::{}", m.name), def(m), m.span)?;
            }
            return Ok(());
        };
        let Some(tr) = self.traits.get(tn).cloned() else {
            return err(format!("cannot find trait `{tn}`"), *tspan);
        };
        let entry = (imp.generics.clone(), imp.bounds.clone(), imp.ty.clone());
        if self.impls.insert((tn.clone(), name.clone()), entry).is_some() {
            return err(format!("`{tn}` is implemented twice for `{name}`"), *tspan);
        }
        for m in &imp.methods {
            let Some(tm) = tr.methods.iter().find(|t| t.name == m.name) else {
                return err(format!("`{}` is not a method of trait `{tn}`", m.name), m.span);
            };
            self.check_impl_sig(tn, tm, m, imp)?;
            self.add_def(format!("{name}::{}", m.name), def(m), m.span)?;
        }
        for tm in &tr.methods {
            if imp.methods.iter().any(|m| m.name == tm.name) {
                continue;
            }
            if !tm.has_body {
                return err(format!("missing method `{}` required by trait `{tn}`", tm.name), imp.ty.span);
            }
            self.add_def(format!("{name}::{}", tm.name), def(tm), imp.ty.span)?;
        }
        Ok(())
    }

    /// Check that an impl's method has the signature its trait declares (with `Self` = the impl type).
    fn check_impl_sig(&self, tn: &str, tm: &Function, m: &Function, imp: &ImplBlock) -> CResult<()> {
        let mut base = Subst::new();
        for g in &imp.generics {
            base.insert(g.clone(), Ty::Struct(g.clone(), vec![]));
        }
        let self_ty = self.resolve(&imp.ty, &base)?;
        base.insert("Self".into(), self_ty.clone());
        let render = |f: &Function, own: &[String]| -> CResult<String> {
            let mut s = base.clone();
            for (i, g) in own.iter().enumerate() {
                s.insert(g.clone(), Ty::Struct(format!("G{i}"), vec![]));
            }
            let mut ps = Vec::new();
            for p in &f.params {
                let mode = match p.mode {
                    ParamMode::Value => "",
                    ParamMode::Ref => "&",
                    ParamMode::RefMut => "&mut ",
                };
                ps.push(match &p.ty {
                    None => format!("{mode}self"),
                    Some(t) => format!("{mode}{}", self.resolve(t, &s)?),
                });
            }
            let ret = match &f.ret {
                Some(t) => format!(" -> {}", self.resolve(t, &s)?),
                None => String::new(),
            };
            let gen = if own.is_empty() { String::new() } else { format!("[{}]", own.len()) };
            Ok(format!("fn {}{gen}({}){ret}", f.name, ps.join(", ")))
        };
        let want = render(tm, &tm.generics)?;
        let got = render(m, &m.generics)?;
        if want != got {
            return err(format!("`{}` does not match trait `{tn}`: expected `{want}`, found `{got}`", m.name), m.span);
        }
        Ok(())
    }

    /// Does `ty` implement trait `tr` (including the bounds of a generic impl)?
    fn implements(&self, ty: &Ty, tr: &str) -> bool {
        let Some(k) = ty.key() else { return false };
        let Some((generics, bounds, _)) = self.impls.get(&(tr.to_string(), k)) else { return false };
        let s = Codegen::type_subst(generics, ty.args());
        bounds.iter().all(|b| self.implements(&s[&b.param], &b.trait_name))
    }

    fn resolve(&self, t: &TypeExpr, subst: &Subst) -> CResult<Ty> {
        let args = || t.args.iter().map(|a| self.resolve(a, subst)).collect::<CResult<Vec<_>>>();
        if t.name == TypeExpr::TUPLE {
            let ts = args()?;
            return Ok(if ts.is_empty() { Ty::Unit } else { Ty::Tuple(ts) });
        }
        let prim = builtin_ty(&t.name);
        if let Some(p) = prim.or_else(|| subst.get(&t.name).cloned()) {
            if !t.args.is_empty() {
                return err(format!("`{}` does not take type arguments", t.name), t.span);
            }
            return Ok(p);
        }
        if t.name == "Ptr" {
            return match args()?.as_slice() {
                [e] => Ok(Ty::Ptr(Box::new(e.clone()))),
                _ => err("`Ptr` takes one type argument", t.span),
            };
        }
        let (generics, is_struct) = match (self.structs.get(&t.name), self.enums.get(&t.name)) {
            (Some((g, _)), _) => (g, true),
            (_, Some((g, _))) => (g, false),
            _ if t.name == "Self" => return err("`Self` is only available inside an `impl` block", t.span),
            _ => return err(format!("cannot find type `{}`", t.name), t.span),
        };
        if generics.len() != t.args.len() {
            return err(
                format!("`{}` takes {} type argument(s) but {} were given", t.name, generics.len(), t.args.len()),
                t.span,
            );
        }
        let a = args()?;
        Ok(if is_struct { Ty::Struct(t.name.clone(), a) } else { Ty::Enum(t.name.clone(), a) })
    }

    fn type_subst(generics: &[String], args: &[Ty]) -> Subst {
        generics.iter().cloned().zip(args.iter().cloned()).collect()
    }

    /// Field names and types of a struct type.
    fn fields(&self, ty: &Ty) -> Vec<(String, Ty)> {
        let Ty::Struct(n, args) = ty else { return vec![] };
        let (generics, fields) = &self.structs[n];
        let s = Self::type_subst(generics, args);
        fields.iter().map(|(f, t)| (f.clone(), self.resolve(t, &s).expect("checked when declared"))).collect()
    }

    /// Variant names and payload types of an enum type.
    fn variants(&self, ty: &Ty) -> Vec<(String, Vec<Ty>)> {
        let Ty::Enum(n, args) = ty else { return vec![] };
        let (generics, variants) = &self.enums[n];
        let s = Self::type_subst(generics, args);
        variants
            .iter()
            .map(|(v, ts)| {
                (v.clone(), ts.iter().map(|t| self.resolve(t, &s).expect("checked when declared")).collect())
            })
            .collect()
    }

    fn declare_types(&mut self, p: &Program) -> CResult<()> {
        // Register names first so types can refer to each other in any order.
        for s in &p.structs {
            if self.structs.contains_key(&s.name) || self.enums.contains_key(&s.name) {
                return err(format!("type `{}` is defined twice", s.name), s.span);
            }
            self.structs.insert(s.name.clone(), (s.generics.clone(), s.fields.clone()));
        }
        for e in &p.enums {
            if self.structs.contains_key(&e.name) || self.enums.contains_key(&e.name) {
                return err(format!("type `{}` is defined twice", e.name), e.span);
            }
            let vs = e.variants.iter().map(|v| (v.name.clone(), v.fields.clone())).collect();
            self.enums.insert(e.name.clone(), (e.generics.clone(), vs));
        }
        // Check every field type, with type parameters standing in as `i64`.
        for s in &p.structs {
            let sub: Subst = s.generics.iter().map(|g| (g.clone(), Ty::I64)).collect();
            let mut seen = Vec::new();
            for (f, t) in &s.fields {
                if seen.contains(&f) {
                    return err(format!("field `{f}` is declared twice"), t.span);
                }
                seen.push(f);
                self.resolve(t, &sub)?;
            }
            let ty = Ty::Struct(s.name.clone(), vec![Ty::I64; s.generics.len()]);
            self.check_not_recursive(&ty, &mut vec![], s.span)?;
        }
        for e in &p.enums {
            let sub: Subst = e.generics.iter().map(|g| (g.clone(), Ty::I64)).collect();
            if e.variants.is_empty() {
                return err(format!("enum `{}` needs at least one variant", e.name), e.span);
            }
            let mut seen = Vec::new();
            for v in &e.variants {
                if seen.contains(&&v.name) {
                    return err(format!("variant `{}` is declared twice", v.name), v.span);
                }
                seen.push(&v.name);
                for t in &v.fields {
                    self.resolve(t, &sub)?;
                }
            }
            let ty = Ty::Enum(e.name.clone(), vec![Ty::I64; e.generics.len()]);
            self.check_not_recursive(&ty, &mut vec![], e.span)?;
        }
        Ok(())
    }

    /// Values are stored inline, so a type may not contain itself.
    fn check_not_recursive(&self, ty: &Ty, stack: &mut Vec<String>, span: Span) -> CResult<()> {
        let children: Vec<Ty> = match ty {
            Ty::Struct(..) => self.fields(ty).into_iter().map(|(_, t)| t).collect(),
            Ty::Enum(..) => self.variants(ty).into_iter().flat_map(|(_, ts)| ts).collect(),
            Ty::Tuple(ts) => ts.clone(),
            _ => return Ok(()),
        };
        if let Some(name) = ty.base() {
            if stack.iter().any(|s| s == name) {
                return err(
                    format!("type `{name}` contains itself, which would make it infinitely large; put the inner one in a `Box[{name}]` or a `Vec`"),
                    span,
                );
            }
            stack.push(name.to_string());
        }
        for c in &children {
            self.check_not_recursive(c, stack, span)?;
        }
        if ty.base().is_some() {
            stack.pop();
        }
        Ok(())
    }

    /// The scalar Cranelift types an Om value is flattened into.
    fn layout(&self, ty: &Ty) -> Vec<types::Type> {
        match ty {
            Ty::Int(t) => vec![t.cl()],
            // A `str` is a view: a pointer to UTF-8 bytes and their length.
            Ty::Str => vec![types::I64, types::I64],
            Ty::Ptr(_) => vec![types::I64],
            Ty::F32 => vec![types::F32],
            Ty::F64 => vec![types::F64],
            Ty::Char => vec![types::I32],
            Ty::Bool => vec![types::I8],
            Ty::Unit | Ty::Never => vec![],
            Ty::Tuple(ts) => ts.iter().flat_map(|t| self.layout(t)).collect(),
            Ty::Struct(..) => self.fields(ty).iter().flat_map(|(_, t)| self.layout(t)).collect(),
            Ty::Enum(..) => {
                let mut l = vec![types::I64];
                for (_, ts) in self.variants(ty) {
                    for t in &ts {
                        l.extend(self.layout(t));
                    }
                }
                l
            }
        }
    }

    /// Where each scalar of `ty` sits when stored in memory, plus the total size
    /// (padded for arrays) and alignment. Scalars are naturally aligned.
    fn mem_layout(&self, ty: &Ty) -> (Vec<(types::Type, i32)>, u32, u32) {
        let mut off = 0u32;
        let mut align = 1u32;
        let mut slots = Vec::new();
        for t in self.layout(ty) {
            let sz = t.bytes();
            off = off.next_multiple_of(sz);
            slots.push((t, off as i32));
            off += sz;
            align = align.max(sz);
        }
        (slots, off.next_multiple_of(align), align)
    }

    /// Does a value of this type own resources that must be released (memory)?
    /// Such types move instead of copying.
    fn needs_drop(&self, ty: &Ty) -> bool {
        if let Some(&b) = self.needs_drop_cache.borrow().get(ty) {
            return b;
        }
        // Recursive types recurse only through `Box` or `Vec`, which implement `Drop`,
        // so this terminates.
        let b = match ty {
            Ty::Struct(..) | Ty::Enum(..) if self.implements(ty, "Drop") => true,
            Ty::Struct(..) => self.fields(ty).iter().any(|(_, t)| self.needs_drop(t)),
            Ty::Enum(..) => self.variants(ty).iter().any(|(_, ts)| ts.iter().any(|t| self.needs_drop(t))),
            Ty::Tuple(ts) => ts.iter().any(|t| self.needs_drop(t)),
            _ => false,
        };
        self.needs_drop_cache.borrow_mut().insert(ty.clone(), b);
        b
    }

    /// Does `ty` contain a type matching `leaf`, looking through fields, variants and pointers?
    fn contains(&self, ty: &Ty, leaf: &dyn Fn(&Ty) -> bool, seen: &mut Vec<Ty>) -> bool {
        if leaf(ty) {
            return true;
        }
        if seen.contains(ty) {
            return false;
        }
        seen.push(ty.clone());
        match ty {
            Ty::Struct(..) => self.fields(ty).iter().any(|(_, t)| self.contains(t, leaf, seen)),
            Ty::Enum(..) => self.variants(ty).iter().any(|(_, ts)| ts.iter().any(|t| self.contains(t, leaf, seen))),
            Ty::Tuple(ts) => ts.iter().any(|t| self.contains(t, leaf, seen)),
            Ty::Ptr(t) => self.contains(t, leaf, seen),
            _ => false,
        }
    }

    /// Does a `ty` value hold a `str` view, which must not outlive the text it points into?
    fn has_view(&self, ty: &Ty) -> bool {
        self.contains(ty, &|t| *t == Ty::Str, &mut vec![])
    }

    /// Does a `ty` value own text that a `str` view could point into?
    fn owns_text(&self, ty: &Ty) -> bool {
        self.contains(ty, &is_string, &mut vec![])
    }

    /// The generated drop or clone function for `ty`, declared on first use.
    fn glue_fn(&mut self, ty: &Ty, kind: Glue) -> CResult<FuncId> {
        if let Some(&id) = self.glue.get(&(ty.clone(), kind)) {
            return Ok(id);
        }
        let mut sig = self.module.make_signature();
        for t in self.layout(ty) {
            sig.params.push(AbiParam::new(t));
            if kind == Glue::Clone {
                sig.returns.push(AbiParam::new(t));
            }
        }
        let what = match kind {
            Glue::Drop => "drop",
            Glue::Clone => "clone",
            Glue::Write(..) => "write",
        };
        let name = format!("om_{what}_{}", self.glue.len());
        let id = self.module.declare_function(&name, Linkage::Local, &sig).map_err(internal)?;
        self.glue.insert((ty.clone(), kind), id);
        if let Some(o) = self.cur_origin {
            self.glue_origins.insert(id, o);
        }
        self.glue_queue.push((id, ty.clone(), kind));
        Ok(id)
    }

    fn width(&self, ty: &Ty) -> usize {
        self.layout(ty).len()
    }

    /// Type and first scalar slot of field `f` (a name, or a tuple index).
    fn field(&self, ty: &Ty, f: &str) -> Option<(Ty, usize)> {
        let members: Vec<(String, Ty)> = match ty {
            Ty::Struct(..) => self.fields(ty),
            Ty::Tuple(ts) => ts.iter().enumerate().map(|(i, t)| (i.to_string(), t.clone())).collect(),
            _ => return None,
        };
        let idx = members.iter().position(|(n, _)| n == f)?;
        let offset = members[..idx].iter().map(|(_, t)| self.width(t)).sum();
        Some((members[idx].1.clone(), offset))
    }

    /// First scalar slot of variant `v`'s payload (slot 0 is the tag).
    fn variant_offset(&self, ty: &Ty, v: usize) -> usize {
        1 + self.variants(ty)[..v].iter().flat_map(|(_, ts)| ts.clone()).map(|t| self.width(&t)).sum::<usize>()
    }

    fn signature(&self, params: &[FnParam], ret: &Ty, is_main: bool) -> Signature {
        let mut sig = self.module.make_signature();
        for p in params {
            for t in self.layout(&p.ty) {
                sig.params.push(AbiParam::new(t));
            }
        }
        if is_main {
            // C's `int main(int argc, char **argv)`; the runtime keeps them for `args()`.
            sig.params.push(AbiParam::new(types::I32));
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I32));
            return sig;
        }
        for t in self.layout(ret) {
            sig.returns.push(AbiParam::new(t));
        }
        // `&mut` parameters hand their final value back to the caller.
        for p in params.iter().filter(|p| p.mode == ParamMode::RefMut) {
            for t in self.layout(&p.ty) {
                sig.returns.push(AbiParam::new(t));
            }
        }
        sig
    }

    /// The concrete instance name for definition `key` with type arguments `subst`.
    fn instance_name(&self, key: &str, subst: &Subst) -> String {
        let def = &self.defs[key];
        let impl_args: Vec<String> = def.impl_generics.iter().map(|g| subst[g].to_string()).collect();
        let own_args: Vec<String> = def.f.generics.iter().map(|g| subst[g].to_string()).collect();
        let mut name = key.to_string();
        if !impl_args.is_empty() {
            let (ty, m) = key.split_once("::").unwrap();
            name = format!("{ty}<{}>::{m}", impl_args.join(", "));
        }
        if !own_args.is_empty() {
            name = format!("{name}<{}>", own_args.join(", "));
        }
        name
    }

    /// Declare the concrete function for `key` with type arguments `subst` (if not
    /// already declared) and queue its body. Returns the instance name.
    fn instantiate(&mut self, key: &str, mut subst: Subst, span: Span) -> CResult<String> {
        let inst = self.instance_name(key, &subst);
        if self.fns.contains_key(&inst) {
            return Ok(inst);
        }
        let def = self.defs[key].clone();
        for b in def.impl_bounds.iter().chain(&def.f.bounds) {
            let t = &subst[&b.param];
            if !self.implements(t, &b.trait_name) {
                return err(
                    format!("{t} does not implement `{}`, which `{key}` requires of `{}`", b.trait_name, b.param),
                    span,
                );
            }
        }
        if let Some(t) = &def.impl_ty {
            let self_ty = self.resolve(t, &subst)?;
            subst.insert("Self".into(), self_ty);
        }
        let f = &def.f;
        let mut params = Vec::new();
        let mut seen_default = false;
        for (i, p) in f.params.iter().enumerate() {
            let ty = match (&p.ty, subst.get("Self")) {
                (Some(t), _) => self.resolve(t, &subst)?,
                (None, Some(s)) if i == 0 => s.clone(),
                (None, Some(_)) => return err("`self` must be the first parameter", p.span),
                (None, None) => return err("`self` is only allowed in methods inside an `impl` block", p.span),
            };
            if params.iter().any(|q: &FnParam| q.name == p.name) {
                return err(format!("parameter `{}` is declared twice", p.name), p.span);
            }
            match &p.default {
                Some(d) => {
                    seen_default = true;
                    match const_type(d, &ty) {
                        Some(t) if t == ty => {}
                        Some(t) => {
                            return err(format!("default for `{}` is {t}, but the parameter is {ty}", p.name), d.span)
                        }
                        None => return err("default values must be constants (a number, bool or string)", d.span),
                    }
                }
                None if seen_default => {
                    return err(format!("`{}` needs a default because an earlier parameter has one", p.name), p.span)
                }
                None => {}
            }
            params.push(FnParam { name: p.name.clone(), ty, mode: p.mode });
        }
        let ret = match &f.ret {
            Some(t) => self.resolve(t, &subst)?,
            None => Ty::Unit,
        };
        let is_main = inst == "main";
        if is_main && !matches!(ret, Ty::Unit | Ty::I64) {
            return err("`main` must return nothing or an i64 exit code", f.span);
        }
        let sig = self.signature(&params, &ret, is_main);
        let symbol = if is_main {
            "main".to_string()
        } else {
            let clean: String = inst.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
            format!("om_{clean}_{}", self.fns.len())
        };
        let id = self
            .module
            .declare_function(&symbol, Linkage::Export, &sig)
            .map_err(|e| CompileError { message: e.to_string(), span })?;
        self.fns.insert(inst.clone(), FnInfo { id, params, ret });
        self.origins.insert(inst.clone(), self.cur_origin.unwrap_or(span));
        self.queue.push((inst.clone(), key.to_string(), subst));
        Ok(inst)
    }

    fn define(
        &mut self,
        inst: &str,
        key: &str,
        subst: Subst,
        ctx: &mut Context,
        fctx: &mut FunctionBuilderContext,
    ) -> CResult<()> {
        let def = self.defs[key].clone();
        let f = &def.f;
        let info = &self.fns[inst];
        let id = info.id;
        let ret = info.ret.clone();
        let params: Vec<(String, Ty, ParamMode)> =
            info.params.iter().map(|p| (p.name.clone(), p.ty.clone(), p.mode)).collect();
        let is_main = inst == "main";
        ctx.func.signature = self.signature(&info.params, &ret, is_main);

        let target = self.module.target_config();
        let mut b = FunctionBuilder::new(&mut ctx.func, fctx);
        let entry = b.create_block();
        b.append_block_params_for_function_params(entry);
        b.switch_to_block(entry);
        b.seal_block(entry);
        let entry_vals: Vec<Value> = b.block_params(entry).to_vec();

        let inouts = params.iter().filter(|p| p.2 == ParamMode::RefMut).map(|p| p.0.clone()).collect();
        let mut lw = Lower::new(self, b, ret.clone(), is_main, subst, inouts, def.prelude);
        if is_main {
            let (argc, argv) = (entry_vals[0], entry_vals[1]);
            lw.rt("om_rt_set_args", &[types::I32, types::I64], None, &[argc, argv])?;
        }
        let mut next = 0;
        for ((pname, pty, mode), p) in params.iter().zip(&f.params) {
            let n = lw.cg.width(pty);
            let vals = entry_vals[next..next + n].to_vec();
            next += n;
            let mutable = p.mutable || *mode == ParamMode::RefMut;
            let own = match mode {
                ParamMode::Value => Own::Owned,
                ParamMode::Ref => Own::Borrowed,
                ParamMode::RefMut => Own::InOut,
            };
            lw.bind_local(pname, pty.clone(), vals, mutable, own, p.span)?;
        }

        lw.fn_body = true;
        let (vals, ty) = lw.block_expect(&f.body, Some(&ret))?;
        if !lw.dead {
            if ty != ret && ty != Ty::Never {
                let span = f.body.tail.as_ref().map_or(f.body.span, |t| t.span);
                let name = &f.name;
                if ret == Ty::Unit {
                    return err(format!("`{name}` returns nothing, but its body ends with a {ty} value"), span);
                }
                return err(format!("`{name}` must return {ret}, but its body ends with {ty}"), span);
            }
            lw.drop_scope(0)?;
            lw.emit_return(vals);
        }
        lw.b.seal_all_blocks();
        lw.b.finalize(target);

        self.module
            .define_function(id, ctx)
            .map_err(|e| CompileError { message: format!("codegen for `{inst}`: {e:?}"), span: f.span })?;
        self.module.clear_context(ctx);
        Ok(())
    }

    fn define_glue(
        &mut self,
        id: FuncId,
        ty: &Ty,
        kind: Glue,
        ctx: &mut Context,
        fctx: &mut FunctionBuilderContext,
    ) -> CResult<()> {
        let mut sig = self.module.make_signature();
        for t in self.layout(ty) {
            sig.params.push(AbiParam::new(t));
            if kind == Glue::Clone {
                sig.returns.push(AbiParam::new(t));
            }
        }
        ctx.func.signature = sig;
        let target = self.module.target_config();
        let mut b = FunctionBuilder::new(&mut ctx.func, fctx);
        let entry = b.create_block();
        b.append_block_params_for_function_params(entry);
        b.switch_to_block(entry);
        b.seal_block(entry);
        let vals: Vec<Value> = b.block_params(entry).to_vec();
        let ret = if kind == Glue::Clone { ty.clone() } else { Ty::Unit };
        let mut lw = Lower::new(self, b, ret, false, Subst::new(), vec![], true);
        let out = match kind {
            Glue::Drop => {
                lw.drop_glue_body(&vals, ty)?;
                vec![]
            }
            Glue::Clone => lw.clone_glue_body(&vals, ty)?,
            Glue::Write(nested, stderr) => {
                lw.stderr = stderr;
                lw.write_value_inline(&vals, ty, nested)?;
                vec![]
            }
        };
        lw.b.ins().return_(&out);
        lw.b.seal_all_blocks();
        lw.b.finalize(target);
        self.module.define_function(id, ctx).map_err(|e| internal(format!("codegen for {kind:?} of {ty}: {e:?}")))?;
        self.module.clear_context(ctx);
        Ok(())
    }

    fn string_data(&mut self, s: &str) -> CResult<DataId> {
        if let Some(&id) = self.strings.get(s) {
            return Ok(id);
        }
        let name = format!("om_str_{}", self.strings.len());
        let id = self.module.declare_data(&name, Linkage::Local, false, false).map_err(internal)?;
        let mut desc = DataDescription::new();
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0);
        desc.define(bytes.into_boxed_slice());
        self.module.define_data(id, &desc).map_err(internal)?;
        self.strings.insert(s.to_string(), id);
        Ok(id)
    }

    fn runtime_fn_sig(&mut self, name: &str, params: &[types::Type], ret: types::Type) -> CResult<FuncId> {
        let mut sig = self.module.make_signature();
        sig.params.extend(params.iter().map(|&p| AbiParam::new(p)));
        sig.returns.push(AbiParam::new(ret));
        self.module.declare_function(name, Linkage::Import, &sig).map_err(internal)
    }

    fn runtime_fn(&mut self, name: &str, param: Option<types::Type>) -> CResult<FuncId> {
        let mut sig = self.module.make_signature();
        if let Some(p) = param {
            sig.params.push(AbiParam::new(p));
        }
        sig.returns.push(AbiParam::new(types::I32));
        self.module.declare_function(name, Linkage::Import, &sig).map_err(internal)
    }

    /// Try to match type expression `te` (which may mention the type variables in
    /// `vars`) against the concrete type `ty`, binding variables in `subst`.
    fn unify(&self, te: &TypeExpr, ty: &Ty, vars: &[String], subst: &mut Subst) -> bool {
        if *ty == Ty::Never {
            return true;
        }
        if vars.contains(&te.name) && te.args.is_empty() {
            return match subst.get(&te.name) {
                Some(bound) => bound == ty,
                None => {
                    subst.insert(te.name.clone(), ty.clone());
                    true
                }
            };
        }
        if te.name == TypeExpr::TUPLE {
            return match ty {
                Ty::Unit => te.args.is_empty(),
                Ty::Tuple(ts) => {
                    ts.len() == te.args.len() && te.args.iter().zip(ts).all(|(a, t)| self.unify(a, t, vars, subst))
                }
                _ => false,
            };
        }
        match ty {
            Ty::Struct(n, args) | Ty::Enum(n, args) if *n == te.name => {
                args.len() == te.args.len() && te.args.iter().zip(args).all(|(a, t)| self.unify(a, t, vars, subst))
            }
            Ty::Ptr(e) if te.name == "Ptr" && te.args.len() == 1 => self.unify(&te.args[0], e, vars, subst),
            _ => match self.resolve(te, subst) {
                Ok(t) => t == *ty,
                Err(_) => false,
            },
        }
    }
}

/// How a local holds its value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Own {
    /// The local owns its value and drops it at the end of its scope unless moved.
    Owned,
    /// A view of a value someone else owns: a `&` parameter or a `&` binding.
    Borrowed,
    /// A `&mut` parameter: the caller owns the value and gets it back on return.
    InOut,
}

struct Local {
    name: String,
    vars: Vec<Variable>,
    ty: Ty,
    mutable: bool,
    own: Own,
    /// For owned values that need dropping: 1 while the value is still here (not moved).
    drop_flag: Option<Variable>,
}

/// One lexical scope: the locals declared in it (including shadowed ones, which
/// still need dropping) and the variables frozen by `&` bindings declared in it.
#[derive(Default)]
struct Scope {
    names: HashMap<String, usize>,
    ids: Vec<usize>,
    /// (frozen local, name of the `&` binding that borrows it)
    freezes: Vec<(usize, String)>,
}

/// Where a place's value lives: in SSA variables, or in memory at `addr` (a
/// `Vec` element or a `Box`'s contents) with one (type, offset) per scalar.
#[derive(Clone)]
enum Loc {
    Vars(Vec<Variable>),
    Mem(Value, Vec<(types::Type, i32)>),
}

impl Loc {
    fn slice(&self, off: usize, w: usize) -> Loc {
        match self {
            Loc::Vars(v) => Loc::Vars(v[off..off + w].to_vec()),
            Loc::Mem(a, slots) => Loc::Mem(*a, slots[off..off + w].to_vec()),
        }
    }
}

struct LoopCtx {
    /// Where `continue` jumps.
    cont: ClBlock,
    exit: Merge,
    /// Scope depth outside the loop body: defers deeper than this run on `break`/`continue`.
    depth: usize,
    /// True for `loop`, whose `break` may carry a value.
    is_loop_expr: bool,
    /// True once some `break` targets this loop.
    broken: bool,
    label: Option<String>,
    /// Locals with ids below this existed before the loop.
    outer: usize,
    /// Moved locals when the loop started, and at each `break`.
    entry_moved: HashMap<usize, Span>,
    break_moved: HashMap<usize, Span>,
}

/// A join point that collects one value from each incoming branch.
struct Merge {
    blk: ClBlock,
    ty: Option<Ty>,
}

/// A variable, or a field path inside one, that can be read and assigned.
struct Place {
    root: String,
    /// The local at the root of the place (`v` in `v.items[2].x`).
    root_id: usize,
    loc: Loc,
    ty: Ty,
    mutable: bool,
}

struct Lower<'a> {
    cg: &'a mut Codegen,
    b: FunctionBuilder<'a>,
    /// The inferred type for the next `let` (see `infer_empty_vec`).
    let_hint: Option<Ty>,
    locals: Vec<Local>,
    scopes: Vec<Scope>,
    /// Locals whose value has been moved out, with where it happened.
    moved: HashMap<usize, Span>,
    /// Owned temporaries made while borrowing, dropped at the end of the statement.
    temps: Vec<(Vals, Ty)>,
    /// Deferred expressions for each open scope, run in reverse when the scope exits.
    defers: Vec<Vec<Expr>>,
    loops: Vec<LoopCtx>,
    ret: Ty,
    is_main: bool,
    /// True after `return`, `break` or `continue`: code that follows is unreachable.
    dead: bool,
    /// Concrete types for this instance's type parameters, and `Self`.
    subst: Subst,
    /// Names of `&mut` parameters, whose final values are returned to the caller.
    inouts: Vec<String>,
    tmp_count: usize,
    /// While true, printing goes to stderr (used by `panic`).
    stderr: bool,
    /// Compiling prelude code, which may use the private fields of `Vec`, `String` and `Box`.
    in_prelude: bool,
    /// Set while starting to lower a function's body block, whose tail is the return value.
    fn_body: bool,
}

impl<'a> Lower<'a> {
    fn new(
        cg: &'a mut Codegen,
        b: FunctionBuilder<'a>,
        ret: Ty,
        is_main: bool,
        subst: Subst,
        inouts: Vec<String>,
        in_prelude: bool,
    ) -> Self {
        Lower {
            cg,
            b,
            locals: vec![],
            scopes: vec![Scope::default()],
            moved: HashMap::new(),
            temps: vec![],
            defers: vec![vec![]],
            loops: vec![],
            ret,
            is_main,
            let_hint: None,
            dead: false,
            subst,
            inouts,
            tmp_count: 0,
            stderr: false,
            in_prelude,
            fn_body: false,
        }
    }

    // ---------- variables and places ----------

    /// Bind an owned value to a new local.
    fn bind_vals(&mut self, name: &str, ty: Ty, vals: Vals, mutable: bool, span: Span) -> CResult<()> {
        self.bind_local(name, ty, vals, mutable, Own::Owned, span).map(|_| ())
    }

    fn bind_local(&mut self, name: &str, ty: Ty, vals: Vals, mutable: bool, own: Own, span: Span) -> CResult<usize> {
        if BUILTINS.contains(&name) {
            return err(format!("`{name}` is the name of a built-in function"), span);
        }
        let layout = self.cg.layout(&ty);
        let mut vars = Vec::with_capacity(vals.len());
        for (t, v) in layout.into_iter().zip(vals) {
            let var = self.b.declare_var(t);
            self.b.def_var(var, v);
            vars.push(var);
        }
        let drop_flag = if own == Own::Owned && self.cg.needs_drop(&ty) {
            let f = self.b.declare_var(types::I8);
            let one = self.b.ins().iconst(types::I8, 1);
            self.b.def_var(f, one);
            Some(f)
        } else {
            None
        };
        let id = self.locals.len();
        self.locals.push(Local { name: name.to_string(), vars, ty, mutable, own, drop_flag });
        let scope = self.scopes.last_mut().unwrap();
        scope.names.insert(name.to_string(), id);
        scope.ids.push(id);
        Ok(id)
    }

    fn lookup_id(&self, name: &str) -> Option<usize> {
        self.scopes.iter().rev().find_map(|s| s.names.get(name).copied())
    }

    fn lookup(&self, name: &str) -> Option<&Local> {
        self.lookup_id(name).map(|id| &self.locals[id])
    }

    /// The `&` binding currently freezing local `id`, if any.
    fn frozen_by(&self, id: usize) -> Option<String> {
        self.scopes.iter().flat_map(|s| &s.freezes).find(|(f, _)| *f == id).map(|(_, b)| b.clone())
    }

    /// Error if local `id` has been moved out of.
    fn check_usable(&self, id: usize, span: Span) -> CResult<()> {
        if self.moved.contains_key(&id) {
            let name = &self.locals[id].name;
            return err(
                format!(
                    "use of moved value `{name}`: it was moved earlier (clone it before the move if you need it again)"
                ),
                span,
            );
        }
        Ok(())
    }

    /// Take the value out of local `id`: it may not be used again until reassigned.
    fn move_out(&mut self, id: usize, span: Span) -> CResult<()> {
        self.check_usable(id, span)?;
        let l = &self.locals[id];
        let name = l.name.clone();
        match l.own {
            Own::Borrowed => {
                return err(format!("cannot move out of `{name}`, which is borrowed; use `{name}.clone()`"), span)
            }
            Own::InOut => {
                return err(format!("cannot move out of `&mut` parameter `{name}`; use `{name}.clone()`"), span)
            }
            Own::Owned => {}
        }
        if let Some(b) = self.frozen_by(id) {
            return err(format!("cannot move `{name}` while {b} borrows it"), span);
        }
        if let Some(f) = l.drop_flag {
            let zero = self.b.ins().iconst(types::I8, 0);
            self.b.def_var(f, zero);
        }
        self.moved.insert(id, span);
        Ok(())
    }

    /// Error if the place rooted at local `id` may not be changed right now.
    fn check_not_frozen(&self, id: usize, span: Span) -> CResult<()> {
        if let Some(b) = self.frozen_by(id) {
            let name = &self.locals[id].name;
            return err(format!("cannot change `{name}` while {b} borrows it"), span);
        }
        Ok(())
    }

    /// Error if local `id` is a shared borrow (`&` parameter, `&x` binding or a
    /// `for x in &list` element): what it points to may not be changed through it.
    fn check_not_borrowed(&self, id: usize, span: Span) -> CResult<()> {
        let l = &self.locals[id];
        // Plain data was copied into the binding; only shared memory must stay unchanged.
        if l.own == Own::Borrowed && self.cg.needs_drop(&l.ty) {
            return err(format!("cannot change `{}`: it is borrowed with `&`, so it is read-only (take it as `&mut`, or `.clone()` it)", l.name), span);
        }
        Ok(())
    }

    /// Evaluate `e` without taking ownership. A place is read where it is; any
    /// other value is a temporary, dropped at the end of the statement.
    fn borrow_expect(&mut self, e: &Expr, expected: Option<&Ty>) -> CResult<(Vals, Ty)> {
        if matches!(
            e.kind,
            ExprKind::Var(_) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Unary(UnOp::Deref, _)
        ) {
            if let Some(p) = self.place(e)? {
                self.check_usable(p.root_id, e.span)?;
                return Ok((self.read_loc(&p.loc), p.ty));
            }
        }
        let (v, t) = self.expr_expect(e, expected)?;
        if !self.dead && self.cg.needs_drop(&t) {
            self.temps.push((v.clone(), t.clone()));
        }
        Ok((v, t))
    }

    fn drop_temps(&mut self, mark: usize) -> CResult<()> {
        let temps = self.temps.split_off(mark);
        for (v, t) in temps.iter().rev() {
            self.drop_value(v, t)?;
        }
        Ok(())
    }

    /// Drop the owned locals of scope `level` that still hold their values, newest first.
    fn drop_scope(&mut self, level: usize) -> CResult<()> {
        if self.dead {
            return Ok(());
        }
        let ids: Vec<usize> = self.scopes[level].ids.iter().rev().copied().collect();
        for id in ids {
            let Some(flag) = self.locals[id].drop_flag else { continue };
            let (vars, ty) = (self.locals[id].vars.clone(), self.locals[id].ty.clone());
            if !self.moved.contains_key(&id) {
                // Definitely still here: drop without checking the flag.
                let v = self.read(&vars);
                self.drop_value(&v, &ty)?;
                continue;
            }
            let f = self.b.use_var(flag);
            let drop_blk = self.b.create_block();
            let cont = self.b.create_block();
            self.b.ins().brif(f, drop_blk, &[], cont, &[]);
            self.b.switch_to_block(drop_blk);
            self.b.seal_block(drop_blk);
            let v = self.read(&vars);
            self.drop_value(&v, &ty)?;
            self.b.ins().jump(cont, &[]);
            self.b.switch_to_block(cont);
            self.b.seal_block(cont);
        }
        Ok(())
    }

    /// Leave scope `level`: run its defers, then drop its locals.
    fn cleanup(&mut self, level: usize) -> CResult<()> {
        self.run_defers(level)?;
        self.drop_scope(level)
    }

    fn read_loc(&mut self, loc: &Loc) -> Vals {
        match loc {
            Loc::Vars(vars) => self.read(vars),
            Loc::Mem(addr, slots) => {
                slots.iter().map(|&(t, off)| self.b.ins().load(t, MemFlagsData::trusted(), *addr, off)).collect()
            }
        }
    }

    fn write_loc(&mut self, loc: &Loc, vals: &[Value]) {
        match loc {
            Loc::Vars(vars) => self.write(vars, vals),
            Loc::Mem(addr, slots) => {
                for (&(_, off), &v) in slots.iter().zip(vals) {
                    self.b.ins().store(MemFlagsData::trusted(), v, *addr, off);
                }
            }
        }
    }

    fn resolve(&self, t: &TypeExpr) -> CResult<Ty> {
        self.cg.resolve(t, &self.subst)
    }

    /// Resolve `x`, `x.field`, `x.0` or `x.a.b` to the variables that hold it, if `e` is such a path.
    fn place(&mut self, e: &Expr) -> CResult<Option<Place>> {
        match &e.kind {
            ExprKind::Var(name) => Ok(self.lookup_id(name).map(|id| {
                let l = &self.locals[id];
                Place {
                    root: name.clone(),
                    root_id: id,
                    loc: Loc::Vars(l.vars.clone()),
                    ty: l.ty.clone(),
                    mutable: l.mutable,
                }
            })),
            ExprKind::Unary(UnOp::Deref, inner) => {
                let Some(p) = self.place(inner)? else { return Ok(None) };
                match self.box_elem(&p.ty) {
                    Some(_) => Ok(Some(self.deref_place(p))),
                    // Om references are not pointers: `*x` on a plain value (a loop
                    // variable from `for x in &list`, a `&` parameter) is just `x`.
                    None if !self.cg.needs_drop(&p.ty) => Ok(Some(p)),
                    None => err(
                        format!(
                            "cannot dereference {}; Om references are not pointers, so write the name without `*`",
                            p.ty
                        ),
                        e.span,
                    ),
                }
            }
            ExprKind::Field(inner, f) => {
                let Some(mut p) = self.place(inner)? else { return Ok(None) };
                while self.box_elem(&p.ty).is_some() && self.cg.field(&p.ty, f).is_none() {
                    p = self.deref_place(p);
                }
                self.check_private(&p.ty, e.span)?;
                let Some((fty, off)) = self.cg.field(&p.ty, f) else {
                    return err(format!("{} has no field `{f}`", p.ty), e.span);
                };
                let w = self.cg.width(&fty);
                Ok(Some(Place { loc: p.loc.slice(off, w), ty: fty, ..p }))
            }
            ExprKind::Index(base, idx) => {
                let Some(p) = self.place(base)? else { return Ok(None) };
                let vals = self.read_loc(&p.loc);
                let (loc, ty) = self.index_loc(vals, &p.ty, idx, e.span)?;
                Ok(Some(Place { loc, ty, ..p }))
            }
            _ => Ok(None),
        }
    }

    fn read(&mut self, vars: &[Variable]) -> Vals {
        vars.iter().map(|&v| self.b.use_var(v)).collect()
    }

    fn write(&mut self, vars: &[Variable], vals: &[Value]) {
        for (&var, &v) in vars.iter().zip(vals) {
            self.b.def_var(var, v);
        }
    }

    /// Locals whose text the value of `e` may point into: the owners borrowed by
    /// `owner.method(...)` and `&owner` inside `e`. A rough stand-in for Rust's
    /// lifetime elision, used where the value of `e` holds a `str` view.
    fn view_roots(&self, e: &Expr, out: &mut Vec<usize>) {
        let owner = |this: &Self, place: &Expr, out: &mut Vec<usize>| {
            if this.place_type(place).is_some_and(|t| this.cg.owns_text(&t)) {
                let mut r = place;
                while let ExprKind::Field(i, _) | ExprKind::Index(i, _) | ExprKind::Unary(UnOp::Deref, i) = &r.kind {
                    r = i;
                }
                if let ExprKind::Var(n) = &r.kind {
                    if let Some(id) = this.lookup_id(n) {
                        out.push(id);
                    }
                }
            }
        };
        let block = |this: &Self, b: &Block, out: &mut Vec<usize>| {
            if let Some(t) = &b.tail {
                this.view_roots(t, out);
            }
        };
        match &e.kind {
            ExprKind::MethodCall(recv, _, args) => {
                owner(self, recv, out);
                self.view_roots(recv, out);
                args.iter().for_each(|a| self.view_roots(&a.value, out));
            }
            ExprKind::Ref(_, inner) => {
                owner(self, inner, out);
                self.view_roots(inner, out);
            }
            ExprKind::Call(_, args) => args.iter().for_each(|a| self.view_roots(&a.value, out)),
            ExprKind::StructLit(_, fs) => fs.iter().for_each(|(_, v)| self.view_roots(v, out)),
            ExprKind::Tuple(xs) | ExprKind::List(xs) => xs.iter().for_each(|x| self.view_roots(x, out)),
            ExprKind::ListRepeat(x, _) | ExprKind::Field(x, _) | ExprKind::Try(x) | ExprKind::Cast(x, _) => {
                self.view_roots(x, out)
            }
            ExprKind::Index(x, i) => {
                self.view_roots(x, out);
                self.view_roots(i, out);
            }
            ExprKind::If(_, a, b) => {
                block(self, a, out);
                if let Some(b) = b {
                    block(self, b, out);
                }
            }
            ExprKind::Match(scrut, arms) => {
                self.view_roots(scrut, out);
                arms.iter().for_each(|a| self.view_roots(&a.body, out));
            }
            ExprKind::Block(b) | ExprKind::Loop(b, _) => block(self, b, out),
            _ => {}
        }
    }

    /// `let` of a value holding a `str` view: the owners it points into are frozen
    /// while the binding lives, and a view of a temporary is an error.
    fn check_view_let(&mut self, value: &Expr, ty: &Ty, temps_mark: usize, name: &str) -> CResult<()> {
        if !self.cg.has_view(ty) {
            return Ok(());
        }
        if self.temps[temps_mark..].iter().any(|(_, t)| self.cg.owns_text(t)) {
            return err(
                "this `str` points into a temporary String that is dropped at the end of the statement; keep the String in a variable first, or use `.to_string()`",
                value.span,
            );
        }
        let mut roots = vec![];
        self.view_roots(value, &mut roots);
        for r in roots {
            self.scopes.last_mut().unwrap().freezes.push((r, name.to_string()));
        }
        Ok(())
    }

    /// A returned value holding a `str` view may only point into what the caller lent.
    fn check_view_return(&self, value: &Expr, ty: &Ty, temps_mark: usize) -> CResult<()> {
        if !self.cg.has_view(ty) {
            return Ok(());
        }
        if self.temps[temps_mark..].iter().any(|(_, t)| self.cg.owns_text(t)) {
            return err(
                "cannot return a `str` that points into a temporary String; return a String instead",
                value.span,
            );
        }
        let mut roots = vec![];
        self.view_roots(value, &mut roots);
        for r in roots {
            if self.locals[r].own == Own::Owned {
                let n = &self.locals[r].name;
                return err(
                    format!("cannot return a `str` that points into `{n}`, which is dropped when the function returns; return a String (`.to_string()`) instead"),
                    value.span,
                );
            }
        }
        Ok(())
    }

    /// The fields of the prelude's `Vec`, `String` and `Box` are private to the prelude.
    fn check_private(&self, ty: &Ty, span: Span) -> CResult<()> {
        if let Ty::Struct(n, _) = ty {
            if PRIVATE_STRUCTS.contains(&n.as_str()) && !self.in_prelude {
                return err(format!("the fields of `{n}` are private; use its methods"), span);
            }
        }
        Ok(())
    }

    fn fresh_tmp(&mut self) -> String {
        self.tmp_count += 1;
        format!("$tmp{}", self.tmp_count)
    }

    // ---------- control flow helpers ----------

    /// After a terminator, keep lowering into a fresh block with no predecessors.
    fn start_dead_block(&mut self) {
        let blk = self.b.create_block();
        self.b.switch_to_block(blk);
        self.dead = true;
    }

    fn new_merge(&mut self) -> Merge {
        Merge { blk: self.b.create_block(), ty: None }
    }

    /// Jump to `m` carrying `vals` of type `ty`, checking it agrees with earlier branches.
    fn jump_to(&mut self, m: &mut Merge, mut ty: Ty, mut vals: Vals, what: &str, span: Span) -> CResult<()> {
        if ty == Ty::Never {
            return Ok(());
        }
        if ty == Ty::Str && m.ty.as_ref().is_some_and(is_string) {
            // One branch built a String and this one gives a `str`: copy it.
            vals = self.str_to_string(&vals)?;
            ty = string_ty();
        }
        match &m.ty {
            None => {
                for t in self.cg.layout(&ty) {
                    self.b.append_block_param(m.blk, t);
                }
                m.ty = Some(ty);
            }
            Some(t) if *t != ty => return err(format!("{what} have different types: {t} and {ty}"), span),
            Some(_) => {}
        }
        let args: Vec<_> = vals.into_iter().map(Into::into).collect();
        self.b.ins().jump(m.blk, &args);
        Ok(())
    }

    /// Switch to a merge block after all branches jumped to it.
    fn finish_merge(&mut self, m: Merge, was_dead: bool) -> (Vals, Ty) {
        self.b.switch_to_block(m.blk);
        self.b.seal_block(m.blk);
        match m.ty {
            None => {
                self.dead = true;
                (vec![], Ty::Never)
            }
            Some(t) => {
                self.dead = was_dead;
                (self.b.block_params(m.blk).to_vec(), t)
            }
        }
    }

    fn emit_return(&mut self, mut vals: Vals) {
        if self.is_main {
            let code = match vals.first() {
                Some(&v) => self.b.ins().ireduce(types::I32, v),
                None => self.b.ins().iconst(types::I32, 0),
            };
            self.b.ins().return_(&[code]);
            return;
        }
        for name in self.inouts.clone() {
            let vars = self.locals[self.scopes[0].names[&name]].vars.clone();
            vals.extend(self.read(&vars));
        }
        self.b.ins().return_(&vals);
    }

    /// Run all defers and return `vals` from the function.
    fn do_return(&mut self, vals: Vals) -> CResult<()> {
        self.run_defers_above(0)?;
        self.emit_return(vals);
        Ok(())
    }

    fn block_expect(&mut self, blk: &Block, expected: Option<&Ty>) -> CResult<(Vals, Ty)> {
        self.scopes.push(Scope::default());
        self.defers.push(Vec::new());
        let mut result = self.block_inner(blk, expected);
        if result.is_ok() && !self.dead {
            let level = self.scopes.len() - 1;
            if let Err(e) = self.cleanup(level) {
                result = Err(e);
            }
        }
        self.defers.pop();
        self.scopes.pop();
        result
    }

    fn block_inner(&mut self, blk: &Block, expected: Option<&Ty>) -> CResult<(Vals, Ty)> {
        let fn_body = std::mem::take(&mut self.fn_body);
        for (k, s) in blk.stmts.iter().enumerate() {
            let mark = self.temps.len();
            self.let_hint = self.infer_empty_vec(s, &blk.stmts[k + 1..], blk.tail.as_deref());
            self.stmt(s)?;
            self.drop_temps(mark)?;
        }
        if let Some(tail) = &blk.tail {
            let mark = self.temps.len();
            let r = self.expr_expect(tail, expected)?;
            if fn_body {
                self.check_view_return(tail, &r.1, mark)?;
            } else if self.cg.has_view(&r.1) && self.temps[mark..].iter().any(|(_, t)| self.cg.owns_text(t)) {
                return err(
                    "this block's `str` points into a temporary String that is dropped here; use `.to_string()`",
                    tail.span,
                );
            }
            self.drop_temps(mark)?;
            return Ok(if self.dead { (vec![], Ty::Never) } else { r });
        }
        Ok((vec![], if self.dead { Ty::Never } else { Ty::Unit }))
    }

    /// Emit the deferred expressions of scope `level`, newest first. Each one sees
    /// only the variables that were visible at that scope, not inner ones.
    fn run_defers(&mut self, level: usize) -> CResult<()> {
        if self.defers[level].is_empty() {
            return Ok(());
        }
        // Take this level's defers while they run, so a deferred expression can never re-run itself.
        let taken = std::mem::take(&mut self.defers[level]);
        let inner_scopes = self.scopes.split_off(level + 1);
        let inner_defers = self.defers.split_off(level + 1);
        let saved_loops = std::mem::take(&mut self.loops);
        let mut result = Ok(());
        for e in taken.iter().rev() {
            result = self.expr(e).map(|_| ());
            if result.is_ok() && self.dead {
                result = err("`return`, `break`, `continue` and `?` are not allowed inside `defer`", e.span);
            }
            if result.is_err() {
                break;
            }
        }
        self.loops = saved_loops;
        self.defers[level] = taken;
        self.scopes.extend(inner_scopes);
        self.defers.extend(inner_defers);
        result
    }

    /// Run the defers of every scope deeper than `depth`, innermost first.
    fn run_defers_above(&mut self, depth: usize) -> CResult<()> {
        for level in (depth..self.scopes.len()).rev() {
            self.cleanup(level)?;
        }
        Ok(())
    }

    /// `let mut v = Vec::new()` with no annotation: find the type from the first
    /// later `v.push(x)` / `v.insert(i, x)` whose `x` has an obvious type.
    fn infer_empty_vec(&self, s: &Stmt, rest: &[Stmt], tail: Option<&Expr>) -> Option<Ty> {
        let Stmt::Let { pat, ty: None, value, .. } = s else { return None };
        let PatKind::Bind(name, _) = &pat.kind else { return None };
        let ExprKind::Call(f, _) = &value.kind else { return None };
        if f != "Vec::new" && f != "Vec::with_capacity" {
            return None;
        }
        let mut found = None;
        for st in rest {
            walk_stmt(st, &mut |e| {
                if found.is_some() {
                    return;
                }
                if let ExprKind::MethodCall(recv, m, args) = &e.kind {
                    if matches!(&recv.kind, ExprKind::Var(v) if v == name) && (m == "push" || m == "insert") {
                        found = args.last().and_then(|a| self.guess_type(&a.value, rest));
                    }
                }
            });
        }
        if found.is_none() {
            if let Some(t) = tail {
                walk_expr(t, &mut |e| {
                    if let ExprKind::MethodCall(recv, m, args) = &e.kind {
                        if found.is_none()
                            && matches!(&recv.kind, ExprKind::Var(v) if v == name)
                            && (m == "push" || m == "insert")
                        {
                            found = args.last().and_then(|a| self.guess_type(&a.value, rest));
                        }
                    }
                });
            }
        }
        found.map(|t| Ty::Struct("Vec".into(), vec![t]))
    }

    /// The type of `e` when it is obvious without compiling it: literals, known
    /// variables, struct literals, casts and calls to non-generic functions.
    fn guess_type(&self, e: &Expr, later: &[Stmt]) -> Option<Ty> {
        match &e.kind {
            ExprKind::Int(_) => Some(Ty::I64),
            ExprKind::Float(_) => Some(Ty::F64),
            ExprKind::Bool(_) => Some(Ty::Bool),
            ExprKind::Char(_) => Some(Ty::Char),
            ExprKind::Str(_) => Some(Ty::Str),
            ExprKind::Interp(_) => Some(string_ty()),
            ExprKind::Var(v) => match self.lookup_id(v) {
                Some(id) => Some(self.locals[id].ty.clone()),
                // Declared later in the block: use its `let`.
                None => later.iter().find_map(|st| match st {
                    Stmt::Let { pat: Pattern { kind: PatKind::Bind(n, _), .. }, ty, value, .. } if n == v => match ty {
                        Some(t) => self.resolve(t).ok(),
                        None => self.guess_type(value, &[]),
                    },
                    _ => None,
                }),
            },
            ExprKind::Cast(_, t) => self.resolve(t).ok(),
            ExprKind::Unary(UnOp::Neg, x) => self.guess_type(x, later),
            ExprKind::Binary(op, l, _)
                if !matches!(
                    op,
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::And | BinOp::Or
                ) =>
            {
                self.guess_type(l, later)
            }
            ExprKind::Binary(..) => Some(Ty::Bool),
            ExprKind::Tuple(es) => {
                es.iter().map(|x| self.guess_type(x, later)).collect::<Option<Vec<_>>>().map(Ty::Tuple)
            }
            ExprKind::List(es) => {
                es.first().and_then(|x| self.guess_type(x, later)).map(|t| Ty::Struct("Vec".into(), vec![t]))
            }
            ExprKind::StructLit(n, _) => match self.cg.structs.get(n) {
                Some((g, _)) if g.is_empty() => Some(Ty::Struct(n.clone(), vec![])),
                _ => None,
            },
            ExprKind::MethodCall(_, m, _) if m == "to_string" => Some(string_ty()),
            ExprKind::Call(f, _) => {
                if f == "String::from" || f == "String::new" {
                    return Some(string_ty());
                }
                let def = self.cg.defs.get(f)?;
                if !def.f.generics.is_empty() || !def.impl_generics.is_empty() {
                    return None;
                }
                match &def.f.ret {
                    Some(t) if t.name != "Self" => self.resolve(t).ok(),
                    Some(_) => def.impl_ty.as_ref().and_then(|t| self.resolve(t).ok()),
                    None => Some(Ty::Unit),
                }
            }
            _ => None,
        }
    }

    // ---------- statements ----------

    fn stmt(&mut self, s: &Stmt) -> CResult<()> {
        match s {
            Stmt::Let { pat, ty, value, span } => {
                let hint = self.let_hint.take();
                let want = match ty {
                    Some(t) => Some(self.resolve(t)?),
                    None => hint,
                };
                // `let x = &temporary` keeps the temporary alive for the whole block, as in
                // Rust: `x` simply owns it.
                let value = match &value.kind {
                    ExprKind::Ref(false, inner)
                        if !matches!(
                            inner.kind,
                            ExprKind::Var(_)
                                | ExprKind::Field(..)
                                | ExprKind::Index(..)
                                | ExprKind::Unary(UnOp::Deref, _)
                        ) =>
                    {
                        &**inner
                    }
                    _ => value,
                };
                let temps_mark = self.temps.len();
                let (vals, vty, own, freeze) = self.subject(value, &[pat], want.as_ref())?;
                if let Some(want) = &want {
                    if *want != vty && vty != Ty::Never {
                        return err(format!("expected {want}, but the value is {vty}"), value.span);
                    }
                }
                if vty == Ty::Never {
                    return Ok(());
                }
                self.check_pattern(pat, &vty)?;
                if self.cg_missing(vec![vec![pat.clone()]], &vty).is_some() {
                    return err("this `let` pattern might not match; use `if let` or `match` instead", pat.span);
                }
                let _ = span;
                self.pat_bind(pat, &vty, &vals, own)?;
                if let Some(root) = freeze {
                    let name = match &pat.kind {
                        PatKind::Bind(n, _) => n.clone(),
                        _ => "a `&` binding".into(),
                    };
                    self.scopes.last_mut().unwrap().freezes.push((root, format!("`{name}`")));
                }
                if own == Own::Owned {
                    self.drop_residual(pat, &vty, &vals)?;
                }
                let name = match &pat.kind {
                    PatKind::Bind(n, _) => format!("`{n}`"),
                    _ => "a binding".into(),
                };
                self.check_view_let(value, &vty, temps_mark, &name)?;
                Ok(())
            }
            Stmt::Assign { target, op, value, span } => self.assign(target, *op, value, *span),
            Stmt::ForEach { pat, iter, body, label, span } => {
                // Looping over a borrowed list (a `&` parameter) visits it by reference, as in Rust.
                let mut root = iter;
                while let ExprKind::Field(i, _) | ExprKind::Index(i, _) = &root.kind {
                    root = i;
                }
                let borrowed = match &root.kind {
                    ExprKind::Var(n) => self.lookup(n).is_some_and(|l| l.own != Own::Owned),
                    _ => false,
                };
                let iter = if borrowed && !matches!(iter.kind, ExprKind::Ref(..)) {
                    Expr { kind: ExprKind::Ref(false, Box::new(iter.clone())), span: iter.span }
                } else {
                    iter.clone()
                };
                self.tmp_count += 1;
                let s =
                    om_parser::desugar_for_each(pat.clone(), iter, body.clone(), label.clone(), *span, self.tmp_count);
                self.stmt(&s)
            }
            Stmt::While { cond, body, label, .. } => {
                let header = self.b.create_block();
                let body_blk = self.b.create_block();
                let exit = self.b.create_block();
                self.b.ins().jump(header, &[]);
                self.b.switch_to_block(header);
                let c = self.cond(cond)?;
                self.b.ins().brif(c, body_blk, &[], exit, &[]);
                // The normal exit carries no value.
                let exit = Merge { blk: exit, ty: Some(Ty::Unit) };
                self.loop_body(body_blk, header, exit, false, label.clone(), body)?;
                self.b.seal_block(header);
                Ok(())
            }
            Stmt::For { var, start, end, inclusive, body, label, span } => {
                let ((sv, st), (ev, et)) = self.operands(start, end, None, BinOp::Add)?;
                let Ty::Int(it) = st else {
                    return err(format!("`for` ranges need integer bounds, found {st}"), start.span);
                };
                if st != et {
                    return err(format!("the bounds of this range have different types: {st} and {et}"), *span);
                }
                let counter = self.b.declare_var(it.cl());
                self.b.def_var(counter, sv[0]);
                // For `..=`, set once the counter reaches `end`, so `0..=u8::MAX` stops
                // instead of wrapping around.
                let finished = self.b.declare_var(types::I8);
                let no = self.b.ins().iconst(types::I8, 0);
                self.b.def_var(finished, no);
                let header = self.b.create_block();
                let body_blk = self.b.create_block();
                let step = self.b.create_block();
                let exit = self.b.create_block();
                self.b.ins().jump(header, &[]);
                self.b.switch_to_block(header);
                let i = self.b.use_var(counter);
                let op = if *inclusive { BinOp::Le } else { BinOp::Lt };
                let cc = if it.signed() { sint_cc(op) } else { uint_cc(op) };
                let mut c = self.b.ins().icmp(cc, i, ev[0]);
                if *inclusive {
                    let f = self.b.use_var(finished);
                    let not_f = self.b.ins().icmp_imm_u(IntCC::Equal, f, 0);
                    c = self.b.ins().band(c, not_f);
                }
                self.b.ins().brif(c, body_blk, &[], exit, &[]);
                // Bind the loop variable in its own scope around the body.
                self.b.switch_to_block(body_blk);
                self.b.seal_block(body_blk);
                self.scopes.push(Scope::default());
                self.defers.push(Vec::new());
                let i = self.b.use_var(counter);
                self.bind_vals(var, st.clone(), vec![i], false, *span)?;
                let after = self.b.create_block();
                self.b.ins().jump(after, &[]);
                let exit_m = Merge { blk: exit, ty: Some(Ty::Unit) };
                let r = self.loop_body(after, step, exit_m, false, label.clone(), body);
                self.defers.pop();
                self.scopes.pop();
                r?;
                self.b.switch_to_block(step);
                self.b.seal_block(step);
                let i = self.b.use_var(counter);
                if *inclusive {
                    let at_end = self.b.ins().icmp(IntCC::Equal, i, ev[0]);
                    self.b.def_var(finished, at_end);
                }
                let next = self.b.ins().iadd_imm_s(i, 1);
                self.b.def_var(counter, next);
                self.b.ins().jump(header, &[]);
                self.b.seal_block(header);
                self.b.switch_to_block(exit);
                Ok(())
            }
            Stmt::Return { value, span } => {
                let ret = self.ret.clone();
                let mark = self.temps.len();
                let (vals, ty) = match value {
                    Some(e) => self.expr_expect(e, Some(&ret))?,
                    None => (vec![], Ty::Unit),
                };
                if let Some(e) = value {
                    self.check_view_return(e, &ty, mark)?;
                }
                if ty != self.ret && ty != Ty::Never {
                    return err(format!("this function returns {}, but `return` gives {ty}", self.ret), *span);
                }
                if !self.dead {
                    self.do_return(vals)?;
                }
                self.start_dead_block();
                Ok(())
            }
            Stmt::Break { label, value, span } => {
                let idx = self.loop_index(label.as_deref(), "break", *span)?;
                let ctx = &self.loops[idx];
                let (depth, is_loop_expr) = (ctx.depth, ctx.is_loop_expr);
                let expected = ctx.exit.ty.clone();
                let (vals, ty) = match value {
                    Some(_) if !is_loop_expr => {
                        return err("only `loop` can break with a value; `while` and `for` cannot", *span)
                    }
                    Some(e) => self.expr_expect(e, expected.as_ref())?,
                    None => (vec![], Ty::Unit),
                };
                if !self.dead {
                    self.run_defers_above(depth)?;
                    let mut ctx = self.loops.remove(idx);
                    ctx.broken = true;
                    ctx.break_moved.extend(self.moved.clone());
                    let r = self.jump_to(&mut ctx.exit, ty, vals, "`break` values", *span);
                    self.loops.insert(idx, ctx);
                    r?;
                }
                self.start_dead_block();
                Ok(())
            }
            Stmt::Continue { label, span } => {
                let idx = self.loop_index(label.as_deref(), "continue", *span)?;
                let ctx = &self.loops[idx];
                let (depth, cont) = (ctx.depth, ctx.cont);
                if !self.dead {
                    self.check_loop_moves(idx)?;
                    self.run_defers_above(depth)?;
                    self.b.ins().jump(cont, &[]);
                }
                self.start_dead_block();
                Ok(())
            }
            Stmt::Defer { body, .. } => {
                self.defers.last_mut().unwrap().push(body.clone());
                Ok(())
            }
            Stmt::Expr(e) => {
                let (v, t) = self.expr(e)?;
                self.drop_value(&v, &t)
            }
        }
    }

    /// Going around loop `idx` again: no value that existed before the loop may have
    /// been moved during this iteration, or the next iteration would use it.
    fn check_loop_moves(&self, idx: usize) -> CResult<()> {
        let ctx = &self.loops[idx];
        for (&id, &span) in &self.moved {
            if id < ctx.outer && !ctx.entry_moved.contains_key(&id) {
                let name = &self.locals[id].name;
                return err(
                    format!("`{name}` is moved inside this loop, so the next iteration would use it after the move; clone it, or `break` right after moving it"),
                    span,
                );
            }
        }
        Ok(())
    }

    /// The loop a `break` or `continue` targets: the innermost, or the one with `label`.
    fn loop_index(&self, label: Option<&str>, what: &str, span: Span) -> CResult<usize> {
        match label {
            None if self.loops.is_empty() => err(format!("`{what}` outside of a loop"), span),
            None => Ok(self.loops.len() - 1),
            Some(l) => match self.loops.iter().rposition(|c| c.label.as_deref() == Some(l)) {
                Some(i) => Ok(i),
                None => err(format!("no enclosing loop is labelled `'{l}`"), span),
            },
        }
    }

    /// Lower a loop body starting at `body_blk`; `continue` jumps to `cont`, `break` to `exit`.
    /// Leaves the builder in the (sealed) exit block and returns the exit merge.
    fn loop_body(
        &mut self,
        body_blk: ClBlock,
        cont: ClBlock,
        exit: Merge,
        is_loop_expr: bool,
        label: Option<String>,
        body: &Block,
    ) -> CResult<Merge> {
        if let Some(l) = &label {
            if self.loops.iter().any(|c| c.label.as_ref() == Some(l)) {
                return err(format!("label `'{l}` is already used by an enclosing loop"), body.span);
            }
        }
        self.b.switch_to_block(body_blk);
        self.b.seal_block(body_blk);
        let was_dead = self.dead;
        self.dead = false;
        let entry_moved = self.moved.clone();
        self.loops.push(LoopCtx {
            cont,
            exit,
            depth: self.scopes.len(),
            is_loop_expr,
            broken: false,
            label,
            outer: self.locals.len(),
            entry_moved,
            break_moved: HashMap::new(),
        });
        let r = self.block_expect(body, None);
        let (vals, ty) = match r {
            Ok(x) => x,
            Err(e) => {
                self.loops.pop();
                return Err(e);
            }
        };
        if !self.dead {
            // A value left at the end of a loop body is unused: drop it, as Rust
            // does for `expr;` (`for _ in 0..4 { rng.next() }` is fine).
            if ty != Ty::Unit {
                if let Err(e) = self.drop_value(&vals, &ty) {
                    self.loops.pop();
                    return Err(e);
                }
            }
            let idx = self.loops.len() - 1;
            if let Err(e) = self.check_loop_moves(idx) {
                self.loops.pop();
                return Err(e);
            }
            self.b.ins().jump(cont, &[]);
        }
        let ctx = self.loops.pop().unwrap();
        let mut after = ctx.entry_moved.clone();
        after.extend(ctx.break_moved.clone());
        self.moved = after;
        let exit = ctx.exit;
        self.b.switch_to_block(exit.blk);
        self.b.seal_block(exit.blk);
        self.dead = was_dead;
        if is_loop_expr && !ctx.broken {
            self.dead = true;
        }
        Ok(exit)
    }

    /// The type of a place expression, without emitting any code.
    fn expr_is_string(&self, e: &Expr) -> bool {
        self.place_type(e).is_some_and(|t| is_string(&t))
    }

    fn place_type(&self, e: &Expr) -> Option<Ty> {
        match &e.kind {
            ExprKind::Var(name) => self.lookup(name).map(|l| l.ty.clone()),
            ExprKind::Field(inner, f) => {
                let mut t = self.place_type(inner)?;
                while let (Some(e), None) = (self.box_elem(&t), self.cg.field(&t, f)) {
                    t = e;
                }
                self.cg.field(&t, f).map(|(t, _)| t)
            }
            ExprKind::Index(inner, _) => self.vec_elem(&self.place_type(inner)?),
            ExprKind::Unary(UnOp::Deref, inner) => self.box_elem(&self.place_type(inner)?),
            _ => None,
        }
    }

    fn assign(&mut self, target: &Expr, op: Option<BinOp>, value: &Expr, span: Span) -> CResult<()> {
        // The new value is computed before the place is located, so a list that the
        // right-hand side grows (and reallocates) is indexed afterwards.
        let hint = self.place_type(target);
        let mark = self.temps.len();
        let new = match op {
            None => Some(self.expr_expect(value, hint.as_ref())?),
            Some(_) => None,
        };
        if let Some((_, t)) = &new {
            if self.cg.has_view(t) {
                let mut roots = vec![];
                self.view_roots(value, &mut roots);
                if !roots.is_empty() || self.temps[mark..].iter().any(|(_, t)| self.cg.owns_text(t)) {
                    return err(
                        "cannot store a `str` view of a String in an existing variable (Om cannot check how long it lives yet); use `.to_string()`, or bind it with a new `let`",
                        value.span,
                    );
                }
            }
        }
        let Some(place) = self.place(target)? else {
            return match &target.kind {
                ExprKind::Var(name) => err(format!("cannot find variable `{name}`"), target.span),
                _ => err("can only assign to a variable, a field, a list element or `*box`", target.span),
            };
        };
        if !place.mutable {
            let r = &place.root;
            return if r == "self" {
                err("cannot change `self` here; take `&mut self` to modify it", span)
            } else {
                err(format!("cannot assign twice to `{r}`; declare it with `let mut {r}`"), span)
            };
        }
        self.check_not_frozen(place.root_id, span)?;
        self.check_not_borrowed(place.root_id, span)?;
        let whole = matches!(target.kind, ExprKind::Var(_));
        if !whole {
            self.check_usable(place.root_id, target.span)?;
        }
        let (vals, vty) = match (op, new) {
            (None, Some(n)) => n,
            (Some(op), _) => {
                self.check_usable(place.root_id, target.span)?;
                let cur = self.read_loc(&place.loc);
                let (rv, rt) = self.expr_expect(value, Some(&place.ty))?;
                self.binop(op, cur, place.ty.clone(), rv, rt, span)?
            }
            (None, None) => unreachable!(),
        };
        if vty == Ty::Never {
            return Ok(());
        }
        if place.ty != vty {
            return err(format!("cannot assign {vty} to a place of type {}", place.ty), value.span);
        }
        if op.is_none() && self.cg.needs_drop(&vty) {
            // Drop the old value first (a moved-out variable has nothing to drop).
            let flag = if whole { self.locals[place.root_id].drop_flag } else { None };
            match flag {
                Some(f) if self.moved.contains_key(&place.root_id) => {
                    let has = self.b.use_var(f);
                    let drop_blk = self.b.create_block();
                    let cont = self.b.create_block();
                    self.b.ins().brif(has, drop_blk, &[], cont, &[]);
                    self.b.switch_to_block(drop_blk);
                    self.b.seal_block(drop_blk);
                    let old = self.read_loc(&place.loc);
                    self.drop_value(&old, &vty)?;
                    self.b.ins().jump(cont, &[]);
                    self.b.switch_to_block(cont);
                    self.b.seal_block(cont);
                }
                _ => {
                    let old = self.read_loc(&place.loc);
                    self.drop_value(&old, &vty)?;
                }
            }
            if let Some(f) = flag {
                let one = self.b.ins().iconst(types::I8, 1);
                self.b.def_var(f, one);
            }
        }
        if whole {
            self.moved.remove(&place.root_id);
        }
        self.write_loc(&place.loc, &vals);
        Ok(())
    }

    // ---------- expressions ----------

    fn cond(&mut self, e: &Expr) -> CResult<Value> {
        let mark = self.temps.len();
        let (v, ty) = self.expr(e)?;
        self.drop_temps(mark)?;
        match ty {
            Ty::Bool => Ok(v[0]),
            Ty::Never => Ok(self.b.ins().iconst(types::I8, 0)),
            _ => err(format!("condition must be bool, found {ty}"), e.span),
        }
    }

    fn zero(&mut self, t: types::Type) -> Value {
        if t == types::F64 {
            self.b.ins().f64const(0.0)
        } else if t == types::F32 {
            self.b.ins().f32const(0.0)
        } else {
            self.b.ins().iconst(t, 0)
        }
    }

    fn expr(&mut self, e: &Expr) -> CResult<(Vals, Ty)> {
        self.expr_expect(e, None)
    }

    /// Lower `e`. `expected` is the type the context wants, if known; it is a hint
    /// for inferring type arguments (as in `let x: Option[i64] = None`), not a check.
    /// Lower `e`, where `expected` (if known) is the type the context wants. A `str`
    /// where a `String` is expected becomes an owned copy, so `name = "Om"` and
    /// `f("text")` work for `String` places and parameters.
    fn expr_expect(&mut self, e: &Expr, expected: Option<&Ty>) -> CResult<(Vals, Ty)> {
        let (v, t) = self.expr_expect_raw(e, expected)?;
        if t == Ty::Str && expected.is_some_and(is_string) && !self.dead {
            return Ok((self.str_to_string(&v)?, string_ty()));
        }
        Ok((v, t))
    }

    /// A new String holding a copy of the `str` `v`.
    fn str_to_string(&mut self, v: &[Value]) -> CResult<Vals> {
        let (ptr, len) = (v[0], v[1]);
        let saved = self.stderr;
        self.stderr = false;
        let r = self.capture(|s| s.write_bytes(ptr, len));
        self.stderr = saved;
        Ok(r?.0)
    }

    fn expr_expect_raw(&mut self, e: &Expr, expected: Option<&Ty>) -> CResult<(Vals, Ty)> {
        match &e.kind {
            ExprKind::Int(n) => {
                let t = match expected {
                    Some(Ty::Int(t)) => *t,
                    _ => IntTy::I64,
                };
                self.int_lit(*n, t, e.span)
            }
            ExprKind::Float(x) => Ok(self.float_lit(*x, expected == Some(&Ty::F32))),
            ExprKind::Bool(v) => Ok((vec![self.b.ins().iconst(types::I8, *v as i64)], Ty::Bool)),
            ExprKind::Char(c) => Ok((vec![self.b.ins().iconst(types::I32, *c as i64)], Ty::Char)),
            ExprKind::Str(s) => Ok((self.str_val(s)?, Ty::Str)),
            ExprKind::Var(name) => {
                if let Some(id) = self.lookup_id(name) {
                    let (vars, ty) = (self.locals[id].vars.clone(), self.locals[id].ty.clone());
                    self.check_usable(id, e.span)?;
                    if self.cg.needs_drop(&ty) {
                        self.move_out(id, e.span)?;
                    }
                    return Ok((self.read(&vars), ty));
                }
                if let Some((ty, value)) = self.cg.consts.get(name).cloned() {
                    let (v, _) = self.expr_expect(&value, Some(&ty))?;
                    return Ok((v, ty));
                }
                if let Some(r) = self.variant(name, &[], e.span, expected)? {
                    return Ok(r);
                }
                if self.cg.defs.contains_key(name) {
                    return err(format!("`{name}` is a function; call it with `{name}(...)`"), e.span);
                }
                err(format!("cannot find variable `{name}`"), e.span)
            }
            ExprKind::Unary(UnOp::Neg, inner) if literal_int(inner).is_some() => {
                // `-128i8` is a literal in range, even though `128i8` is not.
                let (n, suffix) = literal_int(inner).unwrap();
                let t = match (suffix, expected) {
                    (Some(t), _) => self.resolve(t)?,
                    (None, Some(t @ Ty::Int(_))) => t.clone(),
                    _ => Ty::I64,
                };
                match t {
                    Ty::Int(it) => self.int_lit(-n, it, e.span),
                    _ => {
                        let (v, from) = self.int_lit(-n, IntTy::I64, e.span)?;
                        self.cast(v, from, t, e.span)
                    }
                }
            }
            ExprKind::Unary(UnOp::Deref, inner) => {
                if let Some(p) = self.place(e)? {
                    self.check_usable(p.root_id, e.span)?;
                    if !self.cg.needs_drop(&p.ty) {
                        return Ok((self.read_loc(&p.loc), p.ty));
                    }
                    // `*b` moves the value out of an owned box, which is used up (as in Rust).
                    let ExprKind::Var(_) = inner.kind else {
                        return err("cannot move out of this box; use `.clone()`", e.span);
                    };
                    let v = self.read_loc(&p.loc);
                    let Loc::Mem(ptr, _) = p.loc else { unreachable!() };
                    self.move_out(p.root_id, e.span)?;
                    self.rt("om_rt_free", &[types::I64], None, &[ptr])?;
                    return Ok((v, p.ty));
                }
                let (v, ty) = self.expr(inner)?;
                let Some(elem) = self.box_elem(&ty) else {
                    return err(format!("cannot dereference {ty}"), e.span);
                };
                if self.dead {
                    return Ok((vec![], Ty::Never));
                }
                let (slots, _, _) = self.cg.mem_layout(&elem);
                let out = self.read_loc(&Loc::Mem(v[0], slots));
                self.rt("om_rt_free", &[types::I64], None, &[v[0]])?;
                Ok((out, elem))
            }
            ExprKind::Unary(op, inner) => {
                let (v, ty) = self.expr_expect(inner, expected)?;
                match (op, &ty) {
                    (UnOp::Neg, Ty::Int(t)) if t.signed() => Ok((vec![self.b.ins().ineg(v[0])], ty)),
                    (UnOp::Neg, Ty::Int(_)) => err(format!("cannot negate an unsigned {ty}"), e.span),
                    (UnOp::Neg, Ty::F32 | Ty::F64) => Ok((vec![self.b.ins().fneg(v[0])], ty)),
                    (UnOp::Not, Ty::Bool) => Ok((vec![self.b.ins().bxor_imm_u(v[0], 1)], Ty::Bool)),
                    (UnOp::Not, Ty::Int(_)) => Ok((vec![self.b.ins().bnot(v[0])], ty)),
                    (_, Ty::Never) => Ok((vec![], Ty::Never)),
                    (UnOp::Neg, _) if self.cg.implements(&ty, "Neg") => {
                        let a = self.temp_arg(v, ty.clone(), false, e.span)?;
                        let key = format!("{}::neg", ty.key().unwrap());
                        let preset = Codegen::type_subst(&self.cg.defs[&key].impl_generics, ty.args());
                        self.call_fn(&key, &[a], e.span, None, preset)
                    }
                    (UnOp::Neg, _) => err(format!("cannot negate {ty}; implement `Neg` for it"), e.span),
                    (UnOp::Not, _) => err(format!("`!` needs a bool or an integer, found {ty}"), e.span),
                    (UnOp::Deref, _) => err(format!("cannot dereference {ty}"), e.span),
                }
            }
            ExprKind::Binary(op @ (BinOp::And | BinOp::Or), l, r) => self.short_circuit(*op, l, r),
            ExprKind::Binary(op, l, r) => {
                let ((lv, lt), (rv, rt)) = self.operands(l, r, expected, *op)?;
                self.binop(*op, lv, lt, rv, rt, e.span)
            }
            ExprKind::Call(name, args) => self.call(name, args, e.span, expected),
            ExprKind::MethodCall(recv, m, args) => self.method_call(recv, m, args, e.span, expected),
            ExprKind::Field(inner, f) => {
                if let Some(p) = self.place(e)? {
                    self.check_usable(p.root_id, e.span)?;
                    let whole = self.locals[p.root_id].ty.clone();
                    if self.cg.needs_drop(&p.ty)
                        && matches!(inner.kind, ExprKind::Var(_))
                        && !self.cg.implements(&whole, "Drop")
                        && matches!(whole, Ty::Struct(..) | Ty::Tuple(_))
                        && self.locals[p.root_id].own == Own::Owned
                    {
                        // Move one field out of a local: the local is used up, and its
                        // other fields are dropped now. (Rust drops them at the end of the scope.)
                        let vals = self.read_loc(&Loc::Vars(self.locals[p.root_id].vars.clone()));
                        self.move_out(p.root_id, e.span)?;
                        let parts: Vec<(String, Ty)> = match &whole {
                            Ty::Tuple(ts) => ts.iter().enumerate().map(|(i, t)| (i.to_string(), t.clone())).collect(),
                            _ => self.cg.fields(&whole),
                        };
                        let mut o = 0;
                        for (name, t) in parts {
                            let tw = self.cg.width(&t);
                            if name != *f {
                                self.drop_value(&vals[o..o + tw], &t)?;
                            }
                            o += tw;
                        }
                        return Ok((self.read_loc(&p.loc), p.ty));
                    }
                    if self.cg.needs_drop(&p.ty) {
                        return err(
                            format!(
                                "cannot move field `{f}` out of `{}`; use `.clone()`, or match on the whole value",
                                p.root
                            ),
                            e.span,
                        );
                    }
                    return Ok((self.read_loc(&p.loc), p.ty));
                }
                let (vals, ty) = self.expr(inner)?;
                self.check_private(&ty, e.span)?;
                let Some((fty, off)) = self.cg.field(&ty, f) else {
                    return err(format!("{ty} has no field `{f}`"), e.span);
                };
                let w = self.cg.width(&fty);
                if self.cg.needs_drop(&ty) {
                    // Taking one field of a temporary: drop the rest of it.
                    if self.cg.implements(&ty, "Drop") {
                        return err(format!("cannot move a field out of {ty}, which implements Drop"), e.span);
                    }
                    let parts: Vec<(String, Ty)> = match &ty {
                        Ty::Tuple(ts) => ts.iter().enumerate().map(|(i, t)| (i.to_string(), t.clone())).collect(),
                        _ => self.cg.fields(&ty),
                    };
                    let mut o = 0;
                    for (name, t) in parts {
                        let tw = self.cg.width(&t);
                        if name != *f {
                            self.drop_value(&vals[o..o + tw], &t)?;
                        }
                        o += tw;
                    }
                }
                Ok((vals[off..off + w].to_vec(), fty))
            }
            ExprKind::StructLit(name, fields) => self.struct_lit(name, fields, e.span, expected),
            ExprKind::Tuple(items) => {
                if items.is_empty() {
                    return Ok((vec![], Ty::Unit));
                }
                let hints: Vec<Option<Ty>> = match expected {
                    Some(Ty::Tuple(ts)) if ts.len() == items.len() => ts.iter().cloned().map(Some).collect(),
                    _ => vec![None; items.len()],
                };
                let mut vals = Vec::new();
                let mut tys = Vec::new();
                for (it, h) in items.iter().zip(&hints) {
                    let (v, t) = self.expr_expect(it, h.as_ref())?;
                    if t == Ty::Never {
                        return Ok((vec![], Ty::Never));
                    }
                    vals.extend(v);
                    tys.push(t);
                }
                Ok((vals, Ty::Tuple(tys)))
            }
            ExprKind::Try(inner) => self.try_expr(inner, e.span),
            ExprKind::Ref(..) => err(
                "`&` is only used when passing an argument to a `&` or `&mut` parameter (Om has no reference values)",
                e.span,
            ),
            ExprKind::Cast(inner, t) => {
                let to = self.resolve(t)?;
                // A literal cast to a number type is a literal of that type: `5 as u8`, `5u8`.
                if let (ExprKind::Int(n), Ty::Int(it)) = (&inner.kind, &to) {
                    return self.int_lit(*n, *it, e.span);
                }
                if let (ExprKind::Float(x), Ty::F32) = (&inner.kind, &to) {
                    return Ok(self.float_lit(*x, true));
                }
                let (v, from) = self.expr(inner)?;
                self.cast(v, from, to, e.span)
            }
            ExprKind::Interp(parts) => {
                // Evaluate every part first, then format them into a new String.
                let mut vals = Vec::new();
                for part in parts {
                    if let InterpPart::Expr(x) = part {
                        vals.push(self.borrow_expect(x, None)?);
                    }
                }
                let as_str = expected == Some(&Ty::Str);
                let (v, t) = self.capture(|this| {
                    let mut it = vals.iter();
                    for part in parts {
                        match part {
                            InterpPart::Lit(text) => this.write_str(text)?,
                            InterpPart::Expr(_) => {
                                let (v, t) = it.next().unwrap();
                                this.write_value(v, t, false)?;
                            }
                        }
                    }
                    Ok(())
                })?;
                if as_str {
                    // Where a `str` is wanted, the String lives until the end of the statement.
                    self.temps.push((v.clone(), t));
                    return Ok((v[..2].to_vec(), Ty::Str));
                }
                Ok((v, t))
            }
            ExprKind::If(cond, then, els) => self.if_expr(cond, then, els.as_ref(), e.span, expected),
            ExprKind::Match(scrut, arms) => self.match_expr(scrut, arms, e.span, expected),
            ExprKind::Loop(body, label) => {
                let header = self.b.create_block();
                let body_blk = self.b.create_block();
                let mut exit = self.new_merge();
                exit.ty = None;
                self.b.ins().jump(header, &[]);
                self.b.switch_to_block(header);
                self.b.ins().jump(body_blk, &[]);
                let was_dead = self.dead;
                let exit = self.loop_body(body_blk, header, exit, true, label.clone(), body);
                self.b.seal_block(header);
                let exit = exit?;
                let ty = match &exit.ty {
                    None => return Ok((vec![], Ty::Never)),
                    Some(t) => t.clone(),
                };
                self.dead = was_dead;
                Ok((self.b.block_params(exit.blk).to_vec(), ty))
            }
            ExprKind::Block(b) => self.block_expect(b, expected),
            ExprKind::Index(base, idx) => {
                let (loc, ty) = match self.place(e)? {
                    Some(p) => {
                        self.check_usable(p.root_id, e.span)?;
                        (p.loc, p.ty)
                    }
                    None => {
                        let (bv, bt) = self.borrow_expect(base, None)?;
                        self.index_loc(bv, &bt, idx, e.span)?
                    }
                };
                if self.cg.needs_drop(&ty) {
                    return err(
                        "cannot move an element out of a list; use `.clone()`, `remove` or `swap_remove`",
                        e.span,
                    );
                }
                Ok((self.read_loc(&loc), ty))
            }
            ExprKind::List(items) => self.list_lit(items, e.span, expected),
            ExprKind::ListRepeat(v, n) => self.list_repeat(v, n, expected),
        }
    }

    fn int_lit(&mut self, n: i128, t: IntTy, span: Span) -> CResult<(Vals, Ty)> {
        if n < t.min() || n > t.max() {
            return err(format!("literal `{n}` does not fit in {} (range {}..={})", t.name(), t.min(), t.max()), span);
        }
        let bits = t.bits();
        let raw = if bits == 64 { n as i64 } else { (n & ((1i128 << bits) - 1)) as i64 };
        Ok((vec![self.b.ins().iconst(t.cl(), raw)], Ty::Int(t)))
    }

    fn float_lit(&mut self, x: f64, f32: bool) -> (Vals, Ty) {
        if f32 {
            (vec![self.b.ins().f32const(x as f32)], Ty::F32)
        } else {
            (vec![self.b.ins().f64const(x)], Ty::F64)
        }
    }

    /// Lower both operands of a binary operator. A literal takes its type from the other
    /// side, so `1 + x` and `x + 1` both work when `x: u8`. Literals have no side effects,
    /// so evaluating the right side first in that case is not observable.
    fn operands(&mut self, l: &Expr, r: &Expr, expected: Option<&Ty>, op: BinOp) -> CResult<((Vals, Ty), (Vals, Ty))> {
        let shift = matches!(op, BinOp::Shl | BinOp::Shr);
        // Comparisons only look at their operands; other operators consume them.
        let look = matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge);
        let eval = |this: &mut Self, e: &Expr, hint: Option<&Ty>| {
            if look {
                this.borrow_expect(e, hint)
            } else {
                this.expr_expect(e, hint)
            }
        };
        // An arithmetic result type also types a literal on either side (`let x: u8 = 1 + 2`).
        let hint = expected.filter(|t| matches!(t, Ty::Int(_) | Ty::F32));
        if is_literal(l) && !is_literal(r) && !shift {
            let rr = eval(self, r, hint)?;
            let ll = eval(self, l, Some(&rr.1))?;
            return Ok((ll, rr));
        }
        let ll = eval(self, l, hint)?;
        let rhint = if shift { None } else { Some(&ll.1) };
        let rr = eval(self, r, rhint)?;
        Ok((ll, rr))
    }

    fn binop(&mut self, op: BinOp, lv: Vals, lt: Ty, mut rv: Vals, rt: Ty, span: Span) -> CResult<(Vals, Ty)> {
        if lt == Ty::Never || rt == Ty::Never {
            return Ok((vec![], Ty::Never));
        }
        if matches!(op, BinOp::Eq | BinOp::Ne) && (is_string(&lt) && rt == Ty::Str || lt == Ty::Str && is_string(&rt)) {
            // `String == str` compares the text, as in Rust. A String's first two
            // slots (buffer, length) are a view of it.
            return self.binop(op, lv[..2].to_vec(), Ty::Str, rv[..2].to_vec(), Ty::Str, span);
        }
        if op == BinOp::Add && is_string(&lt) && (rt == Ty::Str || is_string(&rt)) {
            // `String + str` appends, reusing the left String's buffer (as in Rust).
            // A String on the right (such as an interpolated string) is used up.
            let f = if rt == Ty::Str { "String::add_str" } else { "String::add_string" };
            let a = self.temp_arg(lv, lt, false, span)?;
            let b = self.temp_arg(rv, rt, false, span)?;
            return self.call_fn(f, &[a, b], span, None, Subst::new());
        }
        if let (BinOp::Shl | BinOp::Shr, Ty::Int(a), Ty::Int(b)) = (op, &lt, &rt) {
            // The shift amount may be any integer type; Cranelift masks it to the width.
            let v = rv[0];
            rv[0] = match a.bits().cmp(&b.bits()) {
                std::cmp::Ordering::Equal => v,
                std::cmp::Ordering::Less => self.b.ins().ireduce(a.cl(), v),
                std::cmp::Ordering::Greater => self.b.ins().uextend(a.cl(), v),
            };
            return self.int_binop(op, *a, lv[0], rv[0], span);
        }
        if lt != rt {
            return err(format!("mismatched types: {lt} {} {rt} (convert with `as`)", op.symbol()), span);
        }
        if op == BinOp::Rem && lt.is_float() {
            // Cranelift has no float remainder instruction; use C's fmod.
            let id = self.cg.runtime_fn_sig("om_rt_math_fmod", &[types::F64, types::F64], types::F64)?;
            let fref = self.cg.module.declare_func_in_func(id, self.b.func);
            let (mut a, mut b) = (lv[0], rv[0]);
            if lt == Ty::F32 {
                a = self.b.ins().fpromote(types::F64, a);
                b = self.b.ins().fpromote(types::F64, b);
            }
            let call = self.b.ins().call(fref, &[a, b]);
            let mut r = self.b.inst_results(call)[0];
            if lt == Ty::F32 {
                r = self.b.ins().fdemote(types::F32, r);
            }
            return Ok((vec![r], lt));
        }
        let ins = self.b.ins();
        let (l, r) = match (lv.first(), rv.first()) {
            (Some(&l), Some(&r)) => (l, r),
            _ => return self.overloaded(op, lv, lt, rv, span),
        };
        let cmp = matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge);
        let v = match &lt {
            Ty::Int(t) => return self.int_binop(op, *t, l, r, span),
            Ty::F32 | Ty::F64 if cmp => {
                let cc = match op {
                    BinOp::Eq => FloatCC::Equal,
                    BinOp::Ne => FloatCC::NotEqual,
                    BinOp::Lt => FloatCC::LessThan,
                    BinOp::Le => FloatCC::LessThanOrEqual,
                    BinOp::Gt => FloatCC::GreaterThan,
                    _ => FloatCC::GreaterThanOrEqual,
                };
                return Ok((vec![ins.fcmp(cc, l, r)], Ty::Bool));
            }
            Ty::F32 | Ty::F64 => match op {
                BinOp::Add => ins.fadd(l, r),
                BinOp::Sub => ins.fsub(l, r),
                BinOp::Mul => ins.fmul(l, r),
                BinOp::Div => ins.fdiv(l, r),
                _ => return err(format!("operator `{}` is not defined for {lt}", op.symbol()), span),
            },
            Ty::Bool | Ty::Char if cmp => {
                if lt == Ty::Bool && !matches!(op, BinOp::Eq | BinOp::Ne) {
                    return err(format!("`{}` is not defined for bool", op.symbol()), span);
                }
                return Ok((vec![ins.icmp(uint_cc(op), l, r)], Ty::Bool));
            }
            Ty::Bool => match op {
                BinOp::BitAnd => ins.band(l, r),
                BinOp::BitOr => ins.bor(l, r),
                BinOp::BitXor => ins.bxor(l, r),
                _ => return err(format!("operator `{}` is not defined for bool", op.symbol()), span),
            },
            Ty::Char => return err(format!("operator `{}` is not defined for char", op.symbol()), span),
            _ => return self.overloaded(op, lv, lt, rv, span),
        };
        Ok((vec![v], lt))
    }

    fn int_binop(&mut self, op: BinOp, t: IntTy, l: Value, r: Value, span: Span) -> CResult<(Vals, Ty)> {
        let ins = self.b.ins();
        let s = t.signed();
        let v = match op {
            BinOp::Add => ins.iadd(l, r),
            BinOp::Sub => ins.isub(l, r),
            BinOp::Mul => ins.imul(l, r),
            BinOp::Div if s => ins.sdiv(l, r),
            BinOp::Div => ins.udiv(l, r),
            BinOp::Rem if s => ins.srem(l, r),
            BinOp::Rem => ins.urem(l, r),
            BinOp::BitAnd => ins.band(l, r),
            BinOp::BitOr => ins.bor(l, r),
            BinOp::BitXor => ins.bxor(l, r),
            BinOp::Shl => ins.ishl(l, r),
            BinOp::Shr if s => ins.sshr(l, r),
            BinOp::Shr => ins.ushr(l, r),
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let cc = if s { sint_cc(op) } else { uint_cc(op) };
                return Ok((vec![ins.icmp(cc, l, r)], Ty::Bool));
            }
            BinOp::And | BinOp::Or => return err(format!("`{}` needs bools, found {}", op.symbol(), t.name()), span),
        };
        Ok((vec![v], Ty::Int(t)))
    }

    /// Store `vals` in a fresh variable and return an argument that reads it (as `&` if `by_ref`).
    fn temp_arg(&mut self, vals: Vals, ty: Ty, by_ref: bool, span: Span) -> CResult<Arg> {
        let tmp = self.fresh_tmp();
        let own = if by_ref { Own::Borrowed } else { Own::Owned };
        self.bind_local(&tmp, ty, vals, false, own, span)?;
        let mut value = Expr { kind: ExprKind::Var(tmp), span };
        if by_ref {
            value = Expr { kind: ExprKind::Ref(false, Box::new(value)), span };
        }
        Ok(Arg { name: None, value, span })
    }

    /// Field-by-field equality for a tuple, a plain struct, or an enum whose
    /// variants carry no data. `None` if the type is not one of those.
    fn structural_eq(&mut self, lv: &[Value], rv: &[Value], ty: &Ty, span: Span) -> CResult<Option<Value>> {
        let members: Vec<Ty> = match ty {
            Ty::Tuple(ts) => ts.clone(),
            Ty::Struct(n, _) if !PRIVATE_STRUCTS.contains(&n.as_str()) => {
                self.cg.fields(ty).into_iter().map(|(_, t)| t).collect()
            }
            Ty::Enum(..) if self.cg.variants(ty).iter().all(|(_, fs)| fs.is_empty()) => {
                return Ok(Some(self.b.ins().icmp(IntCC::Equal, lv[0], rv[0])));
            }
            _ => return Ok(None),
        };
        let mut acc = self.b.ins().iconst(types::I8, 1);
        let mut off = 0;
        for t in members {
            let w = self.cg.width(&t);
            let (v, vt) =
                self.binop(BinOp::Eq, lv[off..off + w].to_vec(), t.clone(), rv[off..off + w].to_vec(), t, span)?;
            if vt != Ty::Bool || v.is_empty() {
                return Ok(None);
            }
            acc = self.b.ins().band(acc, v[0]);
            off += w;
        }
        Ok(Some(acc))
    }

    /// An operator on a user type, through the trait that defines it (`Add`, `Eq`, `Ord`, ...).
    fn overloaded(&mut self, op: BinOp, lv: Vals, ty: Ty, rv: Vals, span: Span) -> CResult<(Vals, Ty)> {
        let (tr, m, by_ref) = match op {
            BinOp::Add => ("Add", "add", false),
            BinOp::Sub => ("Sub", "sub", false),
            BinOp::Mul => ("Mul", "mul", false),
            BinOp::Div => ("Div", "div", false),
            BinOp::Rem => ("Rem", "rem", false),
            BinOp::Eq | BinOp::Ne => ("Eq", "eq", true),
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => ("Ord", "cmp", true),
            BinOp::And | BinOp::Or => return err(format!("`{}` needs bools, found {ty}", op.symbol()), span),
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                return err(format!("operator `{}` is not defined for {ty}", op.symbol()), span)
            }
        };
        let has_eq_impl = ty.key().is_some_and(|k| self.cg.defs.contains_key(&format!("{k}::eq")));
        if tr == "Eq" && !has_eq_impl {
            // No `Eq` impl: compare tuples and structs field by field, and
            // enums without payloads by variant, like Rust's `#[derive(PartialEq)]`.
            if let Some(eq) = self.structural_eq(&lv, &rv, &ty, span)? {
                let out = if op == BinOp::Ne { self.b.ins().bxor_imm_u(eq, 1) } else { eq };
                return Ok((vec![out], Ty::Bool));
            }
        }
        if !self.cg.implements(&ty, tr) {
            return err(format!("operator `{}` is not defined for {ty}; implement `{tr}` for it", op.symbol()), span);
        }
        let key = format!("{}::{m}", ty.key().unwrap());
        let preset = Codegen::type_subst(&self.cg.defs[&key].impl_generics, ty.args());
        let a = self.temp_arg(lv, ty.clone(), by_ref, span)?;
        let b = self.temp_arg(rv, ty.clone(), by_ref, span)?;
        let (v, rt) = self.call_fn(&key, &[a, b], span, None, preset)?;
        if self.dead {
            return Ok((vec![], Ty::Never));
        }
        let ins = self.b.ins();
        // Ordering's tag: Less = 0, Equal = 1, Greater = 2.
        let out = match op {
            BinOp::Ne => ins.bxor_imm_u(v[0], 1),
            BinOp::Lt => ins.icmp_imm_s(IntCC::Equal, v[0], 0),
            BinOp::Le => ins.icmp_imm_s(IntCC::NotEqual, v[0], 2),
            BinOp::Gt => ins.icmp_imm_s(IntCC::Equal, v[0], 2),
            BinOp::Ge => ins.icmp_imm_s(IntCC::NotEqual, v[0], 0),
            _ => return Ok((v, rt)),
        };
        Ok((vec![out], Ty::Bool))
    }

    fn cast(&mut self, v: Vals, from: Ty, to: Ty, span: Span) -> CResult<(Vals, Ty)> {
        let unit_enum = matches!(&from, Ty::Enum(..)) && self.cg.variants(&from).iter().all(|(_, f)| f.is_empty());
        let fty = |t: &Ty| if *t == Ty::F32 { types::F32 } else { types::F64 };
        let out = match (&from, &to) {
            (Ty::Never, _) => return Ok((vec![], Ty::Never)),
            (a, b) if a == b => return Ok((v, to)),
            (Ty::Int(a), Ty::Int(b)) => self.int_resize(v[0], *a, *b),
            (Ty::Bool, Ty::Int(b)) => self.int_resize(v[0], IntTy::U8, *b),
            (Ty::Char, Ty::Int(b)) => self.int_resize(v[0], IntTy::U32, *b),
            (Ty::Int(IntTy::U8), Ty::Char) => self.b.ins().uextend(types::I32, v[0]),
            (Ty::Enum(..), Ty::Int(b)) if unit_enum => self.int_resize(v[0], IntTy::I64, *b),
            (Ty::Int(a), Ty::F32 | Ty::F64) if a.signed() => self.b.ins().fcvt_from_sint(fty(&to), v[0]),
            (Ty::Int(_), Ty::F32 | Ty::F64) => self.b.ins().fcvt_from_uint(fty(&to), v[0]),
            (Ty::F32, Ty::F64) => self.b.ins().fpromote(types::F64, v[0]),
            (Ty::F64, Ty::F32) => self.b.ins().fdemote(types::F32, v[0]),
            // Like Rust, float-to-int casts saturate and map NaN to 0.
            (Ty::F32 | Ty::F64, Ty::Int(b)) => {
                let ins = self.b.ins();
                let wide = if b.signed() {
                    ins.fcvt_to_sint_sat(types::I64, v[0])
                } else {
                    ins.fcvt_to_uint_sat(types::I64, v[0])
                };
                if b.bits() == 64 {
                    wide
                } else {
                    let clamped = if b.signed() {
                        let lo = self.b.ins().iconst(types::I64, b.min() as i64);
                        let hi = self.b.ins().iconst(types::I64, b.max() as i64);
                        let x = self.b.ins().smax(wide, lo);
                        self.b.ins().smin(x, hi)
                    } else {
                        let hi = self.b.ins().iconst(types::I64, b.max() as i64);
                        self.b.ins().umin(wide, hi)
                    };
                    self.b.ins().ireduce(b.cl(), clamped)
                }
            }
            _ => return err(format!("cannot cast {from} to {to}"), span),
        };
        Ok((vec![out], to))
    }

    /// Convert an integer between widths, sign- or zero-extending by the source type.
    fn int_resize(&mut self, v: Value, from: IntTy, to: IntTy) -> Value {
        let ins = self.b.ins();
        match from.bits().cmp(&to.bits()) {
            std::cmp::Ordering::Equal => v,
            std::cmp::Ordering::Greater => ins.ireduce(to.cl(), v),
            std::cmp::Ordering::Less if from.signed() => ins.sextend(to.cl(), v),
            std::cmp::Ordering::Less => ins.uextend(to.cl(), v),
        }
    }

    fn short_circuit(&mut self, op: BinOp, l: &Expr, r: &Expr) -> CResult<(Vals, Ty)> {
        let lv = self.cond(l)?;
        let rhs_blk = self.b.create_block();
        let merge = self.b.create_block();
        self.b.append_block_param(merge, types::I8);
        if op == BinOp::And {
            self.b.ins().brif(lv, rhs_blk, &[], merge, &[lv.into()]);
        } else {
            self.b.ins().brif(lv, merge, &[lv.into()], rhs_blk, &[]);
        }
        self.b.switch_to_block(rhs_blk);
        self.b.seal_block(rhs_blk);
        let rv = self.cond(r)?;
        self.b.ins().jump(merge, &[rv.into()]);
        self.b.switch_to_block(merge);
        self.b.seal_block(merge);
        Ok((vec![self.b.block_params(merge)[0]], Ty::Bool))
    }

    fn if_expr(
        &mut self,
        cond: &Expr,
        then: &Block,
        els: Option<&Block>,
        span: Span,
        expected: Option<&Ty>,
    ) -> CResult<(Vals, Ty)> {
        // `if a { "{x} items" } else { "none" }`: one branch is an interpolated
        // String, so the other's `str` becomes a String too.
        let st = string_ty();
        let interp = |b: &Block| matches!(b.tail.as_deref(), Some(Expr { kind: ExprKind::Interp(_), .. }));
        let expected =
            if expected.is_none() && (interp(then) || els.is_some_and(interp)) { Some(&st) } else { expected };
        let c = self.cond(cond)?;
        let then_blk = self.b.create_block();
        let else_blk = self.b.create_block();
        let mut m = self.new_merge();
        self.b.ins().brif(c, then_blk, &[], else_blk, &[]);
        let was_dead = self.dead;

        self.b.switch_to_block(then_blk);
        self.b.seal_block(then_blk);
        self.dead = false;
        let before = self.moved.clone();
        let (mut tv, mut tt) = self.block_expect(then, expected)?;
        let then_moved = (!self.dead).then(|| std::mem::replace(&mut self.moved, before.clone()));
        self.moved = before;
        if els.is_none() && tt != Ty::Unit && tt != Ty::Never {
            // `if full { list.remove(0) }`: the value is unused, so drop it.
            if !self.dead {
                self.drop_value(&tv, &tt)?;
            }
            tv = vec![];
            tt = Ty::Unit;
        }
        if !self.dead {
            self.jump_to(&mut m, tt, tv, "`if` and `else`", span)?;
        }

        self.b.switch_to_block(else_blk);
        self.b.seal_block(else_blk);
        self.dead = false;
        let hint = m.ty.clone().or(expected.cloned());
        let (ev, et) = match els {
            Some(b) => self.block_expect(b, hint.as_ref())?,
            None => (vec![], Ty::Unit),
        };
        if !self.dead {
            self.jump_to(&mut m, et, ev, "`if` and `else`", span)?;
            if let Some(tm) = then_moved {
                self.moved.extend(tm);
            }
        } else if let Some(tm) = then_moved {
            self.moved = tm;
        }
        Ok(self.finish_merge(m, was_dead))
    }

    // ---------- structs, enums, Option and Result ----------

    /// Replace a leading `Self` in a path with the current `impl` type's name.
    fn self_name(&self, name: &str) -> String {
        self.type_path(name).0
    }

    /// Replace a leading `Self` or type parameter in a path with the concrete type's
    /// name: `T::default` becomes `i64::default`. Also returns that type.
    fn type_path(&self, name: &str) -> (String, Option<Ty>) {
        let (head, rest) = match name.split_once("::") {
            Some((h, r)) => (h, Some(r)),
            None if name == "Self" => (name, None),
            None => return (name.to_string(), None),
        };
        match self.subst.get(head).and_then(|t| Some((t.key()?, t))) {
            Some((k, t)) => (rest.map_or(k.clone(), |r| format!("{k}::{r}")), Some(t.clone())),
            None => (name.to_string(), None),
        }
    }

    /// Generic parameters of a struct or enum, bound from the expected type if it is the same type.
    fn seed_subst(&self, base: &str, generics: &[String], expected: Option<&Ty>) -> Subst {
        let mut s = Subst::new();
        if let Some(Ty::Struct(n, args) | Ty::Enum(n, args)) = expected {
            if n == base {
                s = Codegen::type_subst(generics, args);
            }
        }
        // Inside `impl[T] Foo[T]`, `Foo { .. }` and `Self::V` refer to the same `T`.
        if let Some(Ty::Struct(n, args) | Ty::Enum(n, args)) = self.subst.get("Self") {
            if n == base && s.is_empty() {
                s = Codegen::type_subst(generics, args);
            }
        }
        s
    }

    /// Lower `e` where the declared type is `te`, inferring type variables in `vars`.
    fn infer_arg(
        &mut self,
        e: &Expr,
        te: &TypeExpr,
        vars: &[String],
        subst: &mut Subst,
        what: &str,
    ) -> CResult<(Vals, Ty)> {
        let hint = self.cg.resolve(te, &self.merged(subst)).ok();
        let (v, t) = self.expr_expect(e, hint.as_ref())?;
        if !self.cg.unify(te, &t, vars, subst) {
            let want = hint.map(|h| h.to_string()).unwrap_or_else(|| te_string(te));
            return err(format!("{what} should be {want}, found {t}"), e.span);
        }
        Ok((v, t))
    }

    /// The type arguments to resolve a callee's signature with.
    fn merged(&self, subst: &Subst) -> Subst {
        // The callee's type parameters are its own, even when they share a name with
        // ours (`fn f[T]` calling `g[T]`), so only `subst` applies to its signature.
        subst.clone()
    }

    fn require_all(&self, vars: &[String], subst: &Subst, what: &str, span: Span) -> CResult<Vec<Ty>> {
        vars.iter()
            .map(|g| match subst.get(g) {
                Some(t) => Ok(t.clone()),
                None => err(format!("cannot infer type `{g}` for {what}; add a type annotation"), span),
            })
            .collect()
    }

    fn struct_lit(
        &mut self,
        name: &str,
        fields: &[(String, Expr)],
        span: Span,
        expected: Option<&Ty>,
    ) -> CResult<(Vals, Ty)> {
        let name = self.self_name(name);
        if PRIVATE_STRUCTS.contains(&name.as_str()) && !self.in_prelude {
            return err(format!("`{name}` has private fields; use `{name}::new()` to make one"), span);
        }
        let Some((generics, defs)) = self.cg.structs.get(&name).cloned() else {
            return err(format!("cannot find struct `{name}`"), span);
        };
        let mut subst = self.seed_subst(&name, &generics, expected);
        let mut given: Vec<Option<Vals>> = vec![None; defs.len()];
        for (f, value) in fields {
            let Some(i) = defs.iter().position(|(n, _)| n == f) else {
                return err(format!("struct `{name}` has no field `{f}`"), value.span);
            };
            if given[i].is_some() {
                return err(format!("field `{f}` is given twice"), value.span);
            }
            let (v, _) = self.infer_arg(value, &defs[i].1, &generics, &mut subst, &format!("field `{f}`"))?;
            given[i] = Some(v);
        }
        let missing: Vec<&str> =
            defs.iter().zip(&given).filter(|(_, g)| g.is_none()).map(|((n, _), _)| n.as_str()).collect();
        if !missing.is_empty() {
            return err(format!("missing field(s) in `{name}`: {}", missing.join(", ")), span);
        }
        let args = self.require_all(&generics, &subst, &format!("`{name}`"), span)?;
        Ok((given.into_iter().flatten().flatten().collect(), Ty::Struct(name, args)))
    }

    /// Find the enum and variant index for `E::V`, `Self::V`, or a prelude name like `Some`.
    fn find_variant(&self, path: &str) -> Option<(String, usize)> {
        let path = self.self_name(path);
        let path = prelude_variant(&path).map(str::to_string).unwrap_or(path);
        let (e, v) = path.rsplit_once("::")?;
        let (_, variants) = self.cg.enums.get(e)?;
        let idx = variants.iter().position(|(n, _)| n == v)?;
        Some((e.to_string(), idx))
    }

    /// Assemble an enum value from its variant index and payload.
    fn build_variant(&mut self, ty: &Ty, idx: usize, payload: Vals) -> Vals {
        let mut vals = vec![self.b.ins().iconst(types::I64, idx as i64)];
        let mut payload = payload.into_iter();
        for (i, (_, tys)) in self.cg.variants(ty).iter().enumerate() {
            for t in tys {
                for st in self.cg.layout(t) {
                    let v = if i == idx { payload.next().unwrap() } else { self.zero(st) };
                    vals.push(v);
                }
            }
        }
        vals
    }

    /// Build an enum value if `path` names a variant; `None` if it doesn't.
    fn variant(&mut self, path: &str, args: &[Arg], span: Span, expected: Option<&Ty>) -> CResult<Option<(Vals, Ty)>> {
        let Some((e, idx)) = self.find_variant(path) else { return Ok(None) };
        let (generics, variants) = self.cg.enums[&e].clone();
        let (vname, ftes) = variants[idx].clone();
        if args.len() != ftes.len() {
            return err(format!("`{e}::{vname}` takes {} value(s) but {} were given", ftes.len(), args.len()), span);
        }
        let mut subst = self.seed_subst(&e, &generics, expected);
        let mut payload = Vec::new();
        for (a, te) in args.iter().zip(&ftes) {
            if a.name.is_some() {
                return err("enum variants do not take named arguments", a.span);
            }
            let (v, _) = self.infer_arg(&a.value, te, &generics, &mut subst, "this value")?;
            payload.extend(v);
        }
        let targs = self.require_all(&generics, &subst, &format!("`{vname}`"), span)?;
        let ty = Ty::Enum(e, targs);
        let vals = self.build_variant(&ty, idx, payload);
        Ok(Some((vals, ty)))
    }

    /// `value?` on an `Option` or `Result`.
    fn try_expr(&mut self, inner: &Expr, span: Span) -> CResult<(Vals, Ty)> {
        let (v, ty) = self.expr(inner)?;
        let family = |t: &Ty| match t {
            Ty::Enum(n, _) if n == "Option" || n == "Result" => Some(n.clone()),
            _ => None,
        };
        let Some(kind) = family(&ty) else {
            return err(format!("`?` needs an Option or Result, found {ty}"), span);
        };
        let ret = self.ret.clone();
        if family(&ret).as_deref() != Some(kind.as_str()) {
            return err(
                format!("`?` on {ty} can only be used in a function that returns {kind}, but this one returns {ret}"),
                span,
            );
        }
        let ok_payload = self.cg.variants(&ty)[0].1.clone();
        let ok_off = self.cg.variant_offset(&ty, 0);
        let ok_w: usize = ok_payload.iter().map(|t| self.cg.width(t)).sum();
        let ok_ty = ok_payload.first().cloned().unwrap_or(Ty::Unit);
        let early = if kind == "Result" {
            let err_ty = &self.cg.variants(&ty)[1].1[0];
            let ret_err = &self.cg.variants(&ret)[1].1[0];
            if err_ty != ret_err {
                return err(format!("`?` cannot convert the error type {err_ty} into {ret_err}"), span);
            }
            let off = self.cg.variant_offset(&ty, 1);
            let w = self.cg.width(err_ty);
            v[off..off + w].to_vec()
        } else {
            vec![]
        };
        let ok_blk = self.b.create_block();
        let fail_blk = self.b.create_block();
        // Variant 0 is `Some` / `Ok` in the prelude.
        let is_ok = self.b.ins().icmp_imm_s(IntCC::Equal, v[0], 0);
        self.b.ins().brif(is_ok, ok_blk, &[], fail_blk, &[]);
        self.b.switch_to_block(fail_blk);
        self.b.seal_block(fail_blk);
        let ret_vals = self.build_variant(&ret, 1, early);
        self.do_return(ret_vals)?;
        self.b.switch_to_block(ok_blk);
        self.b.seal_block(ok_blk);
        Ok((v[ok_off..ok_off + ok_w].to_vec(), ok_ty))
    }

    // ---------- calls ----------

    fn call(&mut self, name: &str, args: &[Arg], span: Span, expected: Option<&Ty>) -> CResult<(Vals, Ty)> {
        if let Some(op) = name.strip_prefix("intrinsic::") {
            return self.intrinsic(op, args, span, expected);
        }
        match name {
            "print" => return self.print(args, span, false),
            "eprint" => return self.print(args, span, true),
            "panic" => return self.panic(args, span),
            "assert" => return self.assert(args, span),
            _ => {}
        }
        let (name, path_ty) = self.type_path(name);
        if let Some(r) = self.variant(&name, args, span, expected)? {
            return Ok(r);
        }
        if !self.cg.defs.contains_key(&name) {
            if let Some((ty, f)) = name.rsplit_once("::") {
                if self.cg.structs.contains_key(ty) || self.cg.enums.contains_key(ty) {
                    return err(format!("`{ty}` has no method or variant named `{f}`"), span);
                }
            }
            return err(format!("cannot find function `{name}`"), span);
        }
        if name == "main" {
            return err("`main` cannot be called", span);
        }
        // `Type::f(...)` inside `impl[T] Type[T]` shares the impl's `T`.
        let mut preset = Subst::new();
        if let Some((ty, _)) = name.split_once("::") {
            let def = &self.cg.defs[&name];
            if let Some(t) = path_ty {
                preset = Codegen::type_subst(&def.impl_generics, t.args());
            } else if let Some(Ty::Struct(n, a) | Ty::Enum(n, a)) = self.subst.get("Self") {
                if n == ty {
                    preset = Codegen::type_subst(&def.impl_generics, a);
                }
            }
        }
        self.call_fn(&name, args, span, expected, preset)
    }

    /// Release what a value owns.
    fn drop_value(&mut self, vals: &[Value], ty: &Ty) -> CResult<()> {
        if self.dead || !self.cg.needs_drop(ty) {
            return Ok(());
        }
        let id = self.cg.glue_fn(ty, Glue::Drop)?;
        let f = self.cg.module.declare_func_in_func(id, self.b.func);
        self.b.ins().call(f, vals);
        Ok(())
    }

    /// The user's (or prelude's) `clone` method for `ty`, if it has one.
    fn clone_impl(&self, ty: &Ty) -> Option<String> {
        let key = format!("{}::clone", ty.key()?);
        self.cg.defs.contains_key(&key).then_some(key)
    }

    /// A deep copy of a value.
    fn clone_value(&mut self, vals: &[Value], ty: &Ty) -> CResult<Vals> {
        if self.dead || (!self.cg.needs_drop(ty) && self.clone_impl(ty).is_none()) {
            return Ok(vals.to_vec());
        }
        let id = self.cg.glue_fn(ty, Glue::Clone)?;
        let f = self.cg.module.declare_func_in_func(id, self.b.func);
        let call = self.b.ins().call(f, vals);
        Ok(self.b.inst_results(call).to_vec())
    }

    /// Body of the generated drop function: the type's `Drop::drop`, then its fields.
    fn drop_glue_body(&mut self, vals: &[Value], ty: &Ty) -> CResult<()> {
        let mut vals = vals.to_vec();
        if self.cg.implements(ty, "Drop") {
            let key = format!("{}::drop", ty.key().unwrap());
            let tmp = self.fresh_tmp();
            self.bind_local(&tmp, ty.clone(), vals, true, Own::InOut, Span::default())?;
            let arg = Expr { kind: ExprKind::Var(tmp.clone()), span: Span::default() };
            let arg = Expr { kind: ExprKind::Ref(true, Box::new(arg)), span: Span::default() };
            let preset = Codegen::type_subst(&self.cg.defs[&key].impl_generics, ty.args());
            self.call_fn(
                &key,
                &[Arg { name: None, value: arg, span: Span::default() }],
                Span::default(),
                None,
                preset,
            )?;
            let vars = self.lookup(&tmp).unwrap().vars.clone();
            vals = self.read(&vars);
        }
        match ty {
            Ty::Struct(..) | Ty::Tuple(_) => {
                let parts: Vec<Ty> = match ty {
                    Ty::Tuple(ts) => ts.clone(),
                    _ => self.cg.fields(ty).into_iter().map(|(_, t)| t).collect(),
                };
                let mut off = 0;
                for t in parts {
                    let w = self.cg.width(&t);
                    self.drop_value(&vals[off..off + w], &t)?;
                    off += w;
                }
            }
            Ty::Enum(..) => {
                let variants = self.cg.variants(ty);
                let done = self.b.create_block();
                for (idx, (_, ftys)) in variants.iter().enumerate() {
                    if !ftys.iter().any(|t| self.cg.needs_drop(t)) {
                        continue;
                    }
                    let this = self.b.create_block();
                    let next = self.b.create_block();
                    let is = self.b.ins().icmp_imm_s(IntCC::Equal, vals[0], idx as i64);
                    self.b.ins().brif(is, this, &[], next, &[]);
                    self.b.switch_to_block(this);
                    self.b.seal_block(this);
                    let mut off = self.cg.variant_offset(ty, idx);
                    for t in ftys {
                        let w = self.cg.width(t);
                        self.drop_value(&vals[off..off + w], t)?;
                        off += w;
                    }
                    self.b.ins().jump(done, &[]);
                    self.b.switch_to_block(next);
                    self.b.seal_block(next);
                }
                self.b.ins().jump(done, &[]);
                self.b.switch_to_block(done);
                self.b.seal_block(done);
            }
            _ => {}
        }
        Ok(())
    }

    /// Body of the generated clone function: the type's own `clone`, or field by field.
    fn clone_glue_body(&mut self, vals: &[Value], ty: &Ty) -> CResult<Vals> {
        if let Some(key) = self.clone_impl(ty) {
            let tmp = self.fresh_tmp();
            self.bind_local(&tmp, ty.clone(), vals.to_vec(), false, Own::Borrowed, Span::default())?;
            let arg = Expr { kind: ExprKind::Var(tmp), span: Span::default() };
            let arg = Expr { kind: ExprKind::Ref(false, Box::new(arg)), span: Span::default() };
            let preset = Codegen::type_subst(&self.cg.defs[&key].impl_generics, ty.args());
            let args = [Arg { name: None, value: arg, span: Span::default() }];
            return Ok(self.call_fn(&key, &args, Span::default(), None, preset)?.0);
        }
        if self.cg.implements(ty, "Drop") {
            return err(
                format!("`{ty}` implements Drop, so it needs its own `Clone` impl to be cloned"),
                Span::default(),
            );
        }
        let mut out = vals.to_vec();
        match ty {
            Ty::Struct(..) | Ty::Tuple(_) => {
                let parts: Vec<Ty> = match ty {
                    Ty::Tuple(ts) => ts.clone(),
                    _ => self.cg.fields(ty).into_iter().map(|(_, t)| t).collect(),
                };
                let mut off = 0;
                for t in parts {
                    let w = self.cg.width(&t);
                    let c = self.clone_value(&vals[off..off + w], &t)?;
                    out[off..off + w].copy_from_slice(&c);
                    off += w;
                }
                Ok(out)
            }
            Ty::Enum(..) => {
                let variants = self.cg.variants(ty);
                let done = self.b.create_block();
                for t in self.cg.layout(ty) {
                    self.b.append_block_param(done, t);
                }
                for (idx, (_, ftys)) in variants.iter().enumerate() {
                    if !ftys.iter().any(|t| self.cg.needs_drop(t) || self.clone_impl(t).is_some()) {
                        continue;
                    }
                    let this = self.b.create_block();
                    let next = self.b.create_block();
                    let is = self.b.ins().icmp_imm_s(IntCC::Equal, vals[0], idx as i64);
                    self.b.ins().brif(is, this, &[], next, &[]);
                    self.b.switch_to_block(this);
                    self.b.seal_block(this);
                    let mut v = vals.to_vec();
                    let mut off = self.cg.variant_offset(ty, idx);
                    for t in ftys {
                        let w = self.cg.width(t);
                        let c = self.clone_value(&vals[off..off + w], t)?;
                        v[off..off + w].copy_from_slice(&c);
                        off += w;
                    }
                    let args: Vec<_> = v.iter().map(|&x| x.into()).collect();
                    self.b.ins().jump(done, &args);
                    self.b.switch_to_block(next);
                    self.b.seal_block(next);
                }
                let args: Vec<_> = vals.iter().map(|&x| x.into()).collect();
                self.b.ins().jump(done, &args);
                self.b.switch_to_block(done);
                self.b.seal_block(done);
                Ok(self.b.block_params(done).to_vec())
            }
            _ => Ok(out),
        }
    }

    /// Call a C runtime function.
    fn rt(
        &mut self,
        name: &str,
        params: &[types::Type],
        ret: Option<types::Type>,
        args: &[Value],
    ) -> CResult<Option<Value>> {
        let mut sig = self.cg.module.make_signature();
        sig.params.extend(params.iter().map(|&p| AbiParam::new(p)));
        sig.returns.extend(ret.map(AbiParam::new));
        let id = self.cg.module.declare_function(name, Linkage::Import, &sig).map_err(internal)?;
        let fref = self.cg.module.declare_func_in_func(id, self.b.func);
        let call = self.b.ins().call(fref, args);
        Ok(self.b.inst_results(call).first().copied())
    }

    /// Panic with a message made of literal text and runtime values.
    fn panic_with(&mut self, parts: Vec<PanicPart>, span: Span) -> CResult<()> {
        let mut ip = Vec::new();
        for (lit, val) in parts {
            if let Some(l) = lit {
                ip.push(InterpPart::Lit(l.to_string()));
            }
            if let Some((v, t)) = val {
                let tmp = self.fresh_tmp();
                self.bind_local(&tmp, t, vec![v], false, Own::Borrowed, span)?;
                ip.push(InterpPart::Expr(Expr { kind: ExprKind::Var(tmp), span }));
            }
        }
        let msg = Expr { kind: ExprKind::Interp(ip), span };
        self.panic(&[Arg { name: None, value: msg, span }], span)?;
        Ok(())
    }

    /// The value inside the box at place `p`.
    fn deref_place(&mut self, p: Place) -> Place {
        let elem = self.box_elem(&p.ty).unwrap();
        let ptr = self.read_loc(&p.loc)[0];
        let (slots, _, _) = self.cg.mem_layout(&elem);
        Place { loc: Loc::Mem(ptr, slots), ty: elem, ..p }
    }

    /// The contents type of a `Box`.
    fn box_elem(&self, ty: &Ty) -> Option<Ty> {
        match ty {
            Ty::Struct(n, a) if n == "Box" && a.len() == 1 => Some(a[0].clone()),
            _ => None,
        }
    }

    /// The element type of a `Vec`.
    fn vec_elem(&self, ty: &Ty) -> Option<Ty> {
        match ty {
            Ty::Struct(n, a) if n == "Vec" && a.len() == 1 => Some(a[0].clone()),
            _ => None,
        }
    }

    /// Bounds-check `list[idx]` and return where the element lives.
    fn index_loc(&mut self, list: Vals, list_ty: &Ty, idx: &Expr, span: Span) -> CResult<(Loc, Ty)> {
        let Some(elem) = self.vec_elem(list_ty) else {
            return err(format!("{list_ty} cannot be indexed with `[]`"), span);
        };
        let usize_ty = Ty::Int(IntTy::Usize);
        let (iv, it) = self.expr_expect(idx, Some(&usize_ty))?;
        // Any integer type can index: a negative index is simply out of bounds
        // (it becomes a huge unsigned number), so the one bounds check covers it.
        let Ty::Int(ity) = it else {
            return err(format!("list indexes must be integers, found {it}"), idx.span);
        };
        let shown_ty = if ity.signed() { Ty::Int(IntTy::I64) } else { usize_ty.clone() };
        let iv0 = self.int_resize(iv[0], ity, if ity.signed() { IntTy::I64 } else { IntTy::Usize });
        let (_, buf_off) = self.cg.field(list_ty, "buf").unwrap();
        let (_, len_off) = self.cg.field(list_ty, "len").unwrap();
        let (buf, len, i) = (list[buf_off], list[len_off], iv0);
        let ok = self.b.ins().icmp(IntCC::UnsignedLessThan, i, len);
        let fail = self.b.create_block();
        let cont = self.b.create_block();
        self.b.ins().brif(ok, cont, &[], fail, &[]);
        self.b.switch_to_block(fail);
        self.b.seal_block(fail);
        let was_dead = self.dead;
        self.panic_with(
            vec![
                (Some("index out of bounds: the len is "), Some((len, usize_ty.clone()))),
                (Some(" but the index is "), Some((i, shown_ty))),
            ],
            span,
        )?;
        self.b.switch_to_block(cont);
        self.b.seal_block(cont);
        self.dead = was_dead;
        let addr = self.elem_addr(buf, i, &elem);
        let (slots, _, _) = self.cg.mem_layout(&elem);
        Ok((Loc::Mem(addr, slots), elem))
    }

    /// Address of element `i` of an array of `elem` starting at `base`.
    fn elem_addr(&mut self, base: Value, i: Value, elem: &Ty) -> Value {
        let (_, stride, _) = self.cg.mem_layout(elem);
        let off = self.b.ins().imul_imm_s(i, stride as i64);
        self.b.ins().iadd(base, off)
    }

    fn store_elem(&mut self, base: Value, i: Value, elem: &Ty, vals: &[Value]) {
        let addr = self.elem_addr(base, i, elem);
        let (slots, _, _) = self.cg.mem_layout(elem);
        self.write_loc(&Loc::Mem(addr, slots), vals);
    }

    fn alloc(&mut self, count: Value, elem: &Ty) -> CResult<Value> {
        let (_, stride, _) = self.cg.mem_layout(elem);
        let bytes = self.b.ins().imul_imm_s(count, stride as i64);
        Ok(self.rt("om_rt_alloc", &[types::I64], Some(types::I64), &[bytes])?.unwrap())
    }

    /// Build a `Vec[elem]` value from its buffer, length and capacity.
    fn make_vec(&self, elem: &Ty, buf: Value, len: Value, cap: Value) -> (Vals, Ty) {
        let ty = Ty::Struct("Vec".into(), vec![elem.clone()]);
        let mut vals = vec![buf; 3];
        for (name, v) in [("buf", buf), ("len", len), ("cap", cap)] {
            vals[self.cg.field(&ty, name).unwrap().1] = v;
        }
        (vals, ty)
    }

    /// `[a, b, c]`
    fn list_lit(&mut self, items: &[Expr], span: Span, expected: Option<&Ty>) -> CResult<(Vals, Ty)> {
        let mut elem = expected.and_then(|t| self.vec_elem(t));
        let mut vals = Vec::new();
        for it in items {
            let (v, t) = self.expr_expect(it, elem.as_ref())?;
            if t == Ty::Never {
                return Ok((vec![], Ty::Never));
            }
            match &elem {
                Some(e) if *e != t => {
                    return err(format!("list elements must all have the same type: {e} and {t}"), it.span)
                }
                _ => elem = Some(t),
            }
            vals.push(v);
        }
        let Some(elem) = elem else {
            return err("cannot infer the element type of `[]`; annotate it, as in `let v: Vec[i64] = []`", span);
        };
        let n = self.b.ins().iconst(types::I64, items.len() as i64);
        let buf = if items.is_empty() { self.b.ins().iconst(types::I64, 0) } else { self.alloc(n, &elem)? };
        for (k, v) in vals.iter().enumerate() {
            let i = self.b.ins().iconst(types::I64, k as i64);
            self.store_elem(buf, i, &elem, v);
        }
        Ok(self.make_vec(&elem, buf, n, n))
    }

    /// `[value; count]`
    fn list_repeat(&mut self, value: &Expr, count: &Expr, expected: Option<&Ty>) -> CResult<(Vals, Ty)> {
        let hint = expected.and_then(|t| self.vec_elem(t));
        let (v, elem) = self.expr_expect(value, hint.as_ref())?;
        let usize_ty = Ty::Int(IntTy::Usize);
        let (nv, nt) = self.expr_expect(count, Some(&usize_ty))?;
        let Ty::Int(nty) = nt else {
            return err(format!("the count in `[value; count]` must be an integer, found {nt}"), count.span);
        };
        // A negative count makes an empty list.
        let mut n = self.int_resize(nv[0], nty, if nty.signed() { IntTy::I64 } else { IntTy::Usize });
        if nty.signed() {
            let zero = self.b.ins().iconst(types::I64, 0);
            n = self.b.ins().smax(n, zero);
        }
        let buf = self.alloc(n, &elem)?;
        // for i in 0..n: buf[i] = clone(value); the original value is moved into the last slot.
        let header = self.b.create_block();
        let body = self.b.create_block();
        let exit = self.b.create_block();
        self.b.append_block_param(header, types::I64);
        let zero = self.b.ins().iconst(types::I64, 0);
        self.b.ins().jump(header, &[zero.into()]);
        self.b.switch_to_block(header);
        let i = self.b.block_params(header)[0];
        let more = self.b.ins().icmp(IntCC::UnsignedLessThan, i, n);
        self.b.ins().brif(more, body, &[], exit, &[]);
        self.b.switch_to_block(body);
        self.b.seal_block(body);
        let copy = self.clone_value(&v, &elem)?;
        self.store_elem(buf, i, &elem, &copy);
        let next = self.b.ins().iadd_imm_s(i, 1);
        self.b.ins().jump(header, &[next.into()]);
        self.b.seal_block(header);
        self.b.switch_to_block(exit);
        self.b.seal_block(exit);
        self.drop_value(&v, &elem)?;
        Ok(self.make_vec(&elem, buf, n, n))
    }

    /// Built-in operations the prelude uses to implement number methods and collections.
    fn intrinsic(&mut self, op: &str, args: &[Arg], span: Span, expected: Option<&Ty>) -> CResult<(Vals, Ty)> {
        if op.starts_with("ptr_")
            || matches!(
                op,
                "drop_in_place"
                    | "needs_drop"
                    | "forget"
                    | "str_len"
                    | "str_ptr"
                    | "str_from_raw"
                    | "str_parse_f64"
                    | "str_parse_ok"
            )
        {
            return self.mem_intrinsic(op, args, span, expected);
        }
        match (op, args) {
            ("read_key", []) => {
                let k = self.rt("om_rt_read_key", &[], Some(types::I64), &[])?.unwrap();
                return Ok((vec![k], Ty::Int(IntTy::I64)));
            }
            ("argc", []) => {
                let n = self.rt("om_rt_argc", &[], Some(types::I64), &[])?.unwrap();
                return Ok((vec![n], Ty::Int(IntTy::I64)));
            }
            ("arg_ptr", [i]) | ("arg_len", [i]) => {
                let (v, _) = self.expr_expect(&i.value, Some(&Ty::Int(IntTy::I64)))?;
                let name = if op == "arg_ptr" { "om_rt_arg_ptr" } else { "om_rt_arg_len" };
                let r = self.rt(name, &[types::I64], Some(types::I64), &v)?.unwrap();
                let ty = if op == "arg_ptr" { Ty::Ptr(Box::new(Ty::Int(IntTy::U8))) } else { Ty::Int(IntTy::Usize) };
                return Ok((vec![r], ty));
            }
            ("read_line", []) => {
                let p = self.rt("om_rt_read_line", &[], Some(types::I64), &[])?.unwrap();
                let n = self.rt("om_rt_read_line_len", &[], Some(types::I64), &[])?.unwrap();
                return Ok((vec![p, n, n], string_ty()));
            }
            ("read_line_ok", []) => {
                let ok = self.rt("om_rt_read_line_ok", &[], Some(types::I8), &[])?.unwrap();
                return Ok((vec![ok], Ty::Bool));
            }
            ("poll_key", []) => {
                let k = self.rt("om_rt_poll_key", &[], Some(types::I64), &[])?.unwrap();
                return Ok((vec![k], Ty::Int(IntTy::I64)));
            }
            ("term_cols", []) | ("term_rows", []) => {
                let name = if op == "term_cols" { "om_rt_term_cols" } else { "om_rt_term_rows" };
                let k = self.rt(name, &[], Some(types::I64), &[])?.unwrap();
                return Ok((vec![k], Ty::Int(IntTy::I64)));
            }
            ("flush", []) => {
                self.rt("om_rt_flush", &[], None, &[])?;
                return Ok((vec![], Ty::Unit));
            }
            ("time_ns", []) => {
                let t = self.rt("om_rt_time_ns", &[], Some(types::I64), &[])?.unwrap();
                return Ok((vec![t], Ty::Int(IntTy::I64)));
            }
            ("sleep_ms", [ms]) => {
                let (v, _) = self.expr_expect(&ms.value, Some(&Ty::Int(IntTy::I64)))?;
                self.rt("om_rt_sleep_ms", &[types::I64], None, &v)?;
                return Ok((vec![], Ty::Unit));
            }
            _ => {}
        }
        let mut vals = Vec::new();
        let mut t = Ty::Unit;
        for a in args {
            let (v, ty) = self.expr(&a.value)?;
            vals.extend(v);
            t = ty;
        }
        let Some(&x) = vals.first() else { return err(format!("intrinsic `{op}` needs an argument"), span) };
        let ins = self.b.ins();
        let v = match (op, &t) {
            ("sqrt", Ty::F32 | Ty::F64) => ins.sqrt(x),
            ("floor", Ty::F32 | Ty::F64) => ins.floor(x),
            ("ceil", Ty::F32 | Ty::F64) => ins.ceil(x),
            ("trunc", Ty::F32 | Ty::F64) => ins.trunc(x),
            ("fabs", Ty::F32 | Ty::F64) => ins.fabs(x),
            ("popcnt" | "clz" | "ctz", Ty::Int(it)) => {
                let r = match op {
                    "popcnt" => ins.popcnt(x),
                    "clz" => ins.clz(x),
                    _ => ins.ctz(x),
                };
                let same_unsigned = match it.bits() {
                    8 => IntTy::U8,
                    16 => IntTy::U16,
                    32 => IntTy::U32,
                    _ => IntTy::U64,
                };
                return Ok((vec![self.int_resize(r, same_unsigned, IntTy::U32)], Ty::Int(IntTy::U32)));
            }
            ("u32_to_char", Ty::Int(IntTy::U32)) => return Ok((vec![x], Ty::Char)),
            (f, Ty::F64) if f.starts_with("libm_") => {
                let params = vec![types::F64; vals.len()];
                let id = self.cg.runtime_fn_sig(&format!("om_rt_math_{}", &f[5..]), &params, types::F64)?;
                let fref = self.cg.module.declare_func_in_func(id, self.b.func);
                let call = self.b.ins().call(fref, &vals);
                return Ok((vec![self.b.inst_results(call)[0]], Ty::F64));
            }
            _ => return err(format!("unknown intrinsic `{op}` for {t}"), span),
        };
        Ok((vec![v], t))
    }

    /// Raw memory operations for `Vec`, `String` and `Box` in the prelude. Like Rust's
    /// `ptr::read`, `ptr::write` and `mem::forget`, they are not checked for safety.
    fn mem_intrinsic(&mut self, op: &str, args: &[Arg], span: Span, expected: Option<&Ty>) -> CResult<(Vals, Ty)> {
        let usize_ty = Ty::Int(IntTy::Usize);
        let mut a = Vec::new();
        let mut tys = Vec::new();
        for (k, arg) in args.iter().enumerate() {
            // `ptr_write(p, i, value)`: the value's type comes from the pointer.
            let hint = match (op, k, tys.first()) {
                ("ptr_write", 2, Some(Ty::Ptr(t))) => Some((**t).clone()),
                (_, _, _) if k > 0 => Some(usize_ty.clone()),
                _ => None,
            };
            let (v, t) = self.expr_expect(&arg.value, hint.as_ref())?;
            a.push(v);
            tys.push(t);
        }
        let ptr_elem = |k: usize| -> CResult<Ty> {
            match tys.get(k) {
                Some(Ty::Ptr(t)) => Ok((**t).clone()),
                Some(t) => err(format!("intrinsic `{op}` needs a Ptr, found {t}"), span),
                None => err(format!("intrinsic `{op}` is missing an argument"), span),
            }
        };
        let want_ptr = || -> CResult<Ty> {
            match expected {
                Some(Ty::Ptr(t)) => Ok((**t).clone()),
                _ => err(format!("intrinsic `{op}` needs a known Ptr type"), span),
            }
        };
        let unit = Ok((vec![], Ty::Unit));
        match op {
            "ptr_null" => {
                let t = want_ptr()?;
                Ok((vec![self.b.ins().iconst(types::I64, 0)], Ty::Ptr(Box::new(t))))
            }
            "ptr_alloc" => {
                let t = want_ptr()?;
                let p = self.alloc(a[0][0], &t)?;
                Ok((vec![p], Ty::Ptr(Box::new(t))))
            }
            "ptr_realloc" => {
                let t = ptr_elem(0)?;
                let (_, stride, _) = self.cg.mem_layout(&t);
                let bytes = self.b.ins().imul_imm_s(a[1][0], stride as i64);
                let p = self.rt("om_rt_realloc", &[types::I64, types::I64], Some(types::I64), &[a[0][0], bytes])?;
                Ok((vec![p.unwrap()], tys[0].clone()))
            }
            "ptr_free" => {
                self.rt("om_rt_free", &[types::I64], None, &[a[0][0]])?;
                unit
            }
            "ptr_read" => {
                let t = ptr_elem(0)?;
                let addr = self.elem_addr(a[0][0], a[1][0], &t);
                let (slots, _, _) = self.cg.mem_layout(&t);
                Ok((self.read_loc(&Loc::Mem(addr, slots)), t))
            }
            "ptr_write" => {
                let t = ptr_elem(0)?;
                if tys[2] != t {
                    return err(format!("ptr_write of {} into {}", tys[2], tys[0]), span);
                }
                self.store_elem(a[0][0], a[1][0], &t, &a[2]);
                unit
            }
            "ptr_copy" => {
                // ptr_copy(src, src_index, dst, dst_index, count)
                let t = ptr_elem(0)?;
                let src = self.elem_addr(a[0][0], a[1][0], &t);
                let dst = self.elem_addr(a[2][0], a[3][0], &t);
                let (_, stride, _) = self.cg.mem_layout(&t);
                let bytes = self.b.ins().imul_imm_s(a[4][0], stride as i64);
                self.rt("om_rt_memmove", &[types::I64; 3], None, &[dst, src, bytes])?;
                unit
            }
            "ptr_bytes_eq" => {
                // ptr_bytes_eq(a, a_index, b, b_index, count): compare raw bytes.
                let t = ptr_elem(0)?;
                let x = self.elem_addr(a[0][0], a[1][0], &t);
                let y = self.elem_addr(a[2][0], a[3][0], &t);
                let (_, stride, _) = self.cg.mem_layout(&t);
                let bytes = self.b.ins().imul_imm_s(a[4][0], stride as i64);
                let r = self.rt("om_rt_memcmp", &[types::I64; 3], Some(types::I32), &[x, y, bytes])?.unwrap();
                Ok((vec![self.b.ins().icmp_imm_s(IntCC::Equal, r, 0)], Ty::Bool))
            }
            "ptr_cmp_bytes" => {
                // Like ptr_bytes_eq, but returns memcmp's sign as i32.
                let t = ptr_elem(0)?;
                let x = self.elem_addr(a[0][0], a[1][0], &t);
                let y = self.elem_addr(a[2][0], a[3][0], &t);
                let (_, stride, _) = self.cg.mem_layout(&t);
                let bytes = self.b.ins().imul_imm_s(a[4][0], stride as i64);
                let r = self.rt("om_rt_memcmp", &[types::I64; 3], Some(types::I32), &[x, y, bytes])?.unwrap();
                Ok((vec![r], Ty::Int(IntTy::I32)))
            }
            "drop_in_place" => {
                let t = ptr_elem(0)?;
                if self.cg.needs_drop(&t) {
                    let addr = self.elem_addr(a[0][0], a[1][0], &t);
                    let (slots, _, _) = self.cg.mem_layout(&t);
                    let v = self.read_loc(&Loc::Mem(addr, slots));
                    self.drop_value(&v, &t)?;
                }
                unit
            }
            "needs_drop" => {
                let t = ptr_elem(0)?;
                let nd = self.cg.needs_drop(&t);
                Ok((vec![self.b.ins().iconst(types::I8, nd as i64)], Ty::Bool))
            }
            "forget" => unit,
            "str_len" => Ok((vec![a[0][1]], usize_ty)),
            "str_from_raw" => Ok((vec![a[0][0], a[1][0]], Ty::Str)),
            "ptr_offset" => {
                let t = ptr_elem(0)?;
                let addr = self.elem_addr(a[0][0], a[1][0], &t);
                Ok((vec![addr], tys[0].clone()))
            }
            "str_parse_f64" => {
                let x = self.rt("om_rt_parse_f64", &[types::I64, types::I64], Some(types::F64), &a[0])?.unwrap();
                Ok((vec![x], Ty::F64))
            }
            "str_parse_ok" => {
                let x = self.rt("om_rt_parse_ok", &[], Some(types::I8), &[])?.unwrap();
                Ok((vec![x], Ty::Bool))
            }
            "str_ptr" => Ok((vec![a[0][0]], Ty::Ptr(Box::new(Ty::Int(IntTy::U8))))),
            _ => err(format!("unknown intrinsic `{op}`"), span),
        }
    }

    fn method_call(
        &mut self,
        recv: &Expr,
        m: &str,
        args: &[Arg],
        span: Span,
        expected: Option<&Ty>,
    ) -> CResult<(Vals, Ty)> {
        // Find the receiver's type without evaluating it twice: places are read
        // in the call itself, other receivers go into a temporary first.
        let (recv_expr, ty) = match self.place(recv)? {
            Some(p) => (recv.clone(), p.ty),
            None => {
                let (vals, ty) = self.expr(recv)?;
                let tmp = self.fresh_tmp();
                self.bind_vals(&tmp, ty.clone(), vals, true, recv.span)?;
                (Expr { kind: ExprKind::Var(tmp), span: recv.span }, ty)
            }
        };
        let has = |m: &str| ty.key().is_some_and(|k| self.cg.defs.contains_key(&format!("{k}::{m}")));
        if m == "clone" && args.is_empty() && !has(m) {
            // Every type can be cloned: copy types bit for bit, owning types deeply.
            let (v, t) = self.borrow_expect(&recv_expr, None)?;
            let v = self.clone_value(&v, &t)?;
            return Ok((v, t));
        }
        if m == "to_string" && args.is_empty() && !has(m) {
            // Any value becomes the text `print` would show for it.
            let (v, t) = self.borrow_expect(&recv_expr, None)?;
            return self.capture(|this| this.write_value(&v, &t, false));
        }
        let Some(type_name) = ty.key() else {
            return err(format!("{ty} has no methods"), span);
        };
        let targs = ty.args().to_vec();
        let key = format!("{type_name}::{m}");
        if self.box_elem(&ty).is_some() && !self.cg.defs.contains_key(&key) {
            // Methods of the boxed value work on the box (Rust's auto-deref).
            let inner = Expr { kind: ExprKind::Unary(UnOp::Deref, Box::new(recv_expr)), span: recv.span };
            return self.method_call(&inner, m, args, span, expected);
        }
        if is_string(&ty) && !self.cg.defs.contains_key(&key) && self.cg.defs.contains_key(&format!("str::{m}")) {
            // A String has every method of str (Rust's deref coercion).
            let view =
                Expr { kind: ExprKind::MethodCall(Box::new(recv_expr), "as_str".into(), vec![]), span: recv.span };
            return self.method_call(&view, m, args, span, expected);
        }
        let Some(def) = self.cg.defs.get(&key) else {
            let mut traits: Vec<&String> =
                self.cg.traits.values().filter(|t| t.methods.iter().any(|f| f.name == m)).map(|t| &t.name).collect();
            traits.sort();
            let hint = match traits.first() {
                Some(t) => format!("; trait `{t}` has one, but {ty} does not implement it"),
                None => String::new(),
            };
            return err(format!("{ty} has no method named `{m}`{hint}"), span);
        };
        let self_mode = match def.f.params.first() {
            Some(p) if p.name == "self" => p.mode,
            _ => return err(format!("`{key}` has no `self` parameter; call it as `{key}(...)`"), span),
        };
        let preset = Codegen::type_subst(&def.impl_generics, &targs);
        let self_arg = match self_mode {
            ParamMode::Value => recv_expr,
            ParamMode::Ref => Expr { kind: ExprKind::Ref(false, Box::new(recv_expr)), span: recv.span },
            ParamMode::RefMut => Expr { kind: ExprKind::Ref(true, Box::new(recv_expr)), span: recv.span },
        };
        let mut all = vec![Arg { name: None, value: self_arg, span: recv.span }];
        all.extend(args.iter().cloned());
        self.call_fn(&key, &all, span, expected, preset)
    }

    /// Call definition `key`. Type parameters not in `preset` are inferred from the
    /// arguments and the expected result type, then the matching instance is used.
    fn call_fn(
        &mut self,
        key: &str,
        args: &[Arg],
        span: Span,
        expected: Option<&Ty>,
        mut subst: Subst,
    ) -> CResult<(Vals, Ty)> {
        let def = self.cg.defs[key].clone();
        let vars = def.all_generics();
        if let Some(t) = &def.impl_ty {
            if let Ok(st) = self.cg.resolve(t, &subst) {
                subst.insert("Self".into(), st);
            }
        }
        let ps = &def.f.params;
        // Bind type variables from the type the caller expects back.
        if let (Some(rt), Some(exp)) = (&def.f.ret, expected) {
            // `-> Self` in a generic impl means the impl's own type, such as `Stack[T]`.
            let rt = match &def.impl_ty {
                Some(it) if rt.name == "Self" && rt.args.is_empty() => it,
                _ => rt,
            };
            let mut trial = subst.clone();
            if self.cg.unify(rt, exp, &vars, &mut trial) {
                subst = trial;
            }
        }

        // Match arguments to parameter slots: positional first, then by name.
        let mut filled = vec![false; ps.len()];
        let mut arg_slot = Vec::with_capacity(args.len());
        let mut seen_named = false;
        for (i, a) in args.iter().enumerate() {
            let slot = match &a.name {
                None if seen_named => return err("positional arguments must come before named ones", a.span),
                None if i >= ps.len() => {
                    return err(format!("`{key}` takes {} argument(s) but {} were given", ps.len(), args.len()), span)
                }
                None => i,
                Some(n) => {
                    seen_named = true;
                    match ps.iter().position(|p| &p.name == n) {
                        Some(k) => k,
                        None => return err(format!("`{key}` has no parameter named `{n}`"), a.span),
                    }
                }
            };
            if filled[slot] {
                return err(format!("argument `{}` is given twice", ps[slot].name), a.span);
            }
            filled[slot] = true;
            arg_slot.push(slot);
        }

        // Evaluate the written arguments in source order, then fill in defaults.
        let mut vals: Vec<Option<Vals>> = vec![None; ps.len()];
        let mut writebacks: Vec<(usize, Place)> = Vec::new();
        let mut view_roots: Vec<usize> = Vec::new();
        let temps_mark = self.temps.len();
        for (a, &k) in args.iter().zip(&arg_slot) {
            let p = &ps[k];
            let te = p.ty.clone().unwrap_or_else(|| def.impl_ty.clone().unwrap());
            let what = format!("argument `{}`", p.name);
            let v = match (&a.value.kind, p.mode) {
                (ExprKind::Ref(true, inner), ParamMode::RefMut) => {
                    let Some(place) = self.place(inner)? else {
                        return err("`&mut` needs a variable or field to change", inner.span);
                    };
                    self.check_usable(place.root_id, inner.span)?;
                    self.check_not_frozen(place.root_id, inner.span)?;
                    self.check_not_borrowed(place.root_id, inner.span)?;
                    if !place.mutable {
                        let r = &place.root;
                        let hint = if r == "self" {
                            "take `&mut self`".to_string()
                        } else {
                            format!("declare it with `let mut {r}`")
                        };
                        return err(format!("cannot pass `{r}` as `&mut`; {hint}"), inner.span);
                    }
                    if writebacks.iter().any(|(_, w)| {
                        overlaps(&w.loc, &place.loc) || w.root_id == place.root_id && matches!(place.loc, Loc::Mem(..))
                    }) {
                        return err(format!("cannot pass `{}` as `&mut` twice in one call", place.root), inner.span);
                    }
                    if !self.cg.unify(&te, &place.ty, &vars, &mut subst) {
                        return err(format!("{what} should be {}, found {}", te_string(&te), place.ty), inner.span);
                    }
                    let v = self.read_loc(&place.loc);
                    writebacks.push((k, place));
                    v
                }
                (ExprKind::Ref(false, inner), ParamMode::Ref) => {
                    let hint = self.cg.resolve(&te, &self.merged(&subst)).ok();
                    let (v, t) = self.borrow_expect(inner, hint.as_ref())?;
                    if !self.cg.unify(&te, &t, &vars, &mut subst) {
                        let want = hint.map(|h| h.to_string()).unwrap_or_else(|| te_string(&te));
                        return err(format!("{what} should be {want}, found {t}"), inner.span);
                    }
                    v
                }
                (_, ParamMode::RefMut) => {
                    return err(format!("{what} is `&mut {}`; pass it as `&mut ...`", te_string(&te)), a.value.span)
                }
                (ExprKind::Var(n), ParamMode::Ref) | (ExprKind::Var(n), ParamMode::Value)
                    if p.mode == ParamMode::Ref && self.lookup(n).is_some_and(|l| l.own == Own::Borrowed) =>
                {
                    // Already a borrow (a `&` parameter or a `for x in &list` element): pass it on.
                    let hint = self.cg.resolve(&te, &self.merged(&subst)).ok();
                    let (v, t) = self.borrow_expect(&a.value, hint.as_ref())?;
                    if !self.cg.unify(&te, &t, &vars, &mut subst) {
                        let want = hint.map(|h| h.to_string()).unwrap_or_else(|| te_string(&te));
                        return err(format!("{what} should be {want}, found {t}"), a.value.span);
                    }
                    v
                }
                (_, ParamMode::Ref) => {
                    return err(format!("{what} is `&{}`; pass it as `&...`", te_string(&te)), a.value.span)
                }
                (ExprKind::Ref(false, inner), ParamMode::Value) if te.name == "str" && self.expr_is_string(inner) => {
                    // `&s` where `s: String` gives a `str` view (Rust's deref coercion).
                    let view =
                        Expr { kind: ExprKind::MethodCall(inner.clone(), "as_str".into(), vec![]), span: inner.span };
                    self.view_roots(&view, &mut view_roots);
                    self.expr(&view)?.0
                }
                (ExprKind::Ref(..), ParamMode::Value) => {
                    return err(format!("{what} takes a value, not a reference"), a.value.span)
                }
                _ => {
                    let (v, t) = self.infer_arg(&a.value, &te, &vars, &mut subst, &what)?;
                    if self.cg.has_view(&t) {
                        self.view_roots(&a.value, &mut view_roots);
                    }
                    v
                }
            };
            vals[k] = Some(v);
        }
        // A `&mut` argument that can hold `str` views could keep one of these after the call.
        for (_, w) in &writebacks {
            if view_roots.contains(&w.root_id) {
                return err(format!("cannot pass `{}` as `&mut` while also passing a view of its text", w.root), span);
            }
            let temp_text = self.temps[temps_mark..].iter().any(|(_, t)| self.cg.owns_text(t));
            if self.cg.has_view(&w.ty) && (!view_roots.is_empty() || temp_text) {
                return err(
                    format!("`{}` could keep a `str` view of a String past its lifetime (Om cannot check this yet); pass an owned String or `.to_string()`", w.root),
                    span,
                );
            }
        }
        for (k, p) in ps.iter().enumerate() {
            if vals[k].is_none() {
                let Some(d) = p.default.clone() else {
                    return err(format!("missing argument `{}` in call to `{key}`", p.name), span);
                };
                let te = p.ty.clone().unwrap();
                vals[k] = Some(self.infer_arg(&d, &te, &vars, &mut subst, "default value")?.0);
            }
        }
        let targs = self.require_all(&vars, &subst, &format!("`{key}`"), span)?;
        if self.dead {
            return Ok((vec![], Ty::Never));
        }
        let inst_subst: Subst = vars.iter().cloned().zip(targs).collect();
        let inst = self.cg.instantiate(key, inst_subst, span)?;
        let info = &self.cg.fns[&inst];
        let (id, ret) = (info.id, info.ret.clone());
        let flat: Vals = vals.into_iter().flatten().flatten().collect();
        let fref = self.cg.module.declare_func_in_func(id, self.b.func);
        let call = self.b.ins().call(fref, &flat);
        let results = self.b.inst_results(call).to_vec();
        let n = self.cg.width(&ret);
        // Write `&mut` arguments back to the caller's variables, in parameter order.
        writebacks.sort_by_key(|(k, _)| *k);
        let mut pos = n;
        for (_, place) in writebacks {
            let n = self.cg.width(&place.ty);
            self.write_loc(&place.loc, &results[pos..pos + n]);
            pos += n;
        }
        Ok((results[..n].to_vec(), ret))
    }

    // ---------- patterns and match ----------

    /// Evaluate a `match` (or `let`) subject. `match &x` borrows `x`: bindings are views
    /// and `x` is frozen while they live. `match x` moves `x` only if some pattern binds
    /// a part of it that owns memory; otherwise it just looks at `x`. Returns the value,
    /// its type, how bindings hold their parts, and the local to freeze.
    fn subject(
        &mut self,
        scrut: &Expr,
        pats: &[&Pattern],
        expected: Option<&Ty>,
    ) -> CResult<(Vals, Ty, Own, Option<usize>)> {
        let (inner, by_ref) = match &scrut.kind {
            ExprKind::Ref(false, i) => (&**i, true),
            _ => (scrut, false),
        };
        let is_place = matches!(
            inner.kind,
            ExprKind::Var(_) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Unary(UnOp::Deref, _)
        );
        let place_ty = if is_place { self.place_type(inner) } else { None };
        if let Some(ty) = place_ty {
            let owns = pats.iter().any(|p| self.pat_owns(p, &ty));
            // Matching on something borrowed binds by reference (Rust's match ergonomics).
            let mut root = inner;
            while let ExprKind::Field(i, _) | ExprKind::Index(i, _) | ExprKind::Unary(UnOp::Deref, i) = &root.kind {
                root = i;
            }
            let borrowed_root = match &root.kind {
                ExprKind::Var(n) => self.lookup(n).is_some_and(|l| l.own != Own::Owned),
                _ => false,
            };
            let by_ref = by_ref || borrowed_root;
            let box_move = matches!(&inner.kind, ExprKind::Unary(UnOp::Deref, b) if matches!(b.kind, ExprKind::Var(_)));
            if owns && !by_ref {
                if !matches!(inner.kind, ExprKind::Var(_)) && !box_move {
                    return err(
                        "cannot move parts out of this place; match on `&...` to borrow them, or `.clone()` it",
                        inner.span,
                    );
                }
                let (v, t) = self.expr_expect(inner, expected)?;
                return Ok((v, t, Own::Owned, None));
            }
            let p = self.place(inner)?.unwrap();
            self.check_usable(p.root_id, inner.span)?;
            let v = self.read_loc(&p.loc);
            let freeze = (owns && by_ref).then_some(p.root_id);
            return Ok((v, p.ty, Own::Borrowed, freeze));
        }
        let (v, t) = self.expr_expect(inner, expected)?;
        if by_ref {
            if !self.dead && self.cg.needs_drop(&t) {
                self.temps.push((v.clone(), t.clone()));
            }
            return Ok((v, t, Own::Borrowed, None));
        }
        Ok((v, t, Own::Owned, None))
    }

    /// Does `pat` bind (and so take) some part of a `ty` value that owns memory?
    fn pat_owns(&self, pat: &Pattern, ty: &Ty) -> bool {
        match &pat.kind {
            PatKind::Bind(..) => self.cg.needs_drop(ty),
            PatKind::Tuple(subs) => match ty {
                Ty::Tuple(ts) => subs.iter().zip(ts).any(|(s, t)| self.pat_owns(s, t)),
                _ => false,
            },
            PatKind::Variant(path, subs) => match self.pattern_variant(path, ty, pat.span) {
                Ok(idx) => {
                    let ftys = self.cg.variants(ty)[idx].1.clone();
                    subs.iter().zip(&ftys).any(|(s, t)| self.pat_owns(s, t))
                }
                Err(_) => false,
            },
            PatKind::Or(alts) => alts.iter().any(|a| self.pat_owns(a, ty)),
            _ => false,
        }
    }

    /// After `pat` matched an owned value and bound its parts, drop the parts it did not bind.
    fn drop_residual(&mut self, pat: &Pattern, ty: &Ty, vals: &[Value]) -> CResult<()> {
        match &pat.kind {
            PatKind::Bind(..) => Ok(()),
            PatKind::Tuple(subs) => {
                let Ty::Tuple(ts) = ty else { return Ok(()) };
                let mut off = 0;
                for (s, t) in subs.iter().zip(ts) {
                    let w = self.cg.width(t);
                    self.drop_residual(s, t, &vals[off..off + w])?;
                    off += w;
                }
                Ok(())
            }
            PatKind::Variant(path, subs) if !subs.is_empty() => {
                let idx = self.pattern_variant(path, ty, pat.span)?;
                let ftys = self.cg.variants(ty)[idx].1.clone();
                let mut off = self.cg.variant_offset(ty, idx);
                for (s, t) in subs.iter().zip(&ftys) {
                    let w = self.cg.width(t);
                    self.drop_residual(s, t, &vals[off..off + w])?;
                    off += w;
                }
                Ok(())
            }
            _ => self.drop_value(vals, ty),
        }
    }

    fn match_expr(&mut self, scrut: &Expr, arms: &[Arm], span: Span, expected: Option<&Ty>) -> CResult<(Vals, Ty)> {
        let st = string_ty();
        let expected = if expected.is_none() && arms.iter().any(|a| matches!(a.body.kind, ExprKind::Interp(_))) {
            Some(&st)
        } else {
            expected
        };
        let pats: Vec<&Pattern> = arms.iter().map(|a| &a.pat).collect();
        let (sv, sty, own, freeze) = self.subject(scrut, &pats, None)?;
        if sty == Ty::Never {
            return Ok((vec![], Ty::Never));
        }
        // `match tile { WALL => .., FLOOR => .. }`: a constant's name in a pattern
        // on a number, char or bool means its value.
        let rewritten: Vec<Arm>;
        let arms = if !matches!(sty, Ty::Enum(..)) {
            rewritten = arms.iter().map(|a| Arm { pat: self.const_pattern(&a.pat), ..a.clone() }).collect();
            &rewritten[..]
        } else {
            arms
        };
        for arm in arms {
            self.check_pattern(&arm.pat, &sty)?;
        }
        let rows = arms.iter().filter(|a| a.guard.is_none()).map(|a| vec![a.pat.clone()]).collect();
        if let Some(w) = self.cg_missing(rows, &sty) {
            return err(
                format!("`match` does not cover every case; missing: {w} (add an arm for it or a `_` arm)"),
                span,
            );
        }
        let was_dead = self.dead;
        let mut m = self.new_merge();
        let before = self.moved.clone();
        let mut arm_moved: Vec<HashMap<usize, Span>> = Vec::new();
        for arm in arms {
            self.moved = before.clone();
            let test = self.pat_test(&arm.pat, &sty, &sv)?;
            let bind_blk = self.b.create_block();
            let next = self.b.create_block();
            self.b.ins().brif(test, bind_blk, &[], next, &[]);
            self.b.switch_to_block(bind_blk);
            self.b.seal_block(bind_blk);
            self.dead = false;
            self.scopes.push(Scope::default());
            self.defers.push(Vec::new());
            let r = (|| {
                self.pat_bind(&arm.pat, &sty, &sv, own)?;
                if let Some(root) = freeze {
                    let level = self.scopes.len() - 1;
                    self.scopes[level].freezes.push((root, "a `match &` binding".into()));
                }
                if let Some(g) = &arm.guard {
                    let gv = self.cond(g)?;
                    let body_blk = self.b.create_block();
                    self.b.ins().brif(gv, body_blk, &[], next, &[]);
                    self.b.switch_to_block(body_blk);
                    self.b.seal_block(body_blk);
                }
                if own == Own::Owned {
                    self.drop_residual(&arm.pat, &sty, &sv)?;
                }
                let hint = m.ty.clone().or(expected.cloned());
                let (v, t) = self.expr_expect(&arm.body, hint.as_ref())?;
                if !self.dead {
                    let level = self.scopes.len() - 1;
                    self.cleanup(level)?;
                    self.jump_to(&mut m, t, v, "match arms", arm.body.span)?;
                    arm_moved.push(self.moved.clone());
                }
                Ok(())
            })();
            self.defers.pop();
            self.scopes.pop();
            r?;
            self.b.switch_to_block(next);
            self.b.seal_block(next);
        }
        // Exhaustiveness was checked, so falling past the last arm cannot happen.
        self.b.ins().trap(TrapCode::unwrap_user(1));
        self.moved = before;
        for am in arm_moved {
            self.moved.extend(am);
        }
        Ok(self.finish_merge(m, was_dead))
    }

    /// Replace constant names in `pat` with their literal values.
    fn const_pattern(&self, pat: &Pattern) -> Pattern {
        let kind = match &pat.kind {
            PatKind::Variant(name, subs) if subs.is_empty() => match self.cg.consts.get(name) {
                Some((_, value)) => match (&value.kind, literal_int(value)) {
                    (_, Some((n, _))) => PatKind::Int(n),
                    (ExprKind::Unary(UnOp::Neg, inner), _) if literal_int(inner).is_some() => {
                        PatKind::Int(-literal_int(inner).unwrap().0)
                    }
                    (ExprKind::Char(c), _) => PatKind::Char(*c),
                    (ExprKind::Bool(b), _) => PatKind::Bool(*b),
                    _ => pat.kind.clone(),
                },
                None => pat.kind.clone(),
            },
            PatKind::Or(alts) => PatKind::Or(alts.iter().map(|a| self.const_pattern(a)).collect()),
            PatKind::Tuple(subs) => PatKind::Tuple(subs.iter().map(|a| self.const_pattern(a)).collect()),
            k => k.clone(),
        };
        Pattern { kind, span: pat.span }
    }

    /// Check that `pat` can match a value of type `ty`.
    fn check_pattern(&self, pat: &Pattern, ty: &Ty) -> CResult<()> {
        let mismatch = |what: &str| err(format!("this pattern matches {what}, but the value is {ty}"), pat.span);
        match &pat.kind {
            PatKind::Wild | PatKind::Bind(..) => Ok(()),
            PatKind::Int(_) | PatKind::Range(..) => {
                let Ty::Int(t) = ty else { return mismatch("an integer") };
                let (lo, hi) = pat_range(pat).unwrap();
                for n in [lo, hi] {
                    if n < t.min() || n > t.max() {
                        return err(format!("pattern value `{n}` does not fit in {ty}"), pat.span);
                    }
                }
                Ok(())
            }
            PatKind::Char(_) | PatKind::CharRange(..) if *ty == Ty::Char => Ok(()),
            PatKind::Char(_) | PatKind::CharRange(..) => mismatch("a char"),
            PatKind::Bool(_) if *ty == Ty::Bool => Ok(()),
            PatKind::Bool(_) => mismatch("a bool"),
            PatKind::Tuple(subs) => match ty {
                Ty::Tuple(ts) if ts.len() == subs.len() => {
                    subs.iter().zip(ts).try_for_each(|(s, t)| self.check_pattern(s, t))
                }
                Ty::Unit if subs.is_empty() => Ok(()),
                _ => mismatch(&format!("a tuple of {}", subs.len())),
            },
            PatKind::Or(alts) => {
                for a in alts {
                    if has_bindings(a) {
                        return err("`|` patterns cannot bind names yet", a.span);
                    }
                    self.check_pattern(a, ty)?;
                }
                Ok(())
            }
            PatKind::Variant(path, subs) => {
                if !matches!(ty, Ty::Enum(..)) {
                    return mismatch(&format!("`{path}`"));
                }
                let idx = self.pattern_variant(path, ty, pat.span)?;
                let ftys = &self.cg.variants(ty)[idx].1;
                if subs.len() != ftys.len() {
                    return err(
                        format!("`{path}` has {} field(s), but the pattern has {}", ftys.len(), subs.len()),
                        pat.span,
                    );
                }
                subs.iter().zip(ftys).try_for_each(|(s, t)| self.check_pattern(s, t))
            }
        }
    }

    /// Resolve a variant pattern within enum type `ty`. `Red`, `Color::Red` and `Self::Red` all work.
    fn pattern_variant(&self, path: &str, ty: &Ty, span: Span) -> CResult<usize> {
        let e = ty.base().unwrap_or_default();
        let path = self.self_name(path);
        let (prefix, v) = match path.rsplit_once("::") {
            Some((p, v)) => (Some(p), v),
            None => (None, path.as_str()),
        };
        if let Some(p) = prefix {
            if p != e {
                return err(format!("expected a variant of `{ty}`, found `{path}`"), span);
            }
        }
        match self.cg.variants(ty).iter().position(|(n, _)| n == v) {
            Some(i) => Ok(i),
            None => err(format!("`{ty}` has no variant `{v}`"), span),
        }
    }

    fn cg_missing(&self, rows: Vec<Vec<Pattern>>, ty: &Ty) -> Option<String> {
        self.missing(rows, std::slice::from_ref(ty)).map(|w| w[0].clone())
    }

    /// Pattern-matrix exhaustiveness check (Maranget's algorithm, simplified).
    /// Returns `None` if `rows` cover every value of `tys`, or one example of an
    /// uncovered value per column.
    fn missing(&self, rows: Vec<Vec<Pattern>>, tys: &[Ty]) -> Option<Vec<String>> {
        let Some((ty, rest_tys)) = tys.split_first() else {
            return if rows.is_empty() { Some(vec![]) } else { None };
        };
        let mut expanded = Vec::new();
        for row in rows {
            match &row[0].kind {
                PatKind::Or(alts) => {
                    for a in alts {
                        let mut r = row.clone();
                        r[0] = a.clone();
                        expanded.push(r);
                    }
                }
                _ => expanded.push(row),
            }
        }
        let wild = Pattern { kind: PatKind::Wild, span: Span::default() };
        let ctors: Vec<(String, Vec<Ty>)> = match ty {
            Ty::Bool => vec![("true".into(), vec![]), ("false".into(), vec![])],
            Ty::Enum(..) => {
                let base = ty.base().unwrap();
                self.cg.variants(ty).into_iter().map(|(n, f)| (format!("{base}::{n}"), f)).collect()
            }
            Ty::Tuple(ts) => vec![(String::new(), ts.clone())],
            Ty::Unit => vec![(String::new(), vec![])],
            Ty::Int(_) | Ty::Char => {
                // Split the type's range at every pattern boundary, then check each piece.
                let (min, max) = match ty {
                    Ty::Int(t) => (t.min(), t.max()),
                    _ => (0, char::MAX as i128),
                };
                let mut cuts = std::collections::BTreeSet::from([min, max + 1]);
                for row in &expanded {
                    if let Some((lo, hi)) = pat_range(&row[0]) {
                        if lo <= hi {
                            cuts.insert(lo.clamp(min, max + 1));
                            cuts.insert((hi + 1).clamp(min, max + 1));
                        }
                    }
                }
                let cuts: Vec<i128> = cuts.into_iter().collect();
                for w in cuts.windows(2) {
                    let (a, b) = (w[0], w[1] - 1);
                    let spec: Vec<Vec<Pattern>> = expanded
                        .iter()
                        .filter(|r| match pat_range(&r[0]) {
                            Some((lo, hi)) => lo <= a && b <= hi,
                            None => irrefutable(&r[0]),
                        })
                        .map(|r| r[1..].to_vec())
                        .collect();
                    if let Some(rest) = self.missing(spec, rest_tys) {
                        let show = |n: i128| match ty {
                            Ty::Int(t) if n == t.min() && n != 0 => format!("{}::MIN", t.name()),
                            Ty::Int(t) if n == t.max() => format!("{}::MAX", t.name()),
                            Ty::Char => format!("{:?}", char::from_u32(n as u32).unwrap_or('?')),
                            _ => n.to_string(),
                        };
                        let shown = if a == min && b == max {
                            "_".to_string()
                        } else if a == b {
                            show(a)
                        } else {
                            format!("{}..={}", show(a), show(b))
                        };
                        return Some([vec![shown], rest].concat());
                    }
                }
                return None;
            }
            _ => {
                // Infinitely many values: only catch-all rows help.
                let rows: Vec<_> = expanded.iter().filter(|r| irrefutable(&r[0])).map(|r| r[1..].to_vec()).collect();
                return self.missing(rows, rest_tys).map(|w| [vec!["_".to_string()], w].concat());
            }
        };
        for (label, ftys) in &ctors {
            let mut spec = Vec::new();
            for row in &expanded {
                let subs: Option<Vec<Pattern>> = match &row[0].kind {
                    PatKind::Wild | PatKind::Bind(..) => Some(vec![wild.clone(); ftys.len()]),
                    PatKind::Bool(b) => (b.to_string() == *label).then(Vec::new),
                    PatKind::Tuple(subs) => Some(subs.clone()),
                    PatKind::Variant(path, subs) => {
                        let idx = self.pattern_variant(path, ty, row[0].span).ok()?;
                        let base = ty.base().unwrap();
                        (format!("{base}::{}", self.cg.variants(ty)[idx].0) == *label).then(|| subs.clone())
                    }
                    _ => None,
                };
                if let Some(mut subs) = subs {
                    subs.extend_from_slice(&row[1..]);
                    spec.push(subs);
                }
            }
            let tys: Vec<Ty> = ftys.iter().chain(rest_tys).cloned().collect();
            if let Some(w) = self.missing(spec, &tys) {
                let (fields, rest) = w.split_at(ftys.len());
                let shown = if label.is_empty() {
                    format!("({})", fields.join(", "))
                } else if fields.is_empty() {
                    label.clone()
                } else {
                    format!("{label}({})", fields.join(", "))
                };
                return Some([vec![shown], rest.to_vec()].concat());
            }
        }
        None
    }

    fn pat_test(&mut self, pat: &Pattern, ty: &Ty, vals: &[Value]) -> CResult<Value> {
        Ok(match &pat.kind {
            PatKind::Wild | PatKind::Bind(..) => self.b.ins().iconst(types::I8, 1),
            PatKind::Bool(v) => self.b.ins().icmp_imm_s(IntCC::Equal, vals[0], *v as i64),
            PatKind::Int(_) | PatKind::Range(..) | PatKind::Char(_) | PatKind::CharRange(..) => {
                let (lo, hi) = pat_range(pat).unwrap();
                let t = match ty {
                    Ty::Int(t) => *t,
                    _ => IntTy::U32,
                };
                let lo_v = self.int_lit(lo, t, pat.span)?.0[0];
                if lo == hi {
                    return Ok(self.b.ins().icmp(IntCC::Equal, vals[0], lo_v));
                }
                let hi_v = self.int_lit(hi, t, pat.span)?.0[0];
                let (ge, le) = if t.signed() {
                    (IntCC::SignedGreaterThanOrEqual, IntCC::SignedLessThanOrEqual)
                } else {
                    (IntCC::UnsignedGreaterThanOrEqual, IntCC::UnsignedLessThanOrEqual)
                };
                let a = self.b.ins().icmp(ge, vals[0], lo_v);
                let c = self.b.ins().icmp(le, vals[0], hi_v);
                self.b.ins().band(a, c)
            }
            PatKind::Or(alts) => {
                let mut acc = self.b.ins().iconst(types::I8, 0);
                for a in alts {
                    let t = self.pat_test(a, ty, vals)?;
                    acc = self.b.ins().bor(acc, t);
                }
                acc
            }
            PatKind::Tuple(subs) => {
                let ts = match ty {
                    Ty::Tuple(ts) => ts.clone(),
                    _ => vec![],
                };
                let mut acc = self.b.ins().iconst(types::I8, 1);
                let mut off = 0;
                for (s, t) in subs.iter().zip(&ts) {
                    let w = self.cg.width(t);
                    if !irrefutable(s) {
                        let sub = self.pat_test(s, t, &vals[off..off + w])?;
                        acc = self.b.ins().band(acc, sub);
                    }
                    off += w;
                }
                acc
            }
            PatKind::Variant(path, subs) => {
                let idx = self.pattern_variant(path, ty, pat.span)?;
                let mut acc = self.b.ins().icmp_imm_s(IntCC::Equal, vals[0], idx as i64);
                let ftys = self.cg.variants(ty)[idx].1.clone();
                let mut off = self.cg.variant_offset(ty, idx);
                for (s, t) in subs.iter().zip(&ftys) {
                    let w = self.cg.width(t);
                    if !irrefutable(s) {
                        let sub = self.pat_test(s, t, &vals[off..off + w])?;
                        acc = self.b.ins().band(acc, sub);
                    }
                    off += w;
                }
                acc
            }
        })
    }

    fn pat_bind(&mut self, pat: &Pattern, ty: &Ty, vals: &[Value], own: Own) -> CResult<()> {
        match &pat.kind {
            PatKind::Bind(name, mutable) => {
                self.bind_local(name, ty.clone(), vals.to_vec(), *mutable, own, pat.span).map(|_| ())
            }
            PatKind::Tuple(subs) => {
                let Ty::Tuple(ts) = ty else { return Ok(()) };
                let mut off = 0;
                for (s, t) in subs.iter().zip(ts) {
                    let w = self.cg.width(t);
                    self.pat_bind(s, t, &vals[off..off + w], own)?;
                    off += w;
                }
                Ok(())
            }
            PatKind::Variant(path, subs) => {
                let idx = self.pattern_variant(path, ty, pat.span)?;
                let ftys = self.cg.variants(ty)[idx].1.clone();
                let mut off = self.cg.variant_offset(ty, idx);
                for (s, t) in subs.iter().zip(&ftys) {
                    let w = self.cg.width(t);
                    self.pat_bind(s, t, &vals[off..off + w], own)?;
                    off += w;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    // ---------- printing, panic and assert ----------

    /// Write an argument that may be an interpolated string.
    fn write_arg(&mut self, arg: &Arg) -> CResult<()> {
        if arg.name.is_some() {
            return err("this built-in does not take named arguments", arg.span);
        }
        match &arg.value.kind {
            ExprKind::Interp(parts) => {
                for part in parts {
                    match part {
                        InterpPart::Lit(text) => self.write_str(text)?,
                        InterpPart::Expr(e) => {
                            let (v, t) = self.borrow_expect(e, None)?;
                            self.write_value(&v, &t, false)?;
                        }
                    }
                }
                Ok(())
            }
            _ => {
                let (v, t) = self.borrow_expect(&arg.value, None)?;
                self.write_value(&v, &t, false)
            }
        }
    }

    /// `print(x)` writes any value, or an interpolated string, followed by a newline.
    /// `print(x)` and `eprint(x)` (to stderr), with an optional `end:` that
    /// replaces the newline: `print("Loading", end: "")`.
    fn print(&mut self, args: &[Arg], span: Span, stderr: bool) -> CResult<(Vals, Ty)> {
        let name = if stderr { "eprint" } else { "print" };
        let (value, end) = match args {
            [a] if a.name.is_none() => (a, None),
            [a, e] if a.name.is_none() && e.name.as_deref() == Some("end") => (a, Some(e)),
            _ => {
                return err(
                    format!("`{name}` takes one value and an optional `end:`, like `{name}(x, end: \"\")`"),
                    span,
                )
            }
        };
        if stderr {
            // Keep stdout and stderr in the order they were written.
            self.rt("om_rt_flush", &[], None, &[])?;
        }
        let saved = self.stderr;
        self.stderr = stderr;
        let r = (|| {
            self.write_arg(value)?;
            match end {
                Some(e) => {
                    let plain = Arg { name: None, value: e.value.clone(), span: e.span };
                    self.write_arg(&plain)
                }
                None => self.write_str("\n"),
            }
        })();
        self.stderr = saved;
        r?;
        Ok((vec![], Ty::Unit))
    }

    /// `panic(message)` prints the message to stderr and exits with status 101, like Rust.
    fn panic(&mut self, args: &[Arg], span: Span) -> CResult<(Vals, Ty)> {
        if args.len() > 1 {
            return err("`panic` takes at most one argument", span);
        }
        // Flush what the program printed so far, so the panic message comes after it.
        self.rt("om_rt_flush", &[], None, &[])?;
        self.stderr = true;
        let r = (|| {
            self.write_str("panic: ")?;
            if let Some(a) = args.first() {
                self.write_arg(a)?;
            } else {
                self.write_str("explicit panic")?;
            }
            self.write_str("\n")
        })();
        self.stderr = false;
        r?;
        let id = self.cg.runtime_fn("om_rt_exit_panic", None)?;
        let f = self.cg.module.declare_func_in_func(id, self.b.func);
        self.b.ins().call(f, &[]);
        self.b.ins().trap(TrapCode::unwrap_user(2));
        self.start_dead_block();
        Ok((vec![], Ty::Never))
    }

    /// `assert(cond)` or `assert(cond, message)`.
    fn assert(&mut self, args: &[Arg], span: Span) -> CResult<(Vals, Ty)> {
        let (cond, msg) = match args {
            [c] => (c, None),
            [c, m] => (c, Some(m)),
            _ => return err("`assert` takes a condition and an optional message", span),
        };
        let c = self.cond(&cond.value)?;
        let ok = self.b.create_block();
        let fail = self.b.create_block();
        self.b.ins().brif(c, ok, &[], fail, &[]);
        self.b.switch_to_block(fail);
        self.b.seal_block(fail);
        let was_dead = self.dead;
        let default = Arg { name: None, span, value: Expr { kind: ExprKind::Str("assertion failed".into()), span } };
        self.panic(&[msg.cloned().unwrap_or(default)], span)?;
        self.b.switch_to_block(ok);
        self.b.seal_block(ok);
        self.dead = was_dead;
        Ok((vec![], Ty::Unit))
    }

    /// Run `f` with output going to a buffer, and return the text as a new String.
    fn capture(&mut self, f: impl FnOnce(&mut Self) -> CResult<()>) -> CResult<(Vals, Ty)> {
        self.rt("om_rt_capture_begin", &[], None, &[])?;
        f(self)?;
        let p = self.rt("om_rt_capture_end", &[], Some(types::I64), &[])?.unwrap();
        let n = self.rt("om_rt_capture_len", &[], Some(types::I64), &[])?.unwrap();
        Ok((vec![p, n, n], string_ty()))
    }

    fn write_str(&mut self, s: &str) -> CResult<()> {
        let v = self.str_val(s)?;
        self.write_bytes(v[0], v[1])
    }

    /// Write `len` bytes starting at `ptr` (to stderr while panicking).
    fn write_bytes(&mut self, ptr: Value, len: Value) -> CResult<()> {
        let name = if self.stderr { "om_rt_ewrite_bytes" } else { "om_rt_write_bytes" };
        self.rt(name, &[types::I64, types::I64], None, &[ptr, len])?;
        Ok(())
    }

    /// Write one value with no newline. Structs, tuples and enums print in the
    /// same shape as Rust's `{:?}`, with no `derive` needed.
    fn write_value(&mut self, vals: &[Value], ty: &Ty, nested: bool) -> CResult<()> {
        let special = self.vec_elem(ty).is_some() || self.box_elem(ty).is_some() || is_string(ty);
        if matches!(ty, Ty::Struct(..) | Ty::Enum(..)) && !special && !self.dead {
            // One printing function per type, so recursive types (through Box) work.
            let id = self.cg.glue_fn(ty, Glue::Write(nested, self.stderr))?;
            let f = self.cg.module.declare_func_in_func(id, self.b.func);
            self.b.ins().call(f, vals);
            return Ok(());
        }
        self.write_value_inline(vals, ty, nested)
    }

    fn write_value_inline(&mut self, vals: &[Value], ty: &Ty, nested: bool) -> CResult<()> {
        if let Some(elem) = self.vec_elem(ty) {
            return self.write_list(vals, ty, &elem);
        }
        if let Some(elem) = self.box_elem(ty) {
            let (slots, _, _) = self.cg.mem_layout(&elem);
            let inner = self.read_loc(&Loc::Mem(vals[0], slots));
            return self.write_value(&inner, &elem, nested);
        }
        if is_string(ty) {
            // String { vec: Vec { buf, len, cap } }: print the text, like a str.
            return self.write_value(&vals[..2], &Ty::Str, nested);
        }
        match ty {
            Ty::Int(t) => {
                let wide = self.int_resize(vals[0], *t, if t.signed() { IntTy::I64 } else { IntTy::U64 });
                let f = if t.signed() { "om_rt_write_i64" } else { "om_rt_write_u64" };
                self.rt_call(f, types::I64, wide)
            }
            // Inside a struct, list or enum, floats print like Rust's `{:?}` (`4.0`).
            Ty::F32 => {
                self.rt_call(if nested { "om_rt_write_f32_dbg" } else { "om_rt_write_f32" }, types::F32, vals[0])
            }
            Ty::F64 => {
                self.rt_call(if nested { "om_rt_write_f64_dbg" } else { "om_rt_write_f64" }, types::F64, vals[0])
            }
            Ty::Char => {
                if nested {
                    self.write_str("'")?;
                }
                self.rt_call("om_rt_write_char", types::I32, vals[0])?;
                if nested {
                    self.write_str("'")?;
                }
                Ok(())
            }
            Ty::Bool => {
                let t = self.str_val("true")?;
                let f = self.str_val("false")?;
                let p = self.b.ins().select(vals[0], t[0], f[0]);
                let n = self.b.ins().select(vals[0], t[1], f[1]);
                self.write_bytes(p, n)
            }
            Ty::Str => {
                if nested {
                    self.write_str("\"")?;
                }
                self.write_bytes(vals[0], vals[1])?;
                if nested {
                    self.write_str("\"")?;
                }
                Ok(())
            }
            Ty::Unit => self.write_str("()"),
            Ty::Never => Ok(()),
            Ty::Ptr(_) => self.rt_call("om_rt_write_u64", types::I64, vals[0]),
            Ty::Tuple(ts) => {
                self.write_str("(")?;
                let mut off = 0;
                for (i, t) in ts.iter().enumerate() {
                    if i > 0 {
                        self.write_str(", ")?;
                    }
                    let w = self.cg.width(t);
                    self.write_value(&vals[off..off + w], t, true)?;
                    off += w;
                }
                self.write_str(if ts.len() == 1 { ",)" } else { ")" })
            }
            Ty::Struct(n, _) => {
                let fields = self.cg.fields(ty);
                self.write_str(&format!("{n} {{ "))?;
                let mut off = 0;
                for (i, (f, t)) in fields.iter().enumerate() {
                    let w = self.cg.width(t);
                    self.write_str(&format!("{}{f}: ", if i > 0 { ", " } else { "" }))?;
                    self.write_value(&vals[off..off + w], t, true)?;
                    off += w;
                }
                self.write_str(if fields.is_empty() { "}" } else { " }" })
            }
            Ty::Enum(..) => {
                // Branch on the tag and print the active variant.
                let variants = self.cg.variants(ty);
                let done = self.b.create_block();
                for (i, (vname, ftys)) in variants.iter().enumerate() {
                    let here = self.b.create_block();
                    let next = self.b.create_block();
                    let is = self.b.ins().icmp_imm_s(IntCC::Equal, vals[0], i as i64);
                    self.b.ins().brif(is, here, &[], next, &[]);
                    self.b.switch_to_block(here);
                    self.b.seal_block(here);
                    self.write_str(vname)?;
                    if !ftys.is_empty() {
                        self.write_str("(")?;
                        let mut off = self.cg.variant_offset(ty, i);
                        for (j, t) in ftys.iter().enumerate() {
                            if j > 0 {
                                self.write_str(", ")?;
                            }
                            let w = self.cg.width(t);
                            self.write_value(&vals[off..off + w], t, true)?;
                            off += w;
                        }
                        self.write_str(")")?;
                    }
                    self.b.ins().jump(done, &[]);
                    self.b.switch_to_block(next);
                    self.b.seal_block(next);
                }
                self.b.ins().jump(done, &[]);
                self.b.switch_to_block(done);
                self.b.seal_block(done);
                Ok(())
            }
        }
    }

    /// Print a `Vec` as `[a, b, c]`.
    fn write_list(&mut self, vals: &[Value], ty: &Ty, elem: &Ty) -> CResult<()> {
        let buf = vals[self.cg.field(ty, "buf").unwrap().1];
        let len = vals[self.cg.field(ty, "len").unwrap().1];
        self.write_str("[")?;
        let header = self.b.create_block();
        let body = self.b.create_block();
        let sep = self.b.create_block();
        let item = self.b.create_block();
        let exit = self.b.create_block();
        self.b.append_block_param(header, types::I64);
        let zero = self.b.ins().iconst(types::I64, 0);
        self.b.ins().jump(header, &[zero.into()]);
        self.b.switch_to_block(header);
        let i = self.b.block_params(header)[0];
        let more = self.b.ins().icmp(IntCC::UnsignedLessThan, i, len);
        self.b.ins().brif(more, body, &[], exit, &[]);
        self.b.switch_to_block(body);
        self.b.seal_block(body);
        let first = self.b.ins().icmp_imm_s(IntCC::Equal, i, 0);
        self.b.ins().brif(first, item, &[], sep, &[]);
        self.b.switch_to_block(sep);
        self.b.seal_block(sep);
        self.write_str(", ")?;
        self.b.ins().jump(item, &[]);
        self.b.switch_to_block(item);
        self.b.seal_block(item);
        let addr = self.elem_addr(buf, i, elem);
        let (slots, _, _) = self.cg.mem_layout(elem);
        let v = self.read_loc(&Loc::Mem(addr, slots));
        self.write_value(&v, elem, true)?;
        let next = self.b.ins().iadd_imm_s(i, 1);
        self.b.ins().jump(header, &[next.into()]);
        self.b.seal_block(header);
        self.b.switch_to_block(exit);
        self.b.seal_block(exit);
        self.write_str("]")
    }

    /// A string literal: a pointer to its bytes in the data section, and its length.
    fn str_val(&mut self, s: &str) -> CResult<Vals> {
        let id = self.cg.string_data(s)?;
        let gv = self.cg.module.declare_data_in_func(id, self.b.func);
        let p = self.b.ins().symbol_value(types::I64, gv);
        let n = self.b.ins().iconst(types::I64, s.len() as i64);
        Ok(vec![p, n])
    }

    /// Call a one-argument output function from the C runtime (the stderr variant while panicking).
    fn rt_call(&mut self, name: &str, ty: types::Type, arg: Value) -> CResult<()> {
        let name = if self.stderr { name.replace("om_rt_write", "om_rt_ewrite") } else { name.to_string() };
        let id = self.cg.runtime_fn(&name, Some(ty))?;
        let f = self.cg.module.declare_func_in_func(id, self.b.func);
        self.b.ins().call(f, &[arg]);
        Ok(())
    }
}

/// The inclusive value range a literal or range pattern covers.
fn pat_range(p: &Pattern) -> Option<(i128, i128)> {
    match &p.kind {
        PatKind::Int(n) => Some((*n, *n)),
        PatKind::Range(lo, hi, incl) => Some((*lo, if *incl { *hi } else { hi - 1 })),
        PatKind::Char(c) => Some((*c as i128, *c as i128)),
        PatKind::CharRange(lo, hi, incl) => Some((*lo as i128, *hi as i128 - if *incl { 0 } else { 1 })),
        _ => None,
    }
}

/// Do two places share storage?
fn overlaps(a: &Loc, b: &Loc) -> bool {
    match (a, b) {
        (Loc::Vars(x), Loc::Vars(y)) => x.iter().any(|v| y.contains(v)),
        _ => false,
    }
}

fn irrefutable(p: &Pattern) -> bool {
    match &p.kind {
        PatKind::Wild | PatKind::Bind(..) => true,
        PatKind::Tuple(subs) => subs.iter().all(irrefutable),
        PatKind::Or(alts) => alts.iter().any(irrefutable),
        _ => false,
    }
}

fn has_bindings(p: &Pattern) -> bool {
    match &p.kind {
        PatKind::Bind(..) => true,
        PatKind::Variant(_, subs) | PatKind::Tuple(subs) | PatKind::Or(subs) => subs.iter().any(has_bindings),
        _ => false,
    }
}

/// The C runtime linked into every Om program. Kept tiny: the standard library replaces it later.
pub const RUNTIME_C: &str = r#"#ifdef _WIN32
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0602
#endif
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <conio.h>
#include <fcntl.h>
#include <io.h>
#else
#include <poll.h>
#include <sys/ioctl.h>
#include <termios.h>
#include <time.h>
#include <unistd.h>
#endif
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
double om_rt_math_sin(double x) { return sin(x); }
double om_rt_math_cos(double x) { return cos(x); }
double om_rt_math_tan(double x) { return tan(x); }
double om_rt_math_asin(double x) { return asin(x); }
double om_rt_math_acos(double x) { return acos(x); }
double om_rt_math_atan(double x) { return atan(x); }
double om_rt_math_exp(double x) { return exp(x); }
double om_rt_math_ln(double x) { return log(x); }
double om_rt_math_log10(double x) { return log10(x); }
double om_rt_math_log2(double x) { return log2(x); }
double om_rt_math_cbrt(double x) { return cbrt(x); }
double om_rt_math_powf(double x, double y) { return pow(x, y); }
double om_rt_math_atan2(double y, double x) { return atan2(y, x); }
double om_rt_math_hypot(double x, double y) { return hypot(x, y); }
double om_rt_math_fmod(double x, double y) { return fmod(x, y); }
static void om_oom(void) { fputs("out of memory\n", stderr); exit(134); }
/* All output goes through emit(): to stderr (err = 1), to the String being
   built by `"{x}"` formatting, or to stdout. Strings are built in a growable
   buffer, so no FILE tricks (open_memstream) are needed and it works everywhere. */
typedef struct { char *buf; size_t len, cap; } om_buf_;
static om_buf_ om_caps[256];
static int om_ncaps;
static long long om_last_len;
static void om_buf_push(om_buf_ *b, const char *s, size_t n) {
    if (b->len + n + 1 > b->cap) {
        size_t cap = b->cap ? b->cap * 2 : 64;
        while (cap < b->len + n + 1) cap *= 2;
        char *p = (char *)realloc(b->buf, cap);
        if (!p) om_oom();
        b->buf = p; b->cap = cap;
    }
    if (n) memcpy(b->buf + b->len, s, n);
    b->len += n;
    b->buf[b->len] = 0;
}
static int emit(int err, const char *s, size_t n) {
    if (err) fwrite(s, 1, n, stderr);
    else if (om_ncaps) om_buf_push(&om_caps[om_ncaps - 1], s, n);
    else fwrite(s, 1, n, stdout);
    return (int)n;
}
static int emit_str(int err, const char *s) { return emit(err, s, strlen(s)); }
/* Format a float like Rust: `{}` (Display) never uses an exponent and prints
   4.0 as "4"; `{:?}` (Debug) prints "4.0" and switches to "1e16" / "1e-5" for
   very large or small values. `digits` is 17 for f64 and 9 for f32. */
static int write_float(int err, double x, int is_f32, int debug) {
    if (isnan(x)) return emit_str(err, "NaN");
    if (isinf(x)) return emit_str(err, x > 0 ? "inf" : "-inf");
    char e[40];
    int maxp = is_f32 ? 9 : 17;
    for (int p = 1; p <= maxp; p++) {
        snprintf(e, sizeof e, "%.*e", p - 1, x);
        if (is_f32 ? strtof(e, NULL) == (float)x : strtod(e, NULL) == x) break;
    }
    /* e is like "-1.2345e+07": collect sign, digits and exponent. */
    char digits[24];
    int nd = 0, neg = 0, i = 0;
    if (e[0] == '-') { neg = 1; i = 1; }
    for (; e[i] && e[i] != 'e'; i++) if (e[i] != '.') digits[nd++] = e[i];
    int exp = atoi(e + i + 1);
    while (nd > 1 && digits[nd - 1] == '0') nd--;
    digits[nd] = 0;
    char buf[400];
    int k = 0;
    if (neg) buf[k++] = '-';
    if (debug && (exp < -4 || exp >= 16)) {
        buf[k++] = digits[0];
        if (nd > 1) { buf[k++] = '.'; for (int j = 1; j < nd; j++) buf[k++] = digits[j]; }
        k += snprintf(buf + k, sizeof buf - k, "e%d", exp);
    } else if (exp >= 0) {
        for (int j = 0; j <= exp; j++) buf[k++] = j < nd ? digits[j] : '0';
        if (nd > exp + 1) { buf[k++] = '.'; for (int j = exp + 1; j < nd; j++) buf[k++] = digits[j]; }
        else if (debug) { buf[k++] = '.'; buf[k++] = '0'; }
    } else {
        buf[k++] = '0'; buf[k++] = '.';
        for (int j = 0; j < -exp - 1; j++) buf[k++] = '0';
        for (int j = 0; j < nd; j++) buf[k++] = digits[j];
    }
    return emit(err, buf, (size_t)k);
}
static int write_char(int err, unsigned c) {
    /* UTF-8 encode one Unicode scalar value. */
    char b[4];
    int n;
    if (c < 0x80) { b[0] = (char)c; n = 1; }
    else if (c < 0x800) { b[0] = (char)(0xC0 | (c >> 6)); b[1] = (char)(0x80 | (c & 0x3F)); n = 2; }
    else if (c < 0x10000) {
        b[0] = (char)(0xE0 | (c >> 12)); b[1] = (char)(0x80 | ((c >> 6) & 0x3F)); b[2] = (char)(0x80 | (c & 0x3F)); n = 3;
    } else {
        b[0] = (char)(0xF0 | (c >> 18)); b[1] = (char)(0x80 | ((c >> 12) & 0x3F));
        b[2] = (char)(0x80 | ((c >> 6) & 0x3F)); b[3] = (char)(0x80 | (c & 0x3F)); n = 4;
    }
    return emit(err, b, (size_t)n);
}
static int write_i64(int err, long long n) { char b[24]; return emit(err, b, (size_t)snprintf(b, sizeof b, "%lld", n)); }
static int write_u64(int err, unsigned long long n) {
    char b[24];
    return emit(err, b, (size_t)snprintf(b, sizeof b, "%llu", n));
}
void om_rt_capture_begin(void) {
    if (om_ncaps == 256) { fputs("string formatting nested too deeply\n", stderr); exit(134); }
    om_buf_ empty = { NULL, 0, 0 };
    om_caps[om_ncaps++] = empty;
}
char *om_rt_capture_end(void) {
    om_buf_ *b = &om_caps[--om_ncaps];
    if (!b->buf) om_buf_push(b, "", 0);
    om_last_len = (long long)b->len;
    return b->buf;
}
long long om_rt_capture_len(void) { return om_last_len; }
static char om_parse_ok_;
double om_rt_parse_f64(const char *s, long long n) {
    char buf[64];
    om_parse_ok_ = 0;
    if (n <= 0 || n >= (long long)sizeof buf) return 0.0;
    memcpy(buf, s, (size_t)n);
    buf[n] = 0;
    /* Rust accepts digits, '.', 'e', a sign, "inf", "infinity" and "nan"; no spaces or hex. */
    for (long long i = 0; i < n; i++) {
        char c = buf[i];
        if (c == ' ' || c == '\t' || c == '\n' || c == 'x' || c == 'X' || c == 'p' || c == 'P') return 0.0;
    }
    char *end;
    double x = strtod(buf, &end);
    om_parse_ok_ = *end == 0;
    return x;
}
char om_rt_parse_ok(void) { return om_parse_ok_; }
int om_rt_write_i64(long long n) { return write_i64(0, n); }
int om_rt_write_u64(unsigned long long n) { return write_u64(0, n); }
int om_rt_write_f32(float x) { return write_float(0, x, 1, 0); }
int om_rt_write_f32_dbg(float x) { return write_float(0, x, 1, 1); }
int om_rt_ewrite_f32_dbg(float x) { return write_float(1, x, 1, 1); }
int om_rt_write_f64_dbg(double x) { return write_float(0, x, 0, 1); }
int om_rt_ewrite_f64_dbg(double x) { return write_float(1, x, 0, 1); }
int om_rt_write_char(unsigned c) { return write_char(0, c); }
int om_rt_ewrite_u64(unsigned long long n) { return write_u64(1, n); }
int om_rt_ewrite_f32(float x) { return write_float(1, x, 1, 0); }
int om_rt_ewrite_char(unsigned c) { return write_char(1, c); }
void om_rt_write_bytes(const char *s, long long n) { if (n > 0) emit(0, s, (size_t)n); }
int om_rt_write_f64(double x) { return write_float(0, x, 0, 0); }
int om_rt_ewrite_i64(long long n) { return write_i64(1, n); }
void om_rt_ewrite_bytes(const char *s, long long n) { if (n > 0) emit(1, s, (size_t)n); }
int om_rt_ewrite_f64(double x) { return write_float(1, x, 0, 0); }
int om_rt_exit_panic(void) { fflush(stdout); exit(101); }
void om_rt_flush(void) { fflush(stdout); }
void *om_rt_alloc(long long size) {
    void *p = malloc(size > 0 ? (size_t)size : 1);
    if (!p) om_oom();
    return p;
}
void *om_rt_realloc(void *p, long long size) {
    p = realloc(p, size > 0 ? (size_t)size : 1);
    if (!p) om_oom();
    return p;
}
void om_rt_free(void *p) { free(p); }
/* Terminal input and time. On a terminal, read_key switches to raw mode on first
   use (no echo, no line buffering) and restores the terminal at exit. Arrow keys
   come back as 1000 (up), 1001 (down), 1002 (right), 1003 (left); end of input is -1. */
#ifdef _WIN32
/* Set up the Windows console before main runs: UTF-8 output, ANSI escape codes
   (colours, cursor moves) and "\n" line endings like Rust. Restored at exit. */
#ifndef ENABLE_VIRTUAL_TERMINAL_PROCESSING
#define ENABLE_VIRTUAL_TERMINAL_PROCESSING 0x0004
#endif
static UINT om_saved_cp_;
static DWORD om_saved_mode_[2];
static int om_have_mode_[2];
static void om_win_restore(void) {
    fflush(stdout);
    fflush(stderr);
    if (om_saved_cp_) SetConsoleOutputCP(om_saved_cp_);
    if (om_have_mode_[0]) SetConsoleMode(GetStdHandle(STD_OUTPUT_HANDLE), om_saved_mode_[0]);
    if (om_have_mode_[1]) SetConsoleMode(GetStdHandle(STD_ERROR_HANDLE), om_saved_mode_[1]);
}
static void om_win_init(void) {
    _setmode(_fileno(stdout), _O_BINARY);
    _setmode(_fileno(stderr), _O_BINARY);
    om_saved_cp_ = GetConsoleOutputCP();
    if (om_saved_cp_) SetConsoleOutputCP(CP_UTF8);
    HANDLE h[2] = { GetStdHandle(STD_OUTPUT_HANDLE), GetStdHandle(STD_ERROR_HANDLE) };
    for (int i = 0; i < 2; i++) {
        if (GetConsoleMode(h[i], &om_saved_mode_[i])) {
            om_have_mode_[i] = 1;
            SetConsoleMode(h[i], om_saved_mode_[i] | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
    atexit(om_win_restore);
}
#ifdef _MSC_VER
#pragma section(".CRT$XCU", read)
__declspec(allocate(".CRT$XCU")) void (*om_win_init_ptr)(void) = om_win_init;
#pragma comment(linker, "/include:om_win_init_ptr")
#else
__attribute__((constructor)) static void om_win_init_ctor(void) { om_win_init(); }
#endif
long long om_rt_read_key(void) {
    fflush(stdout);
    if (!_isatty(_fileno(stdin))) { int c = getchar(); return c == EOF ? -1 : c; }
    int c = _getch();
    if (c == 0 || c == 0xE0) {
        switch (_getch()) { case 72: return 1000; case 80: return 1001; case 77: return 1002; case 75: return 1003; default: return 0; }
    }
    if (c == 3) exit(130); /* Ctrl+C still ends the program, as on Linux and macOS. */
    return c == '\r' ? '\n' : c;
}
long long om_rt_time_ns(void) {
    FILETIME ft;
    GetSystemTimePreciseAsFileTime(&ft);
    /* 100 ns ticks since 1601 -> nanoseconds since 1970. */
    unsigned long long t = ((unsigned long long)ft.dwHighDateTime << 32) | ft.dwLowDateTime;
    return (long long)(t - 116444736000000000ULL) * 100LL;
}
void om_rt_sleep_ms(long long ms) { fflush(stdout); if (ms > 0) Sleep((DWORD)ms); }
long long om_rt_poll_key(void) {
    fflush(stdout);
    if (!_isatty(_fileno(stdin))) { int c = getchar(); return c == EOF ? -1 : c; }
    return _kbhit() ? om_rt_read_key() : 0;
}
static int om_console_size(int rows) {
    CONSOLE_SCREEN_BUFFER_INFO info;
    if (!GetConsoleScreenBufferInfo(GetStdHandle(STD_OUTPUT_HANDLE), &info)) return rows ? 24 : 80;
    return rows ? info.srWindow.Bottom - info.srWindow.Top + 1 : info.srWindow.Right - info.srWindow.Left + 1;
}
long long om_rt_term_cols(void) { return om_console_size(0); }
long long om_rt_term_rows(void) { return om_console_size(1); }
static void om_cooked_input(void) {}
#else
static struct termios om_saved_tty_;
static int om_tty_raw_;
static void om_restore_tty(void) { if (om_tty_raw_) tcsetattr(0, TCSANOW, &om_saved_tty_); }
static int om_getc(int wait_ms) {
    if (wait_ms >= 0) {
        struct pollfd p = { 0, POLLIN, 0 };
        if (poll(&p, 1, wait_ms) <= 0) return -1;
    }
    unsigned char c;
    return read(0, &c, 1) == 1 ? c : -1;
}
static int om_tty_saved_;
static void om_enter_raw(void) {
    if (om_tty_raw_ || !isatty(0)) return;
    if (!om_tty_saved_) {
        if (tcgetattr(0, &om_saved_tty_) != 0) return;
        om_tty_saved_ = 1;
        atexit(om_restore_tty);
    }
    struct termios raw = om_saved_tty_;
    raw.c_lflag &= ~(tcflag_t)(ICANON | ECHO);
    raw.c_cc[VMIN] = 1;
    raw.c_cc[VTIME] = 0;
    if (tcsetattr(0, TCSANOW, &raw) == 0) om_tty_raw_ = 1;
}
/* Back to line-by-line input with echo, for read_line. */
static void om_cooked_input(void) {
    if (om_tty_raw_) { tcsetattr(0, TCSANOW, &om_saved_tty_); om_tty_raw_ = 0; }
}
static long long om_decode_key(int c) {
    if (c == 27 && om_tty_raw_) {
        int b = om_getc(30);
        if (b == '[' || b == 'O') {
            switch (om_getc(30)) { case 'A': return 1000; case 'B': return 1001; case 'C': return 1002; case 'D': return 1003; default: return 0; }
        }
        return 27;
    }
    return c == '\r' ? '\n' : c;
}
long long om_rt_poll_key(void) {
    fflush(stdout);
    om_enter_raw();
    struct pollfd p = { 0, POLLIN, 0 };
    if (poll(&p, 1, 0) <= 0) return 0;
    return om_decode_key(om_getc(-1));
}
static int om_term_size(int rows) {
    struct winsize w;
    if (ioctl(1, TIOCGWINSZ, &w) == 0 && w.ws_col > 0 && w.ws_row > 0) return rows ? w.ws_row : w.ws_col;
    const char *e = getenv(rows ? "LINES" : "COLUMNS");
    int n = e ? atoi(e) : 0;
    return n > 0 ? n : (rows ? 24 : 80);
}
long long om_rt_term_cols(void) { return om_term_size(0); }
long long om_rt_term_rows(void) { return om_term_size(1); }
long long om_rt_read_key(void) {
    fflush(stdout);
    om_enter_raw();
    int c = om_getc(-1);
    return om_decode_key(c);
}
long long om_rt_time_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    return (long long)ts.tv_sec * 1000000000LL + ts.tv_nsec;
}
void om_rt_sleep_ms(long long ms) {
    fflush(stdout);
    if (ms <= 0) return;
    struct timespec ts = { (time_t)(ms / 1000), (long)(ms % 1000) * 1000000L };
    nanosleep(&ts, NULL);
}
#endif
/* Command-line arguments, saved by main for args(). */
static int om_argc_;
static char **om_argv_;
void om_rt_set_args(int argc, char **argv) { om_argc_ = argc; om_argv_ = argv; }
long long om_rt_argc(void) { return om_argc_; }
const char *om_rt_arg_ptr(long long i) { return i >= 0 && i < om_argc_ && om_argv_[i] ? om_argv_[i] : ""; }
long long om_rt_arg_len(long long i) { return (long long)strlen(om_rt_arg_ptr(i)); }
/* One line of input without its line ending; read_line_ok is 0 at end of input. */
static long long om_line_len_;
static char om_line_ok_;
char *om_rt_read_line(void) {
    fflush(stdout);
    om_cooked_input();
    size_t cap = 64, n = 0;
    char *buf = (char *)malloc(cap);
    if (!buf) { fputs("out of memory\n", stderr); exit(134); }
    int c = EOF;
    while ((c = getchar()) != EOF && c != '\n') {
        if (n + 1 >= cap) {
            cap *= 2;
            buf = (char *)realloc(buf, cap);
            if (!buf) { fputs("out of memory\n", stderr); exit(134); }
        }
        buf[n++] = (char)c;
    }
    if (n > 0 && buf[n - 1] == '\r') n--;
    om_line_ok_ = !(c == EOF && n == 0);
    om_line_len_ = (long long)n;
    return buf;
}
long long om_rt_read_line_len(void) { return om_line_len_; }
char om_rt_read_line_ok(void) { return om_line_ok_; }
void om_rt_memmove(void *dst, const void *src, long long n) { if (n > 0) memmove(dst, src, (size_t)n); }
int om_rt_memcmp(const void *a, const void *b, long long n) { return n > 0 ? memcmp(a, b, (size_t)n) : 0; }
"#;

#[cfg(test)]
mod tests {
    #[test]
    fn whole_prelude_type_checks() {
        let p = om_parser::parse("fn main() {}").unwrap();
        if let Err(e) = super::compile_with(&p, true) {
            let src = super::prelude_source();
            panic!("{e} at prelude {:?}", &src[e.span.start..e.span.end.max(e.span.start + 40).min(src.len())]);
        }
    }
}
