#include <QuickDraw.h>
#include <Fonts.h>
#include <Windows.h>
#include <Dialogs.h>
#include <StandardFile.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
typedef struct {short mode,nulls,idles,count;long magic;short calls[32][4];} State;
static char output[8192];static State state;
static void log_call(State *s,short tag,short value,DialogPtr d,void *data){if(s->count<32){short *r=s->calls[s->count++];r[0]=tag;r[1]=value;r[2]=GetWRefCon(d)=='stdf';r[3]=data==&state&&s->magic==0x13572468;}}
static pascal short hook(short item,DialogPtr d,void *data){State *s=data;if(item==100)s->idles++;else log_call(s,1,item,d,data);return item==11?2:item;}
static pascal Boolean filter(DialogPtr d,EventRecord *e,short *item,void *data){State *s=data;if(e->what!=nullEvent)return false;s->nulls++;log_call(s,2,s->nulls,d,data);if(s->mode==1&&s->nulls==1){*item=21;return false;}*item=s->mode==2?11:2;return true;}
static char *run(char *p,short mode){StandardFileReply reply;Point where={-1,-1};short i;memset(&state,0,sizeof(state));state.mode=mode;state.magic=0x13572468;memset(&reply,0x5a,sizeof(reply));CustomPutFile("\pCallback probe","\pfilter.txt",&reply,399,where,NewDlgHookYDUPP(hook),NewModalFilterYDUPP(filter),0,0,&state);p+=sprintf(p,"{\"mode\":%d,\"good\":%u,\"null_calls\":%d,\"idle_hooks\":%d,\"name\":[",mode,reply.sfGood,state.nulls,state.idles);for(i=0;i<reply.sfFile.name[0];i++)p+=sprintf(p,"%s%u",i?",":"",reply.sfFile.name[i+1]);p+=sprintf(p,"],\"calls\":[");for(i=0;i<state.count;i++)p+=sprintf(p,"%s[%d,%d,%d,%d]",i?",":"",state.calls[i][0],state.calls[i][1],state.calls[i][2],state.calls[i][3]);return p+sprintf(p,"]}");}
static pascal void user_item(DialogPtr d,short item){(void)d;(void)item;}
static char *item_state(char *p,DialogPtr d,Handle expected){short kind,i;Handle h;Rect r;unsigned char *at=(unsigned char*)*(((DialogPeek)d)->items);GetDialogItem(d,12,&kind,&h,&r);at+=2;for(i=1;i<12;i++)at+=14+at[13]+(at[13]&1);return p+sprintf(p,"{\"kind\":%d,\"rect\":[%d,%d,%d,%d],\"handle_matches\":%d,\"memory_matches\":%d}",kind,r.top,r.left,r.bottom,r.right,h==expected,*(Handle*)at==h&&at[12]==(unsigned char)kind&&!memcmp(at+4,&r,8));}
static char *item_changes(char *p){DialogPtr d=GetNewDialog(1001,0,(WindowPtr)-1);short kind;Handle old,h=(Handle)NewUserItemUPP(user_item);Rect r={40,55,65,125},original;GetDialogItem(d,12,&kind,&old,&original);p+=sprintf(p,"{");p+=sprintf(p,"\"initial\":");p=item_state(p,d,old);SetDialogItem(d,12,128,h,&r);p+=sprintf(p,",\"changed\":");p=item_state(p,d,h);SetDialogItem(d,12,kind,old,&original);p+=sprintf(p,",\"restored\":");p=item_state(p,d,old);DisposeDialog(d);return p+sprintf(p,"}");}
int main(void){char *p=output;short i,f;long n;OSErr e;InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();TEInit();InitDialogs(0);p+=sprintf(p,"{\"schema\":1,\"probe\":\"modal-filter-v1\",\"dialogs\":{\"cases\":[");for(i=0;i<3;i++){p+=sprintf(p,"%s",i?",":"");p=run(p,i);}p+=sprintf(p,"],\"item_changes\":");p=item_changes(p);p+=sprintf(p,"}}\n");e=Create("\pModalFilterProbe.json",0,'ttxt','TEXT');if(e&&e!=dupFNErr)return 2;if(FSOpen("\pModalFilterProbe.json",0,&f))return 3;SetEOF(f,0);n=p-output;e=FSWrite(f,&n,output);FSClose(f);FlushVol(0,0);return e?4:0;}
