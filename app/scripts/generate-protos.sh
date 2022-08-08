#/usr/bin/env bash

cd `dirname $0`/..

OUT=src/generated/protos

mkdir -p ${OUT}
rm -f ${OUT}/*
protoc -I=../server/proto hello.proto --js_out=import_style=commonjs:${OUT} --grpc-web_out=import_style=commonjs+dts,mode=grpcwebtext:${OUT}