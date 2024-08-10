#![allow(unused_variables)]
use crate::http::HttpError;
use anyhow::Context;
use bytes::{BufMut, Bytes, BytesMut};
use core::str;
use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_until},
    character::complete::space0,
    combinator::value,
    sequence::terminated,
    Err, IResult,
};
use std::{
    io::{BufRead, BufReader, Read},
    net::TcpStream,
    str::FromStr,
};

use super::{Headers, HeadersV2, EIGHT_KB_IN_BYTES};

#[derive(Clone, Debug)]
pub(crate) enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl Method {}

pub(crate) struct HttpRequest {
    pub(crate) method: Method,
    pub(crate) path: String,
    pub(crate) headers: Option<Headers>,
    pub(crate) body: Option<Vec<u8>>,
}

pub(crate) struct HttpRequestV2 {
    pub(crate) method: Method,
    pub(crate) path: Bytes,
    pub(crate) headers: Option<HeadersV2>,
    pub(crate) body: Option<Bytes>,
}

fn parse_method(input: &[u8]) -> IResult<&[u8], Method> {
    terminated(
        alt((
            value(Method::Get, tag_no_case(b"get")),
            value(Method::Post, tag_no_case(b"post")),
            value(Method::Put, tag_no_case(b"put")),
            value(Method::Delete, tag_no_case(b"delete")),
            value(Method::Patch, tag_no_case(b"patch")),
        )),
        tag(" "),
    )(input)
}

fn skip_whitespaces0(input: &[u8]) -> IResult<&[u8], &[u8]> {
    space0(input)
}

fn capture_all_till_and_including_space(input: &[u8]) -> IResult<&[u8], &[u8]> {
    terminated(take_until(" "), tag(" "))(input)
}

fn capture_all_till_and_including_crlf(input: &[u8]) -> IResult<&[u8], &[u8]> {
    terminated(take_until("\r\n"), tag("\r\n"))(input)
}
fn capture_all_till_and_including_termination_character<'a>(
    input: &'a [u8],
    termination_bytes: &'a [u8],
) -> IResult<&'a [u8], &'a [u8]> {
    terminated(take_until(termination_bytes), tag(termination_bytes))(input)
}

impl HttpRequestV2 {
    fn parse_method(
        buf: &[u8],
        curr_buff_offset: usize,
        _total_bytes_read: usize,
    ) -> Result<(&[u8], Method, usize, usize), HttpError> {
        parse_method(&buf[curr_buff_offset..])
            .map_err(|_| HttpError::HttpVersionParseError)
            .and_then(|(rest, method)| {
                let curr_buff_offset = buf.len() - rest.len();
                let _total_bytes_read = curr_buff_offset;
                Ok((rest, method, curr_buff_offset, _total_bytes_read))
            })
    }

    fn skip_whitespaces0(
        buf: &[u8],
        curr_buff_offset: usize,
        _total_bytes_read: usize,
    ) -> Result<(&[u8], usize, usize), HttpError> {
        skip_whitespaces0(buf)
            .map_err(|_| HttpError::RequestParsingError("error while skipping spaces"))
            .and_then(|(rest, _)| {
                let curr_buff_offset = buf.len() - rest.len();
                let _total_bytes_read = curr_buff_offset;
                Ok((rest, curr_buff_offset, _total_bytes_read))
            })
    }
    fn parse_http_path(
        buf: &[u8],
        curr_buff_offset: usize,
        _total_bytes_read: usize,
    ) -> Result<(&[u8], &[u8], usize, usize), HttpError> {
        capture_all_till_and_including_space(buf)
            .map_err(|_| HttpError::RequestParsingError("error while parsing path"))
            .and_then(|(rest, path_bytes)| {
                let curr_buff_offset = buf.len() - rest.len();
                let _total_bytes_read = curr_buff_offset;
                Ok((rest, path_bytes, curr_buff_offset, _total_bytes_read))
            })
    }
    fn parse_http_version(
        buf: &[u8],
        curr_buff_offset: usize,
        total_bytes_read: usize,
    ) -> Result<(&[u8], &[u8], usize, usize), HttpError> {
        capture_all_till_and_including_crlf(buf)
            .map_err(|_| HttpError::RequestParsingError("error while capturing all till crlf"))
            .and_then(|(rest, version)| {
                let curr_buff_offset = buf.len() - rest.len();
                let _total_bytes_read = curr_buff_offset;
                Ok((rest, version, curr_buff_offset, _total_bytes_read))
            })
    }

