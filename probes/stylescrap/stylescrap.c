#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char out[4096];
int main(void){GrafPort port;Rect rect;TEHandle te;TextStyle style;Handle scrap;short i,j,f;long n,size;char *p=out;short start[3]={0,0,3},end[3]={5,32767,32767};
 InitGraf(&qd.thePort);InitFonts();OpenPort(&port);TextFont(3);TextSize(12);SetRect(&rect,0,0,150,80);te=TEStyleNew(&rect,&rect);if(!te)return 2;TESetText("abcde",5,te);TESetSelect(1,3,te);memset(&style,0,sizeof(style));style.tsFont=4;style.tsSize=9;TESetStyle(doFont|doSize,&style,false,te);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"style-scrap-v1\",\"textedit\":{\"cases\":[");
 for(i=0;i<3;i++){(**te).selStart=start[i];(**te).selEnd=end[i];scrap=(Handle)TEGetStyleScrapHandle(te);if(!scrap)return 3;size=GetHandleSize(scrap);p+=sprintf(p,"%s{\"selection\":[%d,%d],\"after\":[%d,%d],\"hex\":\"",i?",":"",start[i],end[i],(**te).selStart,(**te).selEnd);for(j=0;j<size;j++)p+=sprintf(p,"%02x",(unsigned char)(*scrap)[j]);p+=sprintf(p,"\"}");DisposeHandle(scrap);}
 p+=sprintf(p,"]}}\n");(**te).selStart=0;(**te).selEnd=0;TEDispose(te);ClosePort(&port);
 if(Create("\pStyleScrapProbe.json",0,'ttxt','TEXT')||FSOpen("\pStyleScrapProbe.json",0,&f))return 4;n=strlen(out);FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return 0;}
