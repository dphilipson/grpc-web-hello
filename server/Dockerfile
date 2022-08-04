FROM rust:latest as cargo-build

RUN apt-get update
RUN apt-get install protobuf-compiler -y
WORKDIR /usr/src/grpc-web-hello
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
RUN mkdir src/
RUN echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs
RUN cargo build --release
RUN rm -f target/release/deps/grpc-web-hello*
COPY . .
RUN cargo build --release

FROM debian:buster-slim

COPY --from=cargo-build /usr/src/grpc-web-hello/target/release/grpc-web-hello /usr/local/bin/grpc-web-hello
CMD ["grpc-web-hello"]

