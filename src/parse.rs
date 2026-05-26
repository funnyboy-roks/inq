use std::{borrow::Cow, fmt::Display};

use kdl::{KdlEntry, KdlNode, NodeKey};
use miette::bail;

use crate::util::WithLabel;

fn entry_value_as_string(entry: &KdlEntry) -> miette::Result<Cow<'_, str>> {
    let value = entry.value();
    let s: Cow<'_, _> = if let Some(s) = value.as_string() {
        s.into()
    } else if let Some(v) = value.as_integer() {
        v.to_string().into()
    } else if let Some(v) = value.as_float() {
        v.to_string().into()
    } else {
        bail! {
            labels = vec![entry.span().with_label("here")],
            "Expected variable value to be a string or number."
        }
    };

    Ok(s)
}

pub fn get_entry_string(
    node: &KdlNode,
    key: impl Into<NodeKey>,
    coerce: bool,
) -> miette::Result<Option<(&KdlEntry, Cow<'_, str>)>> {
    get_entry_string_named(node, key, coerce, "Value")
}

pub fn get_entry_string_named(
    node: &KdlNode,
    key: impl Into<NodeKey>,
    coerce: bool,
    name: impl Display,
) -> miette::Result<Option<(&KdlEntry, Cow<'_, str>)>> {
    let Some(entry) = node.entry(key) else {
        return Ok(None);
    };

    if coerce {
        entry_value_as_string(entry).map(|s| Some((entry, s)))
    } else {
        let Some(s) = entry.value().as_string() else {
            bail! {
                labels = vec![entry.span().with_label("here")],
                "{} must be a string", name
            }
        };

        Ok(Some((entry, Cow::Borrowed(s))))
    }
}

pub fn get_one_of<'a, 'k, const N: usize>(
    node: &'a KdlNode,
    node_name: impl Display,
    keys: [&'k str; N],
) -> miette::Result<Option<(&'k str, &'a KdlEntry, &'a str)>> {
    let mut found = None;

    for e in node.entries() {
        let Some(name) = e.name() else {
            continue;
        };
        let name = name.value();

        if let Some(key) = keys.into_iter().find(|k| *k == name) {
            if found.is_some() {
                bail! {
                    labels = vec![node.span().with_label("In this node")],
                    "Only one of {:?} may be specified", keys
                }
            } else {
                let Some(s) = e.value().as_string() else {
                    bail! {
                        labels = vec![e.span().with_label("here")],
                        "{}.{} must be a string", node_name, key
                    }
                };

                found = Some((key, e, s))
            }
        }
    }

    Ok(found)
}
