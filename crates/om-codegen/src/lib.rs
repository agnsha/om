//! Type checking and native code generation for Om using Cranelift.
//!
//! The checker runs while lowering: each expression is type-checked as it is
//! translated to Cranelift IR. A later phase moves checking into its own crate
//! (`om-check`) and adds an Om IR between the AST and Cranelift.
//!
//! ## Value representation
//!
//! Every Om value is *flattened* into a list of scalar Cranelift values:
//! an `i64` is one `I64`, a struct is its fields' scalars in order, and an enum
//! is an `I64` tag followed by the scalars of every variant's payload. Values
//! live in SSA registers, never on the heap, so structs and enums cost nothing
//! beyond their fields and Cranelift can keep them in registers.
//!
//! `&mut T` parameters use *mutable value semantics* (as in Swift `inout` and
//! Hylo): the callee receives the value, may change it, and hands the new value
//! back as an extra return value that the caller writes to the original place.
//! There are no pointers, so there is nothing for a lifetime to describe.
//!
//! Om functions are emitted as `om_<name>` (methods as `om_Type__name`). The Om
//! `main` becomes the C `main` symbol, and output goes through a small C runtime
//! ([`RUNTIME_C`]) because Cranelift cannot call variadic functions portably.

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

/// Om types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    I64,
    F64,
    Bool,
    Str,
    Unit,
    /// The type of an expression that never finishes, such as `return` or an endless `loop`.
    Never,
    Struct(String),
    Enum(String),
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Ty::I64 => "i64",
            Ty::F64 => "f64",
            Ty::Bool => "bool",
            Ty::Str => "str",
            Ty::Unit => "()",
            Ty::Never => "!",
            Ty::Struct(n) | Ty::Enum(n) => n,
        })
    }
}

/// Flattened scalar values of one Om value.
type Vals = Vec<Value>;

struct FnParam {
    name: String,
    ty: Ty,
    mode: ParamMode,
    /// Constant default value, if the parameter has one.
    default: Option<Expr>,
}

struct FnInfo {
    id: FuncId,
    params: Vec<FnParam>,
    ret: Ty,
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

fn symbol(name: &str) -> String {
    if name == "main" {
        "main".into()
    } else {
        format!("om_{}", name.replace("::", "__"))
    }
}

/// Type-check `program` and compile it to a native object file for the host machine.
pub fn compile_to_object(program: &Program) -> CResult<Vec<u8>> {
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
        fns: HashMap::new(),
        strings: HashMap::new(),
        structs: HashMap::new(),
        enums: HashMap::new(),
        consts: HashMap::new(),
    };

    cg.declare_types(program)?;
    for c in &program.consts {
        let ty = cg.resolve(&c.ty, None)?;
        match const_type(&c.value) {
            Some(t) if t == ty => {}
            Some(t) => return err(format!("constant `{}` is declared as {ty} but its value is {t}", c.name), c.span),
            None => return err("a `const` value must be a literal (number, bool or string)", c.value.span),
        }
        if cg.consts.insert(c.name.clone(), (ty, c.value.clone())).is_some() {
            return err(format!("constant `{}` is defined twice", c.name), c.span);
        }
    }

    // Declare every function and method so calls can refer to ones defined later.
    let mut all: Vec<(String, &Function, Option<String>)> =
        program.functions.iter().map(|f| (f.name.clone(), f, None)).collect();
    for imp in &program.impls {
        let ty_name = imp.ty.name.clone();
        if !cg.structs.contains_key(&ty_name) && !cg.enums.contains_key(&ty_name) {
            return err(format!("cannot find struct or enum `{ty_name}` for this `impl`"), imp.ty.span);
        }
        for m in &imp.methods {
            all.push((format!("{ty_name}::{}", m.name), m, Some(ty_name.clone())));
        }
    }
    for (name, f, self_ty) in &all {
        cg.declare_fn(name, f, self_ty.as_deref())?;
    }
    let Some(main) = program.functions.iter().find(|f| f.name == "main") else {
        return err("program has no `fn main()`", Span::default());
    };
    if !main.params.is_empty() {
        return err("`main` takes no parameters", main.span);
    }

    let mut ctx = cg.module.make_context();
    let mut fctx = FunctionBuilderContext::new();
    for (name, f, self_ty) in &all {
        cg.define(name, f, self_ty.clone(), &mut ctx, &mut fctx)?;
    }
    cg.module.finish().emit().map_err(|e| internal(format!("writing object file: {e}")))
}

struct Codegen {
    module: ObjectModule,
    fns: HashMap<String, FnInfo>,
    strings: HashMap<String, DataId>,
    structs: HashMap<String, Vec<(String, Ty)>>,
    enums: HashMap<String, Vec<(String, Vec<Ty>)>>,
    consts: HashMap<String, (Ty, Expr)>,
}

