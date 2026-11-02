#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[32768],*p=output;
static void snapshot(GDHandle device) {
 PixMapPtr pm=*(*device)->gdPMap;
 p+=sprintf(p,"{\"flags\":%u,\"type\":%d,\"depth\":%d,\"pixel_type\":%d,\"components\":%d,\"component_size\":%d,\"row_bytes\":%u,\"bounds\":[%d,%d,%d,%d],\"device_bounds\":[%d,%d,%d,%d],\"table_size\":%d,\"table_flags\":%u}",(unsigned short)(*device)->gdFlags,(*device)->gdType,pm->pixelSize,pm->pixelType,pm->cmpCount,pm->cmpSize,(unsigned short)pm->rowBytes,pm->bounds.top,pm->bounds.left,pm->bounds.bottom,pm->bounds.right,(*device)->gdRect.top,(*device)->gdRect.left,(*device)->gdRect.bottom,(*device)->gdRect.right,pm->pmTable?(**pm->pmTable).ctSize:-1,pm->pmTable?(unsigned short)(**pm->pmTable).ctFlags:0);
}

#pragma pack(push,2)
typedef struct {GDHandle device;short depth,which,flags,input,result,ccr;long delta;unsigned long guard;} ABI;
#pragma pack(pop)
static void abi(ABI *c,short selector) {
 register ABI *args __asm__("a2")=c;register short op __asm__("d4")=selector;
 __asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.w #0x7b7b,-(%%sp)\n\tmove.l (%%a2),-(%%sp)\n\tmove.w 4(%%a2),-(%%sp)\n\tmove.w 6(%%a2),-(%%sp)\n\tmove.w 8(%%a2),-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.w %%d4,%%d0\n\tmove.w 10(%%a2),%%ccr\n\t.word 0xaaa2\n\t.word 0x42c3\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,16(%%a2)\n\tmove.w (%%sp)+,12(%%a2)\n\tmove.w %%d3,14(%%a2)\n\tmove.l (%%sp)+,20(%%a2)"
 : "+a"(args):"d"(op):"d0","d1","d2","d3","a0","a1","a3","cc","memory");
}
static void indexed(GDHandle device) {
 short i,j;CGrafPort port;Rect r={40,20,42,28};PixMapPtr pm;unsigned char *base;ColorSpec colors[4];RGBColor rgb;long index;
 p+=sprintf(p,",\"indexed\":{");
 if(SetDepth(device,8,0,0)){p+=sprintf(p,"\"available\":0}");return;}
 OpenCPort(&port);pm=*port.portPixMap;base=(unsigned char*)pm->baseAddr;
 p+=sprintf(p,"\"available\":1,\"default_palette\":[");
 for(i=0;i<256;i++){ColorSpec *c=&(**pm->pmTable).ctTable[i];p+=sprintf(p,"%s[%u,%u,%u]",i?",":"",c->rgb.red,c->rgb.green,c->rgb.blue);}
 for(i=0;i<4;i++){colors[i].value=16+i;colors[i].rgb.red=0x1234+i*0x2345;colors[i].rgb.green=0x9876-i*0x1234;colors[i].rgb.blue=0x5678+i*0x1111;}
 SetEntries(16,3,colors);p+=sprintf(p,"],\"custom_entries\":[");
 for(i=0;i<4;i++){ColorSpec *c=&(**pm->pmTable).ctTable[16+i];p+=sprintf(p,"%s[%u,%u,%u]",i?",":"",c->rgb.red,c->rgb.green,c->rgb.blue);}
 p+=sprintf(p,"],\"exact_indices\":[");
 for(i=0;i<4;i++){index=Color2Index(&colors[i].rgb);p+=sprintf(p,"%s%ld",i?",":"",index);}
 p+=sprintf(p,"],\"paints\":[");
 for(i=0;i<3;i++) {
  PenMode(patCopy);RGBForeColor(&colors[0].rgb);PaintRect(&r);
  if(i){RGBForeColor(&colors[1].rgb);PenMode(i==1?patXor:patBic);PaintRect(&r);}
  p+=sprintf(p,"%s[",i?",":"");for(j=0;j<8;j++)p+=sprintf(p,"%s%u",j?",":"",base[40*(pm->rowBytes&0x3fff)+20+j]);p+=sprintf(p,"]");
 }
 SetCPixel(22,40,&colors[2].rgb);GetCPixel(22,40,&rgb);
 p+=sprintf(p,"],\"set_pixel\":{\"index\":%u,\"rgb\":[%u,%u,%u]}",base[40*(pm->rowBytes&0x3fff)+22],rgb.red,rgb.green,rgb.blue);
 {
  struct {unsigned long before;unsigned char data[8];unsigned long after;} source={0x11223344,{16,17,0xaa,0xbb,18,19,0xcc,0xdd},0x55667788};
  PixMap map=*pm;Rect sr={0,0,2,2},dr={44,20,46,22};map.baseAddr=(Ptr)source.data;map.rowBytes=0x8004;map.bounds=sr;
  CopyBits((BitMap*)&map,(BitMap*)pm,&sr,&dr,srcCopy,0);
  p+=sprintf(p,",\"copy_pixels\":[%u,%u,%u,%u],\"copy_source\":[%lu,%u,%u,%u,%u,%lu]",base[44*(pm->rowBytes&0x3fff)+20],base[44*(pm->rowBytes&0x3fff)+21],base[45*(pm->rowBytes&0x3fff)+20],base[45*(pm->rowBytes&0x3fff)+21],source.before,source.data[2],source.data[3],source.data[6],source.data[7],source.after);
 }
 {
  RGBColor black={0,0,0},white={65535,65535,65535};short mode,bit;
  struct {unsigned long before;unsigned char data[8];unsigned long after;} source={0x11223344,{16,17,0xaa,0xbb,18,19,0xcc,0xdd},0x55667788};
  PixMap map=*pm;Rect sr={0,0,2,2},dr={48,20,50,22};map.baseAddr=(Ptr)source.data;map.rowBytes=0x8004;map.bounds=sr;
  RGBForeColor(&black);RGBBackColor(&white);CopyBits((BitMap*)&map,(BitMap*)pm,&sr,&dr,srcCopy,0);
  p+=sprintf(p,",\"neutral_copy_pixels\":[%u,%u,%u,%u]",base[48*(pm->rowBytes&0x3fff)+20],base[48*(pm->rowBytes&0x3fff)+21],base[49*(pm->rowBytes&0x3fff)+20],base[49*(pm->rowBytes&0x3fff)+21]);
  p+=sprintf(p,",\"rop_modes\":[");
  for(mode=0;mode<8;mode++)for(bit=0;bit<2;bit++) {
   PenPat(&qd.black);PenMode(patCopy);RGBForeColor(&colors[0].rgb);PaintRect(&r);
   RGBForeColor(&colors[1].rgb);RGBBackColor(&colors[2].rgb);PenPat(bit?&qd.black:&qd.white);PenMode(8+mode);PaintRect(&r);
   p+=sprintf(p,"%s%u",mode||bit?",":"",base[40*(pm->rowBytes&0x3fff)+20]);
  }
  p+=sprintf(p,"]");
 }
 colors[0].rgb.red=0xaaaa;colors[0].rgb.green=0xbbbb;colors[0].rgb.blue=0xcccc;SetEntries(16,0,colors);GetCPixel(20,48,&rgb);
 p+=sprintf(p,",\"recolored\":{\"index\":%u,\"rgb\":[%u,%u,%u]}}",base[48*(pm->rowBytes&0x3fff)+20],rgb.red,rgb.green,rgb.blue);
 CloseCPort(&port);
}

