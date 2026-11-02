#include <QuickDraw.h>
#include <Fonts.h>
#include <Windows.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
int main(void){
    Rect bounds={60,50,180,250};WindowPtr window;WindowPeek w;GrafPtr before,after;
    char out[4096];short file;long count;OSErr err;int beforePort;
    InitGraf(&qd.thePort);InitFonts();InitWindows();GetPort(&before);beforePort=before!=0;
    window=NewWindow(0,&bounds,"\pReference",true,documentProc,(WindowPtr)-1,true,0x12345678);
    if(!window)return 5;w=(WindowPeek)window;GetPort(&after);
    sprintf(out,"{\"schema\":1,\"probe\":\"windows-v1\",\"initial\":{\"port_nonnull\":%d},"
      "\"window\":{\"kind\":%d,\"visible\":%d,\"hilited\":%d,\"go_away\":%d,\"refcon\":%ld,"
      "\"port_bounds\":[%d,%d,%d,%d],\"bitmap_bounds\":[%d,%d,%d,%d],"
      "\"structure_bounds\":[%d,%d,%d,%d],\"content_bounds\":[%d,%d,%d,%d],\"update_bounds\":[%d,%d,%d,%d],"
      "\"title_width\":%d,\"front_matches\":%d,\"current_matches\":%d}}\n",
      beforePort,w->windowKind,w->visible!=0,w->hilited!=0,w->goAwayFlag!=0,w->refCon,
      w->port.portRect.top,w->port.portRect.left,w->port.portRect.bottom,w->port.portRect.right,
      w->port.portBits.bounds.top,w->port.portBits.bounds.left,w->port.portBits.bounds.bottom,w->port.portBits.bounds.right,
      (**w->strucRgn).rgnBBox.top,(**w->strucRgn).rgnBBox.left,(**w->strucRgn).rgnBBox.bottom,(**w->strucRgn).rgnBBox.right,
      (**w->contRgn).rgnBBox.top,(**w->contRgn).rgnBBox.left,(**w->contRgn).rgnBBox.bottom,(**w->contRgn).rgnBBox.right,
      (**w->updateRgn).rgnBBox.top,(**w->updateRgn).rgnBBox.left,(**w->updateRgn).rgnBBox.bottom,(**w->updateRgn).rgnBBox.right,
      w->titleWidth,FrontWindow()==window,after==window);
    DisposeWindow(window);SetPort(before);
    err=Create("\pWindowProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;
    if(FSOpen("\pWindowProbe.json",0,&file))return 3;
    SetEOF(file,0);count=strlen(out);err=FSWrite(file,&count,out);FSClose(file);FlushVol(0,0);return err?4:0;
}
