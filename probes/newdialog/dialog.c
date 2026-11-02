#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[262144],*p=output;
#pragma pack(push,2)
typedef struct {unsigned long d0,dst,behind;short id;unsigned short input,ccr;unsigned long result;long delta;unsigned long guard;}ABI;
#pragma pack(pop)
static void invoke(ABI*c){register ABI*args __asm__("a2")=c;__asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l #0x7b7b7b7b,-(%%sp)\n\tmove.w 12(%%a2),-(%%sp)\n\tmove.l 4(%%a2),-(%%sp)\n\tmove.l 8(%%a2),-(%%sp)\n\tmove.l (%%a2),%%d0\n\tmove.w 14(%%a2),%%ccr\n\t.word 0xa97c\n\t.word 0x42c3\n\tmove.l %%d0,(%%a2)\n\tmove.w %%d3,16(%%a2)\n\tmove.l (%%sp)+,18(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,22(%%a2)\n\tmove.l (%%sp)+,26(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory");}
static void hex(const unsigned char*b,short n){short i;p+=sprintf(p,"\"");for(i=0;i<n;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
int main(void){short which,storage,seed,k,index=0,file,ids[]={128,129,32760,130},flags[]={16,31,0};long n;OSErr e;GrafPtr original;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();TEInit();InitDialogs(0);GetPort(&original);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"new-dialog-raw-v1\",\"records\":[");
 for(which=0;which<4;which++)for(storage=0;storage<2;storage++)for(seed=0;seed<2;seed++)for(k=0;k<3;k++){
  ABI c;DialogPtr d;GrafPtr after;WindowPtr front=FrontWindow();short mem,res,i;unsigned char *buffer=0;unsigned long prefix=0,suffix=0;
  SetPort(original);if(storage){buffer=(unsigned char*)NewPtr(sizeof(DialogRecord)+8);if(!buffer)return 2;memset(buffer,0xa5,sizeof(DialogRecord)+8);*(unsigned long*)buffer=0xa1b2c3d4;*(unsigned long*)(buffer+4+sizeof(DialogRecord))=0x10203040;}
  c.d0=seed?0:0x12345678;c.dst=storage?(unsigned long)(buffer+4):0;c.behind=which==1?0:0xffffffff;c.id=ids[which];c.input=flags[k];*(volatile short*)0x220=-123;*(volatile short*)0xa60=-192;invoke(&c);mem=*(volatile short*)0x220;res=*(volatile short*)0xa60;GetPort(&after);d=(DialogPtr)c.result;
  if(buffer){prefix=*(unsigned long*)buffer;suffix=*(unsigned long*)(buffer+4+sizeof(DialogRecord));}
  p+=sprintf(p,"%s{\"which\":%d,\"storage\":%d,\"input_d0\":%lu,\"input_ccr\":%d,\"result_address\":%lu,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"mem_error\":%d,\"res_error\":%d,\"current_port_preserved\":%d,\"front_changed\":%d,\"dst_reused\":%d,\"prefix\":%lu,\"suffix\":%lu",index++?",":"",which,storage,seed?0:0x12345678,flags[k],c.result,c.d0,c.ccr&31,c.delta,c.guard,mem,res,after==original,FrontWindow()!=front,d&&storage&&(unsigned long)d==c.dst,prefix,suffix);
  if(d){WindowPeek w=(WindowPeek)d;Str255 title;GetWTitle((WindowPtr)d,title);p+=sprintf(p,",\"window_kind\":%d,\"visible\":%d,\"hilited\":%d,\"go_away\":%d,\"refcon\":%ld,\"port_version\":%u,\"port_rect\":[%d,%d,%d,%d],\"edit_field\":%d,\"edit_open\":%d,\"default_item\":%d,\"items_nonnull\":%d,\"textedit_nonnull\":%d,\"title_hex\":",w->windowKind,w->visible,w->hilited,w->goAwayFlag,GetWRefCon((WindowPtr)d),((CGrafPtr)d)->portVersion,((DialogPeek)d)->window.port.portRect.top,((DialogPeek)d)->window.port.portRect.left,((DialogPeek)d)->window.port.portRect.bottom,((DialogPeek)d)->window.port.portRect.right,((DialogPeek)d)->editField,((DialogPeek)d)->editOpen,((DialogPeek)d)->aDefItem,((DialogPeek)d)->items!=0,((DialogPeek)d)->textH!=0);hex(title+1,title[0]);p+=sprintf(p,",\"items\":[");
   for(i=1;((DialogPeek)d)->items&&i<=(*(short*)*((DialogPeek)d)->items)+1;i++){short type;Handle h;Rect r;Str255 text;GetDialogItem(d,i,&type,&h,&r);if((type&127)==8||(type&127)==16)GetDialogItemText(h,text);else GetControlTitle((ControlHandle)h,text);p+=sprintf(p,"%s{\"kind\":%d,\"nonnull\":%d,\"rect\":[%d,%d,%d,%d],\"text_hex\":",i>1?",":"",type,h!=0,r.top,r.left,r.bottom,r.right);hex(text+1,text[0]);p+=sprintf(p,"}");}p+=sprintf(p,"]");if(storage)CloseDialog(d);else DisposeDialog(d);
  }
  p+=sprintf(p,"}");if(buffer)DisposePtr((Ptr)buffer);SetPort(original);
 }
 p+=sprintf(p,"]}\n");e=Create("\pNewDialogProbe.json",0,'nPrb','TEXT');if(e)return 3;if(FSOpen("\pNewDialogProbe.json",0,&file))return 4;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
