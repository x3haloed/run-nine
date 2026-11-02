#include <Multiverse.h>
#include <stdio.h>
#include <string.h>
static char output[65536],*p=output;
static short status_file=-1;static short volume;static long base,child,other;
static void status(short phase,short error){char b[256];long n;sprintf(b,"{\"phase\":%d,\"error\":%d,\"volume\":%d,\"base\":%ld,\"child\":%ld,\"other\":%ld}\n",phase,error,volume,base,child,other);n=strlen(b);if(status_file>=0){SetFPos(status_file,fsFromStart,0);SetEOF(status_file,0);FSWrite(status_file,&n,b);FlushVol(0,0);}}
static short tag(long id){if(id==0)return 0;if(id==2)return 2;if(id==base)return 3;if(id==child)return 4;if(id==other)return 5;return -1;}
static void str(StringPtr out,const char *s){size_t n=strlen(s);out[0]=n;memcpy(out+1,s,n);}
static short getvol(StringPtr name,short *v,long *dir){WDPBRec pb;OSErr e;memset(&pb,0,sizeof(pb));pb.ioNamePtr=name;e=PBHGetVolSync(&pb);*v=pb.ioWDVRefNum;*dir=pb.ioWDDirID;return e;}
static short setvol(short v,long dir){WDPBRec pb;memset(&pb,0,sizeof(pb));pb.ioVRefNum=v;pb.ioWDDirID=dir;return PBHSetVolSync(&pb);}
static short mkdir_native(short v,long dir,StringPtr name,long *out){HParamBlockRec pb;OSErr e;memset(&pb,0,sizeof(pb));pb.fileParam.ioNamePtr=name;pb.fileParam.ioVRefNum=v;pb.fileParam.ioDirID=dir;e=PBDirCreateSync(&pb);*out=pb.fileParam.ioDirID;return e;}
static short put(long dir,const char *name,const char *data){Str255 n;short f;long size=strlen(data);OSErr e;str(n,name);e=HCreate(volume,dir,n,'pPrb','TEXT');if(e)return e;e=HOpen(volume,dir,n,fsRdWrPerm,&f);if(e)return e;e=FSWrite(f,&size,data);FSClose(f);return e;}
static void row(short index,short hierarchical,short flags,short v,long dir,const char *path){
 struct{unsigned long before;HParamBlockRec pb;unsigned long after;}state;Str255 name;unsigned char snapshot[256],data[4]={0x55,0x55,0x55,0x55};short file=0,read_error=12345,close_error=12345;long length=4;short i;unsigned long result,returned_a0;unsigned short ccr;long stack;
 str(name,path);memcpy(snapshot,name,256);memset(&state,0,sizeof(state));state.before=0x11223344;state.after=0x55667788;state.pb.ioParam.ioCompletion=0;state.pb.ioParam.ioNamePtr=name;state.pb.ioParam.ioVRefNum=v;state.pb.ioParam.ioRefNum=0x1234;state.pb.ioParam.ioPermssn=fsRdPerm;state.pb.ioParam.ioMisc=0;state.pb.fileParam.ioDirID=dir;
 {register HParamBlockRec *a0 __asm__("a0")=&state.pb;register unsigned long d0 __asm__("d0")=0x1111001a;register unsigned short flag __asm__("d4")=flags;register unsigned short out_ccr __asm__("d3");register long delta __asm__("d5");
 if(hierarchical){__asm__ volatile("move.l %%sp,%%d5\n\tmove.w %%d4,%%ccr\n\t.word 0xa260\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+a"(a0),"+d"(d0),"=d"(out_ccr),"=d"(delta):"d"(flag):"d1","d2","a1","cc","memory");}
 else{__asm__ volatile("move.l %%sp,%%d5\n\tmove.w %%d4,%%ccr\n\t.word 0xa060\n\t.word 0x42c3\n\tsub.l %%sp,%%d5":"+a"(a0),"+d"(d0),"=d"(out_ccr),"=d"(delta):"d"(flag):"d1","d2","a1","cc","memory");}
 result=d0;returned_a0=(unsigned long)a0;ccr=out_ccr&31;stack=delta;}
 if(!state.pb.ioParam.ioResult){file=state.pb.ioParam.ioRefNum;read_error=FSRead(file,&length,data);close_error=FSClose(file);}
 p+=sprintf(p,"%s{\"case\":%d,\"hierarchical\":%d,\"input_ccr\":%d,\"d0\":%lu,\"ccr\":%u,\"stack_delta\":%ld,\"a0_preserved\":%d,\"io_result\":%d,\"ref_valid_or_preserved\":%d,\"volume_preserved\":%d,\"dir_preserved\":%d,\"name_preserved\":%d,\"read\":[%d,%ld,%u,%u,%u,%u],\"close_error\":%d,\"guards\":[%lu,%lu]}",index?",":"",index/6,hierarchical,flags,result,ccr,stack,returned_a0==(unsigned long)&state.pb,state.pb.ioParam.ioResult,state.pb.ioParam.ioResult?state.pb.ioParam.ioRefNum==0x1234:state.pb.ioParam.ioRefNum>0,state.pb.ioParam.ioVRefNum==v,state.pb.fileParam.ioDirID==dir,memcmp(name,snapshot,256)==0,read_error,length,data[0],data[1],data[2],data[3],close_error,state.before,state.after);
}
int main(void){Str255 volname,n,original_name;long original_dir;short original_vol;OSErr e;char absolute[300];short scenario,h,k,index=0,flags[3]={16,31,0};long length;short file;
 original_name[0]=0;e=getvol(original_name,&original_vol,&original_dir);if(e)return 2;volume=original_vol;memcpy(volname,original_name,256);
 str(n,"RuntimeOpenDFProbe");e=mkdir_native(volume,2,n,&base);if(e)return 3;str(n,"Child");e=mkdir_native(volume,base,n,&child);if(e)return 4;str(n,"Other");e=mkdir_native(volume,base,n,&other);if(e)return 5;
 if(put(base,"Map","BASE")||put(child,"Map","CHLD")||put(other,"OnlyOther","OTHR"))return 6;if(setvol(volume,base))return 7;
 sprintf(absolute,"%.*s:RuntimeOpenDFProbe:Other:OnlyOther",volname[0],volname+1);
 p+=sprintf(p,"{\"schema\":1,\"probe\":\"open-df-v1\",\"raw_environment\":{\"volume\":%d,\"base\":%ld,\"child\":%ld,\"other\":%ld},\"files\":{\"records\":[",volume,base,child,other);
 for(scenario=0;scenario<12;scenario++)for(h=0;h<2;h++)for(k=0;k<3;k++){
  short v=(scenario==3||scenario==4)?volume:scenario==6?32000:0;long d=(scenario==1||scenario==4)?child:scenario==2?777777:0;const char *name=scenario==5?":Child:Map":scenario==7?"Missing":scenario==8?"Child":scenario==9?"mAp":scenario==10?absolute:scenario==11?"":"Map";
  row(index++,h,flags[k],v,d,name);
 }
 p+=sprintf(p,"]}}\n");setvol(original_vol,original_dir);e=Create("\pOpenDFProbe.json",0,'pPrb','TEXT');if(e)return 8;if(FSOpen("\pOpenDFProbe.json",0,&file))return 9;length=p-output;e=FSWrite(file,&length,output);FSClose(file);FlushVol(0,0);return e?10:0;
}
