use std::{
    borrow::Cow,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{
    eval::{
        ControlFlow, Scope, Variable,
        call_stack::StackFrame,
        lazy::LazyValueRef,
        registry::{FnCtx, Function, Registry, VarArgs},
        value::{CallContext, Value},
    },
    parse::FunctionItem,
};

use super::ValueRef;

/// A unique identifier for every user-defined function
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FunctionId {
    inner: usize,
}

impl FunctionId {
    fn new() -> Self {
        // The identifier doesn't actually need to be specific, so a global counter seems fine
        static GLOBAL: AtomicUsize = AtomicUsize::new(0);
        Self {
            inner: GLOBAL.fetch_add(1, Ordering::SeqCst),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct UserFunction {
    id: FunctionId,
    func: FunctionItem,
    scope: Rc<Scope>,
}
impl UserFunction {
    pub fn new(func: FunctionItem, scope: Rc<Scope>) -> Self {
        Self {
            id: FunctionId::new(),
            func,
            scope,
        }
    }
}

impl Value for UserFunction {
    fn type_name() -> Cow<'static, str>
    where
        Self: Sized,
    {
        "Function".into()
    }

    fn type_name_of(&self) -> Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        use std::fmt::Write;
        write!(out, "<Function {}>", self.func.name).expect("write to string can't fail");
    }

    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(self.clone())
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "<Function {}>", self.func.name)
    }

    fn eq(&self, _: ValueRef) -> bool {
        false
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_call::<fn(CallContext<FnCtx<'_>>, &_, VarArgs) -> _>(
            |mut ctx, Self { id, func, scope }, mut varargs| {
                if varargs.len() != func.args.len() {
                    return varargs.error_n(&ctx, func.args.len());
                }

                let scope = scope.make_child();
                scope.set_special(ctx.scope().special().unwrap_or_default());

                for (i, a) in func.args.iter().enumerate() {
                    let x = varargs.shift().expect("Checked above");
                    scope.declare_variable(Variable {
                        name: a.as_istr(),
                        def: Some(a.clone()),
                        value: LazyValueRef::from_value(ctx.arg_spans[i], x),
                        readonly: false,
                        on_resolve: |_, _| unreachable!(),
                    });
                }

                let frame = StackFrame { id: *id };
                let v = match ctx
                    .call_stack
                    .with_frame(frame, |cs| scope.eval_with_stack(cs, &func.body))?
                {
                    ControlFlow::Value(v) | ControlFlow::Return(v) => v,
                };
                Ok(v)
            },
        );
    }
}

#[derive(derive_more::Debug, Clone)]
pub struct FunctionValue {
    #[expect(unused)]
    pub id: FunctionId,
    #[debug(skip)]
    pub func: Rc<dyn Function>,
}

impl FunctionValue {
    pub fn new<V, R>(func: fn(CallContext<FnCtx>, V) -> R) -> Self
    where
        fn(CallContext<FnCtx>, V) -> R: Function + 'static,
    {
        Self {
            id: FunctionId::new(),
            func: Rc::new(func),
        }
    }
}

impl Value for FunctionValue {
    fn type_name() -> Cow<'static, str>
    where
        Self: Sized,
    {
        "Function".into()
    }

    fn type_name_of(&self) -> Cow<'static, str> {
        "Function".into()
    }

    fn to_string(&self, out: &mut String) {
        out.push_str("<native function>");
    }
    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "<native function>")
    }

    fn snapshot(&self) -> Rc<dyn Value> {
        Rc::new(self.clone())
    }
    fn eq(&self, _other: ValueRef) -> bool {
        false
    }

    fn register(registry: &mut Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_call::<fn(CallContext<FnCtx<'_>>, &_, VarArgs) -> _>(|ctx, f, args| {
            f.func.call(args, ctx)
        });
    }
}
