#include <QuickDraw.h>
#include <Windows.h>
#include <Fonts.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char out[8192];
static char *snapshot(char *p,char *name,ControlHandle h,WindowPtr w) {
    ControlPtr c=*h;
    return p+sprintf(p,"\"%s\":{\"rect\":[%d,%d,%d,%d],\"visible\":%u,\"hilite\":%u,\"range\":[%d,%d,%d],\"refcon\":%ld,\"owner_matches\":%d,\"head_matches\":%d,\"title_length\":%u}",name,
       c->contrlRect.top,c->contrlRect.left,c->contrlRect.bottom,c->contrlRect.right,
       (unsigned char)c->contrlVis,(unsigned char)c->contrlHilite,c->contrlValue,c->contrlMin,c->contrlMax,c->contrlRfCon,c->contrlOwner==w,
       *(ControlHandle*)((char*)w+140)==h,(unsigned char)c->contrlTitle[0]);
}
int main(void) {
    Rect bounds={60,50,240,350},r={0,284,180,300};WindowPtr w;ControlHandle h;char *p=out;short f;long n;OSErr err,missing;ParamBlockRec pb;
    InitGraf(&qd.thePort);InitFonts();InitWindows();
    w=NewWindow(0,&bounds,"\pControls",true,documentProc,(WindowPtr)-1,true,0);
    h=NewControl(w,&r,"\pScroll",true,25,0,100,scrollBarProc,0x12345678);if(!h)return 5;
    p+=sprintf(p,"{\"schema\":1,\"probe\":\"controls-v1\",\"controls\":{");p=snapshot(p,"initial",h,w);
    SetControlValue(h,200);p+=sprintf(p,",");p=snapshot(p,"clamp_high",h,w);
    SetControlMinimum(h,50);SetControlMaximum(h,80);SetControlValue(h,-1);p+=sprintf(p,",");p=snapshot(p,"clamp_low",h,w);
    MoveControl(h,12,34);SizeControl(h,16,120);p+=sprintf(p,",");p=snapshot(p,"geometry",h,w);
    HiliteControl(h,255);HideControl(h);p+=sprintf(p,",");p=snapshot(p,"hidden",h,w);
    ShowControl(h);HiliteControl(h,0);p+=sprintf(p,",");p=snapshot(p,"shown",h,w);
    DisposeControl(h);p+=sprintf(p,",\"disposed_head_null\":%d},",*(ControlHandle*)((char*)w+140)==0);
    memset(&pb,0,sizeof(pb));pb.ioParam.ioNamePtr=(StringPtr)"\p.NoSuchCodexDriver";missing=PBOpenSync(&pb);
    p+=sprintf(p,"\"services\":{\"missing_driver\":%d,\"missing_driver_pb\":%d}}\n",missing,pb.ioParam.ioResult);
    DisposeWindow(w);err=Create("\pControlProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;
    if(FSOpen("\pControlProbe.json",0,&f))return 3;SetEOF(f,0);n=p-out;err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;
}
