use std::{
    any::TypeId,
    cell::{Cell, OnceCell, Ref, RefCell},
    collections::HashMap,
    io::Write,
    rc::Rc,
};

pub mod call_stack;
mod ext;
pub(crate) mod lazy;
pub mod registry;
pub mod value;

mod error;
pub use error::*;

mod scope;
pub use scope::*;

use crate::{
    IStr, Span,
    eval::{
        registry::{AnyRegistry, Indexer, Registry, VarArgs},
        value::{
            Value, ValueRef,
            function::{FunctionValue, UserFunction},
            native::{Array, Float, Int, Null, Object},
            ty::TypeValue,
        },
    },
};

#[derive(Default, Debug)]
struct TypeRegistry {
    /// Key is name of type, value is key-value pair in `regs`
    names: HashMap<IStr, Rc<AnyRegistry>>,
    regs: HashMap<TypeId, Rc<AnyRegistry>>,
}
impl TypeRegistry {
    fn get(&self, value: &ValueRef, span: Span) -> EvalResult<Rc<AnyRegistry>> {
        let tid = value.type_id();
        self.regs
            .get(&tid)
            .cloned()
            .ok_or_else(|| EvalError::UnknownType {
                ty: value.type_name_of().into(),
                span,
            })
    }

    fn get_by_name(&self, name: &str) -> Option<Rc<AnyRegistry>> {
        self.names.get(name).cloned()
    }

    fn add<V: Value>(this: &RefCell<Self>, engine: Rc<Engine>) {
        let tid = TypeId::of::<V>();
        let this_ref = this.borrow();
        if let Some(reg) = this_ref.regs.get(&tid) {
            if !this_ref.names.iter().any(|(_, v)| v.for_same_type(reg)) {
                panic!("Type in regs but not names");
            }
            return;
        }
        drop(this_ref);

        let mut reg = Registry::new(engine);
        V::register(&mut reg);
        let reg = Rc::new(reg.erase());
        let mut this_mut = this.borrow_mut();
        this_mut.regs.insert(reg.type_id, reg.clone());
        this_mut.names.insert(reg.name.clone(), reg);
    }
}

type PrintFn = fn(std::fmt::Arguments<'_>) -> std::io::Result<()>;

#[derive(derive_more::Debug)]
pub struct Engine {
    types: RefCell<TypeRegistry>,
    global: OnceCell<Rc<Scope>>,
    on_stdout: Cell<PrintFn>,
    on_stderr: Cell<PrintFn>,
}

impl Engine {
    pub fn new() -> Rc<Self> {
        let this = Self {
            types: Default::default(),
            global: Default::default(),
            on_stdout: Cell::new(|args| std::io::stdout().write_fmt(args)),
            on_stderr: Cell::new(|args| std::io::stderr().write_fmt(args)),
        };
        let this = Rc::new(this);
        this.register_defaults();
        this.add_default_functions();
        this
    }

    fn add_default_functions(self: &Rc<Self>) {
        self.global().declare_function("print", |ctx, s: VarArgs| {
            let print = |args: std::fmt::Arguments<'_>| {
                (ctx.engine().on_stdout.get())(args).map_err(|e| ctx.wrap_error(e))
            };
            for (i, a) in s.inner.into_iter().enumerate() {
                if i > 0 {
                    print(format_args!(" "))?;
                }
                let mut out = String::new();
                a.value().to_string(&mut out);
                print(format_args!("{}", out))?;
            }
            print(format_args!("\n"))?;
            Ok(ValueRef::null())
        });

        self.global().declare_function("debug", |ctx, s: ValueRef| {
            (ctx.engine().on_stderr.get())(format_args!("{:#?}\n", s.debug()))
                .map_err(|e| ctx.wrap_error(e))
        });
    }

    fn register_defaults(self: &Rc<Self>) {
        self.register_type::<TypeValue>();
        self.register_type::<IStr>();
        self.register_type::<Int>();
        self.register_type::<Float>();
        self.register_type::<Null>();
        self.register_type::<bool>();
        self.register_type::<Array>();
        self.register_type::<Object>();
        self.register_type::<FunctionValue>();
        self.register_type::<UserFunction>();
    }

    pub fn register_type<V: Value>(self: &Rc<Self>) {
        TypeRegistry::add::<V>(&self.types, self.clone());
    }

    pub fn on_stdout(self: &Rc<Self>, func: PrintFn) {
        self.on_stdout.replace(func);
    }
    pub fn on_stderr(self: &Rc<Self>, func: PrintFn) {
        self.on_stdout.replace(func);
    }

    pub fn global(self: &Rc<Self>) -> Rc<Scope> {
        self.global
            .get_or_init({
                let this = self.clone();
                move || Rc::new(Scope::new(this))
            })
            .clone()
    }

    fn types(&self) -> Ref<'_, TypeRegistry> {
        self.types.borrow()
    }

    pub(crate) fn get_type(&self, value: &ValueRef, span: Span) -> EvalResult<Rc<AnyRegistry>> {
        let types = self.types();
        types.get(value, span)
    }

    pub(crate) fn get_type_by_name(&self, name: &str) -> Option<Rc<AnyRegistry>> {
        self.types().get_by_name(name)
    }

    fn get_index(&self, value: &ValueRef, index: &ValueRef, span: Span) -> EvalResult<Indexer> {
        let types = self.types();
        let reg = types.get(value, span)?;
        reg.get_index(index.type_id())
            .ok_or_else(|| EvalError::InvalidIndex {
                ty: value.type_name_of().into(),
                index: index.type_name_of().into(),
                span,
            })
    }
}

