#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
#include "callback.h"
#pragma pack(push,2)
typedef struct {TMTask task;volatile long calls;volatile unsigned long entry_a0;volatile unsigned long entry_a1;volatile short callback_type;volatile long callback_count;long rearm;long limit;unsigned long guard;volatile unsigned long ticks[3];} OwnedTask;
#pragma pack(pop)
static char output[24576],*p=output;
static unsigned long call(short operation,TMTask *task,long value,short flags,unsigned short *out_flags,short *same) {
 register unsigned long d0 __asm__("d0")=value;
 register TMTask *a0 __asm__("a0")=task;
 register unsigned short captured __asm__("d3");register unsigned short input __asm__("d4")=flags;
 switch(operation) {
 case 0:__asm__ volatile("move.w %%d4,%%ccr\n\t.word 0xa058\n\t.word 0x42c3" : "+d"(d0),"+a"(a0),"=d"(captured):"d"(input):"d1","d2","a1","cc","memory");break;
 case 1:__asm__ volatile("move.w %%d4,%%ccr\n\t.word 0xa458\n\t.word 0x42c3" : "+d"(d0),"+a"(a0),"=d"(captured):"d"(input):"d1","d2","a1","cc","memory");break;
 case 2:__asm__ volatile("move.w %%d4,%%ccr\n\t.word 0xa05a\n\t.word 0x42c3" : "+d"(d0),"+a"(a0),"=d"(captured):"d"(input):"d1","d2","a1","cc","memory");break;
 default:__asm__ volatile("move.w %%d4,%%ccr\n\t.word 0xa059\n\t.word 0x42c3" : "+d"(d0),"+a"(a0),"=d"(captured):"d"(input):"d1","d2","a1","cc","memory");break;
 }
 *out_flags=captured&31;*same=a0==task;return d0;
}
static void row(short i,short mode,short step,short flags,unsigned long d0,unsigned short ccr,short same,OwnedTask *task,unsigned long before) {
 p+=sprintf(p,"%s{\"mode\":%d,\"step\":%d,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"a0_same\":%d,\"raw_task\":%lu,\"raw_link\":%lu,\"type\":%u,\"count\":%ld,\"wake\":%ld,\"reserved\":%ld,\"proc_same\":%d,\"guards\":[%lu,%lu]}",i?",":"",mode,step,flags,d0,ccr,same,(unsigned long)&task->task,(unsigned long)task->task.qLink,(unsigned short)task->task.qType,task->task.tmCount,task->task.tmWakeUp,task->task.tmReserved,task->task.tmAddr==0,before,task->guard);
}
int main(void) {
 struct {unsigned long guard;OwnedTask task;} state;short mode,j,step,index=0,same,kind,file;unsigned short ccr;unsigned long d0;short flags[3]={16,31,0};long ticks,n;unsigned long before,after;OSErr e;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"time-manager-raw-v2\",\"records\":[");
 for(mode=0;mode<2;mode++)for(j=0;j<3;j++) {
  memset(&state,0,sizeof(state));state.guard=0x11223344;state.task.guard=0x55667788;
  /* Only extended fields belong to InsXTime. Canary the bytes beyond the
     original14-byte record when measuring InsTime. */
  if(!mode){state.task.task.tmWakeUp=0x55555555;state.task.task.tmReserved=0x66666666;}
  for(step=0;step<3;step++) {
   d0=call(step==0?mode:step==1?2:3,&state.task.task,step==1?10000:0x12345678,flags[j],&ccr,&same);
   row(index++,mode,step,flags[j],d0,ccr,same,&state.task,state.guard);
  }
 }
 p+=sprintf(p,"],\"callbacks\":[");index=0;
 for(mode=0;mode<2;mode++)for(kind=0;kind<3;kind++) {
  memset(&state,0,sizeof(state));state.guard=0x11223344;state.task.guard=0x55667788;
  state.task.task.tmAddr=(ProcPtr)timer_callback;state.task.rearm=30;state.task.limit=kind==2?3:1;
  d0=call(mode,&state.task.task,0x12345678,16,&ccr,&same);
  before=TickCount();d0=call(2,&state.task.task,kind==1?-30000:30,16,&ccr,&same);
  Delay(30,&ticks);after=TickCount();
  p+=sprintf(p,"%s{\"mode\":%d,\"kind\":%d,\"calls\":%ld,\"self_same\":%d,\"entry_a0_proc\":%d,\"callback_type\":%u,\"callback_count\":%ld,\"after_type\":%u,\"after_count\":%ld,\"raw_wake\":%ld,\"raw_reserved\":%ld,\"ticks\":[%lu,%lu,%lu,%lu,%lu],\"guards\":[%lu,%lu]}",index++?",":"",mode,kind,state.task.calls,state.task.entry_a1==(unsigned long)&state.task.task,state.task.entry_a0==(unsigned long)timer_callback,(unsigned short)state.task.callback_type,state.task.callback_count,(unsigned short)state.task.task.qType,state.task.task.tmCount,state.task.task.tmWakeUp,state.task.task.tmReserved,before,state.task.ticks[0],state.task.ticks[1],state.task.ticks[2],after,state.guard,state.task.guard);
  RmvTime((QElemPtr)&state.task.task);
 }
 p+=sprintf(p,"],\"edges\":[");index=0;
 for(mode=0;mode<2;mode++)for(kind=0;kind<4;kind++) {
  long requests[4]={2147483647L,86400000L,-2147483647L,2147483L};short type;long count,remaining;
  memset(&state,0,sizeof(state));state.guard=0x11223344;state.task.guard=0x55667788;
  call(mode,&state.task.task,0x12345678,16,&ccr,&same);
  call(2,&state.task.task,requests[kind],16,&ccr,&same);type=state.task.task.qType;count=state.task.task.tmCount;
  call(3,&state.task.task,0x12345678,16,&ccr,&same);remaining=state.task.task.tmCount;
  p+=sprintf(p,"%s{\"mode\":%d,\"request\":%ld,\"prime_type\":%u,\"prime_count\":%ld,\"remaining\":%ld,\"after_type\":%u,\"guards\":[%lu,%lu]}",index++?",":"",mode,requests[kind],(unsigned short)type,count,remaining,(unsigned short)state.task.task.qType,state.guard,state.task.guard);
 }
 p+=sprintf(p,"]}\n");e=Create("\pTimeManagerProbe.json",0,'pPrb','TEXT');if(e)return 2;if(FSOpen("\pTimeManagerProbe.json",0,&file))return 3;
 n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;
}
