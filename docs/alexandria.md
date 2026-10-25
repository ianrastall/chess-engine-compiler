# Alexandria

[Alexandria](https://github.com/PGG106/Alexandria) is GPL-3.0 C++23. Default source: `v9.1.0`. Install MSYS2 UCRT64 GCC, make, git and curl. The adapter builds regular static AVX2 or BMI2 executables, with parallel compilation. PGO is unavailable. AVX2/FMA and the other selected instruction features are required. Automatic selection avoids PEXT on older AMD CPUs.

The adapter reads the full SHA-256 from source `net-hash.txt`, finds the matching `nn.net` release asset in the official Alexandria-networks repository, downloads it and verifies its exact hash. It runs the source's network preprocessor before embedding the result and records both original and processed hashes. GitHub network metadata uses anonymous API access; rate-limit failures remain explicit. The source reference benchmark is checked when available. A mismatch fails the job.

The tested v9.1.0 source selects `net10`, not the current latest network. Both AVX2 and BMI2 match its 7,415,803-node benchmark. The scalar baseline did not match and is excluded from both the UI and Rust validation. The `_ROOT=.` Make argument avoids an upstream absolute-path parsing problem in folders with spaces; no upstream files are patched. Results retain source/archive/license, compiler/options, hashes, exact commit, reference/actual nodes and full log, and must pass UCI and standalone search with only System32 on PATH.
