use crate::{EmptyHeaders, HeaderWriteError, Version, version};
use core::iter::{self, Chain, Once};
use httparse::Header;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request<'a, T, V> {
    path: Option<&'a str>,
    method: Method<'a>,
    headers: T,
    version: V,
}

impl<'a> Request<'a, EmptyHeaders<'a>, version::UNSPECIFIED> {
    pub fn new(method: Method<'a>) -> Self {
        Self {
            path: None,
            method,
            headers: EmptyHeaders::new(),
            version: version::UNSPECIFIED,
        }
    }
}

impl<'a, T, V> Request<'a, T, V> {
    pub fn path(mut self, path: &'a str) -> Self {
        self.path = Some(path);
        self
    }

    pub fn version<V2>(self, version: V2) -> Request<'a, T, V2> {
        let Self {
            path,
            headers,
            method,
            version: _,
        } = self;

        Request {
            path,
            headers,
            method,
            version,
        }
    }

    pub fn v1(self) -> Request<'a, T, version::V1> {
        self.version(version::V1)
    }

    pub fn v1_1(self) -> Request<'a, T, version::V1_1> {
        self.version(version::V1_1)
    }
}

impl<'a, T, V> Request<'a, T, V>
where
    T: Iterator<Item = Header<'a>>,
{
    pub fn header(
        self,
        name: &'a str,
        value: &'a [u8],
    ) -> Request<'a, Chain<T, Once<Header<'a>>>, V> {
        let h = Header { name, value };
        self.headers(iter::once(h))
    }

    pub fn headers<H: Iterator<Item = Header<'a>>>(self, h: H) -> Request<'a, Chain<T, H>, V> {
        let Self {
            path,
            headers,
            method,
            version,
        } = self;

        let headers = headers.chain(h);

        Request {
            path,
            headers,
            method,
            version,
        }
    }
}

impl<'a, T, V> Request<'a, T, V>
where
    T: Iterator<Item = Header<'a>>,
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
        write!(w, "{method} {path} HTTP/{version}\r\n").unwrap();

        let mut len = 9 + method.len() + path.len() + version.len();
        for header in &mut self.headers {
            len += crate::write_header(w, header).map_err(|e| (len, e))?;
        }

        write!(w, "\r\n").unwrap();
        Ok(len + 2)
    }

    /// # Safety
    ///
    /// Caller must guarantee that all request fields are valid.
    pub unsafe fn write_to_unchecked<W: std::io::Write>(&mut self, w: &mut W) -> usize {
        let path = self.path.unwrap_or("/");
        let version = self.version.as_str();
        let method = self.method.as_str();

        write!(w, "{method} {path} HTTP/{version}\r\n").unwrap();

        let mut len = 9 + method.len() + path.len() + version.len();

        for header in &mut self.headers {
            len += unsafe { crate::write_header_unchecked(w, header) };
        }

        write!(w, "\r\n").unwrap();
        len + 2
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
    InvalidHeader {
        buffer_offset: usize,
        err: HeaderWriteError,
    },
}

impl From<(usize, HeaderWriteError)> for RequestWriteError {
    fn from((buffer_offset, err): (usize, HeaderWriteError)) -> RequestWriteError {
        RequestWriteError::InvalidHeader { buffer_offset, err }
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
