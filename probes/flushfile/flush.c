#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[262144],*p=output;
#pragma pack(push,2)
typedef struct{void*pb;unsigned long d0;unsigned short input,ccr;long delta;unsigned long guard;void*after;}ABI;
#pragma pack(pop)
static void invoke(ABI*c){register ABI*args __asm__("a2")=c;
 __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l (%%a2),%%a0\n\tmove.l 4(%%a2),%%d0\n\tmove.w 8(%%a2),%%ccr\n\t.word 0xa045\n\t.word 0x42c3\n\tmove.l %%d0,4(%%a2)\n\tmove.w %%d3,10(%%a2)\n\tmove.l %%a0,20(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,12(%%a2)\n\tmove.l (%%sp)+,16(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory");
}
static void hex(const char*key,const unsigned char*b,long n){static const char d[]="0123456789abcdef";long i;p+=sprintf(p,",\"%s\":\"",key);for(i=0;i<n;i++){*p++=d[b[i]>>4];*p++=d[b[i]&15];}*p++='"';*p=0;}
static short make(ConstStr255Param name,short resource,short readonly,short empty){FSSpec s;short f;long n=8;OSErr e=FSMakeFSSpec(0,0,name,&s);if(e&&e!=fnfErr)return -1;if(FSpCreate(&s,'pPrb','TEXT',smSystemScript))return -1;e=resource?FSpOpenRF(&s,fsRdWrPerm,&f):FSpOpenDF(&s,fsRdWrPerm,&f);if(e)return -1;if(!empty&&FSWrite(f,&n,"ABCDEFGH"))return -1;if(readonly){FSClose(f);e=resource?FSpOpenRF(&s,fsRdPerm,&f):FSpOpenDF(&s,fsRdPerm,&f);if(e)return -1;}if(!empty&&SetFPos(f,fsFromStart,resource?4:3))return -1;return f;}
int main(void){short refs[9],kind,volume,seed,k,index=0,file,flags[]={16,31,0};long n;OSErr e;InitGraf(&qd.thePort);InitFonts();InitWindows();refs[0]=make("\pFlushDFWrite",0,0,0);refs[1]=make("\pFlushDFRead",0,1,0);refs[2]=make("\pFlushRFWrite",1,0,0);refs[3]=make("\pFlushRFRead",1,1,0);refs[4]=make("\pFlushEmpty",0,0,1);refs[5]=make("\pFlushClosed",0,0,0);for(kind=0;kind<6;kind++)if(refs[kind]==-1)return 2;FSClose(refs[5]);refs[6]=0;refs[7]=-1;refs[8]=12345;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"flush-file-raw-v1\",\"records\":[");
 for(kind=0;kind<9;kind++)for(volume=0;volume<2;volume++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
  unsigned char bytes[96],before[96];IOParam*pb=(IOParam*)bytes;ABI c;long mark_before=0x12345678,mark_after=0x12345678,eof_before=0x55667788,eof_after=0x55667788;short mb,ma,eb,ea,mem,res;
  mb=GetFPos(refs[kind],&mark_before);eb=GetEOF(refs[kind],&eof_before);memset(bytes,0xa5,96);pb->ioCompletion=0;pb->ioNamePtr=0;pb->ioVRefNum=volume?0x1357:0;pb->ioRefNum=refs[kind];pb->ioResult=0x7b7b;memcpy(before,bytes,96);c.pb=pb;c.d0=seed?0:0x12345678;c.input=flags[k];*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(&c);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;ma=GetFPos(refs[kind],&mark_after);ea=GetEOF(refs[kind],&eof_after);
  p+=sprintf(p,"%s{\"kind\":%d,\"volume_input\":%d,\"reference\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"a0_preserved\":%d,\"mem_error\":%d,\"res_error\":%d,\"mark_before_error\":%d,\"mark_after_error\":%d,\"mark_before\":%ld,\"mark_after\":%ld,\"eof_before_error\":%d,\"eof_after_error\":%d,\"eof_before\":%ld,\"eof_after\":%ld",index++?",":"",kind,volume,refs[kind],seed?0:0x12345678,flags[k],c.d0,c.ccr&31,c.delta,c.guard,c.after==c.pb,mem,res,mb,ma,mark_before,mark_after,eb,ea,eof_before,eof_after);hex("before_hex",before,96);hex("after_hex",bytes,96);p+=sprintf(p,"}");
 }
 for(kind=0;kind<5;kind++)FSClose(refs[kind]);p+=sprintf(p,"]}\n");e=Create("\pFlushFileProbe.json",0,'pPrb','TEXT');if(e)return 3;if(FSOpen("\pFlushFileProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
