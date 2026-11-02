#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[1048576],*p=output;
#include "operands.h"
typedef struct{unsigned long d0;unsigned short ccr;long delta;}Result;
static Result environment(short op,unsigned short *word,short flags){register unsigned long d0 __asm__("d0")=0x12345678;register unsigned short *a2 __asm__("a2")=word;register unsigned short selector __asm__("d6")=op;register unsigned short flag __asm__("d4")=flags;register unsigned short ccr __asm__("d3");register long delta __asm__("d5");Result r;
 __asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,-(%%sp)\n\tmove.w %%d6,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xa9eb\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"=d"(ccr),"=d"(delta):"a"(a2),"d"(selector),"d"(flag):"d1","d2","a0","a1","cc","memory");r.d0=d0;r.ccr=ccr&31;r.delta=delta;return r;
}
static void hex(const char *name,const unsigned char *b){short i;p+=sprintf(p,",\"%s\":\"",name);for(i=0;i<10;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}

static void measure(short kind,unsigned short mode,short flags,const unsigned char *input,short comma){
 struct{unsigned long before;unsigned char value[10];unsigned long after;}__attribute__((packed,aligned(2)))state;
 struct{unsigned long before,value,after;}__attribute__((packed,aligned(2)))single;
 register unsigned long d0 __asm__("d0");register unsigned char *a2 __asm__("a2")=state.value;register unsigned short flag __asm__("d4")=flags;register unsigned short ccr __asm__("d3");register long delta __asm__("d5");unsigned long result;unsigned short status,env,word,single_env;long stack;
 state.before=0x11223344;state.after=0x55667788;memcpy(state.value,input,10);word=mode;environment(1,&word,0);d0=0x12345678;
 __asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,-(%%sp)\n\tmove.w #0x001e,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xa9ec\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"=d"(ccr),"=d"(delta):"a"(a2),"d"(flag):"d1","d2","a0","a1","cc","memory");result=d0;status=ccr&31;stack=delta;env=0;environment(3,&env,0);
 word=0;environment(1,&word,0);single.before=0x99aabbcc;single.after=0xddeeff00;single.value=0x7b7b7b7b;
 {register unsigned long *a3 __asm__("a3")=&single.value;
 __asm__ volatile("move.l %%a2,-(%%sp)\n\tmove.l %%a3,-(%%sp)\n\tmove.w #0x1010,-(%%sp)\n\t.word 0xa9eb"::"a"(a2),"a"(a3):"d0","d1","d2","a0","a1","cc","memory");}
 single_env=0;environment(3,&single_env,0);
 p+=sprintf(p,"%s{\"kind\":%d,\"input_env\":%u,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"output_env\":%u,\"guards\":[%lu,%lu],\"single_bits\":%lu,\"single_guards\":[%lu,%lu]",comma?",":"",kind,mode,flags,result,status,stack,env,state.before,state.after,single.value,single.before,single.after);p+=sprintf(p,",\"single_env\":%u",single_env);hex("before_hex",input);hex("after_hex",state.value);p+=sprintf(p,"}");
}
int main(void){short i,j,k,index=0,file,flags[3]={16,31,0};unsigned short saved=0,word,modes[15]={0,0x20,0x40,0x2000,0x2020,0x2040,0x4000,0x4020,0x4040,0x6000,0x6020,0x6040,0x0100,0x0800,0x1f00};unsigned short controls[11]={0,0x20,0x40,0x2000,0x4000,0x6000,0x0100,0x1000,0x1f00,0x001f,0xffff};Result r;long n;OSErr err;
 environment(3,&saved,0);p+=sprintf(p,"{\"schema\":1,\"probe\":\"sane-raw-v3\",\"initial_env\":%u,\"controls\":[",saved);
 for(i=0;i<11;i++)for(k=0;k<3;k++){
  word=controls[i];r=environment(1,&word,flags[k]);p+=sprintf(p,"%s{\"input_env\":%u,\"input_ccr\":%d,\"selector\":1,\"word\":%u,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld}",index++?",":"",controls[i],flags[k],word,r.d0,r.ccr,r.delta);
  word=0x5a5a;r=environment(3,&word,flags[k]);p+=sprintf(p,",{\"input_env\":%u,\"input_ccr\":%d,\"selector\":3,\"word\":%u,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld}",controls[i],flags[k],word,r.d0,r.ccr,r.delta);
 }
 p+=sprintf(p,"],\"atan\":[");index=0;
 for(i=0;i<SANE_BASE_COUNT;i++)for(j=0;j<15;j++)for(k=0;k<3;k++)measure(i,modes[j],flags[k],sane_operands[i],index++);
 p+=sprintf(p,"],\"dense\":[");index=0;for(i=0;i<SANE_DENSE_COUNT;i++)measure(i,0,16,sane_operands[SANE_BASE_COUNT+i],index++);
 environment(1,&saved,0);p+=sprintf(p,"]}\n");err=Create("\pSANEProbe.json",0,'pPrb','TEXT');if(err)return 2;if(FSOpen("\pSANEProbe.json",0,&file))return 3;n=p-output;err=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
