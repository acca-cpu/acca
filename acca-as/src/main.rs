//
// Copyright (C) 2022 Ariel Abreu
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

use std::{
	collections::HashMap,
	ffi::OsString,
	fmt::Display,
	fs,
	path::{Path, PathBuf},
	process::exit,
};

use clap::Parser as ClapParser;

extern crate pest;
#[macro_use]
extern crate pest_derive;
extern crate proc_macro;
#[macro_use]
extern crate acca_as_proc_macro;
extern crate positioned_io;

use lazy_static::lazy_static;
use pest::{
	iterators::{Pair, Pairs},
	pratt_parser::{Assoc, Op, PrattParser},
	Parser,
};

use positioned_io::WriteAt;

#[derive(ClapParser)]
#[command(author, version, about, long_about = None)]
struct Args {
	source: PathBuf,

	#[arg(short, long)]
	output: Option<PathBuf>,
}

#[derive(Parser)]
#[grammar = "grammar.pest"]
struct ASMParser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Size {
	Byte = 0,
	DoubleByte = 1,
	QuadByte = 2,
	Word = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Condition {
	C = 0,
	NC = 1,
	Z = 2,
	NZ = 3,
	O = 4,
	NO = 5,
	S = 6,
	NS = 7,
}

#[derive(Debug, Clone, Copy)]
struct Register {
	/// A number within 0-15 that identifies the register.
	id: u8,
	size: Option<Size>,
}

#[derive(Debug, Clone, Copy)]
enum Argument {
	Register(Register),
	Immediate(u64),
}

#[derive(Debug, Clone, Copy)]
struct NullableRegister(pub Option<Register>);

impl Display for Register {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "r{}", self.id).and_then(|_| match self.size {
			Some(size) => write!(
				f,
				"{}",
				match size {
					Size::Byte => "b",
					Size::DoubleByte => "d",
					Size::QuadByte => "q",
					Size::Word => "w",
				}
			),
			None => std::fmt::Result::Ok(()),
		})
	}
}

impl Display for NullableRegister {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self.0 {
			Some(reg) => write!(f, "{}", reg),
			None => write!(f, "<null>"),
		}
	}
}

impl Display for Argument {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Register(reg) => write!(f, "Register({})", reg),
			Self::Immediate(imm) => write!(f, "Immediate({})", imm),
		}
	}
}

#[allow(dead_code)]
impl Argument {
	pub fn map_immediate<F>(self, f: F) -> Self
	where
		F: FnOnce(u64) -> u64,
	{
		match self {
			Self::Immediate(imm) => Self::Immediate(f(imm)),
			_ => self,
		}
	}

	pub fn map_register<F>(self, f: F) -> Self
	where
		F: FnOnce(Register) -> Register,
	{
		match self {
			Self::Register(reg) => Self::Register(f(reg)),
			_ => self,
		}
	}

