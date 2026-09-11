use std::{cell::RefCell, rc::Rc, string::String};

use miette::{IntoDiagnostic, NamedSource};

use crate::{
    cli::{Cli, ParseCommand},
    config::Config,
    lang::{
        eval::{
            Engine, EvalError, EvalResult,
            registry::FunctionValue,
            value::{
                CallContext, ValueRef,
                native::{Array, Float, Int, Null, Object},
            },
        },
        lex::Lexer,
        parse::Parser,
        string::IStr,
    },
    state::State,
};

fn json(ctx: &CallContext, arg: ValueRef, out: &mut String) -> EvalResult<()> {
    // SAFETY: we only write valid strings
    let outw = unsafe { out.as_mut_vec() };
    #[allow(clippy::redundant_pattern_matching)]
    if let Some(s) = arg.borrow().downcast_ref::<IStr>() {
        serde_json::to_writer(outw, &**s).unwrap();
    } else if let Some(i) = arg.downcast::<Int>() {
        serde_json::to_writer(outw, &i).unwrap();
    } else if let Some(f) = arg.downcast::<Float>() {
        serde_json::to_writer(outw, &f).unwrap();
    } else if let Some(b) = arg.downcast::<bool>() {
        serde_json::to_writer(outw, &b).unwrap();
    } else if let Some(_) = arg.downcast::<Null>() {
        serde_json::to_writer(outw, &()).unwrap();
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

fn env(s: &IStr) -> ValueRef {
    std::env::var(&**s)
        .ok()
        .map(IStr::from)
        .map(ValueRef::new)
        .unwrap_or_else(ValueRef::null)
}

fn make_engine() -> miette::Result<Rc<Engine>> {
    let engine = Engine::new();
    let global = engine.global();

    global.set_variable("env", env as fn(&IStr) -> ValueRef, true);
    global.set_variable(
        "print",
        FunctionValue::new(|_ctx, s: ValueRef| {
            let mut out = String::new();
            s.borrow().to_string(&mut out);
            println!("{}", out);
            ValueRef::null()
        }),
        true,
    );

    global.set_variable(
        "json",
        FunctionValue::new(|ctx, arg| {
            let mut out = String::new();
            json(&ctx, arg, &mut out)?;
            Ok(IStr::from(out))
        }),
        true,
    );

    Ok(engine)
}

pub(crate) fn run(
    _cli: &Cli,
    cmd: &ParseCommand,
    _config: Config,
    _state: Rc<RefCell<State>>,
) -> miette::Result<()> {
    let file_name = cmd.file.file_name().unwrap().to_string_lossy();
    let content = std::fs::read_to_string(&cmd.file).into_diagnostic()?;
    let lex = Lexer::new(&content);

    let mut parser = Parser::new(lex).map_err(|e| {
        miette::Report::from(e).with_source_code(NamedSource::new(&file_name, content.to_string()))
    })?;

    let engine = make_engine()?;

    let mut v = ValueRef::null();
    while let Some(e) = parser.take_expr().map_err(|e| {
        miette::Report::from(e).with_source_code(NamedSource::new(&file_name, content.to_string()))
    })? {
        v = engine.global().eval(e).map_err(|e| {
            miette::Report::from(e)
                .with_source_code(NamedSource::new(&file_name, content.to_string()))
        })?;
    }

    dbg!(v);

    Ok(())
}

#[cfg(test)]
mod test {
    use miette::NamedSource;

    use super::make_engine;
    use crate::lang::{lex::Lexer, parse::Parser, string::IStr};

    macro_rules! eval {
        ($engine: expr, $($tt: tt)*) => {{
            let content = stringify!($($tt)*);
            let lex = Lexer::new(&content);
            let mut parser = Parser::new(lex)
                .map_err(|e| {
                    miette::Report::from(e)
                        .with_source_code(NamedSource::new("literal", content.to_string()))
                })
                .unwrap();
            let expr = parser
                .take_expr()
                .map_err(|e| {
                    miette::Report::from(e)
                        .with_source_code(NamedSource::new("literal", content.to_string()))
                })
                .unwrap()
                .unwrap();

            $engine.global().eval(expr)
                .map_err(|e| {
                    miette::Report::from(e)
                        .with_source_code(NamedSource::new("literal", content.to_string()))
                })
                .unwrap()
        }};
    }

    #[test]
    fn valid_json() {
        macro_rules! json {
            ($inq: tt) => {{
                let e = make_engine().unwrap();
                eval!(e, json($inq))
            }};
        }

        let j = json!({
            "id": "user_123456",
            "username": "john_doe",
            "email": "john.doe@example.com",
            "firstName": "John",
            "lastName": "Doe",
            "avatar": "https://example.com/avatars/john_doe.png",
            "dateOfBirth": "1990-05-15",
            "phoneNumber": "+1-555-0123",
            "address": {
                "street": "123 Main St",
                "city": "New York",
                "state": "NY",
                "zipCode": "10001",
                "country": "USA"
            },
            "preferences": {
                "theme": "dark",
                "language": "en",
                "notifications": {
                    "email": true,
                    "push": false,
                    "sms": true
                }
            },
            "createdAt": "2023-01-15T10:30:00Z",
            "lastLoginAt": "2024-01-15T14:22:00Z",
            "isActive": true,
            "roles": [
                "user",
                "premium"
            ]
        });

        assert_eq!(
            j.downcast::<IStr>().unwrap().as_str(),
            serde_json::to_string(&serde_json::json!({
                "id": "user_123456",
                "username": "john_doe",
                "email": "john.doe@example.com",
                "firstName": "John",
                "lastName": "Doe",
                "avatar": "https://example.com/avatars/john_doe.png",
                "dateOfBirth": "1990-05-15",
                "phoneNumber": "+1-555-0123",
                "address": {
                    "street": "123 Main St",
                    "city": "New York",
                    "state": "NY",
                    "zipCode": "10001",
                    "country": "USA"
                },
                "preferences": {
                    "theme": "dark",
                    "language": "en",
                    "notifications": {
                        "email": true,
                        "push": false,
                        "sms": true
                    }
                },
                "createdAt": "2023-01-15T10:30:00Z",
                "lastLoginAt": "2024-01-15T14:22:00Z",
                "isActive": true,
                "roles": [
                    "user",
                    "premium"
                ]
            }))
            .unwrap()
        )
    }
}
