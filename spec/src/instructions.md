# Instructions

There are various classes/families of instructions in the Acca architecture.
To make them easier to find, we'll describe each of them in this document
grouped by family. Each section header is the name of a family.

You'll note that various instructions have `[.s]` suffixes. This indicates
that those instructions can optionally be suffixed with the following suffixes
to indicate the width of the values they operate on:

  * `b` for "byte" - Operates on 8 bits.
  * `d` for "double-byte" - Operates on 16 bits.
  * `q` for "quad-byte" - Operates on 32 bits.
  * `w` for "word" - Operates on 64 bits (i.e. the whole register).

For example, `pushs.b` pushes a single byte to the stack, while `pops.q` pops a
32-bit value from the stack.

Ordinarily, instructions that accept such prefixes will infer their width based
on the width of the registers used in the instruction. However, some
instructions cannot do this; for such instructions, a suffix of `<.s>` in the
description indicates that the size suffix is required.

Additionally, note that is illegal to mix registers with different width
suffixes within the same instruction. For example, `add r0w, r1w, r2b` is not a
valid instruction. It is also illegal to use registers with a different size
suffix than the one specified in the instruction. For example, `pushs.b r0d`
is not a valid instruction, while `pushs.b r0` and `pushs.b r0b` are both valid.

## Encoding Notes

### Booleans

Booleans (`bool`) are encoded as single bits: `0` for `false` and `1` for
`true`.

### Immediates

Immediates are encoded as their binary representation. For example, an `imm8`
would be encoded as the 8 bits that represent the immediate.

### Registers

Registers are encoded in 4 bits according to their register number. For example,
register `r0` is represented by `0000`; register `r10` is represented by
`1010`; and register `r15` is represented by `1111`.

#### Optionally-null Registers

For registers that can optionally be null, they are represented by 5 bits; the
most-significant bit indicates whether or not they are null. An MSB of `0`
indicates they are present, while an MSB of `1` indicates they are null.
Additionally, if they are null (i.e. the MSB is `1`), all other bits are
required to be `1` as well. The following tables may help:

> Let `r` be the encoding for a register as described in [Registers][registers].
>
> **If the MSB is `0`**:
>
> |  4  |  3-0   |
> | --- | ------ |
> | `0` | `rrrr` |
>
> **If the MSB is `1`**:
>
> |  4  |  3-0   |
> | --- | ------ |
> | `1` | *RES1* |

## Memory

### `pushs[.s] a:reg`

"push single"

This instruction pushes a single register onto the stack. It first decrements
the stack pointer by the necessary amount and then stores the value in the given
register to the location pointed to by the stack pointer.

This instruction does not align the stack or add any padding in any way; only
the space required for the value is used. For example, `pushs.b` will decrement
the stack pointer by a single byte and store only a single byte, while `pushs.q`
will decrement the stack pointer by 4 bytes and store 4 bytes.

Note that if the register given is the stack pointer (`r13` or `rsp`), the value
stored onto the stack is the value of the stack pointer *before* any decrement
performed by this instruction.


#### Encoding

|  31-26   |          25-6          |  5-4  |  3-0   |
| -------- | ---------------------- | ----- | ------ |
| `110111` | `00000000000000000000` | `ss`  | `aaaa` |

### `pushp[.s] a:reg, b:reg`

"push pair"

This instruction pushes a pair of registers onto the stack. It first decrements
the stack pointer by the necessary amount to store both registers and then
stores the values in the given registers to the location pointer to by the stack
pointer. The order of the stored values is `a` at the lowest address and `b` at
the following (higher) address.

Like `pushs`, this instruction only uses the necessary space for both registers.
For example, `pushp.b` will decrement the stack pointer by 2 bytes, while
`pushp.q` will decrement the stack by 8 bytes.

Like `pushs`, if one or both of the registers given are the stack pointer, the
value pushed onto the stack is its value *before* any decrement performed by
this instruction.

#### Encoding

|  31-26   |       25-10        |  9-8  |  7-4   |  3-0   |
| -------- | ------------------ | ----- | ------ | ------ |
| `110110` | `0000000000000000` | `ss`  | `aaaa` | `bbbb` |

### `pops[.s] a:reg`

"pop single"

This instruction pops a single value off the stack. It first loads the value on
the stack (where the stack pointer is currently pointing) into the given
register and then increments the stack pointer by the necessary amount.

This instruction only uses the necessary space for the register. For example,
`pops.b` will increment the stack pointer by 1 byte, while `pops.q` will
increment the stack by 4 bytes.

If the register is the stack pointer, the instruction simply loads the new
value of the stack pointer from the stack; the increment is skipped/ignored.

#### Encoding

|  31-26   |          25-6          |  5-4  |  3-0   |
| -------- | ---------------------- | ----- | ------ |
| `110101` | `00000000000000000000` | `ss`  | `aaaa` |

### `popp[.s] a:reg, b:reg`

"pop pair"

This instruction pops a pair of values off the stack. It first loads the values
on the stack into the given registers and then increments the stack pointer by
the necessary amount. The order of the loaded values is `a` from the lowest
address and `b` from the following (higher) address.

Like `pops`, this instruction only uses the necessary space for both registers.
For example, `popp.b` will increment the stack pointer by 2 bytes, while
`popp.q` will increment the stack by 8 bytes.

Like `pops`, if one of the registers given is the stack pointer, the instruction
simply loads the new value of the stack pointer from the stack; the increment
is skipped/ignored. If both of the registers given are the stack pointer, the
first load (`a`) is ignored and the new value of the stack pointer is taken
from the second load (`b`).

