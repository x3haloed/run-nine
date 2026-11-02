#include <QuickDraw.h>
#include <Fonts.h>
#include <Windows.h>
#include <TextEdit.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char out[16384];
static char *snapshot(char *p,char *name,TEHandle h) {
    TEPtr t;short i;HLock((Handle)h);t=*h;
    p+=sprintf(p,"\"%s\":{\"dest\":[%d,%d,%d,%d],\"view\":[%d,%d,%d,%d],\"selection\":[%d,%d],"
       "\"metrics\":[%d,%d],\"font\":[%d,%u,%d,%d],\"flags\":[%d,%d,%d],\"length\":%d,\"lines\":[",
       name,t->destRect.top,t->destRect.left,t->destRect.bottom,t->destRect.right,
       t->viewRect.top,t->viewRect.left,t->viewRect.bottom,t->viewRect.right,t->selStart,t->selEnd,
       t->lineHeight,t->fontAscent,t->txFont,(unsigned char)t->txFace,t->txMode,t->txSize,
       t->active,t->just,t->crOnly,t->teLength);
    for(i=0;i<=t->nLines;i++)p+=sprintf(p,"%s%d",i?",":"",t->lineStarts[i]);
    p+=sprintf(p,"],\"text\":[");HLock(t->hText);
    for(i=0;i<t->teLength;i++)p+=sprintf(p,"%s%u",i?",":"",(unsigned char)(*t->hText)[i]);
    HUnlock(t->hText);p+=sprintf(p,"]}");HUnlock((Handle)h);return p;
}
#ifndef TEXTEDIT_SNAPSHOT_ONLY
int main(void) {
    Rect bounds={60,50,240,350},text={4,4,164,284};WindowPtr w;TEHandle h;char *p=out;short f;long n;OSErr err;
    InitGraf(&qd.thePort);InitFonts();InitWindows();TEInit();
    w=NewWindow(0,&bounds,"\pText",true,documentProc,(WindowPtr)-1,true,0);
    SetPort(w);TextFont(1);TextSize(12);h=TENew(&text,&text);if(!h)return 5;
    p+=sprintf(p,"{\"schema\":1,\"probe\":\"textedit-v1\",\"textedit\":{");p=snapshot(p,"initial",h);
    TESetText("one two\rthree",13,h);p+=sprintf(p,",");p=snapshot(p,"set",h);
    TESetSelect(4,7,h);TEInsert("X",1,h);p+=sprintf(p,",");p=snapshot(p,"insert",h);
    TEKey(8,h);p+=sprintf(p,",");p=snapshot(p,"backspace",h);
    TEKey('Z',h);p+=sprintf(p,",");p=snapshot(p,"key",h);
    TESetSelect(0,32767,h);p+=sprintf(p,",");p=snapshot(p,"select_all",h);
    TEDelete(h);p+=sprintf(p,",");p=snapshot(p,"delete",h);p+=sprintf(p,"}}\n");
    TEDispose(h);DisposeWindow(w);
    err=Create("\pTextEditProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;
    if(FSOpen("\pTextEditProbe.json",0,&f))return 3;SetEOF(f,0);n=p-out;err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;
}
#endif
