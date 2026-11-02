#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[32768],*p=output;
int main(void){GDHandle main,next;Rect screen,rects[12];short i,k,f,flags[3]={16,31,0};long n;OSErr err;
 InitGraf(&qd.thePort);main=GetMainDevice();next=GetNextDevice(main);if(!main||next)return 8;screen=(**main).gdRect;
 SetRect(&rects[0],screen.left+10,screen.top+10,screen.left+40,screen.top+40);rects[1]=screen;
 SetRect(&rects[2],screen.left-20,screen.top-20,screen.left,screen.top);SetRect(&rects[3],screen.right,screen.top,screen.right+20,screen.top+20);
 SetRect(&rects[4],screen.left-10,screen.top+10,screen.left+10,screen.top+20);SetRect(&rects[5],screen.right-10,screen.top+10,screen.right+10,screen.top+20);
 SetRect(&rects[6],screen.left+10,screen.bottom,screen.left+20,screen.bottom+10);SetRect(&rects[7],screen.left+10,screen.bottom-1,screen.left+20,screen.bottom+10);
 SetRect(&rects[8],screen.left+10,screen.top+10,screen.left+10,screen.top+20);SetRect(&rects[9],screen.left+10,screen.top+10,screen.left+20,screen.top+10);
 SetRect(&rects[10],screen.left+20,screen.top+20,screen.left+10,screen.top+10);SetRect(&rects[11],-1000,-1000,-900,-900);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"max-device-raw-v1\",\"main_handle\":%lu,\"screen\":[%d,%d,%d,%d],\"cases\":[",(unsigned long)main,screen.top,screen.left,screen.bottom,screen.right);
 for(i=0;i<12;i++)for(k=0;k<3;k++){
  register unsigned long d0 __asm__("d0")=0x12345678;register Rect *r __asm__("a2")=&rects[i];register unsigned short ccr __asm__("d3");register unsigned short flag __asm__("d4")=flags[k];register long delta __asm__("d5");register GDHandle selected __asm__("d6");unsigned long result;unsigned short saved;long stack;
  __asm__ volatile("move.l %%sp,%%d5\n\tsubq.l #4,%%sp\n\tmove.l %%a2,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xaa27\n\t.word 0x42c3\n\tmove.l (%%sp)+,%%d6\n\tsub.l %%sp,%%d5":"+d"(d0),"=d"(ccr),"=d"(delta),"=d"(selected):"a"(r),"d"(flag):"d1","d2","a0","a1","cc","memory");result=d0;saved=ccr&31;stack=delta;
  p+=sprintf(p,"%s{\"kind\":%d,\"input_ccr\":%d,\"rect\":[%d,%d,%d,%d],\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"selected\":%lu,\"main_same\":%d,\"null\":%d,\"current_same\":%d}",i||k?",":"",i,flags[k],rects[i].top,rects[i].left,rects[i].bottom,rects[i].right,result,saved,stack,(unsigned long)selected,selected==main,selected==0,GetGDevice()==main);
 }
 p+=sprintf(p,"]}\n");err=Create("\pMaxDeviceProbe.json",0,'pPrb','TEXT');if(err)return 2;if(FSOpen("\pMaxDeviceProbe.json",0,&f))return 3;n=p-output;err=FSWrite(f,&n,output);FSClose(f);FlushVol(0,0);return err?4:0;
}
