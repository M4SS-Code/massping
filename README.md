# massping

[![crates.io](https://img.shields.io/crates/v/massping.svg)](https://crates.io/crates/massping)
[![Documentation](https://docs.rs/massping/badge.svg)](https://docs.rs/massping)
[![dependency status](https://deps.rs/crate/massping/0.2.2/status.svg)](https://deps.rs/crate/massping/0.2.2)
[![Rustc Version 1.64.0+](https://img.shields.io/badge/rustc-1.64.0+-lightgray.svg)](https://forge.rust-lang.org/)
[![CI](https://github.com/M4SS-Code/massping/actions/workflows/ci.yml/badge.svg)](https://github.com/M4SS-Code/massping/actions/workflows/ci.yml)

A simplified version of [fastping-rs](https://github.com/bparli/fastping-rs)
without some of its [issues](https://github.com/bparli/fastping-rs/issues/25).

Depends on the tokio 1 runtime.

Tested on: Linux

As with the original version, this one also requires to create raw sockets,
so the permission must either be explicitly set
(`sudo setcap cap_net_raw=eip /path/to/binary` for example) or be run as root.
