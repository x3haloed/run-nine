/* Anchored measurements separate glyph placement from text ink extents. */
#include <QuickDraw.h>
#include <Fonts.h>
#include <Windows.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char out[32768];
static char *table(char *p,const char *key,WindowPtr w,unsigned char anchor,short mode){
 short i;Str255 text;p+=sprintf(p,"\"%s\":[",key);
 for(i=0;i<256;i++){
  text[0]=mode?2:1;
  text[1]=mode==1?anchor:i;if(mode)text[2]=mode==2?anchor:i;
  SetWTitle(w,text);p+=sprintf(p,"%s%d",i?",":"",((WindowPeek)w)->titleWidth);
 }
 return p+sprintf(p,"]");
}
int main(void){
 Rect r={60,30,180,630};WindowPtr w;GrafPtr wm;Str255 title;short i,j,f;long n;OSErr err;char *p=out;unsigned long seed=0x51444d47;
 unsigned char alphabet[]="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 !?.,-/\x80\x81\x8e\xc4\xff";
 InitGraf(&qd.thePort);InitFonts();InitWindows();GetWMgrPort(&wm);SetPort(wm);TextFont(0);TextSize(12);TextFace(0);
 w=NewWindow(0,&r,"\p",false,documentProc,(WindowPtr)-1,true,0);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"title-bounds-v1\",\"advances\":[");for(i=0;i<256;i++)p+=sprintf(p,"%s%d",i?",":"",CharWidth(i));p+=sprintf(p,"],");
 p=table(p,"single",w,0,0);p+=sprintf(p,",");p=table(p,"prefix_m",w,'M',1);p+=sprintf(p,",");p=table(p,"suffix_m",w,'M',2);p+=sprintf(p,",");
 p=table(p,"prefix_r",w,'R',1);p+=sprintf(p,",");p=table(p,"suffix_r",w,'R',2);p+=sprintf(p,",");
 p=table(p,"leading_space",w,' ',1);p+=sprintf(p,",");p=table(p,"trailing_space",w,' ',2);
 p+=sprintf(p,",\"heldout\":[");
 for(i=0;i<128;i++){
  seed=seed*1664525UL+1013904223UL;title[0]=1+(seed>>24)%31;p+=sprintf(p,"%s{\"bytes\":[",i?",":"");
  for(j=1;j<=title[0];j++){seed=seed*1664525UL+1013904223UL;title[j]=alphabet[(seed>>24)%(sizeof(alphabet)-1)];p+=sprintf(p,"%s%u",j==1?"":",",title[j]);}
  SetWTitle(w,title);p+=sprintf(p,"],\"width\":%d}",((WindowPeek)w)->titleWidth);
 }
 p+=sprintf(p,"]}\n");DisposeWindow(w);err=Create("\pTitleBoundsProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pTitleBoundsProbe.json",0,&f))return 3;
 SetEOF(f,0);n=p-out;err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;
}
