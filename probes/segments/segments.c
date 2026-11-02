/* The build helper appends our two plain MPW jump entries and CODE2.
   This single-segment Retro68 main does not install segment-manager patches. */
#include <QuickDraw.h>
#include <SegLoad.h>
#include <Resources.h>
#include <Memory.h>
#include <Files.h>
#include <stdio.h>
typedef short (*Routine)(void);
static char output[8192], *p=output;
static short value, samples;
static void entry(Routine fn, Handle code) {
 unsigned short *words=(unsigned short *)((char *)fn-2);
 if(words[1]==0x4ef9) p+=sprintf(p,"[%u,%u,%lu]",words[0],words[1],*(unsigned long *)(words+2)-(unsigned long)(*code+4));
 else p+=sprintf(p,"[%u,%u,%u,%u]",words[0],words[1],words[2],words[3]);
}
static void state(char *label, Handle code, Routine first, Routine second) {
 p+=sprintf(p,"%s\"%s\":{\"value\":%d,\"entries\":[",samples++?",":"",label,value);
 entry(first,code);p+=sprintf(p,",");entry(second,code);
 p+=sprintf(p,"],\"handle_state\":%u}",(unsigned char)HGetState(code));
}
int main(void) {
 Handle code,zero; Routine first,second; unsigned long *header; short file;long count;OSErr err;
 zero=Get1Resource('CODE',0);if(!zero){p+=sprintf(p,"{\"error\":\"CODE0\",\"res_error\":%d}\n",ResError());goto save;}header=(unsigned long *)*zero;
 first=(Routine)(SetCurrentA5()+header[3]+header[2]-16+2);second=(Routine)((char*)first+8);
 code=Get1Resource('CODE',2);if(!code){p+=sprintf(p,"{\"error\":\"CODE2\",\"res_error\":%d}\n",ResError());goto save;}
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"segments-v1\",\"segments\":{");
 state("initial",code,first,second);value=first();state("called",code,first,second);
 UnloadSeg((Ptr)first);state("unloaded",code,first,second);
 UnloadSeg((Ptr)first);state("unloaded_twice",code,first,second);
 value=second();state("recalled",code,first,second);
 p+=sprintf(p,"}}\n");
 save:
 err=Create("\pSegmentProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 4;
 if(FSOpen("\pSegmentProbe.json",0,&file))return 5;SetEOF(file,0);count=p-output;err=FSWrite(file,&count,output);FSClose(file);FlushVol(0,0);return err?6:0;
}