const BUILTINS: &[&str] = &["print"];

impl Codegen {
    fn resolve(&self, t: &TypeExpr, self_ty: Option<&str>) -> CResult<Ty> {
        Ok(match t.name.as_str() {
            "i64" => Ty::I64,
            "f64" => Ty::F64,
            "bool" => Ty::Bool,
            "str" => Ty::Str,
            "Self" => match self_ty {
                Some(n) => return self.resolve(&TypeExpr { name: n.into(), span: t.span }, None),
                None => return err("`Self` is only available inside an `impl` block", t.span),
            },
            n if self.structs.contains_key(n) => Ty::Struct(n.into()),
            n if self.enums.contains_key(n) => Ty::Enum(n.into()),
            n => return err(format!("cannot find type `{n}`"), t.span),
        })
    }

    fn declare_types(&mut self, p: &Program) -> CResult<()> {
        // Register names first so types can refer to each other in any order.
        for s in &p.structs {
            if self.structs.insert(s.name.clone(), vec![]).is_some() || self.enums.contains_key(&s.name) {
                return err(format!("type `{}` is defined twice", s.name), s.span);
            }
        }
        for e in &p.enums {
            if self.enums.insert(e.name.clone(), vec![]).is_some() || self.structs.contains_key(&e.name) {
                return err(format!("type `{}` is defined twice", e.name), e.span);
            }
        }
        for s in &p.structs {
            let mut fields: Vec<(String, Ty)> = Vec::new();
            for (f, t) in &s.fields {
                if fields.iter().any(|(n, _)| n == f) {
                    return err(format!("field `{f}` is declared twice"), t.span);
                }
                fields.push((f.clone(), self.resolve(t, None)?));
            }
            self.structs.insert(s.name.clone(), fields);
        }
        for e in &p.enums {
            let mut variants: Vec<(String, Vec<Ty>)> = Vec::new();
            for v in &e.variants {
                if variants.iter().any(|(n, _)| n == &v.name) {
                    return err(format!("variant `{}` is declared twice", v.name), v.span);
                }
                let tys = v.fields.iter().map(|t| self.resolve(t, None)).collect::<CResult<Vec<_>>>()?;
                variants.push((v.name.clone(), tys));
            }
            if variants.is_empty() {
                return err(format!("enum `{}` needs at least one variant", e.name), e.span);
            }
            self.enums.insert(e.name.clone(), variants);
        }
        // Values are stored inline, so a type may not contain itself.
        for s in &p.structs {
            self.check_not_recursive(&Ty::Struct(s.name.clone()), &mut vec![], s.span)?;
        }
        for e in &p.enums {
            self.check_not_recursive(&Ty::Enum(e.name.clone()), &mut vec![], e.span)?;
        }
        Ok(())
    }

    fn check_not_recursive(&self, ty: &Ty, stack: &mut Vec<String>, span: Span) -> CResult<()> {
        let (name, children): (&String, Vec<Ty>) = match ty {
            Ty::Struct(n) => (n, self.structs[n].iter().map(|(_, t)| t.clone()).collect()),
            Ty::Enum(n) => (n, self.enums[n].iter().flat_map(|(_, ts)| ts.clone()).collect()),
            _ => return Ok(()),
        };
        if stack.contains(name) {
            return err(
                format!("type `{name}` contains itself, which would make it infinitely large (heap boxes arrive with the standard library)"),
                span,
            );
        }
        stack.push(name.clone());
        for c in &children {
            self.check_not_recursive(c, stack, span)?;
        }
        stack.pop();
        Ok(())
    }

