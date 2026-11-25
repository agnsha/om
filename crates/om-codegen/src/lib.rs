//! Native code generation for Om using Cranelift.
//!
//! Phase 0 does type checking during lowering: each expression is checked as
//! it is translated to Cranelift IR. Later phases move checking into its own
//! crate (`om-check`) and add an Om IR between the AST and Cranelift.
//!
//! Om functions are emitted as `om_<name>`. The Om `main` becomes the C
//! `main` symbol, so the object file links with the system C compiler and
//! uses libc's `printf` and `puts` for output.

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{types, AbiParam, Block as ClBlock, InstBuilder, Signature, Value};
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

/// Om's phase 0 types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    I64,
    Bool,
    Str,
    Unit,
    /// The type of an expression that never finishes, such as a block ending in `return`.
    Never,
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Ty::I64 => "i64",
            Ty::Bool => "bool",
            Ty::Str => "str",
            Ty::Unit => "()",
            Ty::Never => "!",
        })
    }
}

impl Ty {
    fn cl(self) -> Option<types::Type> {
        match self {
            Ty::I64 | Ty::Str => Some(types::I64),
            Ty::Bool => Some(types::I8),
            Ty::Unit | Ty::Never => None,
        }
    }

    fn from_expr(t: &TypeExpr) -> CResult<Ty> {
        match t.name.as_str() {
            "i64" => Ok(Ty::I64),
            "bool" => Ok(Ty::Bool),
            "str" => Ok(Ty::Str),
            other => err(format!("unknown type `{other}` (phase 0 supports i64, bool and str)"), t.span),
        }
    }
}

struct FnInfo {
    id: FuncId,
    params: Vec<Ty>,
    ret: Ty,
}

/// Type-check `program` and compile it to a native object file for the host machine.
pub fn compile_to_object(program: &Program) -> CResult<Vec<u8>> {
    let mut flags = settings::builder();
    flags.set("opt_level", "speed").unwrap();
    flags.set("is_pic", "true").unwrap();
    let isa_builder = cranelift_native::builder()
        .map_err(|e| CompileError { message: format!("unsupported host: {e}"), span: Span::default() })?;
    let isa = isa_builder
        .finish(settings::Flags::new(flags))
        .map_err(|e| CompileError { message: format!("cranelift: {e}"), span: Span::default() })?;
    let builder = ObjectBuilder::new(isa, "om", cranelift_module::default_libcall_names())
        .map_err(|e| CompileError { message: format!("cranelift: {e}"), span: Span::default() })?;
    let mut cg = Codegen { module: ObjectModule::new(builder), fns: HashMap::new(), strings: HashMap::new() };

    // Pass 1: declare every function so calls can refer to functions defined later.
    for f in &program.functions {
        cg.declare(f)?;
    }
    let Some(main) = program.functions.iter().find(|f| f.name == "main") else {
        return err("program has no `fn main()`", Span::default());
    };
    if !main.params.is_empty() {
        return err("`main` takes no parameters", main.span);
    }

    // Pass 2: check and lower each body.
    let mut ctx = cg.module.make_context();
    let mut fctx = FunctionBuilderContext::new();
    for f in &program.functions {
        cg.define(f, &mut ctx, &mut fctx)?;
    }
    let product = cg.module.finish();
    product.emit().map_err(|e| CompileError { message: format!("writing object file: {e}"), span: Span::default() })
}

struct Codegen {
    module: ObjectModule,
    fns: HashMap<String, FnInfo>,
    strings: HashMap<String, DataId>,
}

const BUILTINS: &[&str] = &["print"];

impl Codegen {
    fn symbol(name: &str) -> String {
        if name == "main" {
            "main".into()
        } else {
            format!("om_{name}")
        }
    }

    fn signature(&self, params: &[Ty], ret: Ty, is_main: bool) -> Signature {
        let mut sig = self.module.make_signature();
        for p in params {
            sig.params.push(AbiParam::new(p.cl().unwrap()));
        }
        if is_main {
            sig.returns.push(AbiParam::new(types::I32));
        } else if let Some(t) = ret.cl() {
            sig.returns.push(AbiParam::new(t));
        }
        sig
    }

