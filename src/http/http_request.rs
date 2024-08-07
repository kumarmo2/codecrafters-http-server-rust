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
    io::{BufRead, BufReader, Read},
    net::TcpStream,
    str::FromStr,
};

use super::Header;

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
    headers: Option<Header>,
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

        println!("method: {:?}, buf_len: {len}", method, len = buf.len());

        let (rest, _) = skip_whitespaces0(rest).map_err(|_| "error while scaping spaces")?;

        bytes_read_from_curr_buff = buf.len() - rest.len();
        _total_bytes_read += bytes_read_from_curr_buff;

        let (rest, path_bytes) =
            capture_all_till_and_including_space(rest).map_err(|_| "error while parsing path")?;

        bytes_read_from_curr_buff = buf.len() - rest.len();
        _total_bytes_read += bytes_read_from_curr_buff;

        println!(
            "path_bytes: {:?}, buf_len: {len}",
            path_bytes,
            len = buf.len()
        );

        let path = String::from_str(str::from_utf8(path_bytes)?)?;
        Ok(Self {
            method,
            path,
            headers: None,
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
