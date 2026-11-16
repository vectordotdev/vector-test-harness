# sighup_correctness

The local check verifies exact records before and after a SIGHUP output switch.
It waits for the product's native reload-completion signal before sending the
second record and requires the same main PID, clean shutdown and drained receivers.
It produces no performance score; the old remote timing case remains retired.

Profiles exist for Vector, Fluent Bit, Telegraf, Fluentd and Logstash. Fluentd's
supervisor can replace its worker during reload. Filebeat and Splunk output
changes do not satisfy this same-main-process SIGHUP contract.

```sh
vth check sighup_correctness --vector-bin /path/to/vector
vth check sighup_correctness --subject telegraf --subject-bin /path/to/telegraf
```
