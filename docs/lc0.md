# Lc0 builds

Select **Lc0** in the Engine field. Its source revision, tool paths, backend and network are independent of Stockfish's selections. The initial revision is `v0.32.1`; an official branch or commit can also be selected. The source adapter uses Meson/Ninja and MSVC, with upstream's pinned lczero-common submodule. It does not run upstream's interactive `build.cmd` or delete existing checkouts.

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

Choose a `.pb.gz` or `.pb` network from [Lc0's network catalogue](https://lczero.org/play/networks/bestnets/). Compressed networks are supported directly. The network remains a separate file and is copied into the result with its SHA-256 recorded. Large networks require more memory. A successful UCI handshake alone does not prove a network can run: the compiler also performs a short standalone search using the selected backend and copied network. An incompatible network, missing runtime or unavailable GPU fails the build with its full diagnostic log retained.

DirectX 12, ONNX/DirectML, cuDNN, OpenCL and external BLAS variants are not exposed. Upstream's DirectX 12 backend cannot load the current transformer networks. Additional backends need their own dependency handling and hardware verification before appearing as supported choices.

## Result and scripts

Each job has its own source directory and full build log. A verified result contains `lc0.exe`, `weights.pb.gz` or `weights.pb`, `lc0.config`, any required CUDA DLLs, engine/runtime licenses, `source.tar`, `dependencies.tar`, Meson compiler/build-option records and `build.json`. The dependency archive includes downloaded subproject sources and the common submodule; the manifest records its commit as well as the parent engine commit. Keep the result files together. `lc0.config` selects the copied network and backend when launching the engine.

**Export build scripts** supplies `engine.json`, `lc0.json`, `build-lc0.cmd`, `build-engine.cmd`, `preview-build.cmd`, `check-tools.cmd`, `build-engine.ps1` and the standalone CLI. The exported configuration preserves absolute tool and original network paths; those paths must exist when rebuilding elsewhere. Engine result networks and runtime libraries retain their upstream licenses. Application releases contain the compiler, not these engine results.

See [the editable configuration](../examples/lc0.json) and [upstream's build sources](https://github.com/LeelaChessZero/lc0/tree/v0.32.1).
