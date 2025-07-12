use crate::{EmptyHeaders, HeaderWriteError, Version, version};
use core::iter::{self, Chain, Once};
use httparse::Header;

#[derive(Clone)]
pub struct Response<T, V> {
    version: V,
    code: http::StatusCode,
    headers: T,
}

impl<'a> Response<EmptyHeaders<'a>, version::UNSPECIFIED> {
    pub fn new(status_code: http::StatusCode) -> Self {
        Self {
            code: status_code,
            version: version::UNSPECIFIED,
            headers: EmptyHeaders::new(),
        }
    }
}

impl<T, V> Response<T, V> {
    pub fn version<V2>(self, version: V2) -> Response<T, V2> {
        let Self {
            code,
            headers,
            version: _,
        } = self;

        Response {
            code,
            headers,
            version,
        }
    }

    pub fn v1(self) -> Response<T, version::V1> {
        self.version(version::V1)
    }

    pub fn v1_1(self) -> Response<T, version::V1_1> {
        self.version(version::V1_1)
    }
}

impl<'a, T, V> Response<T, V>
where
    T: Iterator<Item = Header<'a>>,
{
    pub fn header(self, name: &'a str, value: &'a [u8]) -> Response<Chain<T, Once<Header<'a>>>, V> {
        let h = Header { name, value };
        self.headers(iter::once(h))
    }

    pub fn headers<H: Iterator<Item = Header<'a>>>(self, h: H) -> Response<Chain<T, H>, V> {
        let Self {
            code,
            headers,
            version,
        } = self;

        let headers = headers.chain(h);

        Response {
            code,
            headers,
            version,
        }
    }
}

impl<'a, T, V> Response<T, V>
where
    T: Iterator<Item = Header<'a>>,
    V: Version<'a>,
{
    pub fn write_to<W: std::io::Write>(&mut self, w: &mut W) -> Result<usize, ResponseWriteError> {
        let version = self.version.as_str();

        if version.len() != 3
            || !version
                .as_bytes()
                .iter()
                .any(|ch| ch.is_ascii_digit() || matches!(ch, b'.'))
        {
            return Err(ResponseWriteError::InvalidVersion);
        }

        let code = self.code.as_str();
        let reason = self.code.canonical_reason().unwrap_or_default();

        write!(w, "HTTP/{version} {code} {reason}\r\n").unwrap();

        let mut len = 9 + version.len() + code.len() + reason.len();

        for header in &mut self.headers {
            len += crate::write_header(w, header).map_err(|e| (len, e))?;
        }

        write!(w, "\r\n").unwrap();
        Ok(len + 2)
    }

    /// # Safety
    ///
    /// Caller must guarantee that all response fields are valid.
    pub unsafe fn write_to_unchecked<W: std::io::Write>(&mut self, w: &mut W) -> usize {
        let code = self.code.as_str();
        let reason = self.code.canonical_reason().unwrap_or_default();
        let version = self.version.as_str();

        write!(w, "HTTP/{version} {code} {reason}\r\n").unwrap();

        let mut len = 9 + version.len() + code.len() + reason.len();

        for header in &mut self.headers {
            len += unsafe { crate::write_header_unchecked(w, header) };
        }

        write!(w, "\r\n").unwrap();
        len + 2
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ResponseWriteError {
    InvalidVersion,
    InvalidHeader {
        buffer_offset: usize,
        err: HeaderWriteError,
    },
}

impl From<(usize, HeaderWriteError)> for ResponseWriteError {
    fn from((buffer_offset, err): (usize, HeaderWriteError)) -> ResponseWriteError {
        ResponseWriteError::InvalidHeader { buffer_offset, err }
    }
}

#[test]
fn response() {
    let mut res = Response::new(http::StatusCode::OK)
        .v1_1()
        .header("d", b"4")
        .header("e", b"5")
        .header("f", b"6")
    ;

    let mut buf = Vec::new();
    res.write_to(&mut buf).unwrap();
    
    let mut headers_2 = [httparse::EMPTY_HEADER; 64];
    let mut pres = httparse::Response::new(&mut headers_2);

    assert!(pres.parse(&buf).unwrap().is_complete());
    assert_eq!(pres.headers.len(), 3)
}
