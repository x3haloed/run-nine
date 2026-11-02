#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[2048],raw[2048];
static long delay_call(long request,long *after_arg){register long d0 __asm__("d0")=0x5a5a5a5a;register long a0 __asm__("a0")=request;__asm__ volatile(".word 0xa03b" : "+d"(d0),"+a"(a0) : : "cc","memory");*after_arg=a0;return d0;}
static short write_output(ConstStr255Param name,char *bytes){short f;long n=strlen(bytes);OSErr e=Create(name,0,'ttxt','TEXT');if(e&&e!=dupFNErr)return 2;if(FSOpen(name,0,&f))return 3;SetEOF(f,0);e=FSWrite(f,&n,bytes);FSClose(f);return e?4:0;}
int main(void){short i;long before,after,result,arg;char *p=output,*r=raw;p+=sprintf(p,"{\"schema\":1,\"probe\":\"delay-v1\",\"timing\":{\"cases\":[");r+=sprintf(r,"{\"ticks\":[");for(i=0;i<3;i++){before=TickCount();result=delay_call(i,&arg);after=TickCount();p+=sprintf(p,"%s[%d,%ld,%d,%d,%d]",i?",":"",i,arg,(unsigned long)(after-before)>=(unsigned long)i,(unsigned long)(result-before)>=(unsigned long)i,(unsigned long)(after-result)<0x80000000UL);r+=sprintf(r,"%s[%d,%ld,%ld,%ld,%ld]",i?",":"",i,before,result,after,arg);}p+=sprintf(p,"]}}\n");r+=sprintf(r,"]}\n");if(write_output("\pDelayProbe-raw.json",raw))return 5;if(write_output("\pDelayProbe.json",output))return 6;FlushVol(0,0);return 0;}
