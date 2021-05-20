#!/usr/bin/env bash

set -o errexit
set -o pipefail
set -o nounset
# set -o xtrace

__dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${__dir}/bin"
BUILD="${__dir}/.build"
VECTOR_SHA="cd06cf0ae959990b5d5d2e0320aecd7140a09144"
VECTOR_BIN="${__dir}/bin/vector-${VECTOR_SHA}"
REPO="https://github.com/timberio/vector.git"
REPO_DOT_GIT=$(basename -- "${REPO}")
REPO_NAME="${REPO_DOT_GIT%.*}"

mkdir -p "${__dir}/bin"
mkdir -p "${BUILD}"

if [ ! -d "${BUILD}/${REPO_NAME}" ]; then
    pushd "${BUILD}"
    git clone "${REPO}"
    popd
fi

if [ ! -f "${VECTOR_BIN}" ]; then
    pushd "${BUILD}/${REPO_NAME}"
    git fetch --all
    git checkout ${VECTOR_SHA}
    RUSTFLAGS="-g" cargo build --no-default-features --features "sources-stdin,sinks-blackhole" --release
    cp target/release/vector "${VECTOR_BIN}"
    popd
fi

perf stat --repeat=10 bash -c "zcat baked.log.gz | ${VECTOR_BIN} -qq --config vector.toml"

zcat baked.log.gz | perf record --call-graph dwarf "${VECTOR_BIN}" -qq --config vector.toml
perf script --input=perf.data | inferno-collapse-perf > stacks.folded
inferno-flamegraph < stacks.folded > flamegraph.svg
