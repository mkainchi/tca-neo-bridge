# Third-party notices

TCA-NEO bridge is MIT-licensed (see [LICENSE](LICENSE)), except the driver files derived from Microsoft's sample, which are under the Microsoft Public License. This file lists everything else that is shipped or used to build it.

## Shipped in the release

### Driver derived from Microsoft Windows-driver-samples (MS-PL)

`driver/neohid.c`, `driver/neohid.h` and `driver/neohid.inx` are derived from the [hid/vhidmini2](https://github.com/microsoft/Windows-driver-samples/tree/main/hid/vhidmini2) sample, commit `2dc3fd3a0cc84a2933f2194e7ec0871584979071`. Copyright (c) Microsoft Corporation. They are distributed under the Microsoft Public License; the full text is in [driver/LICENSE-MS-PL.txt](driver/LICENSE-MS-PL.txt) and ships next to the driver in every release.

### Rust crates compiled into `neo-daemon.exe`

| Crate | Version | License used | Source |
| --- | --- | --- | --- |
| serde, serde_core | 1.0.229 | MIT (dual MIT/Apache-2.0) | https://github.com/serde-rs/serde |
| serde_json | 1.0.154 | MIT (dual MIT/Apache-2.0) | https://github.com/serde-rs/json |
| itoa | 1.0.18 | MIT (dual MIT/Apache-2.0) | https://github.com/dtolnay/itoa |
| memchr | 2.8.3 | MIT (dual MIT/Unlicense); Copyright (c) 2015 Andrew Gallant | https://github.com/BurntSushi/memchr |
| zmij | 1.0.23 | MIT | https://crates.io/crates/zmij |

These crates are used under the MIT license, whose terms are:

> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

### Device diagrams

The TCA sidestick (right- and left-hand setups) and quadrant diagrams and the NEO diagram in `daemon/web/device/` are vector drawings made for this project (MIT). They contain no embedded images, manufacturer artwork, photographs or logos.

## Build and test only (not shipped)

- Build-time Rust crates: serde_derive, proc-macro2, quote, syn (MIT/Apache-2.0) and unicode-ident ((MIT OR Apache-2.0) AND Unicode-3.0). Windows resource compilation uses embed-resource (MIT) and its build dependencies, recorded in `daemon/Cargo.lock`.
- Optional icon regeneration: sharp (Apache-2.0) and its image libraries; not needed for normal builds. The bridge SVG and ICO artwork in `daemon/assets/` is original project artwork (MIT).
- Page tests: vitest, @vitest/coverage-v8 and jsdom (MIT), installed from npm by `npm ci`.
- Microsoft Windows Driver Kit and Visual Studio Build Tools, which each developer installs under Microsoft's terms.

## Not included

No Thrustmaster software, SDK, drivers, firmware or manuals, and no game files, are included or needed to build this project. HidHide (Nefarius Software Solutions) is a prerequisite that users install themselves.

## Trademarks

Thrustmaster, T.Flight, TCA and T.A320 are trademarks of Guillemot Corporation. ACE COMBAT is a trademark of Bandai Namco Entertainment. Windows is a trademark of Microsoft. They are used only to describe compatibility. This project is not affiliated with, endorsed by or supported by any of these companies.
