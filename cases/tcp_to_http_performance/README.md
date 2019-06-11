# TCP To HTTP Performance Test

This test measures the HTTP output performance for each subject.

## Design

1. Each subjects is configured to receive data over TCP and output over HTTP.
2. The TCP producer and HTTP consumer are separate instances to help ensure performance is not
   tainted.
3. Each subject is started and ran for 60 seconds.
4. [Performance data][performance_data] is collected and persisted to S3 for analysis.

## Results

```
$ bin/comare -t tcp_to_http_performance

Metric           | fluentbit  | fluentd    | logstash     | vector     
-----------------|------------|------------|--------------|------------
Test count       | 1          | 1          | 1            | 1          
IO Thrpt (avg)   | 19.6mib/s  | 897.8kib/s | 2.7mib/s     | 26.7mib/s W
Disk Thrpt (avg) | 138.4kib/s | 138.1kib/s | 142.5kib/s W | 25.2kib/s  
Duration (avg)   | 63s        | 63s        | 63s          | 61s        
Duration (max)   | 63s        | 63s        | 63s          | 61s        
CPU sys (max)    | 6.5        | 17.3       | 8            | 5.5 W      
CPU usr (max)    | 49.8       | 36.7 W     | 40.9         | 94.5       
Load 1m (avg)    | 0.5 W      | 0.7        | 1.2          | 0.9        
Mem used (max)   | 1.1gib     | 188.9mib W | 772mib       | 233mib     
Disk read (sum)  | 8.5mib     | 8.5mib     | 8.8mib       | 1.5mib W   
Disk writ (sum)  | 11.2mib    | 11.2mib    | 9.8mib       | 2.1mib W   
Net recv (sum)   | 1.2gib     | 55.2mib    | 167.1mib     | 1.6gib W   
Net send (sum)   | 1.9gib     | 79.1mib    | 291.2mib     | 2.9gib     
TCP estab (avg)  | 3218       | 377        | 439          | 487        
TCP sync (avg)   | 2          | 9          | 0            | 0          
TCP close (avg)  | 199        | 50         | 78           | 8          
-------------------------------------------------------------------------------------------------------------
W = winner
fluentbit = 1.1.0
fluentd = 3.3.0-1
logstash = 7.0.1
vector = 0.2.0-6-g434bed8
```

## Try It

```bash
bin/test -t tcp_to_http_performance
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
