//
// Copyright (C) 2023 Ariel Abreu
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

#include "acca/exceptions.h"
#include <stdint.h>
#include <acca/acca.h>

#define MREG_VM_CONSOLE 0xdead1

__attribute__((section(".data.econfig")))
acca_evt_t global_evt;

__attribute__((section(".data.econfig")))
acca_ect_t global_ect;

void print_char(char character) {
	__asm__ __volatile__("stm %0, %1" :: "i" (MREG_VM_CONSOLE), "r" ((uint64_t)character));
};

void print_string(const char* string) {
	while (*string != '\0')
		print_char(*(string++));
};

void print_u64(uint64_t value, uint8_t base) {
	char digits[32];
	char* curr = &digits[0];

	if (value == 0) {
		print_char('0');
		return;
	}

	while (value > 0) {
		char digit = value % base;
		value /= base;

		*(curr++) = digit + '0';
	}

	do {
		--curr;
		print_char(*curr);
	} while (curr != &digits[0]);
};

EXCEPTION_HANDLER(handle_exc_pl0_user) {
	acca_einfo_t einfo = acca_read_einfo();

	if (acca_einfo_get_type(einfo) != acca_etype_user) {
		__builtin_unreachable();
	}

	uint16_t user_val = acca_einfo_get_user_value(einfo);

	print_string("Got user exception: ");
	print_u64(user_val, 10);
	print_string("/0x");
	print_u64(user_val, 16);
	print_char('\n');
};

__attribute__((noreturn))
void main(void) {
	INIT_EVT_ENTRY(&global_evt, 0, acca_etype_user, handle_exc_pl0_user);

	print_string("Hello, world!\n");
	__asm__ __volatile__("exc 0x1234");
	print_string("After exc\n");

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

	// set up the evt and ect
	__asm__ __volatile__(
		"stm mreg.evtable, %0\n"
		"stm mreg.ectable, %1\n"
		:: "m" (global_evt), "m" (global_ect)
	);

	// jump to the actual main function
	__asm__ __volatile__("jmpr _main");
};