    /// The scalar Cranelift types an Om value is flattened into.
    fn layout(&self, ty: &Ty) -> Vec<types::Type> {
        match ty {
            Ty::I64 | Ty::Str => vec![types::I64],
            Ty::F64 => vec![types::F64],
            Ty::Bool => vec![types::I8],
            Ty::Unit | Ty::Never => vec![],
            Ty::Struct(n) => self.structs[n].iter().flat_map(|(_, t)| self.layout(t)).collect(),
            Ty::Enum(n) => {
                let mut l = vec![types::I64];
                for (_, ts) in &self.enums[n] {
                    for t in ts {
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

    /// Index of field `f` in struct `s`, its type, and its first scalar slot.
    fn field(&self, s: &str, f: &str) -> Option<(usize, Ty, usize)> {
        let fields = &self.structs[s];
        let idx = fields.iter().position(|(n, _)| n == f)?;
        let offset = fields[..idx].iter().map(|(_, t)| self.width(t)).sum();
        Some((idx, fields[idx].1.clone(), offset))
    }

    /// First scalar slot of variant `v`'s payload inside enum `e` (slot 0 is the tag).
    fn variant_offset(&self, e: &str, v: usize) -> usize {
        1 + self.enums[e][..v].iter().flat_map(|(_, ts)| ts).map(|t| self.width(t)).sum::<usize>()
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

    fn declare_fn(&mut self, name: &str, f: &Function, self_ty: Option<&str>) -> CResult<()> {
        if self.fns.contains_key(name) {
            return err(format!("function `{name}` is defined twice"), f.span);
        }
        if BUILTINS.contains(&name) {
            return err(format!("`{name}` is a built-in function and cannot be redefined"), f.span);
        }
        let mut params = Vec::new();
        let mut seen_default = false;
        for (i, p) in f.params.iter().enumerate() {
            let ty = match (&p.ty, self_ty) {
                (Some(t), _) => self.resolve(t, self_ty)?,
                (None, Some(s)) if i == 0 => self.resolve(&TypeExpr { name: s.into(), span: p.span }, None)?,
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
            params.push(FnParam { name: p.name.clone(), ty, mode: p.mode, default: p.default.clone() });
        }
        let ret = match &f.ret {
            Some(t) => self.resolve(t, self_ty)?,
            None => Ty::Unit,
        };
        let is_main = name == "main";
        if is_main && !matches!(ret, Ty::Unit | Ty::I64) {
            return err("`main` must return nothing or an i64 exit code", f.span);
        }
        let sig = self.signature(&params, &ret, is_main);
        let id = self
            .module
            .declare_function(&symbol(name), Linkage::Export, &sig)
            .map_err(|e| CompileError { message: e.to_string(), span: f.span })?;
        self.fns.insert(name.to_string(), FnInfo { id, params, ret });
        Ok(())
    }

    fn define(
        &mut self,
        name: &str,
        f: &Function,
        self_ty: Option<String>,
        ctx: &mut Context,
        fctx: &mut FunctionBuilderContext,
    ) -> CResult<()> {
        let info = &self.fns[name];
        let id = info.id;
        let ret = info.ret.clone();
        let params: Vec<(String, Ty, ParamMode)> =
            info.params.iter().map(|p| (p.name.clone(), p.ty.clone(), p.mode)).collect();
        let is_main = name == "main";
        let sig = self.signature(&info.params, &ret, is_main);
        ctx.func.signature = sig;

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
            self_ty,
            inouts,
            tmp_count: 0,
        };
        let mut next = 0;
        for ((pname, pty, mode), p) in params.iter().zip(&f.params) {
            let n = lw.cg.width(pty);
            let vals = entry_vals[next..next + n].to_vec();
            next += n;
            let mutable = p.mutable || *mode == ParamMode::RefMut;
            lw.bind_vals(pname, pty.clone(), vals, mutable, p.span)?;
        }

        let (vals, ty) = lw.block(&f.body)?;
        if !lw.dead {
            if ty != ret && ty != Ty::Never {
                let span = f.body.tail.as_ref().map_or(f.body.span, |t| t.span);
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
            .map_err(|e| CompileError { message: format!("codegen for `{name}`: {e:?}"), span: f.span })?;
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

    fn runtime_fn(&mut self, name: &str, param: types::Type) -> CResult<FuncId> {
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(param));
        sig.returns.push(AbiParam::new(types::I32));
        self.module.declare_function(name, Linkage::Import, &sig).map_err(internal)
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
    self_ty: Option<String>,
    /// Names of `&mut` parameters, whose final values are returned to the caller.
    inouts: Vec<String>,
    tmp_count: usize,
}

impl<'a> Lower<'a> {
    // ---------- variables and places ----------

    fn bind_vals(&mut self, name: &str, ty: Ty, vals: Vals, mutable: bool, span: Span) -> CResult<()> {
        if self.cg.fns.contains_key(name) || BUILTINS.contains(&name) {
            return err(format!("`{name}` is already the name of a function"), span);
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

    /// Resolve `x`, `x.field` or `x.a.b` to the variables that hold it, if `e` is such a path.
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
                let Ty::Struct(s) = &p.ty else {
                    return err(format!("{} has no fields", p.ty), inner.span);
                };
                let Some((_, fty, off)) = self.cg.field(s, f) else {
                    return err(format!("struct `{s}` has no field `{f}`"), e.span);
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

    fn block(&mut self, blk: &Block) -> CResult<(Vals, Ty)> {
        self.scopes.push(HashMap::new());
        self.defers.push(Vec::new());
        let mut result = self.block_inner(blk);
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

    fn block_inner(&mut self, blk: &Block) -> CResult<(Vals, Ty)> {
        for s in &blk.stmts {
            self.stmt(s)?;
        }
        if let Some(tail) = &blk.tail {
            let r = self.expr(tail)?;
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
                result = err("`return`, `break` and `continue` are not allowed inside `defer`", e.span);
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
            Stmt::Let { name, mutable, ty, value, span } => {
                let (vals, vty) = self.expr(value)?;
                if let Some(t) = ty {
                    let want = self.cg.resolve(t, self.self_ty.as_deref())?;
                    if want != vty && vty != Ty::Never {
                        return err(format!("`{name}` is declared as {want} but its value is {vty}"), value.span);
                    }
                }
                if vty == Ty::Never {
                    return Ok(());
                }
                self.bind_vals(name, vty, vals, *mutable, *span)
            }
            Stmt::Assign { target, op, value, span } => self.assign(target, *op, value, *span),
            Stmt::While { cond, body, .. } => {
                let header = self.b.create_block();
                let body_blk = self.b.create_block();
                let exit = self.new_merge();
                self.b.ins().jump(header, &[]);
                self.b.switch_to_block(header);
                let c = self.cond(cond)?;
                self.b.ins().brif(c, body_blk, &[], exit.blk, &[]);
                // The normal exit carries no value.
                let exit = Merge { blk: exit.blk, ty: Some(Ty::Unit) };
                self.loop_body(body_blk, header, exit, false, body)?;
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
                // `exit` is now current; emit the increment in `step` and come back.
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
                let (vals, ty) = match value {
                    Some(e) => self.expr(e)?,
                    None => (vec![], Ty::Unit),
                };
                if ty != self.ret && ty != Ty::Never {
                    return err(format!("this function returns {}, but `return` gives {ty}", self.ret), *span);
                }
                if !self.dead {
                    self.run_defers_above(0)?;
                    self.emit_return(vals);
                }
                self.start_dead_block();
                Ok(())
            }
            Stmt::Break { value, span } => {
                let Some(ctx) = self.loops.last() else {
                    return err("`break` outside of a loop", *span);
                };
                let (depth, is_loop_expr) = (ctx.depth, ctx.is_loop_expr);
                let (vals, ty) = match value {
                    Some(_) if !is_loop_expr => {
                        return err("only `loop` can break with a value; `while` and `for` cannot", *span)
                    }
                    Some(e) => self.expr(e)?,
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
    /// Leaves the builder positioned in the exit block, which is sealed. Returns the exit merge.
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
        let r = self.block(body);
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
            None => self.expr(value)?,
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
                if let Some(r) = self.variant(name, &[], e.span)? {
                    return Ok(r);
                }
                if self.cg.fns.contains_key(name) {
                    return err(format!("`{name}` is a function; call it with `{name}(...)`"), e.span);
                }
                err(format!("cannot find variable `{name}`"), e.span)
            }
            ExprKind::Unary(op, inner) => {
                let (v, ty) = self.expr(inner)?;
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
                let (rv, rt) = self.expr(r)?;
                self.binop(*op, lv, lt, rv, rt, e.span)
            }
            ExprKind::Call(name, args) => self.call(name, args, e.span),
            ExprKind::MethodCall(recv, m, args) => self.method_call(recv, m, args, e.span),
            ExprKind::Field(inner, f) => {
                if let Some(p) = self.place(e)? {
                    return Ok((self.read(&p.vars), p.ty));
                }
                let (vals, ty) = self.expr(inner)?;
                let Ty::Struct(s) = &ty else {
                    return err(format!("{ty} has no field `{f}`"), e.span);
                };
                let Some((_, fty, off)) = self.cg.field(s, f) else {
                    return err(format!("struct `{s}` has no field `{f}`"), e.span);
                };
                let w = self.cg.width(&fty);
                Ok((vals[off..off + w].to_vec(), fty))
            }
            ExprKind::StructLit(name, fields) => self.struct_lit(name, fields, e.span),
            ExprKind::Ref(..) => err(
                "`&` is only used when passing an argument to a `&` or `&mut` parameter (Om has no reference values)",
                e.span,
            ),
            ExprKind::Cast(inner, t) => {
                let to = self.cg.resolve(t, self.self_ty.as_deref())?;
                let (v, from) = self.expr(inner)?;
                self.cast(v, from, to, e.span)
            }
            ExprKind::Interp(_) => err(
                "string interpolation can only be used inside `print` for now (runtime strings arrive with the standard library)",
                e.span,
            ),
            ExprKind::If(cond, then, els) => self.if_expr(cond, then, els.as_ref(), e.span),
            ExprKind::Match(scrut, arms) => self.match_expr(scrut, arms, e.span),
            ExprKind::Loop(body) => {
                let header = self.b.create_block();
                let body_blk = self.b.create_block();
                let exit = self.new_merge();
                self.b.ins().jump(header, &[]);
                self.b.switch_to_block(header);
                self.b.ins().jump(body_blk, &[]);
                let was_dead = self.dead;
                let exit = self.loop_body(body_blk, header, exit, true, body);
                self.b.seal_block(header);
                let exit = exit?;
                // `loop_body` sealed and switched to the exit block.
                let ty = match &exit.ty {
                    None => return Ok((vec![], Ty::Never)),
                    Some(t) => t.clone(),
                };
                self.dead = was_dead;
                Ok((self.b.block_params(exit.blk).to_vec(), ty))
            }
            ExprKind::Block(b) => self.block(b),
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
        let out = match (&from, &to) {
            (Ty::Never, _) => return Ok((vec![], Ty::Never)),
            (a, b) if a == b => return Ok((v, to)),
            (Ty::I64, Ty::F64) => self.b.ins().fcvt_from_sint(types::F64, v[0]),
            // Like Rust, float-to-int casts saturate and map NaN to 0.
            (Ty::F64, Ty::I64) => self.b.ins().fcvt_to_sint_sat(types::I64, v[0]),
            (Ty::Bool, Ty::I64) => self.b.ins().uextend(types::I64, v[0]),
            (Ty::Enum(n), Ty::I64) if self.cg.enums[n].iter().all(|(_, f)| f.is_empty()) => v[0],
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

    fn if_expr(&mut self, cond: &Expr, then: &Block, els: Option<&Block>, span: Span) -> CResult<(Vals, Ty)> {
        let c = self.cond(cond)?;
        let then_blk = self.b.create_block();
        let else_blk = self.b.create_block();
        let mut m = self.new_merge();
        self.b.ins().brif(c, then_blk, &[], else_blk, &[]);
        let was_dead = self.dead;

        self.b.switch_to_block(then_blk);
        self.b.seal_block(then_blk);
        self.dead = false;
        let (tv, tt) = self.block(then)?;
        if els.is_none() && tt != Ty::Unit && tt != Ty::Never {
            return err(format!("`if` without `else` must not produce a value, but this one gives {tt}"), then.span);
        }
        if !self.dead {
            self.jump_to(&mut m, tt, tv, "`if` and `else`", span)?;
        }

        self.b.switch_to_block(else_blk);
        self.b.seal_block(else_blk);
        self.dead = false;
        let (ev, et) = match els {
            Some(b) => self.block(b)?,
            None => (vec![], Ty::Unit),
        };
        if !self.dead {
            self.jump_to(&mut m, et, ev, "`if` and `else`", span)?;
        }
        Ok(self.finish_merge(m, was_dead))
    }

    // ---------- structs and enums ----------

    fn self_name(&self, name: &str) -> String {
        match (name.strip_prefix("Self"), &self.self_ty) {
            (Some(rest), Some(t)) if rest.is_empty() || rest.starts_with("::") => format!("{t}{rest}"),
            _ => name.to_string(),
        }
    }

    fn struct_lit(&mut self, name: &str, fields: &[(String, Expr)], span: Span) -> CResult<(Vals, Ty)> {
        let name = self.self_name(name);
        let Some(defs) = self.cg.structs.get(&name).cloned() else {
            return err(format!("cannot find struct `{name}`"), span);
        };
        let mut given: Vec<Option<Vals>> = vec![None; defs.len()];
        for (f, value) in fields {
            let Some(i) = defs.iter().position(|(n, _)| n == f) else {
                return err(format!("struct `{name}` has no field `{f}`"), value.span);
            };
            if given[i].is_some() {
                return err(format!("field `{f}` is given twice"), value.span);
            }
            let (v, t) = self.expr(value)?;
            if t != defs[i].1 && t != Ty::Never {
                return err(format!("field `{f}` is {}, found {t}", defs[i].1), value.span);
            }
            given[i] = Some(v);
        }
        let missing: Vec<&str> =
            defs.iter().zip(&given).filter(|(_, g)| g.is_none()).map(|((n, _), _)| n.as_str()).collect();
        if !missing.is_empty() {
            return err(format!("missing field(s) in `{name}`: {}", missing.join(", ")), span);
        }
        Ok((given.into_iter().flatten().flatten().collect(), Ty::Struct(name)))
    }

    /// Find enum `E` and variant index for a path like `E::V` (or `Self::V`).
    fn find_variant(&self, path: &str) -> Option<(String, usize)> {
        let path = self.self_name(path);
        let (e, v) = path.rsplit_once("::")?;
        let variants = self.cg.enums.get(e)?;
        let idx = variants.iter().position(|(n, _)| n == v)?;
        Some((e.to_string(), idx))
    }

    /// Build an enum value if `path` names a variant; `None` if it doesn't.
    fn variant(&mut self, path: &str, args: &[Arg], span: Span) -> CResult<Option<(Vals, Ty)>> {
        let Some((e, idx)) = self.find_variant(path) else { return Ok(None) };
        let (vname, ftys) = self.cg.enums[&e][idx].clone();
        if args.len() != ftys.len() {
            return err(format!("`{e}::{vname}` takes {} value(s) but {} were given", ftys.len(), args.len()), span);
        }
        let mut payload = Vec::new();
        for (a, want) in args.iter().zip(&ftys) {
            if a.name.is_some() {
                return err("enum variants do not take named arguments", a.span);
            }
            let (v, t) = self.expr(&a.value)?;
            if t != *want {
                return err(format!("expected {want}, found {t}"), a.value.span);
            }
            payload.push(v);
        }
        let mut vals = vec![self.b.ins().iconst(types::I64, idx as i64)];
        let variants = self.cg.enums[&e].clone();
        let mut payload = payload.into_iter();
        for (i, (_, tys)) in variants.iter().enumerate() {
            for t in tys {
                if i == idx {
                    vals.extend(payload.next().unwrap());
                } else {
                    for st in self.cg.layout(t) {
                        let z = self.zero(st);
                        vals.push(z);
                    }
                }
            }
        }
        Ok(Some((vals, Ty::Enum(e))))
    }

    // ---------- calls ----------

    fn call(&mut self, name: &str, args: &[Arg], span: Span) -> CResult<(Vals, Ty)> {
        if name == "print" {
            return self.print(args, span);
        }
        let name = self.self_name(name);
        if let Some(r) = self.variant(&name, args, span)? {
            return Ok(r);
        }
        if !self.cg.fns.contains_key(&name) {
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
        self.call_fn(&name, args, span)
    }

    fn method_call(&mut self, recv: &Expr, m: &str, args: &[Arg], span: Span) -> CResult<(Vals, Ty)> {
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
        let type_name = match &ty {
            Ty::Struct(n) | Ty::Enum(n) => n.clone(),
            _ => return err(format!("{ty} has no methods yet (methods on built-in types come with traits)"), span),
        };
        let full = format!("{type_name}::{m}");
        let Some(info) = self.cg.fns.get(&full) else {
            return err(format!("`{type_name}` has no method named `{m}`"), span);
        };
        let self_mode = match info.params.first() {
            Some(p) if p.name == "self" => p.mode,
            _ => return err(format!("`{full}` has no `self` parameter; call it as `{full}(...)`"), span),
        };
        let self_arg = match self_mode {
            ParamMode::Value => recv_expr,
            ParamMode::Ref => Expr { kind: ExprKind::Ref(false, Box::new(recv_expr)), span: recv.span },
            ParamMode::RefMut => Expr { kind: ExprKind::Ref(true, Box::new(recv_expr)), span: recv.span },
        };
        let mut all = vec![Arg { name: None, value: self_arg, span: recv.span }];
        all.extend(args.iter().cloned());
        self.call_fn(&full, &all, span)
    }

    fn call_fn(&mut self, name: &str, args: &[Arg], span: Span) -> CResult<(Vals, Ty)> {
        let info = &self.cg.fns[name];
        let id = info.id;
        let ret = info.ret.clone();
        let params: Vec<(String, Ty, ParamMode, Option<Expr>)> =
            info.params.iter().map(|p| (p.name.clone(), p.ty.clone(), p.mode, p.default.clone())).collect();

        // Match arguments to parameter slots: positional first, then by name.
        let mut filled = vec![false; params.len()];
        let mut arg_slot = Vec::with_capacity(args.len());
        let mut seen_named = false;
        for (i, a) in args.iter().enumerate() {
            let slot = match &a.name {
                None if seen_named => return err("positional arguments must come before named ones", a.span),
                None if i >= params.len() => {
                    return err(
                        format!("`{name}` takes {} argument(s) but {} were given", params.len(), args.len()),
                        span,
                    )
                }
                None => i,
                Some(n) => {
                    seen_named = true;
                    match params.iter().position(|p| &p.0 == n) {
                        Some(k) => k,
                        None => return err(format!("`{name}` has no parameter named `{n}`"), a.span),
                    }
                }
            };
            if filled[slot] {
                return err(format!("argument `{}` is given twice", params[slot].0), a.span);
            }
            filled[slot] = true;
            arg_slot.push(slot);
        }

        // Evaluate the written arguments in source order, then fill in defaults.
        let mut vals: Vec<Option<Vals>> = vec![None; params.len()];
        let mut writebacks: Vec<(usize, Vec<Variable>)> = Vec::new();
        for (a, &k) in args.iter().zip(&arg_slot) {
            let (pname, pty, mode, _) = &params[k];
            let (v, t) = match (&a.value.kind, mode) {
                (ExprKind::Ref(true, inner), ParamMode::RefMut) => {
                    let Some(place) = self.place(inner)? else {
                        return err("`&mut` needs a variable or field to change", inner.span);
                    };
                    if !place.mutable {
                        let r = &place.root;
                        return err(format!("cannot pass `{r}` as `&mut`; declare it with `let mut {r}`"), inner.span);
                    }
                    if writebacks.iter().any(|(_, vs)| vs.iter().any(|v| place.vars.contains(v))) {
                        return err(format!("cannot pass `{}` as `&mut` twice in one call", place.root), inner.span);
                    }
                    writebacks.push((k, place.vars.clone()));
                    (self.read(&place.vars), place.ty)
                }
                (ExprKind::Ref(false, inner), ParamMode::Ref) => self.expr(inner)?,
                (_, ParamMode::RefMut) => {
                    return err(format!("argument `{pname}` is `&mut {pty}`; pass it as `&mut ...`"), a.value.span)
                }
                (_, ParamMode::Ref) => {
                    return err(format!("argument `{pname}` is `&{pty}`; pass it as `&...`"), a.value.span)
                }
                (ExprKind::Ref(..), ParamMode::Value) => {
                    return err(format!("argument `{pname}` takes a {pty} value, not a reference"), a.value.span)
                }
                _ => self.expr(&a.value)?,
            };
            if t != *pty && t != Ty::Never {
                return err(format!("argument `{pname}` should be {pty}, found {t}"), a.value.span);
            }
            vals[k] = Some(v);
        }
        for k in 0..params.len() {
            if vals[k].is_none() {
                let Some(d) = params[k].3.clone() else {
                    return err(format!("missing argument `{}` in call to `{name}`", params[k].0), span);
                };
                vals[k] = Some(self.expr(&d)?.0);
            }
        }
        if self.dead {
            return Ok((vec![], Ty::Never));
        }
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

    // ---------- match ----------

    fn match_expr(&mut self, scrut: &Expr, arms: &[Arm], span: Span) -> CResult<(Vals, Ty)> {
        let (sv, sty) = self.expr(scrut)?;
        if sty == Ty::Never {
            return Ok((vec![], Ty::Never));
        }
        for arm in arms {
            self.check_pattern(&arm.pat, &sty)?;
        }
        self.check_exhaustive(arms, &sty, span)?;
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
                let (v, t) = self.expr(&arm.body)?;
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
            PatKind::Wild | PatKind::Bind(_) => Ok(()),
            PatKind::Int(_) | PatKind::Range(..) if *ty == Ty::I64 => Ok(()),
            PatKind::Int(_) | PatKind::Range(..) => mismatch("an integer"),
            PatKind::Bool(_) if *ty == Ty::Bool => Ok(()),
            PatKind::Bool(_) => mismatch("a bool"),
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
                let Ty::Enum(e) = ty else { return mismatch(&format!("`{path}`")) };
                let (_, idx) = self.pattern_variant(path, e, pat.span)?;
                let ftys = &self.cg.enums[e][idx].1;
                if subs.len() != ftys.len() {
                    return err(
                        format!("`{path}` has {} field(s), but the pattern has {}", ftys.len(), subs.len()),
                        pat.span,
                    );
                }
                for (s, t) in subs.iter().zip(ftys) {
                    self.check_pattern(s, t)?;
                }
                Ok(())
            }
        }
    }

    /// Resolve a variant pattern path within enum `e`. `Red`, `Color::Red` and `Self::Red` all work.
    fn pattern_variant(&self, path: &str, e: &str, span: Span) -> CResult<(String, usize)> {
        let path = self.self_name(path);
        let (prefix, v) = match path.rsplit_once("::") {
            Some((p, v)) => (Some(p), v),
            None => (None, path.as_str()),
        };
        if let Some(p) = prefix {
            if p != e {
                return err(format!("expected a variant of `{e}`, found `{path}`"), span);
            }
        }
        match self.cg.enums[e].iter().position(|(n, _)| n == v) {
            Some(i) => Ok((e.to_string(), i)),
            None => err(format!("enum `{e}` has no variant `{v}`"), span),
        }
    }

    fn check_exhaustive(&self, arms: &[Arm], ty: &Ty, span: Span) -> CResult<()> {
        let rows: Vec<Vec<Pattern>> = arms.iter().filter(|a| a.guard.is_none()).map(|a| vec![a.pat.clone()]).collect();
        match self.missing(rows, std::slice::from_ref(ty)) {
            None => Ok(()),
            Some(w) => err(
                format!("`match` does not cover every case; missing: {} (add an arm for it or a `_` arm)", w[0]),
                span,
            ),
        }
    }

    /// Pattern-matrix exhaustiveness check (Maranget's algorithm, simplified).
    /// Returns `None` if `rows` cover every value of `tys`, or one example of an
    /// uncovered value per column.
    fn missing(&self, rows: Vec<Vec<Pattern>>, tys: &[Ty]) -> Option<Vec<String>> {
        let Some((ty, rest_tys)) = tys.split_first() else {
            return if rows.is_empty() { Some(vec![]) } else { None };
        };
        // Expand `A | B` in the first column into separate rows.
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
        // Constructors: (label, field types, does this pattern match it, its sub-patterns).
        let ctors: Vec<(String, Vec<Ty>)> = match ty {
            Ty::Bool => vec![("true".into(), vec![]), ("false".into(), vec![])],
            Ty::Enum(e) => self.cg.enums[e].iter().map(|(n, f)| (format!("{e}::{n}"), f.clone())).collect(),
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
                    PatKind::Wild | PatKind::Bind(_) => Some(vec![wild.clone(); ftys.len()]),
                    PatKind::Bool(b) => (b.to_string() == *label).then(Vec::new),
                    PatKind::Variant(path, subs) => match ty {
                        Ty::Enum(e) => {
                            let idx = self.pattern_variant(path, e, row[0].span).ok()?.1;
                            (format!("{e}::{}", self.cg.enums[e][idx].0) == *label).then(|| subs.clone())
                        }
                        _ => None,
                    },
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
                let shown = if fields.is_empty() { label.clone() } else { format!("{label}({})", fields.join(", ")) };
                return Some([vec![shown], rest.to_vec()].concat());
            }
        }
        None
    }

    fn pat_test(&mut self, pat: &Pattern, ty: &Ty, vals: &[Value]) -> CResult<Value> {
        Ok(match &pat.kind {
            PatKind::Wild | PatKind::Bind(_) => self.b.ins().iconst(types::I8, 1),
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
            PatKind::Variant(path, subs) => {
                let Ty::Enum(e) = ty else { unreachable!("checked by check_pattern") };
                let (_, idx) = self.pattern_variant(path, e, pat.span)?;
                let mut acc = self.b.ins().icmp_imm_s(IntCC::Equal, vals[0], idx as i64);
                let ftys = self.cg.enums[e][idx].1.clone();
                let mut off = self.cg.variant_offset(e, idx);
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
            PatKind::Bind(name) => self.bind_vals(name, ty.clone(), vals.to_vec(), false, pat.span),
            PatKind::Variant(path, subs) => {
                let Ty::Enum(e) = ty else { return Ok(()) };
                let (_, idx) = self.pattern_variant(path, e, pat.span)?;
                let ftys = self.cg.enums[e][idx].1.clone();
                let mut off = self.cg.variant_offset(e, idx);
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

    // ---------- printing ----------

    /// `print(x)` writes any value, or an interpolated string, followed by a newline.
    fn print(&mut self, args: &[Arg], span: Span) -> CResult<(Vals, Ty)> {
        let [arg] = args else {
            return err("`print` takes exactly one argument", span);
        };
        if arg.name.is_some() {
            return err("`print` does not take named arguments", arg.span);
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
            }
            _ => {
                let (v, t) = self.expr(&arg.value)?;
                self.write_value(&v, &t, false)?;
            }
        }
        self.write_str("\n")?;
        Ok((vec![], Ty::Unit))
    }

    fn write_str(&mut self, s: &str) -> CResult<()> {
        let p = self.str_ptr(s)?;
        self.rt_call("om_rt_write_str", types::I64, p)
    }

    /// Write one value to stdout with no newline. Structs and enums print in the
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
            Ty::Struct(n) => {
                let fields = self.cg.structs[n].clone();
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
            Ty::Enum(n) => {
                // Branch on the tag and print the active variant.
                let variants = self.cg.enums[n].clone();
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
                        let mut off = self.cg.variant_offset(n, i);
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

    /// Call a one-argument function from the C runtime.
    fn rt_call(&mut self, name: &str, ty: types::Type, arg: Value) -> CResult<()> {
        let id = self.cg.runtime_fn(name, ty)?;
        let f = self.cg.module.declare_func_in_func(id, self.b.func);
        self.b.ins().call(f, &[arg]);
        Ok(())
    }
}

fn irrefutable(p: &Pattern) -> bool {
    match &p.kind {
        PatKind::Wild | PatKind::Bind(_) => true,
        PatKind::Or(alts) => alts.iter().any(irrefutable),
        _ => false,
    }
}

fn has_bindings(p: &Pattern) -> bool {
    match &p.kind {
        PatKind::Bind(_) => true,
        PatKind::Variant(_, subs) => subs.iter().any(has_bindings),
        PatKind::Or(alts) => alts.iter().any(has_bindings),
        _ => false,
    }
}

/// The C runtime linked into every Om program. Kept tiny: the standard library replaces it later.
pub const RUNTIME_C: &str = r#"#include <stdio.h>
#include <stdlib.h>
int om_rt_write_i64(long long n) { return printf("%lld", n); }
int om_rt_write_str(const char *s) { return fputs(s, stdout); }
/* Shortest text that reads back as the same double, like Rust's `{}` for f64. */
int om_rt_write_f64(double x) {
    char buf[32];
    for (int p = 1; p <= 17; p++) {
        snprintf(buf, sizeof buf, "%.*g", p, x);
        if (strtod(buf, NULL) == x) break;
    }
    return fputs(buf, stdout);
}
"#;