#### Encoding

|  31-26   |       25-10        |  9-8  |  7-4   |  3-0   |
| -------- | ------------------ | ----- | ------ | ------ |
| `110100` | `0000000000000000` | `ss`  | `aaaa` | `bbbb` |

### `lds[.s] d:reg, a:reg`

"load single"

This instructions loads a single value from the memory address specified by `a`
into register `d`. Register `a` is first read and used to locate the desired
memory address, and then the value at that location is loaded into register `d`;
this means that it is safe to use the same register for both arguments.

#### Encoding

|  31-26   |       25-10        |  9-8  |  7-4   |  3-0   |
| -------- | ------------------ | ----- | ------ | ------ |
| `110011` | `0000000000000000` | `ss`  | `dddd` | `aaaa` |

### `ldp[.s] d:reg, e:reg, a:reg`

"load pair"

This instruction loads a pair of values from the memory address specified by `a`
into registers `d` and `e`. The order of the values loaded is `d` from the
lowest address and `e` from the following (higher) address. Register `a` is
first read and used to locate the desired memory address, and then the values
at that location are loaded into registers `d` and `e`; this means that it is
safe to use the same register `a` for one or both registers `d` and `e`.

Note that if the same register is used for `d` and `e`, the first load (`d`) is
ignored and the new value of the register is taken from the second load (`e`).

#### Encoding

|  31-26   |     25-14      | 13-12 |  11-8  |  7-4   |  3-0   |
| -------- | -------------- | ----- | ------ | ------ | ------ |
| `110010` | `000000000000` | `ss`  | `dddd` | `eeee` | `aaaa` |

### `sts[.s] a:reg, b:reg`

"store single"

This instruction stores a single value from register `b` into the memory address
specified by `a`.

#### Encoding

|  31-26   |       25-10        |  9-8  |  7-4   |  3-0   |
| -------- | ------------------ | ----- | ------ | ------ |
| `110001` | `0000000000000000` | `ss`  | `aaaa` | `bbbb` |

### `stp[.s] a:reg, b:reg, c:reg`

"store pair"

This instruction stores a pair of values from registers `b` and `c` into the
memory address specified by `a`. The order of the values stored is `b` at the
lowest address and `c` at the following (higher) address.

#### Encoding

|  31-26   |     25-14      | 13-12 |  11-8  |  7-4   |  3-0   |
| -------- | -------------- | ----- | ------ | ------ | ------ |
| `110000` | `000000000000` | `ss`  | `aaaa` | `bbbb` | `cccc` |

## Arithmetic and Logic

### `ldi d:reg, a:imm16[, b:imm6][, c:imm2]`

"load immediate"

This instruction loads a 16-bit immediate value `a` into register `d`. The value
can optionally be shifted left by `b` bits; the default is not to shift at all.
Additionally, `c` specifies a mask of bits to clear before executing the
operation (which defaults to `0`).

If `c` is `0`, no additional bits are cleared (only the ones that will be
loaded). If `c` is `1`, all bits below the bits that will be loaded are
cleared. If `c` is `2`, all bits above the bits that will be loaded are
cleared. If `c` is `3`, the entire register is cleared.

When loading, the destination bits (those that will be overwritten by the load)
are first cleared. Then, additional bits (if any) are cleared according to `c`.
Then, the value `a` is shifted left by `b` bits. Finally, the shifted value is
loaded (bitwise ORed) into `d`.

In pseudocode, the operation performed is:

```rust
let shift: u64 = b as u64;
let mask: u64 = 0xffffu64 << shift;

d &= ~mask;

if c == 0 {
  // clear nothing
} else if c == 1 {
  // clear the lower bits
  d &= 0xffffffffffffffffu64 << shift;
} else if c == 2 {
  // clear the upper bits
  d &= ~(0xffffffffffffffffu64 << (shift + 16u64));
} else if c == 3 {
  // clear everything
  d = 0u64;
}

d |= (a as u64) << shift;
```

#### Encoding

|  31-26   | 25-24 |        23-8        |   7-2    | 1-0  |
| -------- | ----- | ------------------ | -------- | ---- |
| `101011` | `00`  | `aaaaaaaaaaaaaaaa` | `bbbbbb` | `cc` |

### `copy[.s] d:reg, s:reg`

"copy"

This instruction copies the value from register `s` into register `d`.

#### Encoding

|  31-26   |       25-10        | 9-8  |  7-4   |  3-0   |
| -------- | ------------------ | ---- | ------ | ------ |
| `101010` | `0000000000000000` | `ss` | `dddd` | `SSSS` |

### `add[.s] d:reg | null, a:reg, b:reg[, c:bool][, f:bool]`

"add"

This instruction adds the values in registers `a` and `b` and stores the result
into register `d`. If `c` is `0`/`false` (the default), the addition is
performed without a carry; if `c` is `1`/`true`, the current value of the carry
flag (`C`) is used as a carry, i.e. a third (one-bit) addend.

Without a carry, the operation is `a + b = d`. With a carry, the operation
is `(a + b) + C = d`.

If `b` is an immediate value, it is zero-extended if necessary to fit the bit
width of the operation.

Note that `d` may also be `null`; in this case, the result of the operation is
discarded. This is useful when combined with `f` equal to `1`/`true` as it
allows registers `a` and `b` to be compared for use in conditionals without
having to store the result anywhere.

