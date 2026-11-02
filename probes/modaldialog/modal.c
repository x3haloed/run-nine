#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[262144],*p=output;
static DialogPtr expected;static short mode,calls,observations[32][8];static short *caller_item;
static pascal Boolean filter(DialogPtr d,EventRecord *e,short *item){short n=calls++,written,handled;GrafPtr port;GetPort(&port);if(mode==2&&n==0){written=5;handled=0;}else if(mode==6&&n<3){written=5;handled=0;}else{handled=1;written=mode==0?1:mode==1?2:mode==2?3:mode==3?(n?2:0):mode==4?(n?2:-1):mode==5?(n?2:32767):mode==6?1:5;}
 if(n>4){written=1;handled=1;}if(n<32){observations[n][0]=d==expected;observations[n][1]=port==(GrafPtr)d;observations[n][2]=item==caller_item;observations[n][3]=*item;observations[n][4]=written;observations[n][5]=handled;observations[n][6]=e->what;observations[n][7]=e!=0;}
 *item=written;return handled;}
#pragma pack(push,2)
typedef struct{unsigned long d0,filter,out;unsigned short input,ccr;long delta;unsigned long guard;}ABI;
typedef struct{unsigned long prefix;short item;unsigned long suffix;}Output;
#pragma pack(pop)
static void invoke(ABI*c){register ABI*args __asm__("a2")=c;__asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l 4(%%a2),-(%%sp)\n\tmove.l 8(%%a2),-(%%sp)\n\tmove.l (%%a2),%%d0\n\tmove.w 12(%%a2),%%ccr\n\t.word 0xa991\n\t.word 0x42c3\n\tmove.l %%d0,(%%a2)\n\tmove.w %%d3,14(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,16(%%a2)\n\tmove.l (%%sp)+,20(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory");}
int main(void){short seed,k,index=0,file,flags[]={16,31,0};long n;OSErr error;GrafPtr original,after;EventRecord e;ModalFilterUPP proc;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();TEInit();InitDialogs(0);GetPort(&original);proc=NewModalFilterUPP(filter);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"modal-dialog-raw-v1\",\"records\":[");
 for(mode=0;mode<8;mode++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
  ABI c;Output out;short mem,res,j;expected=GetNewDialog(128,0,(WindowPtr)-1);if(!expected)return 2;
  SetPort(expected);BeginUpdate((WindowPtr)expected);DrawDialog(expected);EndUpdate((WindowPtr)expected);while(GetNextEvent(everyEvent,&e)){}SetPort(original);
  calls=0;memset(observations,0,sizeof(observations));out.prefix=0xa1b2c3d4;out.suffix=0x10203040;out.item=-12345;caller_item=&out.item;c.d0=seed?0:0x12345678;c.filter=(unsigned long)proc;c.out=(unsigned long)&out.item;c.input=flags[k];*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(&c);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;GetPort(&after);
  p+=sprintf(p,"%s{\"mode\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"mem_error\":%d,\"res_error\":%d,\"port_preserved\":%d,\"item\":%d,\"prefix\":%lu,\"suffix\":%lu,\"calls\":[",index++?",":"",mode,seed?0:0x12345678,flags[k],c.d0,c.ccr&31,c.delta,c.guard,mem,res,after==original,out.item,out.prefix,out.suffix);
  for(j=0;j<calls&&j<32;j++)p+=sprintf(p,"%s[%d,%d,%d,%d,%d,%d,%d,%d]",j?",":"",observations[j][0],observations[j][1],observations[j][2],observations[j][3],observations[j][4],observations[j][5],observations[j][6],observations[j][7]);p+=sprintf(p,"]}");DisposeDialog(expected);SetPort(original);
 }
 p+=sprintf(p,"]}\n");error=Create("\pModalDialogProbe.json",0,'mPrb','TEXT');if(error)return 3;if(FSOpen("\pModalDialogProbe.json",0,&file))return 4;n=p-output;error=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return error?5:0;
}
