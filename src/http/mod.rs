#![allow(dead_code, unused_assignments)]
pub(crate) mod http_request;
use std::{
    collections::HashMap,
    io::Write,
    ops::{Deref, DerefMut},
};
use thiserror::Error;

pub(crate) const ACCEPT_ENCODING_HEADER: &'static str = "Accept-Encoding";
pub(crate) const CONTENT_ENCODING_HEADER: &'static str = "Content-Encoding";
pub(crate) const SUPPORTED_ENCODINGS: [&'static str; 1] = ["gzip"];

#[derive(Error, Debug)]
pub(crate) enum HttpError {
    #[error("http request version parsing")]
    HttpVersionParseError,
    #[error("Inner Error")]
    Unknown(&'static str),
    #[error("io error")]
    IoErr(std::io::Error),
    #[error("Utf8Error")]
    Utf8Error(std::str::Utf8Error),
    #[error("error parsing request")]
    RequestParsingError(&'static str),
}

#[derive(Debug)]
pub(crate) struct Headers {
    map: HashMap<String, String>,
}

impl Headers {
    pub(crate) fn new() -> Self {
        Self {
            map: HashMap::default(),
        }
    }
}

impl Deref for Headers {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl DerefMut for Headers {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

pub(crate) enum ContentTypeHttpResponse {
    Json(HttpResponse),
    PlainText(HttpResponse),
    NoBody(HttpResponse),
    File(HttpResponse),
}

impl ContentTypeHttpResponse {
    pub(crate) fn get_content_type_header_value(&self) -> Option<&'static str> {
        match self {
            ContentTypeHttpResponse::Json(_) => Some("application/json"),
            ContentTypeHttpResponse::PlainText(_) => Some("text/plain"),
            ContentTypeHttpResponse::NoBody(_) => None,
            ContentTypeHttpResponse::File(_) => Some("application/octet-stream"),
        }
    }
    pub(crate) fn into_inner(self) -> HttpResponse {
        match self {
            ContentTypeHttpResponse::Json(response) => response,
            ContentTypeHttpResponse::PlainText(response) => response,
            ContentTypeHttpResponse::NoBody(response) => response,
            ContentTypeHttpResponse::File(response) => response,
        }
    }
}

#[derive(Debug)]
pub(crate) struct HttpResponse {
    status_code: u16,
    pub(crate) header: Option<Headers>,
    pub(crate) body: Option<Vec<u8>>, // TODO: see if we can replace body with some type which doesn't need to
                                      // allocate memory on heap.
}

impl Default for HttpResponse {
    fn default() -> Self {
        Self {
            status_code: 200,
            header: None,
            body: None,
        }
    }
}

pub(crate) struct HttpResponseBuilder {
    status_code: u16,
    header: Option<Headers>,
    body: Option<Vec<u8>>, // TODO: see if we can replace body with some type which doesn't need to
}

impl HttpResponseBuilder {
    pub(crate) fn new(code: u16) -> Self {
        Self {
            status_code: code,
            header: None,
            body: None,
        }
    }

    pub(crate) fn with_header(mut self, header: Headers) -> Self {
        self.header = Some(header);
        self
    }

    pub(crate) fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }

    pub(crate) fn build(self) -> HttpResponse {
        HttpResponse {
            status_code: self.status_code,
            header: self.header,
            body: self.body,
        }
    }
}

impl HttpResponse {
    fn get_http_method_contents_to_write(status_code: u16) -> (&'static str, &'static str) {
        match status_code {
            200 => ("200", " OK"),
            201 => ("201", " Created"),
            404 => ("404", " Not Found"),
            500 => ("500", " Internal Server Error"),
            x => unimplemented!("unhandled status_code: {x}"),
        }
    }

    fn copy_to_buf(buf: &mut Vec<u8>, from: &[u8], buf_offset: usize) -> usize {
        let bytes_to_copy = from.len();
        let end = buf_offset + bytes_to_copy;
        buf[buf_offset..end].copy_from_slice(from);
        bytes_to_copy
    }

    pub(crate) fn write<W>(&self, writer: &mut W) -> anyhow::Result<()>
    where
        W: Write,
    {
        let mut buf: Vec<u8> = vec![0; 8196];
        let mut bytes_written_to_buf: usize = 0;
        let b = b"HTTP/1.1 ";
        buf[0..9].copy_from_slice(b); // 9 bytes.
        bytes_written_to_buf += 9;

        let (status_code, method_readable_string) =
            HttpResponse::get_http_method_contents_to_write(self.status_code);

        // TODO: before writing the bytes, make sure buf has enough space.
        bytes_written_to_buf +=
            HttpResponse::copy_to_buf(&mut buf, status_code.as_bytes(), bytes_written_to_buf);

        bytes_written_to_buf += HttpResponse::copy_to_buf(
            &mut buf,
            method_readable_string.as_bytes(),
            bytes_written_to_buf,
        );
        // buf[bytes_written_to_buf..(bytes_written_to_buf + method_readable_string.len())]
        //     .copy_from_slice(method_readable_string.as_bytes());
        // bytes_written_to_buf += method_readable_string.len();

        bytes_written_to_buf += HttpResponse::copy_to_buf(&mut buf, b"\r\n", bytes_written_to_buf);
        // buf[bytes_written_to_buf..(bytes_written_to_buf + 2)].copy_from_slice(b"\r\n"); // 2 bytes
        // bytes_written_to_buf += 2;

        match self.header.as_ref() {
            Some(header) => {
                for (k, v) in header.iter() {
                    let k_bytes = k.as_bytes();
                    let v_bytes = v.as_bytes();

                    bytes_written_to_buf +=
                        HttpResponse::copy_to_buf(&mut buf, k_bytes, bytes_written_to_buf);

                    bytes_written_to_buf +=
                        HttpResponse::copy_to_buf(&mut buf, b": ", bytes_written_to_buf);

                    bytes_written_to_buf +=
                        HttpResponse::copy_to_buf(&mut buf, v_bytes, bytes_written_to_buf);

                    bytes_written_to_buf +=
                        HttpResponse::copy_to_buf(&mut buf, b"\r\n", bytes_written_to_buf);
                }
            }
            None => {}
        }
        bytes_written_to_buf += HttpResponse::copy_to_buf(&mut buf, b"\r\n", bytes_written_to_buf);

        if self.body.is_some() {
            match &self.body {
                Some(body) => {
                    bytes_written_to_buf +=
                        HttpResponse::copy_to_buf(&mut buf, body, bytes_written_to_buf);
                }
                None => {}
            }
        }
        writer.write_all(&buf[0..bytes_written_to_buf])?;
        Ok(())
    }
}
