#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[262144],*p=output;
static void hex(unsigned char*b,short n){short i;p+=sprintf(p,"\"");for(i=0;i<n;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
int main(void){short values[]={0,1,3,7,8,-1,32767,-32768},flags[]={16,31,0},selector,v,k,seed,index=0,file,original;long n,original_stereo;OSErr e;
 InitGraf(&qd.thePort);InitFonts();GetSysBeepVolume(&original_stereo);original=LMGetSdVolume();
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"sound-control-raw-v1\",\"records\":[");
 for(selector=0;selector<2;selector++)for(v=0;v<8;v++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
  struct{unsigned long before;unsigned char bytes[50];unsigned long after;} pb;short before,after,before_sound,after_sound,before_error,after_error;long before_stereo,after_stereo;unsigned long d0,a0;unsigned short ccr;long delta;unsigned char snapshot[50];
  SetSysBeepVolume(0x00600060);before=LMGetSdVolume();before_sound=LMGetSoundLevel();before_error=GetSysBeepVolume(&before_stereo);pb.before=0x11223344;pb.after=0x55667788;memset(pb.bytes,0xa5,50);*(long*)(pb.bytes)=0;*(short*)(pb.bytes+4)=0;*(short*)(pb.bytes+6)=0x5a5a;*(long*)(pb.bytes+8)=0;*(long*)(pb.bytes+12)=0;*(short*)(pb.bytes+16)=0x1234;*(long*)(pb.bytes+18)=0;*(short*)(pb.bytes+22)=0;*(short*)(pb.bytes+24)=-4;*(short*)(pb.bytes+26)=2;*(short*)(pb.bytes+28)=values[v];
  {register unsigned char*a2 __asm__("a2")=pb.bytes;register unsigned long result __asm__("d0")=seed?(unsigned long)(long)values[v]:0x12345678;register unsigned short input __asm__("d4")=flags[k];register unsigned short status __asm__("d3");register long stack __asm__("d5");register unsigned long addr __asm__("d6");
   if(selector==0){__asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,%%a0\n\tmove.w %%d4,%%ccr\n\t.word 0xa004\n\t.word 0x42c3\n\tmove.l %%a0,%%d6\n\tsub.l %%sp,%%d5":"+d"(result),"=d"(status),"=d"(stack),"=d"(addr):"a"(a2),"d"(input):"d1","d2","a0","a1","cc","memory");}
   else{__asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,%%a0\n\tmove.w %%d4,%%ccr\n\t.word 0xa204\n\t.word 0x42c3\n\tmove.l %%a0,%%d6\n\tsub.l %%sp,%%d5":"+d"(result),"=d"(status),"=d"(stack),"=d"(addr):"a"(a2),"d"(input):"d1","d2","a0","a1","cc","memory");}
   d0=result;ccr=status&31;delta=stack;a0=addr;
  }
  memcpy(snapshot,pb.bytes,50);after=LMGetSdVolume();after_sound=LMGetSoundLevel();after_error=GetSysBeepVolume(&after_stereo);
  p+=sprintf(p,"%s{\"trap\":%u,\"value\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"before_volume\":%d,\"after_volume\":%d,\"before_sound_level\":%d,\"after_sound_level\":%d,\"before_stereo\":%ld,\"after_stereo\":%ld,\"before_query_error\":%d,\"after_query_error\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"a0_preserved\":%d,\"before_guard\":%lu,\"after_guard\":%lu,\"pb_hex\":",index++?",":"",selector?0xa204:0xa004,values[v],seed?(unsigned long)(long)values[v]:0x12345678,flags[k],before,after,before_sound,after_sound,before_stereo,after_stereo,before_error,after_error,d0,ccr,delta,a0==(unsigned long)pb.bytes,pb.before,pb.after);hex(snapshot,50);p+=sprintf(p,"}");
 }
 p+=sprintf(p,"]}\n");SetSysBeepVolume(original_stereo);
 e=Create("\pSoundControlProbe.json",0,'pPrb','TEXT');if(e)return 2;if(FSOpen("\pSoundControlProbe.json",0,&file))return 3;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;
}
