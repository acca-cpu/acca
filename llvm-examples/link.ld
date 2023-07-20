SECTIONS {
	. = 0x00;
	.data.econfig : { *(.data.econfig) }
	. = 0x400;
	.text.start : { *(.text.start) }
	.text.exc : { *(.text.exc) }
	.text ALIGN(0x100) : { *(.text) *(.text.*) }
	.data ALIGN(0x100) : { *(.data) *(.data.*) }
	.rodata ALIGN(0x100) : { *(.rodata) *(.rodata.*) }
	.bss ALIGN(0x100) : { *(.bss) *(COMMON) }
}
