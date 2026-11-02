#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[1048576],*p=output;
#pragma pack(push,2)
typedef struct{unsigned long d0;unsigned short input,ccr;long delta;unsigned short same;unsigned long guard;}ABI;
typedef struct{unsigned long prefix;unsigned char event[16];unsigned long suffix;}Buffer;
#pragma pack(pop)
#define STR_(x) #x
#define STR(x) STR_(x)
#define CALL(OP) __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l %%a4,%%a0\n\tmove.l (%%a2),%%d0\n\tmove.w 4(%%a2),%%ccr\n\t.word " STR(OP) "\n\t.word 0x42c3\n\tmove.l %%d0,(%%a2)\n\tmove.w %%d3,6(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,8(%%a2)\n\tcmpa.l %%a4,%%a0\n\tseq %%d1\n\tand.w #1,%%d1\n\tmove.w %%d1,12(%%a2)\n\tmove.l (%%sp)+,14(%%a2)":"+a"(args),"+a"(out)::"d0","d1","d2","d3","a0","a1","a3","cc","memory")
static void invoke(short op,ABI*c,Buffer*b){register ABI*args __asm__("a2")=c;register unsigned char*out __asm__("a4")=b->event;if(op){CALL(0xa031);}else{CALL(0xa030);}}
static short post(short what,unsigned long message){register unsigned long d0 __asm__("d0")=message;register unsigned long a0 __asm__("a0")=(unsigned short)what;__asm__ volatile(".word 0xa02f":"+d"(d0),"+a"(a0)::"d1","d2","a1","cc","memory");return(short)d0;}
static void hex(const unsigned char*b,short n){short i;p+=sprintf(p,"\"");for(i=0;i<n;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
static void queue(void){short i=0;EvQElPtr q=(EvQElPtr)LMGetEventQueue().qHead;p+=sprintf(p,"[");while(q&&i<20){if(i++)p+=sprintf(p,",");hex((unsigned char*)&q->evtQWhat,16);q=(EvQElPtr)q->qLink;}p+=sprintf(p,"]");}
int main(void){short which,mask,op,seed,k,index=0,file,j,flags[]={16,31,0},masks[]={0,-1,8,6,0x100},lengths[]={0,1,1,1,2,2,2,3},inputs[8][3]={{0},{3},{2},{1},{3,2},{2,3},{1,3},{1,2,3}};long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();InitCursor();
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"os-events-raw-v2\",\"records\":[");
 for(which=0;which<8;which++)for(mask=0;mask<5;mask++)for(op=0;op<2;op++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
  ABI c;Buffer b;unsigned long before,after;short mem,res,errors[3];unsigned char phys[32]={0};
  FlushEvents(everyEvent,0);for(j=0;j<lengths[which];j++)errors[j]=post(inputs[which][j],0x12340000UL+j);
  { EvQElPtr item=(EvQElPtr)LMGetEventQueue().qHead;short qindex=0;
    while(item&&qindex<20){item->evtQWhen=0x10203000UL+qindex;item->evtQWhere.v=-123-qindex;item->evtQWhere.h=234+qindex;item->evtQModifiers=0x1a80+qindex;item=(EvQElPtr)item->qLink;qindex++;}
  }
  c.d0=(seed?0x12340000UL:0)|(unsigned short)masks[mask];c.input=flags[k];b.prefix=0xa1b2c3d4;b.suffix=0x10203040;memset(b.event,0xa5,16);
  p+=sprintf(p,"%s{\"queue\":%d,\"mask\":%u,\"op\":%d,\"input_d0\":%lu,\"input_ccr\":%u,\"post_errors\":[",index++?",":"",which,(unsigned short)masks[mask],op,c.d0,c.input);for(j=0;j<lengths[which];j++)p+=sprintf(p,"%s%d",j?",":"",errors[j]);p+=sprintf(p,"],\"queue_before\":");queue();
  for(j=0;j<16;j++)phys[j]=((volatile unsigned char*)0x174)[j];phys[16]=*(volatile unsigned char*)0x172;for(j=0;j<12;j++)phys[18+j]=((volatile unsigned char*)0x828)[j];
  before=TickCount();*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(op,&c,&b);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;after=TickCount();
  p+=sprintf(p,",\"ticks_before\":%lu,\"ticks_after\":%lu,\"physical_hex\":",before,after);hex(phys,30);p+=sprintf(p,",\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"a0_preserved\":%u,\"guard\":%lu,\"prefix\":%lu,\"suffix\":%lu,\"mem_error\":%d,\"res_error\":%d,\"event_hex\":",c.d0,c.ccr&31,c.delta,c.same,c.guard,b.prefix,b.suffix,mem,res);hex(b.event,16);p+=sprintf(p,",\"queue_after\":");queue();p+=sprintf(p,"}");
 }
 p+=sprintf(p,"]}\n");e=Create("\pOSEventProbe.json",0,'pPrb','TEXT');if(e)return 3;if(FSOpen("\pOSEventProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
