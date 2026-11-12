# Disk Buffer Performance Test

This test measures the performance of the subject's disk buffer.

## Design

1. Each subjects is configured to receive and send data over TCP.
2. The TCP producer and consmer are separate instances to help ensure performance is not tainted.
3. Each subject uses the disk-buffer capacity and synchronization policy in its case configuration.
4. Data is piped to the subject, from 5 producers, as fast as possible, for 60 seconds.
5. [Performance data][performance_data] is collected and persisted to S3 for analysis.

### Caveats

The configured durability policies differ. Logstash checkpoints every event,
while [Vector synchronizes its disk buffer every 500ms](https://vector.dev/docs/architecture/buffering-model/).
Interpret the measurements with those settings; they do not compare equivalent
crash-durability guarantees.

This performance case observes traffic and host load. It does not establish
that every accepted event reached durable storage; the separate persistence
case checks recovery after a restart and permits partial loss.

## AWS baseline — 2026-11-12

See the [current AWS report](../../results/2026-11-12/README.md) for this case’s
configurations, pinned versions, hardware, measured outcomes, and sampling limits.
Performance figures use one run per subject/configuration. Correctness results
retain the existing assertions, including any observed failures.

## Historical results

These tables archive earlier Vector versions and workload definitions. Use the
dated AWS baseline above for current measurements; software, configurations, and
measurement methods differ from the historical experiments.

```
$ vth compare -t disk_buffer_performance

| Metric          | filebeat   | fluentbit | fluentd  | logstash | splunk_hf | splunk_uf    | vector   |
|:----------------|:-----------|:----------|:---------|:---------|:----------|:-------------|:---------|
| Test count      | 1          | 1         | 1        | 1        | 1         | 1            | 1        |
| IO Thrpt (avg)  | 5MiB/s     | 67.1MiB/s | 3.9MiB/s | 10MiB/s  | 7.6MiB/s  | 251.7MiB/s W | 68MiB/s  |
| CPU sys (max)   | 4.1        | 8         | 2 W      | 16.6     | 28.4      | 38.3         | 6        |
| CPU usr (max)   | 52.8       | 50.8      | 50.5     | 75       | 72.2      | 30.9 W       | 93.5     |
| Load 1m (avg)   | 0.4 W      | 0.7       | 0.8      | 2.2      | 1.9       | 1.2          | 1.3      |
| Mem used (max)  | 164.2MiB W | 641.2MiB  | 293.6MiB | 725.4MiB | 399.2MiB  | 242.3MiB     | 172.9MiB |
| Disk read (sum) | 9.6MiB     | 8.2MiB    | 8.2MiB W | 8.3MiB   | 372.7MiB  | 11.1MiB      | 9.3MiB   |
| Disk writ (sum) | 2.7MiB     | 2.7MiB    | 2.6MiB W | 2.7MiB   | 8.8MiB    | 3.6MiB       | 2.7MiB   |
| Net recv (sum)  | 308.6MiB   | 4.1gib    | 243.5MiB | 622MiB   | 468.4MiB  | 15.2gib W    | 4.1gib   |
| Net send (sum)  | 178.9MiB   | 4.1gib    | 292MiB   | 1.1gib   | 450.8MiB  | 15.2gib      | 4.1gib   |
| TCP estab (avg) | 431        | 1910      | 425      | 425      | 1562      | 425          | 428      |
| TCP sync (avg)  | 0          | 14        | 5        | 0        | 0         | 0            | 0        |
| TCP close (avg) | 5          | 4         | 0        | 3        | 5         | 5            | 1        |

W = winner
filebeat = 7.1.1
fluentbit = 1.1.0
fluentd = 3.3.0-1
logstash = 7.0.1
splunk_heavy_forwarder = 7.2.6-c0bf0f679ce9
splunk_universal_forwarder = 7.2.5.1-962d9a8e1586
vector = 0.2.0
```

## Try It

```bash
vth run -t disk_buffer_performance
```

## Resources

* [Setup][setup]
* [Usage][usage]
* [Development][development]
* [How it works][how_it_works]
* [Vector docs][docs]
* [Vector repo][repo]
* [Vector website][website]


[development]: /README.md#development
[docs]: https://vector.dev/docs/
[how_it_works]: /README.md#how-it-works
[performance_data]: /README.md#performance-data
[repo]: https://github.com/vectordotdev/vector
[setup]: /README.md#setup
[usage]: /README.md#usage
[website]: https://vector.dev
