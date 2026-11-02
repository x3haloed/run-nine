#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[16384],*p=output;
static unsigned char *entry(void){Handle map=*(Handle*)0xa50;short app=CurResFile(),tries;
 for(tries=0;map&&tries<1024;tries++){unsigned char *m=(unsigned char*)*map;
  if(*(short*)(m+20)==app){unsigned char *types=m+*(unsigned short*)(m+24);short n=*(short*)types,i;
   for(i=0;i<=n;i++){unsigned char *t=types+2+8*i;if(*(unsigned long*)t=='TEST'){unsigned char *r=types+*(unsigned short*)(t+6);short j,count=*(short*)(t+4);for(j=0;j<=count;j++)if(*(short*)(r+12*j)==128)return r+12*j;}}
  }map=*(Handle*)(m+16);
 }return 0;
}
static void payload(const char *key,Handle h,short valid){long n=valid?GetHandleSize(h):0;short i;p+=sprintf(p,",\"%s\":\"",key);if(valid&&n==6)for(i=0;i<6;i++)p+=sprintf(p,"%02x",(unsigned char)(*h)[i]);p+=sprintf(p,"\"");}
int main(void){short scenario,k,index=0,flags[3]={16,31,0},app=CurResFile(),file;long size;OSErr e;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"app-resource-release-v1\",\"resources\":{\"records\":[");
 for(scenario=0;scenario<4;scenario++)for(k=0;k<3;k++){
  Handle resource=Get1Resource('TEST',128),h,again;unsigned char *ref;short before_state,error,initial_error,after_state=-1,home=12345,home_error=12345,reload_error,home_same,current_same,entry_before,entry_zero,entry_after;unsigned long result;unsigned short ccr;long delta;
  if(!resource||GetHandleSize(resource)!=6)return 2;
  h=scenario==1?0:scenario==2?NewHandle(6):resource;if(scenario==2&&!h)return 3;
  if(scenario==2)memcpy(*h,"PLAIN!",6);else if(h)(*h)[0]='Z';
  before_state=h?(unsigned char)HGetState(h):-1;initial_error=ResError();ref=entry();if(!ref)return 4;entry_before=*(Handle*)(ref+8)==resource;
  {register Handle a2 __asm__("a2")=h;register unsigned long d0 __asm__("d0")=0x12345678;register unsigned short flag __asm__("d4")=flags[k];register unsigned short status __asm__("d3");register long stack __asm__("d5");
   if(scenario==3){__asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xa992\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"=d"(status),"=d"(stack):"a"(a2),"d"(flag):"d1","d2","a0","a1","cc","memory");}
   else{__asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xa9a3\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"=d"(status),"=d"(stack):"a"(a2),"d"(flag):"d1","d2","a0","a1","cc","memory");}
   result=d0;ccr=status&31;delta=stack;
  }
  error=ResError();ref=entry();if(!ref)return 5;entry_zero=*(Handle*)(ref+8)==0;
  if(scenario==2||scenario==3){after_state=(unsigned char)HGetState(h);}
  if(scenario==3){home=HomeResFile(h);home_error=ResError();}
  again=Get1Resource('TEST',128);reload_error=ResError();if(!again)return 6;home_same=HomeResFile(again)==app;current_same=CurResFile()==app;ref=entry();entry_after=ref&&*(Handle*)(ref+8)==again;
  p+=sprintf(p,"%s{\"case\":%d,\"input_ccr\":%d,\"initial_error\":%d,\"before_state\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"error\":%d,\"after_state\":%d,\"home\":%d,\"home_error\":%d,\"entry_before\":%d,\"entry_zero_after_call\":%d,\"entry_matches_reload\":%d,\"reload_error\":%d,\"reload_home_same\":%d,\"current_same\":%d,\"reload_resource_bit\":%d",index++?",":"",scenario,flags[k],initial_error,before_state,result,ccr,delta,error,after_state,home,home_error,entry_before,entry_zero,entry_after,reload_error,home_same,current_same,((unsigned char)HGetState(again)&32)!=0);
  payload("retained_payload",h,scenario==2||scenario==3);payload("reloaded_payload",again,1);p+=sprintf(p,"}");
  if(scenario==2||scenario==3)DisposeHandle(h);
 }
 p+=sprintf(p,"]}}\n");e=Create("\pAppReleaseProbe.json",0,'pPrb','TEXT');if(e)return 7;if(FSOpen("\pAppReleaseProbe.json",0,&file))return 8;size=p-output;e=FSWrite(file,&size,output);FSClose(file);FlushVol(0,0);return e?9:0;
}
