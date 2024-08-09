#![allow(unused_variables)]
use anyhow::Context;
use core::str;
use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_until},
    character::complete::space0,
    combinator::value,
    sequence::terminated,
    IResult,
};
use std::{
    io::{BufRead, BufReader},
    net::TcpStream,
    str::FromStr,
};

use crate::http::HttpError;

use super::Headers;

#[derive(Clone, Debug)]
pub(crate) enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

impl Method {
    fn get_text_bytes(&self) -> usize {
        match *self {
            Method::Get => 3,
            Method::Post => 4,
            Method::Put => 3,
            Method::Delete => 6,
            Method::Patch => 5,
        }
    }
}

pub(crate) struct HttpRequest {
    pub(crate) method: Method,
    pub(crate) path: String,
    pub(crate) headers: Option<Headers>,
    pub(crate) body: Option<Vec<u8>>,
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

fn parse_path(input: &[u8]) -> IResult<&[u8], &[u8]> {
    // terminated(take_until(" "), take_while(|c| c == ' ' as u8))(input)
    terminated(take_until(" "), tag(" "))(input)
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

impl HttpRequest {
    pub(crate) fn create_from_tcp_stream<'a>(
        stream: &'a mut TcpStream,
    ) -> Result<HttpRequest, HttpError> {
        let mut reader = BufReader::new(stream);
        let buf = reader.fill_buf().map_err(|e| HttpError::IoErr(e))?;
        let mut _total_bytes_read: usize = 0;
        let mut bytes_read_from_curr_buff: usize = 0;

        let (rest, method) = match parse_method(&buf[bytes_read_from_curr_buff..]) {
            Ok(r) => r,
            Err(_) => return Err(HttpError::HttpVersionParseError),
        };

        // todo!();
        bytes_read_from_curr_buff = buf.len() - rest.len();
        _total_bytes_read += bytes_read_from_curr_buff;

        let (rest, _) = skip_whitespaces0(rest)
            .map_err(|e| HttpError::RequestParsingError("error while scaping spaces"))?;

        bytes_read_from_curr_buff = buf.len() - rest.len();
        _total_bytes_read += bytes_read_from_curr_buff;

        let (rest, path_bytes) = capture_all_till_and_including_space(rest)
            .map_err(|e| HttpError::RequestParsingError("error while parsing path"))?;

        bytes_read_from_curr_buff = buf.len() - rest.len();
        _total_bytes_read += bytes_read_from_curr_buff;

        let path =
            String::from_str(str::from_utf8(path_bytes).map_err(|e| HttpError::Utf8Error(e))?)
                .map_err(|e| HttpError::Unknown("unreachable"))?;

        let (rest, _) = skip_whitespaces0(rest)
            .map_err(|e| HttpError::Unknown("error while skip_whitespaces"))
            .unwrap();

        bytes_read_from_curr_buff = buf.len() - rest.len();
        _total_bytes_read += bytes_read_from_curr_buff;

        let (rest, _http_version) = capture_all_till_and_including_crlf(rest)
            .map_err(|e| HttpError::Unknown("error while capturing all till crlf"))?;

        bytes_read_from_curr_buff = buf.len() - rest.len();
        _total_bytes_read += bytes_read_from_curr_buff;

        let mut headers = Headers::new();

        let mut bytes_read_from_header: usize = 0;
        loop {
            let (loop_rest, captured_header) =
                capture_all_till_and_including_crlf(&rest[bytes_read_from_header..])
                    .map_err(|e| HttpError::Unknown("error while capturing all till crlf"))?;

            bytes_read_from_header = rest.len() - loop_rest.len();
            bytes_read_from_curr_buff = buf.len() - loop_rest.len();
            _total_bytes_read += bytes_read_from_curr_buff;
            if captured_header.len() == 0 {
                break;
            }
            if captured_header.len() == 2 {
                // Found just the crlf
                break;
            }
            let (header_rest, key_bytes) =
                capture_all_till_and_including_termination_character(captured_header, b":")
                    .unwrap();
            let (header_rest, _) = skip_whitespaces0(header_rest).unwrap();
            let value_bytes = &header_rest[..];
            let key = String::from_str(
                str::from_utf8(key_bytes)
                    .map_err(|e| HttpError::Unknown("error while parsing header key"))?,
            )
            .map_err(|e| HttpError::Unknown("Infalling"))?;
            let val = String::from_str(
                str::from_utf8(value_bytes)
                    .map_err(|e| HttpError::Unknown("error while parsing header value"))?,
            )
            .map_err(|e| HttpError::Unknown("Infalling"))?;
            headers.insert(key, val);
        }
        let rest = &rest[bytes_read_from_header..];
        let mut request_body_bytes_to_read: usize = 0;

        let headers = if headers.keys().len() > 0 {
            // NOTE: only if `Content-Length` header is present, we will read the body.
            if let Some(val_str) = headers.get("Content-Length") {
                request_body_bytes_to_read = val_str.parse::<usize>().unwrap(); // TODO: definitely need to remove this unwrap;
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
