#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[131072],*p=output;
#pragma pack(push,2)
typedef struct{unsigned long d0;unsigned short input,ccr;long delta;unsigned short same;unsigned long guard;}ABI;
typedef struct{unsigned long prefix;unsigned char bytes[128];unsigned long suffix;}PB;
typedef struct{unsigned long prefix;unsigned char bytes[256];unsigned long suffix;}Name;
#pragma pack(pop)
static void invoke(ABI*c,PB*b){register ABI*args __asm__("a2")=c;register unsigned char*pb __asm__("a4")=b->bytes;
 __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l %%a4,%%a0\n\tmove.l (%%a2),%%d0\n\tmove.w 4(%%a2),%%ccr\n\t.word 0xa207\n\t.word 0x42c3\n\tmove.l %%d0,(%%a2)\n\tmove.w %%d3,6(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,8(%%a2)\n\tcmpa.l %%a4,%%a0\n\tseq %%d1\n\tand.w #1,%%d1\n\tmove.w %%d1,12(%%a2)\n\tmove.l (%%sp)+,14(%%a2)":"+a"(args),"+a"(pb)::"d0","d1","d2","d3","a0","a1","a3","cc","memory");}
static void hex(const unsigned char*b,short n){short i;p+=sprintf(p,"\"");for(i=0;i<n;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
int main(void){Str255 current;short ref,which,seed,k,index=0,file,flags[]={16,31,0};long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();InitCursor();if(GetVol(current,&ref))return 2;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"hgetvinfo-raw-v1\",\"current_ref\":%d,\"current_name_hex\":",ref);hex(current,current[0]+1);p+=sprintf(p,",\"records\":[");
 for(which=0;which<8;which++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
  ABI c;PB b;Name name;HVolumeParam*v=(HVolumeParam*)b.bytes;unsigned long address;short mem,res;
  b.prefix=0xa1b2c3d4;b.suffix=0x10203040;memset(b.bytes,0xa5,128);name.prefix=0x11223344;name.suffix=0x99aabbcc;memset(name.bytes,0xa5,256);
  if(which==5)memcpy(name.bytes,current,current[0]+1);if(which==6)memcpy(name.bytes,"\pNotAnOwnedVolume",17);
  v->ioCompletion=0;v->ioNamePtr=which==7?0:name.bytes;v->ioVRefNum=which==1?ref:which==4?32767:0;v->ioVolIndex=which==2?1:which==3?2:which==5||which==6?-1:0;v->ioResult=0x7b7b;address=(unsigned long)name.bytes;c.d0=seed?0:0x12345678;c.input=flags[k];
  p+=sprintf(p,"%s{\"which\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"name_address\":%lu,\"before_hex\":",index++?",":"",which,c.d0,c.input,address);hex(b.bytes,128);p+=sprintf(p,",\"name_before_hex\":");hex(name.bytes,256);
  *(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(&c,&b);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;
  p+=sprintf(p,",\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"a0_preserved\":%u,\"guard\":%lu,\"pb_guards\":[%lu,%lu],\"name_guards\":[%lu,%lu],\"mem_error\":%d,\"res_error\":%d,\"after_hex\":",c.d0,c.ccr&31,c.delta,c.same,c.guard,b.prefix,b.suffix,name.prefix,name.suffix,mem,res);hex(b.bytes,128);p+=sprintf(p,",\"name_after_hex\":");hex(name.bytes,256);p+=sprintf(p,"}");
 }
 p+=sprintf(p,"]}\n");e=Create("\pHGetVInfoProbe.json",0,'pPrb','TEXT');if(e)return 3;if(FSOpen("\pHGetVInfoProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