    pub(crate) fn create_from_tcp_stream(
        stream: &mut TcpStream,
    ) -> Result<HttpRequestV2, HttpError> {
        let mut reader = BufReader::with_capacity(EIGHT_KB_IN_BYTES, stream);
        let mut bytes = BytesMut::with_capacity(EIGHT_KB_IN_BYTES);
        let buf = reader.fill_buf().map_err(|e| HttpError::IoErr(e))?;

        let (rest, method, curr_buff_offset, total_bytes_read) =
            HttpRequestV2::parse_method(buf, 0, 0)?;

        let (rest, curr_buff_offset, total_bytes_read) =
            HttpRequestV2::skip_whitespaces0(rest, curr_buff_offset, total_bytes_read)?;

        let (rest, path_bytes, curr_buff_offset, total_bytes_read) =
            HttpRequestV2::parse_http_path(rest, curr_buff_offset, total_bytes_read)?;

        bytes.put(path_bytes);
        let path = bytes.split();

        // Validate that the path is valid UTF-8.
        if let Err(err) = std::str::from_utf8(path.as_ref()) {
            return Err(HttpError::Utf8Error(err));
        }

        let (rest, curr_buff_offset, total_bytes_read) =
            HttpRequestV2::skip_whitespaces0(rest, curr_buff_offset, total_bytes_read)?;

        let (rest, _version, mut curr_buff_offset, mut total_bytes_read) =
            HttpRequestV2::parse_http_version(rest, curr_buff_offset, total_bytes_read)?;

        let mut headers = HeadersV2::new();

        let mut bytes_read_from_header: usize = 0;
        loop {
            // TODO: refactor the header parsing.
            let (loop_rest, captured_header) =
                capture_all_till_and_including_crlf(&rest[bytes_read_from_header..])
                    .map_err(|e| HttpError::Adhoc("error while capturing all till crlf"))?;

            bytes_read_from_header = rest.len() - loop_rest.len();
            curr_buff_offset = buf.len() - loop_rest.len();
            total_bytes_read += curr_buff_offset;

            if captured_header.is_empty() {
                break;
            }
            if captured_header.len() == 2 {
                // Found just the crlf
                break;
            }
            let (header_rest, key_bytes) =
                capture_all_till_and_including_termination_character(captured_header, b":")
                    .map_err(|e| {
                        HttpError::RequestParsingError("error while capturing all till crlf")
                    })?;

            let (header_rest, _) = skip_whitespaces0(header_rest).map_err(|_| {
                HttpError::RequestParsingError("error while skipping skip_whitespaces0")
            })?;
            let value_bytes = header_rest;

            if std::str::from_utf8(key_bytes).is_err() {
                return Err(HttpError::RequestParsingError("invalid header key."));
            }
            if std::str::from_utf8(value_bytes).is_err() {
                return Err(HttpError::RequestParsingError("invalid header val."));
            }

            bytes.put(key_bytes);
            let key = bytes.split();
            bytes.put(value_bytes);
            let val = bytes.split();

            headers.insert(key.into(), val.into());
        }
        let rest = &rest[bytes_read_from_header..];
        let mut request_body_bytes_to_read: usize = 0;

        #[allow(clippy::unwrap_used)] // NOTE: We have already made sure that the `keys` and
        // `values` in the header are UTF-8 safe.
        let headers = if headers.keys().len() > 0 {
            // NOTE: only if `Content-Length` header is present, we will read the body.
            if let Some(val_bytes) = headers.get(&b"Content-Length"[..]) {
                request_body_bytes_to_read = std::str::from_utf8(val_bytes)
                    .unwrap()
                    .parse::<usize>()
                    .map_err(|_| HttpError::InvalidContentLengthInRequest)?;
            }
            Some(headers)
        } else {
            None
        };

        if request_body_bytes_to_read == 0 {
            return Ok(Self {
                method,
                path: path.into(),
                headers,
                body: None,
            });
        }
        if rest.len() < request_body_bytes_to_read {
            // FIXME: instead of panicing from here, return an error.
            todo!(" need to read further bytes from the stream to fill the body, rest_len: {rest_len}, request_body_bytes_to_read: {request_body_bytes_to_read}", rest_len = rest.len());
        }
        let body = &rest[0..request_body_bytes_to_read];
        bytes.put(body);
        let body = bytes.split();

        Ok(Self {
            method,
            path: path.into(),
            headers,
            body: Some(body.into()),
        })
    }
}

