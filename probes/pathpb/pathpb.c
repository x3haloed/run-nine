#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[24576],*p=output;
static short status_file=-1;static short volume;static long base,child,other;
static void status(short phase,short error){char b[256];long n;sprintf(b,"{\"phase\":%d,\"error\":%d,\"volume\":%d,\"base\":%ld,\"child\":%ld,\"other\":%ld}\n",phase,error,volume,base,child,other);n=strlen(b);if(status_file>=0){SetFPos(status_file,fsFromStart,0);SetEOF(status_file,0);FSWrite(status_file,&n,b);FlushVol(0,0);}}
static short tag(long id){if(id==0)return 0;if(id==2)return 2;if(id==base)return 3;if(id==child)return 4;if(id==other)return 5;return -1;}
static void str(StringPtr out,const char *s){size_t n=strlen(s);out[0]=n;memcpy(out+1,s,n);}
static short getvol(StringPtr name,short *v,long *dir){WDPBRec pb;OSErr e;memset(&pb,0,sizeof(pb));pb.ioNamePtr=name;e=PBHGetVolSync(&pb);*v=pb.ioWDVRefNum;*dir=pb.ioWDDirID;return e;}
static short setvol(short v,long dir){WDPBRec pb;memset(&pb,0,sizeof(pb));pb.ioVRefNum=v;pb.ioWDDirID=dir;return PBHSetVolSync(&pb);}
static short mkdir_native(short v,long dir,StringPtr name,long *out){HParamBlockRec pb;OSErr e;memset(&pb,0,sizeof(pb));pb.fileParam.ioNamePtr=name;pb.fileParam.ioVRefNum=v;pb.fileParam.ioDirID=dir;e=PBDirCreateSync(&pb);*out=pb.fileParam.ioDirID;return e;}
static short put(long dir,const char *name,const char *data){Str255 n;short f;long size=strlen(data);OSErr e;str(n,name);e=HCreate(volume,dir,n,'pPrb','TEXT');if(e)return e;e=HOpen(volume,dir,n,fsRdWrPerm,&f);if(e)return e;e=FSWrite(f,&size,data);FSClose(f);return e;}
static void row(short index,short v,long d,const char *path){
 Str255 name;FSSpec spec;HParamBlockRec pb;short pe,pref;short f=0x1234;long size=4;unsigned char data[4]={0x55,0x55,0x55,0x55};OSErr se,oe,re=12345;int i;
 str(name,path);memset(&spec,0x55,sizeof(spec));se=FSMakeFSSpec(v,d,name,&spec);
 oe=HOpen(v,d,name,fsRdPerm,&f);if(!oe){re=FSRead(f,&size,data);FSClose(f);}
 memset(&pb,0x55,sizeof(pb));pb.ioParam.ioNamePtr=name;pb.ioParam.ioVRefNum=v;pb.fileParam.ioDirID=d;pb.ioParam.ioPermssn=fsRdPerm;pb.ioParam.ioMisc=0;pb.ioParam.ioRefNum=0x1234;pb.fileParam.ioFVersNum=0;pb.ioParam.ioCompletion=0;
 pe=PBHOpenSync(&pb);pref=pb.ioParam.ioRefNum;if(!pe)FSClose(pref);
 p+=sprintf(p,"%s{\"case\":%d,\"spec_error\":%d,\"spec_volume_same\":%d,\"spec_volume_zero\":%d,\"spec_parent\":%d,\"spec_name_hex\":\"",index?",":"",index,se,spec.vRefNum==volume,spec.vRefNum==0,tag(spec.parID));
 for(i=0;i<spec.name[0]&&i<63;i++)p+=sprintf(p,"%02x",spec.name[i+1]);
 p+=sprintf(p,"\",\"open_error\":%d,\"failed_ref_preserved\":%d,\"read\":[%d,%ld,%u,%u,%u,%u],\"pb_error\":%d,\"pb_io_result\":%d,\"pb_ref\":%d}",oe,oe?f==0x1234:0,re,size,data[0],data[1],data[2],data[3],pe,pb.ioParam.ioResult,pe?pref:(pref>0));
}
int main(void){
 Str255 volname,n,original_name;long original_dir;short original_vol,f;long size;OSErr e;char absolute[300];
 Create("\pPathFiles-status.json",0,'pPrb','TEXT');FSOpen("\pPathFiles-status.json",0,&status_file);status(1,0);original_name[0]=0;e=getvol(original_name,&original_vol,&original_dir);volume=original_vol;status(2,e);if(e)return 2;
 volume=original_vol;memcpy(volname,original_name,256);
 str(n,"RuntimePathProbe");e=mkdir_native(volume,2,n,&base);status(3,e);if(e)return 3;
 str(n,"Child");e=mkdir_native(volume,base,n,&child);status(4,e);if(e)return 4;
 str(n,"Other");e=mkdir_native(volume,base,n,&other);status(5,e);if(e)return 5;
 if(put(base,"Map","BASE")||put(child,"Map","CHLD")||put(other,"OnlyOther","OTHR")||put(base,"Slash/Name","SLSH"))return 6;
 if(setvol(volume,base))return 7;
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"path-files-pb-v1\",\"raw_environment\":{\"volume\":%d,\"base\":%ld,\"child\":%ld,\"other\":%ld,\"original_dir\":%ld},\"files\":{\"cases\":[",volume,base,child,other,original_dir);
 row(0,volume,base,"Map");row(1,0,0,"Map");row(2,volume,base,":Map");row(3,volume,base,":Child:Map");
 row(4,volume,base,"Child:Map");row(5,volume,child,"::Map");row(6,volume,child,"::Other:OnlyOther");row(7,volume,base,":cHiLd:mAp");
 row(8,volume,base,":Child:Missing");row(9,volume,base,":Missing:Map");row(10,volume,base,":Map:Missing");row(11,volume,base,"");
 row(12,volume,base,":");row(13,volume,base,":Child");row(14,volume,base,":Child:");row(15,volume,child,"::");
 row(16,volume,base,":Slash/Name");row(17,volume,base,":Map:");row(18,volume,0,"Map");row(19,volume,999999,"Map");
 memcpy(absolute,volname+1,volname[0]);absolute[volname[0]]=0;strcat(absolute,":RuntimePathProbe:Child:Map");row(20,-123,999999,absolute);
 memcpy(absolute,volname+1,volname[0]);absolute[volname[0]]=0;strcat(absolute,":RuntimePathProbe:Child:Missing");row(21,volume,base,absolute);
 row(22,volume,base,"NoSuchVolume:RuntimePathProbe:Map");row(23,volume,base,":Other::Map");
 row(24,volume,base,":Child::Child:Map");row(25,volume,2,"::Map");row(26,volume,base,":Map::Map");row(27,volume,base,"Missing");
 p+=sprintf(p,"]}}\n");setvol(original_vol,original_dir);
 str(n,"PathPBProbe.json");e=HCreate(volume,2,n,'pPrb','TEXT');if(e)return 8;
 if(HOpen(volume,2,n,fsRdWrPerm,&f))return 9;size=p-output;e=FSWrite(f,&size,output);FSClose(f);FlushVol(0,volume);return e?10:0;
}
