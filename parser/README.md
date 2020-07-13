# archlinux-repo-parser
[![Build Status](https://travis-ci.org/alesharik/archlinux-repo-rs.svg?branch=master)](https://travis-ci.com/github/alesharik/archlinux-repo-rs)

Arch Linux repository package definitions parser

## Usage
```toml
[dependencies]
archlinux-repo-parser = "0.1.0"
```

```rust
struct Test {
    #[serde(rename = "TEST")]
    test: String
}

fn main() {
    let string = archlinux_repo_parser::to_string(Test {test: "test" }).unwrap();
    let decoded: Test = archlinux_repo_parser::from_str(&string).unwrap();
}
```
## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.