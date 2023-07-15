//
// Copyright (C) 2023 Ariel Abreu
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

mod util;

use std::collections::HashMap;

use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use quote::quote;
use syn::{
	bracketed,
	parse::{Parse, ParseStream},
	parse_macro_input,
	spanned::Spanned,
	Token,
};

use crate::util::CollectIntoArray;

mod kw {
	use proc_macro2::{Ident, Span};
	use syn::{custom_keyword, parse::Parse};

	custom_keyword!(reg);
	custom_keyword!(bool);
	custom_keyword!(null);
	custom_keyword!(size);
	custom_keyword!(cond);

	#[allow(non_camel_case_types)]
	pub struct immrel {
		pub span: Span,
		pub width: u64,
		pub relative: std::primitive::bool,
	}

	impl Parse for immrel {
		fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
			let ident: Ident = input.parse()?;
			let ident_str = ident.to_string();
			let relative = ident_str.starts_with("rel");

			if !relative && !ident_str.starts_with("imm") {
				return Err(syn::Error::new(
					ident.span(),
					"Expected an identifier starting with \"imm\" or \"rel\"",
				));
			}

			let width: u64 = match (&ident_str[3..]).parse() {
				Ok(result) => result,
				Err(_) => {
					return Err(syn::Error::new(
						ident.span(),
						"Expected an identifier ending with an integer",
					))
				},
			};

			Ok(Self {
				span: ident.span(),
				width,
				relative,
			})
		}
	}
}

#[derive(Debug, Clone, Copy)]
enum InstructionBit {
	Zero,
	One,
	Variable(char),
}

#[derive(Debug, Clone, Copy)]
enum ParameterType {
	Register,
	NullableRegister,
	Immediate(u64),
	RelativeImmediate(u64),
	Boolean,
	Size,
	Condition,
	NullableCondition,
}

#[derive(Debug, Clone)]
struct Parameter {
	encoding_name: char,
	ty: ParameterType,
	source_name: Ident,
	encoding_name_span: Span,
}

#[derive(Debug, Clone)]
struct Instruction {
	name: Ident,
	encoding_span: Span,
	bits: [InstructionBit; 32],
	parameters: HashMap<char, Parameter>,
}

struct InstructionWithBody {
	instruction: Instruction,
	body: TokenTree,
}

struct InstructionsWithBodies {
	instructions: Vec<InstructionWithBody>,
	default_case: TokenTree,
}

impl Parse for InstructionsWithBodies {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut instructions = Vec::new();
		let default_case;

		loop {
			if input.peek(Token![_]) {
				input.parse::<Token![_]>()?;
				input.parse::<Token![=>]>()?;
				default_case = input.parse()?;
				if input.peek(Token![,]) {
					input.parse::<Token![,]>()?;
				}
				break;
			}

			instructions.push(input.parse()?);

			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			}
		}

		Ok(Self {
			instructions,
			default_case,
		})
	}
}

impl Parse for InstructionWithBody {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
			instruction: input.parse()?,
			body: input.parse()?,
		})
	}
}

impl Parse for Instruction {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let encoding;
		let encoding_brackets = bracketed!(encoding in input);

		let mut encoding_str = String::with_capacity(32);

		while let Ok(item) = encoding.parse::<TokenTree>() {
			match item {
				TokenTree::Ident(_) | TokenTree::Literal(_) => {
					let as_str = item.to_string();
					let invalid_char_count = as_str
						.chars()
						.filter(|&char| !char.is_ascii_alphabetic() && char != '0' && char != '1')
						.count();
					if invalid_char_count != 0 {
						return syn::Result::Err(syn::Error::new(
							item.span(),
							"Expected a sequence of 0's, 1's, or single-character variables",
						));
					}
					encoding_str.push_str(&as_str);
				},
				_ => {
					return syn::Result::Err(syn::Error::new(
						item.span(),
						"Expected a sequence of 0's, 1's, or single-character variables",
					))
				},
			}
		}

		if encoding_str.chars().count() != 32 {
			return Err(syn::Error::new(
				encoding_brackets.span.span(),
				"Expected 32 characters in encoding",
			));
		}

		let bits: [_; 32] = encoding_str
			.chars()
			.rev()
			.map(|char| match char {
				'0' => InstructionBit::Zero,
				'1' => InstructionBit::One,
				'a'..='z' | 'A'..='Z' => InstructionBit::Variable(char),
				_ => unreachable!(),
			})
			.collect_into_array()
			.unwrap();

		input.parse::<Token![=>]>()?;

		let name: Ident = input.parse()?;

		let mut params = HashMap::new();

		while input.peek(syn::Ident) {
			let param: Parameter = input.parse()?;
			params.insert(param.encoding_name, param);
			if let Err(_) = input.parse::<Token![,]>() {
				// no trailing comma? no way to have another parameter.
				break;
			}
		}

		Ok(Self {
			name,
			encoding_span: encoding_brackets.span.span(),
			bits,
			parameters: params,
		})
	}
}

impl Parse for Parameter {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let source_name: Ident = input.parse()?;
		let mut encoding_name = source_name.to_string();
		let mut encoding_name_span = source_name.span();

