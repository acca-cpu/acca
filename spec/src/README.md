# The Acca Architecture Specification

This document describes the Acca CPU architecture, including instructions
(behaviors, encodings), general processor operation (e.g. startup sequence,
exception behavior), and integration into a computer system (e.g. interfacing
with peripherals).

The Acca architecture is a 64-bit little-endian architecture. This means the
width of the core registers is 64 bits and the order in which multi-byte values
are loaded and stored is least-significant byte at the lowest address.

## General Specification Notes

  * When a value or field is marked "*RES0*", this indicates that writing a `1`
    to it results in an invalid operation exception or an invalid instruction
    exception (depending on whether the reserved field describes the
    instruction itself) and reading from it will return `0`.
  * Likewise, when a value or field is marked "*RES1*", this indicates that
    writing a 0 to it results in an invalid operation exception or an invalid
    instruction exception and reading from it will return `1`.
  * When a value or field is marked "*UDF*", this indicates that the field does
    not have a predefined value; any value is accepted for the field and any
    value may be read from the field.
