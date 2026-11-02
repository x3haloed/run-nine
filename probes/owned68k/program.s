/* Owned, position-independent 68K instructions. A3 points to 36 output bytes.
   The same unmodified image runs on native OS9 and both independent backends. */
.text
.global owned68k
owned68k:
    movem.l %d2-%d7/%a2-%a6,-(%sp)
    moveq #16,%d0
    .word 0xa322                 /* NewHandleClear */
    move.w %d0,0(%a3)
    movea.l %a0,%a2
    move.l %a2,%d2
    beq finished
    .word 0xa025                 /* GetHandleSize */
    move.l %d0,2(%a3)
    movea.l %a2,%a0
    .word 0xa029                 /* HLock */
    move.w %d0,6(%a3)
    movea.l (%a2),%a1
    moveq #0,%d5
    moveq #15,%d6
check_clear:
    or.b (%a1)+,%d5
    dbra %d6,check_clear
    move.l %d5,8(%a3)
    movea.l (%a2),%a1
    moveq #1,%d4
    moveq #15,%d6
fill:
    move.b %d4,(%a1)+
    addq.l #3,%d4
    dbra %d6,fill
    movea.l %a2,%a0
    .word 0xa02a                 /* HUnlock */
    move.w %d0,12(%a3)
    movea.l %a2,%a0
    moveq #64,%d0
    .word 0xa024                 /* SetHandleSize */
    move.w %d0,14(%a3)
    movea.l %a2,%a0
    .word 0xa025
    move.l %d0,16(%a3)
    movea.l %a2,%a0
    .word 0xa029
    movea.l (%a2),%a1
    moveq #0,%d5
    moveq #15,%d6
checksum:
    moveq #0,%d4
    move.b (%a1)+,%d4
    add.l %d4,%d5
    dbra %d6,checksum
    move.l %d5,20(%a3)
    movea.l %a2,%a0
    .word 0xa02a
    movea.l %a2,%a0
    .word 0xa023                 /* DisposeHandle */
    move.w %d0,24(%a3)
    suba.l %a0,%a0
    move.w #0x10,%ccr
    .word 0xa025                 /* Nil handle error, no dereference */
    .word 0x42c1                 /* Move CCR to D1 before other instructions */
    move.w %d0,26(%a3)
    move.w 0x220,28(%a3)
    andi.w #31,%d1
    move.w %d1,30(%a3)
    move.l #0x36384b21,32(%a3)
finished:
    movem.l (%sp)+,%d2-%d7/%a2-%a6
    rts
