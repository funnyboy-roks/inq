use std::{
    cell::RefCell,
    process::{Child, Command, Output, Stdio},
    rc::Rc,
};

use inq_lang::{
    IStr,
    eval::{
        registry::FnCtx,
        value::{
            CallContext, Value, ValueRef,
            native::{Array, Int},
        },
    },
};

use crate::{debug_fmt, script::bytes_value::BytesValue};

pub fn system(
    ctx: CallContext<FnCtx<'_>>,
    a: ValueRef,
) -> Result<ChildValue, inq_lang::eval::EvalError> {
    let a = if let Some(s) = a.downcast::<IStr>() {
        shell_words::split(&s)
            .map_err(|e| ctx.error_arg(0, format!("Unable to parse command line: {}", e)))?
    } else if let Some(a) = a.downcast::<Array>() {
        a.into_vec()
            .into_iter()
            .map(|a| {
                a.downcast::<IStr>()
                    .as_ref()
                    .map(Into::into)
                    .ok_or(ctx.error_arg(0, "Expected String or Array of Strings"))
            })
            .collect::<Result<_, _>>()?
    } else {
        return Err(ctx.error_arg(0, "Expected String or Array of Strings"));
    };

    let command = a.first().ok_or(ctx.error_arg(0, "Command name required"))?;

    let child = Command::new(command)
        .args(&a[1..])
        .stdout(Stdio::piped()) // TODO: make this customisable
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ctx.error_arg(0, format!("Unable to spawn command: {}", e)))?;

    Ok(ChildValue::new(child))
}

#[derive(Debug, Clone)]
// Kind of weird to have this be Rc, but it needs to be global
pub struct ChildValue(Rc<RefCell<Option<Child>>>);

impl ChildValue {
    pub fn new(child: Child) -> Self {
        Self(Rc::new(RefCell::new(Some(child))))
    }
}

impl Value for ChildValue {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Child".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        out.push_str("<Child>");
    }

    fn snapshot(&self) -> std::rc::Rc<dyn Value> {
        Rc::new(self.clone())
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(child) = &*self.0.borrow() {
            debug_fmt! {
                into fmt as "Child",
                id => child.id() as Int,
            }
        } else {
            fmt.debug_tuple("Child")
                .field(&std::fmt::from_fn(|f| f.write_str("<consumed>")))
                .finish()
        }
    }

    fn eq(&self, other: inq_lang::eval::value::ValueRef) -> bool {
        other
            .downcast::<Self>()
            .is_some_and(|o| Rc::ptr_eq(&self.0, &o.0))
    }

    fn register(registry: &mut inq_lang::eval::registry::Registry<Self>)
    where
        Self: Sized,
    {
        registry.engine().register_type::<OutputValue>();

        registry.register_field_get("id", |ctx, this| {
            this.0
                .borrow()
                .as_ref()
                .ok_or_else(|| ctx.error("Child has already been consumed"))
                .map(|x| x.id() as Int)
        });
        registry.register_method::<fn(CallContext<FnCtx<'_>>, &_) -> _>("kill", |ctx, this| {
            if let Some(mut child) = this.0.borrow_mut().take() {
                child
                    .kill()
                    .map_err(|e| ctx.error(format!("Unable to kill child: {}", e)))
            } else {
                Err(ctx.error("Child has already been consumed"))
            }
        });
        registry.register_method::<fn(CallContext<FnCtx<'_>>, &_) -> _>("wait", |ctx, this| {
            if let Some(child) = this.0.borrow_mut().take() {
                child
                    .wait_with_output()
                    .map(OutputValue::new)
                    .map_err(|e| ctx.error(format!("Unable to wait for child: {}", e)))
            } else {
                Err(ctx.error("Child has already been consumed"))
            }
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputValue {
    status: Int,
    stdout: Rc<BytesValue>,
    stderr: Rc<BytesValue>,
}

impl OutputValue {
    pub fn new(output: Output) -> Self {
        Self {
            status: 's: {
                if let Some(c) = output.status.code() {
                    break 's c as Int;
                }

                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;

                    if let Some(s) = output.status.signal() {
                        break 's s as Int;
                    }
                }

                0
            },
            stdout: Rc::new(output.stdout.into()),
            stderr: Rc::new(output.stderr.into()),
        }
    }
}

impl Value for OutputValue {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Output".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, out: &mut String) {
        out.push_str("<Output>");
    }

    fn snapshot(&self) -> std::rc::Rc<dyn Value> {
        Rc::new(self.clone())
    }

    fn debug(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        debug_fmt! {
            into fmt as "Output",
            status => self.status,
            stdout => self.stdout.clone(),
            stderr => self.stderr.clone(),
        }
    }

    fn eq(&self, other: inq_lang::eval::value::ValueRef) -> bool {
        other.downcast::<Self>().is_some_and(|o| *self == o)
    }

    fn register(registry: &mut inq_lang::eval::registry::Registry<Self>)
    where
        Self: Sized,
    {
        registry.register_field_get("status", |_ctx, this| this.status);
        registry.register_field_get("stdout", |_ctx, this| this.stdout.clone());
        registry.register_field_get("stderr", |_ctx, this| this.stderr.clone());
    }
}
