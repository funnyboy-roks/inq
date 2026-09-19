use std::{
    any::TypeId,
    cell::{OnceCell, Ref, RefCell},
    collections::HashMap,
    rc::Rc,
};

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
        registry::{AnyRegistry, Indexer, Registry},
        value::{
            Value, ValueRef,
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

#[derive(Default, Debug)]
pub struct Engine {
    types: RefCell<TypeRegistry>,
    global: OnceCell<Rc<Scope>>,
}

impl Engine {
    pub fn new() -> Rc<Self> {
        let this = Self {
            types: Default::default(),
            global: OnceCell::new(),
        };
        let this = Rc::new(this);
        this.register_defaults();
        this
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
    }

    pub fn register_type<V: Value>(self: &Rc<Self>) {
        TypeRegistry::add::<V>(&self.types, self.clone());
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

    fn get_type(&self, value: &ValueRef, span: Span) -> EvalResult<Rc<AnyRegistry>> {
        let types = self.types();
        types.get(value, span)
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
