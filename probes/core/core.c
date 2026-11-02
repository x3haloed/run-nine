/* Behavioral observations from the real Macintosh managers.
   Does not assert expected results: the host compares observations later. */
#include <Memory.h>
#include <QuickDraw.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>

int main(void)
{
    char output[2048];
    short file;
    long count;
    OSErr err;
    Handle h = NewHandle(16);
    short allocationError = MemError();
    long initialSize = h ? GetHandleSize(h) : -1;
    int preserved = 0;
    short resizeError = -1;
    long resizedSize = -1;
    Rect a = {0, 0, 20, 30};
    Rect b = {10, 15, 40, 50};
    Rect intersection;
    Point inside = {0, 0};
    Point rightEdge = {0, 30};
    Point bottomEdge = {20, 0};
    Boolean intersects;
    if (h) {
        HLock(h);
        memcpy(*h, "Macintosh probe", 16);
        HUnlock(h);
        SetHandleSize(h, 64);
        resizeError = MemError();
        resizedSize = GetHandleSize(h);
        if (!resizeError) {
            HLock(h);
            preserved = memcmp(*h, "Macintosh probe", 16) == 0;
            HUnlock(h);
        }
        DisposeHandle(h);
    }
    intersects = SectRect(&a, &b, &intersection);
    sprintf(output,
        "{\"schema\":1,\"probe\":\"core-v1\","
        "\"memory\":{\"allocation_error\":%d,\"initial_size\":%ld,"
        "\"resize_error\":%d,\"resized_size\":%ld,\"preserved\":%d},"
        "\"rectangles\":{\"intersects\":%d,\"intersection\":[%d,%d,%d,%d],"
        "\"origin_inside\":%d,\"right_edge_inside\":%d,\"bottom_edge_inside\":%d}}\n",
        allocationError, initialSize, resizeError, resizedSize, preserved,
        intersects != 0, intersection.top, intersection.left,
        intersection.bottom, intersection.right, PtInRect(inside, &a) != 0,
        PtInRect(rightEdge, &a) != 0, PtInRect(bottomEdge, &a) != 0);
    /* Relative to the launch volume/directory. No stdio filesystem shim. */
    err = Create("\pCoreProbe.json", 0, 'ttxt', 'TEXT');
    if (err && err != dupFNErr) return 2;
    err = FSOpen("\pCoreProbe.json", 0, &file);
    if (err) return 3;
    SetEOF(file, 0);
    count = (long)strlen(output);
    err = FSWrite(file, &count, output);
    FSClose(file);
    FlushVol(0, 0);
    return err ? 4 : 0;
}
