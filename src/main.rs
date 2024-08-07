use std::{net::TcpListener, thread};

use http::HttpResponseBuilder;

use crate::http::{http_request::HttpRequest, HttpResponse};
mod http;
mod thread_pool;

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
                    let path = &request.path;
                    let status_code: u16 = if path == "/" { 200 } else { 404 };
                    let response = HttpResponseBuilder::new(status_code).build();
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
