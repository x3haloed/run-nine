#include <Multiverse.h>
#include <stdio.h>
static char output[131072],*p=output;
#pragma pack(push,2)
typedef struct{unsigned long d0;unsigned short input,ccr;long delta;unsigned long guard;}ABI;
#pragma pack(pop)
#define STR_(x) #x
#define STR(x) STR_(x)
#define CALL(OP) __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l (%%a2),%%d0\n\tmove.w 4(%%a2),%%ccr\n\t.word " STR(OP) "\n\t.word 0x42c3\n\tmove.l %%d0,(%%a2)\n\tmove.w %%d3,6(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,8(%%a2)\n\tmove.l (%%sp)+,12(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory")
static void invoke(short op,ABI*c){register ABI*args __asm__("a2")=c;switch(op){case 0:CALL(0xa850);break;case 1:CALL(0xa852);break;case 2:CALL(0xa853);break;case 3:CALL(0xa856);break;}}
#define STATE (*(volatile short*)0x8d0)
#define VIS (*(volatile unsigned char*)0x8cc)
#define RAW (*(volatile unsigned char*)0x8d2)
#define MEM (*(volatile short*)0x220)
#define RES (*(volatile short*)0xa60)
int main(void){short ops[]={0,2,2,1,1,2,2,2,1,3,2,1,2,0,3,1,1,2,2,2,0},flags[]={16,31,0};short s,k,i,index=0,file;long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();p+=sprintf(p,"{\"schema\":1,\"probe\":\"cursor-state-raw-v1\",\"records\":[");
 for(s=0;s<2;s++)for(k=0;k<3;k++){
  InitCursor();
  for(i=0;i<21;i++){
   ABI c;short before_state=STATE,before_vis=VIS,before_raw=RAW;c.d0=s?0:0x12345678;c.input=flags[k];MEM=-123;RES=-192;invoke(ops[i],&c);
   {short after_state=STATE,after_vis=VIS,after_raw=RAW,mem=MEM,res=RES;
   p+=sprintf(p,"%s{\"input_d0\":%lu,\"input_ccr\":%d,\"step\":%d,\"op\":%d,\"before_state\":%d,\"before_visible\":%d,\"before_8d2\":%d,\"after_state\":%d,\"after_visible\":%d,\"after_8d2\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"mem_error\":%d,\"res_error\":%d}",index++?",":"",s?0:0x12345678,flags[k],i,ops[i],before_state,before_vis,before_raw,after_state,after_vis,after_raw,c.d0,c.ccr&31,c.delta,c.guard,mem,res);}
  }
 }
 InitCursor();p+=sprintf(p,"]}\n");e=Create("\pCursorStateProbe.json",0,'pPrb','TEXT');if(e)return 2;if(FSOpen("\pCursorStateProbe.json",0,&file))return 3;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;
}
