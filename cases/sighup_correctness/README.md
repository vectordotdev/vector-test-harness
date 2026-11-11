# SIGHUP Correctness Test

This test verifies that the subject correctly responds to the `SIGHUP` signal, gracefully
reloading configuration changes.

## Design

1. 2 TCP consumers are set up, each on separate hosts.
2. Each subject is initially configured to send data to the first TCP consumer.
3. A test line is sent to the subject.
4. The subject's configuration file is updated to send data to the second TCP consumer.
5. The `SIGHUP` signal is sent to the subject's project.
6. A second test line is sent to the subject.
7. Assert that the first TCP server received the first line and the second TCP server
   received the second line.

## Historical results

These results describe earlier Vector versions and workload definitions. They
have not been re-measured with the Vector 0.61 configurations; establish a new
baseline before comparing performance or claiming current correctness.

|     Subject | Result  |
|------------:|:-------:|
|    Filebeat |    ❌    |
|   FluentBit |    ❌    |
|     FluentD |    ❌    |
|    Logstash |  ⚠️[1]  |
|   Splunk HF |    ✅    |
|   Splunk UF |    ✅    |
|  **Vector** |    ✅    |

1. Logstash failed the test because it does not gracefully reload change. Instead of performs
   a full restart, making Logstash unavaible for a period of time. Because of this it failed to
   receive the second line. In order for this test to pass we had to wait 30 seconds to send
   the second line.

## Try It

Run the local Vector check without cloud infrastructure:

```sh
just check-case sighup_correctness --vector-bin /path/to/vector
```

For the remote case:

```sh
just run -t sighup_correctness -s vector
```

## Resources

* [Setup][setup]
* [Development][development]
* [How it works][how_it_works]
* [Vector docs][docs]
* [Vector repo][repo]
* [Vector website][website]


[development]: /README.md#development
[docs]: https://vector.dev/docs/
[how_it_works]: /README.md#how-it-works
[repo]: https://github.com/vectordotdev/vector
[setup]: /README.md#setup
[website]: https://vector.dev
