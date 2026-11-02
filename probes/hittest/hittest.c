#include <QuickDraw.h>
#include <Fonts.h>
#include <Windows.h>
#include <TextEdit.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char out[32768];
int main(void){
 Rect wr={80,60,220,300},tr={20,10,100,190};WindowPtr w,found;GrafPort port;TEHandle te;
 Point points[]={{10,100},{300,400},{100,100},{70,150},{70,70},{215,295},{80,60},{79,59},{220,300},{220,100},{100,59}};
 short xs[]={-10,10,14,15,16,20,24,28,45,100,200},ys[]={0,20,24,34,35,39,50,100};
 short i,j,part,f;long n;OSErr err;char *p=out;char text[]="One two\rABC";
 InitGraf(&qd.thePort);InitFonts();InitWindows();TEInit();
 w=NewWindow(0,&wr,"\pHit",true,documentProc,(WindowPtr)-1,true,0);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"hit-test-v1\",\"windows\":{\"visible\":[");
 for(i=0;i<11;i++){found=(WindowPtr)0x1234;part=FindWindow(points[i],&found);p+=sprintf(p,"%s[%d,%d]",i?",":"",part,found==w?1:found==0?0:-1);}
 p+=sprintf(p,"],\"hilited\":%d,\"header_grid\":[",((WindowPeek)w)->hilited);
 for(i=58;i<80;i++){p+=sprintf(p,"%s[",i==58?"":",");for(j=54;j<116;j++){Point pt={i,j};part=FindWindow(pt,&found);p+=sprintf(p,"%s%d",j==54?"":",",part);}p+=sprintf(p,"]");}
 HideWindow(w);p+=sprintf(p,"],\"hidden\":[");
 for(i=0;i<11;i++){found=(WindowPtr)0x1234;part=FindWindow(points[i],&found);p+=sprintf(p,"%s[%d,%d]",i?",":"",part,found==w?1:found==0?0:-1);}
 DisposeWindow(w);OpenPort(&port);TextFont(3);TextSize(12);TextFace(0);te=TENew(&tr,&tr);TESetText(text,strlen(text),te);
 p+=sprintf(p,"]},\"textedit\":{\"line_height\":%d,\"ascent\":%d,\"offsets\":[",(*te)->lineHeight,(*te)->fontAscent);
 for(i=0;i<8;i++){p+=sprintf(p,"%s[",i?",":"");for(j=0;j<11;j++){Point pt={ys[i],xs[j]};p+=sprintf(p,"%s%d",j?",":"",TEGetOffset(pt,te));}p+=sprintf(p,"]");}
 p+=sprintf(p,"],\"offset_sweep\":[");
 for(i=0;i<3;i++){short just=i==0?0:i==1?1:-1;short y;TESetAlignment(just,te);p+=sprintf(p,"%s[",i?",":"");
  for(y=0;y<2;y++){p+=sprintf(p,"%s[",y?",":"");for(j=-10;j<=200;j++){Point pt={y?39:20,j};p+=sprintf(p,"%s%d",j==-10?"":",",TEGetOffset(pt,te));}p+=sprintf(p,"]");}p+=sprintf(p,"]");
 }
 p+=sprintf(p,"]}}\n");TEDispose(te);ClosePort(&port);
 err=Create("\pHitTestProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;
 if(FSOpen("\pHitTestProbe.json",0,&f))return 3;SetEOF(f,0);n=p-out;err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;
}
