#include <Multiverse.h>
#include <stdio.h>

/* Owned unsigned 8-bit mono input, not an extracted game sound. */
static void word(unsigned char *p,unsigned short x){p[0]=x>>8;p[1]=x;}
static void number(unsigned char *p,unsigned long x){word(p,x>>16);word(p+2,x);}
static void marker(short file,const char *s){long n=0;while(s[n])n++;FSWrite(file,&n,s);FlushVol(0,0);}
int main(void){
 short rawfile,statusfile,file;long n,oldvolume=0;unsigned long i,start,end;OSErr e,query,set,newerr,volerr=-999,playerr=-999,disposeerr=-999;Handle sound;unsigned char *p;SndChannelPtr channel=0;SndCommand command;char output[2048];
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();TEInit();InitDialogs(0);
 if(Create("\pSoundPCMProbe.status",0,'aPrb','TEXT')||FSOpen("\pSoundPCMProbe.status",0,&statusfile))return 2;
 marker(statusfile,"initialized\r");
 sound=NewHandleClear(48036);if(!sound)return 3;HLock(sound);p=(unsigned char*)*sound;
 word(p,2);word(p+2,0);word(p+4,1);word(p+6,0x8051);word(p+8,0);number(p+10,14);
 number(p+14,0);number(p+18,48000);number(p+22,0xbb800000);number(p+26,0);number(p+30,0);p[34]=0;p[35]=60;
 for(i=0;i<48000;i++)p[36+i]=i<12000||i>=36000?128:i<24000?(((i-12000)/32)&1?160:96):(unsigned char)(i-24000);
 if(Create("\pSoundPCMProbe.raw",0,'aPrb','DATA')||FSOpen("\pSoundPCMProbe.raw",0,&rawfile))return 4;
 n=48000;e=FSWrite(rawfile,&n,p+36);FSClose(rawfile);FlushVol(0,0);if(e)return 5;
 query=GetDefaultOutputVolume(&oldvolume);set=SetDefaultOutputVolume(0x01000100);marker(statusfile,"input saved; volume configured\r");
 newerr=SndNewChannel(&channel,5,0x80,0);marker(statusfile,"channel allocation returned\r");
 start=TickCount();
 if(!newerr&&channel){command.cmd=46;command.param1=0;command.param2=0x01000100;volerr=SndDoImmediate(channel,&command);marker(statusfile,"play begin\r");playerr=SndPlay(channel,sound,0);marker(statusfile,"play returned\r");disposeerr=SndDisposeChannel(channel,1);}
 end=TickCount();if(!query)SetDefaultOutputVolume(oldvolume);
 n=sprintf(output,"{\"schema\":1,\"probe\":\"sound-pcm-raw-v1\",\"frames\":48000,\"sample_rate\":48000,\"sample_bits\":8,\"channels\":1,\"query_volume_error\":%d,\"set_volume_error\":%d,\"new_channel_error\":%d,\"channel_nonnull\":%d,\"channel_volume_error\":%d,\"play_error\":%d,\"dispose_error\":%d,\"elapsed_ticks\":%lu}\n",query,set,newerr,channel!=0,volerr,playerr,disposeerr,end-start);
 e=Create("\pSoundPCMProbe.json",0,'aPrb','TEXT');if(e)return 6;if(FSOpen("\pSoundPCMProbe.json",0,&file))return 7;e=FSWrite(file,&n,output);FSClose(file);marker(statusfile,"complete\r");FSClose(statusfile);DisposeHandle(sound);FlushVol(0,0);return e?8:0;
}
