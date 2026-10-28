# Security model

TCA-NEO bridge installs a driver and runs with administrator rights, so this file explains what it trusts and why.

## The driver runs in user mode

`neohid` is a UMDF2 (User-Mode Driver Framework) HID minidriver. It runs inside Windows' `WUDFHost.exe` process under the LocalService account, below Microsoft's inbox `mshidumdf.sys` pass-through. It is not a kernel driver: a fault in it ends one host process instead of crashing Windows. Its only input is a local named pipe that the daemon writes to (see below).

## Driver signing: a single-use certificate created on your PC

Microsoft only signs drivers for registered companies, so the release ships the driver **unsigned** together with `SHA256SUMS`. When you choose **Install driver…** (and approve the UAC prompt), `driver\install-driver.ps1`:

1. checks the package files against `SHA256SUMS`;
2. creates a new code-signing certificate on your PC, signs a copy of the driver with it, exports only the public certificate and **deletes the private key** immediately;
3. trusts that public certificate in LocalMachine `Root` and `TrustedPublisher`, then stages the driver with `pnputil`.

Because the private key no longer exists, the certificate you trusted can never sign anything else, on your PC or anywhere. Each installation creates a fresh certificate and removes the previous one. `driver\uninstall-driver.ps1` removes both the package and the certificate. Test-signing mode is never enabled.

## Data flow and network

- **Daemon → driver:** the pipe `\\.\pipe\neo-hid-v1` accepts local clients only. Its ACL grants the daemon's user full access and LocalService/SYSTEM (the UMDF host accounts) read access. Frames are fixed-size and validated; anything malformed is discarded and the driver falls back to neutral input.
- **Configuration API:** served on `127.0.0.1` only, on a random port, for `neo-config.exe`. Every call needs a random per-launch token, which the daemon hands to the app on its standard input (never on a command line, in the environment or in a URL), and an exact `Host` header, which blocks DNS-rebinding attacks. No page is served; responses send `Content-Security-Policy: default-src 'none'`.
- **No telemetry,** no update checks and no outbound connections of any kind.
- **Logs** (`%LOCALAPPDATA%\tca-neo-bridge`) hold status changes and errors only, never device instance paths or serial numbers.

## Controller isolation

During emulation, HidHide hides the physical TCA devices from every program except the daemon, so the game sees only the virtual NEO. The daemon records what it changed and restores HidHide's previous state when emulation stops, including after a crash, at the next start.

## Reporting a vulnerability

Please open a GitHub security advisory (Security → Report a vulnerability) rather than a public issue.
