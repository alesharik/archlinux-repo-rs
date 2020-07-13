# archlinux-repo-parser
[![Build Status](https://travis-ci.org/alesharik/archlinux-repo-rs.svg?branch=master)](https://travis-ci.com/github/alesharik/archlinux-repo-rs)
[![Docs](https://docs.rs/archlinux-repo-parser/badge.svg)](https://docs.rs/archlinux-repo-parser)

Arch Linux repository package definitions parser

## Usage
```toml
[dependencies]
archlinux-repo-parser = "0.1.1"
```

```rust
struct Test {
    #[serde(rename = "TEST")]
    test: String
}

fn main() {
    let test = Test {test: "test".to_owned() };
let string = archlinux_repo_parser::to_string(&test).unwrap();
    let decoded: Test = archlinux_repo_parser::from_str(&string).unwrap();
}
```

## Example of package definition file
```
%FILENAME%
mingw-w64-x86_64-ag-2.2.0-1-any.pkg.tar.xz

%NAME%
mingw-w64-x86_64-ag

%BASE%
mingw-w64-ag

%VERSION%
2.2.0-1

%DESC%
The Silver Searcher: An attempt to make something better than ack, which itself is better than grep (mingw-w64)

%CSIZE%
79428

%ISIZE%
145408

%MD5SUM%
3368b34f1506e7fd84185901dfd5ac2f

%SHA256SUM%
c2b39a45ddd3983f3f4d7f6df34935999454a4bff345d88c8c6e66c81a2f6d7e

%PGPSIG%
iHUEABEIAB0WIQStNRxQrghXdetZMztfku/BpH1FoQUCXQOnfgAKCRBfku/BpH1FoZzhAQCEjnsM18ZCqJHhEE0BwXVsH9ONj87w0Wt8W77ZElUcKwD/RcnlD4Ef7gmOdl+puSDMUNylHQ2wlOdumaVSkQlOhLw=

%URL%
https://geoff.greer.fm/ag

%LICENSE%
Apache

%ARCH%
any

%BUILDDATE%
1560520506

%PACKAGER%
Alexey Pavlov <alexpux@gmail.com>

%DEPENDS%
mingw-w64-x86_64-pcre
mingw-w64-x86_64-xz
mingw-w64-x86_64-zlib

%MAKEDEPENDS%
mingw-w64-x86_64-gcc
mingw-w64-x86_64-pkg-config
```

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.