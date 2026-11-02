#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static volatile long done;
static pascal void callback(SndChannelPtr ch,SndCommand *cmd){done=1;}
static void line(short f,const char*s){long n=strlen(s);FSWrite(f,&n,(void*)s);FlushVol(0,0);}
static OSErr send(SndChannelPtr ch,short code,short p1,long p2){SndCommand c;c.cmd=code;c.param1=p1;c.param2=p2;return SndDoImmediate(ch,&c);}
int main(void){
 short f,a,i;long old=0,volume,amp,discard;OSErr e,ae,ve;char text[1024];SndChannelPtr ch=0;unsigned char *buffer;SndCommand c;short levels[]={255,127,64,1,0};
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();TEInit();InitDialogs(0);
 if(Create("\pSoundAmplitudeProbe.jsonl",0,'aPrb','TEXT')||FSOpen("\pSoundAmplitudeProbe.jsonl",0,&f))return 2;
 e=SndNewChannel(&ch,5,0x80,NewSndCallbackUPP(callback));sprintf(text,"{\"probe\":\"sound-amplitude-v1\",\"new_error\":%d}\n",e);line(f,text);if(e)return 3;
 for(a=0;a<256;a++){
  e=send(ch,43,a,0);amp=0x55555555;volume=0x55555555;ae=send(ch,45,0,(long)&amp);ve=send(ch,47,0,(long)&volume);
  sprintf(text,"{\"phase\":\"amplitude\",\"value\":%d,\"error\":%d,\"get_amp_error\":%d,\"amp\":%ld,\"get_volume_error\":%d,\"volume\":%lu}\n",a,e,ae,amp,ve,volume);line(f,text);
 }
 {unsigned long volumes[]={0,0x00800100,0x01000100,0x02000200,0x00400080};for(i=0;i<5;i++){
  e=send(ch,46,0,volumes[i]);amp=0x55555555;volume=0x55555555;ae=send(ch,45,0,(long)&amp);ve=send(ch,47,0,(long)&volume);
  sprintf(text,"{\"phase\":\"volume\",\"value\":%lu,\"error\":%d,\"get_amp_error\":%d,\"amp\":%ld,\"get_volume_error\":%d,\"volume\":%lu}\n",volumes[i],e,ae,amp,ve,volume);line(f,text);
 }}
 buffer=(unsigned char*)NewPtrClear(36022);if(!buffer)return 4;
 *(unsigned long*)(buffer+4)=36000;*(unsigned long*)(buffer+8)=0xbb800000;buffer[21]=60;
 /* Use unsigned index: 36000 exceeds a signed short. */
 {unsigned long k;for(k=0;k<36000;k++)buffer[22+k]=(k<12000||k>=24000)?128:((k/32)&1?160:96);}
 GetDefaultOutputVolume(&old);SetDefaultOutputVolume(0x01000100);
 for(i=0;i<5;i++){
  c.cmd=43;c.param1=levels[i];c.param2=0;e=SndDoCommand(ch,&c,0);done=0;c.cmd=81;c.param1=0;c.param2=(long)buffer;ae=SndDoCommand(ch,&c,0);c.cmd=13;c.param1=i;c.param2=0;ve=SndDoCommand(ch,&c,0);
  {unsigned long start=TickCount();while(!done&&TickCount()-start<180)Delay(1,&discard);}
  sprintf(text,"{\"phase\":\"play\",\"amplitude\":%d,\"error\":%d,\"buffer_error\":%d,\"callback_error\":%d,\"completed\":%ld}\n",levels[i],e,ae,ve,done);line(f,text);
 }
 SetDefaultOutputVolume(old);e=SndDisposeChannel(ch,1);DisposePtr((Ptr)buffer);sprintf(text,"{\"complete\":true,\"dispose_error\":%d}\n",e);line(f,text);FSClose(f);FlushVol(0,0);return 0;
}
