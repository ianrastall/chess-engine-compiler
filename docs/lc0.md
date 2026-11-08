# Lc0 builds

Use **Release library** for stable Lc0 releases, or select **Lc0** in **Advanced build** for a specific tag, branch or commit. Its tool paths, backend and network are independent of Stockfish's selections. **Save settings** applies those choices to the library. The source adapter uses Meson/Ninja and MSVC with upstream's pinned lczero-common submodule. [Release library and recipes](release-library.md)

## Tools

Install Git for Windows, Python 3 and Visual Studio with **Desktop development with C++** and a Windows SDK. Select the installation folder containing `VC/Auxiliary/Build/vcvarsall.bat`. **Build > Detect build tools** detects Python on PATH, Visual Studio through vswhere, and CUDA through CUDA_PATH. Detection prefers Visual Studio 2022 because CUDA 12.x does not support the newer VS 2026 compiler.

Install Meson and Ninja in the Python environment selected in the app. For example, create a separate build environment:

```powershell
python -m venv "$env:LOCALAPPDATA\Jupiter\ChessEngineCompiler\tools\lc0"
& "$env:LOCALAPPDATA\Jupiter\ChessEngineCompiler\tools\lc0\Scripts\python.exe" -m pip install meson ninja
```

Paste that environment's `Scripts\python.exe` path into **Python executable**. The application checks the tools and records their versions; it never installs or updates them. Development verification used Python 3.12.10, Meson 1.10.2 and Ninja 1.13.0.

## Backend and network

- **NVIDIA GPU (CUDA):** requires the CUDA Toolkit (including nvcc/cuBLAS/runtime), a compatible NVIDIA driver and GPU. cuDNN is not required. CUDA 12.9 with VS 2022 is the tested toolchain. CUDA's native target builds for the GPU in this machine. `cuda-auto` chooses FP16 or FP32 according to GPU capability. Required cuBLAS and CUDA runtime DLLs and the toolkit license are copied into the result. A compatible GPU and driver are still needed on another computer.
- **CPU (Eigen):** uses upstream's Eigen backend without a separate BLAS library. It supports CPU evaluation and is slower than GPU evaluation for large networks. It uses the same MSVC/Python/Meson tools, with no CUDA Toolkit requirement.

Select an Lc0 release row and choose **Manage Lc0 weights** to open the weights controls in **Advanced build**. **Download and select default weights** gets the small T1 network, verifies its size and SHA-256, and selects the saved file. It reuses an existing verified copy. **Open weights folder** opens `%LOCALAPPDATA%/Jupiter/ChessEngineCompiler/networks`, which survives upgrades and uninstall. For other networks, use **Browse Lc0 network catalogue**, save the `.pb.gz` or `.pb` file in the weights folder, and select it with **Browse…**. Files elsewhere also work.

Choose **Save settings** to keep your selection. A blank weights path lets the release library use its verified T1 network automatically. If a custom file moves, the library blocks the build until you update or clear the path. The network is copied into the result with its SHA-256 recorded. The compiler verifies UCI and a standalone search using the selected backend and copied network. An incompatible network, missing runtime or unavailable GPU fails the build and retains the diagnostic log.

DirectX 12, ONNX/DirectML, cuDNN, OpenCL and external BLAS variants are not exposed. Upstream's DirectX 12 backend cannot load the current transformer networks. Additional backends need their own dependency handling and hardware verification before appearing as supported choices.

## Result and scripts

Each job has its own source directory and full build log. A verified result contains `lc0.exe`, `weights.pb.gz` or `weights.pb`, `lc0.config`, any required CUDA DLLs, engine/runtime licenses, `source.tar`, `dependencies.tar`, Meson compiler/build-option records and `build.json`. The dependency archive includes downloaded subproject sources and the common submodule; the manifest records its commit as well as the parent engine commit. Keep the result files together. `lc0.config` selects the copied network and backend when launching the engine.

**Export build scripts** supplies `engine.json`, `lc0.json`, `build-lc0.cmd`, `build-engine.cmd`, `preview-build.cmd`, `check-tools.cmd`, `build-engine.ps1` and the standalone CLI. The exported configuration preserves absolute tool and original network paths; those paths must exist when rebuilding elsewhere. Engine result networks and runtime libraries retain their upstream licenses. Application releases contain the compiler, not these engine results.

Use `chess-engine-compiler.exe lc0-weights <absolute-folder>` to download or verify the same default network from the CLI.

See [the editable configuration](../examples/lc0.json) and [upstream's build sources](https://github.com/LeelaChessZero/lc0/tree/v0.32.1).
