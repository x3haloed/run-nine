#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[1048576],*p=output;
static unsigned char *entry(void){Handle map=*(Handle*)0xa50;short app=CurResFile(),tries;
 for(tries=0;map&&tries<1024;tries++){unsigned char *m=(unsigned char*)*map;
  if(*(short*)(m+20)==app){unsigned char *types=m+*(unsigned short*)(m+24);short n=*(short*)types,i;
   for(i=0;i<=n;i++){unsigned char *t=types+2+8*i;if(*(unsigned long*)t=='clut'){unsigned char *r=types+*(unsigned short*)(t+6);short j,count=*(short*)(t+4);for(j=0;j<=count;j++)if(*(short*)(r+12*j)==128)return r+12*j;}}
  }map=*(Handle*)(m+16);
 }return 0;
}
#pragma pack(push,2)
typedef struct {short id,input;Handle result;unsigned long d0;unsigned short ccr;long delta;unsigned long guard;} ABI;
#pragma pack(pop)
static void call(ABI *c){register ABI *args __asm__("a2")=c;
 __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l #0x7b7b7b7b,-(%%sp)\n\tmove.w (%%a2),-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l #0x12345678,%%d0\n\tmove.w 2(%%a2),%%ccr\n\t.word 0xaa18\n\t.word 0x42c3\n\tmove.l %%d0,8(%%a2)\n\tmove.w %%d3,12(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,14(%%a2)\n\tmove.l (%%sp)+,4(%%a2)\n\tmove.l (%%sp)+,18(%%a2)"
 : "+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory");
}
static void bytes(const char *key,Handle h){long n=h?GetHandleSize(h):0,i;p+=sprintf(p,",\"%s\":\"",key);if(h&&n>=0&&n<=2056)for(i=0;i<n;i++)p+=sprintf(p,"%02x",(unsigned char)(*h)[i]);p+=sprintf(p,"\"");}
int main(void){short ids[]={-1,0,1,2,3,4,8,16,32,33,34,36,40,64,128,129,256,1000,5000},flags[]={16,31,0};short i,k,index=0,file;long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"get-color-table-raw-v1\",\"records\":[");
 for(i=0;i<19;i++)for(k=0;k<3;k++){ABI c;Handle again;short re,me,state,home,he,same,eq;long size;unsigned long seed;
  c.id=ids[i];c.input=flags[k];call(&c);re=ResError();me=MemError();state=c.result?(unsigned char)HGetState(c.result):-1;size=c.result?GetHandleSize(c.result):0;
  home=c.result?HomeResFile(c.result):12345;he=c.result?ResError():12345;seed=c.result&&size>=8?*(unsigned long*)*c.result:0;
  again=(Handle)GetCTable(ids[i]);same=again==c.result;eq=again&&c.result&&GetHandleSize(again)==size&&!memcmp(*again,*c.result,size);
  p+=sprintf(p,"%s{\"id\":%d,\"input_ccr\":%d,\"present\":%d,\"d0\":%lu,\"d0_is_handle\":%d,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"resource_error\":%d,\"memory_error\":%d,\"state\":%d,\"size\":%ld,\"home\":%d,\"home_error\":%d,\"repeat_same\":%d,\"repeat_bytes_equal\":%d,\"repeat_seed_delta\":%lu",index++?",":"",ids[i],flags[k],c.result!=0,c.d0,c.result&&c.d0==(unsigned long)c.result,c.ccr&31,c.delta,c.guard,re,me,state,size,home,he,same,eq,again&&GetHandleSize(again)>=8?*(unsigned long*)*again-seed:0);
  bytes("bytes",c.result);bytes("repeat_bytes",again);p+=sprintf(p,"}");if(again&&again!=c.result)DisposeCTable((CTabHandle)again);if(c.result)DisposeCTable((CTabHandle)c.result);
 }
 p+=sprintf(p,"],\"ownership\":[");
 for(k=0;k<3;k++){Handle source=Get1Resource('clut',128),h,again;unsigned char *r=entry();short before,after,source_same,source_state;ABI c;if(!source||!r)return 2;
  (*source)[10]=0x55;before=*(Handle*)(r+8)==source;c.id=128;c.input=flags[k];call(&c);h=c.result;if(!h)return 3;r=entry();after=*(Handle*)(r+8)==0;source_same=h==source;source_state=(unsigned char)HGetState(source);
  again=Get1Resource('clut',128);
  p+=sprintf(p,"%s{\"input_ccr\":%d,\"entry_before\":%d,\"entry_zero_after\":%d,\"same_as_source\":%d,\"source_state\":%d,\"result_state\":%d,\"reload_same\":%d",k?",":"",flags[k],before,after,source_same,source_state,(unsigned char)HGetState(h),again==source);
  bytes("result_bytes",h);bytes("reload_bytes",again);p+=sprintf(p,"}");if(h!=again)DisposeCTable((CTabHandle)h);ReleaseResource(again);
 }
 p+=sprintf(p,"]}\n");e=Create("\pGetCTableProbe.json",0,'pPrb','TEXT');if(e)return 7;if(FSOpen("\pGetCTableProbe.json",0,&file))return 8;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?9:0;
}
