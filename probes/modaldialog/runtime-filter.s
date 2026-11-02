/* Owned Pascal Boolean filter. Parameters and state match the native C fixture;
   no native output data or application-specific conditions are read. */
.text
.globl _start
_start:
 move.l %d2,-(%sp)
 lea 4(%sp),%a1
 moveq #0,%d0
 move.w 0x110002,%d0
 addq.w #1,0x110002
 lsl.l #4,%d0
 lea 0x110100,%a0
 adda.l %d0,%a0
 move.l 12(%a1),%d0
 cmp.l 0x110004,%d0
 seq %d0
 and.w #1,%d0
 move.w %d0,(%a0)
 move.l 0x100000,%d0
 cmp.l 12(%a1),%d0
 seq %d0
 and.w #1,%d0
 move.w %d0,2(%a0)
 move.l 4(%a1),%d0
 cmp.l 0x110008,%d0
 seq %d0
 and.w #1,%d0
 move.w %d0,4(%a0)
 move.l 4(%a1),%a1
 move.w (%a1),6(%a0)
 move.l 12(%sp),%a1
 move.w (%a1),12(%a0)
 moveq #0,%d0
 cmpa.l #0,%a1
 sne %d0
 and.w #1,%d0
 move.w %d0,14(%a0)
 move.w 0x110000,%d0
 moveq #1,%d2
 cmp.w #0,%d0
 beq .one
 cmp.w #1,%d0
 beq .two
 cmp.w #2,%d0
 beq .mode_two
 cmp.w #3,%d0
 beq .mode_three
 cmp.w #4,%d0
 beq .mode_four
 cmp.w #5,%d0
 beq .mode_five
 cmp.w #6,%d0
 beq .mode_six
 moveq #5,%d1
 bra .return
.one:
 moveq #1,%d1
 bra .return
.two:
 moveq #2,%d1
 bra .return
.mode_two:
 cmp.w #1,0x110002
 bne .three
 moveq #5,%d1
 moveq #0,%d2
 bra .return
.three:
 moveq #3,%d1
 bra .return
.mode_three:
 cmp.w #1,0x110002
 bne .two
 moveq #0,%d1
 bra .return
.mode_four:
 cmp.w #1,0x110002
 bne .two
 moveq #-1,%d1
 bra .return
.mode_five:
 cmp.w #1,0x110002
 bne .two
 move.w #32767,%d1
 bra .return
.mode_six:
 cmp.w #4,0x110002
 bge .one
 moveq #5,%d1
 moveq #0,%d2
.return:
 move.w %d1,8(%a0)
 move.w %d2,10(%a0)
 move.l 8(%sp),%a1
 move.w %d1,(%a1)
 move.b %d2,20(%sp)
 move.l (%sp)+,%d2
 move.l (%sp)+,%a0
 lea 12(%sp),%sp
 jmp (%a0)
