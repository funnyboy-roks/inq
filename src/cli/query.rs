use std::{cell::RefCell, rc::Rc, time::Instant};

use chrono::Utc;
use fuzzt::processors::{LowerAlphaNumStringProcessor, StringProcessor};
use miette::{Context, IntoDiagnostic, bail};

use crate::{
    cli::{Cli, QueryCommand},
    config::Config,
    print::{print_request, print_response},
    script::{ScriptResponse, script_engine},
    state::{PersistedVariable, State},
    util::WithLabel,
};

fn list_queries(cli: &Cli, config: Rc<Config>, state: Rc<RefCell<State>>) -> miette::Result<()> {
    let vars = Rc::new(config.load_variables(&cli.subcmd, &state.borrow())?);

    for (name, val) in &*vars {
        let mut state = state.borrow_mut();
        if let Some(var) = config.get_variable(name)
            && var.persist.persists()
            && !state.variables.contains_key(name)
        {
            state.variables.insert(
                name.to_string(),
                PersistedVariable {
                    value: val.interpolate(&vars)?.into_owned(),
                    expires_at: var.persist.duration().map(|d| Utc::now() + d),
                },
            );
        }
    }

    let queries: Vec<_> = config.queries()?.collect();

    if queries.is_empty() {
        bail!("No queries defined");
    }

    let name_len = queries
        .iter()
        .map(|n| n.0.len())
        .max()
        .expect("At least one query is defined");
    let method_len = queries
        .iter()
        .map(|n| {
            n.1.as_ref()
                .map(|q| q.method.as_str().len())
                .unwrap_or("ERROR".len())
        })
        .max()
        .expect("At least one query is defined");

    let mut failed = false;
    for (name, q) in queries {
        match q {
            Ok(q) => {
                use owo_colors::OwoColorize as _;
                println!(
                    "{:<name_len$}  {:<method_len$}  {}",
                    name.blue().bold(),
                    q.method.as_str().yellow(),
                    q.url.interpolate(&vars)?.green()
                );
            }
            Err(e) => {
                use owo_colors::OwoColorize as _;
                failed = true;
                println!(
                    "{:<name_len$}  {:<method_len$}  {}",
                    name.red().bold(),
                    "ERROR".red(),
                    e.red(),
                );
            }
        }
    }

    if failed {
        bail! {
            "One or more queries failed to parse"
        }
    } else {
        Ok(())
    }
}

pub(crate) fn run(
    cli: &Cli,
    query_cmd: &QueryCommand,
    config: Rc<Config>,
    state: Rc<RefCell<State>>,
) -> miette::Result<()> {
    let Some(query) = &query_cmd.query else {
        return list_queries(cli, config, state);
    };

    let vars = Rc::new(config.load_variables(&cli.subcmd, &state.borrow())?);

    for (name, val) in &*vars {
        let mut state = state.borrow_mut();
        if let Some(var) = config.get_variable(name)
            && var.persist.persists()
            && !state.variables.contains_key(name)
        {
            state.variables.insert(
                name.to_string(),
                PersistedVariable {
                    value: val.interpolate(&vars)?.into_owned(),
                    expires_at: var.persist.duration().map(|d| Utc::now() + d),
                },
            );
        }
    }

    let Some(query) = config.get_query(query)? else {
        let closest = config
            .queries()?
            .flat_map(|q| q.1.ok())
            .map(|q| {
                (
                    fuzzt::algorithms::normalized_levenshtein(
                        &LowerAlphaNumStringProcessor.process(query),
                        &LowerAlphaNumStringProcessor.process(q._name),
                    ),
                    q,
                )
            })
            .max_by(|l, r| l.0.total_cmp(&r.0));

        let Some(closest) = closest else {
            bail!("No queries defined");
        };

        let (lev, closest) = closest;

        if lev > 0.5 {
            miette::bail! {
                labels = vec![closest.name_span.with_label("Similarly named query defined here")],
                help = "Another query with a similar name exists",
                "Query '{}' not found", query
            }
        } else {
            miette::bail! {
                "Query '{}' not found", query
            }
        }
    };

    let client = config.make_client(&vars)?;
    let (req, req_body) = query.to_request(&client, &vars)?;

    print_request(&req, req_body)?;

    let start = Instant::now();
    let res = client.execute(req).into_diagnostic()?;
    let res = ScriptResponse::from_response(res, query_cmd.raw)?;
    let elapsed = start.elapsed();

    print_response(&res, elapsed)?;

    if let Some(post_script) = query.post_script {
        eprintln!(
            "{}",
            owo_colors::OwoColorize::purple(&"Running post-script")
        );

        let (engine, mut scope) = script_engine(state, config, vars);
        scope.push("response", res);

        engine
            .run_ast_with_scope(&mut scope, &post_script)
            .map_err(|e| miette::miette!("{}", e))
            .context("Evaluating post-script")?;
    }

    Ok(())
}
