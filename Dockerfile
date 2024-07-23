FROM rust:1.76.0-bookworm as builder

WORKDIR /usr/src/bitomc

COPY . .

RUN cargo build --bin bitomc --release

FROM debian:bookworm-slim

COPY --from=builder /usr/src/bitomc/target/release/bitomc /usr/local/bin
RUN apt-get update && apt-get install -y openssl

ENV RUST_BACKTRACE=1
ENV RUST_LOG=info
