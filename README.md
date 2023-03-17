# Acca

Ariel's Custom CPU Architecture

This is my attempt at designing my own toy CPU architecture. Ideally, I'd like
to create:

  * An emulator
  * An assembler
  * A C compiler backend (probably for LLVM)
  * Possibly a JIT recompiler (maybe based on LLVM)
  * Possibly a VHDL implementation of the processor

## Building the Specification

We use [mdBook](https://github.com/rust-lang/mdBook) along with
[mdbook-pdf](https://github.com/HollowMan6/mdbook-pdf) to compile the
specification into a PDF. Additionally, we use
[mdbook-pdf-outline](https://pypi.org/project/mdbook-pdf-outline/) to add a
useful outline/table-of-contents to the generated PDF.

The most-recent compiled draft of the specification can be found
[here](https://github.com/facekapow/acca/releases/latest).

## License

All projects within this repository are license under the Mozilla Public
License 2.0 (MPLv2).

Copyright (C) 2022 Ariel Abreu
