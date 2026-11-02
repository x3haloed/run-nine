#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[262144],*p=output;
static void hexrgn(const char*key,RgnHandle r){short i,n=(**r).rgnSize;unsigned char*b=(unsigned char*)*r;p+=sprintf(p,",\"%s\":\"",key);if(n<0||n>4096)return;for(i=0;i<n;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
static void rect(const char*key,Rect*r){p+=sprintf(p,",\"%s\":[%d,%d,%d,%d]",key,r->top,r->left,r->bottom,r->right);}
#define CALL(OP) __asm__ volatile("move.l %%sp,%%d5\n\tmove.l %%a2,-(%%sp)\n\tmove.w %%d4,%%ccr\n\t.word " #OP "\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+d"(result),"=d"(status),"=d"(stack):"a"(a2),"d"(input):"d1","d2","a0","a1","cc","memory")
int main(void){Rect bounds={80,70,200,250},shapes[]={{10,20,30,60},{-30,-50,180,250},{5,5,5,5},{300,400,320,420}},inner={12,20,36,80},clip={10,10,60,80};WindowPtr w;WindowPeek wp;RgnHandle source,temp;short oi,origin,shape,k,index=0,file,flags[]={16,31,0};unsigned short ops[]={0xa92a,0xa929,0xa928,0xa927};long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();w=(WindowPtr)NewCWindow(0,&bounds,"\pDamage",true,documentProc,(WindowPtr)-1,true,0);if(!w)return 2;wp=(WindowPeek)w;SetPort(w);source=NewRgn();temp=NewRgn();{EventRecord event;GetNextEvent(activMask,&event);GetNextEvent(activMask,&event);BeginUpdate(w);EndUpdate(w);}
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"window-damage-raw-v2\",\"records\":[");
 for(oi=0;oi<4;oi++)for(origin=0;origin<2;origin++)for(shape=0;shape<4;shape++)for(k=0;k<3;k++){
  Rect input_rect=shapes[shape];void*arg;unsigned long d0;unsigned short ccr;long delta;short hit1,hit2;EventRecord event1,event2;GrafPtr current;
  SetPort(w);SetOrigin(origin?31:0,origin?-7:0);ClipRect(&clip);InvalRect(&wp->port.portRect);RectRgn(source,&input_rect);
  if(shape==0&&(oi==1||oi==3)){Rect outer={4,8,50,100};RectRgn(source,&outer);RectRgn(temp,&inner);DiffRgn(source,temp,source);}
  arg=(oi==0||oi==2)?(void*)&input_rect:(void*)source;
  p+=sprintf(p,"%s{\"trap\":%u,\"origin_case\":%d,\"shape\":%d,\"input_ccr\":%d",index++?",":"",ops[oi],origin,shape,flags[k]);rect("input_rect",&input_rect);rect("port_rect",&wp->port.portRect);rect("bitmap_bounds",&(**((CGrafPtr)w)->portPixMap).bounds);hexrgn("input_region",source);hexrgn("before_update",wp->updateRgn);hexrgn("before_vis",wp->port.visRgn);hexrgn("before_clip",wp->port.clipRgn);
  {register void*a2 __asm__("a2")=arg;register unsigned long result __asm__("d0")=0x12345678;register unsigned short input __asm__("d4")=flags[k];register unsigned short status __asm__("d3");register long stack __asm__("d5");if(oi==0){CALL(0xa92a);}else if(oi==1){CALL(0xa929);}else if(oi==2){CALL(0xa928);}else{CALL(0xa927);}d0=result;ccr=status&31;delta=stack;}
  GetPort(&current);p+=sprintf(p,",\"d0\":%lu,\"d0_is_arg\":%d,\"d0_is_window\":%d,\"d0_is_update_handle\":%d,\"d0_is_update_data\":%d,\"ccr\":%u,\"stack_delta\":%ld,\"current_same\":%d",d0,d0==(unsigned long)arg,d0==(unsigned long)w,d0==(unsigned long)wp->updateRgn,d0==(unsigned long)*wp->updateRgn,ccr,delta,current==(GrafPtr)w);hexrgn("after_update",wp->updateRgn);hexrgn("after_vis",wp->port.visRgn);hexrgn("after_clip",wp->port.clipRgn);hexrgn("after_input_region",source);rect("after_input_rect",&input_rect);rect("after_port_rect",&wp->port.portRect);rect("after_bitmap_bounds",&(**((CGrafPtr)w)->portPixMap).bounds);
  memset(&event1,0xa5,sizeof(event1));memset(&event2,0xa5,sizeof(event2));hit1=GetNextEvent(updateMask,&event1);hit2=GetNextEvent(updateMask,&event2);
  p+=sprintf(p,",\"event1\":[%d,%u,%d,%d],\"event2\":[%d,%u,%d,%d]",hit1,event1.what,event1.message==(unsigned long)w,event1.message==0,hit2,event2.what,event2.message==(unsigned long)w,event2.message==0);hexrgn("after_events_update",wp->updateRgn);BeginUpdate(w);hexrgn("during_update",wp->updateRgn);hexrgn("during_vis",wp->port.visRgn);hexrgn("during_clip",wp->port.clipRgn);EndUpdate(w);hexrgn("final_update",wp->updateRgn);hexrgn("final_vis",wp->port.visRgn);hexrgn("final_clip",wp->port.clipRgn);p+=sprintf(p,"}");
 }
 p+=sprintf(p,"]}\n");DisposeRgn(source);DisposeRgn(temp);DisposeWindow(w);e=Create("\pWindowDamageProbe.json",0,'pPrb','TEXT');if(e)return 3;if(FSOpen("\pWindowDamageProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
