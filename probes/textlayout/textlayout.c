#define TEXTEDIT_SNAPSHOT_ONLY
#include "../textedit/textedit.c"
int main(void) {
    Rect bounds={60,50,240,350},text={4,4,164,64};WindowPtr w;TEHandle h;char *p=out;short f,i;long n;OSErr err;
    char *names[]={"words","unbroken","return_end","return_only","empty_line","spaces","space_overflow"};
    char *texts[]={"one two three four","abcdefghijklmnop","a\r","\r","a\r\rb","one   two   three","abcdefghij   klm"};
    InitGraf(&qd.thePort);InitFonts();InitWindows();TEInit();
    w=NewWindow(0,&bounds,"\pLayout",true,documentProc,(WindowPtr)-1,true,0);SetPort(w);TextFont(1);TextSize(12);
    h=TENew(&text,&text);if(!h)return 5;
    p+=sprintf(p,"{\"schema\":1,\"probe\":\"text-layout-v1\",\"textedit\":{");
    for(i=0;i<7;i++){TESetText(texts[i],strlen(texts[i]),h);if(i)p+=sprintf(p,",");p=snapshot(p,names[i],h);}
    TESetText("abc",3,h);TEActivate(h);p+=sprintf(p,",");p=snapshot(p,"active",h);
    TEDeactivate(h);p+=sprintf(p,",");p=snapshot(p,"inactive",h);
    p+=sprintf(p,"}}\n");TEDispose(h);DisposeWindow(w);
    err=Create("\pTextLayoutProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;
    if(FSOpen("\pTextLayoutProbe.json",0,&f))return 3;SetEOF(f,0);n=p-out;err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;
}