If `f` is `0`/`false` (the default), then this instruction does *not* modify
the CPU flags register. If `f` is `1`/`true`, then this instruction *does*
modify the CPU flags register as follows:

> Let `w` be the bit width of the operation and let `m` be the most significant
> bit of the operation (a.k.a. `m = w - 1`).
>
>   * `C` - Set to `1` if an unsigned overflow occurred, cleared otherwise - `(a[m] AND b[m]) OR ((a[m] OR b[m]) AND NOT(result[m]))`
>   * `Z` - Set to `1` if the result consists of all `0` bits, cleared otherwise - `NOT(result[0]) AND NOT(result[1]) ... AND NOT(result[m])`
>   * `O` - Set to `1` if a signed overflow occurred, cleared otherwise - `(a[m] AND b[m] AND NOT(result[m])) OR (NOT(a[m]) AND NOT(b[m]) AND result[m])`
>   * `S` - Set to `1` if the signed interpretation of the result is negative, cleared otherwise - `result[m]`

#### Encoding

|  31-26   |    25-17    | 16-15 | 14  | 13  |  12-8   |  7-4   |  3-0   |
| -------- | ----------- | ----- | --- | --- | ------- | ------ | ------ |
| `101001` | `000000000` | `ss`  | `c` | `f` | `ddddd` | `aaaa` | `bbbb` |

### `add[.s] d:reg | null, a:reg, b:imm11[, S:imm3][, A: bool][, c:bool][, f:bool]`

"add"

This instruction is identical to
[`add` (register)][add_register] except that `b` is
an optionally-shifted immediate instead of a register. The equivalent value for
`b` is calculated by shifting the immediate `b` left by `S` bits. By default,
the immediate is not shifted at all (i.e. the default value of `S` is `0`). The
value is then either zero-extended (if `A` is `0`/`false`, the default) or
sign-extended (if `A` is `1`/`true`) as necessary to fit the bit width of the
operation.

#### Encoding

|  31-26   | 25  | 24  |  23-19  | 18-15  | 14  | 13-11 |     10-0      |
| -------- | --- | --- | ------- | ------ | --- | ----- | ------------- |
| `1011ss` | `c` | `f` | `ddddd` | `aaaa` | `A` | `SSS` | `bbbbbbbbbbb` |

### `sub[.s] d:reg | null, a:reg, b:reg[, B:bool][, f:bool]`

"subtract"

This instruction subtracts the value in register `b` from the value in register
`a` and stores the result into register `d`. If `B` is `0` `false` (the
default), the subtraction is performed without a borrow; if `B` is `1`/`true`,
the current value of the carry flag (`C`) is used as a borrow, i.e. a second
(one-bit) subtrahend.

Without a borrow, the operation is `a - b = d`. With a carry, the operation is
`(a - b) - C = d`.

Note that `d` may also be `null`; in this case, the result of the operation is
discarded. This is useful when combined with `f` equal to `1`/`true` as it
allows registers `a` and `b` to be compared for use in conditionals without
having to store the result anywhere.

If `f` is `0`/`false` (the default), then this instruction does *not* modify
the CPU flags register. If `f` is `1`/`true`, then this instruction *does*
modify the CPU flags register as follows:

> Let `w` be the bit width of the operation and let `m` be the most significant
> bit of the operation (a.k.a. `m = w - 1`).
>
>   * `C` - Set to `1` if an unsigned underflow occurred, i.e. the unsigned value in `b` is larger than the unsigned value in `a`, cleared otherwise - `(NOT(a[m]) AND b[m]) OR (b[m] AND result[m]) OR (result[m] AND NOT(a[m]))`
>   * `Z` - Set to `1` if the result consists of all `0` bits, cleared otherwise - `NOT(result[0]) AND NOT(result[1]) ... AND NOT(result[m])`
>   * `O` - Set to `1` if a signed underflow occurred, cleared otherwise - `(a[m] AND NOT(b[m]) AND NOT(result[m])) OR (NOT(a[m]) AND b[m] AND result[m])`
>   * `S` - Set to `1` if the signed interpretation of the result is negative, cleared otherwise - `result[m]`

#### Encoding

|  31-26   |    25-17    | 16-15 | 14  | 13  |  12-8   |  7-4   |  3-0   |
| -------- | ----------- | ----- | --- | --- | ------- | ------ | ------ |
| `101000` | `000000000` | `ss`  | `B` | `f` | `ddddd` | `aaaa` | `bbbb` |

### `sub[.s] d:reg | null, a:reg, b:imm11[, S:imm3][, A: bool][, B:bool][, f:bool]`

"subtract"

This instruction is identical to
[`sub` (register)][sub_register] except that `b` is
an optionally-shifted immediate instead of a register. The equivalent value for
`b` is calculated by shifting the immediate `b` left by `S` bits. By default,
the immediate is not shifted at all (i.e. the default value of `S` is `0`). The
value is then either zero-extended (if `A` is `0`/`false`, the default) or
sign-extended (if `A` is `1`/`true`) as necessary to fit the bit width of the
operation.

#### Encoding

|  31-26   | 25  | 24  |  23-19  | 18-15  | 14  | 13-11 |     10-0      |
| -------- | --- | --- | ------- | ------ | --- | ----- | ------------- |
| `1001ss` | `B` | `f` | `ddddd` | `aaaa` | `A` | `SSS` | `bbbbbbbbbbb` |

