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
use cranelift_codegen::ir::{types, AbiParam, Block as ClBlock, InstBuilder, Signature, TrapCode, Value};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use om_lexer::Span;
use om_parser::ast::*;
use std::collections::HashMap;
use std::fmt;

/// Types and functions every Om program can use without importing them.
pub const PRELUDE: &str = include_str!("prelude.om");

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

/// Om types, with all generic parameters already substituted.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    I64,
    F64,
    Bool,
    Str,
    Unit,
    /// The type of an expression that never finishes, such as `return` or an endless `loop`.
    Never,
    Tuple(Vec<Ty>),
    Struct(String, Vec<Ty>),
    Enum(String, Vec<Ty>),
}

fn join<T: fmt::Display>(items: &[T]) -> String {
    items.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(", ")
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::I64 => f.write_str("i64"),
            Ty::F64 => f.write_str("f64"),
            Ty::Bool => f.write_str("bool"),
            Ty::Str => f.write_str("str"),
            Ty::Unit => f.write_str("()"),
            Ty::Never => f.write_str("!"),
            Ty::Tuple(ts) if ts.len() == 1 => write!(f, "({},)", ts[0]),
            Ty::Tuple(ts) => write!(f, "({})", join(ts)),
            Ty::Struct(n, a) | Ty::Enum(n, a) if a.is_empty() => f.write_str(n),
            Ty::Struct(n, a) | Ty::Enum(n, a) => write!(f, "{n}[{}]", join(a)),
        }
    }
}

impl Ty {
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
    impl_ty: Option<TypeExpr>,
}

impl FnDef {
    fn all_generics(&self) -> Vec<String> {
        self.impl_generics.iter().chain(&self.f.generics).cloned().collect()
    }
}

/// The type of a constant expression, or `None` if it is not a constant.
fn const_type(e: &Expr) -> Option<Ty> {
    match &e.kind {
        ExprKind::Int(_) => Some(Ty::I64),
        ExprKind::Float(_) => Some(Ty::F64),
        ExprKind::Bool(_) => Some(Ty::Bool),
        ExprKind::Str(_) => Some(Ty::Str),
        ExprKind::Unary(UnOp::Neg, inner) => match inner.kind {
            ExprKind::Int(_) => Some(Ty::I64),
            ExprKind::Float(_) => Some(Ty::F64),
            _ => None,
        },
        _ => None,
    }
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
    let prelude = om_parser::parse(PRELUDE).map_err(|e| internal(format!("prelude: {e}")))?;
    let mut all = prelude;
    all.functions.extend(program.functions.iter().cloned());
    all.structs.extend(program.structs.iter().cloned());
    all.enums.extend(program.enums.iter().cloned());
    all.impls.extend(program.impls.iter().cloned());
    all.consts.extend(program.consts.iter().cloned());
    let program = &all;

    let mut flags = settings::builder();
    flags.set("opt_level", "speed").map_err(internal)?;
    flags.set("is_pic", "true").map_err(internal)?;
    // Structs returned from functions can need more return registers than the ABI has.
    flags.set("enable_multi_ret_implicit_sret", "true").map_err(internal)?;
    let isa = cranelift_native::builder()
        .map_err(|e| internal(format!("unsupported host: {e}")))?
        .finish(settings::Flags::new(flags))
        .map_err(internal)?;
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
    };

    cg.declare_types(program)?;
    for c in &program.consts {
        let ty = cg.resolve(&c.ty, &Subst::new())?;
        match const_type(&c.value) {
            Some(t) if t == ty => {}
            Some(t) => return err(format!("constant `{}` is declared as {ty} but its value is {t}", c.name), c.span),
            None => return err("a `const` value must be a literal (number, bool or string)", c.value.span),
        }
        if cg.consts.insert(c.name.clone(), (ty, c.value.clone())).is_some() {
            return err(format!("constant `{}` is defined twice", c.name), c.span);
        }
    }

    for f in &program.functions {
        cg.add_def(f.name.clone(), FnDef { f: f.clone(), impl_generics: vec![], impl_ty: None }, f.span)?;
    }
    for imp in &program.impls {
        let name = &imp.ty.name;
        let generics = match (cg.structs.get(name), cg.enums.get(name)) {
            (Some((g, _)), _) | (_, Some((g, _))) => g.clone(),
            _ => return err(format!("cannot find struct or enum `{name}` for this `impl`"), imp.ty.span),
        };
        let listed: Vec<&str> = imp.ty.args.iter().map(|a| a.name.as_str()).collect();
        if imp.generics != generics || listed != generics.iter().map(String::as_str).collect::<Vec<_>>() {
            let want = if generics.is_empty() {
                format!("impl {name}")
            } else {
                format!("impl[{0}] {name}[{0}]", generics.join(", "))
            };
            return err(format!("this impl must be written `{want}`"), imp.ty.span);
        }
        for m in &imp.methods {
            let def = FnDef { f: m.clone(), impl_generics: imp.generics.clone(), impl_ty: Some(imp.ty.clone()) };
            cg.add_def(format!("{name}::{}", m.name), def, m.span)?;
        }
    }
    let Some(main) = program.functions.iter().find(|f| f.name == "main") else {
        return err("program has no `fn main()`", Span::default());
    };
    if !main.params.is_empty() || !main.generics.is_empty() {
        return err("`main` takes no parameters", main.span);
    }

    // Instantiate every non-generic function now, so all of them are checked even
    // if unused. Generic ones are instantiated on first use with concrete types.
    let mut keys: Vec<String> =
        cg.defs.iter().filter(|(_, d)| d.all_generics().is_empty()).map(|(k, _)| k.clone()).collect();
    keys.sort();
    for k in keys {
        cg.instantiate(&k, Subst::new(), Span::default())?;
    }
    let mut ctx = cg.module.make_context();
    let mut fctx = FunctionBuilderContext::new();
    while let Some((inst, key, subst)) = cg.queue.pop() {
        cg.define(&inst, &key, subst, &mut ctx, &mut fctx)?;
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
}

