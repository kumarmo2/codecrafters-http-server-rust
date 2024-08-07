#![allow(dead_code, unused_assignments)]
pub(crate) mod http_request;
use std::io::Write;

#[derive(Debug)]
pub(crate) struct Header {}

pub(crate) struct HttpResponse {
    status_code: u16,
    header: Option<Header>,
    body: Option<Vec<u8>>, // TODO: see if we can replace body with some type which doesn't need to
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
        let mut buf: Vec<u8> = vec![0; 512];
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

        if self.header.is_some() {
            // TODO: write the headers to the buf.
        }
        buf[bytes_written_to_buf..(bytes_written_to_buf + 2)].copy_from_slice(b"\r\n"); // 2 bytes
        bytes_written_to_buf += 2;
        if self.body.is_some() {
            // TODO: write the body to the buf.
        }
        buf[bytes_written_to_buf..(bytes_written_to_buf + 2)].copy_from_slice(b"\r\n"); // 2 bytes
        bytes_written_to_buf += 2;
        // println!("buf: {:?}", buf);
        writer.write_all(&buf[0..bytes_written_to_buf])?;
        Ok(())
    }
}
