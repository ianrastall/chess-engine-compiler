# Patricia, Koivisto and Seer

Choose the engine in **Compile** and select an MSYS2 root. **Settings > Build tools** offers explicit installation of the required UCRT64 packages. Git for Windows or MSYS2 Git is required for source acquisition.

| Engine release | Compiler | CPU target | Embedded network |
| --- | --- | --- | --- |
| [Patricia 5.1](https://github.com/Adam-Kulju/Patricia/releases/tag/5.1) | UCRT64 Clang and LLD | x86-64-v3 | Fingolfin, Finarfin and Feanor from the engine source |
| [Koivisto v9.0](https://github.com/Luecx/Koivisto/releases/tag/v9.0) | UCRT64 GCC | AVX2 | `default.net` from the [pinned networks commit](https://github.com/Luecx/KoivistoNetworks/tree/8cae81230c6bf36733a292ee7633602dfbbe9c49) |
| [Seer v2.8.0](https://github.com/connormcmonigle/seer-nnue/releases/tag/v2.8.0) | UCRT64 GCC | x86-64-v3 | [`q0x35ddef41.bin`](https://github.com/connormcmonigle/seer-training/releases/tag/0x35ddef41) |

The GUI selects these releases automatically. CLI and exported scripts accept the listed tag or its exact commit. The adapters support regular compilation and optional symbol stripping. Seer supports parallel jobs. All three include upstream Syzygy code. Tablebase files are separate.

The result includes the engine, source archive, license and a manifest with the resolved commit, compiler version, network paths and hashes, and benchmark results. Koivisto and Seer verify their network downloads against pinned SHA-256 values before compilation. Patricia's three networks come from the verified source commit. The UCI handshake and benchmark run with only System32 on PATH.

See the [example configurations](../examples) and [validation record](validation.md).
