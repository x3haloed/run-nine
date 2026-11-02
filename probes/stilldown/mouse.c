#include <Multiverse.h>
#include <stdio.h>
static char output[262144],*p=output;
static short post_event(short what,unsigned long message){
 register unsigned long d0 __asm__("d0")=message;
 register unsigned long a0 __asm__("a0")=(unsigned short)what;
 __asm__ volatile(".word 0xa02f":"+d"(d0),"+a"(a0)::"d1","d2","a1","cc","memory");
 return (short)d0;
}
#pragma pack(push,2)
typedef struct{unsigned long d0;unsigned short input,ccr,result;long delta;unsigned long guard;}ABI;
#pragma pack(pop)
#define STR_(x) #x
#define STR(x) STR_(x)
#define CALL(OP) __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tsubq.l #2,%%sp\n\tmove.w #0x7b7b,(%%sp)\n\tmove.l (%%a2),%%d0\n\tmove.w 4(%%a2),%%ccr\n\t.word " STR(OP) "\n\t.word 0x42c3\n\tmove.l %%d0,(%%a2)\n\tmove.w %%d3,6(%%a2)\n\tmove.w (%%sp)+,8(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,10(%%a2)\n\tmove.l (%%sp)+,14(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory")
static void invoke(short op,ABI*c){register ABI*args __asm__("a2")=c;if(op){CALL(0xa974);}else{CALL(0xa973);}}
int main(void){Rect bounds={80,80,260,600};WindowPtr w;short phase,op,which,seed,k,index=0,file,flags[]={16,31,0};short lengths[]={0,1,1,1,2,2,2,3},inputs[8][3]={{0},{3},{2},{1},{3,2},{2,3},{1,3},{1,2,3}};long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();InitCursor();w=NewWindow(0,&bounds,"\pStillDown: hold mouse button for samples",true,documentProc,(WindowPtr)-1,false,0);if(!w)return 2;SetPort(w);MoveTo(15,30);DrawString("\pHold the mouse button in this window for 10 seconds.");
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"still-down-raw-v1\",\"records\":[");
 for(phase=0;phase<2;phase++){
  if(phase){FlushEvents(everyEvent,0);while(!Button())SystemTask();}
  for(op=0;op<2;op++)for(which=0;which<8;which++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
   ABI c;short j,before,after,mem,res,count=0,post[3],remaining[20];EventRecord event;
   FlushEvents(everyEvent,0);for(j=0;j<lengths[which];j++)post[j]=post_event(inputs[which][j],0xa000L+j);
   before=Button();c.d0=seed?0:0x12345678;c.input=flags[k];*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(op,&c);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;after=Button();
   while(count<20&&GetOSEvent(everyEvent,&event)){remaining[count++]=event.what;}
   p+=sprintf(p,"%s{\"phase\":%d,\"op\":%d,\"queue\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"button_before\":%d,\"button_after\":%d,\"result_word\":%u,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"mem_error\":%d,\"res_error\":%d,\"post_errors\":[",index++?",":"",phase,op,which,seed?0:0x12345678,flags[k],before,after,c.result,c.d0,c.ccr&31,c.delta,c.guard,mem,res);
   for(j=0;j<lengths[which];j++)p+=sprintf(p,"%s%d",j?",":"",post[j]);p+=sprintf(p,"],\"remaining_types\":[");for(j=0;j<count;j++)p+=sprintf(p,"%s%d",j?",":"",remaining[j]);p+=sprintf(p,"]}");
  }
 }
 p+=sprintf(p,"]}\n");e=Create("\pStillDownProbe.json",0,'pPrb','TEXT');if(e)return 3;if(FSOpen("\pStillDownProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);DisposeWindow(w);return e?5:0;
}
