#include <Multiverse.h>
#include <stdio.h>
static char output[262144],*p=output;
#pragma pack(push,2)
typedef struct{unsigned long type,d0;unsigned short input,ccr;short result;long delta;unsigned long guard;}ABI;
#pragma pack(pop)
#define STR_(x) #x
#define STR(x) STR_(x)
#define CALL(OP) __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tsubq.l #2,%%sp\n\tmove.w #0x7b7b,(%%sp)\n\tmove.l (%%a2),-(%%sp)\n\tmove.l 4(%%a2),%%d0\n\tmove.w 8(%%a2),%%ccr\n\t.word " STR(OP) "\n\t.word 0x42c3\n\tmove.l %%d0,4(%%a2)\n\tmove.w %%d3,10(%%a2)\n\tmove.w (%%sp)+,12(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,14(%%a2)\n\tmove.l (%%sp)+,18(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory")
static void invoke(short op,ABI*c){register ABI*args __asm__("a2")=c;if(op){CALL(0xa80d);}else{CALL(0xa99c);}}
static short make(ConstStr255Param name,short id){short f;Handle h;CreateResFile(name);if(ResError())return -1;f=OpenResFile(name);if(f==-1)return -1;h=NewHandle(2);if(!h)return -1;(*h)[0]=1;(*h)[1]=2;AddResource(h,'pCnt',7,"\pShared");if(ResError())return -1;h=NewHandle(2);if(!h)return -1;(*h)[0]=3;(*h)[1]=4;AddResource(h,'pCnt',id,"\pUnique");if(ResError())return -1;UpdateResFile(f);if(ResError())return -1;return f;}
int main(void){short app,a=-1,b=-1,phase,op,type,seed,k,index=0,file,flags[]={16,31,0};long n;OSErr e;InitGraf(&qd.thePort);InitFonts();InitWindows();app=CurResFile();
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"count-resources-raw-v1\",\"records\":[");
 for(phase=0;phase<9;phase++){
  if(phase==1){a=make("\pCountResources-A",23);if(a==-1)return 2;}
  if(phase==2){b=make("\pCountResources-B",31);if(b==-1)return 3;}
  if(phase==3)UseResFile(app);if(phase==4)UseResFile(a);if(phase==5)UseResFile(0);if(phase==6)UseResFile(b);if(phase==7)CloseResFile(a);if(phase==8)CloseResFile(b);
  for(op=0;op<2;op++)for(type=0;type<2;type++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
   ABI c;short before=CurResFile(),after,mem,res;c.type=type?'pNil':'pCnt';c.d0=seed?0:0x12345678;c.input=flags[k];*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(op,&c);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;after=CurResFile();
   p+=sprintf(p,"%s{\"phase\":%d,\"op\":%d,\"type\":%lu,\"input_d0\":%lu,\"input_ccr\":%d,\"result\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"mem_error\":%d,\"res_error\":%d,\"current_preserved\":%d}",index++?",":"",phase,op,c.type,seed?0:0x12345678,flags[k],c.result,c.d0,c.ccr&31,c.delta,c.guard,mem,res,before==after);
  }
 }
 p+=sprintf(p,"]}\n");e=Create("\pCountResourcesProbe.json",0,'pPrb','TEXT');if(e)return 4;if(FSOpen("\pCountResourcesProbe.json",0,&file))return 5;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?6:0;
}
