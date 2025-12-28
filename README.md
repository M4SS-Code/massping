# massping

[![crates.io](https://img.shields.io/crates/v/massping.svg)](https://crates.io/crates/massping)
[![Documentation](https://docs.rs/massping/badge.svg)](https://docs.rs/massping)
[![dependency status](https://deps.rs/crate/massping/0.3.6/status.svg)](https://deps.rs/crate/massping/0.3.6)
[![Rustc Version 1.85.0+](https://img.shields.io/badge/rustc-1.85.0+-lightgray.svg)](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/)
[![CI](https://github.com/M4SS-Code/massping/actions/workflows/ci.yml/badge.svg)](https://github.com/M4SS-Code/massping/actions/workflows/ci.yml)

Asynchronous ICMP ping library using Linux RAW sockets and the
tokio runtime.

As this crate needs to use RAW sockets, it must either be run as root
or permission must explicitly be set via
`sudo setcap cap_net_raw=+eip path/to/binary`.

## Features

* `stream`: implements `Stream` for `MeasureManyStream`.

## MSRV version policy

This project has a CI job to prevent accidental bumping of the MSRV.
We might bump MSRV version at any time. If you require a lower MSRV
please open an issue.

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
