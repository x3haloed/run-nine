#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[4096],*p=output;
#pragma pack(push,2)
typedef struct{unsigned long d0,text,length;short width,code;unsigned short input,ccr;short result;long delta;unsigned long guard;}ABI;
typedef struct{unsigned long prefix;unsigned char bytes[256];unsigned long suffix;}Buffer;
typedef struct{unsigned long prefix;short length;unsigned long suffix;}Length;
#pragma pack(pop)
#define PRE __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tsubq.l #2,%%sp\n\tmove.w #0x7b7b,(%%sp)\n\tmove.w 12(%%a2),-(%%sp)\n\tmove.l 4(%%a2),-(%%sp)\n\t"
#define POST "move.l (%%a2),%%d0\n\tmove.w 16(%%a2),%%ccr\n\t.word 0xa8b5\n\t.word 0x42c3\n\tmove.l %%d0,(%%a2)\n\tmove.w %%d3,18(%%a2)\n\tmove.w (%%sp)+,20(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,22(%%a2)\n\tmove.l (%%sp)+,26(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory")
static void invoke(short op,ABI*c){register ABI*args __asm__("a2")=c;if(op){PRE "move.w 14(%%a2),-(%%sp)\n\tmove.l #0x8208ffe0,-(%%sp)\n\t" POST;}else{PRE "move.l 8(%%a2),-(%%sp)\n\tmove.w 14(%%a2),-(%%sp)\n\tmove.l #0x820cffde,-(%%sp)\n\t" POST;}}
static void hex(const unsigned char*b,short n){short i;p+=sprintf(p,"\"");for(i=0;i<n;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
int main(void){short op,fontctx,which,mode,w,seed,k,index=0,file,flags[]={16,31,0};long n;OSErr e;GrafPort port;short progress;unsigned char *strings[]={(unsigned char*)"",(unsigned char*)"Marathon",(unsigned char*)"Alpha Beta Gamma",(unsigned char*)"\x80\xc9\xd5Z"};
 Create("\pTruncationProgress.txt",0,'tPrb','TEXT');FSOpen("\pTruncationProgress.txt",0,&progress);{char status[]="before InitGraf\n";long size=sizeof(status)-1;FSWrite(progress,&size,status);FlushVol(0,0);}InitGraf(&qd.thePort);InitFonts();OpenPort(&port);SetPort(&port);e=Create("\pTruncationProbe.json",0,'tPrb','TEXT');if(e)return 3;if(FSOpen("\pTruncationProbe.json",0,&file))return 4;p+=sprintf(p,"{\"schema\":1,\"probe\":\"text-truncation-raw-v1\",\"records\":[");
 for(op=0;op<2;op++)for(fontctx=0;fontctx<2;fontctx++)for(which=0;which<4;which++)for(mode=0;mode<2;mode++){
  short length=strlen((char*)strings[which]),full,ellipsis,widths[9];TextFont(fontctx?0:1);TextSize(fontctx?0:12);TextFace(0);full=TextWidth(strings[which],0,length);ellipsis=CharWidth(0xc9);widths[0]=-1;widths[1]=0;widths[2]=ellipsis-1;widths[3]=ellipsis;widths[4]=ellipsis+1;widths[5]=full-1;widths[6]=full;widths[7]=full+1;widths[8]=20;
  for(w=0;w<9;w++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
   ABI c;Buffer buffer;Length l;short mem,res;buffer.prefix=0xa1b2c3d4;buffer.suffix=0x10203040;memset(buffer.bytes,0xa5,256);if(op){buffer.bytes[0]=length;memcpy(buffer.bytes+1,strings[which],length);}else memcpy(buffer.bytes,strings[which],length);l.prefix=0x11223344;l.suffix=0x99aabbcc;l.length=length;
   c.d0=seed?0:0x12345678;c.text=(unsigned long)buffer.bytes;c.length=(unsigned long)&l.length;c.width=widths[w];c.code=mode?0x4000:0;c.input=flags[k];
   p+=sprintf(p,"%s{\"op\":%d,\"fontctx\":%d,\"which\":%d,\"mode\":%d,\"width_case\":%d,\"width\":%d,\"full_width\":%d,\"ellipsis_width\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"before_length\":%d,\"before_hex\":",index++?",":"",op,fontctx,which,mode,w,c.width,full,ellipsis,seed?0:0x12345678,flags[k],length);hex(buffer.bytes,256);
   {char status[128];long size=sprintf(status,"before op=%d font=%d string=%d mode=%d widthcase=%d seed=%d ccrcase=%d\n",op,fontctx,which,mode,w,seed,k);SetFPos(progress,fsFromStart,0);FSWrite(progress,&size,status);FlushVol(0,0);}*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(op,&c);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;
   p+=sprintf(p,",\"result\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"mem_error\":%d,\"res_error\":%d,\"prefix\":%lu,\"suffix\":%lu,\"length_prefix\":%lu,\"length_suffix\":%lu,\"length_word\":%d,\"after_length\":%d,\"after_hex\":",c.result,c.d0,c.ccr&31,c.delta,c.guard,mem,res,buffer.prefix,buffer.suffix,l.prefix,l.suffix,l.length,op?buffer.bytes[0]:l.length);hex(buffer.bytes,256);p+=sprintf(p,"}");n=p-output;if(FSWrite(file,&n,output))return 5;p=output;
  }
 }
 p+=sprintf(p,"]}\n");n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
