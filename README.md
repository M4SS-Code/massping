# massping

[![crates.io](https://img.shields.io/crates/v/massping.svg)](https://crates.io/crates/massping)
[![Documentation](https://docs.rs/massping/badge.svg)](https://docs.rs/massping)
[![dependency status](https://deps.rs/crate/massping/0.3.2/status.svg)](https://deps.rs/crate/massping/0.3.2)
[![Rustc Version 1.64.0+](https://img.shields.io/badge/rustc-1.64.0+-lightgray.svg)](https://forge.rust-lang.org/)
[![CI](https://github.com/M4SS-Code/massping/actions/workflows/ci.yml/badge.svg)](https://github.com/M4SS-Code/massping/actions/workflows/ci.yml)

Asynchronous ICMP ping library using Linux RAW sockets and the
tokio runtime.

As this crate needs to use RAW sockets, it must either be run as root
or permission must explicitly be set via
`sudo setcap cap_net_raw=+eip path/to/binary`.

## Features

* `strong`: implements strong checking for the RTT. Disabling this
            feature makes the pinger a little bit faster, but opens
            you up to some servers, like those running [pong][ping],
            to faking the ping times.
* `stream`: implements `Stream` for `MeasureManyStream`.

## MSRV version policy

This project has a CI job to prevent accidental bumping of the MSRV.
We might bump MSRV version at any time. If you require a lower MSRV
please open an issue.

[ping]: https://github.com/m-ou-se/pong
