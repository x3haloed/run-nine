#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[8192],*p=output;
static void color(const char *name,RGBColor c){p+=sprintf(p,",\"%s\":[%u,%u,%u]",name,c.red,c.green,c.blue);}
int main(void){short kind,file,ids[4]={22000,22001,22001,22002},oldDepth;GDHandle device;WindowPtr w;GrafPtr old;CGrafPort seed;RGBColor red={65535,0,0},blue={0,0,65535},green={0,65535,0},rgb;Rect global={70,90,190,350},local={0,0,120,260};long n;OSErr err;
 InitGraf(&qd.thePort);InitFonts();InitWindows();device=GetMainDevice();oldDepth=(**(*device)->gdPMap).pixelSize;if(SetDepth(device,8,0,0))return 7;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"window-background-v1\",\"windows\":{\"cases\":[");
 for(kind=0;kind<4;kind++){
  GetPort(&old);OpenCPort(&seed);RGBForeColor(&red);PaintRect(&global);GetCPixel(100,80,&rgb);CloseCPort(&seed);SetPort(old);
  p+=sprintf(p,"%s{\"kind\":%d,\"id\":%d",kind?",":"",kind,ids[kind]);color("seed_rgb",rgb);
  w=(WindowPtr)GetNewCWindow(ids[kind],0,(WindowPtr)-1);if(!w)return 5;SetPort(w);if(kind==0)RGBBackColor(&blue);if(kind>=2)RGBBackColor(&green);
  ShowWindow(w);GetCPixel(10,10,&rgb);color("shown_rgb",rgb);color("port_bg",((CGrafPtr)w)->rgbBkColor);
  EraseRect(&local);GetCPixel(10,10,&rgb);color("erased_rgb",rgb);
  SetWinColor(w,(CTabHandle)Get1Resource('wctb',22003));GetCPixel(10,10,&rgb);color("after_set_rgb",rgb);
  HideWindow(w);ShowWindow(w);GetCPixel(10,10,&rgb);color("reshown_rgb",rgb);p+=sprintf(p,"}");SetPort(old);DisposeWindow(w);
 }
 err=SetDepth(device,oldDepth,0,0);p+=sprintf(p,"]},\"restore_error\":%d}\n",err);err=Create("\pWindowBackgroundProbe.json",0,'pPrb','TEXT');if(err)return 2;if(FSOpen("\pWindowBackgroundProbe.json",0,&file))return 3;n=p-output;err=FSWrite(file,&n,output);FSClose(file);FlushVol(0,0);return err?4:0;
}
