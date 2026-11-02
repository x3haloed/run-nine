.text
.globl vbl_callback
vbl_callback:
 move.l %a0,18(%a0)
 move.w 4(%a0),22(%a0)
 move.w 10(%a0),24(%a0)
 move.w 12(%a0),26(%a0)
 move.l 14(%a0),%d0
 mulu.w #4,%d0
 move.l 0x16a.w,38(%a0,%d0.w)
 move.l 50(%a0),54(%a0,%d0.w)
 addq.l #1,14(%a0)
 move.l 14(%a0),%d0
 cmp.l 28(%a0),%d0
 bge.s done
 move.w 32(%a0),10(%a0)
done:
 rts
