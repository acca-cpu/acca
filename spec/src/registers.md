# Registers

Acca has 16 64-bit core registers: `r0` through `r15`. All of them can be used
as general-purpose registers, although the last three (`r13`-`r15`) have special
processor-defined uses, as well as special aliases (`rsp`, `rfp`, and `rlr`,
respectively).

Note that, because Acca is a 64-bit architecture (and is not based on any
previous architecture), words on Acca are 64 bits wide.

## Suffixes

Registers names can be suffixed to limit their width when used in instructions.Such suffixed registers take the form `r<n><s>`, where `n` is an integer (0
through 15) and `s` is one of the following suffixes:

  * `b` for "byte" - Limits the register to the lowest 8 bits.
  * `d` for "double-byte" - Limits the register to the lowest 16 bits.
  * `q` for "quad-byte" - Limits the register to the lowest 32 bits.
  * `w` for "word" - Limits the register to the lowest 64 bits (i.e. the whole
    register).

Note that when an instruction writes to a portion of a register, it does *not*
clear the rest of the register. For example, an instruction that writes to
`r0b` (i.e. the lowest 8 bits of the `r0` register) does *not* cause the upper
56 bits of `r0` to be cleared.

## Calling Convention

This section describes the standard calling convention for the Acca architecture.

### Register Use

  * 7 registers for parameters/return values (`r0`-`r6`).
    * Caller-saved
    * General-purpose
    * These are all used for passing parameters.
    * Only `r0` and `r1` are used for return values; `r0` is used first, with
      `r1` used for returning the upper 64 bits of 128-bit return values.
  * 2 registers for keeping local variables (`r7`-`r8`).
    * Callee-saved
    * General-purpose
  * 4 scratch registers (`r9`-`r12`).
    * Caller-saved
    * General-purpose
  * A stack pointer (`r13` or `rsp`)
    * Callee-saved (generally just saved in the frame pointer)
    * Special-use
  * A frame pointer (`r14` or `rfp`)
    * Callee-saved
    * Special-use
  * A link register (`r15` or `rlr`)
    * Caller-saved
    * Special-use

You'll note that some registers above say "special-use". Such registers can be
used like other general-purpose registers. However, unlike other
general-purpose registers, they have a special processor-defined use which may
affect how you can/should use them.

  * The stack pointer (`r13` or `rsp`) is automatically modified by the `push`
    and `pop` family of instructions.
  * The frame pointer (`r14` or `rfp`) is not automatically modified or read by
    any instruction. If a frame pointer is not needed in a particular function,
    this register can be used as a general-purpose callee-saved register like
    `r7` and `r8` (useful for storing local variables).
  * The link register (`r15` or `rlr`) is automatically modified by the `call`
    family of instructions. It is also read by the `ret` family of instructions
    to determine the address to return to.

### Stack Use

In the Acca architecture, the stack grows downwards (like most other CPU
architectures). This means that the stack pointer is initially set to the
highest address of the stack; pushing values to the stack decrements the
pointer while popping values off the stack increments the pointer.

Upon entering a function, the stack must be aligned on a 16-byte boundary.
Note that also implies that the stack must be 16-byte aligned when calling a
function.

By default, the stack must also contain a 128-byte "red zone" just below
the stack pointer, which allows leaf functions (those that do not call other
functions) to use this space without touching the stack pointer.

### Parameters and Return Values

As described in [Register Use][register_use], the first 7 parameters should be
passed in registers `r0` through `r6`. Any remaining parameters should be
spilled onto the stack, in reverse order: the last spilled parameter should be
pushed first, leaving it at the highest address, while the first spilled
parameter (i.e. the eight parameter) should be pushed last, leaving it at the
lowest address.

Ordinarily, functions that return a value will do so in `r0`. If necessary,
values larger than 64 bits may be returned in `r1:r0` (meaning the upper 64
bits go in `r1` and the lower 64 bits go in `r0`). If the function needs to
return a value larger than 128 bits, the caller should allocate enough space on
the stack before the call. Upon entering the function, the stack pointer should
be pointing to the base of this return space (i.e. the lowest address of the
space allocated for the return value).

