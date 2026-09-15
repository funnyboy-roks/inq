use std::{borrow::Borrow, cell::RefCell, collections::HashSet, ops::Deref, sync::Arc};

use derive_more::Display;

thread_local! {
    static STRINGS: RefCell<HashSet<Arc<str>>> = RefCell::new(HashSet::new());
}

/// Interned String
#[derive(derive_more::Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Display)]
#[display("{}", _0)]
#[debug("{:?}", _0)]
pub struct IStr(Arc<str>);

impl IStr {
    fn new(s: &str) -> Self {
        let s = STRINGS.with_borrow_mut(|x| {
            if let Some(s) = x.get(s) {
                s.clone()
            } else {
                let s: Arc<str> = s.into();
                let ret = s.clone();
                x.insert(s);
                ret
            }
        });

        Self(s)
    }

    fn from_owned(s: String) -> Self {
        let s = STRINGS.with_borrow_mut(|x| {
            if let Some(s) = x.get(&*s) {
                s.clone()
            } else {
                let s: Arc<str> = s.into();
                let ret = s.clone();
                x.insert(s);
                ret
            }
        });

        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for IStr {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl PartialEq<&str> for IStr {
    fn eq(&self, other: &&str) -> bool {
        self.as_ref().eq(*other)
    }
}

impl PartialEq<String> for IStr {
    fn eq(&self, other: &String) -> bool {
        self.as_ref().eq(other)
    }
}

impl Drop for IStr {
    fn drop(&mut self) {
        if Arc::strong_count(&self.0) <= 2 {
            STRINGS.with_borrow_mut(|s| {
                s.remove(&self.0);
            })
        }
    }
}

impl AsRef<str> for IStr {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<IStr> for String {
    fn from(value: IStr) -> Self {
        value.as_ref().into()
    }
}

impl From<&IStr> for String {
    fn from(value: &IStr) -> Self {
        value.as_ref().into()
    }
}

impl From<&str> for IStr {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<&IStr> for IStr {
    fn from(value: &IStr) -> Self {
        value.clone()
    }
}

impl From<char> for IStr {
    fn from(value: char) -> Self {
        Self::from_owned(format!("{}", value))
    }
}

impl From<String> for IStr {
    fn from(value: String) -> Self {
        Self::from_owned(value)
    }
}

impl Borrow<str> for IStr {
    fn borrow(&self) -> &str {
        self.as_ref()
    }
}

#[cfg(test)]
mod test {
    use super::{IStr, STRINGS};

    #[test]
    fn global() {
        let s = IStr::from("hello");
        assert_eq!(STRINGS.with_borrow(|s| s.len()), 1);
        let a = IStr::from("hello");
        let b = IStr::from("hello");
        let c = IStr::from("hello");
        assert_eq!(STRINGS.with_borrow(|s| s.len()), 1);
        assert_eq!(s, "hello");
        assert_eq!(a, "hello");
        assert_eq!(b, "hello");
        assert_eq!(c, "hello");

        let s2 = IStr::from("hello2");
        assert_eq!(STRINGS.with_borrow(|s| s.len()), 2);

        drop(a);
        assert_eq!(STRINGS.with_borrow(|s| s.len()), 2);
        drop(b);
        assert_eq!(STRINGS.with_borrow(|s| s.len()), 2);
        drop(c);
        assert_eq!(STRINGS.with_borrow(|s| s.len()), 2);
        drop(s);
        assert_eq!(STRINGS.with_borrow(|s| s.len()), 1);
        drop(s2);
        assert_eq!(STRINGS.with_borrow(|s| s.len()), 0);
    }
}
