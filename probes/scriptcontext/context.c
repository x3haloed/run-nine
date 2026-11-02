#include <Multiverse.h>
#include <stdio.h>
static char output[1024];
int main(void){short selectors[]={18,20,22},variables[]={6,8,90},scripts[]={0,-1},i,j,f;char *p=output;long n;OSErr e;p+=sprintf(p,"{\"schema\":1,\"probe\":\"script-context-v1\",\"scripts\":{\"context\":[");for(i=0;i<3;i++)p+=sprintf(p,"%s[%d,%ld]",i?",":"",selectors[i],GetScriptManagerVariable(selectors[i]));p+=sprintf(p,"],\"properties\":[");for(i=0;i<2;i++)for(j=0;j<3;j++)p+=sprintf(p,"%s[%d,%d,%ld]",i||j?",":"",scripts[i],variables[j],GetScriptVariable(scripts[i],variables[j]));p+=sprintf(p,"]}}\n");e=Create("\pScriptContextProbe.json",0,'ttxt','TEXT');if(e&&e!=dupFNErr)return 2;if(FSOpen("\pScriptContextProbe.json",0,&f))return 3;SetEOF(f,0);n=p-output;e=FSWrite(f,&n,output);FSClose(f);FlushVol(0,0);return e?4:0;}
