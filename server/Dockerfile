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

RUN apt update &&\
    apt install debian-keyring debian-archive-keyring apt-transport-https curl lsb-release -y &&\
    curl -sL 'https://deb.dl.getenvoy.io/public/gpg.8115BA8E629CC074.key' | gpg --dearmor -o /usr/share/keyrings/getenvoy-keyring.gpg &&\
    echo a077cb587a1b622e03aa4bf2f3689de14658a9497a9af2c427bba5f4cc3c4723 /usr/share/keyrings/getenvoy-keyring.gpg | sha256sum --check &&\
    echo "deb [arch=amd64 signed-by=/usr/share/keyrings/getenvoy-keyring.gpg] https://deb.dl.getenvoy.io/public/deb/debian $(lsb_release -cs) main" | tee /etc/apt/sources.list.d/getenvoy.list &&\
    apt update &&\
    apt install getenvoy-envoy -y
# .yml vs .yaml is important for Envoy: it needs the latter to not parse as JSON.
COPY ./envoy.build.yml /etc/envoy/envoy.yaml
COPY --from=cargo-build /usr/src/grpc-web-hello/target/release/grpc-web-hello /usr/local/bin/grpc-web-hello
EXPOSE 8080
CMD envoy -c /etc/envoy/envoy.yaml & grpc-web-hello

