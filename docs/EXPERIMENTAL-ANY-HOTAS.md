# Any-TM-HOTAS experimental bridge: guide for players

This bridge lets a Thrustmaster HOTAS **other than** the TCA Airbus sidestick + quadrant play ACE COMBAT 8 with the game's native HOTAS controls. It presents your controllers to Windows as a virtual **T.Flight Hotas NEO**, the one HOTAS the game supports natively.

> **Untested on real controllers.** The maintainer owns only the TCA Airbus pair. The code reads any joystick through its own USB HID description and passes tests with simulated devices, but nobody on the project has run it on the controllers below. You are the test: please [report your results](#report-your-results), good or bad.

Not affiliated with Thrustmaster/Guillemot, Bandai Namco or Microsoft.

## Which controllers?

Every Thrustmaster joystick, throttle or pedal set that Windows lists as a game controller is picked up automatically, for example:

| Controller | Notes |
| --- | --- |
| T.16000M FCS (+ TWCS throttle, + rudder pedals) | Separate USB devices; all of them are used together. |
| HOTAS Warthog (joystick + throttle) | The two throttle levers can drive split-lever High-G. |
| T.Flight Hotas X / 4 / One | One USB device for stick and throttle. |
| TCA Boeing yoke / throttle quadrant | Read like any other controller. |
| TCA Airbus sidestick + quadrant | Works here too, but the [stable TCA-NEO bridge](https://github.com/mkainchi/tca-neo-bridge/releases) is the tested choice. |

To use another brand's controller, tick it under **Axes and settings → Controllers**. It has had even less testing.

## Before you start

- **Windows 11 x64.**
- **[HidHide](https://github.com/nefarius/HidHide/releases)** by Nefarius, installed. Keep Steam out of its application list and leave "inverse application cloak" off.
- **Administrator rights**, for the virtual NEO and its driver.
- In Steam, **disable Steam Input** for ACE COMBAT 8.
- Using the stable TCA-NEO bridge as well? **Quit it** before you start this one; only one bridge can emulate at a time. Both use the same driver.

## Install

1. Download `any-tm-hotas-experimental-bridge-<version>-x64.zip` from the release marked **Pre-release** on [Releases](https://github.com/mkainchi/tca-neo-bridge/releases). The stable TCA build is a separate download.
2. Optional: check the download. In PowerShell, run `Get-FileHash` followed by the zip's path, and compare the result with the `.sha256` file next to the zip.
3. Extract the zip to a folder of its own, for example `C:\Tools\any-tm-hotas-bridge`. Keep `neo-daemon.exe` and the `driver` folder together.
4. Right-click `neo-daemon.exe` → **Run as administrator**. A bridge icon appears in the notification area; its tooltip reads "Any-TM-HOTAS bridge".
5. Right-click the icon → **Install driver…** and approve the Windows prompt. Skip this if you already installed the driver from the stable bridge.

Your settings live in `%LOCALAPPDATA%\any-tm-hotas-bridge`. If you used the stable bridge before, its mapping is **copied** there on first start; the stable bridge's own files are never changed.

## First run

1. Close Steam and the game.
2. Right-click the icon → **Start emulation**. The tooltip shows *Emulating*. Your controllers are now hidden from games, and the virtual NEO replaces them.
3. Right-click the icon → **Configure…**. The **Mapping** page shows the controllers it found, with a button panel for each.
4. Open **Flight axes** and **Throttle** and check each axis while you move it: the previews follow your HOTAS live.
   - **Axis sources:** the bridge guessed roll, pitch, yaw and throttle from the axis names your controllers report. To change one, press **Detect** next to it and move the stick, twist, lever or pedal you want.
   - **Invert:** tick it for any axis that moves the wrong way in the game. Throttles are the usual suspects.
   - **Controllers:** every connected Thrustmaster controller is used. Untick one on the **Controllers** page to leave it visible to the game.
   - Press **Save settings**, or **Try in game** to fly with the changes before saving them.
5. Back on the **Mapping** page, press a button on your HOTAS: it lights up. Click the NEO buttons it should press, then **Record mapping**. The **NEO buttons** tab lists the ACE COMBAT 8 action of each NEO button.
6. Start Steam and the game. It should list a T.Flight Hotas NEO and use its HOTAS controls.
7. When you're done, choose **Stop emulation**, or **Quit**. Both release every control, remove the virtual NEO and unhide your controllers.

Each combination of controllers keeps its own mapping: unplugging the throttle or adding pedals switches to that combination's mapping.

## Known risks and how to recover

| What you might see | What to do |
| --- | --- |
| A controller is missing from the Mapping tab | Check that Windows' *Set up USB game controllers* lists it, then run the probe (below) and report it. |
| An axis moves the wrong way, or the wrong axis moves | Under **Axis sources**, pick or **Detect** the right axis and tick **Invert**. |
| Button numbers don't match the numbers printed on the HOTAS | Expected: the numbering comes from the device. Map a button by pressing it, not by its printed number. |
| The throttle is reversed or stuck at full | Tick **Invert** for it. For split-lever High-G, also tick *Forward is the high raw value*. |
| A control seems stuck | Stop emulation, and check that no switch or lever rests in an active position. |
| The game still sees your real HOTAS | Start emulation **before** Steam and the game. |
| Anything else goes wrong | **Stop emulation**, then **Quit**: this unhides your controllers and removes the virtual NEO. If the bridge crashed while emulating, start it again and choose **Start emulation**, then **Stop emulation**: starting restores HidHide's previous settings from the bridge's recovery file. |

## Report your results

Every report helps, including "it just worked".

1. Quit the bridge.
2. Open the bridge folder in File Explorer, type `cmd` in the address bar and press Enter. In the window that opens, run:

   ```
   neo-daemon.exe --probe
   ```

3. For about two seconds, move every axis and press a few buttons. The bridge then writes `probe.txt` to `%LOCALAPPDATA%\any-tm-hotas-bridge` and opens it. It lists each controller's identity, buttons, axes, hat and one live sample. **It never contains device paths or serial numbers.**
4. Open a [HOTAS test report](https://github.com/mkainchi/tca-neo-bridge/issues/new?template=hotas-test-report.yml) and attach `probe.txt` and `neo-daemon.log` from that folder. Say which game you tested, what worked and what didn't, and which settings you changed.

Want to help further, with mapping suggestions for your model, code or diagrams? See [CONTRIBUTING.md](https://github.com/mkainchi/tca-neo-bridge/blob/main/CONTRIBUTING.md).
