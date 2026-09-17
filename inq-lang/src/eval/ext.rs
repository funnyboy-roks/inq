//! Extensions of other types for purposes of evaluation

use std::rc::Rc;

use crate::{
    IStr, Route,
    eval::{Engine, EvalResult, Scope},
};

impl Route {
    pub fn endpoint_eval(&self, scope: &Rc<Scope>) -> EvalResult<IStr> {
        scope
            .eval(self.endpoint.clone())?
            .expect_downcast(self.endpoint.span)
    }
}
