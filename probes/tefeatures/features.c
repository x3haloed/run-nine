#include <QuickDraw.h>
#include <Fonts.h>
#include <TextEdit.h>
#include <Windows.h>
#include <Files.h>
#include <stdio.h>
static char output[8192];
static long pixels[480];
static long pixel(short x,short y){RGBColor c;GetCPixel(x,y,&c);return ((long)(c.red>>8)<<16)|((long)(c.green>>8)<<8)|(c.blue>>8);}
static char *outline_effect(char *p,TEHandle h){Rect r={30,20,100,200};short x,y;long count=0;short top=32767,left=32767,bottom=-32768,right=-32768;TESetText("ABCD",4,h);TESetSelect(1,3,h);TEDeactivate(h);TEUpdate(&r,h);for(y=28;y<44;y++)for(x=20;x<50;x++)pixels[(y-28)*30+x-20]=pixel(x,y);TEFeatureFlag(2,1,h);TEActivate(h);TEDeactivate(h);for(y=28;y<44;y++)for(x=20;x<50;x++)if(pixels[(y-28)*30+x-20]!=pixel(x,y)){count++;if(y<top)top=y;if(x<left)left=x;if(y+1>bottom)bottom=y+1;if(x+1>right)right=x+1;}return p+sprintf(p,",[%ld,%d,%d,%d,%d]",count,top,left,bottom,right);}
static char *run(char *p,Boolean styled){Rect r={30,20,100,200};TEHandle h=styled?TEStyleNew(&r,&r):TENew(&r,&r);short feature,a,b,c,d,e,i;if(!h)return 0;p+=sprintf(p,"[");for(feature=0;feature<5;feature++){a=TEFeatureFlag(feature,-1,h);b=TEFeatureFlag(feature,1,h);c=TEFeatureFlag(feature,-1,h);d=TEFeatureFlag(feature,0,h);e=TEFeatureFlag(feature,-1,h);p+=sprintf(p,"%s[%d,%d,%d,%d,%d,%d]",feature?",":"",feature,a,b,c,d,e);}TEAutoView(1,h);p+=sprintf(p,",[%d,%d,%d]",TEFeatureFlag(0,-1,h),TEFeatureFlag(2,-1,h),(*h)->clikStuff);p=outline_effect(p,h);p+=sprintf(p,"]");TEDispose(h);return p;}
int main(void){char *p=output;short file;long n;OSErr e;InitGraf(&qd.thePort);InitFonts();InitWindows();TEInit();TextFont(4);TextFace(0);TextSize(9);p+=sprintf(p,"{\"schema\":1,\"probe\":\"te-features-v1\",\"textedit\":{\"plain\":");p=run(p,0);if(!p)return 5;p+=sprintf(p,",\"styled\":");p=run(p,1);if(!p)return 6;p+=sprintf(p,"}}\n");e=Create("\pTEFeaturesProbe.json",0,'ttxt','TEXT');if(e&&e!=dupFNErr)return 2;if(FSOpen("\pTEFeaturesProbe.json",0,&file))return 3;SetEOF(file,0);n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;}
