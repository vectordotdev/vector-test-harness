# Stdin-to-blackhole checks and benchmark

`vth check stdin_to_blackhole --subject <product> --subject-bin /path/to/binary`
feeds the original million-record gzip fixture and requires an exact native
completed-discard count, clean health and shutdown. It verifies no payloads or
identities and publishes no delivery timing. Input lifecycles are recorded;
Filebeat's native CEL profile buffers the finite input through EOF.

Stream a finite fixture through Vector's stdin source into a blackhole sink.
The default fixture is `../file_to_blackhole/baked.log.gz`; the Rust harness
decompresses it and streams it to Vector.

Run from the repository root with an installed `vth` and a supplied Vector binary:

```sh
vth bench stdin_to_blackhole --vector-bin /path/to/vector --runs 10
vth profile stdin_to_blackhole --vector-bin /path/to/vector --tool stat
```

Use `--baseline-bin /path/to/baseline` with `bench` to compare binaries, or
`--input /path/to/fixture.log.gz` to select another finite fixture. The Vector
binary needs the stdin source and blackhole sink enabled. To build a revision,
use `vth build --revision <commit>`.

See the [standalone guide](../README.md) for measurement and artifact details.

## Historical Vector process timings — 2026-11-12

Vector 0.61.0 on an AWS `c5.large` (2 vCPUs, 4 GiB RAM), Ubuntu 22.04.5,
using the checked-in fixture and config:

| Measured runs | Warmups | Mean elapsed | Sample standard deviation |
| --- | --- | --- | --- |
| 10 | 1 | 1.3272 s | 0.0087 s |

The `wall-clock-with-streamed-input-v1` measurement includes process execution
and input streaming. [Individual timings and hashes](../../../results/2026-11-12/standalone/stdin_to_blackhole.json)
identify the binary, fixture, config, and harness revision.
