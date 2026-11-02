#include <Multiverse.h>
#include <stdio.h>
static char output[65536],*p=output;
int main(void){GrafPort port;GrafPtr original;FontInfo info;short sizes[]={0,9,10,12,14,18,24},s,b,c,first=1,file,number;unsigned char name[256],label[]={8,'A',0,128,255,32,'0','a',13};long count;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();GetPort(&original);OpenPort(&port);
 GetFontName(22,name);GetFNum("\pCourier",&number);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"courier-font-raw-v1\",\"name\":[");for(c=0;c<name[0];c++)p+=sprintf(p,"%s%u",c?",":"",name[c+1]);p+=sprintf(p,"],\"number\":%d,\"fonts\":[",number);
 for(s=0;s<7;s++)for(b=0;b<2;b++){
  TextFont(22);TextSize(sizes[s]);TextFace(b);GetFontInfo(&info);
  p+=sprintf(p,"%s{\"font\":22,\"size\":%d,\"face\":%d,\"real\":%d,\"info\":[%d,%d,%d,%d],\"widths\":[",first?"":",",sizes[s],b,RealFont(22,sizes[s]),info.ascent,info.descent,info.widMax,info.leading);first=0;
  for(c=0;c<256;c++)p+=sprintf(p,"%s%d",c?",":"",CharWidth(c));
  p+=sprintf(p,"],\"labels\":[%d,%d,%d]}",StringWidth("\pMarathon"),StringWidth("\pABCDEFGHIJKLMNOPQRSTUVWXYZ"),StringWidth(label));
 }
 p+=sprintf(p,"]}\n");ClosePort(&port);SetPort(original);
 e=Create("\pCourierProbe.json",0,'pPrb','TEXT');if(e)return 2;if(FSOpen("\pCourierProbe.json",0,&file))return 3;count=p-output;e=FSWrite(file,&count,output);FSClose(file);FlushVol(0,0);return e?4:0;
}
