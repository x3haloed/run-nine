#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char out[24576],*p=out;static short volume;static long directory;static long baseline_id;
static void row(short index,short ref,short v,short enumeration,short nil_name,short app){
 struct {unsigned char before[8];FCBPBRec pb;unsigned char after[8];} state;Str255 name;OSErr e;short i,guards=1;FCBPBRec *b=&state.pb;
 memset(&state,0x55,sizeof(state));memset(name,0x55,sizeof(name));b->ioCompletion=0;b->ioNamePtr=nil_name?0:name;b->ioVRefNum=v;b->ioRefNum=ref;b->ioFCBIndx=enumeration;
 e=PBGetFCBInfoSync(b);if(index==0&&!e)baseline_id=b->ioFCBFlNm;
 for(i=0;i<8;i++)if(state.before[i]!=0x55||state.after[i]!=0x55)guards=0;
 p+=sprintf(p,"%s{\"case\":%d,\"error\":%d,\"io_result\":%d,\"ref_same\":%d,\"ref_zero\":%d,\"index_out\":%d,\"volume_out\":[%d,%d,%d],\"file_id\":[%d,%d],\"flags\":%u,\"eof\":%ld,\"position\":%ld,\"file_volume_same\":%d,\"parent_same\":%d,\"fillers\":[%u,%d],\"guards\":%d,\"name33_hex\":\"",index?",":"",index,e,b->ioResult,b->ioRefNum==ref,b->ioRefNum==0,b->ioFCBIndx,b->ioVRefNum==v,b->ioVRefNum==volume,b->ioVRefNum==0,b->ioFCBFlNm>0,b->ioFCBFlNm==baseline_id,b->ioFCBFlags,b->ioFCBEOF,b->ioFCBCrPs,b->ioFCBVRefNum==volume,b->ioFCBParID==directory,b->filler,b->ioFCBobnoxiousfiller,guards);
 for(i=0;i<33;i++)p+=sprintf(p,"%02x",name[i]);
 p+=sprintf(p,"\",\"raw_physical\":[%d,%ld,%ld],\"raw_ids\":[%ld,%ld]}",b->ioFCBStBlk,b->ioFCBPLen,b->ioFCBClpSiz,b->ioFCBFlNm,b->ioFCBParID);
}
int main(void){
 short data,ro,resource,wo,closed,output,app=CurResFile();long n;OSErr e;WDPBRec wd;
 memset(&wd,0,sizeof(wd));if(PBHGetVolSync(&wd))return 2;volume=wd.ioWDVRefNum;directory=wd.ioWDDirID;
 e=Create("\pFCB-owned.data",0,'pPrb','TEXT');if(e)return 3;
 if(HOpen(volume,directory,"\pFCB-owned.data",fsRdWrPerm,&data))return 4;
 n=6;FSWrite(data,&n,"ABCDEF");FSClose(data);
 if(HOpenRF(volume,directory,"\pFCB-owned.data",fsRdWrPerm,&resource))return 5;
 n=2;FSWrite(resource,&n,"RS");FSClose(resource);
 if(HOpen(volume,directory,"\pFCB-owned.data",fsRdWrPerm,&data))return 6;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"fcb-info-v1\",\"raw_environment\":{\"volume\":%d,\"directory\":%ld,\"app_ref\":%d,\"global_ref_same\":%d,\"global_name_same\":%d},\"files\":{\"cases\":[",volume,directory,app,LMGetCurApRefNum()==app,LMGetCurApName()[0]==12&&!memcmp(LMGetCurApName()+1,"FCBInfoProbe",12));
 row(0,data,0,0,0,0);SetFPos(data,fsFromStart,2);row(1,data,volume,0,0,0);
 if(HOpen(volume,directory,"\pFCB-owned.data",fsRdPerm,&ro))return 7;SetFPos(ro,fsFromStart,3);row(2,ro,0,0,0,0);
 if(HOpenRF(volume,directory,"\pFCB-owned.data",fsRdWrPerm,&resource))return 8;SetFPos(resource,fsFromStart,1);row(3,resource,0,0,0,0);
 row(4,data,volume,0,1,0);row(5,32000,0,0,0,0);row(6,0,0,0,0,0);
 closed=ro;FSClose(ro);row(7,closed,0,0,0,0);
 row(8,data,0,-1,0,0);row(9,data,-123,0,0,0);row(10,data,-123,1,0,0);row(11,data,volume,32767,0,0);
 SetFPos(app,fsFromStart,5);row(12,app,0,0,0,1);row(13,app,-123,0,1,1);
 FSClose(resource);FSClose(data);
 if(HOpen(volume,directory,"\pFCB-owned.data",fsWrPerm,&wo))return 9;row(14,wo,0,0,0,0);SetFPos(wo,fsFromLEOF,0);n=1;FSWrite(wo,&n,"!");row(15,wo,0,0,0,0);FlushVol(0,volume);row(16,wo,0,0,0,0);FSClose(wo);
 if(HOpenRF(volume,directory,"\pFCB-owned.data",fsRdWrPerm,&resource))return 13;SetFPos(resource,fsFromLEOF,0);n=1;FSWrite(resource,&n,"!");row(17,resource,0,0,0,0);FSClose(resource);
 p+=sprintf(p,"]}}\n");e=Create("\pFCBInfoProbe.json",0,'pPrb','TEXT');if(e)return 10;if(FSOpen("\pFCBInfoProbe.json",0,&output))return 11;n=p-out;e=FSWrite(output,&n,out);FSClose(output);FlushVol(0,0);return e?12:0;
}
