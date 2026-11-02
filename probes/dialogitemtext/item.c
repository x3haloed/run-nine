#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[524288],*p=output;
#pragma pack(push,2)
typedef struct{unsigned long d0,handle,text;unsigned short input,ccr;long delta;unsigned long guard;}ABI;
typedef struct{unsigned long prefix;unsigned char bytes[600];unsigned long suffix;}Buffer;
#pragma pack(pop)
static void invoke(ABI*c){register ABI*args __asm__("a2")=c;__asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l 4(%%a2),-(%%sp)\n\tmove.l 8(%%a2),-(%%sp)\n\tmove.l (%%a2),%%d0\n\tmove.w 12(%%a2),%%ccr\n\t.word 0xa990\n\t.word 0x42c3\n\tmove.l %%d0,(%%a2)\n\tmove.w %%d3,14(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,16(%%a2)\n\tmove.l %%a3,%%sp\n\tmove.l (%%sp)+,20(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory");}
static void hex(const unsigned char*b,short n){short i;p+=sprintf(p,"\"");for(i=0;i<n;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
int main(void){short which,state,seed,k,i,index=0,file,flags[]={16,31,0},lengths[]={0,1,8,64,127,255,256,300};long n;OSErr e;GrafPort port;
 InitGraf(&qd.thePort);InitFonts();OpenPort(&port);SetPort(&port);p+=sprintf(p,"{\"schema\":1,\"probe\":\"dialog-item-text-raw-v1\",\"records\":[");
 for(which=0;which<8;which++)for(state=0;state<2;state++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
  ABI c;Buffer b;short mem,res;unsigned char before_state,after_state;Handle h=NewHandle(lengths[which]);if(!h)return 2;
  for(i=0;i<lengths[which];i++)(*h)[i]=(unsigned char)(i*37+11);if(state)HLock(h);before_state=HGetState(h);
  b.prefix=0xa1b2c3d4;b.suffix=0x10203040;memset(b.bytes,0xa5,sizeof(b.bytes));c.d0=seed?0:0x12345678;c.handle=(unsigned long)h;c.text=(unsigned long)b.bytes;c.input=flags[k];
  *(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(&c);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;after_state=HGetState(h);
  p+=sprintf(p,"%s{\"which\":%d,\"state\":%d,\"length\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"mem_error\":%d,\"res_error\":%d,\"prefix\":%lu,\"suffix\":%lu,\"before_state\":%u,\"after_state\":%u,\"after_hex\":",index++?",":"",which,state,lengths[which],seed?0:0x12345678,flags[k],c.d0,c.ccr&31,c.delta,c.guard,mem,res,b.prefix,b.suffix,before_state,after_state);hex(b.bytes,600);p+=sprintf(p,"}");HUnlock(h);DisposeHandle(h);
 }
 p+=sprintf(p,"]}\n");e=Create("\pDialogItemTextProbe.json",0,'iPrb','TEXT');if(e)return 3;if(FSOpen("\pDialogItemTextProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
