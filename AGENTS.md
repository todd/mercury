# Agent directives for Mercury

## Bug fixes

Any time a bug is fixed, a regression test must also be added that covers the
failure mode. The test should fail against the pre-fix code and pass after the
fix is applied. Add it in the most specific location that exercises the broken
behaviour:

- A unit test if the failure can be reproduced without a live server.
- An integration test (under `--features integration`) if a live IRC connection
  is required to trigger the condition.
