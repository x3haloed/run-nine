#include <QuickDraw.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
int main(void){
 char out[8192],bytes[16]={0},resource[4]={0};short f,rf,ro,result_file;long n,pos,eof;char *p=out;OSErr err,create,duplicate,missing;
 FInfo info;HParamBlockRec pb;
 create=Create("\pFileProbe.data",0,'ttxt','TEXT');duplicate=Create("\pFileProbe.data",0,'ttxt','TEXT');missing=FSOpen("\pNoSuchFileProbe.data",0,&f);
 if(FSOpen("\pFileProbe.data",0,&f))return 5;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"files-v1\",\"files\":{\"create\":%d,\"duplicate\":%d,\"missing\":%d",create,duplicate,missing);
 n=6;err=FSWrite(f,&n,"abcdef");GetFPos(f,&pos);p+=sprintf(p,",\"write\":[%d,%ld,%ld]",err,n,pos);
 SetFPos(f,fsFromStart,2);n=2;err=FSWrite(f,&n,"XY");GetFPos(f,&pos);p+=sprintf(p,",\"overwrite\":[%d,%ld,%ld]",err,n,pos);
 SetFPos(f,fsFromLEOF,-1);n=4;err=FSRead(f,&n,bytes);GetFPos(f,&pos);p+=sprintf(p,",\"partial\":[%d,%ld,%ld,%u]",err,n,pos,(unsigned char)bytes[0]);
 GetEOF(f,&eof);err=SetEOF(f,3);GetFPos(f,&pos);p+=sprintf(p,",\"truncate\":[%ld,%d,%ld]",eof,err,pos);
 SetFPos(f,fsFromLEOF,0);n=1;FSWrite(f,&n,"z");FSClose(f);
 FSOpen("\pFileProbe.data",0,&f);n=8;memset(bytes,0,sizeof(bytes));err=FSRead(f,&n,bytes);p+=sprintf(p,",\"reopen\":[%d,%ld,%u,%u,%u,%u]",err,n,(unsigned char)bytes[0],(unsigned char)bytes[1],(unsigned char)bytes[2],(unsigned char)bytes[3]);FSClose(f);
 OpenRF("\pFileProbe.data",0,&rf);n=2;FSWrite(rf,&n,"RS");GetEOF(rf,&eof);SetFPos(rf,fsFromStart,0);n=4;err=FSRead(rf,&n,resource);p+=sprintf(p,",\"resource\":[%ld,%d,%ld,%u,%u]",eof,err,n,(unsigned char)resource[0],(unsigned char)resource[1]);FSClose(rf);
 GetFInfo("\pFileProbe.data",0,&info);p+=sprintf(p,",\"finder\":[%lu,%lu]",info.fdType,info.fdCreator);
 memset(&pb,0,sizeof(pb));pb.ioParam.ioNamePtr=(StringPtr)"\pFileProbe.data";pb.ioParam.ioPermssn=fsRdPerm;err=PBHOpenSync(&pb);ro=pb.ioParam.ioRefNum;
 n=1;err=FSWrite(ro,&n,"!");p+=sprintf(p,",\"readonly_write\":[%d,%ld]",err,n);FSClose(ro);
 FSOpen("\pFileProbe.data",0,&f);err=SetFPos(f,fsFromStart,-1);GetFPos(f,&pos);p+=sprintf(p,",\"negative_seek\":[%d,%ld]",err,pos);
 err=SetFPos(f,fsFromStart,20);GetFPos(f,&pos);p+=sprintf(p,",\"past_eof_seek\":[%d,%ld]",err,pos);FSClose(f);
 err=GetEOF(32000,&eof);p+=sprintf(p,",\"bad_ref\":%d}}\n",err);
 err=Create("\pFileProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pFileProbe.json",0,&result_file))return 3;
 SetEOF(result_file,0);n=p-out;err=FSWrite(result_file,&n,out);FSClose(result_file);FlushVol(0,0);return err?4:0;
}