### `mul d:reg.s2, a:reg.s1, b:reg.s1[, S:bool][, f:bool]`

"multiply"

This instruction multiplies the values in registers `a` and `b` and stores the
result in register `d`. If `S` is `0`/`false` (the default), the operation is
an unsigned multiplication; if `S` is `1`/`true`, the operation is a signed
multiplication.

Note that, unlike some other operations, the destination is allowed to have a
different width than the operands. However, the width of the destination must
be greater than or equal to the width of the operands (i.e. `s2 >= s1`).

If `f` is `0`/`false` (the default), then this instruction does *not* modify
the CPU flags register. If `f` is `1`/`true`, then this instruction *does*
modify the CPU flags register as follows:

> Let `w` be the bit width of the destination and let `m` be the most
> significant bit of the operation (a.k.a. `m = w - 1`).
>
>   * `Z` - Set to `1` if the result consists of all `0` bits, cleared otherwise - `NOT(result[0]) AND NOT(result[1]) ... AND NOT(result[m])`
>   * `S` - Set to `1` if the operation is signed and the result is negative, cleared otherwise - `result[m]`

#### Encoding

Let `s` be `s1` above and let `t` be `s2` above.

|  31-26   |   25-18    | 17-16 | 15-14 | 13  | 12  |  11-8  |  7-4   |  3-0   |
| -------- | ---------- | ----- | ----- | --- | --- | ------ | ------ | ------ |
| `100011` | `00000000` | `ss`  | `tt`  | `S` | `f` | `dddd` | `aaaa` | `bbbb` |

### `div[.s] d:reg, r:reg, a:reg, b:reg[, S:bool][, f:bool]`

"divide"

This instruction divides the value in register `a` by the value in register `b`
and stores the result in register `d`; additionally, it stores the remainder
into register `r`. If `S` is `0`/`false` (the default), the operation is an
unsigned division; if `S` is `1`/`true`, the operation is a signed division.

If `d` and `r` are the same register, the remainder (`r`) is discarded and only
the quotient (`d`) is stored into the register.

If `f` is `0`/`false` (the default), then this instruction does *not* modify
the CPU flags register. If `f` is `1`/`true`, then this instruction *does*
modify the CPU flags register as follows:

> Let `w` be the bit width of the operation and let `m` be the most
> significant bit of the operation (a.k.a. `m = w - 1`).
>
>   * `Z` - Set to `1` if the result consists of all `0` bits, cleared otherwise - `NOT(result[0]) AND NOT(result[1]) ... AND NOT(result[m])`
>   * `S` - Set to `1` if the operation is signed and the result is negative, cleared otherwise - `result[m]`

#### Encoding

|  31-26   |  25-20   | 19-18 | 17  | 16  | 15-12  |  11-8  |  7-4   |  3-0   |
| -------- | -------- | ----- | --- | --- | ------ | ------ | ------ | ------ |
| `100010` | `000000` | `ss`  | `S` | `f` | `dddd` | `rrrr` | `aaaa` | `bbbb` |

### `and[.s] d:reg | null, a:reg, b:reg[, f:bool]`

"and"

This instruction performs a bitwise AND on the values in registers `a` and `b`
and stores the result in register `d`.

If `b` is an immediate value, it is zero-extended if necessary to fit the bit
width of the operation.

Note that `d` may also be `null`; in this case, the result of the operation is
discarded. This is useful when combined with `f` equal to `1`/`true` as it
allows registers `a` and `b` to be compared for use in conditionals without
having to store the result anywhere.

If `f` is `0`/`false` (the default), then this instruction does *not* modify
the CPU flags register. If `f` is `1`/`true`, then this instruction *does*
modify the CPU flags register as follows:

> Let `w` be the bit width of the operation and let `m` be the most
> significant bit of the operation (a.k.a. `m = w - 1`).
>
>   * `O` - Cleared
>   * `Z` - Set to `1` if the result consists of all `0` bits, cleared otherwise - `NOT(result[0]) AND NOT(result[1]) ... AND NOT(result[m])`
>   * `S` - Set to `1` if the signed interpretation of the result is negative, cleared otherwise - `result[m]`

#### Encoding

|  31-26   |    25-16     | 15-14 | 13  |  12-8   |  7-4   |  3-0   |
| -------- | ------------ | ----- | --- | ------- | ------ | ------ |
| `100001` | `0000000000` | `ss`  | `f` | `ddddd` | `aaaa` | `bbbb` |

### `and[.s] d:reg | null, a:reg, b:imm11[, S: imm3][, f:bool]`

"and"

This instruction is identical to
[`and` (register)][and_register], except that instead of
using a register for `b`, this instruction uses an immediate with an optional
shift `S`. The equivalent value for `b` is determined by shifting the given
immediate left by `S` multiples of 11 bits and zero extending as necessary to
fit the operation width. Note that if the shifted immediate is also truncated
as necessary to fit the operation width.

#### Encoding

|  31-26   | 25-24 | 23  |  22-18  | 17-14  |     13-3      |  2-0  |
| -------- | ----- | --- | ------- | ------ | ------------- | ----- |
| `100000` | `ss`  | `f` | `ddddd` | `aaaa` | `bbbbbbbbbbb` | `SSS` |

### `or[.s] d:reg | null, a:reg, b:reg[, f:bool]`

"or"

This instruction performs a bitwise OR on the values in registers `a` and `b`
and stores the result in register `d`.

If `b` is an immediate value, it is zero-extended if necessary to fit the bit
width of the operation.

