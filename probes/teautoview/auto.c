#include <QuickDraw.h>
#include <Fonts.h>
#include <TextEdit.h>
#include <Windows.h>
#include <Files.h>
#include <stdio.h>
static char output[16384];
static char *rect(char *p,Rect r){return p+sprintf(p,"[%d,%d,%d,%d]",r.top,r.left,r.bottom,r.right);}
static char *state(char *p,TEHandle h){TEPtr t=*h;p+=sprintf(p,"{\"auto\":%d,\"click_flags\":%d,\"cr_only\":%d,\"selection\":[%d,%d],\"n_lines\":%d,\"dest\":",TEFeatureFlag(0,-1,h),t->clikStuff,t->crOnly,t->selStart,t->selEnd,t->nLines);p=rect(p,t->destRect);p+=sprintf(p,",\"view\":");p=rect(p,t->viewRect);return p+sprintf(p,"}");}
static char *run(char *p,Boolean styled){Rect dest={30,20,200,120},view={30,20,50,120};TEHandle h=styled?TEStyleNew(&dest,&view):TENew(&dest,&view);char keys[]={'A',13,'B',13,'C'};short i;if(!h)return 0;p+=sprintf(p,"{\"initial\":");p=state(p,h);TEActivate(h);TEAutoView(1,h);p+=sprintf(p,",\"enabled\":");p=state(p,h);p+=sprintf(p,",\"typing\":[");for(i=0;i<5;i++){TEKey(keys[i],h);p+=sprintf(p,"%s",i?",":"");p=state(p,h);}p+=sprintf(p,"]");TEAutoView(0,h);p+=sprintf(p,",\"disabled\":");p=state(p,h);TESetSelect(0,0,h);p+=sprintf(p,",\"set_result\":%d",TEFeatureFlag(0,1,h));p+=sprintf(p,",\"set\":");p=state(p,h);TESelView(h);p+=sprintf(p,",\"selection_view\":");p=state(p,h);p+=sprintf(p,",\"clear_result\":%d",TEFeatureFlag(0,0,h));TEScroll(-7,-5,h);p+=sprintf(p,",\"scrolled\":");p=state(p,h);TEPinScroll(1000,1000,h);p+=sprintf(p,",\"pinned_high\":");p=state(p,h);TEPinScroll(-1000,-1000,h);p+=sprintf(p,",\"pinned_low\":");p=state(p,h);TEDispose(h);return p+sprintf(p,"}");}
int main(void){char *p=output;short file;long n;OSErr e;InitGraf(&qd.thePort);InitFonts();InitWindows();TEInit();TextFont(4);TextFace(0);TextSize(9);p+=sprintf(p,"{\"schema\":1,\"probe\":\"te-auto-view-v1\",\"textedit\":{\"plain\":");p=run(p,0);if(!p)return 5;p+=sprintf(p,",\"styled\":");p=run(p,1);if(!p)return 6;p+=sprintf(p,"}}\n");e=Create("\pAutoViewProbe.json",0,'ttxt','TEXT');if(e&&e!=dupFNErr)return 2;if(FSOpen("\pAutoViewProbe.json",0,&file))return 3;SetEOF(file,0);n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;}
