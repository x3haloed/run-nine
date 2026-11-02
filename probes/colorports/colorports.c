#include <QuickDraw.h>
#include <Fonts.h>
#include <Windows.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
int main(void) {
    Rect r={60,50,180,250}; CWindowPtr w; CGrafPtr p; GrafPtr before, after;
    char out[4096]; short f; long n; OSErr err;
    InitGraf(&qd.thePort); InitFonts(); InitWindows(); GetPort(&before);
    w=(CWindowPtr)NewCWindow(0,&r,"\pReference",true,documentProc,(WindowPtr)-1,true,0x12345678);
    if(!w)return 5; p=(CGrafPtr)w; GetPort(&after);
    sprintf(out,"{\"schema\":1,\"probe\":\"color-ports-v1\",\"port\":{"
        "\"version\":%u,\"rect\":[%d,%d,%d,%d],\"bitmap_bounds\":[%d,%d,%d,%d],"
        "\"fg\":[%u,%u,%u],\"bg\":[%u,%u,%u],\"pen\":[%d,%d,%d,%d,%d,%d],"
        "\"text\":[%d,%u,%d,%d],\"extras\":[%d,%d,%ld],"
        "\"patterns\":[%d,%d,%d],\"current_unchanged\":%d,\"grafvars_nonnull\":%d}}\n",
        (unsigned short)p->portVersion,p->portRect.top,p->portRect.left,p->portRect.bottom,p->portRect.right,
        (**p->portPixMap).bounds.top,(**p->portPixMap).bounds.left,(**p->portPixMap).bounds.bottom,(**p->portPixMap).bounds.right,
        p->rgbFgColor.red,p->rgbFgColor.green,p->rgbFgColor.blue,p->rgbBkColor.red,p->rgbBkColor.green,p->rgbBkColor.blue,
        p->pnLoc.v,p->pnLoc.h,p->pnSize.v,p->pnSize.h,p->pnMode,p->pnVis,
        p->txFont,(unsigned char)p->txFace,p->txMode,p->txSize,p->chExtra,p->pnLocHFrac,p->spExtra,
        (**p->bkPixPat).patType,(**p->pnPixPat).patType,(**p->fillPixPat).patType,after==before,p->grafVars!=0);
    DisposeWindow((WindowPtr)w); SetPort(before);
    err=Create("\pColorPortProbe.json",0,'ttxt','TEXT'); if(err&&err!=dupFNErr)return 2;
    if(FSOpen("\pColorPortProbe.json",0,&f))return 3;
    SetEOF(f,0); n=strlen(out); err=FSWrite(f,&n,out); FSClose(f); FlushVol(0,0); return err?4:0;
}
