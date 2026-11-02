/* Owned Pascal ControlAction procedure. Diagnostic record at $110600. */
.text
.global action
action:
 tst.w 0x110600
 bne.s done
 move.w #1,0x110600
 move.w 4(%sp),0x110602
 movea.l 6(%sp),%a0
 move.l %a0,0x11060a
 movea.l (%a0),%a1
 move.w 18(%a1),0x110604
 moveq #0,%d0
 move.b 17(%a1),%d0
 move.w %d0,0x110606
 pea 0x11060e
 .word 0xa874 /* GetPort */
 move.w 4(%sp),%d0
 moveq #-1,%d1
 cmpi.w #20,%d0
 beq.s change
 moveq #1,%d1
 cmpi.w #21,%d0
 beq.s change
 moveq #-10,%d1
 cmpi.w #22,%d0
 beq.s change
 moveq #10,%d1
change:
 move.w 18(%a1),%d0
 add.w %d1,%d0
 move.l %a0,-(%sp)
 move.w %d0,-(%sp)
 .word 0xa963 /* SetControlValue */
done:
 movea.l (%sp)+,%a0
 lea 6(%sp),%sp
 jmp (%a0)
