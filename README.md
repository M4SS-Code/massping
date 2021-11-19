# massping

A simplified version of [fastping-rs](https://github.com/bparli/fastping-rs)
without some of its [issues](https://github.com/bparli/fastping-rs/issues/25).

Depends on the tokio 1 runtime.

Tested on: Linux

As with the original version, this one also requires to create raw sockets,
so the permission must either be explicitly set
(`sudo setcap cap_net_raw=eip /path/to/binary` for example) or be run as root.
