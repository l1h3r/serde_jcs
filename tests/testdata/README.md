# Test Data

The six input/output pairs are from [cyberphone/json-canonicalization](https://github.com/cyberphone/json-canonicalization/tree/32a48195b9b20fe67b1795e39e4509f14d860b45/testdata), pinned at commit `32a48195b9b20fe67b1795e39e4509f14d860b45`.

`numbers.jsonl` contains 2,000 rows selected from the 125,000 source-number rows in suite-sweep's pinned [number vectors](https://github.com/cyberphone/json-canonicalization/releases/download/es6testfile/es6testfile100m.txt.gz); the 19 generated boundary rows are excluded. It keeps the first 200 rows and then selects 1,800 evenly spaced rows from the remainder at index `floor((k + 0.5) * (N - 200) / 1800)`, with `k` from 0 through 1799 and `N = 125000`. Original case IDs are retained.

These test vectors are distributed under the Apache License, Version 2.0; see `LICENSE`.
