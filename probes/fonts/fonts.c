#include <QuickDraw.h>
#include <Fonts.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char output[65536];
int main(void){
    GrafPort port;GrafPtr original;FontInfo info;
    int fonts[]={0,1,3,4};int sizes[]={0,9,12};int faces[]={0,1};
    int f,s,b,c,first=1;char *p=output;short file;long count;OSErr err;
    InitGraf(&qd.thePort);InitFonts();GetPort(&original);OpenPort(&port);
    p+=sprintf(p,"{\"schema\":1,\"probe\":\"fonts-v1\",\"fonts\":[");
    for(f=0;f<4;f++)for(s=0;s<3;s++)for(b=0;b<2;b++){
        TextFont(fonts[f]);TextSize(sizes[s]);TextFace(faces[b]);GetFontInfo(&info);
        p+=sprintf(p,"%s{\"font\":%d,\"size\":%d,\"face\":%d,\"info\":[%d,%d,%d,%d],\"widths\":[",first?"":",",fonts[f],sizes[s],faces[b],info.ascent,info.descent,info.widMax,info.leading);first=0;
        for(c=0;c<256;c++)p+=sprintf(p,"%s%d",c?",":"",CharWidth(c));
        p+=sprintf(p,"]}");
    }
    p+=sprintf(p,"]}\n");ClosePort(&port);SetPort(original);
    err=Create("\pFontProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;
    if(FSOpen("\pFontProbe.json",0,&file))return 3;
    SetEOF(file,0);count=p-output;err=FSWrite(file,&count,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
