/* An owned startup helper launches the intact original app from its intact
   asset directory through the native Process Manager. No game input injection. */
#include <QuickDraw.h>
#include <Files.h>
#include <Processes.h>
#include <stdio.h>
#include <string.h>
int main(void){FSSpec app;LaunchParamBlockRec launch;OSErr lookup,err,save;short file;long count;char output[512];
 memset(&launch,0,sizeof(launch));lookup=FSMakeFSSpec(0,2,"\p:Reference Files:Marathon v1.2",&app);err=lookup;
 if(!lookup){launch.launchBlockID=extendedBlock;launch.launchEPBLength=extendedBlockLen;launch.launchControlFlags=launchContinue|0x0800;launch.launchAppSpec=&app;err=LaunchApplication(&launch);}
 sprintf(output,"{\"lookup_error\":%d,\"launch_error\":%d,\"process_present\":%d,\"preferred_bytes\":%ld,\"minimum_bytes\":%ld}\n",lookup,err,launch.launchProcessSN.highLongOfPSN!=0||launch.launchProcessSN.lowLongOfPSN!=0,launch.launchPreferredSize,launch.launchMinimumSize);
 save=Create("\pMarathonLaunch.json",0,'ttxt','TEXT');if(save&&save!=dupFNErr)return 2;if(FSOpen("\pMarathonLaunch.json",0,&file))return 3;SetEOF(file,0);count=strlen(output);save=FSWrite(file,&count,output);FSClose(file);FlushVol(0,0);return save?4:0;
}
