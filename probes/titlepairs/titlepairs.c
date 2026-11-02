#include <QuickDraw.h>
#include <Fonts.h>
#include <Windows.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char row[4096];
static OSErr write_part(short file,char *p){long n=p-row;return FSWrite(file,&n,row);}
int main(void){
 Rect r={60,30,180,630};WindowPtr w;GrafPtr wm;Str255 text;short a,b,file;char *p;long zero=0;OSErr err;unsigned long seed=0x474c5950;
 InitGraf(&qd.thePort);InitFonts();InitWindows();GetWMgrPort(&wm);SetPort(wm);TextFont(0);TextSize(12);TextFace(0);
 w=NewWindow(0,&r,"\p",false,documentProc,(WindowPtr)-1,true,0);
 err=Create("\pTitlePairsProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pTitlePairsProbe.json",0,&file))return 3;SetEOF(file,0);
 p=row+sprintf(row,"{\"schema\":1,\"probe\":\"title-pairs-v1\",\"pairs\":[");if(write_part(file,p))return 4;
 text[0]=2;
 for(a=0;a<256;a++){
  p=row+sprintf(row,"%s[",a?",":"");text[1]=a;
  for(b=0;b<256;b++){text[2]=b;SetWTitle(w,text);p+=sprintf(p,"%s%d",b?",":"",((WindowPeek)w)->titleWidth);}
  p+=sprintf(p,"]");if(write_part(file,p))return 4;
 }
 p=row+sprintf(row,"],\"long_cases\":[");if(write_part(file,p))return 4;
 for(a=0;a<32;a++){
  seed=seed*1664525UL+1013904223UL;text[0]=128+(seed>>24)%128;p=row+sprintf(row,"%s{\"bytes\":[",a?",":"");
  for(b=1;b<=text[0];b++){seed=seed*1664525UL+1013904223UL;text[b]=seed>>24;p+=sprintf(p,"%s%u",b==1?"":",",text[b]);}
  SetWTitle(w,text);p+=sprintf(p,"],\"width\":%d}",((WindowPeek)w)->titleWidth);if(write_part(file,p))return 4;
 }
 p=row+sprintf(row,"],\"complete\":true}\n");err=write_part(file,p);FSClose(file);FlushVol(0,0);DisposeWindow(w);return err?4:0;
}