    fn declare(&mut self, f: &Function) -> CResult<()> {
        if self.fns.contains_key(&f.name) {
            return err(format!("function `{}` is defined twice", f.name), f.span);
        }
        if BUILTINS.contains(&f.name.as_str()) {
            return err(format!("`{}` is a built-in function and cannot be redefined", f.name), f.span);
        }
        let params = f.params.iter().map(|p| Ty::from_expr(&p.ty)).collect::<CResult<Vec<_>>>()?;
        let ret = f.ret.as_ref().map(Ty::from_expr).transpose()?.unwrap_or(Ty::Unit);
        let is_main = f.name == "main";
        if is_main && !matches!(ret, Ty::Unit | Ty::I64) {
            return err("`main` must return nothing or an i64 exit code", f.span);
        }
        let sig = self.signature(&params, ret, is_main);
        let id = self
            .module
            .declare_function(&Self::symbol(&f.name), Linkage::Export, &sig)
            .map_err(|e| CompileError { message: e.to_string(), span: f.span })?;
        self.fns.insert(f.name.clone(), FnInfo { id, params, ret });
        Ok(())
    }

    fn define(&mut self, f: &Function, ctx: &mut Context, fctx: &mut FunctionBuilderContext) -> CResult<()> {
        let info = &self.fns[&f.name];
        let (id, ret, params) = (info.id, info.ret, info.params.clone());
        let is_main = f.name == "main";
        ctx.func.signature = self.signature(&params, ret, is_main);

        let target = self.module.target_config();
        let mut b = FunctionBuilder::new(&mut ctx.func, fctx);
        let entry = b.create_block();
        b.append_block_params_for_function_params(entry);
        b.switch_to_block(entry);
        b.seal_block(entry);

        let mut lw = Lower { cg: self, b, scopes: vec![HashMap::new()], ret, is_main, dead: false };
        for (i, p) in f.params.iter().enumerate() {
            let val = lw.b.block_params(entry)[i];
            let var = lw.b.declare_var(params[i].cl().unwrap());
            lw.b.def_var(var, val);
            lw.bind(&p.name, var, params[i], false, p.span)?;
        }

        let (val, ty) = lw.block(&f.body)?;
        if !lw.dead {
            if ty != ret && ty != Ty::Never {
                let span = f.body.tail.as_ref().map_or(f.body.span, |t| t.span);
                if ret == Ty::Unit {
                    return err(format!("`{}` returns nothing, but its body ends with a {ty} value", f.name), span);
                }
                return err(format!("`{}` must return {ret}, but its body ends with {ty}", f.name), span);
            }
            lw.emit_return(val);
        }
        lw.b.seal_all_blocks();
        lw.b.finalize(target);

        self.module
            .define_function(id, ctx)
            .map_err(|e| CompileError { message: format!("codegen for `{}`: {e:?}", f.name), span: f.span })?;
        self.module.clear_context(ctx);
        Ok(())
    }

    fn string_data(&mut self, s: &str) -> CResult<DataId> {
        if let Some(&id) = self.strings.get(s) {
            return Ok(id);
        }
        let name = format!("om_str_{}", self.strings.len());
        let id = self
            .module
            .declare_data(&name, Linkage::Local, false, false)
            .map_err(|e| CompileError { message: e.to_string(), span: Span::default() })?;
        let mut desc = DataDescription::new();
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0);
        desc.define(bytes.into_boxed_slice());
        self.module
            .define_data(id, &desc)
            .map_err(|e| CompileError { message: e.to_string(), span: Span::default() })?;
        self.strings.insert(s.to_string(), id);
        Ok(id)
    }

    fn libc(&mut self, name: &str, params: &[types::Type], variadic_ret: types::Type) -> CResult<FuncId> {
        let mut sig = self.module.make_signature();
        for &p in params {
            sig.params.push(AbiParam::new(p));
        }
        sig.returns.push(AbiParam::new(variadic_ret));
        self.module
            .declare_function(name, Linkage::Import, &sig)
            .map_err(|e| CompileError { message: e.to_string(), span: Span::default() })
    }
}

struct Local {
    var: Variable,
    ty: Ty,
    mutable: bool,
}

struct Lower<'a> {
    cg: &'a mut Codegen,
    b: FunctionBuilder<'a>,
    scopes: Vec<HashMap<String, Local>>,
    ret: Ty,
    is_main: bool,
    /// True after a `return`: code that follows is unreachable.
    dead: bool,
}

impl<'a> Lower<'a> {
    fn bind(&mut self, name: &str, var: Variable, ty: Ty, mutable: bool, span: Span) -> CResult<()> {
        if self.cg.fns.contains_key(name) || BUILTINS.contains(&name) {
            return err(format!("`{name}` is already the name of a function"), span);
        }
        self.scopes.last_mut().unwrap().insert(name.to_string(), Local { var, ty, mutable });
        Ok(())
    }

