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

impl HttpResponse {
    pub(crate) fn write<W>(&self, writer: &mut W) -> anyhow::Result<()>
    where
        W: Write,
    {
        let mut buf: [u8; 512] = [0; 512];
        let mut bytes_written_to_buf: usize = 0;
        let b = b"HTTP/1.1 ";
        // println!("b: {:?}", b);
        buf[0..9].copy_from_slice(b); // 9 bytes.
        bytes_written_to_buf += 9;
        buf[bytes_written_to_buf..(bytes_written_to_buf + 3)]
            .copy_from_slice(self.status_code.to_string().as_bytes()); // TODO: how can i removed
                                                                       // this string allocation ?
        bytes_written_to_buf += 3;
        // TODO: before we write to the buf, we need to check if it has been filled. In case yes,
        // we need to write that to the writer first and then start filling it again.
        buf[bytes_written_to_buf..(bytes_written_to_buf + 3)].copy_from_slice(b" OK"); // TODO: `OK` string should be
                                                                                       // replaced accordingly to the response code.
        bytes_written_to_buf += 3;
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
