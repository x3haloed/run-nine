#include <Multiverse.h>
#include <stdio.h>
static char output[32768],*p=output;
#pragma pack(push,2)
typedef struct{unsigned long d0;unsigned short input,ccr;long delta;unsigned short same;unsigned long guard;unsigned short mode;}ABI;
#pragma pack(pop)
static void invoke(ABI*c){register ABI*args __asm__("a2")=c;
 __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l %%a2,%%a0\n\tmoveq #0,%%d5\n\tmove.l (%%a2),%%d0\n\tmove.w 4(%%a2),%%ccr\n\t.word 0xa05d\n\t.word 0x42c3\n\tmove.l %%d0,%%d4\n\tmove.b 0xcb2,%%d5\n\tmove.l %%sp,%%d7\n\tsub.l %%a3,%%d7\n\tcmpa.l %%a2,%%a0\n\tseq %%d6\n\tand.w #1,%%d6\n\tmoveq #1,%%d0\n\t.word 0xa05d\n\tmove.l %%d4,(%%a2)\n\tmove.w %%d3,6(%%a2)\n\tmove.l %%d7,8(%%a2)\n\tmove.w %%d6,12(%%a2)\n\tmove.l (%%sp)+,14(%%a2)\n\tmove.w %%d5,18(%%a2)":"+a"(args)::"d0","d1","d2","d3","d4","d5","d6","d7","a0","a1","a3","cc","memory");}
int main(void){short requested,upper,k,index=0,file,flags[]={16,31,0},requests[]={1,0};unsigned long uppers[]={0,0x12345600UL,0xffff0000UL};long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();InitCursor();
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"mmu-mode-raw-v1\",\"records\":[");
 for(requested=0;requested<2;requested++)for(upper=0;upper<3;upper++)for(k=0;k<3;k++){
  ABI c;short before,after,mem,res;c.d0=uppers[upper]|requests[requested];c.input=flags[k];before=LMGetMMU32Bit();*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(&c);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;after=LMGetMMU32Bit();
  p+=sprintf(p,"%s{\"requested\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"mode_before\":%d,\"mode_after\":%u,\"mode_restored\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"a0_preserved\":%u,\"guard\":%lu,\"mem_error\":%d,\"res_error\":%d}",index++?",":"",requests[requested],uppers[upper]|requests[requested],flags[k],before,c.mode,after,c.d0,c.ccr&31,c.delta,c.same,c.guard,mem,res);
 }
 p+=sprintf(p,"]}\n");e=Create("\pMMUModeProbe.json",0,'pPrb','TEXT');if(e)return 3;if(FSOpen("\pMMUModeProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
