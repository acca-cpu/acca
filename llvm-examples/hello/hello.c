#define MREG_VM_CONSOLE 0xdead1

typedef __UINT8_TYPE__  uint8_t;
typedef __UINT16_TYPE__ uint16_t;
typedef __UINT32_TYPE__ uint32_t;
typedef __UINT64_TYPE__ uint64_t;

typedef struct evt_entry evt_entry_t;
struct evt_entry {
	uint32_t instructions[8];
};

typedef struct evt evt_t;
struct evt {
	evt_entry_t pl0[8];
	evt_entry_t pl1[8];
};

typedef struct ect_entry ect_entry_t;
struct ect_entry {
	uint64_t flags;
	uint64_t stack_pointer;
	uint64_t stack_size;
};

typedef struct ect ect_t;
struct ect {
	ect_entry_t pl0[8];
	ect_entry_t pl1[8];
};

__attribute__((section(".data.econfig")))
evt_t global_evt;

__attribute__((section(".data.econfig")))
ect_t global_ect;

void print_char(char character) {
	__asm__ __volatile__("stm %0, %1" :: "i" (MREG_VM_CONSOLE), "r" ((uint64_t)character));
};

void print_string(const char* string) {
	while (*string != '\0')
		print_char(*(string++));
};

void main(void) {
	print_string("Hello, world!\n");
};

__attribute__((section(".text.start")))
__attribute__((naked))
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
