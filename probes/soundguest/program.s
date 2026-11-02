/* Owned relocatable code, identical in OS9 and the independent guest CPU.
   A3 points to a4928-byte block, standard header64,sample bytes128. */
.text
.global sound_guest_program
sound_guest_program:
    movem.l %d2-%d7/%a2-%a6,-(%sp)
    movea.l %a3,%a2
    subq.l #2,%sp
    move.l %a2,-(%sp)
    move.w #5,-(%sp)
    move.l #0x80,-(%sp)
    lea callback(%pc),%a0
    move.l %a0,-(%sp)
    .word 0xa807
    move.w (%sp)+,24(%a2)
    tst.w 24(%a2)
    bne finished
    move.w #46,16(%a2)
    clr.w 18(%a2)
    move.l #0x00800100,20(%a2)
    subq.l #2,%sp
    move.l (%a2),-(%sp)
    pea 16(%a2)
    .word 0xa804
    move.w (%sp)+,26(%a2)
    move.w #47,16(%a2)
    lea 36(%a2),%a0
    move.l %a0,20(%a2)
    subq.l #2,%sp
    move.l (%a2),-(%sp)
    pea 16(%a2)
    .word 0xa804
    move.w (%sp)+,28(%a2)
    move.w #81,16(%a2)
    lea 64(%a2),%a0
    move.l %a0,20(%a2)
    subq.l #2,%sp
    move.l (%a2),-(%sp)
    pea 16(%a2)
    clr.w -(%sp)
    .word 0xa803
    move.w (%sp)+,30(%a2)
    move.w #13,16(%a2)
    move.w #456,18(%a2)
    move.l %a2,20(%a2)
    subq.l #2,%sp
    move.l (%a2),-(%sp)
    pea 16(%a2)
    clr.w -(%sp)
    .word 0xa803
    move.w (%sp)+,32(%a2)
wait:
    tst.l 4(%a2)
    beq wait
    subq.l #2,%sp
    move.l (%a2),-(%sp)
    move.w #0x0100,-(%sp)
    .word 0xa801
    move.w (%sp)+,34(%a2)
finished:
    movem.l (%sp)+,%d2-%d7/%a2-%a6
    rts
callback:
    movea.l 4(%sp),%a0
    movea.l 4(%a0),%a1
    addq.l #1,4(%a1)
    move.w 2(%a0),%d0
    ext.l %d0
    move.l %d0,8(%a1)
    move.l 8(%sp),%d0
    cmp.l (%a1),%d0
    seq %d0
    and.l #1,%d0
    move.l %d0,12(%a1)
    movea.l (%sp)+,%a0
    addq.l #8,%sp
    jmp (%a0)