    fn lookup(&self, name: &str) -> Option<&Local> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    /// After a terminator, keep lowering into a fresh block with no predecessors.
    fn start_dead_block(&mut self) {
        let blk = self.b.create_block();
        self.b.switch_to_block(blk);
        self.dead = true;
    }

    fn emit_return(&mut self, val: Option<Value>) {
        if self.is_main {
            let code = match val {
                Some(v) => self.b.ins().ireduce(types::I32, v),
                None => self.b.ins().iconst(types::I32, 0),
            };
            self.b.ins().return_(&[code]);
        } else {
            match val {
                Some(v) => self.b.ins().return_(&[v]),
                None => self.b.ins().return_(&[]),
            };
        }
    }

    fn block(&mut self, blk: &Block) -> CResult<(Option<Value>, Ty)> {
        self.scopes.push(HashMap::new());
        let result = self.block_inner(blk);
        self.scopes.pop();
        result
    }

    fn block_inner(&mut self, blk: &Block) -> CResult<(Option<Value>, Ty)> {
        for s in &blk.stmts {
            self.stmt(s)?;
        }
        if let Some(tail) = &blk.tail {
            let r = self.expr(tail)?;
            return Ok(if self.dead { (None, Ty::Never) } else { r });
        }
        Ok((None, if self.dead { Ty::Never } else { Ty::Unit }))
    }

    fn stmt(&mut self, s: &Stmt) -> CResult<()> {
        match s {
            Stmt::Let { name, mutable, ty, value, span } => {
                let (val, vty) = self.expr(value)?;
                if let Some(t) = ty {
                    let want = Ty::from_expr(t)?;
                    if want != vty && vty != Ty::Never {
                        return err(format!("`{name}` is declared as {want} but its value is {vty}"), value.span);
                    }
                }
                let Some(cl) = vty.cl() else {
                    return err(format!("cannot store a {vty} value in `{name}`"), value.span);
                };
                let var = self.b.declare_var(cl);
                self.b.def_var(var, val.unwrap());
                self.bind(name, var, vty, *mutable, *span)
            }
            Stmt::Assign { name, value, span } => {
                let (val, vty) = self.expr(value)?;
                let Some(local) = self.lookup(name) else {
                    return err(format!("cannot find variable `{name}`"), *span);
                };
                if !local.mutable {
                    return err(format!("cannot assign twice to `{name}`; declare it with `let mut {name}`"), *span);
                }
                if local.ty != vty {
                    return err(format!("`{name}` has type {} but the new value is {vty}", local.ty), value.span);
                }
                let var = local.var;
                self.b.def_var(var, val.unwrap());
                Ok(())
            }
            Stmt::While { cond, body, .. } => {
                let header = self.b.create_block();
                let body_blk = self.b.create_block();
                let exit = self.b.create_block();
                self.b.ins().jump(header, &[]);
                self.b.switch_to_block(header);
                let c = self.cond(cond)?;
                self.b.ins().brif(c, body_blk, &[], exit, &[]);
                self.b.switch_to_block(body_blk);
                self.b.seal_block(body_blk);
                let was_dead = self.dead;
                self.block(body)?;
                if !self.dead {
                    self.b.ins().jump(header, &[]);
                }
                self.dead = was_dead;
                self.b.seal_block(header);
                self.b.switch_to_block(exit);
                self.b.seal_block(exit);
                Ok(())
            }
            Stmt::Return { value, span } => {
                let (val, ty) = match value {
                    Some(e) => self.expr(e)?,
                    None => (None, Ty::Unit),
                };
                if ty != self.ret && ty != Ty::Never {
                    return err(format!("this function returns {}, but `return` gives {ty}", self.ret), *span);
                }
                if !self.dead {
                    self.emit_return(val);
                }
                self.start_dead_block();
                Ok(())
            }
            Stmt::Expr(e) => self.expr(e).map(|_| ()),
        }
    }

    fn cond(&mut self, e: &Expr) -> CResult<Value> {
        let (v, ty) = self.expr(e)?;
        if ty != Ty::Bool {
            return err(format!("condition must be bool, found {ty}"), e.span);
        }
        Ok(v.unwrap())
    }

