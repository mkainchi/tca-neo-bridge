# Building and testing

Everything builds on Windows 11 x64.

## Toolchain

| Tool | Used for | Install |
| --- | --- | --- |
| Rust (stable, MSVC) + `llvm-tools-preview` | daemon, coverage | `rustup component add llvm-tools-preview` |
| Windows SDK resource compiler (`rc.exe`) | embedded daemon icons | Included with the Visual Studio C++ workload / Windows SDK; discovered automatically by the Cargo build script. |
| Rust nightly + `llvm-tools-preview` | branch coverage | `rustup toolchain install nightly --component llvm-tools-preview` |
| cargo-llvm-cov | coverage gates | `cargo install cargo-llvm-cov --locked` |
| Visual Studio 2022 Build Tools (C++ workload) | driver | Visual Studio Installer |
| Windows Driver Kit 10.0.26100 | driver headers, UMDF libraries, `stampinf`, `infverif`, `inf2cat` | `winget install Microsoft.WindowsWDK.10.0.26100` |

The WDK's Visual Studio extension is **not** required: `driver\build.ps1` calls `cl`, `link` and the kit tools directly.

## Daemon

The repository root is a Cargo workspace: `common` (shared profile types, axis maths and Win32 declarations), `daemon` and `config-app`. Builds go to `target\` at the root.

```powershell
cargo build --release                                  # target\release\neo-daemon.exe and neo-config.exe
cargo run -p neo-daemon -- --source validation         # tray app with a synthetic input pattern instead of the TCA
```

The daemon needs administrator rights to create the virtual device, and `driver\install-driver.ps1` beside the executable for the **Install driver…** menu, so a development build does not offer installation. Use a release package (below) or run `scripts\install-driver.ps1` from `driver\out\package` after copying the script and a `SHA256SUMS` there.

Command-line options: `--data <dir>` (default `%LOCALAPPDATA%\tca-neo-bridge`), `--source tca|validation`, `--autostart`, `--self-test`.

### Configuration app

`neo-config.exe` is the native configuration window. The daemon's **Configure…** starts it when it sits beside `neo-daemon.exe` (as in `target\release`) and hands it the API port and token on its standard input; started any other way, it only shows where to open it from. If it is missing, **Configure…** shows a tray notice saying so. The app draws with wgpu on Direct3D 12 and uses the Windows fonts named by the design (Bahnschrift, Segoe UI Variable, Cascadia Mono), falling back to Segoe UI and Consolas; it bundles no fonts.

`scripts\package.ps1` ships both executables and runs `scripts\third-party-licenses.ps1`, which regenerates `THIRD_PARTY_LICENSES.md` (every crate compiled into them, with its licence files) from `cargo metadata`. Run it yourself after changing dependencies and commit the result.

### Icon artwork

`daemon/assets/` contains the editable SVG masters and checked-in ICO files. Each ICO contains transparent 32-bit images at 16, 20, 24, 32, 48, 64 and 256 pixels. The resource file keeps the unbadged bridge first (Explorer's executable icon), followed by idle, emulating, busy and error variants. Cargo embeds them into `neo-daemon.exe`; no icon files are needed at runtime. Windows builds fail if resource compilation fails.

After editing the SVGs, regenerate the ICOs from the repository root with `node scripts/build-icons.cjs`. This optional artwork tool requires the Apache-2.0-licensed `sharp` package (`npm install --no-save --package-lock=false sharp`); normal builds use the checked-in ICOs and require neither sharp nor Node. Check the small images on light and dark backgrounds before committing regenerated artwork.

## Driver

```powershell
powershell -ExecutionPolicy Bypass -File driver\build.ps1
```

This runs `frame_test.c`, generates `descriptor.h` from `driver\neo-descriptor.hex`, compiles `neohid.dll` (UMDF 2.33, `/W4 /WX /guard:cf /Qspectre /sdl`), stamps and verifies the INF (`infverif /w`), and creates the unsigned catalog with `inf2cat`. The result is in `driver\out\package`.

## Tests and coverage gates

```powershell
powershell -ExecutionPolicy Bypass -File scripts\check.ps1
```

`check.ps1` fails unless all of these hold:

- `cargo clippy --workspace --all-targets -D warnings` is clean.
- **100 %** of Rust lines, functions and regions are covered across the workspace (stable `cargo llvm-cov --workspace`).
- **100 %** of Rust branches are covered (nightly `cargo llvm-cov --workspace --branch`).

No coverage-exclusion attributes are used. All Win32 calls go through tables of function pointers (`HidApi`, `DeviceApi`, `UiApi`, `DriverApi`, `SystemApi`, `ConfigAppApi`, and the app's own `SystemApi`), which the tests replace with fakes; the real tables are only constructed. Each crate's `tests\binary.rs` runs the real executable so `main` is covered too; the app's plays the daemon (API and events) and opens a real window. The app's window tests (`egui_kittest`) compare against the snapshots in `config-app\tests\snapshots`; after an intended visual change, run them with `UPDATE_SNAPSHOTS=1` and review the new images. See [ENGINEERING.md](ENGINEERING.md#testing) for the reasoning.

## Release package

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package.ps1          # version from daemon\Cargo.toml
```

This produces `dist\tca-neo-bridge-<version>-x64.zip` and its `.sha256`. The zip contains `neo-daemon.exe`, `neo-config.exe`, the `driver\` folder (DLL, INF, unsigned catalog, `SHA256SUMS`, install/uninstall scripts, MS-PL licence), the README, the licence, the notices, `THIRD_PARTY_LICENSES.md` and SECURITY.md. The script checks the content against an explicit allowlist.

## Continuous integration

`.github/workflows/ci.yml` runs clippy and both coverage gates on `windows-latest`. It also builds the driver when the runner image provides the matching WDK, and skips that step with a notice otherwise.