Note that `d` may also be `null`; in this case, the result of the operation is
discarded. This is useful when combined with `f` equal to `1`/`true` as it
allows registers `a` and `b` to be compared for use in conditionals without
having to store the result anywhere.

If `f` is `0`/`false` (the default), then this instruction does *not* modify
the CPU flags register. If `f` is `1`/`true`, then this instruction *does*
modify the CPU flags register as follows:

> Let `w` be the bit width of the operation and let `m` be the most
> significant bit of the operation (a.k.a. `m = w - 1`).
>
>   * `O` - Cleared
>   * `Z` - Set to `1` if the result consists of all `0` bits, cleared otherwise - `NOT(result[0]) AND NOT(result[1]) ... AND NOT(result[m])`
>   * `S` - Set to `1` if the signed interpretation of the result is negative, cleared otherwise - `result[m]`

#### Encoding

|  31-26   |    25-16     | 15-14 | 13  |  12-8   |  7-4   |  3-0   |
| -------- | ------------ | ----- | --- | ------- | ------ | ------ |
| `011111` | `0000000000` | `ss`  | `f` | `ddddd` | `aaaa` | `bbbb` |

### `or[.s] d:reg | null, a:reg, b:imm11[, S: imm3][, f:bool]`

"or"

This instruction is identical to
[`or` (register)][or_register], except that instead of
using a register for `b`, this instruction uses an immediate with an optional
shift `S`. The equivalent value for `b` is determined by shifting the given
immediate left by `S` multiples of 11 bits and zero extending as necessary to
fit the operation width. Note that if the shifted immediate is also truncated
as necessary to fit the operation width.

#### Encoding

|  31-26   | 25-24 | 23  |  22-18  | 17-14  |     13-3      |  2-0  |
| -------- | ----- | --- | ------- | ------ | ------------- | ----- |
| `011110` | `ss`  | `f` | `ddddd` | `aaaa` | `bbbbbbbbbbb` | `SSS` |

### `xor[.s] d:reg | null, a:reg, b:reg[, f:bool]`

"exclusive or"

This instruction performs a bitwise XOR on the values in registers `a` and `b`
and stores the result in register `d`.

Note that this instruction can also be used to perform bitwise NOT if the same
register is provided for `a` and `b`.

Note that `d` may also be `null`; in this case, the result of the operation is
discarded. This is useful when combined with `f` equal to `1`/`true` as it
allows registers `a` and `b` to be compared for use in conditionals without
having to store the result anywhere.

If `f` is `0`/`false` (the default), then this instruction does *not* modify
the CPU flags register. If `f` is `1`/`true`, then this instruction *does*
modify the CPU flags register as follows:

> Let `w` be the bit width of the operation and let `m` be the most
> significant bit of the operation (a.k.a. `m = w - 1`).
>
>   * `O` - Cleared
>   * `Z` - Set to `1` if the result consists of all `0` bits, cleared otherwise - `NOT(result[0]) AND NOT(result[1]) ... AND NOT(result[m])`
>   * `S` - Set to `1` if the signed interpretation of the result is negative, cleared otherwise - `result[m]`

#### Encoding

|  31-26   |    25-16     | 15-14 | 13  |  12-8   |  7-4   |  3-0   |
| -------- | ------------ | ----- | --- | ------- | ------ | ------ |
| `011101` | `0000000000` | `ss`  | `f` | `ddddd` | `aaaa` | `bbbb` |

### `xor[.s] d:reg | null, a:reg, b:imm11[, S: imm3][, f:bool]`

"exclusive or"

This instruction is identical to
[`xor` (register)][xor_register], except that instead of
using a register for `b`, this instruction uses an immediate with an optional
shift `S`. The equivalent value for `b` is determined by shifting the given
immediate left by `S` multiples of 11 bits and zero extending as necessary to
fit the operation width. Note that if the shifted immediate is also truncated
as necessary to fit the operation width.

#### Encoding

|  31-26   | 25-24 | 23  |  22-18  | 17-14  |     13-3      |  2-0  |
| -------- | ----- | --- | ------- | ------ | ------------- | ----- |
| `011100` | `ss`  | `f` | `ddddd` | `aaaa` | `bbbbbbbbbbb` | `SSS` |

### `shl[.s] d:reg | null, a:reg, b:reg | imm7[, f: bool]`

"shift left"

This instruction shifts the value in register `a` left by a number of bits
specified in register or immediate `b` and stores the result in register `d`.

Note that if the value in register `b` is greater than or equal to the width of
the operation, the destination register is simply cleared.

Note that `d` may also be `null`; in this case, the result of the operation is
discarded. This is useful when combined with `f` equal to `1`/`true` as it
allows registers `a` and `b` to be compared for use in conditionals without
having to store the result anywhere.

If `f` is `0`/`false` (the default), then this instruction does *not* modify
the CPU flags register. If `f` is `1`/`true`, then this instruction *does*
modify the CPU flags register as follows:

> Let `w` be the bit width of the operation and let `m` be the most
> significant bit of the operation (a.k.a. `m = w - 1`).
>
>   * `C` - Set to the value of the last bit shifted out, unless `b` is `0`; if `b` is `0`, its previous value is preserved - `a[MAX(w - b, 0)]`
>   * `O` - Cleared
>   * `Z` - Set to `1` if the result consists of all `0` bits, cleared otherwise - `NOT(result[0]) AND NOT(result[1]) ... AND NOT(result[m])`
>   * `S` - Set to `1` if the signed interpretation of the result is negative, cleared otherwise - `result[m]`

