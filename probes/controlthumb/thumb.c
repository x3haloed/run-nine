#include <Multiverse.h>
#include <stdio.h>
#include <string.h>

/* Owned native diagnostic. No disk I/O occurs between mouseDown and tracking. */
static char output[32768], *p = output;
static short file, count, hook_count, hook_saved;
static WindowPtr window;
static ControlHandle control;
struct Hook { short v, h, button, value; unsigned long tick; } hooks[8];
static void persist(void) {
    long n = p - output + 4;
    memcpy(p, "]}}\n", 4);
    SetFPos(file, fsFromStart, 0); SetEOF(file, 0);
    FSWrite(file, &n, output); FlushVol(0, 0);
}
static pascal void indicator(void) {
    Point mouse;
    hook_count++;
    if (hook_saved == 8) return;
    GetMouse(&mouse);
    hooks[hook_saved].v = mouse.v; hooks[hook_saved].h = mouse.h;
    hooks[hook_saved].button = Button();
    hooks[hook_saved].value = GetControlValue(control);
    hooks[hook_saved].tick = TickCount();
    hook_saved++;
}
int main(void) {
    Rect bounds = {41, 4, 476, 571}, bar = {0, 552, 420, 567};
    EventRecord e; WindowPtr hit; Point start, entry, end;
    short part, button, result, i; unsigned long before, after;
    Str255 title; OSErr err;
    InitGraf(&qd.thePort); InitFonts(); InitWindows(); InitMenus(); InitCursor();
    window = NewWindow(0, &bounds, "\pThumb trace: 0/6", true, documentProc, (WindowPtr)-1, false, 0);
    if (!window) return 2;
    SetPort(window);
    control = NewControl(window, &bar, "\p", true, 25, 0, 100, scrollBarProc, 0);
    if (!control) return 3;
    err = Create("\pControlThumbProbe.json", 0, 'pPrb', 'TEXT');
    if (err && err != dupFNErr) return 4;
    if (FSOpen("\pControlThumbProbe.json", 0, &file)) return 5;
    p += sprintf(p, "{\"schema\":1,\"probe\":\"control-thumb-v1\",\"controls\":{\"cases\":[");
    persist();
    while (count < 6) {
        if (!WaitNextEvent(everyEvent, &e, 1, 0)) continue;
        if (e.what == updateEvt) {
            BeginUpdate(window); DrawControls(window); EndUpdate(window); continue;
        }
        if (e.what != mouseDown || FindWindow(e.where, &hit) != inContent || hit != window) continue;
        SetPort(window); start = e.where; GlobalToLocal(&start);
        part = TestControl(control, start);
        if (part != 129) continue;
        hook_count = hook_saved = 0;
        GetMouse(&entry); button = Button(); before = TickCount();
        result = TrackControl(control, start, count < 3 ? 0 : (ControlActionUPP)indicator);
        after = TickCount(); GetMouse(&end);
        p += sprintf(p, "%s{\"action\":%d,\"start\":[%d,%d],\"entry_mouse\":[%d,%d],\"entry_button\":%d,\"event_age\":%lu,\"duration\":%lu,\"end\":[%d,%d],\"result\":%d,\"value\":%d,\"hilite\":%d,\"hook_count\":%d,\"hook\":[",
            count ? "," : "", count >= 3, start.v, start.h, entry.v, entry.h, button,
            before - e.when, after - before, end.v, end.h, result, GetControlValue(control), (**control).contrlHilite, hook_count);
        for (i = 0; i < hook_saved; i++)
            p += sprintf(p, "%s[%d,%d,%d,%d,%lu]", i ? "," : "", hooks[i].v, hooks[i].h, hooks[i].button, hooks[i].value, hooks[i].tick - before);
        p += sprintf(p, "]}"); count++; persist(); SetControlValue(control, 25);
        sprintf((char*)title + 1, "%d/6 result%d value recorded", count, result);
        title[0] = strlen((char*)title + 1); SetWTitle(window, title);
    }
    FSClose(file); FlushVol(0, 0); return 0;
}
