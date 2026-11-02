#include <Multiverse.h>
#include <stdio.h>
#include "program.h"
int main(void){
 unsigned char *block=(unsigned char*)NewPtrClear(4928);char text[1024];short file,i;long n;OSErr e;
 if(!block)return 2;
 *(unsigned long*)(block+64)=(unsigned long)(block+128);*(unsigned long*)(block+68)=4800;
 *(unsigned long*)(block+72)=0xbb800000;block[85]=60;
 for(i=0;i<4800;i++)block[128+i]=i<1200||i>=3600?128:((i-1200)/32)&1?160:96;
 for(i=24;i<36;i+=2)*(short*)(block+i)=0x5555;
 {register unsigned char *result __asm__("a3")=block;register const unsigned char *program __asm__("a0")=sound_guest_program;
 __asm__ volatile("jsr (%%a0)":"+a"(result),"+a"(program)::"d0","d1","a1","cc","memory");}
 n=sprintf(text,"{\"probe\":\"sound-guest-v1\",\"errors\":[%d,%d,%d,%d,%d,%d],\"callbacks\":%lu,\"callback_param1\":%lu,\"callback_channel_matches\":%lu,\"volume\":%lu}\n",*(short*)(block+24),*(short*)(block+26),*(short*)(block+28),*(short*)(block+30),*(short*)(block+32),*(short*)(block+34),*(unsigned long*)(block+4),*(unsigned long*)(block+8),*(unsigned long*)(block+12),*(unsigned long*)(block+36));
 e=Create("\pSoundGuestProbe.json",0,'aPrb','TEXT');if(e||FSOpen("\pSoundGuestProbe.json",0,&file))return 3;
 e=FSWrite(file,&n,text);FSClose(file);DisposePtr((Ptr)block);FlushVol(0,0);return e?4:0;
}
