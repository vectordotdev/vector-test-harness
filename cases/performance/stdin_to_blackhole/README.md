# Stdin-to-blackhole benchmark

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
