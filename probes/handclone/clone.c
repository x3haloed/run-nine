#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[262144],*p=output;
static void hex(const char *name,Handle h,long n){long i;unsigned char *b=h?(unsigned char*)*h:0;p+=sprintf(p,",\"%s\":\"",name);for(i=0;i<n;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
int main(void){short kind,state,k,f,index=0,states[4]={0,64,128,192},flags[3]={16,31,0};long lengths[5]={0,0,1,17,512},i,n;Handle source,dest,guard;OSErr err;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"hand-clone-raw-v1\",\"records\":[");
 for(kind=0;kind<5;kind++)for(state=0;state<(kind?4:1);state++)for(k=0;k<3;k++){
  register unsigned long d0 __asm__("d0")=0x12345678;register Handle a0 __asm__("a0");register unsigned short ccr __asm__("d3");register unsigned short flag __asm__("d4")=flags[k];register long delta __asm__("d5");unsigned long result;unsigned short saved;long stack;short memory_error;
  guard=NewHandle(37);if(!guard)return 8;for(i=0;i<37;i++)((unsigned char*)*guard)[i]=(unsigned char)(i*19+7);
  source=kind?NewHandle(lengths[kind]):0;if(kind&&!source)return 6;if(source){for(i=0;i<lengths[kind];i++)((unsigned char*)*source)[i]=(unsigned char)(i*37+19);HSetState(source,states[state]);}
  /* Load input D0 immediately before the trap, after fixture allocations. */
  d0=0x12345678;a0=source;
  __asm__ volatile("move.l %%sp,%%d5\n\tmove.w %%d4,%%ccr\n\t.word 0xa9e1\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"+a"(a0),"=d"(ccr),"=d"(delta):"d"(flag):"d1","d2","a1","cc","memory");result=d0;saved=ccr&31;stack=delta;dest=a0;memory_error=LMGetMemErr();
  p+=sprintf(p,"%s{\"kind\":%d,\"state\":%d,\"length\":%ld,\"input_d0\":305419896,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"mem_error\":%d,\"output_nonnull\":%d,\"distinct_source\":%d,\"source_size\":%ld,\"copy_size\":%ld,\"source_state\":%d,\"copy_state\":%d",index++?",":"",kind,states[state],lengths[kind],flags[k],result,saved,stack,memory_error,dest!=0,dest&&dest!=source,source?GetHandleSize(source):-1L,dest?GetHandleSize(dest):-1L,source?(unsigned char)HGetState(source):-1,dest?(unsigned char)HGetState(dest):-1);
  hex("source_hex",source,source?lengths[kind]:0);hex("copy_hex",dest,dest?lengths[kind]:0);hex("guard_hex",guard,37);p+=sprintf(p,"}");
  if(dest&&dest!=source)DisposeHandle(dest);if(source)DisposeHandle(source);DisposeHandle(guard);
 }
 p+=sprintf(p,"]}\n");err=Create("\pHandCloneProbe.json",0,'pPrb','TEXT');if(err)return 2;if(FSOpen("\pHandCloneProbe.json",0,&f))return 3;n=p-output;err=FSWrite(f,&n,output);FSClose(f);FlushVol(0,0);return err?4:0;
}