const BUILTINS: &[&str] = &["print", "panic", "assert"];

impl Codegen {
    fn add_def(&mut self, key: String, def: FnDef, span: Span) -> CResult<()> {
        if BUILTINS.contains(&key.as_str()) {
            return err(format!("`{key}` is a built-in function and cannot be redefined"), span);
        }
        if self.defs.insert(key.clone(), def).is_some() {
            return err(format!("function `{key}` is defined twice"), span);
        }
        Ok(())
    }

    fn resolve(&self, t: &TypeExpr, subst: &Subst) -> CResult<Ty> {
        let args = || t.args.iter().map(|a| self.resolve(a, subst)).collect::<CResult<Vec<_>>>();
        if t.name == TypeExpr::TUPLE {
            let ts = args()?;
            return Ok(if ts.is_empty() { Ty::Unit } else { Ty::Tuple(ts) });
        }
        let prim = match t.name.as_str() {
            "i64" => Some(Ty::I64),
            "f64" => Some(Ty::F64),
            "bool" => Some(Ty::Bool),
            "str" => Some(Ty::Str),
            _ => None,
        };
        if let Some(p) = prim.or_else(|| subst.get(&t.name).cloned()) {
            if !t.args.is_empty() {
                return err(format!("`{}` does not take type arguments", t.name), t.span);
            }
            return Ok(p);
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
                    format!("type `{name}` contains itself, which would make it infinitely large (heap boxes arrive with the standard library)"),
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
            Ty::I64 | Ty::Str => vec![types::I64],
            Ty::F64 => vec![types::F64],
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
                    match const_type(d) {
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
        let mut lw = Lower {
            cg: self,
            b,
            scopes: vec![HashMap::new()],
            defers: vec![vec![]],
            loops: vec![],
            ret: ret.clone(),
            is_main,
            dead: false,
            subst,
            inouts,
            tmp_count: 0,
            stderr: false,
        };
        let mut next = 0;
        for ((pname, pty, mode), p) in params.iter().zip(&f.params) {
            let n = lw.cg.width(pty);
            let vals = entry_vals[next..next + n].to_vec();
            next += n;
            let mutable = p.mutable || *mode == ParamMode::RefMut;
            lw.bind_vals(pname, pty.clone(), vals, mutable, p.span)?;
        }

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
            _ => match self.resolve(te, subst) {
                Ok(t) => t == *ty,
                Err(_) => false,
            },
        }
    }
}

struct Local {
    vars: Vec<Variable>,
    ty: Ty,
    mutable: bool,
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
}

/// A join point that collects one value from each incoming branch.
struct Merge {
    blk: ClBlock,
    ty: Option<Ty>,
}

/// A variable, or a field path inside one, that can be read and assigned.
struct Place {
    root: String,
    vars: Vec<Variable>,
    ty: Ty,
    mutable: bool,
}

struct Lower<'a> {
    cg: &'a mut Codegen,
    b: FunctionBuilder<'a>,
    scopes: Vec<HashMap<String, Local>>,
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
}

impl<'a> Lower<'a> {
    // ---------- variables and places ----------

    fn bind_vals(&mut self, name: &str, ty: Ty, vals: Vals, mutable: bool, span: Span) -> CResult<()> {
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
        self.scopes.last_mut().unwrap().insert(name.to_string(), Local { vars, ty, mutable });
        Ok(())
    }

