# Build-tool setup

Open **Settings > Build tools**, or **Build > Set up build tools**. The page lists the selected engine's requirements, detected tools and setup details. Choose a tool, review the action, then choose **Install selected tool**. There are no automatic installations on engine selection or build start. Existing detected tools are not upgraded. **Official downloads** is available for manual setup and when WinGet is unavailable.

| Tool | Setup action |
| --- | --- |
| Git for Windows | WinGet `Git.Git`, interactive installer |
| MSYS2 | WinGet `MSYS2.MSYS2`, interactive installer; select its root in the Compile form afterward |
| C/C++ GCC packages | In the selected MSYS2 root, `pacman -S --needed --noconfirm git make curl wget coreutils mingw-w64-ucrt-x86_64-gcc`; update MSYS2 first using its official guide if its database is out of date |
| Rust | WinGet `Rustlang.Rustup`, interactive installer; choose MSVC and install C++ build tools |
| Python | WinGet `Python.Python.3.14`, interactive installer; existing selected Python 3 installations can be used |
| Meson/Ninja | Create the app-private `tools/lc0` virtual environment, then pip-install Meson 1.12.1 and Ninja 1.13.2 there; global Python packages are unchanged |
| Microsoft C++ | WinGet `Microsoft.VisualStudio.2022.BuildTools`; choose Desktop development with C++, MSVC x64 tools and Windows SDK in its installer |
| Zig | Download official 0.17.0 Windows x64 zip, verify published SHA-256, extract into `tools/zig/0.17.0`, and check the compiler version |
| CUDA | Guided official CUDA 12.9 downloads; GPU/driver-specific setup is left to its installer |

WinGet verifies the package installer's hash. Its explicit install action accepts package/source agreements; native installer prompts and administrator approval remain the user's responsibility. Restart the app after installers change PATH. WinGet is supplied by Microsoft's App Installer; it is not bundled. Official sources: [WinGet install](https://learn.microsoft.com/windows/package-manager/winget/install), [MSYS2](https://www.msys2.org/), [Rust](https://www.rust-lang.org/tools/install), [Python](https://www.python.org/downloads/windows/), [Zig](https://ziglang.org/download/).

Rust owns detection, allowed package IDs/arguments, downloads, extraction, process execution, cancellation and logs. The GUI and CLI share these operations through protobuf/C ABI. The GUI blocks compilation while its setup runs and disables setup while compiling. Cancelling cannot guarantee rollback of an elevated or external installer; partially installed tools may remain. Closing asks before stopping active setup, waits for the worker and preserves logs. Log follow/pause uses the same selectable LogPresenter as engine builds.

Private tools and setup jobs live under `%LOCALAPPDATA%/Jupiter/ChessEngineCompiler/tools`. Every setup has a new `setup-jobs` folder and saved log; verified private installations also save setup provenance. App upgrades/uninstall preserve these tools, preferences and engine outputs. Tools are not bundled in the app installer. No unrelated toolchain, system PATH or user package set is silently changed.

CLI examples:

```powershell
.\chess-engine-compiler.exe tools list
.\chess-engine-compiler.exe tools install zig
.\chess-engine-compiler.exe tools install meson --python-path C:\Python314\python.exe
.\chess-engine-compiler.exe tools install gcc --msys-root C:\msys64
```
