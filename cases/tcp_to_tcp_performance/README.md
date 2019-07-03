# TCP To TCP Performance Test

This test measures the TCP output performance for each subject.

## Design

1. Each subjects is configured to receive and send data over TCP.
2. The TCP producer and consumer are separate instances to help ensure performance is not
   tainted.
3. Each subject is started and ran for 60 seconds.
4. [Performance data][performance_data] is collected and persisted to S3 for analysis.

## Results

```
$ bin/comare -t tcp_to_tcp_performance

| Metric          | filebeat   | fluentbit | fluentd  | logstash | splunk_hf | splunk_uf   | vector    |
|:----------------|:-----------|:----------|:---------|:---------|:----------|:------------|:----------|
| IO Thrpt (avg)  | 5MiB/s     | 67.1MiB/s | 3.9MiB/s | 10MiB/s  | 7.6MiB/s  | 70.4MiB/s W | 69.9MiB/s |
| CPU sys (max)   | 4.1        | 8         | 2 W      | 16.6     | 28.4      | 21.9        | 6         |
| CPU usr (max)   | 52.8       | 50.8      | 50.5 W   | 75       | 72.2      | 99.5        | 93.5      |
| Load 1m (avg)   | 0.4 W      | 0.7       | 0.8      | 2.2      | 1.9       | 1.6         | 0.9       |
| Mem used (max)  | 164.2MiB W | 641.2MiB  | 293.6MiB | 725.4MiB | 399.2MiB  | 250.3MiB    | 171.6MiB  |
| Disk read (sum) | 9.6MiB     | 8.2MiB    | 8.2MiB   | 8.3MiB   | 372.7MiB  | 11.8MiB     | 2.2MiB W  |
| Disk writ (sum) | 2.7MiB     | 2.7MiB    | 2.6MiB W | 2.7MiB   | 8.8MiB    | 6.1MiB      | 12.1MiB   |
| Net recv (sum)  | 308.6MiB   | 4.1gib    | 243.5MiB | 622MiB   | 468.4MiB  | 4.3gib W    | 4.2gib    |
| Net send (sum)  | 178.9MiB   | 4.1gib    | 292MiB   | 1.1gib   | 450.8MiB  | 4.3gib      | 4.2gib    |
| TCP estab (avg) | 431        | 1910      | 425      | 425      | 1562      | 553         | 423       |
| TCP sync (avg)  | 0          | 14        | 5        | 0        | 0         | 0           | 0         |
| TCP close (avg) | 5          | 4         | 0        | 3        | 5         | 3           | 2         |
-------------------------------------------------------------------------------------------------------------
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
bin/test -t tcp_to_tcp_performance
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
[docs]: https://docs.vectorproject.io
[how_it_works]: /README.md#how-it-works
[performance_data]: /README.md#performance-data
[repo]: https://github.com/timberio/vector
[setup]: /README.md#setup
[usage]: /README.md#usage
[website]: https://vectorproject.io
