#include <QuickDraw.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
int main(void){char output[4096],*p=output;short i,file;long count;OSErr err;unsigned long selectors[]={0x73797376,0x71642020,0x70726f63,0x66707520,0x7a5a3921};
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"gestalt-v1\",\"gestalt\":[");
 for(i=0;i<5;i++){register unsigned long d0 __asm__("d0")=selectors[i];register unsigned long a0 __asm__("a0")=0x12345678;register unsigned short flags __asm__("d3");
 __asm__ volatile("move.w #0x10,%%ccr\n\t.word 0xa1ad\n\t.word 0x42c3" : "+d"(d0),"+a"(a0),"=d"(flags) : : "d1","d2","a1","cc","memory");
 p+=sprintf(p,"%s[%lu,%lu,%lu,%u]",i?",":"",selectors[i],d0,a0,flags&31);
 }p+=sprintf(p,"]}\n");err=Create("\pGestaltProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pGestaltProbe.json",0,&file))return 3;SetEOF(file,0);count=p-output;err=FSWrite(file,&count,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
