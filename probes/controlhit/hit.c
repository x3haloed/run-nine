#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[65536],*p=output;static short comma;
static void point(ControlHandle h,short v,short x){Point pt={v,x};p+=sprintf(p,"%s[%d,%d,%d]",comma?",":"",v,x,TestControl(h,pt));comma=1;}
int main(void){Rect bounds={60,50,280,450},r;WindowPtr w;ControlHandle h,back,front,found;GrafPtr before,after;short mode,axis,x,j,k,file,kind;long n;OSErr e;short ys[]={19,20,21,25,30,38,39,40},xs[]={39,40,41,47,50,55,56,60,100,159,160};
InitGraf(&qd.thePort);InitFonts();InitWindows();w=NewWindow(0,&bounds,"\pHits",true,documentProc,(WindowPtr)-1,true,0);if(!w)return 2;SetPort(w);
p+=sprintf(p,"{\"schema\":1,\"probe\":\"control-hit-v1\",\"controls\":{\"scrollbars\":[");
for(mode=0;mode<11;mode++){SetRect(&r,10,20,26,180);if(mode==7)SetRect(&r,40,20,200,36);if(mode==9)SetRect(&r,10,20,26,48);if(mode==10)SetRect(&r,10,20,22,180);
h=NewControl(w,&r,"\p",true,mode==1?0:mode==2?100:mode==7?0:25,mode==7?-100:0,mode==6?0:100,scrollBarProc,0);if(!h)return 3;
if(mode==3)HiliteControl(h,255);if(mode==4)HideControl(h);if(mode==5)HideWindow(w);if(mode==8)HiliteControl(h,inUpButton);
GetPort(&before);r=(**h).contrlRect;p+=sprintf(p,"%s{\"mode\":%d,\"rect\":[%d,%d,%d,%d],\"range\":[%d,%d,%d],\"points\":[",mode?",":"",mode,r.top,r.left,r.bottom,r.right,(**h).contrlValue,(**h).contrlMin,(**h).contrlMax);comma=0;
if(mode==7){for(axis=r.left-2;axis<=r.right+2;axis++)point(h,r.top+8,axis);for(j=0;j<4;j++)for(x=r.top-1;x<=r.bottom;x++)point(h,x,j==0?r.left+8:j==1?r.left+40:j==2?r.left+80:r.right-8);}
else{for(axis=r.top-2;axis<=r.bottom+2;axis++)point(h,axis,r.left+8);for(j=0;j<4;j++)for(x=r.left-1;x<=r.right;x++)point(h,j==0?r.top+8:j==1?r.top+40:j==2?r.top+80:r.bottom-8,x);}
GetPort(&after);p+=sprintf(p,"],\"port_preserved\":%d}",before==after);if(mode==5)ShowWindow(w);DisposeControl(h);}
p+=sprintf(p,"],\"buttons\":[");SetRect(&r,40,20,160,40);for(kind=0;kind<3;kind++){h=NewControl(w,&r,"\pPick",true,1,0,1,kind,0);if(!h)return 4;p+=sprintf(p,"%s{\"kind\":%d,\"points\":[",kind?",":"",kind);comma=0;for(j=0;j<8;j++)for(k=0;k<11;k++)point(h,ys[j],xs[k]);p+=sprintf(p,"]}");DisposeControl(h);}
back=NewControl(w,&r,"\pBack",true,0,0,1,pushButProc,0);front=NewControl(w,&r,"\pFront",true,0,0,1,pushButProc,0);p+=sprintf(p,"],\"chain\":[%d,%d,%d],\"find\":[",*(ControlHandle*)((char*)w+140)==back,*(ControlHandle*)*back==front,*(ControlHandle*)*front==back);for(mode=0;mode<5;mode++){Point pt={30,100};if(mode==1)HideControl(front);if(mode==2){ShowControl(front);HiliteControl(front,255);}if(mode==3)HideControl(back);if(mode==4){DisposeControl(front);DisposeControl(back);}found=(ControlHandle)0x12345678;kind=FindControl(pt,w,&found);p+=sprintf(p,"%s[%d,%d]",mode?",":"",kind,found==front?2:found==back?1:found==0?0:-1);}
p+=sprintf(p,"]}}\n");e=Create("\pControlHitProbe.json",0,'ttxt','TEXT');if(e)return 5;if(FSOpen("\pControlHitProbe.json",0,&file))return 6;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?7:0;}
