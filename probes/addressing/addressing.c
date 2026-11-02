#include <Multiverse.h>
#include <stdio.h>
static char output[8192];
int main(void) {
 unsigned long inputs[4]={0,0x00123456,0xff123456,0xffffffff};unsigned short flags[3]={0x10,0x1f,0};
 char *p=output;short i,j,file;long n;OSErr e;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"addressing-v1\",\"addressing\":{\"mmu32bit\":%u,\"cases\":[",LMGetMMU32Bit());
 for(i=0;i<4;i++)for(j=0;j<3;j++) {
  register unsigned long d0 __asm__("d0")=inputs[i];
  register unsigned long a0 __asm__("a0")=0x12345678;
  register unsigned short ccr __asm__("d3");
  register unsigned short in_flags __asm__("d4")=flags[j];
  register unsigned long delta __asm__("d2");
  __asm__ volatile("move.l %%sp,%%d2\n\tmove.w %%d4,%%ccr\n\t.word 0xa055\n\t.word 0x42c3\n\tsub.l %%sp,%%d2"
   : "+d"(d0), "+a"(a0), "=d"(ccr), "=d"(delta) : "d"(in_flags) : "d1","a1","cc","memory");
  p+=sprintf(p,"%s{\"input\":%lu,\"input_ccr\":%u,\"output\":%lu,\"ccr\":%u,\"a0\":%lu,\"stack_delta\":%ld}",i||j?",":"",inputs[i],flags[j],d0,ccr&31,a0,delta);
 }
 p+=sprintf(p,"]}}\n");e=Create("\pAddressingProbe.json",0,'pPrb','TEXT');if(e)return 2;
 if(FSOpen("\pAddressingProbe.json",0,&file))return 3;
 n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;
}
