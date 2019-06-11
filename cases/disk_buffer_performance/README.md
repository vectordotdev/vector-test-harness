# Disk Buffer Performance Test

This test measures the performance of the subject's disk buffer.

## Design

1. Each subjects is configured to receive and send data over TCP.
2. The TCP producer and consmer are separate instances to help ensure performance is not tainted.
3. Each subject is configured to store it's buffers on-disk, syncing as much as possible, with
   a limit of 100mb.
4. Data is piped to the subject, from 5 producers, as fast as possible, for 60 seconds.
5. [Performance data][performance_data] is collected and persisted to S3 for analysis.

### Caveats

Many of the subjects are designed to buffer data in memory and sync to disk on fixed intervals.
In many cases, the subject failed to write data to disk at all for the duration of our test.
Therefore, for the purpose of this test, we configured each subject to sync to disk as
aggressively as possible. This more accurately embodies the goal of disk-based buffers and it
aligns durability guarantees across subjects since this is the default behavior of Vector.

## Results

```
$ bin/comare -t disk_buffer_performance

Metric           | filebeat   | fluentbit | fluentd    | logstash   | splunk_hf | splunk_uf    | vector    
-----------------|------------|-----------|------------|------------|-----------|--------------|-----------
Test count       | 1          | 1         | 1          | 1          | 1         | 1            | 1         
IO Thrpt (avg)   | 5mib/s     | 67.1mib/s | 3.9mib/s   | 10mib/s    | 7.6mib/s  | 251.7mib/s W | 68mib/s   
CPU sys (max)    | 4.1        | 8         | 2 W        | 16.6       | 28.4      | 38.3         | 6         
CPU usr (max)    | 52.8       | 50.8      | 50.5       | 75         | 72.2      | 30.9 W       | 93.5      
Load 1m (avg)    | 0.4 W      | 0.7       | 0.8        | 2.2        | 1.9       | 1.2          | 1.3       
Mem used (max)   | 164.2mib W | 641.2mib  | 293.6mib   | 725.4mib   | 399.2mib  | 242.3mib     | 172.9mib  
Disk read (sum)  | 9.6mib     | 8.2mib    | 8.2mib W   | 8.3mib     | 372.7mib  | 11.1mib      | 9.3mib    
Disk writ (sum)  | 2.7mib     | 2.7mib    | 2.6mib W   | 2.7mib     | 8.8mib    | 3.6mib       | 2.7mib    
Net recv (sum)   | 308.6mib   | 4.1gib    | 243.5mib   | 622mib     | 468.4mib  | 15.2gib W    | 4.1gib    
Net send (sum)   | 178.9mib   | 4.1gib    | 292mib     | 1.1gib     | 450.8mib  | 15.2gib      | 4.1gib    
TCP estab (avg)  | 431        | 1910      | 425        | 425        | 1562      | 425          | 428       
TCP sync (avg)   | 0          | 14        | 5          | 0          | 0         | 0            | 0         
TCP close (avg)  | 5          | 4         | 0          | 3          | 5         | 5            | 1         

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
bin/test -t disk_buffer_performance
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
