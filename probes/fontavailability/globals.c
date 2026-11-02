#include <Multiverse.h>
#include <stdio.h>
static char output[1024];
int main(void){short f;long n;OSErr e;InitGraf(&qd.thePort);InitFonts();sprintf(output,"{\"schema\":1,\"probe\":\"font-globals-v1\",\"fonts\":{\"globals\":[%d,%d,%d]}}\n",*(short*)0x984,*(short*)0xba6,*(short*)0xba8);e=Create("\pFontGlobalsProbe.json",0,'ttxt','TEXT');if(e&&e!=dupFNErr)return 2;if(FSOpen("\pFontGlobalsProbe.json",0,&f))return 3;SetEOF(f,0);n=0;while(output[n])n++;e=FSWrite(f,&n,output);FSClose(f);FlushVol(0,0);return e?4:0;}
