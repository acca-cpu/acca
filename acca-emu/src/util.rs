//
// Copyright (C) 2023 Ariel Abreu
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

use std::{
	ops::{Index, IndexMut},
	slice::SliceIndex,
};

use byteorder::{ByteOrder, LittleEndian};

use auto_ops::*;
use num_enum::TryFromPrimitive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Condition {
	C = 0,
	NC = 1,
	Z = 2,
	NZ = 3,
	O = 4,
	NO = 5,
	S = 6,
	NS = 7,
	L = 8,
	NL = 9,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RegisterID(u8);

#[derive(Debug, Clone, Copy)]
pub(crate) struct Register(u64);

#[derive(Debug, Clone, Copy)]
pub(crate) enum Size {
	Byte = 0,
	DoubleByte = 1,
	QuadByte = 2,
	Word = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VMAddress(u64);

#[derive(Debug, Clone, Copy)]
pub(crate) struct RegisterFile([Register; 16]);

#[derive(Debug, Clone, Copy)]
pub(crate) struct CPUFlags(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrivilegeLevel {
	PL0 = 0,
	PL1 = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u32)]
#[allow(non_camel_case_types)]
pub(crate) enum MachineRegisterID {
	flags = 0,
	elr = 1,
	esp = 2,
	eflags = 3,
	einfo = 4,
	eaddr = 5,
	evtable = 6,
	ectable = 7,

	vm_console = 0xdead_1,
}

pub(crate) fn zero_extend_immediate(immediate: u64, width: u64) -> u64 {
	const ALL_BITS: u64 = !0u64;
	let mask = ALL_BITS.checked_shr(64 - width as u32).unwrap_or(0);
	immediate & mask
}

pub(crate) fn sign_extend_immediate(immediate: u64, width: u64) -> u64 {
	const ALL_BITS: u64 = !0u64;
	let masked = zero_extend_immediate(immediate, width);
	let msb = 1u64 << (width - 1);
	if (masked & msb) != 0 {
		masked | (ALL_BITS.checked_shl(width as u32).unwrap_or(0))
	} else {
		masked
	}
}

impl Condition {
	pub(crate) fn test(&self, carry: bool, zero: bool, overflow: bool, sign: bool) -> bool {
		match self {
			Condition::C => carry,
			Condition::NC => !carry,
			Condition::Z => zero,
			Condition::NZ => !zero,
			Condition::O => overflow,
			Condition::NO => !overflow,
			Condition::S => sign,
			Condition::NS => !sign,
			Condition::L => sign ^ overflow,
			Condition::NL => !(sign ^ overflow),
		}
	}
}

impl From<u64> for Condition {
	fn from(value: u64) -> Self {
		match value {
			0 => Self::C,
			1 => Self::NC,
			2 => Self::Z,
			3 => Self::NZ,
			4 => Self::O,
			5 => Self::NO,
			6 => Self::S,
			7 => Self::NS,
			8 => Self::L,
			9 => Self::NL,
			_ => panic!("Invalid condition value"),
		}
	}
}

impl From<u64> for RegisterID {
	fn from(value: u64) -> Self {
		match value {
			0..=15 => Self(value as u8),
			_ => panic!("Invalid register value"),
		}
	}
}

impl From<u64> for Size {
	fn from(value: u64) -> Self {
		match value {
			0 => Self::Byte,
			1 => Self::DoubleByte,
			2 => Self::QuadByte,
			3 => Self::Word,
			_ => panic!("Invalid size value"),
		}
	}
}

impl Size {
	pub const fn byte_size(&self) -> u8 {
		match self {
			Size::Byte => 1,
			Size::DoubleByte => 2,
			Size::QuadByte => 4,
			Size::Word => 8,
		}
	}

	pub const fn bit_size(&self) -> u8 {
		self.byte_size() * 8
	}

	pub fn read(&self, src: &[u8], sign_extend: bool) -> u64 {
		let val = match self {
			Size::Byte => src[0] as u64,
			Size::DoubleByte => LittleEndian::read_u16(src) as u64,
			Size::QuadByte => LittleEndian::read_u32(src) as u64,
			Size::Word => LittleEndian::read_u64(src),
		};
		if sign_extend {
			sign_extend_immediate(val, self.byte_size() as u64 * 8)
		} else {
			val
		}
	}

	pub fn write(&self, src: u64, dst: &mut [u8]) {
		match self {
			Size::Byte => dst[0] = src as u8,
			Size::DoubleByte => LittleEndian::write_u16(dst, src as u16),
			Size::QuadByte => LittleEndian::write_u32(dst, src as u32),
			Size::Word => LittleEndian::write_u64(dst, src),
		}
	}

	pub const fn msb_index(&self) -> u8 {
		self.bit_size() - 1
	}

	pub const fn msb_mask(&self) -> u64 {
		1 << (self.msb_index() as u64)
	}

	pub const fn mask(&self) -> u64 {
		const ALL_BITS: u64 = !0u64;
		ALL_BITS >> (64 - (self.bit_size() as u64))
	}
}

impl VMAddress {
	pub const fn new(address: u64) -> Self {
		Self(address)
	}

	pub fn is_valid_instruction_pointer(&self) -> bool {
		(self.0 & 3) == 0
	}
}

impl_op_ex!(+ |addr: &VMAddress, size: &u64| -> VMAddress { VMAddress::new(addr.0.wrapping_add(*size)) });
#[rustfmt::skip]
impl_op_ex!(- |addr: &VMAddress, size: &u64| -> VMAddress { VMAddress::new(addr.0.wrapping_sub(*size)) });
impl_op_ex!(+= |addr: &mut VMAddress, size: &u64| { *addr = *addr + size; });
impl_op_ex!(-= |addr: &mut VMAddress, size: &u64| { *addr = *addr - size; });

impl From<u64> for VMAddress {
	fn from(value: u64) -> Self {
		Self::new(value)
	}
}

impl From<VMAddress> for u64 {
	fn from(value: VMAddress) -> Self {
		value.0
	}
}

impl<T> Index<T> for RegisterFile
where
	T: SliceIndex<[Register], Output = Register>,
{
	type Output = Register;

	fn index(&self, index: T) -> &Self::Output {
		&self.0[index]
	}
}

impl<T> IndexMut<T> for RegisterFile
where
	T: SliceIndex<[Register], Output = Register>,
{
	fn index_mut(&mut self, index: T) -> &mut Self::Output {
		&mut self.0[index]
	}
}

impl Index<RegisterID> for RegisterFile {
	type Output = Register;

	fn index(&self, index: RegisterID) -> &Self::Output {
		&self.0[index.0 as usize]
	}
}

impl IndexMut<RegisterID> for RegisterFile {
	fn index_mut(&mut self, index: RegisterID) -> &mut Self::Output {
		&mut self.0[index.0 as usize]
	}
}

impl RegisterID {
	pub const SP: Self = Self(13);
	pub const FP: Self = Self(14);
	pub const LR: Self = Self(15);
}

impl RegisterFile {
	pub const fn new() -> Self {
		Self([Register(0); 16])
	}
}

impl Default for RegisterFile {
	fn default() -> Self {
		Self::new()
	}
}

impl Register {
	pub fn get_unsigned(&self, size: Size) -> u64 {
		zero_extend_immediate(self.0, size.byte_size() as u64 * 8)
	}

	pub fn get_signed(&self, size: Size) -> i64 {
		sign_extend_immediate(self.0, size.byte_size() as u64 * 8) as i64
	}

	pub const fn get(&self) -> u64 {
		self.0
	}

	pub const fn get_address(&self) -> VMAddress {
		VMAddress(self.0)
	}

	pub fn set(&mut self, size: Size, value: u64) {
		const ALL_BITS: u64 = !0u64;
		let preserve_mask = ALL_BITS
			.checked_shl(size.byte_size() as u32 * 8)
			.unwrap_or(0);
		self.0 = (self.0 & preserve_mask) | (value & !preserve_mask);
	}
}

impl From<Register> for u64 {
	fn from(value: Register) -> Self {
		value.0
	}
}

impl From<u64> for Register {
	fn from(value: u64) -> Self {
		Register(value)
	}
}

impl From<VMAddress> for Register {
	fn from(value: VMAddress) -> Self {
		Register(value.0)
	}
}

impl AsRef<u64> for Register {
	fn as_ref(&self) -> &u64 {
		&self.0
	}
}

impl AsMut<u64> for Register {
	fn as_mut(&mut self) -> &mut u64 {
		&mut self.0
	}
}

pub(crate) trait BitBool {
	fn bit_as_bool(&self, index: Self) -> bool;
	fn set_bit_with_bool(&mut self, index: Self, value: bool);
}

impl BitBool for u64 {
	fn bit_as_bool(&self, index: Self) -> bool {
		(self & (1 << index)) != 0
	}

	fn set_bit_with_bool(&mut self, index: Self, value: bool) {
		if value {
			*self |= 1 << index;
		} else {
			*self &= !(1 << index);
		}
	}
}

impl CPUFlags {
	const VALID_MASK: u64 = 0x3fu64;

	pub const fn new() -> Self {
		Self(0)
	}

	pub fn carry(&self) -> bool {
		self.0.bit_as_bool(0)
	}

	pub fn zero(&self) -> bool {
		self.0.bit_as_bool(1)
	}

	pub fn overflow(&self) -> bool {
		self.0.bit_as_bool(2)
	}

	pub fn sign(&self) -> bool {
		self.0.bit_as_bool(3)
	}

	pub fn exceptions_enabled(&self) -> bool {
		self.0.bit_as_bool(4)
	}

	pub fn privilege_level(&self) -> PrivilegeLevel {
		if self.0.bit_as_bool(5) {
			PrivilegeLevel::PL1
		} else {
			PrivilegeLevel::PL0
		}
	}

	pub fn set_carry(&mut self, value: bool) {
		self.0.set_bit_with_bool(0, value);
	}

	pub fn set_zero(&mut self, value: bool) {
		self.0.set_bit_with_bool(1, value);
	}

	pub fn set_overflow(&mut self, value: bool) {
		self.0.set_bit_with_bool(2, value);
	}

	pub fn set_sign(&mut self, value: bool) {
		self.0.set_bit_with_bool(3, value);
	}

	pub fn set_exceptions_enabled(&mut self, value: bool) {
		self.0.set_bit_with_bool(4, value);
	}

	pub fn set_privilege_level(&mut self, value: PrivilegeLevel) {
		self.0
			.set_bit_with_bool(5, matches!(value, PrivilegeLevel::PL1));
	}

	pub fn test_condition(&self, cond: Condition) -> bool {
		match cond {
			Condition::C => self.carry(),
			Condition::NC => !self.carry(),
			Condition::Z => self.zero(),
			Condition::NZ => !self.zero(),
			Condition::O => self.overflow(),
			Condition::NO => !self.overflow(),
			Condition::S => self.sign(),
			Condition::NS => !self.sign(),
			Condition::L => self.sign() ^ self.overflow(),
			Condition::NL => !(self.sign() ^ self.overflow()),
		}
	}
}

impl Default for CPUFlags {
	fn default() -> Self {
		Self::new()
	}
}

impl TryFrom<u64> for CPUFlags {
	type Error = ();

	fn try_from(value: u64) -> Result<Self, Self::Error> {
		if (value & !(Self::VALID_MASK)) != 0 {
			Err(())
		} else {
			Ok(Self(value))
		}
	}
}

impl From<CPUFlags> for u64 {
	fn from(value: CPUFlags) -> Self {
		value.0
	}
}

impl MachineRegisterID {
	pub fn check_access(&self, priv_level: PrivilegeLevel, write: bool) -> bool {
		match self {
			MachineRegisterID::flags => !write || priv_level == PrivilegeLevel::PL0,
			MachineRegisterID::elr
			| MachineRegisterID::esp
			| MachineRegisterID::eflags
			| MachineRegisterID::evtable
			| MachineRegisterID::ectable => priv_level == PrivilegeLevel::PL0,
			MachineRegisterID::einfo | MachineRegisterID::eaddr => {
				!write && priv_level == PrivilegeLevel::PL0
			},
			MachineRegisterID::vm_console => write,
		}
	}
}
