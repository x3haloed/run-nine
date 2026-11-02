#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[65536],*p=output;
static short file,count;
static void persist(void){long n=p-output+4;memcpy(p,"]}}\n",4);SetFPos(file,fsFromStart,0);SetEOF(file,0);FSWrite(file,&n,output);FlushVol(0,0);}
int main(void){WindowPtr w;Rect b={60,60,400,540};EventRecord e;unsigned char keys[16];Str255 title;short i;Point mouse;OSErr err;
InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();InitCursor();w=NewWindow(0,&b,"\pInput events",true,documentProc,(WindowPtr)-1,false,0);if(!w)return 2;SetPort(w);TextFont(0);TextSize(12);
err=Create("\pInputEventsProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 3;if(FSOpen("\pInputEventsProbe.json",0,&file))return 4;p+=sprintf(p,"{\"schema\":1,\"probe\":\"input-events-v1\",\"input\":{\"events\":[");persist();
while(count<40){if(!WaitNextEvent(everyEvent,&e,1,0))continue;if(e.what==updateEvt){BeginUpdate(w);EraseRect(&w->portRect);MoveTo(15,25);DrawString("\pNative input: keys, modifiers and mouse coordinates.");EndUpdate(w);continue;}if(e.what!=mouseDown&&e.what!=mouseUp&&e.what!=keyDown&&e.what!=keyUp&&e.what!=autoKey)continue;SetPort(w);GetMouse(&mouse);LocalToGlobal(&mouse);GetKeys(keys);
p+=sprintf(p,"%s{\"what\":%d,\"message\":%lu,\"modifiers\":%u,\"where\":[%d,%d],\"mouse\":[%d,%d],\"button\":%d,\"keys\":[",count?",":"",e.what,e.message,e.modifiers,e.where.v,e.where.h,mouse.v,mouse.h,Button());for(i=0;i<16;i++)p+=sprintf(p,"%s%u",i?",":"",((unsigned char*)keys)[i]);p+=sprintf(p,"]}");count++;persist();sprintf((char*)title+1,"%d event%d char%lu mods%u at%d,%d",count,e.what,e.message&255,e.modifiers,e.where.v,e.where.h);title[0]=strlen((char*)title+1);SetWTitle(w,title);}
FSClose(file);FlushVol(0,0);return 0;}
