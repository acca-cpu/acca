//
// Copyright (C) 2022 Ariel Abreu
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate litrs;
extern crate proc_macro2;

use std::{collections::HashMap, iter::Peekable, mem::swap};

use litrs::IntegerLit;
use proc_macro::{Delimiter, Spacing, TokenStream, TokenTree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modifier {
	Absent = 0,
	Optional = 1,
	Required = 2,
}

#[derive(Debug, Clone, Copy)]
enum ArgumentType {
	Register,
	NullableRegister,
	Immediate(u64),
	RelativeImmediate(u64),
	Boolean,
	RegisterOrImmediate(u64),
	RegisterOrRelativeImmediate(u64),
}

#[derive(Debug, Clone)]
struct Argument {
	name: String,
	ty: ArgumentType,
	default: Option<u64>,
	consumer_function: Option<proc_macro2::TokenStream>,
}

impl Argument {
	fn source_name(&self) -> proc_macro2::Ident {
		format_ident!("{}", self.name)
	}
}

fn instruction_helper(item: TokenStream) -> (proc_macro2::TokenStream, proc_macro2::Ident) {
	let all_tokens: Vec<_> = item.into_iter().collect();

	if all_tokens.len() == 0 {
		panic!("Expected an instruction name");
	}

	let first_span: proc_macro2::Span = all_tokens.first().unwrap().span().into();
	let last_span: proc_macro2::Span = all_tokens.last().unwrap().span().into();
	let source_span = first_span
		.join(last_span)
		.unwrap_or(proc_macro2::Span::call_site());

	let mut iter = all_tokens.into_iter().peekable();
	let source_name = iter
		.next()
		.expect("Instruction must have a name")
		.to_string();
	let mut condition_mod = Modifier::Absent;
	let mut size_mod = Modifier::Absent;
	let mut needs_comma = false;
	let mut args: Vec<Argument> = Vec::new();
	let mut instruction_name = source_name.clone();

	match iter.peek() {
		Some(TokenTree::Punct(punct))
			if punct.as_char() == '=' && punct.spacing() == Spacing::Alone =>
		{
			// consume the punctuation
			iter.next();

			instruction_name = iter
				.next()
				.expect("Instruction should have a name after the equal sign")
				.to_string();
		},
		_ => {},
	}

	let mut handle_mod_tok = |iter: &mut Peekable<std::vec::IntoIter<_>>, mod_state| -> () {
		// consume the punctuation
		iter.next();

		match iter.next() {
			Some(TokenTree::Ident(ident)) => {
				let id_str = ident.to_string();

				match id_str.as_str() {
					"c" => {
						if condition_mod != Modifier::Absent {
							panic!("Conditional modifier specified multiple times");
						}

						condition_mod = mod_state;
					},

					"s" => {
						if size_mod != Modifier::Absent {
							panic!("Size modifier specified multiple times");
						}

						size_mod = mod_state;
					},

					_ => panic!("Unknown modifier \"{}\"", id_str),
				}
			},

			_ => panic!("Modifier should have an identifier specifying the type of modifier"),
		}
	};

	while let Some(maybe_cond) = iter.peek() {
		match maybe_cond {
			TokenTree::Group(group) => match group.delimiter() {
				Delimiter::Bracket => {
					let mut iter2 = group
						.stream()
						.into_iter()
						.collect::<Vec<_>>()
						.into_iter()
						.peekable();
					if let Some(tok) = iter2.peek() {
						match tok {
							TokenTree::Punct(punct) => {
								if punct.as_char() == '.' {
									handle_mod_tok(&mut iter2, Modifier::Optional);
									// consume the group
									iter.next();
								} else {
									break;
								}
							},
							_ => break,
						}
					} else {
						break;
					}
				},
				_ => break,
			},
			TokenTree::Punct(punct) => {
				if punct.as_char() == '.' {
					handle_mod_tok(&mut iter, Modifier::Required);
				} else {
					break;
				}
			},
			_ => break,
		}
	}

	let mut handle_tok = |iter: &mut Peekable<std::vec::IntoIter<_>>,
	                      grouped: bool,
	                      needs_comma: &mut bool|
	 -> () {
		if *needs_comma {
			*needs_comma = false;
			match iter.next() {
				Some(TokenTree::Punct(punct)) if punct.as_char() == ',' => {},
				_ => panic!("Argument should be separated be a comma"),
			}
		}

		let argname = match iter.next() {
			Some(TokenTree::Ident(ident)) => ident.to_string(),
			_ => panic!("Argument should have an identifier naming it"),
		};

		match iter.next() {
			Some(TokenTree::Punct(punct)) if punct.as_char() == ':' => {},
			_ => panic!("Argument should have a colon-separated type"),
		}

		let typestring = match iter.next() {
			Some(TokenTree::Ident(ident)) => ident.to_string(),
			_ => panic!("Argument should have an identifier specifying its type"),
		};

		let mut argtype = if typestring.starts_with("imm") || typestring.starts_with("rel") {
			match (&typestring[3..]).parse::<u64>() {
				Ok(width) if typestring.starts_with("imm") => ArgumentType::Immediate(width),
				Ok(width) => ArgumentType::RelativeImmediate(width),
				Err(_) => panic!("Invalid immediate width"),
			}
		} else {
			match typestring.as_str() {
				"reg" => ArgumentType::Register,
				"bool" => ArgumentType::Boolean,
				_ => panic!("Invalid type"),
			}
		};

		while let Some(tok) = iter.peek() {
			match tok {
				TokenTree::Punct(punct) if punct.as_char() == '|' => {
					// consume the punctuation
					iter.next();

					let alt_typestring = match iter.next() {
						Some(TokenTree::Ident(ident)) => ident.to_string(),
						_ => panic!("Alternative type specifier (\"|\" a.k.a. pipe) should be followed by an identifier for the alternative type"),
					};

					argtype = if alt_typestring.as_str() == "null" {
						match argtype {
							ArgumentType::Register => ArgumentType::NullableRegister,
							_ => panic!(
								"Invalid null alternative: only registers can be made nullable"
							),
						}
					} else if alt_typestring.starts_with("imm") || alt_typestring.starts_with("rel")
					{
						match argtype {
							ArgumentType::Register => match (&alt_typestring[3..]).parse::<u64>() {
								Ok(width) if alt_typestring.starts_with("imm") => ArgumentType::RegisterOrImmediate(width),
								Ok(width) => ArgumentType::RegisterOrRelativeImmediate(width),
								Err(_) => panic!("Invalid immediate width"),
							},
							_ => panic!("Cannot specify immediate type as alternative for non-register type"),
						}
					} else {
						match argtype {
							ArgumentType::Immediate(width) | ArgumentType::RelativeImmediate(width) => match typestring.as_str() {
								"reg" if matches!(argtype, ArgumentType::Immediate(_)) => ArgumentType::RegisterOrImmediate(width),
								"reg" => ArgumentType::RegisterOrRelativeImmediate(width),
								_ => panic!("Invalid alternative type for immediate (only registers are allowed)"),
							}
							_ => panic!("Cannot specify non-immediate alternative type for non-immediate type"),
						}
					}
				},
				_ => break,
			}
		}

		let mut consume_func: Option<proc_macro2::TokenStream> = None;

		match iter.peek() {
			Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Parenthesis => {
				consume_func = Some(group.stream().into());

				// consume the group
				iter.next();
			},
			_ => {},
		}

		let default = if grouped {
			match iter.peek() {
				Some(TokenTree::Punct(punct))
					if punct.as_char() == '=' && punct.spacing() == Spacing::Alone =>
				{
					// consume the punctuation
					iter.next();

					Some(match iter.next() {
						Some(TokenTree::Literal(lit)) => IntegerLit::parse(lit.to_string())
							.ok()
							.and_then(|intlit| intlit.value::<u64>())
							.expect("Default value should be an integer literal"),
						_ => panic!("Invalid default value"),
					})
				},
				_ => Some(0),
			}
		} else {
			None
		};

		if grouped && default.is_none() {
			panic!("Optional argument needs to have default value");
		}

		args.push(Argument {
			name: argname,
			ty: argtype,
			default,
			consumer_function: consume_func,
		});

		// consume a comma if one exists
		match iter.peek() {
			Some(TokenTree::Punct(punct)) if punct.as_char() == ',' => {
				*needs_comma = false;
				// consume the comma
				iter.next();
			},
			_ => *needs_comma = true,
		}
	};

	while let Some(tok) = iter.peek() {
		match tok {
			TokenTree::Group(group) => {
				if group.delimiter() != Delimiter::Bracket {
					break;
				}

				handle_tok(
					&mut group
						.stream()
						.into_iter()
						.collect::<Vec<_>>()
						.into_iter()
						.peekable(),
					true,
					&mut needs_comma,
				);

				// consume the group
				iter.next();

				// consume a comma if one exists
				match iter.peek() {
					Some(TokenTree::Punct(punct)) if punct.as_char() == ',' => {
						needs_comma = false;
						// consume the comma
						iter.next();
					},
					_ => needs_comma = true,
				}
			},
			TokenTree::Ident(_) => handle_tok(&mut iter, false, &mut needs_comma),
			TokenTree::Punct(_) => break,
			_ => panic!("Unexpected token"),
		}
	}

	let size_ident = if size_mod != Modifier::Absent {
		quote_spanned!(source_span=> s)
	} else {
		quote_spanned!(source_span=>)
	};
	let cond_ident = if condition_mod != Modifier::Absent {
		quote_spanned!(source_span=> c)
	} else {
		quote_spanned!(source_span=>)
	};

	let default_pattern = args.iter().map(|arg| {
		let type_toks = match arg.ty {
			ArgumentType::Register => quote_spanned!(source_span=> reg),
			ArgumentType::NullableRegister => quote_spanned!(source_span=> reg | null),
			ArgumentType::Immediate(width) => {
				let ident = format_ident!("imm{}", width);
				quote_spanned!(source_span=> #ident)
			},
			ArgumentType::RelativeImmediate(width) => {
				let ident = format_ident!("rel{}", width);
				quote_spanned!(source_span=> #ident)
			},
			ArgumentType::Boolean => quote_spanned!(source_span=> bool),
			ArgumentType::RegisterOrImmediate(width) => {
				let ident = format_ident!("imm{}", width);
				quote_spanned!(source_span=> reg | #ident)
			},
			ArgumentType::RegisterOrRelativeImmediate(width) => {
				let ident = format_ident!("rel{}", width);
				quote_spanned!(source_span=> reg | #ident)
			},
		};

		let name = arg.source_name();

		quote_spanned!(source_span=> #name: #type_toks)
	});

	let body = match iter.peek() {
		Some(TokenTree::Punct(punct))
			if punct.as_char() == '=' && punct.spacing() == Spacing::Joint =>
		{
			// consume the punctuation
			iter.next();

			match iter.next() {
				Some(TokenTree::Punct(punct2)) if punct2.as_char() == '>' => {},
				_ => panic!("Expected \"=>\""),
			}

			match iter.next() {
				Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Bracket => {
					let stream: proc_macro2::TokenStream = group.stream().into();

					quote_spanned!(source_span=> instruction_body! { #(#default_pattern),* ; #size_ident #cond_ident [ #stream ] })
				},
				Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => {
					let stream: proc_macro2::TokenStream = group.stream().into();

					quote_spanned!(source_span=> instruction_body! { #size_ident #cond_ident { #stream }})
				},
				_ => panic!("Expected bracketed encoding"),
			}
		},
		_ => quote_spanned!(source_span=>),
	};

	if iter.peek().is_some() {
		panic!("Expected end-of-stream");
	}

	let source_ident = format_ident!("instr_{}", source_name);

	let mod_parse = if condition_mod != Modifier::Absent {
		if size_mod != Modifier::Absent {
			quote_spanned! {source_span=> let (cond, size) = next_instr_condition_and_size(&mut instr_pairs); }
		} else {
			quote_spanned! {source_span=> let cond = next_instr_condition(&mut instr_pairs); }
		}
	} else if size_mod != Modifier::Absent {
		quote_spanned! {source_span=> let size = next_instr_size(&mut instr_pairs); }
	} else {
		quote_spanned! {source_span=> let _name = instr_pairs.next().unwrap(); }
	};

	let cond_unwrap = if condition_mod == Modifier::Required {
		quote_spanned! {source_span=>
			let cond = cond.unwrap();
			let c = cond as u64;
		}
	} else if condition_mod == Modifier::Optional {
		quote_spanned! {source_span=>
			let c = match cond {
				Some(op_cond) => op_cond as u64,
				None => 31u64,
			};
		}
	} else {
		quote_spanned! {source_span=>}
	};

	let args_as_call_args = args.iter().map(|arg| {
		let arg_source_name = arg.source_name();
		quote_spanned!(source_span=> #arg_source_name)
	});

	let size_unwrap = if size_mod == Modifier::Required {
		quote_spanned! {source_span=>
			let size = size.unwrap();
			let s = common_register_size(&[#(#args_as_call_args),*], Some(size)) as u64;
		}
	} else if size_mod == Modifier::Optional {
		quote_spanned! {source_span=>
			let s = common_register_size(&[#(#args_as_call_args),*], size) as u64;
		}
	} else {
		quote_spanned! {source_span=>}
	};

	let arg_defs = args.iter().map(|arg| {
		let arg_source_name = arg.source_name();

		let mut arg_expr = {
			let consumed = match &arg.consumer_function {
				Some(consumer) => quote_spanned!(source_span=> #consumer(&mut instr_pairs)),
				None => match arg.ty {
					ArgumentType::Register => {
						quote_spanned!(source_span=> next_register(&mut instr_pairs))
					},
					ArgumentType::NullableRegister => {
						quote_spanned!(source_span=> next_register_or_null(&mut instr_pairs))
					},
					ArgumentType::Immediate(_) => {
						quote_spanned!(source_span=> next_immediate(&mut instr_pairs))
					},
					ArgumentType::RelativeImmediate(_) => {
						quote_spanned!(source_span=> next_relative_immediate(&mut instr_pairs, addr))
					},
					ArgumentType::Boolean => {
						quote_spanned!(source_span=> next_immediate(&mut instr_pairs))
					},
					ArgumentType::RegisterOrImmediate(_) => {
						quote_spanned!(source_span=> next_argument(&mut instr_pairs))
					},
					ArgumentType::RegisterOrRelativeImmediate(_) => {
						quote_spanned!(source_span=> next_relative_argument(&mut instr_pairs, addr))
					},
				}
			};
			match arg.ty {
				ArgumentType::Register => {
					quote_spanned!(source_span=> #consumed.map(|reg| Argument::Register(reg)))
				},
				ArgumentType::NullableRegister => {
					quote_spanned!(source_span=> #consumed.map(|reg| Argument::Register(reg)))
				},
				ArgumentType::Immediate(width) => {
					quote_spanned!(source_span=> #consumed.map(|imm| Argument::Immediate(truncate_immediate(imm, #width as u8, false))))
				},
				ArgumentType::RelativeImmediate(width) => {
					quote_spanned!(source_span=> #consumed.map(|imm| Argument::Immediate(truncate_immediate(imm, #width as u8, true))))
				},
				ArgumentType::Boolean => {
					quote_spanned!(source_span=> #consumed.map(|imm| Argument::Immediate(imm)))
				},
				ArgumentType::RegisterOrImmediate(width) => {
					quote_spanned!(source_span=> #consumed.map(|arg| arg.map_immediate(|imm| truncate_immediate(imm, #width as u8, false))))
				},
				ArgumentType::RegisterOrRelativeImmediate(width) => {
					quote_spanned!(source_span=> #consumed.map(|arg| arg.map_immediate(|imm| truncate_immediate(imm, #width as u8, true))))
				},
			}
		};

		arg_expr = match arg.default {
			Some(def) => quote_spanned!(source_span=> #arg_expr.or(Some(Argument::Immediate(#def)))),
			None => arg_expr,
		};

		let do_unwrap = match arg.default {
			None if !matches!(arg.ty, ArgumentType::NullableRegister) => quote_spanned!(source_span=> #arg_source_name.expect("Argument should be present because it is required");),
			_ => quote_spanned!(source_span=>),
		};

		quote_spanned! {source_span=>
			let #arg_source_name = #arg_expr;
			#do_unwrap
		}
	});

	let cond_format = if condition_mod != Modifier::Absent {
		"; cond = {:?}"
	} else {
		""
	};

	let size_format = if size_mod != Modifier::Absent {
		"; size = {:?}"
	} else {
		""
	};

	let cond_print = if condition_mod != Modifier::Absent {
		quote_spanned!(source_span=> , cond)
	} else {
		quote_spanned!(source_span=>)
	};

	let size_print = if size_mod != Modifier::Absent {
		quote_spanned!(source_span=> , size)
	} else {
		quote_spanned!(source_span=>)
	};

	let arg_prints = args.iter().map(|arg| {
		let source_name = arg.source_name();
		quote_spanned!(source_span=> #source_name)
	});

	let arg_print_format = args.iter().fold(String::new(), |acc, arg| {
		format!(
			"{}{}{} = {{:?}}",
			acc,
			if acc.len() == 0 { " " } else { ", " },
			arg.name
		)
	});

	(
		quote_spanned! {source_span=>
			{
				#mod_parse
				#(#arg_defs)*
				#cond_unwrap
				#size_unwrap

				println!(concat!("instruction: ", #instruction_name, "(", #source_name, #cond_format, #size_format, ")", #arg_print_format) #cond_print #size_print, #(#arg_prints),*);

				#body
			}
		}.into(),
		source_ident,
	)
}

#[proc_macro]
pub fn instruction(item: TokenStream) -> TokenStream {
	instruction_helper(item).0.into()
}

#[proc_macro]
pub fn instructions(item: TokenStream) -> TokenStream {
	let mut tokens: Vec<_> = item.into_iter().collect();
	let mut instruction_sequences: Vec<Vec<TokenTree>> = Vec::new();

	loop {
		let idx = tokens
			.iter()
			.position(|tok| matches!(tok, TokenTree::Punct(punct) if punct.as_char() == ';'));

		if idx.is_none() {
			if tokens.len() > 0 {
				instruction_sequences.push(tokens);
			}
			break;
		}

		let idx = idx.unwrap();

		let mut rest = tokens.split_off(idx);
		rest.remove(0);

		instruction_sequences.push(tokens);
		tokens = rest;
	}

	let instr = instruction_sequences.into_iter().map(|sequence| {
		let (instr, source_ident) =
			instruction_helper(sequence.into_iter().collect::<TokenStream>());
		quote! {
			Rule::#source_ident => #instr
		}
	});

	quote! {
		match instr.as_rule() {
			#(#instr),*

			Rule::instr_unknown => {
				let loc = instr.line_col();
				println!(
					"Error: encountered unknown instruction \"{}\": {}:{}:{}",
					instr.into_inner().next().unwrap().as_str(),
					cli.source.display(),
					loc.0,
					loc.1
				);
				exit(1);
			},

			_ => unreachable!(),
		}
	}
	.into()
}

#[proc_macro]
pub fn instruction_encoding(item: TokenStream) -> TokenStream {
	let mut all_tokens: Vec<_> = item.into_iter().collect();

	let first_span: proc_macro2::Span = all_tokens.first().unwrap().span().into();
	let last_span: proc_macro2::Span = all_tokens.last().unwrap().span().into();
	let source_span = first_span
		.join(last_span)
		.unwrap_or(proc_macro2::Span::call_site());

	let split_pos = all_tokens
		.iter()
		.position(|tok| matches!(tok, TokenTree::Punct(punct) if punct.as_char() == ';'))
		.expect("Instruction encoding should have a \";\" separator");

	let mut idents = all_tokens.split_off(split_pos);

	swap(&mut all_tokens, &mut idents);

	// remove the semicolon
	all_tokens.remove(0);

	let var_idents: HashMap<char, proc_macro2::TokenStream> = idents
		.into_iter()
		.map(|tok| match &tok {
			TokenTree::Ident(ident) => {
				let as_str = ident.to_string();
				if as_str.len() != 1 {
					panic!("Invalid identifier in encoding");
				}
				(
					as_str.chars().next().unwrap(),
					proc_macro::TokenStream::from(tok).into(),
				)
			},
			_ => panic!("Invalid token in identifier list \"{:?}\"", tok),
		})
		.collect();

	let iter = all_tokens.into_iter();
	let mut string = String::new();

	for tok in iter {
		match tok {
			TokenTree::Literal(lit) => {
				string.push_str(&lit.to_string());
			},
			TokenTree::Ident(ident) => {
				string.push_str(&ident.to_string());
			},
			_ => {},
		}
	}

	if string.chars().count() != 32 {
		panic!("Expected 32 characters for the encoding");
	}

	let mut var_indices: HashMap<char, u8> = HashMap::new();

	let mut result = quote_spanned!(source_span=> 0u32);

	for (idx, char) in string.chars().rev().enumerate() {
		let bit_idx = idx as u32;

		if char == '0' {
			// nothing
		} else if char == '1' {
			result = quote_spanned! {source_span=>
				(
					#result |
					(1u32 << #bit_idx)
				)
			};
		} else if char.is_ascii_alphabetic() {
			let var_idx = var_indices.entry(char).or_insert(0);
			let var = var_idents.get(&char).expect("Unknown identifier");
			result = quote_spanned! {source_span=>
				(
					#result |
					(
						(
							(
								(#var as u32) & (1u32 << #var_idx)
							)
							>> #var_idx
						)
						<< #bit_idx
					)
				)
			};
			*var_idx += 1;
		} else {
			panic!("Invalid character \"{}\" in encoding", char);
		}
	}

	quote_spanned! {source_span=>
		println!("encoding: {:#034b}", #result);
	}
	.into()
}
