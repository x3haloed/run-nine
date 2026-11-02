/* Independent Pascal void callback. Mutate public GrafPort state exactly as
   the native C callback does after logging; PaintRect has no state side effects.
   The runtime probe observes actual callback-entry arguments and port fields. */
.text
.globl callback
callback:
    movea.l 6(%sp),%a0
    move.w #4,68(%a0)
    move.w #18,74(%a0)
    move.w #3,54(%a0)
    move.w #4,52(%a0)
    movea.l (%sp)+,%a0
    adda.w #6,%sp
    jmp (%a0)
