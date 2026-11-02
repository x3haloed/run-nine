#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
#include "../sane/operands.h"
static char output[2097152],*p=output;
static void environment(short op,unsigned short *word){register unsigned short *a2 __asm__("a2")=word;register unsigned short selector __asm__("d6")=op;__asm__ volatile("move.l %%a2,-(%%sp)\n\tmove.w %%d6,-(%%sp)\n\t.word 0xa9eb"::"a"(a2),"d"(selector):"d0","d1","d2","a0","a1","cc","memory");}
static void hex(const char *name,const unsigned char *b){short i;p+=sprintf(p,",\"%s\":\"",name);for(i=0;i<10;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
int main(void){short pairs[40][2]={{2,2},{4,4},{6,4},{11,6},{3,6},{3,7},{0,2},{1,2},{0,21},{21,0},{21,6},{23,3},{24,2},{32,33},{33,32},{35,2},{36,37},{15,4},{16,4},{17,4},{17,6},{18,4},{19,19},{20,6},{29,2},{30,21},{31,2},{8,9},{10,11},{13,13},{14,14},{20,4},{0,0},{0,2},{1,3},{21,21},{23,21},{21,23},{2,0},{3,1}};unsigned short modes[15]={0,0x20,0x40,0x2000,0x2020,0x2040,0x4000,0x4020,0x4040,0x6000,0x6020,0x6040,0x0100,0x0800,0x1f00},saved=0,word;short i,j,k,index=0,file,flags[3]={16,31,0};long n;OSErr err;environment(3,&saved);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"sane-div-raw-v1\",\"records\":[");
 for(i=0;i<72;i++)for(j=0;j<15;j++)for(k=0;k<3;k++){
  struct{unsigned long before;unsigned char value[10];unsigned long after;}__attribute__((packed,aligned(2)))source,dest;
  short src=i<40?pairs[i][0]:SANE_BASE_COUNT+514+(i-40)*3,dst=i<40?pairs[i][1]:SANE_BASE_COUNT+514+((i-40)*3+37)%128;
  register unsigned long d0 __asm__("d0");register unsigned char *a2 __asm__("a2")=source.value,*a3 __asm__("a3")=dest.value;register unsigned short flag __asm__("d4")=flags[k];register unsigned short ccr __asm__("d3");register long delta __asm__("d5");unsigned long result;unsigned short status,env;long stack;
  source.before=0x11223344;source.after=0x55667788;dest.before=0x99aabbcc;dest.after=0xddeeff00;memcpy(source.value,sane_operands[src],10);memcpy(dest.value,sane_operands[dst],10);word=modes[j];environment(1,&word);d0=0x12345678;
  __asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,-(%%sp)\n\tmove.l %%a3,-(%%sp)\n\tmove.w #6,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xa9eb\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"=d"(ccr),"=d"(delta):"a"(a2),"a"(a3),"d"(flag):"d1","d2","a0","a1","cc","memory");result=d0;status=ccr&31;stack=delta;env=0;environment(3,&env);
  p+=sprintf(p,"%s{\"kind\":%d,\"source_index\":%d,\"dest_index\":%d,\"input_env\":%u,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"output_env\":%u,\"guards\":[%lu,%lu,%lu,%lu]",index++?",":"",i,src,dst,modes[j],flags[k],result,status,stack,env,source.before,source.after,dest.before,dest.after);hex("source_before_hex",sane_operands[src]);hex("dest_before_hex",sane_operands[dst]);hex("source_after_hex",source.value);hex("dest_after_hex",dest.value);p+=sprintf(p,"}");
 }
 environment(1,&saved);p+=sprintf(p,"]}\n");err=Create("\pSANEDivProbe.json",0,'pPrb','TEXT');if(err)return 2;if(FSOpen("\pSANEDivProbe.json",0,&file))return 3;n=p-output;err=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
