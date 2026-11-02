#include <QuickDraw.h>
#include <Fonts.h>
#include <TextEdit.h>
#include <Windows.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char output[16384];
static char *queries(char *p,TEHandle h){short pairs[][2]={{1,1},{0,0},{0,1},{1,0},{2,1},{2,2}},i,offset,lh,asc,n=(*h)->nLines>1?6:4;long point;TextStyle s;p+=sprintf(p,"{\"heights\":[");for(i=0;i<n;i++)p+=sprintf(p,"%s[%d,%d,%ld]",i?",":"",pairs[i][0],pairs[i][1],TEGetHeight(pairs[i][0],pairs[i][1],h));p+=sprintf(p,"],\"styles\":[");for(i=0;i<3;i++){offset=i==0?0:i==1?(*h)->teLength:(*h)->teLength/2;memset(&s,0xa5,sizeof(s));lh=asc=-1234;TEGetStyle(offset,&s,&lh,&asc,h);point=TEGetPoint(offset,h);p+=sprintf(p,"%s{\"offset\":%d,\"style\":[%d,%u,%d,%u,%u,%u],\"metrics\":[%d,%d],\"point\":[%d,%d]}",i?",":"",offset,s.tsFont,s.tsFace,s.tsSize,s.tsColor.red,s.tsColor.green,s.tsColor.blue,lh,asc,(short)(point>>16),(short)point);}return p+sprintf(p,"]}");}
int main(void){Rect r={30,20,200,300};TEHandle h;TextStyle s;char *p=output;short file;long n;OSErr e;InitGraf(&qd.thePort);InitFonts();InitWindows();TEInit();TextFont(3);TextFace(0);TextSize(12);h=TEStyleNew(&r,&r);if(!h)return 5;p+=sprintf(p,"{\"schema\":1,\"probe\":\"style-queries-v1\",\"textedit\":{\"initial\":");p=queries(p,h);s.tsFont=4;s.tsFace=1;s.filler=0;s.tsSize=9;s.tsColor.red=s.tsColor.green=s.tsColor.blue=0;TESetStyle(doAll,&s,0,h);p+=sprintf(p,",\"pending\":");p=queries(p,h);TEKey('A',h);TEKey('B',h);TEKey(13,h);TEKey('C',h);p+=sprintf(p,",\"typed\":");p=queries(p,h);TESetSelect(1,3,h);s.tsFont=3;s.tsSize=12;TESetStyle(doFont|doSize,&s,0,h);p+=sprintf(p,",\"mixed\":");p=queries(p,h);TECalText(h);p+=sprintf(p,",\"recalculated\":");p=queries(p,h);TEDispose(h);p+=sprintf(p,"}}\n");e=Create("\pStyleQueriesProbe.json",0,'ttxt','TEXT');if(e&&e!=dupFNErr)return 2;if(FSOpen("\pStyleQueriesProbe.json",0,&file))return 3;SetEOF(file,0);n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;}
