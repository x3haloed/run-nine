#include <Resources.h>
#include <Memory.h>
#include <Files.h>
#include <stdio.h>
static char output[8192];
int main(void){char *p=output;short app=CurResFile(),count,i,id,f;short indices[]={0,1,2,3,4,-1};ResType type;Str255 name;Handle h,again;OSErr e,info;long n,j,size;
 count=Count1Resources('pTst');e=ResError();p+=sprintf(p,"{\"schema\":1,\"probe\":\"resource-enum-v1\",\"resources\":{\"count\":[%d,%d],",count,e);
 count=Count1Resources('pNil');e=ResError();p+=sprintf(p,"\"absent\":[%d,%d],\"items\":[",count,e);
 for(i=0;i<6;i++){h=Get1IndResource('pTst',indices[i]);e=ResError();p+=sprintf(p,"%s{\"index\":%d,\"nonnull\":%d,\"error\":%d",i?",":"",indices[i],h!=0,e);if(h){GetResInfo(h,&id,&type,name);info=ResError();again=Get1IndResource('pTst',indices[i]);p+=sprintf(p,",\"id\":%d,\"type\":%lu,\"name\":[",id,type);for(j=0;j<name[0];j++)p+=sprintf(p,"%s%u",j?",":"",name[j+1]);p+=sprintf(p,"],\"info_error\":%d,\"same_handle\":%d,\"home_is_current\":%d,\"bytes\":[",info,h==again,HomeResFile(h)==app);size=GetHandleSize(h);for(j=0;j<size;j++)p+=sprintf(p,"%s%u",j?",":"",(unsigned char)(*h)[j]);p+=sprintf(p,"]");}p+=sprintf(p,"}");}
 UseResFile(0);count=Count1Resources('pTst');e=ResError();p+=sprintf(p,"],\"system_count\":[%d,%d]",count,e);UseResFile(app);p+=sprintf(p,"}}\n");e=Create("\pResourceEnumProbe.json",0,'ttxt','TEXT');if(e&&e!=dupFNErr)return 2;if(FSOpen("\pResourceEnumProbe.json",0,&f))return 3;SetEOF(f,0);n=p-output;e=FSWrite(f,&n,output);FSClose(f);FlushVol(0,0);return e?4:0;}
