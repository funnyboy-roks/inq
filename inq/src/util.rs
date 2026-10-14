use ureq::http;

pub(crate) trait ToReqwest {
    type Target;

    fn to_reqwest(self) -> Self::Target;
}

impl ToReqwest for inq_lang::Method {
    type Target = ureq::http::Method;

    fn to_reqwest(self) -> Self::Target {
        match self {
            inq_lang::Method::Get => http::Method::GET,
            inq_lang::Method::Head => http::Method::HEAD,
            inq_lang::Method::Post => http::Method::POST,
            inq_lang::Method::Put => http::Method::PUT,
            inq_lang::Method::Delete => http::Method::DELETE,
            inq_lang::Method::Options => http::Method::OPTIONS,
            inq_lang::Method::Trace => http::Method::TRACE,
            inq_lang::Method::Patch => http::Method::PATCH,
        }
    }
}
