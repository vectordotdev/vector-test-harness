#!/usr/bin/env bash

set -o errexit
set -o pipefail
set -o nounset
# set -o xtrace

__dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${__dir}/bin"
BUILD="${__dir}/.build"
VECTOR_SHA="6193e68e5b3e50b7e5469d6a8699e25d07bcf57e"
VECTOR_BIN="${__dir}/bin/vector_gen-${VECTOR_SHA}"
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
    pushd ~/projects/com/datadog/vector
    git checkout ${VECTOR_SHA}
    RUSTFLAGS="-g" cargo build --no-default-features --features "sources-stdin,sources-internal_metrics,transforms-json_parser,transforms-regex_parser,transforms-add_fields,transforms-coercer,transforms-remap,transforms-reduce,transforms-route,sinks-prometheus,sinks-blackhole" --release
    cp target/release/vector "${VECTOR_BIN}"
    popd
fi

perf stat --repeat=10 bash -c "zcat baked.log.gz | ${VECTOR_BIN} -qq --threads=1 --config vector-multihop.toml"
perf stat --repeat=10 bash -c "zcat baked.log.gz | ${VECTOR_BIN} -qq --threads=1 --config vector-nohop.toml"

zcat baked.log.gz | perf record --call-graph dwarf "${VECTOR_BIN}" -qq --threads=1 --config vector-multihop.toml && mv perf.data perf-multihop.data
zcat baked.log.gz | perf record --call-graph dwarf "${VECTOR_BIN}" -qq --threads=1 --config vector-nohop.toml && mv perf.data perf-nohop.data

perf script --input=perf-multihop.data | inferno-collapse-perf > stacks-multihop.folded
inferno-flamegraph < stacks-multihop.folded > flamegraph-multihop.svg

perf script --input=perf-nohop.data | inferno-collapse-perf > stacks-nohop.folded
inferno-flamegraph < stacks-nohop.folded > flamegraph-nohop-.svg

inferno-diff-folded stacks-multihop.folded stacks-nohop-.folded | inferno-flamegraph > multihop-to-nohop.svg
