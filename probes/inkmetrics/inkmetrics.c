/* Record geometry only, never glyph bitmap artwork. */
#include <QuickDraw.h>
#include <Fonts.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static unsigned char pixels[96*32];
static char out[32768];
static void bounds(short *left,short *right){
 short x,y;*left=256;*right=-1;
 for(y=0;y<96;y++)for(x=0;x<256;x++)if(pixels[y*32+x/8]&(128>>(x%8))){
  if(x<*left)*left=x;if(x>*right)*right=x;
 }
 if(*right<0){*left=0;*right=0;}else{*left-=32;*right-=31;}
}
int main(void){
 GrafPort port;BitMap bitmap;short i,f,l,r,font;long n;OSErr err;char *p=out;
 InitGraf(&qd.thePort);InitFonts();OpenPort(&port);
 bitmap.baseAddr=(Ptr)pixels;bitmap.rowBytes=32;SetRect(&bitmap.bounds,0,0,256,96);
 SetPortBits(&bitmap);port.portRect=bitmap.bounds;RectRgn(port.visRgn,&bitmap.bounds);ClipRect(&bitmap.bounds);
 TextFont(0);TextSize(12);TextFace(0);TextMode(srcCopy);ForeColor(blackColor);BackColor(whiteColor);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"ink-metrics-v1\",\"font\":0,\"size\":12,\"face\":0,\"glyphs\":[");
 for(i=0;i<256;i++){
  memset(pixels,0,sizeof(pixels));MoveTo(32,48);DrawChar(i);bounds(&l,&r);
  p+=sprintf(p,"%s[%d,%d,%d]",i?",":"",CharWidth(i),l,r);
 }
 GetFNum("\pCharcoal",&font);TextFont(font);
 p+=sprintf(p,"],\"charcoal_font_id\":%d,\"charcoal_glyphs\":[",font);
 for(i=0;i<256;i++){
  memset(pixels,0,sizeof(pixels));MoveTo(32,48);DrawChar(i);bounds(&l,&r);
  p+=sprintf(p,"%s[%d,%d,%d]",i?",":"",CharWidth(i),l,r);
 }
 p+=sprintf(p,"],\"outline_glyphs\":[");TextFont(0);
 for(i=0;i<256;i++){
  char c=i;Point unit={1,1};short ymax=0,ymin=0;Fixed advance=0,bearing=0;Rect b={0,0,0,0};
  err=OutlineMetrics(1,&c,unit,unit,&ymax,&ymin,&advance,&bearing,&b);
  p+=sprintf(p,"%s[%d,%ld,%ld,%d,%d,%d,%d]",i?",":"",err,advance,bearing,b.top,b.left,b.bottom,b.right);
 }
 p+=sprintf(p,"],\"bold_glyphs\":[");TextFace(1);
 for(i=0;i<256;i++){
  memset(pixels,0,sizeof(pixels));MoveTo(32,48);DrawChar(i);bounds(&l,&r);
  p+=sprintf(p,"%s[%d,%d,%d]",i?",":"",CharWidth(i),l,r);
 }
 p+=sprintf(p,"]}\n");ClosePort(&port);
 err=Create("\pInkMetricsProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;
 if(FSOpen("\pInkMetricsProbe.json",0,&f))return 3;
 SetEOF(f,0);n=p-out;err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;
}
