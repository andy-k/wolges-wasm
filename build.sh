#!/usr/bin/env bash

set -euo pipefail

# the container sees this directory as /workdir and, as the build stamps the
# commits it is built from, the git directories of this repository and of its
# wolges: a worktree's lies outside this directory, at the full path its .git
# file names, so the container sees it at that same path, read-only
mounts=(-v "$(pwd):/workdir")
here="$(pwd -P)"
for dir in "$here" "$here/wolges"; do
  top="$(git -C "$dir" rev-parse --show-toplevel 2>/dev/null)" || continue
  [ "$top" = "$dir" ] || continue
  common="$(git -C "$dir" rev-parse --path-format=absolute --git-common-dir)" ||
    continue
  case "$common/" in
    "$here"/*) ;;
    *) mounts+=(-v "$common:$common:ro") ;;
  esac
done

docker build - < Dockerfile-wasm-pack -t wolges-wasm-pack &&
docker run --rm "${mounts[@]}" wolges-wasm-pack
