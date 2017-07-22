# Ruyi

Ruyi is an event-driven framework for non-blocking, asynchronous I/O in Rust powered by [futures](https://github.com/alexcrichton/futures-rs).

[![crates.io](https://img.shields.io/crates/v/ruyi.svg)](https://crates.io/crates/ruyi)
[![docs.rs](https://docs.rs/ruyi/badge.svg)](https://docs.rs/ruyi)
[![Build Status](https://travis-ci.org/agemocui/ruyi.svg?branch=master)](https://travis-ci.org/agemocui/ruyi)

## Usage

To use `ruyi`, add the following to your `Cargo.toml`:

```toml
[dependencies]
ruyi = "0.1"
```

and then, add this to your crate:

```rust
extern crate ruyi;
```

Examples can be found in the `examples` folder in this repository.

## Features

* `Share-nothing` - One loop per core model is recommended. Use SPSC queue to communicate between cores.
* `Chained-buffer` - Reduces/avoids memory copy as much as possible.
* `Timer` - Heap based and hashed timing-wheel based.
* `Graceful Shutdown` - `Gate` can be used to ensure that task completes before event loop ends.

## Platforms

Currently supported:

* Linux 2.6.28+

To be supported:

* Windows 7+
* OS X
* FreeBSD 10.0+
* OpenBSD 5.7+
* NetBSD 8.0+

## License

Ruyi is distributed under the terms of both the MIT License and the Apache License (Version 2.0).

See `LICENSE-APACHE` and `LICENSE-MIT` for details.