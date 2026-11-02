.text
.globl callback
callback:
    movea.l 4(%sp),%a0
    movea.l 12(%sp),%a1
    tst.w (%a1)
    bne.s done
    addq.w #1,2(%a0)
    cmpi.w #1,(%a0)
    bne.s handled
    cmpi.w #1,2(%a0)
    bne.s handled
    movea.l 8(%sp),%a1
    move.w #21,(%a1)
    bra.s done
handled:
    movea.l 8(%sp),%a1
    moveq #2,%d0
    cmpi.w #2,(%a0)
    bne.s store
    moveq #11,%d0
store:
    move.w %d0,(%a1)
    move.b #1,20(%sp)
done:
    movea.l (%sp)+,%a0
    adda.w #16,%sp
    jmp (%a0)
