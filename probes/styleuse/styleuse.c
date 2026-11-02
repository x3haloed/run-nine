#include <QuickDraw.h>
#include <Fonts.h>
#include <TextEdit.h>
#include <Windows.h>
#include <Files.h>
#include <Memory.h>
#include <stdio.h>
static char output[32768];
static char *scrap(char *p,StScrpHandle h){short i;ScrpSTElement *e;if(!h)return p+sprintf(p,"null");if((**h).scrpNStyles<0||(**h).scrpNStyles>64)return 0;p+=sprintf(p,"[");for(i=0;i<(**h).scrpNStyles;i++){e=&(**h).scrpStyleTab[i];p+=sprintf(p,"%s[%ld,%d,%d,%d,%u,%d,%u,%u,%u]",i?",":"",e->scrpStartChar,e->scrpHeight,e->scrpAscent,e->scrpFont,e->scrpFace,e->scrpSize,e->scrpColor.red,e->scrpColor.green,e->scrpColor.blue);}return p+sprintf(p,"]");}
static char *state(char *p,TEHandle h){TEPtr t=*h;TEStyleHandle sh=TEGetStyleHandle(h);TEStylePtr s=*sh;short i,lines=t->nLines?t->nLines:1;STElement *e;StScrpHandle a,b;if(s->nRuns<0||s->nRuns>64||s->nStyles<0||s->nStyles>64||lines>64)return 0;
 p+=sprintf(p,"{\"length\":%d,\"selection\":[%d,%d],\"line_height\":%d,\"font_ascent\":%d,\"n_lines\":%d,\"text\":[",t->teLength,t->selStart,t->selEnd,t->lineHeight,t->fontAscent,t->nLines);for(i=0;i<t->teLength;i++)p+=sprintf(p,"%s%u",i?",":"",(unsigned char)(*t->hText)[i]);p+=sprintf(p,"],\"line_starts\":[");for(i=0;i<=t->nLines;i++)p+=sprintf(p,"%s%d",i?",":"",t->lineStarts[i]);p+=sprintf(p,"],\"style_pointer_offsets\":[");{short found=0;for(i=0;i<=94;i+=2)if(*(TEStyleHandle *)((char *)t+i)==sh){p+=sprintf(p,"%s%d",found?",":"",i);found++;}}
 p+=sprintf(p,"],\"n_runs\":%d,\"n_styles\":%d,\"refcon\":%ld,\"runs\":[",s->nRuns,s->nStyles,s->teRefCon);for(i=0;i<=s->nRuns;i++)p+=sprintf(p,"%s[%d,%d]",i?",":"",s->runs[i].startChar,s->runs[i].styleIndex);p+=sprintf(p,"],\"styles\":[");for(i=0;i<s->nStyles;i++){e=&(*s->styleTab)[i];p+=sprintf(p,"%s[%d,%d,%d,%d,%u,%d,%u,%u,%u]",i?",":"",e->stCount,e->stHeight,e->stAscent,e->stFont,e->stFace,e->stSize,e->stColor.red,e->stColor.green,e->stColor.blue);}p+=sprintf(p,"],\"heights\":[");for(i=0;i<lines;i++)p+=sprintf(p,"%s[%d,%d]",i?",":"",(*s->lhTab)[i].lhHeight,(*s->lhTab)[i].lhAscent);
 p+=sprintf(p,"],\"null_scrap\":");p=scrap(p,(**s->nullStyle).nullScrap);if(!p)return 0;a=TEGetStyleScrapHandle(h);b=TEGetStyleScrapHandle(h);p+=sprintf(p,",\"scrap_identity\":[%d,%d,%d],\"selection_scrap\":",a!=0,a==b,a==(**s->nullStyle).nullScrap);p=scrap(p,a);if(!p)return 0;return p+sprintf(p,"}");}
int main(void){Rect r={30,20,200,300};TEHandle source,h;TextStyle style;StScrpHandle styles;char *p=output;short i,file,before,after;long n;short starts[]={0,1,2,0},ends[]={6,5,2,32767};OSErr e;
InitGraf(&qd.thePort);InitFonts();InitWindows();TEInit();TextFont(3);TextFace(0);TextSize(12);
source=TEStyleNew(&r,&r);TESetText("abcde",5,source);TESetSelect(1,3,source);style.tsFont=4;style.tsFace=0;style.filler=0;style.tsSize=9;style.tsColor.red=style.tsColor.green=style.tsColor.blue=0;TESetStyle(doFont|doSize,&style,false,source);TESetSelect(0,5,source);styles=TEGetStyleScrapHandle(source);if(!styles)return 2;
p+=sprintf(p,"{\"schema\":1,\"probe\":\"style-use-v1\",\"textedit\":{\"cases\":[");
for(i=0;i<4;i++){h=TEStyleNew(&r,&r);(**TEGetStyleHandle(h)).teRefCon=0x12345678;TESetText("uvwxyz",6,h);TESetSelect(4,5,h);HSetState((Handle)styles,64);before=(unsigned char)HGetState((Handle)styles);TEUseStyleScrap(starts[i],ends[i],styles,i==0,h);after=(unsigned char)HGetState((Handle)styles);p+=sprintf(p,"%s{\"range\":[%d,%d],\"handle_state\":[%d,%d],\"document\":",i?",":"",starts[i],ends[i],before,after);p=state(p,h);if(!p)return 7;p+=sprintf(p,"}");TEDispose(h);}
p+=sprintf(p,"]}}\n");HSetState((Handle)styles,0);DisposeHandle((Handle)styles);TEDispose(source);
e=Create("\pStyleUseProbe.json",0,'ttxt','TEXT');if(e&&e!=dupFNErr)return 3;if(FSOpen("\pStyleUseProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;}
