#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static unsigned char bits[512];
int main(void){GrafPort port;BitMap bitmap;Rect rect;short results[3],file,i;long n;char out[1800],*p=out;
 InitGraf(&qd.thePort);OpenPort(&port);bitmap.baseAddr=(Ptr)bits;bitmap.rowBytes=8;SetRect(&bitmap.bounds,0,0,64,64);SetPortBits(&bitmap);port.portRect=bitmap.bounds;RectRgn(port.visRgn,&bitmap.bounds);ClipRect(&bitmap.bounds);memset(bits,0xaa,sizeof(bits));
 SetRect(&rect,0,0,16,16);results[0]=PlotIconID(&rect,0,0,2001);
 SetRect(&rect,20,20,52,52);results[1]=PlotIconID(&rect,0,0,2001);
 SetRect(&rect,0,0,16,16);results[2]=PlotIconID(&rect,0,0,3001);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"icon-plot-v1\",\"icons\":{\"errors\":[%d,%d,%d],\"bitmap_hex\":\"",results[0],results[1],results[2]);
 for(i=0;i<512;i++)p+=sprintf(p,"%02x",bits[i]);p+=sprintf(p,"\"}}\n");
 if(Create("\pIconPlotProbe.json",0,'ttxt','TEXT'))return 3;if(FSOpen("\pIconPlotProbe.json",0,&file))return 4;n=strlen(out);FSWrite(file,&n,out);FSClose(file);FlushVol(0,0);ClosePort(&port);return 0;}
