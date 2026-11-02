/* Controlled Pascal ModalFilter: choose item2 and return true. */
.text
.globl callback
callback:
    movea.l 4(%sp),%a0
    move.w #2,(%a0)
    move.b #1,16(%sp)
    movea.l (%sp)+,%a0
    adda.w #12,%sp
    jmp (%a0)
