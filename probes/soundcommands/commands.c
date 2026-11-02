#include <Multiverse.h>
#include <stdio.h>
#include <string.h>

/* Observe actual trap stack/CCR and callbacks, not compiler return conventions. */
#define RESULTS "moveq #0,%%d1\n\tmove.w -6(%%a4),%%d1\n\text.l %%d1\n\tmove.l %%d1,(%%a3)+\n\t" \
 "and.l #31,%%d2\n\tmove.l %%d2,(%%a3)+\n\tmove.l %%sp,%%d1\n\tsub.l %%a4,%%d1\n\tmove.l %%d1,(%%a3)+\n\t" \
 "move.l -4(%%a4),(%%a3)+\n\tmove.l %%a4,%%sp"
static void immediate(SndChannelPtr ch,SndCommand *cmd,unsigned long *out){
 register SndChannelPtr channel __asm__("a2")=ch;register unsigned long *result __asm__("a3")=out;register SndCommand *command __asm__("d5")=cmd;
 __asm__ volatile("move.l %%sp,%%a4\n\tlea -14(%%sp),%%sp\n\tmove.l %%d5,(%%sp)\n\tmove.l %%a2,4(%%sp)\n\tmove.w #0x5555,8(%%sp)\n\tmove.l #0x66667777,10(%%sp)\n\tmove.w #31,%%ccr\n\t.word 0xa804\n\t.word 0x42c2\n\t" RESULTS
 :"+a"(channel),"+a"(result):"d"(command):"d0","d1","d2","a0","a1","a4","cc","memory");
}
static void queued(SndChannelPtr ch,SndCommand *cmd,unsigned long *out){
 register SndChannelPtr channel __asm__("a2")=ch;register unsigned long *result __asm__("a3")=out;register SndCommand *command __asm__("d5")=cmd;
 __asm__ volatile("move.l %%sp,%%a4\n\tlea -16(%%sp),%%sp\n\tclr.w (%%sp)\n\tmove.l %%d5,2(%%sp)\n\tmove.l %%a2,6(%%sp)\n\tmove.w #0x5555,10(%%sp)\n\tmove.l #0x66667777,12(%%sp)\n\tmove.w #31,%%ccr\n\t.word 0xa803\n\t.word 0x42c2\n\t" RESULTS
 :"+a"(channel),"+a"(result):"d"(command):"d0","d1","d2","a0","a1","a4","cc","memory");
}
static pascal void callback(SndChannelPtr channel,SndCommand *command){
 volatile unsigned long *record=(volatile unsigned long*)command->param2;
 record[0]++;record[1]=command->cmd;record[2]=command->param1;record[3]=TickCount();record[4]=(unsigned long)channel;
}
static void line(short f,const char *s){long n=0;while(s[n])n++;FSWrite(f,&n,s);FlushVol(0,0);}
static void row(short f,const char *name,SndCommand *command,unsigned long *r,unsigned long callbacks){
 char text[1024];sprintf(text,"{\"case\":\"%s\",\"command\":%u,\"param1\":%d,\"error\":%ld,\"ccr\":%lu,\"stack_delta\":%ld,\"guard\":%lu,\"callbacks\":%lu}\n",name,(unsigned short)command->cmd,command->param1,r[0],r[1],r[2],r[3],callbacks);line(f,text);
}
int main(void){
 short f,i;OSErr error,playing_error;SndChannelPtr channel=0;SndCommand command;unsigned long r[4],buffer_result[4],callback_result[4],start,discard,buffer_ticks,callback_ticks,buffer_count,queued_count,end_ticks;volatile unsigned long record[5]={0,0,0,0,0};unsigned char *buffer;char text[2048];
 struct{unsigned long before,volume,after;} volume={0x11223344,0x55555555,0x66778899};SCStatus status,playing_status;
 short codes[]={0,46,47,13,0x7fff,3,4};char *names[]={"null","volume_half_right","get_volume","immediate_callback","unknown","quiet","flush"};
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();TEInit();InitDialogs(0);
 if(Create("\pSoundCommandsProbe.jsonl",0,'aPrb','TEXT')||FSOpen("\pSoundCommandsProbe.jsonl",0,&f))return 2;
 error=SndNewChannel(&channel,5,0x80,NewSndCallbackUPP(callback));
 sprintf(text,"{\"probe\":\"sound-commands-v3\",\"new_channel_error\":%d,\"channel_nonnull\":%d}\n",error,channel!=0);line(f,text);if(error||!channel){FSClose(f);return 3;}
 sprintf(text,"{\"phase\":\"allocated_record\",\"callback_matches\":%d,\"user_info\":%ld,\"wait\":%ld,\"flags\":%d,\"queue_length\":%d,\"queue_head\":%d,\"queue_tail\":%d}\n",channel->callBack==NewSndCallbackUPP(callback),channel->userInfo,channel->wait,channel->flags,channel->qLength,channel->qHead,channel->qTail);line(f,text);
 for(i=0;i<7;i++){
  command.cmd=codes[i];command.param1=i==3?123:0;command.param2=i==1?0x00800100:i==2?(long)&volume.volume:i==3?(long)record:0;
  immediate(channel,&command,r);row(f,names[i],&command,r,record[0]);
 }
 sprintf(text,"{\"phase\":\"volume_result\",\"before\":%lu,\"volume\":%lu,\"after\":%lu,\"callback_command\":%lu,\"callback_param1\":%lu,\"callback_channel_matches\":%d}\n",volume.before,volume.volume,volume.after,record[1],record[2],record[4]==(unsigned long)channel);line(f,text);
 buffer=(unsigned char*)NewPtrClear(8022);if(!buffer)return 4;*(unsigned long*)(buffer+4)=8000;*(unsigned long*)(buffer+8)=0x1f400000;buffer[21]=60;
 for(i=0;i<8000;i++)buffer[22+i]=(i/32)&1?160:96;
 /* No filesystem calls or formatting in the measured queue/timing interval. */
 start=TickCount();command.cmd=81;command.param1=0;command.param2=(long)buffer;queued(channel,&command,buffer_result);buffer_ticks=TickCount()-start;buffer_count=record[0];
 memset(&playing_status,0xa5,sizeof(playing_status));playing_error=SndChannelStatus(channel,sizeof(playing_status),&playing_status);
 command.cmd=13;command.param1=456;command.param2=(long)record;queued(channel,&command,callback_result);callback_ticks=TickCount()-start;queued_count=record[0];
 while(record[0]<2&&TickCount()-start<120)Delay(1,&discard);
 memset(&status,0xa5,sizeof(status));error=SndChannelStatus(channel,sizeof(status),&status);end_ticks=TickCount()-start;
 command.cmd=81;command.param1=0;row(f,"queued_buffer",&command,buffer_result,buffer_count);command.cmd=13;command.param1=456;row(f,"queued_callback",&command,callback_result,queued_count);
 sprintf(text,"{\"phase\":\"playing_status\",\"error\":%d,\"busy\":%d,\"disposed\":%d,\"paused\":%d,\"attributes\":%lu,\"buffer_return_ticks\":%lu,\"callback_return_ticks\":%lu}\n",playing_error,playing_status.scChannelBusy,playing_status.scChannelDisposed,playing_status.scChannelPaused,playing_status.scChannelAttributes,buffer_ticks,callback_ticks);line(f,text);
 sprintf(text,"{\"phase\":\"completed_status\",\"error\":%d,\"busy\":%d,\"disposed\":%d,\"paused\":%d,\"attributes\":%lu,\"callbacks\":%lu,\"callback_command\":%lu,\"callback_param1\":%lu,\"callback_channel_matches\":%d,\"callback_elapsed_ticks\":%lu,\"elapsed_ticks\":%lu}\n",error,status.scChannelBusy,status.scChannelDisposed,status.scChannelPaused,status.scChannelAttributes,record[0],record[1],record[2],record[4]==(unsigned long)channel,record[3]-start,end_ticks);line(f,text);
 sprintf(text,"{\"phase\":\"status_times\",\"playing\":[%ld,%ld,%ld],\"completed\":[%ld,%ld,%ld]}\n",playing_status.scStartTime,playing_status.scEndTime,playing_status.scCurrentTime,status.scStartTime,status.scEndTime,status.scCurrentTime);line(f,text);
 error=SndDisposeChannel(channel,1);DisposePtr((Ptr)buffer);sprintf(text,"{\"complete\":true,\"dispose_error\":%d}\n",error);line(f,text);FSClose(f);FlushVol(0,0);return 0;
}
