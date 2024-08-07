use std::{net::TcpListener, thread};

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
                    let response = HttpResponse::default();
                    HttpRequest::create_from_tcp_stream(&mut _stream);
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
