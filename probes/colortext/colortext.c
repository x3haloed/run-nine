/* Geometric observations only; no font bitmap is saved. */
#include <QuickDraw.h>
#include <Fonts.h>
#include <Files.h>
#include <stdio.h>
static char out[32768];
int main(void){
 GWorldPtr world=0;CGrafPtr old;GDHandle device;PixMapHandle map=0;Ptr base=0;Rect rect={0,0,64,128};
 short config,c,x,y,left,right,row,f;long n;OSErr err;char *p=out;
 InitGraf(&qd.thePort);InitFonts();GetGWorld(&old,&device);err=NewGWorld(&world,32,&rect,0,0,0);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"color-text-v1\",\"world_error\":%d,\"records\":[",err);
 if(!err){map=GetGWorldPixMap(world);if(!LockPixels(map))return 5;base=GetPixBaseAddr(map);row=(**map).rowBytes&0x3fff;SetGWorld(world,0);
  ForeColor(blackColor);BackColor(whiteColor);TextFont(0);TextSize(12);TextMode(srcCopy);
  for(config=0;config<8;config++){
   SetOutlinePreferred((config&1)!=0);SetPreserveGlyph((config&2)!=0);TextFace((config&4)?1:0);
   p+=sprintf(p,"%s{\"outline_preferred\":%d,\"preserve\":%d,\"face\":%d,\"glyphs\":[",config?",":"",config&1,(config>>1)&1,(config&4)?1:0);
   for(c=0;c<256;c++){
    EraseRect(&rect);MoveTo(32,32);DrawChar(c);left=128;right=-1;
    for(y=0;y<64;y++)for(x=0;x<128;x++){
     unsigned char *pixel=(unsigned char*)base+y*row+x*4;
     if(pixel[1]!=255||pixel[2]!=255||pixel[3]!=255){if(x<left)left=x;if(x>right)right=x;}
    }
    if(right<0){left=0;right=0;}else{left-=32;right-=31;}
    p+=sprintf(p,"%s[%d,%d,%d]",c?",":"",CharWidth(c),left,right);
   }
   p+=sprintf(p,"]}");
  }
  SetGWorld(old,device);UnlockPixels(map);DisposeGWorld(world);SetOutlinePreferred(false);SetPreserveGlyph(false);
 }
 p+=sprintf(p,"]}\n");err=Create("\pColorTextProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;
 if(FSOpen("\pColorTextProbe.json",0,&f))return 3;SetEOF(f,0);n=p-out;err=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return err?4:0;
}