#### Encoding

**If `b` is a register:**

|  31-26   |    25-16     | 15-14 | 13  |  12-8   |  7-4   |  3-0   |
| -------- | ------------ | ----- | --- | ------- | ------ | ------ |
| `011010` | `0000000000` | `ss`  | `f` | `ddddd` | `aaaa` | `bbbb` |

**If `b` is an immediate:**

|  31-26   |  25-19    | 18-17 | 16  |  15-11  |  10-7  |    6-0    |
| -------- | --------- | ----- | --- | ------- | ------ | --------- |
| `011011` | `0000000` | `ss`  | `f` | `ddddd` | `aaaa` | `bbbbbbb` |

### `shr[.s] d:reg | null, a:reg, b:reg | imm7[, A:bool][, f:bool]`

"shift right"

This instruction shifts the value in register `a` right by a number of bits
specified in register or immediate `b` and stores the result in register `d`.
If `A` is `0`/`false` (the default), the operation is a logical shift that
shifts `0` bits in; if `A` is `1`/`true`, the operation is an arithmetic shift
that shifts copies of the most-significant bit in from the source register.

Note that if the value in register `b` is greater than or equal to the width of
the operation, the destination register is either cleared (if `A` is `0`
`false`) or replaced with copies of the most-significant bit from the source
register (if `A` is `1`/`true`).

Note that `d` may also be `null`; in this case, the result of the operation is
discarded. This is useful when combined with `f` equal to `1`/`true` as it
allows registers `a` and `b` to be compared for use in conditionals without
having to store the result anywhere.

If `f` is `0`/`false` (the default), then this instruction does *not* modify
the CPU flags register. If `f` is `1`/`true`, then this instruction *does*
modify the CPU flags register as follows:

> Let `w` be the bit width of the operation and let `m` be the most
> significant bit of the operation (a.k.a. `m = w - 1`).
>
>   * `C` - Set to the value of the last bit shifted out, unless `b` is `0`; if `b` is `0`, its previous value is preserved - `a[MIN(b - 1, m)]`
>   * `O` - Cleared
>   * `Z` - Set to `1` if the result consists of all `0` bits, cleared otherwise - `NOT(result[0]) AND NOT(result[1]) ... AND NOT(result[m])`
>   * `S` - Set to `1` if the signed interpretation of the result is negative, cleared otherwise - `result[m]`

#### Encoding

**If `b` is a register:**

|  31-26   |    25-17    | 16-15 | 14  | 13  |  12-8   |  7-4   |  3-0   |
| -------- | ----------- | ----- | --- | --- | ------- | ------ | ------ |
| `011000` | `000000000` | `ss`  | `A` | `f` | `ddddd` | `aaaa` | `bbbb` |

**If `b` is an immediate:**

|  31-26   |  25-20   | 19-18 | 17  | 16  |  15-11  |  10-7  |    6-0    |
| -------- | -------- | ----- | --- | --- | ------- | ------ | --------- |
| `011001` | `000000` | `ss`  | `A` | `f` | `ddddd` | `aaaa` | `bbbbbbb` |

### `rot[.s] d:reg | null, a:reg, b:reg | imm7[, f: bool]`

"rotate (right)"

This instruction rotates the value in register `a` right by a number of bits
specified in register `b` and stores the result in register `d`.

Note that `d` may also be `null`; in this case, the result of the operation is
discarded. This is useful when combined with `f` equal to `1`/`true` as it
allows registers `a` and `b` to be compared for use in conditionals without
having to store the result anywhere.

If `f` is `0`/`false` (the default), then this instruction does *not* modify
the CPU flags register. If `f` is `1`/`true`, then this instruction *does*
modify the CPU flags register as follows:

> Let `w` be the bit width of the operation and let `m` be the most
> significant bit of the operation (a.k.a. `m = w - 1`).
>
>   * `S` - Set to `1` if the signed interpretation of the result is negative, cleared otherwise - `result[m]`

#### Encoding

**If `b` is a register:**

|  31-26   |    25-16     | 15-14 | 13  |  12-8   |  7-4   |  3-0   |
| -------- | ------------ | ----- | --- | ------- | ------ | ------ |
| `010110` | `0000000000` | `ss`  | `f` | `ddddd` | `aaaa` | `bbbb` |

**If `b` is an immediate:**

|  31-26   |   25-19   | 18-17 | 16  |  15-11  |  10-7  |    6-0    |
| -------- | --------- | ----- | --- | ------- | ------ | --------- |
| `010111` | `0000000` | `ss`  | `f` | `ddddd` | `aaaa` | `bbbbbbb` |

### `neg[.s] d:reg, a:reg[, f: bool]`

"negate"

