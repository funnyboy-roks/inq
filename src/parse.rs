use std::{borrow::Cow, fmt::Display};

use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue, NodeKey};
use miette::bail;

use crate::util::WithLabel;

fn entry_value_as_string(entry: &KdlEntry) -> miette::Result<Cow<'_, str>> {
    let s: Cow<'_, _> = match entry.value() {
        KdlValue::String(s) => s.as_str().into(),
        KdlValue::Integer(n) => n.to_string().into(),
        KdlValue::Float(f) => f.to_string().into(),
        KdlValue::Bool(_) | KdlValue::Null => bail! {
            labels = vec![entry.span().with_label("here")],
            "Expected variable value to be a string or number."
        },
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
    let Some(entry) = unique_entry(node, key)? else {
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

/// Get a node and error if it is duplicated
pub fn unique_node<'a>(doc: &'a KdlDocument, key: &str) -> miette::Result<Option<&'a KdlNode>> {
    // check for key to exist
    let mut found = None;

    for n in doc.nodes() {
        if n.name().value() == key {
            if found.is_some() {
                bail! {
                    labels = vec![n.span().with_label("This node")],
                    "{} may only be specified once", key
                }
            } else {
                found = Some(n)
            }
        }
    }

    Ok(found)
}

/// Get an entry and error if it is duplicated
pub fn unique_entry(node: &KdlNode, key: impl Into<NodeKey>) -> miette::Result<Option<&KdlEntry>> {
    match key.into() {
        NodeKey::Key(key) => {
            // check for key to exist
            let mut found = None;

            for e in node.entries() {
                let Some(name) = e.name() else {
                    continue;
                };

                if name.value() == key.value() {
                    if found.is_some() {
                        bail! {
                            labels = vec![node.span().with_label("In this node")],
                            "{} may only be specified once", key
                        }
                    } else {
                        found = Some(e)
                    }
                }
            }

            Ok(found)
        }
        NodeKey::Index(idx) => Ok(node.entry(idx)), // numerical indicies are always unique
    }
}

/// Get an entry and error if it is duplicated or not specified
pub fn expect_entry<'a>(
    node: &'a KdlNode,
    key: impl Into<NodeKey>,
    message: &str,
) -> miette::Result<&'a KdlEntry> {
    let Some(limit) = unique_entry(node, key)? else {
        bail! {
            labels = vec![node.span().with_label("in this node")],
            "{}", message
        }
    };
    Ok(limit)
}
