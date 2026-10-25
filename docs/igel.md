# Igel

[Igel](https://github.com/vshcherbyna/igel) is GPL-3.0-or-later C++ with a documented GCC `basic` build. Default: `3.8.0`. Install MSYS2 UCRT64 GCC, make, git and curl. Its standard NNUE requires AVX2 and POPCNT. The source-selected network is embedded in a static executable with Syzygy support. No tablebase files are bundled.

Supported targets: native, avx2 and avx2-pext, filtered for CPU support. Automatic selection avoids slow PEXT on older AMD CPUs. Explicit targets override upstream native probing so the executable uses the selected instruction set. Upstream compiles in one compiler invocation; the form hides parallel jobs. Clang PGO, pure HCE and the separate Fischer Random network build are not included. This does not disable the standard engine's UCI_Chess960 option. PGO is hidden and rejected in the shared Rust validator.

The source's makefile identifies `weights/<network>` and its release URL. The adapter restricts the URL to the official Igel repository, checks the SHA-256 prefix encoded in its name, and for 3.8.0's `6b953e78.standard` checks the full published SHA-256 `6b953e786f925e9df8a641b170087aae1b40071bd3495ba3b91737756b9f06d3`. The network is 110,513,572 bytes.

Each result passes UCI and a standalone `bench` with only System32 on PATH. When the source commit supplies `bench: <nodes>`, a different node count fails verification. Provenance records actual/reference nodes, network SHA-256/URL, commit, compiler, options and full log; source and license are archived. Source layouts before the tested release may be rejected. Syzygy compilation is tested; probes using actual external tablebase files are not claimed verified.
