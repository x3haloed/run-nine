#include <QuickDraw.h>
#include <Fonts.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char output[262144];
int main(void){GrafPort port;GrafPtr original;FontInfo info;short face,c,i,file;long count;OSErr err;char *p=output;unsigned char *labels[]={"\pItalic","\pUnderline","\pOutline","\pShadow","\pCondense","\pExtend","\pWAVy \x80 m","\pA A"};
 InitGraf(&qd.thePort);InitFonts();GetPort(&original);OpenPort(&port);TextFont(0);TextSize(12);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"system-styles-v1\",\"styles\":{\"calibration\":[");
 for(face=0;face<128;face++){TextFace(face);GetFontInfo(&info);p+=sprintf(p,"%s{\"face\":%d,\"info\":[%d,%d,%d,%d],\"widths\":[",face?",":"",face,info.ascent,info.descent,info.widMax,info.leading);for(c=0;c<256;c++)p+=sprintf(p,"%s%d",c?",":"",CharWidth(c));p+=sprintf(p,"]}");}
 p+=sprintf(p,"],\"labels\":[");for(face=0;face<128;face++){TextFace(face);p+=sprintf(p,"%s[",face?",":"");for(i=0;i<8;i++)p+=sprintf(p,"%s%d",i?",":"",StringWidth(labels[i]));p+=sprintf(p,"]");}p+=sprintf(p,"]}}\n");ClosePort(&port);SetPort(original);
 err=Create("\pSystemStyleProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pSystemStyleProbe.json",0,&file))return 3;SetEOF(file,0);count=p-output;if(count>sizeof(output))return 5;err=FSWrite(file,&count,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
