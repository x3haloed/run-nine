/* Observe QuickDraw globals and an application's freshly opened GrafPort. */
#include <QuickDraw.h>
#include <Fonts.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>

int main(void) {
    GrafPtr initial;
    GrafPort port;
    char out[4096];
    char changed[512];
    short file;
    long count;
    OSErr err;
    InitGraf(&qd.thePort);
    InitFonts();
    GetPort(&initial);
    OpenPort(&port);
    sprintf(out,"{\"schema\":1,\"probe\":\"quickdraw-v1\","
      "\"globals\":{\"screen_bounds\":[%d,%d,%d,%d],\"row_bytes\":%d,\"screen_nonnull\":%d,\"port_nonnull\":%d},"
      "\"port\":{\"device\":%d,\"bounds\":[%d,%d,%d,%d],\"row_bytes\":%d,"
      "\"pen\":[%d,%d],\"pen_size\":[%d,%d],\"pen_mode\":%d,\"pen_visibility\":%d,"
      "\"text_font\":%d,\"text_face\":%d,\"text_mode\":%d,\"text_size\":%d,\"foreground\":%ld,\"background\":%ld,"
      "\"pen_pattern\":[%u,%u,%u,%u,%u,%u,%u,%u],"
      "\"visible_bounds\":[%d,%d,%d,%d],\"clip_bounds\":[%d,%d,%d,%d]}",
      qd.screenBits.bounds.top,qd.screenBits.bounds.left,qd.screenBits.bounds.bottom,qd.screenBits.bounds.right,
      qd.screenBits.rowBytes,qd.screenBits.baseAddr!=0,initial!=0,
      port.device,port.portRect.top,port.portRect.left,port.portRect.bottom,port.portRect.right,port.portBits.rowBytes,
      port.pnLoc.v,port.pnLoc.h,port.pnSize.v,port.pnSize.h,port.pnMode,port.pnVis,
      port.txFont,port.txFace,port.txMode,port.txSize,port.fgColor,port.bkColor,
      port.pnPat.pat[0],port.pnPat.pat[1],port.pnPat.pat[2],port.pnPat.pat[3],port.pnPat.pat[4],port.pnPat.pat[5],port.pnPat.pat[6],port.pnPat.pat[7],
      (**port.visRgn).rgnBBox.top,(**port.visRgn).rgnBBox.left,(**port.visRgn).rgnBBox.bottom,(**port.visRgn).rgnBBox.right,
      (**port.clipRgn).rgnBBox.top,(**port.clipRgn).rgnBBox.left,(**port.clipRgn).rgnBBox.bottom,(**port.clipRgn).rgnBBox.right);
    MoveTo(12,34); PenSize(3,4); PenMode(patXor); TextFont(3); TextSize(9); TextMode(srcCopy);
    sprintf(changed,",\"changed\":{\"pen\":[%d,%d],\"pen_size\":[%d,%d],\"pen_mode\":%d,\"text_font\":%d,\"text_size\":%d,\"text_mode\":%d}}\n",
       port.pnLoc.v,port.pnLoc.h,port.pnSize.v,port.pnSize.h,port.pnMode,port.txFont,port.txSize,port.txMode);
    strcat(out,changed);
    ClosePort(&port);SetPort(initial);
    err=Create("\pQuickDrawProbe.json",0,'ttxt','TEXT');
    if(err&&err!=dupFNErr)return 2;
    if(FSOpen("\pQuickDrawProbe.json",0,&file))return 3;
    SetEOF(file,0);count=strlen(out);err=FSWrite(file,&count,out);FSClose(file);FlushVol(0,0);
    return err?4:0;
}
