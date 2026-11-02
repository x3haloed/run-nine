#include <QuickDraw.h>
#include <Fonts.h>
#include <TextEdit.h>
#include <Windows.h>
#include <Files.h>
#include <stdio.h>
static char output[8192];
static char *state(char *p,TEHandle h){long point=TEGetPoint((*h)->selEnd,h);return p+sprintf(p,"[%d,%d,%d,%d,%d,%d,%d]",(*h)->selStart,(*h)->selEnd,(*h)->nLines,(*h)->txSize,(*h)->clikStuff,(short)(point>>16),(short)point);}
static char *run(char *p,Boolean styled){Rect r={30,20,200,200};TEHandle h=styled?TEStyleNew(&r,&r):TENew(&r,&r);short keys[]={31,31,30,30,30,31},i;p+=sprintf(p,"{\"empty_down\":");TEKey(31,h);p=state(p,h);TEKey(30,h);p+=sprintf(p,",\"empty_up\":");p=state(p,h);TESetText("abcd\rxy\rz",9,h);TESetSelect(2,2,h);p+=sprintf(p,",\"moves\":[");for(i=0;i<6;i++){TEKey(keys[i],h);p+=sprintf(p,"%s",i?",":"");p=state(p,h);}p+=sprintf(p,"]");TESetSelect(1,6,h);TEKey(30,h);p+=sprintf(p,",\"selection_up\":");p=state(p,h);TESetSelect(1,6,h);TEKey(31,h);p+=sprintf(p,",\"selection_down\":");p=state(p,h);TEDispose(h);return p+sprintf(p,"}");}
int main(void){char *p=output;short file;long n;OSErr e;InitGraf(&qd.thePort);InitFonts();InitWindows();TEInit();TextFont(4);TextFace(0);TextSize(9);p+=sprintf(p,"{\"schema\":1,\"probe\":\"te-arrows-v1\",\"textedit\":{\"plain\":");p=run(p,0);p+=sprintf(p,",\"styled\":");p=run(p,1);p+=sprintf(p,"}}\n");e=Create("\pTEArrowsProbe.json",0,'ttxt','TEXT');if(e&&e!=dupFNErr)return 2;if(FSOpen("\pTEArrowsProbe.json",0,&file))return 3;SetEOF(file,0);n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;}
