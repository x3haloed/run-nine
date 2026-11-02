#include <QuickDraw.h>
#include <Fonts.h>
#include <Windows.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char out[16384];
int main(void){
 Rect r={60,50,180,250};WindowPtr w;GrafPtr wm;Str255 title;char *p=out;short i,f;long n;OSErr err;
 char *samples[]={"One","Renamed","Reference","Untitled","AV","To","WA","d","dd","Rd","dR","R"};
 InitGraf(&qd.thePort);InitFonts();InitWindows();GetWMgrPort(&wm);w=NewWindow(0,&r,"\p",false,documentProc,(WindowPtr)-1,true,0);
 SetPort(wm);TextFont(0);TextSize(12);TextFace(0);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"title-metrics-v1\",\"metrics\":{\"title_single\":[");
 for(i=0;i<256;i++){title[0]=1;title[1]=i;SetWTitle(w,title);p+=sprintf(p,"%s%d",i?",":"",((WindowPeek)w)->titleWidth);}
 p+=sprintf(p,"],\"char_single\":[");for(i=0;i<256;i++)p+=sprintf(p,"%s%d",i?",":"",CharWidth(i));
 p+=sprintf(p,"],\"string_single\":[");for(i=0;i<256;i++){title[0]=1;title[1]=i;p+=sprintf(p,"%s%d",i?",":"",StringWidth(title));}
 p+=sprintf(p,"],\"samples\":{");for(i=0;i<12;i++){title[0]=strlen(samples[i]);memcpy(title+1,samples[i],title[0]);SetWTitle(w,title);p+=sprintf(p,"%s\"%s\":[%d,%d]",i?",":"",samples[i],((WindowPeek)w)->titleWidth,StringWidth(title));}
 p+=sprintf(p,"}}}\n");DisposeWindow(w);err=Create("\pTitleMetricsProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pTitleMetricsProbe.json",0,&f))return 3;
 SetEOF(f,0);n=p-out;err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;
}
