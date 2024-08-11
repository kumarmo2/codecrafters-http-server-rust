FROM rust:1.80-bookworm AS builder

WORKDIR /app

COPY . .

RUN cargo build --release

FROM debian:bookworm

RUN apt-get update

WORKDIR /app

COPY --from=builder /app/target/release/http-server-starter-rust .

EXPOSE 4221

CMD ["/app/http-server-starter-rust"]

