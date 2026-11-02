#include <QuickDraw.h>
#include <Files.h>
#include <Resources.h>
#include <stdio.h>
#include <string.h>
int main(void){char out[1024];short r,empty,missing,f;OSErr empty_error,missing_error,err;long n;
 err=Create("\pEmptyResource.data",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 5;
 empty=OpenRFPerm("\pEmptyResource.data",0,fsRdPerm);empty_error=ResError();if(empty!=-1)CloseResFile(empty);
 missing=OpenRFPerm("\pNoSuchResource.data",0,fsRdPerm);missing_error=ResError();if(missing!=-1)CloseResFile(missing);
 sprintf(out,"{\"schema\":1,\"probe\":\"res-files-v1\",\"resources\":{\"empty_result\":%d,\"empty_error\":%d,\"missing_result\":%d,\"missing_error\":%d}}\n",empty,empty_error,missing,missing_error);
 err=Create("\pResFileProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pResFileProbe.json",0,&f))return 3;SetEOF(f,0);n=strlen(out);err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;
}
