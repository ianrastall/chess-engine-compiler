# Release library

The release library covers Stockfish, Lc0 and Obsidian. The startup check runs in the background, with **Skip check** available while it runs and a persistent switch in **Settings > Release library**. **Check releases** refreshes all three engines. When the entire upstream catalog fits on one page, the app reuses GitHub's ETag to check for changes. If a request fails, the app keeps that engine's previous catalog and reports the error.

Choose **All engines** or one engine. **New releases** initially includes the two most recently published stable releases for each engine. The queue builds the latest release of each engine before taking the previous releases. Later checks include releases published since the first observation, even if several arrived while the app was closed. The library records release IDs and keeps verified outputs across runs. **Latest release per engine** narrows the list, and **All releases** includes earlier stable releases with their eligibility.

Select **Build new releases** to start the queue. Jobs run in order with a separate source folder and log for each release. Failed and cancelled jobs preserve their files and status, and subsequent ready jobs continue after a failure. A later attempt skips managed releases whose verified manifest and executable are still present. Select a row to open its result or job log. Build-tool setup requires a separate action in Settings. In **Advanced build**, **Save settings** saves the tool paths, output folder and Lc0 backend or weights file used by the library. The advanced form still accepts custom refs.

For an Lc0 row, **Manage Lc0 weights** opens the controls for selecting a file, downloading the verified default, browsing the official catalogue, and opening the persistent weights folder. See [Lc0 builds](lc0.md#backend-and-network).

The library reuses an Lc0 result when the release, backend and weights SHA-256 match and the executable and bundled weights are still present. A copy of the same network can reuse the result. Selecting different weights makes the release eligible to build again, with the previous job kept in its own folder.

## Build recipes

The [Stockfish](../core/domain/recipes/stockfish.json), [Lc0](../core/domain/recipes/lc0.json) and [Obsidian](../core/domain/recipes/obsidian.json) recipes are bundled, typed JSON. They identify the official repositories, supported release families and tag formats. Rust fetches the exact tags, checks the source layout, invokes tools with structured arguments and records the recipe ID and resolved commit. An unfamiliar source layout is reported as **Needs recipe update** before compilation where the recipe can detect the mismatch.

| Engine | Managed source family | Build and verification |
| --- | --- | --- |
| Stockfish | `sf_17.1` onward, with the recognized Windows Makefile and default NNUE declarations | MSYS2 UCRT64 MinGW GCC. Intersect CPU recommendations with the selected source's reported targets, then run PGO and strip. The recipe handles one or several networks and checks each SHA-256 against the hash prefix in its source-declared filename. The result must answer UCI and complete a standalone benchmark with a positive node count. |
| Lc0 | `v0.32.0` onward, with the recognized Meson option file and backend controls | MSVC, Meson and Ninja. Enable NVIDIA CUDA or CPU Eigen, use AVX2 compilation on compatible x64 CPUs, and request native GPU targeting for CUDA. Verify the source-pinned Abseil, Eigen and zlib archives over HTTPS and pin the common submodule. Keep the source and build trees short during compilation, then retain the checkout with the job. Use the recipe's small T1 network with size and SHA-256 checks, or the saved custom weights. A standalone neural search must pass on the requested backend with the result's network and runtime files. |
| Obsidian | `v15.0` onward, with the recognized root Makefile, network declaration, flags and `nopgo` target | MSYS2 UCRT64 GCC. Use the recommended CPU target, PGO and stripping. Embed the source-selected network and record its SHA-256. The result must answer UCI and complete a standalone benchmark. |

The Stockfish research describes the newer runtime-dispatched universal binary. The managed workflow chooses a single host CPU target and uses upstream's PGO workload. Stockfish's source controls its NNUE topology. Lc0 chooses its search and inference implementation through the selected source and backend. DirectX, ONNX, cuDNN and other backend families need separate recipes and hardware verification before they can be offered.

When extending a recipe, compare the exact upstream tag's build files and dependencies, test a fresh job, and check the executable, manifest and saved log. A published release can need a new rule even when its tag fits the current version range. See the [validation record](validation.md).
