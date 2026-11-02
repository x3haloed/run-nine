#include <QuickDraw.h>
#include <Fonts.h>
#include <Files.h>
#include <stdio.h>
static char output[2048];
int main(void){short pairs[][2]={{0,12},{1,12},{3,9},{3,12},{4,9},{4,12},{2002,12},{32767,12}},i,f;char *p=output;long n;OSErr e;InitGraf(&qd.thePort);InitFonts();p+=sprintf(p,"{\"schema\":1,\"probe\":\"font-availability-v1\",\"fonts\":{\"queries\":[");for(i=0;i<8;i++)p+=sprintf(p,"%s[%d,%d,%u]",i?",":"",pairs[i][0],pairs[i][1],RealFont(pairs[i][0],pairs[i][1]));p+=sprintf(p,"]}}\n");e=Create("\pFontAvailabilityProbe.json",0,'ttxt','TEXT');if(e&&e!=dupFNErr)return 2;if(FSOpen("\pFontAvailabilityProbe.json",0,&f))return 3;SetEOF(f,0);n=p-output;e=FSWrite(f,&n,output);FSClose(f);FlushVol(0,0);return e?4:0;}
