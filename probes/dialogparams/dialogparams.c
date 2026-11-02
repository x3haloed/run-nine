#include <QuickDraw.h>
#include <Fonts.h>
#include <Dialogs.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char out[8192];
static char *state(char *p,char *name){short i,j;Handle h;long size;p+=sprintf(p,"\"%s\":[",name);for(i=0;i<4;i++){h=LMGetDAStrings()[i];if(!h){p+=sprintf(p,"%snull",i?",":"");continue;}size=GetHandleSize(h);HLock(h);p+=sprintf(p,"%s{\"size\":%ld,\"text\":[",i?",":"",size);if(size>0&&*h)for(j=0;j<(unsigned char)(*h)[0];j++)p+=sprintf(p,"%s%u",j?",":"",(unsigned char)(*h)[j+1]);HUnlock(h);p+=sprintf(p,"]}");}return p+sprintf(p,"]");}
int main(void){char *p=out;short f;long n;OSErr err;InitGraf(&qd.thePort);InitFonts();InitDialogs(0);p+=sprintf(p,"{\"schema\":1,\"probe\":\"dialog-params-v1\",\"parameters\":{");p=state(p,"initial");ParamText("\pone","\p","\p\x80","\p^0");p+=sprintf(p,",");p=state(p,"set");ParamText(0,"\pTWO",0,"\p");p+=sprintf(p,",");p=state(p,"partial");ParamText(0,0,0,0);p+=sprintf(p,",");p=state(p,"nil");p+=sprintf(p,"}}\n");err=Create("\pDialogParamProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pDialogParamProbe.json",0,&f))return 3;SetEOF(f,0);n=p-out;err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;}