    fn expr(&mut self, e: &Expr) -> CResult<(Option<Value>, Ty)> {
        match &e.kind {
            ExprKind::Int(n) => Ok((Some(self.b.ins().iconst(types::I64, *n)), Ty::I64)),
            ExprKind::Bool(v) => Ok((Some(self.b.ins().iconst(types::I8, *v as i64)), Ty::Bool)),
            ExprKind::Str(s) => Ok((Some(self.str_ptr(s)?), Ty::Str)),
            ExprKind::Var(name) => match self.lookup(name) {
                Some(l) => {
                    let (var, ty) = (l.var, l.ty);
                    Ok((Some(self.b.use_var(var)), ty))
                }
                None => err(format!("cannot find variable `{name}`"), e.span),
            },
            ExprKind::Unary(op, inner) => {
                let (v, ty) = self.expr(inner)?;
                match (op, ty) {
                    (UnOp::Neg, Ty::I64) => Ok((Some(self.b.ins().ineg(v.unwrap())), Ty::I64)),
                    (UnOp::Not, Ty::Bool) => Ok((Some(self.b.ins().bxor_imm_u(v.unwrap(), 1)), Ty::Bool)),
                    (UnOp::Neg, _) => err(format!("cannot negate a {ty}"), e.span),
                    (UnOp::Not, _) => err(format!("`!` needs a bool, found {ty}"), e.span),
                }
            }
            ExprKind::Binary(op @ (BinOp::And | BinOp::Or), l, r) => self.short_circuit(*op, l, r),
            ExprKind::Binary(op, l, r) => {
                let (lv, lt) = self.expr(l)?;
                let (rv, rt) = self.expr(r)?;
                if lt != rt {
                    return err(format!("mismatched types: {lt} and {rt}"), e.span);
                }
                let (a, c) = (lv.unwrap(), rv.unwrap());
                let ins = self.b.ins();
                let out = match (op, lt) {
                    (BinOp::Add, Ty::I64) => (ins.iadd(a, c), Ty::I64),
                    (BinOp::Sub, Ty::I64) => (ins.isub(a, c), Ty::I64),
                    (BinOp::Mul, Ty::I64) => (ins.imul(a, c), Ty::I64),
                    (BinOp::Div, Ty::I64) => (ins.sdiv(a, c), Ty::I64),
                    (BinOp::Rem, Ty::I64) => (ins.srem(a, c), Ty::I64),
                    (BinOp::Eq | BinOp::Ne, Ty::I64 | Ty::Bool) => {
                        let cc = if *op == BinOp::Eq { IntCC::Equal } else { IntCC::NotEqual };
                        (ins.icmp(cc, a, c), Ty::Bool)
                    }
                    (BinOp::Lt, Ty::I64) => (ins.icmp(IntCC::SignedLessThan, a, c), Ty::Bool),
                    (BinOp::Le, Ty::I64) => (ins.icmp(IntCC::SignedLessThanOrEqual, a, c), Ty::Bool),
                    (BinOp::Gt, Ty::I64) => (ins.icmp(IntCC::SignedGreaterThan, a, c), Ty::Bool),
                    (BinOp::Ge, Ty::I64) => (ins.icmp(IntCC::SignedGreaterThanOrEqual, a, c), Ty::Bool),
                    _ => return err(format!("operator {op:?} is not defined for {lt}"), e.span),
                };
                Ok((Some(out.0), out.1))
            }
            ExprKind::Call(name, args) => self.call(name, args, e.span),
            ExprKind::If(cond, then, els) => self.if_expr(cond, then, els.as_ref(), e.span),
            ExprKind::Block(b) => self.block(b),
        }
    }

    fn short_circuit(&mut self, op: BinOp, l: &Expr, r: &Expr) -> CResult<(Option<Value>, Ty)> {
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
        Ok((Some(self.b.block_params(merge)[0]), Ty::Bool))
    }

    fn if_expr(&mut self, cond: &Expr, then: &Block, els: Option<&Block>, span: Span) -> CResult<(Option<Value>, Ty)> {
        let c = self.cond(cond)?;
        let then_blk = self.b.create_block();
        let else_blk = self.b.create_block();
        let merge = self.b.create_block();
        self.b.ins().brif(c, then_blk, &[], else_blk, &[]);
        let was_dead = self.dead;
        // The merge block gets a parameter for the if's value once we know its type.
        let mut has_param = false;

        self.b.switch_to_block(then_blk);
        self.b.seal_block(then_blk);
        self.dead = false;
        let (tv, tt) = self.block(then)?;
        let t_dead = self.dead;
        if !t_dead {
            self.jump_with(merge, tt, tv, &mut has_param);
        }

        self.b.switch_to_block(else_blk);
        self.b.seal_block(else_blk);
        self.dead = false;
        let (ev, et) = match els {
            Some(b) => self.block(b)?,
            None => (None, Ty::Unit),
        };
        let e_dead = self.dead;
        let ty = match (tt, et) {
            (a, b) if a == b => a,
            (Ty::Never, b) => b,
            (a, Ty::Never) => a,
            (a, _) if els.is_none() => {
                return err(format!("`if` without `else` must not produce a value, but this one gives {a}"), then.span);
            }
            (a, b) => return err(format!("`if` and `else` have different types: {a} and {b}"), span),
        };
        if !e_dead {
            self.jump_with(merge, et, ev, &mut has_param);
        }

        self.b.switch_to_block(merge);
        self.b.seal_block(merge);
        self.dead = was_dead || (t_dead && e_dead);
        if t_dead && e_dead {
            return Ok((None, Ty::Never));
        }
        let val = if has_param { Some(self.b.block_params(merge)[0]) } else { None };
        Ok((val, ty))
    }

