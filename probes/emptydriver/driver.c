#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[262144],*p=output;
#define CALL(OP) __asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,%%a0\n\tmove.w %%d4,%%ccr\n\t.word " #OP "\n\t.word 0x42c3\n\tmove.l %%a0,%%d6\n\tsub.l %%sp,%%d5":"+d"(result),"=d"(status),"=d"(stack),"=d"(addr):"a"(a2),"d"(input):"d1","d2","a0","a1","cc","memory")
int main(void){short refs[]={-4,-32768,0,1},codes[]={2,-1},flags[]={16,31,0},ri,ci,k,si,oi,index=0,file,count;unsigned short ops[]={0xa004,0xa204,0xa005,0xa205};long n;OSErr e;Handle*table;
 InitGraf(&qd.thePort);InitFonts();count=LMGetUnitNtryCnt();table=(Handle*)LMGetUTableBase();
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"empty-driver-raw-v1\",\"unit_count\":%d,\"records\":[",count);
 for(oi=0;oi<4;oi++)for(ri=0;ri<4;ri++)for(ci=0;ci<2;ci++)for(si=0;si<2;si++)for(k=0;k<3;k++){
  struct{unsigned long before;unsigned char bytes[50];unsigned long after;} pb;unsigned short unit=(unsigned short)~(unsigned short)refs[ri];short in_range=unit<count,empty=in_range?table[unit]==0:-1;unsigned long d0,a0;unsigned short ccr;long delta;short j;
  if(in_range&&!empty)return 2;
  pb.before=0x11223344;pb.after=0x55667788;memset(pb.bytes,0xa5,50);*(long*)pb.bytes=0;*(short*)(pb.bytes+4)=0;*(short*)(pb.bytes+6)=0x5a5a;*(long*)(pb.bytes+8)=0;*(long*)(pb.bytes+12)=0;*(short*)(pb.bytes+16)=0x1234;*(long*)(pb.bytes+18)=0;*(short*)(pb.bytes+22)=0;*(short*)(pb.bytes+24)=refs[ri];*(short*)(pb.bytes+26)=codes[ci];*(short*)(pb.bytes+28)=3;
  {register unsigned char*a2 __asm__("a2")=pb.bytes;register unsigned long result __asm__("d0")=si?0:0x12345678;register unsigned short input __asm__("d4")=flags[k];register unsigned short status __asm__("d3");register long stack __asm__("d5");register unsigned long addr __asm__("d6");
   if(oi==0){CALL(0xa004);}else if(oi==1){CALL(0xa204);}else if(oi==2){CALL(0xa005);}else{CALL(0xa205);}
   d0=result;ccr=status&31;delta=stack;a0=addr;
  }
  p+=sprintf(p,"%s{\"trap\":%u,\"reference\":%d,\"code\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"unit_index\":%u,\"in_range\":%d,\"entry_empty\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"a0_preserved\":%d,\"before_guard\":%lu,\"after_guard\":%lu,\"pb_hex\":\"",index++?",":"",ops[oi],refs[ri],codes[ci],si?0:0x12345678,flags[k],unit,in_range,empty,d0,ccr,delta,a0==(unsigned long)pb.bytes,pb.before,pb.after);for(j=0;j<50;j++)p+=sprintf(p,"%02x",pb.bytes[j]);p+=sprintf(p,"\"}");
 }
 p+=sprintf(p,"]}\n");e=Create("\pEmptyDriverProbe.json",0,'pPrb','TEXT');if(e)return 3;if(FSOpen("\pEmptyDriverProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
