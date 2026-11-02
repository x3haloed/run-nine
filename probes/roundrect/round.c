#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[16384],*p=output;
#pragma pack(push,2)
typedef struct{unsigned long d0,rect,pattern;short ow,oh;unsigned short input,ccr;long delta;unsigned long guard;}ABI;
#pragma pack(pop)
#define STR_(x) #x
#define STR(x) STR_(x)
#define PRE __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l 4(%%a2),-(%%sp)\n\tmove.w 12(%%a2),-(%%sp)\n\tmove.w 14(%%a2),-(%%sp)\n\t"
#define POST(OP) "move.l (%%a2),%%d0\n\tmove.w 16(%%a2),%%ccr\n\t.word " STR(OP) "\n\t.word 0x42c3\n\tmove.l %%d0,(%%a2)\n\tmove.w %%d3,18(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,20(%%a2)\n\tmove.l %%a3,%%sp\n\tmove.l (%%sp)+,24(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory")
static void invoke(short op,ABI*c){register ABI*args __asm__("a2")=c;switch(op){case 0:PRE POST(0xa8b0);break;case 1:PRE POST(0xa8b1);break;case 2:PRE POST(0xa8b2);break;case 3:PRE POST(0xa8b3);break;case 4:PRE "move.l 8(%%a2),-(%%sp)\n\t" POST(0xa8b4);break;}}
static void hex(const unsigned char*b,short n){short i;p+=sprintf(p,"\"");for(i=0;i<n;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
int main(void){short which,op,seed,k,i,index=0,file,flags[]={16,31,0};long n;OSErr e;GrafPort port;BitMap bitmap;Rect bounds={0,0,32,32},rects[]={{2,2,30,30},{2,2,30,30},{2,2,30,30},{2,2,30,30},{2,2,30,30},{3,4,11,27},{5,5,5,25},{14,14,26,26},{-3,-4,25,37},{2,2,30,30},{2,2,30,30},{2,2,30,30},{2,2,30,30},{2,2,30,30},{20,5,10,25},{5,5,6,6},{2,2,30,30},{2,2,30,30},{2,2,30,30},{2,2,30,30},{2,2,30,30}};Point ovals[]={{16,16},{4,8},{9,15},{0,0},{40,40},{12,16},{16,16},{4,4},{9,15},{16,16},{16,16},{8,-6},{16,16},{16,16},{16,16},{1,1},{16,1},{1,16},{16,16},{2,2},{16,16}};unsigned char bits[128],initial[128],patterns[3][8]={{255,255,255,255,255,255,255,255},{170,85,170,85,170,85,170,85},{129,66,36,24,24,36,66,129}};
 InitGraf(&qd.thePort);InitFonts();OpenPort(&port);SetPort(&port);bitmap.baseAddr=(Ptr)bits;bitmap.rowBytes=4;bitmap.bounds=bounds;SetPortBits(&bitmap);port.portRect=bounds;SetRectRgn(port.visRgn,0,0,32,32);SetRectRgn(port.clipRgn,1,1,31,31);
 e=Create("\pRoundRectProbe.json",0,'rPrb','TEXT');if(e)return 3;if(FSOpen("\pRoundRectProbe.json",0,&file))return 4;p+=sprintf(p,"{\"schema\":1,\"probe\":\"round-rect-raw-v1\",\"records\":[");
 for(which=0;which<21;which++)for(op=0;op<5;op++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
  ABI c;PenState before,after;short mem,res,pattern=(which==9||which==20)?1:0;for(i=0;i<128;i++)bits[i]=(op==0||op==1)?0:((i&1)?0xcc:0x33);memcpy(initial,bits,128);
  PenNormal();PenSize(which==7||which==9?3:which==12?0:which==13?-2:which==18?20:1,which==7||which==9?2:which==12?3:which==18?20:1);PenPat((const Pattern*)&patterns[pattern]);PenMode(which==9?patXor:which==20?patOr:patCopy);BackPat((const Pattern*)&patterns[which==20?2:0]);if(which!=20){unsigned char white[8]={0};BackPat((const Pattern*)&white);}MoveTo(5,7);if(which==10)HidePen();GetPenState(&before);
  c.d0=seed?0:0x12345678;c.rect=(unsigned long)&rects[which];c.pattern=(unsigned long)&patterns[2];c.ow=ovals[which].h;c.oh=ovals[which].v;c.input=flags[k];*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(op,&c);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;GetPenState(&after);
  p+=sprintf(p,"%s{\"which\":%d,\"op\":%d,\"bounds\":[%d,%d,%d,%d],\"oval_width\":%d,\"oval_height\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"mem_error\":%d,\"res_error\":%d,\"pen_visibility\":%d,\"before_pen_hex\":",index++?",":"",which,op,rects[which].top,rects[which].left,rects[which].bottom,rects[which].right,c.ow,c.oh,seed?0:0x12345678,flags[k],c.d0,c.ccr&31,c.delta,c.guard,mem,res,port.pnVis);hex((unsigned char*)&before,18);p+=sprintf(p,",\"after_pen_hex\":");hex((unsigned char*)&after,18);p+=sprintf(p,",\"before_pixels_hex\":");hex(initial,128);p+=sprintf(p,",\"pixels_hex\":");hex(bits,128);p+=sprintf(p,"}");n=p-output;if(FSWrite(file,&n,output))return 5;p=output;if(which==10)ShowPen();
 }
 p+=sprintf(p,"]}\n");n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
