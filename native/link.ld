ENTRY(_start)
SECTIONS {
  . = 0x40080000;
  .text : { KEEP(*(.text.boot)) *(.text .text.*) }
  .rodata : ALIGN(16) { *(.rodata .rodata.*) }
  .data : ALIGN(16) { *(.data .data.*) *(.got .got.*) }
  .bss (NOLOAD) : ALIGN(16) {
    __bss_start = .;
    *(.bss .bss.*) *(COMMON)
    . = ALIGN(16);
    __bss_end = .;
  }
  .stack (NOLOAD) : ALIGN(16) { . += 65536; __stack_top = .; }
  /DISCARD/ : { *(.eh_frame*) *(.comment*) }
}
