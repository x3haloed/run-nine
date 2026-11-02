#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[16384],*p=output;
#define POINT(A) (*(volatile Point*)(A))
#define FLAGS (*(volatile unsigned short*)0x8ce)
#pragma pack(push,2)
typedef struct{unsigned long prefix;Point point;unsigned long suffix;}GuardedPoint;
#pragma pack(pop)
static void pair(Point a){p+=sprintf(p,"[%d,%d]",a.v,a.h);}
static void state(void){Point a=POINT(0x828),b=POINT(0x82c),c=POINT(0x830);unsigned short f=FLAGS;GuardedPoint g={0x11223344,{0x5555,0x5555},0x55667788};GetMouse(&g.point);p+=sprintf(p,"{\"mtemp\":");pair(a);p+=sprintf(p,",\"raw_mouse\":");pair(b);p+=sprintf(p,",\"mouse\":");pair(c);p+=sprintf(p,",\"flags_before_get\":%u,\"flags_after_get\":%u,\"get_mouse\":",f,FLAGS);pair(g.point);p+=sprintf(p,",\"prefix\":%lu,\"suffix\":%lu}",g.prefix,g.suffix);}
int main(void){short hide,origin,mask,which,index=0,file;long n;unsigned long unused,start,elapsed;OSErr e;GrafPort port;Point base={200,210},targets[]={{40,60},{200,210},{1,638},{478,1}},saved[3];Rect pin={0,0,480,640},oldpin;unsigned short oldflags;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();TEInit();InitDialogs(0);InitCursor();
 e=Create("\pMouseCouplingProbe.json",0,'mPrb','TEXT');if(e)return 3;if(FSOpen("\pMouseCouplingProbe.json",0,&file))return 4;p+=sprintf(p,"{\"schema\":1,\"probe\":\"mouse-coupling-raw-v1\",\"records\":[");
 n=p-output;FSWrite(file,&n,output);FlushVol(0,0);p=output;
 OpenPort(&port);SetPort(&port);saved[0]=POINT(0x828);saved[1]=POINT(0x82c);saved[2]=POINT(0x830);oldflags=FLAGS;oldpin=*(volatile Rect*)0x834;*(volatile Rect*)0x834=pin;
 for(hide=0;hide<2;hide++){if(hide)HideCursor();for(origin=0;origin<2;origin++){SetOrigin(origin?7:0,origin?11:0);for(mask=0;mask<10;mask++)for(which=0;which<4;which++){
  POINT(0x828)=base;POINT(0x82c)=base;POINT(0x830)=base;FLAGS=0xffff;Delay(2,&unused);
  p+=sprintf(p,"%s{\"hide\":%d,\"origin\":%d,\"mask\":%d,\"which\":%d,\"requested\":",index++?",":"",hide,origin,mask,which);pair(targets[which]);p+=sprintf(p,",\"baseline\":");state();
  if(mask==1||mask==4||mask>=5)POINT(0x828)=targets[which];if(mask==2||mask==4||mask>=5)POINT(0x82c)=targets[which];if(mask==3||mask==5)POINT(0x830)=targets[which];
  if(mask==6)FLAGS=0xffff;else if(mask==7)FLAGS=0;else if(mask==8)FLAGS=0xff00;else if(mask==9)FLAGS=0x00ff;
  p+=sprintf(p,",\"immediate\":");state();start=TickCount();Delay(2,&unused);elapsed=TickCount()-start;p+=sprintf(p,",\"elapsed_ticks\":%lu,\"settled\":",elapsed);state();SystemTask();p+=sprintf(p,",\"after_system_task\":");state();p+=sprintf(p,"}");n=p-output;if(FSWrite(file,&n,output))return 5;p=output;
 }}if(hide)ShowCursor();}
 POINT(0x828)=saved[0];POINT(0x82c)=saved[1];POINT(0x830)=saved[2];*(volatile Rect*)0x834=oldpin;FLAGS=oldflags|0xff00;
 p+=sprintf(p,"]}\n");n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
