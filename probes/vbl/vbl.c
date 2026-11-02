#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
#include "callback.h"
#pragma pack(push,2)
typedef struct {VBLTask task;volatile long calls;volatile unsigned long entry;volatile short type,count,phase;long limit;short rearm;unsigned long guard;volatile unsigned long ticks[3];volatile unsigned long pulse,pulse_samples[3];} OwnedTask;
#pragma pack(pop)
typedef struct {unsigned long d0;unsigned short ccr;short same;long delta;} Result;
static char output[131072],*p=output;
static Result call(short op,VBLTask *task,unsigned long value,short flags){
 register unsigned long d0 __asm__("d0")=value;register VBLTask *a0 __asm__("a0")=task;register unsigned short ccr __asm__("d3");register unsigned short input __asm__("d4")=flags;register long delta __asm__("d5");Result r;
 switch(op){
 case 0:__asm__ volatile("move.l %%sp,%%d5\n\tmove.w %%d4,%%ccr\n\t.word 0xa06f\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"+a"(a0),"=d"(ccr),"=d"(delta):"d"(input):"d1","d2","a1","cc","memory");break;
 case 1:__asm__ volatile("move.l %%sp,%%d5\n\tmove.w %%d4,%%ccr\n\t.word 0xa070\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"+a"(a0),"=d"(ccr),"=d"(delta):"d"(input):"d1","d2","a1","cc","memory");break;
 case 2:__asm__ volatile("move.l %%sp,%%d5\n\tmove.w %%d4,%%ccr\n\t.word 0xa033\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"+a"(a0),"=d"(ccr),"=d"(delta):"d"(input):"d1","d2","a1","cc","memory");break;
 case 3:__asm__ volatile("move.l %%sp,%%d5\n\tmove.w %%d4,%%ccr\n\t.word 0xa034\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"+a"(a0),"=d"(ccr),"=d"(delta):"d"(input):"d1","d2","a1","cc","memory");break;
 default:__asm__ volatile("move.l %%sp,%%d5\n\tmove.w %%d4,%%ccr\n\t.word 0xa072\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"+a"(a0),"=d"(ccr),"=d"(delta):"d"(input):"d1","d2","a1","cc","memory");break;
 }
 r.d0=d0;r.ccr=ccr&31;r.same=a0==task;r.delta=delta;return r;
}
int main(void){struct {unsigned long guard;OwnedTask task;} state;short main_slot=-1,main_refnum=0;DCtlHandle dce;GDHandle device;short slots[19]={0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,-1,-2};short flags[3]={16,31,0},i,j,k,step,index=0,kind,file;long n,ticks;Result r,installed,removed;unsigned long input,before,after;OSErr err;
 device=GetMainDevice();if(device){main_refnum=(**device).gdRefNum;dce=main_refnum<0&&(unsigned short)~main_refnum<(unsigned short)LMGetUnitNtryCnt()?LMGetUTableBase()[(unsigned short)~main_refnum]:0;if(dce&&GetHandleSize((Handle)dce)>=42)main_slot=((unsigned char*)*dce)[40];}
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"vbl-raw-v3\",\"main_refnum\":%d,\"main_slot\":%d,\"records\":[",main_refnum,main_slot);
 for(i=0;i<19;i++)for(j=0;j<2;j++)for(k=0;k<3;k++){
  memset(&state,0,sizeof(state));state.guard=0x11223344;state.task.guard=0x55667788;state.task.task.qType=1;state.task.task.vblAddr=(ProcPtr)vbl_callback;state.task.task.vblCount=32000;
  input=(j?0x12340000UL:0)|(unsigned short)(i==18?0x3456:slots[i]);
  for(step=0;step<2;step++){
   r=call((i==18?2:0)+step,&state.task.task,input,flags[k]);
   p+=sprintf(p,"%s{\"system\":%d,\"slot\":%d,\"step\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"a0_same\":%d,\"stack_delta\":%ld,\"raw_task\":%lu,\"raw_link\":%lu,\"type\":%u,\"count\":%d,\"phase\":%d,\"proc_same\":%d,\"guards\":[%lu,%lu]}",index++?",":"",i==18,slots[i],step,input,flags[k],r.d0,r.ccr,r.same,r.delta,(unsigned long)&state.task.task,(unsigned long)state.task.task.qLink,(unsigned short)state.task.task.qType,state.task.task.vblCount,state.task.task.vblPhase,state.task.task.vblAddr==(ProcPtr)vbl_callback,state.guard,state.task.guard);
  }
 }
 p+=sprintf(p,"],\"callbacks\":[");index=0;
 for(i=0;i<3;i++)for(kind=0;kind<6;kind++){
  memset(&state,0,sizeof(state));state.guard=0x11223344;state.task.guard=0x55667788;state.task.task.qType=1;state.task.task.vblAddr=(ProcPtr)vbl_callback;state.task.task.vblCount=kind==3?0:kind==5?-2:3;state.task.task.vblPhase=kind==1||kind==4?2:0;state.task.limit=kind==2||kind==4?3:1;state.task.rearm=3;
  before=TickCount();installed=call(i?0:2,&state.task.task,(unsigned short)(i==2?main_slot:0),16);Delay(30,&ticks);after=TickCount();
  p+=sprintf(p,"%s{\"system\":%d,\"slot\":%d,\"kind\":%d,\"install_d0\":%lu,\"install_result\":%d,\"install_ccr\":%u,\"calls\":%ld,\"self_same\":%d,\"callback_type\":%u,\"callback_count\":%d,\"callback_phase\":%d,\"after_type\":%u,\"after_count\":%d,\"after_phase\":%d,\"ticks\":[%lu,%lu,%lu,%lu,%lu],\"guards\":[%lu,%lu]",index++?",":"",i==0,i==2?main_slot:0,kind,installed.d0,(short)installed.d0,installed.ccr,state.task.calls,state.task.entry==(unsigned long)&state.task.task,(unsigned short)state.task.type,state.task.count,state.task.phase,(unsigned short)state.task.task.qType,state.task.task.vblCount,state.task.task.vblPhase,before,state.task.ticks[0],state.task.ticks[1],state.task.ticks[2],after,state.guard,state.task.guard);
  removed=call(i?1:3,&state.task.task,(unsigned short)(i==2?main_slot:0),16);p+=sprintf(p,",\"remove_d0\":%lu,\"remove_result\":%d,\"remove_ccr\":%u}\n",removed.d0,(short)removed.d0,removed.ccr);
 }
 p+=sprintf(p,"],\"manual\":[");
 for(kind=0;kind<6;kind++){
  unsigned long results[12];unsigned short ccrs[12];short pulse;
  memset(&state,0,sizeof(state));state.guard=0x11223344;state.task.guard=0x55667788;state.task.task.qType=1;state.task.task.vblAddr=(ProcPtr)vbl_callback;state.task.task.vblCount=kind==3?0:kind==5?-2:3;state.task.task.vblPhase=kind==1||kind==4?2:0;state.task.limit=kind==2||kind==4?3:1;state.task.rearm=3;
  installed=call(0,&state.task.task,0,16);before=TickCount();
  for(pulse=0;pulse<12;pulse++){state.task.pulse=pulse+1;r=call(4,&state.task.task,0,16);results[pulse]=r.d0;ccrs[pulse]=r.ccr;}
  after=TickCount();
  p+=sprintf(p,"%s{\"kind\":%d,\"install_d0\":%lu,\"install_ccr\":%u,\"calls\":%ld,\"self_same\":%d,\"callback_type\":%u,\"callback_count\":%d,\"callback_phase\":%d,\"after_type\":%u,\"after_count\":%d,\"after_phase\":%d,\"ticks\":[%lu,%lu,%lu,%lu,%lu],\"pulse_samples\":[%lu,%lu,%lu],\"guards\":[%lu,%lu],\"pulse_d0\":[",kind?",":"",kind,installed.d0,installed.ccr,state.task.calls,state.task.entry==(unsigned long)&state.task.task,(unsigned short)state.task.type,state.task.count,state.task.phase,(unsigned short)state.task.task.qType,state.task.task.vblCount,state.task.task.vblPhase,before,state.task.ticks[0],state.task.ticks[1],state.task.ticks[2],after,state.task.pulse_samples[0],state.task.pulse_samples[1],state.task.pulse_samples[2],state.guard,state.task.guard);
  for(pulse=0;pulse<12;pulse++)p+=sprintf(p,"%s%lu",pulse?",":"",results[pulse]);p+=sprintf(p,"],\"pulse_ccr\":[");for(pulse=0;pulse<12;pulse++)p+=sprintf(p,"%s%u",pulse?",":"",ccrs[pulse]);
  removed=call(1,&state.task.task,0,16);p+=sprintf(p,"],\"remove_d0\":%lu,\"remove_ccr\":%u}",removed.d0,removed.ccr);
 }
 p+=sprintf(p,"]}\n");err=Create("\pVBLProbe.json",0,'pPrb','TEXT');if(err)return 2;if(FSOpen("\pVBLProbe.json",0,&file))return 3;n=p-output;err=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
