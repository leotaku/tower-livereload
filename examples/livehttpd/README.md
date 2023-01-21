# livehttpd

[![Crates.io][crates-badge]][crates-url]
[![Lib.rs][librs-badge]][librs-url]

[crates-url]: https://crates.io/crates/livehttpd
[librs-url]: https://lib.rs/crates/livehttpd

[crates-badge]: https://img.shields.io/crates/v/livehttpd.svg
[librs-badge]: https://img.shields.io/badge/lib.rs-linked-informational

A development server with live-reload capabilities.

## Usage

Livehttpd can be pointed at a directory, which it will then serve locally. Whenever any file in the specified directory changes, livehttpd will reload all connected web-browsers.

You can also stop and restart livehttpd, in which case web browsers will reload whenever they can connect to the server again. This may be useful when integrating livehttpd into custom build processes.

```sh
# Serve the current directory
livehttpd .
```

Please refer to the help message for additional command-line options.

## License

`livehttpd` is free and open source software distributed under the terms of either the MIT or the Apache 2.0 license, at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
