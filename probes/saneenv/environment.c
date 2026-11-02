#include <Multiverse.h>
#include <stdio.h>
static char output[1048576],*p=output;
typedef struct{unsigned long d0;unsigned short ccr;long delta;}Result;
static Result environment(short op,unsigned short *word,short flags){register unsigned long d0 __asm__("d0")=0x12345678;register unsigned short *a2 __asm__("a2")=word;register unsigned short selector __asm__("d6")=op;register unsigned short flag __asm__("d4")=flags;register unsigned short ccr __asm__("d3");register long delta __asm__("d5");Result r;
 __asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,-(%%sp)\n\tmove.w %%d6,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xa9eb\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"=d"(ccr),"=d"(delta):"a"(a2),"d"(selector),"d"(flag):"d1","d2","a0","a1","cc","memory");r.d0=d0;r.ccr=ccr&31;r.delta=delta;return r;
}
int main(void){unsigned short modes[]={0,0x20,0x40,0x60,0x2000,0x4000,0x6000,0x80,0x100,0x200,0x400,0x800,0x1000,0x1f00,0x1f,0xffff,0x5a5a,0xa5a5,0x7f80,0xff00},saved=0,current,after;short i,j,k,phase,index=0,file,flags[3]={16,31,0};Result result;long n;OSErr err;
 environment(3,&saved,0);p+=sprintf(p,"{\"schema\":1,\"probe\":\"sane-env-raw-v1\",\"records\":[");
 for(phase=0;phase<2;phase++)for(i=0;i<20;i++)for(j=0;j<(phase?20:1);j++)for(k=0;k<3;k++){
  struct{unsigned long before;unsigned short value;unsigned long after;}__attribute__((packed,aligned(2)))state;
  /* Exit measurements disable halts in the saved environment. Entry retains
     all original bits. Actual halt callbacks are deliberately not invoked. */
  if(phase && ((modes[i]&31)||((modes[j]&31)&(modes[j]>>8)&31)))continue;
  current=phase?modes[j]:modes[i];environment(1,&current,0);
  state.before=0x11223344;state.after=0x55667788;state.value=phase?modes[i]:0x5a5a;
  result=environment(phase?0x19:0x17,&state.value,flags[k]);after=0;environment(3,&after,0);
  p+=sprintf(p,"%s{\"selector\":%u,\"input_env\":%u,\"input_word\":%u,\"input_ccr\":%d,\"word\":%u,\"output_env\":%u,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guards\":[%lu,%lu]}",index++?",":"",phase?0x19:0x17,current,phase?modes[i]:0x5a5a,flags[k],state.value,after,result.d0,result.ccr,result.delta,state.before,state.after);
 }
 environment(1,&saved,0);p+=sprintf(p,"]}\n");err=Create("\pSANEEnvProbe.json",0,'pPrb','TEXT');if(err)return 2;if(FSOpen("\pSANEEnvProbe.json",0,&file))return 3;n=p-output;err=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
