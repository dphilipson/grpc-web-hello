FROM rust as cargo-build

RUN apt update &&\
    apt install curl protobuf-compiler -y
WORKDIR /usr/src/grpc-web-hello
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
RUN mkdir src/ &&\
    echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs &&\
    cargo build --release &&\
    rm -f target/release/deps/grpc-web-hello*
COPY ./build.rs ./build.rs
COPY src/ src/
COPY proto/ proto/
RUN cargo build --release

FROM debian:buster-slim

COPY ./docker-deps/run.sh /home/run.sh
COPY ./docker-deps/grpcwebproxy /usr/local/bin/grpcwebproxy
COPY --from=cargo-build /usr/src/grpc-web-hello/target/release/grpc-web-hello /usr/local/bin/grpc-web-hello
EXPOSE 8080
CMD ["sh", "-c", "/home/run.sh"]