## CPU Flags

The Acca architecture has a 64-bit CPU flags register. This register can be
accessed as the `flags` machine register via the `ldm` and `stm`
instructions. Additionally, many arithmetic and conditional instructions read
from and write to the CPU flags register.

|  63-6  |  5   |  4  |  3  |  2  |  1  |  0  |
| ------ | ---- | --- | --- | --- | --- | --- |
| *RES0* | `PL` | `E` | `S` | `O` | `Z` | `C` |

  * `C` - Carry/borrow flag - Set to `1` if an operation has generated
    a carry or borrow
  * `Z` - Zero flag - Set to `1` if the result of an operation is `0`
  * `O` - Overflow flag - Set to `1` if an operation results in
    overflow or underflow
  * `S` - Sign flag - Set to `1` if the result of an operation is
    negative
  * `E` - Exception enable flag - If this is `1`, maskable exceptions
    are unmasked (enabled); if this is `0`, maskable exceptions are masked
    (disabled)
  * `PL` - Privilege level - The value of this bit indicates
    the current privilege level of the processor. If `0`, the processor is in
    PL0; if `1`, the processor is in PL1.

## Standard Machine Registers

All standard machine registers are 64 bits wide and must be loaded and stored
using 64-bit registers with the [`ldm`][ldm] and
[`stm`][stm] instructions.

The value that must be encoded into the immediate of the `ldm` and `stm`
instructions in order to access these register is the value of their
"Identifier" fields.

### `flags`

"CPU flags"

  * PL0: Read-write; PL1: Mixed access
  * Identifier: 0
  * Startup value: `0` (all zeros)

See [CPU Flags][cpu_flags].

Only bits 0 through 3 may be modified in PL1; in PL0, all unreserved bits may
be freely modified.

### `elr`

"exception link register"

  * PL0: Read-write; PL1: No access
  * Identifier: 1
  * Startup value: *UDF*

This register is set to the absolute address of the instruction to return to
when returning from an exception with [`eret`][eret].

### `esp`

"exception stack pointer"

  * PL0: Read-write; PL1: No access
  * Identifier: 2
  * Startup value: *UDF*

This register is set to the value of the stack pointer (`r13` or `rsp`) when
taking an exception. It is used by [`eret`][eret] as the
value to load into the stack pointer register when returning from an exception.

### `eflags`

"exception CPU flags"

  * PL0: Read-write; PL1: No access
  * Identifier: 3
  * Startup value: *UDF*

This register is set to the value of the [`flags`][flags] machine register when
taking an exception. It is also used by [`eret`][eret] as the
value to load into the `flags` machine register when returning from an
exception.

### `einfo`

"exception information"

  * PL0: Read-only; PL1: No access
  * Identifier: 4
  * Startup value: *UDF*

This register is used to provide information about the current exception.

See [Exception Information][exception_info].

### `eaddr`

"exception address"

  * PL0: Read-only; PL1: No access
  * Identifier: 5
  * Startup value: *UDF*

This register is used to provide information about an address associated with
the current exception. The meaning of the value in this register depends on the
exception type.

See [Exception Information][exception_info].

### `evtable`

"exception vector table"

  * PL0: Read-write; PL1: No access
  * Identifier: 6
  * Startup value: *UDF*

This register contains an absolute address pointing to an
[exception vector table][evt] which the
processor uses to handle exceptions.

### `ectable`

"exception configuration table"

  * PL0: Read-write; PL1: No access
  * Identifier: 7
  * Startup value: *UDF*

This register contains an absolute address pointing to an
[exception configuration table][ect]
which is used to configure exceptions.

[register_use]: #register-use
[ldm]: ./instructions.md#ldm-dreg-aimm22
[stm]: ./instructions.md#stm-dimm22-areg
[cpu_flags]: #cpu-flags
[eret]: ./instructions.md#eret
[flags]: #flags
[exception_info]: ./exceptions.md#exception-information
[evt]: ./exceptions.md#exception-vector-table
[ect]: ./exceptions.md#exception-configuration-table
