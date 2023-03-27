//
// Copyright (C) 2023 Ariel Abreu
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

use std::{fs::File, io::Read, ops::Range};

use acca_emu_proc_macro::instructions;
use memmap2::MmapMut;

use super::util::*;

use bitflags::bitflags;

bitflags! {
	#[derive(Debug, Clone, Copy, PartialEq, Eq)]
	struct ExceptionConfigurationFlags: u64 {
		const USE_STACK = 1;
	}
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ExceptionConfigurationEntry {
	flags: ExceptionConfigurationFlags,
	stack_pointer: VMAddress,
	stack_size: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ExceptionConfigurationTable {
	pl0: [ExceptionConfigurationEntry; 8],
	pl1: [ExceptionConfigurationEntry; 8],
}

#[derive(Debug)]
pub(crate) struct VM {
	print_instructions: bool,

	memory: MmapMut,
	register_file: RegisterFile,
	flags: CPUFlags,
	instruction_pointer: VMAddress,

	elr: VMAddress,
	esp: VMAddress,
	eflags: CPUFlags,
	einfo: u64,
	eaddr: VMAddress,
	evtable_addr: VMAddress,
	ectable_addr: VMAddress,

	ectable: ExceptionConfigurationTable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Exception {
	Unknown = 0,
	InvalidInstruction = 1,
	Debug = 2,
	User(u16) = 3,
	InvalidOperation = 4,
	InstructionLoadError = 5,
	DataLoadError {
		address: VMAddress,
		write: bool,
		byte_size: u16,
	} = 6,
	Interrupt(u64) = 7,
}

impl Exception {
	pub fn id(&self) -> u8 {
		// SAFETY: this enum is a `repr(u8)` enum, meaning the first member
		//         of all its variants is a `u8`. therefore, we can safely
		//         cast the reference to a `u8` pointer and read from it.
		unsafe { *((self as *const _) as *const u8) }
	}
}

impl ExceptionConfigurationEntry {
	pub fn validate(&self) -> bool {
		ExceptionConfigurationFlags::from_bits(self.flags.bits()).is_some()
	}
}

impl Default for ExceptionConfigurationEntry {
	fn default() -> Self {
		Self {
			flags: ExceptionConfigurationFlags::empty(),
			stack_pointer: VMAddress::from(0),
			stack_size: 0,
		}
	}
}

impl VM {
	pub fn new(memory_size: usize) -> Option<Self> {
		Some(Self {
			print_instructions: false,

			memory: MmapMut::map_anon(memory_size).ok()?,
			register_file: RegisterFile::new(),
			flags: CPUFlags::new(),
			instruction_pointer: 0x0400.into(),

			elr: VMAddress::new(0),
			esp: VMAddress::new(0),
			eflags: CPUFlags::new(),
			einfo: 0,
			eaddr: VMAddress::new(0),
			evtable_addr: VMAddress::new(0),
			ectable_addr: VMAddress::new(0),

			ectable: Default::default(),
		})
	}

	pub fn set_print_instructions(&mut self, print_instructions: bool) {
		self.print_instructions = print_instructions;
	}

	pub fn load_file(
		&mut self,
		file: &mut File,
		dest_addr: VMAddress,
	) -> Result<(), std::io::Error> {
		let metadata = file.metadata()?;
		let file_len = metadata.len();
		let dest = self
			.memory
			.get_mut(u64::from(dest_addr) as usize..u64::from(dest_addr + file_len) as usize)
			.unwrap();
		file.read_exact(dest)
	}

	fn get_memory(&self, range: Range<VMAddress>) -> Option<&[u8]> {
		let usize_range = u64::from(range.start) as usize..u64::from(range.end) as usize;
		self.memory.get(usize_range)
	}

	fn get_memory_mut(&mut self, range: Range<VMAddress>) -> Option<&mut [u8]> {
		let usize_range = u64::from(range.start) as usize..u64::from(range.end) as usize;
		self.memory.get_mut(usize_range)
	}

	fn execute_one(&mut self) {
		const ALL_BITS: u64 = !0u64;

		#[rustfmt::skip]
		let encoded = match self.get_memory(self.instruction_pointer..self.instruction_pointer + 4) {
			Some(bytes) => u32::from_le_bytes(bytes.try_into().unwrap()),
			None => {
				return self
					.take_exception(Exception::InstructionLoadError)
			},
		};

		macro_rules! get_memory {
			($addr:expr, $size:expr) => {
				match self.get_memory($addr..$addr + $size) {
					Some(bytes) => bytes,
					None => {
						return self.take_exception(Exception::DataLoadError {
							address: $addr.into(),
							write: false,
							byte_size: $size as u16,
						})
					},
				}
			};
		}

		macro_rules! get_memory_mut {
			($addr:expr, $size:expr) => {
				match self.get_memory_mut($addr..$addr + $size) {
					Some(bytes) => bytes,
					None => {
						return self.take_exception(Exception::DataLoadError {
							address: $addr.into(),
							write: true,
							byte_size: $size as u16,
						})
					},
				}
			};
		}

		fn imm11_with_shift_factor(mut imm: u64, shift_factor: u64, sign_extend: bool) -> u64 {
			imm <<= shift_factor * 11;
			let msb = (shift_factor * 11) + 10;
			if sign_extend && msb < 63 && imm.bit_as_bool(msb) {
				imm |= ALL_BITS << ((shift_factor * 11) + 11);
			}
			imm
		}

		macro_rules! do_jump {
			($addr:expr) => {
				let dest = $addr;
				if (u64::from(dest) & 3) != 0 {
					return self.take_exception(Exception::InvalidOperation);
				}
				self.instruction_pointer = dest - 4;
			};
		}

		macro_rules! jump {
			($addr:expr, $cond:expr) => {
				let should_jump = $cond
					.map(|cond| self.flags.test_condition(cond))
					.unwrap_or(true);

				if should_jump {
					do_jump!($addr);
				}
			};
		}

		macro_rules! compare_and_jump {
			($size:expr, $lhs:expr, $rhs:expr => $addr:expr, $cond:expr) => {
				let size = $size;
				let lhs = self.register_file[$lhs].get_signed(size) as u64;
				let rhs = self.register_file[$rhs].get_signed(size) as u64;

				let result = lhs.wrapping_sub(rhs) as u64;
				let msb = size.msb_index() as u64;

				let lhs_msb = lhs.bit_as_bool(msb);
				let rhs_msb = rhs.bit_as_bool(msb);
				let res_msb = result.bit_as_bool(msb);

				let carry = (!lhs_msb && rhs_msb) || (res_msb && rhs_msb) || (res_msb && !lhs_msb);
				let zero = (result & size.mask()) == 0;
				let overflow =
					(lhs_msb && !rhs_msb && !res_msb) || (!lhs_msb && rhs_msb && res_msb);
				let sign = res_msb;

				let should_jump = match $cond {
					Condition::C => carry,
					Condition::NC => !carry,
					Condition::Z => zero,
					Condition::NZ => !zero,
					Condition::O => overflow,
					Condition::NO => !overflow,
					Condition::S => sign,
					Condition::NS => !sign,
				};

				if should_jump {
					do_jump!($addr);
				}
			};
		}

		macro_rules! call {
			($addr:expr, $cond:expr) => {
				let should_call = $cond
					.map(|cond| self.flags.test_condition(cond))
					.unwrap_or(true);

				if should_call {
					let link_addr = self.instruction_pointer + 4;
					do_jump!($addr);
					self.register_file[RegisterID::LR] = link_addr.into();
				}
			};
		}

		instructions! {
			//
			// memory
			//

			[1101110000000000000000000ssaaaaa] => pushs size = s: size, src = a: reg | null {
				let byte_size = size.byte_size() as u64;
				let val = src.map(|id| self.register_file[id].get_unsigned(size)).unwrap_or(0);

				let new_rsp_val = self.register_file[RegisterID::SP].get_address() - byte_size;
				let mem = get_memory_mut!(new_rsp_val, byte_size);

				size.write(val, mem);

				self.register_file[RegisterID::SP] = new_rsp_val.into();
			},
			[11011000000000000000ssaaaaabbbbb] => pushp size = s: size, src1 = a: reg | null, src2 = b: reg | null {
				let byte_size = size.byte_size() as u64;
				let val1 = src1.map(|id| self.register_file[id].get_unsigned(size)).unwrap_or(0);
				let val2 = src2.map(|id| self.register_file[id].get_unsigned(size)).unwrap_or(0);

				let new_rsp_val = self.register_file[RegisterID::SP].get_address() - 2 * byte_size;
				let mem = get_memory_mut!(new_rsp_val, 2 * byte_size);
				let (mem1, mem2) = mem.split_at_mut(byte_size as usize);

				size.write(val1, mem1);
				size.write(val2, mem2);

				self.register_file[RegisterID::SP] = new_rsp_val.into();
			},
			[1101010000000000000000000ssaaaaa] => pops size = s: size, dst = a: reg | null {
				let byte_size = size.byte_size() as u64;

				let old_rsp_val = self.register_file[RegisterID::SP].get_address();
				let mem = get_memory!(old_rsp_val, byte_size);

				let val = size.read(mem, false);
				dst.map(|id| self.register_file[id].set(size, val));

				self.register_file[RegisterID::SP] = (old_rsp_val + byte_size).into();
			},
			[1101000000000000000000ssaaaabbbb] => popp size = s: size, dst1 = a: reg | null, dst2 = b: reg | null {
				let byte_size = size.byte_size() as u64;

				let old_rsp_val = self.register_file[RegisterID::SP].get_address();
				let mem = get_memory!(old_rsp_val, 2 * byte_size);
				let (mem1, mem2) = mem.split_at(byte_size as usize);

				let (val1, val2) = (size.read(mem1, false), size.read(mem2, false));
				dst1.map(|id| self.register_file[id].set(size, val1));
				dst2.map(|id| self.register_file[id].set(size, val2));

				self.register_file[RegisterID::SP] = (old_rsp_val + 2 * byte_size).into();
			},
			[1100110000000000000000ssddddaaaa] => lds size = s: size, dst = d: reg, src_addr = a: reg {
				let addr = self.register_file[src_addr].get_address();
				let mem = get_memory!(addr, size.byte_size() as u64);
				let val = size.read(mem, false);

				self.register_file[dst].set(size, val);
			},
			[110010000000000000ssddddeeeeaaaa] => ldp size = s: size, dst1 = d: reg, dst2 = e: reg, src_addr = a: reg {
				let addr = self.register_file[src_addr].get_address();
				let mem = get_memory!(addr, 2 * size.byte_size() as u64);
				let (mem1, mem2) = mem.split_at(size.byte_size() as usize);
				let (val1, val2) = (size.read(mem1, false), size.read(mem2, false));

				self.register_file[dst1].set(size, val1);
				self.register_file[dst2].set(size, val2);
			},
			[1100010000000000000000ssaaaabbbb] => sts size = s: size, dst_addr = a: reg, src = b: reg {
				let addr = self.register_file[dst_addr].get_address();
				let val = self.register_file[src].get();
				let mem = get_memory_mut!(addr, size.byte_size() as u64);

				size.write(val, mem);
			},
			[110000000000000000ssaaaabbbbcccc] => stp size = s: size, dst_addr = a: reg, src1 = b: reg, src2 = c: reg {
				let addr = self.register_file[dst_addr].get_address();
				let (val1, val2) = (self.register_file[src1].get(), self.register_file[src2].get());
				let mem = get_memory_mut!(addr, size.byte_size() as u64);
				let (mem1, mem2) = mem.split_at_mut(size.byte_size() as usize);

				size.write(val1, mem1);
				size.write(val2, mem2);
			},
			[1110ccaaaaaaaaaaaaaaaabbbbbbdddd] => ldi dst = d: reg, src = a: imm16, shift = b: imm6, clear = c: imm2 {
				let old = self.register_file[dst].get();
				let mask = 0xffffu64 << shift;
				let shifted = src << shift;
				let masked = old & !mask;
				let cleared = masked & match clear {
					// clear nothing
					0 => ALL_BITS,
					// clear lower
					1 => ALL_BITS.checked_shl(shift as u32).unwrap_or(0),
					// clear upper
					2 => !(ALL_BITS.checked_shl(shift as u32 + 16).unwrap_or(0)),
					// clear everything
					3 => 0,
					_ => unreachable!(),
				};

				self.register_file[dst] = (cleared | shifted).into();
			},
			[001100ddddaaaaaaaaaaaaaaaaaaaaaa] => ldr dst = d: reg, src = a: rel22 {
				let rel_base = self.instruction_pointer + 4;
				let result = rel_base + src;
				self.register_file[dst] = result.into();
			},
			[1010100000000000000000ssddddSSSS] => copy size = s: size, dst = d: reg, src = S: reg {
				let val = self.register_file[src].get_unsigned(size);
				self.register_file[dst].set(size, val);
			},

			//
			// arithmetic and logic
			//

			[101001000000000sscfdddddaaaabbbb] => add size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: reg, carry = c: bool, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_signed(size) as u64;
				let rhs = self.register_file[rhs].get_signed(size) as u64;
				let carry: u64 = if carry && self.flags.carry() { 1 } else { 0 };

				let result = lhs.wrapping_add(rhs).wrapping_add(carry) as u64;
				let msb = size.msb_index() as u64;

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					let lhs_msb = lhs.bit_as_bool(msb);
					let rhs_msb = rhs.bit_as_bool(msb);
					let res_msb = result.bit_as_bool(msb);
					self.flags.set_carry((lhs_msb && rhs_msb) || ((lhs_msb || rhs_msb) && !res_msb));
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_overflow((lhs_msb && rhs_msb && !res_msb) || (!lhs_msb && !rhs_msb && res_msb));
					self.flags.set_sign(res_msb);
				}
			},
			[1011sscfdddddaaaaASSSbbbbbbbbbbb] => add size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: imm11, shift_factor = S: imm3, sign_extend = A: bool, carry = c: bool, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_signed(size) as u64;
				let rhs = imm11_with_shift_factor(rhs, shift_factor, sign_extend);
				let carry: u64 = if carry && self.flags.carry() { 1 } else { 0 };

				let result = lhs.wrapping_add(rhs).wrapping_add(carry) as u64;
				let msb = size.msb_index() as u64;

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					let lhs_msb = lhs.bit_as_bool(msb);
					let rhs_msb = rhs.bit_as_bool(msb);
					let res_msb = result.bit_as_bool(msb);
					self.flags.set_carry((lhs_msb && rhs_msb) || ((lhs_msb || rhs_msb) && !res_msb));
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_overflow((lhs_msb && rhs_msb && !res_msb) || (!lhs_msb && !rhs_msb && res_msb));
					self.flags.set_sign(res_msb);
				}
			},
			[101000000000000ssBfdddddaaaabbbb] => sub size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: reg, borrow = B: bool, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_signed(size) as u64;
				let rhs = self.register_file[rhs].get_signed(size) as u64;
				let borrow: u64 = if borrow && self.flags.carry() { 1 } else { 0 };

				let result = lhs.wrapping_sub(rhs).wrapping_sub(borrow) as u64;
				let msb = size.msb_index() as u64;

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					let lhs_msb = lhs.bit_as_bool(msb);
					let rhs_msb = rhs.bit_as_bool(msb);
					let res_msb = result.bit_as_bool(msb);
					self.flags.set_carry((!lhs_msb && rhs_msb) || (res_msb && rhs_msb) || (res_msb && !lhs_msb));
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_overflow((lhs_msb && !rhs_msb && !res_msb) || (!lhs_msb && rhs_msb && res_msb));
					self.flags.set_sign(res_msb);
				}
			},
			[1001ssBfdddddaaaaASSSbbbbbbbbbbb] => sub size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: imm11, shift_factor = S: imm3, sign_extend = A: bool, borrow = B: bool, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_signed(size) as u64;
				let rhs = imm11_with_shift_factor(rhs, shift_factor, sign_extend);
				let borrow: u64 = if borrow && self.flags.carry() { 1 } else { 0 };

				let result = lhs.wrapping_sub(rhs).wrapping_sub(borrow) as u64;
				let msb = size.msb_index() as u64;

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					let lhs_msb = lhs.bit_as_bool(msb);
					let rhs_msb = rhs.bit_as_bool(msb);
					let res_msb = result.bit_as_bool(msb);
					self.flags.set_carry((!lhs_msb && rhs_msb) || (res_msb && rhs_msb) || (res_msb && !lhs_msb));
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_overflow((lhs_msb && !rhs_msb && !res_msb) || (!lhs_msb && rhs_msb && res_msb));
					self.flags.set_sign(res_msb);
				}
			},
			[10001100000000ssttSfddddaaaabbbb] => mul src_size = s: size, dst_size = t: size, dst = d: reg, lhs = a: reg, rhs = b: reg, signed = S: bool, set_flags = f: bool {
				let result = if signed {
					let lhs = self.register_file[lhs].get_signed(src_size);
					let rhs = self.register_file[rhs].get_signed(src_size);
					(lhs * rhs) as u64
				} else {
					let lhs = self.register_file[lhs].get_unsigned(src_size);
					let rhs = self.register_file[rhs].get_unsigned(src_size);
					lhs * rhs
				};

				self.register_file[dst].set(dst_size, result);

				if set_flags {
					self.flags.set_zero((result & dst_size.mask()) == 0);
					self.flags.set_sign(result.bit_as_bool(dst_size.msb_index() as u64));
				}
			},
			[100010000000ssSfddddrrrraaaabbbb] => div size = s: size, quot = d: reg, rem = r: reg, lhs = a: reg, rhs = b: reg, signed = S: bool, set_flags = f: bool {
				let (result_quot, result_rem) = if signed {
					let lhs = self.register_file[lhs].get_signed(size);
					let rhs = self.register_file[rhs].get_signed(size);
					((lhs / rhs) as u64, (lhs % rhs) as u64)
				} else {
					let lhs = self.register_file[lhs].get_unsigned(size);
					let rhs = self.register_file[rhs].get_unsigned(size);
					(lhs / rhs, lhs % rhs)
				};

				if quot != rem {
					self.register_file[rem].set(size, result_rem);
				}
				self.register_file[quot].set(size, result_quot);

				if set_flags {
					self.flags.set_zero((result_quot & size.mask()) == 0);
					self.flags.set_sign(result_quot.bit_as_bool(size.msb_index() as u64));
				}
			},
			[1000010000000000ssfdddddaaaabbbb] => and size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: reg, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_unsigned(size);
				let rhs = self.register_file[rhs].get_unsigned(size);

				let result = lhs & rhs;

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					self.flags.set_overflow(false);
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_sign(result.bit_as_bool(size.msb_index() as u64));
				}
			},
			[100000ssfdddddaaaabbbbbbbbbbbSSS] => and size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: imm11, shift_factor = S: imm3, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_unsigned(size);
				let rhs = imm11_with_shift_factor(rhs, shift_factor, false);

				let result = lhs & rhs;

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					self.flags.set_overflow(false);
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_sign(result.bit_as_bool(size.msb_index() as u64));
				}
			},
			[0111110000000000ssfdddddaaaabbbb] => or size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: reg, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_unsigned(size);
				let rhs = self.register_file[rhs].get_unsigned(size);

				let result = lhs | rhs;

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					self.flags.set_overflow(false);
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_sign(result.bit_as_bool(size.msb_index() as u64));
				}
			},
			[011110ssfdddddaaaabbbbbbbbbbbSSS] => or size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: imm11, shift_factor = S: imm3, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_unsigned(size);
				let rhs = imm11_with_shift_factor(rhs, shift_factor, false);

				let result = lhs | rhs;

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					self.flags.set_overflow(false);
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_sign(result.bit_as_bool(size.msb_index() as u64));
				}
			},
			[0111010000000000ssfdddddaaaabbbb] => xor size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: reg, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_unsigned(size);
				let rhs = self.register_file[rhs].get_unsigned(size);

				let result = lhs ^ rhs;

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					self.flags.set_overflow(false);
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_sign(result.bit_as_bool(size.msb_index() as u64));
				}
			},
			[011100ssfdddddaaaabbbbbbbbbbbSSS] => xor size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: imm11, shift_factor = S: imm3, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_unsigned(size);
				let rhs = imm11_with_shift_factor(rhs, shift_factor, false);

				let result = lhs ^ rhs;

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					self.flags.set_overflow(false);
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_sign(result.bit_as_bool(size.msb_index() as u64));
				}
			},
			[0110100000000000ssfdddddaaaabbbb] => shl size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: reg, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_unsigned(size);
				let rhs = self.register_file[rhs].get_unsigned(size);
				let too_big = rhs >= size.bit_size() as u64;

				let result = if too_big {
					0
				} else {
					lhs << rhs
				};

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					if rhs != 0 {
						self.flags.set_carry(lhs.bit_as_bool(if too_big { 0 } else { (size.bit_size() as u64) - rhs }));
					}
					self.flags.set_overflow(false);
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_sign(result.bit_as_bool(size.msb_index() as u64));
				}
			},
			[0110110000000ssfdddddaaaabbbbbbb] => shl size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: imm7, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_unsigned(size);
				let too_big = rhs >= size.bit_size() as u64;

				let result = if too_big {
					0
				} else {
					lhs << rhs
				};

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					if rhs != 0 {
						self.flags.set_carry(lhs.bit_as_bool(if too_big { 0 } else { (size.bit_size() as u64) - rhs }));
					}
					self.flags.set_overflow(false);
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_sign(result.bit_as_bool(size.msb_index() as u64));
				}
			},
			[011000000000000ssAfdddddaaaabbbb] => shr size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: reg, signed = A: bool, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_unsigned(size);
				let rhs = self.register_file[rhs].get_unsigned(size);
				let too_big = rhs >= size.bit_size() as u64;

				let result = if too_big {
					if signed && lhs.bit_as_bool(size.msb_index() as u64) {
						ALL_BITS
					} else {
						0
					}
				} else {
					if signed {
						((lhs as i64) >> (rhs as i64)) as u64
					} else {
						lhs >> rhs
					}
				};

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					if rhs != 0 {
						self.flags.set_carry(lhs.bit_as_bool(if too_big { size.msb_index() as u64 } else { rhs - 1 }));
					}
					self.flags.set_overflow(false);
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_sign(result.bit_as_bool(size.msb_index() as u64));
				}
			},
			[011001000000ssAfdddddaaaabbbbbbb] => shr size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: imm7, signed = A: bool, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_unsigned(size);
				let too_big = rhs >= size.bit_size() as u64;

				let result = if too_big {
					if signed && lhs.bit_as_bool(size.msb_index() as u64) {
						ALL_BITS
					} else {
						0
					}
				} else {
					if signed {
						((lhs as i64) >> (rhs as i64)) as u64
					} else {
						lhs >> rhs
					}
				};

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					if rhs != 0 {
						self.flags.set_carry(lhs.bit_as_bool(if too_big { size.msb_index() as u64 } else { rhs - 1 }));
					}
					self.flags.set_overflow(false);
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_sign(result.bit_as_bool(size.msb_index() as u64));
				}
			},
			[0101100000000000ssfdddddaaaabbbb] => rot size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: reg, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_unsigned(size);
				let rhs = self.register_file[rhs].get_unsigned(size);
				let rhs = (rhs % (size.bit_size() as u64)) as u32;

				let result = match size {
					Size::Byte => (lhs as u8).rotate_right(rhs) as u64,
					Size::DoubleByte => (lhs as u16).rotate_right(rhs) as u64,
					Size::QuadByte => (lhs as u32).rotate_right(rhs) as u64,
					Size::Word => lhs.rotate_right(rhs) as u64,
				};

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					self.flags.set_sign(result.bit_as_bool(size.msb_index() as u64));
				}
			},
			[0101110000000ssfdddddaaaabbbbbbb] => rot size = s: size, dst = d: reg | null, lhs = a: reg, rhs = b: imm7, set_flags = f: bool {
				let lhs = self.register_file[lhs].get_unsigned(size);
				let rhs = (rhs % (size.bit_size() as u64)) as u32;

				let result = match size {
					Size::Byte => (lhs as u8).rotate_right(rhs) as u64,
					Size::DoubleByte => (lhs as u16).rotate_right(rhs) as u64,
					Size::QuadByte => (lhs as u32).rotate_right(rhs) as u64,
					Size::Word => lhs.rotate_right(rhs) as u64,
				};

				if let Some(dst) = dst {
					self.register_file[dst].set(size, result);
				}

				if set_flags {
					self.flags.set_sign(result.bit_as_bool(size.msb_index() as u64));
				}
			},
			[010101000000000000000ssfddddaaaa] => neg size = s: size, dst = d: reg, src = a: reg, set_flags = f: bool {
				let src = self.register_file[src].get_signed(size);

				let result = (-src) as u64;

				self.register_file[dst].set(size, result);

				if set_flags {
					self.flags.set_zero((result & size.mask()) == 0);
					self.flags.set_sign(result.bit_as_bool(size.msb_index() as u64));
				}
			},
			[010100000000000000000ssfddddaaaa] => bswap size = s: size, dst = d: reg, src = a: reg, set_flags = f: bool {
				let src = self.register_file[src].get_unsigned(size);

				let result = match size {
					Size::Byte => src,
					Size::DoubleByte => (src as u16).swap_bytes() as u64,
					Size::QuadByte => (src as u32).swap_bytes() as u64,
					Size::Word => src.swap_bytes(),
				};

				self.register_file[dst].set(size, result);

				if set_flags {
					self.flags.set_sign(result.bit_as_bool(size.msb_index() as u64));
				}
			},

			//
			// conditionals and control flow
			//

			[010011000000000000000000ccccaaaa] => jmpa cond = c: cond | null, addr = a: reg {
				let addr = self.register_file[addr].get_address();
				jump!(addr, cond);
			},
			[010010000000000000000000ccccaaaa] => jmpr cond = c: cond | null, addr = a: reg {
				let rel_base = self.instruction_pointer + 4;
				let addr = self.register_file[addr].get();
				let addr = rel_base + (((addr as i64) * 4) as u64);
				jump!(addr, cond);
			},
			[010001ccccaaaaaaaaaaaaaaaaaaaaaa] => jmpr cond = c: cond | null, addr = a: rel22 {
				let rel_base = self.instruction_pointer + 4;
				let addr = rel_base + (((addr as i64) * 4) as u64);
				jump!(addr, cond);
			},
			[001111000000000cccssaaaabbbbCCCC] => cjmpa cond = c: cond, size = s: size, addr = a: reg, lhs = b: reg, rhs = C: reg {
				let addr = self.register_file[addr].get_address();
				compare_and_jump!(size, lhs, rhs => addr, cond);
			},
			[001101000000000cccssaaaabbbbCCCC] => cjmpr cond = c: cond, size = s: size, addr = a: reg, lhs = b: reg, rhs = C: reg {
				let rel_base = self.instruction_pointer + 4;
				let addr = self.register_file[addr].get();
				let addr = rel_base + (((addr as i64) * 4) as u64);
				compare_and_jump!(size, lhs, rhs => addr, cond);
			},
			[001011cccssbbbbCCCCaaaaaaaaaaaaa] => cjmpr cond = c: cond, size = s: size, addr = a: rel13, lhs = b: reg, rhs = C: reg {
				let rel_base = self.instruction_pointer + 4;
				let addr = rel_base + (((addr as i64) * 4) as u64);
				compare_and_jump!(size, lhs, rhs => addr, cond);
			},
			[001010000000000000000000ccccaaaa] => calla cond = c: cond | null, addr = a: reg {
				let addr = self.register_file[addr].get_address();
				call!(addr, cond);
			},
			[001001000000000000000000ccccaaaa] => callr cond = c: cond | null, addr = a: reg {
				let rel_base = self.instruction_pointer + 4;
				let addr = self.register_file[addr].get();
				let addr = rel_base + (((addr as i64) * 4) as u64);
				call!(addr, cond);
			},
			[001000ccccaaaaaaaaaaaaaaaaaaaaaa] => callr cond = c: cond | null, addr = a: rel22 {
				let rel_base = self.instruction_pointer + 4;
				let addr = rel_base + (((addr as i64) * 4) as u64);
				call!(addr, cond);
			},
			[00011100000000000000000000000000] => ret {
				let link_addr = self.register_file[RegisterID::LR].get_address();
				self.instruction_pointer = link_addr - 4;
			},
			[00011000000000000000000000000000] => eret {
				if self.flags.privilege_level() != PrivilegeLevel::PL0 {
					return self.take_exception(Exception::InvalidOperation);
				}

				self.instruction_pointer = self.elr - 4;
				self.flags = self.eflags;
				self.register_file[RegisterID::SP] = self.esp.into();
			},
			[00000000000000000000000000000000] => udf {
				return self.take_exception(Exception::InvalidInstruction);
			},
			[00001000000000000000000000000000] => dbg {
				return self.take_exception(Exception::Debug);
			},
			[0000110000000000aaaaaaaaaaaaaaaa] => exc val = a: imm16 {
				self.instruction_pointer += 4;
				return self.take_exception(Exception::User(val as u16));
			},

			//
			// miscellaneous
			//

			[00000100000000000000000000000000] => nop {},
			[000100ddddaaaaaaaaaaaaaaaaaaaaaa] => ldm dst = d: reg, src_mreg = a: imm22 {
				let src_mreg = match MachineRegisterID::try_from(src_mreg as u32) {
					Ok(x) => x,
					Err(_) => return self.take_exception(Exception::InvalidOperation),
				};

				if !src_mreg.check_access(self.flags.privilege_level(), false) {
					return self.take_exception(Exception::InvalidOperation);
				}

				let val: u64 = match src_mreg {
					MachineRegisterID::flags => self.flags.into(),
					MachineRegisterID::elr => self.elr.into(),
					MachineRegisterID::esp => self.esp.into(),
					MachineRegisterID::eflags => self.eflags.into(),
					MachineRegisterID::einfo => self.einfo,
					MachineRegisterID::eaddr => self.eaddr.into(),
					MachineRegisterID::evtable => self.evtable_addr.into(),
					MachineRegisterID::ectable => self.ectable_addr.into(),
					_ => unreachable!(),
				};

				self.register_file[dst] = val.into();
			},
			[000101aaaadddddddddddddddddddddd] => stm dst_mreg = d: imm22, src = a: reg {
				let dst_mreg = match MachineRegisterID::try_from(dst_mreg as u32) {
					Ok(x) => x,
					Err(_) => return self.take_exception(Exception::InvalidOperation),
				};
				let src = self.register_file[src].get();

				if !dst_mreg.check_access(self.flags.privilege_level(), true) {
					return self.take_exception(Exception::InvalidOperation);
				}

				match dst_mreg {
					MachineRegisterID::flags => {
						if let Ok(flags) = src.try_into() {
							self.flags = flags;
						} else {
							return self.take_exception(Exception::InvalidOperation);
						}
					},
					MachineRegisterID::elr => {
						let src_addr = VMAddress::from(src);
						if src_addr.is_valid_instruction_pointer() {
							self.elr = src_addr;
						} else {
							return self.take_exception(Exception::InvalidOperation);
						}
					},
					MachineRegisterID::esp => {
						self.esp = src.into();
					},
					MachineRegisterID::eflags => {
						if let Ok(flags) = src.try_into() {
							self.eflags = flags;
						} else {
							return self.take_exception(Exception::InvalidOperation);
						}
					},
					MachineRegisterID::evtable => {
						let src_addr = VMAddress::from(src);
						if src_addr.is_valid_instruction_pointer() {
							self.evtable_addr = src_addr;
						} else {
							return self.take_exception(Exception::InvalidOperation);
						}
					},
					MachineRegisterID::ectable => {
						let src_addr = VMAddress::from(src);
						let mem = get_memory!(src_addr, std::mem::size_of::<ExceptionConfigurationTable>() as u64);

						// SAFETY: it's safe to read the table from the pointer since the type (ExceptionConfigurationTable) is Copy.
						//         additionally, we've already verified that we have the necessary space because the slice above is
						//         of the required length.
						let tmp = unsafe { std::ptr::read_unaligned(mem.as_ptr() as *const ExceptionConfigurationTable) };

						// now let's check the table entries
						if !tmp.pl0.iter().all(ExceptionConfigurationEntry::validate) || !tmp.pl1.iter().all(ExceptionConfigurationEntry::validate) {
							return self.take_exception(Exception::InvalidOperation);
						}

						self.ectable_addr = src.into();
						self.ectable = tmp;
					},
					MachineRegisterID::vm_console => {
						let character: char = (src as u8).into();
						print!("{}", character);
					},
					_ => unreachable!(),
				}
			},
			_ => {
				// invalid instruction
				self.take_exception(Exception::InvalidInstruction);
			},
		}

		self.instruction_pointer += 4;
	}

	fn take_exception(&mut self, exception: Exception) {
		println!(
			"***Exception ({:?}) at {:#x}***",
			exception,
			u64::from(self.instruction_pointer)
		);

		self.eflags = self.flags;
		self.elr = self.instruction_pointer;
		self.flags.set_exceptions_enabled(false);
		self.flags.set_privilege_level(PrivilegeLevel::PL0);
		self.esp = self.register_file[RegisterID::SP].get_address();

		self.einfo = match exception {
			Exception::Unknown => 0,
			Exception::InvalidInstruction => 1,
			Exception::Debug => 2,
			Exception::User(val) => 3 | ((val as u64) << 3),
			Exception::InvalidOperation => 4,
			Exception::InstructionLoadError => 5,
			Exception::DataLoadError {
				address: _,
				write,
				byte_size,
			} => 6 | (if write { 1 << 3 } else { 0 }) | ((byte_size as u64) << 4),
			Exception::Interrupt(val) => 7 | (val << 3),
		};

		self.eaddr = match exception {
			Exception::DataLoadError { address, .. } => address,
			_ => 0.into(),
		};

		let ectable_pl = match self.eflags.privilege_level() {
			PrivilegeLevel::PL0 => &self.ectable.pl0,
			PrivilegeLevel::PL1 => &self.ectable.pl1,
		};
		let entry = &ectable_pl[exception.id() as usize];

		let rsp = self.register_file[RegisterID::SP].get();
		let stack_base = u64::from(entry.stack_pointer);
		let stack_top = u64::from(entry.stack_pointer + entry.stack_size);

		if entry.flags.contains(ExceptionConfigurationFlags::USE_STACK)
			&& (rsp < stack_base || rsp > stack_top)
		{
			self.register_file[RegisterID::SP] = stack_top.into();
		}

		let pl_offset: u64 = match self.eflags.privilege_level() {
			PrivilegeLevel::PL0 => 0,
			PrivilegeLevel::PL1 => 1,
		} * 256;
		let exc_offset = (exception.id() as u64) * 32;
		self.instruction_pointer = self.evtable_addr + pl_offset + exc_offset;
	}

	pub fn run(mut self) -> ! {
		loop {
			self.execute_one()
		}
	}
}
