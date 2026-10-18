# Contributing

Thanks for helping. The project has two release lines:

| Line | Branch | For |
| --- | --- | --- |
| **TCA-NEO bridge** (stable) | `main` | TCA Sidestick Airbus + TCA Quadrant Eng 1&2. Tested on that hardware. |
| **Any-TM-HOTAS experimental bridge** | `release/any-tm-hotas-experimental-bridge` | Any other Thrustmaster HOTAS. Untested by the maintainer, who owns only the TCA Airbus pair. |

## Ways to help

- **Test reports (most wanted).** Own a T.16000M, TWCS, Warthog, T.Flight Hotas, TCA Boeing or another Thrustmaster controller? Follow the [guide for players](https://github.com/mkainchi/tca-neo-bridge/blob/release/any-tm-hotas-experimental-bridge/docs/EXPERIMENTAL-ANY-HOTAS.md) and open a [HOTAS test report](https://github.com/mkainchi/tca-neo-bridge/issues/new?template=hotas-test-report.yml) with the output of `neo-daemon.exe --probe`.
- **Default mappings.** If you found settings that work well for your model (axis sources, inversions, button mapping), attach your `config.json` (in `%LOCALAPPDATA%\any-tm-hotas-bridge`) to your report, or propose it as a pull request.
- **Code**: bug fixes, support for a device the generic path misreads, page improvements.
- **Diagrams**: an SVG of your controller with numbered callouts, like `daemon/web/device/sidestick-right.svg`. Draw it yourself; never copy vendor artwork.

## Development setup

Follow [docs/BUILDING.md](docs/BUILDING.md) (Rust stable and nightly, cargo-llvm-cov, Node.js, and the WDK for the driver). Before every pull request, run from the repository root:

```powershell
./scripts/check.ps1
```

It runs clippy with warnings as errors and enforces **100 % coverage**: Rust lines, functions and regions (stable), Rust branches (nightly), and statements, branches, functions and lines of the configuration page. A pull request that lowers coverage, adds coverage exclusions or relaxes a threshold will not be merged. Code that talks to Windows goes through the function-pointer tables in `daemon/src` (`HidApi`, `DeviceApi`, …) so tests can replace it with the fakes in `daemon/tests/common/mod.rs`.

## Where pull requests go

- **Stable fixes** (TCA behaviour, driver, HidHide, tray, security) → `main`. They are cherry-picked into the experimental branch.
- **HOTAS support** (new devices, the generic HID path, per-model defaults, the multi-device page) → `release/any-tm-hotas-experimental-bridge`.
- **Adding a model**: an entry in `daemon/src/models.rs` (product id, stable model id, display name), a test built from the reporter's `--probe` data (layout and a decoded sample), and a row in the players' guide. Only add a product id seen in a probe report or a public device list.

## Rules the code keeps

- Never log, display or send device instance paths or serial numbers. `--probe` output and logs must stay free of them.
- No proprietary source code (including decompiled or leaked code), and no vendor software, firmware, SDK or artwork, in the repository or in builds. Trademarks only to name the hardware.
- Nothing leaves the PC: no telemetry, no network access beyond the loopback configuration page.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat(hid): …`, `fix(tray): …`, `docs: …`).
- Keep player-facing text plain and short; say what to do, not why the code does it.

By contributing you agree that your work is published under the project's [MIT licence](LICENSE).
