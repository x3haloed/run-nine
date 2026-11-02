#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[16384],*p=output;static short calls;static WindowPtr owner;
static pascal Boolean filter(DialogPtr d,EventRecord *event,short *hit) {
 WindowPeek peek=(WindowPeek)d;GrafPtr port;short kind;Handle text;Rect r;Str255 value;short i;
 GetPort(&port);GetDialogItem(d,3,&kind,&text,&r);GetDialogItemText(text,value);calls++;
 p+=sprintf(p,"\"callback\":{\"current_dialog\":%d,\"front_dialog\":%d,\"kind\":%d,\"visible\":%d,\"default_item\":%d,\"edit_field\":%d,\"port_rect\":[%d,%d,%d,%d],\"content\":[%d,%d,%d,%d],\"text_kind\":%d,\"text_hex\":\"",port==(GrafPtr)d,FrontWindow()==(WindowPtr)d,peek->windowKind,peek->visible!=0,((DialogPeek)d)->aDefItem,((DialogPeek)d)->editField,peek->port.portRect.top,peek->port.portRect.left,peek->port.portRect.bottom,peek->port.portRect.right,(**peek->contRgn).rgnBBox.top,(**peek->contRgn).rgnBBox.left,(**peek->contRgn).rgnBBox.bottom,(**peek->contRgn).rgnBBox.right,kind);
 for(i=1;i<=value[0];i++)p+=sprintf(p,"%02x",value[i]);p+=sprintf(p,"\",\"structure\":[%d,%d,%d,%d],\"event_nonnull\":%d,\"hit_nonnull\":%d}",(**peek->strucRgn).rgnBBox.top,(**peek->strucRgn).rgnBBox.left,(**peek->strucRgn).rgnBBox.bottom,(**peek->strucRgn).rgnBBox.right,event!=0,hit!=0);
 *hit=2;return 1;
}
int main(void) {
 short id,mode,result,file;GrafPtr port;Rect bounds={60,90,220,460};long n;OSErr e;
 InitGraf(&qd.thePort);InitFonts();InitWindows();InitMenus();TEInit();InitDialogs(0);
 owner=NewWindow(0,&bounds,"\pOwned alert owner",1,0,(WindowPtr)-1,1,0);if(!owner)return 2;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"alerts-v1\",\"alerts\":{\"cases\":[");
 for(id=128;id<=131;id++)for(mode=0;mode<4;mode++) {
  SetPort((GrafPtr)owner);ParamText("\pfirst","\psecond","\p","\p");calls=0;
  p+=sprintf(p,"%s{\"resource\":%d,\"mode\":%d,",id!=128||mode?",":"",id,mode);
  if(mode==0)result=Alert(id,NewModalFilterUPP(filter));
  else if(mode==1)result=StopAlert(id,NewModalFilterUPP(filter));
  else if(mode==2)result=CautionAlert(id,NewModalFilterUPP(filter));
  else result=NoteAlert(id,NewModalFilterUPP(filter));
  GetPort(&port);p+=sprintf(p,",\"result\":%d,\"calls\":%d,\"old_port_restored\":%d,\"owner_front\":%d}",result,calls,port==(GrafPtr)owner,FrontWindow()==owner);
 }
 p+=sprintf(p,"]}}\n");DisposeWindow(owner);
 e=Create("\pAlertProbe.json",0,'pPrb','TEXT');if(e)return 3;if(FSOpen("\pAlertProbe.json",0,&file))return 4;
 n=p-output;e=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return e?5:0;
}
