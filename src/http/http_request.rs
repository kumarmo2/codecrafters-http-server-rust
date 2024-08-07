#![allow(unused_variables)]
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

use super::Headers;

#[derive(Clone, Debug)]
enum Method {
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
    method: Method,
    pub(crate) path: String,
    pub(crate) headers: Option<Headers>,
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
    pub(crate) fn create_from_tcp_stream(
        stream: &mut TcpStream,
    ) -> Result<HttpRequest, Box<dyn std::error::Error>> {
        let mut reader = BufReader::new(stream);
        let buf = reader.fill_buf()?;
        let mut _total_bytes_read: usize = 0;
        let mut bytes_read_from_curr_buff: usize = 0;

        let (rest, method) = parse_method(&buf[bytes_read_from_curr_buff..])
            .map_err(|_| "error while parsing method")?;

        bytes_read_from_curr_buff = buf.len() - rest.len();
        _total_bytes_read += bytes_read_from_curr_buff;

        // println!("method: {:?}, buf_len: {len}", method, len = buf.len());

        let (rest, _) = skip_whitespaces0(rest).map_err(|_| "error while scaping spaces")?;

        bytes_read_from_curr_buff = buf.len() - rest.len();
        _total_bytes_read += bytes_read_from_curr_buff;

        let (rest, path_bytes) =
            capture_all_till_and_including_space(rest).map_err(|_| "error while parsing path")?;

        bytes_read_from_curr_buff = buf.len() - rest.len();
        _total_bytes_read += bytes_read_from_curr_buff;

        let path = String::from_str(str::from_utf8(path_bytes)?)?;

        let (rest, _) = skip_whitespaces0(rest).map_err(|_| "error while scaping spaces")?;

        bytes_read_from_curr_buff = buf.len() - rest.len();
        _total_bytes_read += bytes_read_from_curr_buff;

        let (rest, _http_version) = capture_all_till_and_including_crlf(rest)
            .map_err(|_| "error while parsing http version")?;

        bytes_read_from_curr_buff = buf.len() - rest.len();
        _total_bytes_read += bytes_read_from_curr_buff;

        let mut headers = Headers::new();

        let mut bytes_read_from_header: usize = 0;
        loop {
            // println!("...., {rest_len}", rest_len = rest.len());
            let (loop_rest, captured_header) =
                capture_all_till_and_including_crlf(&rest[bytes_read_from_header..])
                    .map_err(|_| "error while capturing header")?;

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
            let key = String::from_str(str::from_utf8(key_bytes)?)?;
            let val = String::from_str(str::from_utf8(value_bytes)?)?;
            // println!("header, key: {key}, val: {val}");
            headers.insert(key, val);
        }
        let rest = &rest[bytes_read_from_header..];

        let headers = if headers.keys().len() > 0 {
            Some(headers)
        } else {
            None
        };

        println!("headers: {:?}", headers);

        Ok(Self {
            method,
            path,
            headers,
        })

        // nom::bytes::complete::
        //     .map_err(|e| {
        //     eprintln!("found error: {}", e);
        //     "could not fill buf"
        // })?;
        // let mut buf = reader.buffer();
        // let mut bytes_read: usize = 0;
        // // let buf = reader.fill_buf()?;
        // let buf_len = buf.len();
        // let (buf, method) = parse_method(buf).map_err(|e| {
        //     eprintln!("error while parsing http_method: {}", e);
        //     "error while parsing http_method"
        // })?;
        // println!("method: {:?}", method);
        // let bytes = method.get_text_bytes();
        // bytes_read += bytes + 1; // 1 is for the space after the "http method"
        // {
        //     reader.consume(bytes_read);
        // }
        // let mut buf = &buf[bytes_read..];

        // todo!()
    }
}
