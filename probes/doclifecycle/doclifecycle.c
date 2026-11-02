#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[8192],*p=output;static short inspected;
static pascal Boolean inspect(DialogPtr d,EventRecord *event,short *item,void *user){short count,i,kind,j;Handle h;Rect rect;Str255 title;GrafPtr port;
 if(event->what!=nullEvent)return false;if(!inspected){inspected=1;count=*(short*)**(Handle*)((char*)d+156)+1;GetPort(&port);p+=sprintf(p,"\"default_open\":{\"count\":%d,\"port\":[%d,%d,%d,%d],\"items\":[",count,((GrafPtr)d)->portRect.top,((GrafPtr)d)->portRect.left,((GrafPtr)d)->portRect.bottom,((GrafPtr)d)->portRect.right);
 for(i=1;i<=count;i++){GetDialogItem(d,i,&kind,&h,&rect);title[0]=0;if((kind&127)==4||(kind&127)==5||(kind&127)==6)GetControlTitle((ControlHandle)h,title);if((kind&127)==8||(kind&127)==16)GetDialogItemText(h,title);p+=sprintf(p,"%s{\"number\":%d,\"kind\":%d,\"rect\":[%d,%d,%d,%d],\"text\":[",i>1?",":"",i,kind,rect.top,rect.left,rect.bottom,rect.right);for(j=1;j<=title[0];j++)p+=sprintf(p,"%s%d",j>1?",":"",title[j]);p+=sprintf(p,"]}");}p+=sprintf(p,"]}");}
 *item=2;return true;}
int main(void){Rect bounds={40,30,140,230};WindowPtr w,before;Ptr provided;short mode,rows[2][7],file;GrafPtr oldport,after;long size,n;OSErr e;StandardFileReply reply;Point where={-1,-1};
 InitGraf(&qd.thePort);InitFonts();InitWindows();
 for(mode=0;mode<2;mode++){before=FrontWindow();GetPort(&oldport);provided=mode?NewPtrClear(sizeof(WindowRecord)+64):0;w=NewWindow(provided,&bounds,"\pClose",true,documentProc,(WindowPtr)-1,true,0);if(!w)return 2;size=GetPtrSize((Ptr)w);rows[mode][0]=FrontWindow()==w;CloseWindow(w);rows[mode][1]=FrontWindow()==before;rows[mode][2]=GetPtrSize((Ptr)w)==size;rows[mode][3]=MemError();GetPort(&after);rows[mode][5]=after==(GrafPtr)w;rows[mode][6]=after==oldport;SetPort(oldport);DisposePtr((Ptr)w);rows[mode][4]=MemError();}
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"doc-lifecycle-v1\",\"windows\":{\"close\":[[%d,%d,%d,%d,%d,%d,%d],[%d,%d,%d,%d,%d,%d,%d]]},\"dialogs\":{",rows[0][0],rows[0][1],rows[0][2],rows[0][3],rows[0][4],rows[0][5],rows[0][6],rows[1][0],rows[1][1],rows[1][2],rows[1][3],rows[1][4],rows[1][5],rows[1][6]);
 CustomGetFile(0,-1,0,&reply,0,where,0,inspect,0,0,0);p+=sprintf(p,",\"cancelled\":%d}}\n",!reply.sfGood);
 if(!inspected)return 3;e=Create("\pDocLifecycleProbe.json",0,'ttxt','TEXT');if(e)return 4;if(FSOpen("\pDocLifecycleProbe.json",0,&file))return 5;n=strlen(output);e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?6:0;}
