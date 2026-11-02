#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static DialogPtr expected;
static short rows[8][9],count;
static pascal void draw_item(DialogPtr dialog,short item){
 GrafPtr port; PenState pen;short kind;Handle handle;Rect rect;short *r;
 if(count>=8)return;r=rows[count++];GetPort(&port);GetPenState(&pen);
 r[0]=item;r[1]=dialog==expected;r[2]=port==(GrafPtr)dialog;r[3]=port->txFont;r[4]=port->txSize;r[5]=pen.pnSize.h;r[6]=pen.pnSize.v;r[7]=pen.pnMode;r[8]=port->txMode;
 GetDialogItem(dialog,item,&kind,&handle,&rect);PaintRect(&rect);
 TextFont(4);TextSize(18);PenSize(3,4);
}
static void install(DialogPtr d,short item,Handle proc){short kind;Handle old;Rect rect;GetDialogItem(d,item,&kind,&old,&rect);SetDialogItem(d,item,kind,proc,&rect);}
int main(void){GrafPort caller;GrafPtr before,after;PenState pen;DialogPtr d;short i,j,file;long n;char out[2048],*p=out;
 d=GetNewDialog(2001,0,(WindowPtr)-1);if(!d)return 2;expected=d;
 SetPort((GrafPtr)d);TextFont(3);TextSize(12);PenSize(1,2);TextMode(srcXor);PenMode(patXor);
 OpenPort(&caller);SetPort(&caller);TextFont(4);TextSize(18);PenSize(5,6);GetPort(&before);
 install(d,1,(Handle)draw_item);install(d,2,(Handle)draw_item);DrawDialog(d);
 install(d,2,0);DrawDialog(d);
 GetPort(&after);GetPenState(&pen);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"user-items-v1\",\"dialogs\":{\"calls\":[");
 for(i=0;i<count;i++){p+=sprintf(p,"%s[",i?",":"");for(j=0;j<9;j++)p+=sprintf(p,"%s%d",j?",":"",rows[i][j]);p+=sprintf(p,"]");}
 p+=sprintf(p,"],\"caller\":[%d,%d,%d,%d,%d],",after==before,after->txFont,after->txSize,pen.pnSize.h,pen.pnSize.v);
 SetPort((GrafPtr)d);GetPenState(&pen);p+=sprintf(p,"\"dialog\":[%d,%d,%d,%d,%d,%d]}}\n",((GrafPtr)d)->txFont,((GrafPtr)d)->txSize,pen.pnSize.h,pen.pnSize.v,pen.pnMode,((GrafPtr)d)->txMode);
 SetPort(before);DisposeDialog(d);ClosePort(&caller);
 if(Create("\pUserItemsProbe.json",0,'ttxt','TEXT')!=noErr)return 3;if(FSOpen("\pUserItemsProbe.json",0,&file))return 4;n=strlen(out);FSWrite(file,&n,out);FSClose(file);FlushVol(0,0);return 0;
}
