#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[65536],*p=output;
static void hex(const char *key,const void *data,long size){const unsigned char*b=data;long i;p+=sprintf(p,",\"%s\":\"",key);if(size>=0&&size<=4096)for(i=0;i<size;i++)p+=sprintf(p,"%02x",b[i]);p+=sprintf(p,"\"");}
static void snapshot(const char *key,PixMapHandle h,PixMapHandle device){PixMap *pm=*h;p+=sprintf(p,",\"%s\":{\"size\":%ld,\"state\":%u,\"base_zero\":%d,\"base_is_device\":%d,\"table_present\":%d,\"table_is_device\":%d,\"table_size\":%ld,\"table_state\":%d",key,GetHandleSize((Handle)h),(unsigned char)HGetState((Handle)h),pm->baseAddr==0,pm->baseAddr==(*device)->baseAddr,pm->pmTable!=0,pm->pmTable==(*device)->pmTable,pm->pmTable?GetHandleSize((Handle)pm->pmTable):0L,pm->pmTable?(unsigned char)HGetState((Handle)pm->pmTable):-1);hex("map_hex",pm,50);hex("table_hex",pm->pmTable?*pm->pmTable:0,pm->pmTable?GetHandleSize((Handle)pm->pmTable):0L);p+=sprintf(p,"}");}
#pragma pack(push,2)
typedef struct{PixMapHandle source,dest,result;unsigned short input,ccr;unsigned long d0;long delta;unsigned long guard;} ABI;
#pragma pack(pop)
static void call(ABI*c,short op){register ABI*args __asm__("a2")=c;register short selector __asm__("d4")=op;
 if(op==0){__asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l #0x7b7b7b7b,-(%%sp)\n\tmove.l %%sp,%%a3\n\tmove.l #0x12345678,%%d0\n\tmove.w 12(%%a2),%%ccr\n\t.word 0xaa03\n\t.word 0x42c3\n\tmove.l %%d0,16(%%a2)\n\tmove.w %%d3,14(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,20(%%a2)\n\tmove.l (%%sp)+,8(%%a2)\n\tmove.l (%%sp)+,24(%%a2)":"+a"(args)::"d0","d1","d2","d3","a0","a1","a3","cc","memory");}
 else{__asm__ volatile("move.l #0x55667788,-(%%sp)\n\tmove.l (%%a2),-(%%sp)\n\tcmpi.w #1,%%d4\n\tbeq 1f\n\tmove.l 4(%%a2),-(%%sp)\n1:\tmove.l %%sp,%%a3\n\tmove.l #0x12345678,%%d0\n\tmove.w 12(%%a2),%%ccr\n\tcmpi.w #1,%%d4\n\tbeq 2f\n\tmove.w 12(%%a2),%%ccr\n\t.word 0xaa05\n\tbra 3f\n2:\tmove.w 12(%%a2),%%ccr\n\t.word 0xaa04\n3:\t.word 0x42c3\n\tmove.l %%d0,16(%%a2)\n\tmove.w %%d3,14(%%a2)\n\tmove.l %%sp,%%d2\n\tsub.l %%a3,%%d2\n\tmove.l %%d2,20(%%a2)\n\tmove.l (%%sp)+,24(%%a2)":"+a"(args):"d"(selector):"d0","d1","d2","d3","a0","a1","a3","cc","memory");}
}
static void abi(const char*key,ABI*c,short error){p+=sprintf(p,",\"%s\":{\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"guard\":%lu,\"memory_error\":%d}",key,c->d0,c->ccr&31,c->delta,c->guard,error);}
int main(void){short depths[]={8,32},flags[]={16,31,0},i,k,index=0,file,old;GDHandle gd;PixMapHandle device,warm;long count,before;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();gd=GetGDevice();device=(*gd)->gdPMap;old=(*device)->pixelSize;
 warm=NewPixMap();DisposePixMap(warm);warm=NewPixMap();DisposePixMap(warm);{CTabHandle table=GetCTable(128);DisposeCTable(table);}
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"pixmap-raw-v1\",\"records\":[");
 for(i=0;i<2;i++){if(SetDepth(gd,depths[i],0,0))return 2;for(k=0;k<3;k++){
  ABI c;PixMapHandle h,other;CTabHandle oldtable;Ptr pixels=NewPtr(64);short error,copy_error,dispose_error,base_size;long j;for(j=0;j<64;j++)pixels[j]=j^0x5a;before=FreeMem();c.input=flags[k];call(&c,0);error=MemError();h=c.result;if(!h)return 3;
  p+=sprintf(p,"%s{\"depth\":%d,\"input_ccr\":%d",index++?",":"",depths[i],flags[k]);abi("new_abi",&c,error);snapshot("initial",h,device);
  other=NewPixMap();if(!other)return 4;oldtable=(*other)->pmTable;
  if((*h)->pmTable)DisposeCTable((*h)->pmTable);(*h)->pmTable=GetCTable(128);if(!(*h)->pmTable)return 5;(**(*h)->pmTable).ctSeed=0x10203040;
  (*h)->baseAddr=pixels;(*h)->rowBytes=0x8008;SetRect(&(*h)->bounds,2,1,4,3);(*h)->hRes=90L<<16;(*h)->vRes=96L<<16;(*h)->pixelSize=8;(*h)->cmpCount=1;(*h)->cmpSize=8;
  c.source=h;c.dest=other;c.input=flags[k];call(&c,2);copy_error=MemError();abi("copy_abi",&c,copy_error);snapshot("copied",other,device);
  p+=sprintf(p,",\"distinct_maps\":%d,\"distinct_tables\":%d,\"destination_table_reused\":%d,\"copied_base_same\":%d",h!=other,(*h)->pmTable!=(*other)->pmTable,oldtable==(*other)->pmTable,(*other)->baseAddr==pixels);
  (*(*h)->pmTable)->ctTable[0].rgb.red=0x7777;hex("copy_table_after_source_mutation",*(*other)->pmTable,GetHandleSize((Handle)(*other)->pmTable));
  c.source=other;c.input=flags[k];call(&c,1);dispose_error=MemError();abi("dispose_abi",&c,dispose_error);DisposePixMap(h);base_size=GetPtrSize(pixels);
  p+=sprintf(p,",\"base_size_after_dispose\":%d,\"free_balanced\":%d",base_size,FreeMem()==before);hex("base_bytes_after_dispose",pixels,64);DisposePtr(pixels);p+=sprintf(p,"}");
 }}
 p+=sprintf(p,"],\"nil_dispose\":[");for(k=0;k<3;k++){ABI c;c.source=0;c.input=flags[k];call(&c,1);e=MemError();p+=sprintf(p,"%s{\"input_ccr\":%d",k?",":"",flags[k]);abi("abi",&c,e);p+=sprintf(p,"}");}p+=sprintf(p,"]}\n");SetDepth(gd,old,0,0);
 e=Create("\pPixMapProbe.json",0,'pPrb','TEXT');if(e)return 6;if(FSOpen("\pPixMapProbe.json",0,&file))return 7;count=p-output;e=FSWrite(file,&count,output);FSClose(file);FlushVol(0,0);return e?8:0;
}
