# File to S3 Reliability Test

This is a long-running test of Vector tailing a file and sending the data to S3.
It uses the [`verifiable-logger`][0] project both to generate the log data and
verify that it all reaches S3.

## Try it

You can run this test via:

```
test -t file_to_s3_reliability
```

## Resources

* [Setup][setup]
* [Development][development]
* [How it works][how_it_works]
* [Vector docs][docs]
* [Vector repo][repo]
* [Vector website][website]


[0]: https://github.com/timberio/verifiable-logger
