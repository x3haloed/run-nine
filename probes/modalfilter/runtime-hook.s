.text
.globl callback
callback:
    move.w 12(%sp),%d0
    cmpi.w #11,%d0
    bne.s store
    moveq #2,%d0
store:
    move.w %d0,14(%sp)
    movea.l (%sp)+,%a0
    adda.w #10,%sp
    jmp (%a0)
