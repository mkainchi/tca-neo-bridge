# Any-TM-HOTAS experimental bridge: guide for players

This is an **experimental** build of the bridge for players who own a Thrustmaster HOTAS **other than** the TCA Airbus sidestick + quadrant. It presents your controllers to Windows as a virtual **T.Flight Hotas NEO**, the HOTAS that ACE COMBAT 8 supports natively, so the game uses its NEO profile.

> **Nobody on the project has tested this on these controllers.** The maintainer only owns the TCA Airbus pair. The code reads any joystick through its own USB HID description and was tested against simulated devices, but the real controllers are untested. You are the test. Please report what happens, good or bad; see [Report your results](#report-your-results).

Not affiliated with Thrustmaster/Guillemot, Bandai Namco or Microsoft.

## Which controllers?

Any Thrustmaster joystick, throttle or HOTAS that Windows sees as a game controller should be picked up automatically, for example:

| Controller | Notes |
| --- | --- |
| T.16000M FCS (+ TWCS throttle, + rudder pedals) | Stick, throttle and pedals are separate USB devices; all are used together. |
| HOTAS Warthog (joystick + throttle) | The two throttle levers can drive split-lever High-G. |
| T.Flight HOTAS X / 4 / One | One USB device for stick and throttle. |
| TCA Boeing yoke / throttle quadrant | Works through the generic path. |
| TCA Airbus sidestick + quadrant | Fully supported by the [stable bridge](https://github.com/mkainchi/tca-neo-bridge/releases); also works here. |

Another brand's controller can be added by ticking it under **Axes and settings → Controllers**. That has had even less testing.

## Before you start

- **Windows 11 x64.**
- **[HidHide](https://github.com/nefarius/HidHide/releases)** by Nefarius, installed. Leave Steam out of its application list and keep "inverse application cloak" off.
- **Administrator rights** (the virtual NEO and its driver need them).
- In Steam, **disable Steam Input** for ACE COMBAT 8.
- If you also use the stable TCA-NEO bridge: **quit it first**. Only one bridge can emulate at a time. Both share the same virtual NEO driver, so if you already installed the driver from the stable bridge you can skip step 4 below.

## Install

1. Download `any-tm-hotas-experimental-bridge-<version>-x64.zip` from the [pre-release](https://github.com/mkainchi/tca-neo-bridge/releases) (it is marked **Pre-release**; the stable TCA build is a separate download). Optionally check it against the `.sha256` file next to it.
2. Extract it to a folder of its own, for example `C:\Tools\any-tm-hotas-bridge`. Keep `neo-daemon.exe` and the `driver` folder together.
3. Right-click `neo-daemon.exe` → **Run as administrator**. A bridge icon appears in the notification area; its tooltip reads "Any-TM-HOTAS bridge".
4. Right-click the icon → **Install driver…** and approve the Windows prompt.

Your settings live in `%LOCALAPPDATA%\any-tm-hotas-bridge`. If you used the stable bridge before, its mapping is **copied** on first start; the stable bridge's own folder is never changed.

## First run

1. Close Steam and the game.
2. Right-click the icon → **Configure…**. The page shows the controllers it found on the **Mapping** tab, with a button panel for each.
3. Open **Axes and settings**:
   - **Controllers:** by default every connected Thrustmaster controller is used. Untick one to leave it visible to the game.
   - **Axis sources:** the bridge guesses roll, pitch, yaw and throttle from the axis names your controller reports. Start emulation (next step), press **Detect** next to an axis, and move the stick, twist, lever or pedal you want. Then press **Save settings**.
   - **Invert** any axis that moves the wrong way in the game. Throttles are the usual suspects.
4. Right-click the icon → **Start emulation**. The tooltip shows *Emulating*. Your controllers are now hidden from games; the virtual NEO replaces them.
5. Back on the **Mapping** tab, press a button on your HOTAS: it lights up. Click NEO buttons to choose what it does, then **Record mapping**. Changes apply at once, even while emulating. The table under the NEO picture lists the ACE COMBAT 8 action of each NEO button.
6. Start Steam and the game. The game should list a T.Flight Hotas NEO and use its HOTAS controls.
7. When you're done: **Stop emulation**, or **Quit**. Both release every control, remove the virtual NEO and unhide your controllers.

Each combination of controllers keeps its own mapping, so unplugging the throttle or adding pedals switches to that set's mapping.

## Known risks and how to recover

| What you might see | What to do |
| --- | --- |
| A controller is not listed | Run the probe (below) and report it. Check that Windows' *Set up USB game controllers* shows it. |
| An axis moves the wrong way, or the wrong axis moves | Fix it under **Axis sources**: pick or Detect the right axis, tick Invert. |
| Button numbers don't match what is printed on the HOTAS | Expected: numbering comes from the device. Map by pressing the button, not by its printed number. |
| The throttle is reversed or stuck at full | Invert it. If you use split-lever High-G, also check "Forward is the high raw value". |
| A control seems stuck | Stop emulation; check that no switch or lever rests in an active position. |
| The game still sees your real HOTAS | Start emulation **before** Steam and the game. |
| Anything goes wrong | **Stop emulation**, then **Quit**. This restores HidHide and removes the virtual NEO. If the bridge crashed, start it again and quit: it restores HidHide's previous settings from its recovery file. |

## Report your results

Every report, even "it just worked", helps.

1. Quit the bridge (or stop emulation).
2. Open a terminal in the bridge folder and run:

   ```
   neo-daemon.exe --probe
   ```

   Move every axis and press a few buttons for the first two seconds. It writes `probe.txt` to `%LOCALAPPDATA%\any-tm-hotas-bridge` and opens it. It lists each controller's identity, buttons, axes, hat and one live sample. **It never contains device paths or serial numbers.**
3. Open a [HOTAS test report](https://github.com/mkainchi/tca-neo-bridge/issues/new?template=hotas-test-report.yml) and attach `probe.txt` and `neo-daemon.log` from the same folder. Say which game you tested, what worked and what didn't.

Want to help further: mapping suggestions for your model, code, or diagrams? See [CONTRIBUTING.md](https://github.com/mkainchi/tca-neo-bridge/blob/main/CONTRIBUTING.md).
