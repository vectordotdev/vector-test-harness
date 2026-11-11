set positional-arguments

default:
    @just --list

# Run any harness command, including --help.
harness *args:
    cargo run --locked --quiet --bin vth -- "$@"

doctor *args:
    cargo run --locked --quiet --bin vth -- doctor "$@"

list *args:
    cargo run --locked --quiet --bin vth -- list "$@"

run *args:
    cargo run --locked --quiet --bin vth -- run "$@"

teardown *args:
    cargo run --locked --quiet --bin vth -- teardown "$@"

bootstrap-aws *args:
    cargo run --locked --quiet --bin vth -- bootstrap-aws "$@"

global-bootstrap *args:
    cargo run --locked --quiet --bin vth -- global-bootstrap "$@"

build-amis *args:
    cargo run --locked --quiet --bin vth -- build-amis "$@"

results *args:
    cargo run --locked --quiet --bin vth -- results "$@"

compare *args:
    cargo run --locked --quiet --bin vth -- compare "$@"

export-results *args:
    cargo run --locked --quiet --bin vth -- export "$@"

ssh *args:
    cargo run --locked --quiet --bin vth -- ssh "$@"

build *args:
    cargo run --locked --quiet --bin vth -- build "$@"

bench *args:
    cargo run --locked --quiet --bin vth -- bench "$@"

profile *args:
    cargo run --locked --quiet --bin vth -- profile "$@"

flamegraph *args:
    cargo run --locked --quiet --bin vth -- flamegraph "$@"

generate *args:
    cargo run --locked --quiet --bin vth -- generate "$@"

fmt:
    cargo fmt --all

check:
    cargo fmt --all -- --check
    cargo clippy --locked --all-targets -- -D warnings

test:
    cargo test --locked

lint *args:
    cargo run --locked --quiet --bin vth -- lint "$@"