	pub fn map<F, G>(self, map_reg: F, map_imm: G) -> Self
	where
		F: FnOnce(Register) -> Register,
		G: FnOnce(u64) -> u64,
	{
		match self {
			Self::Register(reg) => Self::Register(map_reg(reg)),
			Self::Immediate(imm) => Self::Immediate(map_imm(imm)),
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
}

lazy_static! {
	static ref PRATT: PrattParser<Rule> = PrattParser::new()
		.op(Op::infix(Rule::or, Assoc::Left))
		.op(Op::infix(Rule::xor, Assoc::Left))
		.op(Op::infix(Rule::and, Assoc::Left))
		.op(Op::infix(Rule::shift_left, Assoc::Left)
			| Op::infix(Rule::shift_right_logical, Assoc::Left)
			| Op::infix(Rule::shift_right_arithmetic, Assoc::Left))
		.op(Op::infix(Rule::add, Assoc::Left) | Op::infix(Rule::sub, Assoc::Left))
		.op(Op::infix(Rule::mul, Assoc::Left)
			| Op::infix(Rule::div, Assoc::Left)
			| Op::infix(Rule::rem, Assoc::Left))
		.op(Op::prefix(Rule::neg) | Op::prefix(Rule::not));
}

fn parse_integer(integer: Pair<Rule>) -> u64 {
	if integer.as_rule() != Rule::integer {
		panic!("Tried to parse integer that wasn't an integer");
	}

	let lit = integer.into_inner().next().unwrap();

	let as_str = match lit.as_rule() {
		Rule::decimal_literal
			if !lit.as_str().starts_with("0d") && !lit.as_str().starts_with("0D") =>
		{
			lit.as_str()
		},
		_ => &lit.as_str()[2..],
	};

	let radix = match lit.as_rule() {
		Rule::binary_literal => 2,
		Rule::octal_literal => 8,
		Rule::decimal_literal => 10,
		Rule::hex_literal => 16,
		_ => unreachable!(),
	};

	let filtered: String = as_str.chars().filter(|&char| char != '_').collect();

	u64::from_str_radix(&filtered, radix).unwrap()
}

fn parse_machine_register(mreg: &str) -> u64 {
	match mreg {
		"flags" => 0,
		"elr" => 1,
		"esp" => 2,
		"eflags" => 3,
		"einfo" => 4,
		"eaddr" => 5,
		"evtable" => 6,
		"ectable" => 7,
		_ => panic!("Invalid machine register literal"),
	}
}

fn evaluate_immediate(
	immediate: Pair<Rule>,
	label_addrs: &HashMap<&str, u64>,
	filepath: &Path,
	current_address: u64,
	for_relative_addr: Option<u64>,
) -> u64 {
	if immediate.as_rule() != Rule::immediate {
		panic!("Tried to evaluate immediate that wasn't an immediate");
	}

	let loc = immediate.line_col();
	let result = PRATT
		.map_primary(|primary| match primary.as_rule() {
			Rule::integer => parse_integer(primary),
			Rule::boolean => match primary.as_str() {
				"true" => 1,
				"false" => 0,
				_ => unreachable!(),
			},
			Rule::machine_register_literal => {
				parse_machine_register(primary.into_inner().next().unwrap().as_str())
			},
			Rule::ident => match label_addrs.get(primary.as_str()) {
				Some(x) => *x,
				None => {
					println!("Unknown label \"{}\"", primary.as_str());
					exit(1);
				},
			},
			Rule::current_address => current_address,
			Rule::character => {
				let inner = primary.into_inner().next().unwrap();

				(match inner.as_rule() {
					Rule::normal_char => inner.as_str().chars().nth(1).unwrap(),
					Rule::escaped_char => match inner.as_str().chars().nth(2).unwrap() {
						'\'' => '\'',
						'\\' => '\\',
						'n' => '\n',
						'f' => char::from_u32(12).unwrap(),
						't' => '\t',
						'r' => '\r',
						'b' => char::from_u32(8).unwrap(),
						_ => unreachable!(),
					},
					_ => unreachable!(),
				} as u32) as u64
			},
			Rule::immediate => {
				evaluate_immediate(primary, label_addrs, filepath, current_address, None)
			},
			_ => unreachable!(),
		})
		.map_prefix(|op, rhs| match op.as_rule() {
			Rule::neg => rhs.wrapping_neg(),
			Rule::not => !rhs,
			_ => unreachable!(),
		})
		.map_infix(move |lhs, op, rhs| match op.as_rule() {
			Rule::or => lhs | rhs,
			Rule::xor => lhs ^ rhs,
			Rule::and => lhs & rhs,
			Rule::shift_left => lhs.checked_shl(rhs as u32).unwrap_or(0),
			Rule::shift_right_logical => lhs.checked_shr(rhs as u32).unwrap_or(0),
			Rule::shift_right_arithmetic => (lhs as i64)
				.checked_shr(rhs as u32)
				.unwrap_or(if (lhs as i64) < 0 { -1 } else { 0 })
				as u64,
			Rule::add => lhs.wrapping_add(rhs),
			Rule::sub => lhs.wrapping_sub(rhs),
			Rule::mul => lhs.wrapping_mul(rhs),
			Rule::div => {
				if rhs == 0 {
					println!(
						"Error: attempt to divide by 0 while evaluating immediate: {}:{}:{}",
						filepath.display(),
						loc.0,
						loc.1
					);
					exit(1);
				} else {
					lhs / rhs
				}
			},
			Rule::rem => lhs % rhs,
			_ => unreachable!(),
		})
		.parse(immediate.into_inner());

	match for_relative_addr {
		Some(rel_addr) => {
			if (result & 3) != 0 {
				panic!("Invalid relative address: not aligned to 4 bytes")
			}

			(result / 4).wrapping_sub(rel_addr / 4)
		},
		None => result,
	}
}

fn parse_size(size: Pair<Rule>) -> Size {
	if size.as_rule() != Rule::size {
		panic!("Tried to parse size that wasn't a size");
	}

	match size.as_str() {
		"b" => Size::Byte,
		"d" => Size::DoubleByte,
		"q" => Size::QuadByte,
		"w" => Size::Word,
		_ => unreachable!(),
	}
}

fn parse_register(register: Pair<Rule>) -> Register {
	if register.as_rule() != Rule::register && register.as_rule() != Rule::register_no_size {
		panic!("Tried to parse register that wasn't a register");
	}

	let mut pairs = register.into_inner();
	let num_or_name = pairs.next().unwrap();

	Register {
		id: match num_or_name.as_rule() {
			Rule::register_number => num_or_name.as_str().parse().unwrap(),
			Rule::register_name => match num_or_name.as_str() {
				"sp" => 13,
				"fp" => 14,
				"lr" => 15,
				_ => unreachable!(),
			},
			_ => unreachable!(),
		},
		size: pairs.next().map(|size| parse_size(size)),
	}
}

fn parse_register_or_null(register_or_null: Pair<Rule>) -> Option<Register> {
	match register_or_null.as_rule() {
		Rule::register => Some(parse_register(register_or_null)),
		Rule::null => None,
		_ => panic!("Tried to parse register-or-null that wasn't a register or null"),
	}
}

fn parse_condition(condition: Pair<Rule>) -> Condition {
	if condition.as_rule() != Rule::condition {
		panic!("Tried to parse condition that wasn't a condition");
	}

	match condition.as_str() {
		"c" => Condition::C,
		"nc" => Condition::NC,
		"z" => Condition::Z,
		"nz" => Condition::NZ,
		"o" => Condition::O,
		"no" => Condition::NO,
		"s" => Condition::S,
		"ns" => Condition::NS,
		_ => unreachable!(),
	}
}

fn parse_instr_size(instr_name: Pair<Rule>) -> Option<Size> {
	instr_name
		.into_inner()
		.next()
		.map(|pair| parse_size(pair.into_inner().next().unwrap()))
}

fn parse_instr_condition(instr_name: Pair<Rule>) -> Option<Condition> {
	instr_name
		.into_inner()
		.next()
		.map(|pair| parse_condition(pair.into_inner().next().unwrap()))
}

fn parse_instr_condition_and_size(instr_name: Pair<Rule>) -> (Option<Condition>, Option<Size>) {
	let mut pairs = instr_name.into_inner();
	(
		pairs
			.next()
			.map(|pair| parse_condition(pair.into_inner().next().unwrap())),
		pairs
			.next()
			.map(|pair| parse_size(pair.into_inner().next().unwrap())),
	)
}

fn parse_argument(
	argument: Pair<Rule>,
	label_addrs: &HashMap<&str, u64>,
	filepath: &Path,
	current_address: u64,
	for_relative_addr: Option<u64>,
) -> Argument {
	match argument.as_rule() {
		Rule::register => Argument::Register(parse_register(argument)),
		Rule::immediate => Argument::Immediate(evaluate_immediate(
			argument,
			label_addrs,
			filepath,
			current_address,
			for_relative_addr,
		)),
		_ => panic!("Tried to parse argument that wasn't a register or immediate"),
	}
}

fn next_register(pairs: &mut Pairs<Rule>) -> Option<Register> {
	pairs.next().map(parse_register)
}

fn next_register_or_null(pairs: &mut Pairs<Rule>) -> Option<Register> {
	pairs.next().and_then(parse_register_or_null)
}

fn next_instr_size(pairs: &mut Pairs<Rule>) -> Option<Size> {
	pairs.next().and_then(parse_instr_size)
}

fn next_instr_condition(pairs: &mut Pairs<Rule>) -> Option<Condition> {
	pairs.next().and_then(parse_instr_condition)
}

fn next_instr_condition_and_size(pairs: &mut Pairs<Rule>) -> (Option<Condition>, Option<Size>) {
	match pairs.next() {
		Some(instr_name) => parse_instr_condition_and_size(instr_name),
		None => (None, None),
	}
}

fn parse_machine_register_or_immediate(
	pair: Pair<Rule>,
	label_addrs: &HashMap<&str, u64>,
	filepath: &Path,
	current_address: u64,
	for_relative_addr: Option<u64>,
) -> u64 {
	match pair.as_rule() {
		Rule::machine_register => parse_machine_register(pair.as_str()),
		Rule::immediate => evaluate_immediate(pair, label_addrs, filepath, current_address, for_relative_addr),
		_ => panic!("Tried to parse machinr register or immediate that wasn't a machine register or immediate"),
	}
}

fn truncate_immediate(immediate: u64, bits: u8, sign_extend: bool) -> u64 {
	const ALL_BITS: u64 = !0u64;

	let msb_pos = bits - 1;
	let msb_mask = 1u64 << msb_pos;

	let mask = ALL_BITS >> (u64::BITS - (bits as u32));
	let masked = immediate & mask;

	if sign_extend && (masked & msb_mask) != 0 {
		!mask | masked
	} else {
		masked
	}
}

fn common_register_size(arguments: &[Option<Argument>], mut operation_size: Option<Size>) -> Size {
	for arg in arguments {
		match arg {
			Some(Argument::Register(reg)) => match &reg.size {
				Some(reg_size) => match &operation_size {
					Some(current_size) => {
						if current_size != reg_size {
							panic!(
								"Incompatible register sizes: {:?} and {:?}",
								current_size, reg_size
							);
						}
					},
					None => operation_size = Some(*reg_size),
				},
				None => {},
			},
			_ => {},
		}
	}

	return *operation_size.get_or_insert(Size::Word);
}

macro_rules! instruction_match_pattern {
	($tmp:tt, reg) => {
		Some(Argument::Register($tmp))
	};
	($tmp:tt, null) => {
		None
	};
	($tmp:tt, $other:ident) => {
		Some(Argument::Immediate($tmp))
	};
}

macro_rules! instruction_match_body {
	($tmp:tt, reg) => {{
		$tmp.id as u64
	}};
	($tmp:tt, null) => {{
		31u64
	}};
	($tmp:tt, $other:ident) => {{
		$tmp
	}};
}

macro_rules! instruction_body {
	($write_instr:ident $($additional_ident:ident)* { $($arg:ident : $($ty:ident)|*),+ $(,)? $({ $(let $size_name:ident = size_of($($reg:ident),+ $(,)?) ;)* })? => [ $($value:tt)* ] $($(,)? $($rest_arg:ident : $($rest_ty:ident)|*),+ $(,)? $({ $($size_block:tt)* })? => $rest_value:tt)* $(,)? }) => {
		if $(matches!($arg, $(instruction_match_pattern!(_, $ty))|+))&&+ {
			$(
				$(
					#[allow(non_snake_case)]
					let $size_name = common_register_size(&[$($reg),+], None) as u64;
				)*
			)?
			$(
				#[allow(non_snake_case)]
				let $arg = match $arg {
					$(
						instruction_match_pattern!(tmp, $ty) => instruction_match_body!(tmp, $ty),
					)*
					_ => unreachable!(),
				};
			)+
			instruction_encoding! { $write_instr $($($size_name)*)? $($arg)+ $($additional_ident)* ; $($value)* }
		}
		instruction_body! { $write_instr $($additional_ident)* { $($($rest_arg : $($rest_ty)|*),+ $({ $($size_block)* })? => $rest_value)* }}
	};
	($write_instr:ident $($additional_ident:ident)* $({})?) => {};
	($write_instr:ident $($arg:ident : $($ty:ident)|*),* ; $($additional_ident:ident)* [ $($value:tt)* ]) => {
		if true $(&& matches!($arg, $(instruction_match_pattern!(_, $ty))|+))* {
			$(
				#[allow(non_snake_case)]
				let $arg = match $arg {
					$(
						instruction_match_pattern!(tmp, $ty) => instruction_match_body!(tmp, $ty),
					)*
					_ => unreachable!(),
				};
			)*
			instruction_encoding! { $write_instr $($arg)* $($additional_ident)* ; $($value)* }
		} else {
			panic!("Internal instruction evaluation error")
		}
	};
}

fn main() {
	let cli = Args::parse();

	let input = match fs::read_to_string(&cli.source) {
		Ok(x) => x,
		Err(e) => {
			println!(
				"Failed to read \"{}\": {}",
				cli.source.display(),
				e.to_string()
			);
			exit(1);
		},
	};

	let output_path = cli.output.unwrap_or_else(|| {
		cli.source.with_extension(match cli.source.extension() {
			Some(source) => {
				let mut tmp = source.to_owned();
				tmp.push(".bin");
				tmp
			},
			None => OsString::from("bin"),
		})
	});

	let pairs = ASMParser::parse(Rule::root, &input).unwrap_or_else(|e| {
		let err = match cli.source.as_os_str().to_str() {
			Some(path) => e.with_path(path),
			None => e,
		};
		println!("Failed to parse input:\n{}", err);
		exit(1);
	});

	let mut label_addrs = HashMap::<&str, u64>::new();
	let mut addr = 0u64;

	let mut peekable_pairs = pairs.clone();

	// first, determine all addresses
	while let Some(pair) = peekable_pairs.next() {
		match pair.as_rule() {
			Rule::label => {
				// first, gather up all consecutive labels
				let mut labels = vec![pair];
				while let Some(next_pair) = peekable_pairs.peek() {
					match next_pair.as_rule() {
						Rule::label => labels.push(peekable_pairs.next().unwrap()),
						_ => break,
					}
				}
				// now check if we need to align the address (i.e. if the next item is an instruction)
				match peekable_pairs.peek() {
					Some(next_pair) if next_pair.as_rule() == Rule::instr => {
						// align it to instruction size
						addr = (addr + 3) & !3u64;
					},
					_ => {},
				}
				// now assign the same address to all the labels
				for label in labels {
					let label_name = label.into_inner().next().unwrap().as_str();
					if let Some(_) = label_addrs.insert(label_name, addr) {
						panic!("Duplicate label \"{}\"", label_name);
					}
				}
			},
			Rule::instr => {
				// align it to instruction size
				addr = (addr + 3) & !3u64;
				// advance by one instruction
				addr += 4;
			},
			Rule::directive => {
				let dir = pair.into_inner().next().unwrap();
				let mut dir_pairs = dir.clone().into_inner();

				match dir.as_rule() {
					Rule::directive_addr => {
						let _name = dir_pairs.next().unwrap();
						let new_addr = dir_pairs
							.next()
							.map(|immediate| {
								evaluate_immediate(immediate, &label_addrs, &cli.source, addr, None)
							})
							.unwrap();

						addr = new_addr;
					},
					Rule::directive_write => {
						let size = next_instr_size(&mut dir_pairs).unwrap();
						let immediate_count = dir_pairs.count() as u64;

						addr += (size.byte_size() as u64) * immediate_count;
					},
					Rule::directive_def => {
						let _name = dir_pairs.next().unwrap();
						let name = dir_pairs.next().unwrap().as_str();
						let val = dir_pairs
							.next()
							.map(|immediate| {
								evaluate_immediate(immediate, &label_addrs, &cli.source, addr, None)
							})
							.unwrap();

						label_addrs.insert(name, val);
					},
					_ => unreachable!(),
				}
			},
			Rule::EOI => {},
			_ => unreachable!(),
		}
	}

	let next_immediate = |pairs: &mut Pairs<Rule>, addr: u64| {
		pairs
			.next()
			.map(|immediate| evaluate_immediate(immediate, &label_addrs, &cli.source, addr, None))
	};

	#[allow(unused)]
	let next_relative_immediate = |pairs: &mut Pairs<Rule>, addr: u64, relative_address: u64| {
		pairs.next().map(|immediate| {
			evaluate_immediate(
				immediate,
				&label_addrs,
				&cli.source,
				addr,
				Some(relative_address),
			)
		})
	};

	let next_byte_relative_immediate = |pairs: &mut Pairs<Rule>, addr: u64| {
		let rel_addr = addr + 4;
		pairs
			.next()
			.map(|immediate| evaluate_immediate(immediate, &label_addrs, &cli.source, addr, None))
			.map(|result| result.wrapping_sub(rel_addr))
	};

	let next_argument = |pairs: &mut Pairs<Rule>, addr: u64| {
		pairs
			.next()
			.map(|argument| parse_argument(argument, &label_addrs, &cli.source, addr, None))
	};

	let next_relative_argument = |pairs: &mut Pairs<Rule>, addr: u64, relative_address: u64| {
		pairs.next().map(|argument| {
			parse_argument(
				argument,
				&label_addrs,
				&cli.source,
				addr,
				Some(relative_address),
			)
		})
	};

	let next_machine_register_or_immediate = |pairs: &mut Pairs<Rule>, addr: u64| {
		pairs.next().map(|machine_register_or_immediate| {
			parse_machine_register_or_immediate(
				machine_register_or_immediate,
				&label_addrs,
				&cli.source,
				addr,
				None,
			)
		})
	};

	addr = 0;

	let mut output_file = match std::fs::File::create(output_path) {
		Ok(file) => file,
		Err(err) => {
			eprintln!("Failed to open output file: {}", err);
			exit(1)
		},
	};

	// now process each instruction
	for pair in pairs {
		match pair.as_rule() {
			Rule::directive => {
				let dir = pair.into_inner().next().unwrap();
				let mut dir_pairs = dir.clone().into_inner();

				match dir.as_rule() {
					Rule::directive_addr => {
						let _name = dir_pairs.next().unwrap();
						let new_addr = dir_pairs
							.next()
							.map(|immediate| {
								evaluate_immediate(immediate, &label_addrs, &cli.source, addr, None)
							})
							.unwrap();

						addr = new_addr;
					},
					Rule::directive_write => {
						let size = next_instr_size(&mut dir_pairs).unwrap();
						let mut immediate_count = 0u64;

						for dir_pair in dir_pairs {
							let val =
								evaluate_immediate(dir_pair, &label_addrs, &cli.source, addr, None);
							let val = truncate_immediate(val, size.byte_size() * 8, false);
							let bytes = val.to_le_bytes();
							match output_file.write_all_at(
								addr + (immediate_count * size.byte_size() as u64),
								&bytes[0..size.byte_size() as usize],
							) {
								Ok(_) => {},
								Err(err) => {
									eprintln!("Failed to write to output file: {}", err);
									exit(1);
								},
							}
							immediate_count += 1;
						}

						addr += (size.byte_size() as u64) * immediate_count;
					},
					Rule::directive_def => {},
					_ => unreachable!(),
				}

				continue;
			},
			Rule::instr => { /* handled below */ },
			_ => continue,
		}

		// align it to instruction size
		addr = (addr + 3) & !3u64;
		// advance by one instruction
		addr += 4;

		let mut write_instruction = |encoded: u32| {
			let bytes = encoded.to_le_bytes();
			match output_file.write_all_at(addr - 4, &bytes) {
				Ok(_) => {},
				Err(err) => {
					eprintln!("Failed to write to output file: {}", err);
					exit(1);
				},
			}
		};

		let instr = pair.into_inner().next().unwrap();
		let mut instr_pairs = instr.clone().into_inner();

		instructions! {
			pushs[.s] a:reg | null               => [1101110000000000000000000ssaaaaa];
			pushp[.s] a:reg | null, b:reg | null => [11011000000000000000ssaaaaabbbbb];
			pops[.s]  a:reg | null               => [1101010000000000000000000ssaaaaa];
			popp[.s]  a:reg | null, b:reg | null => [1101000000000000000000ssaaaabbbb];

			lds[.s] d:reg, a:reg        => [1100110000000000000000ssddddaaaa];
			ldp[.s] d:reg, e:reg, a:reg => [110010000000000000ssddddeeeeaaaa];
			sts[.s] a:reg, b:reg        => [1100010000000000000000ssaaaabbbb];
			stp[.s] a:reg, b:reg, c:reg => [110000000000000000ssaaaabbbbcccc];

			ldi d:reg, a:imm16, [b:imm6], [c:imm2] => [1110ccaaaaaaaaaaaaaaaabbbbbbdddd];

			ldr d:reg, a:rel22(next_byte_relative_immediate) => [001100ddddaaaaaaaaaaaaaaaaaaaaaa];

			copy[.s] d:reg, S:reg => [1010100000000000000000ssddddSSSS];

			add_reg = add[.s] d:reg | null, a:reg, b:reg,                        [c:bool], [f:bool] => [101001000000000sscfdddddaaaabbbb];
			add_imm = add[.s] d:reg | null, a:reg, b:imm11, [S:imm3], [A: bool], [c:bool], [f:bool] => [1011sscfdddddaaaaASSSbbbbbbbbbbb];
			sub_reg = sub[.s] d:reg | null, a:reg, b:reg,                        [B:bool], [f:bool] => [101000000000000ssBfdddddaaaabbbb];
			sub_imm = sub[.s] d:reg | null, a:reg, b:imm11, [S:imm3], [A: bool], [B:bool], [f:bool] => [1001ssBfdddddaaaaASSSbbbbbbbbbbb];

			mul d:reg, a:reg, b:reg, [S:bool], [f:bool] => {
				d: reg, a: reg, b: reg, S: bool, f: bool {
					let s = size_of(a, b);
					let t = size_of(d);
				} => [10001100000000ssttSfddddaaaabbbb],
			};

			div[.s] d:reg, r:reg, a:reg, b:reg, [S:bool], [f:bool] => [100010000000ssSfddddrrrraaaabbbb];

			and_reg = and[.s] d:reg | null, a:reg, b:reg,              [f:bool] => [1000010000000000ssfdddddaaaabbbb];
			and_imm = and[.s] d:reg | null, a:reg, b:imm11, [S: imm3], [f:bool] => [100000ssfdddddaaaabbbbbbbbbbbSSS];
			or_reg  = or[.s]  d:reg | null, a:reg, b:reg,              [f:bool] => [0111110000000000ssfdddddaaaabbbb];
			or_imm  = or[.s]  d:reg | null, a:reg, b:imm11, [S: imm3], [f:bool] => [011110ssfdddddaaaabbbbbbbbbbbSSS];
			xor_reg = xor[.s] d:reg | null, a:reg, b:reg,              [f:bool] => [0111010000000000ssfdddddaaaabbbb];
			xor_imm = xor[.s] d:reg | null, a:reg, b:imm11, [S: imm3], [f:bool] => [011100ssfdddddaaaabbbbbbbbbbbSSS];

			shl[.s] d:reg | null, a:reg, b:reg | imm7, [f: bool] => {
				d: reg | null, a: reg, b: reg,  f: bool => [0110100000000000ssfdddddaaaabbbb],
				d: reg | null, a: reg, b: imm7, f: bool => [0110110000000ssfdddddaaaabbbbbbb],
			};
			shr[.s] d:reg | null, a:reg, b:reg | imm7, [A:bool], [f:bool] => {
				d: reg | null, a: reg, b: reg,  A: bool, f: bool => [011000000000000ssAfdddddaaaabbbb],
				d: reg | null, a: reg, b: imm7, A: bool, f: bool => [011001000000ssAfdddddaaaabbbbbbb],
			};
			rot[.s] d:reg | null, a:reg, b:reg | imm7, [f: bool] => {
				d: reg | null, a: reg, b: reg,  f: bool => [0101100000000000ssfdddddaaaabbbb],
				d: reg | null, a: reg, b: imm7, f: bool => [0101110000000ssfdddddaaaabbbbbbb],
			};

			neg[.s]   d:reg, a:reg, [f: bool] => [010101000000000000000ssfddddaaaa];
			bswap[.s] d:reg, a:reg, [f: bool] => [010100000000000000000ssfddddaaaa];

			jmpa[.c] a:reg => [010011000000000000000000ccccaaaa];
			jmpr[.c] a:reg | rel22 => {
				a: reg   => [010010000000000000000000ccccaaaa],
				a: rel22 => [010001ccccaaaaaaaaaaaaaaaaaaaaaa],
			};

			cjmpa.c[.s] a:reg, b:reg, C:reg => [001111000000000cccssaaaabbbbCCCC];
			cjmpr.c[.s] a:reg | rel13, b:reg, C:reg => {
				a: reg,   b: reg, C: reg => [001101000000000cccssaaaabbbbCCCC],
				a: rel13, b: reg, C: reg => [001011cccssbbbbCCCCaaaaaaaaaaaaa],
			};

			calla[.c] a:reg => [001010000000000000000000ccccaaaa];
			callr[.c] a:reg | rel22 => {
				a: reg   => [001001000000000000000000ccccaaaa],
				a: rel22 => [001000ccccaaaaaaaaaaaaaaaaaaaaaa],
			};

			ret         => [00011100000000000000000000000000];
			eret        => [00011000000000000000000000000000];
			udf         => [00000000000000000000000000000000];
			dbg         => [00001000000000000000000000000000];
			exc a:imm16 => [0000110000000000aaaaaaaaaaaaaaaa];
			nop         => [00000100000000000000000000000000];

			ldm d:reg, a:imm22(next_machine_register_or_immediate) => [000100ddddaaaaaaaaaaaaaaaaaaaaaa];
			stm d:imm22(next_machine_register_or_immediate), a:reg => [000101aaaadddddddddddddddddddddd];
		}
	}
}
