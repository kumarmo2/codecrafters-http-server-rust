#![allow(dead_code, unused_assignments)]
pub(crate) mod http_request;
use std::{
    collections::HashMap,
    io::Write,
    ops::{Deref, DerefMut},
};

#[derive(Debug)]
pub(crate) struct Header {
    map: HashMap<String, String>,
}

impl Header {
    pub(crate) fn new() -> Self {
        Self {
            map: HashMap::default(),
        }
    }
}

impl Deref for Header {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl DerefMut for Header {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

pub(crate) enum ContentTypeHttpResponse {
    Json(HttpResponse),
    PlainText(HttpResponse),
    NoBody(HttpResponse),
}

#[derive(Debug)]
pub(crate) struct HttpResponse {
    status_code: u16,
    pub(crate) header: Option<Header>,
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
    header: Option<Header>,
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

    pub(crate) fn with_header(mut self, header: Header) -> Self {
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
    fn get_http_method_contents_to_write(status_code: u16) -> (String, &'static str) {
        match status_code {
            200 => (200.to_string(), " OK"), //  TODO: if possible, remove the heap allocation for
            404 => (404.to_string(), " Not Found"),
            // the string
            x => unimplemented!("unhandled status_code: {x}"),
        }
    }

    pub(crate) fn write<W>(&self, writer: &mut W) -> anyhow::Result<()>
    where
        W: Write,
    {
        // let mut buf: [u8; 512] = [0; 512];
        let mut buf: Vec<u8> = vec![0; 8196];
        let mut bytes_written_to_buf: usize = 0;
        let b = b"HTTP/1.1 ";
        // println!("b: {:?}", b);
        buf[0..9].copy_from_slice(b); // 9 bytes.
        bytes_written_to_buf += 9;

        let (status_code, method_readable_string) =
            HttpResponse::get_http_method_contents_to_write(self.status_code);

        // TODO: before writing the bytes, make sure buf has enough space.
        buf[bytes_written_to_buf..(bytes_written_to_buf + status_code.len())]
            .copy_from_slice(status_code.as_bytes());
        bytes_written_to_buf += status_code.len();

        buf[bytes_written_to_buf..(bytes_written_to_buf + method_readable_string.len())]
            .copy_from_slice(method_readable_string.as_bytes());
        bytes_written_to_buf += method_readable_string.len();

        buf[bytes_written_to_buf..(bytes_written_to_buf + 2)].copy_from_slice(b"\r\n"); // 2 bytes
        bytes_written_to_buf += 2;

        match self.header.as_ref() {
            Some(header) => {
                for (k, v) in header.iter() {
                    let k_bytes = k.as_bytes();
                    let k_len = k_bytes.len();
                    let v_bytes = v.as_bytes();
                    let v_len = v_bytes.len();

                    buf[bytes_written_to_buf..(bytes_written_to_buf + k_len)]
                        .copy_from_slice(k_bytes);

                    bytes_written_to_buf += k_len;

                    // 2 bytes
                    buf[bytes_written_to_buf..(bytes_written_to_buf + 2)].copy_from_slice(b": ");

                    bytes_written_to_buf += 2;

                    buf[bytes_written_to_buf..(bytes_written_to_buf + v_len)]
                        .copy_from_slice(v_bytes);

                    bytes_written_to_buf += v_len;

                    buf[bytes_written_to_buf..(bytes_written_to_buf + 2)].copy_from_slice(b"\r\n"); // 2 bytes
                    bytes_written_to_buf += 2;
                }
            }
            None => {}
        }
        buf[bytes_written_to_buf..(bytes_written_to_buf + 2)].copy_from_slice(b"\r\n"); // 2 bytes
        bytes_written_to_buf += 2;
        if self.body.is_some() {
            match &self.body {
                Some(body) => {
                    println!("writing body");
                    let body_len = body.len();
                    buf[bytes_written_to_buf..(bytes_written_to_buf + body_len)]
                        .copy_from_slice(body);

                    bytes_written_to_buf += body_len;
                }
                None => {}
            }
        }
        writer.write_all(&buf[0..bytes_written_to_buf])?;
        Ok(())
    }
}
