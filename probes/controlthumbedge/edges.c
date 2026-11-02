#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[16384], *p=output;
static short file,count;
static WindowPtr window;
static ControlHandle control;
static void persist(void) {
    long n=p-output+4; memcpy(p,"]}}\n",4);
    SetFPos(file,fsFromStart,0);SetEOF(file,0);FSWrite(file,&n,output);FlushVol(0,0);
}
static void setup(void) {
    Rect r={0,552,420,567};Str255 title;
    SetPort(window);if(control)DisposeControl(control);
    EraseRect(&window->portRect);
    if(count==6){r.top=200;r.left=20;r.bottom=215;r.right=420;}
    control=NewControl(window,&r,"\p",true,25,0,100,scrollBarProc,0);
    sprintf((char*)title+1,"Edges %d/8: %s",count,count==6?"horizontal":"vertical");
    title[0]=strlen((char*)title+1);SetWTitle(window,title);
}
int main(void) {
    Rect b={41,4,476,571},r;EventRecord e;WindowPtr hit;Point start,entry,end;
    short part,button,result,value;unsigned long before,after;OSErr err;
    InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();InitCursor();
    window=NewWindow(0,&b,"\pThumb edges",true,documentProc,(WindowPtr)-1,false,0);
    if(!window)return 2;
    err=Create("\pControlThumbEdgesProbe.json",0,'pPrb','TEXT');
    if(err&&err!=dupFNErr)return 3;
    if(FSOpen("\pControlThumbEdgesProbe.json",0,&file))return 4;
    p+=sprintf(p,"{\"schema\":1,\"probe\":\"control-thumb-edges-v1\",\"controls\":{\"cases\":[");persist();setup();
    while(count<8) {
        if(!WaitNextEvent(everyEvent,&e,1,0))continue;
        if(e.what==updateEvt){BeginUpdate(window);DrawControls(window);EndUpdate(window);continue;}
        if(e.what!=mouseDown||FindWindow(e.where,&hit)!=inContent||hit!=window)continue;
        SetPort(window);start=e.where;GlobalToLocal(&start);part=TestControl(control,start);
        if(part!=129)continue;
        r=(**control).contrlRect;value=GetControlValue(control);
        GetMouse(&entry);button=Button();before=TickCount();
        result=TrackControl(control,start,0);after=TickCount();GetMouse(&end);
        p+=sprintf(p,"%s{\"case\":%d,\"rect\":[%d,%d,%d,%d],\"initial_value\":%d,\"start\":[%d,%d],\"entry_mouse\":[%d,%d],\"entry_button\":%d,\"event_age\":%lu,\"duration\":%lu,\"end\":[%d,%d],\"result\":%d,\"value\":%d,\"hilite\":%d}",count?",":"",count,r.top,r.left,r.bottom,r.right,value,start.v,start.h,entry.v,entry.h,button,before-e.when,after-before,end.v,end.h,result,GetControlValue(control),(**control).contrlHilite);
        count++;persist();if(count<8)setup();
    }
    FSClose(file);FlushVol(0,0);return 0;
}
