#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[131072],*p=output;
#pragma pack(push,2)
typedef struct{unsigned long d0;unsigned short input,ccr;long delta;unsigned long guard;}ABI;
typedef struct{unsigned long prefix;unsigned char bytes[8];unsigned long suffix;}Source;
#pragma pack(pop)
static void invoke(ABI*c,Source*s){register ABI*args __asm__("a2")=c;register unsigned char*pat __asm__("a4")=s->bytes;
 __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l %%a4,-(%%sp)\n\tmove.l (%%a2),%%d0\n\tmove.w 4(%%a2),%%ccr\n\t.word 0xa89d\n\t.word 0x42c3\n\tmove.l %%d0,(%%a2)\n\tmove.w %%d3,6(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,8(%%a2)\n\tmove.l (%%sp)+,12(%%a2)":"+a"(args),"+a"(pat)::"d0","d1","d2","d3","a0","a1","a3","cc","memory");}
static void hex(const unsigned char*b,short n){short i;p+=sprintf(p,"\"");for(i=0;i<n;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
int main(void){GrafPort mono;CGrafPort color;GrafPtr original;Rect bounds={0,0,32,32},frame={5,7,25,26};BitMap bitmap;unsigned char bits[128];unsigned char patterns[4][8]={{255,255,255,255,255,255,255,255},{0},{170,85,170,85,170,85,170,85},{129,66,36,24,24,36,66,129}};short kind,which,seed,k,index=0,file,flags[]={16,31,0};long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();InitCursor();GetPort(&original);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"pen-pattern-raw-v1\",\"records\":[");
 for(kind=0;kind<2;kind++)for(which=0;which<4;which++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
  ABI c;Source source;PenState before,after;short mem,res;
  if(kind){OpenCPort(&color);SetPort((GrafPtr)&color);}else{OpenPort(&mono);SetPort(&mono);memset(bits,0,128);bitmap.baseAddr=(Ptr)bits;bitmap.rowBytes=4;bitmap.bounds=bounds;SetPortBits(&bitmap);mono.portRect=bounds;SetRectRgn(mono.visRgn,0,0,32,32);SetRectRgn(mono.clipRgn,0,0,32,32);}
  PenNormal();PenSize(3,5);MoveTo(-7,11);PenMode(patXor);GetPenState(&before);
  source.prefix=0xa1b2c3d4;source.suffix=0x10203040;memcpy(source.bytes,patterns[which],8);c.d0=seed?0:0x12345678;c.input=flags[k];*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(&c,&source);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;GetPenState(&after);if(!kind)FrameRect(&frame);
  p+=sprintf(p,"%s{\"kind\":%d,\"pattern\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"prefix\":%lu,\"suffix\":%lu,\"mem_error\":%d,\"res_error\":%d,\"source_hex\":",index++?",":"",kind,which,seed?0:0x12345678,flags[k],c.d0,c.ccr&31,c.delta,c.guard,source.prefix,source.suffix,mem,res);hex(source.bytes,8);p+=sprintf(p,",\"before_hex\":");hex((unsigned char*)&before,18);p+=sprintf(p,",\"after_hex\":");hex((unsigned char*)&after,18);p+=sprintf(p,",\"pixels_hex\":");hex(bits,kind?0:128);p+=sprintf(p,"}");
  if(kind)CloseCPort(&color);else ClosePort(&mono);SetPort(original);
 }
 p+=sprintf(p,"]}\n");e=Create("\pPenPatProbe.json",0,'pPrb','TEXT');if(e)return 3;if(FSOpen("\pPenPatProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
