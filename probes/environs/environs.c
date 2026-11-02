#include <QuickDraw.h>
#include <OSUtils.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static unsigned long query(short version, void *buffer,unsigned short *flags,short *same) {
 register unsigned long d0 __asm__("d0")=0x12340000UL|(unsigned short)version;
 register void *a0 __asm__("a0")=buffer;register unsigned short ccr __asm__("d3");
 __asm__ volatile("move.w #0x10,%%ccr\n\t.word 0xa090\n\t.word 0x42c3" : "+d"(d0),"+a"(a0),"=d"(ccr) : : "d1","d2","a1","cc","memory");
 *flags=ccr&31;*same=a0==buffer;return d0;
}
int main(void){unsigned char buffer[20],baseline[16];unsigned short flags;short versions[]={-1,0,1,2,3,32767},same,i,j,file;unsigned long result;char output[4096],*p=output;long count;OSErr err;
 memset(baseline,0xa5,sizeof(baseline));SysEnvirons(2,(SysEnvRec *)baseline);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"environs-v1\",\"environment\":{\"profile\":[");for(i=0;i<16;i++)p+=sprintf(p,"%s%u",i?",":"",baseline[i]);p+=sprintf(p,"],\"queries\":[");
 for(i=0;i<6;i++){memset(buffer,0xa5,sizeof(buffer));result=query(versions[i],buffer+2,&flags,&same);p+=sprintf(p,"%s{\"version\":%d,\"d0\":%lu,\"ccr\":%u,\"a0_same\":%d,\"bytes\":[",i?",":"",versions[i],result,flags,same);for(j=0;j<20;j++)p+=sprintf(p,"%s%u",j?",":"",buffer[j]);p+=sprintf(p,"]}");}p+=sprintf(p,"]}}\n");
 err=Create("\pEnvironsProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pEnvironsProbe.json",0,&file))return 3;SetEOF(file,0);count=p-output;err=FSWrite(file,&count,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
