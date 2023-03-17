# Exceptions

The Acca architecture has the following 8 types of exceptions:
  0. Unknown
  1. Invalid instruction
  2. Debug
  3. User
  4. Invalid operation
  5. Instruction load error
  6. Data load error
  7. Interrupt

## Exception Behavior

When an exception needs to be taken, the processor first suspends execution at
the next available [exception point][exception_point]. Then, it stores the
current [CPU flags][cpu_flags] into the [`eflags`][eflags] machine register.
The processor then stores the current instruction pointer into the [`elr`][elr]
machine register. Then, it masks maskable exceptions (by clearing the `E` bit
in the CPU flags) and switches to PL0 (by clearing the PL bit in the CPU
flags). The processor then stores the current stack pointer (`r13` or `rsp`)
into the [`esp`][esp] machine register.

The processor then updates the [`einfo`][einfo] and [`eaddr`][eaddr] machine
registers as described in [Exception Information][exception_info]. Next, the
processor uses the appropriate entry in the [Exception Configuration Table][ect]
to determine whether to switch the stack pointer (and what value to switch it
to). Finally, the processor jumps to the appropriate entry in the
[Exception Vector Table][evt] and resumes execution.

## Exception Points

Exception points are points of execution where it is safe to take an exception
without corrupting processor state. Whenever an exception occurs while executing
an instruction as a result of that instruction (making it impossible to finish
execution of the instruction), any state modified by said instruction is rolled
back to its state before the execution of that instruction and then the
exception is taken. When an exception occurs unrelated to the
currently-executing instruction (e.g. an interrupt), the instruction is fully
executed before taking the exception. In this way, exceptions can be said to be
atomic with regard to processor state.

## Maskable Exceptions

Some types of exceptions can be masked (meaning they can be prevented from
being taken). The only type of maskable exceptions are interrupts. Maskable
exceptions can be masked by clearing the `E` bit in the [CPU flags][cpu_flags]
and they can be unmasked by setting the `E` bit.

When an exception is masked, it will be held pending until exceptions are
unmasked. It will not be taken as long as it is masked. When exceptions are
unmasked (by setting the `E` bit), any pending (masked) exceptions will be
taken at the next [exception point][exception_point].

## Exception Vector Table

The exception vector table consists of a 512-byte area with 16 entries. The
first 8 entries are for exceptions taken while running in PL0, while the second
8 entries are for exceptions taken while running in PL1. Each entry has room
for 8 instructions (32 bytes). Each entry is ordered according to their
exception type: the 0th entry (the lowest in memory) is for unknown exceptions,
the 1st entry is for invalid instruction exceptions, the 2nd entry is for debug
exceptions, and so on.

When an exception occurs, the processor sets the appropriate machine registers
and then jumps to the corresponding entry in the exception vector table.

## Exception Configuration Table

The exception configuration table consists of a 384-byte table with 16 entries.
Like the exception vector table, the first 8 entries configure exceptions when
taken from PL0, while the second 8 entries configure exceptions when taken
from PL1. Each entry is of the following form:

```rust
struct ConfigurationEntry {
  flags: u64,
  stack_pointer: u64,
  stack_size: u64,
}
```

The `flags` field consists of the following format:

|  63-1  |  0  |
| ------ | --- |
| *RES0* | `S` |

  * `S` - Use `stack_pointer`
    * If `1`, the processor will load the value of the `stack_pointer` field in
      the entry into the stack pointer register (`r13` or `rsp`) when taking an
      exception of this type. Otherwise, if `0`, the processor will preserve
      the value of the stack pointer register when taking an exception of this
      type.
    * If the processor is currently on the given stack (as determined by
      whether the stack pointer is within `stack_pointer` and
      `stack_pointer + stack_size`), the stack pointer is preserved.
    * Note that the processor will always save the value of the stack pointer
      to the `esp` machine register before taking an exception. This bit merely
      affects whether or not `stack_pointer` is loaded into the stack pointer
      afterwards.

## Exception Information

When an exception is taken, the [`einfo`][einfo] machine register
is updated by the processor with information about the exception.

This register consists of the following format:

| 63-3 | 2-0 |
| ---- | --- |
| `TS` | `T` |

  * `T` - Exception type
    * The value of this field corresponds to one of the 8 exception types (as
      described [here][exceptions]).
  * `TS` - Type-specific
    * The format of this field depends on the type of exception taken.

### Unknown

|  63-3  |  2-0  |
| ------ | ----- |
| *RES0* | `000` |

### Invalid instruction

|  63-3  |  2-0  |
| ------ | ----- |
| *RES0* | `001` |

### Debug

|  63-3  |  2-0  |
| ------ | ----- |
| *RES0* | `010` |

### User

|  63-19 | 18-3 |  2-0  |
| ------ | ---- | ----- |
| *RES0* | `U`  | `011` |

  * `U` - User value
    * The value of this field is set to the value from the
      [`exc`][exc] instruction that caused
      this user exception.

### Invalid operation

|  63-3  |  2-0  |
| ------ | ----- |
| *RES0* | `100` |

### Instruction load error

|  63-3  |  2-0  |
| ------ | ----- |
| *RES0* | `101` |

### Data load error

|  63-4  |  3   |  2-0  |
| ------ | ---- | ----- |
| *RES0* | `RW` | `110` |

  * `RW` - Read or write
    * If `1`, this indicates that the load error occurred during a write;
      otherwise, if `0`, this indicates that the load error occurred during a
      read.

When this type of exception is taken, the address that caused the error will be
loaded into the [`eaddr`][eaddr] machine register.

### Interrupt

| 63-3  |  2-0  |
| ----- | ----- |
| `ICS` | `111` |

  * `ICS` - Interrupt-controller-specific
    * The value of this field depends on the interrupt controller in use.
      Typically, this will contain information about which device triggered
      the interrupt.

[einfo]: ./registers.md#einfo
[exceptions]: #exceptions
[exc]: ./instructions.md#exc-aimm16
[eaddr]: ./registers.md#eaddr
[cpu_flags]: ./registers.md#cpu-flags
[eflags]: ./registers.md#eflags
[esp]: ./registers.md#esp
[exception_point]: #exception-points
[exception_info]: #exception-information
[ect]: #exception-configuration-table
[evt]: #exception-vector-table
[elr]: ./registers.md#elr