		if input.peek(Token![=]) {
			input.parse::<Token![=]>().unwrap();
			let tmp = input.parse::<Ident>()?;
			encoding_name = tmp.to_string();
			encoding_name_span = tmp.span();
		}

		if encoding_name.chars().count() != 1 {
			return Err(syn::Error::new(
				encoding_name_span,
				"Expected a single character name for the encoding name",
			));
		}

		input.parse::<Token![:]>()?;

		Ok(Self {
			encoding_name: encoding_name.chars().next().unwrap(),
			ty: input.parse()?,
			source_name,
			encoding_name_span,
		})
	}
}

impl Parse for ParameterType {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		if input.peek(kw::bool) {
			input.parse::<kw::bool>()?;
			Ok(Self::Boolean)
		} else if input.peek(kw::size) {
			input.parse::<kw::size>()?;
			Ok(Self::Size)
		} else if input.peek(kw::cond) {
			input.parse::<kw::cond>()?;
			if input.peek(Token![|]) {
				input.parse::<Token![|]>()?;
				input.parse::<kw::null>()?;
				Ok(Self::NullableCondition)
			} else {
				Ok(Self::Condition)
			}
		} else if input.peek(kw::null) {
			input.parse::<kw::cond>()?;
			input.parse::<Token![|]>()?;
			input.parse::<kw::reg>()?;
			Ok(Self::NullableRegister)
		} else if input.peek(kw::reg) {
			input.parse::<kw::reg>()?;

			if input.peek(Token![|]) {
				input.parse::<Token![|]>()?;
				input.parse::<kw::null>()?;
				Ok(Self::NullableRegister)
			} else {
				Ok(Self::Register)
			}
		} else {
			let imm = input.parse::<kw::immrel>()?;
			if imm.relative {
				Ok(Self::RelativeImmediate(imm.width))
			} else {
				Ok(Self::Immediate(imm.width))
			}
		}
	}
}

#[proc_macro]
pub fn instructions(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let instructions = parse_macro_input!(item as InstructionsWithBodies);
	let mut result = quote!();

	let def = &instructions.default_case;

	for InstructionWithBody {
		instruction: instr,
		body,
	} in instructions.instructions
	{
		let mut required_mask = 0u32;
		let mut required_mask_value = 0u32;
		let mut var_bits: HashMap<char, (TokenStream, usize)> = HashMap::new();

		for (i, bit) in instr.bits.iter().enumerate() {
			match bit {
				InstructionBit::Zero | InstructionBit::One => {
					let bit_as_val = 1u32 << (i as u32);
					required_mask |= bit_as_val;
					if matches!(bit, InstructionBit::One) {
						required_mask_value |= bit_as_val;
					}
				},
				InstructionBit::Variable(name) => {
					let (stream, dest_bit) =
						var_bits.entry(*name).or_insert_with(|| (quote!(0u64), 0));
					let source_bit = i as u32;
					let source_mask = 1u32 << source_bit;
					let dest_bit_u64 = *dest_bit as u64;
					*stream = quote!((
						(#stream) |
						((((encoded & #source_mask) >> #source_bit) as u64) << #dest_bit_u64)
					));
					*dest_bit += 1;
				},
			}
		}

		for (id, _) in &var_bits {
			if !instr.parameters.contains_key(id) {
				return syn::Error::new(
					instr.encoding_span,
					format!(
						"Missing parameter annotation for encoding variable \"{}\"",
						id
					),
				)
				.to_compile_error()
				.into();
			}
		}

		for (id, param) in &instr.parameters {
			if !var_bits.contains_key(id) {
				return syn::Error::new(param.encoding_name_span, format!("Superfluous parameter annotation \"{}\" with no corresponding encoding variable", id)).to_compile_error().into();
			}
		}

		let vars = instr.parameters.iter().map(|(id, param)| {
			let param_source_name = &param.source_name;
			let bits_stream = var_bits.remove(id).unwrap().0;

			let val = match param.ty {
				ParameterType::Boolean => quote!((#bits_stream & 1) != 0),
				ParameterType::Condition => quote!(Condition::from(#bits_stream)),
				ParameterType::NullableCondition => quote! {
					match #bits_stream {
						bits @ 0..=9 => Some(Condition::from(bits)),
						15 => None,
						_ => unreachable!(),
					}
				},
				ParameterType::Immediate(_) => bits_stream,
				ParameterType::RelativeImmediate(width) => {
					quote!(sign_extend_immediate(#bits_stream, #width))
				},
				ParameterType::NullableRegister => quote! {
					match #bits_stream {
						reg @ 0..=15 => Some(RegisterID::from(reg)),
						31 => None,
						_ => unreachable!(),
					}
				},
				ParameterType::Register => quote!(RegisterID::from(#bits_stream)),
				ParameterType::Size => quote!(Size::from(#bits_stream)),
			};

			quote! {
				#[allow(non_snake_case)]
				let #param_source_name = #val;
			}
		});

		let instr_name = instr.name.to_string();

		result = quote! {
			#result
			_ if (encoded & #required_mask) == #required_mask_value => {
				if self.print_instructions {
					println!(concat!("{:#x} @ ", #instr_name), u64::from(self.instruction_pointer));
				}

				#(#vars)*
				#body
			},
		};
	}

	quote! {
		match encoded {
			#result
			_ => #def,
		}
	}
	.into()
}