    fn lookup(&self, name: &str) -> Option<&Local> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    fn resolve(&self, t: &TypeExpr) -> CResult<Ty> {
        self.cg.resolve(t, &self.subst)
    }

    /// Resolve `x`, `x.field`, `x.0` or `x.a.b` to the variables that hold it, if `e` is such a path.
    fn place(&self, e: &Expr) -> CResult<Option<Place>> {
        match &e.kind {
            ExprKind::Var(name) => Ok(self.lookup(name).map(|l| Place {
                root: name.clone(),
                vars: l.vars.clone(),
                ty: l.ty.clone(),
                mutable: l.mutable,
            })),
            ExprKind::Field(inner, f) => {
                let Some(p) = self.place(inner)? else { return Ok(None) };
                let Some((fty, off)) = self.cg.field(&p.ty, f) else {
                    return err(format!("{} has no field `{f}`", p.ty), e.span);
                };
                let w = self.cg.width(&fty);
                Ok(Some(Place { root: p.root, vars: p.vars[off..off + w].to_vec(), ty: fty, mutable: p.mutable }))
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
    fn jump_to(&mut self, m: &mut Merge, ty: Ty, vals: Vals, what: &str, span: Span) -> CResult<()> {
        if ty == Ty::Never {
            return Ok(());
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
            let vars = self.scopes[0][&name].vars.clone();
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
        self.scopes.push(HashMap::new());
        self.defers.push(Vec::new());
        let mut result = self.block_inner(blk, expected);
        if result.is_ok() && !self.dead {
            let level = self.scopes.len() - 1;
            if let Err(e) = self.run_defers(level) {
                result = Err(e);
            }
        }
        self.defers.pop();
        self.scopes.pop();
        result
    }

    fn block_inner(&mut self, blk: &Block, expected: Option<&Ty>) -> CResult<(Vals, Ty)> {
        for s in &blk.stmts {
            self.stmt(s)?;
        }
        if let Some(tail) = &blk.tail {
            let r = self.expr_expect(tail, expected)?;
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
            self.run_defers(level)?;
        }
        Ok(())
    }

    // ---------- statements ----------

    fn stmt(&mut self, s: &Stmt) -> CResult<()> {
        match s {
            Stmt::Let { pat, ty, value, span } => {
                let want = match ty {
                    Some(t) => Some(self.resolve(t)?),
                    None => None,
                };
                let (vals, vty) = self.expr_expect(value, want.as_ref())?;
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
                self.pat_bind(pat, &vty, &vals)
            }
            Stmt::Assign { target, op, value, span } => self.assign(target, *op, value, *span),
            Stmt::While { cond, body, .. } => {
                let header = self.b.create_block();
                let body_blk = self.b.create_block();
                let exit = self.b.create_block();
                self.b.ins().jump(header, &[]);
                self.b.switch_to_block(header);
                let c = self.cond(cond)?;
                self.b.ins().brif(c, body_blk, &[], exit, &[]);
                // The normal exit carries no value.
                self.loop_body(body_blk, header, Merge { blk: exit, ty: Some(Ty::Unit) }, false, body)?;
                self.b.seal_block(header);
                Ok(())
            }
            Stmt::For { var, start, end, inclusive, body, span } => {
                let (sv, st) = self.expr(start)?;
                let (ev, et) = self.expr(end)?;
                if st != Ty::I64 || et != Ty::I64 {
                    return err(format!("`for` ranges need i64 bounds, found {st} and {et}"), *span);
                }
                let counter = self.b.declare_var(types::I64);
                self.b.def_var(counter, sv[0]);
                let header = self.b.create_block();
                let body_blk = self.b.create_block();
                let step = self.b.create_block();
                let exit = self.b.create_block();
                self.b.ins().jump(header, &[]);
                self.b.switch_to_block(header);
                let i = self.b.use_var(counter);
                let cc = if *inclusive { IntCC::SignedLessThanOrEqual } else { IntCC::SignedLessThan };
                let c = self.b.ins().icmp(cc, i, ev[0]);
                self.b.ins().brif(c, body_blk, &[], exit, &[]);
                // Bind the loop variable in its own scope around the body.
                self.b.switch_to_block(body_blk);
                self.b.seal_block(body_blk);
                self.scopes.push(HashMap::new());
                self.defers.push(Vec::new());
                let i = self.b.use_var(counter);
                self.bind_vals(var, Ty::I64, vec![i], false, *span)?;
                let after = self.b.create_block();
                self.b.ins().jump(after, &[]);
                let r = self.loop_body(after, step, Merge { blk: exit, ty: Some(Ty::Unit) }, false, body);
                self.defers.pop();
                self.scopes.pop();
                r?;
                self.b.switch_to_block(step);
                self.b.seal_block(step);
                let i = self.b.use_var(counter);
                let next = self.b.ins().iadd_imm_s(i, 1);
                self.b.def_var(counter, next);
                self.b.ins().jump(header, &[]);
                self.b.seal_block(header);
                self.b.switch_to_block(exit);
                Ok(())
            }
            Stmt::Return { value, span } => {
                let ret = self.ret.clone();
                let (vals, ty) = match value {
                    Some(e) => self.expr_expect(e, Some(&ret))?,
                    None => (vec![], Ty::Unit),
                };
                if ty != self.ret && ty != Ty::Never {
                    return err(format!("this function returns {}, but `return` gives {ty}", self.ret), *span);
                }
                if !self.dead {
                    self.do_return(vals)?;
                }
                self.start_dead_block();
                Ok(())
            }
            Stmt::Break { value, span } => {
                let Some(ctx) = self.loops.last() else {
                    return err("`break` outside of a loop", *span);
                };
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
                    let mut ctx = self.loops.pop().unwrap();
                    ctx.broken = true;
                    let r = self.jump_to(&mut ctx.exit, ty, vals, "`break` values", *span);
                    self.loops.push(ctx);
                    r?;
                }
                self.start_dead_block();
                Ok(())
            }
            Stmt::Continue { span } => {
                let Some(ctx) = self.loops.last() else {
                    return err("`continue` outside of a loop", *span);
                };
                let (depth, cont) = (ctx.depth, ctx.cont);
                if !self.dead {
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
            Stmt::Expr(e) => self.expr(e).map(|_| ()),
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
        body: &Block,
    ) -> CResult<Merge> {
        self.b.switch_to_block(body_blk);
        self.b.seal_block(body_blk);
        let was_dead = self.dead;
        self.dead = false;
        self.loops.push(LoopCtx { cont, exit, depth: self.scopes.len(), is_loop_expr, broken: false });
        let r = self.block_expect(body, None);
        let ctx = self.loops.pop().unwrap();
        let (_, ty) = r?;
        if !self.dead {
            if ty != Ty::Unit {
                return err(format!("a loop body must not produce a value, but this one gives {ty}"), body.span);
            }
            self.b.ins().jump(cont, &[]);
        }
        let exit = ctx.exit;
        self.b.switch_to_block(exit.blk);
        self.b.seal_block(exit.blk);
        self.dead = was_dead;
        if is_loop_expr && !ctx.broken {
            self.dead = true;
        }
        Ok(exit)
    }

    fn assign(&mut self, target: &Expr, op: Option<BinOp>, value: &Expr, span: Span) -> CResult<()> {
        let Some(place) = self.place(target)? else {
            return match &target.kind {
                ExprKind::Var(name) => err(format!("cannot find variable `{name}`"), target.span),
                _ => err("can only assign to a variable or a field", target.span),
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
        let (vals, vty) = match op {
            None => self.expr_expect(value, Some(&place.ty))?,
            Some(op) => {
                let cur = self.read(&place.vars);
                let (rv, rt) = self.expr(value)?;
                self.binop(op, cur, place.ty.clone(), rv, rt, span)?
            }
        };
        if vty == Ty::Never {
            return Ok(());
        }
        if place.ty != vty {
            return err(format!("cannot assign {vty} to a place of type {}", place.ty), value.span);
        }
        self.write(&place.vars, &vals);
        Ok(())
    }

    // ---------- expressions ----------

    fn cond(&mut self, e: &Expr) -> CResult<Value> {
        let (v, ty) = self.expr(e)?;
        match ty {
            Ty::Bool => Ok(v[0]),
            Ty::Never => Ok(self.b.ins().iconst(types::I8, 0)),
            _ => err(format!("condition must be bool, found {ty}"), e.span),
        }
    }

    fn zero(&mut self, t: types::Type) -> Value {
        if t == types::F64 {
            self.b.ins().f64const(0.0)
        } else {
            self.b.ins().iconst(t, 0)
        }
    }

    fn expr(&mut self, e: &Expr) -> CResult<(Vals, Ty)> {
        self.expr_expect(e, None)
    }

    /// Lower `e`. `expected` is the type the context wants, if known; it is a hint
    /// for inferring type arguments (as in `let x: Option[i64] = None`), not a check.
    fn expr_expect(&mut self, e: &Expr, expected: Option<&Ty>) -> CResult<(Vals, Ty)> {
        match &e.kind {
            ExprKind::Int(n) => Ok((vec![self.b.ins().iconst(types::I64, *n)], Ty::I64)),
            ExprKind::Float(x) => Ok((vec![self.b.ins().f64const(*x)], Ty::F64)),
            ExprKind::Bool(v) => Ok((vec![self.b.ins().iconst(types::I8, *v as i64)], Ty::Bool)),
            ExprKind::Str(s) => Ok((vec![self.str_ptr(s)?], Ty::Str)),
            ExprKind::Var(name) => {
                if let Some(l) = self.lookup(name) {
                    let (vars, ty) = (l.vars.clone(), l.ty.clone());
                    return Ok((self.read(&vars), ty));
                }
                if let Some((ty, value)) = self.cg.consts.get(name).cloned() {
                    let (v, _) = self.expr(&value)?;
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
            ExprKind::Unary(op, inner) => {
                let (v, ty) = self.expr_expect(inner, expected)?;
                match (op, &ty) {
                    (UnOp::Neg, Ty::I64) => Ok((vec![self.b.ins().ineg(v[0])], Ty::I64)),
                    (UnOp::Neg, Ty::F64) => Ok((vec![self.b.ins().fneg(v[0])], Ty::F64)),
                    (UnOp::Not, Ty::Bool) => Ok((vec![self.b.ins().bxor_imm_u(v[0], 1)], Ty::Bool)),
                    (_, Ty::Never) => Ok((vec![], Ty::Never)),
                    (UnOp::Neg, _) => err(format!("cannot negate a {ty}"), e.span),
                    (UnOp::Not, _) => err(format!("`!` needs a bool, found {ty}"), e.span),
                }
            }
            ExprKind::Binary(op @ (BinOp::And | BinOp::Or), l, r) => self.short_circuit(*op, l, r),
            ExprKind::Binary(op, l, r) => {
                let (lv, lt) = self.expr(l)?;
                let (rv, rt) = self.expr_expect(r, Some(&lt))?;
                self.binop(*op, lv, lt, rv, rt, e.span)
            }
            ExprKind::Call(name, args) => self.call(name, args, e.span, expected),
            ExprKind::MethodCall(recv, m, args) => self.method_call(recv, m, args, e.span, expected),
            ExprKind::Field(inner, f) => {
                if let Some(p) = self.place(e)? {
                    return Ok((self.read(&p.vars), p.ty));
                }
                let (vals, ty) = self.expr(inner)?;
                let Some((fty, off)) = self.cg.field(&ty, f) else {
                    return err(format!("{ty} has no field `{f}`"), e.span);
                };
                let w = self.cg.width(&fty);
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
                let (v, from) = self.expr(inner)?;
                self.cast(v, from, to, e.span)
            }
            ExprKind::Interp(_) => err(
                "string interpolation can only be used inside `print` and `panic` for now (runtime strings arrive with the standard library)",
                e.span,
            ),
            ExprKind::If(cond, then, els) => self.if_expr(cond, then, els.as_ref(), e.span, expected),
            ExprKind::Match(scrut, arms) => self.match_expr(scrut, arms, e.span, expected),
            ExprKind::Loop(body) => {
                let header = self.b.create_block();
                let body_blk = self.b.create_block();
                let mut exit = self.new_merge();
                exit.ty = None;
                self.b.ins().jump(header, &[]);
                self.b.switch_to_block(header);
                self.b.ins().jump(body_blk, &[]);
                let was_dead = self.dead;
                let exit = self.loop_body(body_blk, header, exit, true, body);
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
        }
    }

    fn binop(&mut self, op: BinOp, lv: Vals, lt: Ty, rv: Vals, rt: Ty, span: Span) -> CResult<(Vals, Ty)> {
        if lt == Ty::Never || rt == Ty::Never {
            return Ok((vec![], Ty::Never));
        }
        if lt != rt {
            return err(format!("mismatched types: {lt} {} {rt} (convert with `as`)", op.symbol()), span);
        }
        let ins = self.b.ins();
        let out = match (op, &lt) {
            (BinOp::Add, Ty::I64) => (ins.iadd(lv[0], rv[0]), Ty::I64),
            (BinOp::Sub, Ty::I64) => (ins.isub(lv[0], rv[0]), Ty::I64),
            (BinOp::Mul, Ty::I64) => (ins.imul(lv[0], rv[0]), Ty::I64),
            (BinOp::Div, Ty::I64) => (ins.sdiv(lv[0], rv[0]), Ty::I64),
            (BinOp::Rem, Ty::I64) => (ins.srem(lv[0], rv[0]), Ty::I64),
            (BinOp::Add, Ty::F64) => (ins.fadd(lv[0], rv[0]), Ty::F64),
            (BinOp::Sub, Ty::F64) => (ins.fsub(lv[0], rv[0]), Ty::F64),
            (BinOp::Mul, Ty::F64) => (ins.fmul(lv[0], rv[0]), Ty::F64),
            (BinOp::Div, Ty::F64) => (ins.fdiv(lv[0], rv[0]), Ty::F64),
            (BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge, Ty::I64 | Ty::Bool) => {
                if lt == Ty::Bool && !matches!(op, BinOp::Eq | BinOp::Ne) {
                    return err(format!("`{}` is not defined for bool", op.symbol()), span);
                }
                let cc = match op {
                    BinOp::Eq => IntCC::Equal,
                    BinOp::Ne => IntCC::NotEqual,
                    BinOp::Lt => IntCC::SignedLessThan,
                    BinOp::Le => IntCC::SignedLessThanOrEqual,
                    BinOp::Gt => IntCC::SignedGreaterThan,
                    _ => IntCC::SignedGreaterThanOrEqual,
                };
                (ins.icmp(cc, lv[0], rv[0]), Ty::Bool)
            }
            (BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge, Ty::F64) => {
                let cc = match op {
                    BinOp::Eq => FloatCC::Equal,
                    BinOp::Ne => FloatCC::NotEqual,
                    BinOp::Lt => FloatCC::LessThan,
                    BinOp::Le => FloatCC::LessThanOrEqual,
                    BinOp::Gt => FloatCC::GreaterThan,
                    _ => FloatCC::GreaterThanOrEqual,
                };
                (ins.fcmp(cc, lv[0], rv[0]), Ty::Bool)
            }
            _ => return err(format!("operator `{}` is not defined for {lt}", op.symbol()), span),
        };
        Ok((vec![out.0], out.1))
    }

    fn cast(&mut self, v: Vals, from: Ty, to: Ty, span: Span) -> CResult<(Vals, Ty)> {
        let unit_enum = matches!(&from, Ty::Enum(..)) && self.cg.variants(&from).iter().all(|(_, f)| f.is_empty());
        let out = match (&from, &to) {
            (Ty::Never, _) => return Ok((vec![], Ty::Never)),
            (a, b) if a == b => return Ok((v, to)),
            (Ty::I64, Ty::F64) => self.b.ins().fcvt_from_sint(types::F64, v[0]),
            // Like Rust, float-to-int casts saturate and map NaN to 0.
            (Ty::F64, Ty::I64) => self.b.ins().fcvt_to_sint_sat(types::I64, v[0]),
            (Ty::Bool, Ty::I64) => self.b.ins().uextend(types::I64, v[0]),
            (Ty::Enum(..), Ty::I64) if unit_enum => v[0],
            _ => return err(format!("cannot cast {from} to {to}"), span),
        };
        Ok((vec![out], to))
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
        let c = self.cond(cond)?;
        let then_blk = self.b.create_block();
        let else_blk = self.b.create_block();
        let mut m = self.new_merge();
        self.b.ins().brif(c, then_blk, &[], else_blk, &[]);
        let was_dead = self.dead;

        self.b.switch_to_block(then_blk);
        self.b.seal_block(then_blk);
        self.dead = false;
        let (tv, tt) = self.block_expect(then, expected)?;
        if els.is_none() && tt != Ty::Unit && tt != Ty::Never {
            return err(format!("`if` without `else` must not produce a value, but this one gives {tt}"), then.span);
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
        }
        Ok(self.finish_merge(m, was_dead))
    }

    // ---------- structs, enums, Option and Result ----------

    /// Replace a leading `Self` in a path with the current `impl` type's name.
    fn self_name(&self, name: &str) -> String {
        match (name.strip_prefix("Self"), self.subst.get("Self").and_then(|t| t.base())) {
            (Some(rest), Some(t)) if rest.is_empty() || rest.starts_with("::") => format!("{t}{rest}"),
            _ => name.to_string(),
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

    /// `subst` on top of this function's own type arguments.
    fn merged(&self, subst: &Subst) -> Subst {
        let mut m = self.subst.clone();
        m.extend(subst.iter().map(|(k, v)| (k.clone(), v.clone())));
        m
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
        match name {
            "print" => return self.print(args, span),
            "panic" => return self.panic(args, span),
            "assert" => return self.assert(args, span),
            _ => {}
        }
        let name = self.self_name(name);
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
            if let Some(Ty::Struct(n, a) | Ty::Enum(n, a)) = self.subst.get("Self") {
                if n == ty {
                    preset = Codegen::type_subst(&def.impl_generics, a);
                }
            }
        }
        self.call_fn(&name, args, span, expected, preset)
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
        let (Ty::Struct(type_name, targs) | Ty::Enum(type_name, targs)) = &ty else {
            return err(format!("{ty} has no methods yet (methods on built-in types come with traits)"), span);
        };
        let key = format!("{type_name}::{m}");
        let Some(def) = self.cg.defs.get(&key) else {
            return err(format!("`{ty}` has no method named `{m}`"), span);
        };
        let self_mode = match def.f.params.first() {
            Some(p) if p.name == "self" => p.mode,
            _ => return err(format!("`{key}` has no `self` parameter; call it as `{key}(...)`"), span),
        };
        let preset = Codegen::type_subst(&def.impl_generics, targs);
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
        let mut writebacks: Vec<(usize, Vec<Variable>)> = Vec::new();
        for (a, &k) in args.iter().zip(&arg_slot) {
            let p = &ps[k];
            let te = p.ty.clone().unwrap_or_else(|| def.impl_ty.clone().unwrap());
            let what = format!("argument `{}`", p.name);
            let v = match (&a.value.kind, p.mode) {
                (ExprKind::Ref(true, inner), ParamMode::RefMut) => {
                    let Some(place) = self.place(inner)? else {
                        return err("`&mut` needs a variable or field to change", inner.span);
                    };
                    if !place.mutable {
                        let r = &place.root;
                        let hint = if r == "self" {
                            "take `&mut self`".to_string()
                        } else {
                            format!("declare it with `let mut {r}`")
                        };
                        return err(format!("cannot pass `{r}` as `&mut`; {hint}"), inner.span);
                    }
                    if writebacks.iter().any(|(_, vs)| vs.iter().any(|v| place.vars.contains(v))) {
                        return err(format!("cannot pass `{}` as `&mut` twice in one call", place.root), inner.span);
                    }
                    if !self.cg.unify(&te, &place.ty, &vars, &mut subst) {
                        return err(format!("{what} should be {}, found {}", te_string(&te), place.ty), inner.span);
                    }
                    writebacks.push((k, place.vars.clone()));
                    self.read(&place.vars)
                }
                (ExprKind::Ref(false, inner), ParamMode::Ref) => {
                    self.infer_arg(inner, &te, &vars, &mut subst, &what)?.0
                }
                (_, ParamMode::RefMut) => {
                    return err(format!("{what} is `&mut {}`; pass it as `&mut ...`", te_string(&te)), a.value.span)
                }
                (_, ParamMode::Ref) => {
                    return err(format!("{what} is `&{}`; pass it as `&...`", te_string(&te)), a.value.span)
                }
                (ExprKind::Ref(..), ParamMode::Value) => {
                    return err(format!("{what} takes a value, not a reference"), a.value.span)
                }
                _ => self.infer_arg(&a.value, &te, &vars, &mut subst, &what)?.0,
            };
            vals[k] = Some(v);
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
        for (_, vars) in writebacks {
            self.write(&vars, &results[pos..pos + vars.len()]);
            pos += vars.len();
        }
        Ok((results[..n].to_vec(), ret))
    }

    // ---------- patterns and match ----------

    fn match_expr(&mut self, scrut: &Expr, arms: &[Arm], span: Span, expected: Option<&Ty>) -> CResult<(Vals, Ty)> {
        let (sv, sty) = self.expr(scrut)?;
        if sty == Ty::Never {
            return Ok((vec![], Ty::Never));
        }
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
        for arm in arms {
            let test = self.pat_test(&arm.pat, &sty, &sv)?;
            let bind_blk = self.b.create_block();
            let next = self.b.create_block();
            self.b.ins().brif(test, bind_blk, &[], next, &[]);
            self.b.switch_to_block(bind_blk);
            self.b.seal_block(bind_blk);
            self.dead = false;
            self.scopes.push(HashMap::new());
            self.defers.push(Vec::new());
            let r = (|| {
                self.pat_bind(&arm.pat, &sty, &sv)?;
                if let Some(g) = &arm.guard {
                    let gv = self.cond(g)?;
                    let body_blk = self.b.create_block();
                    self.b.ins().brif(gv, body_blk, &[], next, &[]);
                    self.b.switch_to_block(body_blk);
                    self.b.seal_block(body_blk);
                }
                let hint = m.ty.clone().or(expected.cloned());
                let (v, t) = self.expr_expect(&arm.body, hint.as_ref())?;
                if !self.dead {
                    self.jump_to(&mut m, t, v, "match arms", arm.body.span)?;
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
        Ok(self.finish_merge(m, was_dead))
    }

    /// Check that `pat` can match a value of type `ty`.
    fn check_pattern(&self, pat: &Pattern, ty: &Ty) -> CResult<()> {
        let mismatch = |what: &str| err(format!("this pattern matches {what}, but the value is {ty}"), pat.span);
        match &pat.kind {
            PatKind::Wild | PatKind::Bind(..) => Ok(()),
            PatKind::Int(_) | PatKind::Range(..) if *ty == Ty::I64 => Ok(()),
            PatKind::Int(_) | PatKind::Range(..) => mismatch("an integer"),
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
            PatKind::Int(n) => self.b.ins().icmp_imm_s(IntCC::Equal, vals[0], *n),
            PatKind::Bool(v) => self.b.ins().icmp_imm_s(IntCC::Equal, vals[0], *v as i64),
            PatKind::Range(lo, hi, inclusive) => {
                let a = self.b.ins().icmp_imm_s(IntCC::SignedGreaterThanOrEqual, vals[0], *lo);
                let cc = if *inclusive { IntCC::SignedLessThanOrEqual } else { IntCC::SignedLessThan };
                let c = self.b.ins().icmp_imm_s(cc, vals[0], *hi);
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

    fn pat_bind(&mut self, pat: &Pattern, ty: &Ty, vals: &[Value]) -> CResult<()> {
        match &pat.kind {
            PatKind::Bind(name, mutable) => self.bind_vals(name, ty.clone(), vals.to_vec(), *mutable, pat.span),
            PatKind::Tuple(subs) => {
                let Ty::Tuple(ts) = ty else { return Ok(()) };
                let mut off = 0;
                for (s, t) in subs.iter().zip(ts) {
                    let w = self.cg.width(t);
                    self.pat_bind(s, t, &vals[off..off + w])?;
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
                    self.pat_bind(s, t, &vals[off..off + w])?;
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
                            let (v, t) = self.expr(e)?;
                            self.write_value(&v, &t, false)?;
                        }
                    }
                }
                Ok(())
            }
            _ => {
                let (v, t) = self.expr(&arg.value)?;
                self.write_value(&v, &t, false)
            }
        }
    }

    /// `print(x)` writes any value, or an interpolated string, followed by a newline.
    fn print(&mut self, args: &[Arg], span: Span) -> CResult<(Vals, Ty)> {
        let [arg] = args else {
            return err("`print` takes exactly one argument", span);
        };
        self.write_arg(arg)?;
        self.write_str("\n")?;
        Ok((vec![], Ty::Unit))
    }

    /// `panic(message)` prints the message to stderr and exits with status 101, like Rust.
    fn panic(&mut self, args: &[Arg], span: Span) -> CResult<(Vals, Ty)> {
        if args.len() > 1 {
            return err("`panic` takes at most one argument", span);
        }
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

    fn write_str(&mut self, s: &str) -> CResult<()> {
        let p = self.str_ptr(s)?;
        self.rt_call("om_rt_write_str", types::I64, p)
    }

    /// Write one value with no newline. Structs, tuples and enums print in the
    /// same shape as Rust's `{:?}`, with no `derive` needed.
    fn write_value(&mut self, vals: &[Value], ty: &Ty, nested: bool) -> CResult<()> {
        match ty {
            Ty::I64 => self.rt_call("om_rt_write_i64", types::I64, vals[0]),
            Ty::F64 => self.rt_call("om_rt_write_f64", types::F64, vals[0]),
            Ty::Bool => {
                let t = self.str_ptr("true")?;
                let f = self.str_ptr("false")?;
                let s = self.b.ins().select(vals[0], t, f);
                self.rt_call("om_rt_write_str", types::I64, s)
            }
            Ty::Str => {
                if nested {
                    self.write_str("\"")?;
                }
                self.rt_call("om_rt_write_str", types::I64, vals[0])?;
                if nested {
                    self.write_str("\"")?;
                }
                Ok(())
            }
            Ty::Unit => self.write_str("()"),
            Ty::Never => Ok(()),
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

    fn str_ptr(&mut self, s: &str) -> CResult<Value> {
        let id = self.cg.string_data(s)?;
        let gv = self.cg.module.declare_data_in_func(id, self.b.func);
        Ok(self.b.ins().symbol_value(types::I64, gv))
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
pub const RUNTIME_C: &str = r#"#include <stdio.h>
#include <stdlib.h>
static int write_f64(FILE *out, double x) {
    /* Shortest text that reads back as the same double, like Rust's `{}` for f64. */
    char buf[32];
    for (int p = 1; p <= 17; p++) {
        snprintf(buf, sizeof buf, "%.*g", p, x);
        if (strtod(buf, NULL) == x) break;
    }
    return fputs(buf, out);
}
int om_rt_write_i64(long long n) { return printf("%lld", n); }
int om_rt_write_str(const char *s) { return fputs(s, stdout); }
int om_rt_write_f64(double x) { return write_f64(stdout, x); }
int om_rt_ewrite_i64(long long n) { return fprintf(stderr, "%lld", n); }
int om_rt_ewrite_str(const char *s) { return fputs(s, stderr); }
int om_rt_ewrite_f64(double x) { return write_f64(stderr, x); }
int om_rt_exit_panic(void) { fflush(stdout); exit(101); }
"#;
