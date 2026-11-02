#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static void measure(short synth,long init,SndChannelPtr *channel,unsigned long *out,short input_flags) {
 register SndChannelPtr *pointer __asm__("a2")=channel;
 register unsigned long *result __asm__("a3")=out;
 register short kind __asm__("d4")=synth;
 register long options __asm__("d5")=init;
 register short flags __asm__("d6")=input_flags;
 __asm__ volatile(
  "move.l %%sp,%%a4\n\tlea -20(%%sp),%%sp\n\t"
  "clr.l (%%sp)\n\tmove.l %%d5,4(%%sp)\n\tmove.w %%d4,8(%%sp)\n\tmove.l %%a2,10(%%sp)\n\t"
  "move.w #0x5555,14(%%sp)\n\tmove.l #0x66667777,16(%%sp)\n\tmove.w %%d6,%%ccr\n\t.word 0xa807\n\t.word 0x42c2\n\t"
  "moveq #0,%%d1\n\tmove.w -6(%%a4),%%d1\n\text.l %%d1\n\tmove.l %%d1,(%%a3)+\n\t"
  "and.l #31,%%d2\n\tmove.l %%d2,(%%a3)+\n\tmove.l %%sp,%%d1\n\tsub.l %%a4,%%d1\n\tmove.l %%d1,(%%a3)+\n\t"
  "move.l -4(%%a4),(%%a3)+\n\tmove.l %%a4,%%sp"
  : "+a"(pointer), "+a"(result) : "d"(kind), "d"(options), "d"(flags) : "d0","d1","d2","a0","a1","a4","cc","memory");
}
int main(void) {
 char output[8192],*p=output;short i,j,file;long n;OSErr e;unsigned long result[4];short flags[3]={16,31,0};
 struct {unsigned long before;SndChannelPtr channel;unsigned long after;} state;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"sound-channels-raw-v1\",\"cases\":[");
 for(i=0;i<3;i++)for(j=0;j<3;j++) {
  short synth=i? -1:5;long init=i==1?'zZ9!':i==2?'tst!':0;
  state.before=0x11223344;state.channel=0;state.after=0x55667788;memset(result,0,sizeof(result));
  measure(synth,init,&state.channel,result,flags[j]);
  p+=sprintf(p,"%s{\"synth\":%d,\"init\":%lu,\"input_ccr\":%d,\"error\":%ld,\"ccr\":%lu,\"stack_delta\":%ld,\"stack_guard\":%lu,\"channel_null\":%d,\"pointer_guards\":[%lu,%lu]}",i||j?",":"",synth,init,flags[j],result[0],result[1],result[2],result[3],state.channel==0,state.before,state.after);
  if(!result[0]&&state.channel)SndDisposeChannel(state.channel,1);
 }
 p+=sprintf(p,"]}\n");e=Create("\pSoundChannelProbe.json",0,'pPrb','TEXT');if(e)return 2;
 if(FSOpen("\pSoundChannelProbe.json",0,&file))return 3;
 n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;
}
