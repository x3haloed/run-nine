#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[8192];
static char *rect(char *p,Rect r){return p+sprintf(p,"[%d,%d,%d,%d]",r.top,r.left,r.bottom,r.right);}
static char *snapshot(char *p,WindowPtr w){WindowPeek x=(WindowPeek)w;GrafPtr current;GetPort(&current);
p+=sprintf(p,"{\"current\":%d,\"front\":%d,\"port\":",current==(GrafPtr)w,FrontWindow()==w);p=rect(p,x->port.portRect);p+=sprintf(p,",\"bitmap\":");p=rect(p,x->port.portBits.bounds);p+=sprintf(p,",\"structure\":");p=rect(p,(**x->strucRgn).rgnBBox);p+=sprintf(p,",\"content\":");p=rect(p,(**x->contRgn).rgnBBox);p+=sprintf(p,",\"update\":");p=rect(p,(**x->updateRgn).rgnBBox);p+=sprintf(p,",\"visible\":");p=rect(p,(**x->port.visRgn).rgnBBox);p+=sprintf(p,",\"local_pixels\":[%d,%d],\"update_hits\":[%d,%d,%d]}",GetPixel(25,15)!=0,GetPixel(5,5)!=0,PtInRgn((Point){130,120},x->updateRgn)!=0,PtInRgn((Point){150,345},x->updateRgn)!=0,PtInRgn((Point){275,130},x->updateRgn)!=0);return p;}
int main(void){Rect b={60,50,180,250},ink={10,20,30,50};WindowPtr w;char *p=output;short f;long n;OSErr e;
InitGraf(&qd.thePort);InitFonts();InitWindows();w=NewWindow(0,&b,"\pGeometry",true,documentProc,(WindowPtr)-1,true,0);if(!w)return 2;SetPort(w);SetEmptyRgn(((WindowPeek)w)->updateRgn);EraseRect(&w->portRect);PaintRect(&ink);
p+=sprintf(p,"{\"schema\":1,\"probe\":\"window-geometry-v1\",\"windows\":{\"initial\":");p=snapshot(p,w);
MoveWindow(w,90,100,false);p+=sprintf(p,",\"moved\":");p=snapshot(p,w);
MoveWindow(w,110,120,true);p+=sprintf(p,",\"moved_front\":");p=snapshot(p,w);
SizeWindow(w,160,90,true);p+=sprintf(p,",\"shrunk\":");p=snapshot(p,w);
SetEmptyRgn(((WindowPeek)w)->updateRgn);SizeWindow(w,220,140,false);p+=sprintf(p,",\"expanded_no_update\":");p=snapshot(p,w);
SizeWindow(w,260,180,true);p+=sprintf(p,",\"expanded_update\":");p=snapshot(p,w);
SizeWindow(w,0,0,true);p+=sprintf(p,",\"zero_size\":");p=snapshot(p,w);
SetEmptyRgn(((WindowPeek)w)->updateRgn);InvalRect(&ink);MoveWindow(w,140,150,false);p+=sprintf(p,",\"moved_dirty\":");p=snapshot(p,w);
p+=sprintf(p,"}}\n");DisposeWindow(w);e=Create("\pWindowGeometryProbe.json",0,'ttxt','TEXT');if(e)return 3;if(FSOpen("\pWindowGeometryProbe.json",0,&f))return 4;n=p-output;e=FSWrite(f,&n,output);FSClose(f);FlushVol(0,0);return e?5:0;}
