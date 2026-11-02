#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[1048576],*p=output;
#pragma pack(push,2)
typedef struct{RGBColor *color;unsigned long d0;unsigned short input,ccr;long delta;unsigned long guard;}ABI;
#pragma pack(pop)
#define STR_(x) #x
#define STR(x) STR_(x)
#define CALL(OP) __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l (%%a2),-(%%sp)\n\tmove.l 4(%%a2),%%d0\n\tmove.w 8(%%a2),%%ccr\n\t.word " STR(OP) "\n\t.word 0x42c3\n\tmove.l %%d0,4(%%a2)\n\tmove.w %%d3,10(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,12(%%a2)\n\tmove.l (%%sp)+,16(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory")
static void invoke(short op,ABI*c){register ABI*args __asm__("a2")=c;if(op) {CALL(0xaa1a);}else{CALL(0xaa19);}}
int main(void){long colors[]={30,33,205,341,409,273,137,69};short flags[]={16,31,0},kind,op,color,seed,k,index=0,file,old;GDHandle gd;GrafPtr prior;GrafPort mono;CGrafPort colorport;long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();gd=GetGDevice();old=(**(*gd)->gdPMap).pixelSize;if(SetDepth(gd,8,0,0))return 2;GetPort(&prior);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"color-read-raw-v1\",\"depth\":8,\"records\":[");
 for(kind=0;kind<2;kind++){
  GrafPtr port;if(kind){OpenCPort(&colorport);port=(GrafPtr)&colorport;}else{OpenPort(&mono);port=&mono;}SetPort(port);
  for(op=0;op<2;op++)for(color=0;color<8;color++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
   ABI c;RGBColor fg,bg;struct {unsigned long before;RGBColor value;unsigned long after;} result={0x10203040,{0x1234,0x5678,0x9abc},0x89abcdef};unsigned char before[108],after[108];short mem,res,preserved;long f,b;
   ForeColor(colors[color]);BackColor(colors[7-color]);memcpy(before,port,108);
   c.color=&result.value;c.d0=seed?0:0x12345678;c.input=flags[k];*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(op,&c);
   mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;memcpy(after,port,108);f=port->fgColor;b=port->bkColor;GetForeColor(&fg);GetBackColor(&bg);
   preserved=memcmp(before,after,108)==0;
   p+=sprintf(p,"%s{\"port_kind\":%d,\"op\":%d,\"color\":%ld,\"input_d0\":%lu,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"mem_error\":%d,\"res_error\":%d,\"result\":[%u,%u,%u],\"result_before_guard\":%lu,\"result_after_guard\":%lu,\"fg_color\":%ld,\"bg_color\":%ld,\"foreground\":[%u,%u,%u],\"background\":[%u,%u,%u],\"other_port_bytes_preserved\":%d}",index++?",":"",kind,op,colors[color],seed?0:0x12345678,flags[k],c.d0,c.ccr&31,c.delta,c.guard,mem,res,result.value.red,result.value.green,result.value.blue,result.before,result.after,f,b,fg.red,fg.green,fg.blue,bg.red,bg.green,bg.blue,preserved);
  }
  if(kind)CloseCPort(&colorport);else ClosePort(&mono);
 }
 SetPort(prior);SetDepth(gd,old,0,0);p+=sprintf(p,"]}\n");e=Create("\pColorReadProbe.json",0,'pPrb','TEXT');if(e)return 3;if(FSOpen("\pColorReadProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
