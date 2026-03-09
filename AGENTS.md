# Agent directives for Mercury

## Test-Driven Design

Always follow Test-Driven Design (TDD) principles when adding new features or fixing bugs:

1. **Write the test first.** Before writing any implementation code, write a test that describes the expected behaviour. The test must fail at this stage.
2. **Write the minimum implementation** needed to make the test pass.
3. **Refactor** the implementation as needed, keeping all tests green.

## Bug fixes

Any time a bug is fixed, a regression test must also be added that covers the
failure mode. The test should fail against the pre-fix code and pass after the
fix is applied. Add it in the most specific location that exercises the broken
behaviour:

- A unit test if the failure can be reproduced without a live server.
- An integration test (under `--features integration`) if a live IRC connection
  is required to trigger the condition.
