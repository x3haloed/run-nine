#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[65536],*p=output;static short count;
static void state(TEHandle h){p+=sprintf(p,"[%d,%d,%d,%d,%d]",(**h).selStart,(**h).selEnd,(**h).clickLoc,(**h).clikStuff,(TickCount()-(**h).clickTime)<=LMGetDoubleTime());}
static Point at(TEHandle h,short offset){long n=TEGetPoint(offset,h);Point pt={(short)(n>>16),(short)n};pt.h++;pt.v--;return pt;}
static void run(short text_id,const char *text,short offset,short second,short mode,Boolean styled){Rect r={0,0,120,300};TEHandle h=styled?TEStyleNew(&r,&r):TENew(&r,&r);Point first,other;TESetText((Ptr)text,strlen(text),h);(**h).clickTime=TickCount()-LMGetDoubleTime()-10;(**h).clickLoc=-1;(**h).clikStuff=0;if(mode==2)TESetSelect(0,3,h);if(mode==3)TESetSelect(1,7,h);if(mode==5)TEActivate(h);first=at(h,offset);other=at(h,second);if(mode==4)other.h++;
p+=sprintf(p,"%s{\"text\":%d,\"offsets\":[%d,%d],\"mode\":%d,\"styled\":%d,\"points\":[[%d,%d],[%d,%d]],\"states\":[",count?",":"",text_id,offset,second,mode,styled,first.v,first.h,other.v,other.h);TEClick(first,mode==3,h);state(h);if(mode==1)(**h).clickTime=TickCount()-LMGetDoubleTime()-10;TEClick(other,mode==2||mode==3,h);p+=sprintf(p,",");state(h);TEClick(other,mode==2||mode==3,h);p+=sprintf(p,",");state(h);p+=sprintf(p,"]}");count++;TEDispose(h);}
int main(void){short offsets0[]={0,1,3,4,5,7,8,10,13,14,15},offsets1[]={0,1,2,3,4,7,10,12,15},offsets2[]={1,2,3,5,6,7,9};short i,file;long n;OSErr e;InitGraf(&qd.thePort);InitFonts();InitWindows();TEInit();TextFont(3);TextFace(0);TextSize(12);
p+=sprintf(p,"{\"schema\":1,\"probe\":\"te-double-v1\",\"textedit\":{\"double_time\":%ld,\"cases\":[",LMGetDoubleTime());
for(i=0;i<11;i++)run(0,"one two,three\rx",offsets0[i],offsets0[i],0,false);for(i=0;i<9;i++)run(1,"a-b can't 12.34",offsets1[i],offsets1[i],0,false);for(i=0;i<7;i++)run(2,"ab\tcd  ef",offsets2[i],offsets2[i],0,false);run(3,"caf\216..fin",2,2,0,false);run(0,"one two,three\rx",4,4,1,false);run(0,"one two,three\rx",4,5,0,false);run(0,"one two,three\rx",5,5,2,false);run(0,"one two,three\rx",5,5,0,true);
{const char *text="ab.cd a_b a/b a:b a;b a+b ab--cd ab''cd .. fin...end";for(i=0;i<=strlen(text);i++)run(4,text,i,i,0,false);}
for(i=0;i<=8;i++)run(0,"one two,three\rx",i,i,3,false);
run(0,"one two,three\rx",4,4,4,false);run(0,"one two,three\rx",5,5,5,false);
p+=sprintf(p,"]}}\n");e=Create("\pTEDoubleProbe.json",0,'ttxt','TEXT');if(e)return 2;if(FSOpen("\pTEDoubleProbe.json",0,&file))return 3;n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?4:0;}
