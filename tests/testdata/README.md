# Test Data

The input/output pairs come from [cyberphone/json-canonicalization](https://github.com/cyberphone/json-canonicalization/tree/32a48195b9b20fe67b1795e39e4509f14d860b45/testdata), pinned at commit `32a48195b9b20fe67b1795e39e4509f14d860b45`.

`numbers.jsonl` has 2,000 of the 125,000 source-number rows in suite-sweep's pinned [number vectors](https://github.com/cyberphone/json-canonicalization/releases/download/es6testfile/es6testfile100m.txt.gz). It leaves out the 19 generated boundary rows. It takes the first 200 rows. It then takes 1,800 evenly spaced rows from the rest, at index `floor((k + 0.5) * (N - 200) / 1800)`, with `k` from 0 through 1799 and `N = 125000`. Each row has its original case ID.

The test vectors are under the Apache License, Version 2.0. See `LICENSE`.
