#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[16384],*p=output;
static short file,count,seen,entry_button,callback_button,callback_part,callback_value,callback_hilite,callback_owner;
static WindowPtr window;
static ControlHandle current;
static void persist(void){long n=p-output+4;memcpy(p,"]}}\n",4);SetFPos(file,fsFromStart,0);SetEOF(file,0);FSWrite(file,&n,output);FlushVol(0,0);}
static pascal void action(ControlHandle h,short part){GrafPtr port;short delta;if(seen)return;seen=1;callback_button=Button();callback_part=part;callback_value=GetControlValue(h);callback_hilite=(**h).contrlHilite;GetPort(&port);callback_owner=(h==current)&&(port==window);delta=part==20?-1:part==21?1:part==22?-10:part==23?10:0;SetControlValue(h,GetControlValue(h)+delta);}
static pascal void indicator(void){GrafPtr port;if(seen)return;seen=1;callback_part=129;callback_value=GetControlValue(current);callback_hilite=(**current).contrlHilite;GetPort(&port);callback_owner=port==window;}
int main(void){Rect b={100,100,420,580},r={20,20,220,36};ControlHandle controls[2];ControlActionUPP proc;EventRecord e;WindowPtr hit;Point start,end;GrafPtr before,after;short part,index,result;Str255 title;OSErr err;
InitGraf(&qd.thePort);InitFonts();InitWindows();InitCursor();window=NewWindow(0,&b,"\pControl tracking: 0/8",true,documentProc,(WindowPtr)-1,false,0);if(!window)return 2;SetPort(window);controls[0]=NewControl(window,&r,"\p",true,25,0,100,scrollBarProc,0);OffsetRect(&r,60,0);controls[1]=NewControl(window,&r,"\p",true,25,0,100,scrollBarProc,0);proc=NewControlActionUPP(action);
err=Create("\pControlTrackProbe.json",0,'ttxt','TEXT');if(err)return 3;if(FSOpen("\pControlTrackProbe.json",0,&file))return 4;p+=sprintf(p,"{\"schema\":1,\"probe\":\"control-action-v1\",\"controls\":{\"cases\":[");persist();
while(count<8){if(!WaitNextEvent(everyEvent,&e,1,0))continue;if(e.what==updateEvt){BeginUpdate(window);DrawControls(window);EndUpdate(window);continue;}if(e.what!=mouseDown||FindWindow(e.where,&hit)!=inContent||hit!=window)continue;SetPort(window);start=e.where;GlobalToLocal(&start);part=FindControl(start,window,&current);if(!part||part==129)continue;index=current==controls[1];seen=callback_part=callback_value=callback_hilite=callback_owner=callback_button=0;entry_button=Button();GetPort(&before);result=TrackControl(current,start,index?0:part==129?(ControlActionUPP)indicator:proc);GetPort(&after);GetMouse(&end);
p+=sprintf(p,"%s{\"control\":%d,\"button_down\":%d,\"start\":[%d,%d],\"end\":[%d,%d],\"initial_part\":%d,\"result\":%d,\"value\":%d,\"hilite\":%d,\"port_preserved\":%d,\"callback\":[%d,%d,%d,%d,%d,%d]}",count?",":"",index,entry_button,start.v,start.h,end.v,end.h,part,result,GetControlValue(current),(**current).contrlHilite,before==after,seen,callback_part,callback_value,callback_hilite,callback_owner,callback_button);count++;persist();SetControlValue(controls[0],25);SetControlValue(controls[1],25);sprintf((char*)title+1,"%d/8 part%d at%d,%d",count,part,start.v,start.h);title[0]=strlen((char*)title+1);SetWTitle(window,title);}
FSClose(file);FlushVol(0,0);return 0;}
