//
// Copyright (C) 2023 Ariel Abreu
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

mod util;
mod vm;

use std::{fs, path::PathBuf, process::exit};

use clap::Parser as ClapParser;

#[derive(ClapParser)]
#[command(author, version, about, long_about = None)]
struct Args {
	image: PathBuf,

	#[arg(long)]
	print_instructions: bool,
}

const VM_MEMORY_SIZE: usize = /* 32MiB */ 32 * 1024 * 1024;

fn main() {
	let cli = Args::parse();
	let mut vm = match vm::VM::new(VM_MEMORY_SIZE) {
		Some(vm) => vm,
		None => {
			eprintln!("Failed to create VM");
			exit(1);
		},
	};

	// read the input image into memory
	{
		let mut file = match fs::File::open(&cli.image) {
			Ok(x) => x,
			Err(e) => {
				eprintln!("Failed to open \"{}\": {}", cli.image.display(), e);
				exit(1);
			},
		};

		match vm.load_file(&mut file, 0.into()) {
			Ok(_) => {},
			Err(e) => {
				eprintln!(
					"Failed to load file \"{}\" into VM memory: {}",
					cli.image.display(),
					e
				);
				exit(1);
			},
		}
	}

	vm.set_print_instructions(cli.print_instructions);

	vm.run()
}