int main(void) {
 GDHandle device;short depths[7]={1,2,4,8,16,32,3};short i,j,k,first=1,file,oldDepth;long size;OSErr err;short which[3]={0,1,1},flags[3]={0,0,1};
 InitGraf(&qd.thePort);InitFonts();InitWindows();device=GetMainDevice();oldDepth=(**(*device)->gdPMap).pixelSize;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"device-depth-raw-v4\",\"main_is_first\":%d,\"next_null\":%d,\"current_is_main\":%d,\"initial\":",device==GetDeviceList(),GetNextDevice(device)==0,GetGDevice()==device);snapshot(device);
 p+=sprintf(p,",\"queries\":[");
 for(i=0;i<7;i++)for(j=0;j<3;j++) {
  short mode=HasDepth(device,depths[i],which[j],flags[j]);
  p+=sprintf(p,"%s{\"depth\":%d,\"which\":%d,\"flags\":%d,\"mode\":%d}",first?"":",",depths[i],which[j],flags[j],mode);first=0;
 }
 p+=sprintf(p,"],\"abi\":[");first=1;
 for(i=0;i<2;i++)for(j=0;j<2;j++)for(k=0;k<3;k++) {
  ABI c={device,j?3:8,0,0,k==0?16:k==1?31:0,0,0,0,0};abi(&c,i?0x0a13:0x0a14);
  p+=sprintf(p,"%s{\"selector\":%d,\"depth\":%d,\"input_ccr\":%d,\"result\":%d,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu}",first?"":",",i?0x0a13:0x0a14,c.depth,c.input,c.result,c.ccr&31,c.delta,c.guard);first=0;
 }
 p+=sprintf(p,"],\"changes\":[");first=1;
 /* Only supported modes are imposed on this disposable reference overlay. */
 for(i=0;i<6;i++) {
  short mode=HasDepth(device,depths[i],1,1);if(!mode)continue;
  err=SetDepth(device,depths[i],1,1);
  p+=sprintf(p,"%s{\"request\":%d,\"by_mode\":0,\"error\":%d,\"state\":",first?"":",",depths[i],err);snapshot(device);p+=sprintf(p,"}");first=0;
  if(!err){err=SetDepth(device,mode,1,1);p+=sprintf(p,",{\"request\":%d,\"by_mode\":1,\"error\":%d,\"state\":",depths[i],err);snapshot(device);p+=sprintf(p,"}");}
 }
 p+=sprintf(p,"]");indexed(device);
 err=SetDepth(device,oldDepth,0,0);p+=sprintf(p,",\"restore_error\":%d,\"restored\":",err);snapshot(device);p+=sprintf(p,"}\n");
 err=Create("\pDeviceDepthProbe.json",0,'pPrb','TEXT');if(err)return 2;if(FSOpen("\pDeviceDepthProbe.json",0,&file))return 3;size=p-output;err=FSWrite(file,&size,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
