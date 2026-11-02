#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[65536],*p=output;
static void hex(const char *name,const void *bytes,short n){short i;const unsigned char *b=bytes;p+=sprintf(p,",\"%s\":\"",name);for(i=0;i<n;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
static void snapshot(GDHandle h){GDevice *g=*h;PixMap *pm=g->gdPMap?*g->gdPMap:0;ITab *it=g->gdITable?*g->gdITable:0;
 p+=sprintf(p,",\"gd_size\":%ld",GetHandleSize((Handle)h));hex("gd_hex",g,sizeof(GDevice));
 p+=sprintf(p,",\"pm_size\":%ld",g->gdPMap?GetHandleSize((Handle)g->gdPMap):-1L);hex("pm_hex",pm,pm?sizeof(PixMap):0);
 p+=sprintf(p,",\"itable_size\":%ld,\"itable_seed\":%ld,\"itable_res\":%d",g->gdITable?GetHandleSize((Handle)g->gdITable):-1L,it?it->iTabSeed:0,it&&GetHandleSize((Handle)g->gdITable)>=6?it->iTabRes:-1);
 p+=sprintf(p,",\"table_size\":%ld,\"ccdata_size\":%ld,\"ccmask_size\":%ld",pm&&pm->pmTable?GetHandleSize((Handle)pm->pmTable):-1L,g->gdCCXData?GetHandleSize(g->gdCCXData):-1L,g->gdCCXMask?GetHandleSize(g->gdCCXMask):-1L);if(pm&&pm->pmTable)hex("table_hex",*pm->pmTable,GetHandleSize((Handle)pm->pmTable));else hex("table_hex",0,0);
 if(g->gdCCXData)hex("ccdata_hex",*g->gdCCXData,GetHandleSize(g->gdCCXData));else hex("ccdata_hex",0,0);if(g->gdCCXMask)hex("ccmask_hex",*g->gdCCXMask,GetHandleSize(g->gdCCXMask));else hex("ccmask_hex",0,0);
 {Handle handles[6];short i,j,distinct=1;handles[0]=(Handle)h;handles[1]=(Handle)g->gdPMap;handles[2]=(Handle)g->gdITable;handles[3]=g->gdCCXData;handles[4]=g->gdCCXMask;handles[5]=pm?(Handle)pm->pmTable:0;for(i=0;i<6;i++){if(!handles[i])distinct=0;for(j=0;j<i;j++)if(handles[i]==handles[j])distinct=0;}p+=sprintf(p,",\"children_distinct\":%d",distinct);}

}
int main(void){GDHandle main,list,devices[12];short refs[4]={0,-1,42,-51},flags[3]={16,31,0},i,k,index=0,f;long n;OSErr err;
 InitGraf(&qd.thePort);main=GetGDevice();list=GetDeviceList();p+=sprintf(p,"{\"schema\":1,\"probe\":\"off-device-raw-v3\",\"records\":[");
 for(i=0;i<4;i++)for(k=0;k<3;k++){
  register unsigned long d0 __asm__("d0")=0x12345678;register short ref __asm__("d7")=refs[i];register unsigned short ccr __asm__("d3");register unsigned short flag __asm__("d4")=flags[k];register long delta __asm__("d5");register GDHandle h __asm__("d6");unsigned long result;unsigned short saved;long stack;short unique=1,j;
  __asm__ volatile("move.l %%sp,%%d5\n\tsubq.l #4,%%sp\n\tmove.w %%d7,-(%%sp)\n\tmove.l #-1,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xaa2f\n\t.word 0x42c3\n\tmove.l (%%sp)+,%%d6\n\tsub.l %%sp,%%d5":"+d"(d0),"=d"(ccr),"=d"(delta),"=d"(h):"d"(ref),"d"(flag):"d1","d2","a0","a1","cc","memory");result=d0;saved=ccr&31;stack=delta;
  if(!h)return 6;for(j=0;j<index;j++)if(devices[j]==h)unique=0;devices[index]=h;
  p+=sprintf(p,"%s{\"refnum\":%d,\"mode\":-1,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"new_nonnull\":%d,\"distinct_main\":%d,\"distinct_previous\":%d,\"current_same\":%d,\"list_same\":%d",index?",":"",refs[i],flags[k],result,saved,stack,h!=0,h!=main,unique,GetGDevice()==main,GetDeviceList()==list);snapshot(h);p+=sprintf(p,"}");index++;
 }
 p+=sprintf(p,"],\"tables\":[");
 for(i=0;i<3;i++){
  CTabHandle replacement=(CTabHandle)NewHandleClear(32);GDHandle device=devices[i];CTabHandle old=(**(**device).gdPMap).pmTable;register unsigned long d0 __asm__("d0")=0x12345678;register CTabHandle h __asm__("a2")=old;register unsigned short ccr __asm__("d3");register unsigned short flag __asm__("d4")=flags[i];register long delta __asm__("d5");unsigned long result;unsigned short saved;long stack;
  __asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xaa24\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"=d"(ccr),"=d"(delta):"a"(h),"d"(flag):"d1","d2","a0","a1","cc","memory");result=d0;saved=ccr&31;stack=delta;
  (**(**device).gdPMap).pmTable=replacement;p+=sprintf(p,"%s{\"index\":%d,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"replacement_distinct\":%d,\"current_same\":%d,\"list_same\":%d}",i?",":"",i,flags[i],result,saved,stack,replacement&&replacement!=old,GetGDevice()==main,GetDeviceList()==list);
 }
 p+=sprintf(p,"],\"switches\":[");
 for(i=0;i<3;i++)for(k=0;k<3;k++){
  GDHandle target=i==0?devices[0]:i==1?devices[1]:main;register unsigned long d0 __asm__("d0")=0x12345678;register GDHandle h __asm__("a2")=target;register unsigned short ccr __asm__("d3");register unsigned short flag __asm__("d4")=flags[k];register long delta __asm__("d5");unsigned long result;unsigned short saved;long stack;
  __asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xaa31\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"=d"(ccr),"=d"(delta):"a"(h),"d"(flag):"d1","d2","a0","a1","cc","memory");result=d0;saved=ccr&31;stack=delta;
  p+=sprintf(p,"%s{\"target\":%d,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"selected_same\":%d,\"list_same\":%d}",i||k?",":"",i,flags[k],result,saved,stack,GetGDevice()==target,GetDeviceList()==list);
 }
 p+=sprintf(p,"],\"disposals\":[");
 for(i=0;i<12;i++){
  register unsigned long d0 __asm__("d0")=0x12345678;register GDHandle h __asm__("a2")=devices[i];register unsigned short ccr __asm__("d3");register unsigned short flag __asm__("d4")=flags[i%3];register long delta __asm__("d5");unsigned long result;unsigned short saved;long stack;
  __asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xaa30\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"=d"(ccr),"=d"(delta):"a"(h),"d"(flag):"d1","d2","a0","a1","cc","memory");result=d0;saved=ccr&31;stack=delta;
  p+=sprintf(p,"%s{\"index\":%d,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"current_same\":%d,\"list_same\":%d}",i?",":"",i,flags[i%3],result,saved,stack,GetGDevice()==main,GetDeviceList()==list);
 }
 p+=sprintf(p,"]}\n");err=Create("\pOffDeviceProbe.json",0,'pPrb','TEXT');if(err)return 2;if(FSOpen("\pOffDeviceProbe.json",0,&f))return 3;n=p-output;err=FSWrite(f,&n,output);FSClose(f);FlushVol(0,0);return err?4:0;
}
