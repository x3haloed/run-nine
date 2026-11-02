#include <QuickDraw.h>
#include <Files.h>
#include <stdio.h>
#include <string.h>
static char out[8192];
static char *describe(char *p,char *name,RgnHandle h){
    unsigned short *w;short n,i;HLock((Handle)h);w=(unsigned short*)*h;n=w[0]/2;
    p+=sprintf(p,"\"%s\":[",name);for(i=0;i<n;i++)p+=sprintf(p,"%s%u",i?",":"",w[i]);p+=sprintf(p,"]");HUnlock((Handle)h);return p;
}
int main(void){
    RgnHandle a,b,r;char *p=out;short file;long count;OSErr err;Point inside={5,7},gap={10,3};
    a=NewRgn();b=NewRgn();r=NewRgn();
    p+=sprintf(p,"{\"schema\":1,\"probe\":\"regions-v1\",\"regions\":{");p=describe(p,"empty",r);
    SetRectRgn(a,2,1,9,8);SetRectRgn(b,6,4,14,12);
    UnionRgn(a,b,r);p+=sprintf(p,",");p=describe(p,"union",r);
    p+=sprintf(p,",\"inside_union\":%d,\"outside_union\":%d",PtInRgn(inside,r)!=0,PtInRgn(gap,r)!=0);
    SectRgn(a,b,r);p+=sprintf(p,",");p=describe(p,"intersection",r);
    DiffRgn(a,b,r);p+=sprintf(p,",");p=describe(p,"difference",r);
    XorRgn(a,b,r);p+=sprintf(p,",");p=describe(p,"xor",r);
    OffsetRgn(r,-5,3);p+=sprintf(p,",");p=describe(p,"offset_xor",r);
    SetRectRgn(r,9,8,2,1);p+=sprintf(p,",");p=describe(p,"inverted",r);
    p+=sprintf(p,"}}\n");DisposeRgn(a);DisposeRgn(b);DisposeRgn(r);
    err=Create("\pRegionProbe.json",0,'ttxt','TEXT');if(err&&err!=dupFNErr)return 2;
    if(FSOpen("\pRegionProbe.json",0,&file))return 3;
    SetEOF(file,0);count=p-out;err=FSWrite(file,&count,out);FSClose(file);FlushVol(0,0);return err?4:0;
}
