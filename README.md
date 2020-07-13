# archlinux-repo
[![Build Status](https://travis-ci.org/alesharik/archlinux-repo-rs.svg?branch=master)](https://travis-ci.com/github/alesharik/archlinux-repo-rs)

Arch Linux repository parser

## Usage
```toml
[dependencies]
archlinux-repo = "0.1.0"
```

```rust
async fn main() {
    let repo = Repository::load("mingw64", "http://repo.msys2.org/mingw/x86_64")
        .await
        .unwrap();
    let gtk = &repo["mingw-w64-gtk3"];
    for package in &repo {
        println!("{}", &package.name);
    }
}
```

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.