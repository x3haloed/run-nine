#include <Memory.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static long set_state(Handle h, long flags, short *z, short *n) {
 register long d0 __asm__("d0")=flags;
 register Handle a0 __asm__("a0")=h;
 unsigned char zero,negative;
 __asm__ volatile(".word 0xa06a\n\tseq %2\n\tsmi %3" : "+d"(d0),"+a"(a0),"=d"(zero),"=d"(negative) : : "cc","memory");
 *z=zero!=0;*n=negative!=0;return d0;
}
static long purge_state(Handle h,short purge,short *z,short *n) {
 register long d0 __asm__("d0")=0x12345678L;
 register Handle a0 __asm__("a0")=h;
 unsigned char zero,negative;
 if(purge){__asm__ volatile(".word 0xa049\n\tseq %2\n\tsmi %3" : "+d"(d0),"+a"(a0),"=d"(zero),"=d"(negative) : : "cc","memory");}
 else {__asm__ volatile(".word 0xa04a\n\tseq %2\n\tsmi %3" : "+d"(d0),"+a"(a0),"=d"(zero),"=d"(negative) : : "cc","memory");}
 *z=zero!=0;*n=negative!=0;return d0;
}
int main(void) {
 char out[8192],*p=out;short mode,i,z,n,file,error,state_error;long result,count;Handle h;
 unsigned char flags[]={0,32,64,96,128,160,192,224,255,31};
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"handle-state-v1\",\"memory\":{\"states\":[");
 for(mode=0;mode<2;mode++)for(i=0;i<10;i++){
  h=NewHandle(16);if(!h)return 2;if(mode)HSetRBit(h);
  result=set_state(h,0x12345600L|flags[i],&z,&n);error=MemError();
  {unsigned char state=HGetState(h);state_error=MemError();p+=sprintf(p,"%s[%d,%d,%ld,%d,%d,%d,%d,%d]",mode||i?",":"",mode,flags[i],result,error,state,state_error,z,n);}
  HSetState(h,0);DisposeHandle(h);
 }
 result=set_state(0,0x12345680L,&z,&n);error=MemError();
 p+=sprintf(p,"],\"nil_set\":[%ld,%d,%d,%d]",result,error,z,n);
 {unsigned char state=HGetState(0);state_error=MemError();p+=sprintf(p,",\"nil_get\":[%d,%d],\"purge\":[",state,state_error);}
 for(mode=0;mode<2;mode++)for(i=0;i<=8;i++){
  h=i<8?NewHandle(16):0;if(i<8&&!h)return 6;if(h)HSetState(h,i*32);
  result=purge_state(h,mode,&z,&n);error=MemError();
  {unsigned char state=HGetState(h);state_error=MemError();p+=sprintf(p,"%s[%d,%d,%ld,%d,%d,%d,%d,%d]",mode||i?",":"",mode,i*32,result,error,state,state_error,z,n);}
  if(h){HSetState(h,0);DisposeHandle(h);}
 }
 p+=sprintf(p,"]}}\n");
 if(Create("\pHandleStateProbe.json",0,'ttxt','TEXT'))return 3;
 if(FSOpen("\pHandleStateProbe.json",0,&file))return 4;
 count=strlen(out);error=FSWrite(file,&count,out);FSClose(file);FlushVol(0,0);return error?5:0;
}
