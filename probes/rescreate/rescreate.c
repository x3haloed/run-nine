#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char out[12000],*p=out;
static void hex_bytes(const unsigned char *b,long n){long i;for(i=0;i<n;i++)p+=sprintf(p,"%02x",b[i]);}
static short rawfork(FSSpec *s,Boolean resource,char *data,long *n){short r;long size;OSErr e=resource?FSpOpenRF(s,fsRdPerm,&r):FSpOpenDF(s,fsRdPerm,&r);if(e)return e;GetEOF(r,&size);if(size>1024){FSClose(r);return -50;}*n=size;e=FSRead(r,n,data);FSClose(r);return e;}
static void snapshot(short row,FSSpec *s,OSErr error,short current){char data[1024],resource[1024];long dn=0,rn=0;FInfo info;short de=rawfork(s,false,data,&dn),re=rawfork(s,true,resource,&rn);FSpGetFInfo(s,&info);p+=sprintf(p,"%s{\"error\":%d,\"current_same\":%d,\"fork_errors\":[%d,%d],\"finder\":[%lu,%lu],\"data_hex\":\"",row?",":"",error,CurResFile()==current,de,re,(unsigned long)info.fdType,(unsigned long)info.fdCreator);hex_bytes((unsigned char*)data,dn);p+=sprintf(p,"\",\"resource_hex\":\"");hex_bytes((unsigned char*)resource,rn);p+=sprintf(p,"\"}");}
static short put(FSSpec *s,Boolean resource,const char *data,long n){short r;OSErr e=resource?FSpOpenRF(s,fsRdWrPerm,&r):FSpOpenDF(s,fsRdWrPerm,&r);if(e)return e;e=FSWrite(r,&n,data);FSClose(r);return e;}
int main(void){FSSpec fresh,plain,corrupt,bad;short current=CurResFile(),f;long n;OSErr e,invalid_volume,invalid_dir;
 FSMakeFSSpec(0,0,"\pResourceFresh",&fresh);FSMakeFSSpec(0,0,"\pResourcePlain",&plain);FSMakeFSSpec(0,0,"\pResourceCorrupt",&corrupt);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"res-create-v1\",\"resources\":{\"cases\":[");
 FSpCreateResFile(&fresh,'CrPr','TEXT',smSystemScript);e=ResError();snapshot(0,&fresh,e,current);
 if(put(&fresh,false,"keep",4))return 2;
 FSpCreateResFile(&fresh,'New!','BINA',smSystemScript);e=ResError();snapshot(1,&fresh,e,current);
 if(FSpCreate(&plain,'Old!','BINA',smSystemScript)||put(&plain,false,"keep",4))return 3;
 FSpCreateResFile(&plain,'CrPr','TEXT',smSystemScript);e=ResError();snapshot(2,&plain,e,current);
 if(FSpCreate(&corrupt,'Old!','BINA',smSystemScript)||put(&corrupt,true,"bad!",4))return 4;
 FSpCreateResFile(&corrupt,'CrPr','TEXT',smSystemScript);e=ResError();snapshot(3,&corrupt,e,current);
 bad=fresh;bad.vRefNum=32000;FSpCreateResFile(&bad,'CrPr','TEXT',smSystemScript);invalid_volume=ResError();bad=fresh;bad.parID=0x7fffffff;FSpCreateResFile(&bad,'CrPr','TEXT',smSystemScript);invalid_dir=ResError();
 p+=sprintf(p,"],\"invalid\":[%d,%d]}}\n",invalid_volume,invalid_dir);
 if(Create("\pResCreateProbe.json",0,'ttxt','TEXT'))return 5;if(FSOpen("\pResCreateProbe.json",0,&f))return 6;n=strlen(out);e=FSWrite(f,&n,out);FSClose(f);FlushVol(0,0);return e?7:0;
}
