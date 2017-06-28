# unosolo

> **Work-in-progress Rust application that converts C++ libraries to single self-contained headers.**

[![stability][badge.stability]][stability]
[![license][badge.license]][license]
[![gratipay][badge.gratipay]][gratipay]
![badge.rust](https://img.shields.io/badge/rust-nightly-ff69b4.svg?style=flat-square)

[badge.stability]: https://img.shields.io/badge/stability-experimental-orange.svg?style=flat-square
[badge.license]: http://img.shields.io/badge/license-mit-blue.svg?style=flat-square
[badge.gratipay]: https://img.shields.io/gratipay/user/SuperV1234.svg?style=flat-square

[stability]: http://github.com/badges/stability-badges
[license]: https://github.com/SuperV1234/unosolo/blob/master/LICENSE
[gratipay]: https://gratipay.com/~SuperV1234/


## Disclaimer

This is my first Rust project, mainly created to start getting used to the language. The code quality is horrible, the algorithm used to create and traverse the `#include` graph is suboptimal, and only *header-only* libraries with relative `#include "..."` directives are currently supported.

My intention is to improve `unosolo` as I get better with Rust and the final goal is being able to successfully use it on popular libraries.

I also do not encourage people to create single-header libraries and use those in their projects: they're mainly useful when dealing with very complicated build systems or when experimenting on an online compiler that doesn't allow users to easily import multiple files.

*Contributions and code reviews are welcome!*



## Overview

Given a set of paths containing the C++ library's header files and a "top-level include" file where the graph traversal will start from, `unosolo` outputs a self-contained single-header version of the library to `stdout`. Here's the [`clap-rs`](https://github.com/kbknapp/clap-rs) auto-generated help:

```
unosolo 0.1.0
Vittorio Romeo <vittorio.romeo@outlook.com>
transforms a C++ header-only library in a self-contained single header.

USAGE:
    unosolo [FLAGS] [OPTIONS] --topinclude <top_include>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    verbose

OPTIONS:
    -p, --paths <paths>...            paths [default: .]
    -t, --topinclude <top_include>    top-level include
```

`unosolo` is currently able to transform [**`scelta`**](https://github.com/SuperV1234/scelta), my latest C++17 header-only library, to a single-header version. In fact, I've used `unosolo` to add two badges to `scelta`'s README that allow users to try the library either [on wandbox](https://wandbox.org/permlink/wSA55OCJz17k7Jtz) or [on godbolt](https://godbolt.org/g/4sQtkM). This idea was taken from Michael Park's excellent variant implementation: [`mpark::variant`](https://github.com/mpark/variant).

The command used to transform `scelta` was:

```bash
unosolo -p"./scelta/include" -v -t"./scelta/include/scelta.hpp" > scelta_single_header.hpp
```

It produced [this abomination](https://gist.github.com/SuperV1234/a5af0a8b92f75d83085a8e5fccf71d6a).
