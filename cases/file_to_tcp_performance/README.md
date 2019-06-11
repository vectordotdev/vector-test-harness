# File To TCP Performance Test

This test measures the performance of the subject's ability to tail data from local
files and send it over TCP.

## Design

1. Each subjects is configured to tail a single glob pattern and send data over TCP.
2. The TCP consmer is a separate instance to help ensure performance is not tainted.
3. Each subject is started and ran for 60 seconds.
4. This test contains 2 separate configurations: `multi_file` and `single_file` to test
   the performance of both. The `multi_file` configuration simply creates 10 files.
5. [Performance data][performance_data] is collected and persisted to S3 for analysis.

## Results

### Multi-file

```
$ bin/compare -t file_to_tcp_performance -c multi_file

Metric           | filebeat   | fluentbit | fluentd   | logstash   | splunk_hf  | splunk_uf | vector      
-----------------|------------|-----------|-----------|------------|------------|-----------|-------------
Disk Thrpt (avg) | 7.8mib/s   | 35mib/s   | 26.1mib/s | 3.1mib/s   | 39mib/s    | 40.1mib/s | 76.7mib/s W 
CPU sys (max)    | 4.7        | 4.5 W     | 6.6       | 46.5       | 34.7       | 6.6       | 23          
CPU usr (max)    | 63.8       | 50.3      | 50.5      | 98         | 84.7       | 18.1 W    | 74.9        
Load 1m (avg)    | 1          | 0.6 W     | 0.6       | 2.2        | 2.6        | 0.6       | 1.5         
Mem used (max)   | 170.7mib W | 370.3mib  | 890.6mib  | 763.7mib   | 442.6mib   | 241.4mib  | 188.1mib    
Disk read (sum)  | 466.9mib   | 2gib      | 1.5gib    | 187.3mib W | 2.3gib     | 2.4gib    | 4.6gib      
Disk writ (sum)  | 177.6mib   | 6.1mib W  | 11.6mib   | 602.6mib   | 54.3mib    | 24.4mib   | 16.1mib     
Net recv (sum)   | 240.6kib   | 6.6mib    | 2.3mib    | 2.4mib     | 14.7mib    | 3mib      | 22.4mib W   
Net send (sum)   | 167.3mib   | 2gib      | 573.7mib  | 65.2mib    | 1007.4mib  | 1.2gib    | 2.3gib      
TCP estab (avg)  | 183        | 1070      | 183       | 175        | 1358       | 122       | 186         
TCP sync (avg)   | 0          | 0         | 0         | 0          | 0          | 0         | 0           
TCP close (avg)  | 0          | 1         | 0         | 0          | 0          | 0         | 0           
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

### Single-file

```
$ bin/compare -t file_to_tcp_performance -c single_file

```

## Try It

```bash
bin/test -t file_to_tcp_performance -c <multi_file|single_file>
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
