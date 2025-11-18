use crate::{EmptyHeaders, EmptyQueries, HeaderWriteError, Version, version};
use core::iter::{self, Chain, Once};
use httparse::Header;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request<'a, T, Q, V> {
    path: Option<&'a str>,
    method: Method<'a>,
    headers: T,
    version: V,
    queries: Q,
}

impl<'a> Request<'a, EmptyHeaders<'a>, EmptyQueries<'a>, version::UNSPECIFIED> {
    pub fn new(method: Method<'a>) -> Self {
        Self {
            path: None,
            method,
            headers: EmptyHeaders::new(),
            version: version::UNSPECIFIED,
            queries: EmptyQueries::new(),
        }
    }

    pub fn get() -> Self {
        Self::new(Method::Get)
    }

    pub fn head() -> Self {
        Self::new(Method::Head)
    }

    pub fn post() -> Self {
        Self::new(Method::Post)
    }

    pub fn put() -> Self {
        Self::new(Method::Put)
    }

    pub fn delete() -> Self {
        Self::new(Method::Delete)
    }

    pub fn connect() -> Self {
        Self::new(Method::Connect)
    }

    pub fn options() -> Self {
        Self::new(Method::Options)
    }

    pub fn trace() -> Self {
        Self::new(Method::Trace)
    }

    pub fn patch() -> Self {
        Self::new(Method::Patch)
    }
}

impl<'a, T, Q, V> Request<'a, T, Q, V> {
    pub fn path(mut self, path: &'a str) -> Self {
        self.path = Some(path);
        self
    }

    pub fn version<V2>(self, version: V2) -> Request<'a, T, Q, V2> {
        let Self {
            path,
            headers,
            method,
            version: _,
            queries,
        } = self;

        Request {
            path,
            headers,
            method,
            version,
            queries,
        }
    }

    pub fn v1(self) -> Request<'a, T, Q, version::V1> {
        self.version(version::V1)
    }

    pub fn v1_1(self) -> Request<'a, T, Q, version::V1_1> {
        self.version(version::V1_1)
    }
}

impl<'a, T, Q, V> Request<'a, T, Q, V>
where
    T: Iterator<Item = Header<'a>>,
{
    pub fn header(
        self,
        name: &'a str,
        value: &'a [u8],
    ) -> Request<'a, Chain<T, Once<Header<'a>>>, Q, V> {
        let h = Header { name, value };
        self.headers(iter::once(h))
    }

    pub fn headers<H: Iterator<Item = Header<'a>>>(self, h: H) -> Request<'a, Chain<T, H>, Q, V> {
        let Self {
            path,
            headers,
            method,
            version,
            queries,
        } = self;

        let headers = headers.chain(h);

        Request {
            path,
            headers,
            method,
            version,
            queries,
        }
    }
}

pub struct Query<'a> {
    q: &'a str,
}

impl <'a> Query<'a> {
    pub fn new(query: &'a str) -> Self {
        Self {
            q: query,
        }
    }
}

impl<'a, T, Q, V> Request<'a, T, Q, V>
where
    Q: Iterator<Item = Query<'a>>,
{
    pub fn query(self, q: &'a str) -> Request<'a, T, Chain<Q, Once<Query<'a>>>, V> {
        let q = Query::new(q);
        self.queries(iter::once(q))
    }

    pub fn queries<Qs: Iterator<Item = Query<'a>>>(self, qs: Qs) -> Request<'a, T, Chain<Q, Qs>, V> {
        let Self {
            path,
            headers,
            method,
            version,
            queries,
        } = self;

        let queries = queries.chain(qs);

        Request {
            path,
            headers,
            method,
            version,
            queries,
        }

    }
}

