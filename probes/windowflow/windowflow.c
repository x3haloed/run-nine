#include <QuickDraw.h>
#include <Fonts.h>
#include <Windows.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char out[16384];
static char *snap(char *p,char *name,WindowPtr a,WindowPtr b){
    WindowPeek x=(WindowPeek)a,y=(WindowPeek)b;WindowPtr front=FrontWindow();GrafPtr current;Rect u=(**x->updateRgn).rgnBBox,v=(**y->updateRgn).rgnBBox,vis=(**x->port.visRgn).rgnBBox;
    GetPort(&current);
    return p+sprintf(p,"\"%s\":{\"visible\":[%d,%d],\"hilite\":[%d,%d],\"front\":%d,\"current\":%d,\"title_width\":%d,\"update_a\":[%d,%d,%d,%d],\"update_b\":[%d,%d,%d,%d],\"vis_a\":[%d,%d,%d,%d]}",name,x->visible!=0,y->visible!=0,x->hilited!=0,y->hilited!=0,front==a?1:front==b?2:0,current==a?1:current==b?2:0,x->titleWidth,u.top,u.left,u.bottom,u.right,v.top,v.left,v.bottom,v.right,vis.top,vis.left,vis.bottom,vis.right);
}
int main(void){
    Rect a={60,50,180,250},b={220,300,340,500},dirty={10,20,40,80};WindowPtr x,y;char *p=out;short f;long n;OSErr err;
    InitGraf(&qd.thePort);InitFonts();InitWindows();
    x=NewWindow(0,&a,"\pOne",true,documentProc,(WindowPtr)-1,true,0);y=NewWindow(0,&b,"\pTwo",false,documentProc,(WindowPtr)-1,true,0);
    p+=sprintf(p,"{\"schema\":1,\"probe\":\"window-flow-v1\",\"windows\":{");p=snap(p,"initial",x,y);
    SetWTitle(x,"\pRenamed");p+=sprintf(p,",");p=snap(p,"title",x,y);
    BringToFront(y);p+=sprintf(p,",");p=snap(p,"bring_hidden",x,y);
    ShowWindow(y);p+=sprintf(p,",");p=snap(p,"show",x,y);
    SelectWindow(x);p+=sprintf(p,",");p=snap(p,"select",x,y);
    HideWindow(x);p+=sprintf(p,",");p=snap(p,"hide",x,y);
    ShowWindow(x);p+=sprintf(p,",");p=snap(p,"reshow",x,y);
    SetEmptyRgn(((WindowPeek)x)->updateRgn);SetEmptyRgn(((WindowPeek)y)->updateRgn);SetPort(x);InvalRect(&dirty);p+=sprintf(p,",");p=snap(p,"invalidated",x,y);
    BeginUpdate(x);p+=sprintf(p,",");p=snap(p,"begin",x,y);EndUpdate(x);p+=sprintf(p,",");p=snap(p,"end",x,y);
    p+=sprintf(p,"}}\n");DisposeWindow(y);DisposeWindow(x);
    err=Create("\pWindowFlowProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pWindowFlowProbe.json",0,&f))return 3;
    SetEOF(f,0);n=p-out;err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;
}
