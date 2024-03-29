//
// Copyright (C) 2023 Ariel Abreu
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

#include <stdint.h>
#include <acca/acca.h>

#define MREG_VM_CONSOLE 0xdead1

void print_char(char character) {
	__asm__ __volatile__("stm %0, %1" :: "i" (MREG_VM_CONSOLE), "r" ((uint64_t)character));
};

void print_string(const char* string) {
	while (*string != '\0')
		print_char(*(string++));
};

__attribute__((noreturn))
void main(void) {
	print_string("Hello, world!\n");
	while (1) {}
};

__attribute__((section(".text.start")))
__attribute__((naked))
__attribute__((noreturn))
void start(void) {
	// load some arbitrary address into the stack pointer register
	// (clear register before, then load 0x0100 << 16 == 0x0100_0000)
	// note that this is the *top* of the stack; we consider the stack
	// to be from 0x0100_0000 to 0x00ff_0000 (64KiB)
	__asm__ __volatile__("ldi rsp, 0x0100, 16, 3");

	// jump to the actual main function
	__asm__ __volatile__("jmpr _main");
};
