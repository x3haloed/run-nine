#include <QuickDraw.h>
#include <Windows.h>
#include <Fonts.h>
#include <Files.h>
#include <stdio.h>
static char output[8192];
static char *rect(char *p,Rect r){return p+sprintf(p,"[%d,%d,%d,%d]",r.top,r.left,r.bottom,r.right);}
static char *state(char *p,short id,Boolean color){WindowPtr w;WindowPeek peek;GrafPtr current;OSErr e;unsigned char title[256];short i;
 w=color?GetNewCWindow(id,0,(WindowPtr)-1):GetNewWindow(id,0,(WindowPtr)-1);e=ResError();p+=sprintf(p,"{\"nonnull\":%d,\"resource_error\":%d",w!=0,e);if(w){peek=(WindowPeek)w;GetWTitle(w,title);GetPort(&current);
 p+=sprintf(p,",\"color\":%d,\"visible\":%d,\"kind\":%d,\"go_away\":%d,\"refcon\":%ld,\"current\":%d,\"title\":[",(((CGrafPtr)w)->portVersion&0xc000)==0xc000,peek->visible!=0,peek->windowKind,peek->goAwayFlag!=0,peek->refCon,current==(GrafPtr)w);
 for(i=0;i<title[0];i++)p+=sprintf(p,"%s%u",i?",":"",title[i+1]);p+=sprintf(p,"],\"port_rect\":");p=rect(p,peek->port.portRect);p+=sprintf(p,",\"content\":");p=rect(p,(**peek->contRgn).rgnBBox);p+=sprintf(p,",\"structure\":");p=rect(p,(**peek->strucRgn).rgnBBox);DisposeWindow(w);}return p+sprintf(p,"}");}
int main(void){char *p=output;GrafPtr old;short file;long n;OSErr e;InitGraf(&qd.thePort);InitFonts();InitWindows();GetPort(&old);p+=sprintf(p,"{\"schema\":1,\"probe\":\"window-resources-v1\",\"windows\":{\"mono\":");p=state(p,21000,0);SetPort(old);p+=sprintf(p,",\"color\":");p=state(p,21001,1);SetPort(old);p+=sprintf(p,",\"missing_mono\":");p=state(p,31000,0);p+=sprintf(p,",\"missing_color\":");p=state(p,31000,1);p+=sprintf(p,"}}\n");
 e=Create("\pWindowResourcesProbe.json",0,'ttxt','TEXT');if(e&&e!=dupFNErr)return 2;if(FSOpen("\pWindowResourcesProbe.json",0,&file))return 3;SetEOF(file,0);n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;}
