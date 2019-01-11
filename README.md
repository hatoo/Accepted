Accepted
===

[![crates.io](https://img.shields.io/crates/v/accepted.svg)](https://crates.io/crates/accepted)

A terminal text editor to be **ACCEPTED**.

![Accepted screenshot](demo.png "acc")

## Description

A modal text editor for competitive programmer written with Rust.

Currently Accepted supports Rust and C++.

### Features

* Autoformat with [Rustfmt](https://github.com/rust-lang-nursery/rustfmt) / [ClangFormat](https://clang.llvm.org/docs/ClangFormat.html)
* Completion with [RLS](https://github.com/rust-lang-nursery/rls) / [Clangd](https://clang.llvm.org/extra/clangd.html)
* Auto compile and show compiler messages
* Easy to test a single a code
* VScode style snippet support
* Mouse support

## Install

You need nightly Rust.

```
$ cargo install accepted
```

### For Rust users

Install rust components.

```
$ rustup component add rls-preview
$ rustup component add rust-src
$ rustup component add rustfmt-preview
```

### For C++ users

Install clang, clang-format and clangd.

# Usage

```
$ acc [file]
```

TODO: More precisely.

## Basic

Many commands of `acc` is same as Vim.

i, I, a, A, o, O to insert mode and Esc to return.

hjkl, w, e, b to move cursor.

y, d, c, v, V works like vim

## Space Prefix

Some of commands can run with space as a prefix.

SPACE -> q to Quit.

SPACE -> s to Save.

SPACE -> a to Save As.

SPACE -> y to Copy all to clipboard

SPACE -> SPACE to Rustfmt.

SPACE -> t to compile and run with clipboard input.

SPACE -> T to compile (optimized) and run with clipboard input.

SPACE -> q to Quit.

## Snippet support

This supports vscode style snippet.

You can configure by toml file placed in `[config_dir]/acc/init.toml`

config_dir is defined in [here](https://docs.rs/dirs/1.0.3/dirs/fn.config_dir.html).

The only configurable thing is snippet 

```
snippet = ["path_to_snippet_file"]
```


## Contribution

Any kind of contribution including feature request is welcome !!