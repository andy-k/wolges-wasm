#!/usr/bin/env bash

set -euo pipefail

docker build - < Dockerfile-wasm-pack -t wolges-wasm-pack &&
docker run --rm -v "$(pwd):/workdir" wolges-wasm-pack
