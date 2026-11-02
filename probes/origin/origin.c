#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[65536],*p=output;
static void rect(const char *name,Rect r){p+=sprintf(p,",\"%s\":[%d,%d,%d,%d]",name,r.top,r.left,r.bottom,r.right);}
static void region(const char *name,RgnHandle r){short i,n=(**r).rgnSize/2;unsigned short *words=(unsigned short*)*r;p+=sprintf(p,",\"%s\":[",name);for(i=0;i<n;i++)p+=sprintf(p,"%s%u",i?",":"",words[i]);p+=sprintf(p,"]");}
int main(void){GrafPort mono;CGrafPort color;GrafPtr old,g;WindowPtr w=0;Rect bounds={70,90,190,350},a={8,11,42,53},b={25,30,60,75};RgnHandle r=NewRgn(),s=NewRgn();short kind,j,f;short hs[4]={-17,31,31,0},vs[4]={23,-29,-29,0},flags[3]={16,31,0};long n;OSErr err;
 InitGraf(&qd.thePort);InitFonts();InitWindows();GetPort(&old);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"origin-raw-v2\",\"cases\":[");
 for(kind=0;kind<4;kind++){
  if(kind==0){OpenPort(&mono);g=&mono;}else if(kind==1){OpenCPort(&color);g=(GrafPtr)&color;}else {w=kind==2?NewWindow(0,&bounds,"\pOrigin",true,documentProc,(WindowPtr)-1,false,0):NewCWindow(0,&bounds,"\pOrigin",true,documentProc,(WindowPtr)-1,false,0);if(!w)return 5;g=(GrafPtr)w;SetPort(g);}
  SetPort(g);SetRect(&g->portRect,0,0,260,120);if(kind==1||kind==3)SetRect(&(**((CGrafPtr)g)->portPixMap).bounds,-90,-70,550,410);else SetRect(&g->portBits.bounds,-90,-70,550,410);RectRgn(r,&a);RectRgn(s,&b);UnionRgn(r,s,g->visRgn);ClipRect(&b);MoveTo(37,-19);PenSize(2,3);
  for(j=0;j<4;j++){
   register unsigned long d0 __asm__("d0")=0x12345678;register unsigned short ccr __asm__("d3");register short h __asm__("d6")=hs[j],v __asm__("d7")=vs[j];register unsigned short flag __asm__("d4")=flags[j%3];register long delta __asm__("d5");Point local={13,17},global={130,170},pen;GrafPtr current;unsigned long result;unsigned short saved;long stack;
   __asm__ volatile("move.l %%sp,%%d5\n\tmove.w %%d6,-(%%sp)\n\tmove.w %%d7,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word 0xa878\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(d0),"=d"(ccr),"=d"(delta):"d"(h),"d"(v),"d"(flag):"d1","d2","a0","a1","cc","memory");
   result=d0;saved=ccr&31;stack=delta;LocalToGlobal(&local);GlobalToLocal(&global);GetPen(&pen);GetPort(&current);
   p+=sprintf(p,"%s{\"kind\":%d,\"step\":%d,\"h\":%d,\"v\":%d,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"current_same\":%d,\"pen\":[%d,%d],\"local_global\":[%d,%d],\"global_local\":[%d,%d]",kind||j?",":"",kind,j,hs[j],vs[j],flags[j%3],result,saved,stack,current==g,pen.v,pen.h,local.v,local.h,global.v,global.h);
   rect("port_rect",g->portRect);rect("bitmap",kind==1||kind==3?(**((CGrafPtr)g)->portPixMap).bounds:g->portBits.bounds);region("visible",g->visRgn);region("clip",g->clipRgn);p+=sprintf(p,"}");
  }
  SetPort(old);if(kind==0)ClosePort(&mono);else if(kind==1)CloseCPort(&color);else DisposeWindow(w);
 }
 DisposeRgn(r);DisposeRgn(s);p+=sprintf(p,"]}\n");err=Create("\pOriginProbe.json",0,'pPrb','TEXT');if(err)return 2;if(FSOpen("\pOriginProbe.json",0,&f))return 3;n=p-output;err=FSWrite(f,&n,output);FSClose(f);FlushVol(0,0);return err?4:0;
}
