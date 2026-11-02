#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[32768],*p=output;
static void hex(Ptr data,short n){short i;for(i=0;i<n;i++)p+=sprintf(p,"%02x",(unsigned char)data[i]);}
int main(void) {
 short mode,k,i,file,input[3]={16,31,0};long bytes;OSErr err;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"move-high-raw-v4\",\"cases\":[");
 for(mode=0;mode<9;mode++)for(k=0;k<3;k++){
  Ptr guard=NewPtr(16);Handle h=mode==3?0:NewHandle(mode==2?0:32);Handle other=NewHandle(48);Ptr barrier=mode==6||mode==8?NewPtr(32):0;unsigned long before,after,other_before,sum=0,other_sum=0;short state_before,state_after,mem,guard_ok=1,barrier_ok=1,same;long captured_result,captured_stack;unsigned short captured_ccr;
  register Handle a0 __asm__("a0");register long d0 __asm__("d0");register unsigned short ccr __asm__("d3");register unsigned short flags __asm__("d4")=input[k];register unsigned long stack __asm__("d5");
  if(!guard||!other||(mode!=3&&!h)||((mode==6||mode==8)&&!barrier))return 5;memset(guard,0xaa,16);for(i=0;i<48;i++)(*other)[i]=i+33;
  if(h&&mode!=2)for(i=0;i<32;i++)(*h)[i]=i+1;
  if(h&&mode==1)HLock(h);if(h&&mode==4)HPurge(h);if(h&&mode==5)HSetRBit(h);
  if(barrier)memset(barrier,0xbb,32);if(mode==7)HLock(other);if(mode==8){DisposeHandle(other);other=0;}
  before=h?(unsigned long)*h:0;other_before=other?(unsigned long)*other:0;state_before=h?(unsigned char)HGetState(h):0;
  a0=h;d0=0x12345678;
  __asm__ volatile("move.l %%sp,%%d5\n\tmove.w %%d4,%%ccr\n\t.word 0xa064\n\t.word 0x42c3\n\tsub.l %%sp,%%d5" : "+a"(a0),"+d"(d0),"=d"(ccr),"=d"(stack):"d"(flags):"d1","d2","a1","cc","memory");
  captured_result=d0;captured_stack=stack;captured_ccr=ccr&31;same=a0==h;
  mem=MemError();after=h?(unsigned long)*h:0;state_after=h?(unsigned char)HGetState(h):0;
  if(h&&mode!=2)for(i=0;i<32;i++)sum+=(unsigned char)(*h)[i];if(other)for(i=0;i<48;i++)other_sum+=(unsigned char)(*other)[i];for(i=0;i<16;i++)if((unsigned char)guard[i]!=0xaa)guard_ok=0;if(barrier)for(i=0;i<32;i++)if((unsigned char)barrier[i]!=0xbb)barrier_ok=0;
  p+=sprintf(p,"%s{\"mode\":%d,\"input_ccr\":%d,\"result\":%ld,\"ccr\":%u,\"a0_same\":%d,\"stack_delta\":%ld,\"mem_error\":%d,\"before_ptr\":%lu,\"after_ptr\":%lu,\"other_before\":%lu,\"other_after\":%lu,\"state_before\":%d,\"state_after\":%d,\"sum\":%lu,\"other_sum\":%lu,\"guard_ok\":%d,\"barrier_ptr\":%lu,\"barrier_ok\":%d,\"payload_hex\":\"",mode||k?",":"",mode,input[k],captured_result,captured_ccr,same,captured_stack,mem,before,after,other_before,other?(unsigned long)*other:0,state_before,state_after,sum,other_sum,guard_ok,(unsigned long)barrier,barrier_ok);
  if(h&&mode!=2)hex(*h,32);p+=sprintf(p,"\",\"other_hex\":\"");if(other)hex(*other,48);p+=sprintf(p,"\",\"guard_hex\":\"");hex(guard,16);p+=sprintf(p,"\",\"barrier_hex\":\"");if(barrier)hex(barrier,32);p+=sprintf(p,"\"}");
  if(h){HSetState(h,0);DisposeHandle(h);}if(other){HSetState(other,0);DisposeHandle(other);}if(barrier)DisposePtr(barrier);DisposePtr(guard);
 }
 p+=sprintf(p,"]}\n");err=Create("\pMoveHHiProbe.json",0,'pPrb','TEXT');if(err)return 2;if(FSOpen("\pMoveHHiProbe.json",0,&file))return 3;bytes=p-output;err=FSWrite(file,&bytes,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
