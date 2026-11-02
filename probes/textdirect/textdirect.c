#include <QuickDraw.h>
#include <Fonts.h>
#include <TextEdit.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
int main(void){GrafPort port;Rect r={0,0,100,200};TEHandle h;char out[2048],*p=out;short i,f;long n;OSErr err;
 InitGraf(&qd.thePort);InitFonts();OpenPort(&port);TEInit();h=TENew(&r,&r);if(!h)return 5;
 SetHandleSize((*h)->hText,6);HLock((*h)->hText);memcpy(*(*h)->hText,"direct",6);HUnlock((*h)->hText);TECalText(h);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"text-direct-v1\",\"textedit\":{\"grow_length\":%d,\"grow_lines\":[",(*h)->teLength);
 for(i=0;i<=(*h)->nLines;i++)p+=sprintf(p,"%s%d",i?",":"",(*h)->lineStarts[i]);
 TESetSelect(5,6,h);
 SetHandleSize((*h)->hText,3);HLock((*h)->hText);memcpy(*(*h)->hText,"A\rB",3);HUnlock((*h)->hText);TECalText(h);
 p+=sprintf(p,"],\"shrink_length\":%d,\"shrink_lines\":[",(*h)->teLength);for(i=0;i<=(*h)->nLines;i++)p+=sprintf(p,"%s%d",i?",":"",(*h)->lineStarts[i]);p+=sprintf(p,"],\"shrink_selection\":[%d,%d]",(*h)->selStart,(*h)->selEnd);
 TESetText("reference text",14,h);SetHandleSize((*h)->hText,0);TECalText(h);p+=sprintf(p,",\"zero_selection\":[%d,%d]",(*h)->selStart,(*h)->selEnd);TEKey('Z',h);p+=sprintf(p,",\"zero_key\":[%d,%d,%d,%u]}}\n",(*h)->teLength,(*h)->selStart,(*h)->selEnd,(unsigned char)(*(*h)->hText)[0]);
 TEDispose(h);ClosePort(&port);err=Create("\pTextDirectProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pTextDirectProbe.json",0,&f))return 3;SetEOF(f,0);n=p-out;err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;
}
