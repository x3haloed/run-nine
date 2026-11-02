#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[262144],*p=output;
#pragma pack(push,2)
typedef struct{unsigned long d0;unsigned short input,ccr;short h,v;long delta;unsigned long guard;}ABI;
#pragma pack(pop)
#define STR_(x) #x
#define STR(x) STR_(x)
#define CALL(OP) __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.w 8(%%a2),-(%%sp)\n\tmove.w 10(%%a2),-(%%sp)\n\tmove.l (%%a2),%%d0\n\tmove.w 4(%%a2),%%ccr\n\t.word " STR(OP) "\n\t.word 0x42c3\n\tmove.l %%d0,(%%a2)\n\tmove.w %%d3,6(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,12(%%a2)\n\tmove.l (%%sp)+,16(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory")
static void invoke(short op,ABI*c){register ABI*args __asm__("a2")=c;if(op){CALL(0xa892);}else{CALL(0xa891);}}
static void hex(const unsigned char*b,short n){short i;p+=sprintf(p,"\"");for(i=0;i<n;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
int main(void){short which,op,seed,k,index=0,file,flags[]={16,31,0};long n;OSErr e;GrafPort port;GrafPtr original;BitMap bitmap;Rect bounds={0,0,32,32};unsigned char bits[128];Point start[]={{4,3},{4,15},{3,5},{3,3},{3,20},{3,3},{10,20},{-3,-4},{8,8},{3,3},{3,3},{3,3}},end[]={{4,15},{4,3},{19,5},{20,20},{20,3},{10,20},{3,3},{38,35},{8,8},{9,18},{9,18},{9,18}},size[]={{1,1},{1,1},{1,1},{1,1},{1,1},{2,3},{2,3},{3,2},{1,1},{3,0},{1,1},{1,-2}};unsigned char patterns[4][8]={{255,255,255,255,255,255,255,255},{170,85,170,85,170,85,170,85},{129,66,36,24,24,36,66,129},{0}};
 InitGraf(&qd.thePort);InitFonts();GetPort(&original);p+=sprintf(p,"{\"schema\":1,\"probe\":\"qd-line-raw-v1\",\"records\":[");
 for(which=0;which<12;which++)for(op=0;op<2;op++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
  ABI c;PenState before,after;short mem,res,pattern=which==3||which==6?1:which==4||which==7?2:0;
  OpenPort(&port);SetPort(&port);memset(bits,0,sizeof(bits));bitmap.baseAddr=(Ptr)bits;bitmap.rowBytes=4;bitmap.bounds=bounds;SetPortBits(&bitmap);port.portRect=bounds;SetRectRgn(port.visRgn,0,0,32,32);SetRectRgn(port.clipRgn,0,0,32,32);PenNormal();PenSize(size[which].h,size[which].v);PenPat((const Pattern*)&patterns[pattern]);PenMode(which==6?patXor:patCopy);MoveTo(start[which].h,start[which].v);if(which==10)HidePen();GetPenState(&before);
  c.d0=seed?0:0x12345678;c.input=flags[k];c.h=op?end[which].h-start[which].h:end[which].h;c.v=op?end[which].v-start[which].v:end[which].v;*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(op,&c);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;GetPenState(&after);
  p+=sprintf(p,"%s{\"which\":%d,\"op\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"h\":%d,\"v\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"mem_error\":%d,\"res_error\":%d,\"pen_visibility\":%d,\"before_hex\":",index++?",":"",which,op,seed?0:0x12345678,flags[k],c.h,c.v,c.d0,c.ccr&31,c.delta,c.guard,mem,res,port.pnVis);hex((unsigned char*)&before,18);p+=sprintf(p,",\"after_hex\":");hex((unsigned char*)&after,18);p+=sprintf(p,",\"pixels_hex\":");hex(bits,128);p+=sprintf(p,"}");ClosePort(&port);SetPort(original);
 }
 p+=sprintf(p,"]}\n");e=Create("\pQDLineProbe.json",0,'lPrb','TEXT');if(e)return 3;if(FSOpen("\pQDLineProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
