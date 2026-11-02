#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[32768],*p=output;
static void rect(const char *name,Rect r){p+=sprintf(p,",\"%s\":[%d,%d,%d,%d]",name,r.top,r.left,r.bottom,r.right);}
static void snapshot(short id,short step,WindowPtr w,Handle source,GrafPtr old){
 AuxWinHandle a=0;Boolean found=GetAuxWin(w,&a);CGrafPtr c=(CGrafPtr)w;GrafPtr current;short i,n=0;GetPort(&current);
 p+=sprintf(p,"{\"id\":%d,\"step\":%d,\"found\":%d,\"aux_nonnull\":%d,\"owner_same\":%d,\"aux_flags\":%lu,\"aux_refcon\":%ld,\"aux_reserved_nonnull\":%d,\"table_source_same\":%d,\"table_size\":%d,\"table_flags\":%u,\"current_unchanged\":%d,\"visible\":%d,\"variant\":%d,\"bg\":[%u,%u,%u],\"fg\":[%u,%u,%u],\"table\":[",id,step,found,a!=0,a&&(*a)->awOwner==w,a?(unsigned long)(*a)->awFlags:0,a?(*a)->awRefCon:0,a&&(*a)->awReserved!=0,a&&(Handle)(*a)->awCTable==source,a&&(*a)->awCTable?(**(*a)->awCTable).ctSize:-1,a&&(*a)->awCTable?(unsigned short)(**(*a)->awCTable).ctFlags:0,current==old,((WindowPeek)w)->visible!=0,GetWVariant(w),c->rgbBkColor.red,c->rgbBkColor.green,c->rgbBkColor.blue,c->rgbFgColor.red,c->rgbFgColor.green,c->rgbFgColor.blue);
 if(a&&(*a)->awCTable){n=(**(*a)->awCTable).ctSize+1;if(n>32)n=32;for(i=0;i<n;i++){ColorSpec *color=&(**(*a)->awCTable).ctTable[i];p+=sprintf(p,"%s[%u,%u,%u,%u]",i?",":"",(unsigned short)color->value,color->rgb.red,color->rgb.green,color->rgb.blue);}}
 p+=sprintf(p,"]");rect("port_rect",c->portRect);rect("bitmap",(**c->portPixMap).bounds);rect("structure",(**((WindowPeek)w)->strucRgn).rgnBBox);rect("content",(**((WindowPeek)w)->contRgn).rgnBBox);rect("visible_region",(**c->visRgn).rgnBBox);p+=sprintf(p,"}");
}
int main(void){GDHandle device;short oldDepth,id,step,file,index=0;WindowPtr w;GrafPtr old;Handle source,replacement;long n;OSErr err;RGBColor rgb;Rect r={0,0,120,260};
 InitGraf(&qd.thePort);InitFonts();InitWindows();device=GetMainDevice();oldDepth=(**(*device)->gdPMap).pixelSize;if(SetDepth(device,8,0,0))return 7;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"window-colors-raw-v3\",\"cases\":[");
 for(id=22000;id<22003;id++){
  GetPort(&old);source=Get1Resource('wctb',id);w=(WindowPtr)GetNewCWindow(id,0,(WindowPtr)-1);if(!w)return 5;
  for(step=0;step<2;step++){
   if(step){replacement=Get1Resource('wctb',22003);SetWinColor(w,(CTabHandle)replacement);source=replacement;}
   if(index++)p+=sprintf(p,",");snapshot(id,step,w,source,old);
  }
  ShowWindow(w);SetPort(w);p+=sprintf(p,",");snapshot(id,2,w,source,w);EraseRect(&r);GetCPixel(10,10,&rgb);p+=sprintf(p,",{\"id\":%d,\"step\":3,\"erased_rgb\":[%u,%u,%u]}",id,rgb.red,rgb.green,rgb.blue);
  SetPort(old);DisposeWindow(w);
 }
 err=SetDepth(device,oldDepth,0,0);p+=sprintf(p,"],\"restore_error\":%d}\n",err);err=Create("\pWindowColorsProbe.json",0,'pPrb','TEXT');if(err)return 2;if(FSOpen("\pWindowColorsProbe.json",0,&file))return 3;n=p-output;err=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
