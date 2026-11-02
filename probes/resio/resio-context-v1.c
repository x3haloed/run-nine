#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char out[8192],raw[8192];
static short write_file(ConstStr255Param name,char *bytes){short f;long n=strlen(bytes);OSErr e=Create(name,0,'ttxt','TEXT');if(e)return e;if(FSOpen(name,0,&f))return 1;e=FSWrite(f,&n,bytes);FSClose(f);return e;}
int main(void){FSSpec a,b,empty,corrupt,missing;short app=CurResFile(),ar,br,again,rr,f,openerr[3],same[3],i,id;OSErr e;Handle h,found;ResType kind;Str255 name;char *p=out,*r=raw;long n;unsigned char fork[2048];short attr_added,attr_written,attr_read;short add_error,change_error,write_error,update_error,create_b_same;short empty_before[2],b_first[2],b_search[2],app_search[2],a_first[2],close_b[2],close_a[2],read_open[3],read_info_error;short first_open[3],repeated[3];
 FSMakeFSSpec(0,0,"\pResourceA",&a);FSMakeFSSpec(0,0,"\pResourceB",&b);FSMakeFSSpec(0,0,"\pResourceEmpty",&empty);FSMakeFSSpec(0,0,"\pResourceCorrupt",&corrupt);FSMakeFSSpec(0,0,"\pResourceMissing",&missing);
 FSpCreateResFile(&a,'IoPr','TEXT',smSystemScript);if(ResError())return 2;
 ar=FSpOpenResFile(&a,fsRdWrPerm);first_open[0]=ar!=-1;first_open[1]=ResError();first_open[2]=CurResFile()==ar;if(ar==-1)return 3;
 found=Get1Resource('TEST',128);empty_before[0]=found!=0;empty_before[1]=ResError();
 h=NewHandle(4);if(!h)return 4;memcpy(*h,"AB\0!",4);AddResource(h,'TEST',128,"\pOwned");add_error=ResError();attr_added=GetResAttrs(h);
 found=Get1Resource('TEST',128);a_first[0]=found==h;a_first[1]=HomeResFile(h)==ar;
 SetHandleSize(h,6);if(MemError())return 5;memcpy(*h,"WXYZ!\0",6);ChangedResource(h);change_error=ResError();WriteResource(h);write_error=ResError();attr_written=GetResAttrs(h);UpdateResFile(ar);update_error=ResError();
 FSpCreateResFile(&b,'IoPr','TEXT',smSystemScript);if(ResError())return 6;create_b_same=CurResFile()==ar;br=FSpOpenResFile(&b,fsRdWrPerm);if(br==-1)return 7;
 found=Get1Resource('TEST',128);b_first[0]=found!=0;b_first[1]=ResError();found=GetResource('TEST',128);b_search[0]=found==h;b_search[1]=ResError();
 again=FSpOpenResFile(&a,fsRdWrPerm);repeated[0]=again==ar;repeated[1]=ResError();repeated[2]=CurResFile()==br;
 UseResFile(app);e=ResError();found=GetResource('TEST',128);app_search[0]=found!=0;app_search[1]=ResError();if(e)return 8;
 UseResFile(ar);if(ResError())return 9;CloseResFile(br);close_b[0]=ResError();close_b[1]=CurResFile()==ar;CloseResFile(ar);close_a[0]=ResError();close_a[1]=CurResFile()==app;
 rr=FSpOpenResFile(&a,fsRdPerm);read_open[0]=rr!=-1;read_open[1]=ResError();read_open[2]=CurResFile()==rr;if(rr==-1)return 10;
 found=Get1Resource('TEST',128);if(!found)return 11;attr_read=GetResAttrs(found);GetResInfo(found,&id,&kind,name);read_info_error=ResError();
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"res-io-v1\",\"resources\":{\"first_open\":[%d,%d,%d],\"empty_before\":[%d,%d],\"add\":[%d,%d,%d,%d],\"write\":[%d,%d,%d,%d],\"second_create_current_same\":%d,\"second_get1\":[%d,%d],\"second_search\":[%d,%d],\"repeated_open\":[%d,%d,%d],\"app_search\":[%d,%d],\"close_other\":[%d,%d],\"close_current\":[%d,%d],\"read_open\":[%d,%d,%d],\"read_info\":[%d,%d,%lu,%d],\"name_hex\":\"",first_open[0],first_open[1],first_open[2],empty_before[0],empty_before[1],add_error,attr_added,a_first[0],a_first[1],change_error,write_error,attr_written,update_error,create_b_same,b_first[0],b_first[1],b_search[0],b_search[1],repeated[0],repeated[1],repeated[2],app_search[0],app_search[1],close_b[0],close_b[1],close_a[0],close_a[1],read_open[0],read_open[1],read_open[2],read_info_error,id,(unsigned long)kind,attr_read);
 for(i=0;i<name[0];i++)p+=sprintf(p,"%02x",name[i+1]);p+=sprintf(p,"\",\"data_hex\":\"");n=GetHandleSize(found);for(i=0;i<n;i++)p+=sprintf(p,"%02x",(unsigned char)(*found)[i]);p+=sprintf(p,"\"");CloseResFile(rr);
 if(FSpCreate(&empty,'IoPr','TEXT',smSystemScript)||FSpCreate(&corrupt,'IoPr','TEXT',smSystemScript))return 12;if(FSpOpenRF(&corrupt,fsRdWrPerm,&f))return 13;n=4;FSWrite(f,&n,"bad!");FSClose(f);
 {FSSpec *s[3];s[0]=&missing;s[1]=&empty;s[2]=&corrupt;for(i=0;i<3;i++){f=FSpOpenResFile(s[i],fsRdPerm);openerr[i]=ResError();same[i]=CurResFile()==app;if(f!=-1)return 14;}}
 p+=sprintf(p,",\"failed_open\":[[%d,%d],[%d,%d],[%d,%d]]}}\n",openerr[0],same[0],openerr[1],same[1],openerr[2],same[2]);
 r+=sprintf(r,"{\"refs\":[%d,%d,%d,%d,%d],\"fork_hex\":\"",app,ar,br,again,rr);if(FSpOpenRF(&a,fsRdPerm,&f))return 15;GetEOF(f,&n);if(n>sizeof(fork))return 16;FSRead(f,&n,fork);FSClose(f);for(i=0;i<n;i++)r+=sprintf(r,"%02x",fork[i]);r+=sprintf(r,"\"}\n");
 if(write_file("\pResIOProbe.json",out)||write_file("\pResIOProbe-raw.json",raw))return 17;FlushVol(0,0);return 0;
}
