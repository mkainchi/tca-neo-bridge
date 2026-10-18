# TCA-NEO bridge

Fly **ACE COMBAT 8** with a **Thrustmaster TCA Sidestick Airbus Edition** and **TCA Quadrant Eng 1&2**, using the game's **native HOTAS controls**.

ACE COMBAT 8 natively supports the Thrustmaster T.Flight Hotas NEO, but not the TCA Airbus Edition. TCA-NEO bridge is a small Windows tray application plus a user-mode driver. Together they present your TCA sidestick and quadrant to Windows as a virtual T.Flight Hotas NEO, so the game uses its NEO profile: separate throttle, camera mini-stick and native button actions. You choose which TCA control drives which NEO control on a visual mapping page.

> **Status (October 2026):** the virtual NEO is recognised and read correctly by the Thrustmaster SDK, including ten-minute runs and live TCA input. In-game verification with ACE COMBAT 8 is in progress; please report your results in Issues. See [docs/ENGINEERING.md](docs/ENGINEERING.md) for the test evidence and known issues.

Not affiliated with Thrustmaster/Guillemot, Bandai Namco or Microsoft. See [Trademarks](#licence-and-trademarks).

## How it works

```text
TCA Sidestick + Quadrant ─► TCA-NEO bridge (tray app) ─► virtual NEO driver ─► ACE COMBAT 8
       (hidden from the game by HidHide)       your mapping          (user mode)
```

- The tray app reads the TCA devices, applies your mapping and streams NEO input reports to the driver.
- **HidHide** hides the physical TCA devices from the game while emulating, so the game sees only the NEO.
- The driver is a **user-mode** (UMDF) driver built from Microsoft's open-source sample. It is signed on your own PC at install time with a single-use certificate whose private key is deleted immediately; see [SECURITY.md](SECURITY.md).

## Prerequisites

- **Windows 11 x64**, build 22000 or later.
- **TCA Sidestick Airbus Edition** (Pilot or Copilot) and **TCA Quadrant Eng 1&2**, both connected by USB. No Thrustmaster software is needed.
- **[HidHide](https://github.com/nefarius/HidHide/releases)** by Nefarius, installed (reboot if its installer asks). Leave its application list free of Steam and keep "inverse application cloak" off.
- **Administrator rights.** Creating the virtual device and installing the driver require them.
- ACE COMBAT 8. If you play through Steam, **disable Steam Input** for the game so Steam doesn't remap the NEO.

## Installation

1. Download `tca-neo-bridge-<version>-x64.zip` from [Releases](../../releases). Optionally check it against the `.sha256` file published next to it.
2. Extract it to a permanent folder, for example `C:\Tools\tca-neo-bridge`. Keep `neo-daemon.exe` and the `driver` folder together.
3. Right-click `neo-daemon.exe` → **Run as administrator**. Its icon (the standard Windows application icon, tooltip "TCA-NEO bridge") appears in the notification area.
4. Right-click the icon → **Install driver…** and approve the Windows prompt. A window shows the steps, and a notification confirms "Driver installed". The log is in `%LOCALAPPDATA%\tca-neo-bridge\driver-install.log`.

## Usage

1. **Close Steam and the game.**
2. Right-click the tray icon → **Start emulation**. The tooltip shows *Emulating*.
3. Start Steam and ACE COMBAT 8. The game should list the controller as a T.Flight Hotas NEO and use its HOTAS controls.
4. When you're done, choose **Stop emulation**, or **Quit**. Both release every control, remove the virtual NEO and restore HidHide's previous settings.

### Tray menu

| Item | What it does |
| --- | --- |
| Start emulation / Stop emulation | Hide the TCA devices and create the virtual NEO, or undo both. |
| Configure… | Opens the configuration page in your browser (local only). Double-clicking the icon does the same. |
| Install driver… / Reinstall driver… | Installs the driver, or replaces it with a freshly signed copy. Available while not emulating. |
| Quit | Stops emulation and exits. |

### Mapping tab

The header shows the daemon status and, while emulating, whether the TCA is open, whether HidHide hides it and whether the virtual NEO exists. The left panel shows the TCA sidestick (switch **L / R** for a left- or right-hand setup) and quadrant, the middle one a T.Flight Hotas NEO with its numbered buttons and POV hat. Below them, a table lists NEO buttons 1–15 with their ACE COMBAT 8 actions and the TCA controls that feed each one.

- **Click a TCA control,** or press it while emulating. Its current NEO targets light up.
- **Click NEO buttons or POV arrows** to build its new mapping. Blue ring means kept, yellow ring means added, red dashed ring means removed. Then choose **Record mapping**. Changes apply immediately, even while emulating.
- **With nothing selected,** clicking a NEO control or a row of the NEO button table highlights every TCA control that feeds it.
- **Under *Other TCA inputs*,** set the **sidestick hat** to drive either the **Camera** (the NEO mini-stick, the default) or **Mapped controls**. In Mapped mode, its four directions can be mapped like buttons, and the camera gets no input.
- **Recorded corrections** keeps a log of every change you record.

### Axes and settings tab

| Setting | Meaning |
| --- | --- |
| Stick deadzone, camera strength | Applied to roll/pitch/twist and to the camera respectively. Each has a number field and a slider. |
| Quadrant throttle lever | Left or right lever. It shares the throttle with the sidestick slider; the last one moved takes over. |
| Invert | Per axis. |
| High-G from split levers | Thresholds are lever travel: 0 % at idle, 100 % fully forward. While one quadrant lever is at or above the *forward* threshold and the other at or below the *back* threshold, NEO 5 (High-G) is held. The throttle follows the forward lever. While emulating, live bars show both lever positions against the saved thresholds. |

### Default mapping

| TCA | NEO | ACE COMBAT 8 action |
| --- | --- | --- |
| Sidestick buttons 1–15 | NEO buttons 1–15 (same numbers) | 1 Gun · 2 Missile · 3 Weapon select · 4 Target select · 5 High-G · 6 Autopilot · 7 Flares · 8 View · 9 Radar · 10 Gear · 12 Pause/menu |
| Quadrant lever buttons 1, 2 | 5 | High-G |
| Quadrant ENG 1/2 switches (3, 4) and button 6 | 6 | Autopilot |
| Quadrant button 5 | 9 | Radar |
| Quadrant mode selector (7, 8) | 7, 8 | Flares, View |
| Sidestick stick, twist | X/Y, Rz | Roll/pitch, yaw |
| Left quadrant lever or sidestick slider | Z | Throttle |
| Sidestick hat | Rx/Ry mini-stick | Camera |
| Levers split (one ≥ 75 %, other ≤ 25 %) | 5 | High-G |

The ENG switches and the mode selector **latch**: while one rests in an active position, its NEO button stays held. Remap them on the Mapping tab if that doesn't suit you. **Reset everything to defaults** on the settings tab restores this table.

## Files and data

Everything is kept per user in `%LOCALAPPDATA%\tca-neo-bridge`:

- `config.json`: your profile.
- `label-map.json`: the recorded corrections.
- `neo-daemon.log`: status changes and errors (never device paths or serial numbers).
- `driver-install.log`.
- `hidhide-session.json`: only while emulating; used to restore HidHide after an interruption.

Run `neo-daemon.exe --data <folder>` to use another folder.

## Troubleshooting

| Symptom | What to do |
| --- | --- |
| "The virtual NEO could not start / did not appear" | Use **Install driver…** (or **Reinstall driver…**) and make sure the app runs as administrator. |
| "Creating the virtual NEO requires administrator rights" | Restart `neo-daemon.exe` with **Run as administrator**. |
| HidHide errors | Install HidHide, close its configuration client, remove Steam from its application list, turn *inverse application cloak* off, reconnect both TCA devices. |
| "Connect exactly one supported TCA sidestick and one Quadrant" | Both devices must be plugged in, and only one of each. |
| A control is "stuck" | A latching quadrant switch is in an active position; see *Default mapping*. |
| The game sees the TCA devices as well | Start emulation **before** starting Steam and the game. |
| Something else | Check `neo-daemon.log` and `driver-install.log`, then open an issue with their content. |

## Uninstall

1. Quit the tray app.
2. From an elevated PowerShell, run `driver\uninstall-driver.ps1`. This removes the driver package and its certificate.
3. Delete the program folder, and `%LOCALAPPDATA%\tca-neo-bridge` if you don't want to keep your profile.

## Building from source

See [docs/BUILDING.md](docs/BUILDING.md): Rust, Visual Studio Build Tools and the Windows Driver Kit. The repository enforces 100 % test coverage. How the project was designed, researched and verified is described in [docs/ENGINEERING.md](docs/ENGINEERING.md).

## Licence and trademarks

- **Code and docs:** MIT; see [LICENSE](LICENSE).
- **Driver:** `driver/neohid.c`, `neohid.h` and `neohid.inx` derive from Microsoft's `vhidmini2` sample and are under the Microsoft Public License ([driver/LICENSE-MS-PL.txt](driver/LICENSE-MS-PL.txt)).
- **Third-party components:** see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

Thrustmaster, T.Flight, TCA and T.A320 are trademarks of Guillemot Corporation. ACE COMBAT is a trademark of Bandai Namco Entertainment. They are used only to describe compatibility. This project is not affiliated with, endorsed by or supported by these companies, and it contains no software, firmware or artwork of theirs.
