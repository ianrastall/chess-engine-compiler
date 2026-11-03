# PlentyChess, Stormphrax and Viridithas

Open **Compile**, choose an engine, and select an output folder. The default source and network are paired below. The compiler checks the published SHA-256 of these networks, embeds the weights, then checks UCI and the engine's own search benchmark from the result folder with only System32 on PATH. The result keeps the exact commit, compiler and tools, network URL and digest, benchmark count, source archive, license and full log. Each run gets a new job folder.

| Engine | Default source | Network | Windows build |
| --- | --- | --- | --- |
| [PlentyChess](https://github.com/Yoshie2000/PlentyChess/releases/tag/b-v8.0.0) | `b-v8.0.0` | [0178r](https://github.com/Yoshie2000/PlentyNetworks/releases/tag/0178r) | UCRT64 Clang and LLD, Haswell-class AVX2 or BMI2, optional PGO |
| [Stormphrax](https://github.com/Ciekce/Stormphrax/releases/tag/v8.0.0) | `v8.0.0` | [undertown](https://github.com/Ciekce/stormphrax-nets/releases/tag/undertown) | UCRT64 Clang and LLD, native or Haswell-class AVX2/BMI2, regular build |
| [Viridithas](https://github.com/cosmobobak/viridithas/releases/tag/v20.0.0) | `v20.0.0` | [sandhi-s2-b200](https://github.com/cosmobobak/viridithas-networks/releases/tag/v109) | Stable Windows MSVC Rust, native CPU, optional PGO |

## C++ builds

Select the MSYS2 root and use **Settings > Build tools > C/C++ (MSYS2 Clang and LLVM)** to review and install the needed packages. PlentyChess preprocesses its network before compilation. With PGO enabled it instruments the regular benchmark, merges the collected profile, and forces a rebuild of all engine objects with that profile. Both PlentyChess targets require Haswell-class CPU features. The recommendation uses the `avx2` target without PEXT move generation on older AMD CPUs.

Stormphrax preprocesses its network through the upstream permutation tool. Its native target uses upstream CPU detection and avoids PEXT on older AMD CPUs. Its v8 build rules support regular compilation.

For other PlentyChess and Stormphrax revisions, the adapter expects compatible upstream make targets and a `network.txt` pointing to the corresponding official network release. The log and manifest record whether the network matched a known published digest.

## Viridithas builds

Use Rustup with the installed `stable-x86_64-pc-windows-msvc` toolchain, Microsoft C++ build tools and a Windows SDK, Git and curl. For PGO, install the matching profiler explicitly before building.

```powershell
rustup component add llvm-tools --toolchain stable-x86_64-pc-windows-msvc
```

The compiler uses the release's native CPU build with static MSVC CRT and without Syzygy. Its source-to-network mapping is verified for v20.0.0. Upstream omits a Cargo lockfile at this revision. The job resolves dependencies once, uses the same lockfile across PGO passes, and saves that lockfile with the result. Builds never install or update the Rust toolchain.

The installer distributes the compiler. Each locally built engine and network keeps its upstream licensing terms.
