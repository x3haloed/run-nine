#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
#include "operands.h"
static char output[4194304],*p=output;
static void environment(short op,unsigned short *word){register unsigned short *a2 __asm__("a2")=word;register unsigned short selector __asm__("d6")=op;__asm__ volatile("move.l %%a2,-(%%sp)\n\tmove.w %%d6,-(%%sp)\n\t.word 0xa9eb"::"a"(a2),"d"(selector):"d0","d1","d2","a0","a1","cc","memory");}
static void hex(const char *name,const unsigned char *b,short length){short i;p+=sprintf(p,",\"%s\":\"",name);for(i=0;i<length;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
int main(void){unsigned short modes[15]={0,0x20,0x40,0x2000,0x2020,0x2040,0x4000,0x4020,0x4040,0x6000,0x6020,0x6040,0x0100,0x0800,0x1f00},saved=0,word;short i,j,k,index=0,file,flags[3]={16,31,0};long n;OSErr err;environment(3,&saved);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"sane-widen-raw-v1\",\"records\":[");
 for(i=0;i<WIDEN_COUNT;i++)for(j=0;j<15;j++)for(k=0;k<3;k++){
  struct{unsigned long before;unsigned char value[8];unsigned long after;}__attribute__((packed,aligned(2)))source;
  struct{unsigned long before;unsigned char value[10];unsigned long after;}__attribute__((packed,aligned(2)))dest;
  register unsigned long d0 __asm__("d0");register unsigned char *a2 __asm__("a2")=source.value,*a3 __asm__("a3")=dest.value;register unsigned short flag __asm__("d4")=flags[k];register unsigned short ccr __asm__("d3");register long delta __asm__("d5");unsigned long result;unsigned short status,env;long stack;
  source.before=0x11223344;source.after=0x55667788;dest.before=0x99aabbcc;dest.after=0xddeeff00;memcpy(source.value,widen_operands[i],8);memset(dest.value,0x7b,10);word=modes[j];environment(1,&word);d0=0x12345678;
  __asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,-(%%sp)\n\tmove.l %%a3,-(%%sp)\n\tmove.w #0x080e,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xa9eb\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"=d"(ccr),"=d"(delta):"a"(a2),"a"(a3),"d"(flag):"d1","d2","a0","a1","cc","memory");result=d0;status=ccr&31;stack=delta;env=0;environment(3,&env);
  p+=sprintf(p,"%s{\"kind\":%d,\"input_env\":%u,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"output_env\":%u,\"guards\":[%lu,%lu,%lu,%lu]",index++?",":"",i,modes[j],flags[k],result,status,stack,env,source.before,source.after,dest.before,dest.after);hex("source_before_hex",widen_operands[i],8);hex("source_after_hex",source.value,8);p+=sprintf(p,",\"dest_hex\":\"");{short t;for(t=0;t<10;t++)p+=sprintf(p,"%02x",dest.value[t]);}p+=sprintf(p,"\"}");
 }
 environment(1,&saved);p+=sprintf(p,"]}\n");err=Create("\pSANEWidenProbe.json",0,'pPrb','TEXT');if(err)return 2;if(FSOpen("\pSANEWidenProbe.json",0,&file))return 3;n=p-output;err=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
