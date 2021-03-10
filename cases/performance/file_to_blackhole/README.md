Demonstrate the ability to run a controlled `file -> blackhole` experiment


Vector build:

```
RUSTFLAGS="-g" cargo build --no-default-features --features "sources-file,sources-internal_metrics,sinks-prometheus,sinks-blackhole" --release
```
