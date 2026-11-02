/* Owned no-argument Pascal indicator procedure: first state at $110600. */
.text
.global action
action:
 tst.w 0x110600
 bne.s done
 move.w #1,0x110600
 pea 0x110602
 .word 0xa972 /* GetMouse */
 subq.l #2,%sp
 .word 0xa974 /* Button */
 moveq #0,%d0
 move.b (%sp),%d0
 addq.l #2,%sp
 move.w %d0,0x110606
 subq.l #2,%sp
 move.l 0x110620,-(%sp)
 .word 0xa960 /* GetControlValue */
 move.w (%sp)+,0x110608
done:
 rts
