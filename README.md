<p align="center">
  <img src="docs/images/any-tm-hotas-experimental-bridge-logo.png" alt="Any-TM-HOTAS experimental bridge logo" width="180">
</p>

# Any-TM-HOTAS experimental bridge

Fly **ACE COMBAT 8** with **any Thrustmaster HOTAS**, using the game's **native HOTAS controls**.

ACE COMBAT 8 natively supports one HOTAS, the Thrustmaster T.Flight Hotas NEO. This bridge is a small Windows tray app plus a user-mode driver. Together they present your controllers to Windows as a virtual T.Flight Hotas NEO, so the game uses its NEO profile: separate throttle, camera mini-stick and native button actions. You choose which control drives which NEO control on a mapping page.

> **Experimental and untested.** This is the experimental release line of [TCA-NEO bridge](https://github.com/mkainchi/tca-neo-bridge). The maintainer owns only the TCA Airbus sidestick + quadrant; nobody on the project has run this build on a T.16000M, TWCS, Warthog, T.Flight Hotas or TCA Boeing. Players are the testers.
>
> - **Players:** start with the [guide for players](docs/EXPERIMENTAL-ANY-HOTAS.md) (`EXPERIMENTAL.md` in the zip), then send a [HOTAS test report](https://github.com/mkainchi/tca-neo-bridge/issues/new?template=hotas-test-report.yml).
> - **TCA Airbus owners:** the stable [TCA-NEO bridge](https://github.com/mkainchi/tca-neo-bridge/releases) is the tested choice.
> - **Contributors:** see [CONTRIBUTING.md](CONTRIBUTING.md). HOTAS support goes to this branch, `release/any-tm-hotas-experimental-bridge`; stable fixes go to `main`.

Not affiliated with Thrustmaster/Guillemot, Bandai Namco or Microsoft. See [Trademarks](#licence-and-trademarks).

## How it works

```text
Your HOTAS ─► Any-TM-HOTAS bridge (tray app) ─► virtual NEO driver ─► ACE COMBAT 8
(hidden from the game by HidHide)   your mapping          (user mode)
```

- The tray app reads your controllers, applies your mapping and streams NEO input reports to the driver.
- Controllers are read through their own USB HID description: buttons, axes and hat. The TCA Airbus pair keeps the decoders tested on real hardware.
- **HidHide** hides your physical controllers from the game while emulating, so the game sees only the NEO.
- The driver is a **user-mode** (UMDF) driver built from Microsoft's open-source sample, the same one the stable bridge uses. It is signed on your own PC at install time with a single-use certificate whose private key is deleted immediately; see [SECURITY.md](SECURITY.md).

## Prerequisites

- **Windows 11 x64**, build 22000 or later.
- **A Thrustmaster HOTAS** connected by USB: stick, throttle, pedals or all of them. No Thrustmaster software is needed.
- **[HidHide](https://github.com/nefarius/HidHide/releases)** by Nefarius, installed (reboot if its installer asks). Leave its application list free of Steam and keep "inverse application cloak" off.
- **Administrator rights.** Creating the virtual device and installing the driver require them.
- ACE COMBAT 8. If you play through Steam, **disable Steam Input** for the game so Steam doesn't remap the NEO.

## Installation

1. Download `any-tm-hotas-experimental-bridge-<version>-x64.zip` from the **Pre-release** on [Releases](https://github.com/mkainchi/tca-neo-bridge/releases). To check the download, run `Get-FileHash any-tm-hotas-experimental-bridge-<version>-x64.zip` in PowerShell and compare the result with the `.sha256` file next to it.
2. Extract it to a folder of its own, for example `C:\Tools\any-tm-hotas-bridge`. Keep `neo-daemon.exe` and the `driver` folder together.
3. If the stable TCA-NEO bridge is running, quit it. Only one bridge can emulate at a time.
4. Right-click `neo-daemon.exe` → **Run as administrator**. Its bridge icon appears in the notification area; the tooltip shows "Any-TM-HOTAS bridge" and the current status.
5. Right-click the icon → **Install driver…** and approve the Windows prompt. Skip this if you installed the driver with the stable bridge. A notification confirms "Driver installed".

On first start, a profile saved by the stable bridge is **copied** into this bridge's folder. The stable bridge's own files are never changed.

## Usage

1. **Close Steam and the game.**
2. Right-click the tray icon → **Start emulation**. The tooltip shows *Emulating*.
3. Right-click → **Configure…** to check your axes and map your buttons (see below). Mappings apply immediately; settings when you save them.
4. Start Steam and ACE COMBAT 8. The game should list a T.Flight Hotas NEO and use its HOTAS controls.
5. When you're done, choose **Stop emulation**, or **Quit**. Both release every control, remove the virtual NEO and restore HidHide's previous settings.

### Tray menu

The icon's badge shows the emulation status: a gray hollow circle when idle, a green check when emulating, amber dots while starting or stopping, and a red exclamation mark on error. Hover over the icon for the full status or error message. While the driver installs, its menu item reads **Installing driver…**, and a notification reports the result.

| Item | What it does |
| --- | --- |
| Start emulation / Stop emulation | Hide your controllers and create the virtual NEO, or undo both. |
| Configure… | Opens the configuration page in your browser (local only). Double-clicking the icon does the same. |
| Pause input / Resume input | While emulating, releases every control and sends a neutral NEO until resumed. A button combo can do the same (see *Buttons & helpers*). |
| Profile ▸ | Appears once a profile is saved under a name. Choosing one makes it the active profile, even mid-flight; a balloon confirms it. |
| Install driver… / Reinstall driver… | Installs the driver, or replaces it with a freshly signed copy. Available while not emulating. |
| Quit | Stops emulation and exits. |

### The configuration page

The page fits one desktop window (1280 × 800 and up) without scrolling. The header shows the bridge status and, while emulating, how many controllers are open, whether HidHide hides them and whether the virtual NEO exists, next to the **Profile** menu and **Print cheat sheet**. A rail on the left switches between **Mapping**, **Flight axes**, **Throttle**, **Camera & head**, **Buttons & helpers**, **Controllers** and **Live**. On the settings pages, a bar at the bottom holds **Try in game**, **Discard**, **Reset to defaults** and **Save settings**.

**Every preview is live.** While emulating, each page follows what you do on your HOTAS straight away, through your unsaved settings: the curve dot and meters follow the stick, the throttle needle your throttle, the camera graph your hat hold, the helper states and press timeline your buttons. Test controls appear only when emulation is stopped.

### Mapping

The left panel shows your controllers: the TCA Airbus pair as diagrams, any other controller as a numbered button panel. The middle panel is the T.Flight Hotas NEO with its numbered buttons and POV hat. The right panel has three tabs: **Selected control**, **NEO buttons** (NEO buttons 1–15 with their ACE COMBAT 8 actions, the controls that feed each one, and the NEO axes) and **Log**.

- **Click a control,** or press it while emulating. Its current NEO targets light up.
- **Click NEO buttons or POV arrows** to build its new mapping. Blue ring means kept, yellow ring means added, red dashed ring means removed. Then choose **Record mapping**.
- **Behaviour** sets how the control drives its NEO targets: **Hold**, **Pulse** (a short press when it switches on), **Pulse on change** (a short press each time it moves, on and off), **Toggle** or **Repeat**. A small timeline shows what the game receives.
- **Press / Long press / Double tap** picks which action the NEO clicks change. Each tile shows its current mapping.
- **Normal / Shift** switches the layer being mapped once a shift control is set (see *Buttons & helpers*).
- **With nothing selected,** clicking a NEO control or a row of the NEO buttons table highlights every control that feeds it.
- **Camera hat** chooses which hat drives the camera, and whether it drives the **Camera** (the NEO mini-stick, the default) or is **Mapped** like buttons. Other hats are always mappable.

### Settings pages

| Page | Settings |
| --- | --- |
| Flight axes | Per axis (**Roll**, **Pitch**, **Yaw**): **deadzone**, **full deflection at**, **anti-deadzone**, **curve** and **smoothing**, with a live curve. **Axis sources** pick which controller axis drives each NEO axis, with **Invert**; **Detect** binds the next axis you move while emulating. **Yaw source**: the bound axis, **Off**, or the difference between the two split levers. |
| Throttle | **Linear** or ACE COMBAT **Detents** (brake below idle, cruise around the CL detent, boost from the last detent; **Capture** each detent while emulating). **Throttle axes**: every axis in the list shares the throttle; the last one moved takes over. **High-G from split levers**: two levers and their thresholds; tick *Forward is the high raw value* for throttles that report forward as a high number. |
| Camera & head | Camera **strength**, **ramp** and **full strength after a long press**, with the hat hold drawn live. **Hat + a lever** lets another axis pan the camera. **Head tracking** through OpenTrack (UDP over network, 127.0.0.1, port 4242 by default). |
| Buttons & helpers | **Precision aiming** (hold a control to scale the flight axes down), **Shift layer**, **Pause input** and **Next profile** combos, **Safe start** (the throttle rests at centre until it reaches it), and **Timing** for long press, double tap, repeat and pulses, with a timeline of your own presses. |
| Controllers | **Automatic** uses every connected Thrustmaster controller; or tick the ones you want, including another brand's. Each set of controllers keeps its own mapping; the settings on the other pages are shared. |

**Try in game** runs your unsaved settings in the running emulation without saving them. Profiles saved by earlier versions of either bridge, stable or experimental, are upgraded when loaded.

### Live, profiles and the cheat sheet

The **Live** page shows every NEO axis as the raw input above what the NEO receives, the bridge state, the controls being pressed, the 15 NEO buttons as they are sent, and the POV. The **Profile** menu saves the settings under a name, switches between saved profiles, and imports or exports them as JSON. **Print cheat sheet** prints a light A4 page of your controllers' mapping.

If a controller disconnects while emulating, every control is released, the virtual NEO stays and the bridge waits for the same controllers to return, then hides them again. When emulation starts, the bridge warns if Steam was already running or Steam Input is on for ACE COMBAT 8.

### Default mapping

**Any controller set except the TCA Airbus pair** starts from a guess, which you should check in the game:

| Your HOTAS | NEO |
| --- | --- |
| X / Y of the first device that has both | Roll / pitch |
| Its twist (Rz), or any device's Rz or rudder | Yaw |
| Each device's throttle, Z or slider axis | Throttle (shared) |
| Buttons 1–15 of that stick | NEO buttons 1–15 |
| The stick's hat (or the first hat) | Camera |

**TCA Airbus sidestick + quadrant** keeps the stable bridge's tested default:

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

The TCA's ENG switches **pulse on change** and its mode selector **pulses**, so a latched switch sends one short press instead of holding its button. **Reset to defaults** on a settings page restores the default of the current controller set.

## Files and data

Everything is kept per user in `%LOCALAPPDATA%\any-tm-hotas-bridge`:

- `config.json`: your profile, one mapping per controller set.
- `label-map.json`: the recorded corrections.
- `neo-daemon.log`: status changes and errors (never device paths or serial numbers).
- `driver-install.log`.
- `probe.txt`: written by `neo-daemon.exe --probe` for test reports.
- `hidhide-session.json`: only while emulating; used to restore HidHide after an interruption.

Run `neo-daemon.exe --data <folder>` to use another folder.

## Troubleshooting

| Symptom | What to do |
| --- | --- |
| "Connect a supported Thrustmaster HOTAS (or tick your controllers…)" | Plug your controllers in. For another brand, tick it under **Axes and settings → Controllers**. |
| "HidHide must detect the selected controllers" | Reconnect them, close HidHide's configuration client, and start emulation again. |
| Other HidHide errors | Install HidHide, close its configuration client, remove Steam from its application list, turn *inverse application cloak* off. |
| "The virtual NEO could not start / did not appear" | Use **Install driver…** (or **Reinstall driver…**) and make sure the app runs as administrator. |
| "Creating the virtual NEO requires administrator rights" | Restart `neo-daemon.exe` with **Run as administrator**. |
| "The report pipe is already in use" | Another bridge is running, maybe the stable one. Quit it. |
| An axis moves the wrong way, or the wrong axis moves | Under **Axis sources**, pick or **Detect** the right axis and tick **Invert**. |
| A control is "stuck" | A switch or lever rests in an active position; remap it or move it. |
| The game sees your real controllers as well | Start emulation **before** starting Steam and the game. |
| Something else | Run `neo-daemon.exe --probe`, then [open a HOTAS test report](https://github.com/mkainchi/tca-neo-bridge/issues/new?template=hotas-test-report.yml) with `probe.txt`, `neo-daemon.log` and `driver-install.log`. |

## Uninstall

1. Quit the tray app.
2. Right-click **Start** → **Terminal (Admin)**, then run the uninstall script from the program folder. It removes the driver package and its certificate. Skip this if you still use the stable bridge; it uses the same driver.

   ```powershell
   powershell -ExecutionPolicy Bypass -File C:\Tools\any-tm-hotas-bridge\driver\uninstall-driver.ps1
   ```

   Replace `C:\Tools\any-tm-hotas-bridge` with your program folder.
3. Delete the program folder, and `%LOCALAPPDATA%\any-tm-hotas-bridge` if you don't want to keep your profile.

## Building from source

See [docs/BUILDING.md](docs/BUILDING.md): Rust, Visual Studio Build Tools and the Windows Driver Kit. The repository enforces 100 % test coverage. How the project was designed, researched and verified is described in [docs/ENGINEERING.md](docs/ENGINEERING.md).

## Licence and trademarks

- **Code and docs:** MIT; see [LICENSE](LICENSE).
- **Driver:** `driver/neohid.c`, `neohid.h` and `neohid.inx` derive from Microsoft's `vhidmini2` sample and are under the Microsoft Public License ([driver/LICENSE-MS-PL.txt](driver/LICENSE-MS-PL.txt)).
- **Third-party components:** see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
- **No proprietary code:** no proprietary source code was used in this project. Every source file was written for it or comes from the open-source projects listed above. It contains no Thrustmaster, Guillemot or Bandai Namco software, firmware, SDK or artwork. Vendor software was only observed to learn the interface the game expects; see [docs/ENGINEERING.md](docs/ENGINEERING.md).

Thrustmaster, T.Flight, TCA and T.A320 are trademarks of Guillemot Corporation. ACE COMBAT is a trademark of Bandai Namco Entertainment. They are used only to describe compatibility. This project is not affiliated with, endorsed by or supported by these companies.
