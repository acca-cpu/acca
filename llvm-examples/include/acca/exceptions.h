//
// Copyright (C) 2023 Ariel Abreu
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

#ifndef _ACCA_EXCEPTIONS_H_
#define _ACCA_EXCEPTIONS_H_

#include <stdint.h>
#include <stdbool.h>

#include <acca/base.h>

//
// types
//

typedef struct acca_evt_entry acca_evt_entry_t;
struct acca_evt_entry {
	uint32_t instructions[8];
};

typedef struct acca_evt acca_evt_t;
struct acca_evt {
	acca_evt_entry_t pl0[8];
	acca_evt_entry_t pl1[8];
};

typedef struct acca_ect_entry acca_ect_entry_t;
struct acca_ect_entry {
	uint64_t flags;
	uint64_t stack_pointer;
	uint64_t stack_size;
};

typedef struct acca_ect acca_ect_t;
struct acca_ect {
	acca_ect_entry_t pl0[8];
	acca_ect_entry_t pl1[8];
};

typedef uint8_t acca_etype_t;
enum acca_etype {
	acca_etype_unknown = 0,
	acca_etype_invalid_instruction = 1,
	acca_etype_debug = 2,
	acca_etype_user = 3,
	acca_etype_invalid_operation = 4,
	acca_etype_instruction_load_error = 5,
	acca_etype_data_load_error = 6,
	acca_etype_interrupt = 7,
};

typedef uint64_t acca_einfo_t;

//
// macros
//

#define EXCEPTION_HANDLER(_name) \
	void _name ## _ACTUAL(void); \
	__attribute__((section(".text.exc"))) \
	__attribute__((naked)) \
	void _name(void) { \
		__asm__ __volatile__( \
			"pushp.w  r0,  r1\n" \
			"pushp.w  r2,  r3\n" \
			"pushp.w  r4,  r5\n" \
			"pushp.w  r6,  r7\n" \
			"pushp.w  r8,  r9\n" \
			"pushp.w r10, r11\n" \
			"pushp.w r12, r13\n" \
			"pushp.w r14, r15\n" \
			"callr %0\n" \
			"popp.w r14, r15\n" \
			"popp.w r12, r13\n" \
			"popp.w r10, r11\n" \
			"popp.w  r8,  r9\n" \
			"popp.w  r6,  r7\n" \
			"popp.w  r4,  r5\n" \
			"popp.w  r2,  r3\n" \
			"popp.w  r0,  r1\n" \
			"eret\n" \
			:: "i" (_name ## _ACTUAL) \
		); \
	}; \
	void _name ## _ACTUAL(void)

#define INIT_EVT_ENTRY(_evt, _pl, _exc_type, _name) { \
		uint32_t* instr = &(_evt)->pl##_pl[_exc_type].instructions[0]; \
		*instr = (0b010001u << 26) | (0b1111 << 22) | (uint32_t)((((int64_t)_name - ((int64_t)instr + 4)) >> 2) & 0x3fffff); \
	}

ACCA_ALWAYS_INLINE
static acca_etype_t acca_einfo_get_type(acca_einfo_t einfo) {
	return einfo & 0x7;
};

ACCA_ALWAYS_INLINE
static uint16_t acca_einfo_get_user_value(acca_einfo_t einfo) {
	return (einfo >> 3) & 0xffff;
};

ACCA_ALWAYS_INLINE
static bool acca_einfo_is_write(acca_einfo_t einfo) {
	return (einfo & (1ull << 3)) != 0;
};

ACCA_ALWAYS_INLINE
static uint16_t acca_einfo_access_size(acca_einfo_t einfo) {
	return (einfo >> 4) & 0xffff;
};

ACCA_ALWAYS_INLINE
static acca_einfo_t acca_read_einfo(void) {
	acca_einfo_t einfo;
	__asm__("ldm %0, mreg.einfo" : "=r" (einfo));
	return einfo;
};

ACCA_ALWAYS_INLINE
static uintptr_t acca_read_eaddr(void) {
	uintptr_t eaddr;
	__asm__("ldm %0, mreg.eaddr" : "=r" (eaddr));
	return eaddr;
};

ACCA_ALWAYS_INLINE
static uintptr_t acca_read_elr(void) {
	uintptr_t elr;
	__asm__("ldm %0, mreg.elr" : "=r" (elr));
	return elr;
};

ACCA_ALWAYS_INLINE
static void acca_write_elr(uintptr_t elr) {
	__asm__ __volatile__("stm mreg.elr, %0" :: "r" (elr));
};

ACCA_ALWAYS_INLINE
static uintptr_t acca_read_esp(void) {
	uintptr_t esp;
	__asm__("ldm %0, mreg.esp" : "=r" (esp));
	return esp;
};

ACCA_ALWAYS_INLINE
static void acca_write_esp(uintptr_t esp) {
	__asm__ __volatile__("stm mreg.esp, %0" :: "r" (esp));
};

#endif // _ACCA_EXCEPTIONS_H_
