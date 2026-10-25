# Berserk

[Berserk](https://github.com/jhonnold/berserk) is a GPL-3.0 C engine by Jay Honnold with an NNUE network embedded at compile time. The compiler builds it with the same MSYS2 UCRT64 GCC toolchain as Stockfish.

## Tools

In the MSYS2 UCRT64 terminal:

```sh
pacman -S --needed git make wget mingw-w64-ucrt-x86_64-gcc
```

`sha256sum` comes with MSYS2's base coreutils. curl is accepted in place of wget. Clang is not used: upstream's Clang PGO path needs `llvm-profdata`, which a standard MSYS2 Clang install does not provide.

## What a build does

1. Fetches `jhonnold/berserk` at the selected tag, branch or commit. The default is release tag `14`.
2. Runs upstream `make download-network`, which fetches the network named in that source's makefile from `jhonnold/berserk-networks`. Berserk names each network by the first 12 hex digits of its SHA-256. The compiler recomputes the full hash and fails the build if the name and hash disagree.
3. Compiles with `make all CC=gcc ARCH=<target> LIBS="-pthread -lm -static"`. The network is embedded with `INCBIN`, so the result is a single executable.
4. Optionally strips symbols, then copies `berserk.exe`, `LICENSE`, a source archive and `build.json` into the result folder.
5. Runs `uci`/`isready` and `berserk bench 10` from the result folder with only the Windows system directory on PATH.

Upstream adds `-static` only when the makefile runs under cmd.exe. MSYS2 bash takes the other branch, so the compiler passes the static runtime explicitly. Results import only `KERNEL32.dll` and the Windows Universal CRT.

Berserk compiles all sources in one compiler invocation, so **Parallel jobs** is hidden for Berserk.

## CPU targets

Berserk uses its own `ARCH` names: `native`, `x86-64` (64-bit plus POPCNT, no SIMD), `sse41`, `avx2`, `avx512`, and `-pext` variants that add BMI2 move generation. Only targets this CPU supports are offered. The recommendation avoids PEXT on older AMD Zen processors. `x86-64` is very slow for Berserk because its network code has no SIMD path there; use it only when the binary must run on very old CPUs.

## PGO

PGO is off by default. Upstream's `pgo` target passes `-fprofile-generate=pgo`; GCC on Windows writes those profiles under mangled paths that `-fprofile-use=pgo` cannot read back, so upstream's target silently produces an unoptimized build. When PGO is selected, the compiler runs the same steps with plain `-fprofile-generate`/`-fprofile-use` (profiles beside the sources), trains with `berserk bench 13`, and fails the build if GCC reports any missing profile.

With profiles correctly applied, five alternating `bench 13` runs on an i7-10700 averaged 1.68 M nps for PGO and 1.74 M nps for a regular `avx2-pext` build. GCC PGO therefore isn't recommended for Berserk 14; the option remains for experimentation.

## Result record

`build.json` records the commit, options, GCC version, network file name, full SHA-256 and download URL, whether PGO profiles were applied, and the verification results. The network is part of the executable and is not copied separately; its source URL and hash identify it. The source archive contains tracked upstream source only.