This instruction takes the value from register `a`, negates it (according to
its two's complement representation), and stores the result into register `d`.

If `f` is `0`/`false` (the default), then this instruction does *not* modify
the CPU flags register. If `f` is `1`/`true`, then this instruction *does*
modify the CPU flags register as follows:

> Let `w` be the bit width of the operation and let `m` be the most
> significant bit of the operation (a.k.a. `m = w - 1`).
>
>   * `Z` - Set to `1` if the result consists of all `0` bits, cleared otherwise - `NOT(result[0]) AND NOT(result[1]) ... AND NOT(result[m])`
>   * `S` - Set to `1` if the signed interpretation of the result is negative, cleared otherwise - `result[m]`

#### Encoding

|  31-26   |       25-11       | 10-9 |  8  |  7-4   |  3-0   |
| -------- | ----------------- | ---- | --- | ------ | ------ |
| `010101` | `000000000000000` | `ss` | `f` | `dddd` | `aaaa` |

### `bswap[.s] d:reg, a:reg[, f: bool]`

"byte-swap"

This instruction swaps the order of the bytes in register `a` and stores the
result into register `d`.

For example, if `r0d` contains the value `0xabcd`, then `bswap r1d, r0d`
would put the value `0xcdab` into `r1d`.

If `f` is `0`/`false` (the default), then this instruction does *not* modify
the CPU flags register. If `f` is `1`/`true`, then this instruction *does*
modify the CPU flags register as follows:

> Let `w` be the bit width of the operation and let `m` be the most
> significant bit of the operation (a.k.a. `m = w - 1`).
>
>   * `S` - Set to `1` if the signed interpretation of the result is negative, cleared otherwise - `result[m]`

#### Encoding

|  31-26   |       25-11       | 10-9 |  8  |  7-4   |  3-0   |
| -------- | ----------------- | ---- | --- | ------ | ------ |
| `010100` | `000000000000000` | `ss` | `f` | `dddd` | `aaaa` |

## Conditionals and Control Flow

### Absolute and Relative Addresses

An absolute address is a 64-bit byte address indicating a location in
memory. Such addresses can only be specified in registers; they are too wide
to specify in an immediate. For example, the absolute address `0xabcd`
corresponds to the memory address `0xabcd`.

A relative address is an address relative to the instruction pointer, specified
in multiples of the instruction width (i.e. multiples of 4 bytes). For example,
the relative address `0xabcd` corresponds to the memory address `IP + (0xabcd * 4)` or `IP + 0x02AF34`, where `IP` is the instruction pointer.

When calculating a relative address, the instruction pointer is first
incremented past the jump instruction and then the relative address is added to
the instruction pointer. For example, for `jmpr -1`, the instruction pointer is
first increment to the next instruction, then `-1` is added to it, moving it
back to the `jmpr -1` instruction.

### Conditions

A suffix of `.c` indicates that an instruction requires a condition; a suffix of
`[.c]` indicates that it can optionally take a condition.

Valid conditions are one of the following:

  * `c` or `nc` for carry bit (`C`) set or unset (respectively)
  * `z` or `nz` for zero bit (`Z`) set or unset (respectively)
  * `o` or `no` for overflow bit (`O`) set or unset (respectively)
  * `s` or `ns` for sign bit (`S`) set or unset (respectively)

### `jmpa[.c] a:reg`

"jump to absolute register"

This instruction jumps to the absolute address in register `a`.

#### Encoding

|  31-26   |         25-8         |  7-4  |  3-0   |
| -------- | -------------------- | ------ | ------ |
| `010011` | `000000000000000000` | `cccc` | `aaaa` |

### `jmpr[.c] a:reg`

"jump to relative register"

This instruction jumps to the relative address in register `a`.

#### Encoding

|  31-26   |         25-8         |  7-4   |  3-0   |
| -------- | -------------------- | ------ | ------ |
| `010010` | `000000000000000000` | `cccc` | `aaaa` |

### `jmpr[.c] a:imm22`

"jump to relative immediate"

This instruction jumps to the relative address given in the immediate. This
immediate is limited to 22 bits, meaning it can jump +2,097,151 or -2,097,152
instructions relative to the instruction pointer.

#### Encoding

|  31-26   | 25-22  |           21-0           |
| -------- | ------ | ------------------------ |
| `010001` | `cccc` | `aaaaaaaaaaaaaaaaaaaaaa` |

### `cjmpa.c[.s] a:reg, b:reg, C:reg`

"compare and jump to absolute register"

This instruction compares the values in register `b` and register or immediate
`C` by performing the equivalent of a
[`sub` (register)][sub_register] instruction with `d` set to
`null` and `f` set to `true`; however, unlike such a `sub` instruction, it does
*not* modify the CPU flags. Instead, the flags generated by the operation are
used to determine whether to jump or not and immediately discarded.

If the condition evaluates to true, a jump is performed to register `a` like the
[`jmpa`][jmpa] instruction does.

#### Encoding

|  31-26   |    25-17    | 16-14 | 13-12 |  11-8  |  7-4   |  3-0   |
| -------- | ----------- | ----- | ----- | ------ | ------ | ------ |
| `001111` | `000000000` | `ccc` | `ss`  | `aaaa` | `bbbb` | `CCCC` |

### `cjmpr.c[.s] a:reg, b:reg, C:reg`

"compare and jump to relative register"

This instruction compares the values in register `b` and register or immediate
`C` by performing the equivalent of a
[`sub` (register)][sub_register] instruction with `d` set to
`null` and `f` set to `true`; however, unlike such a `sub` instruction, it does
*not* modify the CPU flags. Instead, the flags generated by the operation are
used to determine whether to jump or not and immediately discarded.

If the condition evaluates to true, a jump is performed to register `a` like the
[`jmpr` (register)][jmpr_register] instruction does.

#### Encoding

|  31-26   |    25-17    | 16-14 | 13-12 |  11-8  |  7-4   |  3-0   |
| -------- | ----------- | ----- | ----- | ------ | ------ | ------ |
| `001101` | `000000000` | `ccc` | `ss`  | `aaaa` | `bbbb` | `CCCC` |

### `cjmpr.c[.s] a:imm13, b:reg, C:reg`

"compare and jump to relative immediate"

This instruction compares the values in register `b` and register or immediate
`C` by performing the equivalent of a
[`sub` (register)][sub_register] instruction with `d` set to
`null` and `f` set to `true`; however, unlike such a `sub` instruction, it does
*not* modify the CPU flags. Instead, the flags generated by the operation are
used to determine whether to jump or not and immediately discarded.

If the condition evaluates to true, a jump is performed to the immediate `a`
like the [`jmpr` (immediate)][jmpr_immediate] instruction does. However, unlike
that instruction, the immediate is limited to 13 bits, meaning the maximum jump
range is +4095 or -4096 instructions.

#### Encoding

|  31-26   | 25-23 | 22-21 | 20-17  | 16-13  |      12-0       |
| -------- | ----- | ----- | ------ | ------ | --------------- |
| `001011` | `ccc` | `ss`  | `bbbb` | `CCCC` | `aaaaaaaaaaaaa` |

### `calla[.c] a:reg`

"call absolute register"

This instruction saves the absolute address of the next instruction to the link
register (`r15` or `rlr`) and then performs a jump like
[`jmpa`][jmpa].

#### Encoding

|  31-26   |         25-8         |  7-4   |  3-0   |
| -------- | -------------------- | ------ | ------ |
| `001010` | `000000000000000000` | `cccc` | `aaaa` |

### `callr[.c] a:reg`

"call relative register"

This instruction saves the absolute address of the next instruction to the link
register (`r15` or `rlr`) and then performs a jump like
[`jmpr` (register)][jmpr_register].

#### Encoding

|  31-26   |         25-8         |  7-4   |  3-0   |
| -------- | -------------------- | ------ | ------ |
| `001001` | `000000000000000000` | `cccc` | `aaaa` |

### `callr[.c] a:imm22`

"call relative immediate"

This instruction saves the absolute address of the next instruction to the link
register (`r15` or `rlr`) and then performs a jump like
[`jmpr` (immediate)][jmpr_immediate].

#### Encoding

|  31-26   | 25-22  |           21-0           |
| -------- | ------ | ------------------------ |
| `001000` | `cccc` | `aaaaaaaaaaaaaaaaaaaaaa` |

### `ret`

"return"

This instruction jumps to the absolute address in the link register (`r15` or
`rlr`).

#### Encoding

|  31-26   |             25-0             |
| -------- | ---------------------------- |
| `000111` | `00000000000000000000000000` |

### `eret`

"exception return"

This instruction loads (in no particular order):

  * The value of the `elr` machine register into the instruction pointer.
  * The value of the `eflags` machine register into the `flags` machine
    register.
  * The value of the `esp` machine register into the stack pointer register
    (`r13` or `rsp`).

#### Encoding

|  31-26   |             25-0             |
| -------- | ---------------------------- |
| `000110` | `00000000000000000000000000` |

### `udf`

"undefined"

This instruction causes an invalid instruction exception to be raised. The
instruction pointer is *not* incremented to the next instruction before the
exception is taken.

The main reason for the existence of this instruction is to have a permanent
encoding that is guaranteed to be an invalid instruction; other encodings that
are currently unused may be defined in a later update to the Acca architecture.
However, this encoding is guaranteed to remain invalid forever.

#### Encoding

|  31-26   |             25-0             |
| -------- | ---------------------------- |
| `000000` | `00000000000000000000000000` |

### `dbg`

"debug"

This instruction causes a debug exception to be raised. The instruction pointer
is *not* incremented to the next instruction before the exception is taken.

#### Encoding

|  31-26   |             25-0             |
| -------- | ---------------------------- |
| `000010` | `00000000000000000000000000` |


### `exc a:imm16`

This instruction causes a user exception to be raised with the given value `a`.
The instruction pointer *is* incremented to the next instruction before the
exception is taken.

#### Encoding

|  31-26   |    25-16     |       15-0        |
| -------- | ------------ | ----------------- |
| `000011` | `0000000000` |`aaaaaaaaaaaaaaaa` |

## Miscellaneous

### `nop`

"no operation"

This instruction simply does nothing.

#### Encoding

|  31-26   |             25-0             |
| -------- | ---------------------------- |
| `000001` | `00000000000000000000000000` |

### `ldm d:reg, a:imm22`

"load machine register"

This instruction loads the value of the machine register identified by `a` into
register `d`.

#### Encoding

|  31-26   | 25-22  |           21-0           |
| -------- | ------ | ------------------------ |
| `000100` | `dddd` | `aaaaaaaaaaaaaaaaaaaaaa` |

### `stm d:imm22, a:reg`

"store machine register"

This instruction stores the value of register `d` into the machine register
identified by `a`.

#### Encoding

|  31-26   | 25-22  |           21-0           |
| -------- | ------ | ------------------------ |
| `000101` | `aaaa` | `dddddddddddddddddddddd` |

[add_register]: #adds-dreg--null-areg-breg-cbool-fbool
[sub_register]: #subs-dreg--null-areg-breg-bbool-fbool
[and_register]: #ands-dreg--null-areg-breg-fbool
[or_register]: #ors-dreg--null-areg-breg-fbool
[xor_register]: #xors-dreg--null-areg-breg-fbool
[jmpa]: #jmpac-areg
[jmpr_register]: #jmprc-areg
[jmpr_immediate]: #jmprc-aimm23
[registers]: #registers
