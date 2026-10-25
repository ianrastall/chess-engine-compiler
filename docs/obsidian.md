# Obsidian

[Obsidian](https://github.com/gab8192/Obsidian) is GPL-3.0 C++ with a documented Windows GCC build. Default source: `v16.0`. Install MSYS2 UCRT64 GCC, git, make and curl. Choose the MSYS2 root. The CLI, exported scripts and GUI all call the Rust adapter.

Targets use upstream names: native, sse2, ssse3, avx2, avx2-pext and avx512, filtered for CPU/OS support. Automatic selection avoids PEXT on older AMD Zen. `sse2` also requires POPCNT. Jobs apply to regular and PGO compilation. PGO is optional and runs the built-in benchmark; missing profiles fail compilation. Symbol stripping is honored independently of upstream's unconditional `-s` flag. The adapter supplies fixed architecture flags and uses upstream optimization settings.

The source's `DEFAULT_NET` identifies the network downloaded from Obsidian-nets. Its SHA-256 and URL are recorded; no upstream digest is available for comparison. The static result embeds the network, passes UCI and a completed positive-node benchmark with only System32 on PATH, and retains the source archive, license, compiler, resolved commit, options and full log. Every job has a new directory. Historical makefile layouts may be rejected; v16.0 is the tested baseline.
