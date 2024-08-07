use std::{net::TcpListener, thread};

use http::{ContentTypeHttpResponse, Header, HttpResponseBuilder};
use nom::AsBytes;

use crate::http::{http_request::HttpRequest, HttpResponse};
mod http;
mod thread_pool;

fn handle_echo_endpoint(req: &HttpRequest, path_str: &str) -> ContentTypeHttpResponse {
    let body = path_str.as_bytes().to_vec();
    let response = HttpResponseBuilder::new(200).with_body(body).build();
    println!("response: {:?}", response);

    ContentTypeHttpResponse::PlainText(response)
}

fn handle_request(req: HttpRequest) -> HttpResponse {
    let path = &req.path;
    println!("path: {}", path);

    let response = {
        let path_splits = path.split("/").collect::<Vec<_>>();
        println!("path_splits: {:?}", path_splits);

        if path_splits.len() >= 3 && path_splits[1] == "echo" {
            // `3` is given assuming the path
            // always starts with `/`. Most probably this assumption is wrong
            handle_echo_endpoint(&req, path_splits[2])
        } else {
            let status_code: u16 = if path == "/" { 200 } else { 404 };
            let response = HttpResponseBuilder::new(status_code).build();
            ContentTypeHttpResponse::NoBody(response)
        }
    };

    let mut response = match response {
        ContentTypeHttpResponse::Json(_) => {
            unimplemented!("ContentTypeHttpResponse::Json");
            // match response.header.as_mut()
            //     Some(header) => {
            //         header.insert("Content-Type".to_string(), "text/plain".to_string());
            //     }
            //     None => {
            //         let mut header = Header::new();
            //         header.insert("Content-Type".to_string(), "text/plain".to_string());
            //         response.header = Some(header);
            //     }
        }
        ContentTypeHttpResponse::PlainText(mut response) => {
            match response.header.as_mut() {
                Some(mut header) => {
                    header.insert("Content-Type".to_string(), "text/plain".to_string());
                }
                None => {
                    let mut header = Header::new();
                    header.insert("Content-Type".to_string(), "text/plain".to_string());
                    response.header = Some(header);
                }
            }
            response
        }
        ContentTypeHttpResponse::NoBody(response) => response,
    };

    if let Some(body) = &response.body {
        let body_len = body.as_bytes().len();
        match response.header.as_mut() {
            Some(header) => {
                header.insert("Content-Length".to_string(), body_len.to_string());
            }
            None => {
                let mut header = Header::new();
                header.insert("Content-Length".to_string(), body_len.to_string());
                response.header = Some(header);
            }
        }
    }
    response
}
fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();
    let thread_pool = thread_pool::ThreadPoolBuilder {}.build();
    let pool = thread_pool.start();

    for stream in listener.incoming() {
        match stream {
            Ok(mut _stream) => {
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
                    let response = handle_request(request);
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
