#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[16384],*p=output;static short file,count;
static void persist(void){long n=p-output+4;memcpy(p,"]}}\n",4);SetFPos(file,fsFromStart,0);SetEOF(file,0);FSWrite(file,&n,output);FlushVol(0,0);}
int main(void){WindowPtr w,hit;Rect b={100,100,260,380},drag={50,50,450,600},sizes={80,100,220,300};EventRecord e;short part,kind;long result;Point end;GrafPtr before,after;Rect old,new;OSErr err;
InitGraf(&qd.thePort);InitFonts();InitWindows();InitCursor();w=NewWindow(0,&b,"\pTracking",true,documentProc,(WindowPtr)-1,false,0);if(!w)return 2;SetPort(w);DrawGrowIcon(w);
err=Create("\pWindowTrackingProbe.json",0,'ttxt','TEXT');if(err)return 3;if(FSOpen("\pWindowTrackingProbe.json",0,&file))return 4;p+=sprintf(p,"{\"schema\":1,\"probe\":\"window-tracking-v1\",\"windows\":{\"cases\":[");persist();
for(;;){if(!WaitNextEvent(everyEvent,&e,1,0))continue;if(e.what==keyDown&&(e.message&255)=='q')break;if(e.what==updateEvt){BeginUpdate((WindowPtr)e.message);EraseRect(&w->portRect);DrawGrowIcon(w);EndUpdate((WindowPtr)e.message);continue;}if(e.what!=mouseDown)continue;part=FindWindow(e.where,&hit);if(hit!=w||(part!=inDrag&&part!=inGrow))continue;old=w->portRect;GetPort(&before);kind=part;result=0;if(part==inDrag)DragWindow(w,e.where,&drag);else result=GrowWindow(w,e.where,&sizes);GetPort(&after);new=w->portRect;GetMouse(&end);LocalToGlobal(&end);
p+=sprintf(p,"%s{\"kind\":%d,\"start\":[%d,%d],\"end\":[%d,%d],\"modifiers\":%u,\"result\":%ld,\"port_preserved\":%d,\"before\":[%d,%d,%d,%d],\"after_tracking\":[%d,%d,%d,%d],\"content\":[%d,%d,%d,%d]}",count?",":"",kind,e.where.v,e.where.h,end.v,end.h,e.modifiers,result,before==after,old.top,old.left,old.bottom,old.right,new.top,new.left,new.bottom,new.right,(**((WindowPeek)w)->contRgn).rgnBBox.top,(**((WindowPeek)w)->contRgn).rgnBBox.left,(**((WindowPeek)w)->contRgn).rgnBBox.bottom,(**((WindowPeek)w)->contRgn).rgnBBox.right);count++;persist();if(kind==inGrow&&result)SizeWindow(w,(short)result,(short)(result>>16),true);if(count>=12)break;}
FSClose(file);FlushVol(0,0);return 0;}