impl HttpRequest {
    pub(crate) fn create_from_tcp_stream(stream: &mut TcpStream) -> Result<HttpRequest, HttpError> {
        let mut reader = BufReader::new(stream);
        let buf = reader.fill_buf().map_err(|e| HttpError::IoErr(e))?;
        let mut _total_bytes_read: usize = 0;
        let mut curr_buff_offset: usize = 0;

        let (rest, method) = match parse_method(&buf[curr_buff_offset..]) {
            Ok(r) => r,
            Err(_) => return Err(HttpError::HttpVersionParseError),
        };

        curr_buff_offset = buf.len() - rest.len();
        _total_bytes_read += curr_buff_offset;

        let (rest, _) = skip_whitespaces0(rest)
            .map_err(|e| HttpError::RequestParsingError("error while scaping spaces"))?;

        curr_buff_offset = buf.len() - rest.len();
        _total_bytes_read += curr_buff_offset;

        let (rest, path_bytes) = capture_all_till_and_including_space(rest)
            .map_err(|e| HttpError::RequestParsingError("error while parsing path"))?;

        curr_buff_offset = buf.len() - rest.len();
        _total_bytes_read += curr_buff_offset;

        let path =
            String::from_str(str::from_utf8(path_bytes).map_err(|e| HttpError::Utf8Error(e))?)
                .map_err(|e| HttpError::Adhoc("unreachable"))?;

        let (rest, _) = skip_whitespaces0(rest)
            .map_err(|e| HttpError::Adhoc("error while skip_whitespaces"))?;

        curr_buff_offset = buf.len() - rest.len();
        _total_bytes_read += curr_buff_offset;

        let (rest, _http_version) = capture_all_till_and_including_crlf(rest)
            .map_err(|e| HttpError::Adhoc("error while capturing all till crlf"))?;

        curr_buff_offset = buf.len() - rest.len();
        _total_bytes_read += curr_buff_offset;

        let mut headers = Headers::new();

        let mut bytes_read_from_header: usize = 0;
        loop {
            let (loop_rest, captured_header) =
                capture_all_till_and_including_crlf(&rest[bytes_read_from_header..])
                    .map_err(|e| HttpError::Adhoc("error while capturing all till crlf"))?;

            bytes_read_from_header = rest.len() - loop_rest.len();
            curr_buff_offset = buf.len() - loop_rest.len();
            _total_bytes_read += curr_buff_offset;
            if captured_header.is_empty() {
                break;
            }
            if captured_header.len() == 2 {
                // Found just the crlf
                break;
            }
            let (header_rest, key_bytes) =
                capture_all_till_and_including_termination_character(captured_header, b":")
                    .map_err(|e| {
                        HttpError::RequestParsingError("error while capturing all till crlf")
                    })?;
            let (header_rest, _) = skip_whitespaces0(header_rest).map_err(|_| {
                HttpError::RequestParsingError("error while skipping skip_whitespaces0")
            })?;
            let value_bytes = header_rest;
            let key = String::from_str(
                str::from_utf8(key_bytes)
                    .map_err(|e| HttpError::Adhoc("error while parsing header key"))?,
            )
            .map_err(|e| HttpError::Adhoc("Infalling"))?;
            let val = String::from_str(
                str::from_utf8(value_bytes)
                    .map_err(|e| HttpError::Adhoc("error while parsing header value"))?,
            )
            .map_err(|e| HttpError::Adhoc("Infalling"))?;
            headers.insert(key, val);
        }
        let rest = &rest[bytes_read_from_header..];
        let mut request_body_bytes_to_read: usize = 0;

        let headers = if headers.keys().len() > 0 {
            // NOTE: only if `Content-Length` header is present, we will read the body.
            if let Some(val_str) = headers.get("Content-Length") {
                request_body_bytes_to_read = val_str
                    .parse::<usize>()
                    .map_err(|_| HttpError::InvalidContentLengthInRequest)?;
            }
            Some(headers)
        } else {
            None
        };

        if request_body_bytes_to_read == 0 {
            return Ok(Self {
                method,
                path,
                headers,
                body: None,
            });
        }

        if rest.len() < request_body_bytes_to_read {
            // FIXME: instead of panicing from here, return an error.
            todo!(" need to read further bytes from the stream to fill the body, rest_len: {rest_len}, request_body_bytes_to_read: {request_body_bytes_to_read}", rest_len = rest.len());
        }
        let body = rest[0..request_body_bytes_to_read].to_vec();
        Ok(Self {
            method,
            path,
            headers,
            body: Some(body),
        })
    }
}
