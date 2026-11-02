#include <QuickDraw.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char out[8192];
static char *pstr(char *p,unsigned char *s){short i;p+=sprintf(p,"[");for(i=0;i<s[0];i++)p+=sprintf(p,"%s%u",i?",":"",s[i+1]);return p+sprintf(p,"]");}
int main(void){ProcessSerialNumber current={0,0},alias={0,2},none={0,0},bad={0x12345678,0x76543210};ProcessInfoRec info;FSSpec spec;Str255 name;Boolean same;OSErr currentErr,err;short f,i,found=0;long n;char *p=out;unsigned long lengths[]={0,1,59,60,61,64,68};
 InitGraf(&qd.thePort);currentErr=GetCurrentProcess(&current);memset(&info,0,sizeof(info));memset(&spec,0,sizeof(spec));memset(name,0,sizeof(name));info.processInfoLength=sizeof(info);info.processName=name;info.processAppSpec=&spec;err=GetProcessInformation(&current,&info);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"processes-v1\",\"processes\":{\"current_error\":%d,\"current_shape\":[%d,%d],\"info_error\":%d,\"info_length\":%lu,\"name\":",currentErr,current.highLongOfPSN==0,current.lowLongOfPSN>2,err,info.processInfoLength);p=pstr(p,name);
 p+=sprintf(p,",\"identity\":[%lu,%lu,%lu],\"number_matches\":%d,\"memory_relations\":[%d,%d,%d],\"app_name\":",info.processType,info.processSignature,info.processMode,info.processNumber.highLongOfPSN==current.highLongOfPSN&&info.processNumber.lowLongOfPSN==current.lowLongOfPSN,info.processLocation!=0,info.processSize>0,info.processFreeMem<=info.processSize);p=pstr(p,spec.name);
 p+=sprintf(p,",\"same\":[");
 same=99;err=SameProcess(&current,&current,&same);p+=sprintf(p,"[%d,%d]",err,same);same=99;err=SameProcess(&current,&alias,&same);p+=sprintf(p,",[%d,%d]",err,same);same=99;err=SameProcess(&current,&none,&same);p+=sprintf(p,",[%d,%d]",err,same);same=99;err=SameProcess(&current,&bad,&same);p+=sprintf(p,",[%d,%d]",err,same);p+=sprintf(p,"]");
 p+=sprintf(p,",\"lengths\":[");for(i=0;i<7;i++){memset(&info,0,sizeof(info));info.processInfoLength=lengths[i];err=GetProcessInformation(&current,&info);p+=sprintf(p,"%s[%lu,%d,%lu]",i?",":"",lengths[i],err,info.processType);}p+=sprintf(p,"]");
 memset(&info,0,sizeof(info));info.processInfoLength=sizeof(info);err=GetProcessInformation(&bad,&info);p+=sprintf(p,",\"bad_info\":[%d,%lu]",err,info.processType);
 err=GetNextProcess(&bad);p+=sprintf(p,",\"bad_next\":[%d,%d,%d]",err,bad.highLongOfPSN==0,bad.lowLongOfPSN>2);
 none.highLongOfPSN=none.lowLongOfPSN=0;for(i=0;i<100;i++){err=GetNextProcess(&none);if(err)break;if(none.highLongOfPSN==current.highLongOfPSN&&none.lowLongOfPSN==current.lowLongOfPSN)found++;}p+=sprintf(p,",\"enumeration\":[%d,%d]",found==1,err);
 p+=sprintf(p,"}}\n");err=Create("\pProcessProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pProcessProbe.json",0,&f))return 3;SetEOF(f,0);n=p-out;err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;
}
