# edc2svd

[![Crates.io](https://img.shields.io/crates/v/edc2svd.svg)](https://crates.io/crates/edc2svd)

Convert an MCU register description from the EDC format to the SVD format

EDC files are used to describe the special function registers of PIC32
microcontrollers. This program can generate Peripheral Access Crates to be used
in Rust programs.

## Usage

First, an EDC file is converted with this tool to an SVD file. Then [`svd2rust`]
can be used to generate the Peripheral Access Crate as follows:

    edc2svd PIC32MX170F256B.PIC PIC32MX170F256B.svd
    svd2rust --target none -i PIC32MX170F256B.svd
    rm -rf src
    form -i lib.rs -o src/ && rm lib.rs
    cargo fmt

[`svd2rust`]: https://crates.io/crates/svd2rust

## Installation

    $ cargo install edc2svd
