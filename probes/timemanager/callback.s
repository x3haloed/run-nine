/* Owned TimerProc. A1 points to our TMTask plus observation/control fields. */
.text
.global timer_callback
timer_callback:
    move.l %a0,26(%a1)
    move.l %a1,30(%a1)
    move.w 4(%a1),34(%a1)
    move.l 10(%a1),36(%a1)
    move.l 22(%a1),%d0
    cmpi.l #3,%d0
    bge.s sampled
    lsl.w #2,%d0
    move.l 0x16a,52(%a1,%d0.w)
sampled:
    addq.l #1,22(%a1)
    move.l 22(%a1),%d0
    cmp.l 44(%a1),%d0
    bge.s done
    move.l 40(%a1),%d0
    movea.l %a1,%a0
    .word 0xa05a
done:
    rts
