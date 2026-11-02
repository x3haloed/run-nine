#include <Multiverse.h>
#include <stdio.h>
static char output[65536],*p=output;
static void bytes(const char*key,Handle h,long n){long i;p+=sprintf(p,",\"%s\":\"",key);if(n>=0&&n<=2056)for(i=0;i<n;i++)p+=sprintf(p,"%02x",(unsigned char)(*h)[i]);p+=sprintf(p,"\"");}
int main(void){short which,k,index=0,file,flags[]={16,31,0};long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();p+=sprintf(p,"{\"schema\":1,\"probe\":\"ctab-changed-raw-v1\",\"records\":[");
 for(which=0;which<2;which++)for(k=0;k<3;k++){
  CTabHandle h=GetCTable(which?8:128);unsigned long base,after,d0;unsigned short ccr;long delta,size;short before_state,after_state,mem_error,res_error;if(!h)return 2;size=GetHandleSize((Handle)h);before_state=(unsigned char)HGetState((Handle)h);(**h).ctSeed=0x10203040;base=GetCTSeed();
  p+=sprintf(p,"%s{\"which\":%d,\"input_ccr\":%d,\"size\":%ld,\"before_state\":%d,\"base\":%lu",index++?",":"",which,flags[k],size,before_state,base);bytes("before_hex",(Handle)h,size);*(short*)0x220=-123;*(short*)0xa60=-192;
  {register CTabHandle a2 __asm__("a2")=h;register unsigned long result __asm__("d0")=0x00040007;register unsigned short input __asm__("d4")=flags[k];register unsigned short status __asm__("d3");register long stack __asm__("d5");
   __asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xab1d\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(result),"=d"(status),"=d"(stack):"a"(a2),"d"(input):"d1","d2","a0","a1","cc","memory");d0=result;ccr=status&31;delta=stack;
  }
  mem_error=*(short*)0x220;res_error=*(short*)0xa60;after=GetCTSeed();after_state=(unsigned char)HGetState((Handle)h);
  p+=sprintf(p,",\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"after\":%lu,\"after_state\":%d,\"memory_error\":%d,\"resource_error\":%d",d0,ccr,delta,after,after_state,mem_error,res_error);bytes("after_hex",(Handle)h,size);p+=sprintf(p,"}");DisposeCTable(h);
 }
 p+=sprintf(p,"]}\n");e=Create("\pCTabChangedProbe.json",0,'pPrb','TEXT');if(e)return 3;if(FSOpen("\pCTabChangedProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
