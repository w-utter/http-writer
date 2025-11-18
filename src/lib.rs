mod response;
pub use http::StatusCode;
pub use response::{Response, ResponseWriteError};
mod request;
pub use request::{Method, Request, RequestWriteError};
pub mod version;
use core::marker::PhantomData;
pub use httparse::Header;
pub use version::Version;

#[derive(Debug, PartialEq, Eq)]
pub enum HeaderWriteError {
    InvalidName(usize),
    InvalidValue(usize),
    Io,
}

impl From<std::io::Error> for HeaderWriteError {
    fn from(_: std::io::Error) -> HeaderWriteError {
        HeaderWriteError::Io
    }
}

pub(crate) fn write_header<W: std::io::Write>(
    w: &mut W,
    header: Header<'_>,
) -> Result<usize, HeaderWriteError> {
    if let Some(pos) = header
        .name
        .as_bytes()
        .iter()
        .position(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, b'-' | b'_')))
    {
        return Err(HeaderWriteError::InvalidName(pos));
    } else if let Some(pos) = header
        .value
        .iter()
        .position(|ch| matches!(ch, b'\r' | b'\n' | b'\0'))
    {
        return Err(HeaderWriteError::InvalidValue(pos));
    }
    // SAFETY: header is valid
    Ok(unsafe { write_header_unchecked(w, header)? })
}

pub(crate) unsafe fn write_header_unchecked<W: std::io::Write>(
    w: &mut W,
    header: Header<'_>,
) -> std::io::Result<usize> {
    let mut len = 0;
    len += w.write(header.name.as_bytes())?;
    write!(w, ": ")?;
    len += 2;
    len += w.write(header.value)?;
    write!(w, "\r\n")?;
    len += 2;
    Ok(len)
}

pub struct EmptyHeaders<'a>(PhantomData<&'a ()>);

impl<'a> EmptyHeaders<'a> {
    fn new() -> Self {
        Self(PhantomData)
    }
}

impl<'a> Iterator for EmptyHeaders<'a> {
    type Item = Header<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

pub struct EmptyQueries<'a>(PhantomData<&'a ()>);

impl<'a> EmptyQueries<'a> {
    fn new() -> Self {
        Self(PhantomData)
    }
}

impl<'a> Iterator for EmptyQueries<'a> {
    type Item = crate::request::Query<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}
