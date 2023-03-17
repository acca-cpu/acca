# Operation

## Startup

On startup, the processor starts in PL0 with maskable exceptions masked and all
ALU flags (`CZOS`) cleared. The core registers are cleared (i.e. all zeros).

The processor begins executing code at the address `0x400`.

> The startup address was chosen because it allows an
> [Exception Configuration Table][ect] and an [Exception Vector Table][evt] to
> fit into the first 0x400 bytes.

## Privilege Levels

Privilege levels are used to restrict access to certain hardware components to
trusted software and provide a level of privilege isolation. There are two
available privilege levels: PL0 and PL1. PL0 is fully trusted and has full
access to all hardware components; PL1 is untrusted and has limited access to
hardware components.

For example, certain machine registers can only be accessed from PL0, while
others have limited access (i.e. read-only) from PL1 but are fully accessible
(i.e. read-write) for PL0.

[ect]: ./exceptions.md#exception-configuration-table
[evt]: ./exceptions.md#exception-vector-table
