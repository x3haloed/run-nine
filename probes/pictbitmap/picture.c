#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
#include "fixtures.h"
static char output[8388608],*p=output;
#pragma pack(push,2)
typedef struct{PicHandle picture;Rect*dest;unsigned short input,ccr;unsigned long d0;long delta;unsigned long guard;}ABI;
#pragma pack(pop)
static void draw(ABI*c){register ABI*args __asm__("a2")=c;
 __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l (%%a2),-(%%sp)\n\tmove.l 4(%%a2),-(%%sp)\n\tmove.l 12(%%a2),%%d0\n\tmove.w 8(%%a2),%%ccr\n\t.word 0xa8f6\n\t.word 0x42c3\n\tmove.l %%d0,12(%%a2)\n\tmove.w %%d3,10(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,16(%%a2)\n\tmove.l (%%sp)+,20(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory");
}
static void hex(const char*key,const void*ptr,long size){const unsigned char*b=ptr;static const char digits[]="0123456789abcdef";long j;p+=sprintf(p,",\"%s\":\"",key);for(j=0;j<size;j++){*p++=digits[b[j]>>4];*p++=digits[b[j]&15];}*p++='"';*p=0;}
static void rectangle(const char*key,Rect*r){p+=sprintf(p,",\"%s\":[%d,%d,%d,%d]",key,r->top,r->left,r->bottom,r->right);}
int main(void){Rect bounds={0,0,24,64},destinations[5]={{2,3,10,19},{1,2,17,34},{3,4,7,12},{-2,-3,6,13},{4,4,4,20}},inner={4,6,13,28};short flags[]={16,31,0},old,kind,which,clip,ink,k,seed,index=0,file;GDHandle gd;CGrafPort port,saved;GrafPtr prior;PixMapHandle pm;Ptr pixels;RGBColor fg,bg;long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();gd=GetGDevice();old=(**(*gd)->gdPMap).pixelSize;if(SetDepth(gd,8,0,0))return 2;GetPort(&prior);OpenCPort(&port);SetPort((GrafPtr)&port);pm=port.portPixMap;if((**pm).pixelSize!=8)return 3;pixels=NewPtr(1536);if(!pixels)return 4;(**pm).baseAddr=pixels;(**pm).rowBytes=0x8040;(**pm).bounds=bounds;port.portRect=bounds;RectRgn(port.visRgn,&bounds);RectRgn(port.clipRgn,&bounds);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"pict-bitmap-raw-v1\",\"depth\":%d,\"row_bytes\":%d",(**pm).pixelSize,(**pm).rowBytes&0x3fff);hex("destination_table_hex",*(**pm).pmTable,GetHandleSize((Handle)(**pm).pmTable));p+=sprintf(p,",\"records\":[");
 for(kind=0;kind<4;kind++){
  PicHandle picture=(PicHandle)NewHandle(picture_sizes[kind]);if(!picture)return 5;HLock((Handle)picture);memcpy(*picture,pictures[kind],picture_sizes[kind]);
  for(which=0;which<5;which++)for(clip=0;clip<2;clip++)for(ink=0;ink<2;ink++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
   ABI c;short mem,res,picstate;Rect dest=destinations[which];unsigned char clipbytes[10];memset(pixels,0x5a,1536);ClipRect(clip?&inner:&bounds);
   fg.red=ink?65535:0;fg.green=0;fg.blue=0;bg.red=ink?0:65535;bg.green=65535;bg.blue=ink?0:65535;RGBForeColor(&fg);RGBBackColor(&bg);saved=port;memcpy(clipbytes,*port.clipRgn,10);
   c.picture=picture;c.dest=&dest;c.input=flags[k];c.d0=seed?0:0x12345678;*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;draw(&c);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;picstate=(unsigned char)HGetState((Handle)picture);
   p+=sprintf(p,"%s{\"kind\":%d,\"destination\":%d,\"clip\":%d,\"ink\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"mem_error\":%d,\"res_error\":%d,\"picture_state\":%d,\"picture_preserved\":%d,\"port_preserved\":%d,\"clip_preserved\":%d}",index++?",":"",kind,which,clip,ink,seed?0:0x12345678,flags[k],c.d0,c.ccr&31,c.delta,c.guard,mem,res,picstate,memcmp(*picture,pictures[kind],picture_sizes[kind])==0,memcmp(&port,&saved,sizeof(port))==0,memcmp(clipbytes,*port.clipRgn,10)==0);
   p--;rectangle("dest_rect",&dest);hex("pixels_hex",pixels,1536);p+=sprintf(p,"}");
  }
  HUnlock((Handle)picture);DisposeHandle((Handle)picture);
 }
 p+=sprintf(p,"]}\n");SetPort(prior);CloseCPort(&port);DisposePtr(pixels);SetDepth(gd,old,0,0);
 e=Create("\pPICTBitmapProbe.json",0,'pPrb','TEXT');if(e)return 6;if(FSOpen("\pPICTBitmapProbe.json",0,&file))return 7;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?8:0;
}
