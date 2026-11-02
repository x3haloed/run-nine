#include <Multiverse.h>
#include <stdio.h>

/* Apple Universal Interfaces 3.4.1 Components.h / Sound.h declarations.
   These APIs are absent from this Multiversal SDK. No application code/assets. */
typedef void *ProbeComponent;
typedef struct { OSType type,subtype,manufacturer;unsigned long flags,mask; } ProbeDescription;
pascal ProbeComponent ProbeFindNext(ProbeComponent c,ProbeDescription *d) M68K_INLINE(0x7004,0xA82A);
pascal OSErr ProbeComponentInfo(ProbeComponent c,ProbeDescription *d,Handle name,Handle info,Handle icon) M68K_INLINE(0x7005,0xA82A);
pascal OSErr ProbeOutputInfo(ProbeComponent c,OSType selector,void *info) M68K_INLINE(0x203C,0x0644,0x0018,0xA800);
pascal OSErr ProbeSetOutputInfo(ProbeComponent c,OSType selector,const void *info) M68K_INLINE(0x203C,0x0648,0x0018,0xA800);
static void word(unsigned char *p,unsigned short x){p[0]=x>>8;p[1]=x;}
static void number(unsigned char *p,unsigned long x){word(p,x>>16);word(p+2,x);}
static void line(short f,const char *s){long n=0;while(s[n])n++;FSWrite(f,&n,s);FlushVol(0,0);}
int main(void){
 short f,index;unsigned long i,start;long full=0x01000100,zero=0;Handle sound,name;unsigned char *p;ProbeDescription search={'sdev',0,0,0,0},d;ProbeComponent c=0;char text[2048],namehex[513];
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();TEInit();InitDialogs(0);
 if(Create("\pSoundDevicesProbe.jsonl",0,'aPrb','TEXT')||FSOpen("\pSoundDevicesProbe.jsonl",0,&f))return 2;
 line(f,"{\"probe\":\"sound-devices-v2\",\"frames_per_device\":48000,\"sample_rate\":48000}\n");
 sound=NewHandleClear(48036);name=NewHandle(0);if(!sound||!name)return 3;HLock(sound);p=(unsigned char*)*sound;
 word(p,2);word(p+2,0);word(p+4,1);word(p+6,0x8051);word(p+8,0);number(p+10,14);
 number(p+14,0);number(p+18,48000);number(p+22,0xbb800000);p[35]=60;
 for(i=0;i<48000;i++)p[36+i]=i<12000||i>=36000?128:i<24000?(((i-12000)/32)&1?160:96):(unsigned char)(i-24000);
 for(index=0;index<16&&(c=ProbeFindNext(c,&search));index++){
  OSErr info,mutequery,unmute,volume,newerr,playerr=-999,disposeerr=-999;unsigned char mute[16];SndChannelPtr channel=0;SndCommand command;
  for(i=0;i<16;i++)mute[i]=0xa5;
  info=ProbeComponentInfo(c,&d,name,0,0);namehex[0]=0;
  if(!info&&GetHandleSize(name)>0){unsigned char *s=(unsigned char*)*name;for(i=0;i<s[0];i++)sprintf(namehex+i*2,"%02x",s[i+1]);}
  mutequery=ProbeOutputInfo(c,'hmut',mute);unmute=ProbeSetOutputInfo(c,'hmut',&zero);volume=ProbeSetOutputInfo(c,'hvol',&full);
  sprintf(text,"{\"index\":%d,\"phase\":\"before_play\",\"component\":%lu,\"type\":%lu,\"subtype\":%lu,\"manufacturer\":%lu,\"flags\":%lu,\"name_hex\":\"%s\",\"info_error\":%d,\"mute_query_error\":%d,\"mute_prefix_hex\":\"%02x%02x%02x%02x\",\"unmute_error\":%d,\"volume_error\":%d}\n",index,(unsigned long)c,d.type,d.subtype,d.manufacturer,d.flags,namehex,info,mutequery,mute[0],mute[1],mute[2],mute[3],unmute,volume);line(f,text);
  /* TN1108 passes the Component identifier, not its four-character subtype. */
  start=TickCount();newerr=SndNewChannel(&channel,-1,(long)c,0);
  if(!newerr&&channel){command.cmd=46;command.param1=0;command.param2=full;SndDoImmediate(channel,&command);playerr=SndPlay(channel,sound,0);disposeerr=SndDisposeChannel(channel,1);}
  sprintf(text,"{\"index\":%d,\"phase\":\"after_play\",\"new_channel_error\":%d,\"channel_nonnull\":%d,\"play_error\":%d,\"dispose_error\":%d,\"elapsed_ticks\":%lu}\n",index,newerr,channel!=0,playerr,disposeerr,TickCount()-start);line(f,text);
 }
 sprintf(text,"{\"complete\":true,\"devices\":%d}\n",index);line(f,text);FSClose(f);DisposeHandle(name);DisposeHandle(sound);FlushVol(0,0);return 0;
}
