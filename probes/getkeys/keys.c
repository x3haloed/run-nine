#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[65536],*p=output;
#pragma pack(push,2)
typedef struct{unsigned long d0;unsigned short input,ccr;long delta;unsigned long guard;}ABI;
typedef struct{unsigned long before;unsigned char keys[16];unsigned long after;}Buffer;
#pragma pack(pop)
static void invoke(ABI*c,Buffer*b){register ABI*args __asm__("a2")=c;register unsigned char*out __asm__("a4")=b->keys;
 __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l %%a4,-(%%sp)\n\tmove.l (%%a2),%%d0\n\tmove.w 4(%%a2),%%ccr\n\t.word 0xa976\n\t.word 0x42c3\n\tmove.l %%d0,(%%a2)\n\tmove.w %%d3,6(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,8(%%a2)\n\tmove.l (%%sp)+,12(%%a2)":"+a"(args),"+a"(out)::"d0","d1","d2","d3","a0","a1","a3","cc","memory");}
static void hex(const unsigned char*b){short i;p+=sprintf(p,"\"");for(i=0;i<16;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
static void readmap(unsigned char*b){short i;for(i=0;i<16;i++)b[i]=((volatile unsigned char*)0x174)[i];}
static short bit(unsigned char*b,short key){return (b[key/8]>>(key%8))&1;}
static short ready(short phase,unsigned char*b){short i;if(phase==1)return bit(b,126);if(phase==2)return bit(b,13);if(phase==3)return bit(b,49);if(phase==4)return bit(b,126)&&(bit(b,56)||bit(b,60));for(i=0;i<16;i++)if(b[i])return false;return true;}
int main(void){Rect bounds={80,80,260,600};WindowPtr w;short phase,seed,k,index=0,file,flags[]={16,31,0};unsigned char map[16];long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();InitCursor();w=NewWindow(0,&bounds,"\pGetKeys: native held-key samples",true,documentProc,(WindowPtr)-1,false,0);if(!w)return 2;SetPort(w);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"get-keys-raw-v1\",\"records\":[");
 for(phase=0;phase<6;phase++){
  SetPort(w);EraseRect(&w->portRect);MoveTo(15,30);
  switch(phase){case 1:DrawString("\pHold UP arrow for 3 seconds.");break;case 2:DrawString("\pHold W for 3 seconds.");break;case 3:DrawString("\pHold SPACE for 3 seconds.");break;case 4:DrawString("\pHold SHIFT + UP for 3 seconds.");break;default:DrawString("\pRelease every key.");break;}
  do{SystemTask();readmap(map);}while(!ready(phase,map));
  for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
   ABI c;Buffer b;unsigned char before[16],after[16];short mem,res;
   b.before=0xa1b2c3d4;b.after=0x10203040;memset(b.keys,0x7b,16);c.d0=seed?0:0x12345678;c.input=flags[k];
   readmap(before);*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(&c,&b);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;readmap(after);
   p+=sprintf(p,"%s{\"phase\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"prefix\":%lu,\"suffix\":%lu,\"mem_error\":%d,\"res_error\":%d,\"before_hex\":",index++?",":"",phase,seed?0:0x12345678,flags[k],c.d0,c.ccr&31,c.delta,c.guard,b.before,b.after,mem,res);hex(before);p+=sprintf(p,",\"after_hex\":");hex(after);p+=sprintf(p,",\"keys_hex\":");hex(b.keys);p+=sprintf(p,"}");
  }
  if(phase>0&&phase<5)do{SystemTask();readmap(map);}while(!ready(0,map));
 }
 p+=sprintf(p,"]}\n");e=Create("\pGetKeysProbe.json",0,'pPrb','TEXT');if(e)return 3;if(FSOpen("\pGetKeysProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);DisposeWindow(w);return e?5:0;
}
