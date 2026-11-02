#define TEXTEDIT_SNAPSHOT_ONLY
#include "../textedit/textedit.c"
static short base_count;
static char *bytes(char *p,Handle h,long n){long i;HLock(h);p+=sprintf(p,"[");for(i=0;i<n;i++)p+=sprintf(p,"%s%u",i?",":"",(unsigned char)(*h)[i]);HUnlock(h);return p+sprintf(p,"]");}
static char *state(char *p,char *name,TEHandle h){Handle dest=NewHandle(0);long off=0x12345678,len;PScrapStuff info;
 p+=sprintf(p,"\"%s\":{\"document\":{",name);p=snapshot(p,"state",h);p+=sprintf(p,"},\"te_scrap\":");p=bytes(p,LMGetTEScrpHandle(),LMGetTEScrpLength());
 info=InfoScrap();p+=sprintf(p,",\"system_size\":%ld,\"count_delta\":%d",info->scrapSize,(short)(info->scrapCount-base_count));
 len=GetScrap(dest,'TEXT',&off);p+=sprintf(p,",\"get_text\":[%ld,%ld],\"system_text\":",len,off);p=bytes(p,dest,len<0?0:len);
 p+=sprintf(p,"}");DisposeHandle(dest);return p;}
int main(void){Rect bounds={60,50,240,350},text={4,4,164,284};WindowPtr w;TEHandle h;char *p=out;short f;long n,a,b,c;short i;OSErr err;
 InitGraf(&qd.thePort);InitFonts();InitWindows();TEInit();w=NewWindow(0,&bounds,"\pClipboard",true,documentProc,(WindowPtr)-1,true,0);SetPort(w);TextFont(1);TextSize(12);h=TENew(&text,&text);
 ZeroScrap();base_count=InfoScrap()->scrapCount;TESetText("one two\rthree",13,h);p+=sprintf(p,"{\"schema\":1,\"probe\":\"clipboard-v1\",\"clipboard\":{");p=state(p,"initial",h);p+=sprintf(p,",\"system_edit\":[");for(i=0;i<8;i++)p+=sprintf(p,"%s%d",i?",":"",SystemEdit(i));p+=sprintf(p,"]");
 TESetSelect(4,7,h);TECopy(h);p+=sprintf(p,",");p=state(p,"copy",h);
 TESetSelect(0,3,h);TEPaste(h);p+=sprintf(p,",");p=state(p,"paste",h);
 TESetSelect(3,8,h);TECut(h);p+=sprintf(p,",");p=state(p,"cut",h);
 TESetSelect(1,1,h);TECopy(h);p+=sprintf(p,",");p=state(p,"empty_copy",h);
 TEPaste(h);p+=sprintf(p,",");p=state(p,"empty_paste",h);
 TESetSelect(0,2,h);TEPaste(h);p+=sprintf(p,",");p=state(p,"empty_paste_selection",h);
 a=PutScrap(3,'TEXT',"odd");b=PutScrap(2,'TEST',"xy");c=PutScrap(4,'TEXT',"last");p+=sprintf(p,",\"put_results\":[%ld,%ld,%ld]",a,b,c);
 p+=sprintf(p,",");p=state(p,"system_put",h);ZeroScrap();p+=sprintf(p,",");p=state(p,"system_zero",h);ZeroScrap();p+=sprintf(p,",");p=state(p,"system_zero_again",h);p+=sprintf(p,"}}\n");
 TEDispose(h);DisposeWindow(w);err=Create("\pClipboardProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pClipboardProbe.json",0,&f))return 3;SetEOF(f,0);n=p-out;err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;
}
