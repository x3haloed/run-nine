#include <QuickDraw.h>
#include <Fonts.h>
#include <Windows.h>
#include <Dialogs.h>
#include <StandardFile.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char out[16384];
static short logCount,filterCount,decision,rejected,itemCount;
static Rect replaceBounds,itemRects[16];static short itemKinds[16];static Str255 itemTexts[16];
static long logRef[256];static short logItem[256];
static pascal short hook(short item,DialogPtr dialog,void *data){short i;Handle h;long ref=GetWRefCon(dialog);if(ref=='rplc'&&item==-1&&!itemCount){replaceBounds=((DialogPeek)dialog)->window.port.portRect;itemCount=**(short**)((DialogPeek)dialog)->items+1;if(itemCount>16)itemCount=16;for(i=0;i<itemCount;i++){GetDialogItem(dialog,i+1,&itemKinds[i],&h,&itemRects[i]);if((itemKinds[i]&0x7f)==4)GetControlTitle((ControlHandle)h,itemTexts[i]);else if((itemKinds[i]&0x7f)==8||(itemKinds[i]&0x7f)==16)GetDialogItemText(h,itemTexts[i]);}}if(logCount<256){logRef[logCount]=GetWRefCon(dialog);logItem[logCount++]=item;}return item;}
static pascal Boolean filter(DialogPtr dialog,EventRecord *event,short *item,void *data){long ref=GetWRefCon(dialog);if(event->what!=nullEvent)return false;if(++filterCount>1000){*item=2;return true;}if(ref=='rplc'){*item=decision;rejected=decision==2;return true;}if(ref=='stdf'){*item=rejected?2:1;return true;}*item=2;return true;}
static char *run(char *p,char *label,ConstStr255Param filename,short choice,short vref,long dir){StandardFileReply reply;Point where={-1,-1};short i;logCount=filterCount=rejected=0;decision=choice;memset(&reply,0x5a,sizeof(reply));*(short*)0x214=-vref;*(long*)0x398=dir;
 CustomPutFile("\pSave probe",filename,&reply,-6043,where,NewDlgHookYDUPP(hook),NewModalFilterYDUPP(filter),0,0,0);
 p+=sprintf(p,"\"%s\":{\"good\":%d,\"replacing\":%d,\"type\":%lu,\"script\":%d,\"flags\":%d,\"folder_volume\":[%d,%d],\"name\":[",label,reply.sfGood,reply.sfReplacing,reply.sfType,reply.sfScript,reply.sfFlags,reply.sfIsFolder,reply.sfIsVolume);
 if(reply.sfFile.name[0]<=63)for(i=0;i<reply.sfFile.name[0];i++)p+=sprintf(p,"%s%u",i?",":"",reply.sfFile.name[i+1]);
 p+=sprintf(p,"],\"same_parent\":%d,\"hook\":[",reply.sfFile.vRefNum==vref&&reply.sfFile.parID==dir);for(i=0;i<logCount;i++)p+=sprintf(p,"%s[%ld,%d]",i?",":"",logRef[i],logItem[i]);return p+sprintf(p,"]}");}
int main(void){char *p=out;short vref,file;long dir,n;WDPBRec wd;OSErr err;char bytes[8]={0};long size=8;short i,j;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();TEInit();InitDialogs(0);memset(&wd,0,sizeof(wd));PBHGetVolSync(&wd);vref=wd.ioWDVRefNum;dir=wd.ioWDDirID;
 err=Create("\pexisting.txt",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pexisting.txt",0,&file))return 3;SetEOF(file,0);n=8;FSWrite(file,&n,"ORIGINAL");FSClose(file);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"save-confirm-v1\",\"save\":{");p=run(p,"new","\pfresh.txt",1,vref,dir);p+=sprintf(p,",");p=run(p,"replace_accept","\pexisting.txt",1,vref,dir);p+=sprintf(p,",");p=run(p,"replace_cancel","\pexisting.txt",2,vref,dir);
 if(FSOpen("\pexisting.txt",0,&file))return 3;FSRead(file,&size,bytes);FSClose(file);p+=sprintf(p,",\"original_unchanged\":%d}}\n",size==8&&!memcmp(bytes,"ORIGINAL",8));p-=3;p+=sprintf(p,",\"replace_dialog\":{\"bounds\":[%d,%d,%d,%d],\"items\":[",replaceBounds.top,replaceBounds.left,replaceBounds.bottom,replaceBounds.right);for(i=0;i<itemCount;i++){p+=sprintf(p,"%s{\"kind\":%d,\"rect\":[%d,%d,%d,%d],\"text\":[",i?",":"",itemKinds[i],itemRects[i].top,itemRects[i].left,itemRects[i].bottom,itemRects[i].right);for(j=0;j<itemTexts[i][0];j++)p+=sprintf(p,"%s%u",j?",":"",itemTexts[i][j+1]);p+=sprintf(p,"]}");}p+=sprintf(p,"]}}}\n");
 err=Create("\pSaveConfirmProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;if(FSOpen("\pSaveConfirmProbe.json",0,&file))return 3;SetEOF(file,0);n=p-out;err=FSWrite(file,&n,out);FSClose(file);FlushVol(0,0);return err?4:0;
}
