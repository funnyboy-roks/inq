use std::fmt::{Debug, Display};

#[derive(Debug, Clone)]
pub(crate) struct DisplayList<D: Display>(pub(crate) Vec<D>);
impl<D: Display> Display for DisplayList<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self.0 {
            [] => write!(f, "token"),
            [a] => write!(f, "{a}"),
            [a, b] => write!(f, "one of {a} or {b}"),
            expected => {
                write!(f, "one of ")?;
                for (i, e) in expected.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    if i == expected.len() - 1 {
                        write!(f, "or ")?;
                    }
                    write!(f, "{e}")?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DisplayVec<T>(pub Vec<T>);
impl<T: Display> Display for DisplayVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (i, item) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", item)?;
        }
        write!(f, "]")
    }
}