    fn jump_with(&mut self, merge: ClBlock, ty: Ty, val: Option<Value>, has_param: &mut bool) {
        match (ty.cl(), val) {
            (Some(cl), Some(v)) => {
                if !*has_param {
                    self.b.append_block_param(merge, cl);
                    *has_param = true;
                }
                self.b.ins().jump(merge, &[v.into()]);
            }
            _ => {
                self.b.ins().jump(merge, &[]);
            }
        }
    }

    fn call(&mut self, name: &str, args: &[Expr], span: Span) -> CResult<(Option<Value>, Ty)> {
        if name == "print" {
            return self.print(args, span);
        }
        let Some(info) = self.cg.fns.get(name) else {
            return err(format!("cannot find function `{name}`"), span);
        };
        if name == "main" {
            return err("`main` cannot be called", span);
        }
        let (id, params, ret) = (info.id, info.params.clone(), info.ret);
        if args.len() != params.len() {
            return err(format!("`{name}` takes {} argument(s) but {} were given", params.len(), args.len()), span);
        }
        let mut vals = Vec::new();
        for (a, want) in args.iter().zip(&params) {
            let (v, t) = self.expr(a)?;
            if t != *want {
                return err(format!("expected {want}, found {t}"), a.span);
            }
            vals.push(v.unwrap());
        }
        let fref = self.cg.module.declare_func_in_func(id, self.b.func);
        let call = self.b.ins().call(fref, &vals);
        let v = self.b.inst_results(call).first().copied();
        Ok((v, ret))
    }

    /// `print(x)` writes an i64, bool or str followed by a newline.
    fn print(&mut self, args: &[Expr], span: Span) -> CResult<(Option<Value>, Ty)> {
        let [arg] = args else {
            return err("`print` takes exactly one argument", span);
        };
        let (v, ty) = self.expr(arg)?;
        let ptr = types::I64;
        match ty {
            Ty::I64 => self.call_print_i64(v.unwrap())?,
            Ty::Bool => {
                let t = self.str_ptr("true")?;
                let f = self.str_ptr("false")?;
                let s = self.b.ins().select(v.unwrap(), t, f);
                self.call_puts(s, ptr)?;
            }
            Ty::Str => self.call_puts(v.unwrap(), ptr)?,
            _ => return err(format!("cannot print a {ty} value"), arg.span),
        }
        Ok((None, Ty::Unit))
    }

    fn str_ptr(&mut self, s: &str) -> CResult<Value> {
        let id = self.cg.string_data(s)?;
        let gv = self.cg.module.declare_data_in_func(id, self.b.func);
        Ok(self.b.ins().symbol_value(types::I64, gv))
    }

    fn call_puts(&mut self, s: Value, ptr: types::Type) -> CResult<()> {
        let id = self.cg.libc("puts", &[ptr], types::I32)?;
        let f = self.cg.module.declare_func_in_func(id, self.b.func);
        self.b.ins().call(f, &[s]);
        Ok(())
    }

    fn call_print_i64(&mut self, n: Value) -> CResult<()> {
        // Cranelift cannot call variadic functions like printf portably (Apple ARM64
        // passes variadic arguments on the stack), so integers go through a small
        // non-variadic shim in the runtime that `omc` links into every program.
        let id = self.cg.libc("om_rt_print_i64", &[types::I64], types::I32)?;
        let f = self.cg.module.declare_func_in_func(id, self.b.func);
        self.b.ins().call(f, &[n]);
        Ok(())
    }
}

/// The C runtime linked into every Om program. Kept tiny: phase 4 replaces it with `om-std`.
pub const RUNTIME_C: &str = r#"#include <stdio.h>
int om_rt_print_i64(long long n) { return printf("%lld\n", n); }
"#;
