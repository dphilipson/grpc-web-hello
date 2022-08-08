#!/usr/bin/env bash
# Runs grpcwebproxy and grpc-web-hello in parallel, waits for either to finish,
# then exits with the exit code of whichever process finished.
grpcwebproxy \
  --backend_addr=localhost:50051 \
  --backend_tls=false \
  --run_tls_server=false \
  --allow_all_origins &
grpc-web-hello &
wait -n
exit $?
