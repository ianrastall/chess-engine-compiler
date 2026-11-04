# Release library

Version 1.0 opens on the Stockfish and Lc0 release library. The startup check runs in the background, with **Skip check** available while it runs and a persistent switch in **Settings > Release library**. **Check releases** refreshes both engines. When the entire upstream catalog fits on one page, the app reuses GitHub's ETag to check for changes. If a request fails, the app keeps that engine's previous catalog and reports the error.

Choose both engines or one engine. **New releases** initially includes the two most recently published stable releases for each engine. The queue builds the latest release of each engine before taking the previous releases. Later checks include releases published since the first observation, even if several arrived while the app was closed. The library records release IDs and keeps verified outputs across runs. **Latest release per engine** narrows the list, and **All releases** includes earlier stable releases with their eligibility.

Select **Build new releases** to start the queue. Jobs run in order with a separate source folder and log for each release. Failed and cancelled jobs preserve their files and status, and subsequent ready jobs continue after a failure. A later attempt skips managed releases whose verified manifest and executable are still present. Select a row to open its result or job log. Build-tool setup requires a separate action in Settings. In **Advanced build**, **Save settings** saves the tool paths, output folder and Lc0 backend or weights file used by the library. The advanced form still accepts custom refs.

## Build recipes

The [Stockfish recipe](../core/domain/recipes/stockfish.json) and [Lc0 recipe](../core/domain/recipes/lc0.json) are bundled, typed JSON. They describe the official repository and release tag format, the earliest supported source family, source layout and version-dependent build capabilities. Rust interprets these rules, invokes tools with structured arguments and records the recipe ID and exact source commit. The library fetches tags explicitly from the official repository. An unfamiliar source layout is reported as **Needs recipe update** before compilation where the recipe can detect the mismatch.

| Engine | Managed source family | Build and verification |
| --- | --- | --- |
| Stockfish | `sf_17.1` onward, with the recognized Windows Makefile and default NNUE declarations | MSYS2 UCRT64 MinGW GCC. Intersect CPU recommendations with the selected source's reported targets, then run PGO and strip. The recipe handles one or several networks and checks each SHA-256 against the hash prefix in its source-declared filename. The result must answer UCI and complete a standalone benchmark with a positive node count. |
| Lc0 | `v0.32.0` onward, with the recognized Meson option file and backend controls | MSVC, Meson and Ninja. Enable NVIDIA CUDA or CPU Eigen, use AVX2 compilation on compatible x64 CPUs, and request native GPU targeting for CUDA. Pin and record the common submodule. Use the recipe's small T1 network with size and SHA-256 checks, or the saved custom weights. A standalone neural search must pass on the requested backend with the result's network and runtime files. |

The Stockfish research describes the newer runtime-dispatched universal binary. The managed workflow chooses a single host CPU target and uses upstream's PGO workload. Stockfish's source controls its NNUE topology. Lc0 chooses its search and inference implementation through the selected source and backend. DirectX, ONNX, cuDNN and other backend families need separate recipes and hardware verification before they can be offered.

When extending a recipe, compare the exact upstream tag's build files and dependencies, test a fresh job, and check the executable, manifest and saved log. A published release can need a new rule even when its tag fits the current version range. See the [validation record](validation.md).