impl<'a, T, Q, V> Request<'a, T, Q, V>
where
    T: Iterator<Item = Header<'a>>,
    Q: Iterator<Item = Query<'a>>,
    V: Version<'a>,
{
    pub fn write_to<W: std::io::Write>(&mut self, w: &mut W) -> Result<usize, RequestWriteError> {
        use fluent_uri::encoding::{EStr, encoder::Path};

        let version = self.version.as_str();

        if version.len() != 3
            || !version
                .as_bytes()
                .iter()
                .any(|ch| ch.is_ascii_digit() || matches!(ch, b'.'))
        {
            return Err(RequestWriteError::InvalidVersion);
        }

        let path = if let Some(path) = self.path {
            let p = EStr::<Path>::new(path).ok_or(RequestWriteError::InvalidPath)?;

            if p.is_empty() {
                return Err(RequestWriteError::InvalidPath);
            }
            path
        } else {
            "/"
        };

        let method = self.method.as_str();
        write!(w, "{method} {path}")?;

        let queries = &mut self.queries;
        if let Some(q) = queries.next() {
            EStr::<fluent_uri::encoding::encoder::Query>::new(q.q).ok_or(RequestWriteError::InvalidQuery)?;
            write!(w, "?{}", q.q)?;
            while let Some(q) = queries.next() {
                EStr::<fluent_uri::encoding::encoder::Query>::new(q.q).ok_or(RequestWriteError::InvalidQuery)?;
                write!(w,"&{}", q.q)?;
            }
        }

        write!(w, " HTTP/{version}\r\n")?;

        let mut len = 9 + method.len() + path.len() + version.len();
        for header in &mut self.headers {
            len += crate::write_header(w, header).map_err(|e| (len, e))?;
        }

        write!(w, "\r\n")?;
        Ok(len + 2)
    }

    /// # Safety
    ///
    /// Caller must guarantee that all request fields are valid.
    pub unsafe fn write_to_unchecked<W: std::io::Write>(&mut self, w: &mut W) -> std::io::Result<usize> {
        let path = self.path.unwrap_or("/");
        let version = self.version.as_str();
        let method = self.method.as_str();

        write!(w, "{method} {path} HTTP/{version}\r\n")?;

        let mut len = 9 + method.len() + path.len() + version.len();

        for header in &mut self.headers {
            len += unsafe { crate::write_header_unchecked(w, header)? };
        }

        write!(w, "\r\n")?;
        Ok(len + 2)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Method<'a> {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
    Patch,
    Custom(&'a str),
}

impl<'a> Method<'a> {
    fn as_str(&self) -> &'a str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Connect => "CONNECT",
            Self::Options => "OPTIONS",
            Self::Trace => "TRACE",
            Self::Patch => "PATCH",
            Self::Custom(c) => c,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RequestWriteError {
    InvalidVersion,
    InvalidPath,
    InvalidQuery,
    InvalidHeader {
        buffer_offset: usize,
        err: HeaderWriteError,
    },
    Io,
}

impl From<(usize, HeaderWriteError)> for RequestWriteError {
    fn from((buffer_offset, err): (usize, HeaderWriteError)) -> RequestWriteError {
        RequestWriteError::InvalidHeader { buffer_offset, err }
    }
}

impl From<std::io::Error> for RequestWriteError {
    fn from(_: std::io::Error) -> RequestWriteError {
        RequestWriteError::Io
    }
}

#[test]
fn request() {
    let mut buf = Vec::new();

    let mut req = Request::new(Method::Get)
        .v1_1()
        .header("a", b"1")
        .header("b", b"2")
        .header("c", b"3")
    ;

    req.write_to(&mut buf).unwrap();

    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut preq = httparse::Request::new(&mut headers);
    
    assert!(preq.parse(&buf).unwrap().is_complete());
    assert_eq!(preq.headers.len(), 3);
}

#[test]
fn request_with_query() {
    let mut buf = Vec::new();

    let mut req = Request::new(Method::Get)
        .v1_1()
        .header("a", b"1")
        .header("b", b"2")
        .header("c", b"3")
        .path("abc")
        .query("a=b")
        .query("b=c")
    ;

    req.write_to(&mut buf).unwrap();

    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut preq = httparse::Request::new(&mut headers);
    
    assert!(preq.parse(&buf).unwrap().is_complete());
    assert_eq!(preq.headers.len(), 3);

    let path = preq.path.unwrap();

    use fluent_uri::encoding::{EStr, encoder::Path};
    let query_pos = path.find(|ch| ch == '?').unwrap();
    let (path, query) = path.split_at(query_pos);
    let p = EStr::<Path>::new(path).unwrap();
    let q = EStr::<fluent_uri::encoding::encoder::Query>::new(query).unwrap();

    assert_eq!(p.as_str(), "abc");
    assert_eq!(q.as_str(), "?a=b&b=c");
}
