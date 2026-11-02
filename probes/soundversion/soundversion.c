#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
/* Measure the raw record-return ABI, independently of the LONGINT header. */
static void measure(unsigned long *result, unsigned char *record, unsigned short input_flags) {
 register unsigned long *saved __asm__("a3")=result;
 register unsigned char *buffer __asm__("a2")=record;
 register unsigned short flags __asm__("d4")=input_flags;
 __asm__ volatile(
  "move.l %%sp,%%a4\n\tlea -12(%%sp),%%sp\n\t"
  "move.l %%a2,(%%sp)\n\tmove.l #0x55555555,4(%%sp)\n\tmove.l #0x66666666,8(%%sp)\n\t"
  "move.l %%a2,%%a0\n\tmove.l #0x000c0008,%%d0\n\tmove.w %%d4,%%ccr\n\t.word 0xa800\n\t"
  " .word 0x42c2\n\tmove.l %%d0,(%%a3)+\n\t"
  "move.l %%a0,(%%a3)+\n\t"
  "move.l %%sp,%%d1\n\tsub.l %%a4,%%d1\n\tmove.l %%d1,(%%a3)+\n\t"
  "move.l -12(%%a4),(%%a3)+\n\tmove.l -8(%%a4),(%%a3)+\n\tmove.l -4(%%a4),(%%a3)+\n\t"
  "and.l #31,%%d2\n\tmove.l %%d2,(%%a3)+\n\tmove.l %%a4,%%sp"
  : "+a"(saved), "+a"(buffer) : "d"(flags) : "d0","d1","d2","a0","a1","a4","cc","memory");
}
int main(void) {
 char output[4096],*p=output;unsigned long values[7];unsigned char record[16];unsigned short inputs[3]={0x10,0x1f,0};short i,j,file;long n;OSErr e;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"sound-version-raw-v3\",\"cases\":[");
 for(i=0;i<3;i++) {
  memset(record,0x55,sizeof(record));memset(values,0,sizeof(values));measure(values,record+4,inputs[i]);
  p+=sprintf(p,"%s{\"input_ccr\":%u,\"d0\":%lu,\"raw_a0\":%ld,\"sp_delta\":%ld,\"stack_value\":%lu,\"stack_tail\":[%lu,%lu],\"ccr\":%lu,\"record_hex\":\"",i?",":"",inputs[i],values[0],values[1],values[2],values[3],values[4],values[5],values[6]);
  for(j=0;j<16;j++)p+=sprintf(p,"%02x",record[j]);p+=sprintf(p,"\"}");
 }
 p+=sprintf(p,"]}\n");e=Create("\pSoundVersionProbe.json",0,'pPrb','TEXT');if(e)return 2;
 if(FSOpen("\pSoundVersionProbe.json",0,&file))return 3;
 n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;
}
