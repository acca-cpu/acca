# Developer Notes on the Specification

## Instructions

### Current List of Unused 6-bit Instruction IDs

The following are the unused 6-bit instruction IDs that can be assigned to
future instructions:

  * `001110`
  * `010000`
  * `111000`
  * `111001`
  * `111010`
  * `111011`
  * `111100`
  * `111101`
  * `111110`
  * `111111`

This list must be updated as instructions are added and removed from the
specification.

Note that these are the *6-bit* instruction IDs. More instructions can be added
by using unused bits from existing instructions. For example, the `nop`
instruction only consists of the instruction ID; the other 26 bits are unused
and must be zero for valid `nop` instructions. An instruction could be added in
the future by taking bit 25 as another instruction ID bit; the instruction
would then have 25 bits available for its purposes.
