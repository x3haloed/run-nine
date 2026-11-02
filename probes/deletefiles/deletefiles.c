#include <QuickDraw.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static short last_result;
static OSErr remove_file(short volume,long directory){HParamBlockRec pb;OSErr e;memset(&pb,0,sizeof(pb));pb.ioParam.ioNamePtr=(StringPtr)"\pFileDelete.data";pb.ioParam.ioVRefNum=volume;pb.fileParam.ioDirID=directory;e=PBHDeleteSync(&pb);last_result=pb.ioParam.ioResult;return e;}
int main(void){char out[4096],*p=out;short f,rf,result;long n,eof;OSErr err;FInfo info;
 Create("\pFileDelete.data",0,'ttxt','TEXT');FSOpen("\pFileDelete.data",0,&f);n=4;FSWrite(f,&n,"DATA");OpenRF("\pFileDelete.data",0,&rf);n=2;FSWrite(rf,&n,"RS");FSClose(rf);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"file-delete-v1\",\"files\":{");err=remove_file(0,0);p+=sprintf(p,"\"open_delete\":[%d,%d]",err,last_result);FSClose(f);
 err=remove_file(1234,0);p+=sprintf(p,",\"bad_volume\":[%d,%d]",err,last_result);err=remove_file(0,0x12345678);p+=sprintf(p,",\"bad_directory\":[%d,%d]",err,last_result);
 err=remove_file(0,0);p+=sprintf(p,",\"closed_delete\":[%d,%d]",err,last_result);err=GetFInfo("\pFileDelete.data",0,&info);p+=sprintf(p,",\"deleted_info\":%d",err);err=FSOpen("\pFileDelete.data",0,&f);p+=sprintf(p,",\"deleted_data\":%d",err);err=OpenRF("\pFileDelete.data",0,&rf);p+=sprintf(p,",\"deleted_resource\":%d",err);
 err=remove_file(0,0);p+=sprintf(p,",\"missing_delete\":[%d,%d]",err,last_result);Create("\pFileDelete.data",0,'ttxt','TEXT');FSOpen("\pFileDelete.data",0,&f);GetEOF(f,&eof);FSClose(f);p+=sprintf(p,",\"recreate_eof\":[%ld",eof);OpenRF("\pFileDelete.data",0,&rf);GetEOF(rf,&eof);FSClose(rf);p+=sprintf(p,",%ld]}}\n",eof);
 err=Create("\pFileDeleteProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pFileDeleteProbe.json",0,&result))return 3;SetEOF(result,0);n=p-out;err=FSWrite(result,&n,out);FSClose(result);FlushVol(0,0);return err?4:0;}
