#include <Multiverse.h>
#include <stdio.h>
static char output[8192];
#define STATE (*(volatile short*)0x8d0)
#define VIS (*(volatile unsigned char*)0x8cc)
#define RAW (*(volatile unsigned char*)0x8d2)
int main(void){Rect bounds={80,80,230,540};WindowPtr window;short phase,file,bs[2],bv[2],bo[2],as[2],av[2],ao[2];Point start[2],end[2];long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();TEInit();InitDialogs(0);window=NewWindow(0,&bounds,"\pCursor motion 1 of 2",true,documentProc,(WindowPtr)-1,false,0);if(!window)return 2;SetPort(window);
 for(phase=0;phase<2;phase++){
  if(phase)SetWTitle(window,"\pCursor motion 2 of 2");
  EraseRect(&window->portRect);MoveTo(20,30);DrawString("\pMove the mouse to capture native cursor restoration.");
  InitCursor();if(phase)HideCursor();ObscureCursor();GetMouse(&start[phase]);bs[phase]=STATE;bv[phase]=VIS;bo[phase]=RAW;
  do{SystemTask();GetMouse(&end[phase]);}while(end[phase].v==start[phase].v&&end[phase].h==start[phase].h);
  as[phase]=STATE;av[phase]=VIS;ao[phase]=RAW;
 }
 InitCursor();n=sprintf(output,"{\"schema\":1,\"probe\":\"cursor-motion-raw-v1\",\"records\":[{\"hidden\":0,\"before_state\":%d,\"before_visible\":%d,\"before_8d2\":%d,\"after_state\":%d,\"after_visible\":%d,\"after_8d2\":%d,\"start\":[%d,%d],\"end\":[%d,%d]},{\"hidden\":1,\"before_state\":%d,\"before_visible\":%d,\"before_8d2\":%d,\"after_state\":%d,\"after_visible\":%d,\"after_8d2\":%d,\"start\":[%d,%d],\"end\":[%d,%d]}]}\n",bs[0],bv[0],bo[0],as[0],av[0],ao[0],start[0].v,start[0].h,end[0].v,end[0].h,bs[1],bv[1],bo[1],as[1],av[1],ao[1],start[1].v,start[1].h,end[1].v,end[1].h);
 e=Create("\pCursorMotionProbe.json",0,'pPrb','TEXT');if(e)return 3;if(FSOpen("\pCursorMotionProbe.json",0,&file))return 4;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);DisposeWindow(window);return e?5:0;
}
