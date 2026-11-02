#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
#include "program.h"
int main(void) {
 unsigned char bytes[44];char output[1024],*p=output;short i,file;long n;OSErr e;
 memset(bytes,0x55,sizeof(bytes));
 register unsigned char *result __asm__("a3")=bytes+4;
 register const unsigned char *program __asm__("a0")=owned68k_program;
 __asm__ volatile("jsr (%%a0)" : "+a"(result), "+a"(program) : : "d0","d1","a1","cc","memory");
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"owned-68k-v1\",\"execution\":{\"result_hex\":\"");
 for(i=0;i<44;i++)p+=sprintf(p,"%02x",bytes[i]);
 p+=sprintf(p,"\"}}\n");e=Create("\pOwned68KProbe.json",0,'pPrb','TEXT');if(e)return 2;
 if(FSOpen("\pOwned68KProbe.json",0,&file))return 3;
 n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;
}
