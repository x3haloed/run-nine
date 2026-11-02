#include <Multiverse.h>
#include <stdio.h>
static char output[1048576],*p=output;
#pragma pack(push,2)
typedef struct{unsigned long type,d0;short index;unsigned short input,ccr;unsigned long result;long delta;unsigned long guard;}ABI;
#pragma pack(pop)
#define STR_(x) #x
#define STR(x) STR_(x)
#define CALL(OP) __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l #0x7b7b7b7b,-(%%sp)\n\tmove.l (%%a2),-(%%sp)\n\tmove.w 8(%%a2),-(%%sp)\n\tmove.l 4(%%a2),%%d0\n\tmove.w 10(%%a2),%%ccr\n\t.word " STR(OP) "\n\t.word 0x42c3\n\tmove.l %%d0,4(%%a2)\n\tmove.w %%d3,12(%%a2)\n\tmove.l (%%sp)+,14(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,18(%%a2)\n\tmove.l (%%sp)+,22(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory")
static void invoke(short op,ABI*c){register ABI*args __asm__("a2")=c;if(op){CALL(0xa80e);}else{CALL(0xa99d);}}
static short make(ConstStr255Param name,short id){short f;Handle h;CreateResFile(name);if(ResError())return -1;f=OpenResFile(name);if(f==-1)return -1;h=NewHandle(2);if(!h)return -1;(*h)[0]=1;(*h)[1]=2;AddResource(h,'pCnt',7,"\pShared");if(ResError())return -1;h=NewHandle(2);if(!h)return -1;(*h)[0]=3;(*h)[1]=4;AddResource(h,'pCnt',id,"\pUnique");if(ResError())return -1;UpdateResFile(f);if(ResError())return -1;return f;}

int main(void){short app,a=-1,b=-1,phase,op,which,seed,k,index=0,file,flags[]={16,31,0};long n;OSErr e;InitGraf(&qd.thePort);InitFonts();InitWindows();app=CurResFile();
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"ind-resource-raw-v1\",\"records\":[");
 for(phase=0;phase<9;phase++){
  if(phase==1){a=make("\pIndResource-A",23);if(a==-1)return 2;}
  if(phase==2){b=make("\pIndResource-B",31);if(b==-1)return 3;}
  if(phase==3)UseResFile(app);if(phase==4)UseResFile(a);if(phase==5)UseResFile(0);if(phase==6)UseResFile(b);if(phase==7)CloseResFile(a);if(phase==8)CloseResFile(b);
  for(op=0;op<2;op++)for(which=0;which<11;which++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
   ABI c;short before=CurResFile(),after,mem,res,home=-1,id=0,again=0;long size=0;ResType type=0;Str255 name;Handle h,h2;long j;
   c.type=which==10?'pNil':'pCnt';c.index=which==10?1:which-1;c.d0=seed?0:0x12345678;c.input=flags[k];*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(op,&c);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;after=CurResFile();h=(Handle)c.result;name[0]=0;
   if(h){GetResInfo(h,&id,&type,name);home=HomeResFile(h);home=home==app?1:home==a?2:home==b?3:home==0?0:4;size=GetHandleSize(h);h2=op?Get1IndResource(c.type,c.index):GetIndResource(c.type,c.index);again=h==h2;}
   p+=sprintf(p,"%s{\"phase\":%d,\"op\":%d,\"type\":%lu,\"index\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"result_handle\":%lu,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"mem_error\":%d,\"res_error\":%d,\"current_preserved\":%d,\"home\":%d,\"id\":%d,\"info_type\":%lu,\"same_handle\":%d,\"size\":%ld,\"name_hex\":\"",index++?",":"",phase,op,c.type,c.index,seed?0:0x12345678,flags[k],c.result,c.d0,c.ccr&31,c.delta,c.guard,mem,res,before==after,home,id,type,again,size);
   for(j=0;j<name[0];j++)p+=sprintf(p,"%02x",name[j+1]);p+=sprintf(p,"\",\"data_hex\":\"");for(j=0;j<size;j++)p+=sprintf(p,"%02x",(unsigned char)(*h)[j]);p+=sprintf(p,"\"}");
  }
 }
 p+=sprintf(p,"]}\n");e=Create("\pIndResourceProbe.json",0,'iPrb','TEXT');if(e)return 4;if(FSOpen("\pIndResourceProbe.json",0,&file))return 5;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?6:0;
}
