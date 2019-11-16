# Regex Parsing Performance Test

This test measures the performance of the subject's ability parse strings with Regex
and place the extracted values in named keys.

## Design

1. Each subjects is configured to receive and send data over TCP,
2. The TCP producer and consumer are separate instances to help ensure performance is not tainted.
3. Each subject is configured to parse incoming data in the Apache common format:

   ```
   ^(?P<host>[\w\.]+) - (?P<user>[\w]+) (?P<bytes_in>[\d]+) \[(?P<timestamp>.*)\] "(?P<method>[\w]+) (?P<path>.*)" (?P<status>[\d]+) (?P<bytes_out>[\d]+)$
   ```

4. The test subject is sent valid Apache common lines for 60 seconds.
5. [Performance data][performance_data] is collected and persisted to S3 for analysis.

## Results

* **Note: Splunk Universal Forwarder does not support parsing**
* **Note: Filebeat does not support parsing**

```
$ bin/compare -t regex_parsing_performance

| Metric          | fluentbit   | fluentd  | logstash  | splunk_hf | vector     |
|:----------------|:------------|:---------|:----------|:----------|:-----------|
| IO Thrpt (avg)  | 20.5MiB/s W | 2.6MiB/s | 4.6MiB/s  | 7.8MiB/s  | 13.2MiB/s  |
| CPU sys (max)   | 6.5         | 1.7 W    | 9.1       | 28.9      | 4.5        |
| CPU usr (max)   | 50.8        | 50.5 W   | 90.5      | 72.3      | 95         |
| Load 1m (avg)   | 0.5 W       | 0.8      | 2.1       | 2.2       | 0.9        |
| Mem used (max)  | 584.5MiB    | 246.1MiB | 743.8MiB  | 442.4MiB  | 179.3MiB W |
| Disk read (sum) | 8.3MiB W    | 8.4MiB   | 8.8MiB    | 354.7MiB  | 9.3MiB     |
| Disk writ (sum) | 7.6MiB      | 7.6MiB W | 7.9MiB    | 51.3MiB   | 10.7MiB    |
| Net recv (sum)  | 1.2gib W    | 164MiB   | 282.4MiB  | 475.6MiB  | 804.8MiB   |
| Net send (sum)  | 1.6gib      | 176.8MiB | 1021.2MiB | 454.7MiB  | 2gib       |
| TCP estab (avg) | 794         | 424      | 428       | 1599      | 485        |
| TCP sync (avg)  | 0           | 0        | 0         | 0         | 0          |
| TCP close (avg) | 0           | 0        | 5         | 1         | 3          |
-------------------------------------------------------------------------------------------------------------
W = winner
fluentbit = 1.1.0
fluentd = 3.3.0-1
logstash = 7.0.1
splunk_heavy_forwarder = 7.2.6-c0bf0f679ce9
vector = 0.2.0-6-g434bed8
```

## Try It

```bash
bin/test -t regex_parsing_performance
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
[docs]: https://docs.vector.dev
[how_it_works]: /README.md#how-it-works
[performance_data]: /README.md#performance-data
[repo]: https://github.com/timberio/vector
[setup]: /README.md#setup
[usage]: /README.md#usage
[website]: https://vector.dev
