#![allow(unused_variables)]

use std::{net::TcpListener, sync::Arc};

use http::{ContentTypeHttpResponse, Headers, HttpResponseBuilder};
use itertools::Itertools;
use nom::AsBytes;

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

fn handle_request(req: HttpRequest, state: Arc<State>) -> HttpResponse {
    let path = &req.path;

    let response = {
        let path_splits = path.split("/").collect::<Vec<_>>();
        println!("path_splits: {:?}", path_splits);

        if path_splits.len() >= 3 && path_splits[1] == "echo" {
            // `3` is given assuming the path
            // always starts with `/`. Most probably this assumption is wrong
            handle_echo_endpoint(&req, path_splits[2])
        } else if path_splits.len() >= 2 && path_splits[1] == "user-agent" {
            handle_user_agent_endpoint(&req)
        } else if path_splits.len() >= 3 && path_splits[1] == "files" {
            handle_file_endpoint(&req, path_splits[2], state.clone())
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

    if let Some(body) = &response.body {
        let body_len = body.as_bytes().len();
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
    response
}

struct State {
    directory: Option<String>,
}
fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    let thread_pool = thread_pool::ThreadPoolBuilder {}.build();
    let pool = thread_pool.start();
    let args = std::env::args();
    let args = args.collect::<Vec<_>>();
    let mut state = State { directory: None };
    if let Some((pos, _)) = args.iter().find_position(|a| *a == "--directory") {
        println!("pos: {:?}, directory: {} ", pos, args[pos + 1]);
        state.directory = Some(args[pos + 1].to_string());
    }
    let state = Arc::new(state);

    for stream in listener.incoming() {
        match stream {
            Ok(mut _stream) => {
                let state = state.clone();
                pool.run(Box::new(move || {
                    // let response = HttpResponse::default();
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
                    let response = handle_request(request, state);
                    match response.write(&mut _stream) {
                        Ok(_) => {}
                        Err(e) => eprintln!("{}", e),
                    }
                }));
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
