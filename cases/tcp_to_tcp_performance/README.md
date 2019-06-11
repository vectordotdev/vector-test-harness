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

Metric           | filebeat   | fluentbit | fluentd    | logstash   | splunk_hf | splunk_uf   | vector   
-----------------|------------|-----------|------------|------------|-----------|-------------|----------
IO Thrpt (avg)   | 5mib/s     | 67.1mib/s | 3.9mib/s   | 10mib/s    | 7.6mib/s  | 70.4mib/s W | 69.9mib/s
CPU sys (max)    | 4.1        | 8         | 2 W        | 16.6       | 28.4      | 21.9        | 6        
CPU usr (max)    | 52.8       | 50.8      | 50.5 W     | 75         | 72.2      | 99.5        | 93.5     
Load 1m (avg)    | 0.4 W      | 0.7       | 0.8        | 2.2        | 1.9       | 1.6         | 0.9      
Mem used (max)   | 164.2mib W | 641.2mib  | 293.6mib   | 725.4mib   | 399.2mib  | 250.3mib    | 171.6mib 
Disk read (sum)  | 9.6mib     | 8.2mib    | 8.2mib     | 8.3mib     | 372.7mib  | 11.8mib     | 2.2mib W 
Disk writ (sum)  | 2.7mib     | 2.7mib    | 2.6mib W   | 2.7mib     | 8.8mib    | 6.1mib      | 12.1mib  
Net recv (sum)   | 308.6mib   | 4.1gib    | 243.5mib   | 622mib     | 468.4mib  | 4.3gib W    | 4.2gib   
Net send (sum)   | 178.9mib   | 4.1gib    | 292mib     | 1.1gib     | 450.8mib  | 4.3gib      | 4.2gib   
TCP estab (avg)  | 431        | 1910      | 425        | 425        | 1562      | 553         | 423      
TCP sync (avg)   | 0          | 14        | 5          | 0          | 0         | 0           | 0        
TCP close (avg)  | 5          | 4         | 0          | 3          | 5         | 3           | 2        
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
