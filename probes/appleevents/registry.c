#include <QuickDraw.h>
#include <AppleEvents.h>
#include <Files.h>
#include <stdio.h>
static char output[8192];
static pascal OSErr first(const AppleEvent *event,AppleEvent *reply,long refcon){return 0;}
static pascal OSErr second(const AppleEvent *event,AppleEvent *reply,long refcon){return 0;}
static AEEventHandlerUPP h1,h2;
static char *get(char *p,Boolean system){AEEventHandlerUPP handler=(AEEventHandlerUPP)0x12345678;long refcon=0x12345678;OSErr e=AEGetEventHandler('CRt9','Test',&handler,&refcon,system);return p+sprintf(p,"{\"error\":%d,\"handler\":[%d,%d,%d,%d],\"refcon\":%ld}",e,handler==h1,handler==h2,handler==(AEEventHandlerUPP)0x12345678,handler==0,refcon);}
int main(void){char *p=output;short file;long n;OSErr e;InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();h1=NewAEEventHandlerUPP(first);h2=NewAEEventHandlerUPP(second);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"apple-event-handlers-v1\",\"handlers\":{\"missing\":");p=get(p,0);
 p+=sprintf(p,",\"install\":%d",AEInstallEventHandler('CRt9','Test',h1,0x10203040,0));p+=sprintf(p,",\"installed\":");p=get(p,0);
 p+=sprintf(p,",\"replace\":%d",AEInstallEventHandler('CRt9','Test',h2,-123,0));p+=sprintf(p,",\"replaced\":");p=get(p,0);
 p+=sprintf(p,",\"remove_wrong\":%d",AERemoveEventHandler('CRt9','Test',h1,0));p+=sprintf(p,",\"after_wrong\":");p=get(p,0);
 p+=sprintf(p,",\"remove\":%d",AERemoveEventHandler('CRt9','Test',h2,0));p+=sprintf(p,",\"removed\":");p=get(p,0);p+=sprintf(p,",\"remove_again\":%d",AERemoveEventHandler('CRt9','Test',h2,0));
 p+=sprintf(p,",\"nil_install\":%d",AEInstallEventHandler('CRt9','Test',0,99,0));p+=sprintf(p,",\"nil_lookup\":");p=get(p,0);p+=sprintf(p,",\"nil_remove\":%d",AERemoveEventHandler('CRt9','Test',0,0));
 p+=sprintf(p,",\"system_install\":%d",AEInstallEventHandler('CRt9','Test',h1,456,1));p+=sprintf(p,",\"system_lookup\":");p=get(p,1);p+=sprintf(p,",\"application_lookup\":");p=get(p,0);
 p+=sprintf(p,",\"system_remove\":%d",AERemoveEventHandler('CRt9','Test',h1,1));p+=sprintf(p,",\"system_removed\":");p=get(p,1);p+=sprintf(p,"}}\n");
 DisposeAEEventHandlerUPP(h1);DisposeAEEventHandlerUPP(h2);e=Create("\pAEHandlersProbe.json",0,'ttxt','TEXT');if(e&&e!=dupFNErr)return 2;if(FSOpen("\pAEHandlersProbe.json",0,&file))return 3;SetEOF(file,0);n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;}
