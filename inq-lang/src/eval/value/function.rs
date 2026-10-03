use std::{borrow::Cow, rc::Rc};

use crate::{
    eval::{
        Scope, Variable,
        lazy::LazyValueRef,
        registry::{Registry, VarArgs},
        value::Value,
    },
    parse::FunctionItem,
};

use super::ValueRef;

#[derive(Clone, Debug)]
pub(crate) struct UserFunction {
    func: FunctionItem,
    scope: Rc<Scope>,
}
impl UserFunction {
    pub fn new(func: FunctionItem, scope: Rc<Scope>) -> Self {
        Self { func, scope }
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
        registry.register_call::<fn(_, &_, VarArgs) -> _>(
            |ctx, Self { func, scope }, mut varargs| {
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

                scope.eval(func.body.clone())
            },
        );
    }
}
