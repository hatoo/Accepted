Accepted
===

A terminal text editor to be **ACCEPTED**.

![Accepted screenshot](demo.png?raw=true "acc")

## Description

A modal text editor for competitive rust programmer written with Rust.

A primary target is in competitive programmers.

So this can Rustfmt and Test run with clipboard input which is useful to competitive programming by simple command.

### Features

* Autoformat with [Rustfmt](https://github.com/rust-lang-nursery/rustfmt) 
* Completion with [Racer](https://github.com/racer-rust/racer)
* Easy to test a single rust code
* VScode style snippet support

## Install

You need nightly Rust.

```
$ cargo install accepted
```

If you haven't install [Rustfmt](https://github.com/rust-lang-nursery/rustfmt), i recommend to install it.

# Usage

```
$ acc [file]
```

TODO: More precisely.

## Basic

Many commands of `acc` is same as Vim.

i, I, a, A to insert mode and Esc to return.

hjkl, w, e, b to move cursor.

y, d, c, v, V works like vim

## Space Prefix

Some of commands can run with space as a prefix.

SPACE -> q to Quit.

SPACE -> s to Save.

SPACE -> a to Save As.

SPACE -> SPACE to Rustfmt.

SPACE -> t to conpile and run with clipboard input.

SPACE -> T to conpile and run with clipboard input.

SPACE -> q to Quit.

## Snippet support

This supports vscode style snippet.

You can configure by toml file palaced in `[config_dir]/acc/init.toml`

config_dir is defined in [here](https://docs.rs/dirs/1.0.3/dirs/fn.config_dir.html).

The only configurable this is snippet 

```
snippet = ["path_to_snippet_file"]
```


## Contribution

Any kind of contribution including feature request is welcome !!