#[cfg(test)]
mod test {
    use std::assert_matches;

    use crate::{
        eval::{
            Engine, EvalError, Variable,
            lazy::identity_mapper,
            value::{
                function::FunctionValue,
                native::{Int, Null},
            },
        },
        eval_expr,
    };

    #[test]
    fn variable_declare() {
        let engine = Engine::new();
        let v = eval_expr! { engine,
            let x = 420;
            x
        };
        let n = v.unwrap::<Int>();
        assert_eq!(n, 420);
    }

    #[test]
    fn variable_declare_empty() {
        let engine = Engine::new();
        let v = eval_expr! { engine,
            let x;
            x
        };
        let Null = v.unwrap::<Null>();
    }

    #[test]
    fn variable_overwrite() {
        let engine = Engine::new();
        let v = eval_expr! { engine,
            let x = 42;
            x = 27;
            x
        };
        let n = v.unwrap::<Int>();
        assert_eq!(n, 27);
    }

    #[test]
    fn variable_shadow() {
        let engine = Engine::new();
        let v = eval_expr! { engine,
            let x = 42;
            let x = 27;
            x
        };
        let n = v.unwrap::<Int>();
        assert_eq!(n, 27);
    }

    #[test]
    fn readonly_variable() {
        let engine = Engine::new();
        engine.global().declare_variable(Variable {
            name: "hello".into(),
            def: None,
            value: 42.into(),
            readonly: true,
            on_resolve: identity_mapper,
        });
        // ensure it's set
        let v = eval_expr! { engine,
            hello
        };
        assert_eq!(v.unwrap::<Int>(), 42);

        let err = eval_expr! { try engine,
            hello = 27;
            hello
        }
        .unwrap_err();
        assert_matches!(err, EvalError::ReadonlyVariable { .. })
    }

    #[test]
    fn readonly_variable_shadow() {
        let engine = Engine::new();
        engine.global().declare_variable(Variable {
            name: "hello".into(),
            def: None,
            value: 42.into(),
            readonly: true,
            on_resolve: identity_mapper,
        });
        // ensure it's set
        let v = eval_expr! { engine,
            hello
        };
        assert_eq!(v.unwrap::<Int>(), 42);

        let v = eval_expr! { engine,
            let hello = 27;
            hello
        };
        assert_eq!(v.unwrap::<Int>(), 27);
    }

    #[test]
    fn readonly_function() {
        let engine = Engine::new();
        engine
            .global()
            .declare_function("test", |_ctx, ()| unimplemented!() as ());
        // ensure it's set
        let v = eval_expr! { engine,
            test
        };
        v.unwrap::<FunctionValue>();

        let err = eval_expr! { try engine,
            test = 42;
            test
        }
        .unwrap_err();
        assert_matches!(err, EvalError::ReadonlyVariable { .. })
    }

    #[test]
    fn readonly_function_shadow() {
        let engine = Engine::new();
        engine
            .global()
            .declare_function("test", |_ctx, ()| unimplemented!() as ());
        // ensure it's set
        let v = eval_expr! { engine,
            test
        };
        v.unwrap::<FunctionValue>();

        let v = eval_expr! { engine,
            let hello = 27;
            hello
        };
        assert_eq!(v.unwrap::<Int>(), 27);
    }
}
