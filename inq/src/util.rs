use std::borrow::Cow;

pub const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

pub fn to_lowercase(s: &str) -> Cow<'_, str> {
    if s.chars().all(char::is_lowercase) {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(s.to_lowercase())
    }
}

pub(crate) trait ToReqwest {
    type Target;

    fn to_reqwest(self) -> Self::Target;
}

impl ToReqwest for inq_lang::Method {
    type Target = reqwest::Method;

    fn to_reqwest(self) -> Self::Target {
        match self {
            inq_lang::Method::Get => reqwest::Method::GET,
            inq_lang::Method::Head => reqwest::Method::HEAD,
            inq_lang::Method::Post => reqwest::Method::POST,
            inq_lang::Method::Put => reqwest::Method::PUT,
            inq_lang::Method::Delete => reqwest::Method::DELETE,
            inq_lang::Method::Options => reqwest::Method::OPTIONS,
            inq_lang::Method::Trace => reqwest::Method::TRACE,
            inq_lang::Method::Patch => reqwest::Method::PATCH,
        }
    }
}
