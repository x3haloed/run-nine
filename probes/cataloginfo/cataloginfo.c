#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char out[32768],*p=out;static short vol,known_ref;static long original,base,child,alpha_id;
static void str(StringPtr out,const char *s){out[0]=strlen(s);memcpy(out+1,s,out[0]);}
static short dir(short v,long parent,const char *name,long *id){HParamBlockRec b;Str255 n;OSErr e;memset(&b,0,sizeof(b));str(n,name);b.fileParam.ioNamePtr=n;b.fileParam.ioVRefNum=v;b.fileParam.ioDirID=parent;e=PBDirCreateSync(&b);*id=b.fileParam.ioDirID;return e;}
static short put(long d,const char *name,const char *data,const char *resource){Str255 n;short f;long size;OSErr e;str(n,name);e=HCreate(vol,d,n,'pPrb','TEXT');if(e)return e;if(HOpen(vol,d,n,fsRdWrPerm,&f))return -1;size=strlen(data);e=FSWrite(f,&size,data);FSClose(f);if(e)return e;if(HOpenRF(vol,d,n,fsRdWrPerm,&f))return -2;size=strlen(resource);e=FSWrite(f,&size,resource);FSClose(f);return e;}
static short metadata(long parent,const char *name){CInfoPBRec b;Str255 n;memset(&b,0,sizeof(b));str(n,name);b.hFileInfo.ioNamePtr=n;b.hFileInfo.ioVRefNum=vol;b.hFileInfo.ioDirID=parent;b.hFileInfo.ioFlFndrInfo.fdType='TEXT';b.hFileInfo.ioFlFndrInfo.fdCreator='pPrb';b.hFileInfo.ioFlCrDat=0x12345678;b.hFileInfo.ioFlMdDat=0x12345679;b.hFileInfo.ioFlBkDat=0x1234567a;return PBSetCatInfoSync(&b);}
static short dirmeta(long id){CInfoPBRec b;memset(&b,0,sizeof(b));b.dirInfo.ioVRefNum=vol;b.dirInfo.ioFDirIndex=-1;b.dirInfo.ioDrDirID=id;b.dirInfo.ioDrCrDat=0x12345678;b.dirInfo.ioDrMdDat=0x12345679;b.dirInfo.ioDrBkDat=0x1234567a;return PBSetCatInfoSync(&b);}
static short tag(long id){if(id==0)return 0;if(id==1)return 1;if(id==2)return 2;if(id==base)return 3;if(id==child)return 4;if(id==alpha_id)return 5;return -1;}
static void row(short index,short v,long d,short enumeration,const char *path,short nil_name){
 struct {unsigned char pre[8];CInfoPBRec pb;unsigned char post[8];} s;Str255 n;OSErr e;short i,guard=1;unsigned char *bytes=(unsigned char*)&s.pb;
 memset(&s,0x55,sizeof(s));memset(n,0x55,sizeof(n));str(n,path);s.pb.hFileInfo.ioCompletion=0;s.pb.hFileInfo.ioNamePtr=nil_name?0:n;s.pb.hFileInfo.ioVRefNum=v;s.pb.hFileInfo.ioDirID=d;s.pb.hFileInfo.ioFDirIndex=enumeration;s.pb.hFileInfo.ioACUser=0;
 e=PBGetCatInfoSync(&s.pb);for(i=0;i<8;i++)if(s.pre[i]!=0x55||s.post[i]!=0x55)guard=0;
 p+=sprintf(p,"%s{\"case\":%d,\"error\":%d,\"io_result\":%d,\"volume_same\":%d,\"ref\":[%d,%d,%d],\"version_filler_hex\":\"%02x%02x\",\"index_out\":%d,\"attributes\":%u,\"access\":%u,\"object_tag\":%d,\"object_positive\":%d,\"parent_tag\":%d,\"guards\":%d,\"name33_hex\":\"",index?",":"",index,e,s.pb.hFileInfo.ioResult,s.pb.hFileInfo.ioVRefNum==v,s.pb.hFileInfo.ioFRefNum==0,s.pb.hFileInfo.ioFRefNum==0x5555,known_ref>0&&s.pb.hFileInfo.ioFRefNum==known_ref,bytes[26],bytes[27],s.pb.hFileInfo.ioFDirIndex,(unsigned char)s.pb.hFileInfo.ioFlAttrib,(unsigned char)s.pb.hFileInfo.ioACUser,tag(s.pb.hFileInfo.ioDirID),s.pb.hFileInfo.ioDirID>0,tag(s.pb.hFileInfo.ioFlParID),guard);
 for(i=0;i<33;i++)p+=sprintf(p,"%02x",n[i]);p+=sprintf(p,"\",\"body_hex\":\"");for(i=32;i<108;i++)p+=sprintf(p,"%02x",bytes[i]);p+=sprintf(p,"\"}");
}
int main(void){WDPBRec wd;Str255 n;short f,output;long size;CInfoPBRec b;memset(&wd,0,sizeof(wd));if(PBHGetVolSync(&wd))return 2;vol=wd.ioWDVRefNum;original=wd.ioWDDirID;
 if(dir(vol,2,"Catalog-owned",&base)||dir(vol,base,"Child",&child))return 3;
 if(put(base,"Alpha","ABCDEF","RS")||put(base,"beta","B","")||put(child,"Alpha","CHLD",""))return 4;
 if(metadata(base,"Alpha")||metadata(base,"beta")||metadata(child,"Alpha")||dirmeta(base)||dirmeta(child))return 5;
 memset(&b,0,sizeof(b));b.hFileInfo.ioNamePtr=(StringPtr)"\pAlpha";b.hFileInfo.ioVRefNum=vol;b.hFileInfo.ioDirID=base;if(PBGetCatInfoSync(&b))return 6;alpha_id=b.hFileInfo.ioDirID;
 memset(&wd,0,sizeof(wd));wd.ioVRefNum=vol;wd.ioWDDirID=base;if(PBHSetVolSync(&wd))return 7;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"catalog-info-v1\",\"raw_environment\":{\"volume\":%d,\"base\":%ld,\"child\":%ld,\"alpha_id\":%ld},\"files\":{\"cases\":[",vol,base,child,alpha_id);
 row(0,0,0,0,"Marathon Preferences",0);row(1,vol,base,0,"Alpha",0);row(2,vol,base,0,"Child",0);row(3,vol,base,-1,"IGNORED",0);row(4,vol,child,-1,"IGNORED",0);
 row(5,vol,base,0,"aLPHa",0);row(6,vol,base,0,":Child:Alpha",0);row(7,vol,base,0,":Missing:Alpha",0);row(8,vol,999999,0,"Alpha",0);row(9,-123,base,0,"Alpha",0);
 row(10,vol,base,1,"IGNORED",0);row(11,vol,base,2,"IGNORED",0);row(12,vol,base,3,"IGNORED",0);row(13,vol,base,4,"IGNORED",0);
 row(14,vol,base,0,"",0);row(15,vol,base,0,"IGNORED",1);row(16,vol,base,-1,"IGNORED",1);row(17,vol,base,-32768,"IGNORED",0);row(18,vol,alpha_id,1,"IGNORED",0);
 if(HOpen(vol,base,"\pAlpha",fsRdWrPerm,&f))return 8;known_ref=f;row(19,vol,base,0,"Alpha",0);FSClose(f);known_ref=0;
 if(HOpenRF(vol,base,"\pAlpha",fsRdPerm,&f))return 9;known_ref=f;row(20,vol,base,0,"Alpha",0);FSClose(f);known_ref=0;
 row(21,vol,base,0,"Alpha",0);row(22,vol,base,0,":Alpha:",0);row(23,vol,0,0,"Missing",0);row(24,vol,child,0,"::Alpha",0);row(25,vol,child,1,"IGNORED",0);row(26,vol,999999,-1,"IGNORED",0);row(27,vol,base,0,"::Missing",0);
 p+=sprintf(p,"]}}\n");memset(&wd,0,sizeof(wd));wd.ioVRefNum=vol;wd.ioWDDirID=original;PBHSetVolSync(&wd);if(Create("\pCatalogInfoProbe.json",0,'pPrb','TEXT'))return 10;if(FSOpen("\pCatalogInfoProbe.json",0,&output))return 11;size=p-out;f=FSWrite(output,&size,out);FSClose(output);FlushVol(0,0);return f?12:0;}
