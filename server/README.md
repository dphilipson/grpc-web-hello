# gRPC-Web Hello Server

## Development

Run

```
docker-compose up
```

to start a local Envoy as a dependency, then run the
application with

```
cargo run
```

or with your favorite IDE.

## Build

Just use the Dockerfile:

```
docker build -t grpc-web-hello .
```
