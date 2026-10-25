# Caissa

[Caissa](https://github.com/Witek902/Caissa) is MIT-licensed C++20. Default source: `2.0`. The adapter uses MSYS2 UCRT64 GCC, make, git and curl. It invokes upstream's `ob` target with fixed, CPU-validated flags and static runtime linkage. Targets are x86-64, avx2 and bmi2. Automatic selection avoids slow PEXT on older AMD CPUs. Parallel jobs apply; PGO is hidden and rejected.

The source-selected network is embedded. For 2.0, `eval-ml-8-152B-spsa.pnn` is checked against the full published SHA-256. Other network names have their computed hash recorded without claiming an upstream match. Every result must pass UCI and a positive-node standalone benchmark with only System32 on PATH. The job retains source, archive, license, exact commit, compiler/options, network URL/hash, verification record and full log. Historical source layouts are not promised compatible.
