#include <Multiverse.h>
#include <stdio.h>
static char output[1048576],*p=output;
#pragma pack(push,2)
typedef struct{unsigned long d0;unsigned short input,ccr,result;long delta;unsigned long guard;}ABI;
#pragma pack(pop)
#define STR_(x) #x
#define STR(x) STR_(x)
#define CALL(OP) __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tsubq.l #2,%%sp\n\tmove.w #0x7b7b,(%%sp)\n\tmove.l (%%a2),%%d0\n\tmove.w 4(%%a2),%%ccr\n\t.word " STR(OP) "\n\t.word 0x42c3\n\tmove.l %%d0,(%%a2)\n\tmove.w %%d3,6(%%a2)\n\tmove.w (%%sp)+,8(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,10(%%a2)\n\tmove.l (%%sp)+,14(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory")
static void invoke(ABI*c){register ABI*args __asm__("a2")=c;CALL(0xa861);}
int main(void){unsigned long seeds[]={0,1,2,0x12345678,0x7ffffffe,0x7fffffff,0x80000000,0xffffffff,0xffff0000,0x0000ffff,0x00ff00ff,0x55555555,0xaaaaaaaa};short input,k,seed,step,index=0,file,flags[]={16,31,0};long n;OSErr e;InitGraf(&qd.thePort);InitFonts();InitWindows();
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"qd-random-raw-v1\",\"records\":[");
 for(input=0;input<13;input++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
  qd.randSeed=seeds[input];
  for(step=0;step<16;step++){
   ABI c;unsigned long before=qd.randSeed,after;short mem,res;c.d0=seed?0:0x12345678;c.input=flags[k];*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(&c);after=qd.randSeed;mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;
   p+=sprintf(p,"%s{\"initial_seed\":%lu,\"input_d0\":%lu,\"input_ccr\":%d,\"step\":%d,\"seed_before\":%lu,\"seed_after\":%lu,\"result\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"mem_error\":%d,\"res_error\":%d}",index++?",":"",seeds[input],seed?0:0x12345678,flags[k],step,before,after,(short)c.result,c.d0,c.ccr&31,c.delta,c.guard,mem,res);
  }
 }
 p+=sprintf(p,"]}\n");e=Create("\pQDRandomProbe.json",0,'pPrb','TEXT');if(e)return 3;if(FSOpen("\pQDRandomProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
