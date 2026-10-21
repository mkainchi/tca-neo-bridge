# Engineering notes: how TCA-NEO bridge was built

This document explains why the project is shaped the way it is, how each claim was tested, and what is still unknown. It describes the method and the public interfaces involved. It deliberately leaves out vendor implementation details (binary addresses, decompiled code, the device's proprietary handshake) and raw test logs.

## 1. The problem

ACE COMBAT 8 has native HOTAS bindings for the Thrustmaster **T.Flight Hotas NEO**. It reads that device through Thrustmaster's SDK and gives it its own control profile. The **TCA Sidestick Airbus Edition** and **TCA Quadrant Eng 1&2** are excellent flight controls but are not among the devices the game supports natively. Earlier work in a sister project made them usable through keyboard emulation (Thrustmaster TARGET) and through a virtual Xbox pad (ViGEm). Both work, but both give up the native HOTAS profile: separate throttle, mini-stick camera and button labels that match the game.

**Hypothesis:** if Windows presents a HID device that the vendor SDK identifies as a NEO and that reports NEO-shaped input, the game will use its native NEO profile, and the TCA can drive that device.

## 2. Constraints chosen up front

- No game modification, no code injection or hooking, and no patching of vendor binaries.
- No kernel-mode driver, no test-signing mode, no third-party virtual-device or USB/IP drivers, which the maintainer did not want to trust.
- No proprietary source code is used, and no vendor software, firmware, SDK or artwork is redistributed; nothing in the build depends on it.
- No telemetry; private data such as device instance paths and serial numbers is never logged.
- Every physical or gameplay claim must be backed by a live test, not by unit tests or reasoning.

## 3. Evidence discipline

Every finding was labelled with how it was known:

| Label | Meaning |
| --- | --- |
| **Recovered** | Read from a published interface or from static inspection of how software behaves. |
| **Observed** | Seen in a preserved live run, with the configuration and file hashes archived. |
| **Hypothesis** | Plausible but unproven; never presented as fact. |

A recurring lesson was that a failed experiment often disproves a *test assumption*, not the idea. Examples: a harness that omitted the SDK's periodic update call, or read device information at the wrong offset. Such runs were recorded as "harness defect", not "rejected by the SDK".

## 4. Method, step by step

### 4.1 Characterise the source devices

The TCA sidestick and quadrant were read through the Windows HID capability APIs (`HidP_GetCaps`, value and button caps) with handles opened read-only. This established the exact report layouts:

- **Sidestick:** 64-byte report 01 with X/Y at 14 bits, twist and slider at 8 bits, a hat, and 17 buttons.
- **Quadrant:** 44-byte report with two 16-bit levers and 31 buttons, including detent and add-on bits.

A pure, tested converter (originally Python) mapped these to a NEO report. Its output vectors are still used as a byte-for-byte parity test for the Rust port (`daemon/tests/data/tca-neo-mapping-vectors.json`).

### 4.2 Learn what the SDK expects, from the outside

Static inspection of the installed SDK's behaviour, combined with the public Windows HID APIs it calls, showed how it recognises a controller:

- It enumerates the HID device interface class and reads the vendor and product IDs **from the interface path**.
- It then requires `HidD_GetAttributes`, the preparsed data and `HidP_GetCaps` to succeed, with sensible usages.
- It polls input as DirectInput-style state.

That gave a concrete target: a HID device with the NEO's identity (`VID_044F&PID_2800`, revision 2800), a report descriptor of the same shape (seven 16-bit axes, 15 buttons, a hat, and the feature and output reports the SDK talks to), and input arriving at a steady rate. The descriptor shipped in `driver/neo-descriptor.hex` was reconstructed this way for interoperability and validated live against the SDK. Revision 2800 is used because the official NEO driver binds only to revisions 2801 and 2802, so Windows' inbox HID stack keeps the virtual device.

### 4.3 Prove the HID personality with real hardware first

Before writing any Windows driver, a **Raspberry Pi Zero W** in USB gadget mode (Linux ConfigFS HID) played the NEO. This tested the personality on a real USB bus, with no driver work on the PC side. A small test harness on the developer's PC loaded the SDK installed with the vendor's software (never redistributed). It owned a visible window, pumped messages and polled the SDK exactly as a game would.

Findings:
- With **silent** handling of the vendor control reports (accept, don't answer) and input at 50–100 Hz, the SDK enumerated the device as "T.Flight Hotas NEO" with seven axes and fifteen buttons.
- A ten-minute read gave 29,348 valid samples and no errors after the first.
- Five reconnect cycles all passed.

This became the **oracle**: any later implementation had to match it.

### 4.4 Remove the Pi: a self-built user-mode driver

Options for presenting a HID device from the PC itself were evaluated and ranked:

| Option | Verdict |
| --- | --- |
| Microsoft VHF | Kernel-mode (KMDF) only, and needs Microsoft signing: rejected. |
| USB/IP client drivers | A third-party kernel driver: rejected by the trust constraint. |
| Third-party virtual HID frameworks | Unaudited third-party code or restrictive licences: rejected. |
| vJoy / ViGEm | Fixed identities (generic joystick, Xbox/DualShock): cannot be a NEO. |
| **Own UMDF2 HID minidriver from Microsoft's `vhidmini2` sample** | Microsoft-authored base, small enough to audit, runs in user mode under the inbox `mshidumdf.sys`: **chosen**. |

The driver (`driver/neohid.c`, about 300 lines) does four things:
- It answers the HID minidriver IOCTLs with the NEO identity and descriptor.
- It completes one pending input read every 10 ms, mirroring a full-speed interrupt endpoint.
- It accepts and discards feature and output writes (the "silent" behaviour proven on the Pi).
- It reads 68-byte frames from the daemon over a local named pipe.

When the daemon stops, the driver falls back to a neutral report.

Two findings from this phase:
- **The interface path comes from the parent device.** hidclass builds the child device ID from the parent software device, not from the HID attributes. Naming the software-device enumerator `VID_044F&PID_2800` makes the HID interface path read `HID\VID_044F&PID_2800\…`, which is what the SDK parses. The hardware IDs still differ from a USB device, and the SDK accepts that.
- **The virtual device lives exactly as long as the daemon.** The daemon creates it on demand with `SwDeviceCreate`, and Windows removes it when the daemon stops or crashes.

### 4.5 Gate A: SDK parity with the Pi oracle

Gate A repeated the Pi acceptance tests against the virtual device:

| Check | Pi gadget | Virtual device (UMDF) |
| --- | --- | --- |
| Identified as "T.Flight Hotas NEO", 7 axes / 15 buttons | yes | yes |
| Ten-minute read | 29,348 samples, 25,588 changes, no errors after start | 29,854 samples, 28,994 changes, no errors after start |
| Independent axes, all buttons, button combinations, 8 POV directions + neutral | yes | yes |
| Create/remove cycles with a fresh SDK process | 5 / 5 | 25 clean of 29 (see below) |
| Physical TCA input reaching the SDK, with HidHide isolation | — | yes: full-range axes, buttons and POV |

**Known issue:** in 3 of 29 harness launches, the SDK crashed during its first update with the same signature (an execute access violation at a tiny address from a corrupted stack) previously seen in the vendor's own configuration application on real USB. It looks like an I/O-lifetime race inside the SDK, which this project cannot fix. A relaunch works.

**Gate B** (native profile and gameplay in ACE COMBAT 8) is separate. SDK tests do not prove gameplay.

### 4.6 Mapping and interface, driven by use

The mapping started as the research converter's bindings and was then reshaped through hands-on use on the real hardware:

- **Identifying controls by pressing them didn't work.** The TCA has no printed numbers, so an early "identify by pressing" flow was replaced by a **Mapping tab**: the TCA diagrams sit beside an original NEO schematic. Selecting a TCA control lights its NEO targets, and clicking NEO buttons builds the correction. The layout was designed for a 2560×1600 screen and checked with real 2560×1440 renders, with a stacked layout for narrow windows.
- **Hat mode.** The TCA's single hat could not be both camera and POV, which led to an explicit setting: Camera, or Mapped controls.
- **Split-lever High-G** (one quadrant lever forward, the other back holds NEO 5) came from a user request. A live check against the user's profile then caught a real bug. The first version made the throttle follow "the lever giving the higher NEO value", which with an inverted throttle axis meant the *back* lever. A regression test now reproduces that exact profile. Live testing of 0.2.0 then showed the TCA reports a lever fully forward as raw 0 and at idle as raw 65535, the same convention as NEO Z. Lever positions (page readout, thresholds) are now expressed as travel (forward = 100 %), and the split throttle follows the lever with the greater travel, mapped back to the raw axis value, whatever the inversion setting.
- The defaults shipped today are the mapping and settings tuned on the real hardware.

## 5. Architecture

```text
TCA Sidestick + Quadrant ──raw HID──► neo-daemon.exe (Rust, tray)
                                        │  read reports → mapping (profile) → NEO report 01
                                        │  HidHide: physical TCAs hidden from games, daemon allowed
                                        ▼
                            \\.\pipe\neo-hid-v1 (68-byte frames, local only)
                                        ▼
            neohid.dll (UMDF2, WUDFHost) ── inbox mshidumdf / hidclass ──► HID device 044F:2800
                                                                              ▼
                                                              vendor SDK ──► ACE COMBAT 8
```

The daemon's modules:

| Module | Role |
| --- | --- |
| `tca` | HID input |
| `neo` | Mapping and report encoding |
| `device` | Software device and pipe |
| `hidhide` | Isolation journal and restore |
| `emulation` | Session lifecycle |
| `http` | Loopback configuration server |
| `tray` | Menu |
| `driver` | Install/reinstall |
| `config` and `labels` | Profile and correction log |

## 6. Testing

- **Win32 behind tables of function pointers.** Each module receives its Win32 calls as a table of plain function pointers. Tests substitute `extern "system"` fakes that simulate devices, pipes, UAC, pnputil and the tray, including blocking I/O that is cancelled the way Windows cancels it. This is how window procedures, worker threads and teardown order are tested deterministically, without real hardware.
- **Coverage gates:** 100 % of Rust lines, functions and regions (stable), 100 % of branches (nightly), and 100 % of the page script. No exclusion attributes. Code that could only fail in impossible ways was restructured instead of excluded. Flaky timing-dependent coverage was turned into deterministic tests.
- **Parity tests** against the original converter's vectors, and a C unit test for the driver's frame parser.
- **Live gates** (above) for everything the fakes cannot prove: SDK acceptance, physical input, isolation and, next, gameplay.

## 7. How it was developed

The project was built by its maintainer with an AI coding assistant (Claude Code). The maintainer set the goals and constraints, made every product, trust and safety decision, and ran all hardware and in-game tests. The assistant researched options, wrote code, tests and documentation, and analysed the evidence from each run. Its claims were accepted only when tests or live runs confirmed them.

## 8. Limitations and next steps

- In-game verification with ACE COMBAT 8 (native profile, flight, camera, weapons, High-G, restart and disconnect recovery).
- The SDK startup crash in about 1 of 10 launches is outside this project's control.
- Latching quadrant switches (ENG 1/2, the mode selector) used to hold their NEO button while latched. Profile version 2 pulses them by default (per-source *Behaviour*); profiles migrated from version 1 keep holding until changed.
- Reconnecting after a TCA disconnect (the virtual NEO stays; a replugged device is journaled and hidden again), the warn-only launch checks (`tasklist`, `reg`, Steam's `localconfig.vdf`) and OpenTrack head tracking (UDP on 127.0.0.1) are covered by tests with fake devices, a fake HidHide CLI, fake tools and real local UDP, but still need verification on real hardware, with Steam and OpenTrack, in ACE COMBAT 8.
- Named profiles, the next-profile combo, the tray Profile submenu, the Live tab and the printed cheat sheet are covered by unit, HTTP and page tests (profile names are restricted so they cannot leave the profiles folder), but still need in-game verification in ACE COMBAT 8.
- Long press, double tap, Repeat, the shift layer, the camera ramp and spare-lever camera, and lever-difference yaw (later version 2 settings, absent fields default) are timed with the report clock and covered by timeline tests, but still need in-game verification in ACE COMBAT 8.
- Axis curves, throttle detents, safe start, precision aiming, the pause combo and *Try in game* (profile version 2) are covered by unit tests and the shared shape vectors (`daemon/tests/data/axis-shape-vectors.json`, checked by both the daemon and page tests), but still need in-game verification in ACE COMBAT 8.
- Creating the virtual device needs administrator rights (`SwDeviceCreate`).
- OpenTrack's yaw and pitch sign conventions are assumed (positive pitch looks up); the invert options cover setups that differ.
- Windows 11 (build 22000+) only, because the driver package depends on `MsHidUmdf.inf`.
