#![allow(unused_variables)]
#![deny(clippy::expect_used, clippy::unwrap_used)]
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::prelude::*;
use std::{net::TcpListener, sync::Arc};

use http::{
    http_request::Method, ContentTypeHttpResponse, Headers, HttpResponseBuilder,
    CONTENT_ENCODING_HEADER, SUPPORTED_ENCODINGS,
};
use itertools::Itertools;

use crate::http::{http_request::HttpRequest, HttpResponse};
mod http;
mod thread_pool;

fn handle_echo_endpoint(req: &HttpRequest, path_str: &str) -> ContentTypeHttpResponse {
    let body = path_str.as_bytes().to_vec();
    let response = HttpResponseBuilder::new(200).with_body(body).build();

    ContentTypeHttpResponse::PlainText(response)
}

fn handle_user_agent_endpoint(req: &HttpRequest) -> ContentTypeHttpResponse {
    match req.headers.as_ref() {
        Some(headers) => {
            if let Some(val) = headers.get("User-Agent") {
                let body = val.as_bytes().to_vec();
                ContentTypeHttpResponse::PlainText(
                    HttpResponseBuilder::new(200).with_body(body).build(),
                )
            } else {
                ContentTypeHttpResponse::NoBody(HttpResponse::default())
            }
        }
        None => ContentTypeHttpResponse::NoBody(HttpResponse::default()),
    }
}

fn handle_file_endpoint(
    req: &HttpRequest,
    file_name: &str,
    state: Arc<State>,
) -> ContentTypeHttpResponse {
    let directory = match state.directory.as_ref() {
        Some(dir) => dir,
        None => return ContentTypeHttpResponse::NoBody(HttpResponse::default()),
    };
    let file_path = format!("/{directory}/{file_name}");
    match std::fs::read(file_path) {
        Ok(content) => {
            ContentTypeHttpResponse::File(HttpResponseBuilder::new(200).with_body(content).build())
        }
        Err(_) => ContentTypeHttpResponse::NoBody(HttpResponseBuilder::new(404).build()),
    }
}

fn handle_file_upload_endpoint(
    req: &HttpRequest,
    file_name: &str,
    state: Arc<State>,
) -> ContentTypeHttpResponse {
    let directory = match state.directory.as_ref() {
        Some(dir) => dir,
        None => return ContentTypeHttpResponse::NoBody(HttpResponse::default()),
    };
    let body = match req.body.as_ref() {
        Some(body) => body,
        None => return ContentTypeHttpResponse::NoBody(HttpResponse::default()),
    };
    let file_path = format!("/{directory}/{file_name}");
    match std::fs::write(file_path, body) {
        Ok(_) => ContentTypeHttpResponse::NoBody(HttpResponseBuilder::new(201).build()),
        Err(_) => ContentTypeHttpResponse::NoBody(HttpResponseBuilder::new(500).build()),
    }
}

fn handle_encoding(req: &HttpRequest, response: &mut HttpResponse) -> anyhow::Result<()> {
    let Some(headers) = req.headers.as_ref() else {
        return Ok(());
    };
    let Some(val) = headers.get(http::ACCEPT_ENCODING_HEADER) else {
        return Ok(());
    };
    if response.body.is_none() {
        return Ok(());
    }
    let client_supported_encoding = val.split(",").map(|v| v.trim()).collect::<Vec<_>>(); // TODO:
                                                                                          // Can we remove this `Vec` heap allocation.
    let Some(encoding) = SUPPORTED_ENCODINGS
        .iter()
        .find(|e| client_supported_encoding.iter().any(|c| *c == **e))
    else {
        return Ok(());
    };

    let Some(body) = response.body.as_ref() else {
        return Ok(());
    };
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(body.as_ref())?;
    let x = encoder.finish()?;
    response.body = Some(x);

    if let Some(headers) = response.header.as_mut() {
        headers.insert(CONTENT_ENCODING_HEADER.to_string(), encoding.to_string());
    } else {
        let mut headers = Headers::new();
        headers.insert(CONTENT_ENCODING_HEADER.to_string(), encoding.to_string());
        response.header = Some(headers);
    }
    Ok(())
}

fn handle_request(req: HttpRequest, state: Arc<State>) -> anyhow::Result<HttpResponse> {
    let path = &req.path;

    let response = {
        let path_splits = path.split("/").collect::<Vec<_>>();
        // println!("path_splits: {:?}", path_splits);

        if path_splits.len() >= 3 && path_splits[1] == "echo" {
            // `3` is given assuming the path
            // always starts with `/`. Most probably this assumption is wrong
            handle_echo_endpoint(&req, path_splits[2])
        } else if path_splits.len() >= 2 && path_splits[1] == "user-agent" {
            handle_user_agent_endpoint(&req)
        } else if path_splits.len() >= 3 && path_splits[1] == "files" {
            match req.method {
                Method::Post => handle_file_upload_endpoint(&req, path_splits[2], state.clone()),
                _ => handle_file_endpoint(&req, path_splits[2], state.clone()),
            }
        } else {
            let status_code: u16 = if path == "/" { 200 } else { 404 };
            let response = HttpResponseBuilder::new(status_code).build();
            ContentTypeHttpResponse::NoBody(response)
        }
    };

    let mut response = if let Some(content_type) = response.get_content_type_header_value() {
        let mut response = response.into_inner();
        match response.header.as_mut() {
            Some(header) => {
                header.insert("Content-Type".to_string(), content_type.to_string());
            }
            None => {
                let mut header = Headers::new();
                header.insert("Content-Type".to_string(), content_type.to_string());
                response.header = Some(header);
            }
        }
        response
    } else {
        response.into_inner()
    };

    handle_encoding(&req, &mut response)?;

    if let Some(body) = &response.body {
        let body_len = body.len();
        match response.header.as_mut() {
            Some(header) => {
                header.insert("Content-Length".to_string(), body_len.to_string());
            }
            None => {
                let mut header = Headers::new();
                header.insert("Content-Length".to_string(), body_len.to_string());
                response.header = Some(header);
            }
        }
    }
    Ok(response)
}

struct State {
    directory: Option<String>,
}
fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4221")?;
    let thread_pool = thread_pool::ThreadPoolBuilder {}.build();
    let pool = thread_pool.start();
    let args = std::env::args();
    let args = args.collect::<Vec<_>>();
    let mut state = State { directory: None };
    if let Some((pos, _)) = args.iter().find_position(|a| *a == "--directory") {
        state.directory = Some(args[pos + 1].to_string());
    }
    let state = Arc::new(state);

    for stream in listener.incoming() {
        match stream {
            Ok(mut _stream) => {
                let state = state.clone();
                pool.run(Box::new(move || {
                    let request = match HttpRequest::create_from_tcp_stream(&mut _stream) {
                        Ok(req) => req,
                        Err(_) => {
                            let response = HttpResponseBuilder::new(500).build();
                            match response.write(&mut _stream) {
                                Ok(_) => {}
                                Err(e) => eprintln!("{}", e),
                            }
                            return;
                        }
                    };
                    let response = match handle_request(request, state) {
                        Ok(response) => response,
                        Err(_) => {
                            let response = HttpResponseBuilder::new(500).build();
                            match response.write(&mut _stream) {
                                Ok(_) => {}
                                Err(e) => eprintln!("{}", e),
                            }
                            return;
                        }
                    };
                    match response.write(&mut _stream) {
                        Ok(_) => {}
                        Err(e) => eprintln!("{}", e),
                    }
                }));
            }
            Err(e) => {}
        }
    }
    Ok(())
}
