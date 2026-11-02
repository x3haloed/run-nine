#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
#include "../sane/operands.h"
#include "../sanewiden/operands.h"
static char output[4194304],*p=output;
static void environment(short op,unsigned short *word){register unsigned short *a2 __asm__("a2")=word;register unsigned short selector __asm__("d6")=op;__asm__ volatile("move.l %%a2,-(%%sp)\n\tmove.w %%d6,-(%%sp)\n\t.word 0xa9eb"::"a"(a2),"d"(selector):"d0","d1","d2","a0","a1","cc","memory");}
static void hex(const char *name,const unsigned char *b,short length){short i;p+=sprintf(p,",\"%s\":\"",name);for(i=0;i<length;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
int main(void){short destinations[32]={2,2,4,6,2,3,20,17,0,1,2,3,2,2,6,7,15,16,20,6,2,3,36,32,2,27,35,24,37,2,24,36};unsigned short modes[15]={0,0x20,0x40,0x2000,0x2020,0x2040,0x4000,0x4020,0x4040,0x6000,0x6020,0x6040,0x0100,0x0800,0x1f00},saved=0,word;short i,j,k,op,ops[2]={0x0804,0x0806},index=0,file,flags[3]={16,31,0};long n;OSErr err;environment(3,&saved);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"sane-mixed-raw-v1\",\"records\":[");
 for(op=0;op<2;op++)for(i=0;i<64;i++)for(j=0;j<15;j++)for(k=0;k<3;k++){
  struct{unsigned long before;unsigned char value[8];unsigned long after;}__attribute__((packed,aligned(2)))source;
  struct{unsigned long before;unsigned char value[10];unsigned long after;}__attribute__((packed,aligned(2)))dest;
  short src=i<32?i:80+((i-32)*3)%128,dst=i<32?destinations[i]:SANE_BASE_COUNT+514+((i-32)*3+37)%128;
  register unsigned short selector __asm__("d6")=ops[op];register unsigned long d0 __asm__("d0");register unsigned char *a2 __asm__("a2")=source.value,*a3 __asm__("a3")=dest.value;register unsigned short flag __asm__("d4")=flags[k];register unsigned short ccr __asm__("d3");register long delta __asm__("d5");unsigned long result;unsigned short status,env;long stack;
  source.before=0x11223344;source.after=0x55667788;dest.before=0x99aabbcc;dest.after=0xddeeff00;memcpy(source.value,widen_operands[src],8);memcpy(dest.value,sane_operands[dst],10);word=modes[j];environment(1,&word);d0=0x12345678;
  __asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,-(%%sp)\n\tmove.l %%a3,-(%%sp)\n\tmove.w %%d6,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xa9eb\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"=d"(ccr),"=d"(delta):"a"(a2),"a"(a3),"d"(flag),"d"(selector):"d1","d2","a0","a1","cc","memory");result=d0;status=ccr&31;stack=delta;env=0;environment(3,&env);
  p+=sprintf(p,"%s{\"selector\":%u,\"kind\":%d,\"source_index\":%d,\"dest_index\":%d,\"input_env\":%u,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"output_env\":%u,\"guards\":[%lu,%lu,%lu,%lu]",index++?",":"",selector,i,src,dst,modes[j],flags[k],result,status,stack,env,source.before,source.after,dest.before,dest.after);hex("source_before_hex",widen_operands[src],8);hex("dest_before_hex",sane_operands[dst],10);hex("source_after_hex",source.value,8);hex("dest_after_hex",dest.value,10);p+=sprintf(p,"}");
 }
 environment(1,&saved);p+=sprintf(p,"]}\n");err=Create("\pSANEMixedProbe.json",0,'pPrb','TEXT');if(err)return 2;if(FSOpen("\pSANEMixedProbe.json",0,&file))return 3;n=p-output;err=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
