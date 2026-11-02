#include <Multiverse.h>
#include <stdio.h>
static char output[32768],*p=output;
#pragma pack(push,2)
typedef struct{unsigned long result,d0;unsigned short input,ccr;long delta;unsigned long guard;} ABI;
#pragma pack(pop)
static void call(ABI*c){register ABI*args __asm__("a2")=c;
 __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l #0x7b7b7b7b,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l 4(%%a2),%%d0\n\tmove.w 8(%%a2),%%ccr\n\t.word 0xaa28\n\t.word 0x42c3\n\tmove.l %%d0,4(%%a2)\n\tmove.w %%d3,10(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,12(%%a2)\n\tmove.l (%%sp)+,(%%a2)\n\tmove.l (%%sp)+,16(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory");
}
int main(void){short k,s,index=0,file,flags[]={16,31,0},old;long n;OSErr e;GDHandle gd;PixMapHandle pm;
 InitGraf(&qd.thePort);InitFonts();InitWindows();gd=GetGDevice();pm=(*gd)->gdPMap;old=(*pm)->pixelSize;if(SetDepth(gd,8,0,0))return 2;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"color-seed-raw-v1\",\"records\":[");
 for(s=0;s<2;s++)for(k=0;k<3;k++){
  ABI c;unsigned long base,application,builtin,after_builtin,device_before,device_after,after_entries;CTabHandle a,b;ColorSpec color;
  base=GetCTSeed();c.input=flags[k];c.d0=s?0:0x12345678;call(&c);
  a=GetCTable(128);if(!a)return 3;application=(**a).ctSeed;b=GetCTable(8);if(!b)return 4;builtin=(**b).ctSeed;after_builtin=GetCTSeed();device_before=(**(*pm)->pmTable).ctSeed;
  color.value=16;color.rgb.red=0x1234+k;color.rgb.green=0x5678+s;color.rgb.blue=0x9abc;SetEntries(16,0,&color);device_after=(**(*pm)->pmTable).ctSeed;after_entries=GetCTSeed();
  p+=sprintf(p,"%s{\"input_d0\":%lu,\"input_ccr\":%d,\"base\":%lu,\"result\":%lu,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"application_seed\":%lu,\"builtin_seed\":%lu,\"after_builtin\":%lu,\"device_before\":%lu,\"device_after\":%lu,\"after_entries\":%lu,\"device_rgb\":[%u,%u,%u]}",index++?",":"",s?0:0x12345678,flags[k],base,c.result,c.d0,c.ccr&31,c.delta,c.guard,application,builtin,after_builtin,device_before,device_after,after_entries,(**(*pm)->pmTable).ctTable[16].rgb.red,(**(*pm)->pmTable).ctTable[16].rgb.green,(**(*pm)->pmTable).ctTable[16].rgb.blue);
  DisposeCTable(a);DisposeCTable(b);
 }
 p+=sprintf(p,"]}\n");SetDepth(gd,old,0,0);e=Create("\pCTSeedProbe.json",0,'pPrb','TEXT');if(e)return 5;if(FSOpen("\pCTSeedProbe.json",0,&file))return 6;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?7:0;
}
