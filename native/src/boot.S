.section .text.boot,"ax"
.global _start
_start:
    mov x19, x0
    msr daifset, #15
    adrp x0, __stack_top
    add x0, x0, :lo12:__stack_top
    mov sp, x0
    adrp x0, __bss_start
    add x0, x0, :lo12:__bss_start
    adrp x1, __bss_end
    add x1, x1, :lo12:__bss_end
1:
    cmp x0, x1
    b.hs 2f
    str xzr, [x0], #8
    b 1b
2:
    mrs x0, cpacr_el1
    orr x0, x0, #(3 << 20)
    msr cpacr_el1, x0
    isb
    mov x0, x19
    bl kernel_main
3:
    wfe
    b 3b
