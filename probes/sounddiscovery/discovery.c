#include <Multiverse.h>
#include <stdio.h>
static unsigned long osvector(unsigned long code){
 register unsigned long number __asm__("d0")=code;register unsigned long result __asm__("a0");
 __asm__ volatile(".word 0xa346":"+d"(number),"=a"(result)::"d1","a1","cc","memory");return result;
}
int main(void){short f;long n,attributes=0;char text[1024];OSErr e;
 unsigned long a=osvector(0xad),b=osvector(0xa0ad),c=osvector(0xa1ad),missing=osvector(0xffff);
 e=Gestalt('snd ',&attributes);
 n=sprintf(text,"{\"probe\":\"sound-discovery-v1\",\"osvector_ad_nonnull\":%d,\"osvector_ad_a0ad_equal\":%d,\"osvector_ad_a1ad_equal\":%d,\"gestalt_not_missing_vector\":%d,\"sound_attributes_error\":%d,\"sound_attributes\":%lu}\n",a!=0,a==b,a==c,a!=missing,e,attributes);
 if(Create("\pSoundDiscoveryProbe.json",0,'aPrb','TEXT')||FSOpen("\pSoundDiscoveryProbe.json",0,&f))return 2;
 e=FSWrite(f,&n,text);FSClose(f);FlushVol(0,0);return e?3:0;
}
