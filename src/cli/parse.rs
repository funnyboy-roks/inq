use std::{cell::RefCell, rc::Rc, string::String};

use miette::{IntoDiagnostic, NamedSource};

use crate::{
    cli::{Cli, ParseCommand},
    config::Config,
    lang::{
        eval::{
            Context, EvalError, EvalResult,
            registry::FunctionValue,
            value::{
                CallContext, ValueRef,
                native::{Array, Float, Int, Null, Object},
            },
        },
        lex::Lexer,
        parse::Parser,
    },
    state::State,
};

fn json(ctx: &CallContext, arg: ValueRef, out: &mut String) -> EvalResult<()> {
    // SAFETY: we only write valid strings
    let outw = unsafe { out.as_mut_vec() };
    #[allow(clippy::redundant_pattern_matching)]
    if let Some(s) = arg.borrow().downcast_ref::<String>() {
        serde_json::to_writer(outw, s).unwrap();
    } else if let Some(i) = arg.borrow().downcast_ref::<Int>() {
        serde_json::to_writer(outw, i).unwrap();
    } else if let Some(f) = arg.borrow().downcast_ref::<Float>() {
        serde_json::to_writer(outw, f).unwrap();
    } else if let Some(b) = arg.borrow().downcast_ref::<bool>() {
        serde_json::to_writer(outw, b).unwrap();
    } else if let Some(_) = arg.borrow().downcast_ref::<Null>() {
        serde_json::to_writer(outw, &None::<()>).unwrap();
    } else if let Some(a) = arg.borrow().downcast_ref::<Array>() {
        out.push('[');
        for (i, a) in a.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            json(ctx, a.clone(), out)?;
        }
        out.push(']');
    } else if let Some(o) = arg.borrow().downcast_ref::<Object>() {
        out.push('{');
        for (i, (k, v)) in o.0.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            // SAFETY: we only write valid strings
            let outw = unsafe { out.as_mut_vec() };
            serde_json::to_writer(outw, &**k).unwrap();
            out.push(':');
            json(ctx, v.clone(), out)?;
        }
        out.push('}');
    } else {
        return Err(EvalError::Custom {
            message: format!("Invalid type for JSON: {}", arg.type_name_of()),
            span: ctx.span,
        });
    }
    Ok(())
}

pub(crate) fn run(
    _cli: &Cli,
    cmd: &ParseCommand,
    _config: Config,
    state: Rc<RefCell<State>>,
) -> miette::Result<()> {
    let file_name = cmd.file.file_name().unwrap().to_string_lossy();
    let content = std::fs::read_to_string(&cmd.file).into_diagnostic()?;
    let lex = Lexer::new(&content);
    let mut parser = Parser::new(lex).map_err(|e| {
        miette::Report::from(e).with_source_code(NamedSource::new(&file_name, content.to_string()))
    })?;

    let mut context = Context::new();

    fn env(s: &String) -> ValueRef {
        std::env::var(s)
            .ok()
            .map(ValueRef::new)
            .unwrap_or_else(ValueRef::null)
    }

    context.set_variable("env", env as fn(&String) -> ValueRef);
    context.set_variable::<fn(&String) -> ValueRef>("print", |s: &String| {
        println!("{}", s);
        ValueRef::null()
    });

    context.set_variable(
        "json",
        FunctionValue::new::<fn(CallContext, _) -> _>(|ctx, arg: ValueRef| {
            let mut out = String::new();
            json(&ctx, arg, &mut out)?;
            Ok(out)
        }),
    );

    let context = Rc::new(context);

    let mut v = ValueRef::null();
    while let Some(e) = parser.take_expr().map_err(|e| {
        miette::Report::from(e).with_source_code(NamedSource::new(&file_name, content.to_string()))
    })? {
        v = context.eval(e).map_err(|e| {
            miette::Report::from(e)
                .with_source_code(NamedSource::new(&file_name, content.to_string()))
        })?;
    }

    dbg!(v);

    Ok(())
}
