#!/usr/bin/env bash

set -euo pipefail

docker build - < Dockerfile-wasm-pack -t wolges-wasm-pack &&
docker run -ti -v "$(pwd):/workdir" wolges-wasm-pack
